//! Fuzz target for the AryaVault export parsers (docs/13; SEC-Y05).
//!
//! Two entry points are fed the same bytes: the container (`read_container_info`, which checks
//! magic, version, KDF bounds and lengths without running Argon2, so the target stays fast) and
//! the decrypted payload (`parse_aryavault_payload`, a parser of untrusted structure that is
//! reachable by anyone who knows the password).
#![no_main]

use arya_vault_vault::{ImportLimits, parse_aryavault_payload, read_container_info};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = ImportLimits { max_file_bytes: 1 << 20, max_records: 2_000, max_field_bytes: 64 << 10, max_depth: 16 };
    if let Ok(info) = read_container_info(data, &limits) {
        assert!((65_536..=1_048_576).contains(&info.m_kib) && (3..=10).contains(&info.t) && (1..=8).contains(&info.p));
        assert_eq!((info.ciphertext_len - 16) % 1024, 0);
    }
    if let Ok(bundle) = parse_aryavault_payload(data, &limits) {
        for item in &bundle.items {
            assert!(item.validate().is_ok());
            assert!(item.folder.is_none_or(|f| f < bundle.folders.len()));
        }
    }
});
