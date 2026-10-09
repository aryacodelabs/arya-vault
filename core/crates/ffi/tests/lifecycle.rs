//! docs/14 §4.1 and §5 through the `api` module: lifecycle, the state machine, recovery-key
//! confirmation, password change, recovery, rotation, quick unlock.

mod common;

use arya_vault_ffi::api::dto::{
    AppErrorCode, GroupAnswer, ItemType, KdfProfile, NewItem, QuickUnlockKind,
};
use arya_vault_ffi::api::{items, lifecycle};
use arya_vault_session::quick::fake::FakeProvider;
use common::*;

fn code<T: std::fmt::Debug>(r: Result<T, arya_vault_ffi::AppError>) -> AppErrorCode {
    r.unwrap_err().code
}

fn new_login(title: &str) -> NewItem {
    NewItem {
        item_type: ItemType::Login,
        title: title.into(),
        fields: Default::default(),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![],
    }
}

#[test]
fn sec_a01_create_confirm_lock_unlock() {
    let _g = serial();
    let _dir = empty();
    let s = lifecycle::status().unwrap();
    assert!(!s.exists);

    // Not initialised vault: every vault operation says `locked`.
    assert_eq!(code(items::list(Default::default(), page())), AppErrorCode::Locked);

    let r = lifecycle::create_vault(pw(PASSWORD), KdfProfile::Low).unwrap();
    assert_eq!(r.groups, 8);
    assert_eq!(r.challenge.len(), 3);
    let s = lifecycle::status().unwrap();
    assert!(s.exists && !s.locked && !s.onboarding_complete);

    // A wrong answer: `false`, a new challenge, and the key is still pending.
    let wrong = r
        .challenge
        .iter()
        .map(|i| GroupAnswer {
            index: *i,
            text: "AAAAA".into(),
        })
        .collect();
    assert!(!lifecycle::confirm_recovery_key(wrong).unwrap());
    assert!(!lifecycle::status().unwrap().onboarding_complete);
    // The challenge changed after the miss, so the original answers may no longer cover it.
    let r_again = {
        // Ask for a fresh key to get a challenge we can answer, as the app would.
        lifecycle::regenerate_recovery_key(pw(PASSWORD), false).unwrap()
    };
    assert!(lifecycle::confirm_recovery_key(answers(&r_again)).unwrap());
    assert!(lifecycle::status().unwrap().onboarding_complete);

    // Lock, then everything says `locked`; lock is idempotent.
    lifecycle::lock().unwrap();
    lifecycle::lock().unwrap();
    assert!(lifecycle::status().unwrap().locked);
    assert_eq!(code(items::item_count()), AppErrorCode::Locked);
    assert_eq!(code(items::create_item(new_login("x"))), AppErrorCode::Locked);
    assert_eq!(code(lifecycle::quick_unlock_enable()), AppErrorCode::Locked);

    // Unlock: wrong password, then the right one.
    assert_eq!(
        code(lifecycle::unlock(pw(OTHER_PASSWORD))),
        AppErrorCode::WrongCredentials
    );
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    assert_eq!(items::item_count().unwrap(), 0);
    assert_eq!(code(lifecycle::unlock(pw(PASSWORD))), AppErrorCode::Validation);
    lifecycle::lock().unwrap();
}

fn page() -> arya_vault_ffi::api::dto::Page {
    arya_vault_ffi::api::dto::Page { offset: 0, limit: 50 }
}

#[test]
fn confirm_without_a_pending_key_is_a_validation_error() {
    let _g = serial();
    let _f = unlocked();
    assert_eq!(
        code(lifecycle::confirm_recovery_key(vec![])),
        AppErrorCode::Validation
    );
}

#[test]
fn weak_and_malformed_inputs_map_to_their_codes() {
    let _g = serial();
    let _dir = empty();
    assert_eq!(
        code(lifecycle::create_vault(pw("short"), KdfProfile::Low)),
        AppErrorCode::WeakPassword
    );
    // Not UTF-8: a validation error naming the field, and nothing echoed.
    let e = lifecycle::create_vault(vec![0xFF, 0xFE, 0xFD], KdfProfile::Low).unwrap_err();
    assert_eq!(e.code, AppErrorCode::Validation);
    assert_eq!(e.field.as_deref(), Some("password"));
    let _f = unlocked_after_create();
    assert_eq!(
        code(lifecycle::recover_with_key(pw("not-a-key"), pw(OTHER_PASSWORD))),
        AppErrorCode::RecoveryKeyMalformed
    );
}

fn unlocked_after_create() {
    let r = lifecycle::create_vault(pw(PASSWORD), KdfProfile::Low).unwrap();
    assert!(lifecycle::confirm_recovery_key(answers(&r)).unwrap());
}

