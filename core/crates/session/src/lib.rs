//! Vault lifecycle and session state machine (docs/04 §2-5, §8-9; docs/07 §3-5; docs/14 §4.1, §5).
//!
//! This crate is the single owner of "which key opens what, in which order, and where the bytes
//! go": it creates a vault, unlocks it, locks it, changes the master password, recovers with the
//! recovery key and replaces the recovery key. The CLI and the FFI layer are thin clients.
//!
//! * No UI, no FFI, no networking, no cryptography of its own: every primitive is a call into
//!   `arya-vault-crypto`.
//! * [`Session`] is the state machine of docs/14 §5. The unlocked vault is reachable only through
//!   [`Session::with_vault`], which returns `locked` after [`Session::lock`].
//! * Types that hold key material have no `Debug` (or a redacted one); recovery-key text is only
//!   available as [`zeroize::Zeroizing`] through explicit accessors.
//! * Errors are [`SessionError`]; [`SessionError::code`] maps them to the `AppErrorCode`s of
//!   docs/14 §2.
//!
//! # Concurrency
//! One `Session` per vault directory per process is the supported shape. Two processes (or two
//! sessions) can open the same directory: SQLite (WAL) arbitrates concurrent writers and reports
//! `Busy` when it cannot get the write lock in time, but nothing here stops two unlocked sessions
//! from sharing one device id and clock. See the `docs` note in `tests/concurrency.rs` and the
//! A01 PR ("Spec questions").

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod backoff;
pub mod diag;
pub mod error;
pub mod layout;
mod lifecycle;
pub mod meta;
pub mod profile;
pub mod quick;
pub mod rotation;
mod state;

pub use backoff::{BackoffPolicy, Clock, FailureBackoff, MonotonicClock};
pub use diag::{DbInfo, HeaderInfo, PINNED_SETTING_KEYS};
pub use error::{AppErrorCode, Result, SessionError};
pub use profile::KdfProfile;
pub use quick::{
    Blob, NoProvider, PolicyRecord, ProviderError, QuickUnlockConfig, QuickUnlockDenied,
    QuickUnlockKind, QuickUnlockProvider, QuickUnlockStatus, SystemWallClock, WallClock,
};
pub use rotation::{RotationRecord, Step as RotationStep};
pub use state::{
    CONFIRMATION_GROUPS, CONFIRMATION_POOL, LockRequestHandle, RECOVERY_KEY_GROUPS,
    RecoveryConfirmation, RecoveryKeyResult, RotationOutcome, Session, SessionConfig, SessionState,
    VaultStatus,
};

#[cfg(test)]
pub(crate) mod probe;
