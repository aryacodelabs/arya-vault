//! L02 measurements (ignored by default; run by hand on the reference machine):
//!
//! ```text
//! cargo test -p arya-vault-ffi --release --test l02_bench -- --ignored --nocapture
//! ```
//!
//! Builds a vault of 20,000 fake items (obvious canary text), then reports: time to fill, cold
//! unlock (Default profile), first list page, search, and a scan of every file in the vault
//! folder for the item text (UTF-8 and UTF-16LE).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

mod common;

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use arya_vault_ffi::api::dto::*;
use arya_vault_ffi::api::{items, lifecycle};
use common::*;

const N: u32 = 20_000;
const C: &str = "CANARY-L02-zq7";

fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            files(&p, out);
        } else {
            out.push(p);
        }
    }
}

fn utf16(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(u16::to_le_bytes).collect()
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
#[ignore = "measurement run, takes a while"]
fn twenty_thousand_items() {
    let _g = serial();
    let f = unlocked();

    let t = Instant::now();
    for i in 0..N {
        items::create_item(NewItem {
            item_type: ItemType::Login,
            title: format!("{C}-title-{i:05}"),
            fields: HashMap::from([
                (StdField::Username, format!("{C}-user-{i:05}")),
                (StdField::Password, format!("{C}-password-{i:05}")),
                (StdField::Notes, format!("{C}-notes-{i:05}")),
            ]),
            urls: vec![format!("https://example.invalid/{i}")],
            tags: vec![],
            folder_id: None,
            custom: vec![],
        })
        .unwrap();
    }
    println!("create {N} items: {:?}", t.elapsed());

    let filter = ListFilter::default;
    let page = |offset| Page { offset, limit: 50 };
    let t = Instant::now();
    let first = items::list(filter(), page(0)).unwrap();
    println!("list first page ({} rows): {:?}", first.len(), t.elapsed());
    let t = Instant::now();
    let deep = items::list(filter(), page(N - 50)).unwrap();
    println!("list last page ({} rows): {:?}", deep.len(), t.elapsed());

    for limit in [1u32, 10, 50, 200] {
        let t = Instant::now();
        let rows = items::list(filter(), Page { offset: 0, limit }).unwrap();
        println!(
            "list limit {limit}: {} rows in {:?}",
            rows.len(),
            t.elapsed()
        );
    }
    let ids: Vec<String> = first.iter().map(|r| r.id.clone()).collect();
    let t = Instant::now();
    for id in &ids {
        items::get_item(id.clone()).unwrap();
    }
    println!("get_item x{}: {:?}", ids.len(), t.elapsed());

    for q in [
        format!("{C}-title-12345"),
        "canary".to_owned(),
        "zzzz".to_owned(),
    ] {
        let t = Instant::now();
        let hits = items::search(SearchQuery {
            text: q.clone(),
            filter: filter(),
            page: page(0),
        })
        .unwrap();
        println!("search '{q}': {} hits in {:?}", hits.len(), t.elapsed());
    }

    lifecycle::lock().unwrap();
    let t = Instant::now();
    lifecycle::unlock(pw(PASSWORD)).unwrap();
    println!("unlock ({N} items, Low profile): {:?}", t.elapsed());
    let t = Instant::now();
    assert_eq!(items::item_count().unwrap(), u64::from(N));
    println!("item_count: {:?}", t.elapsed());
    lifecycle::lock().unwrap();

    // `ARYA_BENCH_OUT=<dir>` keeps a copy of the 20,000-item vault for app-level measurements
    // (the master password is `PASSWORD` in tests/common).
    if let Some(out) = std::env::var_os("ARYA_BENCH_OUT") {
        std::fs::create_dir_all(&out).unwrap();
        let mut src = Vec::new();
        files(f.dir.path(), &mut src);
        for p in src {
            std::fs::copy(&p, Path::new(&out).join(p.file_name().unwrap())).unwrap();
        }
        println!("vault copy written to {}", Path::new(&out).display());
    }

    let mut all = Vec::new();
    files(f.dir.path(), &mut all);
    let size: u64 = all
        .iter()
        .map(|p| std::fs::metadata(p).unwrap().len())
        .sum();
    println!("vault folder: {} files, {} bytes", all.len(), size);
    let needles = [
        C.to_owned(),
        "password-".to_owned(),
        "example.invalid".to_owned(),
        PASSWORD.to_owned(),
    ];
    let mut found = 0;
    for p in &all {
        let bytes = std::fs::read(p).unwrap();
        for n in &needles {
            if contains(&bytes, n.as_bytes()) || contains(&bytes, &utf16(n)) {
                println!("FOUND '{n}' in {}", p.display());
                found += 1;
            }
        }
    }
    assert_eq!(found, 0, "item text found in the vault folder");
    println!(
        "canary scan: {} needles x {} files: none found",
        needles.len(),
        all.len()
    );
}
