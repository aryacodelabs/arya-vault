//! Fuzz target for the envelope parser and opener (docs/04 §6; SEC-Y05, SEC-Y10).
//!
//! Properties: `Envelope::decode` never panics or over-allocates and an accepted envelope
//! re-encodes identically; `open_envelope` / `open_envelope_at_path` with a fixed key
//! never panic (they must return a typed error for anything not sealed under that key).
#![no_main]

use arya_vault_crypto::format::envelope::{Envelope, open_envelope, open_envelope_at_path};
use arya_vault_crypto::format::path::PathInfo;
use arya_vault_crypto::hkdf::{SubKeyLabel, subkey};
use arya_vault_crypto::keys::VaultKey;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let vault_id = [0x31u8; 16];
    if let Ok(env) = Envelope::decode(data) {
        assert_eq!(env.encode().expect("decoded envelope must re-encode"), data);
    }
    let vk = VaultKey::from_bytes([0x77; 32]);
    let Ok(key) = subkey(&vk, &vault_id, SubKeyLabel::Log, 1) else { return };
    let _ = open_envelope(&key, data, &vault_id);
    let path = PathInfo::Segment { device_id: [0xa1; 16], seq: 2 };
    let _ = open_envelope_at_path(&key, data, &vault_id, &path);
});
