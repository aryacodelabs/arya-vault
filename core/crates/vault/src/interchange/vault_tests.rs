//! Vault-level tests of commit, export and the round trip, plus the committed fixtures.

use std::collections::BTreeMap;
use std::path::PathBuf;

use arya_vault_crypto::kdf::KdfParams;
use arya_vault_storage::{CreateParams, Db, DbKey, Id, ItemFilter, Store};
use zeroize::Zeroizing;

use super::aryavault::{self, ExportFolder, ExportItem, build_payload, tests::CountingRng};
use super::*;
use crate::model::{CustomKind, ItemType, StdField};
use crate::{
    FieldRef, ListFilter, ManualClock, NewItem, Page, SearchQuery, Vault, VaultError, fault, query,
};

const T0: u64 = 1_700_000_000_000;
const PW: &str = "CANARY-export-password-v1";

struct Env {
    _dir: tempfile::TempDir,
    clock: ManualClock,
    v: Vault,
}

fn env_with(device: u8) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let params = CreateParams {
        vault_id: [0xA1; 16],
        device_id: [device; 16],
        epoch: 1,
        header_version: 1,
    };
    let db = Db::create(&path, DbKey::from_bytes([7; 32]), &params).unwrap();
    let clock = ManualClock::new(T0);
    let v = Vault::open(db, Box::new(clock.clone())).unwrap();
    Env {
        _dir: dir,
        clock,
        v,
    }
}

fn env() -> Env {
    env_with(0xB2)
}

fn lim() -> ImportLimits {
    ImportLimits::default()
}

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/import")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn titles(v: &mut Vault) -> Vec<String> {
    let mut t: Vec<String> = v
        .list(&ListFilter::default(), Page::ALL)
        .unwrap()
        .into_iter()
        .map(|s| s.title)
        .collect();
    t.sort();
    t
}

fn search(v: &mut Vault, text: &str) -> Vec<String> {
    v.search(&SearchQuery {
        text: text.into(),
        ..Default::default()
    })
    .unwrap()
    .into_iter()
    .map(|s| s.title)
    .collect()
}

