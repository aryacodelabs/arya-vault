//! Remote file-name grammar and path binding (docs/06 §3, docs/04 §6, SEC-Y10).
//!
//! Paths are relative to `<provider root>/AryaVault/<vault_id>/` and use **lowercase,
//! fixed-width hex only** (no item names, nothing derived from content):
//!
//! | file | path |
//! |---|---|
//! | header | `header-<epoch:8hex>-<version:8hex>-<device_id:32hex>.bin` |
//! | segment | `devices/<device_id:32hex>/<seq:16hex>.seg` |
//! | manifest | `devices/<device_id:32hex>/manifest-<counter:16hex>.bin` |
//! | snapshot | `snapshots/<hlc:16hex>-<device_id:32hex>.snap` |
//!
//! The parsers accept exactly the canonical spelling that the formatters produce, so a
//! name identifies its value uniquely; anything else (uppercase, other widths, extra
//! components, `..`) is [`FormatError::BadPath`].

use super::FormatError;

/// A parsed remote path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathInfo {
    /// `header-<epoch>-<version>-<device>.bin`
    Header {
        /// Key epoch.
        epoch: u32,
        /// Header version.
        header_version: u32,
        /// Writing device.
        device_id: [u8; 16],
    },
    /// `devices/<device>/<seq>.seg`
    Segment {
        /// Owning device.
        device_id: [u8; 16],
        /// Segment sequence number.
        seq: u64,
    },
    /// `devices/<device>/manifest-<counter>.bin`
    Manifest {
        /// Owning device.
        device_id: [u8; 16],
        /// Manifest counter.
        counter: u64,
    },
    /// `snapshots/<hlc>-<device>.snap`
    Snapshot {
        /// Packed hybrid logical clock value in the name.
        hlc: u64,
        /// Writing device.
        device_id: [u8; 16],
    },
}

fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

/// Parses exactly `2 * N` lowercase hex digits into `N` bytes.
fn hex_bytes<const N: usize>(s: &str) -> Result<[u8; N], FormatError> {
    let b = s.as_bytes();
    if b.len() != 2 * N {
        return Err(FormatError::BadPath);
    }
    let mut out = [0u8; N];
    for (i, pair) in b.chunks_exact(2).enumerate() {
        let hi = hex_val(pair[0]).ok_or(FormatError::BadPath)?;
        let lo = hex_val(pair[1]).ok_or(FormatError::BadPath)?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn hex_u64(s: &str) -> Result<u64, FormatError> {
    Ok(u64::from_be_bytes(hex_bytes::<8>(s)?))
}

fn hex_u32(s: &str) -> Result<u32, FormatError> {
    Ok(u32::from_be_bytes(hex_bytes::<4>(s)?))
}

fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from(DIGITS[usize::from(b >> 4)]));
        s.push(char::from(DIGITS[usize::from(b & 15)]));
    }
    s
}

/// `header-<epoch>-<version>-<device>.bin`
pub fn header_path(epoch: u32, header_version: u32, device_id: &[u8; 16]) -> String {
    format!(
        "header-{}-{}-{}.bin",
        to_hex(&epoch.to_be_bytes()),
        to_hex(&header_version.to_be_bytes()),
        to_hex(device_id)
    )
}

/// `devices/<device>/<seq>.seg`
pub fn segment_path(device_id: &[u8; 16], seq: u64) -> String {
    format!(
        "devices/{}/{}.seg",
        to_hex(device_id),
        to_hex(&seq.to_be_bytes())
    )
}

/// `devices/<device>/manifest-<counter>.bin`
pub fn manifest_path(device_id: &[u8; 16], counter: u64) -> String {
    format!(
        "devices/{}/manifest-{}.bin",
        to_hex(device_id),
        to_hex(&counter.to_be_bytes())
    )
}

/// `snapshots/<hlc>-<device>.snap`
pub fn snapshot_path(hlc: u64, device_id: &[u8; 16]) -> String {
    format!(
        "snapshots/{}-{}.snap",
        to_hex(&hlc.to_be_bytes()),
        to_hex(device_id)
    )
}

/// Parses a header file name (`header-…bin`, no directory components).
pub fn parse_header_path(path: &str) -> Result<PathInfo, FormatError> {
    let rest = path
        .strip_prefix("header-")
        .and_then(|r| r.strip_suffix(".bin"))
        .ok_or(FormatError::BadPath)?;
    let parts: Vec<&str> = rest.split('-').collect();
    let [epoch, version, device] = parts.as_slice() else {
        return Err(FormatError::BadPath);
    };
    Ok(PathInfo::Header {
        epoch: hex_u32(epoch)?,
        header_version: hex_u32(version)?,
        device_id: hex_bytes(device)?,
    })
}

/// Parses `devices/<device>/<seq>.seg`.
pub fn parse_segment_path(path: &str) -> Result<PathInfo, FormatError> {
    let rest = path.strip_prefix("devices/").ok_or(FormatError::BadPath)?;
    let (device, file) = rest.split_once('/').ok_or(FormatError::BadPath)?;
    let seq = file.strip_suffix(".seg").ok_or(FormatError::BadPath)?;
    Ok(PathInfo::Segment {
        device_id: hex_bytes(device)?,
        seq: hex_u64(seq)?,
    })
}

/// Parses `devices/<device>/manifest-<counter>.bin`.
pub fn parse_manifest_path(path: &str) -> Result<PathInfo, FormatError> {
    let rest = path.strip_prefix("devices/").ok_or(FormatError::BadPath)?;
    let (device, file) = rest.split_once('/').ok_or(FormatError::BadPath)?;
    let counter = file
        .strip_prefix("manifest-")
        .and_then(|f| f.strip_suffix(".bin"))
        .ok_or(FormatError::BadPath)?;
    Ok(PathInfo::Manifest {
        device_id: hex_bytes(device)?,
        counter: hex_u64(counter)?,
    })
}

