//! The keyset cursor of `list` (docs/14 section 4.2.1): a position in the list order
//! (`updated_hlc` descending, then `id`), as an opaque string.
//!
//! Format: `v1.` + the item's `updated_hlc` as 16 lowercase hex digits (the `i64` bit pattern) +
//! `.` + the item id as 32 lowercase hex digits. The app never parses it. Decoding is total: any
//! other input, of any length, is `None`.

use arya_vault_storage::Id;

const PREFIX: &str = "v1.";
/// `v1.` + 16 + `.` + 32.
pub const CURSOR_LEN: usize = 3 + 16 + 1 + 32;

/// A position in the list order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListCursor {
    /// `updated_hlc` of the row the cursor names.
    pub hlc: i64,
    /// Its id (breaks ties between equal timestamps).
    pub id: Id,
}

impl ListCursor {
    /// The opaque string for this position.
    #[must_use]
    pub fn encode(&self) -> String {
        let mut s = String::with_capacity(CURSOR_LEN);
        s.push_str(PREFIX);
        s.push_str(&format!("{:016x}", self.hlc as u64));
        s.push('.');
        for b in self.id {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    /// Parses a cursor; `None` for anything this build did not produce.
    #[must_use]
    pub fn decode(text: &str) -> Option<Self> {
        if text.len() != CURSOR_LEN || !text.is_ascii() {
            return None;
        }
        let rest = text.strip_prefix(PREFIX)?;
        let (hlc_hex, id_hex) = rest.split_once('.')?;
        if hlc_hex.len() != 16 || id_hex.len() != 32 {
            return None;
        }
        let hlc = u64::from_str_radix(&lower_hex(hlc_hex)?, 16).ok()? as i64;
        let mut id = [0u8; 16];
        for (i, out) in id.iter_mut().enumerate() {
            *out = u8::from_str_radix(&lower_hex(&id_hex[2 * i..2 * i + 2])?, 16).ok()?;
        }
        Some(Self { hlc, id })
    }

    /// Whether a row at (`hlc`, `id`) comes strictly after this position in the list order.
    #[must_use]
    pub fn is_before(&self, hlc: i64, id: &Id) -> bool {
        hlc < self.hlc || (hlc == self.hlc && id > &self.id)
    }
}

/// `Some(text)` only if every character is a lowercase hex digit (no sign, no upper case).
fn lower_hex(s: &str) -> Option<String> {
    s.bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        .then(|| s.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn round_trip_and_fixed_length() {
        let c = ListCursor {
            hlc: -5,
            id: [0xAB; 16],
        };
        let s = c.encode();
        assert_eq!(s.len(), CURSOR_LEN);
        assert_eq!(ListCursor::decode(&s), Some(c));
    }

    #[test]
    fn rejects_everything_else() {
        let good = ListCursor {
            hlc: 7,
            id: [1; 16],
        }
        .encode();
        for bad in [
            "",
            "v1.",
            "v2.0000000000000007.01010101010101010101010101010101",
            &good.to_uppercase(),
            &good.replace('.', ":"),
            &format!("{good}0"),
            &good[..good.len() - 1],
            "v1.+000000000000007.01010101010101010101010101010101",
            "v1.0000000000000007.0101010101010101010101010101010g",
            "v1.000000000000000é.0101010101010101010101010101010",
        ] {
            assert_eq!(ListCursor::decode(bad), None, "{bad}");
        }
    }

    proptest! {
        #[test]
        fn decode_never_panics_and_encode_is_its_inverse(s in ".*", hlc in any::<i64>(), id in any::<[u8; 16]>()) {
            let _ = ListCursor::decode(&s);
            let c = ListCursor { hlc, id };
            prop_assert_eq!(ListCursor::decode(&c.encode()), Some(c));
        }
    }

    #[test]
    fn ordering_matches_the_list_order() {
        let c = ListCursor {
            hlc: 10,
            id: [5; 16],
        };
        assert!(c.is_before(9, &[0; 16]), "older rows come after");
        assert!(
            c.is_before(10, &[6; 16]),
            "same time, larger id comes after"
        );
        assert!(!c.is_before(10, &[5; 16]), "the row itself is not after");
        assert!(!c.is_before(10, &[4; 16]));
        assert!(!c.is_before(11, &[9; 16]), "newer rows come before");
    }
}
