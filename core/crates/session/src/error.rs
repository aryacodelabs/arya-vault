//! [`SessionError`] and its mapping to the app-facing [`AppErrorCode`]s of docs/14 §2.
//!
//! Messages are static text or the `Display` of typed errors from the core crates, which never
//! contain secrets or echo input.

use arya_vault_crypto::format::FormatError;
use arya_vault_crypto::kdf::KdfError;
use arya_vault_crypto::vault_key::VaultKeyError;
use arya_vault_storage::StorageError;
use arya_vault_vault::VaultError;
use thiserror::Error;

/// The error codes of docs/14 §2 (`AppErrorCode`), minus `quickUnlockUnavailable`, which
/// belongs to task A02.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppErrorCode {
    /// Wrong master password or recovery key.
    WrongCredentials,
    /// Typo or bad checksum in a recovery key.
    RecoveryKeyMalformed,
    /// The master password fails the policy.
    WeakPassword,
    /// The operation requires an unlocked session.
    Locked,
    /// Item, folder or vault unknown.
    NotFound,
    /// Input rejected.
    Validation,
    /// A field, item or size limit was reached.
    LimitReached,
    /// Authentication or integrity failure of vault data.
    CorruptVault,
    /// The data is from a newer format than this build understands.
    UnsupportedFormat,
    /// The target already exists.
    AlreadyExists,
    /// Filesystem failure.
    Io,
    /// The resource is busy (or a local failure delay is running).
    Busy,
    /// Anything else; the message carries no details.
    Internal,
}

impl AppErrorCode {
    /// The canonical camelCase name used by docs/14.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WrongCredentials => "wrongCredentials",
            Self::RecoveryKeyMalformed => "recoveryKeyMalformed",
            Self::WeakPassword => "weakPassword",
            Self::Locked => "locked",
            Self::NotFound => "notFound",
            Self::Validation => "validation",
            Self::LimitReached => "limitReached",
            Self::CorruptVault => "corruptVault",
            Self::UnsupportedFormat => "unsupportedFormat",
            Self::AlreadyExists => "alreadyExists",
            Self::Io => "io",
            Self::Busy => "busy",
            Self::Internal => "internal",
        }
    }
}

