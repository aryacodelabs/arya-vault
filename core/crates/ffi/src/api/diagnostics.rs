//! docs/14 §4.6: diagnostics.

use super::dto::{InfoDto, PinnedSetting};
use crate::error::ApiResult;
use crate::host::{self, Host};
use crate::{API_VERSION, diag};

fn info_in(h: &mut Host) -> ApiResult<InfoDto> {
    let s = h.session()?;
    let format_version = u32::from(s.status()?.format_version);
    // The schema version and the pinned settings live in the database: only while unlocked.
    let db = s.db_info().ok();
    Ok(InfoDto {
        core_version: env!("CARGO_PKG_VERSION").to_owned(),
        api_version: API_VERSION,
        format_version,
        schema_version: db.as_ref().map_or(0, |d| d.schema_version),
        sqlcipher_settings: db.map_or_else(Vec::new, |d| {
            d.pinned_settings
                .into_iter()
                .map(|(k, v)| PinnedSetting {
                    key: k.to_owned(),
                    value: v,
                })
                .collect()
        }),
    })
}

/// `info()`: versions and the pinned cipher settings. Works while locked (with less detail).
///
/// # Errors
/// `io`, `corruptVault` for a damaged vault directory.
pub fn info() -> ApiResult<InfoDto> {
    host::call(info_in)
}

/// `exportDiagnostics()`: scrubbed JSON the user reviews before sharing. It holds versions,
/// the KDF cost and the pinned settings; no vault, device or item ids, no names, no data.
///
/// # Errors
/// `io`, `corruptVault`.
pub fn export_diagnostics() -> ApiResult<Vec<u8>> {
    host::call(|h| {
        let info = info_in(h)?;
        let header = h.session()?.header_info().ok();
        Ok(diag::to_json(&info, header.as_ref()).into_bytes())
    })
}
