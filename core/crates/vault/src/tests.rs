//! Behavioural tests of the vault layer on a real encrypted database.

use arya_vault_storage::{
    CreateParams, Db, DbKey, FieldRow, FolderRow, Id, ItemRow, LocalOp, Store,
};
use proptest::prelude::*;
use zeroize::Zeroizing;

use crate::engine::{self, Op};
use crate::value::{self, Value};
use crate::*;

const T0: u64 = 1_700_000_000_000;
const DAY: u64 = 86_400_000;

struct Env {
    _dir: tempfile::TempDir,
    clock: ManualClock,
    v: Vault,
}

fn env_with(device: u8) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let params = CreateParams {
        vault_id: [0xA1; 16],
        device_id: [device; 16],
        epoch: 1,
        header_version: 1,
    };
    let db = Db::create(&path, DbKey::from_bytes([7; 32]), &params).unwrap();
    let clock = ManualClock::new(T0);
    let v = Vault::open(db, Box::new(clock.clone())).unwrap();
    Env {
        _dir: dir,
        clock,
        v,
    }
}
fn env() -> Env {
    env_with(0xB2)
}
fn login(v: &mut Vault, title: &str) -> Id {
    v.create_item(NewItem::new(ItemType::Login, title)).unwrap()
}
fn titles(v: &mut Vault) -> Vec<String> {
    v.list(&ListFilter::default(), Page::ALL)
        .unwrap()
        .into_iter()
        .map(|s| s.title)
        .collect()
}
fn ops(v: &mut Vault) -> Vec<LocalOp> {
    v.db.with_read(|tx| tx.pending_local_ops(100_000)).unwrap()
}
fn search(v: &mut Vault, text: &str) -> Vec<String> {
    v.search(&SearchQuery {
        text: text.into(),
        ..Default::default()
    })
    .unwrap()
    .into_iter()
    .map(|s| s.title)
    .collect()
}
fn h(pt: u64, c: u16) -> Hlc {
    Hlc::new(pt, c).unwrap()
}

/// Apply an op as if it arrived from another device (what M4 will do), keeping the cache and index in step.
fn remote(
    v: &mut Vault,
    id: &Id,
    key: &str,
    val: Option<Zeroizing<Vec<u8>>>,
    hlc: Hlc,
    dev: u8,
    base: Option<Hlc>,
) {
    let cfg = v.cfg;
    v.db.with_tx(|tx| -> Result<()> {
        let before = engine::load_regs(tx, id)?;
        engine::apply_op(
            tx,
            &Op {
                item_id: *id,
                key: key.into(),
                value: val,
                hlc,
                device_id: [dev; 16],
                base_hlc: base,
            },
            &cfg,
        )?;
        engine::refresh_item(tx, id, &before)
    })
    .unwrap();
}
fn rtext(s: &str) -> Option<Zeroizing<Vec<u8>>> {
    Some(value::encode_text(s))
}
fn rflag(b: bool) -> Option<Zeroizing<Vec<u8>>> {
    Some(value::encode_bool(b))
}

// ------------------------------------------------------------------ whole-db snapshot

#[derive(Debug, PartialEq)]
struct Dump {
    items: Vec<ItemRow>,
    fields: Vec<FieldRow>,
    history: Vec<FieldRow>,
    folders: Vec<FolderRow>,
    local_ops: Vec<LocalOp>,
    fts: Vec<(String, Vec<Id>)>,
    hlc_state: Option<Vec<u8>>,
}

fn dump(v: &mut Vault, probes: &[&str], with_ops: bool) -> Dump {
    v.db.with_read(|tx| -> Result<Dump> {
        let items = tx.list_items(arya_vault_storage::ItemFilter {
            include_deleted: true,
            ..Default::default()
        })?;
        let mut fields = Vec::new();
        let mut history = Vec::new();
        for i in &items {
            for f in tx.fields_for_item(&i.id)? {
                history.extend(tx.history_for(&i.id, &f.key)?);
                fields.push(f);
            }
        }
        let mut fts = Vec::new();
        for p in probes {
            let mut ids = tx.fts_search(&query::fts_query(p).unwrap(), 1000)?;
            ids.sort();
            fts.push(((*p).to_owned(), ids));
        }
        Ok(Dump {
            items,
            fields,
            history,
            folders: tx.list_folders()?,
            local_ops: if with_ops {
                tx.pending_local_ops(100_000)?
            } else {
                Vec::new()
            },
            fts,
            hlc_state: if with_ops {
                tx.meta_get("hlc_state")?
            } else {
                None
            },
        })
    })
    .unwrap()
}

// --------------------------------------------------------------------------- CRUD

#[test]
fn create_get_list_every_type() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(
            NewItem::new(ItemType::Login, "GitHub")
                .with_field(StdField::Username, "alice")
                .with_field(StdField::Password, "CANARY-pw-1")
                .with_field(StdField::Notes, "work"),
        )
        .unwrap();
    let n = v
        .create_item(
            NewItem::new(ItemType::Note, "Ideas").with_field(StdField::Body, "# CANARY-body"),
        )
        .unwrap();
    let c = v
        .create_item(
            NewItem::new(ItemType::Card, "Visa")
                .with_field(StdField::Holder, "A Lice")
                .with_field(StdField::Number, "4111-CANARY"),
        )
        .unwrap();
    let i = v
        .create_item(
            NewItem::new(ItemType::Identity, "Me")
                .with_field(StdField::FirstName, "Alice")
                .with_field(StdField::Email, "a@example.test"),
        )
        .unwrap();
    assert_eq!(v.item_count().unwrap(), 4);
    for id in [l, n, c, i] {
        assert_eq!(id[6] >> 4, 7, "UUIDv7");
    }
    let view = v.get_item(&l).unwrap();
    assert_eq!(view.summary.title, "GitHub");
    assert_eq!(view.summary.item_type, ItemType::Login);
    assert_eq!(view.created_at_ms, Some(T0 as i64));
    assert!(view.fields.contains(&(StdField::Username, "alice".into())));
    assert!(view.fields.contains(&(StdField::Notes, "work".into())));
    assert_eq!(view.secret_fields, vec![StdField::Password]);
    assert_eq!(v.get_item(&n).unwrap().secret_fields, vec![StdField::Body]);
    assert_eq!(
        v.get_text(&l, StdField::Username).unwrap().as_deref(),
        Some("alice")
    );
    assert_eq!(
        &*v.reveal(&l, StdField::Password).unwrap().unwrap(),
        "CANARY-pw-1"
    );
    assert_eq!(
        &*v.reveal(&n, StdField::Body).unwrap().unwrap(),
        "# CANARY-body"
    );
    assert_eq!(
        &*v.reveal(&c, StdField::Number).unwrap().unwrap(),
        "4111-CANARY"
    );
    assert_eq!(v.list(&ListFilter::default(), Page::ALL).unwrap().len(), 4);
    assert_eq!(
        v.list(
            &ListFilter {
                item_type: Some(ItemType::Card),
                ..Default::default()
            },
            Page::ALL
        )
        .unwrap()
        .len(),
        1
    );
    // Newest change first, pagination.
    let all = v.list(&ListFilter::default(), Page::ALL).unwrap();
    assert!(all.windows(2).all(|w| w[0].updated >= w[1].updated));
    assert_eq!(
        v.list(
            &ListFilter::default(),
            Page {
                offset: 1,
                limit: 2
            }
        )
        .unwrap(),
        all[1..3].to_vec()
    );
}

