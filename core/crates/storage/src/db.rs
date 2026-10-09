use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
use zeroize::Zeroizing;

use crate::error::StorageError;
use crate::key::DbKey;
use crate::migrations::{self, MIGRATIONS, Migration};
use crate::pragmas;
use crate::store::{Id, Store, Tx};

/// Values recorded in `meta` when a database is created.
#[derive(Debug, Clone)]
pub struct CreateParams {
    /// Vault id (16 bytes).
    pub vault_id: Id,
    /// This device's id (16 bytes).
    pub device_id: Id,
    /// Current key epoch (docs/04 section 10).
    pub epoch: u64,
    /// Header format version.
    pub header_version: u32,
}

/// An open, unlocked, encrypted vault database.
///
/// A `Db` wraps one connection and is `Send` but not `Sync`: use one `Db` per
/// thread (open the same file several times for concurrent readers; WAL lets
/// readers proceed while one writer commits). Dropping it closes the
/// connection and zeroizes the key; prefer [`Db::close`] for an explicit,
/// checkpointed shutdown.
pub struct Db {
    pub(crate) conn: Connection,
    path: PathBuf,
    key: DbKey,
}

impl core::fmt::Debug for Db {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Db")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

fn sidecars(path: &Path) -> [PathBuf; 3] {
    let with = |suffix: &str| {
        let mut s = path.as_os_str().to_owned();
        s.push(suffix);
        PathBuf::from(s)
    };
    [path.to_path_buf(), with("-wal"), with("-shm")]
}

fn remove_db_files(path: &Path) {
    for p in sidecars(path) {
        let _ = std::fs::remove_file(p);
    }
}

fn create_exclusive(path: &Path) -> Result<(), StorageError> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path)?;
    Ok(())
}

impl Db {
    /// Create a new encrypted database at `path` (which must not exist).
    ///
    /// Built under `<path>.creating` and renamed into place, so a crash never
    /// leaves a half-initialised database at `path`.
    ///
    /// # Errors
    /// [`StorageError::AlreadyExists`], I/O errors, or SQLite errors.
    pub fn create(path: &Path, key: DbKey, params: &CreateParams) -> Result<Self, StorageError> {
        if path.exists() {
            return Err(StorageError::AlreadyExists);
        }
        let mut tmp_os = path.as_os_str().to_owned();
        tmp_os.push(".creating");
        let tmp = PathBuf::from(tmp_os);
        remove_db_files(&tmp); // stale leftovers of a crashed create
        create_exclusive(&tmp)?;
        let built = Self::build(&tmp, &key, params);
        if let Err(e) = built {
            remove_db_files(&tmp);
            return Err(e);
        }
        if path.exists() {
            remove_db_files(&tmp);
            return Err(StorageError::AlreadyExists);
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            remove_db_files(&tmp);
            return Err(e.into());
        }
        Self::open(path, key)
    }

