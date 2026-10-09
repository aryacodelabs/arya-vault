//! Shared helpers. The API drives one process-wide session, so every test takes `serial()` and
//! starts from a private copy of a vault created once per test binary.

#![allow(dead_code)]

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use arya_vault_ffi::api::dto::{GroupAnswer, KdfProfile, RecoveryKeyResult};
use arya_vault_ffi::api::lifecycle;
use tempfile::TempDir;

pub const PASSWORD: &str = "CANARY-master-correct-horse-battery-staple-91";
pub const OTHER_PASSWORD: &str = "CANARY-other-purple-giraffe-dances-slowly-47";

static SERIAL: Mutex<()> = Mutex::new(());

/// Serialises the tests of one binary (they share the process-wide session).
pub fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

pub fn pw(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}

/// The answers a user would type for `r`'s challenge.
pub fn answers(r: &RecoveryKeyResult) -> Vec<GroupAnswer> {
    let key = String::from_utf8(r.recovery_key.clone()).unwrap();
    let groups: Vec<&str> = key.split('-').collect();
    r.challenge
        .iter()
        .map(|i| GroupAnswer {
            index: *i,
            text: groups[*i as usize].to_owned(),
        })
        .collect()
}

pub struct Fixture {
    pub dir: TempDir,
    pub recovery_key: Vec<u8>,
}

fn copy_dir(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
    }
}

/// Creates a confirmed, locked vault (Low KDF profile) once and returns a fresh copy of it, with
/// the session pointed at the copy and locked. Call with `serial()` held.
pub fn fresh() -> Fixture {
    static MASTER: OnceLock<(TempDir, Vec<u8>)> = OnceLock::new();
    let (master, key) = MASTER.get_or_init(|| {
        let dir = tempfile::tempdir().unwrap();
        lifecycle::lock().unwrap();
        lifecycle::init_core(dir.path().to_str().unwrap().to_owned()).unwrap();
        let r = lifecycle::create_vault(pw(PASSWORD), KdfProfile::Low).unwrap();
        assert!(lifecycle::confirm_recovery_key(answers(&r)).unwrap());
        lifecycle::lock().unwrap();
        (dir, r.recovery_key)
    });
    lifecycle::lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    copy_dir(master.path(), dir.path());
    lifecycle::init_core(dir.path().to_str().unwrap().to_owned()).unwrap();
    Fixture {
        dir,
        recovery_key: key.clone(),
    }
}

/// `fresh()` followed by an unlock.
pub fn unlocked() -> Fixture {
    let f = fresh();
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    f
}

/// A directory with no vault, session pointed at it.
pub fn empty() -> TempDir {
    lifecycle::lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    lifecycle::init_core(dir.path().to_str().unwrap().to_owned()).unwrap();
    dir
}

/// Every byte of every file in `dir`, concatenated.
pub fn disk_bytes(dir: &Path) -> Vec<u8> {
    let mut all = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        all.extend(std::fs::read(e.unwrap().path()).unwrap());
    }
    all
}

pub fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}
