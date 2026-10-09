//! Fuzz target for the quick-unlock policy record parser (docs/04 §8; CLAUDE.md rule 7).
//!
//! The record sits in the vault directory as plaintext, so any local process can replace it.
//! Properties checked:
//! * never panics, aborts or allocates unboundedly (a typed `Err` is correct);
//! * if parsing succeeds, re-encoding gives back exactly the input (the format is canonical).
#![no_main]

use arya_vault_session::PolicyRecord;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(rec) = PolicyRecord::decode(data) {
        assert_eq!(rec.encode(), data, "decode/encode must round-trip");
    }
});
