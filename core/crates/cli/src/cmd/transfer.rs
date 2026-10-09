//! `export` and `import`.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;

use arya_vault_vault::{
    ImportLimits, ImportOptions, PlaintextRiskAcknowledged, parse_aryavault, parse_bitwarden_json,
    parse_csv, read_limited,
};
use serde_json::json;

use super::Ctx;
use crate::args::{ExportArgs, ExportFormat, ImportArgs, ImportFormat};
use crate::error::{CliError, Result};
use crate::layout::Unlocked;

/// Creates the output file exclusively (never overwrites), owner-only on Unix.
fn create_new(path: &Path) -> Result<File> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    opts.open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            CliError::failure("the output file already exists")
        } else {
            e.into()
        }
    })
}

pub fn export(ctx: &Ctx, a: ExportArgs) -> Result<()> {
    if matches!(a.format, ExportFormat::Csv) && !a.acknowledge_plaintext_risk {
        return Err(CliError::usage(
            "a CSV export contains every password in clear text; \
             pass --acknowledge-plaintext-risk to continue",
        ));
    }
    if a.include_history && matches!(a.format, ExportFormat::Csv) {
        return Err(CliError::usage(
            "--include-history applies to aryavault exports only",
        ));
    }
    let dir = ctx.vault_dir()?;
    let pw = ctx.master_password()?;
    let mut u = Unlocked::open(dir, &pw)?;
    let (bytes, detail): (zeroize::Zeroizing<Vec<u8>>, serde_json::Value) = match a.format {
        ExportFormat::Aryavault => {
            let export_pw = ctx.secrets.read_new("Export password")?;
            let cost = arya_vault_session::KdfProfile::from(a.kdf_profile).params()?;
            let b = u.run(|v| v.export_aryavault(&export_pw, &cost, a.include_history))?;
            (zeroize::Zeroizing::new(b), json!({}))
        }
        ExportFormat::Csv => {
            let e =
                u.run(|v| v.export_csv(PlaintextRiskAcknowledged::acknowledge_plaintext_risk()))?;
            let r = e.report;
            (
                e.bytes,
                json!({
                    "exported": r.exported,
                    "skipped_unsupported": r.skipped_unsupported,
                    "extra_urls_dropped": r.extra_urls_dropped,
                    "neutralized_cells": r.neutralized_cells,
                }),
            )
        }
    };
    u.close()?;
    let mut f = create_new(&a.out)?;
    f.write_all(&bytes)?;
    f.sync_all()?;
    ctx.out.emit(
        &format!("exported {} bytes to the output file", bytes.len()),
        &json!({ "bytes": bytes.len(), "report": detail }),
    )
}

pub fn import(ctx: &Ctx, a: ImportArgs) -> Result<()> {
    let limits = ImportLimits::default();
    let file = File::open(&a.file)?;
    let bytes = read_limited(file, &limits)?;
    let dir = ctx.vault_dir()?;
    let pw = ctx.master_password()?;
    let mut u = Unlocked::open(dir, &pw)?;
    let bundle = match a.format {
        ImportFormat::Csv => parse_csv(&bytes, &limits)?,
        ImportFormat::Bitwarden => parse_bitwarden_json(&bytes, &limits)?,
        ImportFormat::Aryavault => {
            let export_pw = ctx.secrets.read("Export password")?;
            parse_aryavault(&bytes, &export_pw, &limits)?
        }
    };
    let report = u.run(|v| {
        v.commit_import(
            &bundle,
            &ImportOptions {
                dry_run: a.dry_run,
                skip_duplicates: !a.keep_duplicates,
                target_folder: None,
            },
        )
    })?;
    u.close()?;
    let human = format!(
        "{}: {} item(s) created, {} folder(s) created, {} duplicate(s) skipped, {} invalid, \
         {} record(s) skipped by the parser, {} warning(s)",
        if report.dry_run {
            "dry run"
        } else {
            "imported"
        },
        report.created,
        report.folders_created,
        report.duplicates.len(),
        report.invalid.len(),
        bundle.skipped.len(),
        bundle.warnings.len(),
    );
    ctx.out.emit(
        &human,
        &json!({
            "dry_run": report.dry_run,
            "created": report.created,
            "folders_created": report.folders_created,
            "duplicates": report.duplicates.len(),
            "invalid": report.invalid.len(),
            "skipped": bundle.skipped.len(),
            "warnings": bundle.warnings.len(),
        }),
    )
}
