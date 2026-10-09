//! `bench kdf` and `bench search`.

use std::time::{Duration, Instant};

use arya_vault_crypto::kdf::{self, KdfParams};
use arya_vault_crypto::rng::OsRng;
use arya_vault_generator::wordlist;
use arya_vault_vault::{ItemType, ListFilter, NewItem, Page, SearchQuery, StdField};
use serde_json::json;

use super::Ctx;
use crate::args::{BenchCmd, BenchKdfArgs, BenchSearchArgs};
use crate::error::{CliError, Result};
use crate::layout::VaultDir;

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

/// `(p50, p95, max)` in milliseconds.
fn percentiles(samples: &mut [Duration]) -> (f64, f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    samples.sort_unstable();
    let at = |q: f64| {
        let idx = ((samples.len() as f64 - 1.0) * q).round() as usize;
        ms(samples[idx.min(samples.len() - 1)])
    };
    (at(0.5), at(0.95), ms(samples[samples.len() - 1]))
}

pub fn run(ctx: &Ctx, cmd: BenchCmd) -> Result<()> {
    match cmd {
        BenchCmd::Kdf(a) => kdf_bench(ctx, &a),
        BenchCmd::Search(a) => search_bench(ctx, &a),
    }
}

fn kdf_bench(ctx: &Ctx, a: &BenchKdfArgs) -> Result<()> {
    let max_m_kib = a.max_m_mib.saturating_mul(1024);
    let started = Instant::now();
    let params: KdfParams = kdf::calibrate(a.target_ms, max_m_kib, &mut OsRng)?;
    let calibration = started.elapsed();
    let mut times = Vec::new();
    for _ in 0..a.runs.max(1) {
        let t = Instant::now();
        let _mk = kdf::derive_master_key("CANARY-bench-calibration-password", &params)?;
        times.push(t.elapsed());
    }
    let (p50, _, max) = percentiles(&mut times.clone());
    ctx.out.lines(
        &[
            format!(
                "chosen: argon2id m={} KiB ({} MiB) t={} p={}",
                params.m_kib,
                params.m_kib / 1024,
                params.t,
                params.p
            ),
            format!(
                "target: {} ms; calibration took {:.0} ms",
                a.target_ms,
                ms(calibration)
            ),
            format!(
                "unlock-equivalent derivations: {} (median {p50:.0} ms, max {max:.0} ms)",
                times
                    .iter()
                    .map(|t| format!("{:.0} ms", ms(*t)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ],
        &json!({
            "params": { "m_kib": params.m_kib, "t": params.t, "p": params.p },
            "target_ms": a.target_ms,
            "calibration_ms": ms(calibration),
            "derivation_ms": times.iter().map(|t| ms(*t)).collect::<Vec<_>>(),
            "median_ms": p50,
            "max_ms": max,
        }),
    )
}

fn search_bench(ctx: &Ctx, a: &BenchSearchArgs) -> Result<()> {
    if a.items == 0 {
        return Err(CliError::usage("--items must be at least 1"));
    }
    let scratch = tempfile::tempdir()?;
    let dir = VaultDir::new(scratch.path().join("bench-vault"));
    let password = "CANARY-bench-master-password-0123456789";
    let words = wordlist();
    let word = |i: usize| words[(i.wrapping_mul(7919)) % words.len()];

    let t = Instant::now();
    let _rk = dir.create(password, a.kdf_profile)?;
    let create = t.elapsed();

    let mut u = dir.unlock(password)?;
    let mut writes = Vec::with_capacity(a.items);
    let populate = Instant::now();
    for i in 0..a.items {
        let w1 = word(i);
        let w2 = word(i + 1);
        let new = NewItem::new(ItemType::Login, &format!("{w1} {w2} account {i}"))
            .with_field(StdField::Username, &format!("user{i}@example.com"))
            .with_field(
                StdField::Password,
                &format!("CANARY-bench-item-password-{i}"),
            );
        let mut new = new;
        new.urls = vec![format!("https://{w1}.example.com/login")];
        let t = Instant::now();
        u.vault.create_item(new)?;
        writes.push(t.elapsed());
    }
    let populate = populate.elapsed();
    u.vault.close()?;

    let (w50, w95, wmax) = percentiles(&mut writes);

    // Cold unlock: password -> Argon2id -> VK -> SQLCipher open -> first page of the list.
    let t = Instant::now();
    let mut u = dir.unlock(password)?;
    let unlocked = t.elapsed();
    let first_page = u.vault.list(
        &ListFilter::default(),
        Page {
            offset: 0,
            limit: 50,
        },
    )?;
    let cold = t.elapsed();

    // Header-only unlock (Argon2 + header) for the KDF share.
    let t = Instant::now();
    let _ = dir.check_password(password)?;
    let kdf_only = t.elapsed();

    let mut searches = Vec::with_capacity(a.queries);
    let mut hits = 0usize;
    let mut slowest: (Duration, String, usize, usize) = (Duration::ZERO, String::new(), 0, 0);
    for q in 0..a.queries {
        // A whole word and a 3-letter prefix, alternating.
        let w = word(q * 31 + 5);
        let text = if q.is_multiple_of(2) {
            w.to_owned()
        } else {
            w.chars().take(3).collect()
        };
        let t = Instant::now();
        let r = u.vault.search(&SearchQuery {
            text: text.clone(),
            filter: ListFilter::default(),
            limit: 100,
        })?;
        let took = t.elapsed();
        searches.push(took);
        hits += r.len();
        if took > slowest.0 {
            slowest = (took, text, r.len(), q);
        }
    }
    u.vault.close()?;
    let (s50, s95, smax) = percentiles(&mut searches);
    let active = dir.active_header()?;
    let k = &active.header.kdf;

    ctx.out.lines(
        &[
            format!(
                "scratch vault: {} items, kdf argon2id m={} MiB t={} p={}",
                a.items,
                k.m_kib / 1024,
                k.t,
                k.p
            ),
            format!("vault create (incl. calibration): {:.0} ms", ms(create)),
            format!(
                "item write latency over {} writes: p50 {w50:.2} ms, p95 {w95:.2} ms, max {wmax:.2} ms (total {:.1} s)",
                a.items,
                populate.as_secs_f64()
            ),
            format!(
                "cold unlock + open: {:.0} ms (password -> database open {:.0} ms; first page of {} items {:.0} ms total)",
                ms(cold),
                ms(unlocked),
                first_page.len(),
                ms(cold)
            ),
            format!("Argon2id + header unwrap alone: {:.0} ms", ms(kdf_only)),
            format!(
                "FTS search over {} items, {} queries ({} hits): p50 {s50:.2} ms, p95 {s95:.2} ms, max {smax:.2} ms",
                a.items, a.queries, hits
            ),
            format!(
                "slowest query: `{}` (#{} of {}, {} results) took {:.2} ms",
                slowest.1,
                slowest.3 + 1,
                a.queries,
                slowest.2,
                ms(slowest.0)
            ),
        ],
        &json!({
            "items": a.items,
            "kdf": { "m_kib": k.m_kib, "t": k.t, "p": k.p },
            "create_ms": ms(create),
            "write_ms": { "p50": w50, "p95": w95, "max": wmax, "total_s": populate.as_secs_f64() },
            "cold_unlock_open_ms": ms(cold),
            "unlock_to_open_ms": ms(unlocked),
            "kdf_only_ms": ms(kdf_only),
            "search_ms": { "queries": a.queries, "hits": hits, "p50": s50, "p95": s95, "max": smax },
            "slowest_query": { "text": slowest.1, "index": slowest.3, "results": slowest.2, "ms": ms(slowest.0) },
        }),
    )
}
