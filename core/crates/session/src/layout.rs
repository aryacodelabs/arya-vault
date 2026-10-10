//! The on-disk layout of a vault directory and header selection/publication.
//!
//! ```text
//! <dir>/header-<epoch>-<version>-<device>.bin   docs/04 §5 header (the sync location layout)
//! <dir>/vault.db                                SQLCipher database keyed by K_db (docs/04 §2)
//! <dir>/onboarding.pending                      see `meta` (present only while unconfirmed)
//! ```
//!
//! Everything cryptographic is a call into `arya-vault-crypto`; this module only decides which
//! files are read, which header wins, and how a new header replaces the old one.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use arya_vault_crypto::VaultId;
use arya_vault_crypto::format::FormatError;
use arya_vault_crypto::format::header::{Header, HeaderCandidate, select_active};
use arya_vault_crypto::format::path::{PathInfo, parse_header_path};
use arya_vault_crypto::vault_key::HeaderWraps;

use crate::error::{Result, SessionError};

/// Name of the database inside a vault directory.
pub const DB_FILE: &str = "vault.db";
/// First key epoch of a new vault.
pub const INITIAL_EPOCH: u32 = 1;
/// Suffix of the re-keyed copy of the database that a key rotation builds next to `vault.db`.
pub const NEXT_DB_SUFFIX: &str = ".next";
/// Header files larger than this are not even read (a real header is ~250 bytes; the format
/// limit is 2048).
pub const MAX_HEADER_FILE_BYTES: u64 = 4096;

/// What this process has already learned about the vault in a directory, so a later read
/// cannot silently go backwards (docs/04 §5: headers below the highest epoch ever seen are
/// rejected, and a directory with a second vault's header does not change which vault we mean).
#[derive(Debug, Default, Clone)]
pub(crate) struct Seen {
    pub(crate) vault_id: Option<VaultId>,
    pub(crate) highest_epoch: u32,
}

/// The active header together with where it came from.
#[derive(Debug, Clone)]
pub(crate) struct ActiveHeader {
    pub(crate) header: Header,
    pub(crate) device_id: [u8; 16],
}

pub(crate) fn wraps_of(h: &Header) -> HeaderWraps {
    HeaderWraps {
        vault_id: h.vault_id,
        epoch: h.epoch,
        kdf: h.kdf.clone(),
        wrap_pw: h.wrap_pw.clone(),
        wrap_rk: h.wrap_rk.clone(),
    }
}

/// A vault directory.
#[derive(Debug, Clone)]
pub(crate) struct VaultDir {
    root: PathBuf,
}

/// The epoch of a header file name, if it is one (used by tests).
#[cfg(test)]
pub(crate) fn parse_name(name: &str) -> Option<u32> {
    match parse_header_path(name) {
        Ok(PathInfo::Header { epoch, .. }) => Some(epoch),
        _ => None,
    }
}

fn is_header_name(name: &str) -> bool {
    name.starts_with("header-") && name.ends_with(".bin")
}

impl VaultDir {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn db_path(&self) -> PathBuf {
        self.root.join(DB_FILE)
    }

    pub(crate) fn has_db(&self) -> bool {
        self.db_path().exists()
    }

    /// The re-keyed copy built by a rotation (see `rotation`).
    pub(crate) fn next_db_path(&self) -> PathBuf {
        let mut p = self.db_path().into_os_string();
        p.push(NEXT_DB_SUFFIX);
        PathBuf::from(p)
    }

    /// `vault.db`, `vault.db-wal`, `vault.db-shm` or the same for `vault.db.next`.
    fn db_family(base: &Path) -> [PathBuf; 3] {
        let with = |suffix: &str| {
            let mut s = base.as_os_str().to_owned();
            s.push(suffix);
            PathBuf::from(s)
        };
        [base.to_path_buf(), with("-wal"), with("-shm")]
    }

    pub(crate) fn next_exists(&self) -> bool {
        self.next_db_path().exists()
    }

    /// Removes the re-keyed copy and its SQLite side files (best effort).
    pub(crate) fn remove_next(&self) {
        for f in Self::db_family(&self.next_db_path()) {
            let _ = fs::remove_file(f);
        }
    }