/// Everything a [`crate::Session`] operation can fail with.
#[derive(Debug, Error)]
pub enum SessionError {
    /// The operation requires an unlocked session.
    #[error("the vault is locked")]
    Locked,
    /// The session is already unlocked.
    #[error("the vault is already unlocked")]
    AlreadyUnlocked,
    /// No vault exists in the directory.
    #[error("no vault exists in this directory")]
    NoVault,
    /// The directory already contains a vault.
    #[error("the vault directory already contains a vault")]
    AlreadyExists,
    /// Wrong password or recovery key, or a modified header (indistinguishable by design).
    #[error("wrong password or recovery key, or the vault header was modified")]
    WrongCredentials,
    /// The recovery key text is not well-formed.
    #[error("that is not a valid recovery key (check for typos)")]
    RecoveryKeyMalformed,
    /// The new master password violates the policy (SEC-A07); one entry per violated rule.
    #[error("master password rejected: {}", .0.join("; "))]
    WeakPassword(Vec<String>),
    /// Too many wrong passwords; no attempt was made. Cosmetic local delay (docs/07 §4).
    #[error("too many failed attempts; try again in {retry_after_ms} ms")]
    Backoff {
        /// Milliseconds until the next attempt is accepted.
        retry_after_ms: u64,
    },
    /// No recovery key is waiting for confirmation (it was dropped by `lock` or never created).
    #[error("no recovery key is waiting for confirmation; generate a new one")]
    NoPendingRecoveryKey,
    /// The confirmation answers do not cover exactly the requested groups.
    #[error("the confirmation answers do not match the requested groups")]
    InvalidConfirmation,
    /// The vault directory is damaged or incomplete.
    #[error("{0}")]
    CorruptVault(&'static str),
    /// A header uses a format newer than this build understands.
    #[error("unsupported format version {found} (this build supports up to {max_supported})")]
    UnsupportedFormat {
        /// Version found.
        found: u16,
        /// Highest version this build reads.
        max_supported: u16,
    },
    /// The storage layer failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The vault layer failed.
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// Key derivation failed or the parameters are out of range.
    #[error(transparent)]
    Kdf(#[from] KdfError),
    /// The OS random source failed.
    #[error(transparent)]
    Rng(#[from] arya_vault_crypto::rng::RngError),
    /// Sub-key derivation failed.
    #[error(transparent)]
    Hkdf(#[from] arya_vault_crypto::hkdf::HkdfError),
    /// Filesystem error. `io::Error`'s text can include paths but never vault content.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// An unexpected condition; the text is static and secret-free.
    #[error("internal error: {0}")]
    Internal(&'static str),
}

impl From<VaultKeyError> for SessionError {
    fn from(e: VaultKeyError) -> Self {
        match e {
            VaultKeyError::Wrap(_) => Self::WrongCredentials,
            VaultKeyError::Kdf(k) => Self::Kdf(k),
            VaultKeyError::Hkdf(h) => Self::Hkdf(h),
            VaultKeyError::Rng(r) => Self::Rng(r),
            VaultKeyError::SaltNotRefreshed => Self::Internal("a fresh KDF salt is required"),
        }
    }
}

impl From<FormatError> for SessionError {
    fn from(e: FormatError) -> Self {
        match e {
            FormatError::UnsupportedFormat {
                found,
                max_supported,
            } => Self::UnsupportedFormat {
                found,
                max_supported,
            },
            _ => Self::CorruptVault("unreadable vault header"),
        }
    }
}

fn storage_code(e: &StorageError) -> AppErrorCode {
    match e {
        StorageError::WrongKeyOrCorrupt
        | StorageError::NotFound
        | StorageError::SettingsMismatch { .. }
        | StorageError::IntegrityFailed(_) => AppErrorCode::CorruptVault,
        StorageError::SchemaTooNew { .. } => AppErrorCode::UnsupportedFormat,
        StorageError::AlreadyExists => AppErrorCode::AlreadyExists,
        StorageError::Busy => AppErrorCode::Busy,
        StorageError::Full | StorageError::ReadOnly | StorageError::Io(_) => AppErrorCode::Io,
        _ => AppErrorCode::Internal,
    }
}

fn vault_code(e: &VaultError) -> AppErrorCode {
    match e {
        VaultError::Storage(s) => storage_code(s),
        VaultError::NotFound => AppErrorCode::NotFound,
        VaultError::InTrash
        | VaultError::NotInTrash
        | VaultError::InvalidField { .. }
        | VaultError::SecretField(_)
        | VaultError::NotSecret(_)
        | VaultError::InvalidValue(_) => AppErrorCode::Validation,
        VaultError::LimitExceeded(_) => AppErrorCode::LimitReached,
        VaultError::Corrupt => AppErrorCode::CorruptVault,
        _ => AppErrorCode::Internal,
    }
}

impl SessionError {
    /// The docs/14 §2 code the app shows for this error.
    #[must_use]
    pub fn code(&self) -> AppErrorCode {
        match self {
            Self::Locked => AppErrorCode::Locked,
            Self::AlreadyUnlocked | Self::NoPendingRecoveryKey | Self::InvalidConfirmation => {
                AppErrorCode::Validation
            }
            Self::NoVault => AppErrorCode::NotFound,
            Self::AlreadyExists => AppErrorCode::AlreadyExists,
            Self::WrongCredentials => AppErrorCode::WrongCredentials,
            Self::RecoveryKeyMalformed => AppErrorCode::RecoveryKeyMalformed,
            Self::WeakPassword(_) => AppErrorCode::WeakPassword,
            // docs/14 has no code for a running delay; see "Spec questions" in the A01 PR.
            Self::Backoff { .. } => AppErrorCode::Busy,
            Self::CorruptVault(_) => AppErrorCode::CorruptVault,
            Self::UnsupportedFormat { .. } => AppErrorCode::UnsupportedFormat,
            Self::Storage(s) => storage_code(s),
            Self::Vault(v) => vault_code(v),
            Self::Kdf(KdfError::OutOfRange(_)) => AppErrorCode::CorruptVault,
            Self::Kdf(_) | Self::Rng(_) | Self::Hkdf(_) | Self::Internal(_) => {
                AppErrorCode::Internal
            }
            Self::Io(_) => AppErrorCode::Io,
        }
    }

    /// A message that is safe to show or log. For [`AppErrorCode::Internal`] it carries no
    /// details (docs/14 §2: "`internal` carries an opaque code, no details").
    #[must_use]
    pub fn public_message(&self) -> String {
        if self.code() == AppErrorCode::Internal {
            "internal error".to_owned()
        } else {
            self.to_string()
        }
    }

    /// Milliseconds to wait, if this is a [`SessionError::Backoff`].
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Self::Backoff { retry_after_ms } => Some(*retry_after_ms),
            _ => None,
        }
    }
}

/// Result alias.
pub type Result<T> = core::result::Result<T, SessionError>;

#[cfg(test)]
mod tests {
    use arya_vault_crypto::kdf::KdfParam;

    use super::*;

    fn code(e: SessionError) -> &'static str {
        e.code().as_str()
    }

    #[test]
    fn maps_to_the_codes_of_docs_14_section_2() {
        let cases: Vec<(SessionError, &str)> = vec![
            (SessionError::Locked, "locked"),
            (SessionError::AlreadyUnlocked, "validation"),
            (SessionError::NoPendingRecoveryKey, "validation"),
            (SessionError::InvalidConfirmation, "validation"),
            (SessionError::NoVault, "notFound"),
            (SessionError::AlreadyExists, "alreadyExists"),
            (SessionError::WrongCredentials, "wrongCredentials"),
            (SessionError::RecoveryKeyMalformed, "recoveryKeyMalformed"),
            (SessionError::WeakPassword(vec!["x".into()]), "weakPassword"),
            (SessionError::Backoff { retry_after_ms: 5 }, "busy"),
            (SessionError::CorruptVault("x"), "corruptVault"),
            (
                SessionError::UnsupportedFormat {
                    found: 2,
                    max_supported: 1,
                },
                "unsupportedFormat",
            ),
            (StorageError::WrongKeyOrCorrupt.into(), "corruptVault"),
            (StorageError::NotFound.into(), "corruptVault"),
            (
                StorageError::SchemaTooNew {
                    found: 9,
                    supported: 1,
                }
                .into(),
                "unsupportedFormat",
            ),
            (StorageError::Busy.into(), "busy"),
            (StorageError::Full.into(), "io"),
            (StorageError::ReadOnly.into(), "io"),
            (StorageError::RekeyFailed.into(), "internal"),
            (VaultError::Corrupt.into(), "corruptVault"),
            (VaultError::NotFound.into(), "notFound"),
            (VaultError::LimitExceeded("x").into(), "limitReached"),
            (VaultError::InvalidValue("x").into(), "validation"),
            (VaultError::Storage(StorageError::Busy).into(), "busy"),
            (
                KdfError::OutOfRange(KdfParam::Memory).into(),
                "corruptVault",
            ),
            (KdfError::OutOfMemory.into(), "internal"),
            (
                std::io::Error::from(std::io::ErrorKind::PermissionDenied).into(),
                "io",
            ),
            (SessionError::Internal("x"), "internal"),
        ];
        for (e, want) in cases {
            let shown = format!("{e:?}");
            assert_eq!(code(e), want, "{shown}");
        }
    }

    #[test]
    fn crypto_wrap_failures_are_wrong_credentials() {
        use arya_vault_crypto::wrap::WrapError;
        let e: SessionError = VaultKeyError::Wrap(WrapError::AuthenticationFailed).into();
        assert_eq!(e.code(), AppErrorCode::WrongCredentials);
    }

    #[test]
    fn internal_errors_carry_no_details() {
        let e = SessionError::Storage(StorageError::Sqlite("secret table name".into()));
        assert_eq!(e.code(), AppErrorCode::Internal);
        assert_eq!(e.public_message(), "internal error");
        assert_eq!(SessionError::Locked.public_message(), "the vault is locked");
    }

    #[test]
    fn code_names_are_the_documented_camel_case() {
        use AppErrorCode as C;
        let all = [
            (C::WrongCredentials, "wrongCredentials"),
            (C::RecoveryKeyMalformed, "recoveryKeyMalformed"),
            (C::WeakPassword, "weakPassword"),
            (C::Locked, "locked"),
            (C::NotFound, "notFound"),
            (C::Validation, "validation"),
            (C::LimitReached, "limitReached"),
            (C::CorruptVault, "corruptVault"),
            (C::UnsupportedFormat, "unsupportedFormat"),
            (C::AlreadyExists, "alreadyExists"),
            (C::Io, "io"),
            (C::Busy, "busy"),
            (C::Internal, "internal"),
        ];
        for (c, s) in all {
            assert_eq!(c.as_str(), s);
        }
    }
}
