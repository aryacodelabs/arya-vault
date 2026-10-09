//! Header, envelope and container formats (docs/04 §5-7, §14; docs/06 §3, §6).
//!
//! Everything here parses **untrusted** bytes (cloud files): all parsers are bounded,
//! return typed errors and never panic (SEC-Y05); each has a fuzz target in `core/fuzz`.
//!
//! Modules: [`cbor`] (canonical codec), [`header`] (vault header + ordering),
//! [`envelope`] (encrypted container), [`padding`] (ISO/IEC 7816-4), [`path`]
//! (remote file-name grammar and path binding).

use thiserror::Error;

use crate::aead::AeadError;
use crate::kdf::KdfError;
use crate::rng::RngError;

pub mod cbor;
pub mod envelope;
pub mod header;
pub mod padding;
pub mod path;

/// The current (and only) on-disk/on-wire `format_version`.
pub const FORMAT_VERSION: u16 = 1;
/// The highest `format_version` this build can read. Readers accept exactly the versions
/// in `1..=MAX_SUPPORTED_FORMAT_VERSION` (doc 04 §14).
pub const MAX_SUPPORTED_FORMAT_VERSION: u16 = 1;

/// Maximum accepted size of an encoded header file in bytes (a v1 header is ~300 bytes).
pub const MAX_HEADER_BYTES: usize = 2048;
/// Maximum segment plaintext in bytes before padding (doc T02 §7: 1 MiB).
pub const MAX_SEGMENT_PLAINTEXT: usize = 1 << 20;
/// Maximum manifest plaintext in bytes before padding (not fixed by the spec; conservative).
pub const MAX_MANIFEST_PLAINTEXT: usize = 256 << 10;
/// Maximum snapshot plaintext in bytes before padding, until snapshot streaming exists.
pub const MAX_SNAPSHOT_PLAINTEXT: usize = 64 << 20;

/// Errors from parsing or building formats.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FormatError {
    /// The data uses a `format_version` this build does not understand. The app uses this
    /// to enter "update required" mode (docs/12 §6); never attempt to read such data.
    #[error("unsupported format version {found} (this build supports up to {max_supported})")]
    UnsupportedFormat {
        /// The version found in the data.
        found: u16,
        /// The highest version this build supports.
        max_supported: u16,
    },
    /// Wrong magic bytes.
    #[error("bad magic")]
    BadMagic,
    /// Fewer bytes than the format requires.
    #[error("truncated input")]
    Truncated,
    /// More bytes than the format allows.
    #[error("trailing bytes")]
    TrailingBytes,
    /// A size exceeds the format's limits (checked before allocation).
    #[error("input exceeds size limit")]
    TooLarge,
    /// A field is missing, has the wrong type, or has a disallowed value.
    #[error("invalid field: {0}")]
    InvalidField(&'static str),
    /// A CBOR-level problem.
    #[error("invalid CBOR: {0}")]
    Cbor(#[from] cbor::CborError),
    /// KDF parameters are out of range (SEC-C11); checked at parse time.
    #[error(transparent)]
    InvalidKdf(KdfError),
    /// The KDF algorithm identifier or version is not supported.
    #[error("unsupported KDF algorithm")]
    UnsupportedKdf,
    /// A remote path does not match the exact file-name grammar.
    #[error("invalid remote path")]
    BadPath,
    /// The envelope's identity does not match the path it was found at (SEC-Y10);
    /// quarantine the file.
    #[error("envelope does not match its path")]
    PathMismatch,
    /// The envelope belongs to a different vault.
    #[error("envelope belongs to a different vault")]
    WrongVault,
    /// Plaintext is not validly padded (reported only after authentication).
    #[error("invalid padding")]
    Padding,
    /// Authentication or other AEAD failure.
    #[error(transparent)]
    Aead(#[from] AeadError),
    /// The hash chain does not link.
    #[error("hash chain mismatch")]
    ChainMismatch,
    /// Randomness failure.
    #[error(transparent)]
    Rng(#[from] RngError),
}

/// Returns the `format_version` found in `found` or an `UnsupportedFormat` error.
pub(crate) fn check_version(found: u64) -> Result<u16, FormatError> {
    match u16::try_from(found) {
        Ok(v) if (1..=MAX_SUPPORTED_FORMAT_VERSION).contains(&v) => Ok(v),
        _ => Err(FormatError::UnsupportedFormat {
            found: u16::try_from(found).unwrap_or(u16::MAX),
            max_supported: MAX_SUPPORTED_FORMAT_VERSION,
        }),
    }
}
