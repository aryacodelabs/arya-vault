//! The encrypted container envelope (docs/04 §6-7).
//!
//! # Wire layout (v1, fixed width, big-endian)
//! ```text
//! offset  size  field
//!      0     4  magic "AVLT"
//!      4     2  format_version (u16)
//!      6     1  kind (1 = segment, 2 = snapshot, 3 = manifest)
//!      7    16  vault_id
//!     23     4  epoch (u32)
//!     27    16  device_id
//!     43     8  seq (u64): segment seq / manifest counter; 0 for snapshots
//!     51    32  prev_hash: SHA-256 of the previous segment envelope; zero otherwise
//!     83    24  nonce (random)
//!    107     4  ct_len (u32)
//!    111 ct_len  ciphertext ‖ 16-byte tag (of the padded plaintext)
//! ```
//! The spec leaves the byte-level layout open (it defines the field list and the AAD);
//! a fixed layout needs no parser beyond length checks, which keeps the attack surface
//! minimal. Fields that do not apply to a kind are fixed at zero and rejected otherwise.
//!
//! # AAD (SEC-C05)
//! `AAD = canonical_cbor({ magic, format_version, kind, vault_id, epoch, device_id, seq,
//! prev_hash })`: every header field except `nonce`, ciphertext and tag, as a map with
//! text keys of those names (`magic` is a 4-byte byte string). Changing any field of the
//! envelope therefore makes authentication fail.
//!
//! The plaintext is padded to a multiple of 1 KiB before encryption (SEC-Y02) and size
//! limits are enforced on the wire length **before** anything is allocated (SEC-Y05).

use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use super::cbor::Value;
use super::padding::{self, BUCKET};
use super::path::PathInfo;
use super::{
    FORMAT_VERSION, FormatError, MAX_MANIFEST_PLAINTEXT, MAX_SEGMENT_PLAINTEXT,
    MAX_SNAPSHOT_PLAINTEXT, check_version,
};
use crate::VaultId;
use crate::aead::{self, NONCE_LEN, Nonce, TAG_LEN};
use crate::keys::SubKey;
use crate::rng::Rng;

/// The envelope magic.
pub const MAGIC: [u8; 4] = *b"AVLT";
/// Size of the fixed part of the wire format.
pub const FIXED_LEN: usize = 111;

/// The kind of container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Operation-log segment (`K_log`).
    Segment = 1,
    /// Snapshot (`K_snap`).
    Snapshot = 2,
    /// Device manifest (`K_manifest`).
    Manifest = 3,
}

impl Kind {
    fn from_u8(b: u8) -> Result<Kind, FormatError> {
        match b {
            1 => Ok(Kind::Segment),
            2 => Ok(Kind::Snapshot),
            3 => Ok(Kind::Manifest),
            _ => Err(FormatError::InvalidField("kind")),
        }
    }

    /// Maximum plaintext size (before padding) for this kind.
    pub const fn max_plaintext(self) -> usize {
        match self {
            Kind::Segment => MAX_SEGMENT_PLAINTEXT,
            Kind::Snapshot => MAX_SNAPSHOT_PLAINTEXT,
            Kind::Manifest => MAX_MANIFEST_PLAINTEXT,
        }
    }

    /// Maximum ciphertext (padded plaintext plus tag) accepted on the wire.
    const fn max_ciphertext(self) -> usize {
        padding::padded_len(self.max_plaintext()) + TAG_LEN
    }
}

/// The authenticated header fields of an envelope (everything except nonce and ciphertext).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeFields {
    /// Container kind.
    pub kind: Kind,
    /// Vault identifier.
    pub vault_id: VaultId,
    /// Key epoch the container is encrypted under.
    pub epoch: u32,
    /// Writing device.
    pub device_id: [u8; 16],
    /// Segment sequence number (≥ 1) or manifest counter; must be 0 for snapshots.
    pub seq: u64,
    /// SHA-256 of the previous segment envelope from this device (zero for seq 1);
    /// must be zero for snapshots and manifests.
    pub prev_hash: [u8; 32],
}

