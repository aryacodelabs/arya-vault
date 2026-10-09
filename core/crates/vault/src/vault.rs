//! [`Vault`]: the local vault over a [`Db`]. Every mutation runs in **one**
//! transaction that writes the register, its history, the `local_op` record,
//! the item cache, the search index and the persisted clock together.

use arya_vault_storage::{Db, FolderRow, Id, Store, Tx};
use zeroize::Zeroizing;

use crate::engine::{self, IdGen, Op, Regs};
use crate::error::{Result, VaultError};
use crate::fault;
use crate::hlc::{Clock, Hlc, HlcClock};
use crate::model::{
    CustomKind, DAY_MS, ElementId, HARD_ITEM_LIMIT, ItemType, MAX_CUSTOM_FIELDS, MAX_FIELD_BYTES,
    MAX_FOLDER_NAME_BYTES, MAX_TAGS, SOFT_ITEM_LIMIT, StdField, VaultConfig, check_tag, check_text,
    keys,
};
use crate::value::{self, Value};
use crate::views::{CustomView, Folder, ItemView, NewItem, UrlView};

/// `(device_id, hlc_state)` as stored in `meta`.
type MetaPair = (Option<Vec<u8>>, Option<Vec<u8>>);

const HLC_STATE: &str = "hlc_state";
const FOLDER_OP_KEY: &str = "_folder";

/// A set of register writes made by one user action (all share one HLC).
#[derive(Default)]
pub(crate) struct Edits(pub(crate) Vec<(String, Option<Zeroizing<Vec<u8>>>)>);

impl Edits {
    pub fn set(&mut self, key: &str, v: Option<Zeroizing<Vec<u8>>>) {
        self.0.push((key.to_owned(), v));
    }
    pub fn text(&mut self, key: &str, s: &str) {
        self.set(key, Some(value::encode_text(s)));
    }
    pub fn flag(&mut self, key: &str, b: bool) {
        self.set(key, Some(value::encode_bool(b)));
    }
}

/// The local vault. Not `Sync`; one per unlocked session.
pub struct Vault {
    pub(crate) db: Db,
    pub(crate) clock: HlcClock,
    pub(crate) device_id: Id,
    pub(crate) cfg: VaultConfig,
    pub(crate) ids: IdGen,
}

impl core::fmt::Debug for Vault {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Vault").finish_non_exhaustive()
    }
}

fn utf8_len(s: &str) -> usize {
    s.len()
}

impl Vault {
    /// Wrap an open database. Reads `device_id` and the persisted clock from `meta`.
    ///
    /// # Errors
    /// [`VaultError::Corrupt`] if `device_id` is missing or malformed.
    pub fn open(mut db: Db, clock: Box<dyn Clock>) -> Result<Self> {
        let (device, last) = db.with_read(|tx| -> Result<MetaPair> {
            Ok((tx.meta_get("device_id")?, tx.meta_get(HLC_STATE)?))
        })?;
        let device_id =
            Id::try_from(device.ok_or(VaultError::Corrupt)?).map_err(|_| VaultError::Corrupt)?;
        let last = match last {
            None => Hlc::ZERO,
            Some(b) => Hlc::from_i64(i64::from_le_bytes(
                b.try_into().map_err(|_| VaultError::Corrupt)?,
            ))?,
        };
        Ok(Self {
            db,
            clock: HlcClock::new(clock, last),
            device_id,
            cfg: VaultConfig::default(),
            ids: IdGen::new(),
        })
    }

    /// This device's id.
    #[must_use]
    pub fn device_id(&self) -> Id {
        self.device_id
    }

    /// Current retention settings.
    #[must_use]
    pub fn config(&self) -> VaultConfig {
        self.cfg
    }

    /// Change retention settings.
    pub fn set_config(&mut self, cfg: VaultConfig) {
        self.cfg = cfg;
    }

    /// Reads an app-level setting kept in the encrypted `meta` table under `setting.<name>`.
    ///
    /// Settings are opaque bytes to this crate; the namespace keeps callers away from the keys
    /// the vault itself owns (`device_id`, the clock, the key epoch...).
    ///
    /// # Errors
    /// [`VaultError::InvalidValue`] for a malformed name; storage errors.
    pub fn get_setting(&mut self, name: &str) -> Result<Option<Vec<u8>>> {
        let key = setting_key(name)?;
        self.db
            .with_read(|tx| Ok::<_, VaultError>(tx.meta_get(&key)?))
    }

