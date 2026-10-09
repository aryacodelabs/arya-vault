//! Vault-facing storage interface (`Store`) and its row types.
//!
//! Business rules (HLC comparison, history retention, merge) live in the
//! vault layer; these methods are deliberately mechanical. HLCs are the packed
//! 64-bit value (docs/05 section 6) carried as `i64`.

use rusqlite::{Connection, OptionalExtension, params};

use crate::error::StorageError;

/// 16-byte identifier (UUIDv7 item/folder ids, device ids).
pub type Id = [u8; 16];

/// Result alias for storage calls.
pub type Result<T> = core::result::Result<T, StorageError>;

/// An `item` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    /// Item id.
    pub id: Id,
    /// Item type (`login`, `note`, ...).
    pub item_type: String,
    /// Containing folder.
    pub folder_id: Option<Id>,
    /// Tombstone flag.
    pub deleted: bool,
    /// HLC of the deletion.
    pub deleted_hlc: Option<i64>,
    /// Max of the field HLCs, for sorting.
    pub updated_hlc: i64,
}

/// A field register, also used for `field_history` rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldRow {
    /// Owning item.
    pub item_id: Id,
    /// Field key, e.g. `password`, `custom.<id>.value`.
    pub key: String,
    /// CBOR-encoded value (`None` = cleared).
    pub value: Option<Vec<u8>>,
    /// Packed HLC.
    pub hlc: i64,
    /// Authoring device.
    pub device_id: Id,
    /// HLC of the version the author saw.
    pub base_hlc: Option<i64>,
}

/// A `field_history` row (same shape as a register).
pub type FieldHistoryRow = FieldRow;

/// A `folder` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRow {
    /// Folder id.
    pub id: Id,
    /// Display name.
    pub name: String,
    /// Parent folder.
    pub parent_id: Option<Id>,
    /// Packed HLC.
    pub hlc: i64,
    /// Authoring device.
    pub device_id: Id,
    /// Tombstone flag.
    pub deleted: bool,
}

/// An op awaiting upload (`seq` assigned by the database).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOp {
    /// Autoincrement sequence.
    pub seq: i64,
    /// Target item.
    pub item_id: Id,
    /// Field key.
    pub key: String,
    /// CBOR value.
    pub value: Option<Vec<u8>>,
    /// Packed HLC.
    pub hlc: i64,
    /// HLC the author saw.
    pub base_hlc: Option<i64>,
}

/// A frozen, encrypted segment awaiting confirmed upload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboxRow {
    /// This device's segment number.
    pub seq: i64,
    /// Exact encrypted envelope bytes.
    pub bytes: Vec<u8>,
    /// Upload confirmed.
    pub uploaded: bool,
}

/// A `segment_seen` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentSeen {
    /// Remote device.
    pub device_id: Id,
    /// Segment number.
    pub seq: i64,
    /// Segment hash.
    pub hash: Vec<u8>,
    /// When it was applied (unix seconds).
    pub applied_at: i64,
}

/// A `device` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceRow {
    /// Device id.
    pub device_id: Id,
    /// Display name.
    pub name: Option<String>,
    /// First seen (unix seconds).
    pub first_seen: Option<i64>,
    /// Last seen (unix seconds).
    pub last_seen: Option<i64>,
    /// Revoked flag.
    pub revoked: bool,
}

/// A `provider_state` row (cursor tokens, etags; no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderState {
    /// Provider name.
    pub provider: String,
    /// Opaque cursor.
    pub cursor: Option<String>,
    /// Updated at (unix seconds).
    pub updated_at: Option<i64>,
}

/// Document indexed in the FTS5 table (plaintext, but only inside the encrypted DB).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FtsDoc {
    /// Title.
    pub title: String,
    /// Username.
    pub username: String,
    /// URLs, space separated.
    pub urls: String,
    /// Notes.
    pub notes: String,
    /// Tags, space separated.
    pub tags: String,
}

/// Filter for [`Store::list_items`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemFilter<'a> {
    /// Include tombstoned items.
    pub include_deleted: bool,
    /// Restrict to a type.
    pub item_type: Option<&'a str>,
    /// Restrict to a folder.
    pub folder_id: Option<Id>,
}

