//! Quick unlock: the platform seam, the policy record and the policy itself
//! (docs/04 §8, docs/14 §6, SEC-A02, SEC-A03).
//!
//! # What lives where
//! * **Rust, here:** the [`QuickUnlockProvider`] trait, the plaintext policy record
//!   (`quick-unlock.policy`), the sealed blob file (`quick-unlock.blob`) and the policy that
//!   decides when the password is required again. The raw vault key never leaves Rust: Dart only
//!   calls `unlock_quick()`.
//! * **Per platform (out of scope here, L01 / M3 / M6):** the provider, i.e. the hardware-backed,
//!   non-exportable, user-presence-bound key that `seal`s and `unseal`s the vault key.
//!
//! # What the policy does and does not give you
//! The hardware binding (SEC-A02) is the provider's job and is verified per platform, not
//! here. The policy record is plaintext and **unauthenticated**: somebody who can write the
//! vault directory can reset the failure counter or the clock fields. That can only widen the
//! window in which the provider is asked to release the key; the provider still demands user
//! presence, so a file edit alone never yields the key.

use core::fmt;

use arya_vault_crypto::keys::VaultKey;
use thiserror::Error;
use zeroize::Zeroizing;

#[cfg(feature = "test-support")]
pub mod fake;
pub mod policy;

pub use policy::{MAX_BLOB_BYTES, MAX_BOOT_ID_BYTES, PolicyRecord};

/// Compile-time guard, like `deterministic-rng` in the crypto crate: the fake provider must
/// never be part of a release build.
#[cfg(all(feature = "test-support", not(debug_assertions)))]
compile_error!(
    "the `test-support` feature (fake quick-unlock provider) is for tests only and must not be \
     enabled in release builds"
);

/// The kind of quick unlock a provider offers (docs/14 `QuickUnlockStatus.kind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickUnlockKind {
    /// No provider.
    None,
    /// Windows Hello (`KeyCredentialManager`).
    WindowsHello,
    /// Touch ID.
    TouchId,
    /// Face ID.
    FaceId,
    /// Android biometric prompt.
    Biometric,
    /// An OS keyring without biometrics (Linux Secret Service; lower assurance, docs/04 §8).
    OsKeyring,
}

impl QuickUnlockKind {
    /// The name used by docs/14.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::WindowsHello => "windowsHello",
            Self::TouchId => "touchId",
            Self::FaceId => "faceId",
            Self::Biometric => "biometric",
            Self::OsKeyring => "osKeyring",
        }
    }
}

/// An opaque, provider-defined sealed vault key. Size-limited ([`MAX_BLOB_BYTES`]), wiped on
/// drop, and never printed. It is useless without the platform key that sealed it.
pub struct Blob(Zeroizing<Vec<u8>>);

impl Blob {
    /// Wraps provider output.
    ///
    /// # Errors
    /// [`ProviderError::Failed`] if empty or larger than [`MAX_BLOB_BYTES`].
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProviderError> {
        if bytes.is_empty() || bytes.len() > MAX_BLOB_BYTES {
            return Err(ProviderError::Failed);
        }
        Ok(Self(Zeroizing::new(bytes)))
    }

    /// The provider's bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Blob {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Blob(<{} bytes, redacted>)", self.0.len())
    }
}

/// Why a provider call failed. Carries no detail by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProviderError {
    /// No usable hardware or OS key store right now.
    #[error("quick unlock is unavailable")]
    Unavailable,
    /// The user dismissed the prompt. Not counted as a failed attempt.
    #[error("quick unlock was cancelled")]
    UserCancelled,
    /// The platform key is gone or no longer valid (for example the biometric enrolment
    /// changed). The session disables quick unlock and deletes the blob.
    #[error("quick unlock was invalidated")]
    Invalidated,
    /// Anything else, including a blob that fails authentication or a failed biometric.
    #[error("quick unlock failed")]
    Failed,
}

