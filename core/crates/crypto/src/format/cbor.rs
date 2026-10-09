//! Canonical CBOR (RFC 8949 §4.2.1 deterministic encoding): encoder and **strict** decoder.
//!
//! Used for headers, envelope AAD and (later) ops, so hashes and AAD are reproducible
//! across implementations (docs/04 §5-6, docs/05 §6).
//!
//! Supported data model: unsigned and negative integers, byte strings, text strings
//! (valid UTF-8), arrays, maps, booleans and null. Floats, tags and other simple values
//! are rejected.
//!
//! The decoder accepts **only** the canonical encoding of a value and rejects:
//! non-shortest integer/length heads, indefinite lengths, tags, floats/undefined/simple
//! values, invalid UTF-8, map keys that are duplicated or not in strictly increasing
//! bytewise order of their encoded form, nesting deeper than [`Limits::max_depth`],
//! any string/array/map length above the caller's [`Limits`], and trailing bytes.
//! Declared lengths are checked against both the limits and the remaining input
//! **before** anything is allocated, so a hostile length prefix cannot cause a large
//! allocation. Errors are typed and never panic.
//!
//! Design choice (recorded in the PR): a small in-crate codec instead of `ciborium`,
//! because `ciborium` serializes maps in insertion order, has no canonical-only decoder
//! and would need all of the checks above re-implemented on top of it; this avoids a
//! dependency on a security-critical byte format.

use thiserror::Error;

/// Default maximum nesting depth of arrays/maps.
pub const DEFAULT_MAX_DEPTH: usize = 16;

/// Errors from encoding or decoding canonical CBOR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum CborError {
    /// Input ended in the middle of a value.
    #[error("unexpected end of input")]
    Truncated,
    /// Bytes remain after the top-level value.
    #[error("trailing bytes after value")]
    TrailingBytes,
    /// An integer or length head is not in its shortest form.
    #[error("non-canonical integer or length encoding")]
    NonShortest,
    /// Indefinite-length items are not allowed.
    #[error("indefinite length not allowed")]
    Indefinite,
    /// Tags, floats, `undefined` and other simple values are not supported.
    #[error("unsupported CBOR item")]
    Unsupported,
    /// A reserved additional-information value was used.
    #[error("malformed CBOR head")]
    Malformed,
    /// A text string is not valid UTF-8.
    #[error("invalid UTF-8 in text string")]
    InvalidUtf8,
    /// Map keys are not in strictly increasing canonical order.
    #[error("map keys not in canonical order")]
    MapOrder,
    /// A map contains the same key twice.
    #[error("duplicate map key")]
    DuplicateKey,
    /// Nesting exceeds the configured depth.
    #[error("nesting too deep")]
    TooDeep,
    /// A declared length or count exceeds the configured limits.
    #[error("length exceeds limit")]
    TooLong,
    /// The total number of items exceeds the configured budget.
    #[error("too many items")]
    TooManyItems,
}

/// Caller-supplied bounds for [`decode`]. All are enforced before allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Maximum nesting depth of arrays/maps (the top-level container is depth 1).
    pub max_depth: usize,
    /// Maximum length in bytes of any byte string or text string.
    pub max_len: usize,
    /// Maximum number of elements of any array or entries of any map.
    pub max_items: usize,
    /// Maximum number of values (of any kind) in the whole document.
    pub max_total_items: usize,
}

impl Limits {
    /// Limits with the default depth of [`DEFAULT_MAX_DEPTH`].
    pub const fn new(max_len: usize, max_items: usize, max_total_items: usize) -> Self {
        Self {
            max_depth: DEFAULT_MAX_DEPTH,
            max_len,
            max_items,
            max_total_items,
        }
    }
}

