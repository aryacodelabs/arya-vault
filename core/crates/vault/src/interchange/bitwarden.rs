//! Importer for **unencrypted** Bitwarden JSON exports.
//!
//! Supported: logins (username, password, URIs, TOTP), secure notes, cards, identities,
//! folders (kept as flat names; a `a/b` name is not split), custom fields (text, hidden,
//! boolean as `true`/`false`), favorites. Attachments are ignored with a warning, items in
//! Bitwarden's trash are skipped, and unknown item types are *reported* in
//! [`ImportBundle::skipped`], never dropped silently. Bitwarden's encrypted JSON export is
//! rejected with [`ImportError::EncryptedBitwardenUnsupported`].
//!
//! Mapping notes: a secure note's text becomes the note body; identity address parts are
//! joined into the single `address` field, document numbers (SSN, passport, licence) into
//! `ids`; fields with no AryaVault equivalent (card brand, identity company/title/username)
//! become custom text fields so nothing is lost.

use zeroize::Zeroizing;

use super::json::{self, Json};
use super::{
    ImportBundle, ImportCustom, ImportError, ImportItem, ImportLimits, ImportSource, SkipReason,
    WarningKind, check_file_size, clean_folder_name,
};
use crate::model::{CustomKind, ItemType, StdField};

fn text(j: Option<&Json>) -> Option<&str> {
    j.and_then(Json::as_str).filter(|s| !s.is_empty())
}

fn custom_text(item: &mut ImportItem, label: &str, value: Option<&str>) {
    if let Some(v) = value.filter(|v| !v.is_empty()) {
        item.custom.push(ImportCustom {
            kind: CustomKind::Text,
            label: label.to_owned(),
            value: Zeroizing::new(v.to_owned()),
        });
    }
}