    /// Stores an app-level setting (see [`Vault::get_setting`]) in one transaction.
    ///
    /// # Errors
    /// [`VaultError::InvalidValue`] for a malformed name; [`VaultError::LimitExceeded`] above
    /// [`MAX_SETTING_BYTES`]; storage errors.
    pub fn set_setting(&mut self, name: &str, value: &[u8]) -> Result<()> {
        let key = setting_key(name)?;
        if value.len() > MAX_SETTING_BYTES {
            return Err(VaultError::LimitExceeded("setting value too large"));
        }
        self.db
            .with_tx(|tx| Ok::<_, VaultError>(tx.meta_set(&key, value)?))
    }

    /// Close the vault (checkpoints the WAL and zeroizes the database key).
    ///
    /// # Errors
    /// Storage errors.
    pub fn close(self) -> Result<()> {
        Ok(self.db.close()?)
    }

    /// Number of item rows, including trashed ones and tombstones.
    ///
    /// # Errors
    /// Storage errors.
    pub fn item_count(&mut self) -> Result<u64> {
        self.db
            .with_read(|tx| Ok::<_, VaultError>(tx.count_items()?))
    }

    /// Whether the vault reached the soft target of 20,000 items (docs/05 section 10).
    ///
    /// # Errors
    /// Storage errors.
    pub fn at_soft_item_limit(&mut self) -> Result<bool> {
        Ok(self.item_count()? >= SOFT_ITEM_LIMIT)
    }

    // ------------------------------------------------------------------ create

    /// Create an item. All of its registers are written in one transaction.
    ///
    /// # Errors
    /// Limits ([`VaultError::LimitExceeded`]), unknown folder, invalid fields for the type.
    pub fn create_item(&mut self, new: NewItem) -> Result<Id> {
        validate_new(&new)?;
        let Vault {
            db,
            clock,
            device_id,
            cfg,
            ids,
            ..
        } = self;
        db.with_tx(|tx| {
            if tx.count_items()? >= HARD_ITEM_LIMIT {
                return Err(VaultError::LimitExceeded("item count"));
            }
            if let Some(f) = &new.folder_id {
                require_live_folder(tx, f)?;
            }
            let hlc = clock.now()?;
            let wall = clock.wall_ms();
            let id = ids.next(wall, engine::os_random()?);
            let mut e = Edits::default();
            e.text(keys::TYPE, new.item_type.as_str());
            e.set(
                keys::SCHEMA_VERSION,
                Some(value::encode_int(keys::ITEM_SCHEMA)),
            );
            e.set(
                keys::CREATED_AT,
                Some(value::encode_int(i64::try_from(wall).unwrap_or(i64::MAX))),
            );
            e.text(StdField::Title.key(), &new.title);
            for (f, v) in &new.fields {
                e.text(f.key(), v);
            }
            if let Some(f) = &new.folder_id {
                e.set(keys::FOLDER, Some(value::encode_bytes(f)));
            }
            if new.favorite {
                e.flag(keys::FAVORITE, true);
            }
            for t in &new.tags {
                e.flag(&format!("{}{t}", keys::TAG_PREFIX), true);
            }
            for u in &new.urls {
                let eid = ElementId::from_id(&ids.next(wall, engine::os_random()?));
                e.text(&format!("{}{}", keys::URL_PREFIX, eid.as_str()), u);
            }
            commit_edits(tx, clock, device_id, cfg, &id, &Vec::new(), hlc, e)?;
            Ok(id)
        })
    }

    // -------------------------------------------------------------- mutations

    /// Run a mutation of one item: `build` inspects the current registers and
    /// records edits; they are applied with a fresh HLC that outranks everything
    /// stored for the item. `require_visible` rejects trashed items.
    pub(crate) fn mutate<T>(
        &mut self,
        id: &Id,
        require_visible: bool,
        build: impl FnOnce(&Regs, &mut Edits, &mut IdGen, u64) -> Result<T>,
    ) -> Result<T> {
        let Vault {
            db,
            clock,
            device_id,
            cfg,
            ids,
            ..
        } = self;
        db.with_tx(|tx| {
            let row = engine::require_item(tx, id)?;
            let regs = engine::load_regs(tx, id)?;
            if require_visible && !engine::is_visible(&regs) {
                return Err(VaultError::InTrash);
            }
            let mut e = Edits::default();
            let out = build(&regs, &mut e, ids, clock.wall_ms())?;
            if e.0.is_empty() {
                return Ok(out);
            }
            let hlc = clock.now_after(Hlc::from_i64(row.updated_hlc)?)?;
            commit_edits(tx, clock, device_id, cfg, id, &regs, hlc, e)?;
            Ok(out)
        })
    }

