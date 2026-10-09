# M1 exit check

**Scope:** milestone M1 "Rust core" (`docs/10-roadmap.md`), tasks T00-T08 · **Evidence date:** 2026-10-09 · **Code state:** `main` at the T07 merge (`ae9a179`) plus the T08 branch (`m1/t08-cli`) · **Author:** Claude (cloud session), so this is a **self-assessment by the same agent that wrote most of the code**. It lists evidence; it does not replace the human review that `docs/08` asks for (the `RV` and `AU` verification keys), and shared blind spots are likely.

Status words, used strictly:

* **Verified**: the stated verification method (UT/PT/IT/FZ) ran and passes, and the evidence is named so you can re-run it.
* **Partially verified**: part of the requirement or part of the method is evidenced; the gap is stated.
* **Not verified**: no evidence, or the evidence contradicts the requirement.

`docs/08` has no milestone column. "Assigned to M1" below means: every `SEC-C*` (the roadmap's exit criterion), plus every other requirement that a T00-T08 task prompt or PR claims to cover. Requirements that need sync, a UI or a platform are listed at the end as *outside M1*.

---

## 1. Read this first: what is not verified

These are the honest gaps. Nothing here is hidden elsewhere in the document.

| # | Item | Why it matters | Status |
|---|---|---|---|
| 1 | **"Fuzzers run 1 h clean"** (roadmap exit criterion) | Eight real fuzz targets exist (header, envelope, CBOR, path, recovery key, and the CSV, Bitwarden and AryaVault importers) plus the `smoke` placeholder. The longest runs on record are 5-10 min per target, run locally by the authors (PR #10, #14). The `Nightly` workflow has run **twice, both by manual dispatch, before any real target existed** (run 2 finished in 10 s). It has never fuzzed a real target in CI, and no run was 1 h. | **Not verified** |
| 2 | **SEC-C06 memory inspection** (`MT`) and "zeroize on **lock**" | Wipe-on-drop is unit-tested on the live buffers (`sec_c06_*`). There is no memory-inspection test, no `mlock`, and no "lock" operation in the core yet (the CLI process exits; the vault `close()` zeroizes the DB key). Stale copies (moves, registers, allocator reuse, swap) are not ruled out. | **Partially verified** |
| 3 | **Human review** (`RV`, and two-maintainer review of `crypto`, SEC-R05) | `SEC-C01`, `C03`, `S05`, `H04` list `RV`. `CODEOWNERS` names one owner; its own comment says a second is needed. `.gitleaks.toml`, `docs/13-export-format.md` and the CLI are not covered by `CODEOWNERS`. | **Not verified** |
| 4 | **Owner approval of `docs/13-export-format.md`** (marked Draft in PR #14, merged) and the three open choices in it (key-derivation label reuse, wrapper shape, history without timestamps) | The export format is a compatibility promise once users hold files. | **Not verified** |
| 5 | **The interactive TTY prompt** of the CLI (`rpassword`) | CI has no TTY, so every test uses `--password-stdin`. The no-echo path was exercised by hand only for its failure mode (no TTY -> error, never falls back to stdin). | **Not verified** |
| 6 | **Windows and macOS** for the T08 tests | Only Linux was run locally. CI runs `cargo test --workspace` on all three, so this resolves when the T08 PR's checks finish. | **Pending CI** |
| 7 | **SEC-A06 (VK rotation, `MUST`)** | Not implemented anywhere (`docs/04` §10). The roadmap does not put it in M1, but it is a `MUST`. | **Not verified** (not built) |
| 8 | **Search worst case < 100 ms** | Typical queries are fast (p50 about 2 ms, p95 about 10 ms on 20,000 items) but a broad prefix that matches most of the index took **138-149 ms** in four of five runs (section 4). | **Partially verified** |
| 9 | **Argon2 calibration "converges to 0.5-1.0 s"** | The default calibration target (750 ms) produced parameters that take **about 1.0 s** (median 998 ms, max 1,054 ms) here, because calibration moves in coarse steps (see section 4). Edge of the range, not clearly inside it. | **Partially verified** |
| 10 | **Coverage >= 90 % in crypto and vault** | Measured locally: crypto 98.5 %, vault 95.9 % of lines (section 3). CI's coverage job is informational (no threshold) and has been red on a known profraw flake, so the gate is not enforced anywhere. | **Verified locally; not enforced in CI** |

---

## 2. Roadmap M1 scope checklist (`docs/10` M1)

| Item | Status | Evidence |
|---|---|---|
| Crates: crypto, storage, vault, generator | Verified | PRs #7, #10 (crypto), #9 (storage), #13 (vault), #6 (generator); all merged, CI green on 3 OS. |
| Header and envelope formats | Verified | `crypto/src/format/{header,envelope,padding,path,cbor}.rs`; `docs/04` §16 (PR #11); golden vault; Python cross-check. |
| Recovery key encode/decode | Verified | `crypto/src/recovery_key.rs` tests, `fuzz_recovery_key`, `docs/04` §4 checksum; e2e: mistyped key -> exit 2, wrong key -> exit 3. |
| Import/export (CSV, Bitwarden) | Verified (KDBX deferred) | PR #14; `interchange/*` tests; CLI e2e `csv_import_and_plaintext_export`. KDBX is out of scope by task. |
| CLI test harness for headless vault operations | Verified locally; CI pending | This PR; `core/crates/cli/tests/e2e.rs` (8 tests). |
| Test vectors | Verified | RFC 9106 / RFC 5869 cases 1-3 / draft-irtf-cfrg-xchacha / NFKD vectors (`SEC-C07`, `SEC-C10` below). |
| Property tests | Verified | `proptest` in register merge (`any_order_and_duplication_converges`, `merge_is_commutative`), vault DB state (`database_state_is_independent_of_arrival_order`), codecs, parsers (never-panic). |
| First fuzz targets | Verified (exist and build) | 8 targets besides `smoke`, all in `core/fuzz/fuzz_targets/`; built by the nightly matrix. Duration: see item 1 above. |
| Golden files | Verified | `core/testdata/golden/v1/` (header, segment, snapshot, manifest, `expected.json`); `crypto/tests/golden_v1.rs`; CLI `golden_vault_v1_opens_with_the_documented_password`. Only v1 exists, so "all previous releases" is trivially one. |
| Independent Python decryptor | Verified | `tools/crosscheck/` (PR #12), CI job `crosscheck`, negative tests in `test_negative.py`. It reads the golden vault only; it does not read the T07 export (follow-up). |

---

## 3. Exit criteria (`docs/10` M1 **Exit**)

| Criterion | Status | Evidence |
|---|---|---|
| SEC-C* requirements verified | See the table in section 5: 9 verified, 4 partially verified (C01, C03 pending `RV`; C04 test is 10^6 draws vs 10^7 in `docs/11`; C06 as in item 2 above) | Section 5. |
| 90%+ coverage in crypto/vault | **Verified (local measurement)** | `cargo llvm-cov --workspace --locked --summary-only` on the T08 branch, whole-workspace test run: **crypto 98.5 % of lines (3,099 of 3,145; 97.3 % of regions), vault 95.9 % of lines (4,371 of 4,558; 93.3 % of regions)**. Per file the lowest are `vault/src/interchange/json.rs` (89.9 % lines) and `vault/src/query.rs` (89.0 %). For context: generator 98.6 %, storage 90.8 %, cli 89.0 % (lines). Caveats: measured locally, not in CI (CI's coverage job is informational, has no threshold, and is subject to the known profraw flake); I kept only the summary table and did not capture every test-binary result line of that run, though the plain `cargo test` runs pass; line coverage says a line ran, not that it was asserted. |
| Fuzzers run 1 h clean | **Not verified** | Item 1 in section 1. |
| Benchmark: Argon2 calibration | **Partially verified** | Section 4: parameters are chosen and timed; the result lands at the top of the 0.5-1.0 s window. |
| Benchmark: 20k-item search < 100 ms | **Partially verified** | Section 4: typical queries pass by a wide margin; the worst broad prefix does not. |

---

## 4. Benchmarks

Machine: 4 vCPU Intel Xeon @ 2.10 GHz, 16 GB RAM, Linux (a cloud container, so timings vary by tens of percent between runs; **these are not reference-device numbers**, which `docs/11` §6 asks for: low-end Android, mid iPhone, Windows laptop). Release build (`lto = true`). Reproduce from `core/` with the commands shown.

**Argon2id calibration and unlock** (`arya-vault bench kdf --target-ms N --runs R`; the same code path `vault create --kdf-profile default` uses, memory capped at 256 MiB):

| Target | Chosen parameters | Time of the chosen parameters (unlock-equivalent) |
|---|---|---|
| 500 ms | m = 256 MiB, t = 3, p = 1 | 682, 702, 700 ms (median 700 ms) |
| **750 ms (default profile)** | m = 256 MiB, t = 4, p = 1 | 981, 998, 1054, 1031, 932 ms (**median 998 ms**, max 1,054 ms) |
| 1000 ms | m = 256 MiB, t = 5, p = 1 | 1020, 1062, 1095 ms (median 1,062 ms) |

Calibration itself took 1.1 s to 3.1 s. The target is a lower bound on the first accepted step, so the real cost lands one step above it (t = 3 took about 700 ms, t = 4 about 1 s). With the default target the unlock cost is at or just above the 1.0 s end of the `docs/04` §3 / `docs/11` §6 window; a target of about 600 ms would land near 0.7-0.8 s. This is a tuning decision for the owner, not a defect.

**Cold unlock with 20,000 items** (`arya-vault bench search --items 20000 --queries 1000 --kdf-profile default`; password -> Argon2id -> unwrap -> SQLCipher open -> first page of 50 items):

| Measure | Result | Target |
|---|---|---|
| Cold unlock + open (default KDF, 256 MiB / t = 4) | 868 ms and 918 ms (two runs) | < 2 s (`docs/11` §6): **met** |
| of which Argon2id + header unwrap | 919 ms and 932 ms (equal within noise) | |
| Cold unlock + open with the floor KDF (64 MiB / t = 3) | 222 ms | |

**Item write latency** (one transaction per `create_item`, WAL, `synchronous = FULL`, 20,000 writes): p50 1.9 ms, p95 3.0-3.4 ms, max 27-42 ms (total 41-45 s).

**FTS search latency** (20,000 login items, 3-letter-prefix and whole-word queries over a 7,776-word vocabulary, limit 100):

| Run | Queries | p50 | p95 | max (slowest query) |
|---|---|---|---|---|
| default KDF | 200 | 1.5 ms | 7.9 ms | 90 ms |
| default KDF | 1,000 | 2.0 ms | 9.7 ms | 113 ms |
| floor KDF | 1,000 | 2.1 ms | 11.1 ms | 148 ms (`exa`) |
| floor KDF | 1,000 | 1.9 ms | 9.4 ms | 138 ms (`exa`, query #54) |
| floor KDF | 1,000 | 1.9 ms | 9.5 ms | 149 ms (`user`, query #955) |

Target: < 100 ms (roadmap) for 20k items. **The typical query is 50x under the target; the slowest queries (broad prefixes that match thousands of items, e.g. `user`, which matches every item) are 40-50 % over it.** T06 reported 47-67 ms for a 2,900-match prefix on the same kind of hardware; the all-items prefix here is a harder case. T06 also reports the unpaged list of 20,000 items at 150-180 ms (a `field(key)` index would fix it; it needs a storage schema v2) and the first page of 50 at about 41 ms. A UI should page and debounce, but the roadmap number is not met for the worst case.

---

## 5. Security requirements

Test names are Rust test functions; paths are relative to `core/crates/`.

### Cryptography (`docs/08` section 1)

| ID | Status | Evidence / gap |
|---|---|---|
| SEC-C01 only listed primitives | **Partially verified** (`RV` pending) | `crypto/src/lib.rs::sec_c01_dependencies_are_on_the_allowlist`, `::sec_c01_no_unlisted_primitives_in_source`; `cargo deny check` (CI `supply-chain`, run locally: advisories, bans, licenses, sources ok). The T08 CLI adds `clap`, `rpassword`, `serde_json`, `tempfile` and calls no primitive directly. |
| SEC-C02 Argon2id >= 64 MiB, t >= 3, weaker params refused | Verified | `crypto/src/kdf.rs::sec_c02_hostile_parameters_rejected_before_hashing`, `::sec_c11_memory_bounds`; `vault/src/interchange/aryavault.rs::version_magic_and_kdf_bounds`. |
| SEC-C03 all keys/nonces/salts from the OS CSPRNG | **Partially verified** (`RV` pending) | `crypto/src/lib.rs::sec_c03_only_rng_rs_touches_the_os_rng`, `crypto/src/vault_key.rs::sec_c03_all_vault_randomness_is_drawn_from_the_rng`. The generator crate's `RandomSource` is sealed (only `OsRandom` outside tests). The CLI draws its device id and recovery key through the crypto crate's `OsRng`. |
| SEC-C04 unique random 192-bit nonce | **Partially verified** | `crypto/src/aead.rs::sec_c04_one_million_nonces_are_distinct`. `docs/11` §2 asks for 10^7 draws; the test uses 10^6 (the task's number). Nonces are generated inside `seal` (no caller-supplied nonce). |
| SEC-C05 vault_id, epoch, device_id, seq, kind in AAD | Verified | `crypto/src/format/envelope.rs::sec_c05_every_byte_flip_fails_to_open`, `crypto/src/wrap.rs::sec_c05_wrap_pw_rejects_every_tampered_field`, `vault/src/interchange/aryavault.rs::every_byte_flip_is_rejected_without_rerunning_argon2`; independent re-implementation in `tools/crosscheck`; CLI e2e flips header bytes (exit 3 or 4, never success). |
| SEC-C06 zeroize on lock and drop | **Partially verified** | Item 2 in section 1. `crypto/src/keys.rs::sec_c06_vault_key_zeroized_on_drop`, `crypto/src/kdf.rs::sec_c06_argon2_scratch_memory_is_wiped_after_use`. |
| SEC-C07 published test vectors | Verified | RFC 9106 §5.3 (`kdf.rs::sec_c07_rfc9106_argon2id_known_answer`), RFC 5869 cases 1-3 (`hkdf.rs::sec_c07_rfc5869_case1_basic`, `_case2_longer_inputs`, `_case3_empty_salt_and_info`), XChaCha draft A.1 (`aead.rs`, test at the `sec_c07` comment). |
| SEC-C08 open all golden files from earlier releases | Verified (one version exists) | `crypto/tests/golden_v1.rs::sec_c08_golden_files_are_unchanged`, `::sec_c08_golden_v1_opens_with_password_and_recovery_key`; CI `crosscheck`; CLI `golden_vault_v1_opens_with_the_documented_password` (copies the header, unlocks, resets via the recovery key, original untouched). |
| SEC-C09 unbiased generator | Verified (with a caveat from PR #6) | `generator/src/uniformity_tests.rs` (chi-square at awkward alphabet sizes, shuffle and class-placement uniformity, `rejection_sampling_beats_modulo_on_biased_range`). Caveat from PR #6: chi-square at those sizes cannot statistically detect modulo bias; rejection correctness rests on the scripted-source tests. |
| SEC-C10 identical NFKD on all platforms; version pinned | Verified | `crypto/src/normalize.rs::sec_c10_unicode_version_is_pinned`, `::sec_c10_composed_accent_equals_decomposed`, `crypto/src/kdf.rs::sec_c10_cross_implementation_vectors_for_nfkd_passwords`; CI runs them on Linux, macOS and Windows. |
| SEC-C11 floors **and ceilings** before the KDF | Verified | `crypto/src/format/header.rs::sec_c11_out_of_range_kdf_is_rejected_at_decode`, `kdf.rs::sec_c11_memory_bounds`; the export container rejects hostile triples at parse time. |
| SEC-C12 no `header_version` in wrap AAD; password change without the recovery key | Verified | `crypto/src/wrap.rs::sec_c12_aads_have_no_header_version_input_and_differ`, `crypto/src/vault_key.rs::sec_c12_change_password_leaves_recovery_wrap_untouched_and_valid`; **CLI e2e** `full_lifecycle_with_disk_and_output_scan`: `password change` with no recovery key, then the old password is rejected, the new one works, the data survives, and the original recovery key still resets the password. |
| SEC-C13 SQLCipher settings pinned and verified on open | Verified | `storage/src/tests.rs::recorded_settings_are_written_on_create`, `::every_recorded_setting_mismatch_is_a_typed_error`, `::missing_recorded_setting_is_a_mismatch`; CLI `info_reports_formats_and_pinned_sqlcipher_settings` prints the ten pinned values from `meta`. |

### Authentication and recovery (section 2)

| ID | Status | Evidence / gap |
|---|---|---|
| SEC-A01 recovery key >= 128 bits, shown after creation, **verified by re-entry** | **Partially verified** | 160-bit key from the OS CSPRNG (`recovery_key.rs`); the CLI prints it once, only with `--reveal` (`secrets_print_only_with_reveal`). Re-entry verification during onboarding is a UI flow (M2). |
| SEC-A05 password change does not re-encrypt data | Verified | CLI lifecycle: the database file is not re-keyed, the same `K_db` (derived from the unchanged VK) opens it after the change, and all three items are still readable. (The only database write is the `meta.header_version` update.) |
| SEC-A07 minimum length 12 and a strength check | Verified | `generator/src/strength.rs::master_policy_reasons`; the CLI applies it to `vault create`, `password change` and `recover` through one helper; only `vault create` rejecting a short password is covered by a test (`secrets_print_only_with_reveal`). |
| SEC-A06 VK rotation | **Not verified (not implemented)** | Item 7 in section 1. |

### Storage (section 3)

| ID | Status | Evidence / gap |
|---|---|---|
| SEC-S01 DB is SQLCipher, key derived from the VK | Verified | `storage/src/tests.rs::encrypted_file_has_no_plaintext_header_and_looks_random`; `K_db = HKDF(VK, "db/v1")` is what the CLI uses; CLI scan asserts no `SQLite format 3` header in any file. |
| SEC-S02 no plaintext secrets in files, logs, crash reports, temp files, backups | **Partially verified** | CLI `full_lifecycle_with_disk_and_output_scan`: after create/add/search/trash/password change/recover/rotate/export/import, every file in the temp dir (including `-wal`/`-shm`, the export) and all captured stdout/stderr contain none of the 11 canary strings (also checked as UTF-16LE), and secrets appear in output only for `--reveal` calls. Not covered: crash dumps and core dumps (SEC-H06: the CLI does not disable them; doing so needs `unsafe` or a new dependency), swap, OS backups, process memory, a Windows/macOS run. |
| SEC-S05 search index only inside the encrypted DB | Verified | `storage/src/tests.rs::disk_scan_finds_no_canary_or_sqlite_header`, `vault/src/tests.rs::secrets_are_never_indexed`; CLI: a note body is searchable by design (US-04, `StdField::Body`), and the scan finds it nowhere on disk. |
| SEC-S06 atomic writes, crash-safety tested | **Partially verified** | `storage/tests/crash_safety.rs::kill_at_random_points_never_corrupts_or_loses_commits` (SIGKILL), `vault/src/interchange/vault_tests.rs::commit_is_all_or_nothing_at_every_write_point` (fault injection). Not tested: a crash during `Db::create` or `rekey` (PR #9), power loss, and the CLI's header publish (temp file + rename, then delete the old header) was not kill-tested. |

### Sync-format requirements that M1 touches (section 4)

| ID | Status | Evidence / gap |
|---|---|---|
| SEC-Y02 sizes reveal only the bucket | Verified | `crypto/src/format/envelope.rs::sec_y02_ciphertext_sizes_are_bucketed`, `padding.rs::padded_lengths_at_boundaries`, `interchange/aryavault.rs::exports_are_randomised_and_padded`. |
| SEC-Y04 / SEC-Y14 merge laws, conflicts independent of arrival order | **Partially verified** | Proven for the in-memory register and the local DB (`vault/src/register.rs::merge_is_commutative`, `::any_order_and_duplication_converges`, `vault/src/tests.rs::database_state_is_independent_of_arrival_order`). There is no sync engine or simulator yet (M4), so the end-to-end requirement is not testable. |
| SEC-Y05 every external-bytes parser bounded and fuzzed | **Partially verified** | Bounded and typed-error for header, envelope, CBOR, path, recovery key, CSV, Bitwarden JSON, AryaVault export; a fuzz target for each (`fuzz_header`, `_envelope`, `_cbor`, `_path`, `_recovery_key`, `_import_csv`, `_import_bitwarden`, `_import_aryavault`). Gaps: duration (item 1 in section 1); `vault::value::decode` has no target of its own (PR #13); the CLI's arguments and the `rpassword` input path are not fuzzed. |
| SEC-Y10 envelope `device_id`/`seq` match the path | Verified at the format layer | `crypto/src/format/envelope.rs::sec_y10_segment_from_device_a_at_device_b_path_is_rejected`; quarantine behaviour belongs to M4. |

### Other requirements touched by T08

| ID | Status | Evidence / gap |
|---|---|---|
| SEC-H04 no network except the chosen provider | **Partially verified** (`RV` pending) | `cargo tree --workspace` contains no HTTP/TLS/socket crate. `openssl-sys` is present only as SQLCipher's vendored crypto provider (`bundled-sqlcipher-vendored-openssl`). The CLI opens no sockets. |
| SEC-R01 lockfile, audit, deny | **Partially verified** | `Cargo.lock` committed and `--locked` in CI; `cargo deny check` and `cargo audit --deny warnings` in CI; `cargo-vet` not added (PR #4). |
| SEC-R05 two-person review of crypto | **Not verified** | One `CODEOWNERS` owner (section 1, item 3). |
| SEC-R06 disclosure process | **Partially verified** | `SECURITY.md` exists; its content was not reviewed here. |

### Outside M1 (no evidence expected yet)

SEC-A02/A03/A04 (biometrics, lock policy: M2/M3), SEC-S03/S04 (keystore, backup flags: platform), SEC-Y01/Y03/Y06-Y09/Y11-Y13 (sync and providers: M4/M5), SEC-H01/H02/H03/H05/H06/H07/H08 (clipboard, capture, autofill, crash reporting: M2+), SEC-R02-R04/R07 (release, audit), SEC-P01-P03 (privacy labels). Where the CLI could touch one, it is listed above.

---

## 6. CLI behaviour that backs the claims

Reproduce with `cargo test -p arya-vault-cli --locked` (the lifecycle test takes about 2 minutes in a debug build because every command re-derives Argon2 unoptimized; `[profile.dev.package.argon2] opt-level = 3` would cut it, as PR #7 also noted).

* Secrets: no option accepts a password, recovery key or item secret. A mistyped `--password X` is rejected with exit 2 **and the error text never echoes `X`** (clap's own message can quote it, so the CLI prints only the error kind). Environment variables are never read. Stdin is read only with `--password-stdin` (`secrets_are_never_accepted_from_arguments_or_environment`).
* Reveal: `item get`, `gen`, `vault create`, `rotate-recovery-key`, `recover --regenerate-recovery-key` print a secret only with `--reveal`, and refuse without it (exit 2) rather than succeed silently (`secrets_print_only_with_reveal`).
* Failure behaviour: wrong password -> exit 3, tampered or truncated header -> exit 3 or 4, corrupted or non-database `vault.db` -> non-zero, empty directory -> exit 4, never a panic (`wrong_password_corrupted_and_missing_files`). A panic prints a generic message only (`install_panic_hook`) and release builds use `panic = "abort"`.
* Plaintext CSV export needs `--acknowledge-plaintext-risk`; the file is created exclusively (never overwritten) with owner-only permissions on Unix.

---

## 7. Deferred items, consolidated from T00-T08

Each line names the PR that raised it. "Owner" means it needs a decision or action from a person, not code.

**Tooling and CI**
* #4: nightly fuzz is 10 min per target vs 1 h in `docs/11` §10; no ASan/MSan fuzz builds (`docs/11` §8); `cargo-vet` not added; coverage is informational with no threshold. (The first two are part of item 1 above.)
* #6: `time` was bumped in `Cargo.lock` to clear RUSTSEC-2026-0009, which needs Rust 1.88 while the workspace declares `rust-version = "1.85"`. **Owner:** raise the MSRV or add a justified `deny.toml` ignore.
* #12 / #14 (CI): the `coverage` job fails intermittently on a corrupt `.profraw` from the T05 SIGKILL test; it passed on one re-run each time. A fix task card is queued. **Owner:** decide when.
* `.gitleaks.toml` allowlist (PR #12) and `docs/13` are not in `CODEOWNERS`.

**Crypto and formats**
* #7: reset-password-via-recovery-key and VK rotation not in T01; no memory-inspection tests; no `mlock`; nonce test is 10^6 not 10^7; `cargo audit` not run locally.
* #10: single-segment golden set (the non-zero `prev_hash` chain is covered by unit tests only); no future-version golden file; `gen_golden` lints only with `--features deterministic-rng`.
* #12: the Python reader has no `select_active` and does not read the T07 export.
* #14: KDBX importer (issue text in the PR); 100,000-item hard limit tested as arithmetic only; memory hygiene best effort inside `csv`/`serde_json`; no streaming import/export.

**Storage and vault**
* #9: real `ENOSPC` untested; the read-only-directory test is skipped when run as root; no kill test around `create`/`rekey`; `integrity_check` does not run FTS5's own check.
* #13: no process-kill test for the vault layer; no fuzz target for `value::decode`; remote-op application is not exposed (M4); the unpaged list is over its target (needs a `field(key)` index, storage schema v2); `wifi`/`custom` item types absent.

**CLI (this PR)**
* The interactive TTY prompt is manually exercised only (section 1, item 5).
* No `vault check` command: `Db::integrity_check` exists but is not exposed, and SQLCipher authenticates a page only when it is read, so damage to an unread page is not noticed by `unlock-check` or `item list`.
* No folder commands (folders exist through imports only), no history/version commands, no custom-field editing, no `item edit` for URLs beyond adding.
* The CLI deletes superseded header files immediately after a password change or recovery; `docs/04` §9 says old headers are kept locally for 30 days to survive sync races. There is no sync here, so this is the conservative choice, but M4 must implement the retention.
* `highest_known_epoch` is passed as 0 to `select_active` (no rollback memory yet).
* Windows and macOS runs of the new tests are pending CI.

---

## 8. Spec questions raised across T00-T08 (open ones, consolidated)

Resolved questions (for example the recovery-key grouping in #7, the encoding gaps closed by #11) are not repeated. Numbers refer to the PR's own list.

| PR | Question | Needs |
|---|---|---|
| #4 | `docs/11` §10 says 1 h per fuzz target nightly; implemented 10 min. | Owner |
| #6 | MSRV 1.85 vs the `time` advisory fix (needs 1.88); ambiguous-character set also drops `o` and `\|`; `entropy_bits` is a lower bound for constrained passwords; "very weak" master password = zxcvbn score < 2; `Zeroizing<String>` prints its content in `Debug` (a redacted newtype would be an API change). | Owner |
| #7 | Initial epoch not specified (caller passes it; the CLI uses 1); in-crate canonical CBOR instead of `ciborium`; newest dependency majors (argon2 0.6, chacha20poly1305 0.11, sha2 0.11, hkdf 0.13) need an audit-status check; no maximum master-password length. | Owner / reviewers |
| #9 | Contentless FTS keyed by `item.rowid`, which `VACUUM` may renumber (forbid `VACUUM` or add a mapping column; same note in #13); backups keep the old key after `rekey` (delete them on key rotation: `docs/04` §10 / `docs/12` §7 gap); raw-key mode still pins and records `cipher_kdf_algorithm` and `kdf_iter`. | Owner |
| #10 | Envelope byte layout, file-name widths (16 hex digits vs `docs/06` §3's 10, now in `docs/04` §16; **`docs/06` §3 is still stale**), size limits for manifests (256 KiB) and snapshots (64 MiB) are placeholders; snapshot HLC is not bound in the envelope; `SubKey` has no kind label. | Owner |
| #13 | Purge is a local garbage collection, not an op (the literal reading would resurrect the item); visibility counts every register but `deleted`/`deleted_at`; tie-break beyond `(hlc, device_id)`; no wall-clock "edited"/"password changed" fields in `docs/05` §2 (adding them is a spec change); identity `ids` treated as secret; folder ops have no `local_op` encoding; hard limit counts tombstones. | Owner |
| #14 | Export key derivation reuses the `aryavault/kek-pw/v1` label (a dedicated label needs a crypto-crate change); wrapper shape (`AVEX` + CBOR); history exported without timestamps; dedupe rules; CSV export is lossy; Bitwarden mapping choices; new ids/times on import. | **Owner decision before release** |
| T08 | **The crypto crate has no function for "reset the password with the recovery key" or "replace the recovery wrap" (`docs/04` §9).** The CLI composes them from public primitives (`derive_master_key`, `hkdf::kek_pw`, `wrap::wrap_pw`, `hkdf::kek_rk`, `wrap::wrap_rk`) with the same AADs and order `create_vault`/`change_password` use. No new algorithm, and the end-to-end test proves it (new password works, old stops, recovery key still works or is replaced and the old one stops), but the composition lives in the CLI, outside the two-maintainer `crypto` path, and has no unit test there. **Recommendation:** move it into `vault_key.rs` with unit tests and use it from the CLI. | Owner / reviewers |
| T08 | `meta.header_version` (written by `Db::create`) has no owner: nothing in storage or vault updates it after a password change. The CLI updates it through the public `Store::meta_set`. | Owner |
| T08 | A UUIDv7 item id is time-ordered, so a short prefix is ambiguous for items created close together; the CLI accepts a unique prefix **or suffix** of at least 6 hex characters. | None |
| T08 | The broad-prefix search latency and the unpaged list (section 4) miss the 100 ms / 50 ms targets; whether to fix them in the index design or accept them with paging and debouncing is a product decision. | Owner |