#[test]
fn field_rules_per_type_and_secret_gates() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    let n = v.create_item(NewItem::new(ItemType::Note, "n")).unwrap();
    assert!(matches!(
        v.set_field(&n, StdField::Password, "p"),
        Err(VaultError::InvalidField { .. })
    ));
    assert!(matches!(
        v.set_field(&l, StdField::Body, "p"),
        Err(VaultError::InvalidField { .. })
    ));
    assert!(matches!(
        v.create_item(NewItem::new(ItemType::Note, "t").with_field(StdField::Cvv, "1")),
        Err(VaultError::InvalidField { .. })
    ));
    assert!(matches!(
        v.create_item(NewItem::new(ItemType::Login, "t").with_field(StdField::Title, "dup")),
        Err(VaultError::InvalidField { .. })
    ));
    assert!(matches!(
        v.get_text(&l, StdField::Password),
        Err(VaultError::SecretField("password"))
    ));
    assert!(matches!(
        v.reveal(&l, StdField::Title),
        Err(VaultError::NotSecret("title"))
    ));
    assert!(matches!(
        v.get_text(&[9; 16], StdField::Title),
        Err(VaultError::NotFound)
    ));
    v.set_field(&l, StdField::Password, "CANARY-pw").unwrap();
    v.clear_field(&l, StdField::Password).unwrap();
    assert!(v.reveal(&l, StdField::Password).unwrap().is_none());
    assert!(v.get_item(&l).unwrap().secret_fields.is_empty());
}

#[test]
fn limits_from_doc_05_section_10() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    v.set_field(&l, StdField::Notes, &"a".repeat(MAX_FIELD_BYTES))
        .unwrap();
    assert!(matches!(
        v.set_field(&l, StdField::Notes, &"a".repeat(MAX_FIELD_BYTES + 1)),
        Err(VaultError::LimitExceeded(_))
    ));
    let n = v.create_item(NewItem::new(ItemType::Note, "n")).unwrap();
    v.set_field(&n, StdField::Body, &"b".repeat(MAX_BODY_BYTES))
        .unwrap();
    assert!(matches!(
        v.set_field(&n, StdField::Body, &"b".repeat(MAX_BODY_BYTES + 1)),
        Err(VaultError::LimitExceeded(_))
    ));
    assert!(matches!(
        v.create_item(NewItem::new(
            ItemType::Login,
            &"t".repeat(MAX_FIELD_BYTES + 1)
        )),
        Err(VaultError::LimitExceeded(_))
    ));
    // 50 tags, not 51.
    for i in 0..MAX_TAGS {
        v.add_tag(&l, &format!("t{i}")).unwrap();
    }
    assert!(matches!(
        v.add_tag(&l, "one-too-many"),
        Err(VaultError::LimitExceeded(_))
    ));
    v.add_tag(&l, "t0").unwrap(); // idempotent, not counted again
    v.remove_tag(&l, "t0").unwrap();
    v.add_tag(&l, "one-too-many").unwrap();
    // 100 custom fields, not 101.
    let l2 = login(v, "y");
    for i in 0..MAX_CUSTOM_FIELDS {
        v.add_custom_field(&l2, CustomKind::Text, &format!("l{i}"), "v")
            .unwrap();
    }
    assert!(matches!(
        v.add_custom_field(&l2, CustomKind::Text, "x", "v"),
        Err(VaultError::LimitExceeded(_))
    ));
    // Bad tags.
    for bad in ["", " lead", "trail ", "a\nb", &"x".repeat(65)] {
        assert!(
            matches!(v.add_tag(&l, bad), Err(VaultError::InvalidValue(_))),
            "{bad:?}"
        );
    }
    assert!(matches!(
        v.create_item(NewItem {
            tags: vec!["a".into(), "a".into()],
            ..NewItem::new(ItemType::Login, "t")
        }),
        Err(VaultError::InvalidValue(_))
    ));
    assert!(matches!(
        v.create_item(NewItem {
            urls: vec!["u".into()],
            ..NewItem::new(ItemType::Note, "t")
        }),
        Err(VaultError::InvalidField { .. })
    ));
}

#[test]
fn hard_item_limit() {
    let mut e = env();
    e.v.db
        .with_tx(|tx| -> Result<()> {
            for n in 0..HARD_ITEM_LIMIT {
                let mut id = [0u8; 16];
                id[8..].copy_from_slice(&n.to_be_bytes());
                tx.upsert_item(&ItemRow {
                    id,
                    item_type: "unknown".into(),
                    folder_id: None,
                    deleted: false,
                    deleted_hlc: None,
                    updated_hlc: 1,
                })?;
            }
            Ok(())
        })
        .unwrap();
    assert!(e.v.at_soft_item_limit().unwrap());
    assert!(matches!(
        e.v.create_item(NewItem::new(ItemType::Login, "x")),
        Err(VaultError::LimitExceeded("item count"))
    ));
}

#[test]
fn custom_fields_are_individually_addressable_registers() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    let a = v
        .add_custom_field(&l, CustomKind::Text, "Pet", "Rex")
        .unwrap();
    let b = v
        .add_custom_field(&l, CustomKind::Hidden, "PIN", "CANARY-hidden")
        .unwrap();
    let view = v.get_item(&l).unwrap();
    assert_eq!(view.custom.len(), 2);
    assert_eq!(view.custom[0].id, a, "creation order");
    assert_eq!(view.custom[0].value.as_deref(), Some("Rex"));
    assert_eq!(
        (view.custom[1].value.clone(), view.custom[1].has_value),
        (None, true),
        "hidden value never in a view"
    );
    assert_eq!(&*v.reveal_custom(&l, &b).unwrap().unwrap(), "CANARY-hidden");
    v.set_custom_value(&l, &a, "Fido").unwrap();
    v.set_custom_label(&l, &a, "Dog").unwrap();
    // Each register is separate: label edit and value edit leave their own local ops.
    let keys: Vec<String> = ops(v).into_iter().map(|o| o.key).collect();
    assert!(
        keys.contains(&format!("custom.{}.value", a.as_str()))
            && keys.contains(&format!("custom.{}.label", a.as_str()))
    );
    assert_eq!(v.get_item(&l).unwrap().custom[0].label, "Dog");
    v.remove_custom_field(&l, &a).unwrap();
    assert_eq!(v.get_item(&l).unwrap().custom.len(), 1);
    assert!(matches!(
        v.set_custom_value(&l, &a, "x"),
        Err(VaultError::NotFound)
    ));
    assert!(matches!(
        ElementId::parse("nope"),
        Err(VaultError::InvalidValue(_))
    ));
}

