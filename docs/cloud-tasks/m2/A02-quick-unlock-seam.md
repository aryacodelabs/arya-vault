# A02: Quick-unlock seam and policy

**Branch:** `m2/a02-quick-unlock` · **Depends on:** A01 merged · **Crate:** `core/crates/session` · **Reviewer attention:** high (key release path)

## Context
Read `CLAUDE.md`, `docs/04-crypto-spec.md` §8 (biometric / quick unlock; the review's M2 finding about Windows Hello and the Linux caveat), `docs/03-threat-model.md` threats 8-10, `docs/14-app-api-contract.md` §4.1 and **§6**, `docs/08-security-requirements.md` (SEC-A02, SEC-A03). The raw vault key must **never reach Dart**: the platform piece is a Rust trait implemented per platform; Dart only calls `unlock_quick()`. Real OS providers need Windows/macOS/Android hardware and are **out of scope here** (done locally in L01 and in M3/M6); this task builds the seam, the policy and a fake provider with full tests.

## Deliverables
1. **`QuickUnlockProvider` trait** in `session` exactly as `docs/14` §6 (`kind`, `available`, `seal(&VaultKey) -> Blob`, `unseal(&Blob) -> VaultKey`, `boot_id`), with: `Blob` opaque bytes (provider-defined, size-limited), `QuickUnlockKind`, a typed `ProviderError` (`Unavailable`, `UserCancelled`, `Invalidated`, `Failed`). The trait object is injected into `Session` (`Session::with_provider(Box<dyn QuickUnlockProvider + Send>)`); default is a `NoProvider` that reports unsupported.
2. **Enable/disable**: `quick_unlock_enable()` (requires unlocked; seals the VK via the provider; stores the blob + a plaintext **policy record** `{enabled_at, boot_id, failure_count, kind}` in the vault directory next to the header, atomically; the record contains no secrets), `quick_unlock_disable()` (deletes both, best-effort provider revoke), `quick_unlock_status()`.
3. **`unlock_quick()`** with the policy of `docs/04` §8: fail with `quickUnlockUnavailable` (and require the master password) if **boot_id changed** (reboot; make the "require password after reboot" **configurable**, default on), **> 72 h** since the last *password* unlock (injectable clock; the clock must be monotonic-safe: if the wall clock moved backwards treat as expired), **>= 5 consecutive failures** (counter persisted; reset on success or password unlock), provider `Invalidated` (e.g. biometric enrollment changed: disable and delete the blob), or the blob fails authentication. On success the VK is used to open the DB exactly as a password unlock would (reuse A01's code path; do not duplicate key derivation) and `last_quick_unlock` updates, but the **72 h clock keys off the last password unlock**.
4. **Wipe on every failure/exit path**; no VK or blob contents in `Debug`/errors/logs.
5. **`FakeProvider`** (feature `test-support`, not default; compile-guarded out of release like `deterministic-rng`): configurable behaviors (cancel, invalidate, unavailable, boot id changes, corrupt blob) and a `seal` that uses a random in-memory key with AEAD from the crypto crate (no new crypto: use `aead::seal/open` from `arya-vault-crypto`).
6. **Docs**: add `docs/04` §8 clarification only if implementation shows a real ambiguity (separate `docs(spec)` commit); otherwise list in "Spec questions" what the platform implementers (Windows Hello `KeyCredentialManager`, Android Keystore, Apple Keychain) must guarantee: the unseal must require user presence; the blob is useless without the platform key; blob size limit; `boot_id` semantics.

## Tests
- Policy matrix with the fake clock/provider: fresh enable -> unlock OK; reboot -> denied; 71 h OK / 73 h denied; wall-clock rollback -> denied; 4 failures then success resets; 5 failures denies and password unlock resets; invalidation deletes blob and disables; disable removes files; enabling twice replaces the blob; corrupt/truncated/foreign blob rejected without panic.
- Concurrency: `lock()` during `unlock_quick` leaves a locked session with no keys.
- Disk-scan: policy file and blob contain no VK bytes in plaintext (canary VK pattern via the fake provider's observable seal output).
- Mapped requirements: SEC-A02 (what is testable without hardware; state that hardware binding is verified in L01/M3), SEC-A03, SEC-A04, SEC-C06.

## Acceptance criteria
`forbid(unsafe_code)`, `missing_docs`, no new dependencies (justify any), CI green, API matches `docs/14` §4.1 and §6 (list any divergence).

## Out of scope
Real OS providers, UI, FFI, biometric prompts.

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
