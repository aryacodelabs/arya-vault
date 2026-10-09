//! docs/14 §4.4: import and export. Files are read and written by Dart; the core sees bytes,
//! bounded by the import limits (docs/13).

use arya_vault_generator::{OsRandom, RandomSource};
use arya_vault_vault as cv;
use zeroize::{Zeroize, Zeroizing};

use super::dto::{
    AcknowledgePlaintextRisk, AppError, ImportFormat, ImportOptions, ImportPreview, ImportResult,
    ImportWarning, SkippedRecord,
};
use crate::convert::{hex_id, opt_id};
use crate::error::ApiResult;
use crate::host::{self, Host, Preview, secret_text};

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn source_name(s: cv::ImportSource) -> &'static str {
    match s {
        cv::ImportSource::GenericCsv => "genericCsv",
        cv::ImportSource::ChromeCsv => "chromeCsv",
        cv::ImportSource::FirefoxCsv => "firefoxCsv",
        cv::ImportSource::SafariCsv => "safariCsv",
        cv::ImportSource::Bitwarden => "bitwarden",
        cv::ImportSource::AryaVault => "aryavault",
    }
}

fn skip_text(r: cv::SkipReason) -> String {
    match r {
        cv::SkipReason::Empty => "the record is empty".to_owned(),
        cv::SkipReason::UnknownType(_) => "unknown item type".to_owned(),
        cv::SkipReason::InTrash => "the item is in the source's trash".to_owned(),
        cv::SkipReason::FieldTooLarge(f) => format!("the {f} value is too large"),
        cv::SkipReason::TooMany(what) => format!("too many {what}"),
        cv::SkipReason::Invalid(why) => why.to_owned(),
    }
}

fn warning_text(k: cv::WarningKind) -> String {
    use cv::WarningKind as W;
    match k {
        W::IgnoredColumn(i) => format!("column {i} was not understood and was ignored"),
        W::ExtraCells => "extra cells in a row were ignored".to_owned(),
        W::AttachmentsIgnored => "attachments are not imported".to_owned(),
        W::UnsupportedCustomField(_) => "an unsupported custom field was ignored".to_owned(),
        W::InvalidFolder => "an unusable folder name was dropped".to_owned(),
        W::TagDropped => "an unusable tag was dropped".to_owned(),
        W::ValueIgnored(what) => format!("the {what} value was ignored"),
        W::TitleDerived => "no title in the source; one was derived".to_owned(),
    }
}

fn token() -> ApiResult<String> {
    let mut b = [0u8; 16];
    OsRandom
        .fill_bytes(&mut b)
        .map_err(|_| AppError::internal())?;
    let t = hex_id(&b);
    b.zeroize();
    Ok(t)
}

/// `importPreview({format, file})`: parses the file and reports what a commit would do. Nothing
/// is written. The parsed file is held in memory under `preview_token` until the next preview,
/// the commit, or `lock`.
///
/// # Errors
/// `locked`, `limitReached` (file too large), `validation` (`file`: unreadable).
pub fn import_preview(format: ImportFormat, mut file: Vec<u8>) -> Result<ImportPreview, AppError> {
    let r = host::call(|h| preview_in(h, format, &file));
    file.zeroize();
    r
}

fn preview_in(h: &mut Host, format: ImportFormat, file: &[u8]) -> ApiResult<ImportPreview> {
    let limits = cv::ImportLimits::default();
    let bundle = match format {
        ImportFormat::Csv => cv::parse_csv(file, &limits)?,
        ImportFormat::BitwardenJson => cv::parse_bitwarden_json(file, &limits)?,
    };
    let dry = cv::ImportOptions {
        dry_run: true,
        ..cv::ImportOptions::default()
    };
    let report = h.vault(|v| v.commit_import(&bundle, &dry))?;
    let token = token()?;
    let preview = ImportPreview {
        format: source_name(bundle.source).to_owned(),
        item_count: count(report.created),
        folder_count: count(report.folders_created),
        duplicates: count(report.duplicates.len()),
        warnings: bundle
            .warnings
            .iter()
            .map(|w| ImportWarning {
                record: count(w.record),
                message: warning_text(w.kind),
            })
            .collect(),
        skipped: bundle
            .skipped
            .iter()
            .map(|s| SkippedRecord {
                record: count(s.record),
                reason: skip_text(s.reason),
            })
            .collect(),
        preview_token: token.clone(),
    };
    h.preview = Some(Preview { token, bundle });
    Ok(preview)
}

