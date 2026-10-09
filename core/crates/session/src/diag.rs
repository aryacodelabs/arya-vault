//! Read-only diagnostics: plaintext header facts and the pinned SQLCipher settings
//! (docs/14 §4.6 `info`, the CLI `info` command). No secrets.

use arya_vault_crypto::format::header::Header;

/// Facts from the active header (plaintext, readable while locked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderInfo {
    /// `format_version` of the header.
    pub format_version: u16,
    /// Monotonic header version.
    pub header_version: u32,
    /// Key epoch.
    pub epoch: u32,
    /// Vault id.
    pub vault_id: [u8; 16],
    /// Id of the device that wrote the header (from the file name).
    pub device_id: [u8; 16],
    /// Argon2id memory in KiB.
    pub kdf_m_kib: u32,
    /// Argon2id passes.
    pub kdf_t: u32,
    /// Argon2id parallelism.
    pub kdf_p: u32,
    /// Creation time, seconds since the Unix epoch.
    pub created_at: u64,
}

impl HeaderInfo {
    pub(crate) fn of(h: &Header, device_id: [u8; 16]) -> Self {
        Self {
            format_version: h.format_version,
            header_version: h.header_version,
            epoch: h.epoch,
            vault_id: h.vault_id,
            device_id,
            kdf_m_kib: h.kdf.m_kib,
            kdf_t: h.kdf.t,
            kdf_p: h.kdf.p,
            created_at: h.created_at,
        }
    }
}

/// `meta` keys that record the pinned SQLCipher/SQLite settings (docs/04 §1).
pub const PINNED_SETTING_KEYS: [&str; 10] = [
    "cipher.page_size",
    "cipher.compatibility",
    "cipher.kdf_algorithm",
    "cipher.hmac_algorithm",
    "cipher.kdf_iter",
    "cipher.plaintext_header_size",
    "sqlite.journal_mode",
    "sqlite.synchronous",
    "sqlite.foreign_keys",
    "sqlite.secure_delete",
];

/// Facts from the database (needs the unlocked session).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInfo {
    /// Current schema version of the file.
    pub schema_version: u32,
    /// `(key, value)` for each of [`PINNED_SETTING_KEYS`]; `None` if unset.
    pub pinned_settings: Vec<(&'static str, Option<String>)>,
}
