//! Lifecycle and state-machine tests (SEC-A01, SEC-A05, SEC-A07, SEC-C12; docs/14 §4.1, §5).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use arya_vault_crypto::recovery_key;
use arya_vault_session::{
    KdfProfile, RecoveryConfirmation, Session, SessionError, SessionState, meta,
};
use common::*;

const BIN: &str = "lifecycle";
const LOW: KdfProfile = KdfProfile::Low;

#[test]
fn create_confirm_lock_unlock_change_password_recover() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("vault");
    let mut s = Session::open_dir(&dir).unwrap();
    assert_eq!(s.state(), SessionState::NoVault);
    let st = s.status().unwrap();
    assert_eq!(
        (
            st.exists,
            st.locked,
            st.onboarding_complete,
            st.format_version
        ),
        (false, true, false, 0)
    );
    assert!(!dir.exists(), "opening a missing directory creates nothing");

    // create -> unlocked, onboarding pending, key shown once
    let rk = s.create(PW, LOW).unwrap();
    let key = rk.recovery_key().to_string();
    assert_eq!(rk.groups(), 8);
    assert_eq!(key.split('-').count(), rk.groups());
    assert!(recovery_key::parse(&key).is_ok(), "the shown key parses");
    assert_eq!(rk.challenge().len(), 3);
    assert!(rk.challenge().windows(2).all(|w| w[0] < w[1]));
    assert!(rk.challenge().iter().all(|i| *i < 6));
    assert_eq!(rk.header_version(), 1);
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    let st = s.status().unwrap();
    assert_eq!(
        (
            st.exists,
            st.locked,
            st.onboarding_complete,
            st.format_version
        ),
        (true, false, false, 1)
    );
    assert_eq!(s.pending_recovery_key().unwrap().as_str(), key);
    assert_eq!(s.recovery_challenge().unwrap(), rk.challenge());

    // confirm: wrong, then right (typed sloppily)
    let wrong = rk
        .challenge()
        .iter()
        .map(|i| (*i, "AAAAA".to_owned()))
        .collect();
    assert!(!s.confirm_recovery_key(wrong).unwrap());
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    assert_eq!(s.pending_recovery_key().unwrap().as_str(), key, "key kept");
    let challenge = s.recovery_challenge().unwrap();
    assert_eq!(challenge.len(), 3);
    assert!(
        s.confirm_recovery_key(answers_for(&key, &challenge))
            .unwrap()
    );
    assert_eq!(s.state(), SessionState::Unlocked);
    assert!(s.status().unwrap().onboarding_complete);
    assert!(!meta::onboarding_pending(&dir));
    expect_code(s.pending_recovery_key(), "validation");

    // data
    s.with_vault(|v| {
        v.create_item(
            arya_vault_vault::NewItem::new(arya_vault_vault::ItemType::Login, ITEM_TITLE)
                .with_field(arya_vault_vault::StdField::Password, ITEM_PW),
        )
    })
    .unwrap()
    .unwrap();

    // lock -> unlock (wrong, right)
    s.lock().unwrap();
    assert_eq!(s.state(), SessionState::Locked);
    let st = s.status().unwrap();
    assert_eq!(
        (st.exists, st.locked, st.onboarding_complete),
        (true, true, true)
    );
    expect_code(s.unlock(WRONG), "wrongCredentials");
    assert_eq!(s.state(), SessionState::Locked);
    s.unlock(PW).unwrap();
    assert_eq!(item_count(&mut s), 1);

    // change password without the recovery key (SEC-C12, SEC-A05)
    assert_eq!(s.change_password(PW, PW2, LOW).unwrap(), 2);
    assert_eq!(s.state(), SessionState::Unlocked, "stays unlocked");
    s.lock().unwrap();
    expect_code(s.unlock(PW), "wrongCredentials");
    s.unlock(PW2).unwrap();
    assert_eq!(item_count(&mut s), 1, "no data was re-encrypted or lost");
    s.lock().unwrap();

    // the recovery key still works after the password change; leaves the vault unlocked
    assert_eq!(s.recover(&key, PW3, LOW).unwrap(), 3);
    assert_eq!(s.state(), SessionState::Unlocked);
    assert_eq!(item_count(&mut s), 1);
    s.lock().unwrap();
    expect_code(s.unlock(PW2), "wrongCredentials");
    s.unlock(PW3).unwrap();

    // exactly one header file remains (superseded ones are removed), no temp files
    s.lock().unwrap();
    let names: Vec<_> = all_files(&dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .filter(|n| n.starts_with("header-"))
        .collect();
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(
        names[0].starts_with("header-00000001-00000003-"),
        "{names:?}"
    );
}

