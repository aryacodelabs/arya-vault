//! Applying an [`ImportBundle`] to a vault, and collecting a vault for export.

use std::collections::{HashMap, HashSet};

use arya_vault_crypto::kdf::KdfParams;
use arya_vault_crypto::rng::{OsRng, Rng};
use arya_vault_storage::{Id, ItemFilter, Store, Tx};
use zeroize::Zeroizing;

use super::aryavault::{self, ExportFolder, ExportItem};
use super::export_csv::{self, CsvExport};
use super::{ImportBundle, ImportError, ImportHistory, ImportItem, PlaintextRiskAcknowledged};
use crate::engine::{self, os_random};
use crate::error::VaultError;
use crate::model::{HARD_ITEM_LIMIT, ItemType, StdField, keys};
use crate::value::{self, Value};
use crate::vault::{
    Edits, Vault, commit_edits, custom_ids, live_tags, require_live_folder, write_folder,
};
use crate::views::Folder;

/// How a bundle is applied.
#[derive(Debug, Clone, Copy)]
pub struct ImportOptions {
    /// Compute the report without changing anything (preview).
    pub dry_run: bool,
    /// Skip logins that already exist with the same title, username and URL (case-insensitive,
    /// trimmed), and repeats within the bundle. Default `true`.
    pub skip_duplicates: bool,
    /// Folder for items that have none. It must exist.
    pub target_folder: Option<Id>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            skip_duplicates: true,
            target_folder: None,
        }
    }
}

/// What a commit did (or, for a dry run, would do).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportReport {
    /// This was a preview; nothing was written.
    pub dry_run: bool,
    /// Items created (or that would be created).
    pub created: usize,
    /// Ids of created items (empty for a dry run).
    pub created_ids: Vec<Id>,
    /// Folders created (or that would be).
    pub folders_created: usize,
    /// Indices into [`ImportBundle::items`] skipped as duplicates.
    pub duplicates: Vec<usize>,
    /// Indices into [`ImportBundle::items`] that failed validation when committed.
    pub invalid: Vec<usize>,
}

