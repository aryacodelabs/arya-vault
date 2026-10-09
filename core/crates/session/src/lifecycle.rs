//! The vault lifecycle over a [`VaultDir`]: create, password check, change password, recovery
//! and recovery-key replacement (docs/04 §2, §4, §9).
//!
//! These functions order the calls into `arya-vault-crypto` and decide where the bytes go; they
//! contain no cryptography of their own. Each "prepare" step does the expensive, fallible work
//! and writes nothing; [`publish`] then writes the new header, so a failure before it leaves the
//! directory untouched.

use std::fs;

use arya_vault_crypto::VaultId;
use arya_vault_crypto::format::header::Header;
use arya_vault_crypto::hkdf::{self, SubKeyLabel};
use arya_vault_crypto::kdf;
use arya_vault_crypto::keys::{RecoveryKey, VaultKey};
use arya_vault_crypto::rng::{OsRng, Rng};
use arya_vault_crypto::vault_key::{self, HeaderWraps};
use arya_vault_crypto::wrap;
use arya_vault_storage::{CreateParams, Db, DbKey, Store};
use arya_vault_vault::{SystemClock, Vault};

use crate::error::{Result, SessionError};
use crate::layout::{ActiveHeader, INITIAL_EPOCH, Seen, VaultDir, wraps_of};
use crate::profile::{KdfProfile, random_array};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

pub(crate) fn db_key(vk: &VaultKey, vault_id: &VaultId, epoch: u32) -> Result<DbKey> {
    let sub = hkdf::subkey(vk, vault_id, SubKeyLabel::Db, epoch)?;
    Ok(DbKey::from_bytes(*sub.expose_secret()))
}

/// A freshly created, open vault.
pub(crate) struct Created {
    pub(crate) vault: Vault,
    pub(crate) vk: VaultKey,
    pub(crate) recovery_key: RecoveryKey,
}

/// Creates the header and the database of a new vault.
///
/// `before_write` runs after the directory exists and before the first vault file is written
/// (the session uses it to place the onboarding marker, see `meta`).
pub(crate) fn create(
    dir: &VaultDir,
    seen: &mut Seen,
    password: &str,
    profile: KdfProfile,
    before_write: impl FnOnce(&VaultDir) -> Result<()>,
) -> Result<Created> {
    if dir.has_vault()? {
        return Err(SessionError::AlreadyExists);
    }
    // Everything fallible and slow happens before the first byte is written.
    let kdf = profile.params()?;
    let new = vault_key::create_vault(password, kdf, INITIAL_EPOCH, &mut OsRng)?;
    let device_id: [u8; 16] = random_array(&mut OsRng)?;
    dir.ensure_dir()?;
    before_write(dir)?;
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
    let header_name = dir.write_header(&header, &device_id)?;
    let db = match Db::create(
        &dir.db_path(),
        key,
        &CreateParams {
            vault_id: w.vault_id,
            device_id,
            epoch: u64::from(w.epoch),
            header_version: 1,
        },
    ) {
        Ok(db) => db,
        Err(e) => {
            let _ = fs::remove_file(dir.root().join(header_name));
            return Err(e.into());
        }
    };
    match Vault::open(db, Box::new(SystemClock)) {
        Ok(vault) => {
            seen.vault_id = Some(w.vault_id);
            seen.highest_epoch = seen.highest_epoch.max(w.epoch);
            Ok(Created {
                vault,
                vk: new.vault_key,
                recovery_key: new.recovery_key,
            })
        }
        Err(e) => {
            let _ = fs::remove_file(dir.root().join(header_name));
            let _ = fs::remove_file(dir.db_path());
            Err(e.into())
        }
    }
}

/// Verifies the password against the active header only (no database access).
pub(crate) fn check_password(
    dir: &VaultDir,
    seen: &mut Seen,
    password: &str,
) -> Result<(ActiveHeader, VaultKey)> {
    let active = dir.active_header(seen)?;
    let vk = vault_key::unlock_with_password(password, &wraps_of(&active.header))?;
    Ok((active, vk))
}

/// Opens the database with the key derived from `vk`.
pub(crate) fn open_vault(dir: &VaultDir, vk: &VaultKey, active: &ActiveHeader) -> Result<Vault> {
    let key = db_key(vk, &active.header.vault_id, active.header.epoch)?;
    let db = Db::open(&dir.db_path(), key)?;
    Ok(Vault::open(db, Box::new(SystemClock))?)
}

/// New wraps ready to be published, plus what the caller needs afterwards.
pub(crate) struct Prepared {
    pub(crate) active: ActiveHeader,
    pub(crate) wraps: HeaderWraps,
    /// The vault key, when the caller asked for it (it is needed to touch the database).
    pub(crate) vk: Option<VaultKey>,
    pub(crate) new_recovery_key: Option<RecoveryKey>,
}

