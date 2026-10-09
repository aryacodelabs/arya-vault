//! Minimal deterministic CBOR encoder (RFC 8949 §4.2.1).
//!
//! Implements only what the crate needs for AAD construction: unsigned integers, byte
//! strings, text strings and maps, following the *core deterministic encoding*
//! requirements of RFC 8949 §4.2.1: shortest-form integer/length heads, definite lengths
//! only, and map entries sorted by the bytewise lexicographic order of their encoded
//! keys. The doc 04 §5 `kdf` struct is the only caller today.
//!
//! Design choice (recorded in the PR): a ~60-line encoder instead of `ciborium`, because
//! `ciborium` serializes structs/maps in insertion order and offers no canonical mode, so
//! determinism would have to be re-implemented on top of it anyway; this avoids a
//! dependency on a security-critical byte format.

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
}
