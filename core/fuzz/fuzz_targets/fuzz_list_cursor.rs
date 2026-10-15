//! Fuzz target for `ListCursor::decode` (docs/14 section 4.2.1). The cursor string crosses the
//! FFI boundary from the app, so it is a parser of external input.
//!
//! Properties: never panics; an accepted cursor re-encodes to exactly the input text.
#![no_main]

use arya_vault_vault::ListCursor;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Some(c) = ListCursor::decode(text) {
        assert_eq!(c.encode(), text);
    }
});
