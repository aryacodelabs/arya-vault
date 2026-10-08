//! XChaCha20-Poly1305 wrapper (docs/04 §1, SEC-C04).
//!
//! * [`seal`] draws a fresh random 192-bit nonce from the supplied [`Rng`] on every call;
//!   there is no API to supply or reuse a nonce, so nonce reuse by callers is impossible.
//! * [`open`] verifies the Poly1305 tag in constant time (inside the `chacha20poly1305`
//!   crate) before releasing any plaintext.
//! * Errors distinguish *authentication failure* from *malformed input* (wrong nonce or
//!   ciphertext length) but never reveal which byte differed.
//!
//! The AAD is caller-supplied and authenticated, not encrypted. Binding of context
//! (vault id, epoch, ...) into the AAD is the caller's responsibility (SEC-C05).

use chacha20poly1305::aead::AeadInOut;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::keys::{Kek, SubKey};
use crate::rng::{Rng, RngError, random_array};

/// Nonce length in bytes (192 bits).
pub const NONCE_LEN: usize = 24;
/// Poly1305 tag length in bytes.
pub const TAG_LEN: usize = 16;

/// A 192-bit AEAD nonce.
pub type Nonce = [u8; NONCE_LEN];

/// Errors from AEAD operations.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AeadError {
    /// The tag did not verify: wrong key, wrong AAD, or modified data. No further detail
    /// is given by design.
    #[error("authentication failed")]
    AuthenticationFailed,
    /// The input is structurally invalid (e.g. shorter than a tag, or an unexpected
    /// plaintext length) so no authentication was attempted.
    #[error("malformed input")]
    Malformed,
    /// The random number generator failed while drawing a nonce.
    #[error("random number generator failure")]
    Rng(#[from] RngError),
}

mod sealed {
    pub trait Sealed {
        fn key_bytes(&self) -> &[u8; 32];
    }
}

/// Keys that may be used with [`seal`]/[`open`]. Sealed: implemented only for [`Kek`]
/// and [`SubKey`] so that a vault key or password-derived key is never used directly as a
/// data-encryption key.
pub trait AeadKey: sealed::Sealed {}

impl sealed::Sealed for Kek {
    fn key_bytes(&self) -> &[u8; 32] {
        self.expose_secret()
    }
}
impl AeadKey for Kek {}

impl sealed::Sealed for SubKey {
    fn key_bytes(&self) -> &[u8; 32] {
        self.expose_secret()
    }
}
impl AeadKey for SubKey {}

/// Encrypts `plaintext` under `key`, authenticating `aad`.
///
/// Returns `(nonce, ciphertext ‖ tag)`. The nonce is freshly drawn from `rng` (the OS
/// CSPRNG in production) for every call and must be stored next to the ciphertext.
pub fn seal<K: AeadKey>(
    key: &K,
    aad: &[u8],
    plaintext: &[u8],
    rng: &mut dyn Rng,
) -> Result<(Nonce, Vec<u8>), AeadError> {
    let nonce: Nonce = random_array(rng)?;
    let cipher = XChaCha20Poly1305::new(key.key_bytes().into());
    // Encrypt in a wiped-on-drop buffer so the plaintext copy does not linger.
    let mut buf = Zeroizing::new(Vec::with_capacity(plaintext.len() + TAG_LEN));
    buf.extend_from_slice(plaintext);
    cipher
        .encrypt_in_place(&nonce.into(), aad, &mut *buf)
        .map_err(|_| AeadError::Malformed)?;
    Ok((nonce, buf.to_vec()))
}

