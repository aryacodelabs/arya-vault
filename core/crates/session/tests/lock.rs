//! Lock semantics: SEC-A04 (lock wipes keys and closes the DB), SEC-C06 (zeroize on lock).
//!
//! The key types themselves are proven to zeroize on drop in `arya-vault-crypto`
//! (`sec_c06_*`) and `arya-vault-storage`; these tests prove that `lock()` really drops them and
//! really closes the database, from the outside (open file handles, SQLite side files) and, in
//! the unit tests of `state.rs`, by drop probes on the unlocked state and the pending key.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::path::Path;

use arya_vault_session::{Session, SessionState};
use common::*;

const BIN: &str = "lock";

/// Number of open file descriptors of this process that point at `target` (Linux only).
#[cfg(target_os = "linux")]
fn open_handles_on(target: &Path) -> usize {
    let target = std::fs::canonicalize(target).unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .flatten()
        .filter(|e| std::fs::read_link(e.path()).is_ok_and(|l| l == target))
        .count()
}

#[test]
fn sec_a04_lock_closes_the_database_and_with_vault_refuses_afterwards() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let db = dir.join("vault.db");
    let mut s = Session::open_dir(&dir).unwrap();
    s.unlock(PW).unwrap();
    assert_eq!(item_count(&mut s), 1);

    // while unlocked: the database is open and SQLite's WAL side files exist
    #[cfg(target_os = "linux")]
    assert!(
        open_handles_on(&db) >= 1,
        "unlocked session holds the DB open"
    );
    assert!(dir.join("vault.db-shm").exists());

    s.lock().unwrap();
    assert_eq!(s.state(), SessionState::Locked);

    // after lock: no handle on the database, and closing the last connection removed the WAL
    #[cfg(target_os = "linux")]
    assert_eq!(open_handles_on(&db), 0, "lock must close the DB handle");
    assert!(!dir.join("vault.db-shm").exists());
    assert!(!dir.join("vault.db-wal").exists());

    // the vault cannot be reached any more, and the closure is not even called
    let mut called = false;
    let r = s.with_vault(|_| called = true);
    assert!(!called);
    expect_code(r, "locked");

    // a second lock is a no-op; so is dropping a locked session
    s.lock().unwrap();
    drop(s);
    #[cfg(target_os = "linux")]
    assert_eq!(open_handles_on(&db), 0);
}

#[test]
fn dropping_an_unlocked_session_closes_the_database_too() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let db = dir.join("vault.db");
    {
        let mut s = Session::open_dir(&dir).unwrap();
        s.unlock(PW).unwrap();
        #[cfg(target_os = "linux")]
        assert!(open_handles_on(&db) >= 1);
    }
    #[cfg(target_os = "linux")]
    assert_eq!(open_handles_on(&db), 0, "Drop must lock the session");
    assert!(!dir.join("vault.db-shm").exists());
    let _ = &db;
}

#[test]
fn lock_is_idempotent_across_unlock_cycles() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    for _ in 0..2 {
        s.unlock(PW).unwrap();
        assert_eq!(item_count(&mut s), 1);
        s.lock().unwrap();
        s.lock().unwrap();
        expect_code(s.with_vault(|v| v.item_count()), "locked");
    }
}

#[test]
fn pending_recovery_key_does_not_survive_lock() {
    let tmp = tempfile::tempdir().unwrap();
    let mut s = Session::open_dir(tmp.path().join("v")).unwrap();
    let rk = s.create(PW, arya_vault_session::KdfProfile::Low).unwrap();
    let key = rk.recovery_key().to_string();
    assert!(s.pending_recovery_key().is_ok());
    s.lock().unwrap();
    expect_code(s.pending_recovery_key(), "locked");
    s.unlock(PW).unwrap();
    // gone for good: not recoverable from the session, onboarding still unconfirmed
    expect_code(s.pending_recovery_key(), "validation");
    expect_code(s.recovery_challenge(), "validation");
    expect_code(
        s.confirm_recovery_key(answers_for(&key, rk.challenge())),
        "validation",
    );
    assert_eq!(s.state(), SessionState::UnlockedPendingConfirm);
    assert!(!s.status().unwrap().onboarding_complete);
}
