//! `vault create`, `unlock-check`, `password change`, `recover`, `rotate-recovery-key`, `info`.

use arya_vault_crypto::format::FORMAT_VERSION;
use arya_vault_crypto::recovery_key;
use arya_vault_generator::meets_master_password_policy;
use arya_vault_storage::{Store, latest_schema_version};
use serde_json::json;

use super::Ctx;
use crate::args::{InfoArgs, PasswordCmd, RecoverArgs, RevealArgs, VaultCmd};
use crate::error::{CliError, Result};
use crate::layout::parse_recovery_key;
use crate::out::hex;

/// SEC-A07: the master password policy applies to every new master password.
fn require_policy(password: &str) -> Result<()> {
    meets_master_password_policy(password).map_err(|v| {
        let reasons: Vec<String> = v.iter().map(ToString::to_string).collect();
        CliError::usage(format!("master password rejected: {}", reasons.join("; ")))
    })
}

fn show_recovery_key(
    ctx: &Ctx,
    rk: &arya_vault_crypto::keys::RecoveryKey,
    extra: serde_json::Value,
) -> Result<()> {
    let text = recovery_key::encode(rk);
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
            let dir = ctx.vault_dir()?;
            let pw = ctx.secrets.read_new("New master password")?;
            require_policy(&pw)?;
            let rk = dir.create(&pw, a.kdf_profile)?;
            show_recovery_key(ctx, &rk, json!({ "created": true }))
        }
    }
}

pub fn unlock_check(ctx: &Ctx) -> Result<()> {
    let dir = ctx.vault_dir()?;
    let pw = ctx.master_password()?;
    let database_opened = if dir.has_db() {
        dir.unlock(&pw)?.vault.close()?;
        true
    } else {
        dir.check_password(&pw)?;
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
    let dir = ctx.vault_dir()?;
    let old = ctx.secrets.read("Current master password")?;
    let new = ctx.secrets.read_new("New master password")?;
    require_policy(&new)?;
    let version = dir.change_password(&old, &new, kdf_profile)?;
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
    let dir = ctx.vault_dir()?;
    let rk_text = ctx.secrets.read("Recovery key")?;
    let rk = parse_recovery_key(&rk_text)?;
    let new = ctx.secrets.read_new("New master password")?;
    require_policy(&new)?;
    let (version, new_rk) = dir.recover(&rk, &new, a.kdf_profile, a.regenerate_recovery_key)?;
    match new_rk {
        Some(k) => show_recovery_key(ctx, &k, json!({ "ok": true, "header_version": version })),
        None => ctx.out.emit(
            &format!("master password reset with the recovery key (header version {version})"),
            &json!({ "ok": true, "header_version": version }),
        ),
    }
}

pub fn rotate_recovery_key(ctx: &Ctx, a: RevealArgs) -> Result<()> {
    if !a.reveal {
        return Err(CliError::usage(
            "rotate-recovery-key shows the new key exactly once; pass --reveal to print it",
        ));
    }
    let dir = ctx.vault_dir()?;
    let pw = ctx.master_password()?;
    let (version, rk) = dir.rotate_recovery_key(&pw)?;
    show_recovery_key(ctx, &rk, json!({ "ok": true, "header_version": version }))
}

const PINNED_KEYS: [&str; 10] = [
    "cipher.page_size",
    "cipher.compatibility",
    "cipher.kdf_algorithm",
    "cipher.hmac_algorithm",
    "cipher.kdf_iter",
    "cipher.plaintext_header_size",
    "sqlite.journal_mode",
    "sqlite.synchronous",
    "sqlite.foreign_keys",
    "sqlite.secure_delete",
];

pub fn info(ctx: &Ctx, a: InfoArgs) -> Result<()> {
    let dir = ctx.vault_dir()?;
    let active = dir.active_header()?;
    let h = &active.header;
    let mut lines = vec![
        format!("container format_version: {FORMAT_VERSION}"),
        format!("header format_version:    {}", h.format_version),
        format!("header_version:           {}", h.header_version),
        format!("epoch:                    {}", h.epoch),
        format!("vault_id:                 {}", hex(&h.vault_id)),
        format!("device_id:                {}", hex(&active.device_id)),
        format!(
            "kdf:                      argon2id m={} KiB t={} p={}",
            h.kdf.m_kib, h.kdf.t, h.kdf.p
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
            "device_id": hex(&active.device_id),
            "kdf": { "alg": "argon2id", "m_kib": h.kdf.m_kib, "t": h.kdf.t, "p": h.kdf.p },
            "created_at": h.created_at,
        },
        "database": null,
    });
    if !a.no_unlock && dir.has_db() {
        let pw = ctx.master_password()?;
        let (active, vk) = dir.check_password(&pw)?;
        let key = {
            use arya_vault_crypto::hkdf::{SubKeyLabel, subkey};
            let sub = subkey(
                &vk,
                &active.header.vault_id,
                SubKeyLabel::Db,
                active.header.epoch,
            )?;
            arya_vault_storage::DbKey::from_bytes(*sub.expose_secret())
        };
        let mut db = arya_vault_storage::Db::open(&dir.db_path(), key)?;
        let schema = db.schema_version()?;
        let mut pinned = serde_json::Map::new();
        for k in PINNED_KEYS {
            let v = db.with_read(
                |tx| -> std::result::Result<Option<Vec<u8>>, arya_vault_storage::StorageError> {
                    tx.meta_get(k)
                },
            )?;
            let v = v.map_or_else(
                || "(unset)".to_owned(),
                |b| String::from_utf8_lossy(&b).into_owned(),
            );
            lines.push(format!("{k}: {v}"));
            pinned.insert(k.to_owned(), json!(v));
        }
        lines.push(format!(
            "schema_version:           {schema} (latest supported {})",
            latest_schema_version()
        ));
        value["database"] = json!({
            "schema_version": schema,
            "latest_schema_version": latest_schema_version(),
            "pinned_settings": pinned,
        });
        db.close()?;
    }
    ctx.out.lines(&lines, &value)
}
