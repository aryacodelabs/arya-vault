//! Listing, trash and full-text search. All results respect the visibility
//! rule (doc 06 section 5.2) and contain no secrets.

use std::collections::{HashMap, HashSet};

use arya_vault_storage::{Id, ItemFilter, Store};

use crate::engine::{self, Regs};
use crate::error::Result;
use crate::hlc::Hlc;
use crate::model::{ItemType, keys};
use crate::value::{self, Value};
use crate::vault::Vault;
use crate::views::{ItemSummary, ListFilter, Page, SearchQuery, TrashEntry};

/// FTS candidates fetched before filters are applied.
const FTS_CANDIDATES: usize = 5_000;
/// Result sets up to this size are returned in relevance (bm25) order; larger ones by recency.
const RANK_THRESHOLD: usize = 300;
const DEFAULT_SEARCH_LIMIT: usize = 100;
const MAX_QUERY_TOKENS: usize = 16;
const MAX_TOKEN_CHARS: usize = 64;
/// Pages larger than this read titles with one bulk scan instead of per-row lookups.
const BULK_PAGE_THRESHOLD: usize = 256;

pub(crate) fn summary_of(id: &Id, regs: &Regs) -> Option<ItemSummary> {
    let row = engine::item_row(id, regs);
    Some(ItemSummary {
        id: *id,
        item_type: ItemType::parse(&row.item_type)?,
        title: engine::text_of(regs, "title").unwrap_or_default(),
        favorite: engine::bool_of(regs, keys::FAVORITE),
        folder_id: row.folder_id,
        updated: Hlc::from_i64(row.updated_hlc).ok()?,
    })
}

fn decode_text(bytes: &[u8]) -> Option<String> {
    match value::decode(bytes) {
        Ok(Value::Text(s)) => Some(s),
        _ => None,
    }
}

fn point_is_true(tx: &impl Store, id: &Id, key: &str) -> Result<bool> {
    Ok(tx
        .get_field(id, key)?
        .and_then(|f| f.value)
        .is_some_and(|v| is_true(&v)))
}

fn is_true(bytes: &[u8]) -> bool {
    matches!(value::decode(bytes), Ok(Value::Bool(true)))
}

/// Build an FTS5 query from user text: alphanumeric words, each quoted and
/// matched as a prefix, all required. Nothing the user types can reach the
/// FTS5 operator syntax.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(MAX_QUERY_TOKENS)
        .map(|t| {
            format!(
                "\"{}\"*",
                t.chars().take(MAX_TOKEN_CHARS).collect::<String>()
            )
        })
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join(" "))
    }
}

impl Vault {
    /// Visible items (trash excluded), most recently changed first.
    ///
    /// Small pages read titles/favorites with indexed point lookups for just
    /// the returned rows. Full lists and tag/favorite filters use two bulk
    /// scans instead, which beat one lookup per item.
    ///
    /// # Errors
    /// Storage errors.
    pub fn list(&mut self, filter: &ListFilter, page: Page) -> Result<Vec<ItemSummary>> {
        self.db.with_read(|tx| {
            let rows = tx.list_items(ItemFilter {
                include_deleted: true,
                item_type: filter.item_type.map(ItemType::as_str),
                folder_id: filter.folder_id,
            })?;
            let bulk =
                filter.favorites_only || filter.tag.is_some() || page.limit > BULK_PAGE_THRESHOLD;
            let (titles, favorites): (HashMap<Id, String>, HashSet<Id>) = if bulk {
                (
                    tx.fields_with_key("title")?
                        .into_iter()
                        .filter_map(|f| Some((f.item_id, decode_text(&f.value?)?)))
                        .collect(),
                    tx.fields_with_key(keys::FAVORITE)?
                        .into_iter()
                        .filter(|f| f.value.as_deref().is_some_and(is_true))
                        .map(|f| f.item_id)
                        .collect(),
                )
            } else {
                (HashMap::new(), HashSet::new())
            };
            let tagged: Option<HashSet<Id>> = match &filter.tag {
                None => None,
                Some(t) => {
                    let key = format!("{}{t}", keys::TAG_PREFIX);
                    Some(
                        tx.fields_with_key(&key)?
                            .into_iter()
                            .filter(|f| f.value.as_deref().is_some_and(is_true))
                            .map(|f| f.item_id)
                            .collect(),
                    )
                }
            };
            let mut out = Vec::new();
            let mut skipped = 0;
            for row in rows {
                let Some(item_type) = ItemType::parse(&row.item_type) else {
                    continue;
                };
                if row.deleted && !engine::is_visible(&engine::load_regs(tx, &row.id)?) {
                    continue;
                }
                if filter.favorites_only && !favorites.contains(&row.id) {
                    continue; // favorites_only forces the bulk path
                }
                if tagged.as_ref().is_some_and(|s| !s.contains(&row.id)) {
                    continue;
                }
                if skipped < page.offset {
                    skipped += 1;
                    continue;
                }
                if out.len() >= page.limit {
                    break;
                }
                let favorite = if bulk {
                    favorites.contains(&row.id)
                } else {
                    point_is_true(tx, &row.id, keys::FAVORITE)?
                };
                let title = if bulk {
                    titles.get(&row.id).cloned().unwrap_or_default()
                } else {
                    tx.get_field(&row.id, "title")?
                        .and_then(|f| decode_text(&f.value?))
                        .unwrap_or_default()
                };
                out.push(ItemSummary {
                    id: row.id,
                    item_type,
                    title,
                    favorite,
                    folder_id: row.folder_id,
                    updated: Hlc::from_i64(row.updated_hlc)?,
                });
            }
            Ok(out)
        })
    }

