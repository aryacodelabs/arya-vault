//! Argon2id, XChaCha20-Poly1305, HKDF, key wrapping, recovery key (docs/04).
//!
//! This crate composes standard RustCrypto primitives exactly as specified in
//! `docs/04-crypto-spec.md`. It contains **no** custom primitives or protocols.
//!
//! # Security contracts (summary)
//! * All randomness (keys, nonces, salts) comes from the OS CSPRNG via [`rng::OsRng`];
//!   the only other [`rng::Rng`] implementation is gated behind the non-default
//!   `deterministic-rng` feature, which cannot be compiled in release builds.
//! * Nonces are 192-bit, random and generated *inside* [`aead::seal`]; callers cannot
//!   supply or reuse one.
//! * Key types ([`keys`]) have no `Debug`/`Display`/`Clone`/`Serialize` and are
//!   zeroized on drop.
//! * KDF parameters are range-checked (floors **and** ceilings) before any hashing.
//! * No function in this crate performs I/O or networking.
//!
//! Header/envelope byte formats are out of scope here (T02).

// Tests may unwrap/expect/panic; non-test code may not (CLAUDE.md conventions).
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

#[cfg(all(feature = "deterministic-rng", not(debug_assertions)))]
compile_error!(
    "the `deterministic-rng` feature is for golden-file generation only and must not be \
     enabled in release builds"
);

pub mod aead;
pub mod format;
pub mod hkdf;
pub mod kdf;
pub mod keys;
pub mod normalize;
pub mod recovery_key;
pub mod rng;
mod secret;
pub mod vault_key;
pub mod wrap;

/// Length in bytes of the vault identifier (docs/04 §5).
pub const VAULT_ID_LEN: usize = 16;

/// A vault identifier (random 16 bytes; HKDF salt and AAD component).
pub type VaultId = [u8; VAULT_ID_LEN];

#[cfg(test)]
mod policy_tests {
    //! Source/manifest policy checks (SEC-C01, SEC-C03). These are deliberately blunt
    //! text checks over this crate's own files, complementing `cargo deny` in CI.

    const CARGO_TOML: &str = include_str!("../Cargo.toml");
    const SOURCES: &[(&str, &str)] = &[
        ("aead.rs", include_str!("aead.rs")),
        ("format/cbor.rs", include_str!("format/cbor.rs")),
        ("format/envelope.rs", include_str!("format/envelope.rs")),
        ("format/header.rs", include_str!("format/header.rs")),
        ("format/mod.rs", include_str!("format/mod.rs")),
        ("format/padding.rs", include_str!("format/padding.rs")),
        ("format/path.rs", include_str!("format/path.rs")),
        ("hkdf.rs", include_str!("hkdf.rs")),
        ("kdf.rs", include_str!("kdf.rs")),
        ("keys.rs", include_str!("keys.rs")),
        ("lib.rs", include_str!("lib.rs")),
        ("normalize.rs", include_str!("normalize.rs")),
        ("recovery_key.rs", include_str!("recovery_key.rs")),
        ("rng.rs", include_str!("rng.rs")),
        ("secret.rs", include_str!("secret.rs")),
        ("vault_key.rs", include_str!("vault_key.rs")),
        ("wrap.rs", include_str!("wrap.rs")),
    ];

    fn section<'a>(name: &str) -> Vec<&'a str> {
        let header = format!("[{name}]");
        let mut in_section = false;
        let mut crates = Vec::new();
        for line in CARGO_TOML.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_section = line == header;
                continue;
            }
            if in_section
                && !line.is_empty()
                && !line.starts_with('#')
                && let Some((name, _)) = line.split_once('=')
            {
                crates.push(name.trim());
            }
        }
        crates
    }

    // SEC-C01: only the allowlisted crates (task T01 / docs/09 §crypto) may be depended on.
    #[test]
    fn sec_c01_dependencies_are_on_the_allowlist() {
        const ALLOWED: &[&str] = &[
            "argon2",
            "chacha20poly1305",
            "hkdf",
            "sha2",
            "zeroize",
            "secrecy",
            "subtle",
            "getrandom",
            "ciborium",
            "unicode-normalization",
            "thiserror",
        ];
        for dep in section("dependencies") {
            assert!(
                ALLOWED.contains(&dep),
                "dependency `{dep}` is not on the T01 allowlist"
            );
        }
        for dep in section("dev-dependencies") {
            assert!(
                ["proptest", "hex"].contains(&dep),
                "dev-dependency `{dep}` not allowed"
            );
        }
    }

    // SEC-C01: no other AEAD/hash/KDF primitives are referenced from source.
    #[test]
    fn sec_c01_no_unlisted_primitives_in_source() {
        for (file, src) in SOURCES {
            if *file == "lib.rs" {
                continue; // contains these very strings
            }
            let code = src.split("#[cfg(test)]").next().unwrap_or("");
            for banned in [
                "aes_gcm", "Aes256", "md5", "sha1::", "blake3", "scrypt", "pbkdf2", " ring::",
                "use ring",
            ] {
                assert!(!code.contains(banned), "{file} references `{banned}`");
            }
        }
    }

    // SEC-C03 (source check): the only call into the OS RNG is in rng.rs, and no other RNG
    // crate or API is used anywhere.
    #[test]
    fn sec_c03_only_rng_rs_touches_the_os_rng() {
        for (file, src) in SOURCES {
            if *file == "lib.rs" {
                continue;
            }
            let code = src.split("#[cfg(test)]").next().unwrap_or("");
            for banned in [
                "thread_rng",
                "SmallRng",
                "StdRng",
                "rand::",
                "fastrand",
                "Instant::now().elapsed",
            ] {
                assert!(!code.contains(banned), "{file} references `{banned}`");
            }
            if *file != "rng.rs" {
                assert!(
                    !code.contains("getrandom"),
                    "{file} must obtain randomness via Rng only"
                );
            }
        }
    }
}
