//! Error mapping: core errors -> `AppError` (docs/14 §2) with scrubbed messages.
//!
//! A message is either a static text or the `Display` of a typed core error; the core's errors
//! never carry secrets or echo input (they are tested for that), and nothing here formats a
//! caller-supplied value into a message. Two cases are scrubbed anyway: `Io` (whose text can
//! contain a path) and `Internal` (opaque by contract).

use arya_vault_session::{AppErrorCode as Core, SessionError};
use arya_vault_vault::{ImportError, VaultError};

use crate::api::dto::{AppError, AppErrorCode};

impl From<Core> for AppErrorCode {
    fn from(c: Core) -> Self {
        match c {
            Core::WrongCredentials => Self::WrongCredentials,
            Core::RecoveryKeyMalformed => Self::RecoveryKeyMalformed,
            Core::WeakPassword => Self::WeakPassword,
            Core::Locked => Self::Locked,
            Core::NotFound => Self::NotFound,
            Core::Validation => Self::Validation,
            Core::LimitReached => Self::LimitReached,
            Core::CorruptVault => Self::CorruptVault,
            Core::UnsupportedFormat => Self::UnsupportedFormat,
            Core::QuickUnlockUnavailable => Self::QuickUnlockUnavailable,
            Core::AlreadyExists => Self::AlreadyExists,
            Core::Io => Self::Io,
            Core::Busy => Self::Busy,
            Core::Internal => Self::Internal,
        }
    }
}

impl AppError {
    pub(crate) fn new(code: AppErrorCode, message: &str) -> Self {
        Self {
            code,
            message: message.to_owned(),
            field: None,
        }
    }

    pub(crate) fn with_field(mut self, field: &str) -> Self {
        self.field = Some(field.to_owned());
        self
    }

    /// Input rejected; names the field.
    pub(crate) fn validation(field: &str, message: &str) -> Self {
        Self::new(AppErrorCode::Validation, message).with_field(field)
    }

    /// Opaque: no details by contract.
    pub(crate) fn internal() -> Self {
        Self::new(AppErrorCode::Internal, "internal error")
    }
}

/// The input field a vault error is about, if it names one.
fn vault_field(e: &VaultError) -> Option<&'static str> {
    match e {
        VaultError::InvalidField { field, .. } => Some(field),
        VaultError::InvalidValue(w)
        | VaultError::LimitExceeded(w)
        | VaultError::SecretField(w)
        | VaultError::NotSecret(w) => Some(w),
        _ => None,
    }
}

impl From<SessionError> for AppError {
    fn from(e: SessionError) -> Self {
        let code = AppErrorCode::from(e.code());
        let message = match &e {
            SessionError::Io(_) => "I/O error".to_owned(),
            _ => e.public_message(),
        };
        let field = match &e {
            SessionError::Vault(v) => vault_field(v).map(str::to_owned),
            SessionError::InvalidConfirmation => Some("answers".to_owned()),
            _ => None,
        };
        Self {
            code,
            message,
            field,
        }
    }
}

impl From<VaultError> for AppError {
    fn from(e: VaultError) -> Self {
        SessionError::Vault(e).into()
    }
}

impl From<ImportError> for AppError {
    fn from(e: ImportError) -> Self {
        use ImportError as I;
        let (code, field) = match e {
            I::Vault(v) => return v.into(),
            I::FileTooLarge | I::TooManyRecords | I::FieldTooLarge | I::TooDeeplyNested => {
                (AppErrorCode::LimitReached, Some("file"))
            }
            I::InvalidEncoding
            | I::InvalidCsv
            | I::NoRecognizedColumns
            | I::InvalidJson
            | I::UnexpectedStructure
            | I::EncryptedBitwardenUnsupported
            | I::BadMagic
            | I::Malformed
            | I::InvalidKdf => (AppErrorCode::Validation, Some("file")),
            I::UnsupportedFormat { .. } => (AppErrorCode::UnsupportedFormat, None),
            I::AuthenticationFailed => (AppErrorCode::WrongCredentials, None),
            I::Io => (AppErrorCode::Io, None),
            _ => return AppError::internal(),
        };
        let mut out = AppError::new(code, &e.to_string());
        out.field = field.map(str::to_owned);
        out
    }
}

pub(crate) type ApiResult<T> = Result<T, AppError>;