#[test]
fn regenerate_recovery_key_needs_the_password_and_a_new_confirmation() {
    let (_t, dir, old_key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    assert_eq!(s.state(), SessionState::Unlocked);

    expect_code(s.regenerate_recovery_key(WRONG), "wrongCredentials");
    let rk = s.regenerate_recovery_key(PW).unwrap();
    let new_key = rk.recovery_key().to_string();
    assert_ne!(new_key, old_key);
    assert_eq!(rk.header_version(), 2);
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    assert!(!s.status().unwrap().onboarding_complete);
    assert!(meta::onboarding_pending(&dir));

    // lock before confirming: the pending key is gone and the flag stays "not confirmed"
    s.lock().unwrap();
    assert!(!s.status().unwrap().onboarding_complete);
    s.unlock(PW).unwrap();
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    expect_code(s.pending_recovery_key(), "validation");
    expect_code(s.confirm_recovery_key(vec![]), "validation");

    // the user must regenerate again, then confirm
    let rk2 = s.regenerate_recovery_key(PW).unwrap();
    let key2 = rk2.recovery_key().to_string();
    assert!(
        s.confirm_recovery_key(answers_for(&key2, rk2.challenge()))
            .unwrap()
    );
    assert!(s.status().unwrap().onboarding_complete);
    s.lock().unwrap();

    // the old key stopped working, the newest one works
    expect_code(s.recover(&old_key, PW2, LOW), "wrongCredentials");
    expect_code(s.recover(&new_key, PW2, LOW), "wrongCredentials");
    s.recover(&key2, PW2, LOW).unwrap();
}

#[test]
fn recover_and_regenerate_publishes_once_and_hands_over_a_confirmable_key() {
    let (_t, dir, old_key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    let rk = s
        .recover_and_regenerate(&old_key, PW2, LOW, RecoveryConfirmation::Required)
        .unwrap();
    assert_eq!(rk.header_version(), 2, "one publish for both changes");
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    let key = rk.recovery_key().to_string();
    assert!(
        s.confirm_recovery_key(answers_for(&key, rk.challenge()))
            .unwrap()
    );
    s.lock().unwrap();
    expect_code(s.recover(&old_key, PW3, LOW), "wrongCredentials");
}

#[test]
fn state_machine_violations_return_locked_and_friends() {
    // NoVault
    let tmp = tempfile::tempdir().unwrap();
    let mut s = Session::open_dir(tmp.path().join("none")).unwrap();
    let well_formed_key = recovery_key::encode(
        &recovery_key::generate(&mut arya_vault_crypto::rng::OsRng).unwrap(),
    );
    expect_code(s.with_vault(|_| ()), "locked");
    expect_code(s.unlock(PW), "notFound");
    expect_code(s.change_password(PW, PW2, LOW), "notFound");
    expect_code(s.recover(&well_formed_key, PW2, LOW), "notFound");
    // input validation comes first: a typo is reported even without a vault
    expect_code(s.recover("x", PW2, LOW), "recoveryKeyMalformed");
    expect_code(s.change_password(PW, "short", LOW), "weakPassword");
    expect_code(s.check_header_password(PW), "notFound");
    expect_code(s.header_info(), "notFound");
    expect_code(s.regenerate_recovery_key(PW), "locked");
    expect_code(s.verify_password(PW), "locked");
    expect_code(s.confirm_recovery_key(vec![]), "locked");
    expect_code(s.recovery_challenge(), "locked");
    expect_code(s.pending_recovery_key(), "locked");
    expect_code(s.db_info(), "locked");
    s.lock().unwrap();
    assert_eq!(s.state(), SessionState::NoVault);

    // Locked
    let (_t, dir, key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    assert_eq!(s.state(), SessionState::Locked);
    expect_code(s.with_vault(|_| ()), "locked");
    expect_code(s.create(PW, LOW), "alreadyExists");
    expect_code(s.regenerate_recovery_key(PW), "locked");
    expect_code(s.verify_password(PW), "locked");
    expect_code(s.confirm_recovery_key(vec![]), "locked");
    expect_code(s.db_info(), "locked");

    // malformed recovery key and weak passwords fail before any key derivation
    expect_code(
        s.recover("not a recovery key", PW2, LOW),
        "recoveryKeyMalformed",
    );
    expect_code(s.recover(&key, "short", LOW), "weakPassword");
    expect_code(s.change_password(PW, "password1234", LOW), "weakPassword");

    // Unlocked
    s.unlock(PW).unwrap();
    expect_code(s.unlock(PW), "validation");
    expect_code(s.recover(&key, PW2, LOW), "validation");
    expect_code(s.create(PW, LOW), "alreadyExists");
    s.lock().unwrap();
    s.lock().unwrap();
    expect_code(s.with_vault(|_| ()), "locked");
}

#[test]
fn sec_a07_weak_master_passwords_are_rejected_everywhere() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = Session::open_dir(tmp.path().join("v")).unwrap();
    for weak in [
        "",
        "short",
        "password1234",
        "aaaaaaaaaaaaaaaa",
        "11111111111111",
    ] {
        match s.create(weak, LOW) {
            Err(SessionError::WeakPassword(reasons)) => {
                assert!(!reasons.is_empty(), "{weak:?}");
            }
            other => panic!("{weak:?} accepted or wrong error: {other:?}"),
        }
    }
    assert_eq!(s.state(), SessionState::NoVault, "nothing was written");
    assert!(!tmp.path().join("v").exists());
}

#[test]
fn verify_password_reports_true_false_without_changing_state() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    assert!(s.verify_password(PW).unwrap());
    assert!(!s.verify_password(WRONG).unwrap());
    assert_eq!(s.state(), SessionState::Unlocked);
}

#[test]
fn header_only_directories_can_be_checked_and_recovered_but_not_unlocked() {
    let (_t, dir, key) = fixture_copy(BIN);
    fs::remove_file(dir.join("vault.db")).unwrap();
    let _ = fs::remove_file(dir.join("vault.db-wal"));
    let _ = fs::remove_file(dir.join("vault.db-shm"));
    let mut s = Session::open_dir(&dir).unwrap();
    assert!(!s.has_database());
    s.check_header_password(PW).unwrap();
    expect_code(s.check_header_password(WRONG), "wrongCredentials");
    expect_code(s.unlock(PW), "corruptVault");
    // the recovery key resets the password; with no database the session stays locked
    s.recover(&key, PW2, LOW).unwrap();
    assert_eq!(s.state(), SessionState::Locked);
    s.check_header_password(PW2).unwrap();
}

#[test]
fn onboarding_marker_is_the_only_extra_file_and_cli_style_creation_has_none() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("v");
    let mut s = Session::open_dir(&dir).unwrap();
    let rk = s
        .create_with(PW, LOW, RecoveryConfirmation::NotRequired)
        .unwrap();
    assert!(rk.challenge().is_empty());
    assert_eq!(s.state(), SessionState::Unlocked);
    assert!(s.status().unwrap().onboarding_complete);
    assert!(!meta::onboarding_pending(&dir));
    expect_code(s.pending_recovery_key(), "validation");
    let mut names: Vec<_> = all_files(&dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_str().unwrap().to_owned())
        .collect();
    names.sort();
    assert!(names.iter().any(|n| n == "vault.db"));
    assert_eq!(names.iter().filter(|n| n.starts_with("header-")).count(), 1);
    assert!(!names.iter().any(|n| n.contains("onboarding")));
}

#[test]
fn info_works_locked_and_unlocked() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    let h = s.header_info().unwrap();
    assert_eq!((h.format_version, h.header_version, h.epoch), (1, 1, 1));
    assert_eq!((h.kdf_m_kib, h.kdf_t, h.kdf_p), (65536, 3, 1));
    s.unlock(PW).unwrap();
    let d = s.db_info().unwrap();
    assert!(d.schema_version >= 1);
    assert_eq!(d.pinned_settings.len(), 10);
    assert!(
        d.pinned_settings.iter().all(|(_, v)| v.is_some()),
        "{:?}",
        d.pinned_settings
    );
}
