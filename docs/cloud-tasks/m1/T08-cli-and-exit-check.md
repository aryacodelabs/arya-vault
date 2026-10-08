# T08: CLI harness, benchmarks and M1 exit check

**Branch:** `m1/t08-cli` · **Depends on:** T00-T07 all merged · **Crate:** `core/crates/cli` · **Also:** `docs/reviews/m1-exit-check.md`

## Context
Read `CLAUDE.md`, `docs/10-roadmap.md` M1 exit criteria, `docs/08-security-requirements.md`, `docs/11-testing-strategy.md` §6 and §12. The CLI is a **developer/test harness** over the Rust core (headless vault operations), not a polished product; it also becomes the base for a future user CLI.

## Deliverables
1. Commands (clap; JSON output flag `--json`; non-zero exit on error):
   `vault create`, `unlock-check`, `item add|get|list|edit|delete|restore|purge`, `search`, `gen password|passphrase`, `password change`, `recover` (via recovery key), `rotate-recovery-key`, `export`, `import`, `bench kdf`, `bench search`, `info` (format versions, SQLCipher settings, schema version).
2. **Secret handling:** passwords and recovery keys are read from a TTY prompt (no echo) or from an explicit `--password-stdin`; **never** accepted as command-line arguments or environment variables; secrets never printed unless a `--reveal` flag is passed; output of `--reveal` goes to stdout only; no secrets in logs or panics (`panic = "abort"` plus a panic hook that prints a generic message).
3. **Calibration:** `bench kdf` runs Argon2 calibration and prints chosen parameters and timings; `vault create` uses calibrated parameters (with `--kdf-profile low|default|high` override within the bounds).
4. **End-to-end tests** (`assert_cmd`): full lifecycle in a temp dir: create vault -> add items -> search -> lock/unlock -> change password (**without** the recovery key) -> recover with recovery key -> export/import round trip; wrong-password and corrupted-file behaviors; golden-vault open test.
5. **Disk scan and leak tests:** after the lifecycle, scan the raw bytes of every file in the temp dir (including `-wal`/`-shm`) and the captured stdout/stderr for canary secrets and for the plaintext `SQLite format 3` header. **No** canary may appear anywhere in plaintext, including inside the vault file. Stdout/stderr may contain a secret only when `--reveal` was passed.
6. **Benchmarks** reported in the PR (and `docs/reviews/m1-exit-check.md`): Argon2 default-params unlock time; cold unlock + open with 20,000 generated items; FTS search latency; item write latency.
7. **`docs/reviews/m1-exit-check.md`:** go through the M1 exit criteria in `docs/10-roadmap.md` and the SEC-* requirements assigned to M1, and mark each **verified (with test name / evidence)**, **partially verified**, or **not verified**, honestly. List all deferred items and any spec questions raised across T00-T07 PRs.

## Acceptance criteria
- Lifecycle e2e passes on Linux, macOS and Windows CI.
- Exit-check document complete and honest; any **not verified** MUST item is called out at the top.
- No new crypto; the CLI only calls public APIs of existing crates; no `unsafe`.

## Out of scope
Sync, networking, Flutter/FFI (M2), packaging/release.

---

## Delivery rules (all tasks)
- Work on the branch named above, created from the latest `main`. Open **one PR** against `main`; never push to `main`.
- Read `CLAUDE.md` first and follow its hard rules. **The specs in `docs/` are the source of truth.** If you find a defect or gap, implement the most conservative reading, and describe it in a **"Spec questions"** section of the PR. Do not edit specs except where this prompt explicitly says to, and then only in a separate `docs(spec): ...` commit.
- Stay inside the listed crate/area. Do not refactor unrelated code or change CI beyond what is listed.
- Run from `core/`: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo deny check`. All must pass before you finish.
- PR description must contain: summary; **requirement -> test table** (SEC-* IDs); dependencies added with justification; benchmark numbers where requested; "Spec questions"; "Deferred / not done" (be honest); how you verified.
- Never use real secrets. Test data must be fake and clearly marked (`CANARY-...`). Do not ask for or use any credentials.
- Do not weaken or delete tests to make them pass. If a test cannot pass because the spec is wrong, say so in the PR.
- Commit style: `feat(crypto): ...`, small commits, sign off with `git commit -s`.
