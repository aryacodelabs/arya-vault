//! The vault header (docs/04 §5) and the active-header selection rule.
//!
//! A header is a canonical-CBOR map stored as `header-<epoch>-<version>-<device>.bin`:
//!
//! ```text
//! { format_version: u16, vault_id: bstr(16), header_version: u32, epoch: u32,
//!   kdf: { alg: "argon2id", version: 19, m_kib: u32, t: u32, p: u32, salt: bstr(16) },
//!   wrap_pw: { nonce: bstr(24), ct: bstr(48) }, wrap_rk: { nonce: bstr(24), ct: bstr(48) },
//!   created_at: u64 }
//! ```
//!
//! The header is **unauthenticated** until a wrap has been opened (docs/04 §3, §5), so the
//! decoder is strict and bounded and calls [`KdfParams::validate`] so hostile Argon2
//! parameters are rejected at parse time (SEC-C11). `format_version` is checked first,
//! so data written by a newer format yields [`FormatError::UnsupportedFormat`] instead of
//! a confusing field error. `device_id` is not part of the header body (doc 04 §5); it
//! exists only in the file name, which is why selection works on [`HeaderCandidate`]s.

use super::cbor::{self, Limits, Value};
use super::path::{PathInfo, header_path, parse_header_path};
use super::{FORMAT_VERSION, FormatError, MAX_HEADER_BYTES, check_version};
use crate::VaultId;
use crate::aead::NONCE_LEN;
use crate::kdf::{KDF_ALG, KDF_VERSION, KdfParams, SALT_LEN};
use crate::wrap::{WRAPPED_CT_LEN, WrappedKey};

const LIMITS: Limits = Limits::new(64, 16, 64);

/// The vault header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// Format version (see [`FORMAT_VERSION`]).
    pub format_version: u16,
    /// Vault identifier.
    pub vault_id: VaultId,
    /// Monotonically increasing header version (not part of any AAD, SEC-C12).
    pub header_version: u32,
    /// Key epoch.
    pub epoch: u32,
    /// Argon2id parameters.
    pub kdf: KdfParams,
    /// `AEAD(KEK_pw, VK)`.
    pub wrap_pw: WrappedKey,
    /// `AEAD(KEK_rk, VK)`.
    pub wrap_rk: WrappedKey,
    /// Creation time (seconds since the Unix epoch, UTC).
    pub created_at: u64,
}

fn wrapped_to_value(w: &WrappedKey) -> Result<Value, FormatError> {
    Ok(Value::map(vec![
        (Value::text("nonce"), Value::Bytes(w.nonce.to_vec())),
        (Value::text("ct"), Value::Bytes(w.ct.to_vec())),
    ])?)
}

fn kdf_to_value(k: &KdfParams) -> Result<Value, FormatError> {
    Ok(Value::map(vec![
        (Value::text("alg"), Value::text(KDF_ALG)),
        (Value::text("version"), Value::Uint(u64::from(KDF_VERSION))),
        (Value::text("m_kib"), Value::Uint(u64::from(k.m_kib))),
        (Value::text("t"), Value::Uint(u64::from(k.t))),
        (Value::text("p"), Value::Uint(u64::from(k.p))),
        (Value::text("salt"), Value::Bytes(k.salt.to_vec())),
    ])?)
}

/// Looks up the entries of a map whose keys must be exactly `names` (text), returning the
/// values in `names` order.
pub(crate) fn exact_fields<'a>(
    v: &'a Value,
    names: &[&'static str],
) -> Result<Vec<&'a Value>, FormatError> {
    let Value::Map(entries) = v else {
        return Err(FormatError::InvalidField("expected map"));
    };
    if entries.len() != names.len() {
        return Err(FormatError::InvalidField("unexpected or missing fields"));
    }
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let found = entries
            .iter()
            .find(|(k, _)| matches!(k, Value::Text(t) if t == name));
        out.push(&found.ok_or(FormatError::InvalidField(name))?.1);
    }
    Ok(out)
}

pub(crate) fn as_uint(v: &Value, field: &'static str) -> Result<u64, FormatError> {
    match v {
        Value::Uint(n) => Ok(*n),
        _ => Err(FormatError::InvalidField(field)),
    }
}

