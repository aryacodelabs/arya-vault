//! Import and export (docs/05 section 9, docs/13, US-09, threat 14).
//!
//! Every importer is a **pure parser**: it turns untrusted bytes into a neutral
//! [`ImportBundle`] and never touches a vault. [`Vault::commit_import`](crate::Vault::commit_import)
//! then applies a bundle in one transaction, with a dry-run (preview) mode and duplicate
//! detection. A structural problem in a file (bad encoding, wrong shape, limits exceeded)
//! is an [`ImportError`] and nothing is applied; a bad *record* is skipped and reported
//! (`ImportBundle::skipped`) and the rest still import.
//!
//! All parsers are bounded (SEC-Y05): the file size, record count, field length, nesting
//! depth and total number of values are limited by [`ImportLimits`] and checked before
//! large allocations; no input can cause a panic. Warnings and errors never contain field
//! values, only record numbers and static descriptions.
//!
//! Importers: [`parse_csv`] (generic, Chrome/Edge, Firefox, Safari), [`parse_bitwarden_json`],
//! [`parse_aryavault`]. Exporters: [`Vault::export_csv`](crate::Vault::export_csv) (plaintext,
//! needs [`PlaintextRiskAcknowledged`]) and [`Vault::export_aryavault`](crate::Vault::export_aryavault)
//! (password-protected; format in `docs/13-export-format.md`). KeePass (KDBX) is not
//! implemented; see [`Importer`] for the hook.

use std::io::Read;

use thiserror::Error;
use zeroize::Zeroizing;

use crate::error::VaultError;
use crate::model::{
    CustomKind, ItemType, MAX_BODY_BYTES, MAX_CUSTOM_FIELDS, MAX_FIELD_BYTES, MAX_TAGS, StdField,
    check_tag,
};

pub(crate) mod aryavault;
mod bitwarden;
mod commit;
mod csv_import;
mod export_csv;
mod json;
#[cfg(test)]
mod vault_tests;

pub use aryavault::{ContainerInfo, parse_aryavault, parse_aryavault_payload, read_container_info};
pub use bitwarden::parse_bitwarden_json;
pub use commit::{ImportOptions, ImportReport};
pub use csv_import::parse_csv;
pub use export_csv::{CsvExport, CsvExportReport};

/// Bounds applied to every import file. Exceeding one is a typed [`ImportError`] raised
/// before the data is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportLimits {
    /// Maximum size of the input file in bytes (default 64 MiB).
    pub max_file_bytes: u64,
    /// Maximum number of records (items) in a file (default 100,000 = the vault's hard limit).
    pub max_records: usize,
    /// Maximum length of any single string or CSV field in bytes (default 1 MiB, the largest
    /// vault limit; the per-field vault limits then apply to each record).
    pub max_field_bytes: usize,
    /// Maximum nesting depth of JSON/CBOR containers (default 16).
    pub max_depth: usize,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_file_bytes: 64 << 20,
            max_records: 100_000,
            max_field_bytes: MAX_BODY_BYTES,
            max_depth: 16,
        }
    }
}

impl ImportLimits {
    /// Budget for the total number of JSON/CBOR values in a document.
    pub(crate) fn max_json_values(&self) -> usize {
        self.max_records.saturating_mul(64).max(4096)
    }
}