    /// Set a standard field. Secret fields are accepted here (write-only path).
    ///
    /// # Errors
    /// [`VaultError::InvalidField`] if the type has no such field, size limits, [`VaultError::InTrash`].
    pub fn set_field(&mut self, id: &Id, field: StdField, value: &str) -> Result<()> {
        check_text(value, field.max_bytes())?;
        self.mutate(id, true, |regs, e, _, _| {
            check_field_for(regs, field)?;
            e.text(field.key(), value);
            Ok(())
        })
    }

    /// Clear a standard field (the previous value stays in history).
    ///
    /// # Errors
    /// As [`Vault::set_field`].
    pub fn clear_field(&mut self, id: &Id, field: StdField) -> Result<()> {
        self.mutate(id, true, |regs, e, _, _| {
            check_field_for(regs, field)?;
            e.set(field.key(), None);
            Ok(())
        })
    }

    /// Set or unset the favorite flag by toggling; returns the new state.
    ///
    /// # Errors
    /// [`VaultError::InTrash`], storage errors.
    pub fn toggle_favorite(&mut self, id: &Id) -> Result<bool> {
        self.mutate(id, true, |regs, e, _, _| {
            let now = !engine::bool_of(regs, keys::FAVORITE);
            e.flag(keys::FAVORITE, now);
            Ok(now)
        })
    }

    /// Move an item into a folder (`None` = no folder).
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the folder does not exist or is deleted.
    pub fn move_to_folder(&mut self, id: &Id, folder: Option<Id>) -> Result<()> {
        let Vault {
            db,
            clock,
            device_id,
            cfg,
            ..
        } = self;
        db.with_tx(|tx| {
            let row = engine::require_item(tx, id)?;
            let regs = engine::load_regs(tx, id)?;
            if !engine::is_visible(&regs) {
                return Err(VaultError::InTrash);
            }
            if let Some(f) = &folder {
                require_live_folder(tx, f)?;
            }
            let mut e = Edits::default();
            e.set(
                keys::FOLDER,
                folder.as_ref().map(|f| value::encode_bytes(f)),
            );
            let hlc = clock.now_after(Hlc::from_i64(row.updated_hlc)?)?;
            commit_edits(tx, clock, device_id, cfg, id, &regs, hlc, e)
        })
    }

    /// Add a tag (idempotent).
    ///
    /// # Errors
    /// [`VaultError::LimitExceeded`] beyond 50 tags; [`VaultError::InvalidValue`] for a bad tag.
    pub fn add_tag(&mut self, id: &Id, tag: &str) -> Result<()> {
        check_tag(tag)?;
        self.mutate(id, true, |regs, e, _, _| {
            let key = format!("{}{tag}", keys::TAG_PREFIX);
            if engine::bool_of(regs, &key) {
                return Ok(());
            }
            if live_tags(regs).len() >= MAX_TAGS {
                return Err(VaultError::LimitExceeded("tags per item"));
            }
            e.flag(&key, true);
            Ok(())
        })
    }

    /// Remove a tag (a per-element tombstone; idempotent).
    ///
    /// # Errors
    /// [`VaultError::InTrash`], storage errors.
    pub fn remove_tag(&mut self, id: &Id, tag: &str) -> Result<()> {
        check_tag(tag)?;
        self.mutate(id, true, |regs, e, _, _| {
            let key = format!("{}{tag}", keys::TAG_PREFIX);
            if engine::bool_of(regs, &key) {
                e.flag(&key, false);
            }
            Ok(())
        })
    }

    /// Add a URL element; returns its id.
    ///
    /// # Errors
    /// Size limit, [`VaultError::InvalidField`] unless the item is a login.
    pub fn add_url(&mut self, id: &Id, url: &str) -> Result<ElementId> {
        check_text(url, MAX_FIELD_BYTES)?;
        self.mutate(id, true, |regs, e, ids, wall| {
            require_type(regs, ItemType::Login, "urls")?;
            let eid = ElementId::from_id(&ids.next(wall, engine::os_random()?));
            e.text(&format!("{}{}", keys::URL_PREFIX, eid.as_str()), url);
            Ok(eid)
        })
    }

