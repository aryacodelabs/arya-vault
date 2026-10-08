//! Character-class password generation (docs/04 section 12, SEC-C09).

use zeroize::Zeroizing;

use crate::error::{GeneratorError, OptionsError};
use crate::rng::{RandomSource, shuffle, uniform_index};

/// Minimum password length.
pub const MIN_LENGTH: usize = 8;
/// Maximum password length.
pub const MAX_LENGTH: usize = 128;
/// Characters removed by `exclude_ambiguous`: docs/04 lists `Il1O0`; `o` and
/// `|` are added as visually similar (see PR "Spec questions").
pub const AMBIGUOUS_CHARS: &str = "Il1O0o|";
/// Default symbol set.
pub const DEFAULT_SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>?/~";

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";

/// Options for [`generate_password`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordOptions {
    /// Length in characters, `MIN_LENGTH..=MAX_LENGTH`.
    pub length: usize,
    /// Include `a-z`.
    pub lower: bool,
    /// Include `A-Z`.
    pub upper: bool,
    /// Include `0-9`.
    pub digits: bool,
    /// Include symbols from `symbol_set`.
    pub symbols: bool,
    /// Symbol characters (printable ASCII punctuation); used when `symbols` is true.
    pub symbol_set: String,
    /// Drop visually ambiguous characters ([`AMBIGUOUS_CHARS`]).
    pub exclude_ambiguous: bool,
    /// Guarantee at least one character from every enabled class.
    pub require_each_class: bool,
}

impl Default for PasswordOptions {
    fn default() -> Self {
        Self {
            length: 20,
            lower: true,
            upper: true,
            digits: true,
            symbols: true,
            symbol_set: DEFAULT_SYMBOLS.to_owned(),
            exclude_ambiguous: false,
            require_each_class: true,
        }
    }
}

impl PasswordOptions {
    /// Validate the options without generating anything.
    ///
    /// # Errors
    /// Returns an [`OptionsError`] describing the first problem found.
    pub fn validate(&self) -> Result<(), OptionsError> {
        self.classes().map(|_| ())
    }

    /// Resolve enabled classes (ASCII bytes, deduplicated, exclusions applied).
    fn classes(&self) -> Result<Vec<Vec<u8>>, OptionsError> {
        if !(MIN_LENGTH..=MAX_LENGTH).contains(&self.length) {
            return Err(OptionsError::LengthOutOfRange(
                self.length,
                MIN_LENGTH,
                MAX_LENGTH,
            ));
        }
        let mut out: Vec<Vec<u8>> = Vec::new();
        let mut add = |enabled: bool, set: &str| -> Result<(), OptionsError> {
            if !enabled {
                return Ok(());
            }
            let mut chars: Vec<u8> = Vec::new();
            for b in set.bytes() {
                if self.exclude_ambiguous && AMBIGUOUS_CHARS.as_bytes().contains(&b) {
                    continue;
                }
                if !chars.contains(&b) {
                    chars.push(b);
                }
            }
            if chars.is_empty() {
                return Err(OptionsError::EmptyClass);
            }
            out.push(chars);
            Ok(())
        };
        add(self.lower, LOWER)?;
        add(self.upper, UPPER)?;
        add(self.digits, DIGITS)?;
        if self.symbols {
            if self.symbol_set.is_empty() {
                return Err(OptionsError::EmptySymbolSet);
            }
            if !self.symbol_set.bytes().all(|b| b.is_ascii_punctuation()) {
                return Err(OptionsError::InvalidSymbolSet);
            }
            let set = self.symbol_set.clone();
            add(true, &set).map_err(|_| OptionsError::EmptySymbolSet)?;
        }
        if out.is_empty() {
            return Err(OptionsError::NoClassEnabled);
        }
        if self.require_each_class && self.length < out.len() {
            return Err(OptionsError::LengthBelowClasses {
                length: self.length,
                classes: out.len(),
            });
        }
        Ok(out)
    }
}

