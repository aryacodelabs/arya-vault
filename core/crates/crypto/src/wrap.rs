//! Vault-key wrapping (docs/04 §5, SEC-C05, SEC-C12).
//!
//! The vault key is wrapped twice, independently, with XChaCha20-Poly1305:
//!
//! | wrap | key | AAD |
//! |---|---|---|
//! | `wrap_pw` | `KEK_pw` | `"aryavault/wrap-pw/v1" ‖ vault_id ‖ epoch ‖ canonical_cbor(kdf)` |
//! | `wrap_rk` | `KEK_rk` | `"aryavault/wrap-rk/v1" ‖ vault_id ‖ epoch` |
//!
//! `epoch` is encoded as a 4-byte big-endian integer and `vault_id` is the 16 raw bytes.
//! `header_version` is deliberately **not** part of either AAD (SEC-C12): a password
//! change bumps it, and `wrap_rk` must stay valid without the recovery key.

use thiserror::Error;

use crate::VaultId;
use crate::aead::{self, AeadError, Nonce, TAG_LEN};
use crate::kdf::KdfParams;
use crate::keys::{KEY_LEN, Kek, VaultKey};
use crate::rng::Rng;
use crate::secret::Secret;

/// AAD domain label for `wrap_pw`.
pub const WRAP_PW_LABEL: &[u8] = b"aryavault/wrap-pw/v1";
/// AAD domain label for `wrap_rk`.
pub const WRAP_RK_LABEL: &[u8] = b"aryavault/wrap-rk/v1";
/// Length of a wrapped vault key: 32-byte key plus 16-byte tag.
pub const WRAPPED_CT_LEN: usize = KEY_LEN + TAG_LEN;

/// A wrapped vault key as stored in the header: `{ nonce: 24 B, ct: 48 B }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedKey {
    /// Random 192-bit nonce.
    pub nonce: Nonce,
    /// `AEAD(KEK, VK)` ciphertext ‖ tag.
    pub ct: [u8; WRAPPED_CT_LEN],
}

/// Errors from wrapping/unwrapping.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum WrapError {
    /// Wrong key (password / recovery key) or any modified field. Indistinguishable by
    /// design.
    #[error("authentication failed")]
    AuthenticationFailed,
    /// Structurally invalid (e.g. an unwrapped key of the wrong length).
    #[error("malformed wrapped key")]
    Malformed,
    /// The random number generator failed.
    #[error("random number generator failure")]
    Rng,
}

impl From<AeadError> for WrapError {
    fn from(e: AeadError) -> Self {
        match e {
            AeadError::AuthenticationFailed => WrapError::AuthenticationFailed,
            AeadError::Malformed => WrapError::Malformed,
            AeadError::Rng(_) => WrapError::Rng,
        }
    }
}

/// AAD for `wrap_pw`: label ‖ vault_id ‖ epoch (BE32) ‖ canonical CBOR of `kdf`.
pub fn wrap_pw_aad(vault_id: &VaultId, epoch: u32, kdf: &KdfParams) -> Vec<u8> {
    let cbor = kdf.canonical_cbor();
    let mut aad = Vec::with_capacity(WRAP_PW_LABEL.len() + 16 + 4 + cbor.len());
    aad.extend_from_slice(WRAP_PW_LABEL);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(&epoch.to_be_bytes());
    aad.extend_from_slice(&cbor);
    aad
}

/// AAD for `wrap_rk`: label ‖ vault_id ‖ epoch (BE32). No `header_version`.
pub fn wrap_rk_aad(vault_id: &VaultId, epoch: u32) -> Vec<u8> {
    let mut aad = Vec::with_capacity(WRAP_RK_LABEL.len() + 16 + 4);
    aad.extend_from_slice(WRAP_RK_LABEL);
    aad.extend_from_slice(vault_id);
    aad.extend_from_slice(&epoch.to_be_bytes());
    aad
}

fn wrap(kek: &Kek, aad: &[u8], vk: &VaultKey, rng: &mut dyn Rng) -> Result<WrappedKey, WrapError> {
    let (nonce, ct) = aead::seal(kek, aad, vk.expose_secret(), rng)?;
    let ct: [u8; WRAPPED_CT_LEN] = ct.try_into().map_err(|_| WrapError::Malformed)?;
    Ok(WrappedKey { nonce, ct })
}

fn unwrap(kek: &Kek, aad: &[u8], wrapped: &WrappedKey) -> Result<VaultKey, WrapError> {
    let pt = aead::open(kek, aad, &wrapped.nonce, &wrapped.ct)?;
    let mut key = Secret::<KEY_LEN>::zeroed();
    if pt.len() != KEY_LEN {
        return Err(WrapError::Malformed);
    }
    key.as_mut_bytes().copy_from_slice(&pt);
    Ok(VaultKey::from_bytes(*key.as_bytes()))
}

