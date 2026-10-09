//! The on-disk layout of a CLI vault directory and the key plumbing around it.
//!
//! ```text
//! <dir>/header-<epoch>-<version>-<device>.bin   docs/04 §5 header (the sync location layout)
//! <dir>/vault.db                                SQLCipher database keyed by K_db (docs/04 §2)
//! ```
//!
//! Everything cryptographic is a call into `arya-vault-crypto`; this module only decides which
//! call comes in which order and where the bytes are stored.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::PathBuf;

use arya_vault_crypto::VaultId;
use arya_vault_crypto::format::header::{Header, HeaderCandidate, select_active};
use arya_vault_crypto::format::path::{PathInfo, parse_header_path};
use arya_vault_crypto::hkdf::{self, SubKeyLabel};
use arya_vault_crypto::kdf::{self, KdfParams, M_KIB_CALIBRATION_CAP};
use arya_vault_crypto::keys::{RecoveryKey, VaultKey};
use arya_vault_crypto::rng::{OsRng, Rng};
use arya_vault_crypto::vault_key::{self, HeaderWraps};
use arya_vault_crypto::wrap;
use arya_vault_storage::{CreateParams, Db, DbKey, Store};
use arya_vault_vault::{SystemClock, Vault};
use zeroize::Zeroizing;

use crate::args::KdfProfile;
use crate::error::{CliError, Result};

/// Name of the database inside a vault directory.
pub const DB_FILE: &str = "vault.db";
/// First key epoch of a new vault.
pub const INITIAL_EPOCH: u32 = 1;
/// Header files larger than this are not even read (a real header is ~250 bytes).
const MAX_HEADER_BYTES: u64 = 4096;

/// A vault directory.
#[derive(Debug, Clone)]
pub struct VaultDir {
    root: PathBuf,
}

/// The active header together with where it came from.
#[derive(Debug, Clone)]
pub struct ActiveHeader {
    pub header: Header,
    pub device_id: [u8; 16],
}

/// An unlocked vault.
pub struct Unlocked {
    pub vault: Vault,
}

pub fn wraps_of(h: &Header) -> HeaderWraps {
    HeaderWraps {
        vault_id: h.vault_id,
        epoch: h.epoch,
        kdf: h.kdf.clone(),
        wrap_pw: h.wrap_pw.clone(),
        wrap_rk: h.wrap_rk.clone(),
    }
}

/// KDF parameters for a profile, with a fresh random salt.
pub fn kdf_for(profile: KdfProfile) -> Result<KdfParams> {
    let params = match profile {
        KdfProfile::Low => KdfParams::floor(random_array(&mut OsRng)?),
        KdfProfile::Default => kdf::calibrate(750, M_KIB_CALIBRATION_CAP, &mut OsRng)?,
        KdfProfile::High => kdf::calibrate(1500, 512 * 1024, &mut OsRng)?,
    };
    Ok(params)
}

/// `N` random bytes from the OS CSPRNG (via the crypto crate's `OsRng`).
fn random_array<const N: usize>(rng: &mut dyn Rng) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    rng.fill_bytes(&mut out)?;
    Ok(out)
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn db_key(vk: &VaultKey, vault_id: &VaultId, epoch: u32) -> Result<DbKey> {
    let sub = hkdf::subkey(vk, vault_id, SubKeyLabel::Db, epoch)?;
    Ok(DbKey::from_bytes(*sub.expose_secret()))
}

impl VaultDir {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn db_path(&self) -> PathBuf {
        self.root.join(DB_FILE)
    }

    pub fn has_db(&self) -> bool {
        self.db_path().exists()
    }

    fn header_files(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        let rd = match fs::read_dir(&self.root) {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
            Err(e) => return Err(e.into()),
        };
        for entry in rd {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str()
                && name.starts_with("header-")
                && name.ends_with(".bin")
            {
                names.push(name.to_owned());
            }
        }
        names.sort();
        Ok(names)
    }

