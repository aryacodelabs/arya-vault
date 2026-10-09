//! Vault-key rotation through the real API (SEC-A06, SEC-A05, SEC-C05, SEC-C06, SEC-S06, SEC-C12).
//!
//! The crash-safety proof (every step boundary, 200 random kills, the epoch floor) is in
//! `src/rotation.rs` and runs without Argon2; these tests run the real thing end to end with the
//! floor KDF profile.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use arya_vault_crypto::format::header::Header;
use arya_vault_crypto::hkdf::{self, SubKeyLabel};
use arya_vault_crypto::vault_key::{self, HeaderWraps};
use arya_vault_session::quick::fake::FakeProvider;
use arya_vault_session::quick::policy::{BLOB_FILE, POLICY_FILE};
use arya_vault_session::{KdfProfile, Session, SessionState};
use arya_vault_storage::{Db, DbKey};
use arya_vault_vault::{ListFilter, Page, StdField};
use common::*;

const BIN: &str = "rotation";
const LOW: KdfProfile = KdfProfile::Low;

fn header_names(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = all_files(dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .filter(|n| n.starts_with("header-"))
        .collect();
    v.sort();
    v
}

fn leftovers(dir: &std::path::Path) -> Vec<String> {
    all_files(dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .filter(|n| n.contains(".next") || n.ends_with(".tmp"))
        .collect()
}

/// The item the fixture holds, with its secret revealed.
fn revealed_secret(s: &mut Session) -> String {
    s.with_vault(|v| {
        let rows = v.list(&ListFilter::default(), Page::ALL).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, ITEM_TITLE);
        v.reveal(&rows[0].id, StdField::Password)
            .unwrap()
            .unwrap()
            .to_string()
    })
    .unwrap()
}

/// `K_db` of a header's epoch, derived from the password in that header (test-side helper).
fn db_key_of(header_bytes: &[u8], password: &str) -> DbKey {
    let h = Header::decode(header_bytes).unwrap();
    let vk = vault_key::unlock_with_password(
        password,
        &HeaderWraps {
            vault_id: h.vault_id,
            epoch: h.epoch,
            kdf: h.kdf.clone(),
            wrap_pw: h.wrap_pw.clone(),
            wrap_rk: h.wrap_rk.clone(),
        },
    )
    .unwrap();
    let sub = hkdf::subkey(&vk, &h.vault_id, SubKeyLabel::Db, h.epoch).unwrap();
    DbKey::from_bytes(*sub.expose_secret())
}

#[test]
fn sec_a06_rotation_round_trip() {
    let (_t, dir, old_rk) = fixture_copy(BIN);
    let old_header = header_path(&dir);
    let old_name = old_header.file_name().unwrap().to_owned();
    let old_bytes = fs::read(&old_header).unwrap();

    let wall = FakeWall::at(1_800_000_000_000);
    let mut s = Session::open_dir(&dir)
        .unwrap()
        .with_wall_clock(Box::new(wall.clone()));
    s.unlock(PW).unwrap();
    assert!(s.rotation_record().unwrap().is_none());
    wall.advance(1_000);
    let out = s.rotate_keys(PW).unwrap();

    // the outcome and the state
    assert_eq!((out.record.old_epoch, out.record.new_epoch), (1, 2));
    assert_eq!(out.record.at_ms, 1_800_000_001_000);
    assert_eq!(s.rotation_record().unwrap(), Some(out.record));
    assert_eq!(out.recovery_key.header_version(), 2);
    let new_key = out.recovery_key.recovery_key().to_string();
    assert_ne!(new_key, old_rk);
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    assert!(!s.status().unwrap().onboarding_complete);

    // data are intact, readable under the new keys
    assert_eq!(item_count(&mut s), 1);
    assert_eq!(revealed_secret(&mut s), ITEM_PW);

    // files: one header of epoch 2, no leftovers, the file is still not plaintext
    let names = header_names(&dir);
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(
        names[0].starts_with("header-00000002-00000002-"),
        "{names:?}"
    );
    assert_eq!(leftovers(&dir), Vec::<String>::new());
    assert!(
        !fs::read(dir.join("vault.db"))
            .unwrap()
            .starts_with(b"SQLite format 3")
    );

    // the old K_db no longer opens the file (SEC-S01 / SEC-A06)
    let old_key = db_key_of(&old_bytes, PW);
    s.lock().unwrap();
    assert!(Db::open(&dir.join("vault.db"), old_key).is_err());

    // confirmation was pending; the old recovery key is dead, the new one works
    expect_code(s.recover(&old_rk, PW2, LOW), "wrongCredentials");
    s.unlock(PW).unwrap(); // rotation keeps the master password
    assert_eq!(
        s.state(),
        SessionState::UnlockedPendingConfirm,
        "still unconfirmed after lock"
    );
    s.lock().unwrap();
    s.recover(&new_key, PW3, LOW).unwrap();
    assert_eq!(revealed_secret(&mut s), ITEM_PW);
    s.lock().unwrap();

    // SEC-C05: the new wraps bind epoch 2: put the OLD header back next to the new one ...
    fs::write(dir.join(&old_name), &old_bytes).unwrap();
    let mut s2 = Session::open_dir(&dir).unwrap();
    s2.unlock(PW3).unwrap();
    assert_eq!(
        header_names(&dir).len(),
        1,
        "the old header was collected on open"
    );
    s2.lock().unwrap();

    // ... and with only the old header left (the attacker deleted the new one) the vault does
    // not open: the old key does not decrypt the re-keyed file. A session that has already seen
    // epoch 2 refuses the header outright.
    let new_header = header_path(&dir);
    let new_bytes = fs::read(&new_header).unwrap();
    fs::remove_file(&new_header).unwrap();
    fs::write(dir.join(&old_name), &old_bytes).unwrap();
    let mut s3 = Session::open_dir(&dir).unwrap();
    expect_code(s3.unlock(PW), "corruptVault");
    fs::write(&new_header, &new_bytes).unwrap();
    s2.unlock(PW3).unwrap();
    s2.lock().unwrap();
    fs::remove_file(&new_header).unwrap();
    fs::write(dir.join(&old_name), &old_bytes).unwrap();
    let e = s2.unlock(PW).unwrap_err();
    assert_eq!(e.code().as_str(), "corruptVault");
    assert!(e.to_string().contains("eligible"), "{e}");

    // the plaintext canaries never appear on disk (SEC-S02)
    fs::write(&new_header, &new_bytes).unwrap();
    for f in all_files(&dir) {
        let b = fs::read(&f).unwrap();
        for c in [PW, PW2, PW3, ITEM_PW] {
            assert!(
                !b.windows(c.len()).any(|w| w == c.as_bytes()),
                "{c} in {}",
                f.display()
            );
        }
    }
}

#[test]
fn change_password_and_rotate_is_one_operation_that_replaces_every_credential() {
    let (_t, dir, old_rk) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();

    // refused before any key work
    match s.change_password_and_rotate(PW, "short", LOW) {
        Err(e) => assert_eq!(e.code().as_str(), "weakPassword"),
        Ok(_) => panic!("weak password accepted"),
    }
    let before = fs::read(header_path(&dir)).unwrap();
    // a wrong current password changes nothing and does not lock the session
    expect_code(
        s.change_password_and_rotate(WRONG, PW2, LOW),
        "wrongCredentials",
    );
    assert_eq!(fs::read(header_path(&dir)).unwrap(), before);
    assert_eq!(s.state(), SessionState::Unlocked);
    assert_eq!(item_count(&mut s), 1);
    assert_eq!(leftovers(&dir), Vec::<String>::new());

    let out = s.change_password_and_rotate(PW, PW2, LOW).unwrap();
    assert_eq!(out.record.new_epoch, 2);
    let new_key = out.recovery_key.recovery_key().to_string();
    assert!(
        s.confirm_recovery_key(answers_for(&new_key, out.recovery_key.challenge()))
            .unwrap()
    );
    assert_eq!(s.state(), SessionState::Unlocked);
    s.lock().unwrap();

    // SEC-C12 / SEC-A05 in the rotated vault: the old password and old recovery key are dead
    expect_code(s.unlock(PW), "wrongCredentials");
    expect_code(s.recover(&old_rk, PW3, LOW), "wrongCredentials");
    s.unlock(PW2).unwrap();
    assert_eq!(revealed_secret(&mut s), ITEM_PW);
    s.lock().unwrap();
    s.recover(&new_key, PW3, LOW).unwrap();
    assert_eq!(item_count(&mut s), 1);
}

#[test]
fn rotating_twice_records_both_and_keeps_the_data() {
    let (_t, dir, _rk) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    let a = s.rotate_keys(PW).unwrap();
    let b = s.rotate_keys(PW).unwrap();
    assert_eq!((a.record.old_epoch, a.record.new_epoch), (1, 2));
    assert_eq!((b.record.old_epoch, b.record.new_epoch), (2, 3));
    assert_eq!(s.rotation_record().unwrap(), Some(b.record));
    assert_eq!(header_names(&dir).len(), 1);
    assert_eq!(revealed_secret(&mut s), ITEM_PW);
    // only the newest recovery key works
    let key_b = b.recovery_key.recovery_key().to_string();
    let key_a = a.recovery_key.recovery_key().to_string();
    s.lock().unwrap();
    expect_code(s.recover(&key_a, PW2, LOW), "wrongCredentials");
    s.recover(&key_b, PW2, LOW).unwrap();
}

#[test]
fn the_new_recovery_key_must_be_confirmed_even_after_a_lock() {
    let (_t, dir, _rk) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    let out = s.rotate_keys(PW).unwrap();
    // a wrong confirmation keeps it pending
    let wrong = out
        .recovery_key
        .challenge()
        .iter()
        .map(|i| (*i, "AAAAA".to_owned()))
        .collect();
    assert!(!s.confirm_recovery_key(wrong).unwrap());
    s.lock().unwrap();
    // lost before it was confirmed: the user must generate a fresh one
    assert!(!s.status().unwrap().onboarding_complete);
    s.unlock(PW).unwrap();
    expect_code(s.pending_recovery_key(), "validation");
    let again = s.regenerate_recovery_key(PW).unwrap();
    let key = again.recovery_key().to_string();
    assert!(
        s.confirm_recovery_key(answers_for(&key, again.challenge()))
            .unwrap()
    );
    assert!(s.status().unwrap().onboarding_complete);
}

#[test]
fn rotation_needs_an_unlocked_session_and_the_password() {
    let (_t, dir, _rk) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    expect_code(s.rotate_keys(PW), "locked");
    expect_code(s.change_password_and_rotate(PW, PW2, LOW), "locked");
    expect_code(s.rotation_record(), "locked");
    s.unlock(PW).unwrap();
    expect_code(s.rotate_keys(WRONG), "wrongCredentials");
    assert_eq!(s.state(), SessionState::Unlocked);
    assert_eq!(header_names(&dir).len(), 1);
}

#[test]
fn a_failure_before_the_commit_leaves_the_vault_and_the_session_as_they_were() {
    let (_t, dir, _rk) = fixture_copy(BIN);
    let header_before = fs::read(header_path(&dir)).unwrap();
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    // a directory where the re-keyed copy must go makes step 1 fail
    fs::create_dir(dir.join("vault.db.next")).unwrap();
    assert!(s.rotate_keys(PW).is_err());
    fs::remove_dir(dir.join("vault.db.next")).unwrap();

    assert_eq!(s.state(), SessionState::Unlocked, "still unlocked");
    assert_eq!(revealed_secret(&mut s), ITEM_PW);
    assert_eq!(fs::read(header_path(&dir)).unwrap(), header_before);
    assert!(
        s.status().unwrap().onboarding_complete,
        "the marker was put back"
    );
    assert_eq!(leftovers(&dir), Vec::<String>::new());
    // and a retry works
    s.rotate_keys(PW).unwrap();
    assert_eq!(item_count(&mut s), 1);
}

#[test]
fn quick_unlock_is_switched_off_by_a_rotation_and_a_stale_blob_cannot_come_back() {
    let (_t, dir, _rk) = fixture_copy(BIN);
    let (p, h) = FakeProvider::new();
    let wall = FakeWall::at(1_800_000_000_000);
    let mut s = Session::open_dir(&dir)
        .unwrap()
        .with_provider(Box::new(p))
        .with_wall_clock(Box::new(wall.clone()));
    s.unlock(PW).unwrap();
    s.quick_unlock_enable().unwrap();
    let stale_blob = fs::read(dir.join(BLOB_FILE)).unwrap();
    let stale_policy = fs::read(dir.join(POLICY_FILE)).unwrap();

    s.rotate_keys(PW).unwrap();
    assert!(!dir.join(BLOB_FILE).exists() && !dir.join(POLICY_FILE).exists());
    assert!(h.revoke_calls() >= 1);
    assert!(!s.quick_unlock_status().enabled);
    s.lock().unwrap();
    assert_eq!(
        s.unlock_quick().unwrap_err().code().as_str(),
        "quickUnlockUnavailable"
    );

    // as if the process had died right after the commit, before the files were deleted
    fs::write(dir.join(BLOB_FILE), &stale_blob).unwrap();
    fs::write(dir.join(POLICY_FILE), &stale_policy).unwrap();
    assert_eq!(
        s.unlock_quick().unwrap_err().code().as_str(),
        "quickUnlockUnavailable"
    );
    assert_eq!(
        s.state(),
        SessionState::Locked,
        "the old vault key opens nothing"
    );
    assert!(
        !dir.join(BLOB_FILE).exists(),
        "the stale blob disabled itself"
    );
}
