//! The plaintext policy record and the sealed-blob file, next to the header.
//!
//! `quick-unlock.policy` (fixed layout, big-endian, at most 76 bytes) holds **no secret**:
//!
//! ```text
//! offset size  field
//!      0    4  magic "AVQP"
//!      4    1  version (= 1)
//!      5    1  kind (1 windowsHello, 2 touchId, 3 faceId, 4 biometric, 5 osKeyring)
//!      6    1  flags (bit 0: boot_id present; all other bits 0)
//!      7    1  boot_id length (0..=32; 0 iff bit 0 clear)
//!      8    8  enabled_at           (ms since the Unix epoch)
//!     16    8  last_password_unlock (ms)  the 72 h clock keys off this
//!     24    8  last_seen            (ms)  newest wall-clock reading; detects a clock set back
//!     32    8  last_quick_unlock    (ms, 0 = never)
//!     40    4  failure_count        (consecutive failed attempts)
//!     44    n  boot_id
//! ```
//!
//! `quick-unlock.blob` is the provider's opaque output, at most [`MAX_BLOB_BYTES`]. Both files
//! are written atomically (temp file + rename, owner-only on Unix). The parser is bounded,
//! returns typed errors and never panics (CLAUDE.md rule 7; fuzz target `fuzz_quick_policy`).

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use thiserror::Error;
use zeroize::Zeroizing;

use super::{Blob, QuickUnlockKind};

/// File name of the policy record.
pub const POLICY_FILE: &str = "quick-unlock.policy";
/// File name of the sealed blob.
pub const BLOB_FILE: &str = "quick-unlock.blob";
/// Largest accepted blob (providers seal 32 bytes; real blobs are a few hundred).
pub const MAX_BLOB_BYTES: usize = 4096;
/// Longest accepted boot id.
pub const MAX_BOOT_ID_BYTES: usize = 32;
/// Largest policy file that is even read.
pub const MAX_POLICY_BYTES: usize = 44 + MAX_BOOT_ID_BYTES;

const MAGIC: &[u8; 4] = b"AVQP";
const VERSION: u8 = 1;
const FIXED_LEN: usize = 44;

/// Why a policy record was rejected. Carries no content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PolicyError {
    /// Fewer bytes than the layout needs.
    #[error("policy record is truncated")]
    Truncated,
    /// More bytes than the layout allows.
    #[error("policy record is too large")]
    TooLarge,
    /// Wrong magic.
    #[error("policy record has the wrong magic")]
    BadMagic,
    /// A version this build does not read.
    #[error("policy record version {0} is not supported")]
    UnsupportedVersion(u8),
    /// An unknown or `none` kind.
    #[error("policy record has an unknown kind")]
    BadKind,
    /// Reserved flag bits set, or the boot-id flag and length disagree.
    #[error("policy record has invalid flags")]
    BadFlags,
}

/// The decoded policy record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRecord {
    /// Provider kind that sealed the blob.
    pub kind: QuickUnlockKind,
    /// When quick unlock was enabled (ms).
    pub enabled_at_ms: u64,
    /// Time of the last unlock by master password or recovery (ms). The age limit keys off this.
    pub last_password_unlock_ms: u64,
    /// Newest wall-clock reading written (ms).
    pub last_seen_ms: u64,
    /// Time of the last successful quick unlock (ms; 0 = never).
    pub last_quick_unlock_ms: u64,
    /// Consecutive failed attempts, persisted *before* each prompt.
    pub failure_count: u32,
    /// Boot identifier at the last password unlock.
    pub boot_id: Option<Vec<u8>>,
}

fn kind_to_byte(k: QuickUnlockKind) -> u8 {
    match k {
        QuickUnlockKind::None => 0,
        QuickUnlockKind::WindowsHello => 1,
        QuickUnlockKind::TouchId => 2,
        QuickUnlockKind::FaceId => 3,
        QuickUnlockKind::Biometric => 4,
        QuickUnlockKind::OsKeyring => 5,
    }
}

