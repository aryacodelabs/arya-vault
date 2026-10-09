//! docs/14 §4.2: items and folders (unlocked only; otherwise `locked`).

use std::cmp::Reverse;

use arya_vault_vault as cv;
use zeroize::{Zeroize, Zeroizing};

use super::dto::{
    AppError, CustomKind, Folder, ItemSummary, ItemType, ItemView, ListFilter, NewItem, Page,
    SearchQuery, StdField, TrashEntry, VersionInfo,
};
use crate::convert::{
    hex_id, hlc_ms, opt_id, parse_element, parse_id, summary_of, to_usize, view_of,
};
use crate::error::ApiResult;
use crate::host::{self, Host, into_bytes};

/// Largest page the contract allows (docs/14 §3: `limit <= 200`).
pub const MAX_PAGE_LIMIT: u32 = 200;

fn id_of(id: &str) -> ApiResult<[u8; 16]> {
    parse_id(id, "id")
}

// ---------------------------------------------------------------------------------- reading

fn full_view(v: &mut cv::Vault, id: &[u8; 16]) -> ApiResult<cv::ItemView> {
    Ok(v.get_item(id)?)
}

/// Whether any field of the item has versions concurrent with the current one (docs/06 §6).
fn has_other_versions(v: &mut cv::Vault, view: &cv::ItemView) -> ApiResult<bool> {
    let id = view.summary.id;
    let std = view
        .fields
        .iter()
        .map(|(f, _)| *f)
        .chain(view.secret_fields.iter().copied())
        .map(cv::FieldRef::Std);
    let custom = view
        .custom
        .iter()
        .map(|c| cv::FieldRef::CustomValue(c.id.clone()));
    for f in std.chain(custom) {
        if !v.concurrent_versions(&id, &f)?.is_empty() {
            return Ok(true);
        }
    }
    Ok(false)
}

fn check_page(page: Page) -> ApiResult<(usize, usize)> {
    if page.limit > MAX_PAGE_LIMIT {
        return Err(AppError::validation("limit", "the page limit is 200"));
    }
    Ok((
        to_usize(page.offset, "offset")?,
        to_usize(page.limit, "limit")?,
    ))
}

fn core_filter(f: &ListFilter, t: Option<ItemType>) -> ApiResult<cv::ListFilter> {
    Ok(cv::ListFilter {
        item_type: t.map(Into::into),
        tag: f.tag.clone(),
        folder_id: opt_id(f.folder_id.as_deref(), "folderId")?,
        favorites_only: f.favorites_only,
    })
}

fn matches_filter(s: &ItemSummary, f: &ListFilter) -> bool {
    f.types.as_ref().is_none_or(|t| t.contains(&s.item_type))
        && f.folder_id
            .as_ref()
            .is_none_or(|d| s.folder_id.as_ref() == Some(d))
        && f.tag.as_ref().is_none_or(|t| s.tags.contains(t))
        && (!f.favorites_only || s.favorite)
}

fn summarize(
    v: &mut cv::Vault,
    rows: &[cv::ItemSummary],
    deleted: bool,
) -> ApiResult<Vec<ItemSummary>> {
    rows.iter()
        .map(|r| Ok(summary_of(&full_view(v, &r.id)?, deleted)))
        .collect()
}

/// The rows of `filter` in `[offset, offset + limit)`, newest first, trash included on request.
fn list_in(v: &mut cv::Vault, filter: &ListFilter, page: Page) -> ApiResult<Vec<ItemSummary>> {
    let (offset, limit) = check_page(page)?;
    let types: Vec<Option<ItemType>> = match &filter.types {
        None => vec![None],
        Some(t) => t.iter().copied().map(Some).collect(),
    };
    let single = types.len() <= 1 && !filter.include_trash;
    // One source: let the core page. Several (types, trash): take the head of each, merge, page.
    let (src_offset, src_limit) = if single {
        (offset, limit)
    } else {
        (0, offset + limit)
    };
    let mut rows: Vec<(cv::ItemSummary, bool)> = Vec::new();
    for t in types {
        let f = core_filter(filter, t)?;
        let page = cv::Page {
            offset: src_offset,
            limit: src_limit,
        };
        rows.extend(v.list(&f, page)?.into_iter().map(|s| (s, false)));
    }
    let mut out = summarize_rows(v, &rows)?;
    if filter.include_trash {
        for e in v.list_trash()?.into_iter().filter(|e| !e.purged) {
            let s = summary_of(&full_view(v, &e.summary.id)?, true);
            if matches_filter(&s, filter) {
                out.push(s);
            }
        }
    }
    if !single {
        out.sort_by_key(|s| (Reverse(s.updated_at), s.id.clone()));
        out = out.into_iter().skip(offset).take(limit).collect();
    }
    Ok(out)
}