/// The platform seam of docs/14 §6. Implemented in Rust per platform; the session owns the
/// instance (`Session::with_provider`).
///
/// # What an implementation MUST guarantee
/// * `unseal` requires **user presence** (biometric or device credential) every time, enforced by
///   the platform, not by a UI gate (docs/04 §8, review M2). `UserConsentVerifier` alone and plain
///   DPAPI are not acceptable.
/// * The blob is useless without the platform key: the key is non-exportable and bound to this
///   device (and to the biometric enrolment where the platform offers it).
/// * `seal` returns at most [`MAX_BLOB_BYTES`] bytes.
/// * `boot_id` is stable for one boot of the OS and different after a reboot (for example
///   Linux `/proc/sys/kernel/random/boot_id`, Windows last-boot time, macOS `kern.boottime`);
///   `None` only if the platform has no such value.
/// * Methods may block on a system prompt; the session never calls them while holding a lock the
///   UI needs for `lock()` (see `Session::lock_request_handle`).
pub trait QuickUnlockProvider: Send {
    /// What this provider is.
    fn kind(&self) -> QuickUnlockKind;
    /// Whether quick unlock can be used on this device right now.
    fn available(&self) -> bool;
    /// Seals the vault key under the platform key. May prompt for user presence.
    ///
    /// # Errors
    /// [`ProviderError`].
    fn seal(&self, vk: &VaultKey) -> Result<Blob, ProviderError>;
    /// Releases the vault key from a blob. Requires user presence.
    ///
    /// # Errors
    /// [`ProviderError`].
    fn unseal(&self, blob: &Blob) -> Result<VaultKey, ProviderError>;
    /// An identifier of the current boot, or `None` if the platform has none.
    fn boot_id(&self) -> Option<Vec<u8>>;
    /// Best-effort removal of the platform key when quick unlock is disabled. Default: nothing.
    fn revoke(&self) {}
}

/// The default provider: nothing is supported.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoProvider;

impl QuickUnlockProvider for NoProvider {
    fn kind(&self) -> QuickUnlockKind {
        QuickUnlockKind::None
    }
    fn available(&self) -> bool {
        false
    }
    fn seal(&self, _vk: &VaultKey) -> Result<Blob, ProviderError> {
        Err(ProviderError::Unavailable)
    }
    fn unseal(&self, _blob: &Blob) -> Result<VaultKey, ProviderError> {
        Err(ProviderError::Unavailable)
    }
    fn boot_id(&self) -> Option<Vec<u8>> {
        None
    }
}

/// Why the master password is required instead of quick unlock. All of these map to the
/// `quickUnlockUnavailable` code (docs/14 §2); the UI falls back to the password.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickUnlockDenied {
    /// Quick unlock is not enabled for this vault.
    NotEnabled,
    /// The provider reports it is not available (or there is none).
    ProviderUnavailable,
    /// The provider kind differs from the one that sealed the key.
    ProviderChanged,
    /// The device was restarted since the last password unlock.
    Rebooted,
    /// More than the allowed time since the last password unlock.
    Expired,
    /// The wall clock is earlier than a time already recorded, so elapsed time cannot be trusted.
    ClockRolledBack,
    /// Too many consecutive failed attempts.
    TooManyFailures,
    /// The platform key was invalidated (biometric enrolment changed); quick unlock was disabled.
    Invalidated,
    /// The user dismissed the prompt.
    Cancelled,
    /// The provider could not release the key (failed biometric or blob did not authenticate).
    Failed,
    /// The stored blob or policy record is missing, damaged, oversized or belongs elsewhere;
    /// quick unlock was disabled.
    BlobRejected,
    /// `lock()` was requested while the unlock was in flight; nothing was unlocked.
    LockRequested,
}

impl QuickUnlockDenied {
    /// A short secret-free explanation.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            Self::NotEnabled => "quick unlock is not enabled",
            Self::ProviderUnavailable => "quick unlock is not available on this device",
            Self::ProviderChanged => "the quick-unlock method changed; use the master password",
            Self::Rebooted => "the device restarted; use the master password",
            Self::Expired => "it has been too long since the master password; use it again",
            Self::ClockRolledBack => "the system clock moved backwards; use the master password",
            Self::TooManyFailures => "too many failed attempts; use the master password",
            Self::Invalidated => "quick unlock was invalidated; use the master password",
            Self::Cancelled => "quick unlock was cancelled",
            Self::Failed => "quick unlock failed",
            Self::BlobRejected => "stored quick-unlock data was rejected; use the master password",
            Self::LockRequested => "the vault was locked while unlocking",
        }
    }
}