/// A decoded (or to-be-encoded) CBOR value.
///
/// Maps are a list of entries; use [`Value::map`] to build one (it sorts into canonical
/// order and rejects duplicate keys). Maps returned by [`decode`] are already canonical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// Unsigned integer.
    Uint(u64),
    /// Negative integer `-1 - n`, holding `n`.
    Nint(u64),
    /// Byte string.
    Bytes(Vec<u8>),
    /// Text string.
    Text(String),
    /// Array.
    Array(Vec<Value>),
    /// Map, in canonical key order.
    Map(Vec<(Value, Value)>),
    /// Boolean.
    Bool(bool),
    /// Null.
    Null,
}

impl Value {
    /// Builds a map value from entries, sorting them into canonical order.
    /// Fails on duplicate keys.
    pub fn map(entries: Vec<(Value, Value)>) -> Result<Value, CborError> {
        let mut keyed = Vec::with_capacity(entries.len());
        for (k, v) in entries {
            keyed.push((k.encode()?, k, v));
        }
        keyed.sort_by(|a, b| a.0.cmp(&b.0));
        if keyed.windows(2).any(|w| w[0].0 == w[1].0) {
            return Err(CborError::DuplicateKey);
        }
        Ok(Value::Map(
            keyed.into_iter().map(|(_, k, v)| (k, v)).collect(),
        ))
    }

    /// Shorthand for a text key.
    pub fn text(s: &str) -> Value {
        Value::Text(s.to_owned())
    }

    /// Encodes the value canonically. Fails on duplicate or mis-ordered map keys and on
    /// nesting deeper than [`DEFAULT_MAX_DEPTH`].
    pub fn encode(&self) -> Result<Vec<u8>, CborError> {
        let mut out = Vec::new();
        encode_into(self, &mut out, 1)?;
        Ok(out)
    }
}

fn encode_into(v: &Value, out: &mut Vec<u8>, depth: usize) -> Result<(), CborError> {
    match v {
        Value::Uint(n) => head(out, MAJOR_UINT, *n),
        Value::Nint(n) => head(out, 1, *n),
        Value::Bytes(b) => {
            head(out, MAJOR_BYTES, b.len() as u64);
            out.extend_from_slice(b);
        }
        Value::Text(s) => {
            head(out, MAJOR_TEXT, s.len() as u64);
            out.extend_from_slice(s.as_bytes());
        }
        Value::Bool(false) => out.push(0xf4),
        Value::Bool(true) => out.push(0xf5),
        Value::Null => out.push(0xf6),
        Value::Array(items) => {
            if depth > DEFAULT_MAX_DEPTH {
                return Err(CborError::TooDeep);
            }
            head(out, 4, items.len() as u64);
            for item in items {
                encode_into(item, out, depth + 1)?;
            }
        }
        Value::Map(entries) => {
            if depth > DEFAULT_MAX_DEPTH {
                return Err(CborError::TooDeep);
            }
            head(out, MAJOR_MAP, entries.len() as u64);
            let mut prev: Option<Vec<u8>> = None;
            for (k, val) in entries {
                let mut kb = Vec::new();
                encode_into(k, &mut kb, depth + 1)?;
                match &prev {
                    Some(p) if *p == kb => return Err(CborError::DuplicateKey),
                    Some(p) if *p > kb => return Err(CborError::MapOrder),
                    _ => {}
                }
                out.extend_from_slice(&kb);
                encode_into(val, out, depth + 1)?;
                prev = Some(kb);
            }
        }
    }
    Ok(())
}

/// Decodes exactly one canonical CBOR value from `input`, enforcing `limits`.
pub fn decode(input: &[u8], limits: &Limits) -> Result<Value, CborError> {
    let mut d = Decoder {
        input,
        pos: 0,
        limits,
        budget: limits.max_total_items,
    };
    let v = d.value(1)?;
    if d.pos != input.len() {
        return Err(CborError::TrailingBytes);
    }
    Ok(v)
}

struct Decoder<'a> {
    input: &'a [u8],
    pos: usize,
    limits: &'a Limits,
    budget: usize,
}