fn summarize_rows(
    v: &mut cv::Vault,
    rows: &[(cv::ItemSummary, bool)],
) -> ApiResult<Vec<ItemSummary>> {
    let mut out = Vec::with_capacity(rows.len());
    for (r, deleted) in rows {
        out.extend(summarize(v, std::slice::from_ref(r), *deleted)?);
    }
    Ok(out)
}

/// `list(ListFilter, Page)`.
///
/// # Errors
/// `locked`, `validation` (`limit` above 200, bad ids).
pub fn list(filter: ListFilter, page: Page) -> Result<Vec<ItemSummary>, AppError> {
    host::call(|h| h.vault(|v| list_in(v, &filter, page)))
}

/// `search(SearchQuery)`: prefix full-text search over non-secret fields; trash is not searched
/// (`filter.includeTrash` is ignored).
///
/// # Errors
/// `locked`, `validation`.
pub fn search(query: SearchQuery) -> Result<Vec<ItemSummary>, AppError> {
    host::call(|h| {
        h.vault(|v| {
            let (offset, limit) = check_page(query.page)?;
            let filter = core_filter(&query.filter, None)?;
            let wanted = query.filter.types.as_deref();
            let q = cv::SearchQuery {
                text: query.text.clone(),
                filter,
                limit: offset + limit,
            };
            let rows: Vec<cv::ItemSummary> = v
                .search(&q)?
                .into_iter()
                .filter(|s| wanted.is_none_or(|t| t.contains(&ItemType::from(s.item_type))))
                .skip(offset)
                .take(limit)
                .collect();
            summarize(v, &rows, false)
        })
    })
}

/// `itemCount`: visible items (the trash excluded).
///
/// # Errors
/// `locked`.
pub fn item_count() -> Result<u64, AppError> {
    host::call(|h| {
        h.vault(|v| {
            let all = cv::Page {
                offset: 0,
                limit: usize::MAX,
            };
            Ok::<_, AppError>(v.list(&cv::ListFilter::default(), all)?.len() as u64)
        })
    })
}

/// `getItem`: everything about the item except its secrets.
///
/// # Errors
/// `locked`, `notFound`.
pub fn get_item(id: String) -> Result<ItemView, AppError> {
    let id = id_of(&id)?;
    host::call(|h| {
        h.vault(|v| {
            let view = full_view(v, &id)?;
            let other = has_other_versions(v, &view)?;
            Ok::<_, AppError>(view_of(&view, other))
        })
    })
}

/// `reveal(id, StdField)`: a secret standard field as UTF-8 bytes (`None` when unset).
///
/// # Errors
/// `locked`, `notFound`, `validation` (the field is not secret: read it from `getItem`).
pub fn reveal(id: String, field: StdField) -> Result<Option<Vec<u8>>, AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| Ok::<_, AppError>(v.reveal(&id, field.into())?.map(into_bytes))))
}

/// `revealCustom(id, ElementId)`.
///
/// # Errors
/// `locked`, `notFound`.
pub fn reveal_custom(id: String, element_id: String) -> Result<Option<Vec<u8>>, AppError> {
    let id = id_of(&id)?;
    let eid = parse_element(&element_id, "elementId")?;
    host::call(|h| h.vault(|v| Ok::<_, AppError>(v.reveal_custom(&id, &eid)?.map(into_bytes))))
}

// --------------------------------------------------------------------------------- writing

fn scrub(map: &mut std::collections::HashMap<StdField, String>) {
    for v in map.values_mut() {
        v.zeroize();
    }
}

/// `createItem(NewItem) -> ItemId`. Standard fields, URLs and tags are written in one
/// transaction; custom fields follow one by one, and the item is removed again if one fails.
///
/// # Errors
/// `locked`, `validation`, `limitReached`, `notFound` (folder).
pub fn create_item(mut new: NewItem) -> Result<String, AppError> {
    let result = host::call(|h| create_in(h, &new));
    scrub(&mut new.fields);
    for c in &mut new.custom {
        c.value.zeroize();
    }
    result
}

