//! Fuzz target for `Header::decode` (docs/04 §5). Headers are unauthenticated cloud bytes.
//!
//! Properties: never panics; an accepted header has in-range KDF parameters (SEC-C11) and
//! re-encodes to the identical bytes.
#![no_main]

use arya_vault_crypto::format::header::Header;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(h) = Header::decode(data) {
        h.kdf.validate().expect("decode must enforce KDF bounds");
        assert_eq!(h.encode().expect("decoded header must re-encode"), data);
    }
});