/// Why an import could not even be parsed. No variant carries file contents.
#[derive(Debug, Error)]
pub enum ImportError {
    /// The file is larger than [`ImportLimits::max_file_bytes`].
    #[error("file is too large")]
    FileTooLarge,
    /// More records (or JSON/CBOR values) than the limits allow.
    #[error("too many records")]
    TooManyRecords,
    /// A field or string is longer than [`ImportLimits::max_field_bytes`].
    #[error("a field is too large")]
    FieldTooLarge,
    /// Containers are nested deeper than [`ImportLimits::max_depth`].
    #[error("nesting is too deep")]
    TooDeeplyNested,
    /// The text is not valid UTF-8/UTF-16 (CSV: UTF-8 with or without BOM, or UTF-16 with BOM).
    #[error("unsupported or invalid text encoding")]
    InvalidEncoding,
    /// The CSV could not be read.
    #[error("invalid CSV")]
    InvalidCsv,
    /// The CSV header has no column this importer understands.
    #[error("no recognized columns in CSV header")]
    NoRecognizedColumns,
    /// The JSON could not be read (syntax, duplicate keys, trailing data).
    #[error("invalid JSON")]
    InvalidJson,
    /// Valid JSON, but not the expected structure.
    #[error("unexpected file structure")]
    UnexpectedStructure,
    /// A Bitwarden *encrypted* export. Export an unencrypted `.json` from Bitwarden instead.
    #[error("encrypted Bitwarden exports are not supported; export unencrypted JSON")]
    EncryptedBitwardenUnsupported,
    /// Not an AryaVault export (wrong magic bytes).
    #[error("not an AryaVault export")]
    BadMagic,
    /// The export uses a format version this build cannot read ("update required").
    #[error("unsupported export format version {found} (supported up to {max_supported})")]
    UnsupportedFormat {
        /// Version found in the file.
        found: u16,
        /// Highest version this build supports.
        max_supported: u16,
    },
    /// The export container is damaged or malformed.
    #[error("damaged export file")]
    Malformed,
    /// The export's KDF parameters are outside the allowed range.
    #[error("export parameters are out of range or corrupted")]
    InvalidKdf,
    /// Wrong password, or the file was modified (indistinguishable by design).
    #[error("wrong password or damaged export")]
    AuthenticationFailed,
    /// Reading the input failed.
    #[error("could not read input")]
    Io,
    /// A vault operation failed while committing or exporting.
    #[error(transparent)]
    Vault(#[from] VaultError),
    /// Randomness or key-derivation failure while exporting.
    #[error("cryptographic operation failed")]
    Crypto,
}

/// Which parser produced a bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportSource {
    /// CSV with a header the generic mapper understood.
    GenericCsv,
    /// CSV in the Chrome/Edge password export shape.
    ChromeCsv,
    /// CSV in the Firefox export shape.
    FirefoxCsv,
    /// CSV in the Safari/Passwords export shape.
    SafariCsv,
    /// Unencrypted Bitwarden JSON.
    Bitwarden,
    /// AryaVault encrypted export.
    AryaVault,
}

/// A custom field of an item to import.
pub struct ImportCustom {
    /// Kind.
    pub kind: CustomKind,
    /// Label.
    pub label: String,
    /// Value.
    pub value: Zeroizing<String>,
}

/// Earlier versions of a standard field, **oldest first**, excluding the current value
/// (carried only by AryaVault exports that include history).
pub struct ImportHistory {
    /// The field.
    pub field: StdField,
    /// Previous values, oldest first.
    pub older: Vec<Zeroizing<String>>,
}

/// One item to import, in the vault's own terms.
pub struct ImportItem {
    /// Type.
    pub item_type: ItemType,
    /// Title.
    pub title: String,
    /// Standard fields other than the title.
    pub fields: Vec<(StdField, Zeroizing<String>)>,
    /// URLs (logins only).
    pub urls: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Index into [`ImportBundle::folders`].
    pub folder: Option<usize>,
    /// Favorite flag.
    pub favorite: bool,
    /// Custom fields.
    pub custom: Vec<ImportCustom>,
    /// Field history (AryaVault exports only).
    pub history: Vec<ImportHistory>,
    /// 1-based record number in the source file (for reports).
    pub source_record: usize,
}

impl core::fmt::Debug for ImportItem {
    // Field values may be secrets: show structure only.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ImportItem")
            .field("item_type", &self.item_type)
            .field(
                "fields",
                &self.fields.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            )
            .field("source_record", &self.source_record)
            .finish_non_exhaustive()
    }
}

impl ImportItem {
    /// An item with a type and title.
    #[must_use]
    pub fn new(item_type: ItemType, title: &str, source_record: usize) -> Self {
        Self {
            item_type,
            title: title.to_owned(),
            fields: Vec::new(),
            urls: Vec::new(),
            tags: Vec::new(),
            folder: None,
            favorite: false,
            custom: Vec::new(),
            history: Vec::new(),
            source_record,
        }
    }

    /// Adds a standard field (empty values are ignored).
    pub fn set(&mut self, field: StdField, value: &str) {
        if !value.is_empty() {
            self.fields.push((field, Zeroizing::new(value.to_owned())));
        }
    }

