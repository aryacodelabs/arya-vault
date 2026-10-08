# T00: CI, fuzz and coverage infrastructure

**Branch:** `m1/t00-ci-fuzz` · **Depends on:** nothing · **Touches:** `core/fuzz/`, `.github/workflows/`, `core/deny.toml` (only if justified), `docs/11-testing-strategy.md` (only to document commands)

## Context
AryaVault is a zero-knowledge password manager with a shared Rust core. Read `CLAUDE.md`, then `docs/11-testing-strategy.md` (§1, §4, §8, §10) and `docs/08-security-requirements.md` §6. No crate has real code yet. This task prepares the infrastructure that later tasks plug into.

## Deliverables
1. **`core/fuzz/`**: a `cargo-fuzz` setup (separate non-workspace-member crate, `libfuzzer-sys`) with:
   - one placeholder target `smoke` that compiles and runs for a few seconds in CI;
   - `core/fuzz/README.md` explaining how later tasks add a target (naming convention `fuzz_<parser>`, corpus under `core/fuzz/corpus/<target>/`, seeds from `core/testdata/`).
2. **`.github/workflows/nightly.yml`**: scheduled (cron) + manual dispatch:
   - fuzz: each target in `core/fuzz/fuzz_targets` for 10 minutes (use a matrix generated from the directory listing) on nightly Rust;
   - on crash, upload the artifact (crash input) and fail.
3. **Coverage job** in `ci.yml` (or a new workflow): `cargo llvm-cov --workspace --lcov`, upload the report as an artifact, print a summary; do **not** gate on a threshold yet.
4. **Supply-chain extras:** a scheduled `cargo audit` job (RustSec) and keep `cargo-deny`. Add `cargo-geiger` or equivalent `unsafe` reporting as an informational job.
5. **Secret-scan** job (e.g. gitleaks action pinned by version) on PRs.
6. Pin third-party actions by **commit SHA** in all workflows you add or touch, with a comment showing the tag. (Dependabot keeps them updated.)
7. Add a short "Running locally" section to `docs/11-testing-strategy.md` §10 only (commands for fuzz, coverage, audit).

## Acceptance criteria
- All new workflows pass `actionlint` (run it) and the existing CI still passes.
- `cargo +nightly fuzz run smoke -- -max_total_time=5` works locally in your environment.
- Workflows request the minimum `permissions:`; no secrets required.
- No changes to Rust crates other than `core/fuzz`.

## Out of scope
Real fuzz targets, test code, release/signing workflows.

## Spec questions / assumptions
List any in the PR description.

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
