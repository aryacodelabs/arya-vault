//! Secret-leak tests: canary secrets pushed through the error paths and the DTOs must never show
//! up in an error message, a `Debug` rendering, or (SEC-S02) a file on disk.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::collections::HashMap;

use arya_vault_ffi::AppError;
use arya_vault_ffi::api::dto::*;
use arya_vault_ffi::api::{items, lifecycle, tools, transfer};
use common::*;

const C: &str = "CANARY-LEAK-9f3a";

fn no_canary(e: &AppError) {
    let all = format!("{e:?} | {e} | {} | {:?}", e.message, e.field);
    assert!(!all.contains("CANARY"), "leaked: {all}");
}

#[test]
fn errors_never_echo_what_the_caller_sent() {
    let _g = serial();
    let _f = unlocked();
    let id = items::create_item(NewItem {
        item_type: ItemType::Login,
        title: format!("{C}-title"),
        fields: HashMap::from([(StdField::Password, format!("{C}-pw"))]),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![],
    })
    .unwrap();

    let mut errors: Vec<AppError> = Vec::new();
    let mut push = |r: Result<(), AppError>| {
        errors.push(r.expect_err("this call was supposed to fail"));
    };

    // Every credential and input path with a canary in the input.
    push(lifecycle::unlock(pw(&format!("{C}-pw"))));
    push(lifecycle::change_password(
        pw(&format!("{C}-old")),
        pw(&format!("{C}-new")),
        false,
    ));
    push(lifecycle::change_password(
        pw(PASSWORD),
        pw("CANARY-w"),
        false,
    ));
    push(lifecycle::recover_with_key(
        pw(&format!("{C}-key")),
        pw(OTHER_PASSWORD),
    ));
    push(lifecycle::create_vault(pw(&format!("{C}-x")), KdfProfile::Low).map(|_| ()));
    push(lifecycle::create_vault(vec![0xff, b'C', b'A', b'N'], KdfProfile::Low).map(|_| ()));
    push(lifecycle::regenerate_recovery_key(pw(&format!("{C}-p")), false).map(|_| ()));
    assert!(!lifecycle::verify_password(pw(&format!("{C}-p"))).unwrap());
    push(
        lifecycle::confirm_recovery_key(vec![GroupAnswer {
            index: 99,
            text: format!("{C}-g"),
        }])
        .map(|_| ()),
    );
    push(items::set_field(
        id.clone(),
        StdField::CardNumber,
        format!("{C}-v"),
    ));
    push(items::set_field(
        id.clone(),
        StdField::Notes,
        format!("{C}{}", "x".repeat(70_000)),
    ));
    push(items::add_tag(id.clone(), format!("{C}\n")));
    push(items::add_url(format!("{C}-id"), "https://x".into()).map(|_| ()));
    push(items::set_custom_value(
        id.clone(),
        format!("{C}-eid"),
        format!("{C}-v"),
    ));
    push(items::reveal(id.clone(), StdField::Username).map(|_| ()));
    push(items::reveal_version(id.clone(), StdField::Password, -1).map(|_| ()));
    push(items::create_folder(format!("{C}\u{7}"), None).map(|_| ()));
    push(
        transfer::import_preview(
            ImportFormat::Csv,
            format!("{C}\u{0}\u{0}garbage").into_bytes(),
        )
        .map(|_| ()),
    );
    push(
        transfer::import_preview(
            ImportFormat::BitwardenJson,
            format!("{{\"{C}\": ").into_bytes(),
        )
        .map(|_| ()),
    );
    push(transfer::import_encrypted(format!("{C}-file").into_bytes(), pw(C)).map(|_| ()));
    push(
        tools::generate_password(PasswordOptions {
            length: 1,
            lower: true,
            upper: false,
            digits: false,
            symbols: true,
            symbol_set: C.into(),
            exclude_ambiguous: false,
            require_each_class: true,
        })
        .map(|_| ()),
    );
    push(tools::strength(vec![0xff, 0xfe]).map(|_| ()));
    for e in &errors {
        no_canary(e);
    }
    assert!(errors.len() >= 20);

    // Locked: the same inputs after lock.
    lifecycle::lock().unwrap();
    for r in [
        items::set_field(id.clone(), StdField::Password, format!("{C}-v")),
        items::add_tag(id, format!("{C}-t")),
    ] {
        let e = r.unwrap_err();
        assert_eq!(e.code, AppErrorCode::Locked);
        no_canary(&e);
    }
}

#[test]
fn debug_output_of_secret_carrying_types_is_redacted() {
    let item = NewItem {
        item_type: ItemType::Login,
        title: "t".into(),
        fields: HashMap::from([(StdField::Password, format!("{C}-pw"))]),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![NewCustom {
            kind: CustomKind::Hidden,
            label: "l".into(),
            value: format!("{C}-custom"),
        }],
    };
    let rk = RecoveryKeyResult {
        recovery_key: format!("{C}-key").into_bytes(),
        groups: 8,
        challenge: vec![0, 1, 2],
    };
    let ga = GroupAnswer {
        index: 0,
        text: format!("{C}-g"),
    };
    for s in [
        format!("{item:?}"),
        format!("{rk:?}"),
        format!("{ga:?}"),
        format!("{:#?}", item.custom),
    ] {
        assert!(!s.contains("CANARY"), "{s}");
    }
}

#[test]
fn sec_s02_nothing_secret_is_on_disk_after_a_session() {
    let _g = serial();
    let f = unlocked();
    let id = items::create_item(NewItem {
        item_type: ItemType::Login,
        title: "disk".into(),
        fields: HashMap::from([
            (StdField::Password, format!("{C}-on-disk-pw")),
            (StdField::Username, format!("{C}-on-disk-user")),
        ]),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![],
    })
    .unwrap();
    items::set_field(id, StdField::Notes, format!("{C}-on-disk-notes")).unwrap();
    let r = lifecycle::regenerate_recovery_key(pw(PASSWORD), false).unwrap();
    let key = r.recovery_key.clone();
    lifecycle::lock().unwrap();
    let bytes = disk_bytes(f.dir.path());
    for needle in [
        format!("{C}-on-disk").as_bytes(),
        PASSWORD.as_bytes(),
        key.as_slice(),
        b"on-disk-pw",
    ] {
        assert!(!contains(&bytes, needle), "plaintext found on disk");
    }
}
