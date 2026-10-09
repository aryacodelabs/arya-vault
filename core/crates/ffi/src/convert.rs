//! DTO <-> core conversions. No behaviour beyond mapping and range checks.

use arya_vault_generator as generator;
use arya_vault_session as session;
use arya_vault_vault as cv;

use crate::api::dto::{
    AppError, CustomKind, CustomView, ItemSummary, ItemType, ItemView, KdfProfile,
    PassphraseOptions, PasswordOptions, QuickUnlockKind, QuickUnlockStatus, StdField, UrlView,
};
use crate::error::ApiResult;

// ------------------------------------------------------------------------------------- ids

pub(crate) fn hex_id(id: &[u8; 16]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(32);
    for b in id {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// 32 lowercase hex characters -> 16 bytes.
pub(crate) fn parse_id(s: &str, field: &str) -> ApiResult<[u8; 16]> {
    let bad = || AppError::validation(field, "not a valid id");
    if s.len() != 32 {
        return Err(bad());
    }
    let mut out = [0u8; 16];
    for (i, pair) in s.as_bytes().chunks(2).enumerate() {
        let hi = nibble(pair[0]).ok_or_else(bad)?;
        let lo = nibble(pair[1]).ok_or_else(bad)?;
        out[i] = (hi << 4) | lo;
    }
    Ok(out)
}

fn nibble(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        _ => None,
    }
}

pub(crate) fn parse_element(s: &str, field: &str) -> ApiResult<cv::ElementId> {
    cv::ElementId::parse(s).map_err(|_| AppError::validation(field, "not a valid id"))
}

pub(crate) fn opt_id(s: Option<&str>, field: &str) -> ApiResult<Option<[u8; 16]>> {
    s.map(|s| parse_id(s, field)).transpose()
}

// ----------------------------------------------------------------------------------- enums

impl From<ItemType> for cv::ItemType {
    fn from(t: ItemType) -> Self {
        match t {
            ItemType::Login => Self::Login,
            ItemType::Note => Self::Note,
            ItemType::Card => Self::Card,
            ItemType::Identity => Self::Identity,
        }
    }
}

impl From<cv::ItemType> for ItemType {
    fn from(t: cv::ItemType) -> Self {
        match t {
            cv::ItemType::Login => Self::Login,
            cv::ItemType::Note => Self::Note,
            cv::ItemType::Card => Self::Card,
            cv::ItemType::Identity => Self::Identity,
        }
    }
}

impl From<StdField> for cv::StdField {
    fn from(f: StdField) -> Self {
        match f {
            StdField::Title => Self::Title,
            StdField::Username => Self::Username,
            StdField::Password => Self::Password,
            StdField::TotpSeed => Self::TotpSeed,
            StdField::Notes => Self::Notes,
            StdField::Body => Self::Body,
            StdField::CardHolder => Self::Holder,
            StdField::CardNumber => Self::Number,
            StdField::CardExpiry => Self::Expiry,
            StdField::CardCvv => Self::Cvv,
            StdField::CardPin => Self::Pin,
            StdField::FirstName => Self::FirstName,
            StdField::MiddleName => Self::MiddleName,
            StdField::LastName => Self::LastName,
            StdField::Email => Self::Email,
            StdField::Phone => Self::Phone,
            StdField::Address => Self::Address,
            StdField::Ids => Self::Ids,
        }
    }
}

impl From<cv::StdField> for StdField {
    fn from(f: cv::StdField) -> Self {
        match f {
            cv::StdField::Title => Self::Title,
            cv::StdField::Username => Self::Username,
            cv::StdField::Password => Self::Password,
            cv::StdField::TotpSeed => Self::TotpSeed,
            cv::StdField::Notes => Self::Notes,
            cv::StdField::Body => Self::Body,
            cv::StdField::Holder => Self::CardHolder,
            cv::StdField::Number => Self::CardNumber,
            cv::StdField::Expiry => Self::CardExpiry,
            cv::StdField::Cvv => Self::CardCvv,
            cv::StdField::Pin => Self::CardPin,
            cv::StdField::FirstName => Self::FirstName,
            cv::StdField::MiddleName => Self::MiddleName,
            cv::StdField::LastName => Self::LastName,
            cv::StdField::Email => Self::Email,
            cv::StdField::Phone => Self::Phone,
            cv::StdField::Address => Self::Address,
            cv::StdField::Ids => Self::Ids,
        }
    }
}

impl From<CustomKind> for cv::CustomKind {
    fn from(k: CustomKind) -> Self {
        match k {
            CustomKind::Text => Self::Text,
            CustomKind::Hidden => Self::Hidden,
            CustomKind::Url => Self::Url,
            CustomKind::Date => Self::Date,
        }
    }
}

impl From<cv::CustomKind> for CustomKind {
    fn from(k: cv::CustomKind) -> Self {
        match k {
            cv::CustomKind::Text => Self::Text,
            cv::CustomKind::Hidden => Self::Hidden,
            cv::CustomKind::Url => Self::Url,
            cv::CustomKind::Date => Self::Date,
        }
    }
}

impl From<KdfProfile> for session::KdfProfile {
    fn from(p: KdfProfile) -> Self {
        match p {
            KdfProfile::Low => Self::Low,
            KdfProfile::Default => Self::Default,
            KdfProfile::High => Self::High,
        }
    }
}

impl From<session::QuickUnlockKind> for QuickUnlockKind {
    fn from(k: session::QuickUnlockKind) -> Self {
        use session::QuickUnlockKind as K;
        match k {
            K::None => Self::None,
            K::WindowsHello => Self::WindowsHello,
            K::TouchId => Self::TouchId,
            K::FaceId => Self::FaceId,
            K::Biometric => Self::Biometric,
            K::OsKeyring => Self::OsKeyring,
        }
    }
}

impl From<session::QuickUnlockStatus> for QuickUnlockStatus {
    fn from(s: session::QuickUnlockStatus) -> Self {
        Self {
            supported: s.supported,
            enabled: s.enabled,
            kind: s.kind.into(),
        }
    }
}

// ------------------------------------------------------------------------------- generator

pub(crate) fn to_usize(n: u32, field: &str) -> ApiResult<usize> {
    usize::try_from(n).map_err(|_| AppError::validation(field, "out of range"))
}

impl PasswordOptions {
    pub(crate) fn to_core(&self) -> ApiResult<generator::PasswordOptions> {
        Ok(generator::PasswordOptions {
            length: to_usize(self.length, "length")?,
            lower: self.lower,
            upper: self.upper,
            digits: self.digits,
            symbols: self.symbols,
            symbol_set: self.symbol_set.clone(),
            exclude_ambiguous: self.exclude_ambiguous,
            require_each_class: self.require_each_class,
        })
    }
}

impl PassphraseOptions {
    pub(crate) fn to_core(&self) -> ApiResult<generator::PassphraseOptions> {
        Ok(generator::PassphraseOptions {
            word_count: to_usize(self.word_count, "wordCount")?,
            separator: self.separator.clone(),
            capitalize: self.capitalize,
            number_suffix: self.number_suffix,
        })
    }
}

// ------------------------------------------------------------------------------ item views

/// Milliseconds of a hybrid logical clock value, as the DTO's signed integer.
pub(crate) fn hlc_ms(h: cv::Hlc) -> i64 {
    i64::try_from(h.pt()).unwrap_or(i64::MAX)
}

fn subtitle(t: cv::ItemType, fields: &[(cv::StdField, String)]) -> String {
    let get = |f: cv::StdField| {
        fields
            .iter()
            .find(|(k, _)| *k == f)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    };
    match t {
        cv::ItemType::Login => get(cv::StdField::Username).to_owned(),
        cv::ItemType::Card => get(cv::StdField::Holder).to_owned(),
        cv::ItemType::Identity => [cv::StdField::FirstName, cv::StdField::LastName]
            .map(get)
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        // A note's first line is body text, which is secret (spec question 3).
        cv::ItemType::Note => String::new(),
    }
}

/// Builds the list row from the full view (tags, subtitle and `hasTotp` are not in the core's
/// summary). `deleted` is set by the caller.
pub(crate) fn summary_of(view: &cv::ItemView, deleted: bool) -> ItemSummary {
    let s = &view.summary;
    ItemSummary {
        id: hex_id(&s.id),
        item_type: s.item_type.into(),
        title: s.title.clone(),
        subtitle: subtitle(s.item_type, &view.fields),
        favorite: s.favorite,
        folder_id: s.folder_id.as_ref().map(hex_id),
        tags: view.tags.clone(),
        updated_at: hlc_ms(s.updated),
        has_totp: view.secret_fields.contains(&cv::StdField::TotpSeed),
        deleted,
    }
}

pub(crate) fn view_of(view: &cv::ItemView, other_versions: bool) -> ItemView {
    let s = &view.summary;
    ItemView {
        id: hex_id(&s.id),
        item_type: s.item_type.into(),
        title: s.title.clone(),
        folder_id: s.folder_id.as_ref().map(hex_id),
        favorite: s.favorite,
        tags: view.tags.clone(),
        urls: view
            .urls
            .iter()
            .map(|u| UrlView {
                id: u.id.as_str().to_owned(),
                url: u.url.clone(),
            })
            .collect(),
        custom: view
            .custom
            .iter()
            .map(|c| CustomView {
                id: c.id.as_str().to_owned(),
                label: c.label.clone(),
                kind: c.kind.into(),
                value_if_not_hidden: if c.kind == cv::CustomKind::Hidden {
                    None
                } else {
                    c.value.clone()
                },
            })
            .collect(),
        fields: view
            .fields
            .iter()
            .map(|(f, v)| (StdField::from(*f), Some(v.clone())))
            .collect(),
        secret_fields_present: view.secret_fields.iter().map(|f| (*f).into()).collect(),
        created_at: view.created_at_ms.unwrap_or(0),
        updated_at: hlc_ms(s.updated),
        other_versions,
    }
}
