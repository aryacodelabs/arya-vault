//! Register values: scalar CBOR (RFC 8949 deterministic encoding).
//!
//! Encoding of text/bool/int/bytes is done directly (so secret text is written
//! straight into a zeroizing buffer); decoding goes through the strict,
//! bounded, fuzzed canonical decoder of the `crypto` crate and then accepts only
//! these scalar types.

use arya_vault_crypto::format::cbor::{self, Limits};
use zeroize::Zeroizing;

use crate::error::{Result, VaultError};

/// Maximum encoded size of any stored value (the note body limit, docs/05 section 10).
pub const MAX_VALUE_BYTES: usize = 1 << 20;

/// A decoded scalar register value.
#[derive(Clone, PartialEq, Eq)]
pub enum Value {
    /// UTF-8 text.
    Text(String),
    /// Boolean.
    Bool(bool),
    /// Signed integer.
    Int(i64),
    /// Raw bytes (e.g. a folder id).
    Bytes(Vec<u8>),
}

impl core::fmt::Debug for Value {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Never print contents: values may be secrets.
        match self {
            Value::Text(s) => write!(f, "Text(<{} bytes>)", s.len()),
            Value::Bool(b) => write!(f, "Bool({b})"),
            Value::Int(_) => f.write_str("Int(..)"),
            Value::Bytes(b) => write!(f, "Bytes(<{} bytes>)", b.len()),
        }
    }
}

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let m = major << 5;
    if n < 24 {
        out.push(m | n as u8);
    } else if n <= 0xff {
        out.extend([m | 24, n as u8]);
    } else if n <= 0xffff {
        out.push(m | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n <= 0xffff_ffff {
        out.push(m | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend(n.to_be_bytes());
    }
}

/// Encode text.
#[must_use]
pub fn encode_text(s: &str) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(s.len() + 9));
    head(&mut out, 3, s.len() as u64);
    out.extend(s.as_bytes());
    out
}

/// Encode bytes.
#[must_use]
pub fn encode_bytes(b: &[u8]) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(b.len() + 9));
    head(&mut out, 2, b.len() as u64);
    out.extend(b);
    out
}

/// Encode a boolean.
#[must_use]
pub fn encode_bool(b: bool) -> Zeroizing<Vec<u8>> {
    Zeroizing::new(vec![if b { 0xf5 } else { 0xf4 }])
}

/// Encode a signed integer.
#[must_use]
pub fn encode_int(i: i64) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::with_capacity(9));
    match u64::try_from(i) {
        Ok(n) => head(&mut out, 0, n),
        Err(_) => head(&mut out, 1, i.unsigned_abs() - 1),
    }
    out
}

/// Decode a stored value.
///
/// # Errors
/// [`VaultError::Corrupt`] for anything that is not a canonical scalar of a
/// supported type within [`MAX_VALUE_BYTES`].
pub fn decode(bytes: &[u8]) -> Result<Value> {
    if bytes.len() > MAX_VALUE_BYTES + 16 {
        return Err(VaultError::Corrupt);
    }
    let limits = Limits::new(MAX_VALUE_BYTES, 1, 1);
    match cbor::decode(bytes, &limits).map_err(|_| VaultError::Corrupt)? {
        cbor::Value::Text(s) => Ok(Value::Text(s)),
        cbor::Value::Bool(b) => Ok(Value::Bool(b)),
        cbor::Value::Bytes(b) => Ok(Value::Bytes(b)),
        cbor::Value::Uint(n) => i64::try_from(n)
            .map(Value::Int)
            .map_err(|_| VaultError::Corrupt),
        cbor::Value::Nint(n) => {
            let n = i64::try_from(n).map_err(|_| VaultError::Corrupt)?;
            Ok(Value::Int(-1 - n))
        }
        _ => Err(VaultError::Corrupt),
    }
}

