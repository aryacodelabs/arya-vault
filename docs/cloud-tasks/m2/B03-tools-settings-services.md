# B03: Generator, health, import/export, settings, and the security services

**Branch:** `m2/b03-tools-settings` · **Depends on:** B01 merged (B02 not required; coordinate by only touching the files listed) · **Area:** `app/lib/src/ui/tools/`, `ui/settings/`, `services/`, `state/`

## Context
Read `CLAUDE.md`, `docs/07-ux-and-recovery.md` §2, §5, §7, `docs/01-product-requirements.md` US-05/06/07/09/13/15, **`docs/14-app-api-contract.md` §4.1, §4.3-4.6**, `docs/08` (SEC-A03/A04, SEC-H01-H03, SEC-S02), `docs/13-export-format.md`. Follow B01's architecture. These services contain **logic that must be correct and testable without the OS**; the Windows-specific implementations are done later in L01, so this task defines interfaces + pure-Dart logic + fakes.

## Deliverables
1. **Generator screen** (and a reusable `PasswordGeneratorSheet` that B02's edit form launches through the `PasswordGeneratorLauncher` callback; wire it if B02 is merged, else provide the widget and a TODO note): length slider 8-128, class toggles, symbol set, exclude ambiguous, passphrase mode (words 3-12, separator, capitalize, number), **live entropy** (`entropyBits`) and strength, regenerate, copy via `ClipboardService`, "Use this password". Values held as `SecretBytes`, wiped on dismiss.
2. **Health screen**: reused/weak/old lists from `healthReport()`, tapping an entry opens the item (navigation callback), clear empty states, explains what each check means; no secrets displayed.
3. **Import / export screens** (`docs/14` §4.4): file picker via `FilePickerService`, **preview before commit** (counts, duplicates, warnings, skipped with reasons), commit with progress and result summary; export CSV requires an explicit plaintext-risk acknowledgement dialog (type-to-confirm), export encrypted asks for an export password with the policy meter and shows the "keep this password safe" warning; import encrypted asks for its password. Errors map to messages (`limitReached`, `corruptVault`...).
4. **Settings -> Security**: auto-lock (1/5/15/30/60 min, "on screen lock", "on sleep"; no "never"), clipboard clear (5-120 s, default 30), block screen capture toggle (default on), reveal-hide seconds, quick-unlock toggle (enable requires re-auth via `verifyPassword`), **Change master password** (old/new, policy meter, checkbox "also rotate keys (recommended after a suspected leak)" -> shows the new recovery key flow when `rotateKeys` is true), **Regenerate recovery key** (re-auth, `regenerateRecoveryKey`, reuse B01's key + verify widgets), **View recovery key**: explain that it cannot be shown again (not stored) and offer regeneration. Settings read/write via `getSettings`/`setSettings`; clamping is core's job but the UI offers only valid values.
5. **Settings -> Sync & backup**: a clear "coming soon" panel (no fake toggles); **About**: app/core/api versions from `info()`, licenses page (Flutter license registry + a placeholder for Rust crate licenses), link text to repository; **Diagnostics**: "Export diagnostics" -> shows the scrubbed JSON for review before saving.
6. **Services (pure Dart logic + interfaces + fakes):**
   - `AutoLockController`: injectable clock/timer; resets on user input (pointer/key events via a root `Listener`/`Focus`), locks on timeout, on `PlatformEvents.screenLocked`, `.systemSleep`, and (configurable) on window minimize; cancels pending reveal/copy timers; calls `VaultApi.lock()` and navigates; unit-tested with fake time incl. edge cases (settings change while armed, lock while locked, rapid events).
   - `ClipboardManager`: `copySecret(SecretBytes)` writes via `ClipboardPlatform.write(bytes, sensitive: true)`, schedules clear after N s; **clears only if the clipboard still contains our value** (platform exposes `contentFingerprint()`; compare against the fingerprint of what we wrote) so we do not erase something the user copied since; clears immediately on lock; wipes buffers; tested with fakes (user overwrote clipboard, app locked, timer edge cases, N changes).
   - `CaptureProtection` interface (`setBlocked(bool)`), applied on startup and when the setting changes; fake in tests.
7. **No-ops that must be honest**: any capability not implemented yet (real clipboard/capture/Hello) is behind the interface with a visible **"not available on this build"** state in settings rather than a toggle that does nothing.

## Tests
- Unit tests for the three services as described (these are the core of the task).
- Widget tests for every screen; settings persistence through the fake; re-auth gating; change-password and regenerate flows end-to-end against `FakeVaultApi` (extend the fake in a backwards-compatible way); golden tests for Generator, Health, Security settings, Import preview (light/dark).
- Secret hygiene: generated passwords/export passwords wiped on dismiss, absent from logs/`toString`; clipboard fakes never see secrets after clear; recovery key screen has no copy button.
- Mapped requirements in the PR table: SEC-A03, SEC-A04, SEC-H01 (logic), SEC-H02 (toggle + interface), SEC-H03, SEC-C09 (UI shows core-generated values only: **Dart never generates passwords**; a test greps `lib/` for `Random` usage and fails if any exists outside tests).

## Acceptance criteria
`flutter analyze`, format, tests green; the Dart sources contain **no** crypto/random generation; PR lists every `VaultApi` method used (all from `docs/14`).

## Out of scope
Windows-specific clipboard/capture/Hello implementations (L01), sync UI beyond the placeholder, TOTP, browser integration.

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
