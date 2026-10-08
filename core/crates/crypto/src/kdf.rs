//! Argon2id master-key derivation (docs/04 §3, RFC 9106).
//!
//! # Security contracts
//! * [`KdfParams::validate`] enforces the **floors and ceilings** of doc 04 §3 *before*
//!   any hashing or allocation (SEC-C02, SEC-C11). Header parameters are unauthenticated
//!   until the KDF has run, so a hostile provider must not be able to request unbounded
//!   memory or time. [`derive_master_key`] always validates first.
//! * Argon2id, version 0x13, 32-byte output, no secret key and no associated data.
//! * The password is NFKD-normalized (no trimming) and the normalized copy is wiped.
//! * The ~`m` MiB of Argon2 working memory is allocated by this crate (fallibly; an
//!   allocation failure is an error, never a silent cost reduction) and **wiped after
//!   use**, because the `argon2` crate does not wipe memory it allocates itself.

use std::time::Instant;

use argon2::{Algorithm, Argon2, Block, Params, Version};
use thiserror::Error;
use zeroize::Zeroize;

use crate::cbor;
use crate::keys::MasterKey;
use crate::normalize::normalize_password;
use crate::rng::{Rng, RngError, random_array};
use crate::secret::Secret;

/// Minimum memory cost: 64 MiB (doc 04 §3).
pub const M_KIB_MIN: u32 = 64 * 1024;
/// Maximum memory cost: 1024 MiB (doc 04 §3).
pub const M_KIB_MAX: u32 = 1024 * 1024;
/// Minimum number of passes.
pub const T_MIN: u32 = 3;
/// Maximum number of passes.
pub const T_MAX: u32 = 10;
/// Minimum parallelism.
pub const P_MIN: u32 = 1;
/// Maximum parallelism.
pub const P_MAX: u32 = 8;
/// Salt length in bytes (exactly).
pub const SALT_LEN: usize = 16;
/// Memory-cost cap used by calibration for low-RAM devices (doc 04 §3 "Calibrated").
pub const M_KIB_CALIBRATION_CAP: u32 = 256 * 1024;

/// The `alg` identifier stored in the header (docs/04 §5).
pub const KDF_ALG: &str = "argon2id";
/// Argon2 version 0x13 (decimal 19), stored as `version` in the header.
pub const KDF_VERSION: u32 = 0x13;

/// Which parameter was out of range. Contains no secret data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdfParam {
    /// Memory cost `m_kib`.
    Memory,
    /// Iterations `t`.
    Iterations,
    /// Parallelism `p`.
    Parallelism,
}

/// Errors from key derivation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum KdfError {
    /// A parameter violates the doc 04 §3 bounds. Surface to users as "vault parameters
    /// are out of range or corrupted"; never retry with lowered cost.
    #[error("vault parameters are out of range or corrupted")]
    OutOfRange(KdfParam),
    /// The device could not allocate the required memory. The user should unlock on a
    /// more capable device; the cost is never silently reduced.
    #[error("not enough memory to derive the key with the vault's parameters")]
    OutOfMemory,
    /// The Argon2 implementation rejected the input (not expected after validation).
    #[error("key derivation failed")]
    Internal,
    /// The random number generator failed.
    #[error("random number generator failure")]
    Rng(#[from] RngError),
}

/// Argon2id parameters as stored in the vault header (docs/04 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory cost in KiB (64 MiB..=1024 MiB).
    pub m_kib: u32,
    /// Number of passes (3..=10).
    pub t: u32,
    /// Degree of parallelism (1..=8).
    pub p: u32,
    /// Random salt, exactly 16 bytes.
    pub salt: [u8; SALT_LEN],
}

impl KdfParams {
    /// The default floor profile (64 MiB, t = 3, p = 1) with the given salt.
    pub fn floor(salt: [u8; SALT_LEN]) -> Self {
        Self {
            m_kib: M_KIB_MIN,
            t: T_MIN,
            p: P_MIN,
            salt,
        }
    }