/// The tunable parts of the policy (docs/04 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickUnlockConfig {
    /// Longest time since the last password unlock (default 72 h).
    pub max_age_ms: u64,
    /// Consecutive failures after which the password is required (default 5).
    pub max_failures: u32,
    /// Require the password after a reboot (default `true`; docs/04 §8 "policy configurable").
    pub require_password_after_reboot: bool,
}

impl Default for QuickUnlockConfig {
    fn default() -> Self {
        Self {
            max_age_ms: 72 * 60 * 60 * 1000,
            max_failures: 5,
            require_password_after_reboot: true,
        }
    }
}

/// What the app shows (docs/14 `QuickUnlockStatus`), plus why the password is needed now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickUnlockStatus {
    /// The provider says quick unlock can be used on this device.
    pub supported: bool,
    /// A valid policy record exists for this vault.
    pub enabled: bool,
    /// The provider kind.
    pub kind: QuickUnlockKind,
    /// If enabled but the policy currently demands the password, why. `None` when quick unlock
    /// would be attempted.
    pub password_required: Option<QuickUnlockDenied>,
}

/// Wall-clock time in milliseconds since the Unix epoch, injectable for tests.
pub trait WallClock: Send {
    /// Milliseconds since 1970-01-01 UTC.
    fn now_ms(&self) -> u64;
}

/// The system clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemWallClock;

impl WallClock for SystemWallClock {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
    }
}

