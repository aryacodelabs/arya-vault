use arya_vault_storage::StorageError;
use thiserror::Error;

use crate::hlc::HlcError;

/// Errors from the vault layer. Messages never contain field values or secrets.
#[derive(Debug, Error)]
pub enum VaultError {
    /// The storage layer failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A clock problem (see [`HlcError`]).
    #[error(transparent)]
    Clock(#[from] HlcError),
    /// No such item, folder or element.
    #[error("not found")]
    NotFound,
    /// The item is in the trash; restore it first.
    #[error("item is in the trash")]
    InTrash,
    /// The item is not in the trash.
    #[error("item is not in the trash")]
    NotInTrash,
    /// The field does not exist for this item type.
    #[error("field `{field}` is not valid for item type `{item_type}`")]
    InvalidField {
        /// Field name.
        field: &'static str,
        /// Item type name.
        item_type: &'static str,
    },
    /// A secret field was read through a non-secret accessor (use `reveal`).
    #[error("field `{0}` is secret; use reveal")]
    SecretField(&'static str),
    /// `reveal` was used on a field that is not secret.
    #[error("field `{0}` is not secret")]
    NotSecret(&'static str),
    /// A documented limit (docs/05 section 10) would be exceeded.
    #[error("limit exceeded: {0}")]
    LimitExceeded(&'static str),
    /// A value is not acceptable (empty tag, control characters, bad id...).
    #[error("invalid value: {0}")]
    InvalidValue(&'static str),
    /// Stored data could not be decoded (corrupt or from a newer version).
    #[error("stored data is corrupt")]
    Corrupt,
    /// The OS random source failed.
    #[error("random source failure")]
    Rng,
    /// Injected failure (tests only).
    #[cfg(test)]
    #[error("injected fault")]
    Injected,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, VaultError>;
