//! The failure delay at session level (docs/07 §4): injectable clock, never sleeps.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use arya_vault_session::{BackoffPolicy, KdfProfile, Session};
use common::*;

const BIN: &str = "backoff";
const POLICY: BackoffPolicy = BackoffPolicy {
    free_attempts: 2,
    base_ms: 1_000,
    max_ms: 4_000,
};

#[test]
fn wrong_passwords_start_a_delay_that_blocks_even_the_right_password() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let clock = FakeClock::default();
    let mut s = Session::open_dir_with(&dir, config_with(&clock, POLICY)).unwrap();

    expect_code(s.unlock(WRONG), "wrongCredentials");
    expect_code(s.unlock(WRONG), "wrongCredentials");

    // the second failure started a 1 s delay: the next attempt is refused without trying
    let e = s.unlock(PW).unwrap_err();
    assert_eq!(e.retry_after_ms(), Some(1_000));
    assert_eq!(e.code().as_str(), "busy");
    clock.advance(400);
    assert_eq!(s.unlock(PW).unwrap_err().retry_after_ms(), Some(600));

    // proof that no key derivation or disk read happens during the delay: even a destroyed
    // header still yields `Backoff`
    let hdr = header_path(&dir);
    let good = fs::read(&hdr).unwrap();
    fs::write(&hdr, b"destroyed").unwrap();
    assert_eq!(s.unlock(PW).unwrap_err().retry_after_ms(), Some(600));
    fs::write(&hdr, good).unwrap();

    // after the wait the right password works and the counter resets
    clock.advance(600);
    s.unlock(PW).unwrap();
    s.lock().unwrap();
    expect_code(s.unlock(WRONG), "wrongCredentials");
    expect_code(s.unlock(WRONG), "wrongCredentials");
    assert!(s.unlock(PW).unwrap_err().retry_after_ms().is_some());
}

#[test]
fn delay_doubles_and_is_capped() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let clock = FakeClock::default();
    let mut s = Session::open_dir_with(&dir, config_with(&clock, POLICY)).unwrap();
    let mut seen = Vec::new();
    for _ in 0..5 {
        let _ = s.unlock(WRONG);
        let wait = s.unlock(WRONG).unwrap_err().retry_after_ms();
        if let Some(ms) = wait {
            seen.push(ms);
            clock.advance(ms);
        }
    }
    assert_eq!(seen, [1_000, 2_000, 4_000, 4_000]);
}

#[test]
fn verify_change_and_regenerate_share_the_same_counter() {
    let (_t, dir, _key) = fixture_copy(BIN);
    let clock = FakeClock::default();
    let mut s = Session::open_dir_with(&dir, config_with(&clock, POLICY)).unwrap();
    s.unlock(PW).unwrap();
    assert!(!s.verify_password(WRONG).unwrap());
    expect_code(
        s.change_password(WRONG, PW2, KdfProfile::Low),
        "wrongCredentials",
    );
    // two failures: everything that takes a password is now refused
    for r in [
        s.verify_password(PW).map(|_| ()),
        s.change_password(PW, PW2, KdfProfile::Low).map(|_| ()),
        s.regenerate_recovery_key(PW).map(|_| ()),
        s.check_header_password(PW),
    ] {
        assert_eq!(r.unwrap_err().retry_after_ms(), Some(1_000));
    }
    clock.advance(1_000);
    assert!(s.verify_password(PW).unwrap());
}

#[test]
fn recovery_key_attempts_are_not_delayed_a_typo_is_not_a_failure() {
    let (_t, dir, key) = fixture_copy(BIN);
    let clock = FakeClock::default();
    let mut s = Session::open_dir_with(&dir, config_with(&clock, POLICY)).unwrap();
    for _ in 0..4 {
        expect_code(
            s.recover("typo", PW2, KdfProfile::Low),
            "recoveryKeyMalformed",
        );
    }
    // a 160-bit key cannot be guessed, so the delay (a UX brake on passwords) does not apply
    let _ = key;
    s.unlock(PW).unwrap();
}