    /// Replace a URL element's value.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the element does not exist.
    pub fn set_url(&mut self, id: &Id, url_id: &ElementId, url: &str) -> Result<()> {
        check_text(url, MAX_FIELD_BYTES)?;
        self.mutate(id, true, |regs, e, _, _| {
            let key = format!("{}{}", keys::URL_PREFIX, url_id.as_str());
            engine::live_value(regs, &key).ok_or(VaultError::NotFound)?;
            e.text(&key, url);
            Ok(())
        })
    }

    /// Remove a URL element.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the element does not exist.
    pub fn remove_url(&mut self, id: &Id, url_id: &ElementId) -> Result<()> {
        self.mutate(id, true, |regs, e, _, _| {
            let key = format!("{}{}", keys::URL_PREFIX, url_id.as_str());
            engine::live_value(regs, &key).ok_or(VaultError::NotFound)?;
            e.set(&key, None);
            Ok(())
        })
    }

    /// Add a custom field (individually addressable registers `custom.<id>.{kind,label,value}`).
    ///
    /// # Errors
    /// [`VaultError::LimitExceeded`] beyond 100 custom fields.
    pub fn add_custom_field(
        &mut self,
        id: &Id,
        kind: CustomKind,
        label: &str,
        value: &str,
    ) -> Result<ElementId> {
        check_text(label, MAX_FIELD_BYTES)?;
        check_text(value, MAX_FIELD_BYTES)?;
        self.mutate(id, true, |regs, e, ids, wall| {
            if custom_ids(regs).len() >= MAX_CUSTOM_FIELDS {
                return Err(VaultError::LimitExceeded("custom fields per item"));
            }
            let eid = ElementId::from_id(&ids.next(wall, engine::os_random()?));
            let p = format!("{}{}", keys::CUSTOM_PREFIX, eid.as_str());
            e.text(&format!("{p}.kind"), kind.as_str());
            e.text(&format!("{p}.label"), label);
            e.text(&format!("{p}.value"), value);
            Ok(eid)
        })
    }

    /// Set a custom field's value.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if it does not exist.
    pub fn set_custom_value(&mut self, id: &Id, cid: &ElementId, value: &str) -> Result<()> {
        check_text(value, MAX_FIELD_BYTES)?;
        self.mutate(id, true, |regs, e, _, _| {
            require_custom(regs, cid)?;
            e.text(
                &format!("{}{}.value", keys::CUSTOM_PREFIX, cid.as_str()),
                value,
            );
            Ok(())
        })
    }

    /// Rename a custom field.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if it does not exist.
    pub fn set_custom_label(&mut self, id: &Id, cid: &ElementId, label: &str) -> Result<()> {
        check_text(label, MAX_FIELD_BYTES)?;
        self.mutate(id, true, |regs, e, _, _| {
            require_custom(regs, cid)?;
            e.text(
                &format!("{}{}.label", keys::CUSTOM_PREFIX, cid.as_str()),
                label,
            );
            Ok(())
        })
    }

    /// Remove a custom field (tombstones all three registers).
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if it does not exist.
    pub fn remove_custom_field(&mut self, id: &Id, cid: &ElementId) -> Result<()> {
        self.mutate(id, true, |regs, e, _, _| {
            require_custom(regs, cid)?;
            let p = format!("{}{}", keys::CUSTOM_PREFIX, cid.as_str());
            for part in ["kind", "label", "value"] {
                e.set(&format!("{p}.{part}"), None);
            }
            Ok(())
        })
    }

    // ------------------------------------------------------------ trash/purge

    /// Move an item to the trash (tombstone). Idempotent for items already trashed.
    ///
    /// # Errors
    /// [`VaultError::NotFound`].
    pub fn delete_item(&mut self, id: &Id) -> Result<()> {
        self.mutate(id, false, |regs, e, _, wall| {
            if !engine::is_visible(regs) {
                return Ok(());
            }
            e.flag(keys::DELETED, true);
            e.set(
                keys::DELETED_AT,
                Some(value::encode_int(i64::try_from(wall).unwrap_or(i64::MAX))),
            );
            Ok(())
        })
    }

