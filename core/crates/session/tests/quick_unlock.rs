//! Quick-unlock policy matrix with the fake provider and a fake wall clock
//! (SEC-A02 as far as it is testable without hardware, SEC-A03, SEC-A04, SEC-C06).
//!
//! Hardware binding (the blob is useless without the platform key, `unseal` demands user
//! presence) is a property of each real provider and is verified per platform in L01 / M3 / M6;
//! nothing here can show it.
//!
//! Argon2 dominates test time, so each test unlocks **one** session with the password (`s1`,
//! which enables quick unlock and stays open) and drives the scenarios through further, locked
//! sessions (`fresh()`) that share the same provider object, clock and directory.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use arya_vault_crypto::keys::VaultKey;
use arya_vault_session::quick::fake::{Behavior, FakeHandle, FakeProvider};
use arya_vault_session::quick::policy::{BLOB_FILE, POLICY_FILE};
use arya_vault_session::{
    Blob, PolicyRecord, ProviderError, QuickUnlockConfig, QuickUnlockDenied, QuickUnlockKind,
    QuickUnlockProvider, Session, SessionError, SessionState,
};
use common::*;

const BIN: &str = "quick_unlock";
const HOUR: u64 = 3_600_000;
const T0: u64 = 1_800_000_000_000;

/// Lets several sessions use one provider (one platform key), like several app launches.
struct Shared(Arc<FakeProvider>);

impl QuickUnlockProvider for Shared {
    fn kind(&self) -> QuickUnlockKind {
        self.0.kind()
    }
    fn available(&self) -> bool {
        self.0.available()
    }
    fn seal(&self, vk: &VaultKey) -> Result<Blob, ProviderError> {
        self.0.seal(vk)
    }
    fn unseal(&self, blob: &Blob) -> Result<VaultKey, ProviderError> {
        self.0.unseal(blob)
    }
    fn boot_id(&self) -> Option<Vec<u8>> {
        self.0.boot_id()
    }
    fn revoke(&self) {
        self.0.revoke();
    }
}

struct Rig {
    _tmp: tempfile::TempDir,
    dir: PathBuf,
    provider: Arc<FakeProvider>,
    h: FakeHandle,
    wall: FakeWall,
    cfg: QuickUnlockConfig,
}

impl Rig {
    fn new() -> Self {
        Self::with_config(QuickUnlockConfig::default())
    }

    fn with_config(cfg: QuickUnlockConfig) -> Self {
        let (tmp, dir, _key) = fixture_copy(BIN);
        let (p, h) = FakeProvider::new();
        Self {
            _tmp: tmp,
            dir,
            provider: Arc::new(p),
            h,
            wall: FakeWall::at(T0),
            cfg,
        }
    }

    /// A new, locked session over the same directory, provider and clock.
    fn fresh(&self) -> Session {
        Session::open_dir(&self.dir)
            .unwrap()
            .with_provider(Box::new(Shared(Arc::clone(&self.provider))))
            .with_wall_clock(Box::new(self.wall.clone()))
            .with_quick_unlock_config(self.cfg)
    }

    /// Password-unlocked session with quick unlock enabled. Keep it alive.
    fn s1(&self) -> Session {
        let mut s = self.fresh();
        s.unlock(PW).unwrap();
        s.quick_unlock_enable().unwrap();
        s
    }

    fn record(&self) -> PolicyRecord {
        PolicyRecord::decode(&fs::read(self.dir.join(POLICY_FILE)).unwrap()).unwrap()
    }

    fn files_exist(&self) -> (bool, bool) {
        (
            self.dir.join(POLICY_FILE).exists(),
            self.dir.join(BLOB_FILE).exists(),
        )
    }
}

#[track_caller]
fn expect_denied(r: Result<(), SessionError>, why: QuickUnlockDenied) {
    match r {
        Err(SessionError::QuickUnlockUnavailable(d)) => assert_eq!(d, why),
        other => panic!("expected denial {why:?}, got {other:?}"),
    }
}

