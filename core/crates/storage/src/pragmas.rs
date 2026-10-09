//! Pinned SQLCipher / SQLite settings (docs/04 section 1, SEC-C13).

use rusqlite::Connection;

use crate::error::StorageError;
use crate::key::DbKey;

/// `cipher_page_size`.
pub const CIPHER_PAGE_SIZE: u32 = 4096;
/// `cipher_compatibility` (SQLCipher 4 defaults, set explicitly).
pub const CIPHER_COMPATIBILITY: u32 = 4;
/// `cipher_kdf_algorithm` (unused in raw-key mode but pinned and recorded).
pub const CIPHER_KDF_ALGORITHM: &str = "PBKDF2_HMAC_SHA512";
/// `cipher_hmac_algorithm`.
pub const CIPHER_HMAC_ALGORITHM: &str = "HMAC_SHA512";
/// `kdf_iter` (unused in raw-key mode but pinned and recorded).
pub const KDF_ITER: u32 = 256_000;
/// `cipher_plaintext_header_size`: 0 = the whole file incl. header is encrypted.
pub const CIPHER_PLAINTEXT_HEADER_SIZE: u32 = 0;
/// `busy_timeout` in milliseconds: a connection waits this long for a lock
/// before returning [`StorageError::Busy`]. WAL readers never block the writer.
pub const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Settings recorded in `meta` on create and verified on open: `(meta key, value)`.
pub(crate) const PINNED: &[(&str, &str)] = &[
    ("cipher.page_size", "4096"),
    ("cipher.compatibility", "4"),
    ("cipher.kdf_algorithm", CIPHER_KDF_ALGORITHM),
    ("cipher.hmac_algorithm", CIPHER_HMAC_ALGORITHM),
    ("cipher.kdf_iter", "256000"),
    ("cipher.plaintext_header_size", "0"),
    ("sqlite.journal_mode", "wal"),
    ("sqlite.synchronous", "2"),
    ("sqlite.foreign_keys", "1"),
    ("sqlite.secure_delete", "1"),
];

fn query_string(conn: &Connection, pragma: &str) -> Result<String, StorageError> {
    let v: rusqlite::types::Value =
        conn.query_row(&format!("PRAGMA {pragma}"), [], |r| r.get(0))?;
    Ok(match v {
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Text(t) => t,
        _ => String::new(),
    })
}

fn expect(
    conn: &Connection,
    name: &'static str,
    pragma: &str,
    expected: &str,
) -> Result<(), StorageError> {
    let found = query_string(conn, pragma)?;
    if found.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(StorageError::SettingsMismatch {
            name,
            expected: expected.to_owned(),
            found,
        })
    }
}

/// Apply the key and the cipher settings. Must run before the first read.
pub(crate) fn apply_cipher(conn: &Connection, key: &DbKey) -> Result<(), StorageError> {
    // SQLCipher logs decrypt failures to stderr by default; a wrong key is an
    // expected, typed error here, not log noise.
    conn.execute_batch("PRAGMA cipher_log_level = NONE;")?;
    let lit = key.raw_literal();
    let stmt = zeroize::Zeroizing::new(format!("PRAGMA key = \"{}\";", lit.as_str()));
    conn.execute_batch(&stmt)?;
    // The linked library must be SQLCipher; otherwise PRAGMA key is a silent no-op.
    let ver: Option<String> = conn
        .query_row("PRAGMA cipher_version", [], |r| r.get(0))
        .ok();
    if ver.as_deref().is_none_or(str::is_empty) {
        return Err(StorageError::NotSqlCipher);
    }
    // Compatibility first (it resets related defaults), then every value explicitly.
    conn.execute_batch(&format!(
        "PRAGMA cipher_compatibility = {CIPHER_COMPATIBILITY};
         PRAGMA cipher_page_size = {CIPHER_PAGE_SIZE};
         PRAGMA cipher_kdf_algorithm = {CIPHER_KDF_ALGORITHM};
         PRAGMA cipher_hmac_algorithm = {CIPHER_HMAC_ALGORITHM};
         PRAGMA kdf_iter = {KDF_ITER};
         PRAGMA cipher_plaintext_header_size = {CIPHER_PLAINTEXT_HEADER_SIZE};
         PRAGMA cipher_memory_security = ON;"
    ))?;
    Ok(())
}

/// Detect a wrong key / non-database: the first real read fails or finds no schema.
pub(crate) fn probe(conn: &Connection) -> Result<(), StorageError> {
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })
    .map(|_| ())
    .map_err(|_| StorageError::WrongKeyOrCorrupt)
}

/// Apply per-connection SQLite settings.
pub(crate) fn apply_connection(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(&format!(
        "PRAGMA busy_timeout = {BUSY_TIMEOUT_MS};
         PRAGMA temp_store = MEMORY;
         PRAGMA foreign_keys = ON;
         PRAGMA secure_delete = ON;
         PRAGMA synchronous = FULL;"
    ))?;
    Ok(())
}

/// Verify the live settings match the pinned ones (no fallback).
pub(crate) fn verify_live(conn: &Connection) -> Result<(), StorageError> {
    expect(conn, "cipher_page_size", "cipher_page_size", "4096")?;
    expect(
        conn,
        "cipher_kdf_algorithm",
        "cipher_kdf_algorithm",
        CIPHER_KDF_ALGORITHM,
    )?;
    expect(
        conn,
        "cipher_hmac_algorithm",
        "cipher_hmac_algorithm",
        CIPHER_HMAC_ALGORITHM,
    )?;
    expect(conn, "kdf_iter", "kdf_iter", "256000")?;
    expect(
        conn,
        "cipher_plaintext_header_size",
        "cipher_plaintext_header_size",
        "0",
    )?;
    expect(conn, "journal_mode", "journal_mode", "wal")?;
    expect(conn, "synchronous", "synchronous", "2")?;
    expect(conn, "foreign_keys", "foreign_keys", "1")?;
    expect(conn, "secure_delete", "secure_delete", "1")?;
    expect(conn, "temp_store", "temp_store", "2")?;
    Ok(())
}

/// Verify the settings recorded in `meta` equal the pinned ones.
pub(crate) fn verify_recorded(conn: &Connection) -> Result<(), StorageError> {
    for (name, expected) in PINNED {
        let found: Option<Vec<u8>> = conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [name], |r| {
                r.get(0)
            })
            .ok();
        let found = found.map(|v| String::from_utf8_lossy(&v).into_owned());
        if found.as_deref() != Some(*expected) {
            return Err(StorageError::SettingsMismatch {
                name,
                expected: (*expected).to_owned(),
                found: found.unwrap_or_else(|| "<missing>".into()),
            });
        }
    }
    Ok(())
}
