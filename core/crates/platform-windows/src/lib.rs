//! Windows quick unlock (docs/04 section 8, docs/14 section 6).
//!
//! The vault key is sealed under a key that only Windows Hello can reproduce:
//!
//! 1. `seal`: draw `salt` (16 B) and `challenge` (32 B) from the OS CSPRNG, create the Hello
//!    credential, sign `challenge` **twice** (the two signatures must be identical: the
//!    construction needs a deterministic signature, and a future Windows with randomized padding
//!    must not silently lock the user out), derive `KEK_qu = HKDF(salt, signature)` with
//!    [`arya_vault_crypto::hkdf::kek_quick_unlock`] and seal the vault key with the crypto
//!    crate's AEAD. `blob = 0x01 || salt || challenge || nonce || ciphertext`.
//! 2. `unseal`: read `challenge` from the blob, ask Hello to sign it (the OS shows the prompt
//!    **every time**, verified by spike S4), re-derive `KEK_qu`, open the AEAD.
//!
//! Nothing here is a new primitive. The platform is reached through [`HelloPlatform`], so the
//! blob format, the key derivation and the error mapping are tested on every OS with a fake;
//! the real thing is `windows_hello::WindowsHello` (Windows only).
//!
//! The blob is local only (never synced). It carries its own version byte; this is not a vault
//! `format_version` change.

use arya_vault_crypto::aead::{self, NONCE_LEN};
use arya_vault_crypto::hkdf::{self, QUICK_UNLOCK_SALT_LEN};
use arya_vault_crypto::keys::VaultKey;
use arya_vault_crypto::rng::{OsRng, Rng};
use arya_vault_session::quick::{Blob, ProviderError, QuickUnlockKind, QuickUnlockProvider};
use zeroize::Zeroizing;

#[cfg(windows)]
pub mod dir_security;
#[cfg(windows)]
pub mod windows_hello;

/// AAD of the sealed vault key (docs/04 section 8).
pub const AAD: &[u8] = b"aryavault/quick-unlock/win/v1";
/// Blob format version.
pub const BLOB_VERSION: u8 = 0x01;
/// Length of the random challenge that Hello signs.
pub const CHALLENGE_LEN: usize = 32;
/// `version || salt || challenge || nonce || (32-byte key + 16-byte tag)`.
const BLOB_LEN: usize = 1 + QUICK_UNLOCK_SALT_LEN + CHALLENGE_LEN + NONCE_LEN + 32 + aead::TAG_LEN;

/// Why a Hello call failed. Carries no detail by design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelloError {
    /// No Hello, no TPM, or Hello is not set up.
    Unavailable,
    /// The user dismissed the prompt.
    Cancelled,
    /// The credential is gone (Hello was reset or the credential deleted).
    Invalidated,
    /// Anything else.
    Failed,
}

impl From<HelloError> for ProviderError {
    fn from(e: HelloError) -> Self {
        match e {
            HelloError::Unavailable => Self::Unavailable,
            HelloError::Cancelled => Self::UserCancelled,
            HelloError::Invalidated => Self::Invalidated,
            HelloError::Failed => Self::Failed,
        }
    }
}

/// The Windows Hello operations the provider needs.
pub trait HelloPlatform: Send {
    /// Whether Hello can be used on this device right now.
    fn supported(&self) -> bool;

    /// Creates the credential, replacing an existing one.
    ///
    /// # Errors
    /// [`HelloError`].
    fn create_credential(&self) -> Result<(), HelloError>;

    /// Signs `challenge` with the credential. Requires user presence on every call.
    ///
    /// # Errors
    /// [`HelloError`]; a missing credential is [`HelloError::Invalidated`].
    fn sign(&self, challenge: &[u8]) -> Result<Vec<u8>, HelloError>;

    /// Best-effort removal of the credential.
    fn delete_credential(&self);

    /// An identifier of the current boot, or `None`.
    fn boot_id(&self) -> Option<Vec<u8>>;
}