/// Prepares a password change **without the recovery key** (docs/04 §9, SEC-C12).
pub(crate) fn prepare_change_password(
    dir: &VaultDir,
    seen: &mut Seen,
    old: &str,
    new: &str,
    profile: KdfProfile,
    want_vk: bool,
) -> Result<Prepared> {
    let active = dir.active_header(seen)?;
    let wraps = wraps_of(&active.header);
    let vk = if want_vk {
        Some(vault_key::unlock_with_password(old, &wraps)?)
    } else {
        None
    };
    let new_wraps = vault_key::change_password(old, new, &wraps, profile.params()?, &mut OsRng)?;
    Ok(Prepared {
        active,
        wraps: new_wraps,
        vk,
        new_recovery_key: None,
    })
}

/// Prepares a password reset with the recovery key (docs/04 §9); optionally replaces the
/// recovery key in the same header.
pub(crate) fn prepare_recover(
    dir: &VaultDir,
    seen: &mut Seen,
    rk: &RecoveryKey,
    new_password: &str,
    profile: KdfProfile,
    regenerate: bool,
) -> Result<Prepared> {
    let active = dir.active_header(seen)?;
    let wraps = wraps_of(&active.header);
    let vk = vault_key::unlock_with_recovery_key(rk, &wraps)?;
    let kdf = profile.params()?;
    let mk = kdf::derive_master_key(new_password, &kdf)?;
    let kek = hkdf::kek_pw(&mk, &wraps.vault_id)?;
    let wrap_pw = wrap::wrap_pw(&kek, &wraps.vault_id, wraps.epoch, &kdf, &vk, &mut OsRng)
        .map_err(|_| SessionError::Internal("could not wrap the vault key"))?;
    let mut out = HeaderWraps {
        kdf,
        wrap_pw,
        ..wraps
    };
    let new_recovery_key = if regenerate {
        let (rk2, w) = new_recovery_wrap(&vk, &out.vault_id, out.epoch)?;
        out.wrap_rk = w;
        Some(rk2)
    } else {
        None
    };
    Ok(Prepared {
        active,
        wraps: out,
        vk: Some(vk),
        new_recovery_key,
    })
}

/// Prepares a recovery-key replacement; the master password stays. Proves knowledge of the
/// password first.
pub(crate) fn prepare_regenerate(
    dir: &VaultDir,
    seen: &mut Seen,
    password: &str,
) -> Result<Prepared> {
    let (active, vk) = check_password(dir, seen, password)?;
    let mut wraps = wraps_of(&active.header);
    let (rk, w) = new_recovery_wrap(&vk, &wraps.vault_id, wraps.epoch)?;
    wraps.wrap_rk = w;
    Ok(Prepared {
        active,
        wraps,
        vk: Some(vk),
        new_recovery_key: Some(rk),
    })
}

fn new_recovery_wrap(
    vk: &VaultKey,
    vault_id: &VaultId,
    epoch: u32,
) -> Result<(RecoveryKey, wrap::WrappedKey)> {
    let rk = arya_vault_crypto::recovery_key::generate(&mut OsRng)?;
    let kek = hkdf::kek_rk(&rk, vault_id)?;
    let w = wrap::wrap_rk(&kek, vault_id, epoch, vk, &mut OsRng as &mut dyn Rng)
        .map_err(|_| SessionError::Internal("could not wrap the vault key"))?;
    Ok((rk, w))
}

/// Publishes `wraps` as header version `n + 1` (temp file + rename, then the superseded headers
/// are removed) and records the version in the database `meta`. `vk` must be the vault key.
pub(crate) fn publish(
    dir: &VaultDir,
    old: &ActiveHeader,
    wraps: HeaderWraps,
    vk: &VaultKey,
) -> Result<u32> {
    let version = old
        .header
        .header_version
        .checked_add(1)
        .ok_or(SessionError::Internal("header version overflow"))?;
    let header = Header::new(
        wraps.vault_id,
        version,
        wraps.epoch,
        wraps.kdf,
        wraps.wrap_pw,
        wraps.wrap_rk,
        old.header.created_at,
    );
    dir.publish_header(&header, &old.device_id)?;
    record_header_version(dir, vk, old, version)?;
    Ok(version)
}

/// Records the header version in the database `meta` (kept in step with the header file).
/// A directory without a database (a header-only fixture) has nothing to update.
fn record_header_version(
    dir: &VaultDir,
    vk: &VaultKey,
    active: &ActiveHeader,
    version: u32,
) -> Result<()> {
    if !dir.has_db() {
        return Ok(());
    }
    let key = db_key(vk, &active.header.vault_id, active.header.epoch)?;
    let mut db = Db::open(&dir.db_path(), key)?;
    db.with_tx(
        |tx| -> std::result::Result<(), arya_vault_storage::StorageError> {
            tx.meta_set("header_version", version.to_string().as_bytes())
        },
    )?;
    db.close()?;
    Ok(())
}