/// Generate a password.
///
/// Characters are drawn with rejection sampling. With `require_each_class`
/// one character per enabled class is placed first, the rest are drawn from
/// the full alphabet, and the whole buffer is Fisher-Yates shuffled with the
/// same unbiased sampler, so placement positions are uniform.
///
/// # Errors
/// [`GeneratorError::Options`] for invalid options, [`GeneratorError::Rng`] if
/// the random source fails.
pub fn generate_password<R: RandomSource + ?Sized>(
    opts: &PasswordOptions,
    rng: &mut R,
) -> Result<Zeroizing<String>, GeneratorError> {
    let classes: Zeroizing<Vec<Vec<u8>>> = Zeroizing::new(opts.classes()?);
    let alphabet: Zeroizing<Vec<u8>> = Zeroizing::new(classes.concat());
    let mut buf: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::with_capacity(opts.length));
    if opts.require_each_class {
        for class in classes.iter() {
            buf.push(class[uniform_index(rng, class.len())?]);
        }
    }
    while buf.len() < opts.length {
        buf.push(alphabet[uniform_index(rng, alphabet.len())?]);
    }
    shuffle(rng, &mut buf)?;
    let mut out = Zeroizing::new(String::with_capacity(opts.length));
    for &b in buf.iter() {
        out.push(char::from(b));
    }
    Ok(out)
}

