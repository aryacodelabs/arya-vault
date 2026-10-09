//! Quick unlock on [`Session`]: enable, disable, status and `unlock_quick` (docs/14 §4.1, §6;
//! docs/04 §8). The policy itself is [`crate::quick::evaluate`]; this file wires it to the
//! provider, the two files next to the header, and the same unlock path a password uses.

use super::{Ordering, Result, Session, SessionError, State, lifecycle};
use crate::quick::policy::{BlobReadError, QuickStore, ReadError};
use crate::quick::{PolicyRecord, ProviderError, QuickUnlockDenied, QuickUnlockStatus, evaluate};

fn denied(d: QuickUnlockDenied) -> SessionError {
    SessionError::QuickUnlockUnavailable(d)
}

fn from_provider(e: ProviderError) -> QuickUnlockDenied {
    match e {
        ProviderError::Unavailable => QuickUnlockDenied::ProviderUnavailable,
        ProviderError::UserCancelled => QuickUnlockDenied::Cancelled,
        ProviderError::Invalidated => QuickUnlockDenied::Invalidated,
        ProviderError::Failed => QuickUnlockDenied::Failed,
    }
}

impl Session {
    fn quick_store(&self) -> QuickStore {
        QuickStore::new(self.dir.root())
    }

    /// Deletes the policy record and the blob and asks the provider to drop its key, ignoring
    /// failures (used when quick unlock must stop working *now*).
    fn disable_quick_best_effort(&self) {
        let _ = self.quick_store().remove_all();
        self.provider.revoke();
    }

    /// Quick-unlock status. Works while locked; reads only the policy record and asks the
    /// provider whether it is available.
    #[must_use]
    pub fn quick_unlock_status(&self) -> QuickUnlockStatus {
        let supported = self.provider.available();
        let kind = self.provider.kind();
        let Ok(Some(rec)) = self.quick_store().read_policy() else {
            return QuickUnlockStatus {
                supported,
                enabled: false,
                kind,
                password_required: None,
            };
        };
        let password_required = if !supported {
            Some(QuickUnlockDenied::ProviderUnavailable)
        } else if rec.kind != kind {
            Some(QuickUnlockDenied::ProviderChanged)
        } else {
            let boot = self.provider.boot_id();
            evaluate(&rec, self.wall.now_ms(), boot.as_deref(), &self.quick_cfg).err()
        };
        QuickUnlockStatus {
            supported,
            enabled: true,
            kind,
            password_required,
        }
    }

    /// A password (or recovery) unlock resets the quick-unlock clock and failure counter and
    /// records the current boot. Best effort: if the record cannot be written, the old record
    /// stands, which is the stricter outcome.
    pub(super) fn note_password_unlock(&mut self, now_ms: u64) {
        let store = self.quick_store();
        if let Ok(Some(mut rec)) = store.read_policy() {
            rec.last_password_unlock_ms = now_ms;
            rec.last_seen_ms = now_ms;
            rec.failure_count = 0;
            rec.boot_id = self.provider.boot_id();
            let _ = store.write_policy(&rec);
        }
    }

    /// Seals the vault key with the provider and stores the blob and a policy record. Requires
    /// an unlocked session; the platform shows its user-presence prompt. Enabling again replaces
    /// the blob and resets the counters, but **not** the password-unlock time: it stays that of
    /// the unlock this session descends from.
    ///
    /// # Errors
    /// `Locked`; `QuickUnlockUnavailable(ProviderUnavailable | Cancelled | Failed | ..)`; I/O.
    pub fn quick_unlock_enable(&mut self) -> Result<()> {
        let State::Unlocked(u) = &self.state else {
            return Err(SessionError::Locked);
        };
        if !self.provider.available() {
            return Err(denied(QuickUnlockDenied::ProviderUnavailable));
        }
        let blob = self
            .provider
            .seal(&u.vk)
            .map_err(|e| denied(from_provider(e)))?;
        let now = self.wall.now_ms();
        let rec = PolicyRecord {
            kind: self.provider.kind(),
            enabled_at_ms: now,
            last_password_unlock_ms: u.password_verified_at_ms,
            last_seen_ms: now,
            last_quick_unlock_ms: 0,
            failure_count: 0,
            boot_id: self.provider.boot_id(),
        };
        let store = self.quick_store();
        // Blob first, record second: a record never points at a blob that is not there.
        let written = store
            .write_blob(&blob)
            .and_then(|()| store.write_policy(&rec));
        if let Err(e) = written {
            let _ = store.remove_all();
            return Err(e.into());
        }
        Ok(())
    }

    /// Deletes the policy record and the blob and asks the provider to revoke its key
    /// (best effort). Idempotent; allowed while locked (turning it off is the safe direction).
    ///
    /// # Errors
    /// I/O errors deleting the files (the provider is told to revoke regardless).
    pub fn quick_unlock_disable(&mut self) -> Result<()> {
        let removed = self.quick_store().remove_all();
        self.provider.revoke();
        removed.map_err(Into::into)
    }

