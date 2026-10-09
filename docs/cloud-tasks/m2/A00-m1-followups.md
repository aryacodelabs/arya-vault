# A00: M1 follow-ups (performance, tests and fuzz gaps)

**Branch:** `m2/a00-m1-followups` · **Depends on:** nothing · **Touches:** `core/crates/storage`, `core/crates/vault`, `core/crates/crypto` (tests only), `core/fuzz/`, `.github/workflows/nightly.yml` (only duration/targets)

## Context
Read `CLAUDE.md`, then `docs/reviews/m1-exit-check.md` (all of §1 "not verified" and §4 benchmarks) and `docs/11-testing-strategy.md` §2, §4, §5, §6. This task closes the cheap, concrete gaps the M1 exit check listed. Do **not** implement features.

## Deliverables
1. **Unpaged-list and broad-search performance.** Add a storage schema **v2** migration (forward-only, pre-migration encrypted backup, per `docs/05`/T05 rules) with the index needed for `field(key)` access paths that T06 found slow (unpaged list of 20,000 items 150-180 ms), and make broad-prefix FTS queries (e.g. `user` matching every item) meet **< 100 ms at 20,000 items** (for example by bounding the FTS candidate set via the existing unranked recency path before filtering/paging). Provide before/after numbers from the existing bench (`arya-vault bench search`) in the PR. Keep passwords/TOTP out of the index (SEC-S05 tests must still pass). Migration tests: v1 fixture -> v2, failure rollback, backup created.
2. **SEC-C04 at spec strength.** `docs/11` §2 asks for 10^7 nonce draws; the test uses 10^6. Raise to 10^7 behind `#[ignore]` + a nightly CI job (not on every PR), and keep the 10^6 test in PR CI.
3. **Fuzz gap.** Add `fuzz_vault_value_decode` for `vault::value::decode` (named in the exit check as lacking a target) with a seed corpus; register in the nightly matrix. Extend nightly per-target duration to **60 minutes** for the `main` schedule (matrix is parallel; keep manual dispatch configurable) so "fuzzers run 1 h clean" becomes verifiable by evidence (the first scheduled run after merge is the evidence; do not claim it before then).
4. **SEC-S06 crash tests** for the two untested paths the exit check names: kill during `Db::create`, and kill during `rekey` (child process killed at random points, >= 200 iterations each; after each kill the database either opens with the old state or the new state, never corrupt; `integrity_check` OK, no partial header/db mismatch is possible because ordering is documented, say how). If you find a real bug, fix it in a separate commit with a regression test.
5. **Calibration granularity.** The exit check found calibration lands one step high (~1.0 s for target 750 ms). Make `kdf::calibrate` choose the **step whose measured time is closest to the target** (not first at-or-above), never below the floors, and report the measured time. Add tests with a fake timing source. Do not change the default target constant (owner decides separately).

## Acceptance criteria
- Mapped requirements: SEC-C04, SEC-S05, SEC-S06, SEC-Y05 in the PR table; numbers for item 1; no existing test weakened.
- `cargo deny` passes; no new dependencies unless justified.

## Out of scope
Anything not listed; do not touch the session/CLI layout code (another task is refactoring it).

---

## Delivery rules (all tasks)
- Work on the branch named above, created from the latest `main`. Open **one PR** against `main`; never push to `main`.
- Read `CLAUDE.md` first and follow its hard rules. **The specs in `docs/` and the API contract `docs/14-app-api-contract.md` are the source of truth.** If you find a defect or gap, implement the most conservative reading and describe it in a **"Spec questions"** section of the PR. Do not edit specs or the contract except where this prompt explicitly says so, and then only in a separate `docs(spec): ...` commit.
- Stay inside the listed crate/area. Do not refactor unrelated code or change CI beyond what is listed.
- Rust checks, run from `core/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`. Flutter checks, from `app/`: `dart format --set-exit-if-changed .`, `flutter analyze`, `flutter test`. Everything that applies must pass before you finish.
- PR description must contain: summary; **requirement -> test table** (SEC-* IDs); dependencies added with justification; benchmark numbers where requested; "Spec questions"; "Deferred / not done" (be honest); how you verified (commands + results). If an environment limit stopped you from verifying something (for example Flutter or Dart could not be installed), say exactly what was not run.
- Never use real secrets. Test data must be fake and clearly marked (`CANARY-...`). Do not ask for or use any credentials.
- Do not weaken or delete tests to make them pass. If a test cannot pass because the spec is wrong, say so in the PR.
- Commit style: `feat(session): ...` / `feat(app): ...`, small commits, sign off with `git commit -s`.
