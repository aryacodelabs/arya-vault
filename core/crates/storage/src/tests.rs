//! Unit tests (in-crate so they can reach internals such as the raw connection).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use rusqlite::Connection;

use crate::migrations::{self, MIGRATIONS, Migration};
use crate::pragmas;
use crate::*;

const CANARY: &str = "CANARY-7F3A-0001-DO-NOT-USE";

fn params() -> CreateParams {
    CreateParams {
        vault_id: [0xA1; 16],
        device_id: [0xB2; 16],
        epoch: 1,
        header_version: 1,
    }
}
fn key(b: u8) -> DbKey {
    DbKey::from_bytes([b; 32])
}
fn setup() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("vault.db");
    (dir, p)
}
fn item(n: u8) -> ItemRow {
    ItemRow {
        id: [n; 16],
        item_type: "login".into(),
        folder_id: None,
        deleted: false,
        deleted_hlc: None,
        updated_hlc: i64::from(n),
    }
}
fn field(n: u8, k: &str, v: &[u8], hlc: i64) -> FieldRow {
    FieldRow {
        item_id: [n; 16],
        key: k.into(),
        value: Some(v.to_vec()),
        hlc,
        device_id: [0xB2; 16],
        base_hlc: None,
    }
}
fn raw(path: &Path, k: &DbKey) -> Connection {
    let c = Connection::open(path).unwrap();
    pragmas::apply_cipher(&c, k).unwrap();
    c
}
fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
fn sidecar(p: &Path, suffix: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

// ---------------------------------------------------------------- lifecycle

#[test]
fn create_open_close_round_trip() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    assert_eq!(db.schema_version().unwrap(), latest_schema_version());
    db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        t.put_field(&field(1, "title", CANARY.as_bytes(), 10))?;
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    assert!(
        !sidecar(&p, "-wal").exists(),
        "close must checkpoint and remove the WAL"
    );

    let mut db = Db::open(&p, key(1)).unwrap();
    let got = db
        .with_read(|t| t.get_field(&[1; 16], "title"))
        .unwrap()
        .unwrap();
    assert_eq!(got.value.as_deref(), Some(CANARY.as_bytes()));
    db.integrity_check().unwrap();
    assert_eq!(
        db.with_read(|t| t.meta_get("vault_id")).unwrap().unwrap(),
        vec![0xA1; 16]
    );
    assert_eq!(
        db.with_read(|t| t.meta_get("epoch")).unwrap().unwrap(),
        b"1"
    );
}

#[test]
fn create_refuses_existing_and_open_refuses_missing() {
    let (_d, p) = setup();
    assert!(matches!(Db::open(&p, key(1)), Err(StorageError::NotFound)));
    assert!(!p.exists(), "open must not create the file");
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    assert!(matches!(
        Db::create(&p, key(1), &params()),
        Err(StorageError::AlreadyExists)
    ));
    assert!(
        Db::open(&p, key(1)).is_ok(),
        "failed create must not damage the existing db"
    );
}

#[test]
fn create_leaves_no_temp_or_sidecar_files() {
    let (d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let names: Vec<String> = std::fs::read_dir(d.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["vault.db".to_string()],
        "unexpected files: {names:?}"
    );
}

#[cfg(unix)]
#[test]
fn created_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    assert_eq!(
        std::fs::metadata(&p).unwrap().permissions().mode() & 0o077,
        0
    );
}

// ------------------------------------------------------------ wrong key etc.

#[test]
fn wrong_key_is_typed_fast_and_indistinguishable() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let start = Instant::now();
    let err = Db::open(&p, key(2)).unwrap_err();
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "wrong key must fail fast (raw key, no KDF)"
    );
    assert!(matches!(err, StorageError::WrongKeyOrCorrupt));
    // Same variant and message as for garbage, so the caller cannot tell them apart.
    std::fs::write(&p, vec![0x5Au8; 16 * 1024]).unwrap();
    let err2 = Db::open(&p, key(1)).unwrap_err();
    assert_eq!(err.to_string(), err2.to_string());
}

#[test]
fn truncated_garbage_and_foreign_files_are_wrong_key_or_corrupt() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let full = std::fs::read(&p).unwrap();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("empty", vec![]),
        ("tiny", full[..100].to_vec()),
        ("not page aligned", full[..5000].to_vec()),
        ("first page only", full[..4096].to_vec()),
        ("half", full[..full.len() / 2].to_vec()),
        (
            "garbage",
            (0..20_000u32)
                .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
                .collect(),
        ),
    ];
    for (name, bytes) in cases {
        std::fs::write(&p, &bytes).unwrap();
        let r = Db::open(&p, key(1));
        assert!(
            matches!(r, Err(StorageError::WrongKeyOrCorrupt)),
            "{name}: {r:?}"
        );
    }
    // An unencrypted SQLite database is not accepted either.
    std::fs::remove_file(&p).unwrap();
    Connection::open(&p)
        .unwrap()
        .execute_batch("CREATE TABLE meta(key TEXT, value BLOB);")
        .unwrap();
    assert!(matches!(
        Db::open(&p, key(1)),
        Err(StorageError::WrongKeyOrCorrupt)
    ));
}

