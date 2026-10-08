//! Randomness source abstraction.

#[cfg(all(feature = "deterministic-rng", not(debug_assertions)))]
compile_error!("feature `deterministic-rng` must never be enabled in release builds");

use crate::error::GeneratorError;

mod sealed {
    pub trait Sealed {}
}

/// A source of random bytes.
///
/// Sealed: outside this crate the only implementation is [`OsRandom`]. A
/// deterministic [`SeededRandom`] exists only in tests or under the
/// non-default `deterministic-rng` feature (debug builds only).
pub trait RandomSource: sealed::Sealed {
    /// Fill `dest` with random bytes.
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), GeneratorError>;

    /// Draw a random `u32`.
    fn next_u32(&mut self) -> Result<u32, GeneratorError> {
        let mut b = [0u8; 4];
        self.fill_bytes(&mut b)?;
        Ok(u32::from_le_bytes(b))
    }
}

/// The operating-system CSPRNG (via `getrandom`). Default and only production source.
#[derive(Debug, Default, Clone, Copy)]
pub struct OsRandom;

impl sealed::Sealed for OsRandom {}

impl RandomSource for OsRandom {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), GeneratorError> {
        getrandom::fill(dest).map_err(|_| GeneratorError::Rng)
    }
}

/// Deterministic SplitMix64 source for tests. NOT cryptographically secure.
#[cfg(any(test, feature = "deterministic-rng"))]
#[derive(Debug, Clone)]
pub struct SeededRandom {
    state: u64,
}

#[cfg(any(test, feature = "deterministic-rng"))]
impl SeededRandom {
    /// Create a source from a fixed seed.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(any(test, feature = "deterministic-rng"))]
impl sealed::Sealed for SeededRandom {}

#[cfg(any(test, feature = "deterministic-rng"))]
impl RandomSource for SeededRandom {
    fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), GeneratorError> {
        for chunk in dest.chunks_mut(8) {
            let bytes = self.next_u64().to_le_bytes();
            chunk.copy_from_slice(&bytes[..chunk.len()]);
        }
        Ok(())
    }
}

/// Maximum rejection-sampling attempts before reporting an RNG failure.
/// With a healthy source the rejection probability per draw is < 2^-19 for
/// the bounds used here, so 64 consecutive rejections cannot happen.
const MAX_ATTEMPTS: usize = 64;

/// Uniform integer in `0..n` by rejection sampling (no modulo bias). `n` must be >= 1.
pub(crate) fn uniform_below<R: RandomSource + ?Sized>(
    rng: &mut R,
    n: u32,
) -> Result<u32, GeneratorError> {
    if n <= 1 {
        return Ok(0);
    }
    let n64 = u64::from(n);
    // Largest multiple of n that fits in 2^32; values >= zone are rejected.
    let zone = (1u64 << 32) / n64 * n64;
    for _ in 0..MAX_ATTEMPTS {
        let x = u64::from(rng.next_u32()?);
        if x < zone {
            // x % n < n <= u32::MAX, so the conversion cannot fail.
            return u32::try_from(x % n64).map_err(|_| GeneratorError::Rng);
        }
    }
    Err(GeneratorError::Rng)
}

/// Index in `0..len` (uniform, unbiased).
pub(crate) fn uniform_index<R: RandomSource + ?Sized>(
    rng: &mut R,
    len: usize,
) -> Result<usize, GeneratorError> {
    let n = u32::try_from(len).map_err(|_| GeneratorError::Rng)?;
    Ok(uniform_below(rng, n)? as usize)
}

/// In-place Fisher-Yates shuffle using the unbiased sampler.
pub(crate) fn shuffle<T, R: RandomSource + ?Sized>(
    rng: &mut R,
    items: &mut [T],
) -> Result<(), GeneratorError> {
    for i in (1..items.len()).rev() {
        let j = uniform_index(rng, i + 1)?;
        items.swap(i, j);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Replays fixed `u32` draws (little-endian bytes) to test rejection exactly.
    struct Scripted(std::vec::IntoIter<u32>);

    impl sealed::Sealed for Scripted {}

    impl RandomSource for Scripted {
        fn fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), GeneratorError> {
            let v = self.0.next().ok_or(GeneratorError::Rng)?;
            dest.copy_from_slice(&v.to_le_bytes());
            Ok(())
        }
    }

    #[test]
    fn values_in_the_biased_tail_are_rejected() {
        // 2^32 = 69_273_666 * 62 + 4, so draws >= 4_294_967_292 are rejected.
        let mut rng = Scripted(vec![u32::MAX, 4_294_967_292, 4_294_967_291].into_iter());
        assert_eq!(uniform_below(&mut rng, 62).unwrap(), 4_294_967_291 % 62);
        assert!(rng.0.next().is_none(), "both tail values must be consumed");
    }

    #[test]
    fn exhausted_rejections_report_rng_error() {
        let mut rng = Scripted(vec![u32::MAX; MAX_ATTEMPTS].into_iter());
        assert_eq!(uniform_below(&mut rng, 62), Err(GeneratorError::Rng));
    }

    #[test]
    fn power_of_two_range_never_rejects() {
        let mut rng = Scripted(vec![u32::MAX].into_iter());
        assert_eq!(uniform_below(&mut rng, 64).unwrap(), 63);
    }
}
