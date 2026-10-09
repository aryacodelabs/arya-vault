//! The DTOs of docs/14 §3 and the error of §2.
//!
//! Plain data, no behaviour (the conversions live in `crate::convert`). **List and summary types
//! have no secret-bearing field by construction**: [`ItemSummary`] and [`ItemView`] are separate
//! structs that never had a password, TOTP seed, card number, CVV, PIN or note body to trim.
//! Types that do carry secrets ([`NewItem`], [`RecoveryKeyResult`], [`AcknowledgePlaintextRisk`]
//! excepted) have a redacted `Debug`.

use std::collections::{HashMap, HashSet};

// ------------------------------------------------------------------------------------ errors

/// `AppErrorCode` (docs/14 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppErrorCode {
    /// Wrong master password or recovery key.
    WrongCredentials,
    /// Typo or bad checksum in a recovery key.
    RecoveryKeyMalformed,
    /// The master password fails the policy; `message` carries the reasons.
    WeakPassword,
    /// The operation requires an unlocked session.
    Locked,
    /// Item or folder id unknown.
    NotFound,
    /// Input rejected; `field` names the field.
    Validation,
    /// A field, item or size limit was reached.
    LimitReached,
    /// Authentication or integrity failure of vault data.
    CorruptVault,
    /// Newer format than this app understands.
    UnsupportedFormat,
    /// Quick unlock cannot be used; fall back to the password.
    QuickUnlockUnavailable,
    /// The target already exists.
    AlreadyExists,
    /// Filesystem failure.
    Io,
    /// Resource busy (or a local failure delay is running).
    Busy,
    /// Anything else; opaque.
    Internal,
}

/// `AppError { code, message, field }`: what every call can fail with. `message` is safe to show
/// or log and never contains a secret.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppError {
    /// The machine-readable class.
    pub code: AppErrorCode,
    /// Safe, human-readable text.
    pub message: String,
    /// The offending input, for `validation`.
    pub field: Option<String>,
}

impl core::fmt::Display for AppError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

// ----------------------------------------------------------------------------------- lifecycle

/// How expensive the Argon2id parameters of a new wrap are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KdfProfile {
    /// The floor (64 MiB, t = 3, p = 1).
    Low,
    /// About 0.75 s on this device.
    Default,
    /// About 1.5 s on this device.
    High,
}

/// `QuickUnlockStatus.kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickUnlockKind {
    /// No provider.
    None,
    /// Windows Hello.
    WindowsHello,
    /// Touch ID.
    TouchId,
    /// Face ID.
    FaceId,
    /// Android biometric prompt.
    Biometric,
    /// An OS keyring.
    OsKeyring,
}

/// `QuickUnlockStatus { supported, enabled, kind }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuickUnlockStatus {
    /// The device can do quick unlock now.
    pub supported: bool,
    /// A valid policy record exists for this vault.
    pub enabled: bool,
    /// The provider kind.
    pub kind: QuickUnlockKind,
}

/// `VaultStatus`; readable while locked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultStatus {
    /// A vault exists.
    pub exists: bool,
    /// No keys are in memory.
    pub locked: bool,
    /// The recovery key was confirmed.
    pub onboarding_complete: bool,
    /// Format version of the vault header (0 if none).
    pub format_version: u32,
    /// Quick-unlock state.
    pub quick_unlock: QuickUnlockStatus,
}

/// `RecoveryKeyResult { recoveryKey, groups }`. The text is UTF-8 bytes, shown once.
///
/// `challenge` is an addition (A01 spec question 1): the zero-based groups the user must re-enter.
#[derive(Clone, PartialEq, Eq)]
pub struct RecoveryKeyResult {
    /// The recovery key, `XXXXX-...-CC`.
    pub recovery_key: Vec<u8>,
    /// Number of groups in the displayed key.
    pub groups: u32,
    /// Which groups `confirmRecoveryKey` will ask for.
    pub challenge: Vec<u32>,
}

impl core::fmt::Debug for RecoveryKeyResult {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RecoveryKeyResult")
            .field("recovery_key", &"<redacted>")
            .field("groups", &self.groups)
            .field("challenge", &self.challenge)
            .finish()
    }
}

/// One re-entered recovery-key group.
#[derive(Clone, PartialEq, Eq)]
pub struct GroupAnswer {
    /// Zero-based group index.
    pub index: u32,
    /// What the user typed.
    pub text: String,
}

