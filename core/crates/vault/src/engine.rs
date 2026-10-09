//! The mechanical core shared by local mutations and (later, M4) remote ops:
//! load/apply registers, keep the item cache and the search index consistent.
//!
//! Invariants maintained here, inside the caller's transaction:
//! * `field` holds each register's winner, `field_history` its retained losers
//!   (the exact set computed by [`FieldState`]);
//! * the `item` row is a pure function of the item's registers;
//! * the FTS index holds a document for an item iff the item is visible, has a
//!   known type and the document is non-empty, and that document is a pure
//!   function of the registers. Passwords, TOTP seeds, card numbers/CVV/PIN and
//!   identity ids are never part of it.

use arya_vault_storage::{FieldRow, FtsDoc, Id, ItemRow, Store};
use zeroize::Zeroizing;

use crate::error::{Result, VaultError};
use crate::fault;
use crate::hlc::Hlc;
use crate::model::{ItemType, VaultConfig, keys};
use crate::register::{FieldState, Register};
use crate::value::{self, Value};

/// One mutation of one register.
pub(crate) struct Op {
    pub item_id: Id,
    pub key: String,
    pub value: Option<Zeroizing<Vec<u8>>>,
    pub hlc: Hlc,
    pub device_id: Id,
    pub base_hlc: Option<Hlc>,
}

/// All registers of an item, ordered by key.
pub(crate) type Regs = Vec<(String, Register)>;

pub(crate) fn to_register(row: FieldRow) -> Result<Register> {
    Ok(Register {
        value: row.value.map(Zeroizing::new),
        hlc: Hlc::from_i64(row.hlc)?,
        device_id: row.device_id,
        base_hlc: row.base_hlc.map(Hlc::from_i64).transpose()?,
    })
}

pub(crate) fn to_row(item_id: &Id, key: &str, r: &Register) -> FieldRow {
    FieldRow {
        item_id: *item_id,
        key: key.to_owned(),
        value: r.value.as_ref().map(|v| v.to_vec()),
        hlc: r.hlc.to_i64(),
        device_id: r.device_id,
        base_hlc: r.base_hlc.map(Hlc::to_i64),
    }
}

pub(crate) fn load_regs(s: &impl Store, id: &Id) -> Result<Regs> {
    s.fields_for_item(id)?
        .into_iter()
        .map(|r| Ok((r.key.clone(), to_register(r)?)))
        .collect()
}

pub(crate) fn find<'a>(regs: &'a Regs, key: &str) -> Option<&'a Register> {
    regs.iter().find(|(k, _)| k == key).map(|(_, r)| r)
}

pub(crate) fn live_value<'a>(regs: &'a Regs, key: &str) -> Option<&'a [u8]> {
    find(regs, key).and_then(|r| r.value.as_deref().map(Vec::as_slice))
}

pub(crate) fn text_of(regs: &Regs, key: &str) -> Option<String> {
    match value::decode(live_value(regs, key)?) {
        Ok(Value::Text(s)) => Some(s),
        _ => None,
    }
}

pub(crate) fn bool_of(regs: &Regs, key: &str) -> bool {
    matches!(
        live_value(regs, key).map(value::decode),
        Some(Ok(Value::Bool(true)))
    )
}

pub(crate) fn item_type(regs: &Regs) -> Option<ItemType> {
    text_of(regs, keys::TYPE).and_then(|t| ItemType::parse(&t))
}

/// Visibility (doc 06 section 5.2, review M6): visible iff not deleted, or any
/// register other than the deletion bookkeeping has `hlc > deleted_hlc`.
pub(crate) fn is_visible(regs: &Regs) -> bool {
    let Some(del) = find(regs, keys::DELETED) else {
        return true;
    };
    if !bool_of(regs, keys::DELETED) {
        return true;
    }
    regs.iter()
        .filter(|(k, _)| k != keys::DELETED && k != keys::DELETED_AT)
        .any(|(_, r)| r.hlc > del.hlc)
}

pub(crate) fn state_of(s: &impl Store, id: &Id, key: &str) -> Result<FieldState> {
    let winner = s.get_field(id, key)?.map(to_register).transpose()?;
    let history = s
        .history_for(id, key)?
        .into_iter()
        .map(to_register)
        .collect::<Result<Vec<_>>>()?;
    Ok(FieldState { winner, history })
}

