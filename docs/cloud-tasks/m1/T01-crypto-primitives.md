# T01: Crypto primitives and key hierarchy

**Branch:** `m1/t01-crypto` · **Depends on:** nothing · **Crate:** `core/crates/crypto` (`arya-vault-crypto`) · **Reviewer attention:** maximum (two-person-review area)

## Context
Read in full before coding: `CLAUDE.md`, `docs/04-crypto-spec.md` (all), `docs/03-threat-model.md` (§4, §6), `docs/08-security-requirements.md` §1-2, `docs/reviews/2026-10-spec-review-04-06.md`. The spec is the source of truth. **Do not invent crypto.** If anything is ambiguous or seems wrong, implement the conservative reading, record it under "Spec questions" in the PR, and do not edit the spec except as stated below.

## Deliverables (module layout suggested; keep the public API small and documented)
1. `rng`: a `Rng` trait (`fill_bytes`) with an OS-CSPRNG implementation (`getrandom`). A deterministic implementation for golden-file generation only under the non-default cargo feature `deterministic-rng`; add a compile-time guard so release builds with that feature fail (`#[cfg(all(feature = "deterministic-rng", not(debug_assertions)))] compile_error!`).
2. `normalize`: master-password normalization (Unicode NFKD, UTF-8; **no trimming**). Pin the normalization crate version exactly; add vectors (composed/decomposed accents, Hangul, full-width, emoji) and a test documenting the Unicode version so table changes fail CI (SEC-C10).
3. `kdf`: Argon2id v1.3 via `argon2` crate. `KdfParams {m_kib, t, p, salt}` with `validate()` enforcing **floors and ceilings** from doc 04 §3 (m 64-1024 MiB, t 3-10, p 1-8, salt exactly 16 B) *before* any hashing (SEC-C02, SEC-C11). `calibrate(target_ms, max_m_kib)` that picks parameters for ~0.5-1.0 s unlock, never below the floors. Output: 32-byte `MasterKey`.
4. `aead`: XChaCha20-Poly1305 wrapper: `seal(key, aad, plaintext, rng) -> (nonce, ciphertext||tag)` with a fresh random 24-byte nonce per call, and `open(...)`. Constant-time tag check via the crate; distinct error types for "authentication failed" vs "malformed input" (but never reveal which byte differed).
5. `hkdf`: HKDF-SHA256 helpers: `kek_pw`, `kek_rk`, and `subkey(vk, label, epoch)` with labels `db/v1`, `log/v1`, `snapshot/v1`, `manifest/v1`, `history/v1` exactly as in doc 04 §2 (`salt = vault_id`, `info = label || epoch` encoding specified by you, documented, and tested).
6. `keys`: newtypes `VaultKey`, `MasterKey`, `Kek`, `SubKey`, `RecoveryKey` wrapping `Zeroizing<[u8; N]>`. **No `Debug`/`Display`/`Clone`/`Serialize`** that could leak (derive nothing by default; provide explicit `expose_secret`-style accessors, `pub(crate)` where possible). Implement `Drop` zeroization via `zeroize` (SEC-C06).
7. `recovery_key`: 160 random bits -> 32 Crockford Base32 chars grouped `XXXXX-XXXXX-...` plus a 2-char checksum group; parser tolerant of case, spaces, hyphens, and the Crockford substitutions (I/L->1, O->0); rejects bad checksums with a typed error. **Checksum (decided):** first 10 bits of `SHA-256("aryavault/rk-check/v1" || 20 key bytes)` as 2 Crockford chars. Add this decision to `docs/04-crypto-spec.md` §4 in a **separate commit touching only that file** (commit message `docs(spec): define recovery-key checksum`).
8. `wrap`: `wrap_pw` / `wrap_rk` and the corresponding unwraps with the **separate AADs** from doc 04 §5 (`"aryavault/wrap-pw/v1" || vault_id || epoch || canonical_cbor(kdf)` and `"aryavault/wrap-rk/v1" || vault_id || epoch`). `header_version` must NOT be in any AAD (SEC-C12). Canonical CBOR for the kdf struct: use `ciborium` with deterministic encoding, or implement the minimal deterministic encoder; document the choice and test determinism.
9. `vault_key`: generate a random VK (OS RNG); `change_password(old_pw, new_pw, header_wraps, ...)` that re-wraps VK under a new password-derived KEK **without needing the recovery key** (an integration test must prove the recovery wrap is untouched and still opens after a password change).

## Tests (required; reference requirement IDs in test names or comments)
- Known-answer tests: Argon2id (RFC 9106 test vector), XChaCha20-Poly1305 (draft-irtf-cfrg-xchacha vectors), HKDF-SHA256 (RFC 5869 vectors). Source vectors from the RFCs/drafts or the libraries' own published test suites; cite the source in a comment. **If you cannot verify a vector against its source, do not invent one**: flag it in the PR.
- Round trips and **negative tests**: flip every byte/field of nonce, ciphertext, tag, AAD, vault_id, epoch, kdf params -> unwrap/open must fail.
- Wrong password / wrong recovery key -> typed authentication error, not a panic.
- Parameter bounds: every floor/ceiling boundary (accept at limit, reject beyond) (SEC-C11).
- Property tests (`proptest`): seal/open round trip for arbitrary plaintext/AAD; recovery-key encode/decode round trip; parser never panics on arbitrary strings.
- Nonce sanity: 1,000,000 generated nonces are all distinct (SEC-C04 smoke test).
- Zeroization: test that key types zero their buffers on drop where the language allows observing it (e.g. via a custom wrapper in test), and document limits.
- Unbiasedness is not relevant here (generator is T04).

## Acceptance criteria
- SEC-C01 to SEC-C07, C10, C11, C12 each have at least one test; list them in the PR in a table (requirement -> test).
- Allowed dependencies only: `argon2`, `chacha20poly1305`, `hkdf`, `sha2`, `zeroize`, `secrecy` (optional), `subtle`, `getrandom`, `ciborium`, `unicode-normalization`, `thiserror`, plus dev: `proptest`, `hex`. Justify anything else in the PR; `cargo deny check` must pass.
- No `unsafe`, no `unwrap`/`expect`/`panic` outside tests, `missing_docs` clean.
- Public API doc comments state security contracts (e.g. "caller must not reuse nonce" is not possible: the API generates nonces internally).
- Benchmarks (`criterion` or a simple test `#[ignore]`d) for Argon2 at default params, printed in the PR.

## Out of scope
Header/envelope byte formats (T02), storage, any I/O or networking, biometrics/OS keystore, sync.

## PR checklist
Use the repo PR template. Include: requirement->test table, "Spec questions" section, benchmark numbers, list of dependencies added with justification.

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
