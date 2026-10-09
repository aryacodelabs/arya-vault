//! SEC-S06 for the two storage paths `crash_safety.rs` does not cover: `Db::create` and
//! `Db::rekey`.
//!
//! As there, a child process (this test binary, re-executed) does the operation in a loop and is
//! SIGKILLed / `TerminateProcess`ed at pseudo-random points, >= 200 times per operation; the
//! parent then checks what is on disk.
//!
//! * **create.** The file at the final path is either absent or a complete, openable database
//!   (never a half-initialised one), `integrity_check` passes, every create the child
//!   acknowledged is present, and a leftover `<path>.creating` never blocks the next create.
//!   Why that holds: the database is built, checkpointed and closed under `<path>.creating`,
//!   and only then renamed over `<path>` (one atomic `rename`), so no reader can see the
//!   final name before the content is complete.
//! * **rekey.** After every kill the file opens under exactly one of the two keys involved
//!   (the old one: the rekey did not commit; the new one: it did), passes `integrity_check`
//!   and holds every row. Why that holds is what this test measures rather than assumes: it
//!   relies on SQLCipher rewriting all pages in one WAL transaction (`PRAGMA rekey`), and the
//!   counts below show how often a kill landed on each side of that commit.
//!
//! Scope: process death, not power loss or a lying disk. Killed children send coverage data to
//! the null device (a SIGKILLed process leaves a corrupt .profraw behind).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use arya_vault_storage::{CreateParams, Db, DbKey, Result, Store};

const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
const CREATE_DIR: &str = "ARYA_CRASH_CREATE_DIR";
const REKEY_DB: &str = "ARYA_CRASH_REKEY_DB";
const REKEY_START: &str = "ARYA_CRASH_REKEY_START";
const ROWS: usize = 3_000;
const CANARY: &[u8] = b"CANARY-7F3A-CRASH-DO-NOT-USE";