fn history_limit(cfg: &VaultConfig, key: &str) -> usize {
    use crate::model::StdField;
    if StdField::from_key(key).is_some_and(StdField::is_sensitive_history) {
        cfg.history_sensitive
    } else {
        cfg.history_other
    }
}

fn same_history(a: &[Register], b: &[Register]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.total_cmp(y).is_eq())
}

/// Merge one op into the stored state of its register (idempotent, order-independent).
pub(crate) fn apply_op(s: &impl Store, op: &Op, cfg: &VaultConfig) -> Result<()> {
    if s.get_item(&op.item_id)?.is_none() {
        // The row is rebuilt from the registers by `refresh_item`; this only satisfies the FK.
        s.upsert_item(&ItemRow {
            id: op.item_id,
            item_type: "unknown".into(),
            folder_id: None,
            deleted: false,
            deleted_hlc: None,
            updated_hlc: op.hlc.to_i64(),
        })?;
        fault::point()?;
    }
    let before = state_of(s, &op.item_id, &op.key)?;
    let mut after = before.clone();
    after.apply(
        Register {
            value: op.value.clone(),
            hlc: op.hlc,
            device_id: op.device_id,
            base_hlc: op.base_hlc,
        },
        history_limit(cfg, &op.key),
    );
    if after.winner != before.winner
        && let Some(w) = &after.winner
    {
        s.put_field(&to_row(&op.item_id, &op.key, w))?;
        fault::point()?;
    }
    if !same_history(&after.history, &before.history) {
        s.prune_history(&op.item_id, &op.key, 0)?;
        fault::point()?;
        for h in &after.history {
            s.add_history(&to_row(&op.item_id, &op.key, h))?;
        }
        fault::point()?;
    }
    Ok(())
}

/// FTS document derived from registers (never includes secrets other than the note body).
pub(crate) fn fts_doc(regs: &Regs) -> FtsDoc {
    let t = |k: &str| text_of(regs, k).unwrap_or_default();
    let join = |parts: Vec<String>| {
        parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    };
    let ty = item_type(regs);
    let username = match ty {
        Some(ItemType::Login) => t("username"),
        Some(ItemType::Card) => t("holder"),
        Some(ItemType::Identity) => join(vec![
            t("first_name"),
            t("middle_name"),
            t("last_name"),
            t("email"),
        ]),
        _ => String::new(),
    };
    let notes = match ty {
        Some(ItemType::Note) => t("body"),
        _ => t("notes"),
    };
    let urls = join(
        regs.iter()
            .filter(|(k, _)| k.starts_with(keys::URL_PREFIX))
            .filter_map(|(k, _)| text_of(regs, k))
            .collect(),
    );
    let tags = join(
        regs.iter()
            .filter(|(k, _)| k.starts_with(keys::TAG_PREFIX) && bool_of(regs, k))
            .map(|(k, _)| k[keys::TAG_PREFIX.len()..].to_owned())
            .collect(),
    );
    FtsDoc {
        title: t("title"),
        username,
        urls,
        notes,
        tags,
    }
}

fn is_empty(d: &FtsDoc) -> bool {
    d.title.is_empty()
        && d.username.is_empty()
        && d.urls.is_empty()
        && d.notes.is_empty()
        && d.tags.is_empty()
}

fn indexed_doc(regs: &Regs) -> Option<FtsDoc> {
    if item_type(regs).is_none() || !is_visible(regs) {
        return None;
    }
    let d = fts_doc(regs);
    if is_empty(&d) { None } else { Some(d) }
}

/// The `item` row as a pure function of the registers.
pub(crate) fn item_row(id: &Id, regs: &Regs) -> ItemRow {
    let folder = live_value(regs, keys::FOLDER).and_then(|b| match value::decode(b) {
        Ok(Value::Bytes(v)) => <Id>::try_from(v).ok(),
        _ => None,
    });
    ItemRow {
        id: *id,
        item_type: item_type(regs)
            .map_or("unknown", ItemType::as_str)
            .to_owned(),
        folder_id: folder,
        deleted: bool_of(regs, keys::DELETED),
        deleted_hlc: find(regs, keys::DELETED).map(|r| r.hlc.to_i64()),
        updated_hlc: regs.iter().map(|(_, r)| r.hlc.to_i64()).max().unwrap_or(0),
    }
}

