//! Master-password normalization (docs/04 §3, SEC-C10).
//!
//! Unicode NFKD, encoded as UTF-8. **No trimming** and no case folding: whitespace is
//! part of the password. The `unicode-normalization` crate is pinned to an exact version
//! in `Cargo.toml`; the tests below fail if a dependency update changes the Unicode
//! tables or any vector, because a change would lock users out of correct passwords.

use unicode_normalization::UnicodeNormalization;
use zeroize::Zeroizing;

/// Upper bound (in bytes) of the capacity reserved up front so that normalization does
/// not reallocate (a reallocation would leave an unzeroized copy of the password in
/// freed memory). Inputs whose worst-case expansion exceeds this fall back to normal
/// growth; this is a best-effort hygiene measure, not a security boundary.
const MAX_PREALLOC: usize = 1 << 20;

/// Worst-case NFKD growth factor in UTF-8 bytes (U+FDFA expands to 18 characters of
/// 2 bytes each from 3 bytes, i.e. 12x; 18 leaves margin).
const EXPANSION_BOUND: usize = 18;

/// Normalizes a master password to NFKD and returns its UTF-8 bytes in a buffer that is
/// wiped on drop.
///
/// The input is not trimmed or otherwise altered beyond NFKD.
pub fn normalize_password(password: &str) -> Zeroizing<String> {
    let cap = password
        .len()
        .saturating_mul(EXPANSION_BOUND)
        .min(MAX_PREALLOC);
    let mut out = Zeroizing::new(String::with_capacity(cap));
    out.extend(password.nfkd());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nfkd_hex(s: &str) -> String {
        hex::encode(normalize_password(s).as_bytes())
    }

    // SEC-C10: the Unicode tables behind the pinned crate version. If this fails after a
    // dependency bump, STOP: normalization output may have changed (see module docs).
    #[test]
    fn sec_c10_unicode_version_is_pinned() {
        assert_eq!(unicode_normalization::UNICODE_VERSION, (17, 0, 0));
    }

    // SEC-C10 vectors. Expected values were derived by hand from the Unicode
    // decomposition mappings (UnicodeData.txt) and are written as code points / bytes.
    #[test]
    fn sec_c10_composed_accent_equals_decomposed() {
        // "é" U+00E9 -> "e" U+0065 + U+0301 (combining acute)
        assert_eq!(nfkd_hex("\u{e9}"), "65cc81");
        assert_eq!(nfkd_hex("e\u{301}"), "65cc81");
        assert_eq!(
            normalize_password("caf\u{e9}"),
            normalize_password("cafe\u{301}")
        );
    }

    #[test]
    fn sec_c10_hangul_syllable_decomposes_to_jamo() {
        // "한" U+D55C -> U+1112 U+1161 U+11AB
        assert_eq!(nfkd_hex("\u{d55c}"), "e18492e185a1e186ab");
    }

    #[test]
    fn sec_c10_fullwidth_folds_to_ascii() {
        // "Ａ" U+FF21 -> "A"; "１" U+FF11 -> "1"; ideographic space U+3000 -> " "
        assert_eq!(nfkd_hex("\u{ff21}\u{ff11}\u{3000}"), "413120");
    }

    #[test]
    fn sec_c10_compatibility_ligature_and_circled_digit() {
        // "ﬁ" U+FB01 -> "fi"; "①" U+2460 -> "1"
        assert_eq!(nfkd_hex("\u{fb01}\u{2460}"), "666931");
    }

    #[test]
    fn sec_c10_emoji_is_unchanged() {
        // U+1F600 and a ZWJ sequence are stable under NFKD.
        assert_eq!(nfkd_hex("\u{1f600}"), "f09f9880");
        let zwj = "\u{1f469}\u{200d}\u{1f4bb}";
        assert_eq!(normalize_password(zwj).as_str(), zwj);
    }

    #[test]
    fn no_trimming_whitespace_is_significant() {
        assert_eq!(normalize_password("  pw  ").as_str(), "  pw  ");
        assert_eq!(normalize_password("\tpw\n").as_str(), "\tpw\n");
    }

    #[test]
    fn empty_and_ascii_pass_through() {
        assert_eq!(normalize_password("").as_str(), "");
        assert_eq!(
            normalize_password("CANARY-pass word").as_str(),
            "CANARY-pass word"
        );
    }

    #[test]
    fn worst_case_expansion_does_not_exceed_prealloc_bound() {
        // U+FDFA expands to 18 characters; ensure the reserve covers it (no realloc).
        let s = normalize_password("\u{fdfa}");
        assert!(s.len() <= "\u{fdfa}".len() * EXPANSION_BOUND);
        assert_eq!(s.chars().count(), 18);
    }
}
