//! Damaged, missing, oversized and rolled-back files; and the disk scan for plaintext secrets
//! (SEC-A04/C06 context; SEC-S01 style canary scan over a whole `Session` lifecycle).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use arya_vault_crypto::format::header::Header;
use arya_vault_session::{KdfProfile, Session, SessionState, meta};
use common::*;

const BIN: &str = "files";

#[test]
fn flipped_truncated_oversized_missing_and_foreign_headers_never_unlock() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let hdr = header_path(&dir);
    let original = fs::read(&hdr).unwrap();

    // single-bit flips: authentication fails or the header is rejected, never success
    for i in [0usize, 5, original.len() / 2, original.len() - 1] {
        let mut bad = original.clone();
        bad[i] ^= 0x01;
        fs::write(&hdr, &bad).unwrap();
        let mut s = Session::open_dir(&dir).unwrap();
        let e = s.unlock(PW).unwrap_err();
        assert!(
            matches!(
                e.code().as_str(),
                "wrongCredentials" | "corruptVault" | "unsupportedFormat"
            ),
            "byte {i}: {e:?}"
        );
        assert_eq!(s.state(), SessionState::Locked);
    }

    // truncated
    fs::write(&hdr, &original[..original.len() / 2]).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "corruptVault");
    expect_code(s.status(), "corruptVault");

    // oversized (a real header is ~250 bytes; anything over 4096 is not even read)
    let mut big = original.clone();
    big.resize(5000, 0);
    fs::write(&hdr, &big).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "corruptVault");

    // missing header, database still there
    fs::remove_file(&hdr).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    assert_eq!(
        s.state(),
        SessionState::Locked,
        "a database alone is still a vault"
    );
    expect_code(s.unlock(PW), "corruptVault");
    expect_code(s.status(), "corruptVault");

    // restored: works again
    fs::write(&hdr, &original).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
}

#[test]
fn a_damaged_or_missing_database_is_corrupt_not_a_wrong_password() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let db = dir.join("vault.db");
    let mut bytes = fs::read(&db).unwrap();
    for b in &mut bytes[100..164] {
        *b ^= 0xFF;
    }
    fs::write(&db, &bytes).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "corruptVault");
    assert_eq!(s.state(), SessionState::Locked);

    fs::write(&db, b"SQLite format 3\0 this is not an encrypted database").unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "corruptVault");

    fs::remove_file(&db).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "corruptVault");
}

#[test]
fn a_newer_header_format_means_update_required() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let hdr = header_path(&dir);
    // `encode` refuses to write anything but the current version, so patch the bytes: the value
    // of `format_version` is the last byte of the canonical map (it sorts last).
    let mut bytes = fs::read(&hdr).unwrap();
    let key = b"format_version";
    let at = bytes.windows(key.len()).position(|w| w == key).unwrap() + key.len();
    assert_eq!(bytes[at], 1);
    bytes[at] = 2;
    assert!(Header::decode(&bytes).is_err());
    fs::write(&hdr, bytes).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "unsupportedFormat");
    expect_code(s.status(), "unsupportedFormat");
}

#[test]
fn rollback_to_a_lower_epoch_is_rejected_once_a_higher_one_was_seen() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let real = header_path(&dir);
    let original = fs::read(&real).unwrap();
    let h = Header::decode(&original).unwrap();
    assert_eq!(h.epoch, 1);

    // a provider injects a header of epoch 2 (it cannot make it unwrap, but it wins selection)
    let mut forged = h.clone();
    forged.epoch = 2;
    forged.header_version = 1;
    let forged_path = dir.join(forged.file_name(&[9; 16]));
    fs::write(&forged_path, forged.encode().unwrap()).unwrap();

    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.unlock(PW), "wrongCredentials");

    // it vanishes and the old epoch-1 header is served again: this session refuses to go back
    fs::remove_file(&forged_path).unwrap();
    assert_eq!(fs::read(&real).unwrap(), original);
    expect_code(s.unlock(PW), "corruptVault");
    expect_code(s.status(), "corruptVault");

    // a new session has no memory of epoch 2 (persisting the floor belongs to sync/rotation)
    let mut fresh = Session::open_dir(&dir).unwrap();
    fresh.unlock(PW).unwrap();
}

#[test]
fn header_files_are_replaced_atomically_with_no_temp_leftovers() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    s.change_password(PW, PW2, KdfProfile::Low).unwrap();
    let names: Vec<String> = all_files(&dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    assert!(!names.iter().any(|n| n.ends_with(".tmp")), "{names:?}");
    assert_eq!(names.iter().filter(|n| n.starts_with("header-")).count(), 1);
}

const CANARIES: &[&str] = &[
    PW,
    PW2,
    PW3,
    ITEM_PW,
    "CANARY-master-password",
    "CANARY-item-password",
    "CANARY-second-master",
    "CANARY-third-master",
];

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle)
}

fn scan(dir: &std::path::Path, recovery_keys: &[String], when: &str) {
    for f in all_files(dir) {
        let bytes = fs::read(&f).unwrap();
        for c in CANARIES {
            assert!(
                !contains(&bytes, c.as_bytes()),
                "{when}: `{c}` in {}",
                f.display()
            );
            let utf16: Vec<u8> = c.encode_utf16().flat_map(u16::to_le_bytes).collect();
            assert!(
                !contains(&bytes, &utf16),
                "{when}: UTF-16 `{c}` in {}",
                f.display()
            );
        }
        for k in recovery_keys {
            let compact: String = k.chars().filter(|c| *c != '-').collect();
            assert!(
                !contains(&bytes, k.as_bytes()),
                "{when}: recovery key in {}",
                f.display()
            );
            assert!(
                !contains(&bytes, compact.as_bytes()),
                "{when}: compact recovery key in {}",
                f.display()
            );
        }
        if f.file_name().is_some_and(|n| n == "vault.db") {
            assert!(
                !bytes.starts_with(b"SQLite format 3"),
                "{when}: plaintext SQLite header"
            );
        }
    }
}

#[test]
fn disk_scan_finds_no_plaintext_secret_over_a_whole_lifecycle() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("vault");
    let mut s = Session::open_dir(&dir).unwrap();
    let rk = s.create(PW, KdfProfile::Low).unwrap();
    let key1 = rk.recovery_key().to_string();
    // the key is pending in memory: it must not be on disk
    scan(
        &dir,
        std::slice::from_ref(&key1),
        "after create (unlocked, pending)",
    );
    assert!(
        s.confirm_recovery_key(answers_for(&key1, rk.challenge()))
            .unwrap()
    );
    s.with_vault(|v| {
        v.create_item(
            arya_vault_vault::NewItem::new(arya_vault_vault::ItemType::Login, ITEM_TITLE)
                .with_field(arya_vault_vault::StdField::Password, ITEM_PW),
        )
    })
    .unwrap()
    .unwrap();
    scan(&dir, std::slice::from_ref(&key1), "unlocked (WAL present)");
    s.change_password(PW, PW2, KdfProfile::Low).unwrap();
    let rk2 = s.regenerate_recovery_key(PW2).unwrap();
    let key2 = rk2.recovery_key().to_string();
    scan(
        &dir,
        &[key1.clone(), key2.clone()],
        "after regenerate (pending)",
    );
    s.lock().unwrap();
    scan(&dir, &[key1.clone(), key2.clone()], "after lock");
    s.recover(&key2, PW3, KdfProfile::Low).unwrap();
    s.lock().unwrap();
    scan(&dir, &[key1, key2], "after recover + lock");
    // the marker is a plain flag
    assert!(meta::onboarding_pending(&dir));
}
