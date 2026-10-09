//! Fuzz target for `arya_vault_crypto::recovery_key::parse` (docs/04 §4; CLAUDE.md rule 7).
//!
//! The parser takes user-typed text, so arbitrary bytes are interpreted as UTF-8 (lossily,
//! so invalid sequences still reach the parser as U+FFFD). Properties checked:
//! * never panics, aborts or allocates unboundedly (a typed `Err` is correct);
//! * if parsing succeeds, re-encoding and re-parsing yields the same key bytes.
#![no_main]

use arya_vault_crypto::recovery_key::{encode, parse};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(key) = parse(&text) {
        let again = parse(&encode(&key)).expect("canonical encoding must re-parse");
        assert_eq!(again.expose_secret(), key.expose_secret());
    }
});
