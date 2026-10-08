//! Recovery key encoding and parsing (docs/04 §4).
//!
//! A recovery key is 160 random bits. Its text form is the 32 Crockford Base32 characters
//! of those bits followed by a 2-character checksum group:
//! `XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XX-CC` (hyphens are cosmetic).
//!
//! **Checksum (docs/04 §4):** the first 10 bits of
//! `SHA-256("aryavault/rk-check/v1" ‖ 20 key bytes)`, written as two Crockford characters
//! (5 bits each, most significant first). It catches typos; it is *not* a security
//! mechanism and is derived from the key bytes only.
//!
//! The parser is case-insensitive, ignores spaces/tabs/newlines and hyphens, maps the
//! Crockford substitutions `I`/`L` -> `1` and `O` -> `0`, rejects `U` and any other
//! character, and verifies the checksum before returning a key. Errors never echo input.

use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

use crate::keys::{RECOVERY_KEY_LEN, RecoveryKey};
use crate::rng::{Rng, RngError, random_array};

/// Domain-separation prefix of the checksum hash (docs/04 §4).
pub const CHECKSUM_PREFIX: &[u8] = b"aryavault/rk-check/v1";

/// Crockford Base32 alphabet (no `I`, `L`, `O`, `U`).
const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Number of data characters (160 bits / 5).
const DATA_CHARS: usize = 32;
/// Number of checksum characters (10 bits / 5).
const CHECK_CHARS: usize = 2;

/// Errors from parsing a recovery key.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RecoveryKeyError {
    /// Not exactly 34 significant characters (32 key + 2 checksum).
    #[error("recovery key has the wrong length")]
    InvalidLength,
    /// A character outside the Crockford Base32 alphabet (after substitutions).
    #[error("recovery key contains an invalid character")]
    InvalidCharacter,
    /// The checksum group does not match: a typo is likely.
    #[error("recovery key checksum does not match")]
    ChecksumMismatch,
}

/// Generates a new recovery key from `rng` (the OS CSPRNG in production).
pub fn generate(rng: &mut dyn Rng) -> Result<RecoveryKey, RngError> {
    let mut bytes: [u8; RECOVERY_KEY_LEN] = random_array(rng)?;
    let rk = RecoveryKey::from_bytes(bytes);
    bytes.zeroize();
    Ok(rk)
}

fn check_value(key: &[u8; RECOVERY_KEY_LEN]) -> [u8; CHECK_CHARS] {
    let mut h = Sha256::new();
    h.update(CHECKSUM_PREFIX);
    h.update(key);
    let d = h.finalize();
    // First 10 bits: all 8 of d[0] and the top 2 of d[1].
    [d[0] >> 3, ((d[0] & 0b111) << 2) | (d[1] >> 6)]
}

/// Encodes a recovery key for display: 32 key characters in groups of 5 (last key group
/// has 2), then the checksum group, separated by hyphens. The result is wiped on drop.
pub fn encode(rk: &RecoveryKey) -> Zeroizing<String> {
    let key = rk.expose_secret();
    let mut chars = Zeroizing::new(Vec::<u8>::with_capacity(DATA_CHARS));
    for chunk in key.chunks(5) {
        // 5 bytes = 40 bits = 8 characters.
        let mut acc: u64 = 0;
        for b in chunk {
            acc = (acc << 8) | u64::from(*b);
        }
        for i in (0..8).rev() {
            chars.push(ALPHABET[((acc >> (5 * i)) & 0x1f) as usize]);
        }
        acc.zeroize();
    }
    let mut out = Zeroizing::new(String::with_capacity(DATA_CHARS + 8));
    for (i, group) in chars.chunks(5).enumerate() {
        if i > 0 {
            out.push('-');
        }
        out.extend(group.iter().map(|c| char::from(*c)));
    }
    out.push('-');
    for v in check_value(key) {
        out.push(char::from(ALPHABET[usize::from(v)]));
    }
    out
}

fn symbol_value(c: char) -> Option<u8> {
    let up = c.to_ascii_uppercase();
    let up = match up {
        'I' | 'L' => '1',
        'O' => '0',
        other => other,
    };
    if !up.is_ascii() {
        return None;
    }
    ALPHABET
        .iter()
        .position(|a| char::from(*a) == up)
        .map(|p| p as u8)
}

