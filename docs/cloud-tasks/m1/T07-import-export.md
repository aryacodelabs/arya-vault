# T07: Import and export

**Branch:** `m1/t07-import-export` · **Depends on:** T01 and T06 merged · **Crate:** `core/crates/vault` (module `interchange`) · **Also:** new doc `docs/13-export-format.md`

## Context
Read `CLAUDE.md`, `docs/05-data-model.md` §9-10, `docs/01-product-requirements.md` US-09, `docs/03-threat-model.md` threat 14, `docs/08-security-requirements.md` (SEC-Y05 applies to all external bytes), `docs/04-crypto-spec.md` (primitives and rules).

## Deliverables
1. **Common pipeline:** every importer parses into a neutral `ImportBundle` (items, folders, warnings, skipped records with reasons) with **no vault mutation during parsing**; a separate `commit_import(bundle, options)` applies it in one transaction with a dry-run/preview mode and duplicate detection (same title+username+url).
2. **Hard limits (bounded parsers):** max file size (configurable default 64 MiB), max records, max field length, max nesting depth; exceeding limits is a typed error before large allocations; no panics on any input.
3. **CSV importers:** generic (header-mapped), Chrome/Edge, Firefox, Safari export shapes; encoding detection limited to UTF-8 (+BOM) and UTF-16 (documented); CSV-injection neutralization **on export** only (prefix cells starting with `= + - @` when exporting CSV, documented).
4. **Bitwarden JSON importer:** logins, secure notes, cards, identities, folders, custom fields (text/hidden/boolean), URIs, TOTP; ignore attachments with a warning; unknown item types reported not dropped silently. Do not support Bitwarden's *encrypted* JSON in M1 (return a clear "unsupported, export unencrypted" error).
5. **Exporters:** (a) CSV (plaintext, caller must pass an explicit `acknowledge_plaintext_risk` token type, mirrored by the UI warning), (b) **AryaVault encrypted JSON/CBOR export**: password-protected container using **Argon2id + XChaCha20-Poly1305 from T01** (reuse its KDF bounds/AEAD; no new constructions), self-describing header with format version, KDF params, salt, nonce; includes items, folders, history optionally. Import of that format included.
6. **`docs/13-export-format.md`**: byte-level spec for (b) with test vector, suitable for third-party implementation; mark "Draft for owner review" (the owner approves format before this merges).
7. KeePass (KDBX) is **not** in this task; leave a trait hook and a TODO issue text in the PR description.

## Tests
- Fixture files under `core/testdata/import/` (synthetic data only, canary strings; one per supported format and several malformed ones).
- Round trip: vault -> encrypted export -> new vault -> equality of items (excluding ids/timestamps policy documented).
- Negative: wrong password, flipped bytes anywhere in export (fails authentication), truncated files, oversized fields, deeply nested JSON, giant CSV line, invalid UTF-8, BOM variants.
- CSV-injection export test; plaintext export requires the acknowledgement type (compile-fail test with `trybuild` or doc test).
- **Fuzz targets:** `fuzz_import_csv`, `fuzz_import_bitwarden`, `fuzz_import_aryavault`; run >= 5 min each locally, report results.
- Partial failure: one bad record does not abort the rest (warnings), but a structural error aborts without mutating the vault.

## Acceptance criteria
SEC-Y05 mapped for all three parsers; no secrets in warnings/errors; dependencies justified (e.g. `csv`, `serde_json`); the new doc is complete and consistent with the implementation; `cargo deny` passes.

## Out of scope
KDBX, UI, file dialogs, cloud, attachments, Bitwarden encrypted exports.

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