/// Decode text into a zeroizing string.
///
/// # Errors
/// [`VaultError::Corrupt`] if the value is not text.
pub fn decode_secret_text(bytes: &[u8]) -> Result<Zeroizing<String>> {
    match decode(bytes)? {
        Value::Text(s) => Ok(Zeroizing::new(s)),
        _ => Err(VaultError::Corrupt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn matches_the_canonical_encoder() {
        for s in [
            "",
            "a",
            "x".repeat(23).as_str(),
            "x".repeat(24).as_str(),
            "x".repeat(300).as_str(),
            "héllo ✓",
        ] {
            assert_eq!(
                *encode_text(s),
                cbor::Value::text(s).encode().unwrap(),
                "text {}",
                s.len()
            );
        }
        for b in [vec![], vec![1, 2, 3], vec![0; 256]] {
            assert_eq!(
                *encode_bytes(&b),
                cbor::Value::Bytes(b.clone()).encode().unwrap()
            );
        }
        assert_eq!(
            *encode_bool(true),
            cbor::Value::Bool(true).encode().unwrap()
        );
        assert_eq!(*encode_int(0), cbor::Value::Uint(0).encode().unwrap());
        assert_eq!(*encode_int(-1), cbor::Value::Nint(0).encode().unwrap());
        assert_eq!(
            *encode_int(i64::MIN),
            cbor::Value::Nint(i64::MAX as u64).encode().unwrap()
        );
    }

    #[test]
    fn rejects_non_scalars_and_noncanonical() {
        assert!(decode(&[0x80]).is_err(), "array");
        assert!(decode(&[0xa0]).is_err(), "map");
        assert!(decode(&[0xf6]).is_err(), "null");
        assert!(decode(&[0x18, 0x01]).is_err(), "non-shortest int");
        assert!(decode(&[0x61, b'a', 0x00]).is_err(), "trailing bytes");
        assert!(decode(&[]).is_err());
        assert!(
            decode(&[0x1b, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]).is_err(),
            "u64::MAX does not fit i64"
        );
    }

    /// The same property the `fuzz_vault_value_decode` target checks, run over its seed corpus on
    /// stable (the fuzzer itself needs nightly): no panic, an accepted value re-encodes to the
    /// input, `bad-*` seeds are rejected and the others accepted.
    #[test]
    fn fuzz_seed_corpus_satisfies_the_target_property() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fuzz/corpus/fuzz_vault_value_decode");
        let mut n = 0;
        for e in std::fs::read_dir(dir).unwrap() {
            let e = e.unwrap();
            let name = e.file_name().into_string().unwrap();
            let data = std::fs::read(e.path()).unwrap();
            match decode(&data) {
                Ok(v) => {
                    assert!(!name.starts_with("bad-"), "{name} should be rejected");
                    let again = match &v {
                        Value::Text(s) => encode_text(s),
                        Value::Bool(b) => encode_bool(*b),
                        Value::Int(i) => encode_int(*i),
                        Value::Bytes(b) => encode_bytes(b),
                    };
                    assert_eq!(&again[..], &data[..], "{name} is not canonical");
                }
                Err(_) => assert!(name.starts_with("bad-"), "{name} should be accepted"),
            }
            n += 1;
        }
        assert!(n >= 20, "seed corpus missing ({n} files)");
    }

    #[test]
    fn debug_never_prints_contents() {
        let v = Value::Text("CANARY-SECRET".into());
        assert!(!format!("{v:?}").contains("CANARY"));
        assert!(!format!("{:?}", Value::Bytes(b"CANARY".to_vec())).contains("CANARY"));
    }

    proptest! {
        #[test]
        fn round_trip(s in ".{0,200}", i in any::<i64>(), b in any::<bool>(), bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
            prop_assert_eq!(decode(&encode_text(&s)).unwrap(), Value::Text(s.clone()));
            prop_assert_eq!(decode(&encode_int(i)).unwrap(), Value::Int(i));
            prop_assert_eq!(decode(&encode_bool(b)).unwrap(), Value::Bool(b));
            prop_assert_eq!(decode(&encode_bytes(&bytes)).unwrap(), Value::Bytes(bytes));
        }

        #[test]
        fn decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let _ = decode(&bytes);
        }
    }
}