/// Parses an unencrypted Bitwarden JSON export.
///
/// # Errors
/// Structural problems only; bad records are reported in [`ImportBundle::skipped`].
pub fn parse_bitwarden_json(
    bytes: &[u8],
    limits: &ImportLimits,
) -> Result<ImportBundle, ImportError> {
    check_file_size(bytes, limits)?;
    let root = json::parse(bytes, limits)?;
    if !matches!(root, Json::Obj(_)) {
        return Err(ImportError::UnexpectedStructure);
    }
    if root.get("encrypted").and_then(Json::as_bool) == Some(true)
        || root.get("encKeyValidation_DO_NOT_EDIT").is_some()
    {
        return Err(ImportError::EncryptedBitwardenUnsupported);
    }
    let items = root
        .get("items")
        .and_then(Json::as_array)
        .ok_or(ImportError::UnexpectedStructure)?;
    if items.len() > limits.max_records {
        return Err(ImportError::TooManyRecords);
    }

    let mut bundle = ImportBundle::new(ImportSource::Bitwarden);
    // Bitwarden folder id -> index into bundle.folders (None if the name was unusable).
    let mut folder_map: Vec<(String, Option<usize>)> = Vec::new();
    if let Some(folders) = root.get("folders").and_then(Json::as_array) {
        for f in folders {
            let (Some(id), Some(name)) = (text(f.get("id")), f.get("name").and_then(Json::as_str))
            else {
                bundle.warn(0, WarningKind::InvalidFolder);
                continue;
            };
            match clean_folder_name(name) {
                Some(n) => {
                    let idx = super::folder_index(&mut bundle, &n);
                    folder_map.push((id.to_owned(), Some(idx)));
                }
                None => {
                    bundle.warn(0, WarningKind::InvalidFolder);
                    folder_map.push((id.to_owned(), None));
                }
            }
        }
    }

    for (n, it) in items.iter().enumerate() {
        let rec = n + 1;
        if !matches!(it, Json::Obj(_)) {
            bundle.skip(rec, SkipReason::Invalid("item is not an object"), limits)?;
            continue;
        }
        let Some(kind) = it.get("type").and_then(Json::as_i64) else {
            bundle.skip(rec, SkipReason::Invalid("missing item type"), limits)?;
            continue;
        };
        let item_type = match kind {
            1 => ItemType::Login,
            2 => ItemType::Note,
            3 => ItemType::Card,
            4 => ItemType::Identity,
            other => {
                bundle.skip(rec, SkipReason::UnknownType(other), limits)?;
                continue;
            }
        };
        if !matches!(it.get("deletedDate"), None | Some(Json::Null)) {
            bundle.skip(rec, SkipReason::InTrash, limits)?;
            continue;
        }
        let title = match text(it.get("name")) {
            Some(t) => t.to_owned(),
            None => {
                bundle.warn(rec, WarningKind::TitleDerived);
                "Untitled".to_owned()
            }
        };
        let mut item = ImportItem::new(item_type, &title, rec);
        item.favorite = it.get("favorite").and_then(Json::as_bool).unwrap_or(false);
        let notes = text(it.get("notes"));

        match item_type {
            ItemType::Login => {
                let login = it.get("login");
                if let Some(l) = login {
                    item.set(StdField::Username, text(l.get("username")).unwrap_or(""));
                    item.set(StdField::Password, text(l.get("password")).unwrap_or(""));
                    item.set(StdField::TotpSeed, text(l.get("totp")).unwrap_or(""));
                    if let Some(uris) = l.get("uris").and_then(Json::as_array) {
                        for u in uris {
                            if let Some(uri) = text(u.get("uri")) {
                                if !item.urls.iter().any(|x| x == uri) {
                                    item.urls.push(uri.to_owned());
                                }
                            }
                        }
                    }
                }
                item.set(StdField::Notes, notes.unwrap_or(""));
            }
            ItemType::Note => item.set(StdField::Body, notes.unwrap_or("")),
            ItemType::Card => {
                let c = it.get("card");
                let g = |k: &str| c.and_then(|c| text(c.get(k)));
                item.set(StdField::Holder, g("cardholderName").unwrap_or(""));
                item.set(StdField::Number, g("number").unwrap_or(""));
                item.set(StdField::Cvv, g("code").unwrap_or(""));
                let expiry = match (g("expMonth"), g("expYear")) {
                    (Some(m), Some(y)) => format!("{m}/{y}"),
                    (Some(m), None) => m.to_owned(),
                    (None, Some(y)) => y.to_owned(),
                    (None, None) => String::new(),
                };
                item.set(StdField::Expiry, &expiry);
                item.set(StdField::Notes, notes.unwrap_or(""));
                custom_text(&mut item, "Brand", g("brand"));
            }
            ItemType::Identity => {
                let i = it.get("identity");
                let g = |k: &str| i.and_then(|i| text(i.get(k)));
                item.set(StdField::FirstName, g("firstName").unwrap_or(""));
                item.set(StdField::MiddleName, g("middleName").unwrap_or(""));
                item.set(StdField::LastName, g("lastName").unwrap_or(""));
                item.set(StdField::Email, g("email").unwrap_or(""));
                item.set(StdField::Phone, g("phone").unwrap_or(""));
                let mut lines: Vec<String> = ["address1", "address2", "address3"]
                    .iter()
                    .filter_map(|k| g(k).map(str::to_owned))
                    .collect();
                let region: Vec<&str> = [g("city"), g("state"), g("postalCode")]
                    .into_iter()
                    .flatten()
                    .collect();
                if !region.is_empty() {
                    lines.push(region.join(", "));
                }
                lines.extend(g("country").map(str::to_owned));
                item.set(StdField::Address, &lines.join("\n"));
                let ids: Vec<String> = [
                    ("SSN", "ssn"),
                    ("Passport", "passportNumber"),
                    ("Licence", "licenseNumber"),
                ]
                .iter()
                .filter_map(|(label, k)| g(k).map(|v| format!("{label}: {v}")))
                .collect();
                item.set(StdField::Ids, &ids.join("\n"));
                item.set(StdField::Notes, notes.unwrap_or(""));
                custom_text(&mut item, "Company", g("company"));
                custom_text(&mut item, "Username", g("username"));
                custom_text(&mut item, "Title", g("title"));
            }
        }

        if let Some(fields) = it.get("fields").and_then(Json::as_array) {
            for f in fields {
                let label = text(f.get("name")).unwrap_or("Field");
                let value = f.get("value").and_then(Json::as_str).unwrap_or("");
                let (kind, value) = match f.get("type").and_then(Json::as_i64) {
                    Some(0) | None => (CustomKind::Text, value.to_owned()),
                    Some(1) => (CustomKind::Hidden, value.to_owned()),
                    Some(2) => (
                        CustomKind::Text,
                        if value == "true" { "true" } else { "false" }.to_owned(),
                    ),
                    Some(other) => {
                        bundle.warn(rec, WarningKind::UnsupportedCustomField(other));
                        continue;
                    }
                };
                item.custom.push(ImportCustom {
                    kind,
                    label: label.to_owned(),
                    value: Zeroizing::new(value),
                });
            }
        }
        if it
            .get("attachments")
            .and_then(Json::as_array)
            .is_some_and(|a| !a.is_empty())
        {
            bundle.warn(rec, WarningKind::AttachmentsIgnored);
        }
        if let Some(fid) = text(it.get("folderId")) {
            match folder_map.iter().find(|(id, _)| id == fid) {
                Some((_, Some(idx))) => item.folder = Some(*idx),
                _ => bundle.warn(rec, WarningKind::InvalidFolder),
            }
        }
        bundle.push(item, limits)?;
    }
    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lim() -> ImportLimits {
        ImportLimits::default()
    }

    fn field(it: &ImportItem, f: StdField) -> Option<String> {
        it.fields
            .iter()
            .find(|(g, _)| *g == f)
            .map(|(_, v)| v.to_string())
    }

    const SAMPLE: &str = r#"{
      "encrypted": false,
      "folders": [{"id":"f1","name":"Work"},{"id":"f2","name":"  "}],
      "items": [
        {"id":"1","folderId":"f1","type":1,"name":"Example","notes":"CANARY note","favorite":true,
         "fields":[{"name":"pin","value":"CANARY-1234","type":1},{"name":"vip","value":"true","type":2},{"name":"x","value":"v","type":0},{"name":"linked","value":null,"type":3}],
         "login":{"uris":[{"uri":"https://a.example"},{"uri":"https://a.example"},{"uri":"https://b.example"}],"username":"alice","password":"CANARY-pw","totp":"CANARYSEED"},
         "attachments":[{"id":"a"}]},
        {"id":"2","type":2,"name":"Memo","notes":"CANARY body","secureNote":{"type":0}},
        {"id":"3","type":3,"name":"Visa","card":{"cardholderName":"A B","brand":"Visa","number":"4111111111111111","expMonth":"12","expYear":"2030","code":"123"}},
        {"id":"4","type":4,"name":"Me","identity":{"firstName":"A","lastName":"B","address1":"1 St","city":"X","postalCode":"9","country":"Y","ssn":"CANARY-SSN","company":"Co"}},
        {"id":"5","type":5,"name":"Future"},
        {"id":"6","type":1,"name":"Gone","deletedDate":"2024-01-01T00:00:00Z","login":{}},
        {"id":"7","folderId":"f2","type":1,"login":{"username":"u"}}
      ]}"#;

    #[test]
    fn maps_every_supported_type() {
        let b = parse_bitwarden_json(SAMPLE.as_bytes(), &lim()).unwrap();
        assert_eq!(b.source, ImportSource::Bitwarden);
        assert_eq!(b.items.len(), 5);
        let login = &b.items[0];
        assert_eq!(login.item_type, ItemType::Login);
        assert_eq!(login.urls, ["https://a.example", "https://b.example"]);
        assert_eq!(
            field(login, StdField::Password).as_deref(),
            Some("CANARY-pw")
        );
        assert_eq!(
            field(login, StdField::TotpSeed).as_deref(),
            Some("CANARYSEED")
        );
        assert_eq!(
            field(login, StdField::Notes).as_deref(),
            Some("CANARY note")
        );
        assert!(login.favorite);
        assert_eq!(b.folders[login.folder.unwrap()].name, "Work");
        let kinds: Vec<_> = login
            .custom
            .iter()
            .map(|c| (c.kind, c.label.as_str(), c.value.as_str()))
            .collect();
        assert_eq!(
            kinds,
            [
                (CustomKind::Hidden, "pin", "CANARY-1234"),
                (CustomKind::Text, "vip", "true"),
                (CustomKind::Text, "x", "v")
            ]
        );
        assert_eq!(
            field(&b.items[1], StdField::Body).as_deref(),
            Some("CANARY body")
        );
        let card = &b.items[2];
        assert_eq!(
            field(card, StdField::Number).as_deref(),
            Some("4111111111111111")
        );
        assert_eq!(field(card, StdField::Expiry).as_deref(), Some("12/2030"));
        assert_eq!(field(card, StdField::Cvv).as_deref(), Some("123"));
        assert_eq!(card.custom[0].label, "Brand");
        let id = &b.items[3];
        assert_eq!(
            field(id, StdField::Address).as_deref(),
            Some("1 St\nX, 9\nY")
        );
        assert_eq!(field(id, StdField::Ids).as_deref(), Some("SSN: CANARY-SSN"));
        assert_eq!(id.custom[0].label, "Company");
    }

    #[test]
    fn unknown_trashed_and_untitled_are_reported_not_dropped_silently() {
        let b = parse_bitwarden_json(SAMPLE.as_bytes(), &lim()).unwrap();
        assert_eq!(
            b.skipped
                .iter()
                .map(|s| (s.record, s.reason))
                .collect::<Vec<_>>(),
            [(5, SkipReason::UnknownType(5)), (6, SkipReason::InTrash)]
        );
        assert_eq!(b.items[4].title, "Untitled");
        let w = |r, k| b.warnings.iter().any(|x| x.record == r && x.kind == k);
        assert!(w(1, WarningKind::AttachmentsIgnored));
        assert!(w(1, WarningKind::UnsupportedCustomField(3)));
        assert!(w(7, WarningKind::TitleDerived));
        assert!(w(7, WarningKind::InvalidFolder));
        assert!(w(0, WarningKind::InvalidFolder));
    }

    #[test]
    fn encrypted_exports_are_refused_with_a_clear_error() {
        for doc in [
            r#"{"encrypted":true,"encKeyValidation_DO_NOT_EDIT":"2.x","data":"2.y"}"#,
            r#"{"encKeyValidation_DO_NOT_EDIT":"x","items":[]}"#,
        ] {
            let e = parse_bitwarden_json(doc.as_bytes(), &lim()).unwrap_err();
            assert!(matches!(e, ImportError::EncryptedBitwardenUnsupported));
            assert!(e.to_string().contains("export unencrypted"));
        }
    }

    #[test]
    fn wrong_shapes_are_structural_errors() {
        for doc in [
            "[]",
            "{}",
            r#"{"items":5}"#,
            "null",
            r#"{"items":[1,"x",{"type":"1"}]}"#,
        ] {
            let r = parse_bitwarden_json(doc.as_bytes(), &lim());
            match doc {
                r#"{"items":[1,"x",{"type":"1"}]}"# => {
                    let b = r.unwrap();
                    assert_eq!(b.items.len(), 0);
                    assert_eq!(b.skipped.len(), 3);
                }
                _ => assert!(matches!(r, Err(ImportError::UnexpectedStructure)), "{doc}"),
            }
        }
        assert!(matches!(
            parse_bitwarden_json(b"{", &lim()),
            Err(ImportError::InvalidJson)
        ));
    }

    #[test]
    fn hostile_inputs_hit_limits() {
        let deep = format!("{{\"items\":[{}0{}]}}", "[".repeat(40), "]".repeat(40));
        assert!(matches!(
            parse_bitwarden_json(deep.as_bytes(), &lim()),
            Err(ImportError::TooDeeplyNested)
        ));
        let l = ImportLimits {
            max_records: 2,
            ..lim()
        };
        assert!(matches!(
            parse_bitwarden_json(br#"{"items":[{"type":1},{"type":1},{"type":1}]}"#, &l),
            Err(ImportError::TooManyRecords)
        ));
        let l = ImportLimits {
            max_field_bytes: 10,
            ..lim()
        };
        assert!(matches!(
            parse_bitwarden_json(br#"{"items":[{"type":1,"name":"01234567890"}]}"#, &l),
            Err(ImportError::FieldTooLarge)
        ));
        let l = ImportLimits {
            max_file_bytes: 8,
            ..lim()
        };
        assert!(matches!(
            parse_bitwarden_json(SAMPLE.as_bytes(), &l),
            Err(ImportError::FileTooLarge)
        ));
        // Vault limits skip only the offending record.
        let huge = "x".repeat(70_000);
        let doc = format!(
            r#"{{"items":[{{"type":1,"name":"a","login":{{"password":"{huge}"}}}},{{"type":1,"name":"b"}}]}}"#
        );
        let b = parse_bitwarden_json(doc.as_bytes(), &lim()).unwrap();
        assert_eq!((b.items.len(), b.skipped.len()), (1, 1));
    }

    #[test]
    fn invalid_utf8_and_bom() {
        assert!(matches!(
            parse_bitwarden_json(b"{\"items\":[{\"type\":1,\"name\":\"\xff\"}]}", &lim()),
            Err(ImportError::InvalidJson)
        ));
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend(br#"{"items":[]}"#);
        assert_eq!(parse_bitwarden_json(&bom, &lim()).unwrap().items.len(), 0);
    }

    #[test]
    fn errors_and_debug_contain_no_secrets() {
        let b = parse_bitwarden_json(SAMPLE.as_bytes(), &lim()).unwrap();
        let dbg = format!("{b:?} {:?}", b.warnings);
        for s in ["CANARY-pw", "CANARYSEED", "CANARY-SSN", "4111111111111111"] {
            assert!(!dbg.contains(s), "{s}");
        }
    }
}