/// A full, comparable picture of one item (ids and timestamps excluded by policy).
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Snap {
    item_type: &'static str,
    title: String,
    fields: BTreeMap<&'static str, String>,
    urls: Vec<String>,
    tags: Vec<String>,
    custom: Vec<(String, &'static str, String)>,
    folder: Option<String>,
    favorite: bool,
    history: BTreeMap<&'static str, Vec<String>>, // oldest first, excluding current
}

fn snapshot(v: &mut Vault) -> Vec<Snap> {
    let folders: BTreeMap<Id, String> = v
        .list_folders()
        .unwrap()
        .into_iter()
        .map(|f| (f.id, f.name))
        .collect();
    let mut out = Vec::new();
    for s in v.list(&ListFilter::default(), Page::ALL).unwrap() {
        let view = v.get_item(&s.id).unwrap();
        let mut fields: BTreeMap<&'static str, String> = BTreeMap::new();
        let mut history = BTreeMap::new();
        for (f, val) in &view.fields {
            if *f != StdField::Title {
                fields.insert(f.key(), val.clone());
            }
        }
        for f in &view.secret_fields {
            fields.insert(f.key(), v.reveal(&s.id, *f).unwrap().unwrap().to_string());
        }
        for f in fields.keys().copied().collect::<Vec<_>>() {
            let sf = StdField::from_key(f).unwrap();
            let versions = v.versions(&s.id, &FieldRef::Std(sf)).unwrap();
            let older: Vec<String> = versions
                .iter()
                .filter(|x| !x.current)
                .rev()
                .filter_map(|x| {
                    v.reveal_version(&s.id, &FieldRef::Std(sf), x.hlc, &x.device_id)
                        .unwrap()
                })
                .map(|z| z.to_string())
                .collect();
            if !older.is_empty() {
                history.insert(f, older);
            }
        }
        let mut custom: Vec<(String, &'static str, String)> = view
            .custom
            .iter()
            .map(|c| {
                let val = c.value.clone().unwrap_or_else(|| {
                    v.reveal_custom(&s.id, &c.id)
                        .unwrap()
                        .map(|z| z.to_string())
                        .unwrap_or_default()
                });
                (c.label.clone(), c.kind.as_str(), val)
            })
            .collect();
        custom.sort();
        out.push(Snap {
            item_type: s.item_type.as_str(),
            title: s.title.clone(),
            fields,
            urls: view.urls.iter().map(|u| u.url.clone()).collect(),
            tags: view.tags.clone(),
            custom,
            folder: s.folder_id.and_then(|f| folders.get(&f).cloned()),
            favorite: s.favorite,
            history,
        });
    }
    out.sort();
    out
}

/// Everything a failed import must leave untouched.
#[derive(Debug, PartialEq)]
struct Dump {
    items: Vec<arya_vault_storage::ItemRow>,
    fields: Vec<arya_vault_storage::FieldRow>,
    history: Vec<arya_vault_storage::FieldRow>,
    folders: Vec<arya_vault_storage::FolderRow>,
    ops: Vec<arya_vault_storage::LocalOp>,
    fts: Vec<(String, Vec<Id>)>,
    hlc_state: Option<Vec<u8>>,
}

fn dump(v: &mut Vault, probes: &[&str]) -> Dump {
    v.db.with_read(|tx| -> crate::Result<Dump> {
        let items = tx.list_items(ItemFilter {
            include_deleted: true,
            ..Default::default()
        })?;
        let (mut fields, mut history) = (Vec::new(), Vec::new());
        for i in &items {
            for f in tx.fields_for_item(&i.id)? {
                history.extend(tx.history_for(&i.id, &f.key)?);
                fields.push(f);
            }
        }
        let mut fts = Vec::new();
        for p in probes {
            let mut ids = tx.fts_search(&query::fts_query(p).unwrap(), 1000)?;
            ids.sort();
            fts.push(((*p).to_owned(), ids));
        }
        Ok(Dump {
            items,
            fields,
            history,
            folders: tx.list_folders()?,
            ops: tx.pending_local_ops(100_000)?,
            fts,
            hlc_state: tx.meta_get("hlc_state")?,
        })
    })
    .unwrap()
}

fn floor() -> KdfParams {
    KdfParams::floor([0; 16])
}

// --------------------------------------------------------------------- CSV and Bitwarden

#[test]
fn csv_fixtures_commit_into_a_vault() {
    let mut e = env();
    for (name, n, source) in [
        ("chrome.csv", 2, ImportSource::ChromeCsv),
        ("firefox.csv", 1, ImportSource::FirefoxCsv),
        ("safari.csv", 1, ImportSource::SafariCsv),
        ("generic.csv", 1, ImportSource::GenericCsv),
        ("utf8-bom.csv", 1, ImportSource::ChromeCsv),
        ("utf16le-bom.csv", 1, ImportSource::ChromeCsv),
    ] {
        let b = parse_csv(&fixture(name), &lim()).unwrap();
        assert_eq!((b.source, b.items.len()), (source, n), "{name}");
        let r = e.v.commit_import(&b, &ImportOptions::default()).unwrap();
        assert_eq!(r.created, n, "{name}");
    }
    let t = titles(&mut e.v);
    assert!(
        t.contains(&"Example".to_owned())
            && t.contains(&"accounts.example.test".to_owned())
            && t.contains(&"Café".to_owned())
            && t.contains(&"Émile".to_owned())
    );
    // Imported secrets are retrievable only via reveal, and the right values arrived.
    let id =
        e.v.search(&SearchQuery {
            text: "Shop".into(),
            ..Default::default()
        })
        .unwrap()[0]
            .id;
    assert_eq!(
        e.v.reveal(&id, StdField::Password)
            .unwrap()
            .unwrap()
            .as_str(),
        "CANARY-safari-pw-1"
    );
    assert!(
        e.v.reveal(&id, StdField::TotpSeed)
            .unwrap()
            .unwrap()
            .starts_with("otpauth://")
    );
    // Folder and tags from the generic CSV.
    assert_eq!(
        e.v.list_folders()
            .unwrap()
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["Work"]
    );
    let generic =
        e.v.search(&SearchQuery {
            text: "generic".into(),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(generic.len(), 1);
    assert!(generic[0].favorite);
}

#[test]
fn bitwarden_fixture_commits_and_secrets_stay_out_of_the_index() {
    let mut e = env();
    let b = parse_bitwarden_json(&fixture("bitwarden.json"), &lim()).unwrap();
    assert_eq!((b.items.len(), b.skipped.len()), (4, 2));
    let r = e.v.commit_import(&b, &ImportOptions::default()).unwrap();
    assert_eq!((r.created, r.folders_created), (4, 1));
    assert_eq!(
        titles(&mut e.v),
        ["Card", "Example Login", "Identity", "Secure Note"]
    );
    let login =
        e.v.search(&SearchQuery {
            text: "example".into(),
            ..Default::default()
        })
        .unwrap()[0]
            .id;
    assert_eq!(
        e.v.reveal(&login, StdField::Password)
            .unwrap()
            .unwrap()
            .as_str(),
        "CANARY-bw-pw"
    );
    let view = e.v.get_item(&login).unwrap();
    assert_eq!(view.custom.len(), 2);
    assert!(view.summary.favorite && view.summary.folder_id.is_some());
    // Notes are searchable (US-04), secrets are not.
    let mut hits = search(&mut e.v, "CANARY");
    hits.sort();
    assert_eq!(hits, ["Example Login", "Secure Note"]);
    for secret in [
        "CANARY-bw-pw",
        "CANARYBWSEED",
        "4111111111111111",
        "CANARY-ssn-000",
        "CANARY-bw-pin",
    ] {
        assert!(
            search(&mut e.v, secret).is_empty(),
            "{secret} must not be indexed"
        );
    }
}

#[test]
fn dry_run_changes_nothing_and_predicts_the_commit() {
    let mut e = env();
    let b = parse_bitwarden_json(&fixture("bitwarden.json"), &lim()).unwrap();
    let before = dump(&mut e.v, &["login", "memo"]);
    let preview =
        e.v.commit_import(
            &b,
            &ImportOptions {
                dry_run: true,
                ..Default::default()
            },
        )
        .unwrap();
    assert!(preview.dry_run && preview.created_ids.is_empty());
    assert_eq!((preview.created, preview.folders_created), (4, 1));
    assert_eq!(
        dump(&mut e.v, &["login", "memo"]),
        before,
        "preview must not write anything"
    );
    let real = e.v.commit_import(&b, &ImportOptions::default()).unwrap();
    assert_eq!(
        (real.created, real.folders_created, real.duplicates.clone()),
        (preview.created, preview.folders_created, preview.duplicates)
    );
    assert_eq!(real.created_ids.len(), 4);
}

#[test]
fn duplicates_are_detected_against_the_vault_and_within_the_bundle() {
    let mut e = env();
    let first = parse_csv(b"name,url,username,password\nSite,https://s.example,alice,CANARY-1\nSite,https://s.example,alice,CANARY-2\nSite,https://other.example,alice,CANARY-3\n", &lim()).unwrap();
    let r =
        e.v.commit_import(&first, &ImportOptions::default())
            .unwrap();
    assert_eq!(
        (r.created, r.duplicates.clone()),
        (2, vec![1]),
        "repeat inside the bundle"
    );
    // Case and whitespace do not matter; re-importing is a no-op.
    let again = parse_csv(
        b"name,url,username,password\n SITE ,HTTPS://S.EXAMPLE/,ALICE,CANARY-9\n",
        &lim(),
    )
    .unwrap();
    let r =
        e.v.commit_import(&again, &ImportOptions::default())
            .unwrap();
    assert_eq!((r.created, r.duplicates), (0, vec![0]));
    assert_eq!(e.v.item_count().unwrap(), 2);
    // Opting out imports them anyway; notes are never deduplicated.
    let r =
        e.v.commit_import(
            &again,
            &ImportOptions {
                skip_duplicates: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(r.created, 1);
    let notes = parse_csv(b"name,type,note\nMemo,note,a\nMemo,note,b\n", &lim()).unwrap();
    assert_eq!(
        e.v.commit_import(&notes, &ImportOptions::default())
            .unwrap()
            .created,
        2
    );
}

#[test]
fn folders_are_reused_and_target_folder_applies() {
    let mut e = env();
    let existing = e.v.create_folder("Work", None).unwrap();
    let target = e.v.create_folder("Inbox", None).unwrap();
    let b = parse_csv(b"name,username,Folder\na,u,Work\nb,u,\n", &lim()).unwrap();
    let r =
        e.v.commit_import(
            &b,
            &ImportOptions {
                target_folder: Some(target),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!((r.created, r.folders_created), (2, 0));
    let list = e.v.list(&ListFilter::default(), Page::ALL).unwrap();
    let folder_of = |t: &str| list.iter().find(|s| s.title == t).unwrap().folder_id;
    assert_eq!(folder_of("a"), Some(existing));
    assert_eq!(folder_of("b"), Some(target));
    let missing = ImportOptions {
        target_folder: Some([0xEE; 16]),
        ..Default::default()
    };
    assert!(matches!(
        e.v.commit_import(&b, &missing),
        Err(ImportError::Vault(VaultError::NotFound))
    ));
}

#[test]
fn nested_folders_from_an_encrypted_export_are_recreated() {
    let mut src = env();
    let parent = src.v.create_folder("Parent", None).unwrap();
    let child = src.v.create_folder("Child", Some(parent)).unwrap();
    let id = src
        .v
        .create_item(NewItem {
            folder_id: Some(child),
            ..NewItem::new(ItemType::Login, "Nested")
        })
        .unwrap();
    let _ = id;
    let file = src.v.export_aryavault(PW, &floor(), false).unwrap();
    let mut dst = env_with(0xC3);
    let b = parse_aryavault(&file, PW, &lim()).unwrap();
    dst.v.commit_import(&b, &ImportOptions::default()).unwrap();
    let folders = dst.v.list_folders().unwrap();
    let c = folders.iter().find(|f| f.name == "Child").unwrap();
    let p = folders.iter().find(|f| f.name == "Parent").unwrap();
    assert_eq!(c.parent_id, Some(p.id));
}

// ------------------------------------------------------------------- atomicity (SEC-Y05)

#[test]
fn commit_is_all_or_nothing_at_every_write_point() {
    let mut e = env();
    // Pre-existing data so "unchanged" is a meaningful comparison.
    e.v.create_item(NewItem::new(ItemType::Login, "Existing Login"))
        .unwrap();
    let mut b = parse_bitwarden_json(&fixture("bitwarden.json"), &lim()).unwrap();
    // Add history so the replay path is covered as well.
    b.items[0].history.push(ImportHistory {
        field: StdField::Password,
        older: vec![
            Zeroizing::new("CANARY-old-1".into()),
            Zeroizing::new("CANARY-old-2".into()),
        ],
    });
    let probes = ["existing", "example", "memo", "card", "identity"];
    let before = dump(&mut e.v, &probes);
    let mut points = 0;
    for n in 0.. {
        fault::arm(n);
        let r = e.v.commit_import(&b, &ImportOptions::default());
        let passed = fault::disarm();
        match r {
            Err(ImportError::Vault(VaultError::Injected)) => {
                assert_eq!(
                    dump(&mut e.v, &probes),
                    before,
                    "failure at write point {n} left partial state"
                );
            }
            Ok(rep) => {
                points = n;
                assert!(passed <= n, "point {n} never reached");
                assert_eq!(rep.created, 4);
                break;
            }
            Err(other) => panic!("unexpected error at point {n}: {other:?}"),
        }
        assert!(n < 2000, "runaway");
    }
    assert!(points > 20, "expected many write points, saw {points}");
    assert_ne!(dump(&mut e.v, &probes), before);
}

#[test]
fn structural_errors_never_touch_the_vault() {
    let mut e = env();
    e.v.create_item(NewItem::new(ItemType::Login, "Keep"))
        .unwrap();
    let before = dump(&mut e.v, &["keep"]);
    for (name, f) in [
        ("malformed-bad-utf8.csv", 0),
        ("malformed-no-columns.csv", 0),
        ("malformed-bitwarden-truncated.json", 1),
        ("malformed-bitwarden-wrong-shape.json", 1),
        ("bitwarden-encrypted.json", 1),
    ] {
        let bytes = fixture(name);
        let r = if f == 0 {
            parse_csv(&bytes, &lim()).map(|_| ())
        } else {
            parse_bitwarden_json(&bytes, &lim()).map(|_| ())
        };
        assert!(r.is_err(), "{name} must be rejected");
        // Rendering the error never leaks file content.
        let err = r.unwrap_err();
        let msg = format!("{err:?} {err}");
        assert!(!msg.contains("CANARY"), "{name}: {msg}");
    }
    assert_eq!(dump(&mut e.v, &["keep"]), before);
}

#[test]
fn invalid_items_in_a_hand_built_bundle_are_skipped_and_reported() {
    let mut e = env();
    let mut b = ImportBundle::new(ImportSource::GenericCsv);
    b.items.push(ImportItem::new(ItemType::Login, "ok", 1));
    let mut bad = ImportItem::new(ItemType::Note, "bad", 2);
    bad.fields
        .push((StdField::Password, Zeroizing::new("x".into()))); // not a note field
    b.items.push(bad);
    let mut dangling = ImportItem::new(ItemType::Login, "dangling folder", 3);
    dangling.folder = Some(7);
    b.items.push(dangling);
    let r = e.v.commit_import(&b, &ImportOptions::default()).unwrap();
    assert_eq!((r.created, r.invalid), (1, vec![1, 2]));
}

// ------------------------------------------------------------------- encrypted export

fn populated(e: &mut Env) {
    let v = &mut e.v;
    let work = v.create_folder("Work", None).unwrap();
    let login = v
        .create_item(NewItem {
            urls: vec!["https://a.example".into(), "https://b.example".into()],
            tags: vec!["t1".into(), "t2".into()],
            folder_id: Some(work),
            favorite: true,
            ..NewItem::new(ItemType::Login, "Main Login")
                .with_field(StdField::Username, "alice")
                .with_field(StdField::Password, "CANARY-pw-1")
                .with_field(StdField::TotpSeed, "CANARYSEED")
                .with_field(StdField::Notes, "CANARY notes")
        })
        .unwrap();
    v.add_custom_field(&login, CustomKind::Hidden, "pin", "CANARY-9999")
        .unwrap();
    v.add_custom_field(&login, CustomKind::Text, "region", "eu")
        .unwrap();
    for (i, pw) in ["CANARY-pw-2", "CANARY-pw-3"].into_iter().enumerate() {
        e.clock.advance(1000 * (i as u64 + 1));
        e.v.set_field(&login, StdField::Password, pw).unwrap();
    }
    let v = &mut e.v;
    v.create_item(NewItem::new(ItemType::Note, "Memo").with_field(StdField::Body, "# CANARY body"))
        .unwrap();
    v.create_item(
        NewItem::new(ItemType::Card, "Visa")
            .with_field(StdField::Number, "4111CANARY")
            .with_field(StdField::Cvv, "123")
            .with_field(StdField::Holder, "A B"),
    )
    .unwrap();
    v.create_item(
        NewItem::new(ItemType::Identity, "Me")
            .with_field(StdField::FirstName, "A")
            .with_field(StdField::Ids, "CANARY-ID"),
    )
    .unwrap();
}

#[test]
fn round_trip_through_the_encrypted_export_preserves_everything() {
    for include_history in [false, true] {
        let mut src = env();
        populated(&mut src);
        let want = snapshot(&mut src.v);
        let file = src
            .v
            .export_aryavault(PW, &floor(), include_history)
            .unwrap();
        let mut dst = env_with(0xC3);
        let bundle = parse_aryavault(&file, PW, &lim()).unwrap();
        assert!(bundle.skipped.is_empty() && bundle.warnings.is_empty());
        let r = dst
            .v
            .commit_import(&bundle, &ImportOptions::default())
            .unwrap();
        assert_eq!(r.created, 4);
        let mut got = snapshot(&mut dst.v);
        let mut want = want;
        if !include_history {
            want.iter_mut().for_each(|s| s.history.clear());
        }
        got.sort();
        want.sort();
        assert_eq!(got, want, "include_history = {include_history}");
        if include_history {
            let main = got.iter().find(|s| s.title == "Main Login").unwrap();
            assert_eq!(main.history["password"], ["CANARY-pw-1", "CANARY-pw-2"]);
        }
        // Ids are new (policy): none of the destination ids equals a source id.
        let src_ids: Vec<Id> = src
            .v
            .list(&ListFilter::default(), Page::ALL)
            .unwrap()
            .into_iter()
            .map(|s| s.id)
            .collect();
        assert!(
            dst.v
                .list(&ListFilter::default(), Page::ALL)
                .unwrap()
                .iter()
                .all(|s| !src_ids.contains(&s.id))
        );
        // Search works on the imported vault and secrets are still not indexed.
        assert_eq!(search(&mut dst.v, "main"), ["Main Login"]);
        assert!(search(&mut dst.v, "CANARY-pw-3").is_empty());
    }
}

#[test]
fn trash_and_deleted_folders_are_not_exported() {
    let mut e = env();
    let gone =
        e.v.create_item(NewItem::new(ItemType::Login, "Trashed Item"))
            .unwrap();
    e.v.create_item(NewItem::new(ItemType::Login, "Visible Item"))
        .unwrap();
    e.v.delete_item(&gone).unwrap();
    let f = e.v.create_folder("Dead", None).unwrap();
    e.v.delete_folder(&f).unwrap();
    let file = e.v.export_aryavault(PW, &floor(), true).unwrap();
    let b = parse_aryavault(&file, PW, &lim()).unwrap();
    assert_eq!(
        b.items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
        ["Visible Item"]
    );
    assert!(b.folders.is_empty());
}

#[test]
fn wrong_password_and_tampered_export_do_not_import_anything() {
    let mut src = env();
    populated(&mut src);
    let file = src.v.export_aryavault(PW, &floor(), true).unwrap();
    assert!(matches!(
        parse_aryavault(&file, "CANARY-wrong", &lim()),
        Err(ImportError::AuthenticationFailed)
    ));
    for i in [0usize, 5, 40, 100, 200, file.len() / 2, file.len() - 1] {
        let mut m = file.clone();
        m[i] ^= 0x01;
        // Parse errors or authentication failure; never a bundle. (Argon2 re-runs only when a flip
        // changes the KDF input and survives structural validation.)
        assert!(parse_aryavault(&m, PW, &lim()).is_err(), "flip at {i}");
    }
    assert!(parse_aryavault(&file[..file.len() - 1], PW, &lim()).is_err());
    assert!(parse_aryavault(&[], PW, &lim()).is_err());
    let mut dst = env_with(0xC3);
    let before = dump(&mut dst.v, &["main"]);
    let _ = parse_aryavault(&file, "CANARY-wrong", &lim());
    assert_eq!(dump(&mut dst.v, &["main"]), before);
}

#[test]
fn export_hides_everything_without_the_password() {
    let mut src = env();
    populated(&mut src);
    let file = src.v.export_aryavault(PW, &floor(), true).unwrap();
    for needle in ["CANARY", "Main Login", "alice", "example", "Work", "login"] {
        assert!(
            !file.windows(needle.len()).any(|w| w == needle.as_bytes()),
            "{needle} visible in the export"
        );
    }
    let info = read_container_info(&file, &lim()).unwrap();
    assert_eq!((info.m_kib, info.t, info.p), (65_536, 3, 1));
    // Export sizes are bucketed to 1 KiB steps.
    assert_eq!((info.ciphertext_len - 16) % 1024, 0);
    assert!(
        src.v.export_aryavault("", &floor(), false).is_err(),
        "empty password refused"
    );
    let weak = KdfParams {
        m_kib: 1024,
        ..floor()
    };
    assert!(matches!(
        src.v.export_aryavault(PW, &weak, false),
        Err(ImportError::InvalidKdf)
    ));
}

#[test]
fn csv_export_from_a_vault_neutralises_and_reports() {
    let mut e = env();
    e.v.create_item(
        NewItem::new(ItemType::Login, "=cmd|' /C calc'!A0")
            .with_field(StdField::Password, "-CANARY-pw")
            .with_field(StdField::Username, "@u"),
    )
    .unwrap();
    e.v.create_item(NewItem::new(ItemType::Card, "Visa").with_field(StdField::Number, "4111"))
        .unwrap();
    e.v.create_item(NewItem::new(ItemType::Note, "Memo").with_field(StdField::Body, "plain body"))
        .unwrap();
    let out =
        e.v.export_csv(PlaintextRiskAcknowledged::acknowledge_plaintext_risk())
            .unwrap();
    assert_eq!(
        (
            out.report.exported,
            out.report.skipped_unsupported,
            out.report.neutralized_cells
        ),
        (2, 1, 3)
    );
    let text = String::from_utf8(out.bytes.to_vec()).unwrap();
    assert!(text.starts_with("name,url,username,password,note,totp,type,folder,tags,favorite\n"));
    assert!(text.contains("'=cmd") && text.contains("'-CANARY-pw") && text.contains("'@u"));
    assert!(!text.contains("4111"), "cards are not exported to CSV");
}

// ----------------------------------------------------------------- known-answer fixture

fn kat_payload() -> (Vec<ExportItem>, Vec<ExportFolder>) {
    let item = ExportItem {
        id: [0x11; 16],
        item_type: ItemType::Login,
        title: "CANARY Example".into(),
        folder: Some([0x22; 16]),
        favorite: true,
        fields: vec![
            (StdField::Username, Zeroizing::new("canary-user".into())),
            (
                StdField::Password,
                Zeroizing::new("CANARY-kat-password".into()),
            ),
        ],
        urls: vec!["https://example.test".into()],
        tags: vec!["canary".into()],
        custom: vec![],
        history: vec![],
    };
    (
        vec![item],
        vec![ExportFolder {
            id: [0x22; 16],
            name: "CANARY Folder".into(),
            parent: None,
        }],
    )
}

fn kat_file() -> Vec<u8> {
    let (items, folders) = kat_payload();
    let payload = build_payload(&items, &folders, false);
    // salt = 00..0f, export_id = 10..1f, nonce = 20..37 (CountingRng draws them in that order).
    aryavault::seal_container(
        &payload,
        "CANARY-export-password-v1",
        &KdfParams::floor([0; 16]),
        &mut CountingRng(0),
    )
    .unwrap()
}

#[test]
fn aryavault_known_answer_fixture_is_stable_and_opens() {
    let committed = fixture("aryavault-v1.avex");
    assert_eq!(
        kat_file(),
        committed,
        "the format changed; bump the version and add a new fixture instead"
    );
    let b = parse_aryavault(&committed, "CANARY-export-password-v1", &lim()).unwrap();
    assert_eq!(b.items.len(), 1);
    assert_eq!(b.items[0].title, "CANARY Example");
    assert_eq!(b.folders[0].name, "CANARY Folder");
    assert!(
        b.items[0]
            .fields
            .iter()
            .any(|(f, v)| *f == StdField::Password && v.as_str() == "CANARY-kat-password")
    );
}

fn kat_payload_bytes() -> Vec<u8> {
    let (items, folders) = kat_payload();
    build_payload(&items, &folders, false).to_vec()
}

#[test]
fn aryavault_payload_fixture_is_stable_and_parses() {
    let committed = fixture("aryavault-v1.payload.cbor");
    assert_eq!(kat_payload_bytes(), committed);
    let b = parse_aryavault_payload(&committed, &lim()).unwrap();
    assert_eq!((b.items.len(), b.folders.len()), (1, 1));
}

/// Writes the fixtures. Run once when intentionally introducing a format version:
/// `cargo test -p arya-vault-vault --lib regenerate_aryavault_fixture -- --ignored`
#[test]
#[ignore = "writes core/testdata/import/aryavault-v1.*; only for a new format version"]
fn regenerate_aryavault_fixture() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata/import");
    std::fs::write(dir.join("aryavault-v1.avex"), kat_file()).unwrap();
    std::fs::write(dir.join("aryavault-v1.payload.cbor"), kat_payload_bytes()).unwrap();
}

#[test]
fn import_into_a_busy_vault_keeps_existing_data_intact() {
    let mut e = env();
    let keep =
        e.v.create_item(
            NewItem::new(ItemType::Login, "Keep Me").with_field(StdField::Password, "CANARY-keep"),
        )
        .unwrap();
    let b = parse_bitwarden_json(&fixture("bitwarden.json"), &lim()).unwrap();
    e.v.commit_import(&b, &ImportOptions::default()).unwrap();
    assert_eq!(
        e.v.reveal(&keep, StdField::Password)
            .unwrap()
            .unwrap()
            .as_str(),
        "CANARY-keep"
    );
    assert_eq!(e.v.item_count().unwrap(), 5);
}