/// Bring the item row and the search index in line with the registers after a
/// change from `before` to the current stored registers.
pub(crate) fn refresh_item(s: &impl Store, id: &Id, before: &Regs) -> Result<()> {
    let after = load_regs(s, id)?;
    let row = item_row(id, &after);
    if s.get_item(id)?.as_ref() != Some(&row) {
        s.upsert_item(&row)?;
        fault::point()?;
    }
    let (old, new) = (indexed_doc(before), indexed_doc(&after));
    if old != new {
        if let Some(d) = &old {
            s.fts_remove(id, d)?;
            fault::point()?;
        }
        if let Some(d) = &new {
            s.fts_index(id, d)?;
            fault::point()?;
        }
    }
    Ok(())
}

/// Registers that survive a purge (non-sensitive bookkeeping that keeps the tombstone meaningful).
pub(crate) fn survives_purge(key: &str) -> bool {
    matches!(
        key,
        keys::DELETED | keys::DELETED_AT | keys::TYPE | keys::SCHEMA_VERSION | keys::CREATED_AT
    )
}

/// Garbage-collect a trashed item: blank every sensitive register (same hlc, so
/// it can never outrank anything), drop its history and its index entry.
pub(crate) fn purge(s: &impl Store, id: &Id) -> Result<()> {
    let regs = load_regs(s, id)?;
    for (key, reg) in &regs {
        if survives_purge(key) {
            continue;
        }
        if reg.value.is_some() {
            let mut blank = reg.clone();
            blank.value = None;
            s.put_field(&to_row(id, key, &blank))?;
            fault::point()?;
        }
        s.prune_history(id, key, 0)?;
    }
    refresh_item(s, id, &regs)
}

pub(crate) fn require_item(s: &impl Store, id: &Id) -> Result<ItemRow> {
    s.get_item(id)?.ok_or(VaultError::NotFound)
}

/// UUIDv7 generator (RFC 9562): 48-bit unix ms, version 7, a 12-bit counter in
/// `rand_a` so ids made in the same millisecond still sort in creation order,
/// and 62 random bits.
pub(crate) struct IdGen {
    last_ms: u64,
    counter: u16,
}

impl IdGen {
    pub fn new() -> Self {
        Self {
            last_ms: 0,
            counter: 0,
        }
    }

    pub fn next(&mut self, wall_ms: u64, rand: [u8; 8]) -> Id {
        let mut ms = wall_ms.max(self.last_ms);
        if ms == self.last_ms {
            if self.counter >= 0x0fff {
                ms += 1;
                self.counter = 0;
            } else {
                self.counter += 1;
            }
        } else {
            self.counter = 0;
        }
        self.last_ms = ms;
        let mut id = [0u8; 16];
        id[..6].copy_from_slice(&ms.to_be_bytes()[2..]);
        id[6] = 0x70 | ((self.counter >> 8) as u8 & 0x0f);
        id[7] = self.counter as u8;
        id[8..].copy_from_slice(&rand);
        id[8] = (id[8] & 0x3f) | 0x80;
        id
    }
}

pub(crate) fn os_random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| VaultError::Rng)?;
    Ok(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_v7_layout_and_ordering() {
        let mut g = IdGen::new();
        let a = g.next(1_700_000_000_000, [0xFF; 8]);
        let b = g.next(1_700_000_000_000, [0x00; 8]); // same ms, smaller random part
        let c = g.next(1_700_000_000_001, [0x00; 8]);
        for id in [a, b, c] {
            assert_eq!(id[6] >> 4, 7, "version 7");
            assert_eq!(id[8] >> 6, 0b10, "RFC 4122 variant");
        }
        assert!(a < b && b < c, "ids sort in creation order");
        assert_eq!(&a[..6], &1_700_000_000_000u64.to_be_bytes()[2..]);
        // Wall clock going backwards never makes ids go backwards.
        let d = g.next(5, [0; 8]);
        assert!(d > c);
    }

    #[test]
    fn uuid_counter_overflow_advances_time() {
        let mut g = IdGen::new();
        let mut prev = g.next(1_000, [0; 8]);
        for _ in 0..10_000 {
            let n = g.next(1_000, [0; 8]);
            assert!(n > prev);
            prev = n;
        }
    }
}