impl EnvelopeFields {
    /// Checks the per-kind rules for `seq` and `prev_hash`.
    fn check(&self) -> Result<(), FormatError> {
        match self.kind {
            Kind::Segment => {
                if self.seq == 0 {
                    return Err(FormatError::InvalidField("seq"));
                }
                if self.seq == 1 && self.prev_hash != [0u8; 32] {
                    return Err(FormatError::InvalidField("prev_hash"));
                }
            }
            Kind::Manifest => {
                if self.prev_hash != [0u8; 32] {
                    return Err(FormatError::InvalidField("prev_hash"));
                }
            }
            Kind::Snapshot => {
                if self.seq != 0 {
                    return Err(FormatError::InvalidField("seq"));
                }
                if self.prev_hash != [0u8; 32] {
                    return Err(FormatError::InvalidField("prev_hash"));
                }
            }
        }
        Ok(())
    }

    /// The AAD (see module docs).
    pub fn aad(&self) -> Result<Vec<u8>, FormatError> {
        Ok(Value::map(vec![
            (Value::text("magic"), Value::Bytes(MAGIC.to_vec())),
            (
                Value::text("format_version"),
                Value::Uint(u64::from(FORMAT_VERSION)),
            ),
            (Value::text("kind"), Value::Uint(self.kind as u64)),
            (
                Value::text("vault_id"),
                Value::Bytes(self.vault_id.to_vec()),
            ),
            (Value::text("epoch"), Value::Uint(u64::from(self.epoch))),
            (
                Value::text("device_id"),
                Value::Bytes(self.device_id.to_vec()),
            ),
            (Value::text("seq"), Value::Uint(self.seq)),
            (
                Value::text("prev_hash"),
                Value::Bytes(self.prev_hash.to_vec()),
            ),
        ])?
        .encode()?)
    }
}

/// A parsed (still encrypted) envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Format version found in the data.
    pub format_version: u16,
    /// Authenticated header fields.
    pub fields: EnvelopeFields,
    /// Random nonce.
    pub nonce: Nonce,
    /// Ciphertext ‖ tag.
    pub ciphertext: Vec<u8>,
}

impl Envelope {
    /// Serializes to the wire layout.
    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        check_version(u64::from(self.format_version))?;
        self.fields.check()?;
        let ct_len = u32::try_from(self.ciphertext.len()).map_err(|_| FormatError::TooLarge)?;
        if self.ciphertext.len() > self.fields.kind.max_ciphertext() {
            return Err(FormatError::TooLarge);
        }
        let mut out = Vec::with_capacity(FIXED_LEN + self.ciphertext.len());
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&self.format_version.to_be_bytes());
        out.push(self.fields.kind as u8);
        out.extend_from_slice(&self.fields.vault_id);
        out.extend_from_slice(&self.fields.epoch.to_be_bytes());
        out.extend_from_slice(&self.fields.device_id);
        out.extend_from_slice(&self.fields.seq.to_be_bytes());
        out.extend_from_slice(&self.fields.prev_hash);
        out.extend_from_slice(&self.nonce);
        out.extend_from_slice(&ct_len.to_be_bytes());
        out.extend_from_slice(&self.ciphertext);
        Ok(out)
    }

    /// Strictly parses the wire layout. Size limits are checked before the ciphertext is
    /// copied; the version is checked right after the magic.
    pub fn decode(bytes: &[u8]) -> Result<Envelope, FormatError> {
        if bytes.len() < 6 {
            return Err(FormatError::Truncated);
        }
        if bytes[..4] != MAGIC {
            return Err(FormatError::BadMagic);
        }
        let format_version = check_version(u64::from(u16::from_be_bytes([bytes[4], bytes[5]])))?;
        if bytes.len() < FIXED_LEN {
            return Err(FormatError::Truncated);
        }
        let kind = Kind::from_u8(bytes[6])?;
        let take = |range: std::ops::Range<usize>| &bytes[range];
        let arr16 = |r| <[u8; 16]>::try_from(take(r)).map_err(|_| FormatError::Truncated);
        let fields = EnvelopeFields {
            kind,
            vault_id: arr16(7..23)?,
            epoch: u32::from_be_bytes(
                <[u8; 4]>::try_from(take(23..27)).map_err(|_| FormatError::Truncated)?,
            ),
            device_id: arr16(27..43)?,
            seq: u64::from_be_bytes(
                <[u8; 8]>::try_from(take(43..51)).map_err(|_| FormatError::Truncated)?,
            ),
            prev_hash: <[u8; 32]>::try_from(take(51..83)).map_err(|_| FormatError::Truncated)?,
        };
        fields.check()?;
        let nonce: Nonce =
            <[u8; NONCE_LEN]>::try_from(take(83..107)).map_err(|_| FormatError::Truncated)?;
        let ct_len = u32::from_be_bytes(
            <[u8; 4]>::try_from(take(107..111)).map_err(|_| FormatError::Truncated)?,
        );
        let ct_len = usize::try_from(ct_len).map_err(|_| FormatError::TooLarge)?;
        // Limits first, before comparing with the actual size or allocating.
        if ct_len > kind.max_ciphertext() {
            return Err(FormatError::TooLarge);
        }
        // Padded plaintext is a non-empty multiple of the bucket size, plus the tag.
        if ct_len < BUCKET + TAG_LEN || !(ct_len - TAG_LEN).is_multiple_of(BUCKET) {
            return Err(FormatError::InvalidField("ct_len"));
        }
        let rest = &bytes[FIXED_LEN..];
        if rest.len() < ct_len {
            return Err(FormatError::Truncated);
        }
        if rest.len() > ct_len {
            return Err(FormatError::TrailingBytes);
        }
        Ok(Envelope {
            format_version,
            fields,
            nonce,
            ciphertext: rest.to_vec(),
        })
    }
}

