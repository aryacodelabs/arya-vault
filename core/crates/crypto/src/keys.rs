//! Key newtypes (docs/04 §2, §11).
//!
//! None of these types implement `Debug`, `Display`, `Clone` or `Serialize`, and all are
//! zeroized on drop (SEC-C06). Raw bytes are reachable only through the explicit
//! `expose_secret` accessors, which are `pub(crate)` unless a downstream crate genuinely
//! needs the bytes (VK for the OS keystore wrap, sub-keys for SQLCipher/AEAD,
//! recovery key for display).

use crate::secret::Secret;

/// Length of every symmetric key in bytes.
pub const KEY_LEN: usize = 32;
/// Length of the recovery key in bytes (160 bits, docs/04 §4).
pub const RECOVERY_KEY_LEN: usize = 20;

macro_rules! key_type {
    ($(#[$m:meta])* $name:ident, $n:expr) => {
        $(#[$m])*
        pub struct $name(Secret<$n>);
    };
}

key_type!(
    /// The random 256-bit vault key (VK). Never derived from the password.
    VaultKey,
    32
);
key_type!(
    /// The 256-bit Argon2id output derived from the normalized master password.
    MasterKey,
    32
);
key_type!(
    /// A key-encryption key (`KEK_pw` or `KEK_rk`) used only to wrap the vault key.
    Kek,
    32
);
key_type!(
    /// A purpose-specific subkey derived from the vault key (docs/04 §2).
    SubKey,
    32
);
key_type!(
    /// The 160-bit recovery key. Show once; never persisted.
    RecoveryKey,
    20
);

impl SubKey {
    pub(crate) fn from_secret(s: Secret<32>) -> Self {
        Self(s)
    }
}

impl MasterKey {
    pub(crate) fn from_secret(s: Secret<32>) -> Self {
        Self(s)
    }
}

impl Kek {
    pub(crate) fn from_secret(s: Secret<32>) -> Self {
        Self(s)
    }
}

impl VaultKey {
    /// Builds a vault key from raw bytes (e.g. after unwrapping from an OS keystore).
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Secret::new(bytes))
    }

    /// Exposes the raw key bytes. Callers must not log, copy into long-lived
    /// non-zeroizing storage, or serialize the result.
    pub fn expose_secret(&self) -> &[u8; KEY_LEN] {
        self.0.as_bytes()
    }
}

impl SubKey {
    /// Exposes the raw key bytes (e.g. as the SQLCipher raw key). Same handling rules as
    /// [`VaultKey::expose_secret`].
    pub fn expose_secret(&self) -> &[u8; KEY_LEN] {
        self.0.as_bytes()
    }
}

impl MasterKey {
    pub(crate) fn expose_secret(&self) -> &[u8; KEY_LEN] {
        self.0.as_bytes()
    }

    /// Test/golden-file constructor.
    #[cfg(test)]
    pub(crate) fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Secret::new(bytes))
    }
}

impl Kek {
    pub(crate) fn expose_secret(&self) -> &[u8; KEY_LEN] {
        self.0.as_bytes()
    }
}

impl RecoveryKey {
    /// Builds a recovery key from raw bytes.
    pub fn from_bytes(bytes: [u8; RECOVERY_KEY_LEN]) -> Self {
        Self(Secret::new(bytes))
    }

    /// Exposes the raw 20 bytes (needed to display/encode the key). Same handling rules
    /// as [`VaultKey::expose_secret`].
    pub fn expose_secret(&self) -> &[u8; RECOVERY_KEY_LEN] {
        self.0.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::test_hook;

    const CANARY: u8 = 0xA5;

    fn assert_wiped(expected_len: usize) {
        let drops = test_hook::take();
        assert_eq!(drops.len(), 1, "exactly one key buffer must have dropped");
        let (before, after) = &drops[0];
        assert_eq!(before.len(), expected_len);
        assert!(
            before.iter().all(|b| *b == CANARY),
            "canary must be present before drop"
        );
        assert!(
            after.iter().all(|b| *b == 0),
            "buffer must be zero after drop"
        );
    }

    // SEC-C06: every key type wipes its live buffer in Drop.
    #[test]
    fn sec_c06_vault_key_zeroized_on_drop() {
        test_hook::take();
        drop(VaultKey::from_bytes([CANARY; 32]));
        assert_wiped(32);
    }

    #[test]
    fn sec_c06_sub_key_zeroized_on_drop() {
        test_hook::take();
        drop(SubKey::from_secret(Secret::new([CANARY; 32])));
        assert_wiped(32);
    }

    #[test]
    fn sec_c06_master_key_zeroized_on_drop() {
        test_hook::take();
        drop(MasterKey::from_bytes([CANARY; 32]));
        assert_wiped(32);
    }

    #[test]
    fn sec_c06_kek_zeroized_on_drop() {
        test_hook::take();
        drop(Kek::from_secret(Secret::new([CANARY; 32])));
        assert_wiped(32);
    }

    #[test]
    fn sec_c06_recovery_key_zeroized_on_drop() {
        test_hook::take();
        drop(RecoveryKey::from_bytes([CANARY; 20]));
        assert_wiped(20);
    }

    #[test]
    fn expose_secret_returns_the_stored_bytes() {
        let vk = VaultKey::from_bytes([7u8; 32]);
        assert_eq!(vk.expose_secret(), &[7u8; 32]);
    }
}