#[test]
fn tags_and_urls_are_per_element_registers() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(NewItem {
            urls: vec!["https://one.test".into(), "https://two.test".into()],
            tags: vec!["work".into(), "dev".into()],
            ..NewItem::new(ItemType::Login, "x")
        })
        .unwrap();
    let view = v.get_item(&l).unwrap();
    assert_eq!(view.tags, vec!["dev", "work"]);
    assert_eq!(
        view.urls.iter().map(|u| u.url.as_str()).collect::<Vec<_>>(),
        ["https://one.test", "https://two.test"],
        "creation order"
    );
    let u3 = v.add_url(&l, "https://three.test").unwrap();
    v.set_url(&l, &u3, "https://three.example").unwrap();
    v.remove_url(&l, &view.urls[0].id).unwrap();
    v.remove_tag(&l, "dev").unwrap();
    let view = v.get_item(&l).unwrap();
    assert_eq!(view.tags, vec!["work"]);
    assert_eq!(
        view.urls.iter().map(|u| u.url.as_str()).collect::<Vec<_>>(),
        ["https://two.test", "https://three.example"]
    );
    assert!(matches!(
        v.remove_url(&l, &view.urls[0].id)
            .and_then(|()| v.remove_url(&l, &view.urls[0].id)),
        Err(VaultError::NotFound)
    ));
    // Concurrent tag edits from two devices merge element-wise: both adds survive.
    remote(
        v,
        &l,
        "tags.from-b",
        rflag(true),
        h(T0 + 10_000, 0),
        0xC3,
        None,
    );
    assert_eq!(v.get_item(&l).unwrap().tags, vec!["from-b", "work"]);
}

#[test]
fn favorite_folders_and_moves() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    assert!(v.toggle_favorite(&l).unwrap());
    assert!(v.get_item(&l).unwrap().summary.favorite);
    assert_eq!(
        v.list(
            &ListFilter {
                favorites_only: true,
                ..Default::default()
            },
            Page::ALL
        )
        .unwrap()
        .len(),
        1
    );
    assert!(!v.toggle_favorite(&l).unwrap());
    let f = v.create_folder("Work", None).unwrap();
    let sub = v.create_folder("Sub", Some(f)).unwrap();
    assert!(matches!(
        v.create_folder("  ", None),
        Err(VaultError::InvalidValue(_))
    ));
    assert!(matches!(
        v.create_folder("x", Some([1; 16])),
        Err(VaultError::NotFound)
    ));
    v.move_to_folder(&l, Some(f)).unwrap();
    assert_eq!(v.get_item(&l).unwrap().summary.folder_id, Some(f));
    assert_eq!(
        v.list(
            &ListFilter {
                folder_id: Some(f),
                ..Default::default()
            },
            Page::ALL
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        v.list(
            &ListFilter {
                folder_id: Some(sub),
                ..Default::default()
            },
            Page::ALL
        )
        .unwrap()
        .is_empty()
    );
    v.rename_folder(&f, "Job").unwrap();
    assert_eq!(
        v.list_folders()
            .unwrap()
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["Job", "Sub"]
    );
    v.delete_folder(&sub).unwrap();
    assert_eq!(v.list_folders().unwrap().len(), 1);
    assert!(
        matches!(v.move_to_folder(&l, Some(sub)), Err(VaultError::NotFound)),
        "cannot move into a deleted folder"
    );
    v.move_to_folder(&l, None).unwrap();
    assert_eq!(v.get_item(&l).unwrap().summary.folder_id, None);
    assert!(matches!(
        v.rename_folder(&[3; 16], "x"),
        Err(VaultError::NotFound)
    ));
}

// ------------------------------------------------------- mutation bookkeeping (doc 05)

#[test]
fn mutation_stamps_base_pushes_history_and_records_a_local_op() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    v.set_field(&l, StdField::Password, "CANARY-1").unwrap();
    e.clock.advance(5);
    v.set_field(&l, StdField::Password, "CANARY-2").unwrap();
    let versions = v.versions(&l, &FieldRef::Std(StdField::Password)).unwrap();
    assert_eq!(versions.len(), 2);
    assert!(versions[0].current && !versions[1].current);
    assert_eq!(
        versions[0].base_hlc,
        Some(versions[1].hlc),
        "base_hlc = the version the author saw"
    );
    assert_eq!(
        versions[1].base_hlc, None,
        "first value had nothing before it"
    );
    assert!(versions[0].hlc > versions[1].hlc);
    let pw_ops: Vec<LocalOp> = ops(v).into_iter().filter(|o| o.key == "password").collect();
    assert_eq!(pw_ops.len(), 2);
    assert_eq!(pw_ops[1].base_hlc, Some(pw_ops[0].hlc));
    assert_eq!(
        value::decode(pw_ops[1].value.as_ref().unwrap()).unwrap(),
        Value::Text("CANARY-2".into())
    );
    assert_eq!(
        &*v.reveal_version(
            &l,
            &FieldRef::Std(StdField::Password),
            versions[1].hlc,
            &versions[1].device_id
        )
        .unwrap()
        .unwrap(),
        "CANARY-1"
    );
    // All registers of one user action share one HLC.
    let create_ops: Vec<LocalOp> = ops(v)
        .into_iter()
        .filter(|o| o.item_id == l)
        .take(4)
        .collect();
    assert!(create_ops.windows(2).all(|w| w[0].hlc == w[1].hlc));
}

#[test]
fn history_limits_20_for_sensitive_and_5_for_others() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "t0");
    for i in 1..=30 {
        v.set_field(&l, StdField::Password, &format!("pw{i}"))
            .unwrap();
        v.set_field(&l, StdField::Username, &format!("u{i}"))
            .unwrap();
    }
    assert_eq!(
        v.versions(&l, &FieldRef::Std(StdField::Password))
            .unwrap()
            .len(),
        1 + 20
    );
    assert_eq!(
        v.versions(&l, &FieldRef::Std(StdField::Username))
            .unwrap()
            .len(),
        1 + 5
    );
    // Sequential edits form a chain: nothing is "concurrent".
    assert!(
        v.concurrent_versions(&l, &FieldRef::Std(StdField::Password))
            .unwrap()
            .is_empty()
    );
    // The newest retained password is the one just replaced.
    let vs = v.versions(&l, &FieldRef::Std(StdField::Password)).unwrap();
    assert_eq!(
        &*v.reveal_version(
            &l,
            &FieldRef::Std(StdField::Password),
            vs[1].hlc,
            &vs[1].device_id
        )
        .unwrap()
        .unwrap(),
        "pw29"
    );
    v.set_config(VaultConfig {
        history_other: 2,
        ..VaultConfig::default()
    });
    v.set_field(&l, StdField::Username, "u31").unwrap();
    assert_eq!(
        v.versions(&l, &FieldRef::Std(StdField::Username))
            .unwrap()
            .len(),
        1 + 2,
        "limit is user-adjustable"
    );
}

#[test]
fn restore_version_writes_a_new_edit_on_top_of_the_current_one() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    v.set_field(&l, StdField::Password, "old").unwrap();
    v.set_field(&l, StdField::Password, "new").unwrap();
    let f = FieldRef::Std(StdField::Password);
    let vs = v.versions(&l, &f).unwrap();
    v.restore_version(&l, &f, vs[1].hlc, &vs[1].device_id)
        .unwrap();
    assert_eq!(&*v.reveal(&l, StdField::Password).unwrap().unwrap(), "old");
    let after = v.versions(&l, &f).unwrap();
    assert_eq!(
        after.len(),
        3,
        "restore adds a version; nothing is rewritten"
    );
    assert_eq!(after[0].base_hlc, Some(vs[0].hlc));
    assert!(matches!(
        v.restore_version(&l, &f, h(1, 1), &[0; 16]),
        Err(VaultError::NotFound)
    ));
}