#[cfg(unix)]
#[test]
fn read_only_directory_is_a_typed_error() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let ro = dir.path().join("ro");
    std::fs::create_dir(&ro).unwrap();
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
    if std::fs::File::create(ro.join("probe")).is_ok() {
        eprintln!("skipped: running as a user that ignores directory permissions (root)");
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        return;
    }
    let r = Db::create(&ro.join("vault.db"), key(1), &params());
    assert!(matches!(r, Err(StorageError::ReadOnly)), "{r:?}");
    std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn database_full_rolls_back_cleanly() {
    // Simulates SQLITE_FULL with max_page_count (not a real ENOSPC, see PR notes).
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> { t.upsert_item(&item(1)) })
        .unwrap();
    let pages: i64 = db
        .conn
        .query_row("PRAGMA page_count", [], |r| r.get(0))
        .unwrap();
    db.conn
        .execute_batch(&format!("PRAGMA max_page_count = {};", pages + 8))
        .unwrap();
    let big = vec![0xEEu8; 60 * 1024];
    let r = db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(2))?;
        for i in 0..200 {
            t.put_field(&field(2, &format!("f{i}"), &big, i))?;
        }
        Ok(())
    });
    assert!(matches!(r, Err(StorageError::Full)), "{r:?}");
    db.integrity_check().unwrap();
    assert!(
        db.with_read(|t| t.get_item(&[2; 16])).unwrap().is_none(),
        "failed tx fully rolled back"
    );
    assert!(
        db.with_read(|t| t.get_item(&[1; 16])).unwrap().is_some(),
        "earlier commit intact"
    );
}

// ------------------------------------------------------------- pinned config

#[test]
fn recorded_settings_are_written_on_create() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    for (name, expected) in pragmas::PINNED {
        let got = db.with_read(|t| t.meta_get(name)).unwrap().unwrap();
        assert_eq!(got, expected.as_bytes(), "{name}");
    }
}

#[test]
fn every_recorded_setting_mismatch_is_a_typed_error() {
    for (name, _) in pragmas::PINNED {
        let (_d, p) = setup();
        Db::create(&p, key(1), &params()).unwrap().close().unwrap();
        let c = raw(&p, &key(1));
        c.execute(
            "UPDATE meta SET value = ?1 WHERE key = ?2",
            rusqlite::params![b"MUTATED".as_slice(), name],
        )
        .unwrap();
        drop(c);
        match Db::open(&p, key(1)) {
            Err(StorageError::SettingsMismatch { name: n, found, .. }) => {
                assert_eq!(n, *name);
                assert_eq!(found, "MUTATED");
            }
            other => panic!("{name}: expected SettingsMismatch, got {other:?}"),
        }
    }
}

#[test]
fn missing_recorded_setting_is_a_mismatch() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    raw(&p, &key(1))
        .execute("DELETE FROM meta WHERE key = 'cipher.page_size'", [])
        .unwrap();
    assert!(matches!(
        Db::open(&p, key(1)),
        Err(StorageError::SettingsMismatch {
            name: "cipher.page_size",
            ..
        })
    ));
}

#[test]
fn live_journal_mode_change_is_detected() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let c = raw(&p, &key(1));
    let mode: String = c
        .query_row("PRAGMA journal_mode = DELETE", [], |r| r.get(0))
        .unwrap();
    assert_eq!(mode, "delete");
    drop(c);
    assert!(matches!(
        Db::open(&p, key(1)),
        Err(StorageError::SettingsMismatch {
            name: "journal_mode",
            ..
        })
    ));
}

#[test]
fn live_pragmas_are_applied_and_verified() {
    let (_d, p) = setup();
    let db = Db::create(&p, key(1), &params()).unwrap();
    pragmas::verify_live(&db.conn).unwrap();
    for (pragma, name) in [
        ("synchronous = NORMAL", "synchronous"),
        ("foreign_keys = OFF", "foreign_keys"),
        ("secure_delete = OFF", "secure_delete"),
        ("temp_store = FILE", "temp_store"),
    ] {
        db.conn.execute_batch(&format!("PRAGMA {pragma};")).unwrap();
        assert!(
            matches!(pragmas::verify_live(&db.conn), Err(StorageError::SettingsMismatch { name: n, .. }) if n == name),
            "{name}"
        );
        pragmas::apply_connection(&db.conn).unwrap();
    }
    pragmas::verify_live(&db.conn).unwrap();
}

