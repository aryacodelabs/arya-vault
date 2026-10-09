# A03: Vault-key rotation (SEC-A06, a MUST requirement)

**Branch:** `m2/a03-key-rotation` · **Depends on:** A01 merged · **Crates:** `session`, `crypto` (if the wrap/derive helpers need a rotation entry point), `storage` (uses existing `rekey`) · **Reviewer attention:** maximum (two-person-review area)

## Context
Read `CLAUDE.md`, `docs/04-crypto-spec.md` §2, §5, §9, **§10 (Vault key rotation)**, the review report `docs/reviews/2026-10-spec-review-04-06.md` (findings M3, M4), `docs/03-threat-model.md` threat 17, `docs/08-security-requirements.md` SEC-A06, `docs/06-sync-protocol.md` §8.1. M1 did not implement rotation (`m1-exit-check` item 7). This task implements the **local** part only; the cloud/sync parts (rotation snapshot, `covers` map, late-device re-emission) are M4.

## Deliverables
1. **`Session::rotate_keys(password) -> RotationOutcome`** (unlocked session; requires re-authentication with the master password, a policy requirement because the operation is destructive): 
   1. generate VK' from the OS RNG;
   2. increment `epoch`;
   3. derive `K_db'` (HKDF label `db/v1` with the new epoch per `docs/04` §2) and **re-key the SQLCipher DB** (`Db::rekey`), crash-safe: the sequence must guarantee that a crash at any point leaves a vault that opens either entirely under the old keys/epoch or entirely under the new ones. Design the ordering (e.g. write the new header to a temp name, re-key inside a transaction-like protocol, publish the new header last; keep the old header until the DB is verified under the new key), document it in the module docs and prove it with a kill-at-random-points test (>= 200 iterations, like `storage/tests/crash_safety.rs`);
   4. produce a new header with `wrap_pw'` and `wrap_rk'` for VK'. **A rotation needs the recovery key or a new one**: since `wrap_rk` can only be created with the RK, rotation **always generates a new recovery key** and returns it (onboarding-style pending confirmation, `docs/14` §4.1 `regenerateRecoveryKey`); the old RK stops working;
   5. delete superseded headers (the old header is a brute-force/verification target for the old VK; per `docs/04` §9 delete promptly), re-derive and replace the quick-unlock blob if enabled (the blob holds the old VK: **disable quick unlock on rotation** and ask the user to re-enable);
   6. emit a typed **`RotationRecord { old_epoch, new_epoch, at }`** stored in the vault meta so M4's sync can build the rotation snapshot (do **not** implement sync).
2. **`change_password_and_rotate(old, new)`** = the "Change password and rotate keys" action (`docs/14` §4.1 `changePassword(rotateKeys: true)`): one atomic user-visible operation (rotate + new password wrap), returning the new recovery key.
3. **Old-epoch handling in the format layer:** headers with `epoch` lower than the highest the **local device** has recorded are rejected (`docs/04` §5, review M4); persist the highest-seen epoch in the vault `meta`. Add tests for the downgrade attempt.
4. **CLI**: add `arya-vault vault rotate-keys` (reads password from stdin/TTY like other commands; prints the new recovery key only with `--reveal`) plus an e2e test. This is the only CLI change.

## Tests
- Full rotation round trip: items readable before/after; old password still works **only if** the user kept it (rotate keeps the master password; check `change_password_and_rotate` changes it); old recovery key rejected; new recovery key works; old header files gone; old epoch header re-introduced by an attacker (copy the old header back) is rejected, and cannot decrypt the DB (DB key changed).
- **Crash safety**: kill at random points in rotation; the vault always opens (with the old or the new credentials as documented), `integrity_check` OK, no state where neither header opens the DB.
- Quick unlock disabled after rotation; pending recovery key confirmation required after rotation (`onboardingComplete=false`).
- Property: after N random rotations interleaved with edits, all items survive.
- Disk-scan canary: after rotation, **old** `K_db` cannot open the file, plaintext canaries absent.
- Mapped: SEC-A06, SEC-A05, SEC-C05 (AAD with new epoch), SEC-C06, SEC-S06, SEC-C12.

## Acceptance criteria
No new crypto; uses only existing primitives; `rekey` crash-safety argument written in the PR; two clear paragraphs under "Deferred" about what M4 must add.

## Out of scope
Sync/cloud steps of `docs/04` §10 (items 2, 5, 7 in the cloud sense), UI, FFI.

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