#[test]
fn hlc_persists_across_reopen_and_stays_monotonic() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("v.db");
    let params = CreateParams {
        vault_id: [1; 16],
        device_id: [2; 16],
        epoch: 1,
        header_version: 1,
    };
    let clock = ManualClock::new(T0);
    let mut v = Vault::open(
        Db::create(&path, DbKey::from_bytes([7; 32]), &params).unwrap(),
        Box::new(clock.clone()),
    )
    .unwrap();
    let l = login(&mut v, "x");
    let first = v.get_item(&l).unwrap().summary.updated;
    v.close().unwrap();
    clock.set(T0 - 10 * DAY); // wall clock went far back while the app was closed
    let mut v = Vault::open(
        Db::open(&path, DbKey::from_bytes([7; 32])).unwrap(),
        Box::new(clock),
    )
    .unwrap();
    v.set_field(&l, StdField::Notes, "later").unwrap();
    assert!(v.get_item(&l).unwrap().summary.updated > first);
    assert_eq!(v.device_id(), [2; 16]);
}

// ------------------------------------------------------------- trash, purge, visibility

#[test]
fn trash_restore_and_visibility() {
    let mut e = env();
    let v = &mut e.v;
    let a = login(v, "keep");
    let b = login(v, "trash me");
    v.delete_item(&b).unwrap();
    v.delete_item(&b).unwrap(); // idempotent
    assert_eq!(titles(v), ["keep"]);
    let trash = v.list_trash().unwrap();
    assert_eq!(
        (
            trash.len(),
            trash[0].summary.title.as_str(),
            trash[0].deleted_at_ms,
            trash[0].purged
        ),
        (1, "trash me", Some(T0 as i64), false)
    );
    assert!(matches!(
        v.set_field(&b, StdField::Notes, "x"),
        Err(VaultError::InTrash)
    ));
    assert!(matches!(v.add_tag(&b, "x"), Err(VaultError::InTrash)));
    assert!(matches!(v.restore_item(&a), Err(VaultError::NotInTrash)));
    assert!(matches!(v.purge_item(&a), Err(VaultError::NotInTrash)));
    v.restore_item(&b).unwrap();
    assert_eq!(titles(v).len(), 2);
    assert!(v.list_trash().unwrap().is_empty());
    assert!(matches!(v.delete_item(&[5; 16]), Err(VaultError::NotFound)));
}

#[test]
fn edit_after_delete_resurrects_and_edit_before_delete_does_not() {
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "Resurrect Me");
    let created = v.get_item(&l).unwrap().summary.updated;
    e.clock.advance(1_000);
    v.delete_item(&l).unwrap();
    assert!(titles(v).is_empty());
    let deleted_at =
        v.db.with_read(|tx| tx.get_item(&l))
            .unwrap()
            .unwrap()
            .deleted_hlc
            .unwrap();
    let deleted_hlc = Hlc::from_i64(deleted_at).unwrap();
    // An edit made on another device BEFORE the delete (older hlc) arrives late: stays deleted.
    remote(
        v,
        &l,
        "notes",
        rtext("old edit"),
        h(created.pt() + 1, 0),
        0xC3,
        None,
    );
    assert!(
        titles(v).is_empty(),
        "edit before delete must not resurrect"
    );
    assert_eq!(v.list_trash().unwrap().len(), 1);
    assert!(search(v, "resurrect").is_empty());
    // An edit AFTER the delete (the other device had not seen it) resurrects, deterministically.
    remote(
        v,
        &l,
        "notes",
        rtext("new edit"),
        h(deleted_hlc.pt(), deleted_hlc.counter() + 1),
        0xC3,
        Some(h(created.pt() + 1, 0)),
    );
    assert_eq!(titles(v), ["Resurrect Me"]);
    assert!(v.list_trash().unwrap().is_empty());
    assert_eq!(
        search(v, "resurrect"),
        ["Resurrect Me"],
        "the index follows visibility"
    );
    v.set_field(&l, StdField::Notes, "can edit again").unwrap();
}

#[test]
fn delete_bookkeeping_register_does_not_resurrect_its_own_item() {
    // `deleted_at` is written with the same HLC as `deleted`; it must not count as an edit after the delete.
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    v.delete_item(&l).unwrap();
    assert!(titles(v).is_empty());
    let regs = v.db.with_read(|tx| engine::load_regs(tx, &l)).unwrap();
    assert_eq!(
        engine::find(&regs, "deleted").unwrap().hlc,
        engine::find(&regs, "deleted_at").unwrap().hlc
    );
    assert!(!engine::is_visible(&regs));
}

#[test]
fn purge_expired_uses_the_30_day_retention_and_keeps_a_tombstone() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(
            NewItem::new(ItemType::Login, "Purge Me")
                .with_field(StdField::Password, "CANARY-pw-3")
                .with_field(StdField::Notes, "CANARY-note"),
        )
        .unwrap();
    v.set_field(&l, StdField::Password, "CANARY-pw-4").unwrap();
    let keep = login(v, "Stay");
    v.delete_item(&l).unwrap();
    e.clock.advance(29 * DAY + 23 * 3_600_000);
    assert_eq!(v.purge_expired().unwrap(), 0, "not yet 30 days");
    e.clock.advance(3_600_000 + 1);
    assert_eq!(v.purge_expired().unwrap(), 1);
    assert_eq!(v.purge_expired().unwrap(), 0, "idempotent");
    // Content gone, history gone, index gone; tombstone (id, deleted_hlc) and type remain.
    let t = v.list_trash().unwrap();
    assert_eq!(
        (t.len(), t[0].purged, t[0].summary.title.as_str()),
        (1, true, "")
    );
    assert!(v.reveal_key(&l, "password").unwrap().is_none());
    assert!(
        v.versions(&l, &FieldRef::Std(StdField::Password))
            .unwrap()
            .iter()
            .all(|x| x.cleared)
    );
    let d = dump(v, &["purge"], false);
    assert!(d.history.is_empty() || d.history.iter().all(|h| h.item_id != l));
    assert!(d.fts[0].1.is_empty());
    let row = d.items.iter().find(|i| i.id == l).unwrap();
    assert!(row.deleted && row.deleted_hlc.is_some(), "tombstone kept");
    assert!(
        d.fields
            .iter()
            .filter(|f| f.item_id == l)
            .all(|f| f.value.is_none()
                || matches!(
                    f.key.as_str(),
                    "deleted" | "deleted_at" | "type" | "schema_version" | "created_at"
                ))
    );
    assert_eq!(titles(v), ["Stay"]);
    assert!(v.get_item(&keep).is_ok());
    // A purged tombstone is still older than any late op, so a stale edit does not resurrect it.
    remote(v, &l, "notes", rtext("stale"), h(T0, 0), 0xC3, None);
    assert_eq!(titles(v), ["Stay"]);
    // The 180-day tombstone horizon is only *reported* (dropping needs compaction, M4).
    assert!(v.expired_tombstones().unwrap().is_empty());
    e.clock.advance(151 * DAY);
    assert_eq!(v.expired_tombstones().unwrap(), vec![l]);
}

