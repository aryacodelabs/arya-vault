//! Randomness source (docs/04 §1, SEC-C03).
//!
//! All keys, nonces and salts are drawn through the [`Rng`] trait. Production code uses
//! [`OsRng`], which reads the operating-system CSPRNG via `getrandom`. A deterministic
//! generator exists only under the non-default `deterministic-rng` feature, for producing
//! reproducible golden files; release builds with that feature fail to compile.

use thiserror::Error;

/// Error from a random number source.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RngError {
    /// The operating-system random number generator could not be read.
    #[error("operating-system random number generator is unavailable")]
    Unavailable,
}

/// A source of random bytes.
///
/// Implementations used in shipped builds MUST be cryptographically secure. The only
/// shipped implementation is [`OsRng`].
pub trait Rng {
    /// Fills `dest` entirely with random bytes, or returns an error without any
    /// guarantee about the contents of `dest` (callers must not use it on error).
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError>;
}

/// The operating-system CSPRNG (`getrandom`).
#[derive(Debug, Default, Clone, Copy)]
pub struct OsRng;

impl Rng for OsRng {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
        getrandom::fill(dest).map_err(|_| RngError::Unavailable)
    }
}

/// Deterministic **non-secure** generator for golden-file generation only.
///
/// Output is `SHA-256(seed ‖ counter_be64)` blocks. It exists so that test data
/// can be reproduced bit-for-bit; it must never protect real data.
#[cfg(feature = "deterministic-rng")]
#[derive(Debug, Clone)]
pub struct DeterministicRng {
    seed: [u8; 32],
    counter: u64,
}

#[cfg(feature = "deterministic-rng")]
impl DeterministicRng {
    /// Creates a generator from a fixed seed.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self { seed, counter: 0 }
    }
}

#[cfg(feature = "deterministic-rng")]
impl Rng for DeterministicRng {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), RngError> {
        use sha2::{Digest, Sha256};
        for chunk in dest.chunks_mut(32) {
            let mut h = Sha256::new();
            h.update(self.seed);
            h.update(self.counter.to_be_bytes());
            self.counter = self.counter.wrapping_add(1);
            let block = h.finalize();
            chunk.copy_from_slice(&block[..chunk.len()]);
        }
        Ok(())
    }
}

/// Draws `N` random bytes into a fresh array.
pub(crate) fn random_array<const N: usize>(rng: &mut dyn Rng) -> Result<[u8; N], RngError> {
    let mut out = [0u8; N];
    rng.fill_bytes(&mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_rng_fills_and_varies() {
        let mut rng = OsRng;
        let a: [u8; 32] = random_array(&mut rng).unwrap();
        let b: [u8; 32] = random_array(&mut rng).unwrap();
        assert_ne!(a, b);
        assert_ne!(a, [0u8; 32]);
    }

    #[cfg(feature = "deterministic-rng")]
    #[test]
    fn deterministic_rng_is_reproducible() {
        let mut a = DeterministicRng::from_seed([1; 32]);
        let mut b = DeterministicRng::from_seed([1; 32]);
        let x: [u8; 70] = random_array(&mut a).unwrap();
        let y: [u8; 70] = random_array(&mut b).unwrap();
        assert_eq!(x, y);
        let mut c = DeterministicRng::from_seed([2; 32]);
        let z: [u8; 70] = random_array(&mut c).unwrap();
        assert_ne!(x, z);
    }
}
