//! Fuzz target for the CSV importers (`parse_csv`; SEC-Y05, US-09).
//!
//! Properties: never panics or allocates unboundedly under tight limits; every accepted bundle
//! only contains items that satisfy the vault's own validation, with in-range folder indices.
#![no_main]

use arya_vault_vault::{ImportLimits, parse_csv};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let limits = ImportLimits { max_file_bytes: 1 << 20, max_records: 2_000, max_field_bytes: 64 << 10, max_depth: 16 };
    if let Ok(bundle) = parse_csv(data, &limits) {
        assert!(bundle.items.len() + bundle.skipped.len() <= limits.max_records);
        for item in &bundle.items {
            assert!(item.validate().is_ok());
            assert!(item.folder.is_none_or(|f| f < bundle.folders.len()));
        }
        for f in &bundle.folders {
            assert!(!f.name.is_empty());
        }
    }
});