/// Operations the vault layer needs. Implemented by [`Tx`]; obtain one with
/// [`Db::with_tx`](crate::Db::with_tx).
pub trait Store {
    /// Read a `meta` value.
    fn meta_get(&self, key: &str) -> Result<Option<Vec<u8>>>;
    /// Upsert a `meta` value.
    fn meta_set(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Insert or replace an item row.
    fn upsert_item(&self, item: &ItemRow) -> Result<()>;
    /// Fetch an item.
    fn get_item(&self, id: &Id) -> Result<Option<ItemRow>>;
    /// List items, newest first (`updated_hlc` descending).
    fn list_items(&self, filter: ItemFilter<'_>) -> Result<Vec<ItemRow>>;

    /// Insert or replace a field register.
    fn put_field(&self, field: &FieldRow) -> Result<()>;
    /// Fetch one register.
    fn get_field(&self, item_id: &Id, key: &str) -> Result<Option<FieldRow>>;
    /// All registers of an item, ordered by key.
    fn fields_for_item(&self, item_id: &Id) -> Result<Vec<FieldRow>>;

    /// Add a history version (idempotent on the primary key).
    fn add_history(&self, row: &FieldHistoryRow) -> Result<()>;
    /// History for a field, newest HLC first.
    fn history_for(&self, item_id: &Id, key: &str) -> Result<Vec<FieldHistoryRow>>;
    /// Keep only the newest `keep` versions; returns rows removed.
    fn prune_history(&self, item_id: &Id, key: &str, keep: usize) -> Result<usize>;

    /// Insert or replace a folder.
    fn upsert_folder(&self, folder: &FolderRow) -> Result<()>;
    /// Fetch a folder.
    fn get_folder(&self, id: &Id) -> Result<Option<FolderRow>>;
    /// List folders (including tombstones), ordered by name.
    fn list_folders(&self) -> Result<Vec<FolderRow>>;

    /// Queue a local op; returns its `seq`.
    fn append_local_op(
        &self,
        item_id: &Id,
        key: &str,
        value: Option<&[u8]>,
        hlc: i64,
        base_hlc: Option<i64>,
    ) -> Result<i64>;
    /// Oldest pending ops (at most `limit`).
    fn pending_local_ops(&self, limit: usize) -> Result<Vec<LocalOp>>;
    /// Delete ops with `seq <= up_to`; returns rows removed.
    fn delete_local_ops_through(&self, up_to: i64) -> Result<usize>;

    /// Store a frozen segment (idempotent: an existing `seq` is left untouched).
    fn outbox_put(&self, seq: i64, bytes: &[u8]) -> Result<()>;
    /// Segments not yet confirmed uploaded, oldest first.
    fn outbox_pending(&self) -> Result<Vec<OutboxRow>>;
    /// Mark a segment as uploaded.
    fn outbox_mark_uploaded(&self, seq: i64) -> Result<()>;
    /// Remove a segment.
    fn outbox_delete(&self, seq: i64) -> Result<()>;

    /// Highest manifest counter seen for a device.
    fn manifest_seen_get(&self, device_id: &Id) -> Result<Option<i64>>;
    /// Record a manifest counter; never lowers an existing value (rollback detection).
    fn manifest_seen_raise(&self, device_id: &Id, counter: i64) -> Result<()>;

    /// Record an applied remote segment.
    fn segment_seen_put(&self, row: &SegmentSeen) -> Result<()>;
    /// Look up an applied remote segment.
    fn segment_seen_get(&self, device_id: &Id, seq: i64) -> Result<Option<SegmentSeen>>;

    /// Insert or replace a device.
    fn upsert_device(&self, device: &DeviceRow) -> Result<()>;
    /// All known devices.
    fn list_devices(&self) -> Result<Vec<DeviceRow>>;

    /// Read provider sync state.
    fn provider_state_get(&self, provider: &str) -> Result<Option<ProviderState>>;
    /// Write provider sync state.
    fn provider_state_set(&self, state: &ProviderState) -> Result<()>;

    /// Index an item. The item row must exist. Remove the previous document
    /// first with [`Store::fts_remove`] when re-indexing.
    fn fts_index(&self, item_id: &Id, doc: &FtsDoc) -> Result<()>;
    /// Remove a previously indexed document (contentless FTS5 needs the old values).
    fn fts_remove(&self, item_id: &Id, old: &FtsDoc) -> Result<()>;
    /// Drop the whole index (rebuild by re-indexing every item).
    fn fts_clear(&self) -> Result<()>;
    /// Item ids matching an FTS5 query, best first.
    fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<Id>>;
}

/// A transaction handle passed to [`Db::with_tx`](crate::Db::with_tx).
pub struct Tx<'a> {
    pub(crate) conn: &'a Connection,
}

fn id_from(v: Vec<u8>) -> rusqlite::Result<Id> {
    v.try_into().map_err(|_| {
        rusqlite::Error::InvalidColumnType(0, "id".into(), rusqlite::types::Type::Blob)
    })
}

fn item_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ItemRow> {
    Ok(ItemRow {
        id: id_from(r.get(0)?)?,
        item_type: r.get(1)?,
        folder_id: r.get::<_, Option<Vec<u8>>>(2)?.map(id_from).transpose()?,
        deleted: r.get::<_, i64>(3)? != 0,
        deleted_hlc: r.get(4)?,
        updated_hlc: r.get(5)?,
    })
}

fn field_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<FieldRow> {
    Ok(FieldRow {
        item_id: id_from(r.get(0)?)?,
        key: r.get(1)?,
        value: r.get(2)?,
        hlc: r.get(3)?,
        device_id: id_from(r.get(4)?)?,
        base_hlc: r.get(5)?,
    })
}

fn folder_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<FolderRow> {
    Ok(FolderRow {
        id: id_from(r.get(0)?)?,
        name: r.get(1)?,
        parent_id: r.get::<_, Option<Vec<u8>>>(2)?.map(id_from).transpose()?,
        hlc: r.get(3)?,
        device_id: id_from(r.get(4)?)?,
        deleted: r.get::<_, i64>(5)? != 0,
    })
}

fn limit_i64(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

impl Store for Tx<'_> {
    fn meta_get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |r| r.get(0))
            .optional()?)
    }
    fn meta_set(&self, key: &str, value: &[u8]) -> Result<()> {
        self.conn.prepare("INSERT INTO meta(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value")?
            .execute(params![key, value])?;
        Ok(())
    }

    fn upsert_item(&self, i: &ItemRow) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO item(id, type, folder_id, deleted, deleted_hlc, updated_hlc) VALUES (?1,?2,?3,?4,?5,?6)")?
            .execute(params![&i.id[..], i.item_type, i.folder_id.as_ref().map(|f| &f[..]), i64::from(i.deleted), i.deleted_hlc, i.updated_hlc])?;
        Ok(())
    }
    fn get_item(&self, id: &Id) -> Result<Option<ItemRow>> {
        Ok(self.conn.prepare("SELECT id, type, folder_id, deleted, deleted_hlc, updated_hlc FROM item WHERE id = ?1")?
            .query_row([&id[..]], item_row).optional()?)
    }
    fn list_items(&self, f: ItemFilter<'_>) -> Result<Vec<ItemRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, type, folder_id, deleted, deleted_hlc, updated_hlc FROM item
             WHERE (?1 = 1 OR deleted = 0) AND (?2 IS NULL OR type = ?2) AND (?3 IS NULL OR folder_id = ?3)
             ORDER BY updated_hlc DESC, id")?;
        let rows = stmt.query_map(
            params![
                i64::from(f.include_deleted),
                f.item_type,
                f.folder_id.as_ref().map(|x| &x[..])
            ],
            item_row,
        )?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn put_field(&self, f: &FieldRow) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO field(item_id, key, value, hlc, device_id, base_hlc) VALUES (?1,?2,?3,?4,?5,?6)")?
            .execute(params![&f.item_id[..], f.key, f.value, f.hlc, &f.device_id[..], f.base_hlc])?;
        Ok(())
    }
    fn get_field(&self, item_id: &Id, key: &str) -> Result<Option<FieldRow>> {
        Ok(self.conn.prepare("SELECT item_id, key, value, hlc, device_id, base_hlc FROM field WHERE item_id = ?1 AND key = ?2")?
            .query_row(params![&item_id[..], key], field_row).optional()?)
    }
    fn fields_for_item(&self, item_id: &Id) -> Result<Vec<FieldRow>> {
        let mut stmt = self.conn.prepare("SELECT item_id, key, value, hlc, device_id, base_hlc FROM field WHERE item_id = ?1 ORDER BY key")?;
        let rows = stmt.query_map([&item_id[..]], field_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn add_history(&self, h: &FieldHistoryRow) -> Result<()> {
        self.conn.prepare("INSERT OR IGNORE INTO field_history(item_id, key, value, hlc, device_id, base_hlc) VALUES (?1,?2,?3,?4,?5,?6)")?
            .execute(params![&h.item_id[..], h.key, h.value, h.hlc, &h.device_id[..], h.base_hlc])?;
        Ok(())
    }
    fn history_for(&self, item_id: &Id, key: &str) -> Result<Vec<FieldHistoryRow>> {
        let mut stmt = self.conn.prepare("SELECT item_id, key, value, hlc, device_id, base_hlc FROM field_history WHERE item_id = ?1 AND key = ?2 ORDER BY hlc DESC, device_id")?;
        let rows = stmt.query_map(params![&item_id[..], key], field_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    fn prune_history(&self, item_id: &Id, key: &str, keep: usize) -> Result<usize> {
        Ok(self.conn.prepare(
            "DELETE FROM field_history WHERE item_id = ?1 AND key = ?2 AND (hlc, device_id) NOT IN
             (SELECT hlc, device_id FROM field_history WHERE item_id = ?1 AND key = ?2 ORDER BY hlc DESC, device_id LIMIT ?3)")?
            .execute(params![&item_id[..], key, limit_i64(keep)])?)
    }

    fn upsert_folder(&self, f: &FolderRow) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO folder(id, name, parent_id, hlc, device_id, deleted) VALUES (?1,?2,?3,?4,?5,?6)")?
            .execute(params![&f.id[..], f.name, f.parent_id.as_ref().map(|p| &p[..]), f.hlc, &f.device_id[..], i64::from(f.deleted)])?;
        Ok(())
    }
    fn get_folder(&self, id: &Id) -> Result<Option<FolderRow>> {
        Ok(self
            .conn
            .prepare(
                "SELECT id, name, parent_id, hlc, device_id, deleted FROM folder WHERE id = ?1",
            )?
            .query_row([&id[..]], folder_row)
            .optional()?)
    }
    fn list_folders(&self) -> Result<Vec<FolderRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, parent_id, hlc, device_id, deleted FROM folder ORDER BY name, id",
        )?;
        let rows = stmt.query_map([], folder_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn append_local_op(
        &self,
        item_id: &Id,
        key: &str,
        value: Option<&[u8]>,
        hlc: i64,
        base_hlc: Option<i64>,
    ) -> Result<i64> {
        self.conn
            .prepare(
                "INSERT INTO local_op(item_id, key, value, hlc, base_hlc) VALUES (?1,?2,?3,?4,?5)",
            )?
            .execute(params![&item_id[..], key, value, hlc, base_hlc])?;
        Ok(self.conn.last_insert_rowid())
    }
    fn pending_local_ops(&self, limit: usize) -> Result<Vec<LocalOp>> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, item_id, key, value, hlc, base_hlc FROM local_op ORDER BY seq LIMIT ?1",
        )?;
        let rows = stmt.query_map([limit_i64(limit)], |r| {
            Ok(LocalOp {
                seq: r.get(0)?,
                item_id: id_from(r.get(1)?)?,
                key: r.get(2)?,
                value: r.get(3)?,
                hlc: r.get(4)?,
                base_hlc: r.get(5)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    fn delete_local_ops_through(&self, up_to: i64) -> Result<usize> {
        Ok(self
            .conn
            .prepare("DELETE FROM local_op WHERE seq <= ?1")?
            .execute([up_to])?)
    }

    fn outbox_put(&self, seq: i64, bytes: &[u8]) -> Result<()> {
        self.conn
            .prepare("INSERT OR IGNORE INTO outbox(seq, bytes, uploaded) VALUES (?1, ?2, 0)")?
            .execute(params![seq, bytes])?;
        Ok(())
    }
    fn outbox_pending(&self) -> Result<Vec<OutboxRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT seq, bytes, uploaded FROM outbox WHERE uploaded = 0 ORDER BY seq")?;
        let rows = stmt.query_map([], |r| {
            Ok(OutboxRow {
                seq: r.get(0)?,
                bytes: r.get(1)?,
                uploaded: r.get::<_, i64>(2)? != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    fn outbox_mark_uploaded(&self, seq: i64) -> Result<()> {
        self.conn
            .prepare("UPDATE outbox SET uploaded = 1 WHERE seq = ?1")?
            .execute([seq])?;
        Ok(())
    }
    fn outbox_delete(&self, seq: i64) -> Result<()> {
        self.conn
            .prepare("DELETE FROM outbox WHERE seq = ?1")?
            .execute([seq])?;
        Ok(())
    }

    fn manifest_seen_get(&self, device_id: &Id) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT counter FROM manifest_seen WHERE device_id = ?1",
                [&device_id[..]],
                |r| r.get(0),
            )
            .optional()?)
    }
    fn manifest_seen_raise(&self, device_id: &Id, counter: i64) -> Result<()> {
        self.conn.prepare("INSERT INTO manifest_seen(device_id, counter) VALUES (?1, ?2) ON CONFLICT(device_id) DO UPDATE SET counter = max(counter, excluded.counter)")?
            .execute(params![&device_id[..], counter])?;
        Ok(())
    }

    fn segment_seen_put(&self, s: &SegmentSeen) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO segment_seen(device_id, seq, hash, applied_at) VALUES (?1,?2,?3,?4)")?
            .execute(params![&s.device_id[..], s.seq, s.hash, s.applied_at])?;
        Ok(())
    }
    fn segment_seen_get(&self, device_id: &Id, seq: i64) -> Result<Option<SegmentSeen>> {
        Ok(self.conn.query_row("SELECT device_id, seq, hash, applied_at FROM segment_seen WHERE device_id = ?1 AND seq = ?2", params![&device_id[..], seq], |r| {
            Ok(SegmentSeen { device_id: id_from(r.get(0)?)?, seq: r.get(1)?, hash: r.get(2)?, applied_at: r.get(3)? })
        }).optional()?)
    }

    fn upsert_device(&self, d: &DeviceRow) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO device(device_id, name, first_seen, last_seen, revoked) VALUES (?1,?2,?3,?4,?5)")?
            .execute(params![&d.device_id[..], d.name, d.first_seen, d.last_seen, i64::from(d.revoked)])?;
        Ok(())
    }
    fn list_devices(&self) -> Result<Vec<DeviceRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, name, first_seen, last_seen, revoked FROM device ORDER BY device_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(DeviceRow {
                device_id: id_from(r.get(0)?)?,
                name: r.get(1)?,
                first_seen: r.get(2)?,
                last_seen: r.get(3)?,
                revoked: r.get::<_, Option<i64>>(4)?.unwrap_or(0) != 0,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    fn provider_state_get(&self, provider: &str) -> Result<Option<ProviderState>> {
        Ok(self
            .conn
            .query_row(
                "SELECT provider, cursor, updated_at FROM provider_state WHERE provider = ?1",
                [provider],
                |r| {
                    Ok(ProviderState {
                        provider: r.get(0)?,
                        cursor: r.get(1)?,
                        updated_at: r.get(2)?,
                    })
                },
            )
            .optional()?)
    }
    fn provider_state_set(&self, s: &ProviderState) -> Result<()> {
        self.conn.prepare("INSERT OR REPLACE INTO provider_state(provider, cursor, updated_at) VALUES (?1,?2,?3)")?
            .execute(params![s.provider, s.cursor, s.updated_at])?;
        Ok(())
    }

    fn fts_index(&self, item_id: &Id, d: &FtsDoc) -> Result<()> {
        let n = self.conn.prepare("INSERT INTO item_fts(rowid, title, username, urls, notes, tags) SELECT rowid, ?2, ?3, ?4, ?5, ?6 FROM item WHERE id = ?1")?
            .execute(params![&item_id[..], d.title, d.username, d.urls, d.notes, d.tags])?;
        if n == 0 {
            return Err(StorageError::Constraint(
                "fts_index: item does not exist".into(),
            ));
        }
        Ok(())
    }
    fn fts_remove(&self, item_id: &Id, o: &FtsDoc) -> Result<()> {
        self.conn.prepare("INSERT INTO item_fts(item_fts, rowid, title, username, urls, notes, tags) SELECT 'delete', rowid, ?2, ?3, ?4, ?5, ?6 FROM item WHERE id = ?1")?
            .execute(params![&item_id[..], o.title, o.username, o.urls, o.notes, o.tags])?;
        Ok(())
    }
    fn fts_clear(&self) -> Result<()> {
        self.conn
            .execute("INSERT INTO item_fts(item_fts) VALUES ('delete-all')", [])?;
        Ok(())
    }
    fn fts_search(&self, query: &str, limit: usize) -> Result<Vec<Id>> {
        let mut stmt = self.conn.prepare("SELECT item.id FROM item_fts JOIN item ON item.rowid = item_fts.rowid WHERE item_fts MATCH ?1 ORDER BY rank LIMIT ?2")?;
        let rows = stmt.query_map(params![query, limit_i64(limit)], |r| id_from(r.get(0)?))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}