fn create_in(h: &mut Host, new: &NewItem) -> ApiResult<String> {
    let folder = opt_id(new.folder_id.as_deref(), "folderId")?;
    h.vault(|v| {
        let mut item = cv::NewItem::new(new.item_type.into(), &new.title);
        // Sorted so that the order of writes does not depend on hash-map iteration.
        let mut fields: Vec<_> = new.fields.iter().collect();
        fields.sort_by_key(|(f, _)| **f);
        for (f, value) in fields {
            if *f != StdField::Title {
                item = item.with_field((*f).into(), value);
            }
        }
        item.urls.clone_from(&new.urls);
        item.tags.clone_from(&new.tags);
        item.folder_id = folder;
        let id = v.create_item(item)?;
        for c in &new.custom {
            let r = v.add_custom_field(&id, c.kind.into(), &c.label, &c.value);
            if let Err(e) = r {
                let _ = v.delete_item(&id).and_then(|()| v.purge_item(&id));
                return Err(AppError::from(e));
            }
        }
        Ok::<_, AppError>(hex_id(&id))
    })
}

/// `setField(id, StdField, String)`.
///
/// # Errors
/// `locked`, `notFound`, `validation`, `limitReached`.
pub fn set_field(id: String, field: StdField, value: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let value = Zeroizing::new(value);
    host::call(|h| h.vault(|v| v.set_field(&id, field.into(), &value)))
}

/// `clearField(id, StdField)`.
///
/// # Errors
/// `locked`, `notFound`, `validation`.
pub fn clear_field(id: String, field: StdField) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.clear_field(&id, field.into())))
}

/// `setFields(id, Map<StdField,String>)`: all-or-nothing. The core has no multi-field
/// transaction, so the previous values are checked first and restored if a later write fails.
///
/// # Errors
/// `locked`, `notFound`, `validation`, `limitReached`; on any error nothing changed.
pub fn set_fields(
    id: String,
    mut fields: std::collections::HashMap<StdField, String>,
) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let r = host::call(|h| h.vault(|v| set_fields_in(v, &id, &fields)));
    scrub(&mut fields);
    r
}

fn set_fields_in(
    v: &mut cv::Vault,
    id: &[u8; 16],
    fields: &std::collections::HashMap<StdField, String>,
) -> ApiResult<()> {
    let view = v.get_item(id)?;
    // Validate against the item type and sizes before the first write.
    for (f, value) in fields {
        let f: cv::StdField = (*f).into();
        if !f.allowed_for(view.summary.item_type) {
            return Err(AppError::validation(
                f.key(),
                "the field does not exist for this item type",
            ));
        }
        if value.len() > f.max_bytes() {
            return Err(AppError::new(
                super::dto::AppErrorCode::LimitReached,
                "field value too large",
            )
            .with_field(f.key()));
        }
    }
    let mut order: Vec<_> = fields
        .iter()
        .map(|(f, value)| (cv::StdField::from(*f), value))
        .collect();
    order.sort_by_key(|(f, _)| *f);
    // Snapshot of what is there, to put back on failure.
    let mut before: Vec<(cv::StdField, Option<Zeroizing<String>>)> = Vec::new();
    for (f, _) in &order {
        let old = if f.is_secret() {
            v.reveal(id, *f)?
        } else {
            v.get_text(id, *f)?.map(Zeroizing::new)
        };
        before.push((*f, old));
    }
    for (done, (f, value)) in order.iter().enumerate() {
        if let Err(e) = v.set_field(id, *f, value) {
            for (f, old) in before.iter().take(done) {
                let _ = match old {
                    Some(o) => v.set_field(id, *f, o),
                    None => v.clear_field(id, *f),
                };
            }
            return Err(e.into());
        }
    }
    Ok(())
}

/// `toggleFavorite(id)`: returns the new state.
///
/// # Errors
/// `locked`, `notFound`.
pub fn toggle_favorite(id: String) -> Result<bool, AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.toggle_favorite(&id)))
}

/// `moveToFolder(id, FolderId?)`.
///
/// # Errors
/// `locked`, `notFound`.
pub fn move_to_folder(id: String, folder_id: Option<String>) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let folder = opt_id(folder_id.as_deref(), "folderId")?;
    host::call(|h| h.vault(|v| v.move_to_folder(&id, folder)))
}

/// `addTag`.
///
/// # Errors
/// `locked`, `notFound`, `validation`, `limitReached`.
pub fn add_tag(id: String, tag: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.add_tag(&id, &tag)))
}

/// `removeTag`.
///
/// # Errors
/// `locked`, `notFound`.
pub fn remove_tag(id: String, tag: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.remove_tag(&id, &tag)))
}