    /// Checks floors and ceilings. Runs in constant time and allocates nothing; it is the
    /// gate in front of every hash computation (SEC-C02, SEC-C11).
    pub fn validate(&self) -> Result<(), KdfError> {
        if !(M_KIB_MIN..=M_KIB_MAX).contains(&self.m_kib) {
            return Err(KdfError::OutOfRange(KdfParam::Memory));
        }
        if !(T_MIN..=T_MAX).contains(&self.t) {
            return Err(KdfError::OutOfRange(KdfParam::Iterations));
        }
        if !(P_MIN..=P_MAX).contains(&self.p) {
            return Err(KdfError::OutOfRange(KdfParam::Parallelism));
        }
        // The salt length is enforced by the `[u8; 16]` type.
        Ok(())
    }

    /// Returns a copy with a fresh random salt (used when changing the password).
    pub fn with_fresh_salt(&self, rng: &mut dyn Rng) -> Result<Self, KdfError> {
        Ok(Self {
            salt: random_array(rng)?,
            ..self.clone()
        })
    }

    /// Canonical (deterministic) CBOR of the `kdf` struct, used inside the `wrap_pw` AAD
    /// (docs/04 §5): a map with the keys `alg`, `version`, `m_kib`, `t`, `p`, `salt`,
    /// encoded per RFC 8949 §4.2.1 (shortest integers, entries sorted by encoded key).
    pub fn canonical_cbor(&self) -> Vec<u8> {
        cbor::map(vec![
            (cbor::text("alg"), cbor::text(KDF_ALG)),
            (cbor::text("version"), cbor::uint(u64::from(KDF_VERSION))),
            (cbor::text("m_kib"), cbor::uint(u64::from(self.m_kib))),
            (cbor::text("t"), cbor::uint(u64::from(self.t))),
            (cbor::text("p"), cbor::uint(u64::from(self.p))),
            (cbor::text("salt"), cbor::bytes(&self.salt)),
        ])
    }
}

/// Argon2 working memory: zero-initialised, allocated fallibly, wiped on drop (the
/// `argon2` crate does not wipe memory it allocates itself).
struct Scratch(Vec<Block>);

impl Scratch {
    fn new(count: usize) -> Result<Self, KdfError> {
        let mut v: Vec<Block> = Vec::new();
        v.try_reserve_exact(count)
            .map_err(|_| KdfError::OutOfMemory)?;
        v.resize(count, Block::default());
        Ok(Self(v))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        #[cfg(test)]
        let was_used = self.0.iter().any(|b| b.as_ref().iter().any(|w| *w != 0));
        for block in &mut self.0 {
            block.zeroize();
        }
        #[cfg(test)]
        wipe_hook::record(
            was_used,
            self.0.iter().all(|b| b.as_ref().iter().all(|w| *w == 0)),
        );
    }
}

#[cfg(test)]
mod wipe_hook {
    //! Test-only: records, per dropped Argon2 scratch buffer, whether it held data before
    //! the wipe and whether every word is zero after it.
    use std::cell::RefCell;

    thread_local! {
        static WIPES: RefCell<Vec<(bool, bool)>> = const { RefCell::new(Vec::new()) };
    }

    pub(super) fn record(was_used: bool, all_zero_after: bool) {
        WIPES.with(|w| w.borrow_mut().push((was_used, all_zero_after)));
    }

    pub(super) fn take() -> Vec<(bool, bool)> {
        WIPES.with(|w| std::mem::take(&mut *w.borrow_mut()))
    }
}

