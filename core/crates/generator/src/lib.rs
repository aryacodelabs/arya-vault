//! Password and passphrase generation (docs/04 section 12).
//!
//! * [`generate_password`] / [`PasswordOptions`]: character-class passwords.
//! * [`generate_passphrase`] / [`PassphraseOptions`]: EFF large wordlist passphrases.
//! * [`estimate_strength`] / [`meets_master_password_policy`]: zxcvbn-based checks.
//!
//! All randomness comes from a [`RandomSource`]; the only implementation
//! available in normal builds is [`OsRandom`] (the OS CSPRNG). Indices are
//! chosen by rejection sampling (SEC-C09), never by modulo reduction.
//! Generated secrets are returned as `Zeroizing<String>` and every
//! intermediate buffer is zeroized on drop.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod error;
mod passphrase;
mod password;
mod rng;
mod strength;
mod wordlist;

pub use error::{GeneratorError, OptionsError};
pub use passphrase::{
    MAX_WORDS, MIN_WORDS, PassphraseOptions, entropy_bits_passphrase, generate_passphrase,
};
pub use password::{
    AMBIGUOUS_CHARS, DEFAULT_SYMBOLS, MAX_LENGTH, MIN_LENGTH, PasswordOptions, entropy_bits,
    generate_password,
};
#[cfg(any(test, feature = "deterministic-rng"))]
pub use rng::SeededRandom;
pub use rng::{OsRandom, RandomSource};
pub use strength::{
    MASTER_PASSWORD_MIN_CHARS, PolicyViolation, Strength, estimate_strength,
    meets_master_password_policy,
};
pub use wordlist::{WORDLIST_LEN, wordlist};

#[cfg(test)]
mod uniformity_tests;
