//! Strength estimation and master-password policy (SEC-A07, docs/04 section 3).

use thiserror::Error;

/// Minimum master password length in Unicode scalar values (SEC-A07).
pub const MASTER_PASSWORD_MIN_CHARS: usize = 12;
/// Minimum acceptable zxcvbn score for a master password ("very weak" = 0 or 1 is rejected).
const MASTER_PASSWORD_MIN_SCORE: u8 = 2;
/// Inputs are truncated to this many characters before estimation. A prefix is
/// never harder to guess than the whole, so truncation only under-estimates,
/// and it bounds zxcvbn's running time.
const MAX_ESTIMATE_CHARS: usize = 128;

/// Result of a strength estimate. Contains no part of the password.
#[derive(Debug, Clone, PartialEq)]
pub struct Strength {
    /// zxcvbn score, 0 (trivial) to 4 (strong).
    pub score: u8,
    /// log10 of the estimated number of guesses.
    pub guesses_log10: f64,
    /// Human-readable warning and suggestions (static texts, empty for strong passwords).
    pub feedback: Vec<String>,
}

/// Estimate password strength with zxcvbn.
///
/// Note: zxcvbn copies the input internally and cannot be zeroized; the
/// estimate is intended for the unlocked, in-process UI path only.
#[must_use]
pub fn estimate_strength(password: &str) -> Strength {
    let truncated: String;
    let input = if password.chars().count() > MAX_ESTIMATE_CHARS {
        truncated = password.chars().take(MAX_ESTIMATE_CHARS).collect();
        truncated.as_str()
    } else {
        password
    };
    let entropy = zxcvbn::zxcvbn(input, &[]);
    let mut feedback = Vec::new();
    if let Some(f) = entropy.feedback() {
        if let Some(w) = f.warning() {
            feedback.push(w.to_string());
        }
        feedback.extend(f.suggestions().iter().map(ToString::to_string));
    }
    Strength {
        score: u8::from(entropy.score()),
        guesses_log10: entropy.guesses_log10(),
        feedback,
    }
}

/// Reason a master password was rejected.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PolicyViolation {
    /// Shorter than [`MASTER_PASSWORD_MIN_CHARS`].
    #[error("master password must be at least {min} characters (got {actual})")]
    TooShort {
        /// Required minimum.
        min: usize,
        /// Actual length in characters.
        actual: usize,
    },
    /// zxcvbn judges it very weak.
    #[error("master password is too easy to guess (strength {score} of 4)")]
    TooWeak {
        /// The zxcvbn score.
        score: u8,
    },
}

/// Check the master-password policy (SEC-A07): minimum length 12 and a
/// strength check that rejects very weak passwords.
///
/// # Errors
/// Returns every violated rule (never empty).
pub fn meets_master_password_policy(password: &str) -> Result<(), Vec<PolicyViolation>> {
    let mut reasons = Vec::new();
    let actual = password.chars().count();
    if actual < MASTER_PASSWORD_MIN_CHARS {
        reasons.push(PolicyViolation::TooShort {
            min: MASTER_PASSWORD_MIN_CHARS,
            actual,
        });
    }
    let score = estimate_strength(password).score;
    if score < MASTER_PASSWORD_MIN_SCORE {
        reasons.push(PolicyViolation::TooWeak { score });
    }
    if reasons.is_empty() {
        Ok(())
    } else {
        Err(reasons)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::password::{PasswordOptions, generate_password};
    use crate::rng::SeededRandom;

    #[test]
    fn weak_passwords_score_low() {
        for pw in [
            "password123",
            "qwertyuiop",
            "123456789",
            "asdfghjkl",
            "CANARY",
        ] {
            let s = estimate_strength(pw);
            assert!(s.score <= 1, "{pw} scored {}", s.score);
        }
        assert!(!estimate_strength("password123").feedback.is_empty());
    }

    #[test]
    fn long_random_scores_high() {
        let o = PasswordOptions {
            length: 24,
            ..PasswordOptions::default()
        };
        let p = generate_password(&o, &mut SeededRandom::new(42)).unwrap();
        let s = estimate_strength(&p);
        assert_eq!(s.score, 4);
        assert!(s.guesses_log10 > 10.0);
    }

    #[test]
    fn strength_debug_does_not_contain_password() {
        let s = estimate_strength("CANARY-hunter2-do-not-use");
        assert!(!format!("{s:?}").contains("CANARY"));
    }

    #[test]
    fn master_policy_reasons() {
        let short = meets_master_password_policy("Ab1!").unwrap_err();
        assert!(
            short
                .iter()
                .any(|r| matches!(r, PolicyViolation::TooShort { actual: 4, .. }))
        );
        let weak = meets_master_password_policy("password1234").unwrap_err();
        assert!(
            weak.iter()
                .any(|r| matches!(r, PolicyViolation::TooWeak { .. }))
        );
        assert!(
            !weak
                .iter()
                .any(|r| matches!(r, PolicyViolation::TooShort { .. }))
        );
        assert!(meets_master_password_policy("CANARY-correct horse battery staple 7").is_ok());
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        let e = meets_master_password_policy("ééééé").unwrap_err();
        assert!(e.contains(&PolicyViolation::TooShort { min: 12, actual: 5 }));
    }

    #[test]
    fn very_long_input_is_bounded() {
        let s = estimate_strength(&"a".repeat(10_000));
        assert!(s.score <= 1);
    }
}