    /// Reads every well-formed header and returns the active one (docs/04 §5).
    pub fn active_header(&self) -> Result<ActiveHeader> {
        let mut candidates = Vec::new();
        for name in self.header_files()? {
            let mut buf = Vec::new();
            File::open(self.root.join(&name))?
                .take(MAX_HEADER_BYTES + 1)
                .read_to_end(&mut buf)?;
            if buf.len() as u64 > MAX_HEADER_BYTES {
                continue;
            }
            if let Ok(header) = Header::decode(&buf) {
                candidates.push(HeaderCandidate {
                    file_name: name,
                    header,
                });
            }
        }
        let Some(first) = candidates.first() else {
            return Err(CliError::vault(
                "no readable vault header found in the vault directory",
            ));
        };
        let vault_id = first.header.vault_id;
        let selection = select_active(&candidates, &vault_id, 0);
        let idx = selection.active.ok_or_else(|| {
            CliError::vault("no eligible vault header found in the vault directory")
        })?;
        let c = &candidates[idx];
        let device_id = match parse_header_path(&c.file_name) {
            Ok(PathInfo::Header { device_id, .. }) => device_id,
            _ => return Err(CliError::vault("bad header file name")),
        };
        Ok(ActiveHeader {
            header: c.header.clone(),
            device_id,
        })
    }

    /// Writes a header atomically (temp file + rename), owner-only on Unix.
    fn write_header(&self, header: &Header, device_id: &[u8; 16]) -> Result<String> {
        let name = header.file_name(device_id);
        let bytes = header.encode()?;
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
        Ok(name)
    }

