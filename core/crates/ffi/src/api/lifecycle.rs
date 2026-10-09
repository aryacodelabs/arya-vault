//! docs/14 §4.1: lifecycle and credentials.

use std::path::PathBuf;

use arya_vault_session::{RecoveryKeyResult as CoreKey, SessionState};
use zeroize::Zeroizing;

use super::dto::{AppError, GroupAnswer, KdfProfile, RecoveryKeyResult, VaultStatus};
use crate::convert::to_usize;
use crate::error::ApiResult;
use crate::host::{self, Host, into_bytes, secret_text};
use crate::prefs;

/// **Not in docs/14** (spec question 1): tells the core which directory holds the vault. The app
/// calls it once at start-up with its per-user data directory (docs/14 §8 question 1: a single
/// default vault). Calling it again with the same path is a no-op; a different path needs a
/// locked session.
///
/// # Errors
/// `busy` if the session is unlocked and the path differs; `validation` for an empty path.
pub fn init_core(vault_dir: String) -> ApiResult<()> {
    if vault_dir.is_empty() {
        return Err(AppError::validation("vaultDir", "the directory is empty"));
    }
    host::call(|h| h.set_dir(PathBuf::from(vault_dir)))
}

/// `status`: works while locked, from plaintext files only.
///
/// # Errors
/// `io`, `corruptVault`, `unsupportedFormat` for a damaged or newer vault directory.
pub fn status() -> ApiResult<VaultStatus> {
    host::call(|h| {
        let s = h.session()?.status()?;
        Ok(VaultStatus {
            exists: s.exists,
            locked: s.locked,
            onboarding_complete: s.onboarding_complete,
            format_version: u32::from(s.format_version),
            quick_unlock: s.quick_unlock.into(),
        })
    })
}

fn key_result(r: &CoreKey) -> RecoveryKeyResult {
    RecoveryKeyResult {
        recovery_key: into_bytes(r.recovery_key()),
        groups: u32::try_from(r.groups()).unwrap_or(u32::MAX),
        challenge: r
            .challenge()
            .iter()
            .map(|g| u32::try_from(*g).unwrap_or(u32::MAX))
            .collect(),
    }
}

/// Applies the stored retention settings to a freshly unlocked vault.
fn apply_settings(h: &mut Host) -> ApiResult<()> {
    h.vault(|v| prefs::load(v).map(|_| ()))
}

/// `createVault`: the vault is created **unlocked**; the recovery key is returned once.
///
/// # Errors
/// `weakPassword`, `alreadyExists`, `io`.
pub fn create_vault(password: Vec<u8>, profile: KdfProfile) -> ApiResult<RecoveryKeyResult> {
    let pw = secret_text(Zeroizing::new(password), "password")?;
    host::call(|h| {
        let core_profile = profile.into();
        let r = h.session()?.create(&pw, core_profile)?;
        h.profile = core_profile;
        apply_settings(h)?;
        Ok(key_result(&r))
    })
}

/// `confirmRecoveryKey`: `true` when the answers match, `false` to show the key again.
///
/// # Errors
/// `locked`, `validation` (no key is waiting, or the answers do not cover the asked groups).
pub fn confirm_recovery_key(answers: Vec<GroupAnswer>) -> ApiResult<bool> {
    let answers = answers
        .into_iter()
        .map(|a| Ok((to_usize(a.index, "answers")?, a.text)))
        .collect::<ApiResult<Vec<_>>>()?;
    host::call(|h| Ok(h.session()?.confirm_recovery_key(answers)?))
}

/// `unlock`: applies the local failure delay (cosmetic, docs/07 §4).
///
/// # Errors
/// `wrongCredentials`, `busy` (delay running), `corruptVault`, `unsupportedFormat`.
pub fn unlock(password: Vec<u8>) -> ApiResult<()> {
    let pw = secret_text(Zeroizing::new(password), "password")?;
    host::call(|h| {
        h.session()?.unlock(&pw)?;
        apply_settings(h)
    })
}

/// `unlockQuick`: the platform provider runs inside Rust; the vault key never reaches Dart.
///
/// # Errors
/// `quickUnlockUnavailable`, `locked` if a `lock()` raced the prompt.
pub fn unlock_quick() -> ApiResult<()> {
    host::call(|h| {
        h.session()?.unlock_quick()?;
        apply_settings(h)
    })
}