#[test]
fn linked_library_is_sqlcipher() {
    let (_d, p) = setup();
    let db = Db::create(&p, key(1), &params()).unwrap();
    let v: String = db
        .conn
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .unwrap();
    assert!(v.starts_with('4'), "SQLCipher 4.x expected, got {v}");
}

// ----------------------------------------------------------------- disk scan

fn scan_files(files: &[PathBuf], needles: &[&[u8]]) {
    for f in files {
        if let Ok(bytes) = std::fs::read(f) {
            for n in needles {
                assert!(
                    !contains(&bytes, n),
                    "{} contains {:?}",
                    f.display(),
                    String::from_utf8_lossy(n)
                );
            }
        }
    }
}

#[test]
fn disk_scan_finds_no_canary_or_sqlite_header() {
    // SEC-S01 / SEC-S02 / SEC-S05
    let (d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    let doc = FtsDoc {
        title: format!("{CANARY}-title"),
        username: format!("{CANARY}-user"),
        urls: format!("https://{CANARY}.example"),
        notes: format!("{CANARY}-notes"),
        tags: format!("{CANARY}-tag"),
    };
    db.with_tx(|t| -> Result<()> {
        for n in 1..=200u8 {
            let mut it = item(n);
            it.item_type = format!("{CANARY}-type");
            t.upsert_item(&it)?;
            t.put_field(&field(
                n,
                &format!("custom.{CANARY}.value"),
                format!("{CANARY}-value-{n}").as_bytes(),
                1,
            ))?;
            t.add_history(&field(n, "password", format!("{CANARY}-old").as_bytes(), 0))?;
            t.append_local_op(&[n; 16], "password", Some(CANARY.as_bytes()), 5, None)?;
            t.fts_index(&[n; 16], &doc)?;
        }
        t.upsert_folder(&FolderRow {
            id: [1; 16],
            name: format!("{CANARY}-folder"),
            parent_id: None,
            hlc: 1,
            device_id: [2; 16],
            deleted: false,
        })?;
        t.upsert_device(&DeviceRow {
            device_id: [3; 16],
            name: Some(format!("{CANARY}-device")),
            first_seen: None,
            last_seen: None,
            revoked: false,
        })?;
        t.provider_state_set(&ProviderState {
            provider: "folder".into(),
            cursor: Some(format!("{CANARY}-cursor")),
            updated_at: None,
        })?;
        t.outbox_put(1, CANARY.as_bytes())?;
        Ok(())
    })
    .unwrap();
    // Force sorting/grouping work that could spill to temp files.
    let _ = db
        .with_read(|t| t.list_items(ItemFilter::default()))
        .unwrap();
    db.conn.execute_batch("SELECT key, value FROM field ORDER BY value DESC; SELECT count(*) FROM field GROUP BY key;").ok();

    let needles: [&[u8]; 3] = [CANARY.as_bytes(), b"CANARY-7F3A", b"SQLite format 3"];
    // While open: main file, live WAL and shm.
    let live = [p.clone(), sidecar(&p, "-wal"), sidecar(&p, "-shm")];
    assert!(
        sidecar(&p, "-wal").exists() && std::fs::metadata(sidecar(&p, "-wal")).unwrap().len() > 0,
        "test must exercise a non-empty WAL"
    );
    scan_files(&live, &needles);
    // Temp files SQLite would create (etilqs_*), in the system temp dir.
    let tmp: Vec<PathBuf> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("etilqs_"))
        .map(|e| e.path())
        .collect();
    scan_files(&tmp, &needles);

    // After rekey, checkpoint and close, plus every file in the directory.
    db.rekey(&key(1), &key(2)).unwrap();
    db.close().unwrap();
    let all: Vec<PathBuf> = std::fs::read_dir(d.path())
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    scan_files(&all, &needles);
    // Sanity: the canary really is in the database once decrypted.
    let db = Db::open(&p, key(2)).unwrap();
    let hits = db
        .conn
        .query_row(
            "SELECT count(*) FROM field WHERE value LIKE 'CANARY-7F3A%'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(hits, 200);
}

#[test]
fn encrypted_file_has_no_plaintext_header_and_looks_random() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(bytes.len() % 4096, 0);
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    // Crude entropy check on page 1: no byte value dominates.
    let mut counts = [0u32; 256];
    for b in &bytes[..4096] {
        counts[usize::from(*b)] += 1;
    }
    assert!(
        counts.iter().all(|&c| c < 40),
        "page 1 does not look encrypted"
    );
}