#[test]
fn enable_then_quick_unlock_opens_the_vault_like_a_password_would() {
    let rig = Rig::new();
    let mut s1 = rig.fresh();
    let st = s1.quick_unlock_status();
    assert!(st.supported && !st.enabled);
    assert_eq!(st.kind, QuickUnlockKind::WindowsHello);
    s1.unlock(PW).unwrap();
    s1.quick_unlock_enable().unwrap();
    let st = s1.quick_unlock_status();
    assert!(st.enabled);
    assert_eq!(st.password_required, None);
    let rec = rig.record();
    assert_eq!(rec.failure_count, 0);
    assert_eq!(rec.last_password_unlock_ms, T0);
    assert_eq!(rec.boot_id.as_deref(), Some(&b"boot-1"[..]));

    let mut s2 = rig.fresh();
    // status works while locked, from the plaintext record only
    assert!(s2.status().unwrap().quick_unlock.enabled);
    rig.wall.advance(HOUR);
    s2.unlock_quick().unwrap();
    assert_eq!(s2.state(), SessionState::Unlocked);
    assert_eq!(item_count(&mut s2), 1, "same vault, same data");
    let rec = rig.record();
    assert_eq!(rec.last_quick_unlock_ms, T0 + HOUR);
    assert_eq!(
        rec.last_password_unlock_ms, T0,
        "a quick unlock is not a password unlock"
    );
    assert_eq!(rig.h.unseal_calls(), 1);
}

#[test]
fn sec_a03_reboot_denies_until_the_password_is_used_again() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    rig.h.set_boot_id(Some(b"boot-2"));
    let mut s2 = rig.fresh();
    let r = s2.unlock_quick();
    expect_denied(r, QuickUnlockDenied::Rebooted);
    assert_eq!(rig.h.unseal_calls(), 0, "the provider is not even asked");
    assert_eq!(s2.state(), SessionState::Locked);
    let st = s2.quick_unlock_status();
    assert!(st.enabled);
    assert_eq!(st.password_required, Some(QuickUnlockDenied::Rebooted));
    assert_eq!(
        SessionError::QuickUnlockUnavailable(QuickUnlockDenied::Rebooted)
            .code()
            .as_str(),
        "quickUnlockUnavailable"
    );
    // the password unlock records the new boot; quick unlock works again until the next reboot
    rig.wall.advance(HOUR);
    s2.unlock(PW).unwrap();
    assert_eq!(rig.record().boot_id.as_deref(), Some(&b"boot-2"[..]));
    assert_eq!(rig.record().last_password_unlock_ms, T0 + HOUR);
    s2.lock().unwrap();
    s2.unlock_quick().unwrap();
}

#[test]
fn reboot_check_can_be_turned_off() {
    let rig = Rig::with_config(QuickUnlockConfig {
        require_password_after_reboot: false,
        ..QuickUnlockConfig::default()
    });
    let _s1 = rig.s1();
    rig.h.set_boot_id(Some(b"boot-2"));
    let mut s2 = rig.fresh();
    s2.unlock_quick().unwrap();
}

#[test]
fn sec_a03_72_hours_from_the_last_password_unlock_and_a_quick_unlock_does_not_extend_it() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();

    rig.wall.set(T0 + 71 * HOUR);
    s2.unlock_quick().unwrap();
    s2.lock().unwrap();
    // 71 h in, a successful quick unlock just happened; at 73 h it is still too old
    rig.wall.set(T0 + 73 * HOUR);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Expired);
    assert_eq!(
        s2.quick_unlock_status().password_required,
        Some(QuickUnlockDenied::Expired)
    );

    // the password restarts the clock
    s2.unlock(PW).unwrap();
    s2.lock().unwrap();
    rig.wall.set(T0 + 73 * HOUR + 71 * HOUR);
    s2.unlock_quick().unwrap();
}

#[test]
fn the_limit_is_exactly_72_hours() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    rig.wall.set(T0 + 72 * HOUR);
    s2.unlock_quick().unwrap();
    s2.lock().unwrap();
    rig.wall.set(T0 + 72 * HOUR + 1);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Expired);
}