    /// Checks the item against the vault's rules (docs/05 sections 2 and 10): fields valid
    /// for the type, no duplicates, size limits, counts.
    ///
    /// # Errors
    /// The reason the record cannot be imported.
    pub fn validate(&self) -> Result<(), SkipReason> {
        let too_big = |what: &'static str| SkipReason::FieldTooLarge(what);
        if self.title.len() > StdField::Title.max_bytes() {
            return Err(too_big("title"));
        }
        for (i, (f, v)) in self.fields.iter().enumerate() {
            if !f.allowed_for(self.item_type) || *f == StdField::Title {
                return Err(SkipReason::Invalid("field not valid for item type"));
            }
            if self.fields[..i].iter().any(|(g, _)| g == f) {
                return Err(SkipReason::Invalid("duplicate field"));
            }
            if v.len() > f.max_bytes() {
                return Err(too_big(f.key()));
            }
        }
        if !self.urls.is_empty() && self.item_type != ItemType::Login {
            return Err(SkipReason::Invalid("urls on a non-login item"));
        }
        if self.urls.iter().any(|u| u.len() > MAX_FIELD_BYTES) {
            return Err(too_big("url"));
        }
        if self.tags.len() > MAX_TAGS {
            return Err(SkipReason::TooMany("tags"));
        }
        let mut tags = self.tags.clone();
        tags.sort();
        tags.dedup();
        if tags.len() != self.tags.len() || tags.iter().any(|t| check_tag(t).is_err()) {
            return Err(SkipReason::Invalid("tag"));
        }
        if self.custom.len() > MAX_CUSTOM_FIELDS {
            return Err(SkipReason::TooMany("custom fields"));
        }
        if self
            .custom
            .iter()
            .any(|c| c.label.len() > MAX_FIELD_BYTES || c.value.len() > MAX_FIELD_BYTES)
        {
            return Err(too_big("custom field"));
        }
        for h in &self.history {
            if !h.field.allowed_for(self.item_type) || h.field == StdField::Title {
                return Err(SkipReason::Invalid("history for an invalid field"));
            }
            if h.older.iter().any(|v| v.len() > h.field.max_bytes()) {
                return Err(too_big("history"));
            }
        }
        Ok(())
    }
}

/// A folder to import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportFolder {
    /// Name.
    pub name: String,
    /// Index of the parent in [`ImportBundle::folders`] (must be smaller than this folder's own index).
    pub parent: Option<usize>,
}

/// Why a record was not imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    /// The record has no content.
    Empty,
    /// An item type this importer does not know (the number is the source's type code).
    UnknownType(i64),
    /// The source marks the item as deleted/in the trash.
    InTrash,
    /// A value exceeds a vault limit (the name of the field).
    FieldTooLarge(&'static str),
    /// Too many tags/custom fields.
    TooMany(&'static str),
    /// The record is malformed (static description).
    Invalid(&'static str),
}

/// A record that was not imported, with its 1-based record number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkippedRecord {
    /// 1-based record number in the source.
    pub record: usize,
    /// Why.
    pub reason: SkipReason,
}

/// A non-fatal observation. Contains no file contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImportWarning {
    /// 1-based record number, or 0 for file-level warnings.
    pub record: usize,
    /// What happened.
    pub kind: WarningKind,
}

/// Kinds of [`ImportWarning`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    /// A CSV column was not understood and was ignored (the 0-based column index).
    IgnoredColumn(usize),
    /// A row had more cells than the header; extra cells were ignored.
    ExtraCells,
    /// Attachments are not imported.
    AttachmentsIgnored,
    /// An unsupported custom-field type was ignored (the source's type code).
    UnsupportedCustomField(i64),
    /// A folder name was unusable; the item was imported without a folder.
    InvalidFolder,
    /// A tag was unusable and dropped.
    TagDropped,
    /// A value was dropped because it was not a string/number as expected.
    ValueIgnored(&'static str),
    /// No title in the source; one was derived (from the URL host or username) or defaulted.
    TitleDerived,
}

/// The result of parsing a file: nothing here has touched a vault.
#[derive(Debug)]
pub struct ImportBundle {
    /// Which parser produced it.
    pub source: ImportSource,
    /// Items to create.
    pub items: Vec<ImportItem>,
    /// Folders referenced by items.
    pub folders: Vec<ImportFolder>,
    /// Non-fatal observations.
    pub warnings: Vec<ImportWarning>,
    /// Records that will not be imported.
    pub skipped: Vec<SkippedRecord>,
}

impl ImportBundle {
    pub(crate) fn new(source: ImportSource) -> Self {
        Self {
            source,
            items: Vec::new(),
            folders: Vec::new(),
            warnings: Vec::new(),
            skipped: Vec::new(),
        }
    }