impl<'a> Decoder<'a> {
    fn remaining(&self) -> usize {
        self.input.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], CborError> {
        if n > self.remaining() {
            return Err(CborError::Truncated);
        }
        let s = &self.input[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    /// Reads a head, returning `(major, additional-info, argument)` with the shortest-form
    /// rule enforced. Major 7 heads are returned with the raw additional info as argument.
    fn head(&mut self) -> Result<(u8, u8, u64), CborError> {
        let b = self.take(1)?[0];
        let (major, ai) = (b >> 5, b & 0x1f);
        let arg = match ai {
            0..=23 => u64::from(ai),
            24 => {
                let v = u64::from(self.take(1)?[0]);
                if major != 7 && v < 24 {
                    return Err(CborError::NonShortest);
                }
                v
            }
            25 => {
                let s = self.take(2)?;
                let v = u64::from(u16::from_be_bytes([s[0], s[1]]));
                if major != 7 && v <= 0xff {
                    return Err(CborError::NonShortest);
                }
                v
            }
            26 => {
                let s = self.take(4)?;
                let v = u64::from(u32::from_be_bytes([s[0], s[1], s[2], s[3]]));
                if major != 7 && v <= 0xffff {
                    return Err(CborError::NonShortest);
                }
                v
            }
            27 => {
                let s = self.take(8)?;
                let v = u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]);
                if major != 7 && v <= 0xffff_ffff {
                    return Err(CborError::NonShortest);
                }
                v
            }
            28..=30 => return Err(CborError::Malformed),
            _ => return Err(CborError::Indefinite),
        };
        Ok((major, ai, arg))
    }

    fn charge(&mut self) -> Result<(), CborError> {
        self.budget = self.budget.checked_sub(1).ok_or(CborError::TooManyItems)?;
        Ok(())
    }

    fn length(&self, arg: u64, max: usize) -> Result<usize, CborError> {
        let n = usize::try_from(arg).map_err(|_| CborError::TooLong)?;
        if n > max {
            return Err(CborError::TooLong);
        }
        Ok(n)
    }

    fn value(&mut self, depth: usize) -> Result<Value, CborError> {
        self.charge()?;
        let (major, ai, arg) = self.head()?;
        match major {
            0 => Ok(Value::Uint(arg)),
            1 => Ok(Value::Nint(arg)),
            2 => {
                let n = self.length(arg, self.limits.max_len)?;
                Ok(Value::Bytes(self.take(n)?.to_vec()))
            }
            3 => {
                let n = self.length(arg, self.limits.max_len)?;
                let s = std::str::from_utf8(self.take(n)?).map_err(|_| CborError::InvalidUtf8)?;
                Ok(Value::Text(s.to_owned()))
            }
            4 => {
                if depth > self.limits.max_depth {
                    return Err(CborError::TooDeep);
                }
                let n = self.length(arg, self.limits.max_items)?;
                // Every element takes at least one byte: bound before allocating.
                if n > self.remaining() {
                    return Err(CborError::Truncated);
                }
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            5 => {
                if depth > self.limits.max_depth {
                    return Err(CborError::TooDeep);
                }
                let n = self.length(arg, self.limits.max_items)?;
                // Every entry takes at least two bytes.
                if n.saturating_mul(2) > self.remaining() {
                    return Err(CborError::Truncated);
                }
                let mut entries: Vec<(Value, Value)> = Vec::with_capacity(n);
                let mut prev_key: Option<&'a [u8]> = None;
                for _ in 0..n {
                    let start = self.pos;
                    let k = self.value(depth + 1)?;
                    let key_bytes = &self.input[start..self.pos];
                    if let Some(p) = prev_key {
                        match p.cmp(key_bytes) {
                            std::cmp::Ordering::Less => {}
                            std::cmp::Ordering::Equal => return Err(CborError::DuplicateKey),
                            std::cmp::Ordering::Greater => return Err(CborError::MapOrder),
                        }
                    }
                    prev_key = Some(key_bytes);
                    let v = self.value(depth + 1)?;
                    entries.push((k, v));
                }
                Ok(Value::Map(entries))
            }
            6 => Err(CborError::Unsupported),
            _ => match ai {
                20 => Ok(Value::Bool(false)),
                21 => Ok(Value::Bool(true)),
                22 => Ok(Value::Null),
                _ => Err(CborError::Unsupported),
            },
        }
    }
}

// ---- Raw encoders used for AAD construction (kdf struct) ----

const MAJOR_UINT: u8 = 0;
const MAJOR_BYTES: u8 = 2;
const MAJOR_TEXT: u8 = 3;
const MAJOR_MAP: u8 = 5;

fn head(out: &mut Vec<u8>, major: u8, value: u64) {
    let m = major << 5;
    if value < 24 {
        out.push(m | value as u8);
    } else if value <= u64::from(u8::MAX) {
        out.push(m | 24);
        out.push(value as u8);
    } else if value <= u64::from(u16::MAX) {
        out.push(m | 25);
        out.extend_from_slice(&(value as u16).to_be_bytes());
    } else if value <= u64::from(u32::MAX) {
        out.push(m | 26);
        out.extend_from_slice(&(value as u32).to_be_bytes());
    } else {
        out.push(m | 27);
        out.extend_from_slice(&value.to_be_bytes());
    }
}

/// Encodes an unsigned integer.
pub(crate) fn uint(value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    head(&mut out, MAJOR_UINT, value);
    out
}

/// Encodes a byte string.
pub(crate) fn bytes(value: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 9);
    head(&mut out, MAJOR_BYTES, value.len() as u64);
    out.extend_from_slice(value);
    out
}