/// SHA-256 of the full envelope bytes (the value stored as the next segment's `prev_hash`).
pub fn envelope_hash(envelope_bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(envelope_bytes).into()
}

/// Verifies that `fields` links to the previous segment (doc 04 §7).
///
/// `prev_hash_of_previous` is the [`envelope_hash`] of the same device's segment
/// `seq - 1`, or `None` if the caller does not have it. For `seq == 1` the stored
/// `prev_hash` must be zero; for `seq > 1` the previous hash must be supplied and equal.
/// Only meaningful for [`Kind::Segment`].
pub fn verify_chain(
    fields: &EnvelopeFields,
    prev_hash_of_previous: Option<&[u8; 32]>,
) -> Result<(), FormatError> {
    if fields.kind != Kind::Segment {
        return Err(FormatError::InvalidField("kind"));
    }
    if fields.seq == 1 {
        return if fields.prev_hash == [0u8; 32] {
            Ok(())
        } else {
            Err(FormatError::ChainMismatch)
        };
    }
    match prev_hash_of_previous {
        Some(h) if *h == fields.prev_hash => Ok(()),
        _ => Err(FormatError::ChainMismatch),
    }
}

/// A decrypted envelope.
pub struct Opened {
    /// The authenticated header fields.
    pub fields: EnvelopeFields,
    /// The unpadded plaintext (wiped on drop).
    pub plaintext: Zeroizing<Vec<u8>>,
}

/// Pads, encrypts and serializes a container.
///
/// `key` must be the sub-key for the kind (`K_log` for segments, `K_snap` for snapshots,
/// `K_manifest` for manifests, derived at `fields.epoch`); using another key simply fails
/// to open later. A fresh random nonce is drawn from `rng` on every call.
pub fn seal_envelope(
    key: &SubKey,
    fields: &EnvelopeFields,
    plaintext: &[u8],
    rng: &mut dyn Rng,
) -> Result<Vec<u8>, FormatError> {
    fields.check()?;
    if plaintext.len() > fields.kind.max_plaintext() {
        return Err(FormatError::TooLarge);
    }
    let padded = padding::pad(plaintext);
    let (nonce, ciphertext) = aead::seal(key, &fields.aad()?, &padded, rng)?;
    Envelope {
        format_version: FORMAT_VERSION,
        fields: fields.clone(),
        nonce,
        ciphertext,
    }
    .encode()
}

