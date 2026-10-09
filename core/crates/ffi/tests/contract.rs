//! The contract, pinned: the set of API functions equals docs/14 §4, `API_VERSION`, and the
//! secret-free shape of the summary and view DTOs.
//!
//! docs/14 is prose, not machine-readable, so the check is two-sided only in this sense: the
//! `EXPECTED` list below must equal the `pub fn`s found in `src/api/*.rs` (fails when the code
//! changes alone), and every name in it must occur in docs/14 §4 (fails when a name is removed
//! from the doc alone or misspelled). A function added to the doc and to neither the code nor
//! this list is the one change this cannot see; the PR template asks reviewers to look.

use std::collections::BTreeSet;

use arya_vault_ffi::api::dto::{ItemSummary, ItemView, StdField};

/// docs/14 §4, in camelCase.
const EXPECTED: &[&str] = &[
    // 4.1
    "status", "createVault", "confirmRecoveryKey", "unlock", "unlockQuick", "lock",
    "changePassword", "recoverWithKey", "regenerateRecoveryKey", "quickUnlockEnable",
    "quickUnlockDisable", "verifyPassword",
    // 4.2
    "list", "search", "itemCount", "getItem", "reveal", "revealCustom", "createItem", "setField",
    "clearField", "setFields", "toggleFavorite", "moveToFolder", "addTag", "removeTag", "addUrl",
    "setUrl", "removeUrl", "addCustomField", "setCustomValue", "setCustomLabel",
    "removeCustomField", "deleteItem", "restoreItem", "purgeItem", "listTrash", "emptyTrash",
    "purgeExpired", "history", "revealVersion", "restoreVersion", "createFolder", "renameFolder",
    "deleteFolder", "listFolders",
    // 4.3
    "generatePassword", "generatePassphrase", "entropyBits", "strength", "checkMasterPassword",
    "healthReport",
    // 4.4
    "importPreview", "importCommit", "exportCsv", "exportEncrypted", "importEncrypted",
    // 4.5
    "getSettings", "setSettings",
    // 4.6
    "info", "exportDiagnostics",
];

/// Functions the crate adds on purpose, with the reason (PR "Spec questions").
const EXTRAS: &[&str] = &["initCore"];

fn camel(snake: &str) -> String {
    let mut out = String::new();
    let mut up = false;
    for c in snake.chars() {
        if c == '_' {
            up = true;
        } else if up {
            out.extend(c.to_uppercase());
            up = false;
        } else {
            out.push(c);
        }
    }
    out
}

fn api_functions() -> BTreeSet<String> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src/api");
    let mut names = BTreeSet::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let path = e.unwrap().path();
        let file = path.file_name().unwrap().to_str().unwrap().to_owned();
        if file == "dto.rs" || file == "mod.rs" {
            continue;
        }
        for line in std::fs::read_to_string(&path).unwrap().lines() {
            if let Some(rest) = line.strip_prefix("pub fn ") {
                names.insert(camel(rest.split(['(', '<']).next().unwrap()));
            }
        }
    }
    names
}

#[test]
fn the_api_functions_are_exactly_the_contract() {
    let found = api_functions();
    let want: BTreeSet<String> = EXPECTED.iter().chain(EXTRAS).map(|s| (*s).to_owned()).collect();
    let missing: Vec<_> = want.difference(&found).collect();
    let extra: Vec<_> = found.difference(&want).collect();
    assert!(missing.is_empty(), "in docs/14 but not implemented: {missing:?}");
    assert!(extra.is_empty(), "implemented but not in docs/14: {extra:?}");
    assert_eq!(EXPECTED.iter().collect::<BTreeSet<_>>().len(), EXPECTED.len(), "duplicate in EXPECTED");
}

#[test]
fn every_contract_name_occurs_in_docs_14_section_4() {
    let doc = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/14-app-api-contract.md"
    ))
    .unwrap();
    let a = doc.find("## 4. Operations").unwrap();
    let b = doc.find("## 5. State machine").unwrap();
    let sec = &doc[a..b];
    let words: BTreeSet<&str> = sec
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for name in EXPECTED {
        assert!(words.contains(name), "`{name}` is not mentioned in docs/14 section 4");
    }
}

#[test]
fn api_version_is_one() {
    assert_eq!(arya_vault_ffi::API_VERSION, 1);
}

/// Compile-time: adding a field to either type breaks these destructurings, which forces the
/// author to look at whether the new field can carry a secret (docs/14 §3).
#[test]
fn summary_and_view_have_exactly_these_fields() {
    fn summary(s: ItemSummary) {
        let ItemSummary {
            id: _,
            item_type: _,
            title: _,
            subtitle: _,
            favorite: _,
            folder_id: _,
            tags: _,
            updated_at: _,
            has_totp: _,
            deleted: _,
        } = s;
    }
    fn view(v: ItemView) {
        let ItemView {
            id: _,
            item_type: _,
            title: _,
            folder_id: _,
            favorite: _,
            tags: _,
            urls: _,
            custom: _,
            fields: _,
            secret_fields_present: _,
            created_at: _,
            updated_at: _,
            other_versions: _,
        } = v;
    }
    let _ = (summary, view);
}

/// The names of the fields (source of truth: `dto.rs`), checked against secret-bearing names.
#[test]
fn no_field_of_summary_or_view_is_named_like_a_secret() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/api/dto.rs")).unwrap();
    let mut fields = Vec::new();
    for ty in ["ItemSummary", "ItemView", "UrlView", "CustomView", "TrashEntry", "VersionInfo"] {
        let start = src.find(&format!("pub struct {ty} {{")).unwrap();
        let body = &src[start..start + src[start..].find("\n}\n").unwrap()];
        for line in body.lines().skip(1) {
            let l = line.trim();
            if l.starts_with("///") || l.is_empty() {
                continue;
            }
            fields.push((ty, l.split(':').next().unwrap().trim_start_matches("pub ").to_owned()));
        }
    }
    assert!(fields.len() > 30);
    for (ty, f) in &fields {
        for bad in ["password", "secret", "seed", "cvv", "pin", "number", "body", "key", "value"] {
            // `value_if_not_hidden` is the one field that holds a value: it is `None` for hidden
            // custom fields (asserted in tests/items.rs); `secret_fields_present` is a set of
            // names, not values.
            if f == "value_if_not_hidden" || f == "secret_fields_present" {
                continue;
            }
            assert!(!f.contains(bad), "{ty}.{f} looks secret-bearing");
        }
    }
}

/// The standard fields the DTO knows are exactly the core's (`StdField` has 18 variants).
#[test]
fn std_field_covers_the_core_set() {
    let all = [
        StdField::Title, StdField::Username, StdField::Password, StdField::TotpSeed,
        StdField::Notes, StdField::Body, StdField::CardHolder, StdField::CardNumber,
        StdField::CardExpiry, StdField::CardCvv, StdField::CardPin, StdField::FirstName,
        StdField::MiddleName, StdField::LastName, StdField::Email, StdField::Phone,
        StdField::Address, StdField::Ids,
    ];
    let distinct: BTreeSet<_> = all.iter().collect();
    assert_eq!(distinct.len(), 18);
}