#[test]
fn sec_a03_a_clock_set_back_counts_as_expired() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    // earlier than the moment quick unlock was enabled
    rig.wall.set(T0 - HOUR);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::ClockRolledBack);
    // earlier than the newest reading after a successful unlock
    rig.wall.set(T0 + 10 * HOUR);
    s2.unlock_quick().unwrap();
    s2.lock().unwrap();
    rig.wall.set(T0 + 5 * HOUR);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::ClockRolledBack);
    assert_eq!(s2.state(), SessionState::Locked);
    // typing the password re-baselines the clock
    s2.unlock(PW).unwrap();
    s2.lock().unwrap();
    s2.unlock_quick().unwrap();
}

#[test]
fn sec_a03_four_failures_then_success_resets_and_five_deny() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();

    rig.h.script(&[Behavior::Fail; 4]);
    for n in 1..=4 {
        expect_denied(s2.unlock_quick(), QuickUnlockDenied::Failed);
        assert_eq!(rig.record().failure_count, n);
    }
    s2.unlock_quick().unwrap();
    assert_eq!(rig.record().failure_count, 0, "success resets the counter");
    s2.lock().unwrap();

    // five in a row: the password is required, and the provider is no longer asked
    rig.h.script(&[Behavior::Fail; 5]);
    for _ in 0..5 {
        expect_denied(s2.unlock_quick(), QuickUnlockDenied::Failed);
    }
    let calls = rig.h.unseal_calls();
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::TooManyFailures);
    assert_eq!(rig.h.unseal_calls(), calls);
    assert_eq!(
        s2.quick_unlock_status().password_required,
        Some(QuickUnlockDenied::TooManyFailures)
    );

    // the password resets it
    s2.unlock(PW).unwrap();
    assert_eq!(rig.record().failure_count, 0);
    s2.lock().unwrap();
    s2.unlock_quick().unwrap();
}

#[test]
fn an_attempt_is_counted_before_the_prompt_so_killing_the_process_does_not_help() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    let dir = rig.dir.clone();
    rig.h.on_unseal(move || {
        let rec = PolicyRecord::decode(&fs::read(dir.join(POLICY_FILE)).unwrap()).unwrap();
        assert_eq!(rec.failure_count, 1, "persisted while the prompt is up");
    });
    s2.unlock_quick().unwrap();
    assert_eq!(rig.record().failure_count, 0);
}

#[test]
fn cancel_and_unavailable_are_not_failed_attempts() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    rig.h
        .script(&[Behavior::Cancel, Behavior::Unavailable, Behavior::Cancel]);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Cancelled);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::ProviderUnavailable);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Cancelled);
    assert_eq!(rig.record().failure_count, 0);
    assert_eq!(s2.state(), SessionState::Locked);
    assert_eq!(rig.files_exist(), (true, true), "still enabled");
    s2.unlock_quick().unwrap();
}

#[test]
fn sec_a02_invalidation_disables_quick_unlock_and_deletes_the_blob() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    rig.h.script(&[Behavior::Invalidate]);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Invalidated);
    assert_eq!(rig.files_exist(), (false, false));
    assert_eq!(rig.h.revoke_calls(), 1);
    let st = s2.quick_unlock_status();
    assert!(!st.enabled);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::NotEnabled);
    assert_eq!(rig.h.unseal_calls(), 1, "never asked again");
    // the password still works, and quick unlock can be switched on again
    s2.unlock(PW).unwrap();
    s2.quick_unlock_enable().unwrap();
    assert_eq!(rig.files_exist(), (true, true));
}

#[test]
fn disable_removes_both_files_revokes_and_is_idempotent_even_while_locked() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let mut s2 = rig.fresh();
    assert_eq!(s2.state(), SessionState::Locked);
    s2.quick_unlock_disable().unwrap();
    s2.quick_unlock_disable().unwrap();
    assert_eq!(rig.files_exist(), (false, false));
    assert!(rig.h.revoke_calls() >= 1);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::NotEnabled);
}

