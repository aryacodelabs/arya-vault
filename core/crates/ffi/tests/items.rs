//! docs/14 §4.2-§4.6 through the `api` module: items, folders, history, search, trash, generator,
//! health, import/export, settings, diagnostics.

mod common;

use std::collections::HashMap;

use arya_vault_ffi::api::dto::*;
use arya_vault_ffi::api::{diagnostics, items, lifecycle, settings, tools, transfer};
use common::*;

fn page(limit: u32) -> Page {
    Page { offset: 0, limit }
}

fn login(title: &str, user: &str, password: &str) -> NewItem {
    NewItem {
        item_type: ItemType::Login,
        title: title.into(),
        fields: HashMap::from([
            (StdField::Username, user.to_owned()),
            (StdField::Password, password.to_owned()),
        ]),
        urls: vec!["https://example.test/login".into()],
        tags: vec!["work".into()],
        folder_id: None,
        custom: vec![
            NewCustom {
                kind: CustomKind::Hidden,
                label: "pin".into(),
                value: "CANARY-custom-hidden".into(),
            },
            NewCustom {
                kind: CustomKind::Text,
                label: "site".into(),
                value: "visible".into(),
            },
        ],
    }
}

fn titles(v: &[ItemSummary]) -> Vec<&str> {
    v.iter().map(|s| s.title.as_str()).collect()
}

#[test]
fn create_get_reveal_edit_and_the_view_never_holds_a_secret() {
    let _g = serial();
    let _f = unlocked();
    let id = items::create_item(login("GitHub", "octo", "CANARY-pw-1")).unwrap();
    assert_eq!(id.len(), 32);

    let v = items::get_item(id.clone()).unwrap();
    assert_eq!(v.title, "GitHub");
    assert_eq!(v.fields.get(&StdField::Username), Some(&Some("octo".to_owned())));
    assert!(!v.fields.contains_key(&StdField::Password));
    assert!(v.secret_fields_present.contains(&StdField::Password));
    assert_eq!(v.tags, vec!["work"]);
    assert_eq!(v.urls.len(), 1);
    assert_eq!(v.custom.len(), 2);
    let hidden = v.custom.iter().find(|c| c.kind == CustomKind::Hidden).unwrap();
    assert_eq!(hidden.value_if_not_hidden, None);
    let text = v.custom.iter().find(|c| c.kind == CustomKind::Text).unwrap();
    assert_eq!(text.value_if_not_hidden.as_deref(), Some("visible"));
    assert!(!v.other_versions);
    assert!(!format!("{v:?}").contains("CANARY"));

    assert_eq!(
        items::reveal(id.clone(), StdField::Password).unwrap().unwrap(),
        b"CANARY-pw-1"
    );
    assert_eq!(
        items::reveal_custom(id.clone(), hidden.id.clone()).unwrap().unwrap(),
        b"CANARY-custom-hidden"
    );
    // A non-secret field is not revealed: read it from the view.
    assert_eq!(
        items::reveal(id.clone(), StdField::Username).unwrap_err().code,
        AppErrorCode::Validation
    );

    items::set_field(id.clone(), StdField::Password, "CANARY-pw-2".into()).unwrap();
    items::set_fields(
        id.clone(),
        HashMap::from([
            (StdField::Username, "octocat".to_owned()),
            (StdField::Notes, "n".to_owned()),
        ]),
    )
    .unwrap();
    // setFields is all-or-nothing: a field of another type rejects the whole batch.
    let e = items::set_fields(
        id.clone(),
        HashMap::from([
            (StdField::Username, "changed?".to_owned()),
            (StdField::CardNumber, "4111".to_owned()),
        ]),
    )
    .unwrap_err();
    assert_eq!(e.code, AppErrorCode::Validation);
    let v = items::get_item(id.clone()).unwrap();
    assert_eq!(v.fields.get(&StdField::Username), Some(&Some("octocat".to_owned())));

    items::clear_field(id.clone(), StdField::Notes).unwrap();
    assert!(!items::get_item(id.clone()).unwrap().fields.contains_key(&StdField::Notes));

    // Element-level edits.
    let url = items::add_url(id.clone(), "https://two.test".into()).unwrap();
    items::set_url(id.clone(), url.clone(), "https://three.test".into()).unwrap();
    items::remove_url(id.clone(), url).unwrap();
    items::add_tag(id.clone(), "x".into()).unwrap();
    items::remove_tag(id.clone(), "x".into()).unwrap();
    let c = items::add_custom_field(id.clone(), CustomKind::Hidden, "k".into(), "CANARY-v".into())
        .unwrap();
    items::set_custom_value(id.clone(), c.clone(), "CANARY-v2".into()).unwrap();
    items::set_custom_label(id.clone(), c.clone(), "k2".into()).unwrap();
    items::remove_custom_field(id.clone(), c).unwrap();
    assert!(items::toggle_favorite(id.clone()).unwrap());
    assert!(items::get_item(id).unwrap().favorite);
}