/// Parses user-entered text into a [`RecoveryKey`], verifying the checksum.
///
/// Never panics on arbitrary input (including non-ASCII and very long strings).
pub fn parse(input: &str) -> Result<RecoveryKey, RecoveryKeyError> {
    let mut values = Zeroizing::new(Vec::<u8>::with_capacity(DATA_CHARS + CHECK_CHARS));
    for c in input.chars() {
        if c == '-' || c.is_whitespace() {
            continue;
        }
        if values.len() >= DATA_CHARS + CHECK_CHARS {
            // Too long; stop early so arbitrarily long input stays cheap.
            return Err(RecoveryKeyError::InvalidLength);
        }
        match symbol_value(c) {
            Some(v) => values.push(v),
            None => return Err(RecoveryKeyError::InvalidCharacter),
        }
    }
    if values.len() != DATA_CHARS + CHECK_CHARS {
        return Err(RecoveryKeyError::InvalidLength);
    }

    let mut key = [0u8; RECOVERY_KEY_LEN];
    for (i, group) in values[..DATA_CHARS].chunks(8).enumerate() {
        let mut acc: u64 = 0;
        for v in group {
            acc = (acc << 5) | u64::from(*v);
        }
        let bytes = acc.to_be_bytes();
        key[i * 5..i * 5 + 5].copy_from_slice(&bytes[3..8]);
        acc.zeroize();
    }
    let want = check_value(&key);
    let ok = want.ct_eq(&[values[DATA_CHARS], values[DATA_CHARS + 1]]);
    if !bool::from(ok) {
        key.zeroize();
        return Err(RecoveryKeyError::ChecksumMismatch);
    }
    let rk = RecoveryKey::from_bytes(key);
    key.zeroize();
    Ok(rk)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::OsRng;
    use proptest::prelude::*;

    // Independent reference vectors, computed with a separate Python implementation
    // (hashlib.sha256 + a straight-line base32 encoder), not with this crate:
    //   key = 00 x20           (20 zero bytes)
    //   key = 00 01 02 .. 13   (bytes 0..=19)
    const ZERO_ENCODED: &str = "00000-00000-00000-00000-00000-00000-00-MH";
    const SEQ_ENCODED: &str = "000G4-0R40M-30E20-9185G-R38E1-W8124-GK-EY";

    fn seq_key() -> RecoveryKey {
        let mut b = [0u8; 20];
        for (i, v) in b.iter_mut().enumerate() {
            *v = i as u8;
        }
        RecoveryKey::from_bytes(b)
    }

    #[test]
    fn encode_matches_independent_reference_vectors() {
        assert_eq!(
            encode(&RecoveryKey::from_bytes([0; 20])).as_str(),
            ZERO_ENCODED
        );
        assert_eq!(encode(&seq_key()).as_str(), SEQ_ENCODED);
    }

    #[test]
    fn parse_reference_vectors() {
        assert_eq!(parse(ZERO_ENCODED).unwrap().expose_secret(), &[0u8; 20]);
        assert_eq!(
            parse(SEQ_ENCODED).unwrap().expose_secret(),
            seq_key().expose_secret()
        );
    }

    #[test]
    fn format_is_six_groups_of_five_then_two_then_checksum() {
        let s = encode(&seq_key());
        let groups: Vec<&str> = s.split('-').collect();
        let lens: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        assert_eq!(lens, [5, 5, 5, 5, 5, 5, 2, 2]);
    }

    #[test]
    fn parser_tolerates_case_spaces_hyphens_and_substitutions() {
        let canon = encode(&seq_key());
        let lower = canon.to_lowercase();
        assert_eq!(
            parse(&lower).unwrap().expose_secret(),
            seq_key().expose_secret()
        );
        let spaced = canon.replace('-', "  \t");
        assert!(parse(&spaced).is_ok());
        let none = canon.replace('-', "");
        assert!(parse(&none).is_ok());
        let padded = format!("  {}\n", canon.as_str());
        assert!(parse(&padded).is_ok());
        // Crockford substitutions on the all-zero key: 'O' -> '0'; and 'I'/'L' -> '1'
        // (checked on a key whose encoding contains a '1').
        let zero = encode(&RecoveryKey::from_bytes([0; 20]));
        let o_form = zero.replacen('0', "O", 5);
        assert_eq!(parse(&o_form).unwrap().expose_secret(), &[0u8; 20]);
        let with_one = encode(&RecoveryKey::from_bytes([
            0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]));
        assert!(with_one.contains('1'));
        for sub in ["I", "i", "L", "l"] {
            let s = with_one.replacen('1', sub, 1);
            assert_eq!(
                parse(&s).unwrap().expose_secret(),
                &[
                    0x08, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
                ],
                "{sub}"
            );
        }
    }

    #[test]
    fn rejects_bad_checksum_with_typed_error() {
        let good = encode(&seq_key());
        // Change the last character to a different valid one.
        let mut chars: Vec<char> = good.chars().collect();
        let last = chars.len() - 1;
        chars[last] = if chars[last] == '0' { '1' } else { '0' };
        let bad: String = chars.into_iter().collect();
        assert_eq!(
            parse(&bad).err().unwrap(),
            RecoveryKeyError::ChecksumMismatch
        );
    }

    #[test]
    fn single_character_typo_in_key_is_detected() {
        let good = encode(&seq_key());
        let chars: Vec<char> = good.chars().collect();
        let mut detected = 0;
        let mut total = 0;
        for i in 0..chars.len() {
            if chars[i] == '-' {
                continue;
            }
            total += 1;
            let mut c = chars.clone();
            c[i] = if c[i] == 'Z' { 'Y' } else { 'Z' };
            let s: String = c.into_iter().collect();
            if parse(&s).is_err() {
                detected += 1;
            }
        }
        // A 10-bit checksum can miss ~1/1024 of random corruptions; for these 34
        // single-character edits of a fixed key we require all to be detected.
        assert_eq!(detected, total);
    }

    #[test]
    fn rejects_wrong_length_and_invalid_characters() {
        let good = encode(&seq_key()).to_string();
        assert_eq!(parse("").err().unwrap(), RecoveryKeyError::InvalidLength);
        assert_eq!(
            parse(&good[..good.len() - 1]).err().unwrap(),
            RecoveryKeyError::InvalidLength
        );
        assert_eq!(
            parse(&format!("{good}0")).err().unwrap(),
            RecoveryKeyError::InvalidLength
        );
        assert_eq!(
            parse(&good.replacen('0', "U", 1)).err().unwrap(),
            RecoveryKeyError::InvalidCharacter
        );
        assert_eq!(
            parse(&good.replacen('0', "!", 1)).err().unwrap(),
            RecoveryKeyError::InvalidCharacter
        );
        assert_eq!(
            parse(&good.replacen('0', "\u{ff10}", 1)).err().unwrap(),
            RecoveryKeyError::InvalidCharacter
        );
        assert_eq!(
            parse(&good.replacen('0', "\u{0130}", 1)).err().unwrap(),
            RecoveryKeyError::InvalidCharacter
        );
    }

    #[test]
    fn error_messages_do_not_echo_input() {
        let secret = "CANARY-SECRET-INPUT";
        let msg = parse(secret).err().unwrap().to_string();
        assert!(!msg.contains(secret) && !msg.contains("CANARY"));
    }

    #[test]
    fn generate_produces_distinct_valid_keys() {
        let a = generate(&mut OsRng).unwrap();
        let b = generate(&mut OsRng).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
        assert_eq!(
            parse(&encode(&a)).unwrap().expose_secret(),
            a.expose_secret()
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn prop_encode_decode_round_trip(bytes in proptest::array::uniform20(any::<u8>())) {
            let rk = RecoveryKey::from_bytes(bytes);
            let text = encode(&rk);
            prop_assert_eq!(text.len(), 32 + 7 + 2); // 32 key chars, 7 hyphens, 2 check chars
            let back = parse(&text).unwrap();
            prop_assert_eq!(back.expose_secret(), &bytes);
        }

        #[test]
        fn prop_parser_never_panics_on_arbitrary_strings(s in ".*") {
            let _ = parse(&s);
        }

        #[test]
        fn prop_parser_never_panics_on_alphabet_soup(s in "[0-9A-Za-z \\-_!\u{e9}\u{ff10}]{0,80}") {
            let _ = parse(&s);
        }
    }
}
