//! HKDF-SHA256 key derivation (docs/04 §2, RFC 5869).
//!
//! * `KEK_pw = HKDF(salt = vault_id, ikm = MK, info = "aryavault/kek-pw/v1")`
//! * `KEK_rk = HKDF(salt = vault_id, ikm = RK, info = "aryavault/kek-rk/v1")`
//! * `SubKey = HKDF(salt = vault_id, ikm = VK, info = label ‖ epoch_be32)`
//!
//! **`info` encoding for sub-keys (specified here, tested below):** the ASCII bytes of
//! the label (`"db/v1"`, `"log/v1"`, `"snapshot/v1"`, `"manifest/v1"`, `"history/v1"`)
//! followed directly by the key epoch as a 4-byte **big-endian** unsigned integer. Labels
//! come from a closed enum, and no label is a prefix of another, so the concatenation is
//! unambiguous. All outputs are 32 bytes (one SHA-256 block of HKDF-Expand).

use hkdf::Hkdf;
use sha2::Sha256;
use thiserror::Error;

use crate::VaultId;
use crate::keys::{Kek, MasterKey, RecoveryKey, SubKey, VaultKey};
use crate::secret::Secret;

/// `info` for `KEK_pw` (docs/04 §2).
pub const INFO_KEK_PW: &[u8] = b"aryavault/kek-pw/v1";
/// `info` for `KEK_rk` (docs/04 §2).
pub const INFO_KEK_RK: &[u8] = b"aryavault/kek-rk/v1";

/// Error from key derivation. HKDF-Expand to 32 bytes cannot fail for SHA-256; this
/// exists so no code path has to panic or fall back to a zero key.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("key derivation failed")]
pub struct HkdfError;

/// The purpose labels for vault-key sub-keys (docs/04 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SubKeyLabel {
    /// `K_db` (`"db/v1"`): SQLCipher raw key.
    Db,
    /// `K_log` (`"log/v1"`): op-log segments.
    Log,
    /// `K_snap` (`"snapshot/v1"`): snapshots.
    Snapshot,
    /// `K_manifest` (`"manifest/v1"`): device manifests.
    Manifest,
    /// `K_item_hist` (`"history/v1"`): reserved.
    History,
}

impl SubKeyLabel {
    /// All labels, in doc 04 §2 order.
    pub const ALL: [SubKeyLabel; 5] = [
        SubKeyLabel::Db,
        SubKeyLabel::Log,
        SubKeyLabel::Snapshot,
        SubKeyLabel::Manifest,
        SubKeyLabel::History,
    ];

    /// The label string exactly as written in doc 04 §2.
    pub const fn as_str(self) -> &'static str {
        match self {
            SubKeyLabel::Db => "db/v1",
            SubKeyLabel::Log => "log/v1",
            SubKeyLabel::Snapshot => "snapshot/v1",
            SubKeyLabel::Manifest => "manifest/v1",
            SubKeyLabel::History => "history/v1",
        }
    }
}

/// Builds the HKDF `info` for a sub-key: `label ‖ epoch (u32, big-endian)`.
pub fn subkey_info(label: SubKeyLabel, epoch: u32) -> Vec<u8> {
    let mut info = Vec::with_capacity(label.as_str().len() + 4);
    info.extend_from_slice(label.as_str().as_bytes());
    info.extend_from_slice(&epoch.to_be_bytes());
    info
}

fn derive32(salt: &VaultId, ikm: &[u8], info: &[u8]) -> Result<Secret<32>, HkdfError> {
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut out = Secret::<32>::zeroed();
    hk.expand(info, out.as_mut_bytes()).map_err(|_| HkdfError)?;
    Ok(out)
}

/// Derives `KEK_pw` from the master key.
pub fn kek_pw(mk: &MasterKey, vault_id: &VaultId) -> Result<Kek, HkdfError> {
    derive32(vault_id, mk.expose_secret(), INFO_KEK_PW).map(Kek::from_secret)
}

/// Derives `KEK_rk` from the recovery key (160 bits of entropy, so no Argon2 is needed).
pub fn kek_rk(rk: &RecoveryKey, vault_id: &VaultId) -> Result<Kek, HkdfError> {
    derive32(vault_id, rk.expose_secret(), INFO_KEK_RK).map(Kek::from_secret)
}

/// Derives the purpose-specific sub-key for `label` at key `epoch` from the vault key.
pub fn subkey(
    vk: &VaultKey,
    vault_id: &VaultId,
    label: SubKeyLabel,
    epoch: u32,
) -> Result<SubKey, HkdfError> {
    derive32(vault_id, vk.expose_secret(), &subkey_info(label, epoch)).map(SubKey::from_secret)
}

