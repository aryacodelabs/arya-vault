//! Version history, the derived concurrency view and restore (US-14, doc 06 section 5.3).
//! Reading is pure; restoring is an ordinary mutation.

use arya_vault_storage::Id;
use zeroize::Zeroizing;

use crate::engine;
use crate::error::{Result, VaultError};
use crate::hlc::Hlc;
use crate::model::FieldRef;
use crate::register::concurrent_losers;
use crate::value;
use crate::vault::Vault;
use crate::views::VersionInfo;

fn info(r: &crate::register::Register, current: bool, concurrent: bool) -> VersionInfo {
    VersionInfo {
        hlc: r.hlc,
        device_id: r.device_id,
        base_hlc: r.base_hlc,
        current,
        concurrent,
        cleared: r.value.is_none(),
    }
}

impl Vault {
    /// The current version followed by retained history, newest first, with the
    /// derived `concurrent` flag on each older version.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the item does not exist.
    pub fn versions(&mut self, id: &Id, field: &FieldRef) -> Result<Vec<VersionInfo>> {
        let key = field.key();
        self.db.with_read(|tx| {
            engine::require_item(tx, id)?;
            let state = engine::state_of(tx, id, &key)?;
            let Some(w) = &state.winner else {
                return Ok(Vec::new());
            };
            let conc: Vec<_> = concurrent_losers(w, &state.history)
                .iter()
                .map(|r| r.key())
                .collect();
            let mut out = vec![info(w, true, false)];
            out.extend(
                state
                    .history
                    .iter()
                    .map(|h| info(h, false, conc.contains(&h.key()))),
            );
            Ok(out)
        })
    }

    /// Only the versions that are not ancestors of the current one ("Other versions").
    /// A pure read: it never changes stored state and does not depend on the
    /// order in which ops arrived.
    ///
    /// # Errors
    /// [`VaultError::NotFound`].
    pub fn concurrent_versions(&mut self, id: &Id, field: &FieldRef) -> Result<Vec<VersionInfo>> {
        Ok(self
            .versions(id, field)?
            .into_iter()
            .filter(|v| v.concurrent)
            .collect())
    }

    /// The value of a stored version (current or historical), in a zeroizing buffer.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if no such version is retained.
    pub fn reveal_version(
        &mut self,
        id: &Id,
        field: &FieldRef,
        hlc: Hlc,
        device_id: &Id,
    ) -> Result<Option<Zeroizing<String>>> {
        let key = field.key();
        self.db.with_read(|tx| {
            engine::require_item(tx, id)?;
            let state = engine::state_of(tx, id, &key)?;
            let hit = state
                .winner
                .iter()
                .chain(state.history.iter())
                .find(|r| r.hlc == hlc && &r.device_id == device_id)
                .ok_or(VaultError::NotFound)?;
            hit.value
                .as_deref()
                .map(|v| value::decode_secret_text(v))
                .transpose()
        })
    }

    /// Make an older version current again by writing its value as a new edit
    /// (based on the current version). Nothing is rewritten in place.
    ///
    /// # Errors
    /// [`VaultError::NotFound`] if the version is not retained; [`VaultError::InTrash`].
    pub fn restore_version(
        &mut self,
        id: &Id,
        field: &FieldRef,
        hlc: Hlc,
        device_id: &Id,
    ) -> Result<()> {
        let key = field.key();
        let Vault { db, .. } = self;
        let old = db.with_read(|tx| {
            let state = engine::state_of(tx, id, &key)?;
            let hit = state
                .winner
                .iter()
                .chain(state.history.iter())
                .find(|r| r.hlc == hlc && &r.device_id == device_id)
                .ok_or(VaultError::NotFound)?;
            Ok::<_, VaultError>(hit.value.clone())
        })?;
        self.mutate(id, true, |_, e, _, _| {
            e.set(&key, old);
            Ok(())
        })
    }
}
