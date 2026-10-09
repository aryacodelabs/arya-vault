//! [`Session`]: the state machine of docs/14 §5 over one vault directory.
//!
//! ```text
//! NoVault --create--> UnlockedPendingConfirm --confirm_recovery_key--> Unlocked
//! Locked --unlock / recover--> Unlocked --lock--> Locked
//! ```
//!
//! The session owns the unlocked [`Vault`], the vault key and (until it is confirmed) the
//! pending recovery key. `lock` (or dropping the session) closes the database and drops all of
//! them; every key type is zeroized on drop (SEC-C06). The vault is reachable only through
//! [`Session::with_vault`], which fails with `locked` once the session is locked.

use std::path::PathBuf;

use arya_vault_crypto::keys::{RecoveryKey, VaultKey};
use arya_vault_crypto::recovery_key;
use arya_vault_crypto::rng::{OsRng, Rng};
use arya_vault_generator::meets_master_password_policy;
use arya_vault_storage::{Db, Store};
use arya_vault_vault::Vault;
use subtle::{Choice, ConstantTimeEq};
use zeroize::{Zeroize, Zeroizing};

use crate::backoff::{BackoffPolicy, Clock, FailureBackoff, MonotonicClock};
use crate::diag::{DbInfo, HeaderInfo, PINNED_SETTING_KEYS};
use crate::error::{Result, SessionError};
use crate::layout::{Seen, VaultDir};
use crate::lifecycle::{self, Prepared};
use crate::meta;
use crate::profile::KdfProfile;

/// Number of recovery-key groups shown to the user: six of five characters, one of two, and the
/// two-character checksum group (docs/04 §4).
pub const RECOVERY_KEY_GROUPS: usize = 8;
/// Groups that may be asked for in the confirmation: the six full key groups (indices 0..=5).
pub const CONFIRMATION_POOL: usize = 6;
/// How many groups the user must re-enter (docs/07 §3 step H).
pub const CONFIRMATION_GROUPS: usize = 3;

/// The externally visible state (docs/14 §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// No vault in the directory.
    NoVault,
    /// A vault exists; no keys in memory.
    Locked,
    /// Unlocked, but the recovery key has not been confirmed yet (onboarding incomplete).
    UnlockedPendingConfirm,
    /// Unlocked and onboarding complete.
    Unlocked,
}

/// What [`Session::status`] reports; readable while locked, from plaintext files only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultStatus {
    /// A vault exists in the directory.
    pub exists: bool,
    /// No keys are in memory.
    pub locked: bool,
    /// The recovery key was confirmed (see `meta` for where this lives). `false` if no vault.
    pub onboarding_complete: bool,
    /// `format_version` of the active header; `0` if there is no vault.
    pub format_version: u16,
}

/// Whether a freshly shown recovery key must be re-entered before onboarding completes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryConfirmation {
    /// The app flow (docs/07 §3 step H): the marker is set and the session holds the key until
    /// `confirm_recovery_key` succeeds.
    Required,
    /// A caller that shows the key itself and has no onboarding (the CLI harness): no marker,
    /// the session keeps nothing.
    NotRequired,
}

/// Injectable parts of a [`Session`] (tests use a fake clock).
pub struct SessionConfig {
    /// Clock for the failure delay.
    pub clock: Box<dyn Clock>,
    /// The failure-delay schedule.
    pub backoff: BackoffPolicy,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            clock: Box::new(MonotonicClock::new()),
            backoff: BackoffPolicy::default(),
        }
    }
}

/// A new recovery key, returned once. Its text is only available through
/// [`RecoveryKeyResult::recovery_key`]; `Debug` is redacted.
pub struct RecoveryKeyResult {
    text: Zeroizing<String>,
    challenge: Vec<usize>,
    header_version: u32,
}

impl RecoveryKeyResult {
    /// The recovery key as shown to the user (`XXXXX-...-CC`). The copy is wiped on drop.
    #[must_use]
    pub fn recovery_key(&self) -> Zeroizing<String> {
        self.text.clone()
    }

    /// Number of groups in the displayed key (docs/14 `RecoveryKeyResult.groups`).
    #[must_use]
    pub fn groups(&self) -> usize {
        RECOVERY_KEY_GROUPS
    }

    /// The zero-based group indices the user must re-enter, ascending. Empty if
    /// [`RecoveryConfirmation::NotRequired`].
    #[must_use]
    pub fn challenge(&self) -> &[usize] {
        &self.challenge
    }

    /// The header version that now carries this key's wrap.
    #[must_use]
    pub fn header_version(&self) -> u32 {
        self.header_version
    }
}

impl core::fmt::Debug for RecoveryKeyResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RecoveryKeyResult")
            .field("recovery_key", &"<redacted>")
            .field("challenge", &self.challenge)
            .field("header_version", &self.header_version)
            .finish()
    }
}