/// Decrypts and authenticates `ciphertext` (`ct ‖ tag`).
///
/// Returns [`AeadError::Malformed`] if `ciphertext` is shorter than a tag, and
/// [`AeadError::AuthenticationFailed`] for any verification failure. The plaintext is
/// returned in a buffer that is wiped on drop.
pub fn open<K: AeadKey>(
    key: &K,
    aad: &[u8],
    nonce: &Nonce,
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, AeadError> {
    if ciphertext.len() < TAG_LEN {
        return Err(AeadError::Malformed);
    }
    let cipher = XChaCha20Poly1305::new(key.key_bytes().into());
    let mut buf = Zeroizing::new(ciphertext.to_vec());
    cipher
        .decrypt_in_place(&(*nonce).into(), aad, &mut *buf)
        .map_err(|_| AeadError::AuthenticationFailed)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::OsRng;
    use crate::secret::Secret;
    use proptest::prelude::*;
    use std::collections::HashSet;

    fn key(b: u8) -> SubKey {
        SubKey::from_secret(Secret::new([b; 32]))
    }

    // SEC-C07: draft-irtf-cfrg-xchacha appendix A.1 (also RFC 8439 §2.8.2 key/aad/plaintext).
    // Vector source: the `chacha20poly1305` crate's own test suite (tests/lib.rs,
    // `mod xchacha20`), which cites draft-irtf-cfrg-xchacha-03 Appendix A.1.
    // This test drives the underlying cipher with the fixed nonce, because `seal` always
    // draws a fresh random nonce by design; `open` is tested through the public API.
    const KAT_KEY: [u8; 32] = [
        0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8a, 0x8b, 0x8c, 0x8d, 0x8e,
        0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9a, 0x9b, 0x9c, 0x9d,
        0x9e, 0x9f,
    ];
    const KAT_AAD: [u8; 12] = [
        0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7,
    ];
    const KAT_PLAINTEXT: &[u8] = b"Ladies and Gentlemen of the class of '99: \
        If I could offer you only one tip for the future, sunscreen would be it.";
    const KAT_NONCE: Nonce = [
        0x40, 0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e,
        0x4f, 0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57,
    ];
    const KAT_CT: &str = "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb\
731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b4522f8c9ba40db5d945b11b69b9\
82c1bb9e3f3fac2bc369488f76b2383565d3fff921f9664c97637da9768812f615c68b13b52e";
    const KAT_TAG: &str = "c0875924c1c7987947deafd8780acf49";

    #[test]
    fn sec_c07_xchacha20poly1305_known_answer() {
        let expected_ct = hex::decode(KAT_CT).unwrap();
        let expected_tag = hex::decode(KAT_TAG).unwrap();
        // Encrypt with the raw cipher at the fixed nonce and compare to the draft vector.
        let cipher = XChaCha20Poly1305::new((&KAT_KEY).into());
        let mut buf = KAT_PLAINTEXT.to_vec();
        cipher
            .encrypt_in_place(&KAT_NONCE.into(), &KAT_AAD, &mut buf)
            .unwrap();
        let (ct, tag) = buf.split_at(KAT_PLAINTEXT.len());
        assert_eq!(ct, expected_ct.as_slice());
        assert_eq!(tag, expected_tag.as_slice());
        // And the public `open` accepts the published ciphertext||tag.
        let k = SubKey::from_secret(Secret::new(KAT_KEY));
        let pt = open(&k, &KAT_AAD, &KAT_NONCE, &buf).unwrap();
        assert_eq!(pt.as_slice(), KAT_PLAINTEXT);
    }

    #[test]
    fn round_trip() {
        let k = key(1);
        let (nonce, ct) = seal(&k, b"aad", b"CANARY-secret", &mut OsRng).unwrap();
        assert_eq!(ct.len(), b"CANARY-secret".len() + TAG_LEN);
        assert_eq!(
            open(&k, b"aad", &nonce, &ct).unwrap().as_slice(),
            b"CANARY-secret"
        );
    }

    #[test]
    fn empty_plaintext_round_trip() {
        let k = key(1);
        let (nonce, ct) = seal(&k, b"", b"", &mut OsRng).unwrap();
        assert_eq!(ct.len(), TAG_LEN);
        assert!(open(&k, b"", &nonce, &ct).unwrap().is_empty());
    }

    // Negative tests: flipping any bit of any byte of nonce / ciphertext / tag / AAD, or
    // using a different key, must fail with AuthenticationFailed.
    #[test]
    fn flipping_every_byte_of_nonce_ct_tag_and_aad_fails() {
        let k = key(2);
        let aad = b"CANARY-aad-0123456789";
        let (nonce, ct) = seal(&k, aad, b"CANARY-plaintext-0123456789", &mut OsRng).unwrap();
        assert!(open(&k, aad, &nonce, &ct).is_ok());
        for i in 0..NONCE_LEN {
            let mut n = nonce;
            n[i] ^= 0x01;
            assert_eq!(
                open(&k, aad, &n, &ct).err().unwrap(),
                AeadError::AuthenticationFailed,
                "nonce[{i}]"
            );
        }
        // Covers the ciphertext body and the 16 tag bytes (the tail of `ct`).
        for i in 0..ct.len() {
            let mut c = ct.clone();
            c[i] ^= 0x80;
            assert_eq!(
                open(&k, aad, &nonce, &c).err().unwrap(),
                AeadError::AuthenticationFailed,
                "ct[{i}]"
            );
        }
        for i in 0..aad.len() {
            let mut a = aad.to_vec();
            a[i] ^= 0x01;
            assert_eq!(
                open(&k, &a, &nonce, &ct).err().unwrap(),
                AeadError::AuthenticationFailed,
                "aad[{i}]"
            );
        }
        // Truncated / extended AAD.
        assert_eq!(
            open(&k, &aad[..aad.len() - 1], &nonce, &ct).err().unwrap(),
            AeadError::AuthenticationFailed
        );
        let mut longer = aad.to_vec();
        longer.push(0);
        assert_eq!(
            open(&k, &longer, &nonce, &ct).err().unwrap(),
            AeadError::AuthenticationFailed
        );
        assert_eq!(
            open(&key(3), aad, &nonce, &ct).err().unwrap(),
            AeadError::AuthenticationFailed
        );
    }

    #[test]
    fn short_ciphertext_is_malformed_not_authentication() {
        let k = key(2);
        for len in 0..TAG_LEN {
            assert_eq!(
                open(&k, b"", &[0u8; NONCE_LEN], &vec![0u8; len])
                    .err()
                    .unwrap(),
                AeadError::Malformed
            );
        }
        // Exactly one tag of zeros is well-formed but unauthentic.
        assert_eq!(
            open(&k, b"", &[0u8; NONCE_LEN], &[0u8; TAG_LEN])
                .err()
                .unwrap(),
            AeadError::AuthenticationFailed
        );
    }

    #[test]
    fn truncating_or_extending_ciphertext_fails() {
        let k = key(2);
        let (nonce, ct) = seal(&k, b"a", b"CANARY-plaintext", &mut OsRng).unwrap();
        assert_eq!(
            open(&k, b"a", &nonce, &ct[..ct.len() - 1]).err().unwrap(),
            AeadError::AuthenticationFailed
        );
        let mut longer = ct.clone();
        longer.push(0);
        assert_eq!(
            open(&k, b"a", &nonce, &longer).err().unwrap(),
            AeadError::AuthenticationFailed
        );
    }

    // SEC-C04 smoke test: 1,000,000 nonces produced through the public `seal` path are
    // pairwise distinct. (Statistical sanity only; the guarantee comes from 192-bit random.)
    #[test]
    fn sec_c04_one_million_nonces_are_distinct() {
        let k = key(4);
        let mut seen: HashSet<Nonce> = HashSet::with_capacity(1_000_000);
        for _ in 0..1_000_000 {
            let (nonce, _) = seal(&k, b"", b"", &mut OsRng).unwrap();
            assert!(seen.insert(nonce), "duplicate nonce observed");
        }
    }

    #[test]
    fn same_inputs_give_different_nonce_and_ciphertext() {
        let k = key(5);
        let (n1, c1) = seal(&k, b"a", b"CANARY-same", &mut OsRng).unwrap();
        let (n2, c2) = seal(&k, b"a", b"CANARY-same", &mut OsRng).unwrap();
        assert_ne!(n1, n2);
        assert_ne!(c1, c2);
    }

    struct FailingRng;
    impl crate::rng::Rng for FailingRng {
        fn fill_bytes(&mut self, _: &mut [u8]) -> Result<(), RngError> {
            Err(RngError::Unavailable)
        }
    }

    #[test]
    fn rng_failure_is_a_typed_error() {
        assert_eq!(
            seal(&key(1), b"", b"x", &mut FailingRng).err().unwrap(),
            AeadError::Rng(RngError::Unavailable)
        );
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]
        #[test]
        fn prop_seal_open_round_trip(
            pt in proptest::collection::vec(any::<u8>(), 0..2048),
            aad in proptest::collection::vec(any::<u8>(), 0..256),
            kb in any::<u8>(),
        ) {
            let k = key(kb);
            let (nonce, ct) = seal(&k, &aad, &pt, &mut OsRng).unwrap();
            prop_assert_eq!(ct.len(), pt.len() + TAG_LEN);
            let out = open(&k, &aad, &nonce, &ct).unwrap();
            prop_assert_eq!(out.as_slice(), pt.as_slice());
        }

        #[test]
        fn prop_any_single_bit_flip_is_rejected(
            pt in proptest::collection::vec(any::<u8>(), 0..128),
            pos in any::<usize>(),
            bit in 0u8..8,
        ) {
            let k = key(9);
            let (nonce, mut ct) = seal(&k, b"x", &pt, &mut OsRng).unwrap();
            let i = pos % ct.len();
            ct[i] ^= 1 << bit;
            prop_assert_eq!(open(&k, b"x", &nonce, &ct).err().unwrap(), AeadError::AuthenticationFailed);
        }
    }
}