    fn build(tmp: &Path, key: &DbKey, params: &CreateParams) -> Result<(), StorageError> {
        let mut conn = Connection::open_with_flags(
            tmp,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        pragmas::apply_cipher(&conn, key)?;
        pragmas::apply_connection(&conn)?;
        let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |r| r.get(0))?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(StorageError::SettingsMismatch {
                name: "journal_mode",
                expected: "wal".into(),
                found: mode,
            });
        }
        migrations::migrate(&mut conn, MIGRATIONS)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let t = Tx { conn: &tx };
            for (name, value) in pragmas::PINNED {
                t.meta_set(name, value.as_bytes())?;
            }
            t.meta_set("vault_id", &params.vault_id)?;
            t.meta_set("device_id", &params.device_id)?;
            t.meta_set("epoch", params.epoch.to_string().as_bytes())?;
            t.meta_set(
                "header_version",
                params.header_version.to_string().as_bytes(),
            )?;
        }
        tx.commit()?;
        pragmas::verify_live(&conn)?;
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
        conn.close().map_err(|(_, e)| StorageError::from(e))?;
        Ok(())
    }

    /// Open an existing database, verify the pinned settings and apply any
    /// pending migrations (after an encrypted backup).
    ///
    /// # Errors
    /// [`StorageError::WrongKeyOrCorrupt`] for a wrong key or an unreadable
    /// file, [`StorageError::SettingsMismatch`] if pinned settings differ,
    /// [`StorageError::SchemaTooNew`] for a newer database.
    pub fn open(path: &Path, key: DbKey) -> Result<Self, StorageError> {
        Self::open_with(path, key, MIGRATIONS)
    }

    pub(crate) fn open_with(
        path: &Path,
        key: DbKey,
        migrations_list: &[Migration],
    ) -> Result<Self, StorageError> {
        if !path.is_file() {
            return Err(StorageError::NotFound);
        }
        let mut conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|_| StorageError::WrongKeyOrCorrupt)?;
        pragmas::apply_cipher(&conn, &key)?;
        pragmas::probe(&conn)?;
        pragmas::apply_connection(&conn)?;
        let version = migrations::read_version(&conn)
            .map_err(|_| StorageError::WrongKeyOrCorrupt)?
            .ok_or(StorageError::WrongKeyOrCorrupt)?;
        pragmas::verify_live(&conn)?;
        pragmas::verify_recorded(&conn)?;
        let latest = migrations_list.last().map_or(0, |m| m.version);
        if version > latest {
            return Err(StorageError::SchemaTooNew {
                found: version,
                supported: latest,
            });
        }
        if version < latest {
            migrations::backup_before_migration(&conn, path, &key, version)?;
            migrations::migrate(&mut conn, migrations_list)?;
        }
        migrations::prune_backups(path);
        Ok(Self {
            conn,
            path: path.to_path_buf(),
            key,
        })
    }

    /// Path of the database file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current `schema_version`.
    ///
    /// # Errors
    /// SQLite errors.
    pub fn schema_version(&self) -> Result<u32, StorageError> {
        migrations::read_version(&self.conn)?.ok_or(StorageError::WrongKeyOrCorrupt)
    }

    /// Run `f` in an immediate (write) transaction: commit on `Ok`, roll back on `Err`.
    ///
    /// # Errors
    /// Whatever `f` returns, or a storage error converted via `From`.
    pub fn with_tx<T, E: From<StorageError>>(
        &mut self,
        f: impl FnOnce(&Tx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| E::from(e.into()))?;
        let out = f(&Tx { conn: &tx })?;
        tx.commit().map_err(|e| E::from(e.into()))?;
        Ok(out)
    }

    /// Run `f` in a deferred (read) transaction that does not take the write
    /// lock, so readers never block the writer under WAL. `f` must not write.
    ///
    /// # Errors
    /// Whatever `f` returns, or a storage error converted via `From`.
    pub fn with_read<T, E: From<StorageError>>(
        &mut self,
        f: impl FnOnce(&Tx<'_>) -> Result<T, E>,
    ) -> Result<T, E> {
        let tx = self
            .conn
            .transaction_with_behavior(TransactionBehavior::Deferred)
            .map_err(|e| E::from(e.into()))?;
        f(&Tx { conn: &tx })
    }

    /// Checkpoint the WAL into the main file and truncate it.
    ///
    /// # Errors
    /// SQLite errors (e.g. [`StorageError::Busy`]).
    pub fn checkpoint(&self) -> Result<(), StorageError> {
        self.conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                r.get::<_, i64>(0)
            })?;
        Ok(())
    }

    /// `PRAGMA integrity_check` plus `PRAGMA foreign_key_check`.
    ///
    /// # Errors
    /// [`StorageError::IntegrityFailed`] listing the problems.
    pub fn integrity_check(&self) -> Result<(), StorageError> {
        let mut problems: Vec<String> = Vec::new();
        let mut stmt = self.conn.prepare("PRAGMA integrity_check")?;
        for row in stmt.query_map([], |r| r.get::<_, String>(0))? {
            let line = row?;
            if line != "ok" {
                problems.push(line);
            }
        }
        drop(stmt);
        let mut stmt = self.conn.prepare("PRAGMA foreign_key_check")?;
        for row in stmt.query_map([], |r| {
            Ok(format!(
                "foreign key violation in {} (rowid {:?})",
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?
            ))
        })? {
            problems.push(row?);
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(StorageError::IntegrityFailed(problems))
        }
    }

    /// Re-key the database (key rotation, docs/04 section 10).
    ///
    /// `old` must equal the key this handle was opened with. The WAL is
    /// checkpointed first, and the new key is verified with a fresh
    /// connection afterwards.
    ///
    /// # Errors
    /// [`StorageError::WrongKeyOrCorrupt`] if `old` is wrong,
    /// [`StorageError::RekeyFailed`] if re-keying or its verification fails.
    pub fn rekey(&mut self, old: &DbKey, new: &DbKey) -> Result<(), StorageError> {
        if !self.key.ct_eq(old) {
            return Err(StorageError::WrongKeyOrCorrupt);
        }
        self.checkpoint()?;
        let lit = new.raw_literal();
        let stmt = Zeroizing::new(format!("PRAGMA rekey = \"{}\";", lit.as_str()));
        self.conn
            .execute_batch(&stmt)
            .map_err(|_| StorageError::RekeyFailed)?;
        self.key = new.duplicate();
        let verify = Connection::open(&self.path).map_err(|_| StorageError::RekeyFailed)?;
        pragmas::apply_cipher(&verify, new).map_err(|_| StorageError::RekeyFailed)?;
        pragmas::probe(&verify).map_err(|_| StorageError::RekeyFailed)?;
        migrations::read_version(&verify)
            .ok()
            .flatten()
            .ok_or(StorageError::RekeyFailed)?;
        Ok(())
    }

    /// Checkpoint the WAL, close the connection and
    /// zeroize the key.
    ///
    /// # Errors
    /// SQLite errors; the key is zeroized regardless.
    pub fn close(self) -> Result<(), StorageError> {
        let Self { conn, key, .. } = self;
        let checkpointed = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |r| {
                r.get::<_, i64>(0)
            })
            .map(|_| ())
            .map_err(StorageError::from);
        let closed = conn.close().map_err(|(_, e)| StorageError::from(e));
        drop(key);
        checkpointed.and(closed)
    }
}