/// Wraps `vk` under `KEK_pw`, binding `(vault_id, epoch, kdf)`.
pub fn wrap_pw(
    kek_pw: &Kek,
    vault_id: &VaultId,
    epoch: u32,
    kdf: &KdfParams,
    vk: &VaultKey,
    rng: &mut dyn Rng,
) -> Result<WrappedKey, WrapError> {
    wrap(kek_pw, &wrap_pw_aad(vault_id, epoch, kdf), vk, rng)
}

/// Unwraps the vault key from `wrap_pw`. Fails with
/// [`WrapError::AuthenticationFailed`] for a wrong password or any altered
/// `vault_id`/`epoch`/`kdf`/`nonce`/`ct`.
pub fn unwrap_pw(
    kek_pw: &Kek,
    vault_id: &VaultId,
    epoch: u32,
    kdf: &KdfParams,
    wrapped: &WrappedKey,
) -> Result<VaultKey, WrapError> {
    unwrap(kek_pw, &wrap_pw_aad(vault_id, epoch, kdf), wrapped)
}

/// Wraps `vk` under `KEK_rk`, binding `(vault_id, epoch)` only.
pub fn wrap_rk(
    kek_rk: &Kek,
    vault_id: &VaultId,
    epoch: u32,
    vk: &VaultKey,
    rng: &mut dyn Rng,
) -> Result<WrappedKey, WrapError> {
    wrap(kek_rk, &wrap_rk_aad(vault_id, epoch), vk, rng)
}

