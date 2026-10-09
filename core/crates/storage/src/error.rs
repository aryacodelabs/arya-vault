use thiserror::Error;

/// Errors from the storage layer.
///
/// Messages never contain key material or stored values. SQLite messages are
/// limited to its own diagnostics (table/column names, error classes).
#[derive(Debug, Error)]
pub enum StorageError {
    /// The key is wrong, or the file is corrupt, truncated, not a database, or
    /// otherwise unreadable. Deliberately indistinguishable (SEC-S01): the
    /// caller must not learn which.
    #[error("wrong key or corrupt database")]
    WrongKeyOrCorrupt,
    /// The database file does not exist.
    #[error("database file not found")]
    NotFound,
    /// `create` was asked to overwrite an existing file.
    #[error("database file already exists")]
    AlreadyExists,
    /// A pinned setting differs from what this build requires (SEC-C13). Never
    /// a silent fallback.
    #[error(
        "pinned setting `{name}` has unexpected value (expected `{expected}`, found `{found}`)"
    )]
    SettingsMismatch {
        /// Setting name, e.g. `cipher_page_size`.
        name: &'static str,
        /// Required value.
        expected: String,
        /// Value found.
        found: String,
    },
    /// The linked SQLite is not SQLCipher (no `cipher_version`), so data would be plaintext.
    #[error("SQLCipher is not available in this build")]
    NotSqlCipher,
    /// The database was written by a newer version; downgrade is unsupported (docs/12 section 7).
    #[error("database schema version {found} is newer than supported version {supported}")]
    SchemaTooNew {
        /// Version found in the file.
        found: u32,
        /// Highest version this build supports.
        supported: u32,
    },
    /// A migration failed and was rolled back; the database is unchanged.
    #[error("migration to schema version {version} failed and was rolled back")]
    MigrationFailed {
        /// Version being applied.
        version: u32,
    },
    /// `PRAGMA integrity_check` / `foreign_key_check` reported problems.
    #[error("integrity check failed: {0:?}")]
    IntegrityFailed(Vec<String>),
    /// The database is locked by another connection (after `busy_timeout`).
    #[error("database is busy")]
    Busy,
    /// Disk or database is full.
    #[error("storage is full")]
    Full,
    /// The file or directory is read-only.
    #[error("storage is read-only")]
    ReadOnly,
    /// A constraint (unique, foreign key, ...) was violated.
    #[error("constraint violation: {0}")]
    Constraint(String),
    /// A key of the wrong length was supplied.
    #[error("invalid key length")]
    InvalidKeyLength,
    /// Re-keying failed.
    #[error("rekey failed")]
    RekeyFailed,
    /// Filesystem error.
    #[error("i/o error: {0:?}")]
    Io(std::io::ErrorKind),
    /// Any other SQLite error (diagnostic text only).
    #[error("sqlite error: {0}")]
    Sqlite(String),
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        match e.kind() {
            std::io::ErrorKind::NotFound => Self::NotFound,
            std::io::ErrorKind::AlreadyExists => Self::AlreadyExists,
            std::io::ErrorKind::PermissionDenied => Self::ReadOnly,
            k => Self::Io(k),
        }
    }
}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode as C;
        if let rusqlite::Error::SqliteFailure(f, msg) = &e {
            return match f.code {
                C::NotADatabase => Self::WrongKeyOrCorrupt,
                C::DatabaseBusy | C::DatabaseLocked => Self::Busy,
                C::DiskFull => Self::Full,
                C::ReadOnly => Self::ReadOnly,
                C::ConstraintViolation => {
                    Self::Constraint(msg.clone().unwrap_or_else(|| "constraint".into()))
                }
                C::DatabaseCorrupt => Self::WrongKeyOrCorrupt,
                _ => Self::Sqlite(e.to_string()),
            };
        }
        Self::Sqlite(e.to_string())
    }
}