    /// Restore an item from the trash.
    ///
    /// # Errors
    /// [`VaultError::NotInTrash`] if it is not trashed.
    pub fn restore_item(&mut self, id: &Id) -> Result<()> {
        self.mutate(id, false, |regs, e, _, _| {
            if engine::is_visible(regs) {
                return Err(VaultError::NotInTrash);
            }
            e.flag(keys::DELETED, false);
            e.set(keys::DELETED_AT, None);
            Ok(())
        })
    }

    /// Permanently remove a trashed item's content now ("delete forever"): every
    /// register except the minimal tombstone is blanked and its history dropped.
    /// Other devices purge on their own after the retention period.
    ///
    /// # Errors
    /// [`VaultError::NotInTrash`] if the item is not trashed.
    pub fn purge_item(&mut self, id: &Id) -> Result<()> {
        self.db.with_tx(|tx| {
            engine::require_item(tx, id)?;
            if engine::is_visible(&engine::load_regs(tx, id)?) {
                return Err(VaultError::NotInTrash);
            }
            engine::purge(tx, id)
        })
    }

    /// Purge every item that has been in the trash for at least `trash_days`.
    /// Deterministic from the converged state, so devices agree without an op
    /// (see PR "Spec questions"). Returns the number of items purged.
    ///
    /// # Errors
    /// Storage errors.
    pub fn purge_expired(&mut self) -> Result<usize> {
        let (now, days) = (self.clock.wall_ms(), self.cfg.trash_days);
        self.db.with_tx(|tx| {
            let mut purged = 0;
            let rows = tx.list_items(arya_vault_storage::ItemFilter {
                include_deleted: true,
                ..Default::default()
            })?;
            for row in rows.iter().filter(|r| r.deleted) {
                let Some(dh) = row.deleted_hlc else { continue };
                if now.saturating_sub(Hlc::from_i64(dh)?.pt()) < days * DAY_MS {
                    continue;
                }
                let regs = engine::load_regs(tx, &row.id)?;
                let has_content = regs
                    .iter()
                    .any(|(k, r)| !engine::survives_purge(k) && r.value.is_some());
                if engine::is_visible(&regs) || !has_content {
                    continue;
                }
                engine::purge(tx, &row.id)?;
                purged += 1;
            }
            Ok(purged)
        })
    }

    /// Items whose tombstone is older than `tombstone_days` (180). Dropping them
    /// also needs every device to have acknowledged a snapshot, so that is
    /// compaction's job (M4); this only reports candidates.
    ///
    /// # Errors
    /// Storage errors.
    pub fn expired_tombstones(&mut self) -> Result<Vec<Id>> {
        let (now, days) = (self.clock.wall_ms(), self.cfg.tombstone_days);
        self.db.with_read(|tx| {
            let rows = tx.list_items(arya_vault_storage::ItemFilter {
                include_deleted: true,
                ..Default::default()
            })?;
            let mut out = Vec::new();
            for row in rows.iter().filter(|r| r.deleted) {
                let Some(dh) = row.deleted_hlc else { continue };
                if now.saturating_sub(Hlc::from_i64(dh)?.pt()) >= days * DAY_MS
                    && !engine::is_visible(&engine::load_regs(tx, &row.id)?)
                {
                    out.push(row.id);
                }
            }
            Ok(out)
        })
    }

    // ----------------------------------------------------------------- reading

    /// Everything about one item except its secrets.
    ///
    /// # Errors
    /// [`VaultError::NotFound`]; [`VaultError::InTrash`] is *not* an error (use `list_trash`).
    pub fn get_item(&mut self, id: &Id) -> Result<ItemView> {
        self.db.with_read(|tx| {
            engine::require_item(tx, id)?;
            let regs = engine::load_regs(tx, id)?;
            view_of(id, &regs)
        })
    }

    /// A non-secret standard field.
    ///
    /// # Errors
    /// [`VaultError::SecretField`] for secret fields (use [`Vault::reveal`]).
    pub fn get_text(&mut self, id: &Id, field: StdField) -> Result<Option<String>> {
        if field.is_secret() {
            return Err(VaultError::SecretField(field.key()));
        }
        self.db.with_read(|tx| {
            engine::require_item(tx, id)?;
            Ok(engine::text_of(&engine::load_regs(tx, id)?, field.key()))
        })
    }