fn kind_from_byte(b: u8) -> Option<QuickUnlockKind> {
    Some(match b {
        1 => QuickUnlockKind::WindowsHello,
        2 => QuickUnlockKind::TouchId,
        3 => QuickUnlockKind::FaceId,
        4 => QuickUnlockKind::Biometric,
        5 => QuickUnlockKind::OsKeyring,
        _ => return None,
    })
}

fn be_u64(b: &[u8]) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[..8]);
    u64::from_be_bytes(a)
}

impl PolicyRecord {
    /// Serialises the record. A boot id longer than [`MAX_BOOT_ID_BYTES`] is truncated (the
    /// session never produces one: providers are required to return short ids).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let boot: &[u8] = self.boot_id.as_deref().unwrap_or(&[]);
        let boot = &boot[..boot.len().min(MAX_BOOT_ID_BYTES)];
        let has_boot = self.boot_id.is_some();
        let mut out = Vec::with_capacity(FIXED_LEN + boot.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(kind_to_byte(self.kind));
        out.push(u8::from(has_boot));
        out.push(u8::try_from(boot.len()).unwrap_or(0));
        out.extend_from_slice(&self.enabled_at_ms.to_be_bytes());
        out.extend_from_slice(&self.last_password_unlock_ms.to_be_bytes());
        out.extend_from_slice(&self.last_seen_ms.to_be_bytes());
        out.extend_from_slice(&self.last_quick_unlock_ms.to_be_bytes());
        out.extend_from_slice(&self.failure_count.to_be_bytes());
        out.extend_from_slice(boot);
        out
    }

    /// Parses a record. Strict: exact length, known version, known kind, no reserved bits.
    ///
    /// # Errors
    /// [`PolicyError`].
    pub fn decode(bytes: &[u8]) -> Result<Self, PolicyError> {
        if bytes.len() > MAX_POLICY_BYTES {
            return Err(PolicyError::TooLarge);
        }
        if bytes.len() < FIXED_LEN {
            return Err(PolicyError::Truncated);
        }
        if &bytes[..4] != MAGIC {
            return Err(PolicyError::BadMagic);
        }
        if bytes[4] != VERSION {
            return Err(PolicyError::UnsupportedVersion(bytes[4]));
        }
        let kind = kind_from_byte(bytes[5]).ok_or(PolicyError::BadKind)?;
        let flags = bytes[6];
        let boot_len = usize::from(bytes[7]);
        let has_boot = match flags {
            0 => false,
            1 => true,
            _ => return Err(PolicyError::BadFlags),
        };
        if boot_len > MAX_BOOT_ID_BYTES || has_boot == (boot_len == 0) {
            return Err(PolicyError::BadFlags);
        }
        if bytes.len() != FIXED_LEN + boot_len {
            return Err(if bytes.len() < FIXED_LEN + boot_len {
                PolicyError::Truncated
            } else {
                PolicyError::TooLarge
            });
        }
        Ok(Self {
            kind,
            enabled_at_ms: be_u64(&bytes[8..]),
            last_password_unlock_ms: be_u64(&bytes[16..]),
            last_seen_ms: be_u64(&bytes[24..]),
            last_quick_unlock_ms: be_u64(&bytes[32..]),
            failure_count: u32::from_be_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
            boot_id: has_boot.then(|| bytes[FIXED_LEN..].to_vec()),
        })
    }
}

/// Reading the policy record failed.
#[derive(Debug)]
pub(crate) enum ReadError {
    /// The file exists but is not a valid record.
    Invalid,
    /// Filesystem error.
    Io(std::io::Error),
}

/// Reading the blob failed.
#[derive(Debug)]
pub(crate) enum BlobReadError {
    /// Missing, empty or larger than the limit.
    Rejected,
    /// Filesystem error.
    Io(std::io::Error),
}

/// The two files in one vault directory.
pub(crate) struct QuickStore {
    root: PathBuf,
}

fn read_bounded(path: &Path, max: usize) -> std::io::Result<Option<Vec<u8>>> {
    let f = match File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut buf = Vec::new();
    f.take(max as u64 + 1).read_to_end(&mut buf)?;
    Ok(Some(buf))
}