pub(crate) fn as_u32(v: &Value, field: &'static str) -> Result<u32, FormatError> {
    u32::try_from(as_uint(v, field)?).map_err(|_| FormatError::InvalidField(field))
}

pub(crate) fn as_bytes<const N: usize>(
    v: &Value,
    field: &'static str,
) -> Result<[u8; N], FormatError> {
    match v {
        Value::Bytes(b) => b
            .as_slice()
            .try_into()
            .map_err(|_| FormatError::InvalidField(field)),
        _ => Err(FormatError::InvalidField(field)),
    }
}

fn wrapped_from_value(v: &Value, field: &'static str) -> Result<WrappedKey, FormatError> {
    let f = exact_fields(v, &["nonce", "ct"]).map_err(|_| FormatError::InvalidField(field))?;
    Ok(WrappedKey {
        nonce: as_bytes::<NONCE_LEN>(f[0], field)?,
        ct: as_bytes::<WRAPPED_CT_LEN>(f[1], field)?,
    })
}

fn kdf_from_value(v: &Value) -> Result<KdfParams, FormatError> {
    let f = exact_fields(v, &["alg", "version", "m_kib", "t", "p", "salt"])
        .map_err(|_| FormatError::InvalidField("kdf"))?;
    match f[0] {
        Value::Text(a) if a == KDF_ALG => {}
        _ => return Err(FormatError::UnsupportedKdf),
    }
    if as_uint(f[1], "kdf.version")? != u64::from(KDF_VERSION) {
        return Err(FormatError::UnsupportedKdf);
    }
    let params = KdfParams {
        m_kib: as_u32(f[2], "kdf.m_kib")?,
        t: as_u32(f[3], "kdf.t")?,
        p: as_u32(f[4], "kdf.p")?,
        salt: as_bytes::<SALT_LEN>(f[5], "kdf.salt")?,
    };
    params.validate().map_err(FormatError::InvalidKdf)?;
    Ok(params)
}

impl Header {
    /// Encodes the header canonically. Fails if `format_version` is not supported or the
    /// KDF parameters are out of range (a header that could not be read back is never
    /// written).
    pub fn encode(&self) -> Result<Vec<u8>, FormatError> {
        check_version(u64::from(self.format_version))?;
        self.kdf.validate().map_err(FormatError::InvalidKdf)?;
        let v = Value::map(vec![
            (
                Value::text("format_version"),
                Value::Uint(u64::from(self.format_version)),
            ),
            (
                Value::text("vault_id"),
                Value::Bytes(self.vault_id.to_vec()),
            ),
            (
                Value::text("header_version"),
                Value::Uint(u64::from(self.header_version)),
            ),
            (Value::text("epoch"), Value::Uint(u64::from(self.epoch))),
            (Value::text("kdf"), kdf_to_value(&self.kdf)?),
            (Value::text("wrap_pw"), wrapped_to_value(&self.wrap_pw)?),
            (Value::text("wrap_rk"), wrapped_to_value(&self.wrap_rk)?),
            (Value::text("created_at"), Value::Uint(self.created_at)),
        ])?;
        let bytes = v.encode()?;
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(FormatError::TooLarge);
        }
        Ok(bytes)
    }

    /// Strictly decodes a header. Rejects oversized input before parsing.
    pub fn decode(bytes: &[u8]) -> Result<Header, FormatError> {
        if bytes.len() > MAX_HEADER_BYTES {
            return Err(FormatError::TooLarge);
        }
        let v = cbor::decode(bytes, &LIMITS)?;
        // Version first: a newer format may legitimately have a different field set.
        let Value::Map(entries) = &v else {
            return Err(FormatError::InvalidField("expected map"));
        };
        let ver = entries
            .iter()
            .find(|(k, _)| matches!(k, Value::Text(t) if t == "format_version"))
            .map(|(_, v)| v)
            .ok_or(FormatError::InvalidField("format_version"))?;
        let format_version = check_version(as_uint(ver, "format_version")?)?;

        let f = exact_fields(
            &v,
            &[
                "format_version",
                "vault_id",
                "header_version",
                "epoch",
                "kdf",
                "wrap_pw",
                "wrap_rk",
                "created_at",
            ],
        )?;
        Ok(Header {
            format_version,
            vault_id: as_bytes(f[1], "vault_id")?,
            header_version: as_u32(f[2], "header_version")?,
            epoch: as_u32(f[3], "epoch")?,
            kdf: kdf_from_value(f[4])?,
            wrap_pw: wrapped_from_value(f[5], "wrap_pw")?,
            wrap_rk: wrapped_from_value(f[6], "wrap_rk")?,
            created_at: as_uint(f[7], "created_at")?,
        })
    }

    /// The file name this header must be stored under when written by `device_id`.
    pub fn file_name(&self, device_id: &[u8; 16]) -> String {
        header_path(self.epoch, self.header_version, device_id)
    }

    /// Creates a header at [`FORMAT_VERSION`].
    pub fn new(
        vault_id: VaultId,
        header_version: u32,
        epoch: u32,
        kdf: KdfParams,
        wrap_pw: WrappedKey,
        wrap_rk: WrappedKey,
        created_at: u64,
    ) -> Header {
        Header {
            format_version: FORMAT_VERSION,
            vault_id,
            header_version,
            epoch,
            kdf,
            wrap_pw,
            wrap_rk,
            created_at,
        }
    }
}

