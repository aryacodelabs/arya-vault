//! Two sessions on one directory.
//!
//! **Chosen behaviour (SQLite locking, no lock file).** The database is in WAL mode, so a second
//! process or session can open the same directory and unlock it; SQLite arbitrates writers (one
//! at a time, `busy_timeout`, then `StorageError::Busy` which maps to the `busy` code). Header
//! operations are atomic renames, so a reader sees the old or the new header, never a torn file.
//!
//! **Why no lock file.** A portable advisory lock needs `File::lock` (Rust 1.89, above this
//! workspace's MSRV 1.88) or a new dependency; a create-exclusive lock file goes stale after a
//! crash and would lock a user out of their own vault. A second unlocked session is therefore
//! *allowed but unsupported*: both would share one device id and one persisted clock, which is
//! unsafe once sync exists. The FFI layer must hold exactly one `Session` per directory. This is
//! raised as a spec question in the A01 PR; these tests pin the current behaviour.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use arya_vault_session::{AppErrorCode, KdfProfile, Session, SessionError};
use arya_vault_storage::StorageError;
use arya_vault_vault::{ItemType, NewItem};
use common::*;

const BIN: &str = "concurrency";

#[test]
fn a_second_session_can_open_and_sees_committed_writes() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut a = Session::open_dir(&dir).unwrap();
    let mut b = Session::open_dir(&dir).unwrap();
    a.unlock(PW).unwrap();
    b.unlock(PW).unwrap();
    assert_eq!(item_count(&mut a), 1);
    a.with_vault(|v| v.create_item(NewItem::new(ItemType::Note, "CANARY note")))
        .unwrap()
        .unwrap();
    assert_eq!(item_count(&mut b), 2, "WAL readers see committed data");
    // locking one does not close the other's database
    a.lock().unwrap();
    assert_eq!(item_count(&mut b), 2);
}

#[test]
fn creating_twice_in_one_directory_is_refused() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut a = Session::open_dir(&dir).unwrap();
    expect_code(a.create(PW, KdfProfile::Low), "alreadyExists");
    // a session opened before the vault existed also notices it is there at create time
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("v");
    let mut early = Session::open_dir(&p).unwrap();
    let mut other = Session::open_dir(&p).unwrap();
    other.create(PW, KdfProfile::Low).unwrap();
    expect_code(early.create(PW2, KdfProfile::Low), "alreadyExists");
}

#[test]
fn a_header_change_by_another_session_is_picked_up_on_the_next_unlock() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut a = Session::open_dir(&dir).unwrap();
    let mut b = Session::open_dir(&dir).unwrap();
    a.unlock(PW).unwrap();
    b.change_password(PW, PW2, KdfProfile::Low).unwrap();
    // A is still unlocked (the vault key did not change), and a fresh unlock needs the new one
    assert_eq!(item_count(&mut a), 1);
    a.lock().unwrap();
    expect_code(a.unlock(PW), "wrongCredentials");
    a.unlock(PW2).unwrap();
}

#[test]
fn busy_storage_errors_map_to_the_busy_code() {
    let e = SessionError::from(StorageError::Busy);
    assert_eq!(e.code(), AppErrorCode::Busy);
    assert_eq!(e.code().as_str(), "busy");
}
