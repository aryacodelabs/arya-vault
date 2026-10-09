//! Item types, standard fields, register keys and limits (docs/05 sections 2, 8, 10).

use arya_vault_storage::Id;

use crate::error::{Result, VaultError};

/// Maximum size of one field value (docs/05 section 10).
pub const MAX_FIELD_BYTES: usize = 64 * 1024;
/// Maximum size of a note body.
pub const MAX_BODY_BYTES: usize = 1 << 20;
/// Maximum custom fields per item.
pub const MAX_CUSTOM_FIELDS: usize = 100;
/// Maximum tags per item.
pub const MAX_TAGS: usize = 50;
/// Maximum length of one tag, in bytes.
pub const MAX_TAG_BYTES: usize = 64;
/// Maximum folder name length, in bytes.
pub const MAX_FOLDER_NAME_BYTES: usize = 256;
/// Soft item limit (target); see [`Vault::at_soft_item_limit`](crate::Vault::at_soft_item_limit).
pub const SOFT_ITEM_LIMIT: u64 = 20_000;
/// Hard item limit: creating more fails with [`VaultError::LimitExceeded`].
pub const HARD_ITEM_LIMIT: u64 = 100_000;

/// Reserved register keys (never collide with standard field keys).
pub(crate) mod keys {
    pub const TYPE: &str = "type";
    pub const FOLDER: &str = "folder_id";
    pub const FAVORITE: &str = "favorite";
    pub const CREATED_AT: &str = "created_at";
    pub const DELETED: &str = "deleted";
    pub const DELETED_AT: &str = "deleted_at";
    pub const SCHEMA_VERSION: &str = "schema_version";
    pub const TAG_PREFIX: &str = "tags.";
    pub const URL_PREFIX: &str = "urls.";
    pub const CUSTOM_PREFIX: &str = "custom.";
    /// Item schema version written to the `schema_version` register.
    pub const ITEM_SCHEMA: i64 = 1;
}

/// The kind of an item (doc 05 section 2; `wifi`/`custom` are P2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ItemType {
    /// Website/app login.
    Login,
    /// Secure Markdown note.
    Note,
    /// Payment card.
    Card,
    /// Identity record.
    Identity,
}

impl ItemType {
    /// Stored name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            ItemType::Login => "login",
            ItemType::Note => "note",
            ItemType::Card => "card",
            ItemType::Identity => "identity",
        }
    }

    /// Parse a stored name.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "login" => ItemType::Login,
            "note" => ItemType::Note,
            "card" => ItemType::Card,
            "identity" => ItemType::Identity,
            _ => return None,
        })
    }
}

/// Standard fields (the `key` of the register is [`StdField::key`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StdField {
    /// Title (all types).
    Title,
    /// Login username.
    Username,
    /// Login password. **Secret.**
    Password,
    /// Login TOTP seed. **Secret.**
    TotpSeed,
    /// Free-form notes (login, card, identity).
    Notes,
    /// Note body, Markdown (note). **Secret** (reveal-gated) but searchable (US-04).
    Body,
    /// Card holder.
    Holder,
    /// Card number. **Secret.**
    Number,
    /// Card expiry.
    Expiry,
    /// Card CVV. **Secret.**
    Cvv,
    /// Card PIN. **Secret.**
    Pin,
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
    /// Identity document numbers. **Secret** (conservative reading).
    Ids,
}

impl StdField {
    /// Register key.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            StdField::Title => "title",
            StdField::Username => "username",
            StdField::Password => "password",
            StdField::TotpSeed => "totp_seed",
            StdField::Notes => "notes",
            StdField::Body => "body",
            StdField::Holder => "holder",
            StdField::Number => "number",
            StdField::Expiry => "expiry",
            StdField::Cvv => "cvv",
            StdField::Pin => "pin",
            StdField::FirstName => "first_name",
            StdField::MiddleName => "middle_name",
            StdField::LastName => "last_name",
            StdField::Email => "email",
            StdField::Phone => "phone",
            StdField::Address => "address",
            StdField::Ids => "ids",
        }
    }

    /// Parse a register key.
    #[must_use]
    pub fn from_key(k: &str) -> Option<Self> {
        ALL_FIELDS.iter().copied().find(|f| f.key() == k)
    }

    /// Whether the value is returned only through `reveal`.
    #[must_use]
    pub fn is_secret(self) -> bool {
        matches!(
            self,
            StdField::Password
                | StdField::TotpSeed
                | StdField::Body
                | StdField::Number
                | StdField::Cvv
                | StdField::Pin
                | StdField::Ids
        )
    }

    /// Whether the field exists for `t`.
    #[must_use]
    pub fn allowed_for(self, t: ItemType) -> bool {
        use StdField::*;
        match t {
            ItemType::Login => matches!(self, Title | Username | Password | TotpSeed | Notes),
            ItemType::Note => matches!(self, Title | Body),
            ItemType::Card => matches!(self, Title | Holder | Number | Expiry | Cvv | Pin | Notes),
            ItemType::Identity => {
                matches!(
                    self,
                    Title
                        | FirstName
                        | MiddleName
                        | LastName
                        | Email
                        | Phone
                        | Address
                        | Ids
                        | Notes
                )
            }
        }
    }

    /// Size limit for this field's value, in bytes.
    #[must_use]
    pub fn max_bytes(self) -> usize {
        if self == StdField::Body {
            MAX_BODY_BYTES
        } else {
            MAX_FIELD_BYTES
        }
    }

    /// History versions retained by default: 20 for sensitive fields, 5 otherwise (doc 05 section 8).
    #[must_use]
    pub fn is_sensitive_history(self) -> bool {
        matches!(
            self,
            StdField::Password | StdField::Body | StdField::TotpSeed
        )
    }
}