#[test]
fn purge_item_now_and_restore_of_purged_item() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(
            NewItem::new(ItemType::Login, "Gone").with_field(StdField::Password, "CANARY-gone"),
        )
        .unwrap();
    v.delete_item(&l).unwrap();
    v.purge_item(&l).unwrap();
    assert!(v.list_trash().unwrap()[0].purged);
    assert!(v.reveal_key(&l, "password").unwrap().is_none());
    v.restore_item(&l).unwrap(); // restoring a purged item yields an empty shell, never old secrets
    assert!(v.reveal_key(&l, "password").unwrap().is_none());
    assert_eq!(v.get_item(&l).unwrap().summary.title, "");
}

// ------------------------------------------------------------------------- search

#[test]
fn search_typical_queries() {
    let mut e = env();
    let v = &mut e.v;
    let gh = v
        .create_item(NewItem {
            urls: vec!["https://github.com/login".into()],
            tags: vec!["dev".into()],
            ..NewItem::new(ItemType::Login, "GitHub personal")
                .with_field(StdField::Username, "alice.smith")
                .with_field(StdField::Notes, "Two-factor via authenticator")
        })
        .unwrap();
    let bank = v
        .create_item(
            NewItem::new(ItemType::Card, "Everyday Visa")
                .with_field(StdField::Holder, "Alice Smith"),
        )
        .unwrap();
    let note = v
        .create_item(NewItem::new(ItemType::Note, "Trip ideas").with_field(
            StdField::Body,
            "# Lisbon\nBook the tram and pastéis de nata",
        ))
        .unwrap();
    let id = v
        .create_item(
            NewItem::new(ItemType::Identity, "Passport")
                .with_field(StdField::FirstName, "Alice")
                .with_field(StdField::LastName, "Smith")
                .with_field(StdField::Email, "alice@example.test"),
        )
        .unwrap();
    assert_eq!(search(v, "github"), ["GitHub personal"]);
    assert_eq!(search(v, "gith"), ["GitHub personal"], "prefix matching");
    assert_eq!(
        search(v, "GITHUB.com"),
        ["GitHub personal"],
        "case-insensitive, url tokens"
    );
    assert_eq!(
        search(v, "alice smith").len(),
        3,
        "login username, card holder, identity name (all words must match)"
    );
    assert_eq!(
        search(v, "lisbon tram"),
        ["Trip ideas"],
        "note bodies are searchable (US-04)"
    );
    assert_eq!(
        search(v, "pasteis"),
        ["Trip ideas"],
        "diacritics are folded"
    );
    assert_eq!(search(v, "authenticator"), ["GitHub personal"]);
    assert_eq!(search(v, "dev"), ["GitHub personal"], "tags are indexed");
    assert_eq!(search(v, "alice@example"), ["Passport"]);
    assert!(search(v, "nothing-matches-this").is_empty());
    assert!(search(v, "   ").len() == 4, "no text = list");
    // Filters.
    let mut q = |text: &str, filter: ListFilter| {
        v.search(&SearchQuery {
            text: text.into(),
            filter,
            limit: 0,
        })
        .unwrap()
        .len()
    };
    assert_eq!(
        q(
            "alice",
            ListFilter {
                item_type: Some(ItemType::Card),
                ..Default::default()
            }
        ),
        1
    );
    assert_eq!(
        q(
            "alice",
            ListFilter {
                tag: Some("dev".into()),
                ..Default::default()
            }
        ),
        1
    );
    assert_eq!(
        q(
            "alice",
            ListFilter {
                tag: Some("other".into()),
                ..Default::default()
            }
        ),
        0
    );
    let folder = v.create_folder("F", None).unwrap();
    v.move_to_folder(&bank, Some(folder)).unwrap();
    let mut q = |text: &str, filter: ListFilter| {
        v.search(&SearchQuery {
            text: text.into(),
            filter,
            limit: 0,
        })
        .unwrap()
        .len()
    };
    assert_eq!(
        q(
            "alice",
            ListFilter {
                folder_id: Some(folder),
                ..Default::default()
            }
        ),
        1
    );
    let _ = (gh, note, id);
    assert_eq!(
        v.search(&SearchQuery {
            text: "alice".into(),
            limit: 1,
            ..Default::default()
        })
        .unwrap()
        .len(),
        1,
        "limit"
    );
}

#[test]
fn search_index_follows_every_mutation() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(NewItem::new(ItemType::Login, "Alpha Service"))
        .unwrap();
    assert_eq!(search(v, "alpha"), ["Alpha Service"]);
    v.set_field(&l, StdField::Title, "Bravo Service").unwrap();
    assert!(search(v, "alpha").is_empty(), "old title no longer matches");
    assert_eq!(search(v, "bravo"), ["Bravo Service"]);
    v.set_field(&l, StdField::Username, "charlie").unwrap();
    v.add_tag(&l, "delta").unwrap();
    let u = v.add_url(&l, "https://echo.test").unwrap();
    assert_eq!(
        (
            search(v, "charlie").len(),
            search(v, "delta").len(),
            search(v, "echo").len()
        ),
        (1, 1, 1)
    );
    v.remove_tag(&l, "delta").unwrap();
    v.remove_url(&l, &u).unwrap();
    v.clear_field(&l, StdField::Username).unwrap();
    assert_eq!(
        (
            search(v, "charlie").len(),
            search(v, "delta").len(),
            search(v, "echo").len()
        ),
        (0, 0, 0)
    );
    v.delete_item(&l).unwrap();
    assert!(
        search(v, "bravo").is_empty(),
        "trashed items are not searchable"
    );
    v.restore_item(&l).unwrap();
    assert_eq!(search(v, "bravo"), ["Bravo Service"]);
    // Repair path rebuilds the same index.
    let before = dump(v, &["bravo"], false);
    v.reindex_all().unwrap();
    assert_eq!(dump(v, &["bravo"], false), before);
}

#[test]
fn secrets_are_never_indexed() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(
            NewItem::new(ItemType::Login, "Login")
                .with_field(StdField::Password, "zzpasswordcanary")
                .with_field(StdField::TotpSeed, "zztotpcanary"),
        )
        .unwrap();
    let c = v
        .create_item(
            NewItem::new(ItemType::Card, "Card")
                .with_field(StdField::Number, "zznumbercanary")
                .with_field(StdField::Cvv, "zzcvvcanary")
                .with_field(StdField::Pin, "zzpincanary"),
        )
        .unwrap();
    let i = v
        .create_item(
            NewItem::new(ItemType::Identity, "Id").with_field(StdField::Ids, "zzidscanary"),
        )
        .unwrap();
    let cf = v
        .add_custom_field(&l, CustomKind::Hidden, "secret", "zzhiddencanary")
        .unwrap();
    v.set_field(&l, StdField::Password, "zzpasswordcanaryx")
        .unwrap();
    let _ = (c, i, cf);
    // Query the FTS index itself (not just the Vault API) for every secret token.
    for tok in [
        "zzpasswordcanary",
        "zzpasswordcanaryx",
        "zztotpcanary",
        "zznumbercanary",
        "zzcvvcanary",
        "zzpincanary",
        "zzidscanary",
        "zzhiddencanary",
        "zz",
    ] {
        let hits =
            v.db.with_read(|tx| tx.fts_search(&query::fts_query(tok).unwrap(), 10))
                .unwrap();
        assert!(hits.is_empty(), "{tok} leaked into the FTS index");
        // Also as a raw FTS5 prefix/phrase query, bypassing our query builder.
        for raw in [format!("{tok}*"), format!("\"{tok}\"")] {
            assert!(
                v.db.with_read(|tx| tx.fts_search(&raw, 10))
                    .unwrap()
                    .is_empty(),
                "{raw}"
            );
        }
    }
    // Sanity: the same machinery does find non-secret content.
    assert_eq!(search(v, "login").len(), 1);
}