#[test]
fn sec_a05_change_password_without_the_recovery_key() {
    let _g = serial();
    let _f = unlocked();
    items::create_item(new_login("kept")).unwrap();
    assert_eq!(
        code(lifecycle::change_password(pw(OTHER_PASSWORD), pw(OTHER_PASSWORD), false)),
        AppErrorCode::WrongCredentials
    );
    assert_eq!(
        code(lifecycle::change_password(pw(PASSWORD), pw("weak"), false)),
        AppErrorCode::WeakPassword
    );
    lifecycle::change_password(pw(PASSWORD), pw(OTHER_PASSWORD), false).unwrap();
    lifecycle::lock().unwrap();
    assert_eq!(
        code(lifecycle::unlock(pw(PASSWORD))),
        AppErrorCode::WrongCredentials
    );
    lifecycle::unlock(pw(OTHER_PASSWORD)).unwrap();
    assert_eq!(items::item_count().unwrap(), 1);
    assert!(lifecycle::verify_password(pw(OTHER_PASSWORD)).unwrap());
    assert!(!lifecycle::verify_password(pw(PASSWORD)).unwrap());
}

#[test]
fn sec_c12_recover_with_the_key_leaves_the_vault_unlocked() {
    let _g = serial();
    let f = fresh();
    let key = f.recovery_key.clone();
    lifecycle::recover_with_key(key.clone(), pw(OTHER_PASSWORD)).unwrap();
    assert_eq!(items::item_count().unwrap(), 0); // unlocked
    lifecycle::lock().unwrap();
    lifecycle::unlock(pw(OTHER_PASSWORD)).unwrap();
}

#[test]
fn sec_a06_rotation_through_regenerate_and_change_password() {
    let _g = serial();
    let _f = unlocked();
    let id = items::create_item(new_login("survives rotation")).unwrap();

    // regenerateRecoveryKey(rotateKeys: true)
    let r = lifecycle::regenerate_recovery_key(pw(PASSWORD), true).unwrap();
    assert!(!lifecycle::status().unwrap().onboarding_complete);
    assert!(lifecycle::confirm_recovery_key(answers(&r)).unwrap());
    assert_eq!(items::get_item(id.clone()).unwrap().title, "survives rotation");

    // changePassword(rotateKeys: true): the new key stays pending; the app asks for a key to show.
    lifecycle::change_password(pw(PASSWORD), pw(OTHER_PASSWORD), true).unwrap();
    assert!(!lifecycle::status().unwrap().onboarding_complete);
    let r2 = lifecycle::regenerate_recovery_key(pw(OTHER_PASSWORD), false).unwrap();
    assert!(lifecycle::confirm_recovery_key(answers(&r2)).unwrap());
    lifecycle::lock().unwrap();
    assert_eq!(code(lifecycle::unlock(pw(PASSWORD))), AppErrorCode::WrongCredentials);
    lifecycle::unlock(pw(OTHER_PASSWORD)).unwrap();
    assert_eq!(items::get_item(id).unwrap().title, "survives rotation");
}

#[test]
fn quick_unlock_with_the_fake_provider() {
    let _g = serial();
    let _f = fresh();
    // Default: no provider.
    let s = lifecycle::status().unwrap().quick_unlock;
    assert!(!s.supported && !s.enabled && s.kind == QuickUnlockKind::None);
    assert_eq!(
        code(lifecycle::unlock_quick()),
        AppErrorCode::QuickUnlockUnavailable
    );

    let (provider, _handle) = FakeProvider::new();
    arya_vault_ffi::register_quick_unlock_provider(Box::new(provider)).unwrap();
    let s = lifecycle::status().unwrap().quick_unlock;
    assert!(s.supported && !s.enabled && s.kind == QuickUnlockKind::WindowsHello);

    lifecycle::unlock(pw(PASSWORD)).unwrap();
    lifecycle::quick_unlock_enable().unwrap();
    assert!(lifecycle::status().unwrap().quick_unlock.enabled);
    lifecycle::lock().unwrap();
    lifecycle::unlock_quick().unwrap();
    assert_eq!(items::item_count().unwrap(), 0);

    // A rotation disables it (the blob holds the old key).
    let r = lifecycle::regenerate_recovery_key(pw(PASSWORD), true).unwrap();
    assert!(lifecycle::confirm_recovery_key(answers(&r)).unwrap());
    lifecycle::lock().unwrap();
    assert_eq!(
        code(lifecycle::unlock_quick()),
        AppErrorCode::QuickUnlockUnavailable
    );
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    lifecycle::quick_unlock_disable().unwrap();
}
