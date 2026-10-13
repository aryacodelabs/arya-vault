//! Tests of the sealing logic with a fake Hello (the same on every OS). The real Hello is
//! exercised by hand (docs/reviews/m2-windows-evidence.md).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use arya_vault_crypto::keys::VaultKey;
use arya_vault_session::quick::{Blob, ProviderError, QuickUnlockKind, QuickUnlockProvider};

use super::{BLOB_LEN, BLOB_VERSION, HelloError, HelloPlatform, HelloProvider};

const VK: [u8; 32] = [0xAB; 32];

#[derive(Default)]
struct State {
    supported: bool,
    /// The secret the "TPM key" signs with; `None` = no credential.
    credential: Option<u8>,
    next_secret: u8,
    /// One-shot errors for the next `sign` calls.
    sign_errors: Vec<HelloError>,
    create_error: Option<HelloError>,
    /// Makes every signature different (a randomized-padding Windows).
    random_signatures: bool,
    counter: u8,
    creates: usize,
    signs: usize,
    deletes: usize,
    seen_signatures: Vec<Vec<u8>>,
}

#[derive(Clone)]
struct Fake(Arc<Mutex<State>>);

impl Fake {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(State {
            supported: true,
            next_secret: 1,
            ..State::default()
        })))
    }
    fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.0.lock().unwrap())
    }
}

impl HelloPlatform for Fake {
    fn supported(&self) -> bool {
        self.with(|s| s.supported)
    }
    fn create_credential(&self) -> Result<(), HelloError> {
        self.with(|s| {
            s.creates += 1;
            if let Some(e) = s.create_error {
                return Err(e);
            }
            // A new credential is a new key: signatures change.
            s.credential = Some(s.next_secret);
            s.next_secret += 1;
            Ok(())
        })
    }
    fn sign(&self, challenge: &[u8]) -> Result<Vec<u8>, HelloError> {
        self.with(|s| {
            s.signs += 1;
            if let Some(e) = s.sign_errors.pop() {
                return Err(e);
            }
            let secret = s.credential.ok_or(HelloError::Invalidated)?;
            s.counter = s.counter.wrapping_add(1);
            let salt = if s.random_signatures { s.counter } else { 0 };
            let sig: Vec<u8> = (0..256usize)
                .map(|i| {
                    challenge[i % challenge.len()] ^ secret.wrapping_mul(31) ^ (i as u8) ^ salt
                })
                .collect();
            s.seen_signatures.push(sig.clone());
            Ok(sig)
        })
    }
    fn delete_credential(&self) {
        self.with(|s| {
            s.deletes += 1;
            s.credential = None;
        });
    }
    fn boot_id(&self) -> Option<Vec<u8>> {
        Some(b"boot-7".to_vec())
    }
}

fn provider() -> (HelloProvider<Fake>, Fake) {
    let fake = Fake::new();
    (HelloProvider::new(fake.clone()), fake)
}