fn iterations() -> usize {
    std::env::var("ARYA_CRASH_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200)
}

fn key(i: u64) -> DbKey {
    let mut b = [0u8; 32];
    for (j, byte) in b.iter_mut().enumerate() {
        *byte = (i as u8)
            .wrapping_mul(31)
            .wrapping_add(j as u8)
            .wrapping_add(1);
    }
    b[..8].copy_from_slice(&i.to_le_bytes());
    DbKey::from_bytes(b)
}

fn params() -> CreateParams {
    CreateParams {
        vault_id: [1; 16],
        device_id: [2; 16],
        epoch: 1,
        header_version: 1,
    }
}

/// Small deterministic generator so failures are reproducible.
struct Lcg(u64);
impl Lcg {
    fn next(&mut self, bound: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % bound
    }
}

fn spawn(test: &str, env: &[(&str, &str)]) -> Child {
    let mut c = Command::new(std::env::current_exe().unwrap());
    c.args(["--exact", test, "--nocapture", "--test-threads=1"])
        .env("LLVM_PROFILE_FILE", NULL_DEVICE)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for (k, v) in env {
        c.env(k, v);
    }
    c.spawn().unwrap()
}

/// Reads child lines until `wants(line)` has matched `count` times, handing every line to `on`.
fn read_until(child: &mut Child, count: u64, mut on: impl FnMut(&str) -> bool) {
    if count == 0 {
        return;
    }
    let mut seen = 0;
    let reader = BufReader::new(child.stdout.as_mut().unwrap());
    for line in reader.lines() {
        let Ok(line) = line else { break };
        if on(&line) {
            seen += 1;
            if seen >= count {
                break;
            }
        }
    }
}

fn kill(child: &mut Child) {
    child.kill().unwrap();
    child.wait().unwrap();
}

// ------------------------------------------------------------------------------------ create

fn db_path(dir: &Path, n: u64) -> PathBuf {
    dir.join(format!("v{n}.db"))
}

/// Child: creates databases `v1.db`, `v2.db`, ... forever, acknowledging each.
#[test]
fn crash_create_child() {
    let Ok(dir) = std::env::var(CREATE_DIR) else {
        return;
    };
    let dir = PathBuf::from(dir);
    // Continue after the databases that exist. A stale `.creating` is deliberately not skipped:
    // the child's next create must overwrite it (a leftover never blocks a create).
    let mut n = 1;
    while db_path(&dir, n).exists() {
        n += 1;
    }
    let out = std::io::stdout();
    loop {
        let path = db_path(&dir, n);
        let db = Db::create(&path, key(n), &params()).unwrap();
        db.close().unwrap();
        let mut lock = out.lock();
        writeln!(lock, "ACK {n}").unwrap();
        lock.flush().unwrap();
        n += 1;
    }
}

#[test]
fn sec_s06_kill_during_create_never_leaves_a_half_built_database() {
    let dir = tempfile::tempdir().unwrap();
    let mut rng = Lcg(0xC4EA_7E01);
    let mut acked_total = 0u64;
    let (mut mid_create, mut between) = (0, 0);
    // Databases already checked; a complete one does not change afterwards.
    let mut verified = std::collections::HashSet::new();
    for i in 0..iterations() {
        let mut child = spawn(
            "crash_create_child",
            &[(CREATE_DIR, dir.path().to_str().unwrap())],
        );
        let target = rng.next(6); // 0 = kill during start-up / the very first create
        let mut last_ack = 0u64;
        read_until(&mut child, target, |l| {
            l.split_once("ACK ")
                .and_then(|(_, n)| n.trim().parse::<u64>().ok())
                .is_some_and(|n| {
                    last_ack = n;
                    true
                })
        });
        std::thread::sleep(Duration::from_micros(rng.next(3_000)));
        kill(&mut child);
        // Let go of the pipe's remaining output so nothing is left half-read.
        drop(child.stdout.take());

        // Everything at a final path is complete and openable; everything acknowledged exists.
        let mut finals = Vec::new();
        let mut creating = Vec::new();
        for e in std::fs::read_dir(dir.path()).unwrap() {
            let name = e.unwrap().file_name().into_string().unwrap();
            if let Some(n) = name.strip_prefix('v').and_then(|r| r.strip_suffix(".db")) {
                finals.push(n.parse::<u64>().unwrap());
            } else if name.ends_with(".db.creating") {
                creating.push(name);
            }
        }
        for n in 1..=last_ack {
            assert!(
                finals.contains(&n),
                "iteration {i}: acknowledged create v{n}.db is missing"
            );
        }
        for n in finals.iter().filter(|n| verified.insert(**n)) {
            let mut db = Db::open(&db_path(dir.path(), *n), key(*n))
                .unwrap_or_else(|e| panic!("iteration {i}: v{n}.db does not open: {e}"));
            db.integrity_check()
                .unwrap_or_else(|e| panic!("iteration {i}: v{n}.db integrity: {e}"));
            let (vault, epoch) = db
                .with_read(|t| -> Result<_> { Ok((t.meta_get("vault_id")?, t.meta_get("epoch")?)) })
                .unwrap();
            assert_eq!(
                vault.as_deref(),
                Some(&[1u8; 16][..]),
                "iteration {i}: v{n}.db meta"
            );
            assert_eq!(epoch.as_deref(), Some(&b"1"[..]));
            db.close().unwrap();
        }
        if creating.is_empty() {
            between += 1;
        } else {
            mid_create += 1;
            // A leftover must not block creating the same name again.
            let n = finals.iter().max().copied().unwrap_or(0) + 1;
            let p = db_path(dir.path(), n);
            Db::create(&p, key(n), &params())
                .unwrap_or_else(|e| panic!("iteration {i}: create after a crash failed: {e}"))
                .close()
                .unwrap();
            assert!(
                !dir.path().join(format!("v{n}.db.creating")).exists(),
                "iteration {i}: a successful create left its .creating file behind"
            );
        }
        acked_total = acked_total.max(last_ack);
    }
    println!(
        "{} kills survived: {mid_create} landed inside a create (a .creating leftover), {between} between creates; up to {acked_total} acknowledged creates",
        iterations()
    );
    assert!(
        acked_total > 0,
        "the child never finished a create; test is vacuous"
    );
    assert!(
        mid_create >= 5,
        "only {mid_create} kills landed inside a create; the test is not exercising the window"
    );
}

// ------------------------------------------------------------------------------------ rekey

fn seed_rekey_db(path: &Path) {
    let mut db = Db::create(path, key(0), &params()).unwrap();
    for chunk in 0..(ROWS / 500) {
        db.with_tx(|t| -> Result<()> {
            for r in 0..500 {
                let n = i64::try_from(chunk * 500 + r).unwrap();
                t.append_local_op(
                    &[7; 16],
                    &format!("k{n}"),
                    Some(&[CANARY, &[0u8; 900]].concat()),
                    n,
                    None,
                )?;
            }
            Ok(())
        })
        .unwrap();
    }
    db.close().unwrap();
}

/// Child: re-keys `key(start)` -> `key(start+1)` -> ... forever.
#[test]
fn crash_rekey_child() {
    let (Ok(path), Ok(start)) = (std::env::var(REKEY_DB), std::env::var(REKEY_START)) else {
        return;
    };
    let mut at: u64 = start.parse().unwrap();
    let mut db = Db::open(Path::new(&path), key(at)).unwrap();
    let out = std::io::stdout();
    loop {
        {
            let mut l = out.lock();
            writeln!(l, "BEGIN {}", at + 1).unwrap();
            l.flush().unwrap();
        }
        db.rekey(&key(at), &key(at + 1)).unwrap();
        at += 1;
        let mut l = out.lock();
        writeln!(l, "ACK {at}").unwrap();
        l.flush().unwrap();
    }
}

#[test]
fn sec_s06_kill_during_rekey_leaves_a_database_that_opens_under_one_of_the_two_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    seed_rekey_db(&path);

    // How long one rekey of this file takes here (median of 5, on a copy), so the kill delays
    // can be spread over exactly that span and the kills that surely landed *inside* a rekey
    // can be counted.
    let rekey_time = {
        let scratch = dir.path().join("timing.db");
        std::fs::copy(&path, &scratch).unwrap();
        let mut db = Db::open(&scratch, key(0)).unwrap();
        let mut times: Vec<Duration> = (0..5)
            .map(|i| {
                let t = std::time::Instant::now();
                db.rekey(&key(i), &key(i + 1)).unwrap();
                t.elapsed()
            })
            .collect();
        db.close().unwrap();
        std::fs::remove_file(&scratch).unwrap();
        times.sort();
        times[2]
    };
    let span = u64::try_from(rekey_time.as_micros()).unwrap().max(1_000) * 3 / 2;

    let mut rng = Lcg(0x4E4B_E701);
    let mut current = 0u64; // the key the file is known to open under
    let (mut kept_old, mut took_new, mut inside) = (0u32, 0u32, 0u32);
    for i in 0..iterations() {
        let mut child = spawn(
            "crash_rekey_child",
            &[
                (REKEY_DB, path.to_str().unwrap()),
                (REKEY_START, &current.to_string()),
            ],
        );
        // Let the child complete a random number of rekeys, then kill it somewhere in the next.
        let target = rng.next(4);
        let mut last_ack = current;
        let mut began = false;
        let mut acks = 0;
        let reader = BufReader::new(child.stdout.as_mut().unwrap());
        for line in reader.lines() {
            let Ok(line) = line else { break };
            if let Some((_, n)) = line.split_once("ACK ") {
                last_ack = n.trim().parse().unwrap();
                acks += 1;
                began = false;
            } else if line.contains("BEGIN ") {
                began = true;
                if acks >= target {
                    break;
                }
            }
        }
        let delay = Duration::from_micros(rng.next(span));
        let began_at = std::time::Instant::now();
        std::thread::sleep(delay);
        kill(&mut child);
        // Less than half a typical rekey after its BEGIN line: it cannot have finished.
        if began && began_at.elapsed() < rekey_time / 2 {
            inside += 1;
        }
        drop(child.stdout.take());

        // The file opens under the last acknowledged key (rekey did not commit) or the next one
        // (it did), never under both and never under neither.
        let old = Db::open(&path, key(last_ack));
        let new = Db::open(&path, key(last_ack + 1));
        let (mut db, which) = match (old, new) {
            (Ok(db), Err(_)) => {
                kept_old += 1;
                (db, last_ack)
            }
            (Err(_), Ok(db)) => {
                took_new += 1;
                (db, last_ack + 1)
            }
            (Ok(_), Ok(_)) => panic!(
                "iteration {i}: opens under both keys {last_ack} and {}",
                last_ack + 1
            ),
            (Err(a), Err(b)) => panic!(
                "iteration {i}: opens under neither key {last_ack} ({a}) nor {} ({b})",
                last_ack + 1
            ),
        };
        db.integrity_check()
            .unwrap_or_else(|e| panic!("iteration {i}: integrity after kill: {e}"));
        let rows = db
            .with_read(|t| -> Result<usize> { Ok(t.pending_local_ops(usize::MAX >> 1)?.len()) })
            .unwrap();
        assert_eq!(
            rows, ROWS,
            "iteration {i}: rows lost or duplicated by a killed rekey"
        );
        db.close().unwrap();
        current = which;
    }
    println!(
        "{} kills survived (one rekey takes ~{rekey_time:?}): {inside} landed certainly inside a rekey; {kept_old} left the old key, {took_new} the new one; ended at key {current}",
        iterations()
    );
    assert!(current > 0, "no rekey ever committed; test is vacuous");
    assert!(
        inside >= 50,
        "only {inside} kills landed inside a rekey; the test is not exercising the window"
    );
    assert!(
        kept_old > 0 && took_new > 0,
        "kills only ever landed on one side of the commit ({kept_old} old / {took_new} new)"
    );
}
