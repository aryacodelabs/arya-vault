//! Fuzz target for `vault::value::decode` (docs/05, docs/11 section 4). Register values come out
//! of the encrypted database, but a modified or hostile peer's op can put any bytes there once
//! sync exists, so the decoder is treated as a parser of external bytes.
//!
//! Properties: never panics; an accepted value is a canonical scalar, so re-encoding it gives
//! exactly the input bytes.
#![no_main]

use arya_vault_vault::fuzzing::{
    MAX_VALUE_BYTES, Value, decode, encode_bool, encode_bytes, encode_int, encode_text,
};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(v) = decode(data) else { return };
    assert!(data.len() <= MAX_VALUE_BYTES + 16);
    let again = match &v {
        Value::Text(s) => encode_text(s),
        Value::Bool(b) => encode_bool(*b),
        Value::Int(i) => encode_int(*i),
        Value::Bytes(b) => encode_bytes(b),
    };
    assert_eq!(&again[..], data, "accepted a non-canonical encoding");
});