#[test]
fn hostile_search_text_cannot_reach_fts_syntax_or_error() {
    let mut e = env();
    let v = &mut e.v;
    login(v, "plain");
    for q in [
        "\"",
        "\"unterminated",
        "a AND",
        "NEAR(a b)",
        "title:plain",
        "*",
        "plain*",
        "-plain",
        "(",
        ")",
        "\u{0}",
        "' OR 1=1 --",
        &"a ".repeat(500),
        &"x".repeat(10_000),
    ] {
        let r = v.search(&SearchQuery {
            text: q.into(),
            ..Default::default()
        });
        assert!(r.is_ok(), "{q:?}: {r:?}");
    }
    assert!(
        search(v, "title:plain").is_empty(),
        "the colon is a separator, so this is the two words `title` AND `plain`, not a column filter"
    );
    assert_eq!(search(v, "plain"), ["plain"]);
}

// ------------------------------------------------------------- secrets in output

#[test]
fn no_secret_in_views_debug_or_errors() {
    let mut e = env();
    let v = &mut e.v;
    let secret = "CANARY-SECRET-0000";
    let l = v
        .create_item(
            NewItem::new(ItemType::Login, "Visible title")
                .with_field(StdField::Password, secret)
                .with_field(StdField::TotpSeed, secret),
        )
        .unwrap();
    let n = v
        .create_item(NewItem::new(ItemType::Note, "N").with_field(StdField::Body, secret))
        .unwrap();
    v.add_custom_field(&l, CustomKind::Hidden, "h", secret)
        .unwrap();
    let mut seen = vec![
        format!("{:?}", v.get_item(&l).unwrap()),
        format!("{:?}", v.get_item(&n).unwrap()),
    ];
    seen.push(format!(
        "{:?}",
        v.list(&ListFilter::default(), Page::ALL).unwrap()
    ));
    seen.push(format!("{:?}", search(v, "visible")));
    seen.push(format!("{:?}", v.list_trash().unwrap()));
    seen.push(format!(
        "{:?}",
        v.versions(&l, &FieldRef::Std(StdField::Password)).unwrap()
    ));
    seen.push(format!(
        "{:?}",
        NewItem::new(ItemType::Login, "t").with_field(StdField::Password, secret)
    ));
    seen.push(format!("{v:?}"));
    for err in [
        v.get_text(&l, StdField::Password).unwrap_err(),
        v.reveal(&l, StdField::Title).unwrap_err(),
        v.set_field(&n, StdField::Password, secret).unwrap_err(),
        v.set_field(&l, StdField::Notes, &secret.repeat(10_000))
            .unwrap_err(),
    ] {
        seen.push(format!("{err} {err:?}"));
    }
    let regs = v.db.with_read(|tx| engine::load_regs(tx, &l)).unwrap();
    seen.push(format!("{regs:?}"));
    for s in seen {
        assert!(!s.contains("CANARY"), "secret leaked: {s}");
    }
}

// --------------------------------------------------------------------- health

#[test]
fn health_reused_weak_and_old() {
    let mut e = env();
    let v = &mut e.v;
    let shared = "CANARY-shared-Pass-77!";
    let a = v
        .create_item(NewItem::new(ItemType::Login, "a").with_field(StdField::Password, shared))
        .unwrap();
    let b = v
        .create_item(NewItem::new(ItemType::Login, "b").with_field(StdField::Password, shared))
        .unwrap();
    let c = v
        .create_item(
            NewItem::new(ItemType::Login, "c").with_field(StdField::Password, "password1234"),
        )
        .unwrap();
    let d = v
        .create_item(
            NewItem::new(ItemType::Login, "d")
                .with_field(StdField::Password, "CANARY-unique-xK9#vQ2$mL7&"),
        )
        .unwrap();
    let trashed = v
        .create_item(NewItem::new(ItemType::Login, "t").with_field(StdField::Password, shared))
        .unwrap();
    v.delete_item(&trashed).unwrap();
    v.create_item(NewItem::new(ItemType::Login, "empty"))
        .unwrap();
    let groups = v.reused_passwords().unwrap();
    assert_eq!(groups.len(), 1);
    let mut ids = groups[0].item_ids.clone();
    ids.sort();
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(ids, want, "trashed and empty-password items are ignored");
    assert!(!format!("{groups:?}").contains("CANARY"));
    let weak = v.weak_passwords(1).unwrap();
    assert_eq!(weak.iter().map(|w| w.item_id).collect::<Vec<_>>(), vec![c]);
    assert!(v.weak_passwords(4).unwrap().len() >= 3);
    assert!(!weak.iter().any(|w| w.item_id == d));
    e.clock.advance(400 * DAY);
    v.set_field(&d, StdField::Password, "CANARY-rotated-xK9#vQ2$mL7&")
        .unwrap();
    let old = v.old_passwords(365).unwrap();
    assert!(
        old.iter().all(|o| o.age_days >= 400)
            && old.len() == 3
            && !old.iter().any(|o| o.item_id == d)
    );
    assert!(v.old_passwords(0).unwrap().iter().any(|o| o.item_id == d));
}

// --------------------------------------------------- convergence on the database

type GenOp = (u8, u8, u64, u16, u8, Option<(u64, u16)>);
type RemoteOp = (String, Option<Zeroizing<Vec<u8>>>, Hlc, u8, Option<Hlc>);

fn arb_ops() -> impl Strategy<Value = Vec<GenOp>> {
    // (key index, value index, pt, counter, device, base)
    proptest::collection::vec(
        (
            0u8..8,
            0u8..4,
            1u64..7,
            0u16..2,
            1u8..4,
            proptest::option::of((1u64..7, 0u16..2)),
        ),
        1..16,
    )
}

const KEYS: [&str; 8] = [
    "title",
    "username",
    "notes",
    "password",
    "tags.red",
    "tags.blue",
    "deleted",
    "favorite",
];

fn to_remote(op: &(u8, u8, u64, u16, u8, Option<(u64, u16)>)) -> RemoteOp {
    let key = KEYS[op.0 as usize];
    let val = match key {
        "tags.red" | "tags.blue" | "deleted" | "favorite" => match op.1 {
            0 => None,
            1 => rflag(false),
            _ => rflag(true),
        },
        _ => match op.1 {
            0 => None,
            1 => rtext("alpha"),
            2 => rtext("beta"),
            _ => rtext("alpha beta"),
        },
    };
    (
        key.to_owned(),
        val,
        h(op.2, op.3),
        op.4,
        op.5.map(|(p, c)| h(p, c)),
    )
}