fn vk() -> VaultKey {
    VaultKey::from_bytes(VK)
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn it_is_windows_hello() {
    let (p, _) = provider();
    assert_eq!(p.kind(), QuickUnlockKind::WindowsHello);
    assert!(p.available());
    assert_eq!(p.boot_id().as_deref(), Some(&b"boot-7"[..]));
}

#[test]
fn seal_then_unseal_returns_the_vault_key() {
    let (p, _) = provider();
    let blob = p.seal(&vk()).unwrap();
    assert_eq!(blob.as_bytes().len(), BLOB_LEN);
    assert_eq!(blob.as_bytes()[0], BLOB_VERSION);
    assert_eq!(p.unseal(&blob).unwrap().expose_secret(), &VK);
}

#[test]
fn the_blob_holds_neither_the_key_nor_a_signature() {
    let (p, fake) = provider();
    let blob = p.seal(&vk()).unwrap();
    assert!(!contains(blob.as_bytes(), &VK), "vault key in the clear");
    for sig in fake.with(|s| s.seen_signatures.clone()) {
        assert!(
            !contains(blob.as_bytes(), &sig[..32]),
            "signature in the blob"
        );
    }
}

#[test]
fn every_seal_is_different_and_each_unseals() {
    let (p, _) = provider();
    let a = p.seal(&vk()).unwrap();
    let b = p.seal(&vk()).unwrap();
    assert_ne!(
        a.as_bytes(),
        b.as_bytes(),
        "salt, challenge and nonce are random"
    );
    // The credential is replaced by the second seal, so only the newest blob opens.
    assert_eq!(p.unseal(&b).unwrap().expose_secret(), &VK);
    assert_eq!(p.unseal(&a).err(), Some(ProviderError::Failed));
}

#[test]
fn seal_signs_twice_and_unseal_signs_once() {
    // Each signature is a user-presence prompt (spike S4), so the count is part of the contract.
    let (p, fake) = provider();
    let blob = p.seal(&vk()).unwrap();
    assert_eq!(fake.with(|s| (s.creates, s.signs)), (1, 2));
    p.unseal(&blob).unwrap();
    assert_eq!(fake.with(|s| s.signs), 3);
}

#[test]
fn a_non_deterministic_signature_refuses_to_enable_and_cleans_up() {
    let (p, fake) = provider();
    fake.with(|s| s.random_signatures = true);
    assert_eq!(p.seal(&vk()).err(), Some(ProviderError::Failed));
    assert!(
        fake.with(|s| s.credential.is_none()),
        "credential left behind"
    );
    assert_eq!(fake.with(|s| s.deletes), 1);
}

#[test]
fn unavailable_hello_is_unavailable() {
    let (p, fake) = provider();
    fake.with(|s| s.supported = false);
    assert!(!p.available());
    assert_eq!(p.seal(&vk()).err(), Some(ProviderError::Unavailable));
    assert_eq!(
        fake.with(|s| s.creates),
        0,
        "no credential is created without Hello"
    );
}

#[test]
fn errors_map_to_the_provider_errors() {
    for (hello, expected) in [
        (HelloError::Unavailable, ProviderError::Unavailable),
        (HelloError::Cancelled, ProviderError::UserCancelled),
        (HelloError::Invalidated, ProviderError::Invalidated),
        (HelloError::Failed, ProviderError::Failed),
    ] {
        // While enabling: the credential could not be created.
        let (p, fake) = provider();
        fake.with(|s| s.create_error = Some(hello));
        assert_eq!(p.seal(&vk()).err(), Some(expected), "create: {hello:?}");

        // While enabling: the prompt fails on the first signature.
        let (p, fake) = provider();
        fake.with(|s| s.sign_errors = vec![hello]);
        assert_eq!(p.seal(&vk()).err(), Some(expected), "seal sign: {hello:?}");

        // While unlocking.
        let (p, fake) = provider();
        let blob = p.seal(&vk()).unwrap();
        fake.with(|s| s.sign_errors = vec![hello]);
        assert_eq!(p.unseal(&blob).err(), Some(expected), "unseal: {hello:?}");
    }
}

#[test]
fn a_deleted_credential_is_invalidated() {
    let (p, fake) = provider();
    let blob = p.seal(&vk()).unwrap();
    fake.with(|s| s.credential = None);
    assert_eq!(p.unseal(&blob).err(), Some(ProviderError::Invalidated));
}

#[test]
fn a_different_credential_cannot_open_the_blob() {
    // Hello was reset and set up again: same name, new key, so the signature differs.
    let (p, fake) = provider();
    let blob = p.seal(&vk()).unwrap();
    fake.with(|s| s.credential = Some(200));
    assert_eq!(p.unseal(&blob).err(), Some(ProviderError::Failed));
}

#[test]
fn a_damaged_blob_is_failed_never_a_wrong_key() {
    let (p, _) = provider();
    let blob = p.seal(&vk()).unwrap();
    // Every single byte flipped (version, salt, challenge, nonce, ciphertext, tag).
    for i in 0..blob.as_bytes().len() {
        let mut bad = blob.as_bytes().to_vec();
        bad[i] ^= 0x01;
        let bad = Blob::new(bad).unwrap();
        assert_eq!(
            p.unseal(&bad).err(),
            Some(ProviderError::Failed),
            "byte {i}"
        );
    }
}

#[test]
fn a_wrong_size_blob_is_failed_without_prompting() {
    let (p, fake) = provider();
    let blob = p.seal(&vk()).unwrap();
    let signs = fake.with(|s| s.signs);
    for len in [1usize, BLOB_LEN - 1] {
        let short = Blob::new(blob.as_bytes()[..len].to_vec()).unwrap();
        assert_eq!(p.unseal(&short).err(), Some(ProviderError::Failed));
    }
    let mut long = blob.as_bytes().to_vec();
    long.push(0);
    assert_eq!(
        p.unseal(&Blob::new(long).unwrap()).err(),
        Some(ProviderError::Failed)
    );
    assert_eq!(
        fake.with(|s| s.signs),
        signs,
        "a malformed blob must not trigger a prompt"
    );
}

#[test]
fn revoke_deletes_the_credential() {
    let (p, fake) = provider();
    p.seal(&vk()).unwrap();
    p.revoke();
    assert!(fake.with(|s| s.credential.is_none()));
}
