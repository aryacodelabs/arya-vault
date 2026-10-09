//! A configurable fake provider for tests (feature `test-support`; compile-guarded out of
//! release builds, see the parent module).
//!
//! `seal` encrypts the vault key under a random in-memory key with `aead::seal` from the crypto
//! crate (no new cryptography). Tests keep a [`FakeHandle`] to steer the provider after it has
//! been moved into the session, and to observe what it saw (including the vault key bytes, so a
//! disk scan can look for them).

// Test-only code: a failing HKDF on fixed-size input is a bug worth a loud panic.
#![allow(clippy::expect_used)]

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arya_vault_crypto::aead::{self, NONCE_LEN};
use arya_vault_crypto::hkdf;
use arya_vault_crypto::keys::{Kek, RecoveryKey, VaultKey};
use arya_vault_crypto::rng::{OsRng, Rng};
use zeroize::Zeroizing;

use super::{Blob, ProviderError, QuickUnlockKind, QuickUnlockProvider};

const AAD: &[u8] = b"arya-vault/fake-quick-unlock/v1";

/// What the next `unseal` does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    /// Release the key.
    Succeed,
    /// The user dismisses the prompt.
    Cancel,
    /// The platform key was invalidated (biometric enrolment changed).
    Invalidate,
    /// No hardware right now.
    Unavailable,
    /// A failed biometric / failed authentication.
    Fail,
}

struct State {
    key: Kek,
    kind: QuickUnlockKind,
    available: bool,
    boot_id: Option<Vec<u8>>,
    /// One-shot behaviours, consumed first.
    script: VecDeque<Behavior>,
    /// What `unseal` does when the script is empty.
    default: Behavior,
    fail_seal: Option<ProviderError>,
    seal_calls: usize,
    unseal_calls: usize,
    revoke_calls: usize,
    last_vk: Option<Zeroizing<[u8; 32]>>,
    last_blob: Option<Vec<u8>>,
    on_unseal: Option<Box<dyn FnMut() + Send>>,
}

fn lock(s: &Mutex<State>) -> MutexGuard<'_, State> {
    s.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The provider handed to `Session::with_provider`.
pub struct FakeProvider {
    state: Arc<Mutex<State>>,
}

/// The test's remote control for a [`FakeProvider`].
#[derive(Clone)]
pub struct FakeHandle {
    state: Arc<Mutex<State>>,
}

fn fresh_key() -> Kek {
    // A random key; `kek_rk` is only used as an HKDF-based constructor for a `Kek`, the one
    // key type `aead::seal` accepts besides sub-keys.
    let mut ikm = [0u8; 20];
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut ikm).expect("OS RNG");
    OsRng.fill_bytes(&mut salt).expect("OS RNG");
    hkdf::kek_rk(&RecoveryKey::from_bytes(ikm), &salt).expect("HKDF on fixed-size input")
}

impl FakeProvider {
    /// A provider of kind `WindowsHello` that is available, has boot id `boot-1` and succeeds.
    #[must_use]
    pub fn new() -> (Self, FakeHandle) {
        let state = Arc::new(Mutex::new(State {
            key: fresh_key(),
            kind: QuickUnlockKind::WindowsHello,
            available: true,
            boot_id: Some(b"boot-1".to_vec()),
            script: VecDeque::new(),
            default: Behavior::Succeed,
            fail_seal: None,
            seal_calls: 0,
            unseal_calls: 0,
            revoke_calls: 0,
            last_vk: None,
            last_blob: None,
            on_unseal: None,
        }));
        (
            Self {
                state: Arc::clone(&state),
            },
            FakeHandle { state },
        )
    }
}