fn write_atomic(root: &Path, name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = root.join(format!("{name}.tmp"));
    let mut opts = OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut f = opts.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, root.join(name))
}

fn remove_if_present(path: &Path) -> std::io::Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

impl QuickStore {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
        }
    }

    /// `Ok(None)`: no record. `Err(Invalid)`: a file is there but unusable.
    pub(crate) fn read_policy(&self) -> Result<Option<PolicyRecord>, ReadError> {
        let bytes =
            read_bounded(&self.root.join(POLICY_FILE), MAX_POLICY_BYTES).map_err(ReadError::Io)?;
        match bytes {
            None => Ok(None),
            Some(b) => PolicyRecord::decode(&b)
                .map(Some)
                .map_err(|_| ReadError::Invalid),
        }
    }

    pub(crate) fn write_policy(&self, rec: &PolicyRecord) -> std::io::Result<()> {
        write_atomic(&self.root, POLICY_FILE, &rec.encode())
    }

    pub(crate) fn read_blob(&self) -> Result<Blob, BlobReadError> {
        let bytes = read_bounded(&self.root.join(BLOB_FILE), MAX_BLOB_BYTES)
            .map_err(BlobReadError::Io)?
            .map(Zeroizing::new);
        match bytes {
            Some(b) if !b.is_empty() && b.len() <= MAX_BLOB_BYTES => {
                Blob::new(b.to_vec()).map_err(|_| BlobReadError::Rejected)
            }
            _ => Err(BlobReadError::Rejected),
        }
    }

    pub(crate) fn write_blob(&self, blob: &Blob) -> std::io::Result<()> {
        write_atomic(&self.root, BLOB_FILE, blob.as_bytes())
    }

    /// Deletes the policy record first (a crash then leaves "disabled"), then the blob, then
    /// leftover temp files.
    pub(crate) fn remove_all(&self) -> std::io::Result<()> {
        let policy = remove_if_present(&self.root.join(POLICY_FILE));
        let blob = remove_if_present(&self.root.join(BLOB_FILE));
        let _ = remove_if_present(&self.root.join(format!("{POLICY_FILE}.tmp")));
        let _ = remove_if_present(&self.root.join(format!("{BLOB_FILE}.tmp")));
        policy.and(blob)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(boot: Option<Vec<u8>>) -> PolicyRecord {
        PolicyRecord {
            kind: QuickUnlockKind::WindowsHello,
            enabled_at_ms: 1_700_000_000_000,
            last_password_unlock_ms: 1_700_000_100_000,
            last_seen_ms: 1_700_000_200_000,
            last_quick_unlock_ms: 0,
            failure_count: 3,
            boot_id: boot,
        }
    }

    #[test]
    fn round_trips_with_and_without_a_boot_id() {
        for r in [
            rec(None),
            rec(Some(vec![7; 16])),
            rec(Some(vec![1; MAX_BOOT_ID_BYTES])),
        ] {
            assert_eq!(PolicyRecord::decode(&r.encode()).unwrap(), r);
        }
        assert_eq!(rec(None).encode().len(), 44);
        assert!(rec(Some(vec![1; 32])).encode().len() <= MAX_POLICY_BYTES);
    }

    #[test]
    fn every_kind_round_trips_and_none_is_refused() {
        for k in [
            QuickUnlockKind::WindowsHello,
            QuickUnlockKind::TouchId,
            QuickUnlockKind::FaceId,
            QuickUnlockKind::Biometric,
            QuickUnlockKind::OsKeyring,
        ] {
            let mut r = rec(None);
            r.kind = k;
            assert_eq!(PolicyRecord::decode(&r.encode()).unwrap().kind, k);
        }
        let mut b = rec(None).encode();
        b[5] = 0;
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadKind));
        b[5] = 6;
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadKind));
    }

    #[test]
    fn rejects_every_truncation_and_every_extension() {
        let full = rec(Some(vec![9; 16])).encode();
        for n in 0..full.len() {
            assert!(PolicyRecord::decode(&full[..n]).is_err(), "prefix {n}");
        }
        let mut longer = full.clone();
        longer.push(0);
        assert_eq!(PolicyRecord::decode(&longer), Err(PolicyError::TooLarge));
        assert_eq!(
            PolicyRecord::decode(&[0u8; MAX_POLICY_BYTES + 1]),
            Err(PolicyError::TooLarge)
        );
    }

    #[test]
    fn rejects_bad_magic_version_flags_and_inconsistent_boot_ids() {
        let ok = rec(Some(vec![9; 16])).encode();
        let mut b = ok.clone();
        b[0] ^= 1;
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadMagic));
        let mut b = ok.clone();
        b[4] = 2;
        assert_eq!(
            PolicyRecord::decode(&b),
            Err(PolicyError::UnsupportedVersion(2))
        );
        let mut b = ok.clone();
        b[6] = 2;
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadFlags));
        let mut b = ok.clone();
        b[6] = 0; // length says 16, flag says none
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadFlags));
        let mut b = rec(None).encode();
        b[6] = 1; // flag says present, length 0
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadFlags));
        let mut b = ok;
        b[7] = 33;
        assert_eq!(PolicyRecord::decode(&b), Err(PolicyError::BadFlags));
    }

    #[test]
    fn no_single_byte_change_panics() {
        let full = rec(Some(vec![5; 16])).encode();
        for i in 0..full.len() {
            for v in [0u8, 1, 0x7f, 0x80, 0xff] {
                let mut b = full.clone();
                b[i] = v;
                let _ = PolicyRecord::decode(&b);
            }
        }
    }

    #[test]
    fn store_round_trip_bounds_and_removal() {
        let d = tempfile::tempdir().unwrap();
        let s = QuickStore::new(d.path());
        assert!(s.read_policy().unwrap().is_none());
        assert!(matches!(s.read_blob(), Err(BlobReadError::Rejected)));
        s.write_policy(&rec(None)).unwrap();
        s.write_blob(&Blob::new(vec![1, 2, 3]).unwrap()).unwrap();
        assert_eq!(s.read_policy().unwrap().unwrap(), rec(None));
        assert_eq!(s.read_blob().unwrap().as_bytes(), [1, 2, 3]);
        // oversized, empty and garbage files are rejected without being fully read
        fs::write(d.path().join(BLOB_FILE), vec![0u8; MAX_BLOB_BYTES + 1]).unwrap();
        assert!(matches!(s.read_blob(), Err(BlobReadError::Rejected)));
        fs::write(d.path().join(BLOB_FILE), b"").unwrap();
        assert!(matches!(s.read_blob(), Err(BlobReadError::Rejected)));
        fs::write(d.path().join(POLICY_FILE), b"garbage").unwrap();
        assert!(matches!(s.read_policy(), Err(ReadError::Invalid)));
        s.remove_all().unwrap();
        s.remove_all().unwrap();
        assert!(s.read_policy().unwrap().is_none());
        assert!(!d.path().join(BLOB_FILE).exists());
    }

    #[test]
    fn the_fuzz_seed_corpus_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fuzz/corpus/fuzz_quick_policy");
        let mut n = 0;
        for e in fs::read_dir(dir).unwrap() {
            let bytes = fs::read(e.unwrap().path()).unwrap();
            let r = PolicyRecord::decode(&bytes).unwrap();
            assert_eq!(r.encode(), bytes);
            n += 1;
        }
        assert!(n >= 2);
    }

    #[test]
    fn blob_new_enforces_the_size_limit() {
        assert!(Blob::new(vec![]).is_err());
        assert!(Blob::new(vec![0; MAX_BLOB_BYTES]).is_ok());
        assert!(Blob::new(vec![0; MAX_BLOB_BYTES + 1]).is_err());
        assert!(format!("{:?}", Blob::new(vec![0xAA; 8]).unwrap()).contains("redacted"));
    }
}