    /// Unlocks with the platform provider instead of the master password.
    ///
    /// The vault key is released by the provider (inside Rust; it never reaches the UI) and then
    /// opens the database through the same path as a password unlock. The policy of docs/04 §8
    /// applies first: the password is required after a reboot (configurable), more than 72 h
    /// after the last **password** unlock, after 5 consecutive failures, when the provider
    /// reports the key invalidated, or when the stored data is rejected. Each denial is
    /// `QuickUnlockUnavailable(reason)`; the caller falls back to `unlock(password)`.
    ///
    /// The failure counter is persisted **before** the prompt, so killing the process during the
    /// prompt still counts as an attempt; a cancelled or unavailable prompt gives it back.
    ///
    /// # Errors
    /// `AlreadyUnlocked`, `NoVault`, `QuickUnlockUnavailable(..)`, header/storage errors.
    pub fn unlock_quick(&mut self) -> Result<()> {
        self.sync_presence()?;
        match self.state {
            State::Unlocked(_) => return Err(SessionError::AlreadyUnlocked),
            State::NoVault => return Err(SessionError::NoVault),
            State::Locked => {}
        }
        let started = self.lock_gen.load(Ordering::SeqCst);
        let store = self.quick_store();
        let mut rec = match store.read_policy() {
            Ok(Some(r)) => r,
            Ok(None) => return Err(denied(QuickUnlockDenied::NotEnabled)),
            Err(ReadError::Invalid) => {
                self.disable_quick_best_effort();
                return Err(denied(QuickUnlockDenied::BlobRejected));
            }
            Err(ReadError::Io(e)) => return Err(e.into()),
        };
        if !self.provider.available() {
            return Err(denied(QuickUnlockDenied::ProviderUnavailable));
        }
        if self.provider.kind() != rec.kind {
            self.disable_quick_best_effort();
            return Err(denied(QuickUnlockDenied::ProviderChanged));
        }
        let now = self.wall.now_ms();
        let boot = self.provider.boot_id();
        evaluate(&rec, now, boot.as_deref(), &self.quick_cfg).map_err(denied)?;
        let blob = match store.read_blob() {
            Ok(b) => b,
            Err(BlobReadError::Rejected) => {
                self.disable_quick_best_effort();
                return Err(denied(QuickUnlockDenied::BlobRejected));
            }
            Err(BlobReadError::Io(e)) => return Err(e.into()),
        };

        // Count the attempt before the prompt.
        let before = rec.failure_count;
        rec.failure_count = before.saturating_add(1);
        store.write_policy(&rec)?;
        let give_back = |rec: &mut PolicyRecord| {
            rec.failure_count = before;
            let _ = store.write_policy(rec);
        };

        let vk = match self.provider.unseal(&blob) {
            Ok(vk) => vk,
            Err(ProviderError::UserCancelled) => {
                give_back(&mut rec);
                return Err(denied(QuickUnlockDenied::Cancelled));
            }
            Err(ProviderError::Unavailable) => {
                give_back(&mut rec);
                return Err(denied(QuickUnlockDenied::ProviderUnavailable));
            }
            Err(ProviderError::Invalidated) => {
                self.disable_quick_best_effort();
                return Err(denied(QuickUnlockDenied::Invalidated));
            }
            // A failed biometric or a blob that does not authenticate: the attempt stays counted.
            Err(ProviderError::Failed) => return Err(denied(QuickUnlockDenied::Failed)),
        };
        if self.lock_gen.load(Ordering::SeqCst) != started {
            give_back(&mut rec);
            return Err(denied(QuickUnlockDenied::LockRequested));
        }

        // From here on it is the password path: header, derived database key, open.
        let active = match self.dir.active_header(&mut self.seen) {
            Ok(a) => a,
            Err(e) => {
                give_back(&mut rec);
                return Err(e);
            }
        };
        let vault = match lifecycle::open_vault(&self.dir, &vk, &active) {
            Ok(v) => v,
            // The key does not open this vault's database: a blob of another vault or a damaged
            // file. The attempt stays counted and quick unlock is switched off.
            Err(SessionError::Storage(arya_vault_storage::StorageError::WrongKeyOrCorrupt)) => {
                self.disable_quick_best_effort();
                return Err(denied(QuickUnlockDenied::BlobRejected));
            }
            Err(e) => {
                give_back(&mut rec);
                return Err(e);
            }
        };
        if self.lock_gen.load(Ordering::SeqCst) != started {
            let _ = vault.close();
            give_back(&mut rec);
            return Err(denied(QuickUnlockDenied::LockRequested));
        }

        rec.failure_count = 0;
        rec.last_seen_ms = now;
        rec.last_quick_unlock_ms = now;
        // If this write fails the counter stays raised and the old times stand: stricter.
        let _ = store.write_policy(&rec);
        let pw_at = rec.last_password_unlock_ms;
        self.install(vault, vk, None, pw_at);
        Ok(())
    }
}
