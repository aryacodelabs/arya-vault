# A01: Extract the `session` library from the CLI

**Branch:** `m2/a01-session` · **Depends on:** nothing · **New crate:** `core/crates/session` (`arya-vault-session`) · **Also touches:** `core/crates/cli` (becomes a thin client) · **Reviewer attention:** high (key plumbing)

## Context
Read `CLAUDE.md`, `docs/04-crypto-spec.md` §2-5, §8-9, `docs/07-ux-and-recovery.md` §3-5, **`docs/14-app-api-contract.md`** (§4.1, §5), `docs/reviews/m1-exit-check.md`. Today the vault lifecycle lives in the CLI: `core/crates/cli/src/layout.rs` (`VaultDir`, header read/select/publish, KDF profiles, `db_key`, `Unlocked`) and `cli/src/cmd/vault_cmd.rs` (`create`, `unlock-check`, `password change`, `recover`, `rotate-recovery-key`, `info`). A Flutter app cannot reuse that. Move it into a library with a clean API; the CLI then calls the library. **Behavior must not change**: the CLI's 8 end-to-end tests (`core/crates/cli/tests/e2e.rs`) must pass unmodified (except for imports/paths if unavoidable, and say so in the PR).

## Deliverables
1. **Crate `arya-vault-session`** (no UI, no FFI, no networking). Suggested modules: `layout` (vault directory + header files, atomic header publish: temp file + rename, then remove superseded headers), `profile` (KDF profiles with the `kdf::calibrate` entry points), `lifecycle` (create, unlock, change password, recover, regenerate recovery key), `state` (the session state machine), `error` (typed `SessionError` mapped to the `AppErrorCode`s in `docs/14` §2 via a `code()` method), `meta` (onboarding flag etc.).
2. **`Session` type** implementing the state machine of `docs/14` §5:
   - `Session::open_dir(path) -> Session` (Locked or NoVault); `status() -> VaultStatus` that works while locked and reads only plaintext header data (format version, exists, onboarding flag stored in plaintext marker or derived; **document where `onboardingComplete` lives**: it must be readable while locked to route the UI, so store a small non-secret file/marker next to the header and keep it consistent with the vault; never put secrets there).
   - `create(password, profile) -> RecoveryKeyResult` leaving the session **unlocked, onboarding pending**; the pending recovery key is held in a `Zeroizing` field until `confirm_recovery_key(answers)` succeeds (3 random groups chosen by the session; the API takes `Vec<(index, text)>`, compares in constant time, normalizes case/hyphens/Crockford substitutions) or the session locks/drops (then the key is gone and the user must regenerate, `docs/14` §4.1).
   - `unlock(password)`, `lock()` (zeroizes the vault key and DB key, closes the DB via `Vault::close`, drops the pending recovery key; idempotent; a second `lock()` is a no-op), `change_password(old, new)`, `recover(recovery_key, new_password)`, `regenerate_recovery_key(password)`, `verify_password(password)`.
   - Failure delay: after N wrong passwords apply an increasing **local** delay (cosmetic, `docs/07` §4); implement as a policy object with an injectable clock; never block the thread for long (return the remaining wait in a typed error `Backoff { retry_after_ms }` instead of sleeping).
   - Access to the unlocked vault through a single guarded accessor (`with_vault(|v| ...)` returning `locked` when not unlocked) so the FFI layer cannot touch a vault after `lock()`.
3. **No secrets in public types**: `Debug` for any struct holding key material is redacted or not derived; recovery key text only via an explicit accessor returning `Zeroizing<String>`.
4. **CLI refactor**: `layout.rs` and the lifecycle parts of `vault_cmd.rs` shrink to argument parsing + calls into `session`. Keep `secrets.rs`, output formatting and the Windows/no-TTY fix as they are.
5. **Docs**: `docs/14` §5 stays authoritative; if implementation reveals a contract problem, describe it in "Spec questions" and do **not** edit the contract in this PR.

## Tests
- All existing tests (crypto, storage, vault, CLI e2e) pass.
- New unit/integration tests for `Session`: full lifecycle incl. `create -> confirm (wrong, right) -> lock -> unlock -> change password -> recover`; state-machine violations return `locked`; **no vault access after `lock()`** (use a probe to prove the DB handle is closed and keys zeroized); pending key dropped on lock; backoff policy with a fake clock; header ordering/rollback rules (`select_active` with lower epoch rejected); corrupted/missing/oversized header files; two processes opening the same dir (document behavior: second gets `busy`, add a lock file or rely on SQLite locking, choose and justify).
- SEC mapping: SEC-A01 (re-entry verification logic), SEC-A03 partial (reboot/72h belongs to A02), SEC-A04 (lock wipes keys), SEC-A05, SEC-A07, SEC-C06 (lock), SEC-C12.
- Disk-scan canary test over a `Session` lifecycle (like the CLI's) proving no plaintext secrets in the directory.

## Acceptance criteria
- `cli` behavior unchanged; line count of `cli/src/layout.rs` reduced to what is truly CLI-specific.
- New crate has `#![forbid(unsafe_code)]` via workspace lints, `missing_docs` clean, no new crypto, dependencies only from existing crates (+ `thiserror`/`zeroize`, already present).
- Report any code you found in the CLI that is lifecycle logic but you intentionally left there, and why.

## Out of scope
Quick unlock (A02), key rotation (A03), FFI (A04), any UI, sync.

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
