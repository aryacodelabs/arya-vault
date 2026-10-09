//! Plaintext CSV export with CSV-injection neutralisation.
//!
//! **Neutralisation (export only):** a spreadsheet application may execute a cell that
//! begins with `=`, `+`, `-` or `@` (and, per OWASP, a tab or carriage return) as a
//! formula. Every such cell is prefixed with a single quote `'`. This deliberately changes
//! those cell values (a password starting with `-` is exported as `'-...`), because an
//! export that runs code when opened is the worse failure; importing the file back does
//! **not** strip the quote. Use the encrypted export for lossless backups.
//!
//! Columns: `name,url,username,password,note,totp,type,folder,tags,favorite`. Only logins
//! and secure notes can be represented; cards and identities are skipped and counted in
//! [`CsvExportReport::skipped_unsupported`], and only the first URL of a login is written
//! (the rest are counted in [`CsvExportReport::extra_urls_dropped`]).

use zeroize::Zeroizing;

use super::ImportError;
use super::aryavault::{ExportFolder, ExportItem};
use crate::model::{ItemType, StdField};

/// What the CSV export did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CsvExportReport {
    /// Rows written.
    pub exported: usize,
    /// Items not written because CSV cannot represent their type (cards, identities).
    pub skipped_unsupported: usize,
    /// Additional URLs of logins that were not written.
    pub extra_urls_dropped: usize,
    /// Cells that received the injection-neutralising prefix.
    pub neutralized_cells: usize,
}

/// A plaintext CSV document. The bytes contain passwords in clear text and are wiped on drop.
pub struct CsvExport {
    /// The CSV file (UTF-8, LF line endings, header row first).
    pub bytes: Zeroizing<Vec<u8>>,
    /// Counts.
    pub report: CsvExportReport,
}

impl core::fmt::Debug for CsvExport {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CsvExport")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

/// Prefixes dangerous leading characters. Returns the cell and whether it was changed.
pub(crate) fn neutralize(cell: &str) -> (std::borrow::Cow<'_, str>, bool) {
    if cell.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        (std::borrow::Cow::Owned(format!("'{cell}")), true)
    } else {
        (std::borrow::Cow::Borrowed(cell), false)
    }
}

pub(crate) fn write_csv(
    items: &[ExportItem],
    folders: &[ExportFolder],
) -> Result<CsvExport, ImportError> {
    let mut report = CsvExportReport::default();
    let mut w = csv::WriterBuilder::new().from_writer(Vec::new());
    w.write_record([
        "name", "url", "username", "password", "note", "totp", "type", "folder", "tags", "favorite",
    ])
    .map_err(|_| ImportError::Io)?;
    for it in items {
        let kind = match it.item_type {
            ItemType::Login => "login",
            ItemType::Note => "note",
            ItemType::Card | ItemType::Identity => {
                report.skipped_unsupported += 1;
                continue;
            }
        };
        let get = |f: StdField| {
            it.fields
                .iter()
                .find(|(k, _)| *k == f)
                .map_or("", |(_, v)| v.as_str())
        };
        report.extra_urls_dropped += it.urls.len().saturating_sub(1);
        let folder = it
            .folder
            .and_then(|id| folders.iter().find(|f| f.id == id))
            .map_or("", |f| f.name.as_str());
        let tags = it.tags.join(", ");
        let note = if it.item_type == ItemType::Note {
            get(StdField::Body)
        } else {
            get(StdField::Notes)
        };
        let cells: [&str; 10] = [
            &it.title,
            it.urls.first().map_or("", String::as_str),
            get(StdField::Username),
            get(StdField::Password),
            note,
            get(StdField::TotpSeed),
            kind,
            folder,
            &tags,
            if it.favorite { "1" } else { "0" },
        ];
        let mut row = Vec::with_capacity(10);
        for c in cells {
            let (cell, changed) = neutralize(c);
            report.neutralized_cells += usize::from(changed);
            row.push(cell);
        }
        w.write_record(row.iter().map(|c| c.as_bytes()))
            .map_err(|_| ImportError::Io)?;
        report.exported += 1;
    }
    let bytes = w.into_inner().map_err(|_| ImportError::Io)?;
    Ok(CsvExport {
        bytes: Zeroizing::new(bytes),
        report,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interchange::{ImportLimits, parse_csv};

    fn login(title: &str, user: &str, pw: &str) -> ExportItem {
        ExportItem {
            id: [1; 16],
            item_type: ItemType::Login,
            title: title.into(),
            folder: None,
            favorite: false,
            fields: vec![
                (StdField::Username, Zeroizing::new(user.into())),
                (StdField::Password, Zeroizing::new(pw.into())),
            ],
            urls: vec!["https://a.example".into(), "https://b.example".into()],
            tags: vec!["x".into(), "y".into()],
            custom: vec![],
            history: vec![],
        }
    }

    #[test]
    fn neutralize_rules() {
        for (cell, changed) in [
            ("=1+1", true),
            ("+x", true),
            ("-x", true),
            ("@SUM(A1)", true),
            ("\tx", true),
            ("\rx", true),
            ("plain", false),
            ("", false),
            ("a=b", false),
            (" =x", false),
        ] {
            let (out, c) = neutralize(cell);
            assert_eq!(c, changed, "{cell:?}");
            if changed {
                assert_eq!(out, format!("'{cell}"));
            } else {
                assert_eq!(out, cell);
            }
        }
    }

    #[test]
    fn csv_injection_cells_are_neutralised_in_the_export() {
        let items = vec![login("=HYPERLINK(\"http://evil\")", "@user", "-CANARY-pw")];
        let out = write_csv(&items, &[]).unwrap();
        let text = String::from_utf8(out.bytes.to_vec()).unwrap();
        assert!(text.contains("'=HYPERLINK"), "{text}");
        assert!(text.contains("'@user"));
        assert!(text.contains("'-CANARY-pw"));
        assert_eq!(out.report.neutralized_cells, 3);
        // No cell in the file starts with a dangerous character.
        let mut r = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(out.bytes.as_slice());
        for rec in r.records() {
            for c in rec.unwrap().iter() {
                assert!(!c.starts_with(['=', '+', '-', '@', '\t', '\r']), "{c:?}");
            }
        }
    }

    #[test]
    fn report_counts_and_round_trip_through_the_importer() {
        let mut card = login("Visa", "", "");
        card.item_type = ItemType::Card;
        card.urls.clear();
        let items = vec![login("Example, \"Inc\"", "alice", "CANARY-pw"), card];
        let out = write_csv(&items, &[]).unwrap();
        assert_eq!(
            (
                out.report.exported,
                out.report.skipped_unsupported,
                out.report.extra_urls_dropped
            ),
            (1, 1, 1)
        );
        let b = parse_csv(&out.bytes, &ImportLimits::default()).unwrap();
        assert_eq!(b.items.len(), 1);
        assert_eq!(b.items[0].title, "Example, \"Inc\"");
        assert_eq!(b.items[0].tags, ["x", "y"]);
    }

    #[test]
    fn debug_has_no_secret() {
        let out = write_csv(&[login("t", "u", "CANARY-secret")], &[]).unwrap();
        assert!(!format!("{out:?}").contains("CANARY-secret"));
    }
}
