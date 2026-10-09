//! Vault-key lifecycle: creation, unlock and password change (docs/04 §2, §9).
//!
//! The vault key (VK) is random (OS CSPRNG) and **never derived from the password**.
//! Changing the master password re-wraps the same VK under a new `KEK_pw`; `wrap_rk` is
//! carried over byte-for-byte, so no recovery key is needed and no data is re-encrypted
//! (SEC-C12). Header (de)serialization and `header_version` handling belong to T02.

use thiserror::Error;

use crate::VaultId;
use crate::hkdf::{self, HkdfError};
use crate::kdf::{KdfError, KdfParams, derive_master_key};
use crate::keys::{RecoveryKey, VaultKey};
use crate::recovery_key;
use crate::rng::{Rng, RngError, random_array};
use crate::secret::Secret;
use crate::wrap::{self, WrapError, WrappedKey};

/// The cryptographic fields of a vault header (docs/04 §5) that the wraps depend on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderWraps {
    /// Vault identifier (HKDF salt and AAD component).
    pub vault_id: VaultId,
    /// Key epoch (AAD component).
    pub epoch: u32,
    /// Argon2id parameters used for `wrap_pw`.
    pub kdf: KdfParams,
    /// `AEAD(KEK_pw, VK)`.
    pub wrap_pw: WrappedKey,
    /// `AEAD(KEK_rk, VK)`.
    pub wrap_rk: WrappedKey,
}

/// Result of [`create_vault`].
pub struct NewVault {
    /// The freshly generated vault key.
    pub vault_key: VaultKey,
    /// The recovery key to show the user once.
    pub recovery_key: RecoveryKey,
    /// The wraps to store in the header.
    pub wraps: HeaderWraps,
}

