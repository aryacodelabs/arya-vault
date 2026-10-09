# B01: Flutter foundation, fake API, lock screen and onboarding

**Branch:** `m2/b01-flutter-foundation` · **Depends on:** the contract only (`docs/14`), **not** on the Rust code · **Area:** `app/` · **New files only** (plus a CI job)

## Context
Read `CLAUDE.md`, `docs/02-architecture.md` §3, §8, `docs/07-ux-and-recovery.md` (all), **`docs/14-app-api-contract.md` (binding)**, `docs/08` (SEC-A01, SEC-H03), `docs/09` (Flutter/Riverpod choices). The UI never implements crypto or parsing; it talks to an abstract `VaultApi` that mirrors the contract. In M2 the real implementation arrives later (FFI, A04); for now a **fake in-memory implementation** makes the whole app buildable and testable.

## Environment
Install a **pinned** stable Flutter version in the session (official tarball from flutter.dev, verify the version string; record the version in `app/.flutter-version` and in `pubspec.yaml` `environment`). If Flutter cannot be installed in this environment, **stop and report**; do not fake results. Tests run headless with `flutter test`.

## Deliverables
1. **Project scaffold** (`flutter create --platforms=windows,android --org <placeholder.invalid> app`): remove sample code; Material 3; strict `analysis_options.yaml` (`flutter_lints` + extra rules: `avoid_print`, `unawaited_futures`, `prefer_const_constructors`, no `dart:mirrors`); `dart format` enforced. App id/organization are placeholders pending the owner's naming decision (note in PR).
2. **Dependencies (justify each in the PR; keep the list short):** `flutter_riverpod`, `go_router`, `flutter_localizations`/`intl`. No analytics, no networking packages, no crash-reporting SDKs (SEC-H04).
3. **Layers** under `lib/src/`: `api/` (`vault_api.dart` interface + DTOs + `AppError`, 1:1 with `docs/14`; **`fake_vault_api.dart`**: in-memory implementation with realistic behaviors: wrong password -> `wrongCredentials`, `checkMasterPassword` policy (min 12), onboarding-pending flow with a fake recovery key and 3-group confirmation, lock/unlock state machine, failure backoff, 2,000 sample items for later tasks), `state/` (Riverpod providers; `VaultStatus` driving routing), `ui/`, `theme/`, `l10n/` (ARB, English only, all strings externalized; **no hard-coded user-visible strings**), `services/` (interfaces only: `ClipboardService`, `FilePickerService`, `PlatformEvents`; fakes for tests).
4. **`SecretBytes`** helper (wraps `Uint8List`, `wipe()`, `use((bytes) {...})` that wipes in `finally`) used for every password/recovery-key buffer; text fields feed it; unit tests. State plainly in code docs and the PR the Dart limitation (rendering to `String` can't be wiped).
5. **Adaptive layout scaffolding:** breakpoint-based shell (desktop >= 900 px three-pane placeholder, compact single-pane); theme tokens (light/dark/system), large-text safe, focus visuals for keyboard navigation, `Semantics` on every interactive widget.
6. **Routing guard** from `VaultStatus`: no vault -> Welcome; vault + locked -> Lock; onboarding incomplete -> Recovery step; unlocked -> shell (empty placeholder home with a "vault is unlocked" message and a Lock button).
7. **Screens (per `docs/07` §3-5):**
   - Welcome (create / add device placeholder "available with sync").
   - Create vault: password + confirm, **live strength meter and policy messages** from `checkMasterPassword` (works while locked), KDF profile hidden under "Advanced".
   - "No reset exists" explainer (`docs/07` §3 step E copy, localized) and **Recovery key screen**: key shown in grouped monospace blocks with the checksum group, **no clipboard-copy button on this screen**, buttons for "Print" and "Save as file" through `FilePickerService`/`PrintService` interfaces (fakes in tests), a "Show/hide" toggle, screen capture warning text; the Continue button enabled only after the user ticks "I stored it".
   - **Verify step** (`docs/07` §3 step H): asks for 3 random groups; mismatch -> returns to the key screen; **no Skip** (a test asserts there is no code path that completes onboarding without a successful `confirmRecoveryKey`; SEC-A01).
   - Optional quick-unlock offer (shown only if `status.quickUnlock.supported`).
   - Lock screen: master password field with reveal toggle, quick-unlock button when enabled, "Use recovery key" link, backoff message from `AppError`/retry info.
   - Recover flow: recovery key entry (tolerant formatting hints, `recoveryKeyMalformed` handling) -> new password (policy meter) -> unlocked.
   - "Forgot both" explanation screen (`docs/07` §4).
8. **Accessibility:** every control labelled; focus order tests; contrast AA tokens; works at 200% text scale without overflow (tests).

## Tests
- Widget tests for every screen and for the routing guard; a full **onboarding integration-style widget test** against `FakeVaultApi` (create -> read key -> verify wrong -> verify right -> unlocked); lock/unlock/recover flows; error-code rendering for each `AppErrorCode` relevant here.
- Golden tests (light/dark/large text) for Welcome, Create, Recovery key, Verify, Lock; goldens committed with a documented command to regenerate and a note that fonts must be bundled/pinned so they are stable on Linux and Windows.
- Secret hygiene tests: no password/recovery key appears in widget `toString`, error messages, or logs captured via `debugPrint` overrides; `SecretBytes` wipe verified.
- A CI job `flutter` in `ci.yml` (Linux): analyze, format check, test; cache pub; actions pinned by SHA like the others.

## Acceptance criteria
`flutter analyze` clean, tests green, no network access code, no hard-coded strings, README in `app/` (how to run the fake-backed app on Windows: `flutter run -d windows`), contract mapping table in the PR (each `VaultApi` method <-> `docs/14` row, nothing missing, nothing extra).

## Out of scope
FFI/real core, vault item screens (B02), settings/generator (B03), Windows plugins, localization beyond English, app icons/branding.

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