    /// Items in the trash (deleted and not resurrected), newest deletion first.
    ///
    /// # Errors
    /// Storage errors.
    pub fn list_trash(&mut self) -> Result<Vec<TrashEntry>> {
        self.db.with_read(|tx| {
            let mut out = Vec::new();
            let rows = tx.list_items(ItemFilter {
                include_deleted: true,
                ..Default::default()
            })?;
            for row in rows.iter().filter(|r| r.deleted) {
                let regs = engine::load_regs(tx, &row.id)?;
                if engine::is_visible(&regs) {
                    continue;
                }
                let Some(summary) = summary_of(&row.id, &regs) else {
                    continue;
                };
                let deleted_at_ms =
                    match engine::live_value(&regs, keys::DELETED_AT).map(value::decode) {
                        Some(Ok(Value::Int(i))) => Some(i),
                        _ => None,
                    };
                let purged = !regs
                    .iter()
                    .any(|(k, r)| !engine::survives_purge(k) && r.value.is_some());
                out.push(TrashEntry {
                    summary,
                    deleted_at_ms,
                    purged,
                });
            }
            out.sort_by(|a, b| {
                b.deleted_at_ms
                    .cmp(&a.deleted_at_ms)
                    .then(a.summary.id.cmp(&b.summary.id))
            });
            Ok(out)
        })
    }

    /// Full-text search over title, username/holder/name, URLs, notes/body and tags,
    /// with prefix matching ("gith" finds "GitHub") and the filters of [`ListFilter`].
    /// Passwords, TOTP seeds, card numbers, CVV/PIN and identity ids are never indexed.
    /// Results are ranked by relevance. With no text this is [`Vault::list`] with a limit.
    ///
    /// # Errors
    /// Storage errors.
    pub fn search(&mut self, q: &SearchQuery) -> Result<Vec<ItemSummary>> {
        let limit = if q.limit == 0 {
            DEFAULT_SEARCH_LIMIT
        } else {
            q.limit
        };
        if q.text.trim().is_empty() {
            return self.list(&q.filter, Page { offset: 0, limit });
        }
        let Some(query) = fts_query(&q.text) else {
            return Ok(Vec::new());
        };
        self.db.with_read(|tx| {
            let mut out = Vec::new();
            // Ranking by relevance costs time proportional to the number of matches (about 70 ms for
            // 3,000 matches in 20,000 items). So fetch the matches by recency (a few ms), and only
            // when the match set is small enough to rank cheaply redo it in relevance order.
            // Filters are applied afterwards, over up to `FTS_CANDIDATES` matches.
            let by_recency = tx.fts_search_recent(&query, FTS_CANDIDATES)?;
            let ids = if by_recency.len() <= RANK_THRESHOLD {
                tx.fts_search(&query, RANK_THRESHOLD)?
            } else {
                by_recency
            };
            for id in ids {
                let regs = engine::load_regs(tx, &id)?;
                if !engine::is_visible(&regs) {
                    continue; // defence in depth: the index only holds visible items
                }
                let Some(s) = summary_of(&id, &regs) else {
                    continue;
                };
                if q.filter.item_type.is_some_and(|t| t != s.item_type)
                    || q.filter.folder_id.is_some_and(|f| Some(f) != s.folder_id)
                    || (q.filter.favorites_only && !s.favorite)
                    || q.filter.tag.as_ref().is_some_and(|t| {
                        !engine::bool_of(&regs, &format!("{}{t}", keys::TAG_PREFIX))
                    })
                {
                    continue;
                }
                out.push(s);
                if out.len() >= limit {
                    break;
                }
            }
            Ok(out)
        })
    }

    /// Rebuild the whole search index from the registers (repair; normally never needed).
    ///
    /// # Errors
    /// Storage errors.
    pub fn reindex_all(&mut self) -> Result<usize> {
        self.db.with_tx(|tx| {
            tx.fts_clear()?;
            let mut n = 0;
            for row in tx.list_items(ItemFilter {
                include_deleted: true,
                ..Default::default()
            })? {
                let regs = engine::load_regs(tx, &row.id)?;
                engine::refresh_item(tx, &row.id, &Vec::new())?;
                n += usize::from(
                    engine::fts_doc(&regs) != Default::default() && engine::is_visible(&regs),
                );
            }
            Ok(n)
        })
    }
}
