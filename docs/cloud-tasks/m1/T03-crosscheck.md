# T03: Python cross-check decryptor

**Branch:** `m1/t03-crosscheck` · **Depends on:** T02 merged (golden files) · **Touches:** `tools/crosscheck/`, `.github/workflows/` (one job) · **No Rust changes**

## Context
Read `CLAUDE.md`, `docs/04-crypto-spec.md` (all), `docs/11-testing-strategy.md` §2, and `core/testdata/golden/v1/README.md`. The goal is an **independent implementation** that decrypts the golden vault using only the written spec, so spec/implementation drift is caught. **Do not read the Rust source to learn the format**: work from the docs; if the docs are insufficient to decrypt a golden file, that is a spec bug: report it in the PR as a blocking "Spec questions" item rather than peeking at the Rust code.

## Deliverables
1. `tools/crosscheck/crosscheck.py` (Python 3.11+, run with `python -I`): given a golden directory, the master password (or recovery key), it:
   - parses the header (own minimal canonical CBOR reader/writer, e.g. `cbor2` only for decoding with strictness checks, or hand-written),
   - derives MK with Argon2id (`argon2-cffi`), KEK via HKDF-SHA256 (`cryptography`), unwraps VK with XChaCha20-Poly1305 (`PyNaCl`'s `crypto_aead_xchacha20poly1305_ietf_decrypt`) using the exact AAD rules,
   - unwraps via the recovery key too and checks both yield the same VK,
   - derives `K_log`, `K_snap`, `K_manifest`, decrypts segment/snapshot/manifest envelopes with correct AAD, verifies padding and hash chain,
   - compares plaintexts to the expectations in the golden README (machine-readable `expected.json` next to it; create it if T02 did not).
2. `tools/crosscheck/requirements.txt` with **pinned versions and hashes** (`pip-compile --generate-hashes`); a short `README.md`.
3. Negative checks: the script must **fail** (non-zero exit) on a tampered copy of each golden file (include a test script that flips bytes and asserts failure).
4. A CI job `crosscheck` (Linux only) that installs the pinned requirements with `--require-hashes` and runs the script and the negative tests against `core/testdata/golden/*/`.
5. Output: concise human-readable report plus exit code; never print keys.

## Acceptance criteria
- Decrypts every golden version directory currently present; adding a new `v2/` directory later should be picked up automatically.
- Uses no Rust code, no copy-pasted constants beyond those in the spec.
- Any discrepancy between spec text and golden files is listed in the PR under "Spec questions" with the doc section number.

## Out of scope
Producing new golden files (T02), a Python implementation of the whole app, fuzzing.

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