    /// Moves the verified re-keyed copy into place (atomic replace). The stale write-ahead log and
    /// shared-memory file of the old database are removed first: applied to the new file they
    /// would corrupt it.
    pub(crate) fn swap_next_into_place(&self) -> Result<()> {
        let db = self.db_path();
        let [_, wal, shm] = Self::db_family(&db);
        let _ = fs::remove_file(wal);
        let _ = fs::remove_file(shm);
        fs::rename(self.next_db_path(), &db)?;
        let [_, nwal, nshm] = Self::db_family(&self.next_db_path());
        let _ = fs::remove_file(nwal);
        let _ = fs::remove_file(nshm);
        self.sync_dir();
        Ok(())
    }

    /// Flushes directory entries (renames) to disk where the platform allows it; best effort.
    pub(crate) fn sync_dir(&self) {
        #[cfg(unix)]
        if let Ok(d) = File::open(&self.root) {
            let _ = d.sync_all();
        }
    }

    /// After a database has opened under `active`: removes what an interrupted or finished key
    /// rotation leaves behind. That is the re-keyed copy, every pre-migration backup (encrypted
    /// under the retired database key) and every header of a lower epoch (they are brute-force
    /// targets for the old key, docs/04 §5, §9). Best effort.
    ///
    /// The headers go **last**: a lower-epoch header is the marker `has_rotation_leftovers` looks
    /// for, so a crash before the backups are gone leaves the marker and the next open finishes.
    pub(crate) fn collect_rotation_leftovers(&self, active: &ActiveHeader) {
        self.remove_next();
        let Ok(names) = self.header_files() else {
            return;
        };
        let older: Vec<&String> = names
            .iter()
            .filter(|n| {
                matches!(
                    parse_header_path(n),
                    Ok(PathInfo::Header { epoch, .. }) if epoch < active.header.epoch
                )
            })
            .collect();
        // Backups are only retired by a rotation that committed (an older header proves it): an
        // abandoned attempt leaves the old key in force and its backups still useful.
        if !older.is_empty() {
            arya_vault_storage::remove_all_backups(&self.db_path());
        }
        for name in older {
            let _ = fs::remove_file(self.root.join(name));
        }
    }

    /// Whether anything of a rotation is lying around (cheap check before collecting).
    pub(crate) fn has_rotation_leftovers(&self, active: &ActiveHeader) -> bool {
        if self.next_exists() {
            return true;
        }
        self.header_files().is_ok_and(|names| {
            names.iter().any(|n| {
                matches!(
                    parse_header_path(n),
                    Ok(PathInfo::Header { epoch, .. }) if epoch < active.header.epoch
                )
            })
        })
    }

