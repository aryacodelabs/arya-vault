//! KDF profiles (docs/04 §3): which Argon2id parameters a new password wrap uses.

use arya_vault_crypto::kdf::{self, KdfParams, M_KIB_CALIBRATION_CAP};
use arya_vault_crypto::rng::{OsRng, Rng};

use crate::error::Result;

/// Memory cap of the `High` profile (512 MiB; `Default` uses the low-RAM cap of 256 MiB).
const HIGH_PROFILE_MAX_M_KIB: u32 = 512 * 1024;

/// How expensive the Argon2id parameters of a new wrap should be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdfProfile {
    /// The floor: 64 MiB, t = 3, p = 1 (fast; for tests and very small devices).
    Low,
    /// Calibrated on this device for about 0.75 s, memory capped at 256 MiB.
    Default,
    /// Calibrated on this device for about 1.5 s, memory capped at 512 MiB.
    High,
}

impl KdfProfile {
    /// Parameters for this profile with a fresh random salt.
    ///
    /// `Default` and `High` run [`kdf::calibrate`], which takes a few seconds.
    ///
    /// # Errors
    /// Random source or calibration failures.
    pub fn params(self) -> Result<KdfParams> {
        Ok(match self {
            Self::Low => KdfParams::floor(random_array(&mut OsRng)?),
            Self::Default => kdf::calibrate(750, M_KIB_CALIBRATION_CAP, &mut OsRng)?,
            Self::High => kdf::calibrate(1500, HIGH_PROFILE_MAX_M_KIB, &mut OsRng)?,
        })
    }
}

/// `N` random bytes from the OS CSPRNG (via the crypto crate's `OsRng`).
pub(crate) fn random_array<const N: usize>(rng: &mut dyn Rng) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    rng.fill_bytes(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_is_the_floor_with_a_fresh_salt() {
        let a = KdfProfile::Low.params().unwrap();
        let b = KdfProfile::Low.params().unwrap();
        assert_eq!((a.m_kib, a.t, a.p), (64 * 1024, 3, 1));
        assert_ne!(a.salt, b.salt);
        a.validate().unwrap();
    }
}
