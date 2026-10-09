//! `vault create`, `unlock-check`, `password change`, `recover`, `rotate-recovery-key`, `info`.
//!
//! Argument handling and output only: the lifecycle itself is `arya-vault-session`.

use arya_vault_crypto::format::FORMAT_VERSION;
use arya_vault_session::{RecoveryConfirmation, RecoveryKeyResult};
use arya_vault_storage::latest_schema_version;
use serde_json::json;

use super::Ctx;
use crate::args::{InfoArgs, PasswordCmd, RecoverArgs, RevealArgs, VaultCmd};
use crate::error::{CliError, Result};
use crate::out::hex;

/// The CLI prints the key itself, so there is no onboarding step to confirm.
const CLI_CONFIRMATION: RecoveryConfirmation = RecoveryConfirmation::NotRequired;

fn show_recovery_key(ctx: &Ctx, rk: &RecoveryKeyResult, extra: serde_json::Value) -> Result<()> {
    let text = rk.recovery_key();
    let mut value = extra;
    value["recovery_key"] = json!(text.as_str());
    ctx.out.lines(
        &[
            "Recovery key (shown once; store it offline):".to_owned(),
            format!("  {}", text.as_str()),
        ],
        &value,
    )
}

pub fn vault(ctx: &Ctx, cmd: VaultCmd) -> Result<()> {
    match cmd {
        VaultCmd::Create(a) => {
            if !a.reveal {
                return Err(CliError::usage(
                    "vault create shows the recovery key exactly once; pass --reveal to print it",
                ));
            }
            let mut session = ctx.session()?;
            let pw = ctx.secrets.read_new("New master password")?;
            let rk = session.create_with(&pw, a.kdf_profile.into(), CLI_CONFIRMATION)?;
            show_recovery_key(ctx, &rk, json!({ "created": true }))
        }
    }
}

pub fn unlock_check(ctx: &Ctx) -> Result<()> {
    let mut session = ctx.session()?;
    let pw = ctx.master_password()?;
    let database_opened = if session.has_database() {
        session.unlock(&pw)?;
        session.lock()?;
        true
    } else {
        session.check_header_password(&pw)?;
        false
    };
    ctx.out.emit(
        if database_opened {
            "ok: password accepted, database opened"
        } else {
            "ok: password accepted (header only; no database in this directory)"
        },
        &json!({ "ok": true, "database_opened": database_opened }),
    )
}

pub fn password(ctx: &Ctx, cmd: PasswordCmd) -> Result<()> {
    let PasswordCmd::Change { kdf_profile } = cmd;
    let mut session = ctx.session()?;
    let old = ctx.secrets.read("Current master password")?;
    let new = ctx.secrets.read_new("New master password")?;
    let version = session.change_password(&old, &new, kdf_profile.into())?;
    ctx.out.emit(
        &format!(
            "master password changed (header version {version}); the recovery key is unchanged"
        ),
        &json!({ "ok": true, "header_version": version }),
    )
}

pub fn recover(ctx: &Ctx, a: RecoverArgs) -> Result<()> {
    if a.regenerate_recovery_key && !a.reveal {
        return Err(CliError::usage(
            "--regenerate-recovery-key prints the new key once; pass --reveal as well",
        ));
    }
    let mut session = ctx.session()?;
    let rk_text = ctx.secrets.read("Recovery key")?;
    let new = ctx.secrets.read_new("New master password")?;
    let profile = a.kdf_profile.into();
    if a.regenerate_recovery_key {
        let k = session.recover_and_regenerate(&rk_text, &new, profile, CLI_CONFIRMATION)?;
        show_recovery_key(
            ctx,
            &k,
            json!({ "ok": true, "header_version": k.header_version() }),
        )
    } else {
        let version = session.recover(&rk_text, &new, profile)?;
        ctx.out.emit(
            &format!("master password reset with the recovery key (header version {version})"),
            &json!({ "ok": true, "header_version": version }),
        )
    }
}

pub fn rotate_recovery_key(ctx: &Ctx, a: RevealArgs) -> Result<()> {
    if !a.reveal {
        return Err(CliError::usage(
            "rotate-recovery-key shows the new key exactly once; pass --reveal to print it",
        ));
    }
    let mut session = ctx.session()?;
    let pw = ctx.master_password()?;
    session.unlock(&pw)?;
    let rk = session.regenerate_recovery_key_with(&pw, CLI_CONFIRMATION)?;
    show_recovery_key(
        ctx,
        &rk,
        json!({ "ok": true, "header_version": rk.header_version() }),
    )
}

pub fn info(ctx: &Ctx, a: InfoArgs) -> Result<()> {
    let mut session = ctx.session()?;
    let h = session.header_info()?;
    let mut lines = vec![
        format!("container format_version: {FORMAT_VERSION}"),
        format!("header format_version:    {}", h.format_version),
        format!("header_version:           {}", h.header_version),
        format!("epoch:                    {}", h.epoch),
        format!("vault_id:                 {}", hex(&h.vault_id)),
        format!("device_id:                {}", hex(&h.device_id)),
        format!(
            "kdf:                      argon2id m={} KiB t={} p={}",
            h.kdf_m_kib, h.kdf_t, h.kdf_p
        ),
        format!("created_at (unix s):      {}", h.created_at),
    ];
    let mut value = json!({
        "format_version": FORMAT_VERSION,
        "header": {
            "format_version": h.format_version,
            "header_version": h.header_version,
            "epoch": h.epoch,
            "vault_id": hex(&h.vault_id),
            "device_id": hex(&h.device_id),
            "kdf": { "alg": "argon2id", "m_kib": h.kdf_m_kib, "t": h.kdf_t, "p": h.kdf_p },
            "created_at": h.created_at,
        },
        "database": null,
    });
    if !a.no_unlock && session.has_database() {
        let pw = ctx.master_password()?;
        session.unlock(&pw)?;
        let db = session.db_info()?;
        let mut pinned = serde_json::Map::new();
        for (k, v) in &db.pinned_settings {
            let v = v.clone().unwrap_or_else(|| "(unset)".to_owned());
            lines.push(format!("{k}: {v}"));
            pinned.insert((*k).to_owned(), json!(v));
        }
        lines.push(format!(
            "schema_version:           {} (latest supported {})",
            db.schema_version,
            latest_schema_version()
        ));
        value["database"] = json!({
            "schema_version": db.schema_version,
            "latest_schema_version": latest_schema_version(),
            "pinned_settings": pinned,
        });
        session.lock()?;
    }
    ctx.out.lines(&lines, &value)
}
