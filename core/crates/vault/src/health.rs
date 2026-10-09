//! Password health reports: reuse, weakness, age. Nothing here is stored.

use std::collections::HashMap;

use arya_vault_storage::{Id, Store};
use hkdf::Hkdf;
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::engine;
use crate::error::Result;
use crate::hlc::Hlc;
use crate::model::{DAY_MS, ItemType};
use crate::value;
use crate::vault::Vault;
use crate::views::{OldPassword, ReuseGroup, WeakPassword};

impl Vault {
    /// All visible logins with a password: `(id, password register hlc, password)`.
    fn passwords(&mut self) -> Result<Vec<(Id, Hlc, Zeroizing<String>)>> {
        self.db.with_read(|tx| {
            let rows: HashMap<Id, _> = tx
                .list_items(arya_vault_storage::ItemFilter {
                    include_deleted: true,
                    item_type: Some(ItemType::Login.as_str()),
                    ..Default::default()
                })?
                .into_iter()
                .map(|r| (r.id, r))
                .collect();
            let mut out = Vec::new();
            for f in tx.fields_with_key("password")? {
                let (Some(row), Some(raw)) = (rows.get(&f.item_id), &f.value) else {
                    continue;
                };
                if row.deleted && !engine::is_visible(&engine::load_regs(tx, &f.item_id)?) {
                    continue;
                }
                let pw = value::decode_secret_text(raw)?;
                if !pw.is_empty() {
                    out.push((f.item_id, Hlc::from_i64(f.hlc)?, pw));
                }
            }
            out.sort_by_key(|(id, _, _)| *id);
            Ok(out)
        })
    }

    /// Groups of visible logins that share a password. Passwords are compared
    /// through HMAC-SHA-256 under a random key that exists only for this call
    /// (nothing is stored); the result holds item ids only.
    ///
    /// # Errors
    /// Storage errors.
    pub fn reused_passwords(&mut self) -> Result<Vec<ReuseGroup>> {
        let key: Zeroizing<[u8; 32]> = Zeroizing::new(engine::os_random::<32>()?);
        let mut groups: HashMap<[u8; 32], Vec<Id>> = HashMap::new();
        for (id, _, pw) in self.passwords()? {
            // HKDF-Extract(salt = key, ikm = password) is exactly HMAC-SHA-256(key, password).
            let (prk, _) = Hkdf::<Sha256>::extract(Some(&key[..]), pw.as_bytes());
            let mut tag = [0u8; 32];
            tag.copy_from_slice(&prk);
            groups.entry(tag).or_default().push(id);
        }
        let mut out: Vec<ReuseGroup> = groups
            .into_values()
            .filter(|g| g.len() > 1)
            .map(|item_ids| ReuseGroup { item_ids })
            .collect();
        out.sort_by(|a, b| a.item_ids.cmp(&b.item_ids));
        Ok(out)
    }

    /// Logins whose password scores `<= max_score` (0-4) with the zxcvbn estimator.
    ///
    /// # Errors
    /// Storage errors.
    pub fn weak_passwords(&mut self, max_score: u8) -> Result<Vec<WeakPassword>> {
        let mut out = Vec::new();
        for (id, _, pw) in self.passwords()? {
            let score = arya_vault_generator::estimate_strength(&pw).score;
            if score <= max_score {
                out.push(WeakPassword { item_id: id, score });
            }
        }
        Ok(out)
    }

    /// Logins whose password was last set at least `older_than_days` ago. Age comes
    /// from the register's HLC time, the only timestamp stored for a password
    /// (see PR "Spec questions").
    ///
    /// # Errors
    /// Storage errors.
    pub fn old_passwords(&mut self, older_than_days: u64) -> Result<Vec<OldPassword>> {
        let now = self.clock.wall_ms();
        Ok(self
            .passwords()?
            .into_iter()
            .filter_map(|(item_id, hlc, _)| {
                let age = now.saturating_sub(hlc.pt()) / DAY_MS;
                (age >= older_than_days).then_some(OldPassword {
                    item_id,
                    age_days: age,
                })
            })
            .collect())
    }
}
