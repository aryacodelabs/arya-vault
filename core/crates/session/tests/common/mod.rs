//! Shared helpers for the session integration tests.
//!
//! Argon2 dominates test time in debug builds, so each test binary creates **one** fixture vault
//! (create, confirm, one item) and every test starts from a copy of that closed directory.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use arya_vault_session::{BackoffPolicy, Clock, KdfProfile, Session, SessionConfig, SessionError};
use arya_vault_vault::{ItemType, NewItem, StdField};

pub const PW: &str = "CANARY-master-password-correct horse battery staple";
pub const PW2: &str = "CANARY-second-master-password-tr0ub4dor&3-xyz";
pub const PW3: &str = "CANARY-third-master-password-after-recovery-91";
pub const WRONG: &str = "CANARY-not-the-password-1234567890";
pub const ITEM_PW: &str = "CANARY-item-password-hunter2-S3CR3T";
pub const ITEM_TITLE: &str = "CANARY Bank";

/// A clock the test moves by hand.
#[derive(Clone, Default)]
pub struct FakeClock(pub Arc<AtomicU64>);

impl FakeClock {
    pub fn advance(&self, ms: u64) {
        self.0.fetch_add(ms, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

pub fn config_with(clock: &FakeClock, policy: BackoffPolicy) -> SessionConfig {
    SessionConfig {
        clock: Box::new(clock.clone()),
        backoff: policy,
    }
}

pub struct Fixture {
    pub dir: PathBuf,
    pub recovery_key: String,
}

/// Parses `answers` for exactly the groups of `challenge` from a displayed recovery key.
pub fn answers_for(key_text: &str, challenge: &[usize]) -> Vec<(usize, String)> {
    let groups: Vec<&str> = key_text.split('-').collect();
    challenge
        .iter()
        .map(|i| (*i, groups[*i].to_owned()))
        .collect()
}

fn build_fixture(name: &str) -> Fixture {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = fs::remove_dir_all(&dir);
    let mut s = Session::open_dir(&dir).unwrap();
    let rk = s.create(PW, KdfProfile::Low).unwrap();
    let text = rk.recovery_key().to_string();
    assert!(
        s.confirm_recovery_key(answers_for(&text, rk.challenge()))
            .unwrap()
    );
    s.with_vault(|v| {
        v.create_item(
            NewItem::new(ItemType::Login, ITEM_TITLE).with_field(StdField::Password, ITEM_PW),
        )
    })
    .unwrap()
    .unwrap();
    s.lock().unwrap();
    Fixture {
        dir,
        recovery_key: text,
    }
}

static FIXTURE: OnceLock<Fixture> = OnceLock::new();

/// A fresh temp directory holding a copy of the fixture vault (locked, onboarding complete,
/// one item, password [`PW`]). Returns the temp guard, the vault path and the recovery key.
pub fn fixture_copy(binary: &str) -> (tempfile::TempDir, PathBuf, String) {
    let fx = FIXTURE.get_or_init(|| build_fixture(&format!("fixture-{binary}")));
    let tmp = tempfile::tempdir().unwrap();
    let dst = tmp.path().join("vault");
    copy_dir(&fx.dir, &dst);
    (tmp, dst, fx.recovery_key.clone())
}

pub fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.path().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            fs::copy(e.path(), to).unwrap();
        }
    }
}

pub fn all_files(dir: &Path) -> Vec<PathBuf> {
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

pub fn header_path(dir: &Path) -> PathBuf {
    all_files(dir)
        .into_iter()
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("header-")
        })
        .unwrap()
}

pub fn item_count(s: &mut Session) -> u64 {
    s.with_vault(|v| v.item_count()).unwrap().unwrap()
}

#[track_caller]
pub fn expect_code(r: Result<impl std::fmt::Debug, SessionError>, code: &str) {
    match r {
        Ok(v) => panic!("expected `{code}`, got Ok({v:?})"),
        Err(e) => assert_eq!(e.code().as_str(), code, "{e:?}"),
    }
}