/// Applies the policy of docs/04 §8 to a record. Pure: no I/O, no provider call.
///
/// Order: clock sanity, reboot, age, failures. `current_boot` is the provider's `boot_id()`.
///
/// # Errors
/// The first reason the password is required.
pub fn evaluate(
    rec: &PolicyRecord,
    now_ms: u64,
    current_boot: Option<&[u8]>,
    cfg: &QuickUnlockConfig,
) -> Result<(), QuickUnlockDenied> {
    // Wall clocks can be set back. Anything earlier than a time we already saw makes "elapsed"
    // meaningless, so it counts as expired.
    if now_ms < rec.last_seen_ms || now_ms < rec.last_password_unlock_ms {
        return Err(QuickUnlockDenied::ClockRolledBack);
    }
    if cfg.require_password_after_reboot && rec.boot_id.as_deref() != current_boot {
        return Err(QuickUnlockDenied::Rebooted);
    }
    if now_ms - rec.last_password_unlock_ms > cfg.max_age_ms {
        return Err(QuickUnlockDenied::Expired);
    }
    if rec.failure_count >= cfg.max_failures {
        return Err(QuickUnlockDenied::TooManyFailures);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600_000;
    const T0: u64 = 1_700_000_000_000;

    fn rec() -> PolicyRecord {
        PolicyRecord {
            kind: QuickUnlockKind::TouchId,
            enabled_at_ms: T0,
            last_password_unlock_ms: T0,
            last_seen_ms: T0,
            last_quick_unlock_ms: 0,
            failure_count: 0,
            boot_id: Some(b"boot-1".to_vec()),
        }
    }

    fn eval(r: &PolicyRecord, now: u64, boot: Option<&[u8]>) -> Result<(), QuickUnlockDenied> {
        evaluate(r, now, boot, &QuickUnlockConfig::default())
    }

    // SEC-A03: the matrix of docs/04 §8, without a provider or a vault.
    #[test]
    fn sec_a03_age_limit_is_72_hours_from_the_last_password_unlock() {
        let b = Some(&b"boot-1"[..]);
        assert_eq!(eval(&rec(), T0, b), Ok(()));
        assert_eq!(eval(&rec(), T0 + 71 * HOUR, b), Ok(()));
        assert_eq!(
            eval(&rec(), T0 + 72 * HOUR, b),
            Ok(()),
            "exactly 72 h is allowed"
        );
        assert_eq!(
            eval(&rec(), T0 + 72 * HOUR + 1, b),
            Err(QuickUnlockDenied::Expired)
        );
        assert_eq!(
            eval(&rec(), T0 + 73 * HOUR, b),
            Err(QuickUnlockDenied::Expired)
        );
        // a recent quick unlock does not extend it: only `last_password_unlock_ms` counts
        let mut r = rec();
        r.last_quick_unlock_ms = T0 + 70 * HOUR;
        r.last_seen_ms = T0 + 70 * HOUR;
        assert_eq!(eval(&r, T0 + 73 * HOUR, b), Err(QuickUnlockDenied::Expired));
    }

    #[test]
    fn sec_a03_a_clock_set_back_is_treated_as_expired() {
        let b = Some(&b"boot-1"[..]);
        let mut r = rec();
        r.last_seen_ms = T0 + 10 * HOUR;
        assert_eq!(
            eval(&r, T0 + 5 * HOUR, b),
            Err(QuickUnlockDenied::ClockRolledBack)
        );
        assert_eq!(
            eval(&rec(), T0 - 1, b),
            Err(QuickUnlockDenied::ClockRolledBack)
        );
        // even if only the password-unlock time is in the future
        let mut r = rec();
        r.last_password_unlock_ms = T0 + HOUR;
        assert_eq!(eval(&r, T0, b), Err(QuickUnlockDenied::ClockRolledBack));
    }

    #[test]
    fn sec_a03_reboot_means_password_unless_configured_otherwise() {
        assert_eq!(
            eval(&rec(), T0, Some(b"boot-2")),
            Err(QuickUnlockDenied::Rebooted)
        );
        assert_eq!(eval(&rec(), T0, None), Err(QuickUnlockDenied::Rebooted));
        let lax = QuickUnlockConfig {
            require_password_after_reboot: false,
            ..QuickUnlockConfig::default()
        };
        assert_eq!(evaluate(&rec(), T0, Some(b"boot-2"), &lax), Ok(()));
        // a platform without boot ids: None == None
        let mut r = rec();
        r.boot_id = None;
        assert_eq!(eval(&r, T0, None), Ok(()));
        assert_eq!(eval(&r, T0, Some(b"x")), Err(QuickUnlockDenied::Rebooted));
    }

    #[test]
    fn sec_a03_five_failures_require_the_password() {
        let b = Some(&b"boot-1"[..]);
        let mut r = rec();
        for n in 0..5 {
            r.failure_count = n;
            assert_eq!(eval(&r, T0, b), Ok(()), "{n} failures");
        }
        r.failure_count = 5;
        assert_eq!(eval(&r, T0, b), Err(QuickUnlockDenied::TooManyFailures));
        r.failure_count = u32::MAX;
        assert_eq!(eval(&r, T0, b), Err(QuickUnlockDenied::TooManyFailures));
        let strict = QuickUnlockConfig {
            max_failures: 1,
            ..QuickUnlockConfig::default()
        };
        r.failure_count = 1;
        assert_eq!(
            evaluate(&r, T0, b, &strict),
            Err(QuickUnlockDenied::TooManyFailures)
        );
    }

    #[test]
    fn defaults_match_docs_04_section_8() {
        let c = QuickUnlockConfig::default();
        assert_eq!(c.max_age_ms, 72 * HOUR);
        assert_eq!(c.max_failures, 5);
        assert!(c.require_password_after_reboot);
    }

    #[test]
    fn no_provider_supports_nothing_and_never_hands_out_a_key() {
        let p = NoProvider;
        assert!(!p.available());
        assert_eq!(p.kind(), QuickUnlockKind::None);
        assert_eq!(p.boot_id(), None);
        let vk = VaultKey::from_bytes([1; 32]);
        assert!(matches!(p.seal(&vk), Err(ProviderError::Unavailable)));
        let b = Blob::new(vec![1]).unwrap();
        assert!(matches!(p.unseal(&b), Err(ProviderError::Unavailable)));
    }

    #[test]
    fn kind_names_follow_docs_14() {
        assert_eq!(QuickUnlockKind::WindowsHello.as_str(), "windowsHello");
        assert_eq!(QuickUnlockKind::OsKeyring.as_str(), "osKeyring");
        assert_eq!(QuickUnlockKind::None.as_str(), "none");
    }
}