/// `addUrl` -> the new element id.
///
/// # Errors
/// `locked`, `notFound`, `validation`.
pub fn add_url(id: String, url: String) -> Result<String, AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.add_url(&id, &url).map(|e| e.as_str().to_owned())))
}

/// `setUrl`.
///
/// # Errors
/// `locked`, `notFound`, `validation`.
pub fn set_url(id: String, url_id: String, url: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let uid = parse_element(&url_id, "urlId")?;
    host::call(|h| h.vault(|v| v.set_url(&id, &uid, &url)))
}

/// `removeUrl`.
///
/// # Errors
/// `locked`, `notFound`.
pub fn remove_url(id: String, url_id: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let uid = parse_element(&url_id, "urlId")?;
    host::call(|h| h.vault(|v| v.remove_url(&id, &uid)))
}

/// `addCustomField` -> the new element id.
///
/// # Errors
/// `locked`, `notFound`, `limitReached` (100 per item).
pub fn add_custom_field(
    id: String,
    kind: CustomKind,
    label: String,
    value: String,
) -> ApiResult<String> {
    let id = id_of(&id)?;
    let value = Zeroizing::new(value);
    host::call(|h| {
        h.vault(|v| {
            v.add_custom_field(&id, kind.into(), &label, &value)
                .map(|e| e.as_str().to_owned())
        })
    })
}

/// `setCustomValue`.
///
/// # Errors
/// `locked`, `notFound`, `limitReached`.
pub fn set_custom_value(id: String, element_id: String, value: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let eid = parse_element(&element_id, "elementId")?;
    let value = Zeroizing::new(value);
    host::call(|h| h.vault(|v| v.set_custom_value(&id, &eid, &value)))
}

/// `setCustomLabel`.
///
/// # Errors
/// `locked`, `notFound`, `limitReached`.
pub fn set_custom_label(id: String, element_id: String, label: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let eid = parse_element(&element_id, "elementId")?;
    host::call(|h| h.vault(|v| v.set_custom_label(&id, &eid, &label)))
}

/// `removeCustomField`.
///
/// # Errors
/// `locked`, `notFound`.
pub fn remove_custom_field(id: String, element_id: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    let eid = parse_element(&element_id, "elementId")?;
    host::call(|h| h.vault(|v| v.remove_custom_field(&id, &eid)))
}

// --------------------------------------------------------------------------------- trash

/// `deleteItem(id)`: moves the item to the trash.
///
/// # Errors
/// `locked`, `notFound`.
pub fn delete_item(id: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.delete_item(&id)))
}

/// `restoreItem(id)`.
///
/// # Errors
/// `locked`, `notFound`, `validation` (not in the trash).
pub fn restore_item(id: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.restore_item(&id)))
}

/// `purgeItem(id)`: permanently removes an item that is in the trash.
///
/// # Errors
/// `locked`, `notFound`, `validation` (not in the trash).
pub fn purge_item(id: String) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| h.vault(|v| v.purge_item(&id)))
}

/// `listTrash()`.
///
/// # Errors
/// `locked`.
pub fn list_trash() -> Result<Vec<TrashEntry>, AppError> {
    host::call(|h| {
        h.vault(|v| {
            let day = u64::from(24u32 * 60 * 60 * 1000);
            let retention = v.config().trash_days.saturating_mul(day);
            let mut out = Vec::new();
            for e in v.list_trash()?.into_iter().filter(|e| !e.purged) {
                let deleted_at = e.deleted_at_ms.unwrap_or(0);
                let purges_at =
                    deleted_at.saturating_add(i64::try_from(retention).unwrap_or(i64::MAX));
                out.push(TrashEntry {
                    summary: summary_of(&full_view(v, &e.summary.id)?, true),
                    deleted_at,
                    purges_at,
                });
            }
            Ok::<_, AppError>(out)
        })
    })
}

/// `emptyTrash()`: purges every trashed item; returns how many.
///
/// # Errors
/// `locked`.
pub fn empty_trash() -> Result<u32, AppError> {
    host::call(|h| {
        h.vault(|v| {
            let mut n = 0u32;
            for e in v.list_trash()?.into_iter().filter(|e| !e.purged) {
                v.purge_item(&e.summary.id)?;
                n += 1;
            }
            Ok::<_, AppError>(n)
        })
    })
}

/// `purgeExpired()`: purges items that have been in the trash for the retention period.
///
/// # Errors
/// `locked`.
pub fn purge_expired() -> Result<u32, AppError> {
    host::call(|h| {
        h.vault(|v| Ok::<_, AppError>(u32::try_from(v.purge_expired()?).unwrap_or(u32::MAX)))
    })
}