/// Whether adding `new` items to a vault holding `existing` would pass the hard limit (docs/05 section 10).
fn exceeds_hard_limit(existing: u64, new: usize) -> bool {
    existing.saturating_add(new as u64) > HARD_ITEM_LIMIT
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

fn norm_url(s: &str) -> String {
    norm(s).trim_end_matches('/').to_owned()
}

/// Duplicate keys of a login: one `(title, username, url)` per URL (or with an empty URL).
fn dup_keys(title: &str, user: &str, urls: &[&str]) -> Vec<(String, String, String)> {
    let (t, u) = (norm(title), norm(user));
    if urls.is_empty() {
        vec![(t, u, String::new())]
    } else {
        urls.iter()
            .map(|x| (t.clone(), u.clone(), norm_url(x)))
            .collect()
    }
}

fn text_of_raw(raw: &[u8]) -> Option<String> {
    match value::decode(raw) {
        Ok(Value::Text(s)) => Some(s),
        _ => None,
    }
}

/// Keys of every live login already in the vault.
fn existing_login_keys(tx: &Tx<'_>) -> Result<HashSet<(String, String, String)>, VaultError> {
    let rows = tx.list_items(ItemFilter {
        include_deleted: false,
        item_type: Some("login"),
        folder_id: None,
    })?;
    let live: HashSet<Id> = rows.iter().map(|r| r.id).collect();
    let by_id = |key: &str| -> Result<HashMap<Id, String>, VaultError> {
        Ok(tx
            .fields_with_key(key)?
            .into_iter()
            .filter(|f| live.contains(&f.item_id))
            .filter_map(|f| Some((f.item_id, text_of_raw(&f.value?)?)))
            .collect())
    };
    let titles = by_id("title")?;
    let users = by_id("username")?;
    let mut urls: HashMap<Id, Vec<String>> = HashMap::new();
    for f in tx.fields_with_key_prefix(keys::URL_PREFIX)? {
        if let (true, Some(u)) = (
            live.contains(&f.item_id),
            f.value.as_deref().and_then(text_of_raw),
        ) {
            urls.entry(f.item_id).or_default().push(u);
        }
    }
    let mut out = HashSet::new();
    for r in rows {
        let u: Vec<&str> = urls
            .get(&r.id)
            .map(|v| v.iter().map(String::as_str).collect())
            .unwrap_or_default();
        out.extend(dup_keys(
            titles.get(&r.id).map_or("", String::as_str),
            users.get(&r.id).map_or("", String::as_str),
            &u,
        ));
    }
    Ok(out)
}

fn item_dup_keys(it: &ImportItem) -> Vec<(String, String, String)> {
    let user = it
        .fields
        .iter()
        .find(|(f, _)| *f == StdField::Username)
        .map_or("", |(_, v)| v.as_str());
    let urls: Vec<&str> = it.urls.iter().map(String::as_str).collect();
    dup_keys(&it.title, user, &urls)
}

enum FolderPlan {
    Existing(Id),
    New,
}

struct Plan {
    folders: Vec<Option<FolderPlan>>, // by bundle folder index; None = not needed
    create: Vec<usize>,
    duplicates: Vec<usize>,
    invalid: Vec<usize>,
}

fn plan(tx: &Tx<'_>, bundle: &ImportBundle, opts: &ImportOptions) -> Result<Plan, VaultError> {
    if let Some(f) = &opts.target_folder {
        require_live_folder(tx, f)?;
    }
    let mut seen = if opts.skip_duplicates {
        existing_login_keys(tx)?
    } else {
        HashSet::new()
    };
    let (mut create, mut duplicates, mut invalid) = (Vec::new(), Vec::new(), Vec::new());
    for (i, it) in bundle.items.iter().enumerate() {
        if it.validate().is_err() || it.folder.is_some_and(|f| f >= bundle.folders.len()) {
            invalid.push(i);
            continue;
        }
        if opts.skip_duplicates && it.item_type == ItemType::Login {
            let keys = item_dup_keys(it);
            if keys.iter().any(|k| seen.contains(k)) {
                duplicates.push(i);
                continue;
            }
            seen.extend(keys);
        }
        create.push(i);
    }
    // Which bundle folders are needed (referenced by a created item, or an ancestor of one).
    let mut needed = vec![false; bundle.folders.len()];
    for &i in &create {
        let mut f = bundle.items[i].folder;
        while let Some(idx) = f {
            if needed[idx] {
                break;
            }
            needed[idx] = true;
            f = bundle.folders[idx].parent.filter(|p| *p < idx);
        }
    }
    let existing = tx.list_folders()?;
    let mut resolved: Vec<Option<Id>> = vec![None; bundle.folders.len()];
    let mut folders = Vec::with_capacity(bundle.folders.len());
    for (idx, f) in bundle.folders.iter().enumerate() {
        if !needed[idx] {
            folders.push(None);
            continue;
        }
        let parent_id = f.parent.filter(|p| *p < idx).and_then(|p| resolved[p]);
        let hit = existing
            .iter()
            .find(|e| !e.deleted && e.name == f.name && e.parent_id == parent_id)
            .map(|e| e.id);
        resolved[idx] = hit;
        folders.push(Some(hit.map_or(FolderPlan::New, FolderPlan::Existing)));
    }
    Ok(Plan {
        folders,
        create,
        duplicates,
        invalid,
    })
}

impl Vault {
    /// Applies `bundle` in **one transaction**: either every created item (with its fields,
    /// history, `local_op` records and search-index entries) and folder is written, or nothing is.
    ///
    /// Items are created with fresh ids and the current time (source ids and timestamps are
    /// not preserved). With [`ImportOptions::dry_run`] nothing is written and the report says
    /// what would happen. Items that fail validation, and duplicates (when
    /// [`ImportOptions::skip_duplicates`] is set), are skipped and listed in the report.
    ///
    /// # Errors
    /// [`VaultError::LimitExceeded`] (via [`ImportError::Vault`]) if the import would exceed the
    /// hard item limit; storage failures. On any error the vault is unchanged.
    pub fn commit_import(
        &mut self,
        bundle: &ImportBundle,
        opts: &ImportOptions,
    ) -> Result<ImportReport, ImportError> {
        if opts.dry_run {
            let p = self.db.with_read(|tx| {
                let p = plan(tx, bundle, opts)?;
                if exceeds_hard_limit(tx.count_items()?, p.create.len()) {
                    return Err(VaultError::LimitExceeded("item count"));
                }
                Ok::<_, VaultError>(p)
            })?;
            return Ok(ImportReport {
                dry_run: true,
                created: p.create.len(),
                created_ids: Vec::new(),
                folders_created: p
                    .folders
                    .iter()
                    .filter(|f| matches!(f, Some(FolderPlan::New)))
                    .count(),
                duplicates: p.duplicates,
                invalid: p.invalid,
            });
        }
        let Vault {
            db,
            clock,
            device_id,
            cfg,
            ids,
            ..
        } = self;
        let report = db.with_tx(|tx| {
            let p = plan(tx, bundle, opts)?;
            if exceeds_hard_limit(tx.count_items()?, p.create.len()) {
                return Err(VaultError::LimitExceeded("item count"));
            }
            // Folders first (parents before children: the bundle guarantees parent < child).
            let mut folder_ids: Vec<Option<Id>> = vec![None; bundle.folders.len()];
            let mut folders_created = 0;
            for (idx, fp) in p.folders.iter().enumerate() {
                match fp {
                    None => {}
                    Some(FolderPlan::Existing(id)) => folder_ids[idx] = Some(*id),
                    Some(FolderPlan::New) => {
                        let hlc = clock.now()?;
                        let id = ids.next(clock.wall_ms(), os_random()?);
                        let parent_id = bundle.folders[idx]
                            .parent
                            .filter(|q| *q < idx)
                            .and_then(|q| folder_ids[q]);
                        write_folder(
                            tx,
                            clock,
                            device_id,
                            &Folder {
                                id,
                                name: bundle.folders[idx].name.clone(),
                                parent_id,
                            },
                            false,
                            hlc,
                            None,
                        )?;
                        folder_ids[idx] = Some(id);
                        folders_created += 1;
                    }
                }
            }
            let mut created_ids = Vec::with_capacity(p.create.len());
            for &i in &p.create {
                let it = &bundle.items[i];
                let folder = it.folder.and_then(|f| folder_ids[f]).or(opts.target_folder);
                let hlc = clock.now()?;
                let wall = clock.wall_ms();
                let id = ids.next(wall, os_random()?);
                let mut e = Edits::default();
                e.text(keys::TYPE, it.item_type.as_str());
                e.set(
                    keys::SCHEMA_VERSION,
                    Some(value::encode_int(keys::ITEM_SCHEMA)),
                );
                e.set(
                    keys::CREATED_AT,
                    Some(value::encode_int(i64::try_from(wall).unwrap_or(i64::MAX))),
                );
                e.text(StdField::Title.key(), &it.title);
                for (f, v) in &it.fields {
                    // With history, the oldest version is written first and the rest replayed below.
                    let first = it
                        .history
                        .iter()
                        .find(|h| h.field == *f)
                        .and_then(|h| h.older.first())
                        .unwrap_or(v);
                    e.text(f.key(), first);
                }
                if let Some(f) = folder {
                    e.set(keys::FOLDER, Some(value::encode_bytes(&f)));
                }
                if it.favorite {
                    e.flag(keys::FAVORITE, true);
                }
                for t in &it.tags {
                    e.flag(&format!("{}{t}", keys::TAG_PREFIX), true);
                }
                for u in &it.urls {
                    let eid = crate::model::ElementId::from_id(&ids.next(wall, os_random()?));
                    e.text(&format!("{}{}", keys::URL_PREFIX, eid.as_str()), u);
                }
                for c in &it.custom {
                    let eid = crate::model::ElementId::from_id(&ids.next(wall, os_random()?));
                    let p = format!("{}{}", keys::CUSTOM_PREFIX, eid.as_str());
                    e.text(&format!("{p}.kind"), c.kind.as_str());
                    e.text(&format!("{p}.label"), &c.label);
                    e.text(&format!("{p}.value"), &c.value);
                }
                commit_edits(tx, clock, device_id, cfg, &id, &Vec::new(), hlc, e)?;

                // Replay history: older[1..] then the current value, each a normal edit.
                let mut last = hlc;
                for (f, current) in &it.fields {
                    let Some(ImportHistory { older, .. }) =
                        it.history.iter().find(|h| h.field == *f)
                    else {
                        continue;
                    };
                    let later = older
                        .iter()
                        .skip(1)
                        .map(|v| v.as_str())
                        .chain(std::iter::once(current.as_str()));
                    for v in later.filter(|_| !older.is_empty()) {
                        let regs = engine::load_regs(tx, &id)?;
                        let h = clock.now_after(last)?;
                        let mut e = Edits::default();
                        e.text(f.key(), v);
                        commit_edits(tx, clock, device_id, cfg, &id, &regs, h, e)?;
                        last = h;
                    }
                }
                created_ids.push(id);
            }
            Ok(ImportReport {
                dry_run: false,
                created: created_ids.len(),
                created_ids,
                folders_created,
                duplicates: p.duplicates,
                invalid: p.invalid,
            })
        })?;
        Ok(report)
    }

    /// Reads every visible item (and live folder) for export. Secrets are copied into
    /// zeroizing buffers.
    fn collect_export(
        &mut self,
        include_history: bool,
    ) -> Result<(Vec<ExportItem>, Vec<ExportFolder>), VaultError> {
        self.db.with_read(|tx| {
            // Folders, parents before children.
            let mut pending: Vec<_> = tx
                .list_folders()?
                .into_iter()
                .filter(|f| !f.deleted)
                .collect();
            let mut folders: Vec<ExportFolder> = Vec::new();
            while !pending.is_empty() {
                let before = pending.len();
                let mut rest = Vec::new();
                for f in pending {
                    if f.parent_id
                        .is_none_or(|p| folders.iter().any(|x| x.id == p))
                    {
                        folders.push(ExportFolder {
                            id: f.id,
                            name: f.name,
                            parent: f.parent_id,
                        });
                    } else {
                        rest.push(f);
                    }
                }
                pending = rest;
                if pending.len() == before {
                    // Orphaned or cyclic parents: export them at the top level.
                    for f in pending.drain(..) {
                        folders.push(ExportFolder {
                            id: f.id,
                            name: f.name,
                            parent: None,
                        });
                    }
                }
            }
            let mut rows = tx.list_items(ItemFilter {
                include_deleted: true,
                item_type: None,
                folder_id: None,
            })?;
            rows.sort_by_key(|r| r.id);
            let mut items = Vec::new();
            for row in rows {
                let regs = engine::load_regs(tx, &row.id)?;
                let Some(item_type) = engine::item_type(&regs) else {
                    continue;
                };
                if !engine::is_visible(&regs) {
                    continue;
                }
                let mut fields = Vec::new();
                let mut history = Vec::new();
                for f in crate::model::ALL_FIELDS
                    .iter()
                    .copied()
                    .filter(|f| *f != StdField::Title && f.allowed_for(item_type))
                {
                    let Some(raw) = engine::live_value(&regs, f.key()) else {
                        continue;
                    };
                    fields.push((f, value::decode_secret_text(raw)?));
                    if include_history {
                        let state = engine::state_of(tx, &row.id, f.key())?;
                        let mut older = Vec::new();
                        for h in state.history.iter().rev() {
                            if let Some(v) = h.value.as_deref() {
                                older.push(value::decode_secret_text(v)?);
                            }
                        }
                        if !older.is_empty() {
                            history.push(ImportHistory { field: f, older });
                        }
                    }
                }
                let mut urls: Vec<(String, String)> = regs
                    .iter()
                    .filter(|(k, _)| k.starts_with(keys::URL_PREFIX))
                    .filter_map(|(k, _)| Some((k.clone(), engine::text_of(&regs, k)?)))
                    .collect();
                urls.sort();
                let mut custom = Vec::new();
                for c in custom_ids(&regs) {
                    let p = format!("{}{c}", keys::CUSTOM_PREFIX);
                    let Some(kind) = engine::text_of(&regs, &format!("{p}.kind"))
                        .and_then(|k| crate::model::CustomKind::parse(&k))
                    else {
                        continue;
                    };
                    let value = engine::live_value(&regs, &format!("{p}.value"))
                        .map(value::decode_secret_text)
                        .transpose()?
                        .unwrap_or_else(|| Zeroizing::new(String::new()));
                    custom.push(super::ImportCustom {
                        kind,
                        label: engine::text_of(&regs, &format!("{p}.label")).unwrap_or_default(),
                        value,
                    });
                }
                let folder = row.folder_id.filter(|f| folders.iter().any(|x| x.id == *f));
                items.push(ExportItem {
                    id: row.id,
                    item_type,
                    title: engine::text_of(&regs, StdField::Title.key()).unwrap_or_default(),
                    folder,
                    favorite: engine::bool_of(&regs, keys::FAVORITE),
                    fields,
                    urls: urls.into_iter().map(|(_, u)| u).collect(),
                    tags: live_tags(&regs),
                    custom,
                    history,
                });
            }
            Ok::<_, VaultError>((items, folders))
        })
    }

    /// Exports logins and secure notes as **plaintext** CSV (see [`CsvExport`] and the
    /// module docs for what is neutralised and dropped). Requires proof that the user was
    /// shown the plaintext warning.
    ///
    /// ```no_run
    /// use arya_vault_vault::{PlaintextRiskAcknowledged, Vault};
    /// # fn demo(v: &mut Vault) {
    /// // With the acknowledgement token the call compiles...
    /// let _ = v.export_csv(PlaintextRiskAcknowledged::acknowledge_plaintext_risk());
    /// # }
    /// ```
    ///
    /// ```compile_fail
    /// # fn demo(v: &mut arya_vault_vault::Vault) {
    /// // ...without it, it does not.
    /// let _ = v.export_csv();
    /// # }
    /// ```
    ///
    /// # Errors
    /// Vault or I/O failures.
    pub fn export_csv(
        &mut self,
        _ack: PlaintextRiskAcknowledged,
    ) -> Result<CsvExport, ImportError> {
        let (items, folders) = self.collect_export(false)?;
        export_csv::write_csv(&items, &folders)
    }

    /// Exports every visible item and live folder to a password-protected AryaVault export
    /// (docs/13-export-format.md), optionally with field history.
    ///
    /// `cost` supplies the Argon2id `m_kib`, `t` and `p` (floors and ceilings are enforced);
    /// its salt is ignored and replaced by fresh randomness.
    ///
    /// # Errors
    /// [`ImportError::InvalidKdf`] for out-of-range cost, [`ImportError::Crypto`] for an empty
    /// password or RNG failure, vault failures.
    pub fn export_aryavault(
        &mut self,
        password: &str,
        cost: &KdfParams,
        include_history: bool,
    ) -> Result<Vec<u8>, ImportError> {
        self.export_aryavault_with_rng(password, cost, include_history, &mut OsRng)
    }

    pub(crate) fn export_aryavault_with_rng(
        &mut self,
        password: &str,
        cost: &KdfParams,
        include_history: bool,
        rng: &mut dyn Rng,
    ) -> Result<Vec<u8>, ImportError> {
        let (items, folders) = self.collect_export(include_history)?;
        let payload = aryavault::build_payload(&items, &folders, include_history);
        aryavault::seal_container(&payload, password, cost, rng)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hard_limit_boundaries() {
        assert!(!exceeds_hard_limit(0, 100_000));
        assert!(exceeds_hard_limit(0, 100_001));
        assert!(!exceeds_hard_limit(99_999, 1));
        assert!(exceeds_hard_limit(100_000, 1));
        assert!(!exceeds_hard_limit(100_000, 0));
        assert!(exceeds_hard_limit(u64::MAX, 1));
    }
}
