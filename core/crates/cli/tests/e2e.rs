//! End-to-end tests of the CLI harness (T08): lifecycle, failure behaviour, the golden vault, and
//! the disk/output leak scans (docs/11 §8, SEC-S01, SEC-S02).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use assert_cmd::Command;
use serde_json::Value;

const PW: &str = "CANARY-master-password-correct horse battery staple";
const PW2: &str = "CANARY-second-master-password-tr0ub4dor&3-xyz";
const PW3: &str = "CANARY-third-master-password-after-recovery-91";
const ITEM_PW: &str = "CANARY-item-password-hunter2-S3CR3T";
const CARD_NO: &str = "CANARY-card-number-4111111111111111";
const NOTE_BODY: &str = "CANARY-note-body-the-launch-codes";
const EXPORT_PW: &str = "CANARY-export-password-ultra-secret-55";
/// No CLI call may hang CI: a command that blocks (for example on a prompt) fails the test.
const CMD_TIMEOUT: Duration = Duration::from_secs(300);
const SNAPSHOT_HEADER: &[u8] = b"SQLite format 3";

/// Every canary that must never appear in plaintext on disk or in output (unless `--reveal`).
const CANARIES: &[&str] = &[
    PW,
    PW2,
    PW3,
    ITEM_PW,
    CARD_NO,
    NOTE_BODY,
    EXPORT_PW,
    "CANARY-item-password",
    "CANARY-master-password",
    "CANARY-card-number",
    "CANARY-note-body",
];

struct Harness {
    dir: tempfile::TempDir,
    /// Everything the CLI printed, for the output scan.
    transcript: Vec<u8>,
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn json(&self) -> Value {
        serde_json::from_str(self.stdout.trim()).unwrap_or_else(|e| {
            panic!("stdout is not JSON ({e}): {}", self.stdout);
        })
    }
}

impl Harness {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
            transcript: Vec::new(),
        }
    }

    fn vault(&self) -> PathBuf {
        self.dir.path().join("vault")
    }

    /// Runs the CLI with `secrets` on stdin (one per line). `reveal` = the invocation passes
    /// `--reveal`, so its output may legitimately contain secrets and is excluded from the scan.
    fn run(&mut self, args: &[&str], secrets: &[&str], reveal: bool) -> Run {
        let mut cmd = Command::cargo_bin("arya-vault").unwrap();
        cmd.timeout(CMD_TIMEOUT)
            .arg("--vault-dir")
            .arg(self.vault())
            .arg("--password-stdin")
            .args(args)
            .write_stdin(secrets.iter().map(|s| format!("{s}\n")).collect::<String>());
        let out = cmd.output().unwrap();
        let run = Run {
            code: out.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        };
        if !reveal {
            self.transcript.extend_from_slice(&out.stdout);
            self.transcript.extend_from_slice(&out.stderr);
        }
        run
    }

    fn ok(&mut self, args: &[&str], secrets: &[&str]) -> Run {
        let r = self.run(args, secrets, false);
        assert_eq!(r.code, 0, "{args:?} failed: {}", r.stderr);
        r
    }
}

fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(all_files(&p));
        } else {
            out.push(p);
        }
    }
    out
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

/// Scans every file under `root` (including `-wal`/`-shm`) and the captured output.
fn assert_no_leaks(root: &Path, transcript: &[u8]) {
    let files = all_files(root);
    assert!(!files.is_empty());
    for f in &files {
        let bytes = fs::read(f).unwrap();
        for c in CANARIES {
            assert!(
                !contains(&bytes, c.as_bytes()),
                "canary `{c}` found in plaintext in {}",
                f.display()
            );
        }
        // UTF-16 renderings too (a Windows-side leak would look like this).
        for c in CANARIES {
            let utf16: Vec<u8> = c.encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert!(
                !contains(&bytes, &utf16),
                "UTF-16 canary in {}",
                f.display()
            );
        }
        assert!(
            !bytes.starts_with(SNAPSHOT_HEADER)
                && !contains(&bytes[..bytes.len().min(64)], SNAPSHOT_HEADER),
            "plaintext SQLite header in {}",
            f.display()
        );
    }
    for c in CANARIES {
        assert!(
            !contains(transcript, c.as_bytes()),
            "canary `{c}` appeared in command output without --reveal"
        );
    }
}

