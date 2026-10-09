//! Plaintext, non-secret session metadata that must be readable while the vault is locked.
//!
//! # Where `onboardingComplete` lives
//! `VaultStatus.onboardingComplete` routes the UI before any unlock, so it cannot live in the
//! encrypted database. It is the **absence** of a marker file `onboarding.pending` next to the
//! header files:
//!
//! * `create` (with confirmation) and `regenerate_recovery_key` write the marker **before** they
//!   write the header, so a crash can leave the marker without the new key, never the new key
//!   without the marker;
//! * a successful `confirm_recovery_key` removes it;
//! * a vault without the marker (created by the CLI, or before this task) counts as complete.
//!
//! The marker holds no secret (a fixed text line) and is **not authenticated**: anyone with write
//! access to the directory can add or remove it. That is acceptable because it gates only a UX
//! step (docs/07 §3 step H); no key or ciphertext depends on it. It is device-local state and
//! must not be synchronised to other devices.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

use crate::error::Result;

/// File name of the "recovery key not yet confirmed" marker.
pub const ONBOARDING_MARKER: &str = "onboarding.pending";
const ONBOARDING_MARKER_TMP: &str = "onboarding.pending.tmp";
const MARKER_BODY: &[u8] = b"arya-vault: recovery key not yet confirmed (no secret here)\n";

/// Whether the recovery key of the vault in `root` still has to be confirmed.
#[must_use]
pub fn onboarding_pending(root: &Path) -> bool {
    root.join(ONBOARDING_MARKER).exists()
}

/// Creates the marker (atomically: temp file + rename; owner-only on Unix).
///
/// # Errors
/// Filesystem errors.
pub(crate) fn set_onboarding_pending(root: &Path) -> Result<()> {
    let tmp = root.join(ONBOARDING_MARKER_TMP);
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(MARKER_BODY)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, root.join(ONBOARDING_MARKER))?;
    Ok(())
}

/// Removes the marker; a missing marker is not an error.
///
/// # Errors
/// Filesystem errors other than "not found".
pub(crate) fn clear_onboarding_pending(root: &Path) -> Result<()> {
    match fs::remove_file(root.join(ONBOARDING_MARKER)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_round_trip_and_idempotent_clear() {
        let d = tempfile::tempdir().unwrap();
        assert!(!onboarding_pending(d.path()));
        set_onboarding_pending(d.path()).unwrap();
        assert!(onboarding_pending(d.path()));
        set_onboarding_pending(d.path()).unwrap();
        clear_onboarding_pending(d.path()).unwrap();
        clear_onboarding_pending(d.path()).unwrap();
        assert!(!onboarding_pending(d.path()));
        assert!(!d.path().join(ONBOARDING_MARKER_TMP).exists());
    }

    #[test]
    fn marker_contains_no_secret_material() {
        let d = tempfile::tempdir().unwrap();
        set_onboarding_pending(d.path()).unwrap();
        let body = fs::read(d.path().join(ONBOARDING_MARKER)).unwrap();
        assert_eq!(body, MARKER_BODY);
    }
}