    /// A secret standard field, in a zeroizing buffer.
    ///
    /// # Errors
    /// [`VaultError::NotSecret`] for non-secret fields.
    pub fn reveal(&mut self, id: &Id, field: StdField) -> Result<Option<Zeroizing<String>>> {
        if !field.is_secret() {
            return Err(VaultError::NotSecret(field.key()));
        }
        self.reveal_key(id, field.key())
    }

    /// The value of a hidden custom field.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the custom field does not exist.
    pub fn reveal_custom(&mut self, id: &Id, cid: &ElementId) -> Result<Option<Zeroizing<String>>> {
        self.reveal_key(
            id,
            &format!("{}{}.value", keys::CUSTOM_PREFIX, cid.as_str()),
        )
    }

    pub(crate) fn reveal_key(&mut self, id: &Id, key: &str) -> Result<Option<Zeroizing<String>>> {
        self.db.with_read(|tx| {
            engine::require_item(tx, id)?;
            let regs = engine::load_regs(tx, id)?;
            engine::live_value(&regs, key)
                .map(value::decode_secret_text)
                .transpose()
        })
    }

    // ----------------------------------------------------------------- folders

    /// Create a folder.
    ///
    /// # Errors
    /// [`VaultError::InvalidValue`] for an empty/oversized name or a missing parent.
    pub fn create_folder(&mut self, name: &str, parent: Option<Id>) -> Result<Id> {
        check_folder_name(name)?;
        let Vault {
            db,
            clock,
            device_id,
            ids,
            ..
        } = self;
        db.with_tx(|tx| {
            if let Some(p) = &parent {
                require_live_folder(tx, p)?;
            }
            let hlc = clock.now()?;
            let id = ids.next(clock.wall_ms(), engine::os_random()?);
            write_folder(
                tx,
                clock,
                device_id,
                &Folder {
                    id,
                    name: name.to_owned(),
                    parent_id: parent,
                },
                false,
                hlc,
                None,
            )?;
            Ok(id)
        })
    }

    /// Rename a folder.
    ///
    /// # Errors
    /// [`VaultError::NotFound`].
    pub fn rename_folder(&mut self, id: &Id, name: &str) -> Result<()> {
        check_folder_name(name)?;
        self.edit_folder(id, |f| f.name = name.to_owned(), false)
    }

    /// Delete a folder (tombstone). Items in it keep their `folder_id`; the UI shows them at the top level.
    ///
    /// # Errors
    /// [`VaultError::NotFound`].
    pub fn delete_folder(&mut self, id: &Id) -> Result<()> {
        self.edit_folder(id, |_| {}, true)
    }

    fn edit_folder(
        &mut self,
        id: &Id,
        change: impl FnOnce(&mut Folder),
        deleted: bool,
    ) -> Result<()> {
        let Vault {
            db,
            clock,
            device_id,
            ..
        } = self;
        db.with_tx(|tx| {
            let row = tx.get_folder(id)?.ok_or(VaultError::NotFound)?;
            let mut f = Folder {
                id: row.id,
                name: row.name,
                parent_id: row.parent_id,
            };
            change(&mut f);
            let hlc = clock.now_after(Hlc::from_i64(row.hlc)?)?;
            write_folder(
                tx,
                clock,
                device_id,
                &f,
                deleted || row.deleted,
                hlc,
                Some(row.hlc),
            )
        })
    }

    /// Live (not deleted) folders ordered by name.
    ///
    /// # Errors
    /// Storage errors.
    pub fn list_folders(&mut self) -> Result<Vec<Folder>> {
        self.db.with_read(|tx| {
            Ok::<_, VaultError>(
                tx.list_folders()?
                    .into_iter()
                    .filter(|f| !f.deleted)
                    .map(|f| Folder {
                        id: f.id,
                        name: f.name,
                        parent_id: f.parent_id,
                    })
                    .collect(),
            )
        })
    }
}

// ---------------------------------------------------------------------- helpers

/// Largest value [`Vault::set_setting`] accepts.
pub const MAX_SETTING_BYTES: usize = 4096;
const MAX_SETTING_NAME_BYTES: usize = 64;

