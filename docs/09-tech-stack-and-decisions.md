# 09 — Tech Stack and Architecture Decision Records

## 1. Stack at a glance

| Layer | Choice | Version policy |
|---|---|---|
| UI framework | Flutter (Dart) stable channel | Pin minor; upgrade quarterly |
| State mgmt | Riverpod | — |
| Core language | Rust (stable, 2021+ edition) | MSRV pinned in CI |
| FFI | `flutter_rust_bridge` | Pinned |
| Crypto | RustCrypto crates (`argon2`, `chacha20poly1305`, `hkdf`, `sha2`, `zeroize`, `subtle`, `secrecy`) | Audited-crate allowlist |
| DB | SQLite + SQLCipher via `rusqlite` (bundled-sqlcipher) | — |
| Serialization | Deterministic CBOR (`ciborium`) | — |
| Async / HTTP | `tokio`, `reqwest` (rustls) | — |
| Cloud | Google Drive REST v3; CloudKit; filesystem | — |
| Native glue | Kotlin (Android), Swift (iOS/macOS), C++/Win32 or Rust (Windows), C/GTK-free via plugins (Linux) | — |
| Secure storage plugins | Keychain, Android Keystore, Windows Hello/DPAPI, libsecret | Thin custom plugins |
| i18n | ARB / `flutter_localizations` | — |
| CI | GitHub Actions (+ self-hosted Mac runner if needed) | — |
| Quality | `clippy`, `rustfmt`, `cargo-deny`, `cargo-audit`, `cargo-fuzz`, `proptest`, `dart analyze`, `flutter test` | — |
| Packaging | MSIX/MSI, notarized DMG, AppImage/Flatpak/deb/rpm, AAB, IPA | — |

## 2. ADR index
| ADR | Title | Status |
|---|---|---|
| 0001 | Flutter for UI | Accepted |
| 0002 | Shared Rust core | Accepted |
| 0003 | No custom crypto; algorithm choices | Accepted |
| 0004 | Local-first with no server | Accepted |
| 0005 | Op-log + LWW CRDT sync over dumb storage | Accepted |
| 0006 | CloudKit for iCloud (not iCloud Drive) | Proposed |
| 0007 | Mandatory recovery key | Accepted |
| 0008 | Platform order: Windows + Android first | Accepted |
| 0009 | Open source and license (MPL-2.0) | Proposed — needs owner confirmation |
| 0010 | SQLCipher for local storage | Accepted |

---

## ADR-0001 — Flutter for UI
**Context:** Need one UI codebase for Windows, macOS, Linux, Android, iOS; native look/feel and performance; "not a web app".
**Options:** Flutter · Tauri 2 (mobile support, web-view UI) · Kotlin Multiplatform + Compose Multiplatform · React Native (+ desktop forks) · fully native ×5.
**Decision:** Flutter.
**Rationale:** Single codebase renders natively on all five targets (Skia/Impeller, not a webview); mature desktop + mobile; large ecosystem; good a11y; FFI to Rust well supported.
**Consequences:** Dart language in the team; platform plugins needed for keystore/autofill; desktop UI conventions must be handled deliberately; Linux shell is GTK-based.
**Rejected:** Tauri (webview UI conflicts with "not a web app" intent and weakens screen-capture controls); native ×5 (5× cost); KMP (iOS/desktop UI maturity & Rust interop less direct).

## ADR-0002 — Shared Rust core
**Context:** Crypto, sync, and parsing must behave identically on 5 platforms and be auditable once.
**Decision:** All security-critical logic in Rust crates, exposed through a minimal FFI.
**Rationale:** Memory safety, `zeroize`, strong ecosystem of audited crypto, easy cross-compilation, fuzzing tooling, same approach as other reputable password managers.
**Consequences:** Build complexity (cross-compile for 5 OS + ABIs); two-language onboarding; FFI surface must stay small and reviewed.
**Fallback:** Pure-Dart core (`cryptography` package) if Rust toolchain proves a blocker; the spec (docs 04–06) is language-neutral so this stays possible.

## ADR-0003 — No custom crypto
**Decision:** Argon2id + XChaCha20-Poly1305 + HKDF-SHA256, wrapped-key hierarchy per doc 04. No bespoke constructions, no bespoke protocols; design mirrors well-known published manager designs. Any change requires a spec PR + two-maintainer approval + new test vectors.
**Rationale:** Standard, reviewed, library-backed, misuse-resistant (large random nonces).
**Alternative:** AES-256-GCM (hardware acceleration; nonce-misuse risk with 96-bit nonces). Kept as a documented fallback only.