#[test]
fn key_and_errors_never_print_key_material() {
    let k = DbKey::from_bytes([0xAB; 32]);
    assert_eq!(format!("{k:?}"), "DbKey(<redacted>)");
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let k = DbKey::from_bytes([0xCD; 32]);
    let err = Db::open(&p, k).unwrap_err();
    let shown = format!("{err} {err:?}");
    assert!(!shown.to_lowercase().contains("cdcdcd") && !shown.contains("205"));
    assert!(DbKey::from_slice(&[0u8; 31]).is_err());
    assert!(DbKey::from_slice(&[0u8; 32]).is_ok());
}

// -------------------------------------------------------------------- rekey

#[test]
fn rekey_round_trip_and_old_key_rejected() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        t.put_field(&field(1, "title", CANARY.as_bytes(), 1))
    })
    .unwrap(); // leave data in the WAL on purpose
    assert!(
        matches!(
            db.rekey(&key(9), &key(2)),
            Err(StorageError::WrongKeyOrCorrupt)
        ),
        "wrong `old` argument"
    );
    db.rekey(&key(1), &key(2)).unwrap();
    db.with_tx(|t| -> Result<()> { t.upsert_item(&item(2)) })
        .unwrap(); // still usable after rekey
    db.close().unwrap();
    assert!(matches!(
        Db::open(&p, key(1)),
        Err(StorageError::WrongKeyOrCorrupt)
    ));
    let mut db = Db::open(&p, key(2)).unwrap();
    db.integrity_check().unwrap();
    assert!(
        db.with_read(|t| t.get_field(&[1; 16], "title"))
            .unwrap()
            .is_some()
    );
    assert!(db.with_read(|t| t.get_item(&[2; 16])).unwrap().is_some());
}

// ----------------------------------------------------------------- integrity

#[test]
fn integrity_check_reports_page_corruption() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        for n in 1..=100u8 {
            t.upsert_item(&item(n))?;
            t.put_field(&field(n, "title", &[n; 200], 1))?;
        }
        Ok(())
    })
    .unwrap();
    db.close().unwrap();
    let mut bytes = std::fs::read(&p).unwrap();
    let off = 4096 * 6 + 100;
    bytes[off] ^= 0xFF;
    std::fs::write(&p, &bytes).unwrap();
    // Page 1 is intact so open succeeds; the damaged page's HMAC fails during the check.
    let db = Db::open(&p, key(1)).unwrap();
    assert!(db.integrity_check().is_err());
}

// -------------------------------------------------------------- transactions

#[test]
fn with_tx_rolls_back_on_error() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    let r = db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        Err(StorageError::Busy)
    });
    assert!(matches!(r, Err(StorageError::Busy)));
    assert!(db.with_read(|t| t.get_item(&[1; 16])).unwrap().is_none());
}

#[test]
fn foreign_keys_are_enforced() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    let r = db.with_tx(|t| -> Result<()> { t.put_field(&field(7, "title", b"x", 1)) });
    assert!(matches!(r, Err(StorageError::Constraint(_))), "{r:?}");
}

// -------------------------------------------------------------- store methods

