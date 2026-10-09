//! CSV importers: generic header-mapped, Chrome/Edge, Firefox and Safari export shapes.
//!
//! Encoding: UTF-8 (with or without a BOM) and UTF-16 **with** a BOM (LE or BE). Anything
//! else, including UTF-16 without a BOM and legacy code pages, is
//! [`ImportError::InvalidEncoding`]; guessing encodings is how passwords get silently
//! corrupted. Columns are matched by normalised header name (lower-case letters and digits
//! only), so the same mapper serves every shape; the detected shape is reported in
//! [`ImportBundle::source`].
//!
//! Imports never neutralise CSV-injection prefixes (`=`, `+`, `-`, `@`): that protection
//! exists on **export** only, and applying it on import would alter passwords.

use zeroize::Zeroizing;

use super::{
    ImportBundle, ImportError, ImportItem, ImportLimits, ImportSource, SkipReason, WarningKind,
    check_file_size, clean_folder_name, folder_index,
};
use crate::model::{ItemType, StdField};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Title,
    Url,
    Username,
    Password,
    Notes,
    Totp,
    Folder,
    Tags,
    Favorite,
    Kind,
}

const ALIASES: &[(Role, &[&str])] = &[
    (
        Role::Title,
        &[
            "name",
            "title",
            "itemname",
            "account",
            "accountname",
            "entryname",
        ],
    ),
    (
        Role::Url,
        &[
            "url",
            "website",
            "loginuri",
            "uri",
            "webaddress",
            "site",
            "address",
        ],
    ),
    (
        Role::Username,
        &[
            "username",
            "user",
            "login",
            "loginusername",
            "email",
            "emailaddress",
            "userid",
        ],
    ),
    (
        Role::Password,
        &["password", "pass", "loginpassword", "pwd"],
    ),
    (
        Role::Notes,
        &["note", "notes", "extra", "comments", "comment"],
    ),
    (
        Role::Totp,
        &[
            "totp",
            "otpauth",
            "otp",
            "loginotp",
            "totpseed",
            "authenticator",
        ],
    ),
    (Role::Folder, &["folder", "grouping", "group", "category"]),
    (Role::Tags, &["tags", "tag", "labels"]),
    (Role::Favorite, &["favorite", "favourite", "fav"]),
    (Role::Kind, &["type", "itemtype"]),
];

/// Columns that are part of the Firefox shape and carry nothing we import (no warning).
const KNOWN_IGNORED: &[&str] = &[
    "httprealm",
    "formactionorigin",
    "guid",
    "timecreated",
    "timelastused",
    "timepasswordchanged",
];

fn normalise(h: &[u8]) -> String {
    String::from_utf8_lossy(h)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Decodes the file to text: UTF-8 (+BOM) or UTF-16 with BOM.
fn decode_text(bytes: &[u8]) -> Result<Zeroizing<String>, ImportError> {
    decode_text_inner(bytes).map(Zeroizing::new)
}

fn decode_text_inner(bytes: &[u8]) -> Result<String, ImportError> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return std::str::from_utf8(rest)
            .map(str::to_owned)
            .map_err(|_| ImportError::InvalidEncoding);
    }
    let utf16 = |rest: &[u8], le: bool| -> Result<String, ImportError> {
        if rest.len() % 2 != 0 {
            return Err(ImportError::InvalidEncoding);
        }
        let units = rest.chunks_exact(2).map(|c| {
            if le {
                u16::from_le_bytes([c[0], c[1]])
            } else {
                u16::from_be_bytes([c[0], c[1]])
            }
        });
        char::decode_utf16(units)
            .collect::<Result<String, _>>()
            .map_err(|_| ImportError::InvalidEncoding)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| ImportError::InvalidEncoding)
}

fn host_of(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .rsplit('@')
        .next()
        .unwrap_or("")
        .split(':')
        .next()
        .unwrap_or("")
        .trim();
    if host.is_empty() || host.chars().any(char::is_control) {
        None
    } else {
        Some(host.to_owned())
    }
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y"
    )
}