/// The recovery key waiting for confirmation, and the groups currently being asked for.
struct PendingKey {
    key: RecoveryKey,
    challenge: Vec<usize>,
    #[cfg(test)]
    _probe: crate::probe::PendingProbe,
}

/// Everything that exists only while unlocked. No `Debug` (it holds key material).
struct Unlocked {
    vault: Vault,
    /// Kept for the session lifetime for key-bound features (quick unlock, rotation); wiped on drop.
    vk: VaultKey,
    onboarding_pending: bool,
    pending: Option<PendingKey>,
    #[cfg(test)]
    _probe: crate::probe::UnlockedProbe,
}

enum State {
    NoVault,
    Locked,
    Unlocked(Box<Unlocked>),
}

/// One vault directory and the lifecycle state around it.
pub struct Session {
    dir: VaultDir,
    seen: Seen,
    backoff: FailureBackoff,
    state: State,
}

impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Session")
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.lock();
    }
}

/// SEC-A07: the master password policy applies to every new master password.
fn require_policy(password: &str) -> Result<()> {
    meets_master_password_policy(password)
        .map_err(|v| SessionError::WeakPassword(v.iter().map(ToString::to_string).collect()))
}

/// A uniform value in `0..n` by rejection sampling (no modulo bias), from the OS CSPRNG.
fn random_below(n: usize) -> Result<usize> {
    debug_assert!(n > 0 && n <= 256);
    let limit = 256 - (256 % n);
    loop {
        let mut b = [0u8; 1];
        OsRng.fill_bytes(&mut b)?;
        if usize::from(b[0]) < limit {
            return Ok(usize::from(b[0]) % n);
        }
    }
}

/// Picks [`CONFIRMATION_GROUPS`] distinct group indices from the pool (Fisher-Yates), ascending.
fn draw_challenge() -> Result<Vec<usize>> {
    let mut pool: Vec<usize> = (0..CONFIRMATION_POOL).collect();
    for i in (1..pool.len()).rev() {
        pool.swap(i, random_below(i + 1)?);
    }
    pool.truncate(CONFIRMATION_GROUPS);
    pool.sort_unstable();
    Ok(pool)
}

/// Crockford normalisation of typed text, as the recovery-key parser does (docs/04 §4): drops
/// hyphens and whitespace, upper-cases, maps `I`/`L` -> `1` and `O` -> `0`.
fn normalize_group(text: &str) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(text.len()));
    for c in text.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        let up = match c.to_ascii_uppercase() {
            'I' | 'L' => '1',
            'O' => '0',
            other => other,
        };
        let mut buf = [0u8; 4];
        out.extend_from_slice(up.encode_utf8(&mut buf).as_bytes());
    }
    out
}

impl Session {
    /// Opens the vault directory at `path` (which need not exist yet). The result is `Locked`
    /// if a vault is there, `NoVault` otherwise. Reads no keys.
    ///
    /// # Errors
    /// Filesystem errors reading the directory.
    pub fn open_dir(path: impl Into<PathBuf>) -> Result<Self> {
        Self::open_dir_with(path, SessionConfig::default())
    }

    /// [`open_dir`](Self::open_dir) with an injected clock and delay schedule.
    ///
    /// # Errors
    /// Filesystem errors reading the directory.
    pub fn open_dir_with(path: impl Into<PathBuf>, config: SessionConfig) -> Result<Self> {
        let dir = VaultDir::new(path.into());
        let state = if dir.has_vault()? {
            State::Locked
        } else {
            State::NoVault
        };
        Ok(Self {
            dir,
            seen: Seen::default(),
            backoff: FailureBackoff::new(config.backoff, config.clock),
            state,
        })
    }

    /// Re-checks the disk while no keys are held (another process may have created or removed
    /// the vault).
    fn sync_presence(&mut self) -> Result<()> {
        if !matches!(self.state, State::Unlocked(_)) {
            self.state = if self.dir.has_vault()? {
                State::Locked
            } else {
                State::NoVault
            };
        }
        Ok(())
    }

    /// The current state (see [`SessionState`]); does not touch the disk.
    #[must_use]
    pub fn state(&self) -> SessionState {
        match &self.state {
            State::NoVault => SessionState::NoVault,
            State::Locked => SessionState::Locked,
            State::Unlocked(u) if u.onboarding_pending => SessionState::UnlockedPendingConfirm,
            State::Unlocked(_) => SessionState::Unlocked,
        }
    }

    /// Whether a database file exists next to the header. (A header-only directory is a golden
    /// fixture or a partial copy; it can be password-checked but not unlocked.)
    #[must_use]
    pub fn has_database(&self) -> bool {
        self.dir.has_db()
    }