#[test]
fn store_items_fields_history_folders() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        let mut a = item(1);
        a.folder_id = Some([9; 16]);
        t.upsert_item(&a)?;
        let mut b = item(2);
        b.item_type = "note".into();
        b.deleted = true;
        b.deleted_hlc = Some(3);
        t.upsert_item(&b)?;
        assert_eq!(t.list_items(ItemFilter::default())?.len(), 1);
        assert_eq!(
            t.list_items(ItemFilter {
                include_deleted: true,
                ..Default::default()
            })?
            .len(),
            2
        );
        assert_eq!(
            t.list_items(ItemFilter {
                include_deleted: true,
                item_type: Some("note"),
                folder_id: None
            })?[0]
                .id,
            [2; 16]
        );
        assert_eq!(
            t.list_items(ItemFilter {
                folder_id: Some([9; 16]),
                ..Default::default()
            })?[0]
                .id,
            [1; 16]
        );
        assert_eq!(t.get_item(&[2; 16])?.unwrap().deleted_hlc, Some(3));

        t.put_field(&field(1, "title", b"a", 1))?;
        t.put_field(&field(1, "title", b"b", 2))?; // replace
        t.put_field(&field(1, "password", b"p", 1))?;
        assert_eq!(
            t.get_field(&[1; 16], "title")?.unwrap().value.unwrap(),
            b"b"
        );
        assert_eq!(
            t.fields_for_item(&[1; 16])?
                .iter()
                .map(|f| f.key.as_str())
                .collect::<Vec<_>>(),
            ["password", "title"]
        );

        for h in 1..=30 {
            t.add_history(&field(1, "password", format!("v{h}").as_bytes(), h))?;
        }
        t.add_history(&field(1, "password", b"dup", 30))?; // same PK: ignored
        assert_eq!(t.history_for(&[1; 16], "password")?.len(), 30);
        assert_eq!(t.history_for(&[1; 16], "password")?[0].hlc, 30);
        assert_eq!(t.prune_history(&[1; 16], "password", 20)?, 10);
        let left = t.history_for(&[1; 16], "password")?;
        assert_eq!((left.len(), left[19].hlc), (20, 11));

        t.upsert_folder(&FolderRow {
            id: [9; 16],
            name: "Work".into(),
            parent_id: None,
            hlc: 1,
            device_id: [2; 16],
            deleted: false,
        })?;
        t.upsert_folder(&FolderRow {
            id: [8; 16],
            name: "Home".into(),
            parent_id: Some([9; 16]),
            hlc: 2,
            device_id: [2; 16],
            deleted: true,
        })?;
        let f = t.list_folders()?;
        assert_eq!(
            (f[0].name.as_str(), f[0].deleted, f[0].parent_id),
            ("Home", true, Some([9; 16]))
        );
        assert_eq!(t.get_folder(&[9; 16])?.unwrap().name, "Work");
        Ok(())
    })
    .unwrap();
}

#[test]
fn upserting_an_item_keeps_its_rowid_so_the_search_index_stays_attached() {
    // Regression: `INSERT OR REPLACE` re-inserted the row with a new rowid, orphaning the FTS entry
    // and corrupting the index on the next `fts_remove`.
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        t.put_field(&field(1, "title", b"x", 1))?; // a child row must survive the upsert too
        let doc = FtsDoc {
            title: "Findable".into(),
            ..Default::default()
        };
        t.fts_index(&[1; 16], &doc)?;
        let mut changed = item(1);
        changed.folder_id = Some([9; 16]);
        changed.deleted = true;
        changed.deleted_hlc = Some(5);
        changed.updated_hlc = 7;
        t.upsert_item(&changed)?;
        assert_eq!(t.get_item(&[1; 16])?.unwrap(), changed);
        assert_eq!(
            t.fts_search("findable", 5)?,
            vec![[1; 16]],
            "index still attached after the upsert"
        );
        assert_eq!(t.fields_for_item(&[1; 16])?.len(), 1);
        t.fts_remove(&[1; 16], &doc)?; // would corrupt the index if the rowid had changed
        assert!(t.fts_search("findable", 5)?.is_empty());
        Ok(())
    })
    .unwrap();
    db.integrity_check().unwrap();
}

#[test]
fn store_bulk_field_reads_and_item_count() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        assert_eq!(t.count_items()?, 0);
        for n in 1..=3u8 {
            t.upsert_item(&item(n))?;
            t.put_field(&field(n, "title", &[n], 1))?;
            t.put_field(&field(n, &format!("tags.t{n}"), b"x", 1))?;
        }
        t.put_field(&field(1, "titlex", b"no", 1))?;
        t.put_field(&field(1, "tags%_", b"literal wildcard chars", 1))?;
        assert_eq!(t.count_items()?, 3);
        assert_eq!(t.fields_with_key("title")?.len(), 3, "exact key only");
        assert_eq!(t.fields_with_key("nothing")?.len(), 0);
        let mut tags: Vec<String> = t
            .fields_with_key_prefix("tags.")?
            .into_iter()
            .map(|f| f.key)
            .collect();
        tags.sort();
        assert_eq!(
            tags,
            ["tags.t1", "tags.t2", "tags.t3"],
            "prefix is literal (no LIKE wildcards)"
        );
        assert_eq!(t.fields_with_key_prefix("tags%")?.len(), 1);
        Ok(())
    })
    .unwrap();
}