// -------------------------------------------------------------------------------- history

fn versions_of(
    v: &mut cv::Vault,
    id: &[u8; 16],
    field: StdField,
) -> ApiResult<Vec<cv::VersionInfo>> {
    let f: cv::StdField = field.into();
    Ok(v.versions(id, &cv::FieldRef::Std(f))?)
}

fn find_version(
    v: &mut cv::Vault,
    id: &[u8; 16],
    field: StdField,
    hlc_ms: i64,
) -> ApiResult<cv::VersionInfo> {
    versions_of(v, id, field)?
        .into_iter()
        .find(|i| i.hlc.to_i64() == hlc_ms)
        .ok_or_else(|| AppError::new(super::dto::AppErrorCode::NotFound, "not found"))
}

/// `history(id, StdField)`: the current version followed by retained history, newest first.
///
/// # Errors
/// `locked`, `notFound`.
pub fn history(id: String, field: StdField) -> Result<Vec<VersionInfo>, AppError> {
    let id = id_of(&id)?;
    host::call(|h| {
        h.vault(|v| {
            Ok::<_, AppError>(
                versions_of(v, &id, field)?
                    .into_iter()
                    .map(|i| VersionInfo {
                        field,
                        hlc_ms: i.hlc.to_i64(),
                        at_ms: hlc_ms(i.hlc),
                        device_name: None,
                        is_current: i.current,
                        concurrent: i.concurrent,
                    })
                    .collect(),
            )
        })
    })
}

/// `revealVersion(id, StdField, hlcMs)`: the value of a stored version.
///
/// # Errors
/// `locked`, `notFound`.
pub fn reveal_version(
    id: String,
    field: StdField,
    hlc_ms: i64,
) -> Result<Option<Vec<u8>>, AppError> {
    let id = id_of(&id)?;
    host::call(|h| {
        h.vault(|v| {
            let info = find_version(v, &id, field, hlc_ms)?;
            let f = cv::FieldRef::Std(field.into());
            Ok::<_, AppError>(
                v.reveal_version(&id, &f, info.hlc, &info.device_id)?
                    .map(into_bytes),
            )
        })
    })
}

/// `restoreVersion(id, StdField, hlcMs)`: writes the old value as a new edit.
///
/// # Errors
/// `locked`, `notFound`, `validation` (item in the trash).
pub fn restore_version(id: String, field: StdField, hlc_ms: i64) -> Result<(), AppError> {
    let id = id_of(&id)?;
    host::call(|h| {
        h.vault(|v| {
            let info = find_version(v, &id, field, hlc_ms)?;
            let f = cv::FieldRef::Std(field.into());
            Ok::<_, AppError>(v.restore_version(&id, &f, info.hlc, &info.device_id)?)
        })
    })
}

// ------------------------------------------------------------------------------- folders

/// `createFolder(name, parentId?) -> FolderId`.
///
/// # Errors
/// `locked`, `validation`, `notFound` (parent).
pub fn create_folder(name: String, parent_id: Option<String>) -> Result<String, AppError> {
    let parent = opt_id(parent_id.as_deref(), "parentId")?;
    host::call(|h| h.vault(|v| v.create_folder(&name, parent).map(|id| hex_id(&id))))
}

/// `renameFolder`.
///
/// # Errors
/// `locked`, `notFound`, `validation`.
pub fn rename_folder(id: String, name: String) -> Result<(), AppError> {
    let id = parse_id(&id, "id")?;
    host::call(|h| h.vault(|v| v.rename_folder(&id, &name)))
}

/// `deleteFolder`: the items keep their folder id and show at the top level.
///
/// # Errors
/// `locked`, `notFound`.
pub fn delete_folder(id: String) -> Result<(), AppError> {
    let id = parse_id(&id, "id")?;
    host::call(|h| h.vault(|v| v.delete_folder(&id)))
}

/// `listFolders()`.
///
/// # Errors
/// `locked`.
pub fn list_folders() -> Result<Vec<Folder>, AppError> {
    host::call(|h| {
        h.vault(|v| {
            Ok::<_, AppError>(
                v.list_folders()?
                    .into_iter()
                    .map(|f| Folder {
                        id: hex_id(&f.id),
                        name: f.name,
                        parent_id: f.parent_id.as_ref().map(hex_id),
                    })
                    .collect(),
            )
        })
    })
}