/// Unwraps the vault key from `wrap_rk`.
pub fn unwrap_rk(
    kek_rk: &Kek,
    vault_id: &VaultId,
    epoch: u32,
    wrapped: &WrappedKey,
) -> Result<VaultKey, WrapError> {
    unwrap(kek_rk, &wrap_rk_aad(vault_id, epoch), wrapped)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::aead::NONCE_LEN;
    use crate::rng::OsRng;

    const VID: VaultId = [0x5a; 16];

    fn kek(b: u8) -> Kek {
        Kek::from_secret(Secret::new([b; 32]))
    }
    fn kdf() -> KdfParams {
        KdfParams {
            m_kib: 65_536,
            t: 3,
            p: 1,
            salt: [0x77; 16],
        }
    }
    fn vk() -> VaultKey {
        VaultKey::from_bytes([0xC4; 32])
    }

    #[test]
    fn aad_layouts_match_spec() {
        let aad = wrap_rk_aad(&VID, 0x0102_0304);
        let mut want = b"aryavault/wrap-rk/v1".to_vec();
        want.extend_from_slice(&[0x5a; 16]);
        want.extend_from_slice(&[1, 2, 3, 4]);
        assert_eq!(aad, want);

        let aad = wrap_pw_aad(&VID, 9, &kdf());
        let mut want = b"aryavault/wrap-pw/v1".to_vec();
        want.extend_from_slice(&[0x5a; 16]);
        want.extend_from_slice(&[0, 0, 0, 9]);
        want.extend_from_slice(&kdf().canonical_cbor());
        assert_eq!(aad, want);
    }

    // SEC-C12: header_version is not an input to either AAD (the functions do not even
    // accept one), and the two AADs are domain-separated.
    #[test]
    fn sec_c12_aads_have_no_header_version_input_and_differ() {
        assert_ne!(wrap_pw_aad(&VID, 1, &kdf()), wrap_rk_aad(&VID, 1));
        assert!(wrap_pw_aad(&VID, 1, &kdf()).starts_with(WRAP_PW_LABEL));
        assert!(wrap_rk_aad(&VID, 1).starts_with(WRAP_RK_LABEL));
    }

    #[test]
    fn wrap_pw_round_trip() {
        let w = wrap_pw(&kek(1), &VID, 3, &kdf(), &vk(), &mut OsRng).unwrap();
        let out = unwrap_pw(&kek(1), &VID, 3, &kdf(), &w).unwrap();
        assert_eq!(out.expose_secret(), vk().expose_secret());
    }

    #[test]
    fn wrap_rk_round_trip() {
        let w = wrap_rk(&kek(2), &VID, 3, &vk(), &mut OsRng).unwrap();
        let out = unwrap_rk(&kek(2), &VID, 3, &w).unwrap();
        assert_eq!(out.expose_secret(), vk().expose_secret());
    }

    #[test]
    fn wrong_kek_is_a_typed_authentication_error() {
        let w = wrap_pw(&kek(1), &VID, 3, &kdf(), &vk(), &mut OsRng).unwrap();
        assert_eq!(
            unwrap_pw(&kek(9), &VID, 3, &kdf(), &w).err(),
            Some(WrapError::AuthenticationFailed)
        );
        let w = wrap_rk(&kek(2), &VID, 3, &vk(), &mut OsRng).unwrap();
        assert_eq!(
            unwrap_rk(&kek(9), &VID, 3, &w).err(),
            Some(WrapError::AuthenticationFailed)
        );
    }

    // SEC-C05 / negative tests: flip every byte/field of nonce, ct (incl. tag), vault_id,
    // epoch and each kdf field; unwrap must fail.
    #[test]
    fn sec_c05_wrap_pw_rejects_every_tampered_field() {
        let w = wrap_pw(&kek(1), &VID, 3, &kdf(), &vk(), &mut OsRng).unwrap();
        assert!(unwrap_pw(&kek(1), &VID, 3, &kdf(), &w).is_ok());
        let fails = |vid: &VaultId, e: u32, k: &KdfParams, w: &WrappedKey| {
            unwrap_pw(&kek(1), vid, e, k, w).err() == Some(WrapError::AuthenticationFailed)
        };
        for i in 0..NONCE_LEN {
            let mut x = w.clone();
            x.nonce[i] ^= 1;
            assert!(fails(&VID, 3, &kdf(), &x), "nonce[{i}]");
        }
        for i in 0..WRAPPED_CT_LEN {
            let mut x = w.clone();
            x.ct[i] ^= 1;
            assert!(fails(&VID, 3, &kdf(), &x), "ct[{i}]");
        }
        for i in 0..16 {
            let mut v = VID;
            v[i] ^= 1;
            assert!(fails(&v, 3, &kdf(), &w), "vault_id[{i}]");
        }
        for bit in 0..32 {
            assert!(fails(&VID, 3 ^ (1 << bit), &kdf(), &w), "epoch bit {bit}");
        }
        let mut k = kdf();
        k.m_kib += 1;
        assert!(fails(&VID, 3, &k, &w), "m_kib");
        let mut k = kdf();
        k.t += 1;
        assert!(fails(&VID, 3, &k, &w), "t");
        let mut k = kdf();
        k.p += 1;
        assert!(fails(&VID, 3, &k, &w), "p");
        for i in 0..16 {
            let mut k = kdf();
            k.salt[i] ^= 1;
            assert!(fails(&VID, 3, &k, &w), "salt[{i}]");
        }
    }

    #[test]
    fn sec_c05_wrap_rk_rejects_every_tampered_field() {
        let w = wrap_rk(&kek(2), &VID, 3, &vk(), &mut OsRng).unwrap();
        let fails = |vid: &VaultId, e: u32, w: &WrappedKey| {
            unwrap_rk(&kek(2), vid, e, w).err() == Some(WrapError::AuthenticationFailed)
        };
        for i in 0..NONCE_LEN {
            let mut x = w.clone();
            x.nonce[i] ^= 1;
            assert!(fails(&VID, 3, &x), "nonce[{i}]");
        }
        for i in 0..WRAPPED_CT_LEN {
            let mut x = w.clone();
            x.ct[i] ^= 1;
            assert!(fails(&VID, 3, &x), "ct[{i}]");
        }
        for i in 0..16 {
            let mut v = VID;
            v[i] ^= 1;
            assert!(fails(&v, 3, &w), "vault_id[{i}]");
        }
        for bit in 0..32 {
            assert!(fails(&VID, 3 ^ (1 << bit), &w), "epoch bit {bit}");
        }
    }

    #[test]
    fn pw_and_rk_wraps_are_not_interchangeable() {
        // Same KEK bytes, same ids: the separate AADs still prevent cross-use.
        let w_pw = wrap_pw(&kek(1), &VID, 3, &kdf(), &vk(), &mut OsRng).unwrap();
        assert!(unwrap_rk(&kek(1), &VID, 3, &w_pw).is_err());
        let w_rk = wrap_rk(&kek(1), &VID, 3, &vk(), &mut OsRng).unwrap();
        assert!(unwrap_pw(&kek(1), &VID, 3, &kdf(), &w_rk).is_err());
    }

    #[test]
    fn wrapping_twice_uses_fresh_nonces() {
        let a = wrap_rk(&kek(2), &VID, 3, &vk(), &mut OsRng).unwrap();
        let b = wrap_rk(&kek(2), &VID, 3, &vk(), &mut OsRng).unwrap();
        assert_ne!(a.nonce, b.nonce);
        assert_ne!(a.ct, b.ct);
    }
}