fn setting_key(name: &str) -> Result<String> {
    if name.is_empty()
        || name.len() > MAX_SETTING_NAME_BYTES
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.')
    {
        return Err(VaultError::InvalidValue("setting name"));
    }
    Ok(format!("setting.{name}"))
}

fn check_folder_name(name: &str) -> Result<()> {
    if name.trim().is_empty()
        || utf8_len(name) > MAX_FOLDER_NAME_BYTES
        || name.chars().any(char::is_control)
    {
        return Err(VaultError::InvalidValue("folder name"));
    }
    Ok(())
}

pub(crate) fn require_live_folder(tx: &Tx<'_>, id: &Id) -> Result<()> {
    match tx.get_folder(id)? {
        Some(f) if !f.deleted => Ok(()),
        _ => Err(VaultError::NotFound),
    }
}

pub(crate) fn write_folder(
    tx: &Tx<'_>,
    clock: &HlcClock,
    device: &Id,
    f: &Folder,
    deleted: bool,
    hlc: Hlc,
    base: Option<i64>,
) -> Result<()> {
    tx.upsert_folder(&FolderRow {
        id: f.id,
        name: f.name.clone(),
        parent_id: f.parent_id,
        hlc: hlc.to_i64(),
        device_id: *device,
        deleted,
    })?;
    fault::point()?;
    // Folder ops are recorded as [name, parent|null, deleted] under key `_folder` (see PR "Spec questions").
    let op = arya_vault_crypto::format::cbor::Value::Array(vec![
        arya_vault_crypto::format::cbor::Value::Text(f.name.clone()),
        f.parent_id
            .map_or(arya_vault_crypto::format::cbor::Value::Null, |p| {
                arya_vault_crypto::format::cbor::Value::Bytes(p.to_vec())
            }),
        arya_vault_crypto::format::cbor::Value::Bool(deleted),
    ])
    .encode()
    .map_err(|_| VaultError::Corrupt)?;
    tx.append_local_op(&f.id, FOLDER_OP_KEY, Some(&op), hlc.to_i64(), base)?;
    fault::point()?;
    tx.meta_set(HLC_STATE, &clock.last().to_i64().to_le_bytes())?;
    Ok(())
}

/// Apply one action's edits and persist the clock, all inside the caller's transaction.
#[allow(clippy::too_many_arguments)]
pub(crate) fn commit_edits(
    tx: &Tx<'_>,
    clock: &HlcClock,
    device: &Id,
    cfg: &VaultConfig,
    id: &Id,
    before: &Regs,
    hlc: Hlc,
    edits: Edits,
) -> Result<()> {
    for (key, value) in edits.0 {
        let base = engine::find(before, &key).map(|r| r.hlc);
        let op = Op {
            item_id: *id,
            key,
            value,
            hlc,
            device_id: *device,
            base_hlc: base,
        };
        engine::apply_op(tx, &op, cfg)?;
        tx.append_local_op(
            id,
            &op.key,
            op.value.as_deref().map(Vec::as_slice),
            hlc.to_i64(),
            base.map(Hlc::to_i64),
        )?;
        fault::point()?;
    }
    engine::refresh_item(tx, id, before)?;
    tx.meta_set(HLC_STATE, &clock.last().to_i64().to_le_bytes())?;
    fault::point()?;
    Ok(())
}

fn validate_new(new: &NewItem) -> Result<()> {
    check_text(&new.title, StdField::Title.max_bytes())?;
    for (f, v) in &new.fields {
        if !f.allowed_for(new.item_type) || *f == StdField::Title {
            return Err(VaultError::InvalidField {
                field: f.key(),
                item_type: new.item_type.as_str(),
            });
        }
        check_text(v, f.max_bytes())?;
    }
    for (i, (f, _)) in new.fields.iter().enumerate() {
        if new.fields[..i].iter().any(|(g, _)| g == f) {
            return Err(VaultError::InvalidValue("duplicate field"));
        }
    }
    if !new.urls.is_empty() && new.item_type != ItemType::Login {
        return Err(VaultError::InvalidField {
            field: "urls",
            item_type: new.item_type.as_str(),
        });
    }
    for u in &new.urls {
        check_text(u, MAX_FIELD_BYTES)?;
    }
    let mut tags = new.tags.clone();
    tags.sort();
    tags.dedup();
    if tags.len() != new.tags.len() {
        return Err(VaultError::InvalidValue("duplicate tag"));
    }
    if tags.len() > MAX_TAGS {
        return Err(VaultError::LimitExceeded("tags per item"));
    }
    tags.iter().try_for_each(|t| check_tag(t))
}