#[cfg(test)]
pub(crate) fn hkdf_raw(salt: Option<&[u8]>, ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut out = vec![0u8; len];
    hk.expand(info, &mut out).unwrap();
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    // SEC-C07: RFC 5869 Appendix A, test cases 1-3 (SHA-256). Typed from RFC 5869 (case 1 PRK/OKM,
    // cases 2-3 OKM) and checked against the `hkdf` crate's own suite (tests/rfc5869.rs).
    #[test]
    fn sec_c07_rfc5869_case1_basic() {
        let ikm = [0x0b; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let okm = hkdf_raw(Some(&salt), &ikm, &info, 42);
        assert_eq!(
            hex::encode(okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
        let (prk, _) = Hkdf::<Sha256>::extract(Some(&salt), &ikm);
        assert_eq!(
            hex::encode(prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );
    }

    #[test]
    fn sec_c07_rfc5869_case2_longer_inputs() {
        let ikm: Vec<u8> = (0x00..=0x4f).collect();
        let salt: Vec<u8> = (0x60..=0xaf).collect();
        let info: Vec<u8> = (0xb0..=0xff).collect();
        let okm = hkdf_raw(Some(&salt), &ikm, &info, 82);
        assert_eq!(
            hex::encode(okm),
            "b11e398dc80327a1c8e7f78c596a49344f012eda2d4efad8a050cc4c19afa97c\
59045a99cac7827271cb41c65e590e09da3275600c2f09b8367793a9aca3db71\
cc30c58179ec3e87c14c01d5c1f3434f1d87"
        );
    }

    #[test]
    fn sec_c07_rfc5869_case3_empty_salt_and_info() {
        let ikm = [0x0b; 22];
        let okm = hkdf_raw(None, &ikm, &[], 42);
        assert_eq!(
            hex::encode(okm),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
        );
    }

    const VID: VaultId = [0x11; 16];

    #[test]
    fn labels_are_exactly_the_spec_strings() {
        let got: Vec<&str> = SubKeyLabel::ALL.iter().map(|l| l.as_str()).collect();
        assert_eq!(
            got,
            [
                "db/v1",
                "log/v1",
                "snapshot/v1",
                "manifest/v1",
                "history/v1"
            ]
        );
    }

    #[test]
    fn subkey_info_encoding_is_label_then_epoch_big_endian() {
        assert_eq!(subkey_info(SubKeyLabel::Db, 1), b"db/v1\x00\x00\x00\x01");
        assert_eq!(
            subkey_info(SubKeyLabel::Snapshot, 0x0102_0304),
            b"snapshot/v1\x01\x02\x03\x04"
        );
    }

    #[test]
    fn subkey_matches_independent_hkdf_computation() {
        let vk = VaultKey::from_bytes([0x42; 32]);
        for label in SubKeyLabel::ALL {
            let got = subkey(&vk, &VID, label, 7).unwrap();
            let want = hkdf_raw(Some(&VID), &[0x42; 32], &subkey_info(label, 7), 32);
            assert_eq!(got.expose_secret().as_slice(), want.as_slice(), "{label:?}");
        }
    }

    #[test]
    fn subkeys_differ_by_label_epoch_vault_id_and_vk() {
        let vk = VaultKey::from_bytes([0x42; 32]);
        let mut seen = HashSet::new();
        for label in SubKeyLabel::ALL {
            for epoch in [0u32, 1, 2, 0x0100, u32::MAX] {
                let k = subkey(&vk, &VID, label, epoch).unwrap();
                assert!(
                    seen.insert(*k.expose_secret()),
                    "collision at {label:?}/{epoch}"
                );
            }
        }
        let other_vid = subkey(&vk, &[0x12; 16], SubKeyLabel::Db, 0).unwrap();
        assert!(!seen.contains(other_vid.expose_secret()));
        let other_vk = subkey(&VaultKey::from_bytes([0x43; 32]), &VID, SubKeyLabel::Db, 0).unwrap();
        assert!(!seen.contains(other_vk.expose_secret()));
    }

    #[test]
    fn kek_pw_and_kek_rk_match_spec_construction_and_are_domain_separated() {
        let mk = MasterKey::from_bytes([0x33; 32]);
        let got = kek_pw(&mk, &VID).unwrap();
        let want = hkdf_raw(Some(&VID), &[0x33; 32], b"aryavault/kek-pw/v1", 32);
        assert_eq!(got.expose_secret().as_slice(), want.as_slice());

        let rk = RecoveryKey::from_bytes([0x33; 20]);
        let got = kek_rk(&rk, &VID).unwrap();
        let want = hkdf_raw(Some(&VID), &[0x33; 20], b"aryavault/kek-rk/v1", 32);
        assert_eq!(got.expose_secret().as_slice(), want.as_slice());

        // Same ikm prefix, different info => different keys.
        let a = kek_pw(&MasterKey::from_bytes([0x33; 32]), &VID).unwrap();
        let b = kek_rk(&RecoveryKey::from_bytes([0x33; 20]), &VID).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
    }
}