pub(crate) const ALL_FIELDS: [StdField; 18] = [
    StdField::Title,
    StdField::Username,
    StdField::Password,
    StdField::TotpSeed,
    StdField::Notes,
    StdField::Body,
    StdField::Holder,
    StdField::Number,
    StdField::Expiry,
    StdField::Cvv,
    StdField::Pin,
    StdField::FirstName,
    StdField::MiddleName,
    StdField::LastName,
    StdField::Email,
    StdField::Phone,
    StdField::Address,
    StdField::Ids,
];

/// Kind of a custom field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomKind {
    /// Plain text.
    Text,
    /// Hidden (secret) text; returned only through `reveal_custom`.
    Hidden,
    /// URL.
    Url,
    /// Date (free text, e.g. ISO 8601).
    Date,
}

impl CustomKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            CustomKind::Text => "text",
            CustomKind::Hidden => "hidden",
            CustomKind::Url => "url",
            CustomKind::Date => "date",
        }
    }
    pub(crate) fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "text" => CustomKind::Text,
            "hidden" => CustomKind::Hidden,
            "url" => CustomKind::Url,
            "date" => CustomKind::Date,
            _ => return None,
        })
    }
}

/// Identifier of a custom field or URL element (32 lowercase hex characters, a UUIDv7).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ElementId(pub(crate) String);

impl ElementId {
    /// The hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Parse from a string.
    ///
    /// # Errors
    /// [`VaultError::InvalidValue`] if it is not 32 lowercase hex digits.
    pub fn parse(s: &str) -> Result<Self> {
        if s.len() == 32
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(s.to_owned()))
        } else {
            Err(VaultError::InvalidValue("element id"))
        }
    }

    pub(crate) fn from_id(id: &Id) -> Self {
        Self(id.iter().map(|b| format!("{b:02x}")).collect())
    }
}

/// Reference to a field for history/restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldRef {
    /// A standard field.
    Std(StdField),
    /// The value of a custom field.
    CustomValue(ElementId),
}

impl FieldRef {
    pub(crate) fn key(&self) -> String {
        match self {
            FieldRef::Std(f) => f.key().to_owned(),
            FieldRef::CustomValue(id) => format!("{}{}.value", keys::CUSTOM_PREFIX, id.0),
        }
    }
}

/// Tunable retention (doc 05 sections 7-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultConfig {
    /// History versions kept for password/body/TOTP fields.
    pub history_sensitive: usize,
    /// History versions kept for every other field.
    pub history_other: usize,
    /// Days an item stays recoverable in the trash before purge.
    pub trash_days: u64,
    /// Days a purged item's tombstone is kept (dropping it is compaction's job, M4).
    pub tombstone_days: u64,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            history_sensitive: 20,
            history_other: 5,
            trash_days: 30,
            tombstone_days: 180,
        }
    }
}

pub(crate) const DAY_MS: u64 = 24 * 60 * 60 * 1000;

pub(crate) fn check_text(s: &str, max: usize) -> Result<()> {
    if s.len() > max {
        return Err(VaultError::LimitExceeded("field value too large"));
    }
    Ok(())
}

pub(crate) fn check_tag(t: &str) -> Result<()> {
    if t.is_empty() || t.len() > MAX_TAG_BYTES || t.chars().any(char::is_control) || t != t.trim() {
        return Err(VaultError::InvalidValue("tag"));
    }
    Ok(())
}
