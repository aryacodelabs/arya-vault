//! Key rotation on [`Session`] (docs/04 §10, docs/14 §4.1 `changePassword(rotateKeys)` and
//! `regenerateRecoveryKey(rotateKeys)`, SEC-A06). The file protocol and its crash argument are in
//! [`crate::rotation`]; this file orders the user-visible steps around it.

use arya_vault_crypto::format::header::Header;
use arya_vault_storage::{Db, StorageError, Store};

use super::{
    RecoveryConfirmation, RecoveryKeyResult, Result, Session, SessionError, State, Unlocked,
    lifecycle,
};
use crate::layout::ActiveHeader;
use crate::profile::KdfProfile;
use crate::rotation::{self, META_ROTATION_LAST, RotationRecord, Step};

/// What a rotation hands back.
#[derive(Debug)]
pub struct RotationOutcome {
    /// The **new** recovery key (the old one no longer works). Onboarding is pending until
    /// `confirm_recovery_key` succeeds.
    pub recovery_key: RecoveryKeyResult,
    /// The record stored in the vault `meta` for sync (M4).
    pub record: RotationRecord,
}

impl Session {
    /// Rotates the vault key (docs/04 §10): a new random vault key and epoch, the database
    /// re-keyed under it, a **new recovery key** (the old one stops working), the old header
    /// deleted, quick unlock disabled. The master password stays.
    ///
    /// Requires an unlocked session and the master password again (the operation is destructive).
    /// Crash-safe: see [`crate::rotation`]. Takes two Argon2 evaluations plus the copy and re-key
    /// of the database.
    ///
    /// Honest limit (docs/04 §10): rotation protects **future** data; it cannot protect what an
    /// attacker already decrypted, and a device that holds the old key can still read old-epoch
    /// files.
    ///
    /// # Errors
    /// `Locked`, `Backoff`, `WrongCredentials`, storage/filesystem errors. On an error before the
    /// commit the vault is exactly as it was and the session stays unlocked.
    pub fn rotate_keys(&mut self, password: &str) -> Result<RotationOutcome> {
        self.rotate_impl(password, None, RecoveryConfirmation::Required)
    }

    /// [`rotate_keys`](Self::rotate_keys) with an explicit [`RecoveryConfirmation`]. The CLI
    /// harness, which prints the key itself and has no onboarding, passes `NotRequired`.
    ///
    /// # Errors
    /// As [`rotate_keys`](Self::rotate_keys).
    pub fn rotate_keys_with(
        &mut self,
        password: &str,
        confirmation: RecoveryConfirmation,
    ) -> Result<RotationOutcome> {
        self.rotate_impl(password, None, confirmation)
    }

    /// "Change password and rotate keys": [`rotate_keys`](Self::rotate_keys) with the new master
    /// password wrapped in the same step (one commit). The old password stops working.
    ///
    /// # Errors
    /// As [`rotate_keys`](Self::rotate_keys), plus `WeakPassword`.
    pub fn change_password_and_rotate(
        &mut self,
        old: &str,
        new: &str,
        profile: KdfProfile,
    ) -> Result<RotationOutcome> {
        self.rotate_impl(old, Some((new, profile)), RecoveryConfirmation::Required)
    }

    /// [`change_password_and_rotate`](Self::change_password_and_rotate) with an explicit
    /// [`RecoveryConfirmation`].
    ///
    /// # Errors
    /// As [`change_password_and_rotate`](Self::change_password_and_rotate).
    pub fn change_password_and_rotate_with(
        &mut self,
        old: &str,
        new: &str,
        profile: KdfProfile,
        confirmation: RecoveryConfirmation,
    ) -> Result<RotationOutcome> {
        self.rotate_impl(old, Some((new, profile)), confirmation)
    }

    /// The newest rotation record of this vault, if it was ever rotated. Needs the unlocked session.
    ///
    /// # Errors
    /// `Locked`, storage errors.
    pub fn rotation_record(&mut self) -> Result<Option<RotationRecord>> {
        let State::Unlocked(u) = &self.state else {
            return Err(SessionError::Locked);
        };
        let active = self.dir.active_header(&mut self.seen)?;
        let key = lifecycle::db_key(&u.vk, &active.header.vault_id, active.header.epoch)?;
        let mut db = Db::open(&self.dir.db_path(), key)?;
        let raw = db.with_read(|tx| -> std::result::Result<Option<Vec<u8>>, StorageError> {
            tx.meta_get(META_ROTATION_LAST)
        })?;
        db.close()?;
        Ok(raw.and_then(|b| RotationRecord::decode(&b)))
    }

