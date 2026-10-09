//! CLI glue over `arya-vault-session`: the vault lifecycle (header selection, key plumbing,
//! create/unlock/change/recover) lives in that library; this module only adapts it to the CLI's
//! error type and one-shot command style.

use std::path::PathBuf;

use arya_vault_session::Session;
use arya_vault_vault::Vault;

use crate::args::KdfProfile;
use crate::error::{CliError, Result};

impl From<KdfProfile> for arya_vault_session::KdfProfile {
    fn from(p: KdfProfile) -> Self {
        match p {
            KdfProfile::Low => Self::Low,
            KdfProfile::Default => Self::Default,
            KdfProfile::High => Self::High,
        }
    }
}

/// An unlocked vault for the duration of one command. Dropping it locks the session.
pub struct Unlocked {
    session: Session,
}

impl Unlocked {
    /// Opens the vault directory and unlocks it with `password`.
    pub fn open(dir: PathBuf, password: &str) -> Result<Self> {
        let mut session = Session::open_dir(dir)?;
        session.unlock(password)?;
        Ok(Self { session })
    }

    /// Runs `f` on the vault.
    pub fn run<R, E: Into<CliError>>(
        &mut self,
        f: impl FnOnce(&mut Vault) -> std::result::Result<R, E>,
    ) -> Result<R> {
        self.session.with_vault(f)?.map_err(Into::into)
    }

    /// Locks the session, closing the database (checkpoint and key wipe).
    pub fn close(mut self) -> Result<()> {
        self.session.lock()?;
        Ok(())
    }
}
