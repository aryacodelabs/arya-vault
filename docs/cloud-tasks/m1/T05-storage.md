# T05: Encrypted local storage (SQLCipher)

**Branch:** `m1/t05-storage` · **Depends on:** nothing · **Crate:** `core/crates/storage` · **Reviewer attention:** high (key handling, crash safety)

## Context
Read `CLAUDE.md`, `docs/05-data-model.md` (all), `docs/04-crypto-spec.md` §1 (SQLCipher pinning), §2 (K_db) and §11, `docs/08-security-requirements.md` §3 (SEC-S01 to S06, SEC-C13), `docs/11-testing-strategy.md` §5 and §8 (disk scan test).

The storage crate does **not** derive keys. It receives a 256-bit key (`DbKey`, a zeroizing newtype defined in this crate; the vault layer will convert from T01's `SubKey`). Do not depend on the `crypto` crate.

## Deliverables
1. `Db::create(path, key, params)` / `Db::open(path, key)` using `rusqlite` with a bundled SQLCipher (`bundled-sqlcipher-vendored-openssl` or the best bundled option that builds on Linux/Windows/macOS CI; justify the choice, and note Android/iOS cross-compile implications in the PR). **Raw key mode** (`PRAGMA key = "x'<64 hex>'"`) so SQLCipher's own KDF is bypassed.
2. **Pinned SQLCipher settings** (SEC-C13): `cipher_page_size`, `cipher_compatibility = 4`, HMAC/KDF algorithms, `cipher_plaintext_header_size = 0`, `journal_mode = WAL`, `synchronous = FULL`, `foreign_keys = ON`, `secure_delete = ON`. Record the values in a `meta` row on create and **verify them on open**; mismatch is a typed error, never a silent fallback.
3. **Schema and migrations** per doc 05 §5 including the review additions (`base_hlc` columns, `outbox`, `manifest_seen`), embedded migrations with a `schema_version` meta row, forward-only, each migration in a transaction, with an automatic **pre-migration encrypted backup copy** (SQLite backup API, retained 14 days, doc 12 §7).
4. API shape: typed transactional access (`with_tx(|tx| ...)`), no raw SQL leaking outside the crate except through documented query methods the vault layer needs (keep the surface small; write the vault-facing trait `Store` with the methods T06 will need: items/fields/history/folders/local_op/outbox/meta/device/provider_state/FTS maintenance).
5. `rekey(old_key, new_key)` (for key rotation, doc 04 §10) and `integrity_check()` (`PRAGMA integrity_check` + `foreign_key_check`).
6. Wrong key detection: opening with a wrong key must return `StorageError::WrongKeyOrCorrupt` quickly and never panic; never reveal which.
7. Close/lock: `Db::close(self)` that finalizes statements, checkpoints WAL, and zeroizes the held key; ensure `Drop` is safe.
8. FTS5 virtual table created inside the encrypted DB (SEC-S05); expose maintenance hooks only (the vault layer indexes).

## Tests
- Create/open/close round trip; wrong key; truncated file; garbage file; read-only directory; disk-full simulation if feasible (tmpfs quota or fault-injection wrapper).
- **Disk scan test** (SEC-S01/S02): insert canary strings (`CANARY-7F3A-...`), close, then scan the DB file, the `-wal`, `-shm` and temp directory for the canary and for the plaintext `SQLite format 3` header: neither may be found.
- **Crash safety** (SEC-S06): spawn a child process that writes in a loop and is killed (`SIGKILL`/terminate) at random points >= 200 times; after each, reopen, `integrity_check` is OK and a committed marker row is never lost or half-applied.
- Migration tests: start from an embedded v1 fixture, migrate, verify; migration failure rolls back and leaves the original intact; backup file is created and encrypted.
- Pragmas pinned: test fails if any setting differs on open (mutate a setting in a test DB using raw SQLCipher access).
- `rekey` round trip and old key rejection.
- Concurrency: concurrent readers with one writer do not deadlock (`busy_timeout` documented).

## Acceptance criteria
- SEC-S01, S02 (disk canary), S05, S06, C13 mapped to tests in the PR table.
- If vendored OpenSSL requires adding a license (e.g. `OpenSSL`) to `core/deny.toml`, add it in a **separate commit** with justification in the commit message; `cargo deny check` must pass.
- Report cold build time and binary size impact in the PR.
- No key material in `Debug` output or error strings; keys zeroized on drop.

## Out of scope
Business logic (T06), key derivation (T01), sync tables' *behavior* (M4: only create the tables).

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