impl core::fmt::Debug for GroupAnswer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupAnswer")
            .field("index", &self.index)
            .field("text", &"<redacted>")
            .finish()
    }
}

// ----------------------------------------------------------------------------------- vault data

/// Item type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ItemType {
    /// Login.
    Login,
    /// Secure note.
    Note,
    /// Payment card.
    Card,
    /// Identity.
    Identity,
}

/// Standard fields. Names follow docs/14 §3 where it lists them (`card*`); the rest are the core
/// names (spec question 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StdField {
    /// Title.
    Title,
    /// Login username.
    Username,
    /// Login password (secret).
    Password,
    /// TOTP seed (secret).
    TotpSeed,
    /// Notes.
    Notes,
    /// Note body (secret, searchable).
    Body,
    /// Card holder.
    CardHolder,
    /// Card number (secret).
    CardNumber,
    /// Card expiry.
    CardExpiry,
    /// Card CVV (secret).
    CardCvv,
    /// Card PIN (secret).
    CardPin,
    /// Identity first name.
    FirstName,
    /// Identity middle name.
    MiddleName,
    /// Identity last name.
    LastName,
    /// Identity email.
    Email,
    /// Identity phone.
    Phone,
    /// Identity address.
    Address,
    /// Identity document numbers (secret).
    Ids,
}

/// Kind of a custom field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomKind {
    /// Text.
    Text,
    /// Hidden (revealed only on request).
    Hidden,
    /// URL.
    Url,
    /// Date.
    Date,
}

/// A row in a list, search or trash view. **No secret field exists on this type.**
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    /// 32 lowercase hex characters.
    pub id: String,
    /// Item type.
    pub item_type: ItemType,
    /// Title.
    pub title: String,
    /// Username (login), holder (card), name (identity); empty for notes (a note's first line
    /// is body text, which is secret: spec question 3).
    pub subtitle: String,
    /// Favorite flag.
    pub favorite: bool,
    /// Containing folder.
    pub folder_id: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Last change, ms since the Unix epoch.
    pub updated_at: i64,
    /// A TOTP seed is set.
    pub has_totp: bool,
    /// The item is in the trash.
    pub deleted: bool,
}

/// `UrlView { id, url }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlView {
    /// Element id.
    pub id: String,
    /// The URL.
    pub url: String,
}

/// `CustomView`. `value_if_not_hidden` is `None` for hidden fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomView {
    /// Element id.
    pub id: String,
    /// Label.
    pub label: String,
    /// Kind.
    pub kind: CustomKind,
    /// The value, unless hidden.
    pub value_if_not_hidden: Option<String>,
}

/// Everything about one item except its secrets. **No secret field exists on this type**: `fields`
/// holds non-secret standard fields only and `secret_fields_present` is presence, not value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemView {
    /// Item id.
    pub id: String,
    /// Item type.
    pub item_type: ItemType,
    /// Title.
    pub title: String,
    /// Containing folder.
    pub folder_id: Option<String>,
    /// Favorite flag.
    pub favorite: bool,
    /// Tags.
    pub tags: Vec<String>,
    /// URLs.
    pub urls: Vec<UrlView>,
    /// Custom fields.
    pub custom: Vec<CustomView>,
    /// Non-secret standard fields that have a value.
    pub fields: HashMap<StdField, Option<String>>,
    /// Secret standard fields that have a value.
    pub secret_fields_present: HashSet<StdField>,
    /// Creation time, ms since the Unix epoch (0 if unknown).
    pub created_at: i64,
    /// Last change, ms since the Unix epoch.
    pub updated_at: i64,
    /// Some field has concurrent versions (docs/06 §6).
    pub other_versions: bool,
}

/// A custom field of a new item.
#[derive(Clone, PartialEq, Eq)]
pub struct NewCustom {
    /// Kind.
    pub kind: CustomKind,
    /// Label.
    pub label: String,
    /// Value.
    pub value: String,
}

