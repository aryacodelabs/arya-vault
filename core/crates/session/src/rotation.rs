//! Local vault-key rotation (docs/04 §10 steps 1-6; SEC-A06). The cloud steps (rotation snapshot,
//! `covers`, late-device re-emission) belong to M4.
//!
//! # Why not `PRAGMA rekey` in place
//! `Db::rekey` rewrites every page of the live file. A crash in the middle leaves a file whose
//! pages are under two different keys, which neither the old nor the new header can open. So the
//! rotation never touches `vault.db` until the very end: it re-keys a **copy**.
//!
//! # Protocol and crash argument
//! State before: header `H(E)` (epoch `E`) and `vault.db` under `K_db(VK, E)`.
//!
//! | # | step | a crash after it leaves |
//! |---|------|-------------------------|
//! | 1 | close the vault (checkpoint), copy `vault.db` to `vault.db.next` | old state (`.next` is junk) |
//! | 2 | re-key `.next` with the existing `Db::rekey`, write the new `meta` rows, `integrity_check` | old state |
//! | 3 | re-open `.next` under the new key, check the old key is refused | old state |
//! | 4 | fsync `.next` and the directory | old state |
//! | **5** | **commit: atomically rename the new header `H(E+1)` into place** | **new state** (below) |
//! | 6 | rename `.next` over `vault.db` (stale `-wal`/`-shm` removed first) | new state |
//! | 7 | delete the pre-migration backups (`vault.db.bak-v*`, encrypted under the retired key), then `H(E)` and any leftover | new state |
//!
//! * Before step 5 the active header is `H(E)` and `vault.db` is untouched, so the vault opens
//!   entirely under the old credentials; `.next` is deleted the next time the vault opens.
//! * From step 5 on the active header is `H(E+1)` (highest epoch wins). `vault.db` is either still
//!   the old file (crash between 5 and 6) or already the new one. `lifecycle::open_vault` opens
//!   `vault.db`; if that fails with a wrong key it tries `vault.db.next` **under the same key** and,
//!   only if that opens, completes step 6. A half-re-keyed `.next` can never be chosen: `H(E+1)` is
//!   written only after step 3 proved `.next` complete, and `.next` is only ever trusted together with
//!   a header that unwraps.
//! * The commit is one `rename` of a fully written, fsynced file. There is no instant at which the
//!   active header and the database that opens under it disagree, and no state in which neither
//!   header opens a database.
//!
//! **Which credentials work after a crash:** the old ones if it happened before step 5, the new
//! ones (new recovery key; the new password for "change password and rotate") if it happened at or
//! after step 5. If the new recovery key was never shown to the user, the unconfirmed-key marker
//! (written before step 5) makes the app require `regenerate_recovery_key` after the next unlock.
//!
//! **Honest limits:** SQLCipher/filesystem blocks of the old file may survive on disk after the
//! rename (no secure erase is possible from user space); a crash test covers process death, not
//! power loss or a lying disk (the files are fsynced, but that is not fault-injected).

use std::fs::{self, OpenOptions};

use arya_vault_crypto::format::header::Header;
use arya_vault_crypto::keys::VaultKey;
use arya_vault_crypto::vault_key::HeaderWraps;
use arya_vault_storage::{Db, DbKey, StorageError, Store};

use crate::error::{Result, SessionError};
use crate::layout::{ActiveHeader, VaultDir};
use crate::lifecycle::db_key;

/// `meta` key of the newest rotation record.
pub const META_ROTATION_LAST: &str = "rotation.last";
/// Prefix of the per-epoch rotation records (`rotation.<new_epoch>`).
pub const META_ROTATION_PREFIX: &str = "rotation.";
/// `meta` key of the highest key epoch this device has used (decimal text).
pub const META_HIGHEST_EPOCH: &str = "highest_epoch";

/// A record that a rotation happened, stored in the vault `meta` so that sync (M4) can build the
/// rotation snapshot. Plain numbers; no key material.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RotationRecord {
    /// The epoch that ended.
    pub old_epoch: u32,
    /// The epoch that began.
    pub new_epoch: u32,
    /// Wall-clock time of the rotation, ms since the Unix epoch.
    pub at_ms: u64,
}

impl RotationRecord {
    /// Fixed 16-byte big-endian encoding: `old_epoch`, `new_epoch`, `at_ms`.
    #[must_use]
    pub fn encode(&self) -> [u8; 16] {
        let mut out = [0u8; 16];
        out[..4].copy_from_slice(&self.old_epoch.to_be_bytes());
        out[4..8].copy_from_slice(&self.new_epoch.to_be_bytes());
        out[8..].copy_from_slice(&self.at_ms.to_be_bytes());
        out
    }