/// Runs Argon2id with already-validated, structurally correct parameters.
fn argon2id_into(
    password: &[u8],
    salt: &[u8],
    m_kib: u32,
    t: u32,
    p: u32,
    out: &mut [u8],
) -> Result<(), KdfError> {
    let params = Params::new(m_kib, t, p, Some(out.len())).map_err(|_| KdfError::Internal)?;
    let mut scratch = Scratch::new(params.block_count())?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(password, salt, out, scratch.0.as_mut_slice())
        .map_err(|_| KdfError::Internal)
}

/// Derives the 32-byte master key from `password` (NFKD-normalized, not trimmed).
///
/// Parameters are validated first; on failure no hashing or large allocation occurs.
pub fn derive_master_key(password: &str, params: &KdfParams) -> Result<MasterKey, KdfError> {
    params.validate()?;
    let normalized = normalize_password(password);
    let mut out = Secret::<32>::zeroed();
    argon2id_into(
        normalized.as_bytes(),
        &params.salt,
        params.m_kib,
        params.t,
        params.p,
        out.as_mut_bytes(),
    )?;
    Ok(MasterKey::from_secret(out))
}

/// Picks parameters for roughly `target_ms` milliseconds per unlock **on this device**.
///
/// Doc 04 §3 targets 0.5-1.0 s; pass a `target_ms` in that range. The result never goes
/// below the floors and never exceeds `max_m_kib` (itself clamped to
/// `M_KIB_MIN..=M_KIB_MAX`; use [`M_KIB_CALIBRATION_CAP`] for low-RAM devices) or the
/// ceilings. Strategy: measure the floor profile, then scale memory first (doubling, up to
/// the cap), then passes (up to [`T_MAX`]), re-measuring after each step; parallelism is
/// kept at 1 (the doc 04 §3 default) so the cost is the same on every device.
///
/// Calibration is timing-based and therefore not reproducible; the chosen parameters are
/// stored in the header and are what other devices use. A fresh random salt is drawn from
/// `rng`.
pub fn calibrate(target_ms: u64, max_m_kib: u32, rng: &mut dyn Rng) -> Result<KdfParams, KdfError> {
    let max_m = max_m_kib.clamp(M_KIB_MIN, M_KIB_MAX);
    let mut params = KdfParams::floor(random_array(rng)?);
    let target = std::time::Duration::from_millis(target_ms);
    let measure = |p: &KdfParams| -> Result<std::time::Duration, KdfError> {
        let start = Instant::now();
        let mut sink = Secret::<32>::zeroed();
        // A fixed dummy password; the output is discarded and wiped.
        argon2id_into(
            b"calibration",
            &p.salt,
            p.m_kib,
            p.t,
            p.p,
            sink.as_mut_bytes(),
        )?;
        Ok(start.elapsed())
    };

    let mut elapsed = measure(&params)?;
    while elapsed < target && params.m_kib < max_m {
        params.m_kib = params.m_kib.saturating_mul(2).min(max_m);
        elapsed = measure(&params)?;
    }
    while elapsed < target && params.t < T_MAX {
        params.t += 1;
        elapsed = measure(&params)?;
    }
    params.validate()?;
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::OsRng;
    use argon2::{AssociatedData, ParamsBuilder};

    // SEC-C07: RFC 9106 §5.3 Argon2id test vector (v=0x13, m=32 KiB, t=3, p=4, 32-byte
    // tag, with the RFC's secret and associated data). Source: RFC 9106 §5.3, and
    // reproduced in the `argon2` crate's tests/kat.rs (`argon2id_v0x13`), from which the
    // tag bytes were checked. m=32 KiB is below our floor, so this exercises the primitive
    // directly through the same crate (not through `derive_master_key`).
    #[test]
    fn sec_c07_rfc9106_argon2id_known_answer() {
        let params = ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .data(AssociatedData::new(&[0x04; 12]).unwrap())
            .build()
            .unwrap();
        let ctx = Argon2::new_with_secret(&[0x03; 8], Algorithm::Argon2id, Version::V0x13, params)
            .unwrap();
        let mut out = [0u8; 32];
        ctx.hash_password_into(&[0x01; 32], &[0x02; 16], &mut out)
            .unwrap();
        assert_eq!(
            hex::encode(out),
            "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
        );
    }

    // Our own entry point with params at the floor: must equal a direct Argon2id v1.3
    // computation (guards against wiring mistakes: version, output length, normalization).
    #[test]
    fn derive_matches_direct_argon2id_at_floor() {
        let params = KdfParams::floor([0x02; 16]);
        let mk = derive_master_key("CANARY-password", &params).unwrap();
        let direct = Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(M_KIB_MIN, T_MIN, P_MIN, Some(32)).unwrap(),
        );
        let mut want = [0u8; 32];
        direct
            .hash_password_into(b"CANARY-password", &[0x02; 16], &mut want)
            .unwrap();
        assert_eq!(mk.expose_secret(), &want);
    }

    // SEC-C06 / review open question 3: the Argon2 working memory (which holds
    // password-dependent data) is wiped after use. The `argon2` crate does not do this for
    // memory it allocates itself, so this crate owns and wipes the buffer.
    #[test]
    fn sec_c06_argon2_scratch_memory_is_wiped_after_use() {
        wipe_hook::take();
        derive_master_key("CANARY-password", &KdfParams::floor([0x09; 16])).unwrap();
        let wipes = wipe_hook::take();
        assert_eq!(
            wipes,
            vec![(true, true)],
            "one used buffer, all-zero after drop"
        );
    }

    // SEC-C10 / docs/04 §13: project-specific Argon2id vectors for NFKD inputs. Expected
    // values were produced by an independent implementation (Python `argon2-cffi`, i.e.
    // the Argon2 reference C library, with `unicodedata.normalize("NFKD", ...)`), floor
    // profile m=65536 KiB, t=3, p=1, 32-byte output:
    //   hash_secret_raw(nfkd(pw), salt, time_cost=3, memory_cost=65536, parallelism=1,
    //                   hash_len=32, type=ID, version=19)
    #[test]
    fn sec_c10_cross_implementation_vectors_for_nfkd_passwords() {
        let cases: [(&str, u8, &str); 3] = [
            (
                "CANARY-password",
                0x02,
                "2a8db46707e60d79fa42c709461049126aa01f218ce2a5f3a2ea269aaf5b0f9e",
            ),
            (
                "caf\u{e9}-CANARY",
                0x07,
                "04053ecd87765ad2feb2da2a8843c776dea6d93f9c16cb771a0933854ce1b862",
            ),
            (
                "\u{d55c}\u{ff21}\u{fb01}\u{1f600}-CANARY",
                0x09,
                "934c2813e7297d53c332c15595f655267c5147cf1720a1aff70633372314062f",
            ),
        ];
        for (pw, salt, want) in cases {
            let mk = derive_master_key(pw, &KdfParams::floor([salt; 16])).unwrap();
            assert_eq!(hex::encode(mk.expose_secret()), want, "password {pw:?}");
        }
    }

    #[test]
    fn derive_applies_nfkd_before_hashing() {
        let params = KdfParams::floor([0x07; 16]);
        let composed = derive_master_key("caf\u{e9}-CANARY", &params).unwrap();
        let decomposed = derive_master_key("cafe\u{301}-CANARY", &params).unwrap();
        assert_eq!(composed.expose_secret(), decomposed.expose_secret());
        let other = derive_master_key("cafe-CANARY", &params).unwrap();
        assert_ne!(composed.expose_secret(), other.expose_secret());
    }

    #[test]
    fn different_salts_and_passwords_give_different_keys() {
        let a = derive_master_key("CANARY-a", &KdfParams::floor([1; 16])).unwrap();
        let b = derive_master_key("CANARY-a", &KdfParams::floor([2; 16])).unwrap();
        let c = derive_master_key("CANARY-b", &KdfParams::floor([1; 16])).unwrap();
        assert_ne!(a.expose_secret(), b.expose_secret());
        assert_ne!(a.expose_secret(), c.expose_secret());
    }

    // Benchmark (run explicitly: `cargo test --release -p arya-vault-crypto -- --ignored
    // --nocapture bench_`). Prints Argon2id wall-clock at the floor profile and what
    // `calibrate` picks for a 750 ms target on this machine.
    #[test]
    #[ignore = "benchmark; run explicitly with --ignored --nocapture"]
    fn bench_argon2id_default_and_calibrated() {
        let runs = 5u32;
        let p = KdfParams::floor([0x42; 16]);
        let _warm = derive_master_key("CANARY-bench", &p).unwrap();
        let start = Instant::now();
        for _ in 0..runs {
            let _k = derive_master_key("CANARY-bench", &p).unwrap();
        }
        eprintln!(
            "BENCH argon2id floor m={} KiB t={} p={}: {:.1} ms/derive (mean of {runs})",
            p.m_kib,
            p.t,
            p.p,
            start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs)
        );
        let start = Instant::now();
        let c = calibrate(750, M_KIB_CALIBRATION_CAP, &mut OsRng).unwrap();
        eprintln!(
            "BENCH calibrate(750 ms, cap 256 MiB) -> m={} KiB t={} p={} (calibration took {:.0} ms)",
            c.m_kib,
            c.t,
            c.p,
            start.elapsed().as_secs_f64() * 1000.0
        );
        let start = Instant::now();
        let _k = derive_master_key("CANARY-bench", &c).unwrap();
        eprintln!(
            "BENCH derive at calibrated params: {:.1} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
    }

    fn ok() -> KdfParams {
        KdfParams {
            m_kib: M_KIB_MIN,
            t: T_MIN,
            p: P_MIN,
            salt: [0; 16],
        }
    }

    // SEC-C11 / SEC-C02: every floor and ceiling, accept at the limit and reject beyond.
    #[test]
    fn sec_c11_memory_bounds() {
        let mut p = ok();
        p.m_kib = M_KIB_MIN;
        assert!(p.validate().is_ok());
        p.m_kib = M_KIB_MAX;
        assert!(p.validate().is_ok());
        for bad in [0, 1, M_KIB_MIN - 1, M_KIB_MAX + 1, u32::MAX] {
            p.m_kib = bad;
            assert_eq!(
                p.validate(),
                Err(KdfError::OutOfRange(KdfParam::Memory)),
                "m={bad}"
            );
        }
    }

    #[test]
    fn sec_c11_iteration_bounds() {
        let mut p = ok();
        for good in [T_MIN, T_MAX] {
            p.t = good;
            assert!(p.validate().is_ok());
        }
        for bad in [0, 1, T_MIN - 1, T_MAX + 1, u32::MAX] {
            p.t = bad;
            assert_eq!(
                p.validate(),
                Err(KdfError::OutOfRange(KdfParam::Iterations)),
                "t={bad}"
            );
        }
    }

    #[test]
    fn sec_c11_parallelism_bounds() {
        let mut p = ok();
        for good in [P_MIN, P_MAX] {
            p.p = good;
            assert!(p.validate().is_ok());
        }
        for bad in [0, P_MAX + 1, u32::MAX] {
            p.p = bad;
            assert_eq!(
                p.validate(),
                Err(KdfError::OutOfRange(KdfParam::Parallelism)),
                "p={bad}"
            );
        }
    }

    #[test]
    fn sec_c11_floor_is_the_documented_default() {
        assert_eq!((M_KIB_MIN, T_MIN, P_MIN), (65_536, 3, 1));
        assert_eq!((M_KIB_MAX, T_MAX, P_MAX), (1_048_576, 10, 8));
        assert_eq!(SALT_LEN, 16);
    }

    // SEC-C02: out-of-range parameters are rejected before any hashing happens. A hostile
    // header asking for 4 TiB would otherwise OOM or hang; here it returns immediately.
    #[test]
    fn sec_c02_hostile_parameters_rejected_before_hashing() {
        let start = Instant::now();
        let hostile = KdfParams {
            m_kib: u32::MAX,
            t: u32::MAX,
            p: 0xFF_FFFF,
            salt: [0; 16],
        };
        assert!(matches!(
            derive_master_key("CANARY", &hostile),
            Err(KdfError::OutOfRange(_))
        ));
        let weak = KdfParams {
            m_kib: 8,
            t: 1,
            p: 1,
            salt: [0; 16],
        };
        assert!(matches!(
            derive_master_key("CANARY", &weak),
            Err(KdfError::OutOfRange(_))
        ));
        assert!(
            start.elapsed().as_millis() < 200,
            "validation must precede hashing"
        );
    }

    #[test]
    fn calibrate_never_goes_below_floors_and_respects_caps() {
        // A tiny target must return exactly the floor profile.
        let p = calibrate(1, M_KIB_CALIBRATION_CAP, &mut OsRng).unwrap();
        assert_eq!((p.m_kib, p.t, p.p), (M_KIB_MIN, T_MIN, P_MIN));
        assert!(p.validate().is_ok());
        // A max below the floor is clamped up to the floor, not below it.
        let p = calibrate(1, 1, &mut OsRng).unwrap();
        assert_eq!(p.m_kib, M_KIB_MIN);
        // Salt is fresh randomness.
        let q = calibrate(1, M_KIB_CALIBRATION_CAP, &mut OsRng).unwrap();
        assert_ne!(p.salt, q.salt);
    }

    #[test]
    fn calibrate_raises_cost_for_a_larger_target_but_stays_in_bounds() {
        // 400 ms is above the floor profile on any plausible CI machine; whatever it
        // picks must be valid, capped, and at least the floor.
        let p = calibrate(400, 128 * 1024, &mut OsRng).unwrap();
        assert!(p.validate().is_ok());
        assert!(p.m_kib <= 128 * 1024);
        assert!(p.m_kib >= M_KIB_MIN && p.t >= T_MIN);
    }

    // Canonical CBOR of the kdf struct, written out by hand:
    //  a6                      map(6)
    //   61 70  | 01            "p": 1
    //   61 74  | 03            "t": 3
    //   63 616c67 | 68 6172676f6e326964   "alg": "argon2id"
    //   64 73616c74 | 50 <16>             "salt": bytes(16)
    //   65 6d5f6b6962 | 1a 00010000       "m_kib": 65536
    //   67 76657273696f6e | 13            "version": 19
    #[test]
    fn canonical_cbor_golden_bytes() {
        let p = KdfParams {
            m_kib: 65_536,
            t: 3,
            p: 1,
            salt: [0xAB; 16],
        };
        let want = "a6\
            6170 01\
            6174 03\
            63616c67 686172676f6e326964\
            6473616c74 50abababababababababababababababab\
            656d5f6b6962 1a00010000\
            6776657273696f6e 13";
        assert_eq!(hex::encode(p.canonical_cbor()), want.replace(' ', ""));
    }

    #[test]
    fn canonical_cbor_is_deterministic_and_field_sensitive() {
        let p = KdfParams {
            m_kib: 262_144,
            t: 4,
            p: 2,
            salt: [9; 16],
        };
        assert_eq!(p.canonical_cbor(), p.clone().canonical_cbor());
        for q in [
            KdfParams {
                m_kib: p.m_kib + 1,
                ..p.clone()
            },
            KdfParams {
                t: p.t + 1,
                ..p.clone()
            },
            KdfParams {
                p: p.p + 1,
                ..p.clone()
            },
            KdfParams {
                salt: [8; 16],
                ..p.clone()
            },
        ] {
            assert_ne!(p.canonical_cbor(), q.canonical_cbor());
        }
    }
}