impl core::fmt::Debug for NewCustom {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewCustom")
            .field("kind", &self.kind)
            .field("label", &self.label)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// `NewItem`. Its `fields` can hold secrets: `Debug` is redacted.
#[derive(Clone, PartialEq, Eq)]
pub struct NewItem {
    /// Item type.
    pub item_type: ItemType,
    /// Title.
    pub title: String,
    /// Standard fields (the title is `title`, not an entry here).
    pub fields: HashMap<StdField, String>,
    /// URLs.
    pub urls: Vec<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Folder.
    pub folder_id: Option<String>,
    /// Custom fields.
    pub custom: Vec<NewCustom>,
}

impl core::fmt::Debug for NewItem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NewItem")
            .field("item_type", &self.item_type)
            .field("title", &self.title)
            .field("fields", &self.fields.len())
            .field("urls", &self.urls.len())
            .field("tags", &self.tags.len())
            .field("folder_id", &self.folder_id)
            .field("custom", &self.custom.len())
            .finish()
    }
}

/// `ListFilter`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListFilter {
    /// Only these types (all if `None`).
    pub types: Option<Vec<ItemType>>,
    /// Only this folder.
    pub folder_id: Option<String>,
    /// Only this tag.
    pub tag: Option<String>,
    /// Only favorites.
    pub favorites_only: bool,
    /// Include items in the trash (marked `deleted`).
    pub include_trash: bool,
}

/// `Page { offset, limit }`; `limit` is at most [`MAX_PAGE_LIMIT`](crate::MAX_PAGE_LIMIT).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Page {
    /// Rows to skip.
    pub offset: u32,
    /// Rows to return.
    pub limit: u32,
}

/// `SearchQuery`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    /// Free text (prefix matching; empty = list).
    pub text: String,
    /// Filter.
    pub filter: ListFilter,
    /// Page.
    pub page: Page,
}

/// `Folder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// Folder id.
    pub id: String,
    /// Name.
    pub name: String,
    /// Parent folder.
    pub parent_id: Option<String>,
}

/// `TrashEntry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashEntry {
    /// The row.
    pub summary: ItemSummary,
    /// When it was deleted (ms; 0 if unknown).
    pub deleted_at: i64,
    /// When it will be purged (ms).
    pub purges_at: i64,
}

/// `VersionInfo`. Values are fetched with `revealVersion`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// The field.
    pub field: StdField,
    /// Identifies the version: the packed hybrid logical clock (milliseconds in the upper bits,
    /// a 16-bit counter below). Pass it back to `revealVersion` / `restoreVersion`.
    pub hlc_ms: i64,
    /// When the version was written, ms since the Unix epoch.
    pub at_ms: i64,
    /// Device name (not recorded yet; always `None`).
    pub device_name: Option<String>,
    /// This is the current version.
    pub is_current: bool,
    /// Concurrent with the current version.
    pub concurrent: bool,
}

// -------------------------------------------------------------------------------- generator etc.

/// `PasswordOptions` (mirrors the generator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordOptions {
    /// Length.
    pub length: u32,
    /// Lowercase letters.
    pub lower: bool,
    /// Uppercase letters.
    pub upper: bool,
    /// Digits.
    pub digits: bool,
    /// Symbols.
    pub symbols: bool,
    /// Symbol alphabet.
    pub symbol_set: String,
    /// Exclude look-alike characters.
    pub exclude_ambiguous: bool,
    /// At least one of each enabled class.
    pub require_each_class: bool,
}

/// `PassphraseOptions` (mirrors the generator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassphraseOptions {
    /// Number of words.
    pub word_count: u32,
    /// Separator.
    pub separator: String,
    /// Capitalise words.
    pub capitalize: bool,
    /// Append a number.
    pub number_suffix: bool,
}

/// The `options` argument of `entropyBits`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntropyOptions {
    /// For a password.
    Password(PasswordOptions),
    /// For a passphrase.
    Passphrase(PassphraseOptions),
}

/// `Strength`. Contains no part of the password.
#[derive(Debug, Clone, PartialEq)]
pub struct Strength {
    /// 0 (trivial) to 4 (strong).
    pub score: u8,
    /// log10 of the estimated guesses.
    pub guesses_log10: f64,
    /// Static warnings and suggestions.
    pub feedback: Vec<String>,
}

/// `PolicyResult` of `checkMasterPassword`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyResult {
    /// The password meets the policy.
    pub acceptable: bool,
    /// One static text per violated rule.
    pub reasons: Vec<String>,
}

/// `HealthReport.reused[]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReuseGroup {
    /// Items sharing a password.
    pub item_ids: Vec<String>,
}