    /// Parses [`encode`](Self::encode); `None` for any other length.
    #[must_use]
    pub fn decode(bytes: &[u8]) -> Option<Self> {
        let b: [u8; 16] = bytes.try_into().ok()?;
        Some(Self {
            old_epoch: u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
            new_epoch: u32::from_be_bytes([b[4], b[5], b[6], b[7]]),
            at_ms: u64::from_be_bytes([b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]]),
        })
    }
}

/// The points between steps at which a test can "crash" a rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Step 1 done: `.next` is a copy of the old database.
    Copied,
    /// Step 2 done: `.next` is re-keyed.
    Rekeyed,
    /// Step 3 done: `.next` verified under the new key.
    Verified,
    /// Just before the commit.
    BeforeCommit,
    /// Step 5 done: the new header is in place.
    Committed,
    /// Step 6 done: `.next` is `vault.db`.
    Swapped,
    /// Step 7 done.
    Collected,
}

/// Why a rotation stopped, and whether it had already committed.
#[derive(Debug)]
pub(crate) struct Failure {
    /// `true` once the new header is in place (step 5 done).
    pub(crate) committed: bool,
    pub(crate) error: SessionError,
}

fn pre(e: impl Into<SessionError>) -> Failure {
    Failure {
        committed: false,
        error: e.into(),
    }
}

fn post(e: impl Into<SessionError>) -> Failure {
    Failure {
        committed: true,
        error: e.into(),
    }
}

fn fsync_file(path: &std::path::Path) -> std::io::Result<()> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?
        .sync_all()
}

/// Runs steps 1-7. The caller has **closed** the old vault and holds `old_vk`; `wraps` are the new
/// epoch's wraps. `hook` is called after each step; returning `Err` stops the rotation right there
/// with the files exactly as a crash would leave them (tests use this; production passes a no-op).
///
/// Returns the committed header. Does not clean up on failure: the caller decides (before the
/// commit it removes `.next`; after it, recovery completes the job on the next open).
pub(crate) fn execute(
    dir: &VaultDir,
    old: &ActiveHeader,
    old_vk: &VaultKey,
    new_vk: &VaultKey,
    wraps: &HeaderWraps,
    record: &RotationRecord,
    hook: &mut dyn FnMut(Step) -> Result<()>,
) -> std::result::Result<Header, Failure> {
    let vault_id = old.header.vault_id;
    let new_epoch = wraps.epoch;
    let old_key = db_key(old_vk, &vault_id, old.header.epoch).map_err(pre)?;
    let new_key = db_key(new_vk, &vault_id, new_epoch).map_err(pre)?;
    let new_version = old
        .header
        .header_version
        .checked_add(1)
        .ok_or(SessionError::Internal("header version overflow"))
        .map_err(pre)?;

    // 1. copy (the old vault is closed, so its WAL is empty and the file is complete)
    dir.remove_next();
    let wal = dir.root().join(format!("{}-wal", crate::layout::DB_FILE));
    if fs::metadata(&wal).is_ok_and(|m| m.len() > 0) {
        return Err(pre(SessionError::Internal(
            "the database has uncheckpointed changes; another session is writing to it",
        )));
    }
    fs::copy(dir.db_path(), dir.next_db_path()).map_err(pre)?;
    hook(Step::Copied).map_err(pre)?;

    // 2. re-key the copy with the existing `Db::rekey`, then record the new epoch in its meta
    {
        let mut db = Db::open(&dir.next_db_path(), old_key.duplicate()).map_err(pre)?;
        db.rekey(&old_key, &new_key).map_err(pre)?;
        db.with_tx(|tx| -> std::result::Result<(), StorageError> {
            tx.meta_set("epoch", new_epoch.to_string().as_bytes())?;
            tx.meta_set("header_version", new_version.to_string().as_bytes())?;
            tx.meta_set(META_HIGHEST_EPOCH, new_epoch.to_string().as_bytes())?;
            let rec = record.encode();
            tx.meta_set(META_ROTATION_LAST, &rec)?;
            tx.meta_set(&format!("{META_ROTATION_PREFIX}{new_epoch}"), &rec)
        })
        .map_err(pre)?;
        db.integrity_check().map_err(pre)?;
        db.close().map_err(pre)?;
    }
    hook(Step::Rekeyed).map_err(pre)?;

    // 3. the copy must open under the new key and be refused under the old one
    {
        let mut db = Db::open(&dir.next_db_path(), new_key.duplicate()).map_err(pre)?;
        db.integrity_check().map_err(pre)?;
        let epoch = db
            .with_read(|tx| -> std::result::Result<Option<Vec<u8>>, StorageError> {
                tx.meta_get("epoch")
            })
            .map_err(pre)?;
        if epoch.as_deref() != Some(new_epoch.to_string().as_bytes()) {
            return Err(pre(SessionError::Internal(
                "re-keyed database has the wrong epoch",
            )));
        }
        db.close().map_err(pre)?;
    }
    if Db::open(&dir.next_db_path(), old_key.duplicate()).is_ok() {
        return Err(pre(SessionError::Internal(
            "re-keyed database still opens under the old key",
        )));
    }
    hook(Step::Verified).map_err(pre)?;

    // 4. make the copy durable before the header can point at it
    fsync_file(&dir.next_db_path()).map_err(pre)?;
    dir.sync_dir();
    hook(Step::BeforeCommit).map_err(pre)?;

    // 5. COMMIT: one atomic rename of a fully written header
    let header = Header::new(
        vault_id,
        new_version,
        new_epoch,
        wraps.kdf.clone(),
        wraps.wrap_pw.clone(),
        wraps.wrap_rk.clone(),
        old.header.created_at,
    );
    dir.write_header(&header, &old.device_id).map_err(pre)?;
    hook(Step::Committed).map_err(post)?;

    // 6. the database follows the header
    dir.swap_next_into_place().map_err(post)?;
    hook(Step::Swapped).map_err(post)?;

    // 7. the old header and anything else left over
    dir.collect_rotation_leftovers(&ActiveHeader {
        header: header.clone(),
        device_id: old.device_id,
    });
    hook(Step::Collected).map_err(post)?;
    Ok(header)
}