fn check_field_for(regs: &Regs, field: StdField) -> Result<()> {
    let ty = engine::item_type(regs).ok_or(VaultError::Corrupt)?;
    if field.allowed_for(ty) {
        Ok(())
    } else {
        Err(VaultError::InvalidField {
            field: field.key(),
            item_type: ty.as_str(),
        })
    }
}

fn require_type(regs: &Regs, want: ItemType, what: &'static str) -> Result<()> {
    let ty = engine::item_type(regs).ok_or(VaultError::Corrupt)?;
    if ty == want {
        Ok(())
    } else {
        Err(VaultError::InvalidField {
            field: what,
            item_type: ty.as_str(),
        })
    }
}

pub(crate) fn live_tags(regs: &Regs) -> Vec<String> {
    let mut t: Vec<String> = regs
        .iter()
        .filter(|(k, _)| k.starts_with(keys::TAG_PREFIX) && engine::bool_of(regs, k))
        .map(|(k, _)| k[keys::TAG_PREFIX.len()..].to_owned())
        .collect();
    t.sort();
    t
}

/// Ids of live custom fields (those whose `kind` register has a value), in creation order.
pub(crate) fn custom_ids(regs: &Regs) -> Vec<String> {
    let mut ids: Vec<String> = regs
        .iter()
        .filter(|(k, r)| {
            k.starts_with(keys::CUSTOM_PREFIX) && k.ends_with(".kind") && r.value.is_some()
        })
        .map(|(k, _)| k[keys::CUSTOM_PREFIX.len()..k.len() - ".kind".len()].to_owned())
        .collect();
    ids.sort();
    ids
}

fn require_custom(regs: &Regs, cid: &ElementId) -> Result<()> {
    if custom_ids(regs).iter().any(|c| c == cid.as_str()) {
        Ok(())
    } else {
        Err(VaultError::NotFound)
    }
}

pub(crate) fn view_of(id: &Id, regs: &Regs) -> Result<ItemView> {
    let ty = engine::item_type(regs).ok_or(VaultError::Corrupt)?;
    let summary = crate::query::summary_of(id, regs).ok_or(VaultError::Corrupt)?;
    let mut fields = Vec::new();
    let mut secret_fields = Vec::new();
    for f in crate::model::ALL_FIELDS
        .iter()
        .copied()
        .filter(|f| f.allowed_for(ty))
    {
        if let Some(raw) = engine::live_value(regs, f.key()) {
            if f.is_secret() {
                secret_fields.push(f);
            } else if let Ok(Value::Text(s)) = value::decode(raw) {
                fields.push((f, s));
            }
        }
    }
    let mut urls: Vec<UrlView> = regs
        .iter()
        .filter(|(k, _)| k.starts_with(keys::URL_PREFIX))
        .filter_map(|(k, _)| {
            Some(UrlView {
                id: ElementId::parse(&k[keys::URL_PREFIX.len()..]).ok()?,
                url: engine::text_of(regs, k)?,
            })
        })
        .collect();
    urls.sort_by(|a, b| a.id.cmp(&b.id));
    let custom = custom_ids(regs)
        .into_iter()
        .filter_map(|c| {
            let p = format!("{}{c}", keys::CUSTOM_PREFIX);
            let kind = CustomKind::parse(&engine::text_of(regs, &format!("{p}.kind"))?)?;
            let has_value = engine::live_value(regs, &format!("{p}.value")).is_some();
            Some(CustomView {
                id: ElementId::parse(&c).ok()?,
                label: engine::text_of(regs, &format!("{p}.label")).unwrap_or_default(),
                kind,
                value: if kind == CustomKind::Hidden {
                    None
                } else {
                    engine::text_of(regs, &format!("{p}.value"))
                },
                has_value,
            })
        })
        .collect();
    let created_at_ms = match engine::live_value(regs, keys::CREATED_AT).map(value::decode) {
        Some(Ok(Value::Int(i))) => Some(i),
        _ => None,
    };
    Ok(ItemView {
        summary,
        created_at_ms,
        fields,
        secret_fields,
        urls,
        tags: live_tags(regs),
        custom,
    })
}