#[test]
fn errors_for_unknown_ids_and_bad_input() {
    let _g = serial();
    let _f = unlocked();
    let unknown = "0".repeat(32);
    assert_eq!(items::get_item(unknown).unwrap_err().code, AppErrorCode::NotFound);
    for bad in ["", "xyz", &"A".repeat(32), &"0".repeat(31)] {
        let e = items::get_item(bad.to_owned()).unwrap_err();
        assert_eq!(e.code, AppErrorCode::Validation, "{bad:?}");
        assert_eq!(e.field.as_deref(), Some("id"));
    }
    let e = items::list(ListFilter::default(), page(201)).unwrap_err();
    assert_eq!((e.code, e.field.as_deref()), (AppErrorCode::Validation, Some("limit")));
    let id = items::create_item(login("a", "u", "CANARY-p")).unwrap();
    let e = items::set_field(id.clone(), StdField::CardNumber, "x".into()).unwrap_err();
    assert_eq!(e.code, AppErrorCode::Validation);
    assert!(e.field.is_some());
    let e = items::set_field(id, StdField::Notes, "x".repeat(70_000)).unwrap_err();
    assert_eq!(e.code, AppErrorCode::LimitReached);
}

#[test]
fn list_search_filters_and_summaries_are_secret_free() {
    let _g = serial();
    let _f = unlocked();
    let a = items::create_item(login("GitHub", "octo", "CANARY-p1")).unwrap();
    let b = items::create_item(NewItem {
        item_type: ItemType::Note,
        title: "Diary".into(),
        fields: HashMap::from([(StdField::Body, "CANARY-secret-body".to_owned())]),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![],
    })
    .unwrap();
    let c = items::create_item(NewItem {
        item_type: ItemType::Card,
        title: "Visa".into(),
        fields: HashMap::from([
            (StdField::CardHolder, "A. Holder".to_owned()),
            (StdField::CardNumber, "CANARY-4111".to_owned()),
            (StdField::CardCvv, "CANARY-123".to_owned()),
        ]),
        urls: vec![],
        tags: vec!["money".into()],
        folder_id: None,
        custom: vec![],
    })
    .unwrap();
    items::toggle_favorite(a.clone()).unwrap();

    let all = items::list(ListFilter::default(), page(50)).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(items::item_count().unwrap(), 3);
    let by_id: HashMap<_, _> = all.iter().map(|s| (s.id.clone(), s)).collect();
    assert_eq!(by_id[&a].subtitle, "octo");
    assert_eq!(by_id[&c].subtitle, "A. Holder");
    assert_eq!(by_id[&b].subtitle, ""); // a note's first line would be body text
    assert!(by_id[&a].favorite && !by_id[&b].favorite);
    assert_eq!(by_id[&c].tags, vec!["money"]);
    assert!(!by_id[&a].has_totp);
    // No secret in any summary, even rendered with Debug.
    let rendered = format!("{all:?}");
    assert!(!rendered.contains("CANARY"), "{rendered}");

    // Filters: types (a list), tag, favorites, paging.
    let f = |types: Option<Vec<ItemType>>, tag: Option<&str>, fav: bool| ListFilter {
        types,
        tag: tag.map(str::to_owned),
        favorites_only: fav,
        ..Default::default()
    };
    let two = items::list(f(Some(vec![ItemType::Login, ItemType::Card]), None, false), page(50))
        .unwrap();
    assert_eq!(two.len(), 2);
    assert!(two.iter().all(|s| s.item_type != ItemType::Note));
    assert_eq!(
        titles(&items::list(f(None, Some("money"), false), page(50)).unwrap()),
        ["Visa"]
    );
    assert_eq!(
        titles(&items::list(f(None, None, true), page(50)).unwrap()),
        ["GitHub"]
    );
    assert!(items::list(f(Some(vec![]), None, false), page(50)).unwrap().is_empty());
    let p1 = items::list(ListFilter::default(), Page { offset: 1, limit: 1 }).unwrap();
    assert_eq!(p1.len(), 1);
    assert_eq!(p1[0].id, all[1].id);
    let merged =
        items::list(f(Some(vec![ItemType::Login, ItemType::Note, ItemType::Card]), None, false), Page { offset: 1, limit: 1 })
            .unwrap();
    assert_eq!(merged[0].id, all[1].id);

    // Search: prefix; the body is searchable but never returned; passwords are not indexed.
    let q = |text: &str| SearchQuery {
        text: text.into(),
        filter: ListFilter::default(),
        page: page(50),
    };
    assert_eq!(titles(&items::search(q("gith")).unwrap()), ["GitHub"]);
    assert_eq!(titles(&items::search(q("CANARY-p1")).unwrap()), Vec::<&str>::new());
    assert_eq!(items::search(q("")).unwrap().len(), 3);
    assert!(!format!("{:?}", items::search(q("diary")).unwrap()).contains("CANARY"));
}

