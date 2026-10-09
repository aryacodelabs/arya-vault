//! docs/14 §4.3: generator, strength, health.

use arya_vault_generator as g;
use zeroize::Zeroizing;

use super::dto::{
    AppError, EntropyOptions, HealthReport, OldPassword, PassphraseOptions, PasswordOptions,
    PolicyResult, ReuseGroup, Strength, WeakPassword,
};
use crate::convert::hex_id;
use crate::host::{self, into_bytes, secret_text};

/// Passwords with a zxcvbn score up to this are reported as weak.
const WEAK_MAX_SCORE: u8 = 1;
/// Passwords older than this are reported as old.
const OLD_AFTER_DAYS: u64 = 365;

fn options_error(e: g::GeneratorError) -> AppError {
    match e {
        g::GeneratorError::Options(o) => AppError::validation("options", &o.to_string()),
        _ => AppError::internal(),
    }
}

fn options_only(e: g::OptionsError) -> AppError {
    AppError::validation("options", &e.to_string())
}

/// `generatePassword(PasswordOptions)`: UTF-8 bytes. Works while locked.
///
/// # Errors
/// `validation` (`options`), `internal` (the OS random source failed).
pub fn generate_password(options: PasswordOptions) -> Result<Vec<u8>, AppError> {
    host::guarded(|| {
        let o = options.to_core()?;
        let pw = g::generate_password(&o, &mut g::OsRandom).map_err(options_error)?;
        Ok(into_bytes(pw))
    })
}

/// `generatePassphrase(PassphraseOptions)`: UTF-8 bytes. Works while locked.
///
/// # Errors
/// `validation` (`options`), `internal`.
pub fn generate_passphrase(options: PassphraseOptions) -> Result<Vec<u8>, AppError> {
    host::guarded(|| {
        let o = options.to_core()?;
        let pw = g::generate_passphrase(&o, &mut g::OsRandom).map_err(options_error)?;
        Ok(into_bytes(pw))
    })
}

/// `entropyBits(options)`.
///
/// # Errors
/// `validation` (`options`).
pub fn entropy_bits(options: EntropyOptions) -> Result<f64, AppError> {
    host::guarded(|| match options {
        EntropyOptions::Password(o) => g::entropy_bits(&o.to_core()?).map_err(options_only),
        EntropyOptions::Passphrase(o) => {
            g::entropy_bits_passphrase(&o.to_core()?).map_err(options_only)
        }
    })
}

/// `strength(Uint8List)`: the estimate holds no part of the password.
///
/// # Errors
/// `validation` (not UTF-8).
pub fn strength(password: Vec<u8>) -> Result<Strength, AppError> {
    host::guarded(|| {
        let pw = secret_text(Zeroizing::new(password), "password")?;
        let s = g::estimate_strength(&pw);
        Ok(Strength {
            score: s.score,
            guesses_log10: s.guesses_log10,
            feedback: s.feedback,
        })
    })
}

/// `checkMasterPassword(Uint8List)`: the master-password policy (SEC-A07). Works while locked.
///
/// # Errors
/// `validation` (not UTF-8).
pub fn check_master_password(password: Vec<u8>) -> Result<PolicyResult, AppError> {
    host::guarded(|| {
        let pw = secret_text(Zeroizing::new(password), "password")?;
        Ok(match g::meets_master_password_policy(&pw) {
            Ok(()) => PolicyResult {
                acceptable: true,
                reasons: Vec::new(),
            },
            Err(reasons) => PolicyResult {
                acceptable: false,
                reasons: reasons.iter().map(ToString::to_string).collect(),
            },
        })
    })
}

/// `healthReport()`: reused, weak and old passwords. Holds item ids and numbers only.
///
/// # Errors
/// `locked`.
pub fn health_report() -> Result<HealthReport, AppError> {
    host::call(|h| {
        h.vault(|v| {
            let reused = v
                .reused_passwords()?
                .into_iter()
                .map(|g| ReuseGroup {
                    item_ids: g.item_ids.iter().map(hex_id).collect(),
                })
                .collect();
            let weak = v
                .weak_passwords(WEAK_MAX_SCORE)?
                .into_iter()
                .map(|w| WeakPassword {
                    item_id: hex_id(&w.item_id),
                    score: w.score,
                })
                .collect();
            let old = v
                .old_passwords(OLD_AFTER_DAYS)?
                .into_iter()
                .map(|o| OldPassword {
                    item_id: hex_id(&o.item_id),
                    age_days: u32::try_from(o.age_days).unwrap_or(u32::MAX),
                })
                .collect();
            Ok::<_, AppError>(HealthReport { reused, weak, old })
        })
    })
}
