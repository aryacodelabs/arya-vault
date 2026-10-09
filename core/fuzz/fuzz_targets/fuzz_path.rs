//! Fuzz target for the remote path grammar (`format::path::parse_path`, SEC-Y10).
//!
//! Properties: never panics; an accepted path is exactly the canonical spelling produced
//! by the formatters (no second spelling of the same file is ever accepted).
#![no_main]

use arya_vault_crypto::format::path::{PathInfo, header_path, manifest_path, parse_path, segment_path, snapshot_path};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let text = String::from_utf8_lossy(data);
    if let Ok(info) = parse_path(&text) {
        let canonical = match info {
            PathInfo::Header { epoch, header_version, device_id } => header_path(epoch, header_version, &device_id),
            PathInfo::Segment { device_id, seq } => segment_path(&device_id, seq),
            PathInfo::Manifest { device_id, counter } => manifest_path(&device_id, counter),
            PathInfo::Snapshot { hlc, device_id } => snapshot_path(hlc, &device_id),
        };
        assert_eq!(canonical, text);
    }
});