    pub(crate) fn warn(&mut self, record: usize, kind: WarningKind) {
        self.warnings.push(ImportWarning { record, kind });
    }

    /// Adds `item` if it validates, otherwise records it as skipped. Enforces
    /// `max_records` on the number of *accepted plus skipped* records.
    pub(crate) fn push(
        &mut self,
        item: ImportItem,
        limits: &ImportLimits,
    ) -> Result<(), ImportError> {
        if self.items.len() + self.skipped.len() >= limits.max_records {
            return Err(ImportError::TooManyRecords);
        }
        match item.validate() {
            Ok(()) => self.items.push(item),
            Err(reason) => self.skipped.push(SkippedRecord {
                record: item.source_record,
                reason,
            }),
        }
        Ok(())
    }

    pub(crate) fn skip(
        &mut self,
        record: usize,
        reason: SkipReason,
        limits: &ImportLimits,
    ) -> Result<(), ImportError> {
        if self.items.len() + self.skipped.len() >= limits.max_records {
            return Err(ImportError::TooManyRecords);
        }
        self.skipped.push(SkippedRecord { record, reason });
        Ok(())
    }
}

/// Extension point for further importers (e.g. KeePass KDBX, which is not part of M1).
///
/// An implementation parses untrusted bytes into an [`ImportBundle`] under [`ImportLimits`],
/// following the same rules as the built-in parsers (bounded, typed errors, no panics, no
/// vault access, a fuzz target).
pub trait Importer {
    /// Parses `bytes`.
    ///
    /// # Errors
    /// [`ImportError`] for structural problems; bad records go to `skipped`.
    fn parse(&self, bytes: &[u8], limits: &ImportLimits) -> Result<ImportBundle, ImportError>;
}

/// Proof that the caller has shown the user the plaintext-export warning.
///
/// A plaintext export writes every password in clear text. The UI must display the warning
/// and only then construct this token; code cannot call
/// [`Vault::export_csv`](crate::Vault::export_csv) without one, and the token cannot be
/// built with a struct literal.
///
/// ```compile_fail
/// use arya_vault_vault::PlaintextRiskAcknowledged;
/// // The field is private: the token cannot be forged.
/// let _ = PlaintextRiskAcknowledged { _private: () };
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PlaintextRiskAcknowledged {
    _private: (),
}

impl PlaintextRiskAcknowledged {
    /// Records that the user was told the export is unencrypted and confirmed it.
    #[must_use]
    pub fn acknowledge_plaintext_risk() -> Self {
        Self { _private: () }
    }
}

/// Reads at most `limits.max_file_bytes` from `reader`; a longer input is
/// [`ImportError::FileTooLarge`] (at most one extra byte is read to detect that).
///
/// # Errors
/// [`ImportError::FileTooLarge`] or [`ImportError::Io`].
pub fn read_limited(
    reader: impl Read,
    limits: &ImportLimits,
) -> Result<Zeroizing<Vec<u8>>, ImportError> {
    let mut out = Zeroizing::new(Vec::new());
    reader
        .take(limits.max_file_bytes.saturating_add(1))
        .read_to_end(&mut out)
        .map_err(|_| ImportError::Io)?;
    if out.len() as u64 > limits.max_file_bytes {
        return Err(ImportError::FileTooLarge);
    }
    Ok(out)
}

pub(crate) fn check_file_size(bytes: &[u8], limits: &ImportLimits) -> Result<(), ImportError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        Err(ImportError::FileTooLarge)
    } else {
        Ok(())
    }
}

/// Normalises a source folder name; `None` if unusable (empty, too long, control characters).
pub(crate) fn clean_folder_name(name: &str) -> Option<String> {
    let n = name.trim();
    if n.is_empty()
        || n.len() > crate::model::MAX_FOLDER_NAME_BYTES
        || n.chars().any(char::is_control)
    {
        None
    } else {
        Some(n.to_owned())
    }
}

/// Finds or appends a folder to `bundle` and returns its index.
pub(crate) fn folder_index(bundle: &mut ImportBundle, name: &str) -> usize {
    if let Some(i) = bundle
        .folders
        .iter()
        .position(|f| f.parent.is_none() && f.name == name)
    {
        return i;
    }
    bundle.folders.push(ImportFolder {
        name: name.to_owned(),
        parent: None,
    });
    bundle.folders.len() - 1
}
