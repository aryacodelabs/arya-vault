//! Fuzz target for the strict canonical-CBOR decoder (`format::cbor::decode`).
//!
//! Properties: never panics or allocates unboundedly (typed `Err` is correct), and any
//! accepted input is *exactly* the canonical encoding of the decoded value.
#![no_main]

use arya_vault_crypto::format::cbor::{Limits, decode};
use libfuzzer_sys::fuzz_target;

const LIMITS: Limits = Limits::new(4096, 256, 4096);

fuzz_target!(|data: &[u8]| {
    if let Ok(v) = decode(data, &LIMITS) {
        let again = v.encode().expect("a decoded value must re-encode");
        assert_eq!(again, data, "decoder accepted a non-canonical encoding");
    }
});