/// `HealthReport.weak[]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakPassword {
    /// The item.
    pub item_id: String,
    /// zxcvbn score.
    pub score: u8,
}

/// `HealthReport.old[]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldPassword {
    /// The item.
    pub item_id: String,
    /// Days since the password was set.
    pub age_days: u32,
}

/// `HealthReport`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthReport {
    /// Reused passwords.
    pub reused: Vec<ReuseGroup>,
    /// Weak passwords.
    pub weak: Vec<WeakPassword>,
    /// Old passwords.
    pub old: Vec<OldPassword>,
}

// ----------------------------------------------------------------------------- import / export

/// The unencrypted formats `importPreview` reads (the encrypted AryaVault export has its own call).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportFormat {
    /// CSV (generic, Chrome, Firefox, Safari shapes are detected).
    Csv,
    /// Unencrypted Bitwarden JSON.
    BitwardenJson,
}

/// `ImportPreview.skipped[]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedRecord {
    /// 1-based record number.
    pub record: u32,
    /// Static reason text.
    pub reason: String,
}

/// An import observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWarning {
    /// 1-based record number (0 = whole file).
    pub record: u32,
    /// Static text; never file content.
    pub message: String,
}

/// `ImportPreview`. `preview_token` is an addition: `importCommit` needs something to name the
/// parsed file (spec question 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPreview {
    /// Which parser recognised the file.
    pub format: String,
    /// Items that would be created.
    pub item_count: u32,
    /// Folders that would be created.
    pub folder_count: u32,
    /// Items skipped as duplicates.
    pub duplicates: u32,
    /// Warnings.
    pub warnings: Vec<ImportWarning>,
    /// Records the parser skipped.
    pub skipped: Vec<SkippedRecord>,
    /// Names the parsed file for `importCommit`; valid until the next preview, the commit or lock.
    pub preview_token: String,
}

/// `importCommit` options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOptions {
    /// Skip duplicates.
    pub skip_duplicates: bool,
    /// Folder for items without one.
    pub target_folder: Option<String>,
}

/// What an import did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportResult {
    /// Items created.
    pub created: u32,
    /// Folders created.
    pub folders_created: u32,
    /// Duplicates skipped.
    pub duplicates: u32,
    /// Items rejected by validation.
    pub invalid: u32,
    /// Records the parser skipped.
    pub skipped: u32,
}

/// The user has been told that a CSV export is unencrypted and confirmed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcknowledgePlaintextRisk {
    /// Must be `true`.
    pub acknowledged: bool,
}

// ------------------------------------------------------------------------------------- settings

/// Vault retention (core `VaultConfig`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultConfig {
    /// Versions kept for password / body / TOTP fields.
    pub history_sensitive: u32,
    /// Versions kept for other fields.
    pub history_other: u32,
    /// Days in the trash before purge.
    pub trash_days: u32,
    /// Days a purged item's tombstone is kept.
    pub tombstone_days: u32,
}

/// `AppSettings`. The core clamps every value to its allowed range on `setSettings`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSettings {
    /// Auto-lock after this many idle minutes (1-60).
    pub auto_lock_minutes: u32,
    /// Lock when the device sleeps.
    pub lock_on_sleep: bool,
    /// Lock when the screen locks.
    pub lock_on_screen_lock: bool,
    /// Clear the clipboard after this many seconds (5-120).
    pub clipboard_clear_seconds: u32,
    /// Block screen capture of vault screens.
    pub block_screen_capture: bool,
    /// Hide a revealed secret after this many seconds (1-15, SEC-H03).
    pub reveal_hide_seconds: u32,
    /// Retention.
    pub vault_config: VaultConfig,
}

// ---------------------------------------------------------------------------------- diagnostics

/// One pinned SQLCipher / SQLite setting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedSetting {
    /// Key.
    pub key: String,
    /// Value, if recorded.
    pub value: Option<String>,
}

/// `InfoDto`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoDto {
    /// The core's crate version.
    pub core_version: String,
    /// `API_VERSION`; the Dart side asserts equality at startup (docs/14 §7).
    pub api_version: u32,
    /// Header format version (0 if no vault).
    pub format_version: u32,
    /// Database schema version (0 while locked).
    pub schema_version: u32,
    /// The pinned settings (empty while locked).
    pub sqlcipher_settings: Vec<PinnedSetting>,
}