/// Parses `snapshots/<hlc>-<device>.snap`.
pub fn parse_snapshot_path(path: &str) -> Result<PathInfo, FormatError> {
    let file = path
        .strip_prefix("snapshots/")
        .ok_or(FormatError::BadPath)?;
    let rest = file.strip_suffix(".snap").ok_or(FormatError::BadPath)?;
    let (hlc, device) = rest.split_once('-').ok_or(FormatError::BadPath)?;
    Ok(PathInfo::Snapshot {
        hlc: hex_u64(hlc)?,
        device_id: hex_bytes(device)?,
    })
}

/// Parses any of the four path shapes.
pub fn parse_path(path: &str) -> Result<PathInfo, FormatError> {
    if path.starts_with("header-") {
        parse_header_path(path)
    } else if path.starts_with("snapshots/") {
        parse_snapshot_path(path)
    } else if path.ends_with(".seg") {
        parse_segment_path(path)
    } else {
        parse_manifest_path(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const DEV: [u8; 16] = [0xab; 16];

    #[test]
    fn formatters_and_parsers_round_trip() {
        assert_eq!(
            parse_path(&segment_path(&DEV, 1)).unwrap(),
            PathInfo::Segment {
                device_id: DEV,
                seq: 1
            }
        );
        assert_eq!(
            segment_path(&DEV, 255),
            "devices/abababababababababababababababab/00000000000000ff.seg"
        );
        assert_eq!(
            parse_path(&manifest_path(&DEV, u64::MAX)).unwrap(),
            PathInfo::Manifest {
                device_id: DEV,
                counter: u64::MAX
            }
        );
        assert_eq!(
            parse_path(&snapshot_path(0x0123_4567_89ab_cdef, &DEV)).unwrap(),
            PathInfo::Snapshot {
                hlc: 0x0123_4567_89ab_cdef,
                device_id: DEV
            }
        );
        assert_eq!(
            parse_path(&header_path(2, 0x10, &DEV)).unwrap(),
            PathInfo::Header {
                epoch: 2,
                header_version: 0x10,
                device_id: DEV
            }
        );
        assert_eq!(
            header_path(2, 16, &DEV),
            "header-00000002-00000010-abababababababababababababababab.bin"
        );
    }

    #[test]
    fn hostile_paths_are_rejected() {
        let d = "abababababababababababababababab";
        let bad = [
            "".to_string(),
            "/".to_string(),
            format!("devices/{d}/0000000000000001.SEG"),
            format!("devices/{d}/0000000000000001.seg/"),
            format!("devices/{d}/000000000000001.seg"), // 15 digits
            format!("devices/{d}/00000000000000001.seg"), // 17 digits
            format!("devices/{d}/000000000000000G.seg"),
            format!("devices/{d}/000000000000000A.seg"), // uppercase hex
            format!("devices/{d}/+000000000000001.seg"),
            format!("devices/{}/0000000000000001.seg", d.to_uppercase()),
            format!("devices/{}/0000000000000001.seg", &d[1..]),
            format!("devices/../{d}/0000000000000001.seg"),
            format!("devices/{d}/../{d}/0000000000000001.seg"),
            format!("devices/{d}/x/0000000000000001.seg"),
            format!("/devices/{d}/0000000000000001.seg"),
            format!("devices\\{d}\\0000000000000001.seg"),
            format!("devices/{d}/manifest-0000000000000001.seg"),
            format!("devices/{d}/manifest-1.bin"),
            format!("snapshots/0000000000000001-{d}.snap.bak"),
            format!("snapshots/0000000000000001_{d}.snap"),
            format!("snapshots/0000000000000001-{d}-x.snap"),
            format!("header-1-2-{d}.bin"),
            format!("header-00000001-00000002-{d}-extra.bin"),
            "header-00000001-00000002.bin".to_string(),
            format!("sub/header-00000001-00000002-{d}.bin"),
            "devices/\u{ff10}/x.seg".to_string(),
        ];
        for p in &bad {
            assert_eq!(parse_path(p).err(), Some(FormatError::BadPath), "{p:?}");
        }
    }

    proptest! {
        #[test]
        fn prop_round_trip(dev in proptest::array::uniform16(any::<u8>()), n in any::<u64>(), e in any::<u32>(), v in any::<u32>()) {
            prop_assert_eq!(parse_path(&segment_path(&dev, n)).unwrap(), PathInfo::Segment { device_id: dev, seq: n });
            prop_assert_eq!(parse_path(&manifest_path(&dev, n)).unwrap(), PathInfo::Manifest { device_id: dev, counter: n });
            prop_assert_eq!(parse_path(&snapshot_path(n, &dev)).unwrap(), PathInfo::Snapshot { hlc: n, device_id: dev });
            prop_assert_eq!(parse_path(&header_path(e, v, &dev)).unwrap(), PathInfo::Header { epoch: e, header_version: v, device_id: dev });
        }

        #[test]
        fn prop_parse_never_panics_and_accepts_only_canonical(s in ".{0,120}") {
            if let Ok(info) = parse_path(&s) {
                let back = match info {
                    PathInfo::Header { epoch, header_version, device_id } => header_path(epoch, header_version, &device_id),
                    PathInfo::Segment { device_id, seq } => segment_path(&device_id, seq),
                    PathInfo::Manifest { device_id, counter } => manifest_path(&device_id, counter),
                    PathInfo::Snapshot { hlc, device_id } => snapshot_path(hlc, &device_id),
                };
                prop_assert_eq!(back, s);
            }
        }
    }
}
