//! Value types returned by the [`Vault`](crate::Vault) API.
//!
//! None of these carries a secret: secret fields appear only as a presence
//! flag, and are read through the explicit `reveal*` methods.

use arya_vault_storage::Id;
use zeroize::Zeroizing;

use crate::hlc::Hlc;
use crate::model::{CustomKind, ElementId, ItemType, StdField};

/// A row in a list or search result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSummary {
    /// Item id (UUIDv7).
    pub id: Id,
    /// Item type.
    pub item_type: ItemType,
    /// Title (may be empty).
    pub title: String,
    /// Favorite flag.
    pub favorite: bool,
    /// Containing folder.
    pub folder_id: Option<Id>,
    /// Most recent change (an HLC; use for ordering, not for display).
    pub updated: Hlc,
}

/// A URL element of a login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlView {
    /// Element id.
    pub id: ElementId,
    /// The URL.
    pub url: String,
}

/// A custom field. `value` is `None` for hidden fields (use `reveal_custom`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomView {
    /// Element id.
    pub id: ElementId,
    /// Label.
    pub label: String,
    /// Kind.
    pub kind: CustomKind,
    /// Value, unless the field is hidden.
    pub value: Option<String>,
    /// Whether a value is set.
    pub has_value: bool,
}

/// Everything about one item except its secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemView {
    /// The list row.
    pub summary: ItemSummary,
    /// Wall-clock creation time, ms since the Unix epoch (display field).
    pub created_at_ms: Option<i64>,
    /// Non-secret standard fields that have a value.
    pub fields: Vec<(StdField, String)>,
    /// Secret standard fields that have a value (presence only).
    pub secret_fields: Vec<StdField>,
    /// URLs, in creation order.
    pub urls: Vec<UrlView>,
    /// Tags, sorted.
    pub tags: Vec<String>,
    /// Custom fields, in creation order.
    pub custom: Vec<CustomView>,
}

/// An entry in the trash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrashEntry {
    /// The list row (title is empty once purged).
    pub summary: ItemSummary,
    /// Wall-clock deletion time (ms), a display field.
    pub deleted_at_ms: Option<i64>,
    /// Whether the content was already purged (only the tombstone remains).
    pub purged: bool,
}

/// Which items [`Vault::list`](crate::Vault::list) returns (trash excluded).
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    /// Only this type.
    pub item_type: Option<ItemType>,
    /// Only items carrying this tag.
    pub tag: Option<String>,
    /// Only items in this folder.
    pub folder_id: Option<Id>,
    /// Only favorites.
    pub favorites_only: bool,
}

/// Pagination for lists.
#[derive(Debug, Clone, Copy)]
pub struct Page {
    /// Rows to skip.
    pub offset: usize,
    /// Maximum rows.
    pub limit: usize,
}

impl Page {
    /// Everything.
    pub const ALL: Page = Page {
        offset: 0,
        limit: usize::MAX,
    };
}

/// A full-text search.
#[derive(Debug, Clone, Default)]
pub struct SearchQuery {
    /// Words; each is matched as a prefix, all must match. Empty = no text filter.
    pub text: String,
    /// Filters (as in [`ListFilter`]).
    pub filter: ListFilter,
    /// Maximum results (0 = default of 100).
    pub limit: usize,
}

/// A stored version of a field, without its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionInfo {
    /// Version timestamp.
    pub hlc: Hlc,
    /// Authoring device.
    pub device_id: Id,
    /// The version the author saw.
    pub base_hlc: Option<Hlc>,
    /// This is the current version.
    pub current: bool,
    /// Not an ancestor of the current version (derived, doc 06 section 5.3).
    pub concurrent: bool,
    /// The version cleared the field.
    pub cleared: bool,
}

/// A group of items sharing a password. Contains ids only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReuseGroup {
    /// Items with the same password (at least two).
    pub item_ids: Vec<Id>,
}

/// A weak password finding. Contains no password.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeakPassword {
    /// The item.
    pub item_id: Id,
    /// zxcvbn score 0-4.
    pub score: u8,
}

/// An old password finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldPassword {
    /// The item.
    pub item_id: Id,
    /// Whole days since the password was last set (from the register's HLC time).
    pub age_days: u64,
}

/// Input to [`Vault::create_item`](crate::Vault::create_item).
pub struct NewItem {
    /// Type.
    pub item_type: ItemType,
    /// Title.
    pub title: String,
    /// Initial standard fields (other than the title).
    pub fields: Vec<(StdField, Zeroizing<String>)>,
    /// Initial URLs.
    pub urls: Vec<String>,
    /// Initial tags.
    pub tags: Vec<String>,
    /// Initial folder.
    pub folder_id: Option<Id>,
    /// Initial favorite flag.
    pub favorite: bool,
}

impl NewItem {
    /// A new item with just a type and title.
    #[must_use]
    pub fn new(item_type: ItemType, title: &str) -> Self {
        Self {
            item_type,
            title: title.to_owned(),
            fields: Vec::new(),
            urls: Vec::new(),
            tags: Vec::new(),
            folder_id: None,
            favorite: false,
        }
    }

    /// Add an initial field value.
    #[must_use]
    pub fn with_field(mut self, field: StdField, value: &str) -> Self {
        self.fields.push((field, Zeroizing::new(value.to_owned())));
        self
    }
}

impl core::fmt::Debug for NewItem {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // Field values may be secrets: print only which fields are set.
        f.debug_struct("NewItem")
            .field("item_type", &self.item_type)
            .field(
                "fields",
                &self.fields.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// A folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// Folder id.
    pub id: Id,
    /// Name.
    pub name: String,
    /// Parent folder.
    pub parent_id: Option<Id>,
}