#[test]
fn trash_restore_purge_and_history() {
    let _g = serial();
    let _f = unlocked();
    let id = items::create_item(login("T", "u", "CANARY-v1")).unwrap();
    for v in ["CANARY-v2", "CANARY-v3"] {
        items::set_field(id.clone(), StdField::Password, v.into()).unwrap();
    }
    let h = items::history(id.clone(), StdField::Password).unwrap();
    assert_eq!(h.len(), 3);
    assert!(h[0].is_current && !h[1].is_current);
    let oldest = h.last().unwrap();
    assert_eq!(
        items::reveal_version(id.clone(), StdField::Password, oldest.hlc_ms)
            .unwrap()
            .unwrap(),
        b"CANARY-v1"
    );
    items::restore_version(id.clone(), StdField::Password, oldest.hlc_ms).unwrap();
    assert_eq!(items::reveal(id.clone(), StdField::Password).unwrap().unwrap(), b"CANARY-v1");
    assert_eq!(
        items::reveal_version(id.clone(), StdField::Password, 12345).unwrap_err().code,
        AppErrorCode::NotFound
    );
    assert!(!format!("{h:?}").contains("CANARY"));

    items::delete_item(id.clone()).unwrap();
    assert_eq!(items::item_count().unwrap(), 0);
    let trash = items::list_trash().unwrap();
    assert_eq!(trash.len(), 1);
    assert!(trash[0].summary.deleted && trash[0].purges_at > trash[0].deleted_at);
    let with_trash = items::list(
        ListFilter {
            include_trash: true,
            ..Default::default()
        },
        page(50),
    )
    .unwrap();
    assert_eq!(with_trash.len(), 1);
    assert!(with_trash[0].deleted);
    items::restore_item(id.clone()).unwrap();
    assert_eq!(items::item_count().unwrap(), 1);
    items::delete_item(id.clone()).unwrap();
    assert_eq!(items::empty_trash().unwrap(), 1);
    assert!(items::list_trash().unwrap().is_empty());
    assert_eq!(items::purge_expired().unwrap(), 0);
}

#[test]
fn folders() {
    let _g = serial();
    let _f = unlocked();
    let root = items::create_folder("Work".into(), None).unwrap();
    let sub = items::create_folder("Sub".into(), Some(root.clone())).unwrap();
    items::rename_folder(sub.clone(), "Sub2".into()).unwrap();
    let id = items::create_item(NewItem {
        folder_id: Some(root.clone()),
        ..login("in folder", "u", "CANARY-p")
    })
    .unwrap();
    assert_eq!(items::get_item(id.clone()).unwrap().folder_id.as_deref(), Some(root.as_str()));
    let list = items::list_folders().unwrap();
    assert_eq!(list.len(), 2);
    assert!(list.iter().any(|f| f.name == "Sub2" && f.parent_id.as_deref() == Some(&root)));
    let filter = ListFilter {
        folder_id: Some(root.clone()),
        ..Default::default()
    };
    assert_eq!(items::list(filter, page(50)).unwrap().len(), 1);
    items::move_to_folder(id.clone(), None).unwrap();
    assert!(items::get_item(id).unwrap().folder_id.is_none());
    items::delete_folder(sub).unwrap();
    assert_eq!(items::list_folders().unwrap().len(), 1);
    assert_eq!(
        items::create_folder("x".into(), Some("1".repeat(32))).unwrap_err().code,
        AppErrorCode::NotFound
    );
}