#[test]
fn store_sync_state() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        let s1 = t.append_local_op(&[1; 16], "title", Some(b"x"), 1, None)?;
        let s2 = t.append_local_op(&[1; 16], "title", None, 2, Some(1))?;
        assert!(s2 > s1);
        let ops = t.pending_local_ops(10)?;
        assert_eq!(
            (ops.len(), ops[1].value.clone(), ops[1].base_hlc),
            (2, None, Some(1))
        );
        assert_eq!(t.pending_local_ops(1)?.len(), 1);
        assert_eq!(t.delete_local_ops_through(s1)?, 1);
        assert_eq!(t.pending_local_ops(10)?[0].seq, s2);

        t.outbox_put(1, b"first")?;
        t.outbox_put(1, b"DIFFERENT")?; // retries must resend identical bytes: existing row wins
        t.outbox_put(2, b"second")?;
        assert_eq!(t.outbox_pending()?[0].bytes, b"first");
        t.outbox_mark_uploaded(1)?;
        assert_eq!(t.outbox_pending()?.len(), 1);
        t.outbox_delete(2)?;
        assert!(t.outbox_pending()?.is_empty());

        assert_eq!(t.manifest_seen_get(&[5; 16])?, None);
        t.manifest_seen_raise(&[5; 16], 10)?;
        t.manifest_seen_raise(&[5; 16], 4)?; // never lowers
        assert_eq!(t.manifest_seen_get(&[5; 16])?, Some(10));

        let seg = SegmentSeen {
            device_id: [6; 16],
            seq: 3,
            hash: vec![1, 2, 3],
            applied_at: 99,
        };
        t.segment_seen_put(&seg)?;
        assert_eq!(t.segment_seen_get(&[6; 16], 3)?, Some(seg));
        assert_eq!(t.segment_seen_get(&[6; 16], 4)?, None);

        let dev = DeviceRow {
            device_id: [7; 16],
            name: Some("Laptop".into()),
            first_seen: Some(1),
            last_seen: Some(2),
            revoked: true,
        };
        t.upsert_device(&dev)?;
        assert_eq!(t.list_devices()?, vec![dev]);

        assert_eq!(t.provider_state_get("folder")?, None);
        let ps = ProviderState {
            provider: "folder".into(),
            cursor: Some("c1".into()),
            updated_at: Some(5),
        };
        t.provider_state_set(&ps)?;
        assert_eq!(t.provider_state_get("folder")?, Some(ps));

        t.meta_set("hlc_state", &[1, 2, 3])?;
        t.meta_set("hlc_state", &[4])?;
        assert_eq!(t.meta_get("hlc_state")?, Some(vec![4]));
        Ok(())
    })
    .unwrap();
}

#[test]
fn fts_index_remove_clear_search() {
    let (_d, p) = setup();
    let mut db = Db::create(&p, key(1), &params()).unwrap();
    db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        t.upsert_item(&item(2))?;
        let d1 = FtsDoc {
            title: "GitHub login".into(),
            username: "alice".into(),
            urls: "https://github.com".into(),
            ..Default::default()
        };
        let d2 = FtsDoc {
            title: "Bank".into(),
            notes: "github token backup".into(),
            ..Default::default()
        };
        t.fts_index(&[1; 16], &d1)?;
        t.fts_index(&[2; 16], &d2)?;
        assert_eq!(t.fts_search("github", 10)?.len(), 2);
        assert_eq!(t.fts_search("title:github", 10)?, vec![[1; 16]]);
        assert_eq!(t.fts_search("alice", 10)?, vec![[1; 16]]);
        t.fts_remove(&[1; 16], &d1)?;
        assert_eq!(t.fts_search("github", 10)?, vec![[2; 16]]);
        assert!(t.fts_search("alice", 10)?.is_empty());
        t.fts_clear()?;
        assert!(t.fts_search("github", 10)?.is_empty());
        assert!(matches!(
            t.fts_index(&[3; 16], &d1),
            Err(StorageError::Constraint(_))
        ));
        Ok(())
    })
    .unwrap();
}

// ------------------------------------------------------------------ migrations

const V2_OK: Migration = Migration {
    version: 2,
    sql: "ALTER TABLE item ADD COLUMN note TEXT; CREATE TABLE extra(x INTEGER);",
};
const V2_BAD: Migration = Migration {
    version: 2,
    sql: "CREATE TABLE extra(x INTEGER); INSERT INTO table_that_does_not_exist VALUES (1);",
};