fn item_id(r: &Run) -> String {
    r.json()["id"].as_str().unwrap().to_owned()
}

#[test]
fn full_lifecycle_with_disk_and_output_scan() {
    let mut h = Harness::new();

    // create (the recovery key is shown once, with --reveal)
    let r = h.run(
        &[
            "--json",
            "vault",
            "create",
            "--kdf-profile",
            "low",
            "--reveal",
        ],
        &[PW],
        true,
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let rk = r.json()["recovery_key"].as_str().unwrap().to_owned();
    assert_eq!(
        rk.len(),
        41,
        "32 key chars + 2 checksum chars + 7 hyphens: {rk}"
    );

    // add three item types; secrets are read from stdin after the master password
    let login = item_id(&h.ok(
        &[
            "--json",
            "item",
            "add",
            "--title",
            "CANARY Bank",
            "--field",
            "username=alice",
            "--set-secret",
            "password",
            "--url",
            "https://bank.example.test",
            "--tag",
            "money",
        ],
        &[PW, ITEM_PW],
    ));
    h.ok(
        &[
            "item",
            "add",
            "--type",
            "card",
            "--title",
            "CANARY Visa",
            "--field",
            "holder=Alice",
            "--set-secret",
            "number",
        ],
        &[PW, CARD_NO],
    );
    h.ok(
        &[
            "item",
            "add",
            "--type",
            "note",
            "--title",
            "CANARY Plans",
            "--set-secret",
            "body",
        ],
        &[PW, NOTE_BODY],
    );

    // search finds by title/username/url, and never by a secret
    let r = h.ok(&["--json", "search", "bank"], &[PW]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 1);
    // the login's username and the card's holder are both indexed
    let r = h.ok(&["--json", "search", "alice"], &[PW]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 2);
    let r = h.ok(&["--json", "search", "hunter2"], &[PW]);
    assert!(r.json()["items"].as_array().unwrap().is_empty());
    let r = h.ok(&["--json", "search", "4111111111111111"], &[PW]);
    assert!(
        r.json()["items"].as_array().unwrap().is_empty(),
        "card numbers are not indexed"
    );
    // note bodies are reveal-gated but deliberately searchable (US-04, docs/05); the index lives
    // inside the encrypted database, which the disk scan below checks
    let r = h.ok(&["--json", "search", "launch"], &[PW]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 1);

    // get: masked by default, revealed only with --reveal
    let r = h.ok(&["item", "get", &login[login.len() - 8..]], &[PW]);
    assert!(r.stdout.contains("password: (set; use --reveal to print)"));
    let r = h.run(&["--json", "item", "get", &login, "--reveal"], &[PW], true);
    assert_eq!(r.code, 0);
    assert_eq!(r.json()["secrets"]["password"], ITEM_PW);

    // edit, trash, restore
    h.ok(
        &[
            "item",
            "edit",
            &login,
            "--title",
            "CANARY Bank 2",
            "--add-tag",
            "x",
        ],
        &[PW],
    );
    h.ok(&["item", "delete", &login], &[PW]);
    let r = h.ok(&["--json", "item", "list"], &[PW]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 2);
    let r = h.ok(&["--json", "item", "list", "--trash"], &[PW]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 1);
    h.ok(&["item", "restore", &login], &[PW]);

    // unlock-check, wrong password
    h.ok(&["unlock-check"], &[PW]);
    let r = h.run(
        &["unlock-check"],
        &["CANARY-not-the-password-123456"],
        false,
    );
    assert_eq!(r.code, 3, "wrong password exits 3: {}", r.stderr);

    // change password WITHOUT the recovery key (SEC-C12, SEC-A05); old one stops working
    h.ok(&["password", "change", "--kdf-profile", "low"], &[PW, PW2]);
    assert_eq!(h.run(&["unlock-check"], &[PW], false).code, 3);
    h.ok(&["unlock-check"], &[PW2]);
    let r = h.ok(&["--json", "item", "list"], &[PW2]);
    assert_eq!(
        r.json()["items"].as_array().unwrap().len(),
        3,
        "data survives a password change"
    );

    // the recovery key still works after a password change
    let r = h.run(&["recover", "--kdf-profile", "low"], &[&rk, PW3], true);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(h.run(&["unlock-check"], &[PW2], false).code, 3);
    h.ok(&["unlock-check"], &[PW3]);

    // a wrong / mistyped recovery key
    let r = h.run(
        &["recover", "--kdf-profile", "low"],
        &["MH365-7P3RV-640RY-YD7T8-RN1R5-65JCX-SA-65", PW2],
        true,
    );
    assert_eq!(r.code, 3, "{}", r.stderr);
    let r = h.run(
        &["recover", "--kdf-profile", "low"],
        &["not a recovery key", PW2],
        true,
    );
    assert_eq!(r.code, 2);

    // rotate the recovery key: the old one stops working, the new one works
    let r = h.run(
        &[
            "--json",
            "rotate-recovery-key",
            "--reveal",
            "--kdf-profile",
            "low",
        ],
        &[PW3],
        true,
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let rk2 = r.json()["recovery_key"].as_str().unwrap().to_owned();
    assert_ne!(rk, rk2);
    assert_eq!(
        h.run(&["recover", "--kdf-profile", "low"], &[&rk, PW2], true)
            .code,
        3
    );
    assert_eq!(
        h.run(&["recover", "--kdf-profile", "low"], &[&rk2, PW2], true)
            .code,
        0
    );
    h.ok(&["unlock-check"], &[PW2]);

    // export (aryavault) -> import into a fresh vault -> same items
    let export = h.dir.path().join("export.avex");
    let export_s = export.to_str().unwrap().to_owned();
    h.ok(
        &[
            "export",
            "--format",
            "aryavault",
            "--out",
            &export_s,
            "--kdf-profile",
            "low",
        ],
        &[PW2, EXPORT_PW],
    );
    assert!(
        !contains(&fs::read(&export).unwrap(), b"CANARY"),
        "export is encrypted"
    );
    // refuses to overwrite
    assert_ne!(
        h.run(
            &["export", "--out", &export_s, "--kdf-profile", "low"],
            &[PW2, EXPORT_PW],
            false
        )
        .code,
        0
    );

    let mut h2 = Harness::new();
    let r = h2.run(
        &["vault", "create", "--kdf-profile", "low", "--reveal"],
        &[PW3],
        true,
    );
    assert_eq!(r.code, 0);
    let wrong = h2.run(
        &["import", "--format", "aryavault", "--file", &export_s],
        &[PW3, "CANARY-wrong-export-password"],
        false,
    );
    assert_eq!(wrong.code, 3, "{}", wrong.stderr);
    let r = h2.ok(
        &[
            "--json",
            "import",
            "--format",
            "aryavault",
            "--file",
            &export_s,
        ],
        &[PW3, EXPORT_PW],
    );
    assert_eq!(r.json()["created"], 3);
    let again = h2.ok(
        &[
            "--json",
            "import",
            "--format",
            "aryavault",
            "--file",
            &export_s,
        ],
        &[PW3, EXPORT_PW],
    );
    assert_eq!(
        again.json()["duplicates"],
        1,
        "the login is skipped as a duplicate"
    );
    let r = h2.ok(&["--json", "search", "bank"], &[PW3]);
    assert_eq!(r.json()["items"].as_array().unwrap().len(), 1);
    let id2 = r.json()["items"][0]["id"].as_str().unwrap().to_owned();
    let r = h2.run(&["--json", "item", "get", &id2, "--reveal"], &[PW3], true);
    assert_eq!(r.json()["secrets"]["password"], ITEM_PW);

    // plaintext CSV needs an explicit acknowledgement
    let csv = h.dir.path().join("out.csv");
    let csv_s = csv.to_str().unwrap().to_owned();
    let r = h.run(
        &["export", "--format", "csv", "--out", &csv_s],
        &[PW2],
        false,
    );
    assert_eq!(r.code, 2);
    assert!(!csv.exists());

    // disk scan + output scan
    assert_no_leaks(h.dir.path(), &h.transcript);
    assert_no_leaks(h2.dir.path(), &h2.transcript);
    let db = fs::read(h.vault().join("vault.db")).unwrap();
    assert!(!db.starts_with(SNAPSHOT_HEADER));
}

#[test]
fn csv_import_and_plaintext_export() {
    let mut h = Harness::new();
    assert_eq!(
        h.run(
            &["vault", "create", "--kdf-profile", "low", "--reveal"],
            &[PW],
            true
        )
        .code,
        0
    );
    let src = h.dir.path().join("in.csv");
    fs::write(
        &src,
        "name,url,username,password\nCANARY Site,https://a.example.test,bob,CANARY-item-password-csv\n=evil,https://b.example.test,eve,CANARY-item-password-two\n",
    )
    .unwrap();
    let r = h.ok(
        &[
            "--json",
            "import",
            "--format",
            "csv",
            "--file",
            src.to_str().unwrap(),
            "--dry-run",
        ],
        &[PW],
    );
    assert_eq!(r.json()["dry_run"], true);
    assert_eq!(r.json()["created"], 2);
    assert!(
        h.ok(&["--json", "item", "list"], &[PW]).json()["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let r = h.ok(
        &[
            "--json",
            "import",
            "--format",
            "csv",
            "--file",
            src.to_str().unwrap(),
        ],
        &[PW],
    );
    assert_eq!(r.json()["created"], 2);

    let out = h.dir.path().join("out.csv");
    let r = h.run(
        &[
            "--json",
            "export",
            "--format",
            "csv",
            "--out",
            out.to_str().unwrap(),
            "--acknowledge-plaintext-risk",
        ],
        &[PW],
        true,
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let text = fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("CANARY-item-password-csv"),
        "plaintext CSV carries the password"
    );
    assert!(
        text.contains("'=evil"),
        "formula injection is neutralised on export"
    );
}

#[test]
fn wrong_password_corrupted_and_missing_files() {
    let mut h = Harness::new();
    assert_eq!(
        h.run(
            &["vault", "create", "--kdf-profile", "low", "--reveal"],
            &[PW],
            true
        )
        .code,
        0
    );

    // wrong password: exit 3, no secret in the messages
    let r = h.run(&["item", "list"], &["CANARY-wrong-password-1234"], false);
    assert_eq!(r.code, 3);
    assert!(!r.stderr.contains("CANARY"));

    // a flipped bit in the header: authentication fails or the header is rejected, never a crash
    let hdr = all_files(&h.vault())
        .into_iter()
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("header-")
        })
        .unwrap();
    let original = fs::read(&hdr).unwrap();
    for i in [0usize, 5, original.len() / 2, original.len() - 1] {
        let mut bad = original.clone();
        bad[i] ^= 0x01;
        fs::write(&hdr, &bad).unwrap();
        let r = h.run(&["unlock-check"], &[PW], false);
        assert!(
            r.code == 3 || r.code == 4,
            "byte {i}: exit {} ({})",
            r.code,
            r.stderr
        );
    }
    fs::write(&hdr, &original[..original.len() / 2]).unwrap();
    assert_eq!(
        h.run(&["unlock-check"], &[PW], false).code,
        4,
        "truncated header"
    );
    fs::write(&hdr, &original).unwrap();
    assert_eq!(h.run(&["unlock-check"], &[PW], false).code, 0);

    // a corrupted database file is an error, not a panic. (SQLCipher authenticates each page
    // when it is read, so damage is guaranteed to surface for page 1, which every open reads.)
    let db = h.vault().join("vault.db");
    let mut bytes = fs::read(&db).unwrap();
    for b in &mut bytes[100..164] {
        *b ^= 0xFF;
    }
    fs::write(&db, &bytes).unwrap();
    let r = h.run(&["item", "list"], &[PW], false);
    assert_ne!(r.code, 0);
    assert!(
        !r.stderr.contains("panick") && !r.stderr.contains("unexpected condition"),
        "{}",
        r.stderr
    );
    // garbage instead of a database
    fs::write(&db, b"SQLite format 3\0 this is not an encrypted database").unwrap();
    assert_ne!(h.run(&["item", "list"], &[PW], false).code, 0);

    // no vault at all
    let mut empty = Harness::new();
    fs::create_dir_all(empty.vault()).unwrap();
    assert_eq!(empty.run(&["unlock-check"], &[PW], false).code, 4);
}

#[test]
fn secrets_are_never_accepted_from_arguments_or_environment() {
    let mut h = Harness::new();
    assert_eq!(
        h.run(
            &["vault", "create", "--kdf-profile", "low", "--reveal"],
            &[PW],
            true
        )
        .code,
        0
    );

    // there is no --password / --recovery-key option, and the error does not echo the value
    for flag in [
        "--password",
        "--master-password",
        "--recovery-key",
        "--secret",
    ] {
        let out = Command::cargo_bin("arya-vault")
            .unwrap()
            .timeout(CMD_TIMEOUT)
            .args([
                "--vault-dir",
                h.vault().to_str().unwrap(),
                flag,
                "CANARY-argv-secret-5150",
                "unlock-check",
            ])
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{flag}");
        let all = [out.stdout, out.stderr].concat();
        assert!(
            !contains(&all, b"CANARY-argv-secret-5150"),
            "{flag} echoed its value"
        );
    }
    // an environment variable is not a way to supply the password: with no TTY and no stdin
    // flag the command fails instead of consuming it
    let out = Command::cargo_bin("arya-vault")
        .unwrap()
        .timeout(CMD_TIMEOUT)
        .env("ARYAVAULT_PASSWORD", PW)
        .env("PASSWORD", PW)
        .args(["--vault-dir", h.vault().to_str().unwrap(), "unlock-check"])
        .write_stdin("")
        .output()
        .unwrap();
    assert_ne!(out.status.code(), Some(0));
    // stdin without --password-stdin is not read either
    let out = Command::cargo_bin("arya-vault")
        .unwrap()
        .timeout(CMD_TIMEOUT)
        .args(["--vault-dir", h.vault().to_str().unwrap(), "unlock-check"])
        .write_stdin(format!("{PW}\n"))
        .output()
        .unwrap();
    assert_ne!(out.status.code(), Some(0));
}

#[test]
fn secrets_print_only_with_reveal() {
    let mut h = Harness::new();
    // creating without --reveal would lose the recovery key: refused up front
    let r = h.run(&["vault", "create", "--kdf-profile", "low"], &[PW], false);
    assert_eq!(r.code, 2);
    assert!(!h.vault().exists() || fs::read_dir(h.vault()).unwrap().next().is_none());
    assert_eq!(
        h.run(
            &["vault", "create", "--kdf-profile", "low", "--reveal"],
            &[PW],
            true
        )
        .code,
        0
    );
    assert_eq!(h.run(&["rotate-recovery-key"], &[PW], false).code, 2);
    assert_eq!(h.run(&["gen", "password"], &[], false).code, 2);
    assert_eq!(h.run(&["gen", "passphrase"], &[], false).code, 2);
    let r = h.run(
        &["--json", "gen", "password", "--length", "24", "--reveal"],
        &[],
        true,
    );
    assert_eq!(r.code, 0);
    assert_eq!(r.json()["value"].as_str().unwrap().chars().count(), 24);
    let r = h.run(
        &["--json", "gen", "passphrase", "--words", "5", "--reveal"],
        &[],
        true,
    );
    assert_eq!(r.code, 0);
    assert!(r.json()["entropy_bits"].as_f64().unwrap() > 60.0);

    // a weak master password is refused (SEC-A07)
    let mut w = Harness::new();
    let r = w.run(
        &["vault", "create", "--kdf-profile", "low", "--reveal"],
        &["short"],
        false,
    );
    assert_eq!(r.code, 2);
}

#[test]
fn golden_vault_v1_opens_with_the_documented_password() {
    // SEC-C08: the header written by the first release must open forever.
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/golden/v1");
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(golden.join("expected.json")).unwrap()).unwrap();
    let password = expected["password"].as_str().unwrap();
    let recovery = expected["recovery_key"].as_str().unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let copy = tmp.path().join("golden");
    fs::create_dir_all(&copy).unwrap();
    let header = expected["files"]["header"]["path"].as_str().unwrap();
    fs::copy(golden.join(header), copy.join(header)).unwrap();

    let run = |args: &[&str], secrets: &[&str]| {
        let mut cmd = Command::cargo_bin("arya-vault").unwrap();
        cmd.timeout(CMD_TIMEOUT)
            .arg("--vault-dir")
            .arg(&copy)
            .arg("--password-stdin")
            .args(args)
            .write_stdin(secrets.iter().map(|s| format!("{s}\n")).collect::<String>());
        cmd.output().unwrap()
    };
    let out = run(&["--json", "unlock-check"], &[password]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(
        v["database_opened"], false,
        "the golden vault is header-only"
    );
    assert_eq!(
        run(&["unlock-check"], &["CANARY-wrong-golden-password"])
            .status
            .code(),
        Some(3)
    );

    let out = run(&["--json", "info", "--no-unlock"], &[]);
    assert_eq!(out.status.code(), Some(0));
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["header"]["vault_id"], expected["vault_id"]);
    assert_eq!(v["header"]["epoch"], expected["epoch"]);
    assert_eq!(v["header"]["kdf"]["m_kib"], expected["kdf"]["m_kib"]);

    // the golden recovery key resets the password; the golden header is copied, never modified
    let out = run(&["recover", "--kdf-profile", "low"], &[recovery, PW]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(run(&["unlock-check"], &[PW]).status.code(), Some(0));
    assert_eq!(run(&["unlock-check"], &[password]).status.code(), Some(3));
    assert!(golden.join(header).exists());
}

#[test]
fn info_reports_formats_and_pinned_sqlcipher_settings() {
    let mut h = Harness::new();
    assert_eq!(
        h.run(
            &["vault", "create", "--kdf-profile", "low", "--reveal"],
            &[PW],
            true
        )
        .code,
        0
    );
    let r = h.ok(&["--json", "info"], &[PW]);
    let v = r.json();
    assert_eq!(v["format_version"], 1);
    assert_eq!(v["header"]["header_version"], 1);
    assert_eq!(v["database"]["pinned_settings"]["cipher.page_size"], "4096");
    assert_eq!(
        v["database"]["pinned_settings"]["cipher.plaintext_header_size"],
        "0"
    );
    assert_eq!(
        v["database"]["schema_version"],
        v["database"]["latest_schema_version"]
    );
    // the header version in the database follows password changes
    h.ok(&["password", "change", "--kdf-profile", "low"], &[PW, PW2]);
    let v = h.ok(&["--json", "info"], &[PW2]).json();
    assert_eq!(v["header"]["header_version"], 2);
}

#[test]
fn bench_commands_run() {
    let mut h = Harness::new();
    let r = h.ok(
        &[
            "--json",
            "bench",
            "kdf",
            "--target-ms",
            "50",
            "--max-m-mib",
            "64",
            "--runs",
            "1",
        ],
        &[],
    );
    assert_eq!(r.json()["params"]["m_kib"], 65536);
    let r = h.ok(
        &[
            "--json",
            "bench",
            "search",
            "--items",
            "150",
            "--queries",
            "10",
            "--kdf-profile",
            "low",
        ],
        &[],
    );
    let v = r.json();
    assert_eq!(v["items"], 150);
    assert!(v["search_ms"]["hits"].as_u64().unwrap() > 0);
}