## ADR-0004 — Local-first, no server
**Decision:** No AryaVault backend. Sync uses the user's own storage.
**Consequences:** No breach target; no hosting cost; no password reset (ADR-0007); no real-time push except CloudKit; sync correctness is entirely on the client (hence docs 06 & 11).

## ADR-0005 — Op-log + LWW over dumb storage
**Context:** Drive/iCloud/folders have no transactions; multi-device concurrent edits.
**Options:** (a) one encrypted vault file replaced on sync — conflicts & data loss; (b) per-item files — many files, rate limits, still conflict on same item; (c) **single-writer op-log + deterministic merge**; (d) full CRDT library (Automerge/Yjs).
**Decision:** (c), with field-level LWW registers on HLC and conflict copies for long text.
**Rationale:** Single-writer files eliminate storage-level conflicts; LWW per field is simple, provable, and fits credentials data; avoids heavyweight CRDT libs' metadata growth.
**Consequences:** Needs compaction logic and manifest/hash-chain integrity; note bodies don't merge character-wise (conflict copy instead).

## ADR-0006 — CloudKit for iCloud
**Decision (proposed):** Use CloudKit private database in a custom zone; fall back to iCloud Drive ubiquity container if needed.
**Rationale:** Push notifications, CAS via change tags, structured quotas; avoids file-placeholder download complexity.
**Consequences:** Requires iCloud entitlement + container; Apple-only (documented); testing on real devices needed; Apple Developer Program membership ($99/yr).

## ADR-0007 — Mandatory recovery key
**Decision:** Recovery key generated at creation; onboarding cannot finish until the user proves they saved it (re-entry check). Accepted per product review.
**Rationale:** No server ⇒ no reset path. Without a hard rule, forgotten-password data loss is inevitable.
**Consequences:** Slightly longer onboarding; clear user education required; regeneration flow needed.

## ADR-0008 — Platform order
**Decision:** Windows + Android → macOS + iOS → Linux. Accepted per product review.
**Rationale:** Matches the owner's current devices and tooling (Windows dev machine); Android has simplest dev loop for mobile; iOS/macOS builds need Apple hardware & fees, scheduled when CloudKit work begins.
**Consequences:** Early Drive-based sync demo covers Windows↔Android — the most common cross-ecosystem pair; iCloud arrives with Apple platforms.

## ADR-0009 — Open source & license
**Decision (proposed):** Open source under **MPL-2.0** (file-level copyleft).
**Options:** Apache-2.0 (permissive; closed forks possible) · GPL-3.0/AGPL (strong copyleft; friction with app-store distribution and external contributors unless CLA) · MPL-2.0.
**Rationale:** MPL keeps modifications to our files open, is App Store–compatible, and doesn't burden consumers of the Rust core as a library.
**Action:** Owner to confirm license and copyright holder name; add `LICENSE`, SPDX headers, DCO sign-off (preferred over CLA).
**Also public:** security audit reports, threat model, release signing keys fingerprints.

## ADR-0010 — SQLCipher
**Decision:** Local store is SQLite via SQLCipher with a raw 256-bit key from HKDF(VK).
**Rationale:** Mature, widely audited, FTS5 search inside encrypted file, transactions/crash safety for free.
**Alternatives:** Custom encrypted file (rejected: reinvention), plain SQLite + per-field encryption (rejected: leaks structure/metadata, harder search).
**Consequences:** SQLCipher licensing (BSD-style Community Edition) acceptable; build must bundle it per platform.

---

## 3. Dependency policy
- Prefer std/RustCrypto; each new crate needs justification in the PR (maintenance, audit status, transitive count).
- No dependency with network/telemetry behavior.
- Dart packages: only for UI and platform glue; crypto never in Dart.
- All deps pinned by lockfile; updates via reviewed PRs.

## 4. Build matrix (targets)
| Platform | Rust target(s) | Notes |
|---|---|---|
| Windows | `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc` | Flutter Windows runner |
| macOS | `aarch64-apple-darwin`, `x86_64-apple-darwin` | Universal binary |
| Linux | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | Flatpak + AppImage |
| Android | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | NDK via `cargo-ndk` |
| iOS | `aarch64-apple-ios`, `aarch64-apple-ios-sim` | XCFramework |