/// Parses, authenticates, decrypts and unpads an envelope for `vault_id`.
///
/// Padding is validated only after authentication succeeds.
pub fn open_envelope(
    key: &SubKey,
    bytes: &[u8],
    vault_id: &VaultId,
) -> Result<Opened, FormatError> {
    let env = Envelope::decode(bytes)?;
    if &env.fields.vault_id != vault_id {
        return Err(FormatError::WrongVault);
    }
    let padded = aead::open(key, &env.fields.aad()?, &env.nonce, &env.ciphertext)?;
    let plaintext = Zeroizing::new(padding::unpad(&padded)?.to_vec());
    if plaintext.len() > env.fields.kind.max_plaintext() {
        return Err(FormatError::TooLarge);
    }
    Ok(Opened {
        fields: env.fields,
        plaintext,
    })
}

/// Like [`open_envelope`], but first requires the envelope's identity to match the path
/// it was found at (SEC-Y10). A mismatch yields [`FormatError::PathMismatch`] and the file
/// must be quarantined; no decryption is attempted.
///
/// Rules: the path kind must match the envelope kind; `device_id` must match for every
/// kind; `seq` must equal the file's sequence number (segments) or counter (manifests).
/// Snapshot file names carry an HLC that the envelope does not repeat, so only the
/// device is bound there.
pub fn open_envelope_at_path(
    key: &SubKey,
    bytes: &[u8],
    vault_id: &VaultId,
    path: &PathInfo,
) -> Result<Opened, FormatError> {
    let env = Envelope::decode(bytes)?;
    let f = &env.fields;
    let ok = match (path, f.kind) {
        (PathInfo::Segment { device_id, seq }, Kind::Segment) => {
            *device_id == f.device_id && *seq == f.seq
        }
        (PathInfo::Manifest { device_id, counter }, Kind::Manifest) => {
            *device_id == f.device_id && *counter == f.seq
        }
        (PathInfo::Snapshot { device_id, .. }, Kind::Snapshot) => *device_id == f.device_id,
        _ => false,
    };
    if !ok {
        return Err(FormatError::PathMismatch);
    }
    open_envelope(key, bytes, vault_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::path::{manifest_path, parse_path, segment_path, snapshot_path};
    use crate::hkdf::{SubKeyLabel, subkey};
    use crate::keys::VaultKey;
    use crate::rng::OsRng;
    use proptest::prelude::*;

    const VID: VaultId = [0x31; 16];
    const DEV_A: [u8; 16] = [0xa1; 16];
    const DEV_B: [u8; 16] = [0xb2; 16];

    fn key(label: SubKeyLabel) -> SubKey {
        subkey(&VaultKey::from_bytes([0x77; 32]), &VID, label, 1).unwrap()
    }

    fn seg_fields(seq: u64, prev: [u8; 32]) -> EnvelopeFields {
        EnvelopeFields {
            kind: Kind::Segment,
            vault_id: VID,
            epoch: 1,
            device_id: DEV_A,
            seq,
            prev_hash: prev,
        }
    }

    fn sealed_segment() -> (EnvelopeFields, Vec<u8>) {
        let f = seg_fields(2, [0x44; 32]);
        let b = seal_envelope(
            &key(SubKeyLabel::Log),
            &f,
            b"CANARY-segment-plaintext",
            &mut OsRng,
        )
        .unwrap();
        (f, b)
    }

    #[test]
    fn round_trip_all_kinds() {
        for (kind, label, seq, prev) in [
            (Kind::Segment, SubKeyLabel::Log, 5u64, [0x55; 32]),
            (Kind::Segment, SubKeyLabel::Log, 1, [0; 32]),
            (Kind::Manifest, SubKeyLabel::Manifest, 7, [0; 32]),
            (Kind::Manifest, SubKeyLabel::Manifest, 0, [0; 32]),
            (Kind::Snapshot, SubKeyLabel::Snapshot, 0, [0; 32]),
        ] {
            let f = EnvelopeFields {
                kind,
                vault_id: VID,
                epoch: 9,
                device_id: DEV_A,
                seq,
                prev_hash: prev,
            };
            for len in [0usize, 1, 1023, 1024, 1025, 5000] {
                let pt = vec![0xCDu8; len];
                let bytes = seal_envelope(&key(label), &f, &pt, &mut OsRng).unwrap();
                assert_eq!(
                    bytes.len(),
                    FIXED_LEN + padding::padded_len(len) + TAG_LEN,
                    "{kind:?} {len}"
                );
                let o = open_envelope(&key(label), &bytes, &VID).unwrap();
                assert_eq!(o.fields, f);
                assert_eq!(o.plaintext.as_slice(), pt.as_slice());
                assert_eq!(Envelope::decode(&bytes).unwrap().encode().unwrap(), bytes);
            }
        }
    }

    // SEC-Y02: sizes reveal only the 1 KiB bucket.
    #[test]
    fn sec_y02_ciphertext_sizes_are_bucketed() {
        let f = seg_fields(1, [0; 32]);
        let k = key(SubKeyLabel::Log);
        let sizes: Vec<usize> = [0usize, 1, 500, 1023]
            .iter()
            .map(|n| {
                seal_envelope(&k, &f, &vec![1; *n], &mut OsRng)
                    .unwrap()
                    .len()
            })
            .collect();
        assert!(sizes.iter().all(|s| *s == sizes[0]));
        let big = seal_envelope(&k, &f, &vec![1; 1024], &mut OsRng)
            .unwrap()
            .len();
        assert_eq!(big, sizes[0] + 1024);
    }

    // SEC-C05 mutation tests: every authenticated header field, the nonce and every
    // ciphertext/tag byte. Header-field flips must be rejected (either structurally or by
    // authentication) and never open successfully.
    #[test]
    fn sec_c05_every_byte_flip_fails_to_open() {
        let (_, bytes) = sealed_segment();
        let k = key(SubKeyLabel::Log);
        assert!(open_envelope(&k, &bytes, &VID).is_ok());
        for i in 0..bytes.len() {
            for bit in [0x01u8, 0x80] {
                let mut m = bytes.clone();
                m[i] ^= bit;
                assert!(
                    open_envelope(&k, &m, &VID).is_err(),
                    "flip at byte {i} bit {bit:#x} was accepted"
                );
            }
        }
    }

    #[test]
    fn sec_c05_each_field_is_bound_in_the_aad() {
        let (f, _) = sealed_segment();
        let base = f.aad().unwrap();
        let muts = [
            EnvelopeFields {
                vault_id: [0x32; 16],
                ..f.clone()
            },
            EnvelopeFields {
                epoch: 2,
                ..f.clone()
            },
            EnvelopeFields {
                device_id: DEV_B,
                ..f.clone()
            },
            EnvelopeFields {
                seq: 3,
                ..f.clone()
            },
            EnvelopeFields {
                prev_hash: [0x45; 32],
                ..f.clone()
            },
            EnvelopeFields {
                kind: Kind::Manifest,
                prev_hash: [0; 32],
                ..f.clone()
            },
        ];
        for m in muts {
            assert_ne!(m.aad().unwrap(), base);
        }
        // Re-labelling a valid ciphertext by editing header fields to values that are still
        // structurally valid must fail authentication specifically.
        let (_, bytes) = sealed_segment();
        let k = key(SubKeyLabel::Log);
        for (range, delta) in [(26usize..27, 1u8), (42..43, 1), (50..51, 1), (82..83, 1)] {
            let mut m = bytes.clone();
            m[range.start] = m[range.start].wrapping_add(delta);
            assert_eq!(
                open_envelope(&k, &m, &VID).err(),
                Some(FormatError::Aead(
                    crate::aead::AeadError::AuthenticationFailed
                )),
                "byte {}",
                range.start
            );
        }
    }

    #[test]
    fn wrong_key_kind_or_vault_fails() {
        let (_, bytes) = sealed_segment();
        assert!(open_envelope(&key(SubKeyLabel::Snapshot), &bytes, &VID).is_err());
        assert!(open_envelope(&key(SubKeyLabel::Manifest), &bytes, &VID).is_err());
        assert_eq!(
            open_envelope(&key(SubKeyLabel::Log), &bytes, &[0; 16]).err(),
            Some(FormatError::WrongVault)
        );
    }

    #[test]
    fn structural_rejections() {
        let (_, bytes) = sealed_segment();
        assert_eq!(Envelope::decode(&[]).err(), Some(FormatError::Truncated));
        assert_eq!(Envelope::decode(b"AVL").err(), Some(FormatError::Truncated));
        assert_eq!(
            Envelope::decode(&bytes[..FIXED_LEN - 1]).err(),
            Some(FormatError::Truncated)
        );
        assert_eq!(
            Envelope::decode(&bytes[..bytes.len() - 1]).err(),
            Some(FormatError::Truncated)
        );
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(
            Envelope::decode(&longer).err(),
            Some(FormatError::TrailingBytes)
        );
        let mut m = bytes.clone();
        m[0] = b'X';
        assert_eq!(Envelope::decode(&m).err(), Some(FormatError::BadMagic));
        let mut m = bytes.clone();
        m[6] = 9;
        assert_eq!(
            Envelope::decode(&m).err(),
            Some(FormatError::InvalidField("kind"))
        );
        // seq 0 for a segment; non-zero prev_hash for seq 1; non-zero seq for a snapshot.
        let mut m = bytes.clone();
        m[43..51].copy_from_slice(&0u64.to_be_bytes());
        assert_eq!(
            Envelope::decode(&m).err(),
            Some(FormatError::InvalidField("seq"))
        );
        let mut m = bytes.clone();
        m[43..51].copy_from_slice(&1u64.to_be_bytes());
        assert_eq!(
            Envelope::decode(&m).err(),
            Some(FormatError::InvalidField("prev_hash"))
        );
        let snap = EnvelopeFields {
            kind: Kind::Snapshot,
            vault_id: VID,
            epoch: 1,
            device_id: DEV_A,
            seq: 0,
            prev_hash: [0; 32],
        };
        assert!(
            seal_envelope(
                &key(SubKeyLabel::Snapshot),
                &EnvelopeFields {
                    seq: 1,
                    ..snap.clone()
                },
                b"x",
                &mut OsRng
            )
            .is_err()
        );
        assert!(
            seal_envelope(
                &key(SubKeyLabel::Snapshot),
                &EnvelopeFields {
                    prev_hash: [1; 32],
                    ..snap
                },
                b"x",
                &mut OsRng
            )
            .is_err()
        );
    }

    // doc 04 §14 / docs/12 §6
    #[test]
    fn unsupported_format_version_is_distinct() {
        let (_, bytes) = sealed_segment();
        for v in [0u16, 2, 0x0100, 0xffff] {
            let mut m = bytes.clone();
            m[4..6].copy_from_slice(&v.to_be_bytes());
            assert_eq!(
                Envelope::decode(&m).err(),
                Some(FormatError::UnsupportedFormat {
                    found: v,
                    max_supported: 1
                })
            );
            // Even a future-format file that is too short to be a v1 envelope.
            let mut short = m[..8].to_vec();
            short[4..6].copy_from_slice(&v.to_be_bytes());
            assert_eq!(
                Envelope::decode(&short).err(),
                Some(FormatError::UnsupportedFormat {
                    found: v,
                    max_supported: 1
                })
            );
        }
    }

    // SEC-Y05: crafted length prefixes are rejected from the 4-byte field alone, without
    // allocating anything near the claimed size.
    #[test]
    fn sec_y05_hostile_length_prefixes_are_rejected_before_allocation() {
        let (_, bytes) = sealed_segment();
        for claimed in [
            u32::MAX,
            0x8000_0000,
            (MAX_SEGMENT_PLAINTEXT as u32) + 5000,
            0,
            1,
            16,
            1039,
            1041,
        ] {
            let mut m = bytes[..FIXED_LEN].to_vec();
            m[107..111].copy_from_slice(&claimed.to_be_bytes());
            let err = Envelope::decode(&m).err();
            assert!(
                matches!(
                    err,
                    Some(FormatError::TooLarge) | Some(FormatError::InvalidField("ct_len"))
                ),
                "claimed {claimed}: {err:?}"
            );
        }
        // A claim that is within limits but longer than the data present is Truncated.
        let mut m = bytes[..FIXED_LEN].to_vec();
        m[107..111].copy_from_slice(&((MAX_SEGMENT_PLAINTEXT as u32) + 1024 + 16).to_be_bytes());
        assert_eq!(Envelope::decode(&m).err(), Some(FormatError::Truncated));
    }

    #[test]
    fn plaintext_over_the_limit_is_refused_when_sealing() {
        let f = seg_fields(1, [0; 32]);
        let k = key(SubKeyLabel::Log);
        let ok = seal_envelope(&k, &f, &vec![0u8; MAX_SEGMENT_PLAINTEXT], &mut OsRng).unwrap();
        assert!(open_envelope(&k, &ok, &VID).is_ok());
        assert_eq!(
            seal_envelope(&k, &f, &vec![0u8; MAX_SEGMENT_PLAINTEXT + 1], &mut OsRng).err(),
            Some(FormatError::TooLarge)
        );
    }

    #[test]
    fn bad_padding_is_rejected_only_after_authentication() {
        // Seal a raw, mis-padded plaintext (all zeros: no 0x80 marker) with the real AAD.
        let f = seg_fields(1, [0; 32]);
        let k = key(SubKeyLabel::Log);
        let (nonce, ct) = aead::seal(&k, &f.aad().unwrap(), &vec![0u8; 1024], &mut OsRng).unwrap();
        let bytes = Envelope {
            format_version: 1,
            fields: f,
            nonce,
            ciphertext: ct,
        }
        .encode()
        .unwrap();
        assert_eq!(
            open_envelope(&k, &bytes, &VID).err(),
            Some(FormatError::Padding)
        );
        // Under the wrong key the answer is an authentication failure, never a padding oracle.
        assert_eq!(
            open_envelope(&key(SubKeyLabel::Manifest), &bytes, &VID).err(),
            Some(FormatError::Aead(
                crate::aead::AeadError::AuthenticationFailed
            ))
        );
    }

    // SEC-Y10: path binding.
    #[test]
    fn sec_y10_segment_from_device_a_at_device_b_path_is_rejected() {
        let (f, bytes) = sealed_segment();
        let k = key(SubKeyLabel::Log);
        let good = parse_path(&segment_path(&DEV_A, f.seq)).unwrap();
        assert!(open_envelope_at_path(&k, &bytes, &VID, &good).is_ok());
        for (path, why) in [
            (segment_path(&DEV_B, f.seq), "other device"),
            (segment_path(&DEV_A, f.seq + 1), "other seq"),
            (segment_path(&DEV_A, 1), "seq 1"),
            (manifest_path(&DEV_A, f.seq), "manifest path"),
            (snapshot_path(f.seq, &DEV_A), "snapshot path"),
        ] {
            let p = parse_path(&path).unwrap();
            assert_eq!(
                open_envelope_at_path(&k, &bytes, &VID, &p).err(),
                Some(FormatError::PathMismatch),
                "{why}"
            );
        }
    }

    #[test]
    fn sec_y10_manifest_and_snapshot_binding() {
        let km = key(SubKeyLabel::Manifest);
        let mf = EnvelopeFields {
            kind: Kind::Manifest,
            vault_id: VID,
            epoch: 1,
            device_id: DEV_A,
            seq: 4,
            prev_hash: [0; 32],
        };
        let mb = seal_envelope(&km, &mf, b"CANARY-manifest", &mut OsRng).unwrap();
        assert!(
            open_envelope_at_path(
                &km,
                &mb,
                &VID,
                &parse_path(&manifest_path(&DEV_A, 4)).unwrap()
            )
            .is_ok()
        );
        for p in [
            manifest_path(&DEV_B, 4),
            manifest_path(&DEV_A, 5),
            segment_path(&DEV_A, 4),
        ] {
            assert_eq!(
                open_envelope_at_path(&km, &mb, &VID, &parse_path(&p).unwrap()).err(),
                Some(FormatError::PathMismatch),
                "{p}"
            );
        }
        let ks = key(SubKeyLabel::Snapshot);
        let sf = EnvelopeFields {
            kind: Kind::Snapshot,
            vault_id: VID,
            epoch: 1,
            device_id: DEV_A,
            seq: 0,
            prev_hash: [0; 32],
        };
        let sb = seal_envelope(&ks, &sf, b"CANARY-snapshot", &mut OsRng).unwrap();
        assert!(
            open_envelope_at_path(
                &ks,
                &sb,
                &VID,
                &parse_path(&snapshot_path(0x1234, &DEV_A)).unwrap()
            )
            .is_ok()
        );
        assert_eq!(
            open_envelope_at_path(
                &ks,
                &sb,
                &VID,
                &parse_path(&snapshot_path(0x1234, &DEV_B)).unwrap()
            )
            .err(),
            Some(FormatError::PathMismatch)
        );
    }

    #[test]
    fn hash_chain_helpers() {
        let k = key(SubKeyLabel::Log);
        let f1 = seg_fields(1, [0; 32]);
        let b1 = seal_envelope(&k, &f1, b"one", &mut OsRng).unwrap();
        assert_eq!(envelope_hash(&b1), <[u8; 32]>::from(Sha256::digest(&b1)));
        assert!(verify_chain(&f1, None).is_ok());
        let f2 = seg_fields(2, envelope_hash(&b1));
        let b2 = seal_envelope(&k, &f2, b"two", &mut OsRng).unwrap();
        assert!(verify_chain(&f2, Some(&envelope_hash(&b1))).is_ok());
        assert_eq!(
            verify_chain(&f2, None).err(),
            Some(FormatError::ChainMismatch)
        );
        assert_eq!(
            verify_chain(&f2, Some(&[0u8; 32])).err(),
            Some(FormatError::ChainMismatch)
        );
        // A modified previous segment breaks the link.
        let mut tampered = b1.clone();
        tampered[FIXED_LEN] ^= 1;
        assert_eq!(
            verify_chain(&f2, Some(&envelope_hash(&tampered))).err(),
            Some(FormatError::ChainMismatch)
        );
        // The chain value is bound by the AAD: swapping it breaks authentication.
        let mut m = b2.clone();
        m[51] ^= 1;
        assert!(open_envelope(&k, &m, &VID).is_err());
        assert_eq!(
            verify_chain(
                &EnvelopeFields {
                    kind: Kind::Manifest,
                    ..f2
                },
                None
            )
            .err(),
            Some(FormatError::InvalidField("kind"))
        );
    }

    #[test]
    fn nonces_are_fresh_per_seal() {
        let k = key(SubKeyLabel::Log);
        let f = seg_fields(1, [0; 32]);
        let a = Envelope::decode(&seal_envelope(&k, &f, b"x", &mut OsRng).unwrap()).unwrap();
        let b = Envelope::decode(&seal_envelope(&k, &f, b"x", &mut OsRng).unwrap()).unwrap();
        assert_ne!(a.nonce, b.nonce);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]
        #[test]
        fn prop_round_trip(
            pt in proptest::collection::vec(any::<u8>(), 0..3000),
            epoch in any::<u32>(), seq in 1u64..u64::MAX,
            dev in proptest::array::uniform16(any::<u8>()),
        ) {
            let prev = if seq == 1 { [0u8; 32] } else { [7u8; 32] };
            let f = EnvelopeFields { kind: Kind::Segment, vault_id: VID, epoch, device_id: dev, seq, prev_hash: prev };
            let k = key(SubKeyLabel::Log);
            let bytes = seal_envelope(&k, &f, &pt, &mut OsRng).unwrap();
            let o = open_envelope(&k, &bytes, &VID).unwrap();
            prop_assert_eq!(o.fields, f);
            prop_assert_eq!(o.plaintext.as_slice(), pt.as_slice());
        }

        #[test]
        fn prop_decode_and_open_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..2500)) {
            let _ = Envelope::decode(&bytes);
            let _ = open_envelope(&key(SubKeyLabel::Log), &bytes, &VID);
        }
    }
}
