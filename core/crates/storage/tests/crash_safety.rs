//! SEC-S06: crash safety under abrupt process termination.
//!
//! A child process (this same test binary, re-executed) commits transactions
//! in a loop and is killed (SIGKILL / TerminateProcess) at pseudo-random
//! points, >= 200 times. After every kill the database is reopened and must
//! pass `integrity_check`, must contain every transaction the child reported as
//! committed, and must show each transaction atomically (the marker row and
//! the rows written in the same transaction always agree).
//!
//! Scope: this validates atomicity/durability against *process* death. It does
//! not simulate power loss or a lying disk (fsync is exercised by
//! `synchronous = FULL` but not fault-injected).
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use arya_vault_storage::{CreateParams, Db, DbKey, ItemFilter, Result, Store};

const KEY: [u8; 32] = [0x5A; 32];
const DB_ENV: &str = "ARYA_CRASH_CHILD_DB";
/// Where killed children send coverage profile data so nothing corrupt is left for the merge step.
const NULL_DEVICE: &str = if cfg!(windows) { "NUL" } else { "/dev/null" };
const CANARY: &[u8] = b"CANARY-7F3A-CRASH-DO-NOT-USE";

fn marker(t: &impl Store) -> i64 {
    t.meta_get("marker")
        .unwrap()
        .map_or(0, |v| i64::from_le_bytes(v.try_into().unwrap()))
}

/// Child entry point: only active when re-executed by `kill_at_random_points`.
#[test]
fn crash_child() {
    let Ok(path) = std::env::var(DB_ENV) else {
        return;
    };
    let mut db = Db::open(Path::new(&path), DbKey::from_bytes(KEY)).unwrap();
    let mut n = db.with_read(|t| -> Result<i64> { Ok(marker(t)) }).unwrap();
    let out = std::io::stdout();
    loop {
        n += 1;
        db.with_tx(|t| -> Result<()> {
            t.meta_set("marker", &n.to_le_bytes())?;
            // Several rows per transaction so a half-applied commit would be visible.
            for k in 0..5 {
                t.append_local_op(&[7; 16], &format!("k{k}"), Some(CANARY), n, None)?;
            }
            Ok(())
        })
        .unwrap();
        let mut lock = out.lock();
        writeln!(lock, "ACK {n}").unwrap();
        lock.flush().unwrap();
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

#[test]
fn kill_at_random_points_never_corrupts_or_loses_commits() {
    let iterations: usize = std::env::var("ARYA_CRASH_ITERATIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(200);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("vault.db");
    let params = CreateParams {
        vault_id: [1; 16],
        device_id: [2; 16],
        epoch: 1,
        header_version: 1,
    };
    Db::create(&path, DbKey::from_bytes(KEY), &params)
        .unwrap()
        .close()
        .unwrap();

    let exe = std::env::current_exe().unwrap();
    let mut rng = Lcg(0xA27A_0001);
    let mut previous = 0i64;
    for i in 0..iterations {
        let mut child = Command::new(&exe)
            .args(["--exact", "crash_child", "--nocapture", "--test-threads=1"])
            .env(DB_ENV, &path)
            // The child is SIGKILLed on purpose. Under `cargo llvm-cov` a killed process leaves a
            // corrupt .profraw behind and the report step then fails ("no profile can be merged").
            .env("LLVM_PROFILE_FILE", NULL_DEVICE)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        // Let the child commit a random number of transactions (0 = kill during startup/open).
        let target = rng.next(40);
        let mut last_ack = 0i64;
        let mut acks = 0u64;
        let reader = BufReader::new(child.stdout.take().unwrap());
        if target > 0 {
            for line in reader.lines() {
                let line = line.unwrap();
                if let Some(n) = line.strip_prefix("ACK ") {
                    last_ack = n.parse().unwrap();
                    acks += 1;
                    if acks >= target {
                        break;
                    }
                }
            }
        }
        // Occasionally add jitter so the kill lands inside a commit, not just between them.
        if rng.next(3) == 0 {
            std::thread::sleep(std::time::Duration::from_micros(rng.next(2_000)));
        }
        child.kill().unwrap();
        child.wait().unwrap();

        let mut db = Db::open(&path, DbKey::from_bytes(KEY))
            .unwrap_or_else(|e| panic!("iteration {i}: reopen failed: {e}"));
        db.integrity_check()
            .unwrap_or_else(|e| panic!("iteration {i}: integrity: {e}"));
        let (m, ops) = db
            .with_read(|t| -> Result<(i64, usize)> {
                Ok((marker(t), t.pending_local_ops(usize::MAX >> 1)?.len()))
            })
            .unwrap();
        assert!(
            m >= last_ack,
            "iteration {i}: committed tx {last_ack} lost (marker {m})"
        );
        assert!(
            m >= previous,
            "iteration {i}: marker went backwards ({previous} -> {m})"
        );
        assert_eq!(
            i64::try_from(ops).unwrap(),
            m * 5,
            "iteration {i}: half-applied transaction (marker {m}, ops {ops})"
        );
        assert!(
            db.with_read(|t| t.list_items(ItemFilter::default()))
                .unwrap()
                .is_empty()
        );
        previous = m;
        db.close().unwrap();
    }
    assert!(
        previous > 0,
        "the child never committed anything; test is vacuous"
    );
    println!("{iterations} kills survived, {previous} transactions committed in total");
}