    /// Creates the directory (owner-only on Unix). Idempotent.
    pub(crate) fn ensure_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(())
    }

    /// Sorted names of the files that look like headers. A missing directory has none.
    pub(crate) fn header_files(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let rd = match fs::read_dir(&self.root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(e.into()),
        };
        for entry in rd {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str()
                && is_header_name(name)
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Whether a vault (any header or a database) is present.
    pub(crate) fn has_vault(&self) -> Result<bool> {
        Ok(self.has_db() || !self.header_files()?.is_empty())
    }

    /// Reads every well-formed header and returns the active one (docs/04 §5).
    ///
    /// Oversized files are skipped. A header of a newer `format_version` aborts with
    /// `UnsupportedFormat` rather than being skipped, so an old build never silently falls
    /// back to an older header ("update required", docs/04 §14).
    pub(crate) fn active_header(&self, seen: &mut Seen) -> Result<ActiveHeader> {
        let names = self.header_files()?;
        if names.is_empty() {
            return Err(if self.has_db() {
                SessionError::CorruptVault("the vault header is missing")
            } else {
                SessionError::NoVault
            });
        }
        let mut candidates = Vec::new();
        for name in names {
            let mut buf = Vec::new();
            File::open(self.root.join(&name))?
                .take(MAX_HEADER_FILE_BYTES + 1)
                .read_to_end(&mut buf)?;
            if buf.len() as u64 > MAX_HEADER_FILE_BYTES {
                continue;
            }
            match Header::decode(&buf) {
                Ok(header) => candidates.push(HeaderCandidate {
                    file_name: name,
                    header,
                }),
                Err(e @ FormatError::UnsupportedFormat { .. }) => return Err(e.into()),
                Err(_) => {}
            }
        }
        let Some(first) = candidates.first() else {
            return Err(SessionError::CorruptVault(
                "no readable vault header found in the vault directory",
            ));
        };
        let vault_id = seen.vault_id.unwrap_or(first.header.vault_id);
        let selection = select_active(&candidates, &vault_id, seen.highest_epoch);
        let idx = selection.active.ok_or(SessionError::CorruptVault(
            "no eligible vault header found in the vault directory",
        ))?;
        let c = &candidates[idx];
        let device_id = match parse_header_path(&c.file_name) {
            Ok(PathInfo::Header { device_id, .. }) => device_id,
            _ => return Err(SessionError::CorruptVault("bad header file name")),
        };
        seen.vault_id = Some(c.header.vault_id);
        seen.highest_epoch = seen.highest_epoch.max(c.header.epoch);
        Ok(ActiveHeader {
            header: c.header.clone(),
            device_id,
        })
    }

    /// Writes a header atomically (temp file + rename), owner-only on Unix.
    pub(crate) fn write_header(&self, header: &Header, device_id: &[u8; 16]) -> Result<String> {
        let name = header.file_name(device_id);
        let bytes = header
            .encode()
            .map_err(|_| SessionError::Internal("header could not be encoded"))?;
        let tmp = self.root.join(format!("{name}.tmp"));
        let mut opts = OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, self.root.join(&name))?;
        self.sync_dir();
        Ok(name)
    }

    /// Writes `header` and then removes every other header file, so superseded headers (which
    /// remain brute-force targets, docs/04 §5) do not linger. Stale `*.bin.tmp` leftovers of a
    /// crashed write are removed too.
    pub(crate) fn publish_header(&self, header: &Header, device_id: &[u8; 16]) -> Result<()> {
        let name = self.write_header(header, device_id)?;
        for other in self.header_files()? {
            if other != name {
                let _ = fs::remove_file(self.root.join(other));
            }
        }
        if let Ok(rd) = fs::read_dir(&self.root) {
            for entry in rd.flatten() {
                if let Some(n) = entry.file_name().to_str()
                    && n.starts_with("header-")
                    && n.ends_with(".bin.tmp")
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use arya_vault_crypto::kdf::KdfParams;
    use arya_vault_crypto::wrap::WrappedKey;

    use super::*;

    fn wrapped(b: u8) -> WrappedKey {
        WrappedKey {
            nonce: [b; 24],
            ct: [b ^ 0x55; 48],
        }
    }

    fn header(vault: u8, epoch: u32, version: u32) -> Header {
        Header::new(
            [vault; 16],
            version,
            epoch,
            KdfParams::floor([7; 16]),
            wrapped(1),
            wrapped(2),
            1_700_000_000,
        )
    }

    fn put(dir: &VaultDir, h: &Header, device: u8) {
        dir.ensure_dir().unwrap();
        dir.write_header(h, &[device; 16]).unwrap();
    }

    fn setup() -> (tempfile::TempDir, VaultDir) {
        let t = tempfile::tempdir().unwrap();
        let d = VaultDir::new(t.path().join("v"));
        (t, d)
    }

    #[test]
    fn empty_and_missing_directories_have_no_vault() {
        let (t, d) = setup();
        assert!(!d.has_vault().unwrap());
        assert!(matches!(
            d.active_header(&mut Seen::default()),
            Err(SessionError::NoVault)
        ));
        fs::create_dir_all(t.path().join("v")).unwrap();
        assert!(matches!(
            d.active_header(&mut Seen::default()),
            Err(SessionError::NoVault)
        ));
    }

    #[test]
    fn highest_epoch_then_version_wins() {
        let (_t, d) = setup();
        put(&d, &header(9, 1, 5), 1);
        put(&d, &header(9, 2, 1), 1);
        put(&d, &header(9, 2, 3), 1);
        let a = d.active_header(&mut Seen::default()).unwrap();
        assert_eq!((a.header.epoch, a.header.header_version), (2, 3));
    }

    #[test]
    fn lower_epoch_than_ever_seen_is_rejected() {
        // A rollback: the provider re-serves only the old-epoch header.
        let (_t, d) = setup();
        put(&d, &header(9, 1, 4), 1);
        let mut seen = Seen {
            vault_id: Some([9; 16]),
            highest_epoch: 2,
        };
        let e = d.active_header(&mut seen).unwrap_err();
        assert!(matches!(e, SessionError::CorruptVault(m) if m.contains("eligible")));
        // ... and the floor was not lowered by the failed read
        assert_eq!(seen.highest_epoch, 2);
    }

    #[test]
    fn a_newer_epoch_raises_the_floor_so_the_old_header_cannot_come_back() {
        let (_t, d) = setup();
        put(&d, &header(9, 1, 4), 1);
        put(&d, &header(9, 2, 1), 1);
        let mut seen = Seen::default();
        assert_eq!(d.active_header(&mut seen).unwrap().header.epoch, 2);
        assert_eq!(seen.highest_epoch, 2);
        // the newest header disappears; only the epoch-1 one is left
        fs::remove_file(d.root().join(header(9, 2, 1).file_name(&[1; 16]))).unwrap();
        assert!(d.active_header(&mut seen).is_err());
    }

    #[test]
    fn headers_of_another_vault_are_ignored_once_the_vault_is_known() {
        let (_t, d) = setup();
        put(&d, &header(1, 1, 1), 1);
        put(&d, &header(2, 1, 9), 1); // a stranger's header with a higher version
        let mut seen = Seen {
            vault_id: Some([1; 16]),
            highest_epoch: 0,
        };
        assert_eq!(d.active_header(&mut seen).unwrap().header.vault_id, [1; 16]);
    }

    #[test]
    fn file_name_must_agree_with_the_body() {
        let (_t, d) = setup();
        d.ensure_dir().unwrap();
        let h = header(9, 1, 1);
        let liar = header(9, 1, 2).file_name(&[1; 16]);
        fs::write(d.root().join(liar), h.encode().unwrap()).unwrap();
        assert!(d.active_header(&mut Seen::default()).is_err());
    }

    #[test]
    fn oversized_garbage_truncated_and_unnamed_files_are_skipped_not_fatal() {
        let (_t, d) = setup();
        put(&d, &header(9, 1, 1), 1);
        let name = |v: u32| header(9, 1, v).file_name(&[2; 16]);
        fs::write(
            d.root().join(name(7)),
            vec![0u8; usize::try_from(MAX_HEADER_FILE_BYTES).unwrap() + 1],
        )
        .unwrap();
        fs::write(d.root().join(name(8)), b"not cbor at all").unwrap();
        let full = header(9, 1, 9).encode().unwrap();
        fs::write(d.root().join(name(9)), &full[..full.len() / 2]).unwrap();
        fs::write(d.root().join("header-nonsense.bin"), &full).unwrap();
        let a = d.active_header(&mut Seen::default()).unwrap();
        assert_eq!(a.header.header_version, 1);
    }

    #[test]
    fn only_unreadable_headers_is_corrupt_and_a_missing_header_with_a_db_is_corrupt() {
        let (_t, d) = setup();
        d.ensure_dir().unwrap();
        fs::write(d.root().join(header(9, 1, 1).file_name(&[1; 16])), b"junk").unwrap();
        assert!(matches!(
            d.active_header(&mut Seen::default()),
            Err(SessionError::CorruptVault(_))
        ));
        let (_t2, d2) = setup();
        d2.ensure_dir().unwrap();
        fs::write(d2.db_path(), b"x").unwrap();
        assert!(matches!(
            d2.active_header(&mut Seen::default()),
            Err(SessionError::CorruptVault(m)) if m.contains("missing")
        ));
    }

    /// The v1 header with its `format_version` value patched to `v` (`encode` refuses to write
    /// anything but the current version, so the bytes are edited in place).
    fn with_format_version(h: &Header, v: u8) -> Vec<u8> {
        let mut bytes = h.encode().unwrap();
        let key = b"format_version";
        let at = bytes.windows(key.len()).position(|w| w == key).unwrap() + key.len();
        assert_eq!(bytes[at], 1, "small unsigned integer follows the key");
        bytes[at] = v;
        bytes
    }

    #[test]
    fn a_newer_format_version_is_reported_not_skipped() {
        let (_t, d) = setup();
        put(&d, &header(9, 1, 1), 1);
        let newer = header(9, 1, 2);
        d.ensure_dir().unwrap();
        fs::write(
            d.root().join(newer.file_name(&[1; 16])),
            with_format_version(&newer, 2),
        )
        .unwrap();
        let e = d.active_header(&mut Seen::default()).unwrap_err();
        assert!(
            matches!(e, SessionError::UnsupportedFormat { found: 2, .. }),
            "{e:?}"
        );
    }

    #[test]
    fn publish_replaces_older_headers_and_stale_temp_files() {
        let (_t, d) = setup();
        put(&d, &header(9, 1, 1), 1);
        fs::write(d.root().join("header-1-1-x.bin.tmp"), b"half").unwrap();
        d.publish_header(&header(9, 1, 2), &[1; 16]).unwrap();
        let names = d.header_files().unwrap();
        assert_eq!(names, [header(9, 1, 2).file_name(&[1; 16])]);
        assert!(!d.root().join("header-1-1-x.bin.tmp").exists());
    }
}