/// Parses a CSV password export.
///
/// # Errors
/// Structural problems only ([`ImportError`]); individual bad rows are reported in
/// [`ImportBundle::skipped`].
pub fn parse_csv(bytes: &[u8], limits: &ImportLimits) -> Result<ImportBundle, ImportError> {
    check_file_size(bytes, limits)?;
    let text = decode_text(bytes)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rec = csv::ByteRecord::new();

    if !rdr
        .read_byte_record(&mut rec)
        .map_err(|_| ImportError::InvalidCsv)?
    {
        return Err(ImportError::InvalidCsv);
    }
    let names: Vec<String> = rec.iter().map(normalise).collect();
    let mut roles: Vec<Option<Role>> = Vec::with_capacity(names.len());
    let mut taken: Vec<Role> = Vec::new();
    let mut bundle_warnings = Vec::new();
    for (i, n) in names.iter().enumerate() {
        let role = ALIASES
            .iter()
            .find(|(_, a)| a.contains(&n.as_str()))
            .map(|(r, _)| *r);
        match role {
            Some(r) if !taken.contains(&r) => {
                taken.push(r);
                roles.push(Some(r));
            }
            _ => {
                if !KNOWN_IGNORED.contains(&n.as_str()) {
                    bundle_warnings.push(i);
                }
                roles.push(None);
            }
        }
    }
    let has = |r: Role| taken.contains(&r);
    if !(has(Role::Title)
        || has(Role::Url)
        || has(Role::Username)
        || has(Role::Password)
        || has(Role::Notes)
        || has(Role::Totp))
    {
        return Err(ImportError::NoRecognizedColumns);
    }
    let has_name = |n: &str| names.iter().any(|x| x == n);
    let source = if has(Role::Url)
        && has(Role::Password)
        && (has_name("httprealm") || has_name("formactionorigin") || has_name("guid"))
    {
        ImportSource::FirefoxCsv
    } else if has_name("title")
        && has_name("url")
        && has_name("username")
        && has_name("password")
        && has_name("otpauth")
    {
        ImportSource::SafariCsv
    } else if has_name("name")
        && has_name("url")
        && has_name("username")
        && has_name("password")
        && names.len() <= 5
    {
        ImportSource::ChromeCsv
    } else {
        ImportSource::GenericCsv
    };

    let mut bundle = ImportBundle::new(source);
    for i in bundle_warnings {
        bundle.warn(0, WarningKind::IgnoredColumn(i));
    }

    let mut row = 0usize;
    loop {
        match rdr.read_byte_record(&mut rec) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => return Err(ImportError::InvalidCsv),
        }
        row += 1;
        if row > limits.max_records {
            return Err(ImportError::TooManyRecords);
        }
        if rec.len() > roles.len() {
            bundle.warn(row, WarningKind::ExtraCells);
        }
        if rec.iter().any(|c| c.len() > limits.max_field_bytes) {
            bundle.skip(row, SkipReason::FieldTooLarge("csv field"), limits)?;
            continue;
        }
        // Cells are valid UTF-8 because the whole text was validated.
        let cell = |r: Role| -> Option<String> {
            let idx = roles.iter().position(|x| *x == Some(r))?;
            let c = rec.get(idx)?;
            let s = String::from_utf8_lossy(c).into_owned();
            let t = s.trim_matches(|c: char| c == '\r' || c == '\n');
            if t.trim().is_empty() {
                None
            } else {
                Some(t.to_owned())
            }
        };
        let (title, url, user, pass, notes, totp) = (
            cell(Role::Title),
            cell(Role::Url),
            cell(Role::Username),
            cell(Role::Password).map(Zeroizing::new),
            cell(Role::Notes),
            cell(Role::Totp).map(Zeroizing::new),
        );
        let kind = cell(Role::Kind).map(|k| k.to_ascii_lowercase());
        let is_note = matches!(kind.as_deref(), Some("note" | "securenote" | "secure note"));
        if title.is_none()
            && url.is_none()
            && user.is_none()
            && pass.is_none()
            && notes.is_none()
            && totp.is_none()
        {
            bundle.skip(row, SkipReason::Empty, limits)?;
            continue;
        }
        if let Some(k) = &kind {
            if !is_note && k != "login" && k != "password" {
                bundle.skip(
                    row,
                    SkipReason::Invalid("unsupported item type column"),
                    limits,
                )?;
                continue;
            }
        }
        let derived = title.is_none();
        let title = title
            .or_else(|| url.as_deref().and_then(host_of))
            .or_else(|| user.clone())
            .unwrap_or_else(|| "Untitled".to_owned());
        if derived && source != ImportSource::FirefoxCsv {
            bundle.warn(row, WarningKind::TitleDerived);
        }
        let mut item = ImportItem::new(
            if is_note {
                ItemType::Note
            } else {
                ItemType::Login
            },
            &title,
            row,
        );
        if is_note {
            if let Some(n) = &notes {
                item.fields
                    .push((StdField::Body, Zeroizing::new(n.clone())));
            }
        } else {
            if let Some(u) = &user {
                item.set(StdField::Username, u);
            }
            if let Some(p) = &pass {
                item.fields.push((StdField::Password, p.clone()));
            }
            if let Some(n) = &notes {
                item.set(StdField::Notes, n);
            }
            if let Some(t) = &totp {
                item.fields.push((StdField::TotpSeed, t.clone()));
            }
            if let Some(u) = &url {
                item.urls.push(u.clone());
            }
        }
        if let Some(f) = cell(Role::Folder) {
            match clean_folder_name(&f) {
                Some(name) => item.folder = Some(folder_index(&mut bundle, &name)),
                None => bundle.warn(row, WarningKind::InvalidFolder),
            }
        }
        if let Some(tags) = cell(Role::Tags) {
            for t in tags
                .split([',', ';'])
                .map(str::trim)
                .filter(|t| !t.is_empty())
            {
                if crate::model::check_tag(t).is_ok() && !item.tags.iter().any(|x| x == t) {
                    item.tags.push(t.to_owned());
                } else {
                    bundle.warn(row, WarningKind::TagDropped);
                }
            }
        }
        item.favorite = cell(Role::Favorite).is_some_and(|f| truthy(&f));
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

    fn pw(b: &ImportBundle, i: usize) -> String {
        b.items[i]
            .fields
            .iter()
            .find(|(f, _)| *f == StdField::Password)
            .map(|(_, v)| v.to_string())
            .unwrap()
    }

    #[test]
    fn chrome_shape() {
        let b = parse_csv(b"name,url,username,password,note\nExample,https://example.com/login,alice,CANARY-pw-1,CANARY note\n", &lim()).unwrap();
        assert_eq!(b.source, ImportSource::ChromeCsv);
        assert_eq!(b.items.len(), 1);
        let it = &b.items[0];
        assert_eq!(
            (it.item_type, it.title.as_str()),
            (ItemType::Login, "Example")
        );
        assert_eq!(it.urls, ["https://example.com/login"]);
        assert_eq!(pw(&b, 0), "CANARY-pw-1");
    }

    #[test]
    fn firefox_shape_derives_title_from_host() {
        let csv = "\"url\",\"username\",\"password\",\"httpRealm\",\"formActionOrigin\",\"guid\",\"timeCreated\",\"timeLastUsed\",\"timePasswordChanged\"\n\"https://accounts.example.org:8443/signin?x=1\",\"bob\",\"CANARY-pw-2\",\"\",\"https://accounts.example.org\",\"{abc}\",\"1\",\"2\",\"3\"\n";
        let b = parse_csv(csv.as_bytes(), &lim()).unwrap();
        assert_eq!(b.source, ImportSource::FirefoxCsv);
        assert_eq!(b.items[0].title, "accounts.example.org");
        assert!(
            b.warnings.is_empty(),
            "known Firefox metadata columns are not warned about"
        );
    }

    #[test]
    fn safari_shape_with_totp() {
        let b = parse_csv(b"Title,URL,Username,Password,Notes,OTPAuth\nBank,https://bank.example,carol,CANARY-pw-3,n,otpauth://totp/x?secret=CANARYSEED\n", &lim()).unwrap();
        assert_eq!(b.source, ImportSource::SafariCsv);
        assert!(
            b.items[0]
                .fields
                .iter()
                .any(|(f, v)| *f == StdField::TotpSeed && v.starts_with("otpauth://"))
        );
    }

    #[test]
    fn generic_mapping_with_aliases_folder_tags_favorite() {
        let b = parse_csv(b"Website,Login,Pass,Folder,Tags,Favorite,Weird\nhttps://a.example,u,CANARY-p,Work,\"x, y;z\",true,?\n", &lim()).unwrap();
        assert_eq!(b.source, ImportSource::GenericCsv);
        let it = &b.items[0];
        assert_eq!(it.tags, ["x", "y", "z"]);
        assert!(it.favorite);
        assert_eq!(b.folders[it.folder.unwrap()].name, "Work");
        assert!(b.warnings.contains(&super::super::ImportWarning {
            record: 0,
            kind: WarningKind::IgnoredColumn(6)
        }));
    }

    #[test]
    fn notes_round_trip_shape() {
        let b = parse_csv(b"name,type,note\nMemo,note,CANARY body\n", &lim()).unwrap();
        assert_eq!(b.items[0].item_type, ItemType::Note);
        assert!(
            b.items[0]
                .fields
                .iter()
                .any(|(f, v)| *f == StdField::Body && &***v == "CANARY body")
        );
    }

    #[test]
    fn multiline_and_quoted_cells() {
        let b = parse_csv(
            b"name,url,username,password,note\n\"A, \"\"B\"\"\",,,CANARY,\"line1\nline2\"\n",
            &lim(),
        )
        .unwrap();
        assert_eq!(b.items[0].title, "A, \"B\"");
        assert!(
            b.items[0]
                .fields
                .iter()
                .any(|(f, v)| *f == StdField::Notes && v.contains("line1\nline2"))
        );
    }

    #[test]
    fn utf8_bom_utf16_and_bad_encodings() {
        let plain = "name,url,username,password\nÉmile,https://x.example,u,CANARY-é\n";
        let mut bom = vec![0xEF, 0xBB, 0xBF];
        bom.extend(plain.as_bytes());
        assert_eq!(parse_csv(&bom, &lim()).unwrap().items[0].title, "Émile");
        let le: Vec<u8> = [0xFFu8, 0xFE]
            .into_iter()
            .chain(plain.encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        let be: Vec<u8> = [0xFEu8, 0xFF]
            .into_iter()
            .chain(plain.encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        for f in [&le, &be] {
            let b = parse_csv(f, &lim()).unwrap();
            assert_eq!(b.items[0].title, "Émile");
            assert_eq!(pw(&b, 0), "CANARY-é");
        }
        // UTF-16 without BOM, invalid UTF-8, odd UTF-16 length, lone surrogate.
        assert!(matches!(
            parse_csv(
                &plain
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes)
                    .collect::<Vec<_>>(),
                &lim()
            ),
            Err(ImportError::InvalidEncoding
                | ImportError::NoRecognizedColumns
                | ImportError::InvalidCsv)
        ));
        assert!(matches!(
            parse_csv(b"name,url\n\xff\xfe,x\n", &lim()),
            Err(ImportError::InvalidEncoding)
        ));
        assert!(matches!(
            parse_csv(&[0xFF, 0xFE, 0x41], &lim()),
            Err(ImportError::InvalidEncoding)
        ));
        assert!(matches!(
            parse_csv(&[0xFF, 0xFE, 0x00, 0xD8, 0x41, 0x00], &lim()),
            Err(ImportError::InvalidEncoding)
        ));
    }

    #[test]
    fn structural_errors() {
        assert!(matches!(
            parse_csv(b"", &lim()),
            Err(ImportError::InvalidCsv)
        ));
        assert!(matches!(
            parse_csv(b"foo,bar\n1,2\n", &lim()),
            Err(ImportError::NoRecognizedColumns)
        ));
    }

    #[test]
    fn bad_rows_do_not_abort_the_rest() {
        let big = "x".repeat(2000);
        let csv = format!(
            "name,url,username,password\nok1,,u,CANARY1\n,,,\n{big},,u,CANARY2\nok2,,u,CANARY3\n"
        );
        let l = ImportLimits {
            max_field_bytes: 1000,
            ..lim()
        };
        let b = parse_csv(csv.as_bytes(), &l).unwrap();
        assert_eq!(
            b.items.iter().map(|i| i.title.as_str()).collect::<Vec<_>>(),
            ["ok1", "ok2"]
        );
        assert_eq!(b.skipped.len(), 2);
        assert_eq!(b.skipped[0].reason, SkipReason::Empty);
        assert_eq!(b.skipped[1].reason, SkipReason::FieldTooLarge("csv field"));
        assert_eq!(b.skipped[1].record, 3);
    }

    #[test]
    fn vault_field_limits_skip_only_that_record() {
        let big = "x".repeat(70_000); // > 64 KiB vault limit, < 1 MiB parse limit
        let csv = format!("name,username,password\nbig,u,{big}\nsmall,u,p\n");
        let b = parse_csv(csv.as_bytes(), &lim()).unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.skipped[0].reason, SkipReason::FieldTooLarge("password"));
    }

    #[test]
    fn size_and_record_limits() {
        let l = ImportLimits {
            max_file_bytes: 10,
            ..lim()
        };
        assert!(matches!(
            parse_csv(b"name,url,username,password\n", &l),
            Err(ImportError::FileTooLarge)
        ));
        let l = ImportLimits {
            max_records: 2,
            ..lim()
        };
        assert!(matches!(
            parse_csv(b"name\na\nb\nc\n", &l),
            Err(ImportError::TooManyRecords)
        ));
        assert!(parse_csv(b"name\na\nb\n", &l).is_ok());
    }

    #[test]
    fn giant_line_is_bounded_by_the_file_limit_and_field_limit() {
        let giant = "y".repeat(3 << 20);
        let csv = format!("name,password\nrow,{giant}\nok,p\n");
        let b = parse_csv(csv.as_bytes(), &lim()).unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.skipped[0].reason, SkipReason::FieldTooLarge("csv field"));
        let l = ImportLimits {
            max_file_bytes: 1 << 20,
            ..lim()
        };
        assert!(matches!(
            parse_csv(csv.as_bytes(), &l),
            Err(ImportError::FileTooLarge)
        ));
    }

    #[test]
    fn injection_prefixes_are_not_altered_on_import() {
        let b = parse_csv(b"name,username,password\nx,=cmd,+CANARY\n", &lim()).unwrap();
        assert_eq!(
            b.items[0]
                .fields
                .iter()
                .find(|(f, _)| *f == StdField::Username)
                .unwrap()
                .1
                .as_str(),
            "=cmd"
        );
        assert_eq!(pw(&b, 0), "+CANARY");
    }

    #[test]
    fn extra_cells_warn_and_short_rows_pad() {
        let b = parse_csv(b"name,username\na,u,EXTRA\nb\n", &lim()).unwrap();
        assert_eq!(b.items.len(), 2);
        assert!(
            b.warnings
                .iter()
                .any(|w| w.record == 1 && w.kind == WarningKind::ExtraCells)
        );
    }

    #[test]
    fn debug_output_has_no_secrets() {
        let b = parse_csv(
            b"name,username,password\nx,CANARY-user,CANARY-secret\n",
            &lim(),
        )
        .unwrap();
        let dbg = format!("{b:?}");
        assert!(!dbg.contains("CANARY-secret"));
    }

    #[test]
    fn arbitrary_garbage_never_panics() {
        for seed in 0u32..200 {
            let bytes: Vec<u8> = (0..300u32)
                .map(|i| (i.wrapping_mul(seed.wrapping_add(7)).wrapping_add(seed * 31) >> 3) as u8)
                .collect();
            let _ = parse_csv(&bytes, &lim());
        }
    }
}
