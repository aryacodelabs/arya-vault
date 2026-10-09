//! docs/14 §4.5: vault-level settings, kept in the encrypted vault and clamped by the core.

use super::dto::{AppError, AppSettings};
use crate::host;
use crate::prefs;

/// `getSettings()`: the stored settings, or the defaults for a vault that never stored any.
///
/// # Errors
/// `locked`.
pub fn get_settings() -> Result<AppSettings, AppError> {
    host::call(|h| h.vault(prefs::load))
}

/// `setSettings(AppSettings)`: every value is clamped to its allowed range (auto-lock 1-60 min,
/// clipboard 5-120 s, reveal auto-hide 1-15 s, retention bounds); read them back with
/// `getSettings` to see what was applied.
///
/// # Errors
/// `locked`.
pub fn set_settings(settings: AppSettings) -> Result<(), AppError> {
    host::call(|h| h.vault(|v| prefs::store(v, settings)))
}