fn v1_list() -> Vec<Migration> {
    MIGRATIONS.to_vec()
}
fn list_with(extra: Migration) -> Vec<Migration> {
    let mut v = v1_list();
    v.push(extra);
    v
}
fn backups(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().contains(".bak-v"))
        .collect();
    v.sort();
    v
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/v1")
}
fn fixture_key() -> DbKey {
    key(0x42)
}
fn fill_fixture(db: &mut Db) {
    db.with_tx(|t| -> Result<()> {
        t.upsert_item(&item(1))?;
        t.put_field(&field(1, "title", b"CANARY-FIXTURE-v1", 7))?;
        t.add_history(&field(1, "title", b"CANARY-FIXTURE-old", 3))?;
        t.upsert_folder(&FolderRow {
            id: [9; 16],
            name: "CANARY-FIXTURE-folder".into(),
            parent_id: None,
            hlc: 1,
            device_id: [2; 16],
            deleted: false,
        })?;
        t.upsert_device(&DeviceRow {
            device_id: [2; 16],
            name: Some("CANARY-FIXTURE-device".into()),
            first_seen: Some(1),
            last_seen: Some(2),
            revoked: false,
        })?;
        t.append_local_op(&[1; 16], "title", Some(b"CANARY-FIXTURE-v1"), 7, None)?;
        t.fts_index(
            &[1; 16],
            &FtsDoc {
                title: "CANARY-FIXTURE-v1".into(),
                ..Default::default()
            },
        )
    })
    .unwrap();
}
fn assert_fixture(db: &mut Db) {
    db.integrity_check().unwrap();
    db.with_read(|t| -> Result<()> {
        assert_eq!(
            t.get_field(&[1; 16], "title")?.unwrap().value.unwrap(),
            b"CANARY-FIXTURE-v1"
        );
        assert_eq!(t.history_for(&[1; 16], "title")?.len(), 1);
        assert_eq!(t.list_folders()?[0].name, "CANARY-FIXTURE-folder");
        assert_eq!(t.list_devices()?.len(), 1);
        assert_eq!(t.pending_local_ops(5)?.len(), 1);
        assert_eq!(t.fts_search("fixture", 5)?, vec![[1; 16]]);
        assert_eq!(t.meta_get("vault_id")?.unwrap(), vec![0xA1; 16]);
        Ok(())
    })
    .unwrap();
}

/// Regenerates the committed v1 fixture. Golden files are append-only: this refuses
/// to overwrite. Run: `ARYA_WRITE_FIXTURE=1 cargo test -p arya-vault-storage -- --ignored regenerate_v1_fixture`
#[test]
#[ignore = "writes tests/fixtures/v1/vault.db; only for adding a new fixture version"]
fn regenerate_v1_fixture() {
    assert_eq!(std::env::var("ARYA_WRITE_FIXTURE").as_deref(), Ok("1"));
    let target = fixture_dir().join("vault.db");
    assert!(
        !target.exists(),
        "fixtures are append-only; refusing to overwrite {}",
        target.display()
    );
    let mut db = Db::create(&target, fixture_key(), &params()).unwrap();
    fill_fixture(&mut db);
    db.close().unwrap();
}

fn fixture_copy() -> (tempfile::TempDir, PathBuf) {
    let (d, p) = setup();
    std::fs::copy(fixture_dir().join("vault.db"), &p).unwrap();
    (d, p)
}

#[test]
fn committed_v1_fixture_still_opens() {
    let (_d, p) = fixture_copy();
    let mut db = Db::open(&p, fixture_key()).unwrap();
    assert_eq!(db.schema_version().unwrap(), 1);
    assert_fixture(&mut db);
    assert!(
        backups(p.parent().unwrap()).is_empty(),
        "no backup when nothing to migrate"
    );
}

#[test]
fn migration_from_v1_fixture_makes_encrypted_backup_and_keeps_data() {
    let (d, p) = fixture_copy();
    let mut db = Db::open_with(&p, fixture_key(), &list_with(V2_OK)).unwrap();
    assert_eq!(db.schema_version().unwrap(), 2);
    assert_fixture(&mut db);
    db.conn
        .execute_batch("INSERT INTO extra VALUES (1); UPDATE item SET note = 'n';")
        .unwrap();
    db.close().unwrap();

    let b = backups(d.path());
    assert_eq!(b.len(), 1, "{b:?}");
    let bytes = std::fs::read(&b[0]).unwrap();
    assert!(!bytes.starts_with(b"SQLite format 3\0"));
    assert!(
        !contains(&bytes, b"CANARY-FIXTURE"),
        "backup must be encrypted"
    );
    // The backup is a complete v1 database under the same key.
    let mut old = Db::open(&b[0], fixture_key()).unwrap();
    assert_eq!(old.schema_version().unwrap(), 1);
    assert_fixture(&mut old);
    assert!(
        old.conn.prepare("SELECT x FROM extra").is_err(),
        "backup predates the migration"
    );
}

#[test]
fn failed_migration_rolls_back_and_leaves_original_intact() {
    let (d, p) = fixture_copy();
    let r = Db::open_with(&p, fixture_key(), &list_with(V2_BAD));
    assert!(
        matches!(r, Err(StorageError::MigrationFailed { version: 2 })),
        "{r:?}"
    );
    let mut db = Db::open(&p, fixture_key()).unwrap();
    assert_eq!(db.schema_version().unwrap(), 1);
    assert!(
        db.conn.prepare("SELECT x FROM extra").is_err(),
        "partial DDL must be rolled back"
    );
    assert_fixture(&mut db);
    assert_eq!(
        backups(d.path()).len(),
        1,
        "backup is taken before the attempt"
    );
}

