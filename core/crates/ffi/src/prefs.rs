//! `AppSettings` persistence (docs/14 §4.5): one fixed-layout record in the vault's encrypted
//! `meta` table, and the clamping the core owes the app.
//!
//! The record is authenticated (it lives inside the encrypted database) but the decoder is still
//! total: any wrong version or length yields the defaults, never an error or a panic, and every
//! value is clamped after decoding.

use arya_vault_vault::{Vault, VaultConfig, VaultError};

use crate::api::dto::{AppSettings, VaultConfig as VaultConfigDto};

const NAME: &str = "app";
const VERSION: u8 = 1;
const LEN: usize = 1 + 4 + 1 + 4 + 4 + 4 * 4;

pub(crate) const AUTO_LOCK_MINUTES: (u32, u32) = (1, 60);
pub(crate) const CLIPBOARD_SECONDS: (u32, u32) = (5, 120);
/// SEC-H03: secrets auto-hide within 15 s.
pub(crate) const REVEAL_HIDE_SECONDS: (u32, u32) = (1, 15);
const HISTORY: (u32, u32) = (1, 100);
const TRASH_DAYS: (u32, u32) = (1, 365);
const TOMBSTONE_DAYS: (u32, u32) = (30, 730);

impl Default for AppSettings {
    fn default() -> Self {
        let c = VaultConfig::default();
        Self {
            auto_lock_minutes: 5,
            lock_on_sleep: true,
            lock_on_screen_lock: true,
            clipboard_clear_seconds: 30,
            block_screen_capture: true,
            reveal_hide_seconds: 15,
            vault_config: VaultConfigDto {
                history_sensitive: to_u32(c.history_sensitive as u64),
                history_other: to_u32(c.history_other as u64),
                trash_days: to_u32(c.trash_days),
                tombstone_days: to_u32(c.tombstone_days),
            },
        }
    }
}

fn to_u32(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn clamp(v: u32, (lo, hi): (u32, u32)) -> u32 {
    v.clamp(lo, hi)
}

impl AppSettings {
    /// Every value inside its allowed range (docs/14 §4.5, SEC-H03).
    pub(crate) fn clamped(mut self) -> Self {
        self.auto_lock_minutes = clamp(self.auto_lock_minutes, AUTO_LOCK_MINUTES);
        self.clipboard_clear_seconds = clamp(self.clipboard_clear_seconds, CLIPBOARD_SECONDS);
        self.reveal_hide_seconds = clamp(self.reveal_hide_seconds, REVEAL_HIDE_SECONDS);
        let c = &mut self.vault_config;
        c.history_sensitive = clamp(c.history_sensitive, HISTORY);
        c.history_other = clamp(c.history_other, HISTORY);
        c.trash_days = clamp(c.trash_days, TRASH_DAYS);
        c.tombstone_days = clamp(c.tombstone_days, TOMBSTONE_DAYS).max(c.trash_days);
        self
    }

    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(LEN);
        out.push(VERSION);
        out.extend(self.auto_lock_minutes.to_le_bytes());
        out.push(
            u8::from(self.lock_on_sleep)
                | u8::from(self.lock_on_screen_lock) << 1
                | u8::from(self.block_screen_capture) << 2,
        );
        out.extend(self.clipboard_clear_seconds.to_le_bytes());
        out.extend(self.reveal_hide_seconds.to_le_bytes());
        let c = &self.vault_config;
        for n in [
            c.history_sensitive,
            c.history_other,
            c.trash_days,
            c.tombstone_days,
        ] {
            out.extend(n.to_le_bytes());
        }
        out
    }

    /// Total: anything that is not a version-1 record decodes to the defaults.
    pub(crate) fn decode(bytes: &[u8]) -> Self {
        if bytes.len() != LEN || bytes[0] != VERSION {
            return Self::default();
        }
        let u32_at = |i: usize| {
            let mut b = [0u8; 4];
            b.copy_from_slice(&bytes[i..i + 4]);
            u32::from_le_bytes(b)
        };
        let flags = bytes[5];
        Self {
            auto_lock_minutes: u32_at(1),
            lock_on_sleep: flags & 1 != 0,
            lock_on_screen_lock: flags & 2 != 0,
            block_screen_capture: flags & 4 != 0,
            clipboard_clear_seconds: u32_at(6),
            reveal_hide_seconds: u32_at(10),
            vault_config: VaultConfigDto {
                history_sensitive: u32_at(14),
                history_other: u32_at(18),
                trash_days: u32_at(22),
                tombstone_days: u32_at(26),
            },
        }
        .clamped()
    }

    fn to_core(self) -> VaultConfig {
        let c = self.vault_config;
        VaultConfig {
            history_sensitive: c.history_sensitive as usize,
            history_other: c.history_other as usize,
            trash_days: u64::from(c.trash_days),
            tombstone_days: u64::from(c.tombstone_days),
        }
    }
}

/// Reads the stored settings (defaults if none) and applies the retention part to the vault.
pub(crate) fn load(v: &mut Vault) -> Result<AppSettings, VaultError> {
    let s = v
        .get_setting(NAME)?
        .map_or_else(AppSettings::default, |b| AppSettings::decode(&b));
    v.set_config(s.to_core());
    Ok(s)
}

/// Clamps, stores and applies `s`.
pub(crate) fn store(v: &mut Vault, s: AppSettings) -> Result<(), VaultError> {
    let s = s.clamped();
    v.set_setting(NAME, &s.encode())?;
    v.set_config(s.to_core());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_docs_07() {
        let d = AppSettings::default();
        assert_eq!(
            (
                d.auto_lock_minutes,
                d.clipboard_clear_seconds,
                d.reveal_hide_seconds
            ),
            (5, 30, 15)
        );
        assert_eq!(d, d.clamped());
    }

    #[test]
    fn round_trip_and_clamping() {
        let mut s = AppSettings {
            auto_lock_minutes: 0,
            clipboard_clear_seconds: 9_999,
            reveal_hide_seconds: 600,
            ..AppSettings::default()
        };
        s.vault_config.trash_days = 0;
        s.vault_config.tombstone_days = 1;
        let c = s.clamped();
        assert_eq!(c.auto_lock_minutes, 1);
        assert_eq!(c.clipboard_clear_seconds, 120);
        assert_eq!(c.reveal_hide_seconds, 15);
        assert_eq!(c.vault_config.trash_days, 1);
        assert_eq!(c.vault_config.tombstone_days, 30);
        assert_eq!(AppSettings::decode(&c.encode()), c);
    }

    #[test]
    fn decode_is_total() {
        let good = AppSettings::default().encode();
        for n in 0..good.len() + 3 {
            let _ = AppSettings::decode(&vec![0xFF; n]);
        }
        let mut wrong_version = good.clone();
        wrong_version[0] = 9;
        assert_eq!(AppSettings::decode(&wrong_version), AppSettings::default());
        assert_eq!(
            AppSettings::decode(&good[..good.len() - 1]),
            AppSettings::default()
        );
        // Out-of-range stored values are clamped, not trusted.
        let mut hostile = good;
        hostile[1..5].copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(AppSettings::decode(&hostile).auto_lock_minutes, 60);
    }
}
