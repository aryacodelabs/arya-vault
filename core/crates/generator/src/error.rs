use thiserror::Error;

/// Invalid generator options. Never contains generated or user secrets.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OptionsError {
    /// Length outside the supported range.
    #[error("length {0} is outside the supported range {1}..={2}")]
    LengthOutOfRange(usize, usize, usize),
    /// No character class enabled.
    #[error("at least one character class must be enabled")]
    NoClassEnabled,
    /// Symbols enabled but the symbol set is empty (possibly after exclusions).
    #[error("symbols are enabled but the symbol set is empty")]
    EmptySymbolSet,
    /// Symbol set contains something other than printable ASCII punctuation.
    #[error("symbol set must contain only printable ASCII punctuation")]
    InvalidSymbolSet,
    /// A class has no characters left after excluding ambiguous ones.
    #[error("a character class is empty after excluding ambiguous characters")]
    EmptyClass,
    /// `require_each_class` needs at least one position per enabled class.
    #[error("length {length} is smaller than the {classes} required character classes")]
    LengthBelowClasses {
        /// Requested length.
        length: usize,
        /// Number of enabled classes.
        classes: usize,
    },
    /// Word count outside the supported range.
    #[error("word count {0} is outside the supported range {1}..={2}")]
    WordCountOutOfRange(usize, usize, usize),
    /// Separator too long or contains control characters.
    #[error("separator must be at most 16 characters without control characters")]
    InvalidSeparator,
}

/// Errors from generating a secret.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GeneratorError {
    /// Options failed validation.
    #[error(transparent)]
    Options(#[from] OptionsError),
    /// The random source failed (OS CSPRNG unavailable).
    #[error("random source failure")]
    Rng,
    /// The embedded wordlist failed its integrity check.
    #[error("embedded wordlist is corrupt")]
    WordlistCorrupt,
}