#[test]
fn generator_strength_policy_and_health() {
    let _g = serial();
    // No unlock needed for the generator and the policy check (onboarding).
    let _dir = empty();
    let opts = PasswordOptions {
        length: 24,
        lower: true,
        upper: true,
        digits: true,
        symbols: true,
        symbol_set: "!@#".into(),
        exclude_ambiguous: true,
        require_each_class: true,
    };
    let a = tools::generate_password(opts.clone()).unwrap();
    let b = tools::generate_password(opts.clone()).unwrap();
    assert_eq!(a.len(), 24);
    assert_ne!(a, b);
    let bad = PasswordOptions {
        length: 2,
        ..opts.clone()
    };
    assert_eq!(tools::generate_password(bad).unwrap_err().code, AppErrorCode::Validation);
    let words = PassphraseOptions {
        word_count: 5,
        separator: "-".into(),
        capitalize: false,
        number_suffix: false,
    };
    let p = String::from_utf8(tools::generate_passphrase(words.clone()).unwrap()).unwrap();
    assert_eq!(p.split('-').count(), 5);
    assert!(tools::entropy_bits(EntropyOptions::Password(opts)).unwrap() > 100.0);
    assert!(tools::entropy_bits(EntropyOptions::Passphrase(words)).unwrap() > 50.0);
    assert!(tools::strength(pw("password123")).unwrap().score <= 1);
    assert!(tools::strength(a).unwrap().score >= 3);
    assert!(!tools::check_master_password(pw("short")).unwrap().acceptable);
    let ok = tools::check_master_password(pw(PASSWORD)).unwrap();
    assert!(ok.acceptable && ok.reasons.is_empty());
    assert_eq!(tools::health_report().unwrap_err().code, AppErrorCode::Locked);
}

#[test]
fn health_report_finds_reuse_weak_and_nothing_else() {
    let _g = serial();
    let _f = unlocked();
    let a = items::create_item(login("a", "u", "CANARY-shared-password-x9!")).unwrap();
    let b = items::create_item(login("b", "u", "CANARY-shared-password-x9!")).unwrap();
    let w = items::create_item(login("w", "u", "password123")).unwrap();
    let r = tools::health_report().unwrap();
    assert_eq!(r.reused.len(), 1);
    let mut ids = r.reused[0].item_ids.clone();
    ids.sort();
    let mut want = vec![a, b];
    want.sort();
    assert_eq!(ids, want);
    assert!(r.weak.iter().any(|x| x.item_id == w));
    assert!(r.old.is_empty());
    assert!(!format!("{r:?}").contains("CANARY"));
}

const CSV: &str = "name,url,username,password\nGit,https://git.test,u1,CANARY-csv-pw\nMail,https://mail.test,u2,CANARY-csv-pw2\n";