/// Probes whether `path` is a database that opens under `key` (used by recovery).
pub(crate) fn opens_under(path: &std::path::Path, key: &DbKey) -> bool {
    match Db::open(path, key.duplicate()) {
        Ok(db) => db.close().is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    //! Crash-safety proof for the protocol above, without Argon2: the vault key of epoch `n` is a
    //! known function of `n`, and the wraps in the headers are placeholders (their cryptography is
    //! covered by `arya-vault-crypto`; what is under test here is which files exist and which
    //! database opens under which header). `tests/rotation.rs` runs the real API end to end.

    use std::io::{BufRead, BufReader, Write};
    use std::path::Path;
    use std::process::{Command, Stdio};

    use arya_vault_crypto::kdf::KdfParams;
    use arya_vault_crypto::wrap::WrappedKey;
    use arya_vault_vault::{ItemType, NewItem, Vault};

    use super::*;
    use crate::layout::Seen;
    use crate::lifecycle;

    const VAULT_ID: [u8; 16] = [0x77; 16];
    const DEVICE: [u8; 16] = [0x11; 16];
    const CHILD_ENV: &str = "ARYA_ROTATION_CHILD_DIR";
    const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };

    fn vk_for(epoch: u32) -> VaultKey {
        let mut b = [0x33u8; 32];
        b[..4].copy_from_slice(&epoch.to_be_bytes());
        VaultKey::from_bytes(b)
    }

    fn placeholder_wraps(epoch: u32) -> HeaderWraps {
        let w = |b: u8| WrappedKey {
            nonce: [b; 24],
            ct: [b ^ 0x55; 48],
        };
        HeaderWraps {
            vault_id: VAULT_ID,
            epoch,
            kdf: KdfParams::floor([epoch as u8; 16]),
            wrap_pw: w(epoch as u8),
            wrap_rk: w(!(epoch as u8)),
        }
    }

    /// A fresh vault directory at epoch 1 holding `items` items.
    fn make_vault(root: &Path, items: usize) -> VaultDir {
        let dir = VaultDir::new(root.join("v"));
        dir.ensure_dir().unwrap();
        let w = placeholder_wraps(1);
        let header = Header::new(
            VAULT_ID,
            1,
            1,
            w.kdf.clone(),
            w.wrap_pw.clone(),
            w.wrap_rk.clone(),
            1_700_000_000,
        );
        dir.write_header(&header, &DEVICE).unwrap();
        let key = db_key(&vk_for(1), &VAULT_ID, 1).unwrap();
        let db = Db::create(
            &dir.db_path(),
            key,
            &arya_vault_storage::CreateParams {
                vault_id: VAULT_ID,
                device_id: DEVICE,
                epoch: 1,
                header_version: 1,
            },
        )
        .unwrap();
        let mut v = Vault::open(db, Box::new(arya_vault_vault::SystemClock)).unwrap();
        for i in 0..items {
            v.create_item(NewItem::new(ItemType::Note, &format!("CANARY note {i}")))
                .unwrap();
        }
        v.close().unwrap();
        dir
    }

    /// "Restart": pick the active header with a fresh memory and open its database the way
    /// `Session::unlock` does (including finishing an interrupted rotation).
    fn restart(dir: &VaultDir) -> (ActiveHeader, Vault) {
        let mut seen = Seen::default();
        let active = dir.active_header(&mut seen).unwrap();
        let vk = vk_for(active.header.epoch);
        let v = lifecycle::open_vault(dir, &vk, &active, &mut seen).unwrap();
        (active, v)
    }

    fn rotate(
        dir: &VaultDir,
        hook: &mut dyn FnMut(Step) -> Result<()>,
    ) -> std::result::Result<Header, Failure> {
        let mut seen = Seen::default();
        let active = dir.active_header(&mut seen).unwrap();
        let e = active.header.epoch;
        let wraps = placeholder_wraps(e + 1);
        let rec = RotationRecord {
            old_epoch: e,
            new_epoch: e + 1,
            at_ms: 42,
        };
        execute(dir, &active, &vk_for(e), &vk_for(e + 1), &wraps, &rec, hook)
    }

    fn header_epochs(dir: &VaultDir) -> Vec<u32> {
        dir.header_files()
            .unwrap()
            .iter()
            .map(|n| match crate::layout::parse_name(n) {
                Some(e) => e,
                None => panic!("bad header name {n}"),
            })
            .collect()
    }

    fn check_integrity(dir: &VaultDir, epoch: u32) {
        let key = db_key(&vk_for(epoch), &VAULT_ID, epoch).unwrap();
        let db = Db::open(&dir.db_path(), key).unwrap();
        db.integrity_check().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn record_round_trips_and_rejects_other_lengths() {
        let r = RotationRecord {
            old_epoch: 3,
            new_epoch: 4,
            at_ms: 1_800_000_000_123,
        };
        assert_eq!(RotationRecord::decode(&r.encode()), Some(r));
        assert_eq!(RotationRecord::decode(&[0; 15]), None);
        assert_eq!(RotationRecord::decode(&[0; 17]), None);
        assert_eq!(RotationRecord::decode(&[]), None);
    }

    #[test]
    fn a_rotation_without_interruption_swaps_everything() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 3);
        let h = rotate(&dir, &mut |_| Ok(())).unwrap();
        assert_eq!((h.epoch, h.header_version), (2, 2));
        assert_eq!(header_epochs(&dir), [2]);
        assert!(!dir.next_exists());
        let (active, mut v) = restart(&dir);
        assert_eq!(active.header.epoch, 2);
        assert_eq!(v.item_count().unwrap(), 3);
        v.close().unwrap();
        check_integrity(&dir, 2);
        // the old key no longer opens the file
        let old = db_key(&vk_for(1), &VAULT_ID, 1).unwrap();
        assert!(Db::open(&dir.db_path(), old).is_err());
    }

    // SEC-S06 / SEC-A06: every boundary between two steps, deterministically.
    #[test]
    fn sec_s06_a_crash_after_any_step_leaves_a_vault_that_opens() {
        let steps = [
            (Step::Copied, 1),
            (Step::Rekeyed, 1),
            (Step::Verified, 1),
            (Step::BeforeCommit, 1),
            (Step::Committed, 2),
            (Step::Swapped, 2),
            (Step::Collected, 2),
        ];
        for (stop_at, expect_epoch) in steps {
            let t = tempfile::tempdir().unwrap();
            let dir = make_vault(t.path(), 4);
            let failure = rotate(&dir, &mut |s| {
                if s == stop_at {
                    Err(SessionError::Internal("simulated crash"))
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(failure.committed, expect_epoch == 2, "{stop_at:?}");

            // "restart": nothing is cleaned up by the crashed run
            let (active, mut v) = restart(&dir);
            assert_eq!(active.header.epoch, expect_epoch, "{stop_at:?}");
            assert_eq!(v.item_count().unwrap(), 4, "{stop_at:?}: no item lost");
            v.close().unwrap();
            check_integrity(&dir, expect_epoch);
            // opening collected the leftovers
            assert!(!dir.next_exists(), "{stop_at:?}");
            assert_eq!(header_epochs(&dir), [expect_epoch], "{stop_at:?}");
            // and the vault keeps working: another rotation succeeds
            rotate(&dir, &mut |_| Ok(())).unwrap();
            let (a2, mut v2) = restart(&dir);
            assert_eq!(a2.header.epoch, expect_epoch + 1);
            assert_eq!(v2.item_count().unwrap(), 4);
            v2.close().unwrap();
        }
    }

    /// Plants what `Db::open` leaves after a schema upgrade: an encrypted copy of the database under
    /// the current key, a sidecar, and two files that merely look similar.
    fn plant_backups(dir: &VaultDir) -> Vec<std::path::PathBuf> {
        let db = dir.db_path();
        let with = |suffix: &str| {
            let mut s = db.as_os_str().to_owned();
            s.push(suffix);
            std::path::PathBuf::from(s)
        };
        // recent, so that the 14-day pruning of `Db::open` cannot be what removes them
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let backups = vec![
            with(&format!(".bak-v1-{now}")),
            with(&format!(".bak-v1-{now}-1")),
            with(&format!(".bak-v1-{now}-wal")),
        ];
        for b in &backups {
            fs::copy(&db, b).unwrap();
        }
        backups
    }

    fn decoys(dir: &VaultDir) -> Vec<std::path::PathBuf> {
        let d = vec![
            dir.root().join("vault.db.bak-vX-1"),
            dir.root().join("vault.db.bak-v1"),
            dir.root().join("other.db.bak-v1-1700000000"),
            dir.root().join("vault.db.bak-v1-1700000000.txt"),
        ];
        for f in &d {
            fs::write(f, b"not ours").unwrap();
        }
        d
    }

    // SEC-A06: a pre-migration backup is encrypted under the retired key, so it must go with it.
    #[test]
    fn sec_a06_a_committed_rotation_deletes_the_pre_migration_backups() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 2);
        let backups = plant_backups(&dir);
        let decoys = decoys(&dir);
        rotate(&dir, &mut |_| Ok(())).unwrap();
        for b in &backups {
            assert!(!b.exists(), "{b:?} survived the rotation");
        }
        for d in &decoys {
            assert!(d.exists(), "{d:?} is not a backup and must stay");
        }
    }

    // The same, for a crash at every step after the commit: the next open finishes the job.
    #[test]
    fn sec_a06_a_crash_after_the_commit_still_deletes_the_backups_on_the_next_open() {
        for stop_at in [Step::Committed, Step::Swapped] {
            let t = tempfile::tempdir().unwrap();
            let dir = make_vault(t.path(), 2);
            let backups = plant_backups(&dir);
            let _ = rotate(&dir, &mut |s| {
                if s == stop_at {
                    Err(SessionError::Internal("simulated crash"))
                } else {
                    Ok(())
                }
            });
            let (_, v) = restart(&dir);
            v.close().unwrap();
            for b in &backups {
                assert!(!b.exists(), "{stop_at:?}: {b:?} survived");
            }
            assert_eq!(header_epochs(&dir), [2]);
        }
    }

    // A rotation that never committed leaves the old key in force; its backups stay usable.
    #[test]
    fn a_rotation_that_did_not_commit_keeps_the_backups() {
        for stop_at in [Step::Copied, Step::Verified, Step::BeforeCommit] {
            let t = tempfile::tempdir().unwrap();
            let dir = make_vault(t.path(), 2);
            let backups = plant_backups(&dir);
            let _ = rotate(&dir, &mut |s| {
                if s == stop_at {
                    Err(SessionError::Internal("simulated crash"))
                } else {
                    Ok(())
                }
            });
            let (_, v) = restart(&dir);
            v.close().unwrap();
            assert!(!dir.next_exists(), "{stop_at:?}");
            for b in &backups {
                assert!(b.exists(), "{stop_at:?}: {b:?} was deleted");
            }
        }
    }

    #[test]
    fn a_crash_before_the_commit_keeps_the_old_credentials_valid() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 1);
        let _ = rotate(&dir, &mut |s| {
            if s == Step::Verified {
                Err(SessionError::Internal("crash"))
            } else {
                Ok(())
            }
        });
        // the old header is still the only header and `vault.db` is untouched
        assert_eq!(header_epochs(&dir), [1]);
        check_integrity(&dir, 1);
        assert!(dir.next_exists(), "junk copy is still lying there");
    }

    #[test]
    fn a_complete_next_without_a_committed_header_is_never_used() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 2);
        let _ = rotate(&dir, &mut |s| {
            if s == Step::BeforeCommit {
                Err(SessionError::Internal("crash"))
            } else {
                Ok(())
            }
        });
        // `.next` is a perfect epoch-2 database, but only the epoch-1 header exists
        let (active, mut v) = restart(&dir);
        assert_eq!(active.header.epoch, 1);
        assert_eq!(v.item_count().unwrap(), 2);
        v.close().unwrap();
        assert!(!dir.next_exists());
    }

    #[test]
    fn a_garbage_next_is_never_swapped_in_even_next_to_a_newer_header() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 2);
        // a committed epoch-2 header whose database never arrived, and a junk `.next`
        let w = placeholder_wraps(2);
        let h = Header::new(VAULT_ID, 2, 2, w.kdf, w.wrap_pw, w.wrap_rk, 1);
        dir.write_header(&h, &DEVICE).unwrap();
        fs::write(dir.next_db_path(), vec![0xA5u8; 8192]).unwrap();
        let mut seen = Seen::default();
        let active = dir.active_header(&mut seen).unwrap();
        assert_eq!(active.header.epoch, 2);
        let r = lifecycle::open_vault(&dir, &vk_for(2), &active, &mut seen);
        assert!(r.is_err(), "no database opens under epoch 2");
        // nothing was destroyed: the epoch-1 database is intact and still `vault.db`
        check_integrity(&dir, 1);
    }

    #[test]
    fn sec_a06_an_old_header_put_back_cannot_open_the_database_and_is_rejected_by_floor() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 2);
        let old_header_bytes = fs::read(dir.root().join(&dir.header_files().unwrap()[0])).unwrap();
        let old_name = dir.header_files().unwrap()[0].clone();
        rotate(&dir, &mut |_| Ok(())).unwrap();

        // 1. the attacker copies the old header back: the newer one still wins and the old file is
        //    collected on the next open
        fs::write(dir.root().join(&old_name), &old_header_bytes).unwrap();
        let (active, v) = restart(&dir);
        assert_eq!(active.header.epoch, 2);
        v.close().unwrap();
        assert_eq!(header_epochs(&dir), [2]);

        // 2. the attacker also removes the new header: only the old one is left. Its key no longer
        //    opens the database ...
        let new_name = dir.header_files().unwrap()[0].clone();
        let new_bytes = fs::read(dir.root().join(&new_name)).unwrap();
        fs::remove_file(dir.root().join(&new_name)).unwrap();
        fs::write(dir.root().join(&old_name), &old_header_bytes).unwrap();
        let mut seen = Seen::default();
        let active = dir.active_header(&mut seen).unwrap();
        assert_eq!(active.header.epoch, 1);
        assert!(lifecycle::open_vault(&dir, &vk_for(1), &active, &mut seen).is_err());
        // ... and a session that already saw epoch 2 does not even select it
        let mut seen = Seen {
            vault_id: Some(VAULT_ID),
            highest_epoch: 2,
        };
        assert!(dir.active_header(&mut seen).is_err());
        let _ = new_bytes;
    }

    #[test]
    fn the_database_remembers_the_highest_epoch_and_refuses_a_lower_header() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 1);
        rotate(&dir, &mut |_| Ok(())).unwrap();
        // pretend this device has already used epoch 9
        let key = db_key(&vk_for(2), &VAULT_ID, 2).unwrap();
        let mut db = Db::open(&dir.db_path(), key).unwrap();
        db.with_tx(|tx| -> std::result::Result<(), StorageError> {
            tx.meta_set(META_HIGHEST_EPOCH, b"9")
        })
        .unwrap();
        db.close().unwrap();
        let mut seen = Seen::default();
        let active = dir.active_header(&mut seen).unwrap();
        let r = lifecycle::open_vault(&dir, &vk_for(2), &active, &mut seen);
        assert!(matches!(r, Err(SessionError::CorruptVault(m)) if m.contains("older")));
        assert_eq!(seen.highest_epoch, 9);
    }

    #[test]
    fn the_rotation_record_and_epoch_are_in_the_new_databases_meta() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 1);
        rotate(&dir, &mut |_| Ok(())).unwrap();
        rotate(&dir, &mut |_| Ok(())).unwrap();
        let key = db_key(&vk_for(3), &VAULT_ID, 3).unwrap();
        let mut db = Db::open(&dir.db_path(), key).unwrap();
        let get = |db: &mut Db, k: &str| {
            db.with_read(|tx| -> std::result::Result<Option<Vec<u8>>, StorageError> {
                tx.meta_get(k)
            })
            .unwrap()
        };
        assert_eq!(get(&mut db, "epoch").unwrap(), b"3");
        assert_eq!(get(&mut db, "header_version").unwrap(), b"3");
        assert_eq!(get(&mut db, META_HIGHEST_EPOCH).unwrap(), b"3");
        let last = RotationRecord::decode(&get(&mut db, META_ROTATION_LAST).unwrap()).unwrap();
        assert_eq!((last.old_epoch, last.new_epoch), (2, 3));
        let first = RotationRecord::decode(&get(&mut db, "rotation.2").unwrap()).unwrap();
        assert_eq!((first.old_epoch, first.new_epoch), (1, 2));
        db.close().unwrap();
    }

    /// Small deterministic generator so failures are reproducible.
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, bound: u64) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 33) % bound
        }
    }

    // Property: after N random rotations interleaved with edits and restarts, every item survives.
    #[test]
    fn after_random_rotations_interleaved_with_edits_every_item_survives() {
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 0);
        let mut rng = Lcg(0xA03_0001);
        let mut expected: Vec<String> = Vec::new();
        let mut epoch = 1u32;
        for step in 0..60 {
            match rng.next(3) {
                0 | 1 => {
                    let (_, mut v) = restart(&dir);
                    for _ in 0..=rng.next(3) {
                        let title = format!("CANARY item {} at step {step}", expected.len());
                        v.create_item(NewItem::new(ItemType::Note, &title)).unwrap();
                        expected.push(title);
                    }
                    v.close().unwrap();
                }
                _ => {
                    rotate(&dir, &mut |_| Ok(())).unwrap();
                    epoch += 1;
                }
            }
            let (active, mut v) = restart(&dir);
            assert_eq!(active.header.epoch, epoch, "step {step}");
            assert_eq!(
                v.item_count().unwrap(),
                expected.len() as u64,
                "step {step}"
            );
            let page = arya_vault_vault::Page::ALL;
            let listed = v
                .list(&arya_vault_vault::ListFilter::default(), page)
                .unwrap();
            let mut titles: Vec<String> = listed.iter().map(|s| s.title.clone()).collect();
            titles.sort();
            let mut want = expected.clone();
            want.sort();
            assert_eq!(titles, want, "step {step}");
            v.close().unwrap();
        }
        assert!(epoch > 5, "the run must actually have rotated ({epoch})");
        check_integrity(&dir, epoch);
    }

    /// Child entry point: only active when re-executed by the kill test.
    #[test]
    fn crash_child() {
        let Ok(root) = std::env::var(CHILD_ENV) else {
            return;
        };
        let dir = VaultDir::new(root.into());
        let out = std::io::stdout();
        let ack = |line: String| {
            let mut l = out.lock();
            writeln!(l, "{line}").unwrap();
            l.flush().unwrap();
        };
        loop {
            let (active, mut v) = restart(&dir);
            let n = v.item_count().unwrap();
            v.create_item(NewItem::new(ItemType::Note, &format!("CANARY item {n}")))
                .unwrap();
            v.close().unwrap();
            ack(format!("ACK ITEMS {}", n + 1));
            let e = active.header.epoch;
            let wraps = placeholder_wraps(e + 1);
            let rec = RotationRecord {
                old_epoch: e,
                new_epoch: e + 1,
                at_ms: 1,
            };
            let mut seen = Seen::default();
            let active = dir.active_header(&mut seen).unwrap();
            // closed above, so `execute` may run
            // Pause a little between steps so that kills spread over every phase.
            let mut pause = |step: Step| {
                // longer after the commit: a short window that kills would otherwise rarely hit
                let micros = if matches!(step, Step::Committed | Step::Swapped) {
                    8_000
                } else {
                    400
                };
                std::thread::sleep(std::time::Duration::from_micros(micros));
                Ok(())
            };
            execute(
                &dir,
                &active,
                &vk_for(e),
                &vk_for(e + 1),
                &wraps,
                &rec,
                &mut pause,
            )
            .unwrap();
            ack(format!("ACK EPOCH {}", e + 1));
        }
    }

    // SEC-S06: process death at random points (>= 200), like `storage/tests/crash_safety.rs`.
    #[test]
    #[allow(clippy::print_stdout)]
    fn sec_s06_kill_at_random_points_during_rotations_never_loses_the_vault() {
        let iterations: usize = std::env::var("ARYA_CRASH_ITERATIONS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        let t = tempfile::tempdir().unwrap();
        let dir = make_vault(t.path(), 1);
        let exe = std::env::current_exe().unwrap();
        let mut rng = Lcg(0xA03_C0DE);
        let (mut items_acked, mut epoch_acked) = (1u64, 1u32);
        let (mut kills_after_commit, mut kills_before_commit) = (0usize, 0usize);
        for i in 0..iterations {
            let mut child = Command::new(&exe)
                .args([
                    "--exact",
                    "rotation::tests::crash_child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD_ENV, dir.root())
                // a killed child leaves a corrupt .profraw under cargo llvm-cov
                .env("LLVM_PROFILE_FILE", NULL_DEVICE)
                .stdout(Stdio::piped())
                .stderr(std::fs::File::create(t.path().join("child.err")).unwrap())
                .spawn()
                .unwrap();
            let target = rng.next(12);
            let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
            // libtest prints "test <name> ... " on the same line before the child's first output,
            // so the marker is searched for, not expected at the start.
            let absorb = |line: &str, items: &mut u64, epoch: &mut u32| -> bool {
                let line = line.find("ACK ").map_or("", |at| &line[at..]);
                if let Some(n) = line.strip_prefix("ACK ITEMS ") {
                    *items = (*items).max(n.parse().unwrap());
                    true
                } else if let Some(e) = line.strip_prefix("ACK EPOCH ") {
                    *epoch = (*epoch).max(e.parse().unwrap());
                    true
                } else {
                    false
                }
            };
            let mut seen_acks = 0u64;
            while seen_acks < target {
                let Some(line) = lines.next() else { break };
                if absorb(&line.unwrap(), &mut items_acked, &mut epoch_acked) {
                    seen_acks += 1;
                }
            }
            if i % 2 == 1 {
                // Aimed: wait until the child has committed a rotation (a header of a higher
                // epoch appears), then kill within a few ms, inside the swap/collect phase.
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                while std::time::Instant::now() < deadline
                    && !header_epochs(&dir).iter().any(|e| *e > epoch_acked)
                {
                    std::thread::sleep(std::time::Duration::from_micros(150));
                }
                std::thread::sleep(std::time::Duration::from_micros(rng.next(6_000)));
            } else {
                // Random: the kill lands anywhere in the child's cycle, not just after an ack.
                std::thread::sleep(std::time::Duration::from_micros(rng.next(30_000)));
            }
            child.kill().unwrap();
            child.wait().unwrap();
            let err = fs::read_to_string(t.path().join("child.err")).unwrap_or_default();
            assert!(
                !err.contains("panicked"),
                "iteration {i}: the child panicked instead of being killed:\n{err}"
            );
            // Everything the child acknowledged before it died is still in the pipe.
            for line in lines {
                let _ = absorb(&line.unwrap(), &mut items_acked, &mut epoch_acked);
            }

            // Where did it die? (looked at before anything recovers)
            let epochs_on_disk = header_epochs(&dir);
            if epochs_on_disk.len() > 1 || epochs_on_disk.iter().any(|e| *e > epoch_acked) {
                kills_after_commit += 1; // the new header was in place, the rest unfinished/unacked
            } else if dir.next_exists() {
                kills_before_commit += 1; // mid-rotation, header still the old one
            }

            // After the kill: some header is active and a database opens under it, no matter
            // where the child died.
            let (active, mut v) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| restart(&dir)))
                    .unwrap_or_else(|_| {
                        panic!("iteration {i}: the vault does not open after a kill")
                    });
            let e = active.header.epoch;
            assert!(
                e >= epoch_acked,
                "iteration {i}: epoch went backwards ({e} < {epoch_acked})"
            );
            let n = v.item_count().unwrap();
            assert!(
                n >= items_acked,
                "iteration {i}: acknowledged items lost ({n} < {items_acked})"
            );
            assert!(
                n <= items_acked + 1,
                "iteration {i}: items appeared from nowhere (db {n}, acked {items_acked}, epoch {e} vs {epoch_acked})"
            );
            v.close().unwrap();
            check_integrity(&dir, e);
            assert!(!dir.next_exists(), "iteration {i}: leftovers not collected");
            assert_eq!(header_epochs(&dir), [e], "iteration {i}");
            epoch_acked = e;
            items_acked = n;
        }
        assert!(
            epoch_acked > 20,
            "the child never rotated enough: {epoch_acked}"
        );
        assert!(
            kills_before_commit >= 10 && kills_after_commit >= 20,
            "kills did not cover both sides of the commit: {kills_before_commit} before, \
             {kills_after_commit} after"
        );
        println!(
            "{iterations} kills survived; epoch {epoch_acked}, {items_acked} items; \
             {kills_before_commit} kills mid-rotation before the commit, \
             {kills_after_commit} after it"
        );
    }
}
