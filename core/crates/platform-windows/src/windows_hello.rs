//! The real Windows Hello: `KeyCredentialManager` through the `windows` crate (WinRT). One
//! commented `unsafe` call (`GetTickCount64`, no pointers) for the boot identifier.
//!
//! Spike S4 (docs/reviews/m2-spikes.md): the credential signs a fixed challenge with a
//! deterministic RSA PKCS#1 v1.5 signature (256 bytes), identical across calls and processes,
//! and Windows asks the user on **every** signature.

use std::time::{SystemTime, UNIX_EPOCH};

use windows::Security::Credentials::{
    KeyCredential, KeyCredentialCreationOption, KeyCredentialManager, KeyCredentialStatus,
};
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::core::{Array, HSTRING};

use crate::{HelloError, HelloPlatform};

/// Name of the credential (docs/04 section 8). One per Windows user, so one vault per user for
/// now (the single-vault MVP). It must not contain `/` or `\`: Windows rejects such a name with
/// "The parameter is incorrect" (0x80090027), found when the first build tried to enable quick
/// unlock with `aryavault/quick-unlock/v1`.
const CREDENTIAL_NAME: &str = "aryavault-quick-unlock-v1";

/// Windows Hello through `KeyCredentialManager`.
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsHello;

fn status_error(status: KeyCredentialStatus) -> HelloError {
    match status {
        KeyCredentialStatus::UserCanceled => HelloError::Cancelled,
        KeyCredentialStatus::NotFound => HelloError::Invalidated,
        KeyCredentialStatus::UserPrefersPassword | KeyCredentialStatus::SecurityDeviceLocked => {
            HelloError::Unavailable
        }
        _ => HelloError::Failed,
    }
}

/// Blocks on a WinRT async operation. A macro, because the operation type is not nameable
/// through the `windows` crate's public paths.
macro_rules! wait {
    ($op:expr) => {
        $op.and_then(|o| o.join()).map_err(|_| HelloError::Failed)
    };
}

/// Seconds since boot, including time spent asleep.
fn uptime_secs() -> u64 {
    // SAFETY: `GetTickCount64` takes no arguments and no pointers, has no preconditions and
    // cannot fail; it only reads a kernel counter.
    #[allow(unsafe_code)]
    let ms = unsafe { GetTickCount64() };
    ms / 1000
}

fn name() -> HSTRING {
    HSTRING::from(CREDENTIAL_NAME)
}

fn open_credential() -> Result<KeyCredential, HelloError> {
    let result = wait!(KeyCredentialManager::OpenAsync(&name()))?;
    let status = result.Status().map_err(|_| HelloError::Failed)?;
    if status != KeyCredentialStatus::Success {
        return Err(status_error(status));
    }
    result.Credential().map_err(|_| HelloError::Failed)
}

impl HelloPlatform for WindowsHello {
    fn supported(&self) -> bool {
        wait!(KeyCredentialManager::IsSupportedAsync()).unwrap_or(false)
    }

    fn create_credential(&self) -> Result<(), HelloError> {
        let result = wait!(KeyCredentialManager::RequestCreateAsync(
            &name(),
            KeyCredentialCreationOption::ReplaceExisting,
        ))?;
        let status = result.Status().map_err(|_| HelloError::Failed)?;
        if status == KeyCredentialStatus::Success {
            Ok(())
        } else {
            Err(status_error(status))
        }
    }

    fn sign(&self, challenge: &[u8]) -> Result<Vec<u8>, HelloError> {
        let credential = open_credential()?;
        let buffer =
            CryptographicBuffer::CreateFromByteArray(challenge).map_err(|_| HelloError::Failed)?;
        let result = wait!(credential.RequestSignAsync(&buffer))?;
        let status = result.Status().map_err(|_| HelloError::Failed)?;
        if status != KeyCredentialStatus::Success {
            return Err(status_error(status));
        }
        let signature = result.Result().map_err(|_| HelloError::Failed)?;
        let mut bytes = Array::<u8>::new();
        CryptographicBuffer::CopyToByteArray(&signature, &mut bytes)
            .map_err(|_| HelloError::Failed)?;
        Ok(bytes.to_vec())
    }

    fn delete_credential(&self) {
        let _ = wait!(KeyCredentialManager::DeleteAsync(&name()));
    }

    /// The boot time (Unix seconds minus the tick count), in whole minutes, big-endian. It
    /// changes after a reboot; a clock step of a minute or more also changes it, which only
    /// asks for the master password once more (the safe direction).
    fn boot_id(&self) -> Option<Vec<u8>> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
        let up = uptime_secs();
        let boot_minute = now.checked_sub(up)? / 60;
        Some(boot_minute.to_be_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::CREDENTIAL_NAME;

    #[test]
    fn the_credential_name_has_no_path_separators() {
        assert!(!CREDENTIAL_NAME.contains(['/', '\\']));
        assert!(!CREDENTIAL_NAME.is_empty());
    }
}
