//! Embedded EFF large wordlist (CC BY 3.0 US; see `data/ATTRIBUTION.md`).

use std::sync::OnceLock;

/// Number of words in the EFF large wordlist (6^5 dice rolls).
pub const WORDLIST_LEN: usize = 7776;

const RAW: &str = include_str!("../data/eff_large_wordlist.txt");

static WORDS: OnceLock<Vec<&'static str>> = OnceLock::new();

/// The 7776 words in canonical (dice-roll) order. Empty only if the embedded
/// data is malformed, which the unit tests rule out.
#[must_use]
pub fn wordlist() -> &'static [&'static str] {
    WORDS.get_or_init(|| {
        let mut words = Vec::with_capacity(WORDLIST_LEN);
        for line in RAW.lines() {
            match line.split_once('\t') {
                Some((_, w)) if !w.is_empty() => words.push(w),
                _ => return Vec::new(),
            }
        }
        if words.len() == WORDLIST_LEN {
            words
        } else {
            Vec::new()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn exactly_7776_unique_words_in_expected_format() {
        let mut keys = Vec::new();
        for a in 1..=6 {
            for b in 1..=6 {
                for c in 1..=6 {
                    for d in 1..=6 {
                        for e in 1..=6 {
                            keys.push(format!("{a}{b}{c}{d}{e}"));
                        }
                    }
                }
            }
        }
        let lines: Vec<&str> = RAW.lines().collect();
        assert_eq!(lines.len(), WORDLIST_LEN);
        let mut seen = HashSet::new();
        for (line, key) in lines.iter().zip(&keys) {
            let (k, w) = line.split_once('\t').expect("tab separated");
            assert_eq!(k, key, "dice keys must be sequential");
            assert!(!w.is_empty() && w.is_ascii());
            assert!(w.bytes().all(|b| b.is_ascii_lowercase() || b == b'-'));
            assert!(seen.insert(w), "duplicate word {w}");
        }
        assert_eq!(seen.len(), WORDLIST_LEN);
        assert_eq!(wordlist().len(), WORDLIST_LEN);
        assert_eq!(wordlist()[0], "abacus");
        assert_eq!(wordlist()[WORDLIST_LEN - 1], "zoom");
    }
}