/// Encodes a text string.
pub(crate) fn text(value: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(value.len() + 9);
    head(&mut out, MAJOR_TEXT, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
    out
}

/// Encodes a map from already-encoded `(key, value)` pairs, sorting entries by encoded
/// key bytes (RFC 8949 §4.2.1). Duplicate keys must not be supplied.
pub(crate) fn map(mut entries: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut out = Vec::new();
    head(&mut out, MAJOR_MAP, entries.len() as u64);
    for (k, v) in entries {
        out.extend_from_slice(&k);
        out.extend_from_slice(&v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // RFC 8949 Appendix A examples.
    #[test]
    fn uint_shortest_form_matches_rfc8949_appendix_a() {
        assert_eq!(uint(0), [0x00]);
        assert_eq!(uint(10), [0x0a]);
        assert_eq!(uint(23), [0x17]);
        assert_eq!(uint(24), [0x18, 0x18]);
        assert_eq!(uint(25), [0x18, 0x19]);
        assert_eq!(uint(100), [0x18, 0x64]);
        assert_eq!(uint(1000), [0x19, 0x03, 0xe8]);
        assert_eq!(uint(1_000_000), [0x1a, 0x00, 0x0f, 0x42, 0x40]);
        assert_eq!(
            uint(1_000_000_000_000),
            [0x1b, 0x00, 0x00, 0x00, 0xe8, 0xd4, 0xa5, 0x10, 0x00]
        );
        assert_eq!(uint(255), [0x18, 0xff]);
        assert_eq!(uint(256), [0x19, 0x01, 0x00]);
        assert_eq!(uint(65535), [0x19, 0xff, 0xff]);
        assert_eq!(uint(65536), [0x1a, 0x00, 0x01, 0x00, 0x00]);
    }

    #[test]
    fn text_and_bytes_match_rfc8949_appendix_a() {
        assert_eq!(text(""), [0x60]);
        assert_eq!(text("a"), [0x61, 0x61]);
        assert_eq!(text("IETF"), [0x64, 0x49, 0x45, 0x54, 0x46]);
        assert_eq!(bytes(&[]), [0x40]);
        assert_eq!(bytes(&[1, 2, 3, 4]), [0x44, 1, 2, 3, 4]);
    }

    #[test]
    fn map_sorts_by_encoded_key_independent_of_insertion_order() {
        let a = map(vec![(text("b"), uint(2)), (text("a"), uint(1))]);
        let b = map(vec![(text("a"), uint(1)), (text("b"), uint(2))]);
        assert_eq!(a, b);
        // RFC 8949 Appendix A: {"a": 1, "b": [2, 3]}-style ordering check on the head bytes.
        assert_eq!(a, [0xa2, 0x61, 0x61, 0x01, 0x61, 0x62, 0x02]);
    }

    #[test]
    fn shorter_keys_sort_before_longer_keys() {
        let m = map(vec![(text("zz"), uint(0)), (text("a"), uint(0))]);
        assert_eq!(m, [0xa2, 0x61, 0x61, 0x00, 0x62, 0x7a, 0x7a, 0x00]);
    }

    // ---- decoder ----

    const L: Limits = Limits::new(1024, 64, 4096);

    fn h(s: &str) -> Vec<u8> {
        hex::decode(s.replace(' ', "")).unwrap()
    }

    // Hand-built non-canonical / disallowed encodings: each MUST be rejected with the
    // given error. (RFC 8949 §4.2.1 rules plus this crate's profile.)
    #[test]
    fn non_canonical_corpus_is_rejected() {
        let cases: Vec<(&str, &str, CborError)> = vec![
            ("uint 0 as 1-byte", "18 00", CborError::NonShortest),
            ("uint 23 as 1-byte", "18 17", CborError::NonShortest),
            ("uint 255 as 2-byte", "19 00 ff", CborError::NonShortest),
            (
                "uint 65535 as 4-byte",
                "1a 00 00 ff ff",
                CborError::NonShortest,
            ),
            (
                "uint 2^32-1 as 8-byte",
                "1b 00 00 00 00 ff ff ff ff",
                CborError::NonShortest,
            ),
            ("nint -1 as 1-byte", "38 00", CborError::NonShortest),
            ("bstr len 0 as 1-byte", "58 00", CborError::NonShortest),
            (
                "tstr len 5 as 2-byte",
                "79 00 05 6162636465",
                CborError::NonShortest,
            ),
            ("array len 1 as 1-byte", "98 01 01", CborError::NonShortest),
            ("map len 0 as 1-byte", "b8 00", CborError::NonShortest),
            ("indefinite bstr", "5f 41 01 ff", CborError::Indefinite),
            ("indefinite tstr", "7f 61 61 ff", CborError::Indefinite),
            ("indefinite array", "9f 01 ff", CborError::Indefinite),
            ("indefinite map", "bf 61 61 01 ff", CborError::Indefinite),
            ("stray break", "ff", CborError::Indefinite),
            ("reserved ai 28", "1c", CborError::Malformed),
            ("reserved ai 30", "1e", CborError::Malformed),
            ("tag 0", "c0 61 61", CborError::Unsupported),
            ("tag 24", "d8 18 41 00", CborError::Unsupported),
            ("float16", "f9 3c 00", CborError::Unsupported),
            ("float32", "fa 3f 80 00 00", CborError::Unsupported),
            (
                "float64",
                "fb 3f f0 00 00 00 00 00 00",
                CborError::Unsupported,
            ),
            ("undefined", "f7", CborError::Unsupported),
            ("simple 16", "f0", CborError::Unsupported),
            ("simple(32)", "f8 20", CborError::Unsupported),
            ("invalid utf-8", "62 c3 28", CborError::InvalidUtf8),
            ("overlong utf-8", "62 c0 af", CborError::InvalidUtf8),
            (
                "duplicate map key",
                "a2 61 61 01 61 61 02",
                CborError::DuplicateKey,
            ),
            (
                "unsorted map keys",
                "a2 61 62 01 61 61 02",
                CborError::MapOrder,
            ),
            (
                "longer key before shorter",
                "a2 62 61 61 01 61 7a 02",
                CborError::MapOrder,
            ),
            ("trailing byte", "01 00", CborError::TrailingBytes),
            ("two top-level values", "01 02", CborError::TrailingBytes),
            ("truncated bstr", "44 01 02", CborError::Truncated),
            ("truncated head", "19 01", CborError::Truncated),
            ("empty input", "", CborError::Truncated),
            ("truncated map value", "a1 61 61", CborError::Truncated),
        ];
        for (name, hexs, want) in cases {
            assert_eq!(decode(&h(hexs), &L).err(), Some(want), "{name}");
        }
    }

    #[test]
    fn canonical_values_decode() {
        assert_eq!(decode(&h("00"), &L).unwrap(), Value::Uint(0));
        assert_eq!(
            decode(&h("1b ffffffffffffffff"), &L).unwrap(),
            Value::Uint(u64::MAX)
        );
        assert_eq!(decode(&h("20"), &L).unwrap(), Value::Nint(0));
        assert_eq!(decode(&h("f4"), &L).unwrap(), Value::Bool(false));
        assert_eq!(decode(&h("f5"), &L).unwrap(), Value::Bool(true));
        assert_eq!(decode(&h("f6"), &L).unwrap(), Value::Null);
        assert_eq!(decode(&h("60"), &L).unwrap(), Value::text(""));
        assert_eq!(
            decode(&h("83 01 02 03"), &L).unwrap(),
            Value::Array(vec![Value::Uint(1), Value::Uint(2), Value::Uint(3)])
        );
        // Keys ordered by encoded bytes: "a" (61 61) < "b" (61 62) < "aa" (62 61 61).
        let m = decode(&h("a3 6161 01 6162 02 626161 03"), &L).unwrap();
        assert_eq!(
            m,
            Value::Map(vec![
                (Value::text("a"), Value::Uint(1)),
                (Value::text("b"), Value::Uint(2)),
                (Value::text("aa"), Value::Uint(3))
            ])
        );
        // Integer keys sort before text keys (major type 0 < 3).
        assert!(decode(&h("a2 01 01 6161 02"), &L).is_ok());
        assert_eq!(
            decode(&h("a2 6161 02 01 01"), &L).err(),
            Some(CborError::MapOrder)
        );
    }

    #[test]
    fn depth_limit_is_enforced() {
        let nest = |n: usize| {
            let mut v = vec![0x81u8; n];
            v.push(0x00);
            v
        };
        assert!(decode(&nest(DEFAULT_MAX_DEPTH), &L).is_ok());
        assert_eq!(
            decode(&nest(DEFAULT_MAX_DEPTH + 1), &L).err(),
            Some(CborError::TooDeep)
        );
        // A pathological 100k-deep input is rejected without recursion blow-up.
        assert_eq!(decode(&nest(100_000), &L).err(), Some(CborError::TooDeep));
        let mut maps = Vec::new();
        for _ in 0..100_000 {
            maps.extend_from_slice(&[0xa1, 0x00]);
        }
        maps.push(0);
        assert_eq!(decode(&maps, &L).err(), Some(CborError::TooDeep));
        // The encoder refuses too-deep values as well.
        let mut v = Value::Null;
        for _ in 0..(DEFAULT_MAX_DEPTH + 1) {
            v = Value::Array(vec![v]);
        }
        assert_eq!(v.encode().err(), Some(CborError::TooDeep));
    }

    // SEC-Y05: crafted length prefixes are rejected before any allocation of that size.
    #[test]
    fn hostile_length_prefixes_are_rejected_without_allocating() {
        let tiny = Limits::new(16, 4, 64);
        for input in [
            "5b ffffffffffffffff",                    // bstr of 2^64-1 bytes
            "5b 0000000100000000",                    // 4 GiB
            "5a ffffffff",                            // 4 GiB
            "7b 7fffffffffffffff",                    // tstr
            "9b ffffffffffffffff",                    // array of 2^64-1 elements
            "9a ffffffff",                            // 4 G elements
            "bb 7fffffffffffffff",                    // map
            "51 00000000000000000000000000000000 00", // 17 > max_len 16
            "85 00 00 00 00 00",                      // 5 elements > max_items 4
        ] {
            let e = decode(&h(input), &tiny).err();
            assert!(matches!(e, Some(CborError::TooLong)), "{input}: {e:?}");
        }
        // Within the limits but larger than the input: Truncated, not an allocation.
        let big = Limits::new(usize::MAX, usize::MAX, usize::MAX);
        assert_eq!(
            decode(&h("5b 0000000100000000"), &big).err(),
            Some(CborError::Truncated)
        );
        assert_eq!(
            decode(&h("9a ffffffff"), &big).err(),
            Some(CborError::Truncated)
        );
        assert_eq!(
            decode(&h("ba ffffffff"), &big).err(),
            Some(CborError::Truncated)
        );
    }

    #[test]
    fn total_item_budget_is_enforced() {
        let small = Limits::new(16, 64, 5);
        assert!(decode(&h("84 01 02 03 04"), &small).is_ok()); // 5 values
        assert_eq!(
            decode(&h("85 01 02 03 04 05"), &small).err(),
            Some(CborError::TooManyItems)
        );
    }

    #[test]
    fn encode_rejects_duplicate_and_misordered_maps_and_sorts_via_constructor() {
        assert_eq!(
            Value::map(vec![
                (Value::text("a"), Value::Null),
                (Value::text("a"), Value::Null)
            ])
            .err(),
            Some(CborError::DuplicateKey)
        );
        let raw = Value::Map(vec![
            (Value::text("b"), Value::Null),
            (Value::text("a"), Value::Null),
        ]);
        assert_eq!(raw.encode().err(), Some(CborError::MapOrder));
        let raw = Value::Map(vec![
            (Value::text("a"), Value::Null),
            (Value::text("a"), Value::Null),
        ]);
        assert_eq!(raw.encode().err(), Some(CborError::DuplicateKey));
        let sorted = Value::map(vec![
            (Value::text("b"), Value::Uint(1)),
            (Value::text("a"), Value::Uint(2)),
        ])
        .unwrap();
        assert_eq!(sorted.encode().unwrap(), h("a2 6161 02 6162 01"));
    }

    // The new encoder agrees with the raw helpers used for the T01 kdf AAD.
    #[test]
    fn value_encoder_matches_raw_helpers() {
        let v = Value::map(vec![
            (Value::text("a"), Value::Uint(1000)),
            (Value::text("b"), Value::Bytes(vec![1, 2, 3, 4])),
        ])
        .unwrap();
        let raw = map(vec![
            (text("a"), uint(1000)),
            (text("b"), bytes(&[1, 2, 3, 4])),
        ]);
        assert_eq!(v.encode().unwrap(), raw);
    }

    fn arb_value() -> impl Strategy<Value = Value> {
        let leaf = prop_oneof![
            any::<u64>().prop_map(Value::Uint),
            any::<u64>().prop_map(Value::Nint),
            proptest::collection::vec(any::<u8>(), 0..40).prop_map(Value::Bytes),
            ".{0,20}".prop_map(Value::Text),
            any::<bool>().prop_map(Value::Bool),
            Just(Value::Null),
        ];
        leaf.prop_recursive(4, 40, 6, |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..5).prop_map(Value::Array),
                proptest::collection::vec((inner.clone(), inner), 0..5)
                    .prop_filter_map("duplicate keys", |e| Value::map(e).ok()),
            ]
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn prop_decode_encode_round_trip(v in arb_value()) {
            let bytes = v.encode().unwrap();
            let big = Limits::new(1 << 16, 1 << 10, 1 << 16);
            prop_assert_eq!(decode(&bytes, &big).unwrap(), v);
        }

        // Canonicality: whatever the decoder accepts re-encodes to the identical bytes, so
        // no second encoding of a value is ever accepted.
        #[test]
        fn prop_accepted_input_is_exactly_the_canonical_encoding(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
            if let Ok(v) = decode(&bytes, &L) {
                prop_assert_eq!(v.encode().unwrap(), bytes);
            }
        }

        #[test]
        fn prop_decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = decode(&bytes, &L);
        }
    }
}