/// Entropy estimate in bits for the configured generator.
///
/// Without `require_each_class` this is exactly `length * log2(alphabet)`.
/// With it, the result is a documented **conservative lower bound** (a
/// min-entropy bound on the placement procedure):
/// `(length - k) * log2(N) + sum(log2(class size))` for `k` enabled classes
/// and alphabet size `N`. It ignores the extra uncertainty from the random
/// positions, because distinct placements can yield the same string.
///
/// # Errors
/// Returns an [`OptionsError`] if the options are invalid.
#[allow(clippy::cast_precision_loss)]
pub fn entropy_bits(opts: &PasswordOptions) -> Result<f64, OptionsError> {
    let classes = opts.classes()?;
    let n: usize = classes.iter().map(Vec::len).sum();
    let log_n = (n as f64).log2();
    if !opts.require_each_class {
        return Ok(opts.length as f64 * log_n);
    }
    let k = classes.len();
    let fixed: f64 = classes.iter().map(|c| (c.len() as f64).log2()).sum();
    Ok((opts.length - k) as f64 * log_n + fixed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{OsRandom, SeededRandom};
    use proptest::prelude::*;

    #[test]
    fn boundary_validation() {
        let with = |length| PasswordOptions {
            length,
            ..PasswordOptions::default()
        };
        assert!(matches!(
            with(MIN_LENGTH - 1).validate(),
            Err(OptionsError::LengthOutOfRange(..))
        ));
        assert!(matches!(
            with(MAX_LENGTH + 1).validate(),
            Err(OptionsError::LengthOutOfRange(..))
        ));
        assert!(with(MIN_LENGTH).validate().is_ok());
        assert!(with(MAX_LENGTH).validate().is_ok());
    }

    #[test]
    fn no_class_enabled_rejected() {
        let o = PasswordOptions {
            lower: false,
            upper: false,
            digits: false,
            symbols: false,
            ..PasswordOptions::default()
        };
        assert_eq!(o.validate(), Err(OptionsError::NoClassEnabled));
    }

    #[test]
    fn symbol_set_conflicts() {
        let mut o = PasswordOptions {
            symbol_set: String::new(),
            ..PasswordOptions::default()
        };
        assert_eq!(o.validate(), Err(OptionsError::EmptySymbolSet));
        o.symbol_set = "|".into();
        o.exclude_ambiguous = true; // only symbol is ambiguous -> empty class
        assert_eq!(o.validate(), Err(OptionsError::EmptySymbolSet));
        o.exclude_ambiguous = false;
        o.symbol_set = "ab".into();
        assert_eq!(o.validate(), Err(OptionsError::InvalidSymbolSet));
        o.symbol_set = "é".into();
        assert_eq!(o.validate(), Err(OptionsError::InvalidSymbolSet));
    }

    #[test]
    fn digits_only_ambiguous_digits_still_nonempty() {
        let o = PasswordOptions {
            lower: false,
            upper: false,
            symbols: false,
            exclude_ambiguous: true,
            ..PasswordOptions::default()
        };
        assert!(o.validate().is_ok());
    }

    #[test]
    fn length_below_classes_is_unreachable_by_range_check() {
        // There are at most 4 classes and MIN_LENGTH is 8, so the range check
        // always fires first; `LengthBelowClasses` is kept as defence in depth
        // in case the bounds are ever loosened.
        const { assert!(MIN_LENGTH >= 4) };
        let o = PasswordOptions {
            length: 3,
            ..PasswordOptions::default()
        };
        assert!(matches!(
            o.validate(),
            Err(OptionsError::LengthOutOfRange(3, ..))
        ));
    }

    #[test]
    fn deterministic_with_seed() {
        let o = PasswordOptions::default();
        let a = generate_password(&o, &mut SeededRandom::new(7)).unwrap();
        let b = generate_password(&o, &mut SeededRandom::new(7)).unwrap();
        let c = generate_password(&o, &mut SeededRandom::new(8)).unwrap();
        assert_eq!(*a, *b);
        assert_ne!(*a, *c);
        assert_eq!(a.len(), o.length);
    }

    #[test]
    fn os_source_works() {
        let o = PasswordOptions::default();
        let p = generate_password(&o, &mut OsRandom).unwrap();
        assert_eq!(p.len(), 20);
    }

    #[test]
    fn output_type_is_zeroizing() {
        fn takes(_: Zeroizing<String>) {}
        takes(generate_password(&PasswordOptions::default(), &mut OsRandom).unwrap());
    }

    #[test]
    fn entropy_exact_without_constraints() {
        let o = PasswordOptions {
            length: 16,
            lower: true,
            upper: false,
            digits: false,
            symbols: false,
            require_each_class: false,
            ..PasswordOptions::default()
        };
        let want = 16.0 * 26f64.log2();
        assert!((entropy_bits(&o).unwrap() - want).abs() < 1e-9);
    }

    /// log2 of the count of strings with >= 1 char of each class (inclusion-exclusion).
    fn log2_valid_count(sizes: &[usize], len: u32) -> f64 {
        let n: usize = sizes.iter().sum();
        let k = sizes.len();
        let mut total = 0f64;
        for mask in 0..(1u32 << k) {
            let removed: usize = (0..k)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| sizes[i])
                .sum();
            let term = ((n - removed) as f64).powi(len as i32);
            if mask.count_ones() % 2 == 0 {
                total += term
            } else {
                total -= term
            }
        }
        total.log2()
    }

    #[test]
    fn constrained_entropy_is_conservative_bound() {
        for length in [8usize, 12, 20, 64, 128] {
            let o = PasswordOptions {
                length,
                ..PasswordOptions::default()
            };
            let bound = entropy_bits(&o).unwrap();
            let sizes: Vec<usize> = o.classes().unwrap().iter().map(Vec::len).collect();
            let valid = log2_valid_count(&sizes, length as u32);
            let n: usize = sizes.iter().sum();
            assert!(bound <= valid + 1e-9, "bound {bound} > log2(valid) {valid}");
            assert!(valid <= length as f64 * (n as f64).log2() + 1e-9);
            assert!(bound > 0.0);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(10_000))]

        #[test]
        fn classes_present_and_exclusions_respected(
            seed in any::<u64>(),
            length in MIN_LENGTH..=MAX_LENGTH,
            lower in any::<bool>(), upper in any::<bool>(),
            digits in any::<bool>(), symbols in any::<bool>(),
            exclude_ambiguous in any::<bool>(),
            require_each_class in any::<bool>(),
        ) {
            let o = PasswordOptions {
                length, lower, upper, digits, symbols,
                exclude_ambiguous, require_each_class,
                ..PasswordOptions::default()
            };
            match generate_password(&o, &mut SeededRandom::new(seed)) {
                Err(GeneratorError::Options(OptionsError::NoClassEnabled)) => {
                    prop_assert!(!(lower || upper || digits || symbols));
                }
                Err(e) => prop_assert!(false, "unexpected error {e:?}"),
                Ok(p) => {
                    prop_assert_eq!(p.chars().count(), length);
                    if exclude_ambiguous {
                        prop_assert!(!p.chars().any(|c| AMBIGUOUS_CHARS.contains(c)));
                    }
                    let has = |f: fn(char) -> bool| p.chars().any(f);
                    if !lower { prop_assert!(!has(|c| c.is_ascii_lowercase())); }
                    if !upper { prop_assert!(!has(|c| c.is_ascii_uppercase())); }
                    if !digits { prop_assert!(!has(|c| c.is_ascii_digit())); }
                    if !symbols { prop_assert!(!has(|c| c.is_ascii_punctuation())); }
                    if require_each_class {
                        if lower { prop_assert!(has(|c| c.is_ascii_lowercase())); }
                        if upper { prop_assert!(has(|c| c.is_ascii_uppercase())); }
                        if digits { prop_assert!(has(|c| c.is_ascii_digit())); }
                        if symbols { prop_assert!(has(|c| c.is_ascii_punctuation())); }
                    }
                }
            }
        }
    }
}