#[test]
fn enabling_twice_replaces_the_blob_and_does_not_extend_the_age_limit() {
    let rig = Rig::new();
    let mut s1 = rig.s1();
    let first = fs::read(rig.dir.join(BLOB_FILE)).unwrap();
    rig.h.script(&[Behavior::Fail, Behavior::Fail]);
    let mut s2 = rig.fresh();
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Failed);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Failed);
    assert_eq!(rig.record().failure_count, 2);

    // enabling again from the password session: new blob, counters reset, same password time
    rig.wall.advance(5 * HOUR);
    s1.quick_unlock_enable().unwrap();
    let second = fs::read(rig.dir.join(BLOB_FILE)).unwrap();
    assert_ne!(first, second, "fresh nonce, fresh blob");
    assert_eq!(rig.h.seal_calls(), 2);
    assert_eq!(rig.record().failure_count, 0);
    assert_eq!(rig.record().last_password_unlock_ms, T0);

    // a quick-unlocked session cannot buy itself a new 72 h by enabling again
    rig.wall.set(T0 + 70 * HOUR);
    s2.unlock_quick().unwrap();
    rig.wall.set(T0 + 71 * HOUR);
    s2.quick_unlock_enable().unwrap();
    assert_eq!(rig.record().last_password_unlock_ms, T0);
    s2.lock().unwrap();
    rig.wall.set(T0 + 73 * HOUR);
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::Expired);
}

#[test]
fn corrupt_truncated_oversized_foreign_and_missing_data_is_rejected_without_a_panic() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let good_blob = fs::read(rig.dir.join(BLOB_FILE)).unwrap();
    let good_policy = fs::read(rig.dir.join(POLICY_FILE)).unwrap();
    let restore = || {
        fs::write(rig.dir.join(BLOB_FILE), &good_blob).unwrap();
        fs::write(rig.dir.join(POLICY_FILE), &good_policy).unwrap();
    };

    // (what to write to the blob file, expected denial, does the provider get asked?)
    let mut flipped = good_blob.clone();
    flipped[30] ^= 1;
    let cases: Vec<(Vec<u8>, QuickUnlockDenied, bool)> = vec![
        (
            good_blob[..good_blob.len() / 2].to_vec(),
            QuickUnlockDenied::Failed,
            true,
        ),
        (good_blob[..3].to_vec(), QuickUnlockDenied::Failed, true),
        (flipped, QuickUnlockDenied::Failed, true),
        (vec![0x41; good_blob.len()], QuickUnlockDenied::Failed, true),
        (vec![], QuickUnlockDenied::BlobRejected, false),
        (vec![7; 4097], QuickUnlockDenied::BlobRejected, false),
        (vec![7; 1 << 20], QuickUnlockDenied::BlobRejected, false),
    ];
    for (bytes, want, asked) in cases {
        restore();
        fs::write(rig.dir.join(BLOB_FILE), &bytes).unwrap();
        let before = rig.h.unseal_calls();
        let mut s = rig.fresh();
        expect_denied(s.unlock_quick(), want);
        assert_eq!(
            rig.h.unseal_calls() - before,
            usize::from(asked),
            "len {}",
            bytes.len()
        );
        assert_eq!(s.state(), SessionState::Locked);
        if want == QuickUnlockDenied::BlobRejected {
            assert_eq!(
                rig.files_exist(),
                (false, false),
                "rejected data is removed"
            );
        }
    }

    // a blob that decrypts to the key of a *different* vault
    restore();
    fs::write(rig.dir.join(BLOB_FILE), rig.h.forge_blob([0x5A; 32])).unwrap();
    let mut s = rig.fresh();
    expect_denied(s.unlock_quick(), QuickUnlockDenied::BlobRejected);
    assert_eq!(s.state(), SessionState::Locked);
    assert_eq!(rig.files_exist(), (false, false));

    // missing blob with a policy record
    restore();
    fs::remove_file(rig.dir.join(BLOB_FILE)).unwrap();
    expect_denied(rig.fresh().unlock_quick(), QuickUnlockDenied::BlobRejected);

    // damaged policy record
    for bad in [
        b"garbage".to_vec(),
        good_policy[..20].to_vec(),
        vec![0; 1000],
        vec![],
    ] {
        restore();
        fs::write(rig.dir.join(POLICY_FILE), &bad).unwrap();
        expect_denied(rig.fresh().unlock_quick(), QuickUnlockDenied::BlobRejected);
        assert_eq!(rig.files_exist(), (false, false));
        assert!(!rig.fresh().quick_unlock_status().enabled);
    }
}