/// The quick-unlock provider over a [`HelloPlatform`].
pub struct HelloProvider<P: HelloPlatform> {
    platform: P,
}

impl<P: HelloPlatform> HelloProvider<P> {
    /// Wraps `platform`.
    #[must_use]
    pub fn new(platform: P) -> Self {
        Self { platform }
    }
}

fn random<const N: usize>() -> Result<[u8; N], ProviderError> {
    let mut out = [0u8; N];
    OsRng
        .fill_bytes(&mut out)
        .map_err(|_| ProviderError::Failed)?;
    Ok(out)
}

impl<P: HelloPlatform> QuickUnlockProvider for HelloProvider<P> {
    fn kind(&self) -> QuickUnlockKind {
        QuickUnlockKind::WindowsHello
    }

    fn available(&self) -> bool {
        self.platform.supported()
    }

    fn seal(&self, vk: &VaultKey) -> Result<Blob, ProviderError> {
        if !self.platform.supported() {
            return Err(ProviderError::Unavailable);
        }
        let salt: [u8; QUICK_UNLOCK_SALT_LEN] = random()?;
        let challenge: [u8; CHALLENGE_LEN] = random()?;

        self.platform.create_credential()?;
        let result = (|| {
            let first = self.platform.sign(&challenge)?;
            let second = self.platform.sign(&challenge)?;
            // The wrapping key is only reproducible if signing is deterministic.
            if first != second {
                return Err(HelloError::Failed);
            }
            let kek = hkdf::kek_quick_unlock(&first, &salt).map_err(|_| HelloError::Failed)?;
            let (nonce, ciphertext) = aead::seal(&kek, AAD, vk.expose_secret(), &mut OsRng)
                .map_err(|_| HelloError::Failed)?;
            let mut blob = Vec::with_capacity(BLOB_LEN);
            blob.push(BLOB_VERSION);
            blob.extend_from_slice(&salt);
            blob.extend_from_slice(&challenge);
            blob.extend_from_slice(&nonce);
            blob.extend_from_slice(&ciphertext);
            Ok(blob)
        })();
        match result {
            Ok(blob) => Blob::new(blob),
            Err(e) => {
                // Do not leave a credential behind for a quick unlock that was not enabled.
                self.platform.delete_credential();
                Err(e.into())
            }
        }
    }

    fn unseal(&self, blob: &Blob) -> Result<VaultKey, ProviderError> {
        let bytes = blob.as_bytes();
        if bytes.len() != BLOB_LEN || bytes[0] != BLOB_VERSION {
            return Err(ProviderError::Failed);
        }
        let salt_end = 1 + QUICK_UNLOCK_SALT_LEN;
        let challenge_end = salt_end + CHALLENGE_LEN;
        let nonce_end = challenge_end + NONCE_LEN;
        let mut salt = [0u8; QUICK_UNLOCK_SALT_LEN];
        salt.copy_from_slice(&bytes[1..salt_end]);
        let challenge = &bytes[salt_end..challenge_end];
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[challenge_end..nonce_end]);
        let ciphertext = &bytes[nonce_end..];

        let signature = self.platform.sign(challenge)?;
        let kek = hkdf::kek_quick_unlock(&signature, &salt).map_err(|_| ProviderError::Failed)?;
        let plain = aead::open(&kek, AAD, &nonce, ciphertext).map_err(|_| ProviderError::Failed)?;
        let mut key = Zeroizing::new([0u8; 32]);
        if plain.len() != key.len() {
            return Err(ProviderError::Failed);
        }
        key.copy_from_slice(&plain);
        Ok(VaultKey::from_bytes(*key))
    }

    fn boot_id(&self) -> Option<Vec<u8>> {
        self.platform.boot_id()
    }

    fn revoke(&self) {
        self.platform.delete_credential();
    }
}

#[cfg(test)]
mod tests;