    /// Vault status. Works while locked and reads only plaintext header data and the
    /// onboarding marker.
    ///
    /// # Errors
    /// `CorruptVault` / `UnsupportedFormat` if the header cannot be used; filesystem errors.
    pub fn status(&mut self) -> Result<VaultStatus> {
        self.sync_presence()?;
        match &self.state {
            State::NoVault => Ok(VaultStatus {
                exists: false,
                locked: true,
                onboarding_complete: false,
                format_version: 0,
            }),
            State::Locked => {
                let active = self.dir.active_header(&mut self.seen)?;
                Ok(VaultStatus {
                    exists: true,
                    locked: true,
                    onboarding_complete: !meta::onboarding_pending(self.dir.root()),
                    format_version: active.header.format_version,
                })
            }
            State::Unlocked(u) => {
                let active = self.dir.active_header(&mut self.seen)?;
                Ok(VaultStatus {
                    exists: true,
                    locked: false,
                    onboarding_complete: !u.onboarding_pending,
                    format_version: active.header.format_version,
                })
            }
        }
    }

    /// Plaintext facts about the active header (works while locked).
    ///
    /// # Errors
    /// `NoVault`, `CorruptVault`, `UnsupportedFormat`, filesystem errors.
    pub fn header_info(&mut self) -> Result<HeaderInfo> {
        self.sync_presence()?;
        let active = self.dir.active_header(&mut self.seen)?;
        Ok(HeaderInfo::of(&active.header, active.device_id))
    }

    /// Database facts (schema version, pinned SQLCipher settings). Requires the unlocked session.
    ///
    /// # Errors
    /// `Locked`, or storage errors.
    pub fn db_info(&mut self) -> Result<DbInfo> {
        let State::Unlocked(u) = &self.state else {
            return Err(SessionError::Locked);
        };
        let active = self.dir.active_header(&mut self.seen)?;
        let key = lifecycle::db_key(&u.vk, &active.header.vault_id, active.header.epoch)?;
        let mut db = Db::open(&self.dir.db_path(), key)?;
        let schema_version = db.schema_version()?;
        let mut pinned_settings = Vec::with_capacity(PINNED_SETTING_KEYS.len());
        for k in PINNED_SETTING_KEYS {
            let v = db.with_read(
                |tx| -> std::result::Result<Option<Vec<u8>>, arya_vault_storage::StorageError> {
                    tx.meta_get(k)
                },
            )?;
            pinned_settings.push((k, v.map(|b| String::from_utf8_lossy(&b).into_owned())));
        }
        db.close()?;
        Ok(DbInfo {
            schema_version,
            pinned_settings,
        })
    }

    // ----- password attempts and the failure delay ---------------------------------------