#[test]
fn import_preview_commit_and_exports() {
    let _g = serial();
    let _f = unlocked();
    items::create_item(NewItem {
        urls: vec!["https://git.test".into()],
        ..login("Git", "u1", "x")
    })
    .unwrap();
    let p = transfer::import_preview(ImportFormat::Csv, CSV.as_bytes().to_vec()).unwrap();
    assert_eq!(p.format, "chromeCsv"); // name,url,username,password is the Chrome shape
    assert_eq!(p.item_count, 1); // the duplicate login is skipped
    assert_eq!(p.duplicates, 1);
    assert_eq!(items::item_count().unwrap(), 1); // preview mutated nothing
    // A wrong token is refused and does not consume the preview.
    let opts = ImportOptions {
        skip_duplicates: true,
        target_folder: None,
    };
    assert_eq!(
        transfer::import_commit("00".repeat(16), opts.clone()).unwrap_err().field.as_deref(),
        Some("previewToken")
    );
    let r = transfer::import_commit(p.preview_token.clone(), opts.clone()).unwrap();
    assert_eq!((r.created, r.duplicates), (1, 1));
    assert_eq!(items::item_count().unwrap(), 2);
    // The token is single-use.
    assert!(transfer::import_commit(p.preview_token, opts).is_err());

    // Garbage is a typed error, not a panic; limits map to limitReached.
    let e = transfer::import_preview(ImportFormat::BitwardenJson, b"not json".to_vec()).unwrap_err();
    assert_eq!((e.code, e.field.as_deref()), (AppErrorCode::Validation, Some("file")));

    // CSV export needs the acknowledgement.
    assert_eq!(
        transfer::export_csv(AcknowledgePlaintextRisk { acknowledged: false }).unwrap_err().code,
        AppErrorCode::Validation
    );
    let csv = transfer::export_csv(AcknowledgePlaintextRisk { acknowledged: true }).unwrap();
    assert!(contains(&csv, b"CANARY-csv-pw"));

    // Encrypted export round trip into a fresh vault.
    let enc = transfer::export_encrypted(pw("CANARY-export-password-1")).unwrap();
    assert!(!contains(&enc, b"CANARY-csv-pw"));
    assert_eq!(
        transfer::export_encrypted(vec![]).unwrap_err().code,
        AppErrorCode::Validation
    );
    lifecycle::lock().unwrap();
    let _dir = empty();
    let k = lifecycle::create_vault(pw(PASSWORD), KdfProfile::Low).unwrap();
    assert!(lifecycle::confirm_recovery_key(answers(&k)).unwrap());
    assert_eq!(
        transfer::import_encrypted(enc.clone(), pw("CANARY-wrong")).unwrap_err().code,
        AppErrorCode::WrongCredentials
    );
    let r = transfer::import_encrypted(enc, pw("CANARY-export-password-1")).unwrap();
    assert_eq!(r.created, 2);
    assert_eq!(items::item_count().unwrap(), 2);
}

#[test]
fn settings_are_clamped_persisted_and_apply_retention() {
    let _g = serial();
    let _f = unlocked();
    let d = settings::get_settings().unwrap();
    assert_eq!((d.auto_lock_minutes, d.clipboard_clear_seconds, d.reveal_hide_seconds), (5, 30, 15));
    let hostile = AppSettings {
        auto_lock_minutes: 0,
        clipboard_clear_seconds: 100_000,
        reveal_hide_seconds: 3_600,
        lock_on_sleep: false,
        block_screen_capture: false,
        vault_config: VaultConfig {
            trash_days: 0,
            ..d.vault_config
        },
        ..d
    };
    settings::set_settings(hostile).unwrap();
    let s = settings::get_settings().unwrap();
    assert_eq!(
        (s.auto_lock_minutes, s.clipboard_clear_seconds, s.reveal_hide_seconds, s.vault_config.trash_days),
        (1, 120, 15, 1)
    );
    assert!(!s.lock_on_sleep && !s.block_screen_capture && s.lock_on_screen_lock);
    // Persisted in the vault: survives lock/unlock.
    lifecycle::lock().unwrap();
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    assert_eq!(settings::get_settings().unwrap(), s);
    lifecycle::lock().unwrap();
    assert_eq!(settings::get_settings().unwrap_err().code, AppErrorCode::Locked);
}

#[test]
fn info_and_diagnostics_have_the_version_and_no_ids() {
    let _g = serial();
    let _f = fresh();
    let locked = diagnostics::info().unwrap();
    assert_eq!(locked.api_version, arya_vault_ffi::API_VERSION);
    assert_eq!(locked.format_version, 1);
    assert_eq!(locked.schema_version, 0);
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    let info = diagnostics::info().unwrap();
    assert!(info.schema_version > 0);
    assert!(info.sqlcipher_settings.iter().any(|s| s.key == "cipher.page_size"));
    let json = String::from_utf8(diagnostics::export_diagnostics().unwrap()).unwrap();
    assert!(json.starts_with("{\"apiVersion\":"));
    assert!(json.contains("\"header\":{"));
    assert!(!json.contains("CANARY"));
}