/// A header found in the sync location together with the file name it was found under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderCandidate {
    /// The file name (`header-<epoch>-<version>-<device>.bin`).
    pub file_name: String,
    /// The decoded header.
    pub header: Header,
}

/// Why a candidate was not eligible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    /// The file name is not a valid header name.
    BadName,
    /// The epoch/version in the name differ from the header body.
    NameMismatch,
    /// The header belongs to another vault.
    WrongVault,
    /// Epoch lower than the highest this device has ever seen (rollback attempt).
    EpochRollback,
}

/// Result of [`select_active`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    /// Index into the candidate slice of the winning header, if any is eligible.
    pub active: Option<usize>,
    /// Candidates that were not eligible and why.
    pub rejected: Vec<(usize, RejectReason)>,
}

/// Picks the active header: the eligible candidate with the highest
/// `(epoch, header_version, device_id)` (docs/04 §5).
///
/// Eligible means: well-formed name that agrees with the body, same `vault_id`, and
/// `epoch >= highest_known_epoch` (headers below the highest epoch this device has ever
/// seen are rejected). A newer header must still be *adopted* only after it unwraps with the
/// user's credentials; that is the caller's job.
pub fn select_active(
    candidates: &[HeaderCandidate],
    vault_id: &VaultId,
    highest_known_epoch: u32,
) -> Selection {
    type Rank = (u32, u32, [u8; 16]);
    let mut best: Option<(usize, Rank)> = None;
    let mut rejected = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let (epoch, version, device) = match parse_header_path(&c.file_name) {
            Ok(PathInfo::Header {
                epoch,
                header_version,
                device_id,
            }) => (epoch, header_version, device_id),
            _ => {
                rejected.push((i, RejectReason::BadName));
                continue;
            }
        };
        if epoch != c.header.epoch || version != c.header.header_version {
            rejected.push((i, RejectReason::NameMismatch));
        } else if &c.header.vault_id != vault_id {
            rejected.push((i, RejectReason::WrongVault));
        } else if epoch < highest_known_epoch {
            rejected.push((i, RejectReason::EpochRollback));
        } else {
            let key = (epoch, version, device);
            if best.as_ref().is_none_or(|(_, b)| key > *b) {
                best = Some((i, key));
            }
        }
    }
    Selection {
        active: best.map(|(i, _)| i),
        rejected,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn wrapped(b: u8) -> WrappedKey {
        WrappedKey {
            nonce: [b; 24],
            ct: [b ^ 0x55; 48],
        }
    }

    fn sample() -> Header {
        Header::new(
            [0x11; 16],
            3,
            2,
            KdfParams {
                m_kib: 65_536,
                t: 3,
                p: 1,
                salt: [0x22; 16],
            },
            wrapped(1),
            wrapped(2),
            1_700_000_000,
        )
    }

    #[test]
    fn round_trip() {
        let h = sample();
        let bytes = h.encode().unwrap();
        assert!(bytes.len() < MAX_HEADER_BYTES);
        assert_eq!(Header::decode(&bytes).unwrap(), h);
        // Deterministic.
        assert_eq!(bytes, h.encode().unwrap());
    }

    #[test]
    fn kdf_value_matches_the_aad_encoding_from_t01() {
        // The kdf sub-map embedded in the header is byte-identical to the canonical CBOR
        // used in the wrap_pw AAD, so header and AAD cannot drift apart.
        let k = sample().kdf;
        assert_eq!(
            kdf_to_value(&k).unwrap().encode().unwrap(),
            k.canonical_cbor()
        );
    }

    // Mutation tests: change each field and prove parse rejects or yields a different
    // header (never the original), and that a changed wrap/kdf field fails unwrap (covered
    // end-to-end in `wrap` and `vault_key` tests).
    #[test]
    fn every_byte_flip_is_rejected_or_changes_the_header() {
        let h = sample();
        let bytes = h.encode().unwrap();
        for i in 0..bytes.len() {
            for bit in [0x01u8, 0x80] {
                let mut m = bytes.clone();
                m[i] ^= bit;
                if let Ok(parsed) = Header::decode(&m) {
                    assert_ne!(parsed, h, "flip at byte {i} was invisible");
                }
            }
        }
    }

    #[test]
    fn each_field_mutation_changes_the_encoding_and_decodes_to_the_mutation() {
        let h = sample();
        let muts: Vec<Header> = vec![
            Header {
                vault_id: [0x12; 16],
                ..h.clone()
            },
            Header {
                header_version: 4,
                ..h.clone()
            },
            Header {
                epoch: 3,
                ..h.clone()
            },
            Header {
                kdf: KdfParams {
                    m_kib: 65_537,
                    ..h.kdf.clone()
                },
                ..h.clone()
            },
            Header {
                kdf: KdfParams {
                    t: 4,
                    ..h.kdf.clone()
                },
                ..h.clone()
            },
            Header {
                kdf: KdfParams {
                    p: 2,
                    ..h.kdf.clone()
                },
                ..h.clone()
            },
            Header {
                kdf: KdfParams {
                    salt: [0x23; 16],
                    ..h.kdf.clone()
                },
                ..h.clone()
            },
            Header {
                wrap_pw: wrapped(9),
                ..h.clone()
            },
            Header {
                wrap_rk: wrapped(9),
                ..h.clone()
            },
            Header {
                created_at: 1,
                ..h.clone()
            },
        ];
        for m in muts {
            let bytes = m.encode().unwrap();
            assert_ne!(bytes, h.encode().unwrap());
            assert_eq!(Header::decode(&bytes).unwrap(), m);
        }
    }

    // SEC-C11 at parse time.
    #[test]
    fn sec_c11_out_of_range_kdf_is_rejected_at_decode() {
        for kdf in [
            KdfParams {
                m_kib: 1,
                t: 3,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 65_535,
                t: 3,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 1_048_577,
                t: 3,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: u32::MAX,
                t: 3,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 65_536,
                t: 2,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 65_536,
                t: 11,
                p: 1,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 65_536,
                t: 3,
                p: 0,
                salt: [0; 16],
            },
            KdfParams {
                m_kib: 65_536,
                t: 3,
                p: 9,
                salt: [0; 16],
            },
        ] {
            // `encode` refuses to write it; hand-build the bytes to prove `decode` refuses.
            let h = Header {
                kdf: kdf.clone(),
                ..sample()
            };
            assert!(matches!(h.encode(), Err(FormatError::InvalidKdf(_))));
            let v = Value::map(vec![
                (Value::text("format_version"), Value::Uint(1)),
                (Value::text("vault_id"), Value::Bytes(vec![0x11; 16])),
                (Value::text("header_version"), Value::Uint(3)),
                (Value::text("epoch"), Value::Uint(2)),
                (Value::text("kdf"), kdf_to_value(&kdf).unwrap()),
                (
                    Value::text("wrap_pw"),
                    wrapped_to_value(&wrapped(1)).unwrap(),
                ),
                (
                    Value::text("wrap_rk"),
                    wrapped_to_value(&wrapped(2)).unwrap(),
                ),
                (Value::text("created_at"), Value::Uint(1)),
            ])
            .unwrap();
            assert!(
                matches!(
                    Header::decode(&v.encode().unwrap()),
                    Err(FormatError::InvalidKdf(_))
                ),
                "{kdf:?}"
            );
        }
    }

    fn with_field(name: &str, new: Option<Value>) -> Vec<u8> {
        let h = sample();
        let Value::Map(mut entries) = Value::map(vec![
            (Value::text("format_version"), Value::Uint(1)),
            (Value::text("vault_id"), Value::Bytes(h.vault_id.to_vec())),
            (Value::text("header_version"), Value::Uint(3)),
            (Value::text("epoch"), Value::Uint(2)),
            (Value::text("kdf"), kdf_to_value(&h.kdf).unwrap()),
            (
                Value::text("wrap_pw"),
                wrapped_to_value(&h.wrap_pw).unwrap(),
            ),
            (
                Value::text("wrap_rk"),
                wrapped_to_value(&h.wrap_rk).unwrap(),
            ),
            (Value::text("created_at"), Value::Uint(1)),
        ])
        .unwrap() else {
            unreachable!()
        };
        entries.retain(|(k, _)| !matches!(k, Value::Text(t) if t == name));
        if let Some(v) = new {
            entries.push((Value::text(name), v));
        }
        Value::map(entries).unwrap().encode().unwrap()
    }

    #[test]
    fn missing_extra_and_mistyped_fields_are_rejected() {
        for f in [
            "vault_id",
            "header_version",
            "epoch",
            "kdf",
            "wrap_pw",
            "wrap_rk",
            "created_at",
        ] {
            assert!(Header::decode(&with_field(f, None)).is_err(), "missing {f}");
        }
        assert!(Header::decode(&with_field("extra", Some(Value::Uint(1)))).is_err());
        assert!(Header::decode(&with_field("epoch", Some(Value::text("2")))).is_err());
        assert!(
            Header::decode(&with_field(
                "epoch",
                Some(Value::Uint(u64::from(u32::MAX) + 1))
            ))
            .is_err()
        );
        assert!(Header::decode(&with_field("vault_id", Some(Value::Bytes(vec![0; 15])))).is_err());
        assert!(Header::decode(&with_field("vault_id", Some(Value::Bytes(vec![0; 17])))).is_err());
        assert!(Header::decode(&with_field("created_at", Some(Value::Nint(0)))).is_err());
        // Unknown KDF algorithm.
        let mut k = kdf_to_value(&sample().kdf).unwrap();
        if let Value::Map(e) = &mut k {
            for (key, val) in e.iter_mut() {
                if matches!(key, Value::Text(t) if t == "alg") {
                    *val = Value::text("scrypt");
                }
            }
        }
        assert_eq!(
            Header::decode(&with_field("kdf", Some(k))).err(),
            Some(FormatError::UnsupportedKdf)
        );
    }

    #[test]
    fn unsupported_format_version_is_distinct_and_checked_first() {
        for v in [0u64, 2, 3, 0xffff, 0x1_0000, u64::MAX] {
            // A future header may have a totally different body; only the version matters.
            let bytes = Value::map(vec![
                (Value::text("format_version"), Value::Uint(v)),
                (Value::text("future"), Value::Null),
            ])
            .unwrap()
            .encode()
            .unwrap();
            match Header::decode(&bytes) {
                Err(FormatError::UnsupportedFormat {
                    found,
                    max_supported,
                }) => {
                    assert_eq!(max_supported, 1);
                    assert_eq!(u64::from(found), v.min(0xffff));
                }
                other => panic!("v={v}: {other:?}"),
            }
        }
        let mut h = sample();
        h.format_version = 2;
        assert!(matches!(
            h.encode(),
            Err(FormatError::UnsupportedFormat { .. })
        ));
    }

    #[test]
    fn oversized_input_is_rejected_before_parsing() {
        assert_eq!(
            Header::decode(&vec![0u8; MAX_HEADER_BYTES + 1]).err(),
            Some(FormatError::TooLarge)
        );
    }

    // ---- ordering ----

    fn cand(epoch: u32, version: u32, dev: u8, vault: u8) -> HeaderCandidate {
        let mut h = sample();
        h.epoch = epoch;
        h.header_version = version;
        h.vault_id = [vault; 16];
        HeaderCandidate {
            file_name: h.file_name(&[dev; 16]),
            header: h,
        }
    }

    #[test]
    fn highest_epoch_then_version_then_device_wins() {
        let cs = vec![
            cand(1, 9, 9, 0x11),
            cand(2, 1, 1, 0x11),
            cand(2, 3, 1, 0x11),
            cand(2, 3, 5, 0x11),
            cand(2, 2, 9, 0x11),
        ];
        let s = select_active(&cs, &[0x11; 16], 0);
        assert_eq!(s.active, Some(3));
        assert!(s.rejected.is_empty());
    }

    #[test]
    fn header_race_picks_the_higher_device_id_deterministically() {
        // Two devices write version n+1 concurrently (docs/06 §11).
        let a = cand(1, 5, 0x01, 0x11);
        let b = cand(1, 5, 0x02, 0x11);
        let fwd = select_active(&[a.clone(), b.clone()], &[0x11; 16], 0);
        let rev = select_active(&[b, a], &[0x11; 16], 0);
        assert_eq!(fwd.active, Some(1));
        assert_eq!(rev.active, Some(0));
    }

    #[test]
    fn epoch_rollback_attempt_is_rejected() {
        // The device has seen epoch 5; a provider serves only epoch 3 and 4 headers.
        let cs = vec![cand(3, 10, 1, 0x11), cand(4, 2, 1, 0x11)];
        let s = select_active(&cs, &[0x11; 16], 5);
        assert_eq!(s.active, None);
        assert_eq!(
            s.rejected,
            vec![
                (0, RejectReason::EpochRollback),
                (1, RejectReason::EpochRollback)
            ]
        );
        // A header at exactly the known epoch is fine; a higher one wins.
        let cs = vec![cand(5, 1, 1, 0x11), cand(4, 99, 1, 0x11)];
        assert_eq!(select_active(&cs, &[0x11; 16], 5).active, Some(0));
        let cs = vec![cand(5, 1, 1, 0x11), cand(6, 1, 1, 0x11)];
        assert_eq!(select_active(&cs, &[0x11; 16], 5).active, Some(1));
    }

    #[test]
    fn malformed_mismatched_and_foreign_candidates_are_ignored() {
        let mut bad_name = cand(9, 9, 1, 0x11);
        bad_name.file_name = "header-9-9-x.bin".into();
        let mut mismatch = cand(9, 9, 1, 0x11);
        mismatch.file_name = header_path(9, 8, &[1; 16]);
        let foreign = cand(9, 9, 1, 0x99);
        let good = cand(1, 1, 1, 0x11);
        let s = select_active(&[bad_name, mismatch, foreign, good], &[0x11; 16], 0);
        assert_eq!(s.active, Some(3));
        assert_eq!(
            s.rejected,
            vec![
                (0, RejectReason::BadName),
                (1, RejectReason::NameMismatch),
                (2, RejectReason::WrongVault)
            ]
        );
        assert_eq!(select_active(&[], &[0x11; 16], 0).active, None);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]
        #[test]
        fn prop_round_trip(
            vid in proptest::array::uniform16(any::<u8>()),
            hv in any::<u32>(), ep in any::<u32>(), ts in any::<u64>(),
            m in 65_536u32..=1_048_576, t in 3u32..=10, p in 1u32..=8,
            salt in proptest::array::uniform16(any::<u8>()),
            n1 in proptest::array::uniform24(any::<u8>()), n2 in proptest::array::uniform24(any::<u8>()),
        ) {
            let h = Header::new(vid, hv, ep, KdfParams { m_kib: m, t, p, salt },
                WrappedKey { nonce: n1, ct: [1; 48] }, WrappedKey { nonce: n2, ct: [2; 48] }, ts);
            prop_assert_eq!(Header::decode(&h.encode().unwrap()).unwrap(), h);
        }

        #[test]
        fn prop_decode_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..3000)) {
            let _ = Header::decode(&bytes);
        }
    }
}