#[test]
fn a_provider_of_another_kind_or_none_at_all_is_not_trusted() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    // provider gone
    rig.h.set_available(false);
    let mut s = rig.fresh();
    expect_denied(s.unlock_quick(), QuickUnlockDenied::ProviderUnavailable);
    assert_eq!(
        s.quick_unlock_status().password_required,
        Some(QuickUnlockDenied::ProviderUnavailable)
    );
    assert_eq!(rig.record().failure_count, 0);
    // provider replaced by a different kind: the old blob is not offered to it
    rig.h.set_available(true);
    rig.h.set_kind(QuickUnlockKind::OsKeyring);
    expect_denied(
        rig.fresh().unlock_quick(),
        QuickUnlockDenied::ProviderChanged,
    );
    assert_eq!(rig.files_exist(), (false, false));
}

#[test]
fn the_default_provider_supports_nothing() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let mut s = Session::open_dir(&dir).unwrap();
    let st = s.status().unwrap().quick_unlock;
    assert!(!st.supported && !st.enabled);
    assert_eq!(st.kind, QuickUnlockKind::None);
    expect_denied(s.unlock_quick(), QuickUnlockDenied::NotEnabled);
    expect_code(s.quick_unlock_enable(), "locked");
    s.unlock(PW).unwrap();
    expect_denied(
        s.quick_unlock_enable(),
        QuickUnlockDenied::ProviderUnavailable,
    );
}

#[test]
fn state_machine_rules_for_quick_unlock() {
    let tmp = tempfile::tempdir().unwrap();
    let mut none = Session::open_dir(tmp.path().join("none")).unwrap();
    expect_code(none.unlock_quick(), "notFound");
    let rig = Rig::new();
    let mut s1 = rig.s1();
    expect_code(s1.unlock_quick(), "validation"); // already unlocked
    let mut s2 = rig.fresh();
    expect_code(s2.quick_unlock_enable(), "locked");
    s2.lock().unwrap();
}

#[test]
fn a_failed_quick_unlock_enable_leaves_nothing_behind() {
    let rig = Rig::new();
    let mut s = rig.fresh();
    s.unlock(PW).unwrap();
    rig.h.fail_seal(Some(ProviderError::UserCancelled));
    expect_denied(s.quick_unlock_enable(), QuickUnlockDenied::Cancelled);
    rig.h.fail_seal(Some(ProviderError::Failed));
    expect_denied(s.quick_unlock_enable(), QuickUnlockDenied::Failed);
    assert_eq!(rig.files_exist(), (false, false));
    assert!(!s.quick_unlock_status().enabled);
}

#[test]
fn sec_a04_lock_during_unlock_quick_leaves_a_locked_session_with_no_keys() {
    let rig = Rig::new();
    {
        let mut s1 = rig.s1();
        s1.lock().unwrap(); // nobody holds the database now
    }
    let mut s2 = rig.fresh();
    let handle = s2.lock_request_handle();
    // the user's auto-lock fires while the biometric prompt is up
    rig.h.on_unseal(move || handle.request_lock());
    expect_denied(s2.unlock_quick(), QuickUnlockDenied::LockRequested);
    assert_eq!(s2.state(), SessionState::Locked);
    expect_code(s2.with_vault(|v| v.item_count()), "locked");
    assert_eq!(rig.record().failure_count, 0, "not a failed attempt");
    #[cfg(target_os = "linux")]
    assert_eq!(open_handles_on(&rig.dir.join("vault.db")), 0);
    assert!(!rig.dir.join("vault.db-shm").exists());
    // the same session unlocks fine afterwards; the cancel only affected the one in flight
    rig.h.on_unseal(|| {});
    s2.unlock_quick().unwrap();
}

