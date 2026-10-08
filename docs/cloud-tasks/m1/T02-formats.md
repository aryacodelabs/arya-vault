# T02: Header, envelope and container formats

**Branch:** `m1/t02-formats` · **Depends on:** T01 merged · **Crate:** `core/crates/crypto` (module `format`) · **Reviewer attention:** high

## Context
Read: `CLAUDE.md`, `docs/04-crypto-spec.md` §5-7 and §14, `docs/05-data-model.md` §6, `docs/06-sync-protocol.md` §3 and §6, `docs/08-security-requirements.md` (SEC-Y02, SEC-Y05, SEC-Y10), the review report. T01 provides the primitives; use them, do not reimplement.

## Deliverables
1. **Canonical CBOR layer** (`format::cbor`): deterministic encoding (RFC 8949 §4.2) used for headers, envelope AAD and (later) ops. Strict decoder: reject non-canonical encodings, duplicate map keys, indefinite lengths, trailing bytes, nesting beyond a fixed depth (e.g. 16), and any length beyond caller-supplied limits.
2. **`Header`** exactly as doc 04 §5 (format_version, vault_id, header_version, epoch, kdf, wrap_pw, wrap_rk, created_at) with `encode`/`decode`, the **ordering rule** `(epoch, header_version, device_id)` from doc 04 §5 as a function over a list of candidate headers (+ the "reject lower epoch than known" rule), and file-name helpers for `header-<epoch>-<version>-<device>.bin`. Decode must call KDF-parameter `validate()` (T01) so out-of-range params are rejected at parse time (SEC-C11).
3. **`Envelope`** (doc 04 §6): magic `AVLT`, format_version, kind (segment/snapshot/manifest), vault_id, epoch, device_id, seq, prev_hash, nonce, ciphertext+tag. AAD = canonical CBOR of all fields except nonce/ciphertext/tag. `seal_envelope(subkey, header_fields, plaintext, rng)` and `open_envelope(...)`.
4. **Padding** (ISO/IEC 7816-4, multiple of 1 KiB) with strict unpad (reject malformed padding after authentication).
5. **Path binding** (SEC-Y10): `open_envelope_at_path(path_info, ...)` that rejects envelopes whose `device_id`/`seq` differ from the path (`<device_id-hex>/<seq-hex>.seg`, `manifest-<counter>.bin`, `<hlc>-<device>.snap`). Provide parsers for those path shapes (hex only; reject anything else).
6. **Hash chain helpers**: `envelope_hash(bytes) -> [u8;32]` (SHA-256 of full envelope bytes) and `verify_chain(prev_hash, ...)`.
7. **Size limits** as constants (segment <= 1 MiB plaintext, padded; snapshot streaming is later) enforced at decode before allocation.
8. **Format versioning:** readers accept only known `format_version`s and return a distinct `UnsupportedFormat { found, max_supported }` error (the app uses it for "update required" mode, doc 12 §6).

## Tests
- Round trip every structure; **mutation tests**: for each field of Header and Envelope, change it and prove open/unwrap fails or parse rejects.
- Non-canonical CBOR corpus (hand-built): each must be rejected.
- Padding boundaries: lengths 0, 1, 1023, 1024, 1025; malformed padding.
- Path-binding negative tests (envelope from device A presented at device B's path).
- Header ordering tests incl. epoch rollback attempt and ties.
- Property tests: decode(encode(x)) == x; decode never panics on arbitrary bytes.
- **Fuzz targets** (register with T00's infrastructure if merged, else add `core/fuzz/fuzz_targets/fuzz_header.rs`, `fuzz_envelope.rs`, `fuzz_cbor.rs` following `core/fuzz/README.md`); seed corpus from golden files created below; run each locally for >= 5 minutes and report in the PR.
- **Golden files** in `core/testdata/golden/v1/`: a header, one segment, one snapshot envelope and one manifest, generated deterministically with the `deterministic-rng` feature from fixed seeds, together with a `README.md` listing master password, recovery key, vault_id, expected plaintexts (all fake, obviously canary-labelled) and the exact command to regenerate. A test opens them and asserts contents (SEC-C08). Golden files are append-only: never regenerate an existing version dir.

## Acceptance criteria
- SEC-Y05 (bounded parsers), SEC-Y10, SEC-C05, SEC-C08, SEC-Y02 (padding) each mapped to tests in the PR table.
- No allocation larger than declared limits on hostile input (test with crafted length prefixes).
- No new crypto dependencies; CBOR crate choice justified.

## Out of scope
Segment *contents* (ops), snapshots' internal structure, upload/download logic, the Python cross-check (T03).

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
