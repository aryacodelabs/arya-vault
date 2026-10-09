//! Items, folders, history and search over the encrypted store (docs/05,
//! docs/06 sections 4-5). The local vault only: no network and no sync engine;
//! mutations are *recorded* in `local_op` and the data structures are already
//! merge-correct (LWW registers on hybrid logical clocks).
//!
//! * [`Vault`] is the API; secrets are returned only by `reveal*`.
//! * [`Register`], [`FieldState`] and [`concurrent_losers`] are the pure merge model.
//! * [`HlcClock`] is the hybrid logical clock.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

mod engine;
mod error;
mod fault;
mod health;
mod history;
mod hlc;
mod interchange;
mod model;
mod query;
mod register;
mod value;
mod vault;
mod views;

#[cfg(test)]
mod tests;

pub use error::{Result, VaultError};
pub use hlc::{
    Clock, Hlc, HlcClock, HlcError, MAX_PT, ManualClock, SKEW_CORRUPT_MS, SKEW_FLAG_MS, Skew,
    SystemClock,
};
pub use interchange::{
    ContainerInfo, CsvExport, CsvExportReport, ImportBundle, ImportCustom, ImportError,
    ImportFolder, ImportHistory, ImportItem, ImportLimits, ImportOptions, ImportReport,
    ImportSource, ImportWarning, Importer, PlaintextRiskAcknowledged, SkipReason, SkippedRecord,
    WarningKind, parse_aryavault, parse_aryavault_payload, parse_bitwarden_json, parse_csv,
    read_container_info, read_limited,
};
pub use model::{
    CustomKind, ElementId, FieldRef, HARD_ITEM_LIMIT, ItemType, MAX_BODY_BYTES, MAX_CUSTOM_FIELDS,
    MAX_FIELD_BYTES, MAX_TAGS, SOFT_ITEM_LIMIT, StdField, VaultConfig,
};
pub use register::{FieldState, Merged, Register, concurrent_losers};
pub use vault::{MAX_SETTING_BYTES, Vault};
pub use views::{
    CustomView, Folder, ItemSummary, ItemView, ListFilter, NewItem, OldPassword, Page, ReuseGroup,
    SearchQuery, TrashEntry, UrlView, VersionInfo, WeakPassword,
};
