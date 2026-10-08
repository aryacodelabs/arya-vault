# T06: Vault model, history and search

**Branch:** `m1/t06-vault` · **Depends on:** T05 merged (T01 helpful, not required) · **Crate:** `core/crates/vault`

## Context
Read `CLAUDE.md`, `docs/05-data-model.md` (all), `docs/06-sync-protocol.md` §4 (HLC), §5 (merge, review-corrected), `docs/01-product-requirements.md` US-03/04/08/14, `docs/08-security-requirements.md` SEC-Y04/Y14, `docs/11-testing-strategy.md` §3.1. This is the **local** vault only: no network, no sync engine (M4), but the data structures must already be merge-correct.

## Deliverables
1. **`Hlc`** (48-bit ms + 16-bit counter) and `HlcClock` with `now()` (local event) and `observe(remote)` (receive rule that **always adopts** remote pt, per doc 06 §4), a pluggable `Clock` for tests, skew-flag helper (`> now + 24 h` flagged; `> now + 1 year` => `Err(CorruptClock)`), packing to/from `i64` safely for SQLite (document the range). `Hlc` lives in this crate in M1 and must have **no dependency on storage**.
2. **Register model:** `Register { value, hlc, device_id, base_hlc }` and `Register::merge(a, b)` implementing `max` over `(hlc, device_id)` with the losing version returned for history.
3. **Item model** (`login`, `note`, `card`, `identity`) as maps of field registers (doc 05 §2): typed accessors, custom fields as individually addressable registers, tags/urls as per-element registers, UUIDv7 ids, folders. Validation limits from doc 05 §10 enforced with typed errors.
4. **Local mutation API** on `Vault` (wrapping `Store` from T05): `create_item`, `set_field`, `delete_item` (tombstone), `restore_item`, `purge` (trash retention 30 days, tombstone kept 180 days, doc 05 §7), `move_to_folder`, `toggle_favorite`; every mutation stamps HLC, sets `base_hlc` from the currently stored version, writes `field`, pushes the previous version to `field_history` (retention limits doc 05 §8), and appends to `local_op`, all in **one transaction**.
5. **Visibility rule** (doc 06 §5.2/M6): visible iff `deleted == false` or any field `hlc > deleted_hlc`; used by all list/search APIs.
6. **Derived concurrency view** (doc 06 §5.3/H3): `concurrent_versions(item, field)` computing losers that are not ancestors of the winner via `base_hlc` links in history (conservative when ancestors are pruned). Pure function over stored history; **no mutation of state**.
7. **Search:** maintain the FTS5 index (title, username, urls, notes, tags) inside the same transaction as mutations; `search(query, filters)` with prefix matching, type/tag/folder filters; passwords and TOTP seeds are **never** indexed.
8. **Secret access:** secret fields (`password`, `totp_seed`, card number/cvv/pin, note body) returned only through explicit `reveal_*` methods returning `Zeroizing`; listing/search results contain no secrets (types enforce it).
9. **Health report helpers:** reused-password detection (compare via keyed hash held in memory only, not stored), weak (use T04's estimator if merged, otherwise an interface `StrengthEstimator` stub), old (>N days).

## Tests
- Property tests (`proptest`) for `Register::merge`: commutative, associative, idempotent; for arbitrary ops sets, applying in any permutation/with duplicates yields identical register state and identical **concurrent_versions** output (SEC-Y04, SEC-Y14).
- Spurious-conflict regression: B's edit based on A's edit arriving **before** A's must not be reported concurrent once A arrives.
- HLC: monotonic under backwards wall clock, counter overflow behavior, skew thresholds, round trip packing.
- CRUD, trash/purge/restore with a fake clock; history limits; visibility rule (edit after delete resurrects; edit before delete does not).
- Search: ranking not required, but results correct for typical queries; secrets never present in FTS content (query the FTS table directly in a test).
- Atomicity: kill-style test via storage fault injection: a mutation either fully applies (field + history + local_op + FTS) or not at all.
- Performance (ignored by default, run in PR): 20,000 items search < 100 ms, list < 50 ms; report numbers.

## Acceptance criteria
Requirement IDs mapped in the PR table; no secrets in `Debug`/errors; `missing_docs` clean; public API small and documented; no sync-engine code (ops are only *recorded* in `local_op`).

## Out of scope
Applying remote ops, segments, snapshots (M4), import/export (T07), encryption/KDF (T01).

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