    /// Creates a new vault. Returns the recovery key (shown once by the caller).
    pub fn create(&self, password: &str, profile: KdfProfile) -> Result<RecoveryKey> {
        if self.has_db() || !self.header_files()?.is_empty() {
            return Err(CliError::failure(
                "the vault directory already contains a vault",
            ));
        }
        fs::create_dir_all(&self.root)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&self.root, fs::Permissions::from_mode(0o700))?;
        }
        let kdf = kdf_for(profile)?;
        let new = vault_key::create_vault(password, kdf, INITIAL_EPOCH, &mut OsRng)?;
        let device_id: [u8; 16] = random_array(&mut OsRng)?;
        let w = &new.wraps;
        let header = Header::new(
            w.vault_id,
            1,
            w.epoch,
            w.kdf.clone(),
            w.wrap_pw.clone(),
            w.wrap_rk.clone(),
            now_secs(),
        );
        let key = db_key(&new.vault_key, &w.vault_id, w.epoch)?;
        let header_name = self.write_header(&header, &device_id)?;
        let created = Db::create(
            &self.db_path(),
            key,
            &CreateParams {
                vault_id: w.vault_id,
                device_id,
                epoch: u64::from(w.epoch),
                header_version: 1,
            },
        );
        match created {
            Ok(db) => db.close()?,
            Err(e) => {
                let _ = fs::remove_file(self.root.join(header_name));
                return Err(e.into());
            }
        }
        Ok(new.recovery_key)
    }

    /// Verifies the password against the active header only (no database access).
    pub fn check_password(&self, password: &str) -> Result<(ActiveHeader, VaultKey)> {
        let active = self.active_header()?;
        let vk = vault_key::unlock_with_password(password, &wraps_of(&active.header))?;
        Ok((active, vk))
    }

    fn open_db(&self, vk: &VaultKey, active: &ActiveHeader) -> Result<Vault> {
        let key = db_key(vk, &active.header.vault_id, active.header.epoch)?;
        let db = Db::open(&self.db_path(), key)?;
        Ok(Vault::open(db, Box::new(SystemClock))?)
    }

    /// Unlocks with the master password and opens the database.
    pub fn unlock(&self, password: &str) -> Result<Unlocked> {
        let (active, vk) = self.check_password(password)?;
        let vault = self.open_db(&vk, &active)?;
        Ok(Unlocked { vault })
    }

    /// Records the header version in the database `meta` (kept in step with the header file).
    fn record_header_version(
        &self,
        vk: &VaultKey,
        active: &ActiveHeader,
        version: u32,
    ) -> Result<()> {
        if !self.has_db() {
            return Ok(());
        }
        let key = db_key(vk, &active.header.vault_id, active.header.epoch)?;
        let mut db = Db::open(&self.db_path(), key)?;
        db.with_tx(
            |tx| -> std::result::Result<(), arya_vault_storage::StorageError> {
                tx.meta_set("header_version", version.to_string().as_bytes())
            },
        )?;
        db.close()?;
        Ok(())
    }

    /// Publishes `wraps` as header version `n + 1`, then drops the superseded header files.
    fn publish(&self, old: &ActiveHeader, vk: &VaultKey, wraps: HeaderWraps) -> Result<u32> {
        let version = old
            .header
            .header_version
            .checked_add(1)
            .ok_or_else(|| CliError::failure("header version overflow"))?;
        let header = Header::new(
            wraps.vault_id,
            version,
            wraps.epoch,
            wraps.kdf,
            wraps.wrap_pw,
            wraps.wrap_rk,
            old.header.created_at,
        );
        let name = self.write_header(&header, &old.device_id)?;
        for other in self.header_files()? {
            if other != name {
                let _ = fs::remove_file(self.root.join(other));
            }
        }
        self.record_header_version(vk, old, version)?;
        Ok(version)
    }

    /// Changes the master password **without** the recovery key (docs/04 §9, SEC-C12).
    pub fn change_password(&self, old: &str, new: &str, profile: KdfProfile) -> Result<u32> {
        let active = self.active_header()?;
        let wraps = wraps_of(&active.header);
        let vk = vault_key::unlock_with_password(old, &wraps)?;
        let new_wraps =
            vault_key::change_password(old, new, &wraps, kdf_for(profile)?, &mut OsRng)?;
        self.publish(&active, &vk, new_wraps)
    }

    /// Resets the master password using the recovery key (docs/04 §9). Optionally replaces the
    /// recovery key and returns the new one.
    pub fn recover(
        &self,
        rk: &RecoveryKey,
        new_password: &str,
        profile: KdfProfile,
        regenerate: bool,
    ) -> Result<(u32, Option<RecoveryKey>)> {
        let active = self.active_header()?;
        let wraps = wraps_of(&active.header);
        let vk = vault_key::unlock_with_recovery_key(rk, &wraps)?;
        let kdf = kdf_for(profile)?;
        let mk = kdf::derive_master_key(new_password, &kdf)?;
        let kek = hkdf::kek_pw(&mk, &wraps.vault_id)?;
        let wrap_pw = wrap::wrap_pw(&kek, &wraps.vault_id, wraps.epoch, &kdf, &vk, &mut OsRng)
            .map_err(|_| CliError::failure("could not wrap the vault key"))?;
        let mut out = HeaderWraps {
            kdf,
            wrap_pw,
            ..wraps
        };
        let new_rk = if regenerate {
            let (rk2, w) = self.new_recovery_wrap(&vk, &out.vault_id, out.epoch)?;
            out.wrap_rk = w;
            Some(rk2)
        } else {
            None
        };
        let v = self.publish(&active, &vk, out)?;
        Ok((v, new_rk))
    }

    /// Replaces the recovery key; the master password stays.
    pub fn rotate_recovery_key(&self, password: &str) -> Result<(u32, RecoveryKey)> {
        let (active, vk) = self.check_password(password)?;
        let mut wraps = wraps_of(&active.header);
        let (rk, w) = self.new_recovery_wrap(&vk, &wraps.vault_id, wraps.epoch)?;
        wraps.wrap_rk = w;
        let v = self.publish(&active, &vk, wraps)?;
        Ok((v, rk))
    }

    fn new_recovery_wrap(
        &self,
        vk: &VaultKey,
        vault_id: &VaultId,
        epoch: u32,
    ) -> Result<(RecoveryKey, wrap::WrappedKey)> {
        let rk = arya_vault_crypto::recovery_key::generate(&mut OsRng)?;
        let kek = hkdf::kek_rk(&rk, vault_id)?;
        let w = wrap::wrap_rk(&kek, vault_id, epoch, vk, &mut OsRng as &mut dyn Rng)
            .map_err(|_| CliError::failure("could not wrap the vault key"))?;
        Ok((rk, w))
    }
}

/// Parses a recovery key typed by the user. Errors never echo the input.
pub fn parse_recovery_key(text: &Zeroizing<String>) -> Result<RecoveryKey> {
    arya_vault_crypto::recovery_key::parse(text)
        .map_err(|_| CliError::usage("that is not a valid recovery key (check for typos)"))
}