fn replica(ops: &[GenOp], order: &[usize], dups: &[usize]) -> Dump {
    let mut e = env_with(0xEE);
    let id = [0x77; 16];
    // Baseline: every replica knows the item's type (an op from some device).
    remote(&mut e.v, &id, "type", rtext("login"), h(1, 0), 9, None);
    for &i in order.iter().chain(dups) {
        let (k, v, hlc, dev, base) = to_remote(&ops[i]);
        remote(&mut e.v, &id, &k, v, hlc, dev, base);
    }
    dump(&mut e.v, &["alpha", "beta", "red", "blue"], false)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(60))]

    // SEC-Y04 / SEC-Y14 on the real storage path: any arrival order and duplication gives identical
    // registers, history, item cache, search index and (hence) concurrency view.
    #[test]
    fn database_state_is_independent_of_arrival_order(ops in arb_ops(), seed in any::<u64>(), dups in proptest::collection::vec(any::<prop::sample::Index>(), 0..6)) {
        let n = ops.len();
        let straight: Vec<usize> = (0..n).collect();
        let mut shuffled = straight.clone();
        let mut s = seed | 1;
        for i in (1..n).rev() {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            shuffled.swap(i, (s >> 33) as usize % (i + 1));
        }
        let dup_idx: Vec<usize> = dups.iter().map(|d| d.index(n)).collect();
        let a = replica(&ops, &straight, &[]);
        let b = replica(&ops, &shuffled, &dup_idx);
        prop_assert_eq!(a, b);
    }
}

#[test]
fn spurious_conflict_regression_through_the_database() {
    // B (device 2) edited on top of A (device 1). B arrives first, then A: no conflict either way.
    let (a_h, b_h) = (h(100, 0), h(200, 0));
    for a_first in [true, false] {
        let mut e = env();
        let v = &mut e.v;
        let l = login(v, "x");
        let steps: [(&str, Hlc, u8, Option<Hlc>); 2] =
            [("A", a_h, 1, None), ("B", b_h, 2, Some(a_h))];
        let order: Vec<_> = if a_first { vec![0, 1] } else { vec![1, 0] };
        for i in order {
            let (val, hlc, dev, base) = steps[i];
            remote(v, &l, "notes", rtext(val), hlc, dev, base);
        }
        let f = FieldRef::Std(StdField::Notes);
        assert_eq!(
            v.get_text(&l, StdField::Notes).unwrap().as_deref(),
            Some("B")
        );
        assert!(
            v.concurrent_versions(&l, &f).unwrap().is_empty(),
            "a_first={a_first}"
        );
    }
    // Genuinely concurrent: C also based on A.
    let mut e = env();
    let v = &mut e.v;
    let l = login(v, "x");
    for (val, hlc, dev, base) in [
        ("C", h(300, 0), 3, Some(a_h)),
        ("A", a_h, 1, None),
        ("B", b_h, 2, Some(a_h)),
    ] {
        remote(v, &l, "notes", rtext(val), hlc, dev, base);
    }
    let conc = v
        .concurrent_versions(&l, &FieldRef::Std(StdField::Notes))
        .unwrap();
    assert_eq!(conc.len(), 1);
    assert_eq!(conc[0].hlc, b_h);
    assert_eq!(
        &*v.reveal_version(
            &l,
            &FieldRef::Std(StdField::Notes),
            conc[0].hlc,
            &conc[0].device_id
        )
        .unwrap()
        .unwrap(),
        "B"
    );
    // Reading is pure: state is unchanged by asking.
    let before = dump(v, &[], true);
    let _ = v
        .concurrent_versions(&l, &FieldRef::Std(StdField::Notes))
        .unwrap();
    let _ = v.versions(&l, &FieldRef::Std(StdField::Notes)).unwrap();
    assert_eq!(dump(v, &[], true), before);
}