fn result_of(r: &cv::ImportReport, skipped: usize) -> ImportResult {
    ImportResult {
        created: count(r.created),
        folders_created: count(r.folders_created),
        duplicates: count(r.duplicates.len()),
        invalid: count(r.invalid.len()),
        skipped: count(skipped),
    }
}

fn core_options(o: &ImportOptions) -> ApiResult<cv::ImportOptions> {
    Ok(cv::ImportOptions {
        dry_run: false,
        skip_duplicates: o.skip_duplicates,
        target_folder: opt_id(o.target_folder.as_deref(), "targetFolder")?,
    })
}

/// `importCommit({previewToken, options})`: applies the previewed file in one transaction.
///
/// # Errors
/// `locked`, `validation` (`previewToken`: unknown or expired), `limitReached` (item limit).
pub fn import_commit(
    preview_token: String,
    options: ImportOptions,
) -> Result<ImportResult, AppError> {
    let opts = core_options(&options)?;
    host::call(|h| {
        let Some(p) = h.preview.take() else {
            return Err(AppError::validation("previewToken", "no such preview"));
        };
        if p.token != preview_token {
            h.preview = Some(p);
            return Err(AppError::validation("previewToken", "no such preview"));
        }
        match h.vault(|v| v.commit_import(&p.bundle, &opts)) {
            Ok(r) => Ok(result_of(&r, p.bundle.skipped.len())),
            Err(e) => {
                // Keep the preview: the user can fix the cause (a full vault) and retry.
                h.preview = Some(p);
                Err(e)
            }
        }
    })
}

/// `exportCsv({AcknowledgePlaintextRisk token})`: every visible login in clear text.
///
/// # Errors
/// `locked`, `validation` (`token`: the risk was not acknowledged).
pub fn export_csv(token: AcknowledgePlaintextRisk) -> Result<Vec<u8>, AppError> {
    if !token.acknowledged {
        return Err(AppError::validation(
            "token",
            "the plaintext risk must be acknowledged",
        ));
    }
    host::call(|h| {
        let ack = cv::PlaintextRiskAcknowledged::acknowledge_plaintext_risk();
        let e = h.vault(|v| v.export_csv(ack))?;
        Ok(e.bytes.to_vec())
    })
}

/// `exportEncrypted({exportPassword})`: a password-protected AryaVault export (docs/13), without
/// history. The KDF cost is the one chosen for the vault.
///
/// # Errors
/// `locked`, `validation` (empty password).
pub fn export_encrypted(export_password: Vec<u8>) -> Result<Vec<u8>, AppError> {
    let pw = secret_text(Zeroizing::new(export_password), "exportPassword")?;
    if pw.is_empty() {
        return Err(AppError::validation(
            "exportPassword",
            "the password is empty",
        ));
    }
    host::call(|h| {
        let cost = h.profile.params()?;
        h.vault(|v| v.export_aryavault(&pw, &cost, false))
    })
}

/// `importEncrypted({file, exportPassword})`: decrypts an AryaVault export and imports it,
/// skipping duplicates.
///
/// # Errors
/// `locked`, `wrongCredentials` (wrong password or damaged file), `unsupportedFormat`,
/// `validation` (`file`).
pub fn import_encrypted(file: Vec<u8>, export_password: Vec<u8>) -> Result<ImportResult, AppError> {
    let export_password = Zeroizing::new(export_password);
    let pw = secret_text(export_password, "exportPassword")?;
    host::call(|h| {
        let bundle = cv::parse_aryavault(&file, &pw, &cv::ImportLimits::default())?;
        let r = h.vault(|v| v.commit_import(&bundle, &cv::ImportOptions::default()))?;
        Ok(result_of(&r, bundle.skipped.len()))
    })
}
