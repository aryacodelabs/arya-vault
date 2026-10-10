//! Embedded, forward-only migrations and the pre-migration encrypted backup
//! (docs/05 section 5, docs/12 section 7).

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension, TransactionBehavior, backup::Backup};

use crate::error::StorageError;
use crate::key::DbKey;
use crate::pragmas;

/// How long pre-migration backups are kept (docs/12 section 7).
pub const BACKUP_RETENTION: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// One schema migration.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Migration {
    /// Version after this migration is applied (strictly increasing from 1).
    pub version: u32,
    /// SQL to execute inside one transaction.
    pub sql: &'static str,
}

/// All migrations shipped in this build, oldest first.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        sql: include_str!("migrations/v001_initial.sql"),
    },
    Migration {
        version: 2,
        sql: include_str!("migrations/v002_title_favorite_indexes.sql"),
    },
];

/// Highest schema version this build writes.
#[must_use]
pub fn latest_schema_version() -> u32 {
    MIGRATIONS.last().map_or(0, |m| m.version)
}

/// Current `schema_version`, or `None` if the `meta` table does not exist.
pub(crate) fn read_version(conn: &Connection) -> Result<Option<u32>, StorageError> {
    let has_meta: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'meta')",
        [],
        |r| r.get(0),
    )?;
    if !has_meta {
        return Ok(None);
    }
    let raw: Option<Vec<u8>> = conn
        .query_row(
            "SELECT value FROM meta WHERE key = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    match raw {
        None => Ok(None),
        Some(b) => String::from_utf8(b)
            .ok()
            .and_then(|s| s.parse().ok())
            .map(Some)
            .ok_or(StorageError::WrongKeyOrCorrupt),
    }
}

/// Apply every migration newer than `from`, each in its own immediate
/// transaction. A failing migration rolls back and leaves the previous
/// version intact. Re-checks the version inside the transaction so two
/// processes opening at once cannot apply the same migration twice.
pub(crate) fn migrate(
    conn: &mut Connection,
    migrations: &[Migration],
) -> Result<u32, StorageError> {
    let mut current = read_version(conn)?.unwrap_or(0);
    for m in migrations {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        current = read_version(&tx)?.unwrap_or(0);
        if current >= m.version {
            tx.commit()?;
            continue;
        }
        let version = m.version;
        tx.execute_batch(m.sql)
            .map_err(|_| StorageError::MigrationFailed { version })?;
        tx.execute(
            "INSERT INTO meta(key, value) VALUES ('schema_version', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [version.to_string().as_bytes()],
        )
        .map_err(|_| StorageError::MigrationFailed { version })?;
        tx.commit()
            .map_err(|_| StorageError::MigrationFailed { version })?;
        current = version;
    }
    Ok(current)
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn backup_prefix(db_path: &Path) -> Option<String> {
    Some(format!("{}.bak-v", db_path.file_name()?.to_str()?))
}

/// Copy the open database to `<db>.bak-v<from>-<unix secs>` with the SQLite
/// backup API. The copy is encrypted with the same key and settings; no
/// plaintext ever touches disk. Returns the backup path.
pub(crate) fn backup_before_migration(
    src: &Connection,
    db_path: &Path,
    key: &DbKey,
    from_version: u32,
) -> Result<PathBuf, StorageError> {
    let prefix =
        backup_prefix(db_path).ok_or(StorageError::Io(std::io::ErrorKind::InvalidInput))?;
    let mut dest_path = db_path.with_file_name(format!("{prefix}{from_version}-{}", now_secs()));
    let mut n = 0u32;
    while dest_path.exists() {
        n += 1;
        dest_path = db_path.with_file_name(format!("{prefix}{from_version}-{}-{n}", now_secs()));
    }
    {
        let mut dest = Connection::open(&dest_path)?;
        pragmas::apply_cipher(&dest, key)?;
        let backup = Backup::new(src, &mut dest)?;
        backup.run_to_completion(256, Duration::from_millis(0), None)?;
    }
    Ok(dest_path)
}

/// Delete backups of `db_path` older than [`BACKUP_RETENTION`]. Best effort;
/// returns the number removed.
pub(crate) fn prune_backups(db_path: &Path) -> usize {
    let (Some(prefix), Some(dir)) = (backup_prefix(db_path), db_path.parent()) else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let cutoff = now_secs().saturating_sub(BACKUP_RETENTION.as_secs());
    let mut removed = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix(&prefix) else {
            continue;
        };
        // "<from>-<ts>[-n]"; sidecar files (-wal/-shm) are not matched on purpose.
        let mut parts = rest.split('-');
        let (Some(_), Some(ts)) = (parts.next(), parts.next()) else {
            continue;
        };
        let Ok(ts) = ts.parse::<u64>() else { continue };
        if parts.next().is_some_and(|p| p.parse::<u32>().is_err()) {
            continue;
        }
        if ts < cutoff && std::fs::remove_file(entry.path()).is_ok() {
            removed += 1;
        }
    }
    removed
}