#[test]
fn large_result_sets_are_ordered_by_recency_and_small_ones_ranked() {
    let mut e = env();
    for i in 0..320u32 {
        e.clock.advance(1);
        let tag = if i.is_multiple_of(2) {
            vec!["even".to_owned()]
        } else {
            vec![]
        };
        e.v.create_item(NewItem {
            tags: tag,
            ..NewItem::new(ItemType::Login, &format!("common {i}"))
        })
        .unwrap();
    }
    let v = &mut e.v;
    // 320 matches (> 300): most recently changed first, limit respected.
    let top = v
        .search(&SearchQuery {
            text: "common".into(),
            limit: 5,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        top.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        [
            "common 319",
            "common 318",
            "common 317",
            "common 316",
            "common 315"
        ]
    );
    // Filters look past the first page of candidates.
    let even = v
        .search(&SearchQuery {
            text: "common".into(),
            filter: ListFilter {
                tag: Some("even".into()),
                ..Default::default()
            },
            limit: 3,
        })
        .unwrap();
    assert_eq!(
        even.iter().map(|s| s.title.as_str()).collect::<Vec<_>>(),
        ["common 318", "common 316", "common 314"]
    );
    // A narrow query (<= 300 matches) returns every match.
    assert_eq!(
        v.search(&SearchQuery {
            text: "common 31".into(),
            limit: 1000,
            ..Default::default()
        })
        .unwrap()
        .len(),
        11,
        "common 31 and common 310..319"
    );
    // Touching an old item moves it to the front.
    let old = v
        .search(&SearchQuery {
            text: "common 0".into(),
            limit: 1,
            ..Default::default()
        })
        .unwrap();
    let old_id = old
        .iter()
        .find(|s| s.title == "common 0")
        .map_or([0; 16], |s| s.id);
    v.set_field(&old_id, StdField::Notes, "bump").unwrap();
    let top = v
        .search(&SearchQuery {
            text: "common".into(),
            limit: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(top[0].title, "common 0");
}

// ---------------------------------------------------------------- atomicity

/// Run `action` with a fault injected at every possible point in turn. A failed
/// run must leave the database exactly as it was (no field, history, local_op,
/// FTS, item, folder or clock change); the run that finally succeeds must have
/// changed all of them together.
fn assert_atomic(
    name: &str,
    v: &mut Vault,
    probes: &[&str],
    mut action: impl FnMut(&mut Vault) -> Result<()>,
) {
    let before = dump(v, probes, true);
    let mut points = 0;
    for n in 0.. {
        fault::arm(n);
        let r = action(v);
        let passed = fault::disarm();
        match r {
            Err(VaultError::Injected) => {
                let after = dump(v, probes, true);
                assert_eq!(
                    after, before,
                    "{name}: failure at point {n} left partial state"
                );
            }
            Ok(()) => {
                points = n;
                assert!(passed <= n, "{name}: injected point {n} was never reached");
                break;
            }
            Err(other) => panic!("{name}: unexpected error {other:?} at point {n}"),
        }
        assert!(n < 200, "{name}: runaway");
    }
    assert!(
        points >= 2,
        "{name}: expected several fault points, saw {points}"
    );
    assert_ne!(
        dump(v, probes, true),
        before,
        "{name}: success changed nothing"
    );
}

#[test]
fn every_mutation_is_all_or_nothing() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(NewItem {
            urls: vec!["https://u.test".into()],
            tags: vec!["t".into()],
            ..NewItem::new(ItemType::Login, "Atomic Alpha").with_field(StdField::Password, "p0")
        })
        .unwrap();
    v.set_field(&l, StdField::Password, "p1").unwrap(); // history exists
    let f = v.create_folder("F", None).unwrap();
    let cid = v.add_custom_field(&l, CustomKind::Text, "L", "V").unwrap();
    let probes = ["atomic", "beta", "gamma", "tag2"];
    assert_atomic("create_item", v, &probes, |v| {
        v.create_item(NewItem {
            tags: vec!["tag2".into()],
            ..NewItem::new(ItemType::Login, "Gamma").with_field(StdField::Password, "x")
        })
        .map(|_| ())
    });
    assert_atomic(
        "set_field (title: register+history+fts+op)",
        v,
        &probes,
        |v| v.set_field(&l, StdField::Title, "Atomic Beta"),
    );
    assert_atomic("set_field (password: history rewrite)", v, &probes, |v| {
        v.set_field(
            &l,
            StdField::Password,
            &format!("pw{}", v.clock.last().to_i64()),
        )
    });
    assert_atomic("add_tag", v, &probes, |v| v.add_tag(&l, "tag2"));
    assert_atomic("move_to_folder", v, &probes, |v| {
        v.move_to_folder(&l, Some(f))
    });
    assert_atomic("toggle_favorite", v, &probes, |v| {
        v.toggle_favorite(&l).map(|_| ())
    });
    assert_atomic("set_custom_value", v, &probes, |v| {
        v.set_custom_value(&l, &cid, &format!("v{}", v.clock.last().to_i64()))
    });
    assert_atomic("create_folder", v, &probes, |v| {
        v.create_folder("G", Some(f)).map(|_| ())
    });
    assert_atomic("rename_folder", v, &probes, |v| {
        v.rename_folder(&f, &format!("F{}", v.clock.last().to_i64()))
    });
    assert_atomic("delete_item", v, &probes, |v| v.delete_item(&l));
    assert_atomic("restore_item", v, &probes, |v| v.restore_item(&l));
    v.delete_item(&l).unwrap();
    assert_atomic("purge_item", v, &probes, |v| v.purge_item(&l));
}

#[test]
fn successful_mutation_writes_register_history_op_index_and_clock_together() {
    let mut e = env();
    let v = &mut e.v;
    let l = v
        .create_item(NewItem::new(ItemType::Login, "Together").with_field(StdField::Password, "p0"))
        .unwrap();
    let before = dump(v, &["together", "renamed"], true);
    v.set_field(&l, StdField::Title, "Renamed").unwrap();
    v.set_field(&l, StdField::Password, "p1").unwrap();
    let after = dump(v, &["together", "renamed"], true);
    assert_eq!(after.local_ops.len(), before.local_ops.len() + 2);
    assert_eq!(
        after.history.len(),
        before.history.len() + 2,
        "previous title and password pushed to history"
    );
    assert_eq!(after.fts[0].1.len(), 0);
    assert_eq!(after.fts[1].1, vec![l]);
    assert_ne!(after.hlc_state, before.hlc_state);
    assert_eq!(
        Hlc::from_i64(i64::from_le_bytes(
            after.hlc_state.unwrap().try_into().unwrap()
        ))
        .unwrap(),
        v.clock.last()
    );
}

// ------------------------------------------------------------ performance (ignored)

/// `cargo test -p arya-vault-vault --release -- --ignored --nocapture perf`
#[test]
#[ignore = "20,000-item benchmark; run in release mode"]
fn perf_20k_items_search_and_list() {
    use std::time::Instant;
    let mut e = env();
    let words = [
        "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel", "india", "juliet",
    ];
    let started = Instant::now();
    let Vault {
        db,
        device_id,
        cfg,
        clock,
        ..
    } = &mut e.v;
    db.with_tx(|tx| -> Result<()> {
        for n in 0..20_000u64 {
            let mut id = [0u8; 16];
            id[6] = 0x70;
            id[8..].copy_from_slice(&n.to_be_bytes());
            let hlc = clock.now()?;
            let w = |i: u64| words[((n / i) % 10) as usize];
            let kv = [
                ("type", value::encode_text("login")),
                (
                    "title",
                    value::encode_text(&format!("{} {} site {n}", w(1), w(10))),
                ),
                (
                    "username",
                    value::encode_text(&format!("user{n}@{}.test", w(100))),
                ),
                ("password", value::encode_text(&format!("CANARY-pw-{n}"))),
                (
                    "notes",
                    value::encode_text(&format!("note about {} and {}", w(1000), w(7))),
                ),
                (
                    "urls.00000000000000000000000000000001",
                    value::encode_text(&format!("https://{}.example/{n}", w(3))),
                ),
            ];
            let before = Vec::new();
            for (k, val) in kv {
                engine::apply_op(
                    tx,
                    &Op {
                        item_id: id,
                        key: k.into(),
                        value: Some(val),
                        hlc,
                        device_id: *device_id,
                        base_hlc: None,
                    },
                    cfg,
                )?;
            }
            engine::refresh_item(tx, &id, &before)?;
        }
        Ok(())
    })
    .unwrap();
    eprintln!("setup of 20,000 items: {:?}", started.elapsed());
    let v = &mut e.v;
    let time = |label: &str, f: &mut dyn FnMut()| {
        f(); // warm up
        let t = Instant::now();
        for _ in 0..5 {
            f();
        }
        let per = t.elapsed() / 5;
        eprintln!("{label}: {per:?}");
        per
    };
    let s_prefix = time("search 'alph' (prefix, 100 results)", &mut || {
        assert_eq!(
            v.search(&SearchQuery {
                text: "alph".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
            100
        )
    });
    let s_multi = time("search 'bravo site' (two words)", &mut || {
        assert!(
            !v.search(&SearchQuery {
                text: "bravo site".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
        )
    });
    let s_rare = time("search 'user19999' (single hit)", &mut || {
        assert_eq!(
            v.search(&SearchQuery {
                text: "user19999".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
            1
        )
    });
    let l_all = time("list all 20,000 summaries", &mut || {
        assert_eq!(
            v.list(&ListFilter::default(), Page::ALL).unwrap().len(),
            20_000
        )
    });
    let l_page = time("list first page of 50", &mut || {
        assert_eq!(
            v.list(
                &ListFilter::default(),
                Page {
                    offset: 0,
                    limit: 50
                }
            )
            .unwrap()
            .len(),
            50
        )
    });
    let l_type = time("list type=login favorites_only", &mut || {
        assert!(
            v.list(
                &ListFilter {
                    favorites_only: true,
                    ..Default::default()
                },
                Page::ALL
            )
            .unwrap()
            .is_empty()
        )
    });
    // Targets (docs: search < 100 ms, list < 50 ms at 20,000 items). "list" is the page a UI shows;
    // the unpaged numbers are informational: they decrypt the whole `field` table twice (no index on
    // `field(key)`; a storage migration would be needed to change that, see the PR).
    for (label, d, target_ms) in [
        ("search prefix", s_prefix, 100),
        ("search multi", s_multi, 100),
        ("search rare", s_rare, 100),
        ("list first page", l_page, 50),
    ] {
        assert!(
            d.as_millis() < target_ms,
            "{label} took {d:?} (target < {target_ms} ms)"
        );
    }
    eprintln!("(informational, unpaged) list all: {l_all:?}, list favorites_only: {l_type:?}");
}
