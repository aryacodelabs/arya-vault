//! EFF large wordlist passphrases (docs/04 section 12).

use zeroize::Zeroizing;

use crate::error::{GeneratorError, OptionsError};
use crate::rng::{RandomSource, uniform_index};
use crate::wordlist::{WORDLIST_LEN, wordlist};

/// Minimum number of words.
pub const MIN_WORDS: usize = 3;
/// Maximum number of words.
pub const MAX_WORDS: usize = 12;
const MAX_SEPARATOR_CHARS: usize = 16;

/// Options for [`generate_passphrase`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassphraseOptions {
    /// Number of words, `MIN_WORDS..=MAX_WORDS` (default 6).
    pub word_count: usize,
    /// Text between words (at most 16 characters, no control characters).
    pub separator: String,
    /// Uppercase the first letter of every word.
    pub capitalize: bool,
    /// Append one random digit to one randomly chosen word.
    pub number_suffix: bool,
}

impl Default for PassphraseOptions {
    fn default() -> Self {
        Self {
            word_count: 6,
            separator: "-".to_owned(),
            capitalize: false,
            number_suffix: false,
        }
    }
}

impl PassphraseOptions {
    /// Validate the options.
    ///
    /// # Errors
    /// Returns an [`OptionsError`] for an out-of-range word count or bad separator.
    pub fn validate(&self) -> Result<(), OptionsError> {
        if !(MIN_WORDS..=MAX_WORDS).contains(&self.word_count) {
            return Err(OptionsError::WordCountOutOfRange(
                self.word_count,
                MIN_WORDS,
                MAX_WORDS,
            ));
        }
        if self.separator.chars().count() > MAX_SEPARATOR_CHARS
            || self.separator.chars().any(char::is_control)
        {
            return Err(OptionsError::InvalidSeparator);
        }
        Ok(())
    }
}

/// Generate a passphrase from the EFF large wordlist.
///
/// # Errors
/// Invalid options, RNG failure, or a corrupt embedded wordlist.
pub fn generate_passphrase<R: RandomSource + ?Sized>(
    opts: &PassphraseOptions,
    rng: &mut R,
) -> Result<Zeroizing<String>, GeneratorError> {
    opts.validate()?;
    let words = wordlist();
    if words.len() != WORDLIST_LEN {
        return Err(GeneratorError::WordlistCorrupt);
    }
    // Word indices are the secret; keep them in a zeroizing buffer.
    let mut picked: Zeroizing<Vec<usize>> = Zeroizing::new(Vec::with_capacity(opts.word_count));
    for _ in 0..opts.word_count {
        picked.push(uniform_index(rng, WORDLIST_LEN)?);
    }
    let (suffix_word, suffix_digit) = if opts.number_suffix {
        (
            uniform_index(rng, opts.word_count)?,
            u8::try_from(crate::rng::uniform_below(rng, 10)?).map_err(|_| GeneratorError::Rng)?,
        )
    } else {
        (usize::MAX, 0)
    };
    let cap: usize = picked.iter().map(|&i| words[i].len()).sum::<usize>()
        + opts.separator.len() * opts.word_count
        + 1;
    let mut out = Zeroizing::new(String::with_capacity(cap));
    for (i, &idx) in picked.iter().enumerate() {
        let word = words[idx];
        if i > 0 {
            out.push_str(&opts.separator);
        }
        let mut chars = word.chars();
        match (opts.capitalize, chars.next()) {
            (true, Some(first)) => {
                out.push(first.to_ascii_uppercase());
                out.push_str(chars.as_str());
            }
            _ => out.push_str(word),
        }
        if i == suffix_word {
            out.push(char::from(b'0' + suffix_digit));
        }
    }
    Ok(out)
}

/// Entropy in bits: `word_count * log2(7776)`, plus `log2(10)` for the number
/// suffix. The suffix position is conservatively not counted.
///
/// # Errors
/// Returns an [`OptionsError`] if the options are invalid.
#[allow(clippy::cast_precision_loss)]
pub fn entropy_bits_passphrase(opts: &PassphraseOptions) -> Result<f64, OptionsError> {
    opts.validate()?;
    let mut bits = opts.word_count as f64 * (WORDLIST_LEN as f64).log2();
    if opts.number_suffix {
        bits += 10f64.log2();
    }
    Ok(bits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SeededRandom;

    #[test]
    fn defaults_and_entropy() {
        let o = PassphraseOptions::default();
        assert_eq!(o.word_count, 6);
        let bits = entropy_bits_passphrase(&o).unwrap();
        assert!((bits - 6.0 * 12.924_812_503_605_78).abs() < 1e-6);
        let with = PassphraseOptions {
            number_suffix: true,
            ..o
        };
        assert!((entropy_bits_passphrase(&with).unwrap() - bits - 10f64.log2()).abs() < 1e-9);
    }

    #[test]
    fn word_count_bounds() {
        for bad in [0, 2, 13, 100] {
            let o = PassphraseOptions {
                word_count: bad,
                ..PassphraseOptions::default()
            };
            assert!(matches!(
                o.validate(),
                Err(OptionsError::WordCountOutOfRange(..))
            ));
        }
        for ok in [3, 12] {
            let o = PassphraseOptions {
                word_count: ok,
                ..PassphraseOptions::default()
            };
            assert!(o.validate().is_ok());
        }
    }

    #[test]
    fn separator_validation() {
        let mut o = PassphraseOptions {
            separator: "a\nb".into(),
            ..PassphraseOptions::default()
        };
        assert_eq!(o.validate(), Err(OptionsError::InvalidSeparator));
        o.separator = "x".repeat(17);
        assert_eq!(o.validate(), Err(OptionsError::InvalidSeparator));
        o.separator = String::new();
        assert!(o.validate().is_ok());
    }

    #[test]
    fn format_separator_capitalize_suffix() {
        let o = PassphraseOptions {
            word_count: 5,
            separator: " ".into(),
            capitalize: true,
            number_suffix: true,
        };
        let p = generate_passphrase(&o, &mut SeededRandom::new(1)).unwrap();
        let parts: Vec<&str> = p.split(' ').collect();
        assert_eq!(parts.len(), 5);
        let mut digits = 0;
        for part in parts {
            assert!(part.chars().next().unwrap().is_ascii_uppercase());
            let bare = part.trim_end_matches(|c: char| c.is_ascii_digit());
            digits += part.len() - bare.len();
            assert!(wordlist().contains(&bare.to_ascii_lowercase().as_str()));
        }
        assert_eq!(digits, 1);
    }

    #[test]
    fn plain_words_come_from_list() {
        let o = PassphraseOptions {
            separator: "/".into(),
            ..PassphraseOptions::default()
        };
        let p = generate_passphrase(&o, &mut SeededRandom::new(2)).unwrap();
        assert_eq!(p.split('/').count(), 6);
        for w in p.split('/') {
            assert!(wordlist().contains(&w));
        }
    }
}