    fn rotate_impl(
        &mut self,
        current_password: &str,
        new_password: Option<(&str, KdfProfile)>,
        confirmation: RecoveryConfirmation,
    ) -> Result<RotationOutcome> {
        let required = confirmation == RecoveryConfirmation::Required;
        if let Some((new, _)) = new_password {
            super::require_policy(new)?;
        }
        if !matches!(self.state, State::Unlocked(_)) {
            return Err(SessionError::Locked);
        }
        // Re-authenticate and do all the slow, fallible key work before anything is touched.
        let prep = self.attempt(|s| {
            lifecycle::prepare_rotation(&s.dir, &mut s.seen, current_password, new_password)
        })?;
        let lifecycle::RotationPrep {
            active,
            old_vk,
            new_vk,
            new_recovery_key,
            wraps,
        } = prep;
        let record = RotationRecord {
            old_epoch: active.header.epoch,
            new_epoch: wraps.epoch,
            at_ms: self.wall.now_ms(),
        };

        // Marker first: if the process dies after the commit but before the new recovery key is
        // shown, the next start finds "unconfirmed" and asks for a fresh key.
        let marker_before = self.set_marker(required)?;

        // Take the unlocked state apart: the old vault must be closed to copy its file.
        let State::Unlocked(u) = std::mem::replace(&mut self.state, State::Locked) else {
            self.restore_marker(marker_before);
            return Err(SessionError::Locked);
        };
        let Unlocked {
            vault,
            vk: held_vk,
            onboarding_pending,
            pending,
            password_verified_at_ms,
            #[cfg(test)]
            _probe,
        } = *u;
        if let Err(e) = vault.close() {
            // Keys are dropped (the state is Locked); nothing was changed on disk.
            self.restore_marker(marker_before);
            return Err(e.into());
        }

        let mut hook = |_: Step| Ok(());
        match rotation::execute(
            &self.dir, &active, &old_vk, &new_vk, &wraps, &record, &mut hook,
        ) {
            Ok(header) => self
                .finish_rotation(new_vk, header, active, new_recovery_key, record, required)
                .map(|recovery_key| RotationOutcome {
                    recovery_key,
                    record,
                }),
            Err(f) if f.committed => {
                // The new header is in place; opening completes the interrupted steps.
                let header = Header::new(
                    active.header.vault_id,
                    active.header.header_version + 1,
                    wraps.epoch,
                    wraps.kdf.clone(),
                    wraps.wrap_pw.clone(),
                    wraps.wrap_rk.clone(),
                    active.header.created_at,
                );
                // Whether or not that worked, the caller is told the step that failed.
                let _ = self.finish_rotation(
                    new_vk,
                    header,
                    active,
                    new_recovery_key,
                    record,
                    required,
                );
                Err(f.error)
            }
            Err(f) => {
                // Nothing was committed: drop the junk, put the old vault back.
                self.dir.remove_next();
                self.restore_marker(marker_before);
                if let Ok(vault) =
                    lifecycle::open_vault(&self.dir, &held_vk, &active, &mut self.seen)
                {
                    self.state = State::Unlocked(Box::new(Unlocked {
                        vault,
                        vk: held_vk,
                        onboarding_pending,
                        pending,
                        password_verified_at_ms,
                        #[cfg(test)]
                        _probe,
                    }));
                }
                Err(f.error)
            }
        }
    }

    /// After the commit: open the new database (completing any interrupted swap), switch quick
    /// unlock off, and install the new unlocked state with the new recovery key pending.
    fn finish_rotation(
        &mut self,
        new_vk: arya_vault_crypto::keys::VaultKey,
        header: Header,
        old: ActiveHeader,
        new_recovery_key: arya_vault_crypto::keys::RecoveryKey,
        record: RotationRecord,
        required: bool,
    ) -> Result<RecoveryKeyResult> {
        let active = ActiveHeader {
            header,
            device_id: old.device_id,
        };
        // The blob holds the OLD vault key: switch quick unlock off now. (If this is skipped by a
        // crash, the next `unlock_quick` unseals the old key, fails to open the database and
        // disables itself.)
        self.disable_quick_best_effort();
        self.seen.highest_epoch = self.seen.highest_epoch.max(record.new_epoch);
        let vault = lifecycle::open_vault(&self.dir, &new_vk, &active, &mut self.seen)?;
        let (result, pending) =
            Self::new_key_parts(new_recovery_key, required, active.header.header_version)?;
        let now = self.wall.now_ms();
        self.state = State::Unlocked(Box::new(Unlocked {
            vault,
            vk: new_vk,
            onboarding_pending: required,
            pending,
            password_verified_at_ms: now,
            #[cfg(test)]
            _probe: crate::probe::UnlockedProbe,
        }));
        Ok(result)
    }
}