impl FakeHandle {
    /// Changes what `unseal` does by default.
    pub fn set_default(&self, b: Behavior) {
        lock(&self.state).default = b;
    }
    /// Queues one-shot behaviours for the next `unseal` calls.
    pub fn script(&self, items: &[Behavior]) {
        lock(&self.state).script.extend(items.iter().copied());
    }
    /// Sets the provider kind.
    pub fn set_kind(&self, k: QuickUnlockKind) {
        lock(&self.state).kind = k;
    }
    /// Sets availability.
    pub fn set_available(&self, a: bool) {
        lock(&self.state).available = a;
    }
    /// Sets the boot id (a reboot is a different id).
    pub fn set_boot_id(&self, id: Option<&[u8]>) {
        lock(&self.state).boot_id = id.map(<[u8]>::to_vec);
    }
    /// Makes `seal` fail with `e`.
    pub fn fail_seal(&self, e: Option<ProviderError>) {
        lock(&self.state).fail_seal = e;
    }
    /// Runs `f` inside every `unseal`, before it returns (a "prompt is up" hook).
    pub fn on_unseal(&self, f: impl FnMut() + Send + 'static) {
        lock(&self.state).on_unseal = Some(Box::new(f));
    }
    /// Number of `seal` calls so far.
    #[must_use]
    pub fn seal_calls(&self) -> usize {
        lock(&self.state).seal_calls
    }
    /// Number of `unseal` calls so far.
    #[must_use]
    pub fn unseal_calls(&self) -> usize {
        lock(&self.state).unseal_calls
    }
    /// Number of `revoke` calls so far.
    #[must_use]
    pub fn revoke_calls(&self) -> usize {
        lock(&self.state).revoke_calls
    }
    /// The vault key bytes of the last `seal` (for disk scans).
    #[must_use]
    pub fn last_sealed_vk(&self) -> Option<[u8; 32]> {
        lock(&self.state).last_vk.as_ref().map(|k| **k)
    }
    /// The blob returned by the last `seal`.
    #[must_use]
    pub fn last_blob(&self) -> Option<Vec<u8>> {
        lock(&self.state).last_blob.clone()
    }
    /// Seals arbitrary 32 bytes the way the provider would, e.g. to forge a blob of a
    /// *different* vault key.
    #[must_use]
    pub fn forge_blob(&self, vk_bytes: [u8; 32]) -> Vec<u8> {
        seal_bytes(&lock(&self.state).key, &vk_bytes)
    }
}

fn seal_bytes(key: &Kek, vk: &[u8; 32]) -> Vec<u8> {
    match aead::seal(key, AAD, vk, &mut OsRng) {
        Ok((nonce, ct)) => {
            let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
            out.extend_from_slice(&nonce);
            out.extend_from_slice(&ct);
            out
        }
        Err(_) => Vec::new(),
    }
}

impl QuickUnlockProvider for FakeProvider {
    fn kind(&self) -> QuickUnlockKind {
        lock(&self.state).kind
    }

    fn available(&self) -> bool {
        lock(&self.state).available
    }

    fn seal(&self, vk: &VaultKey) -> Result<Blob, ProviderError> {
        let mut st = lock(&self.state);
        st.seal_calls += 1;
        if let Some(e) = st.fail_seal {
            return Err(e);
        }
        let bytes = seal_bytes(&st.key, vk.expose_secret());
        st.last_vk = Some(Zeroizing::new(*vk.expose_secret()));
        st.last_blob = Some(bytes.clone());
        Blob::new(bytes)
    }

    fn unseal(&self, blob: &Blob) -> Result<VaultKey, ProviderError> {
        // Take the hook out so it can call back into the session's handles without deadlocking.
        let (hook, behavior) = {
            let mut st = lock(&self.state);
            st.unseal_calls += 1;
            let b = st.script.pop_front().unwrap_or(st.default);
            (st.on_unseal.take(), b)
        };
        if let Some(mut f) = hook {
            f();
            lock(&self.state).on_unseal = Some(f);
        }
        match behavior {
            Behavior::Cancel => return Err(ProviderError::UserCancelled),
            Behavior::Invalidate => return Err(ProviderError::Invalidated),
            Behavior::Unavailable => return Err(ProviderError::Unavailable),
            Behavior::Fail => return Err(ProviderError::Failed),
            Behavior::Succeed => {}
        }
        let bytes = blob.as_bytes();
        if bytes.len() < NONCE_LEN {
            return Err(ProviderError::Failed);
        }
        let mut nonce = [0u8; NONCE_LEN];
        nonce.copy_from_slice(&bytes[..NONCE_LEN]);
        let pt = aead::open(&lock(&self.state).key, AAD, &nonce, &bytes[NONCE_LEN..])
            .map_err(|_| ProviderError::Failed)?;
        let arr: [u8; 32] = pt
            .as_slice()
            .try_into()
            .map_err(|_| ProviderError::Failed)?;
        Ok(VaultKey::from_bytes(arr))
    }

    fn boot_id(&self) -> Option<Vec<u8>> {
        lock(&self.state).boot_id.clone()
    }

    fn revoke(&self) {
        lock(&self.state).revoke_calls += 1;
    }
}
