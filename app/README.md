# AryaVault app (Flutter)

The UI of AryaVault. It contains **no security logic**: all cryptography,
parsing and validation live in the Rust core and are reached through the
`VaultApi` interface (`lib/src/api/vault_api.dart`), which mirrors
`docs/14-app-api-contract.md` one to one.

**Status (M2 / B01-B03):** the app runs against `FakeVaultApi`, an in-memory
stand-in. Nothing is encrypted and nothing is saved; closing the app forgets the
vault. The real implementation (flutter_rust_bridge over `core/crates/ffi`) is
wired in by task L01.

Implemented so far: Welcome, Create vault (live strength meter, KDF profile under
Advanced), "no reset" explainer, recovery key (print / save as file, no clipboard
button), mandatory verification (no skip), optional quick-unlock offer, Lock,
Recover with recovery key, "I have neither" explanation (B01); and the vault
screens (B02): desktop three panes / compact list-to-detail, debounced search,
type filters, folders, tags, favorites, item detail with reveal/copy, safe
Markdown notes, edit forms for login / note / card / identity, trash, history
with restore, compare for concurrent versions, folder management.

Tools and settings (B03): password generator (also the sheet the edit form
opens), password health, import (CSV, Bitwarden JSON, encrypted export) with a
preview before anything is committed, export (encrypted, or plain CSV behind a
type-to-confirm warning), and Settings: Security (auto-lock, clipboard, reveal
time, screen capture, quick unlock, change master password, recovery key), a
"coming soon" Sync panel, and About with a scrubbed diagnostics report. The
security timers are plain Dart logic over interfaces (`lib/src/services/`):
`AutoLockController`, `ClipboardManager` (clears only what it wrote) and
`CaptureProtection`. Their Windows implementations arrive with L01; until then
the settings screen says "Not available on this build" instead of showing a
switch that does nothing.

Vault behaviors worth knowing:

- Secrets are masked until an explicit Reveal; a reveal hides itself after at
  most 15 s, when the app loses focus, when you leave the item, and on lock, and
  the bytes are wiped.
- A lock drops everything, drafts of an open edit form included.
- Notes render a safe Markdown subset: no HTML, no image loading, only http(s)
  links and only after a confirmation.
- Keyboard: the item list is one Tab stop; arrows move the selection, Enter
  opens, Delete moves to the trash (with Undo).

## Toolchain

Flutter **3.47.7** (stable, Dart 3.13.5), pinned in `.flutter-version` and in
`pubspec.yaml`. CI installs exactly that tarball and checks its SHA-256. Install
the same version locally (https://docs.flutter.dev/install) and verify with
`flutter --version`.

## Run on Windows (fake-backed)

```
cd app
flutter pub get
flutter run -d windows
```

The fake pretends Windows Hello is available so the quick-unlock screens can be
exercised. Tip: create a vault, then use the recovery key that is shown to try
the recovery flow after locking (there is no persistence; do not close the app).

## Checks (from `app/`)

```
dart format --output=none --set-exit-if-changed .
flutter analyze
flutter test                       # everything, including goldens
flutter test --exclude-tags golden # without pixel goldens
TZ=UTC flutter test --update-goldens test/golden   # regenerate goldens (see test/golden/README.md)
flutter gen-l10n                   # after editing lib/src/l10n/app_en.arb
```

## Layout

| Path | What |
|---|---|
| `lib/src/api/` | `VaultApi` interface, DTOs, `AppError`, `SecretBytes`, `FakeVaultApi` |
| `lib/src/state/` | Riverpod providers, `VaultStatus`, onboarding state, routing guard |
| `lib/src/ui/` | onboarding and lock screens, shared widgets |
| `lib/src/ui/vault/` | vault screens (B02): shell, list, detail, edit, history, folders, Markdown |
| `lib/src/theme/` | light/dark themes and tokens (contrast-tested) |
| `lib/src/l10n/` | ARB (English) and generated localizations (committed) |
| `lib/src/services/` | `ClipboardService`, `FilePickerService`, `PrintService`, `PlatformEvents` interfaces and fakes |
| `lib/src/rust/` | generated FFI bindings (A04/L01; not present yet) |

## Rules this app follows

- No network code, analytics or crash-reporting packages (SEC-H04). A test scans
  `lib/` and `pubspec.yaml`.
- Every user-visible string is in the ARB file. A test fails on a literal in UI code.
- Secrets cross the API as `Uint8List` held in `SecretBytes` and are wiped after
  each call. **Limit:** once a secret is rendered as a Dart `String` or widget
  (text field content, the recovery key on screen) it cannot be wiped; the UI
  keeps those lifetimes short and never logs or prints them.
