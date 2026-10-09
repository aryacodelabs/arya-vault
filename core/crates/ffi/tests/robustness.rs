//! Concurrency, `lock()` while a call is in flight, panics, and the cross-check with the session
//! library (what the CLI uses).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use arya_vault_ffi::api::dto::*;
use arya_vault_ffi::api::{items, lifecycle};
use common::*;

fn login(title: String) -> NewItem {
    NewItem {
        item_type: ItemType::Login,
        title,
        fields: Default::default(),
        urls: vec![],
        tags: vec![],
        folder_id: None,
        custom: vec![],
    }
}

/// Runs `f` and fails (instead of hanging the suite) if it does not finish in time.
fn within<T: Send + 'static>(secs: u64, f: impl FnOnce() -> T + Send + 'static) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(Duration::from_secs(secs))
        .expect("deadlock or hang")
}

#[test]
fn calls_from_many_threads_are_serialised_without_losing_writes() {
    let _g = serial();
    let _f = unlocked();
    within(120, || {
        let handles: Vec<_> = (0..8)
            .map(|t| {
                thread::spawn(move || {
                    for i in 0..10 {
                        items::create_item(login(format!("t{t}-{i}"))).unwrap();
                        // Interleave reads with the writes.
                        let _ = items::item_count().unwrap();
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    });
    assert_eq!(items::item_count().unwrap(), 80);
    let all = items::list(
        ListFilter::default(),
        Page {
            offset: 0,
            limit: 200,
        },
    )
    .unwrap();
    assert_eq!(all.len(), 80);
}

#[test]
fn lock_while_an_unlock_is_in_flight_ends_locked_and_does_not_deadlock() {
    let _g = serial();
    let _f = fresh();
    let unlocker = thread::spawn(|| lifecycle::unlock(pw(PASSWORD)));
    thread::sleep(Duration::from_millis(150));
    within(120, || lifecycle::lock().unwrap());
    let r = within(120, move || unlocker.join().unwrap());
    // Either the unlock noticed the lock request and gave up, or it finished first and the lock
    // then locked it. Both end locked.
    if let Err(e) = r {
        assert_eq!(e.code, AppErrorCode::Locked);
    }
    assert!(lifecycle::status().unwrap().locked);
    assert_eq!(items::item_count().unwrap_err().code, AppErrorCode::Locked);
    // The vault is intact and unlockable afterwards.
    lifecycle::unlock(pw(PASSWORD)).unwrap();
}

#[test]
fn lock_during_a_burst_of_writes_is_safe() {
    let _g = serial();
    let _f = unlocked();
    let writer = thread::spawn(|| {
        let mut ok = 0;
        for i in 0..200 {
            match items::create_item(login(format!("w{i}"))) {
                Ok(_) => ok += 1,
                Err(e) => {
                    assert_eq!(e.code, AppErrorCode::Locked);
                    break;
                }
            }
        }
        ok
    });
    thread::sleep(Duration::from_millis(30));
    within(60, || lifecycle::lock().unwrap());
    let written = within(60, move || writer.join().unwrap());
    assert!(lifecycle::status().unwrap().locked);
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    // Everything acknowledged before the lock is there; nothing after.
    assert_eq!(items::item_count().unwrap(), written);
}

#[test]
fn a_panic_becomes_internal_locks_the_session_and_does_not_poison_it() {
    let _g = serial();
    let _f = unlocked();
    items::create_item(login("before".into())).unwrap();
    let e = arya_vault_ffi::__panic_for_test().unwrap_err();
    assert_eq!(e.code, AppErrorCode::Internal);
    assert_eq!(e.message, "internal error");
    // Fail closed: the session was locked.
    assert!(lifecycle::status().unwrap().locked);
    assert_eq!(items::item_count().unwrap_err().code, AppErrorCode::Locked);
    // And the API still works (poisoning is ignored).
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    assert_eq!(items::item_count().unwrap(), 1);
}

#[test]
fn a_vault_made_by_the_session_library_opens_through_the_api_and_back() {
    use arya_vault_session::{KdfProfile as K, RecoveryConfirmation, Session};

    let _g = serial();
    // The CLI is a thin client of `Session`: create with it, open with the API.
    let dir = tempfile::tempdir().unwrap();
    {
        let mut s = Session::open_dir(dir.path()).unwrap();
        s.create_with(PASSWORD, K::Low, RecoveryConfirmation::NotRequired)
            .unwrap();
        s.with_vault(|v| {
            v.create_item(arya_vault_vault::NewItem::new(
                arya_vault_vault::ItemType::Login,
                "made by the CLI",
            ))
            .unwrap();
        })
        .unwrap();
        s.lock().unwrap();
    }
    lifecycle::lock().unwrap();
    lifecycle::init_core(dir.path().to_str().unwrap().to_owned()).unwrap();
    let st = lifecycle::status().unwrap();
    assert!(st.exists && st.locked && st.onboarding_complete);
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    let all = items::list(
        ListFilter::default(),
        Page {
            offset: 0,
            limit: 10,
        },
    )
    .unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].title, "made by the CLI");
    items::create_item(login("made by the API".into())).unwrap();
    lifecycle::lock().unwrap();

    // ...and the other way round.
    let mut s = Session::open_dir(dir.path()).unwrap();
    s.unlock(PASSWORD).unwrap();
    let n = s.with_vault(|v| v.item_count().unwrap()).unwrap();
    assert_eq!(n, 2);
}
