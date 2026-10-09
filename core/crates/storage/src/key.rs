use zeroize::Zeroizing;

use crate::error::StorageError;

/// 256-bit SQLCipher raw key (`K_db`, docs/04 section 2).
///
/// Zeroized on drop. Intentionally not `Clone`/`Copy`/`PartialEq`; `Debug`
/// is redacted. Use [`DbKey::duplicate`] for an explicit copy.
pub struct DbKey(Zeroizing<[u8; 32]>);

impl DbKey {
    /// Wrap 32 key bytes. The caller's array is `Copy`, so zeroize it afterwards.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Build from a slice; must be exactly 32 bytes.
    ///
    /// # Errors
    /// [`StorageError::InvalidKeyLength`] if the slice is not 32 bytes.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, StorageError> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| StorageError::InvalidKeyLength)?;
        Ok(Self::from_bytes(arr))
    }

    /// Explicit copy (also zeroized on drop).
    #[must_use]
    pub fn duplicate(&self) -> Self {
        Self::from_bytes(*self.0)
    }

    /// `x'<64 hex>'` raw-key literal as a zeroizing string.
    pub(crate) fn raw_literal(&self) -> Zeroizing<String> {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut s = Zeroizing::new(String::with_capacity(67));
        s.push_str("x'");
        for b in self.0.iter() {
            s.push(char::from(HEX[usize::from(b >> 4)]));
            s.push(char::from(HEX[usize::from(b & 0x0f)]));
        }
        s.push('\'');
        s
    }

    /// Constant-time equality (no early exit on the first differing byte).
    pub(crate) fn ct_eq(&self, other: &Self) -> bool {
        let mut acc = 0u8;
        for (a, b) in self.0.iter().zip(other.0.iter()) {
            acc |= a ^ b;
        }
        acc == 0
    }
}

impl core::fmt::Debug for DbKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DbKey(<redacted>)")
    }
}