/// Errors from vault-key operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultKeyError {
    /// KDF parameter or derivation problem.
    #[error(transparent)]
    Kdf(#[from] KdfError),
    /// Wrong password / recovery key, or a tampered header (indistinguishable).
    #[error(transparent)]
    Wrap(#[from] WrapError),
    /// Key-derivation (HKDF) failure.
    #[error(transparent)]
    Hkdf(#[from] HkdfError),
    /// Random number generator failure.
    #[error(transparent)]
    Rng(#[from] RngError),
    /// A password change must use new KDF parameters with a fresh salt (docs/04 §9).
    #[error("a password change requires a fresh KDF salt")]
    SaltNotRefreshed,
}

/// Generates a random 256-bit vault key from `rng`.
pub fn generate_vault_key(rng: &mut dyn Rng) -> Result<VaultKey, RngError> {
    let mut s = Secret::<32>::zeroed();
    rng.fill_bytes(s.as_mut_bytes())?;
    Ok(VaultKey::from_bytes(*s.as_bytes()))
}

/// Creates a new vault: random `vault_id`, VK and recovery key, wrapped under the
/// password-derived and recovery-derived KEKs at the given key `epoch`.
///
/// `kdf` must carry a fresh random salt (e.g. from [`crate::kdf::calibrate`]) and is
/// validated before use. The initial epoch is a caller decision (doc 04 does not fix it).
pub fn create_vault(
    password: &str,
    kdf: KdfParams,
    epoch: u32,
    rng: &mut dyn Rng,
) -> Result<NewVault, VaultKeyError> {
    let vault_id: VaultId = random_array(rng)?;
    let vault_key = generate_vault_key(rng)?;
    let recovery_key = recovery_key::generate(rng)?;

    let mk = derive_master_key(password, &kdf)?;
    let kek_pw = hkdf::kek_pw(&mk, &vault_id)?;
    let kek_rk = hkdf::kek_rk(&recovery_key, &vault_id)?;
    let wrap_pw = wrap::wrap_pw(&kek_pw, &vault_id, epoch, &kdf, &vault_key, rng)?;
    let wrap_rk = wrap::wrap_rk(&kek_rk, &vault_id, epoch, &vault_key, rng)?;
    Ok(NewVault {
        vault_key,
        recovery_key,
        wraps: HeaderWraps {
            vault_id,
            epoch,
            kdf,
            wrap_pw,
            wrap_rk,
        },
    })
}

/// Unlocks the vault key with the master password.
///
/// KDF parameters from the (untrusted) header are range-checked before hashing. A wrong
/// password yields `Wrap(AuthenticationFailed)`.
pub fn unlock_with_password(
    password: &str,
    wraps: &HeaderWraps,
) -> Result<VaultKey, VaultKeyError> {
    let mk = derive_master_key(password, &wraps.kdf)?;
    let kek = hkdf::kek_pw(&mk, &wraps.vault_id)?;
    Ok(wrap::unwrap_pw(
        &kek,
        &wraps.vault_id,
        wraps.epoch,
        &wraps.kdf,
        &wraps.wrap_pw,
    )?)
}

/// Unlocks the vault key with the recovery key. A wrong key yields
/// `Wrap(AuthenticationFailed)`.
pub fn unlock_with_recovery_key(
    rk: &RecoveryKey,
    wraps: &HeaderWraps,
) -> Result<VaultKey, VaultKeyError> {
    let kek = hkdf::kek_rk(rk, &wraps.vault_id)?;
    Ok(wrap::unwrap_rk(
        &kek,
        &wraps.vault_id,
        wraps.epoch,
        &wraps.wrap_rk,
    )?)
}

/// Changes the master password **without the recovery key** (docs/04 §9, SEC-C12).
///
/// Unwraps VK with `old_password`, derives a new `KEK_pw` from `new_password` under
/// `new_kdf` (new salt, validated), and returns wraps with a new `wrap_pw`. `wrap_rk`,
/// `vault_id` and `epoch` are carried over unchanged, so the recovery key still opens the
/// vault. The caller bumps `header_version` and publishes (T02).
///
/// This does not revoke a leaked old password: anyone holding an old header can still use
/// it (docs/04 §9); only key rotation does.
pub fn change_password(
    old_password: &str,
    new_password: &str,
    wraps: &HeaderWraps,
    new_kdf: KdfParams,
    rng: &mut dyn Rng,
) -> Result<HeaderWraps, VaultKeyError> {
    if new_kdf.salt == wraps.kdf.salt {
        return Err(VaultKeyError::SaltNotRefreshed);
    }
    new_kdf.validate()?;
    let vk = unlock_with_password(old_password, wraps)?;
    let mk = derive_master_key(new_password, &new_kdf)?;
    let kek = hkdf::kek_pw(&mk, &wraps.vault_id)?;
    let wrap_pw = wrap::wrap_pw(&kek, &wraps.vault_id, wraps.epoch, &new_kdf, &vk, rng)?;
    Ok(HeaderWraps {
        vault_id: wraps.vault_id,
        epoch: wraps.epoch,
        kdf: new_kdf,
        wrap_pw,
        wrap_rk: wraps.wrap_rk.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::OsRng;

    fn floor(salt: u8) -> KdfParams {
        KdfParams::floor([salt; 16])
    }

    #[test]
    fn generated_vault_keys_are_distinct_and_not_zero() {
        let a = generate_vault_key(&mut OsRng).unwrap();
        let b = generate_vault_key(&mut OsRng).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
        assert_ne!(a.expose_secret(), &[0u8; 32]);
    }

    /// Emits 0, 1, 2, ... so tests can see exactly which bytes each secret was drawn from.
    struct CountingRng(u8);
    impl Rng for CountingRng {
        fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
            for b in dest {
                *b = self.0;
                self.0 = self.0.wrapping_add(1);
            }
            Ok(())
        }
    }

    // SEC-C03 (functional): every random value in a new vault -- vault_id, VK, RK and both
    // wrap nonces -- comes from the injected Rng, in a fixed order, and VK is NOT derived
    // from the password. (A different password must give the same VK for the same stream.)
    #[test]
    fn sec_c03_all_vault_randomness_is_drawn_from_the_rng() {
        let nv = create_vault("CANARY-a", floor(1), 1, &mut CountingRng(0)).unwrap();
        let seq = |from: u8, n: usize| -> Vec<u8> {
            (0..n).map(|i| from.wrapping_add(i as u8)).collect()
        };
        assert_eq!(nv.wraps.vault_id.as_slice(), seq(0, 16).as_slice());
        assert_eq!(
            nv.vault_key.expose_secret().as_slice(),
            seq(16, 32).as_slice()
        );
        assert_eq!(
            nv.recovery_key.expose_secret().as_slice(),
            seq(48, 20).as_slice()
        );
        assert_eq!(nv.wraps.wrap_pw.nonce.as_slice(), seq(68, 24).as_slice());
        assert_eq!(nv.wraps.wrap_rk.nonce.as_slice(), seq(92, 24).as_slice());
    }

    // One Argon2 run per derive; kept to the minimum number of derivations that proves
    // each property (debug-build Argon2 is slow).
    #[test]
    fn create_unlock_and_wrong_credentials() {
        let nv = create_vault("CANARY-correct horse", floor(1), 1, &mut OsRng).unwrap();
        let vk = unlock_with_password("CANARY-correct horse", &nv.wraps).unwrap();
        assert_eq!(vk.expose_secret(), nv.vault_key.expose_secret());
        let vk = unlock_with_recovery_key(&nv.recovery_key, &nv.wraps).unwrap();
        assert_eq!(vk.expose_secret(), nv.vault_key.expose_secret());

        // Wrong password / wrong recovery key => typed authentication error, no panic.
        assert_eq!(
            unlock_with_password("CANARY-wrong", &nv.wraps).err(),
            Some(VaultKeyError::Wrap(WrapError::AuthenticationFailed))
        );
        let other_rk = recovery_key::generate(&mut OsRng).unwrap();
        assert_eq!(
            unlock_with_recovery_key(&other_rk, &nv.wraps).err(),
            Some(VaultKeyError::Wrap(WrapError::AuthenticationFailed))
        );
    }

    #[test]
    fn unlock_rejects_hostile_header_kdf_before_hashing() {
        let nv = create_vault("CANARY-pw", floor(2), 1, &mut OsRng).unwrap();
        let mut hostile = nv.wraps.clone();
        hostile.kdf.m_kib = u32::MAX;
        let start = std::time::Instant::now();
        assert!(matches!(
            unlock_with_password("CANARY-pw", &hostile),
            Err(VaultKeyError::Kdf(KdfError::OutOfRange(_)))
        ));
        assert!(start.elapsed().as_millis() < 200);
    }

    // SEC-C12 integration: password change succeeds without the recovery key, the
    // recovery wrap is bit-identical and still opens, the old password no longer opens the
    // new header, and the new password does. VK is unchanged (no re-encryption needed).
    #[test]
    fn sec_c12_change_password_leaves_recovery_wrap_untouched_and_valid() {
        let nv = create_vault("CANARY-old password", floor(3), 1, &mut OsRng).unwrap();
        let new_kdf = floor(3).with_fresh_salt(&mut OsRng).unwrap();
        let changed = change_password(
            "CANARY-old password",
            "CANARY-new password",
            &nv.wraps,
            new_kdf,
            &mut OsRng,
        )
        .unwrap();

        // wrap_rk and identifiers untouched, byte for byte.
        assert_eq!(changed.wrap_rk, nv.wraps.wrap_rk);
        assert_eq!(changed.vault_id, nv.wraps.vault_id);
        assert_eq!(changed.epoch, nv.wraps.epoch);
        // wrap_pw and kdf (salt) actually changed.
        assert_ne!(changed.wrap_pw, nv.wraps.wrap_pw);
        assert_ne!(changed.kdf.salt, nv.wraps.kdf.salt);

        // New password opens the new header and yields the SAME vault key.
        let vk = unlock_with_password("CANARY-new password", &changed).unwrap();
        assert_eq!(vk.expose_secret(), nv.vault_key.expose_secret());
        // The recovery key still opens the vault against the NEW header.
        let vk = unlock_with_recovery_key(&nv.recovery_key, &changed).unwrap();
        assert_eq!(vk.expose_secret(), nv.vault_key.expose_secret());
        // The old password no longer opens the new header.
        assert_eq!(
            unlock_with_password("CANARY-old password", &changed).err(),
            Some(VaultKeyError::Wrap(WrapError::AuthenticationFailed))
        );
    }

    #[test]
    fn change_password_with_wrong_old_password_fails_and_requires_fresh_salt() {
        let nv = create_vault("CANARY-pw", floor(4), 1, &mut OsRng).unwrap();
        assert_eq!(
            change_password("CANARY-nope", "CANARY-new", &nv.wraps, floor(5), &mut OsRng).err(),
            Some(VaultKeyError::Wrap(WrapError::AuthenticationFailed))
        );
        assert_eq!(
            change_password("CANARY-pw", "CANARY-new", &nv.wraps, floor(4), &mut OsRng).err(),
            Some(VaultKeyError::SaltNotRefreshed)
        );
        let weak = KdfParams {
            m_kib: 8,
            ..floor(6)
        };
        assert!(matches!(
            change_password("CANARY-pw", "CANARY-new", &nv.wraps, weak, &mut OsRng),
            Err(VaultKeyError::Kdf(KdfError::OutOfRange(_)))
        ));
    }

    // Tampering with any header field the wrap_pw AAD binds must make the password unlock
    // fail (kdf fields are tampered within the valid range so validation passes).
    #[test]
    fn tampered_header_fields_fail_unlock() {
        let nv = create_vault("CANARY-pw", floor(7), 5, &mut OsRng).unwrap();
        let mut h = nv.wraps.clone();
        h.epoch += 1;
        assert!(unlock_with_password("CANARY-pw", &h).is_err());
        assert!(unlock_with_recovery_key(&nv.recovery_key, &h).is_err());
        let mut h = nv.wraps.clone();
        h.vault_id[0] ^= 1;
        assert!(unlock_with_password("CANARY-pw", &h).is_err());
        assert!(unlock_with_recovery_key(&nv.recovery_key, &h).is_err());
        let mut h = nv.wraps.clone();
        h.kdf.t += 1; // still in range, but different AAD and different key
        assert!(unlock_with_password("CANARY-pw", &h).is_err());
    }
}