/// `lock`: zeroizes keys and closes the database. Idempotent. Asks an unlock that is still in
/// flight to give up first, so it does not wait for it.
///
/// # Errors
/// `io` if the database cannot be closed cleanly (the keys are gone either way).
pub fn lock() -> ApiResult<()> {
    host::request_lock();
    host::call(|h| {
        h.forget_unlocked_state();
        match h.session() {
            Ok(s) => Ok(s.lock()?),
            // Not initialised: nothing is unlocked.
            Err(_) => Ok(()),
        }
    })
}

/// `changePassword`. With `rotate_keys` this is "Change password and rotate keys" (SEC-A06): the
/// vault key and the recovery key are replaced. The contract returns nothing, so the new
/// recovery key stays pending inside the core: afterwards `status().onboardingComplete` is
/// `false` and the app calls `regenerateRecoveryKey` to obtain a key to show (spec question 4).
///
/// # Errors
/// `wrongCredentials`, `weakPassword`, `busy`, `locked` is *not* returned: a locked vault can be
/// re-wrapped with the old password, a rotation needs the unlocked session.
pub fn change_password(
    old_password: Vec<u8>,
    new_password: Vec<u8>,
    rotate_keys: bool,
) -> ApiResult<()> {
    let (old_password, new_password) = (Zeroizing::new(old_password), Zeroizing::new(new_password));
    let old = secret_text(old_password, "oldPassword")?;
    let new = secret_text(new_password, "newPassword")?;
    host::call(|h| {
        let profile = h.profile;
        let s = h.session()?;
        if rotate_keys {
            s.change_password_and_rotate(&old, &new, profile)?;
        } else {
            s.change_password(&old, &new, profile)?;
        }
        Ok(())
    })
}

/// `recoverWithKey`: leaves the vault unlocked (docs/04 §9).
///
/// # Errors
/// `recoveryKeyMalformed`, `wrongCredentials`, `weakPassword`, `busy`.
pub fn recover_with_key(recovery_key: Vec<u8>, new_password: Vec<u8>) -> ApiResult<()> {
    let (recovery_key, new_password) = (Zeroizing::new(recovery_key), Zeroizing::new(new_password));
    let key = secret_text(recovery_key, "recoveryKey")?;
    let new = secret_text(new_password, "newPassword")?;
    host::call(|h| {
        let profile = h.profile;
        h.session()?.recover(&key, &new, profile)?;
        if matches!(
            h.session()?.state(),
            SessionState::Unlocked | SessionState::UnlockedPendingConfirm
        ) {
            apply_settings(h)?;
        }
        Ok(())
    })
}

/// `regenerateRecoveryKey`. `rotate_keys` also replaces the vault key (SEC-A06); either way the
/// old recovery key stops working for new unlocks and onboarding is pending until confirmed.
///
/// # Errors
/// `locked`, `wrongCredentials`, `busy`.
pub fn regenerate_recovery_key(
    password: Vec<u8>,
    rotate_keys: bool,
) -> ApiResult<RecoveryKeyResult> {
    let pw = secret_text(Zeroizing::new(password), "password")?;
    host::call(|h| {
        let s = h.session()?;
        let r = if rotate_keys {
            s.rotate_keys(&pw)?.recovery_key
        } else {
            s.regenerate_recovery_key(&pw)?
        };
        Ok(key_result(&r))
    })
}

/// `quickUnlockEnable`: needs the unlocked session; the platform shows the presence prompt.
///
/// # Errors
/// `locked`, `quickUnlockUnavailable`.
pub fn quick_unlock_enable() -> ApiResult<()> {
    host::call(|h| Ok(h.session()?.quick_unlock_enable()?))
}

/// `quickUnlockDisable`.
///
/// # Errors
/// `locked`, `io`.
pub fn quick_unlock_disable() -> ApiResult<()> {
    host::call(|h| Ok(h.session()?.quick_unlock_disable()?))
}

/// `verifyPassword`: re-authentication before a sensitive action. `false` for a wrong password.
///
/// # Errors
/// `locked`, `busy`.
pub fn verify_password(password: Vec<u8>) -> ApiResult<bool> {
    let pw = secret_text(Zeroizing::new(password), "password")?;
    host::call(|h| Ok(h.session()?.verify_password(&pw)?))
}