    /// Runs a password-dependent step under the failure delay: refuses without trying while a
    /// delay runs, counts a wrong password, resets on success.
    fn attempt<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.backoff.check()?;
        let r = f(self);
        match &r {
            Ok(_) => self.backoff.record_success(),
            Err(SessionError::WrongCredentials) => self.backoff.record_failure(),
            Err(_) => {}
        }
        r
    }

    /// Checks the master password against the header only, with no database access and without
    /// requiring (or changing) the unlock state. For header-only directories and tooling.
    ///
    /// # Errors
    /// `WrongCredentials`, `Backoff`, `NoVault`, header errors.
    pub fn check_header_password(&mut self, password: &str) -> Result<()> {
        self.sync_presence()?;
        self.attempt(|s| lifecycle::check_password(&s.dir, &mut s.seen, password).map(|_| ()))
    }

    // ----- lifecycle ----------------------------------------------------------------------

    /// Creates a vault and leaves the session unlocked with onboarding pending
    /// ([`RecoveryConfirmation::Required`]). The returned recovery key is shown once; the
    /// session keeps a copy until `confirm_recovery_key` succeeds or the session locks.
    ///
    /// # Errors
    /// `AlreadyExists`, `WeakPassword`, KDF/storage/filesystem errors.
    pub fn create(&mut self, password: &str, profile: KdfProfile) -> Result<RecoveryKeyResult> {
        self.create_with(password, profile, RecoveryConfirmation::Required)
    }

    /// [`create`](Self::create) with an explicit [`RecoveryConfirmation`].
    ///
    /// # Errors
    /// As [`create`](Self::create).
    pub fn create_with(
        &mut self,
        password: &str,
        profile: KdfProfile,
        confirmation: RecoveryConfirmation,
    ) -> Result<RecoveryKeyResult> {
        // Input validation first (as the CLI always did), then the state.
        require_policy(password)?;
        self.sync_presence()?;
        if !matches!(self.state, State::NoVault) {
            return Err(SessionError::AlreadyExists);
        }
        let required = confirmation == RecoveryConfirmation::Required;
        let created = lifecycle::create(&self.dir, &mut self.seen, password, profile, |dir| {
            if required {
                meta::set_onboarding_pending(dir.root())
            } else {
                meta::clear_onboarding_pending(dir.root())
            }
        })
        .inspect_err(|e| {
            // Not for `AlreadyExists`: the marker then belongs to the vault that is there.
            if !matches!(e, SessionError::AlreadyExists) {
                let _ = meta::clear_onboarding_pending(self.dir.root());
            }
        })?;
        let text = recovery_key::encode(&created.recovery_key);
        let (pending, challenge) = if required {
            let challenge = draw_challenge()?;
            (
                Some(PendingKey {
                    key: created.recovery_key,
                    challenge: challenge.clone(),
                    #[cfg(test)]
                    _probe: crate::probe::PendingProbe,
                }),
                challenge,
            )
        } else {
            (None, Vec::new())
        };
        self.state = State::Unlocked(Box::new(Unlocked {
            vault: created.vault,
            vk: created.vk,
            onboarding_pending: required,
            pending,
            #[cfg(test)]
            _probe: crate::probe::UnlockedProbe,
        }));
        Ok(RecoveryKeyResult {
            text,
            challenge,
            header_version: 1,
        })
    }

    /// Unlocks with the master password: derives the vault key, verifies the header, opens
    /// the database.
    ///
    /// # Errors
    /// `AlreadyUnlocked`, `NoVault`, `Backoff`, `WrongCredentials`, header/storage errors.
    pub fn unlock(&mut self, password: &str) -> Result<()> {
        self.sync_presence()?;
        match self.state {
            State::Unlocked(_) => return Err(SessionError::AlreadyUnlocked),
            State::NoVault => return Err(SessionError::NoVault),
            State::Locked => {}
        }
        let (active, vk) =
            self.attempt(|s| lifecycle::check_password(&s.dir, &mut s.seen, password))?;
        let vault = lifecycle::open_vault(&self.dir, &vk, &active)?;
        self.install(vault, vk, None);
        Ok(())
    }

    fn install(&mut self, vault: Vault, vk: VaultKey, pending: Option<PendingKey>) {
        let onboarding_pending = pending.is_some() || meta::onboarding_pending(self.dir.root());
        self.state = State::Unlocked(Box::new(Unlocked {
            vault,
            vk,
            onboarding_pending,
            pending,
            #[cfg(test)]
            _probe: crate::probe::UnlockedProbe,
        }));
    }

    /// Locks: closes the database (`Vault::close`), drops the vault key, the database key and any
    /// pending recovery key (all zeroized on drop). Idempotent: locking a locked session does
    /// nothing.
    ///
    /// # Errors
    /// A failure to checkpoint or close the database. The session is locked and every key is
    /// gone regardless; the data stays consistent in the write-ahead log.
    pub fn lock(&mut self) -> Result<()> {
        match std::mem::replace(&mut self.state, State::Locked) {
            State::Unlocked(u) => {
                let Unlocked {
                    vault, vk, pending, ..
                } = *u;
                let closed = vault.close();
                drop(vk);
                drop(pending);
                closed.map_err(SessionError::from)
            }
            State::NoVault => {
                self.state = State::NoVault;
                Ok(())
            }
            State::Locked => Ok(()),
        }
    }

    /// Runs `f` on the unlocked vault. The only way to reach the vault: after `lock()` it
    /// returns `Locked` and `f` is not called, so a stale handle cannot touch the database.
    ///
    /// # Errors
    /// [`SessionError::Locked`].
    pub fn with_vault<R>(&mut self, f: impl FnOnce(&mut Vault) -> R) -> Result<R> {
        match &mut self.state {
            State::Unlocked(u) => Ok(f(&mut u.vault)),
            _ => Err(SessionError::Locked),
        }
    }

    /// Re-authenticates: `true` if `password` is the master password. For sensitive actions
    /// (view recovery key, export). A wrong password counts toward the failure delay.
    ///
    /// # Errors
    /// `Locked`, `Backoff`, header errors.
    pub fn verify_password(&mut self, password: &str) -> Result<bool> {
        if !matches!(self.state, State::Unlocked(_)) {
            return Err(SessionError::Locked);
        }
        match self.attempt(|s| lifecycle::check_password(&s.dir, &mut s.seen, password)) {
            Ok(_) => Ok(true),
            Err(SessionError::WrongCredentials) => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// Changes the master password **without** the recovery key (docs/04 §9, SEC-C12, SEC-A05):
    /// re-wraps the vault key; no data is re-encrypted. Works locked or unlocked and keeps the
    /// lock state. Returns the new header version.
    ///
    /// # Errors
    /// `NoVault`, `WeakPassword`, `Backoff`, `WrongCredentials` (old password), storage errors.
    pub fn change_password(&mut self, old: &str, new: &str, profile: KdfProfile) -> Result<u32> {
        require_policy(new)?;
        self.sync_presence()?;
        if matches!(self.state, State::NoVault) {
            return Err(SessionError::NoVault);
        }
        let want_vk = !matches!(self.state, State::Unlocked(_));
        let prepared = self.attempt(|s| {
            lifecycle::prepare_change_password(&s.dir, &mut s.seen, old, new, profile, want_vk)
        })?;
        let Prepared {
            active, wraps, vk, ..
        } = prepared;
        let held = match &self.state {
            State::Unlocked(u) => Some(&u.vk),
            _ => None,
        };
        let vk = held
            .or(vk.as_ref())
            .ok_or(SessionError::Internal("no vault key to publish with"))?;
        lifecycle::publish(&self.dir, &active, wraps, vk)
    }

    /// Adds or removes the onboarding marker; returns the previous state so a failed publish can
    /// put it back.
    fn set_marker(&self, pending: bool) -> Result<bool> {
        let before = meta::onboarding_pending(self.dir.root());
        if pending {
            meta::set_onboarding_pending(self.dir.root())?;
        } else {
            meta::clear_onboarding_pending(self.dir.root())?;
        }
        Ok(before)
    }

    fn restore_marker(&self, was_pending: bool) {
        let _ = if was_pending {
            meta::set_onboarding_pending(self.dir.root())
        } else {
            meta::clear_onboarding_pending(self.dir.root())
        };
    }

    /// Resets the master password with the recovery key (docs/04 §9). Leaves the session
    /// unlocked. (A header-only directory, which has no database, stays locked.) Returns the
    /// new header version. The recovery key itself is not replaced; see
    /// [`recover_and_regenerate`](Self::recover_and_regenerate).
    ///
    /// # Errors
    /// `AlreadyUnlocked`, `NoVault`, `RecoveryKeyMalformed`, `WeakPassword`, `WrongCredentials`.
    pub fn recover(
        &mut self,
        recovery_key_text: &str,
        new_password: &str,
        profile: KdfProfile,
    ) -> Result<u32> {
        self.recover_impl(recovery_key_text, new_password, profile, None)
            .map(|(version, _)| version)
    }

    /// [`recover`](Self::recover) that also replaces the recovery key in the same header (one
    /// publish) and returns the new key.
    ///
    /// # Errors
    /// As [`recover`](Self::recover).
    pub fn recover_and_regenerate(
        &mut self,
        recovery_key_text: &str,
        new_password: &str,
        profile: KdfProfile,
        confirmation: RecoveryConfirmation,
    ) -> Result<RecoveryKeyResult> {
        let (_, result) =
            self.recover_impl(recovery_key_text, new_password, profile, Some(confirmation))?;
        result.ok_or(SessionError::Internal("no new recovery key"))
    }

    fn recover_impl(
        &mut self,
        recovery_key_text: &str,
        new_password: &str,
        profile: KdfProfile,
        regenerate: Option<RecoveryConfirmation>,
    ) -> Result<(u32, Option<RecoveryKeyResult>)> {
        // Input validation (typo in the key, weak password) fails before the state is looked at
        // and before any key derivation.
        let rk = recovery_key::parse(recovery_key_text)
            .map_err(|_| SessionError::RecoveryKeyMalformed)?;
        require_policy(new_password)?;
        self.sync_presence()?;
        match self.state {
            State::Unlocked(_) => return Err(SessionError::AlreadyUnlocked),
            State::NoVault => return Err(SessionError::NoVault),
            State::Locked => {}
        }
        let Prepared {
            active,
            wraps,
            vk,
            new_recovery_key,
        } = lifecycle::prepare_recover(
            &self.dir,
            &mut self.seen,
            &rk,
            new_password,
            profile,
            regenerate.is_some(),
        )?;
        let vk = vk.ok_or(SessionError::Internal("no vault key after recovery"))?;
        let required = regenerate == Some(RecoveryConfirmation::Required);
        // Marker first: a crash leaves "unconfirmed" with the old key still valid, never a new
        // key that nothing will ask the user to confirm.
        let marker_before = if regenerate.is_some() {
            Some(self.set_marker(required)?)
        } else {
            None
        };
        let version = match lifecycle::publish(&self.dir, &active, wraps, &vk) {
            Ok(v) => v,
            Err(e) => {
                if let Some(before) = marker_before {
                    self.restore_marker(before);
                }
                return Err(e);
            }
        };
        let mut result = None;
        let mut pending = None;
        if let Some(new_rk) = new_recovery_key {
            let (r, p) = Self::new_key_parts(new_rk, required, version)?;
            result = Some(r);
            pending = p;
        }
        if self.dir.has_db() {
            let vault = lifecycle::open_vault(&self.dir, &vk, &active)?;
            self.install(vault, vk, pending);
        }
        Ok((version, result))
    }

    /// Builds the result for a new recovery key and, if confirmation is required, the pending
    /// state the session keeps.
    fn new_key_parts(
        rk: RecoveryKey,
        required: bool,
        header_version: u32,
    ) -> Result<(RecoveryKeyResult, Option<PendingKey>)> {
        let text = recovery_key::encode(&rk);
        if required {
            let challenge = draw_challenge()?;
            Ok((
                RecoveryKeyResult {
                    text,
                    challenge: challenge.clone(),
                    header_version,
                },
                Some(PendingKey {
                    key: rk,
                    challenge,
                    #[cfg(test)]
                    _probe: crate::probe::PendingProbe,
                }),
            ))
        } else {
            Ok((
                RecoveryKeyResult {
                    text,
                    challenge: Vec::new(),
                    header_version,
                },
                None,
            ))
        }
    }

    /// Replaces the recovery key: re-wraps the vault key under a new recovery key (the master
    /// password stays) and returns the new key once. Requires the master password again
    /// (docs/07 §5) and an unlocked session. The old key stops working with the new header.
    /// Onboarding is pending again until `confirm_recovery_key` succeeds.
    ///
    /// Honest limit (docs/04 §4): this does not revoke the old key against someone who already
    /// holds an old header; only key rotation does (A03).
    ///
    /// # Errors
    /// `Locked`, `Backoff`, `WrongCredentials`, storage/filesystem errors.
    pub fn regenerate_recovery_key(&mut self, password: &str) -> Result<RecoveryKeyResult> {
        self.regenerate_recovery_key_with(password, RecoveryConfirmation::Required)
    }

    /// [`regenerate_recovery_key`](Self::regenerate_recovery_key) with an explicit
    /// [`RecoveryConfirmation`].
    ///
    /// # Errors
    /// As [`regenerate_recovery_key`](Self::regenerate_recovery_key).
    pub fn regenerate_recovery_key_with(
        &mut self,
        password: &str,
        confirmation: RecoveryConfirmation,
    ) -> Result<RecoveryKeyResult> {
        if !matches!(self.state, State::Unlocked(_)) {
            return Err(SessionError::Locked);
        }
        let Prepared {
            active,
            wraps,
            vk,
            new_recovery_key,
        } = self.attempt(|s| lifecycle::prepare_regenerate(&s.dir, &mut s.seen, password))?;
        let vk = vk.ok_or(SessionError::Internal("no vault key after check"))?;
        let new_rk = new_recovery_key.ok_or(SessionError::Internal("no new recovery key"))?;
        let required = confirmation == RecoveryConfirmation::Required;
        let marker_before = self.set_marker(required)?;
        let version = match lifecycle::publish(&self.dir, &active, wraps, &vk) {
            Ok(v) => v,
            Err(e) => {
                self.restore_marker(marker_before);
                return Err(e);
            }
        };
        let (result, pending) = Self::new_key_parts(new_rk, required, version)?;
        if let State::Unlocked(u) = &mut self.state {
            u.onboarding_pending = required;
            u.pending = pending;
        }
        Ok(result)
    }

    // ----- recovery-key confirmation (docs/07 §3 step H) ---------------------------------

    /// The group indices (zero-based, ascending) the user is currently asked to re-enter.
    ///
    /// # Errors
    /// `Locked`, `NoPendingRecoveryKey`.
    pub fn recovery_challenge(&self) -> Result<Vec<usize>> {
        match &self.state {
            State::Unlocked(u) => u
                .pending
                .as_ref()
                .map(|p| p.challenge.clone())
                .ok_or(SessionError::NoPendingRecoveryKey),
            _ => Err(SessionError::Locked),
        }
    }

    /// The pending recovery key text, so the UI can show it again after a failed confirmation
    /// (docs/07 §3: mismatch goes back to "show recovery key"). Wiped on drop.
    ///
    /// # Errors
    /// `Locked`, `NoPendingRecoveryKey`.
    pub fn pending_recovery_key(&self) -> Result<Zeroizing<String>> {
        match &self.state {
            State::Unlocked(u) => u
                .pending
                .as_ref()
                .map(|p| recovery_key::encode(&p.key))
                .ok_or(SessionError::NoPendingRecoveryKey),
            _ => Err(SessionError::Locked),
        }
    }

    /// Checks the user's re-entry of the requested groups. `answers` are `(group index, text)`
    /// and must cover exactly the groups of [`recovery_challenge`](Self::recovery_challenge).
    /// Typed text is normalised like the recovery-key parser does (case, hyphens and spaces,
    /// Crockford `I`/`L`/`O` substitutions) and compared in constant time. Consumes and wipes
    /// the answers.
    ///
    /// On success the onboarding marker is removed, the pending key is dropped and the state
    /// becomes `Unlocked`. On a mismatch it returns `Ok(false)`, keeps the key and asks for
    /// three **new** random groups.
    ///
    /// # Errors
    /// `Locked`, `NoPendingRecoveryKey`, `InvalidConfirmation` (wrong set of indices).
    pub fn confirm_recovery_key(&mut self, mut answers: Vec<(usize, String)>) -> Result<bool> {
        let State::Unlocked(u) = &mut self.state else {
            wipe_answers(&mut answers);
            return Err(SessionError::Locked);
        };
        let Some(pending) = u.pending.as_mut() else {
            wipe_answers(&mut answers);
            return Err(SessionError::NoPendingRecoveryKey);
        };
        let mut indices: Vec<usize> = answers.iter().map(|(i, _)| *i).collect();
        indices.sort_unstable();
        if indices != pending.challenge {
            wipe_answers(&mut answers);
            return Err(SessionError::InvalidConfirmation);
        }
        let shown = recovery_key::encode(&pending.key);
        let groups: Vec<&str> = shown.split('-').collect();
        // Every group is compared; the results are combined without an early exit.
        let mut all = Choice::from(1u8);
        for (idx, text) in &answers {
            let typed = normalize_group(text);
            let expected = groups.get(*idx).map_or(&[][..], |g| g.as_bytes());
            all &= typed.as_slice().ct_eq(expected);
        }
        wipe_answers(&mut answers);
        if bool::from(all) {
            meta::clear_onboarding_pending(self.dir.root())?;
            u.pending = None;
            u.onboarding_pending = false;
            Ok(true)
        } else {
            pending.challenge = draw_challenge()?;
            Ok(false)
        }
    }
}

fn wipe_answers(answers: &mut [(usize, String)]) {
    for (_, text) in answers {
        text.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::probe::{PENDING_DROPS, UNLOCKED_DROPS};

    const PW: &str = "CANARY-state-test-master-password-8472 correct";

    fn drops() -> (usize, usize) {
        (
            UNLOCKED_DROPS.with(std::cell::Cell::get),
            PENDING_DROPS.with(std::cell::Cell::get),
        )
    }

    fn create(dir: &std::path::Path) -> (Session, RecoveryKeyResult) {
        let mut s = Session::open_dir(dir).unwrap();
        let rk = s.create(PW, KdfProfile::Low).unwrap();
        (s, rk)
    }

    fn groups(text: &str) -> Vec<String> {
        text.split('-').map(str::to_owned).collect()
    }

    // SEC-A04 / SEC-C06: lock releases the unlocked state (vault key, DB handle) and the pending
    // recovery key; the key types wipe themselves on drop (crypto/storage `sec_c06_*` tests).
    #[test]
    fn sec_c06_lock_drops_the_unlocked_state_and_the_pending_key() {
        let t = tempfile::tempdir().unwrap();
        let (mut s, _rk) = create(&t.path().join("v"));
        assert_eq!(drops(), (0, 0));
        s.lock().unwrap();
        assert_eq!(
            drops(),
            (1, 1),
            "unlocked state and pending key dropped by lock"
        );
        s.lock().unwrap();
        assert_eq!(drops(), (1, 1), "a second lock drops nothing");
        s.unlock(PW).unwrap();
        assert_eq!(drops(), (1, 1));
        s.lock().unwrap();
        assert_eq!(drops(), (2, 1), "no pending key after a re-unlock");
    }

    #[test]
    fn sec_c06_dropping_the_session_releases_everything() {
        let t = tempfile::tempdir().unwrap();
        let (s, _rk) = create(&t.path().join("v"));
        drop(s);
        assert_eq!(drops(), (1, 1));
    }

    #[test]
    fn sec_c06_confirming_drops_the_pending_key_immediately() {
        let t = tempfile::tempdir().unwrap();
        let (mut s, rk) = create(&t.path().join("v"));
        let key = rk.recovery_key();
        let ans: Vec<_> = rk
            .challenge()
            .iter()
            .map(|i| (*i, groups(&key)[*i].clone()))
            .collect();
        assert!(s.confirm_recovery_key(ans).unwrap());
        assert_eq!(drops(), (0, 1), "key gone, session still unlocked");
    }

    // SEC-A01: the re-entry check.
    #[test]
    fn sec_a01_confirmation_normalises_and_rejects_everything_else() {
        let t = tempfile::tempdir().unwrap();
        let (mut s, rk) = create(&t.path().join("v"));
        let key = rk.recovery_key().to_string();
        let g = groups(&key);

        // answers for the wrong set of groups are an error and change nothing
        let challenge = s.recovery_challenge().unwrap();
        let other: Vec<usize> = (0..CONFIRMATION_POOL)
            .filter(|i| !challenge.contains(i))
            .collect();
        let bad_sets: Vec<Vec<usize>> = vec![
            vec![],
            vec![challenge[0]],
            vec![challenge[0], challenge[1]],
            vec![challenge[0], challenge[0], challenge[1]],
            vec![challenge[0], challenge[1], challenge[2], other[0]],
            vec![other[0], other[1], other[2]],
            vec![challenge[0], challenge[1], 99],
        ];
        for set in bad_sets {
            let ans: Vec<_> = set.iter().map(|i| (*i, "X".to_owned())).collect();
            assert!(
                matches!(
                    s.confirm_recovery_key(ans),
                    Err(SessionError::InvalidConfirmation)
                ),
                "{set:?}"
            );
            assert_eq!(
                s.recovery_challenge().unwrap(),
                challenge,
                "no redraw on misuse"
            );
        }

        // wrong text for the right groups -> false, key kept, a new challenge is drawn
        let wrongs: Vec<String> = vec![
            String::new(),
            "AAAAA".to_owned(),
            "ÄÄÄÄÄ".to_owned(),
            "０００００".to_owned(), // full-width digits are not Crockford digits
            format!("{}X", g[challenge[0]]),
        ];
        for w in wrongs {
            let c = s.recovery_challenge().unwrap();
            let ans: Vec<_> = c.iter().map(|i| (*i, w.clone())).collect();
            assert!(!s.confirm_recovery_key(ans).unwrap(), "{w:?}");
            assert_eq!(s.pending_recovery_key().unwrap().as_str(), key);
            let c2 = s.recovery_challenge().unwrap();
            assert_eq!(c2.len(), CONFIRMATION_GROUPS);
            assert!(
                c2.windows(2).all(|p| p[0] < p[1]) && c2.iter().all(|i| *i < CONFIRMATION_POOL)
            );
        }
        // one wrong group among right ones is still wrong
        let c = s.recovery_challenge().unwrap();
        let mut ans: Vec<_> = c.iter().map(|i| (*i, g[*i].clone())).collect();
        let mut flipped = ans[2].1.clone().into_bytes();
        flipped[0] = if flipped[0] == b'A' { b'B' } else { b'A' };
        ans[2].1 = String::from_utf8(flipped).unwrap();
        assert!(!s.confirm_recovery_key(ans).unwrap());
        assert_eq!(drops().1, 0, "still pending");

        // case, hyphens, spaces and the Crockford substitutions are all accepted
        let c = s.recovery_challenge().unwrap();
        let ans: Vec<_> = c
            .iter()
            .map(|i| {
                let typed: String = g[*i]
                    .chars()
                    .map(|ch| match ch {
                        '1' => 'l',
                        '0' => 'o',
                        other => other.to_ascii_lowercase(),
                    })
                    .collect();
                (*i, format!(" {}-{} ", &typed[..2], &typed[2..]))
            })
            .collect();
        assert!(s.confirm_recovery_key(ans).unwrap());
        assert_eq!(s.state(), SessionState::Unlocked);
    }

    #[test]
    fn crockford_normalisation_matches_the_recovery_key_parser() {
        let n = |t: &str| String::from_utf8(normalize_group(t).to_vec()).unwrap();
        assert_eq!(n("ab-cd ef\t"), "ABCDEF");
        assert_eq!(n("IiLl"), "1111");
        assert_eq!(n("Oo0"), "000");
        assert_eq!(n("ÄÖ"), "ÄÖ", "non-ASCII is kept (and never matches)");
    }

    #[test]
    fn challenge_draws_are_distinct_sorted_and_roughly_uniform() {
        let mut hits = [0u32; CONFIRMATION_POOL];
        for _ in 0..3000 {
            let c = draw_challenge().unwrap();
            assert_eq!(c.len(), CONFIRMATION_GROUPS);
            assert!(c.windows(2).all(|w| w[0] < w[1]));
            for i in c {
                hits[i] += 1;
            }
        }
        // each group is picked with probability 1/2: expect 1500 +- a generous margin
        for h in hits {
            assert!((1300..=1700).contains(&h), "{hits:?}");
        }
    }

    #[test]
    fn random_below_has_no_modulo_bias_at_awkward_sizes() {
        let mut counts = [0u32; 5];
        for _ in 0..10_000 {
            counts[random_below(5).unwrap()] += 1;
        }
        for c in counts {
            assert!((1700..=2300).contains(&c), "{counts:?}");
        }
    }

    #[test]
    fn debug_output_never_contains_key_material() {
        let t = tempfile::tempdir().unwrap();
        let (s, rk) = create(&t.path().join("v"));
        let key = rk.recovery_key().to_string();
        let compact: String = key.chars().filter(|c| *c != '-').collect();
        for dbg in [
            format!("{rk:?}"),
            format!("{s:?}"),
            format!("{:?}", s.state()),
        ] {
            assert!(!dbg.contains(&key) && !dbg.contains(&compact), "{dbg}");
        }
        assert!(format!("{rk:?}").contains("redacted"));
    }
}
