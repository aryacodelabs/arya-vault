//! The CLI error type and process exit codes.
//!
//! Messages are static text or the `Display` of typed errors from the core crates, which never
//! contain secrets or echo input.

use std::fmt;

/// Process exit codes (stable; scripts may rely on them).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// Any other failure.
    Failure = 1,
    /// Bad command line (also used for missing `--reveal` / acknowledgements).
    Usage = 2,
    /// Wrong password / recovery key, or a tampered header or file.
    Auth = 3,
    /// The vault directory is missing, corrupt or from an unsupported version.
    Vault = 4,
}

/// A failed command.
#[derive(Debug)]
pub struct CliError {
    /// Exit code.
    pub exit: Exit,
    /// Stable machine-readable code (`--json`).
    pub code: &'static str,
    /// Human-readable, secret-free message.
    pub message: String,
}

impl CliError {
    pub fn new(exit: Exit, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            exit,
            code,
            message: message.into(),
        }
    }

    pub fn usage(message: impl Into<String>) -> Self {
        Self::new(Exit::Usage, "usage", message)
    }

    pub fn failure(message: impl Into<String>) -> Self {
        Self::new(Exit::Failure, "error", message)
    }

    pub fn vault(message: impl Into<String>) -> Self {
        Self::new(Exit::Vault, "vault", message)
    }

    pub fn auth() -> Self {
        Self::new(
            Exit::Auth,
            "authentication_failed",
            "wrong password or recovery key, or the vault header was modified",
        )
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

pub type Result<T> = std::result::Result<T, CliError>;

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        // `io::Error`'s Display can include paths but never vault content.
        Self::failure(format!("I/O error: {e}"))
    }
}

impl From<arya_vault_session::SessionError> for CliError {
    fn from(e: arya_vault_session::SessionError) -> Self {
        use arya_vault_session::SessionError as E;
        match e {
            E::WrongCredentials => Self::auth(),
            E::RecoveryKeyMalformed | E::WeakPassword(_) => Self::usage(e.to_string()),
            E::NoVault => Self::vault("no readable vault header found in the vault directory"),
            E::CorruptVault(_) | E::UnsupportedFormat { .. } => Self::vault(e.to_string()),
            E::Storage(s) => Self::vault(s.to_string()),
            E::Io(io) => io.into(),
            other => Self::failure(other.to_string()),
        }
    }
}

impl From<arya_vault_vault::VaultError> for CliError {
    fn from(e: arya_vault_vault::VaultError) -> Self {
        Self::failure(e.to_string())
    }
}

impl From<arya_vault_vault::ImportError> for CliError {
    fn from(e: arya_vault_vault::ImportError) -> Self {
        use arya_vault_vault::ImportError as E;
        match e {
            E::AuthenticationFailed => Self::new(
                Exit::Auth,
                "authentication_failed",
                "wrong export password, or the export file was modified",
            ),
            other => Self::failure(other.to_string()),
        }
    }
}

impl From<arya_vault_crypto::kdf::KdfError> for CliError {
    fn from(e: arya_vault_crypto::kdf::KdfError) -> Self {
        Self::failure(e.to_string())
    }
}

impl From<arya_vault_crypto::rng::RngError> for CliError {
    fn from(e: arya_vault_crypto::rng::RngError) -> Self {
        Self::failure(e.to_string())
    }
}

impl From<arya_vault_generator::GeneratorError> for CliError {
    fn from(e: arya_vault_generator::GeneratorError) -> Self {
        Self::usage(e.to_string())
    }
}