#[cfg(target_os = "linux")]
fn open_handles_on(target: &std::path::Path) -> usize {
    let target = fs::canonicalize(target).unwrap();
    fs::read_dir("/proc/self/fd")
        .unwrap()
        .flatten()
        .filter(|e| fs::read_link(e.path()).is_ok_and(|l| l == target))
        .count()
}

#[test]
fn sec_c06_lock_after_quick_unlock_closes_the_database() {
    let rig = Rig::new();
    {
        let mut s1 = rig.s1();
        s1.lock().unwrap();
    }
    let mut s2 = rig.fresh();
    s2.unlock_quick().unwrap();
    #[cfg(target_os = "linux")]
    assert!(open_handles_on(&rig.dir.join("vault.db")) >= 1);
    s2.lock().unwrap();
    #[cfg(target_os = "linux")]
    assert_eq!(open_handles_on(&rig.dir.join("vault.db")), 0);
    expect_code(s2.with_vault(|_| ()), "locked");
}

#[test]
fn recovery_key_unlock_counts_as_a_credential_unlock() {
    let (tmp, dir, key) = fixture_copy(BIN);
    let _keep = tmp;
    let (p, h) = FakeProvider::new();
    let wall = FakeWall::at(T0);
    let mut s = Session::open_dir(&dir)
        .unwrap()
        .with_provider(Box::new(p))
        .with_wall_clock(Box::new(wall.clone()));
    s.unlock(PW).unwrap();
    s.quick_unlock_enable().unwrap();
    s.lock().unwrap();
    wall.advance(80 * HOUR);
    expect_denied(s.unlock_quick(), QuickUnlockDenied::Expired);
    s.recover(&key, PW2, arya_vault_session::KdfProfile::Low)
        .unwrap();
    s.lock().unwrap();
    s.unlock_quick().unwrap();
    let _ = h;
}

const CANARIES: &[&str] = &[PW, ITEM_PW, "CANARY-master-password"];

#[test]
fn disk_scan_policy_record_and_blob_contain_no_vault_key_or_password() {
    let rig = Rig::new();
    let _s1 = rig.s1();
    let vk = rig.h.last_sealed_vk().unwrap();
    assert_ne!(vk, [0u8; 32]);
    let blob = fs::read(rig.dir.join(BLOB_FILE)).unwrap();
    assert_eq!(Some(blob.clone()), rig.h.last_blob());
    for f in all_files(&rig.dir) {
        let bytes = fs::read(&f).unwrap();
        assert!(
            !bytes.windows(32).any(|w| w == vk),
            "vault key bytes in plaintext in {}",
            f.display()
        );
        for c in CANARIES {
            assert!(
                !bytes.windows(c.len()).any(|w| w == c.as_bytes()),
                "`{c}` in {}",
                f.display()
            );
        }
    }
    let policy = fs::read(rig.dir.join(POLICY_FILE)).unwrap();
    assert!(policy.len() <= 76 && policy.starts_with(b"AVQP"));
    // after a quick unlock and a lock too
    let mut s2 = rig.fresh();
    s2.unlock_quick().unwrap();
    s2.lock().unwrap();
    for f in all_files(&rig.dir) {
        let bytes = fs::read(&f).unwrap();
        assert!(!bytes.windows(32).any(|w| w == vk), "{}", f.display());
    }
}

#[test]
fn no_debug_output_contains_the_blob_or_key() {
    let (p, h) = FakeProvider::new();
    let vk = VaultKey::from_bytes([0xAB; 32]);
    let blob = p.seal(&vk).unwrap();
    let dbg = format!("{blob:?}");
    assert!(dbg.contains("redacted"));
    let hex_blob: String = blob.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    assert!(!dbg.contains(&hex_blob));
    let err = SessionError::QuickUnlockUnavailable(QuickUnlockDenied::Failed);
    assert!(!format!("{err:?}{err}").contains("abab"));
    let _ = h;
}