#[test]
fn newer_database_is_refused() {
    let (_d, p) = fixture_copy();
    Db::open_with(&p, fixture_key(), &list_with(V2_OK))
        .unwrap()
        .close()
        .unwrap();
    let r = Db::open(&p, fixture_key());
    assert!(
        matches!(
            r,
            Err(StorageError::SchemaTooNew {
                found: 2,
                supported: 1
            })
        ),
        "{r:?}"
    );
}

#[test]
fn two_openers_migrate_once() {
    let (_d, p) = fixture_copy();
    let list = list_with(V2_OK);
    let (p1, p2) = (p.clone(), p.clone());
    let (l1, l2) = (list.clone(), list);
    let t1 = std::thread::spawn(move || Db::open_with(&p1, fixture_key(), &l1).map(|_| ()));
    let t2 = std::thread::spawn(move || Db::open_with(&p2, fixture_key(), &l2).map(|_| ()));
    t1.join().unwrap().unwrap();
    t2.join().unwrap().unwrap();
    let db = Db::open_with(&p, fixture_key(), &list_with(V2_OK)).unwrap();
    assert_eq!(db.schema_version().unwrap(), 2);
}

#[test]
fn backups_older_than_14_days_are_pruned() {
    let (d, p) = fixture_copy();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let old = d.path().join("vault.db.bak-v1-1000");
    let old_n = d.path().join("vault.db.bak-v1-1000-2");
    let recent = d
        .path()
        .join(format!("vault.db.bak-v1-{}", now - 13 * 86_400));
    let stale = d
        .path()
        .join(format!("vault.db.bak-v1-{}", now - 15 * 86_400));
    let unrelated = d.path().join("other.db.bak-v1-1000");
    let junk = d.path().join("vault.db.bak-vX");
    for f in [&old, &old_n, &recent, &stale, &unrelated, &junk] {
        std::fs::write(f, b"x").unwrap();
    }
    Db::open(&p, fixture_key()).unwrap().close().unwrap();
    assert!(!old.exists() && !old_n.exists() && !stale.exists());
    assert!(recent.exists() && unrelated.exists() && junk.exists());
    assert_eq!(
        migrations::BACKUP_RETENTION,
        Duration::from_secs(14 * 86_400)
    );
}

// ----------------------------------------------------------------- concurrency

#[test]
fn readers_and_one_writer_do_not_deadlock() {
    let (_d, p) = setup();
    Db::create(&p, key(1), &params()).unwrap().close().unwrap();
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..4 {
        let (p, stop) = (p.clone(), stop.clone());
        readers.push(std::thread::spawn(move || {
            let mut db = Db::open(&p, key(1)).unwrap();
            let (mut last, mut reads) = (0usize, 0u32);
            while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                let n = db
                    .with_read(|t| t.list_items(ItemFilter::default()).map(|v| v.len()))
                    .unwrap();
                assert!(n >= last, "readers must see a monotonic committed state");
                last = n;
                reads += 1;
            }
            reads
        }));
    }
    let mut w = Db::open(&p, key(1)).unwrap();
    for i in 0..=255u8 {
        w.with_tx(|t| -> Result<()> {
            t.upsert_item(&item(i))?;
            t.put_field(&field(i, "title", &[i; 64], 1))
        })
        .unwrap();
        if i % 64 == 63 {
            w.checkpoint().unwrap();
        }
    }
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    for r in readers {
        assert!(r.join().unwrap() > 0);
    }
    w.integrity_check().unwrap();
    assert_eq!(
        w.with_read(|t| t.list_items(ItemFilter::default()).map(|v| v.len()))
            .unwrap(),
        256
    );
}

#[test]
fn second_writer_gets_busy_not_a_hang() {
    let (_d, p) = setup();
    let mut a = Db::create(&p, key(1), &params()).unwrap();
    let mut b = Db::open(&p, key(1)).unwrap();
    b.conn.execute_batch("PRAGMA busy_timeout = 100;").unwrap();
    let r = a.with_tx(|_| -> Result<()> {
        let started = Instant::now();
        let inner = b.with_tx(|t| -> Result<()> { t.upsert_item(&item(1)) });
        assert!(started.elapsed() < Duration::from_secs(3));
        inner
    });
    assert!(matches!(r, Err(StorageError::Busy)), "{r:?}");
    assert_eq!(pragmas::BUSY_TIMEOUT_MS, 5_000);
}
