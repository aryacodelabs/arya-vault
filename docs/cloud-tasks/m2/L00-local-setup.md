# L00: Local setup and spikes (owner's machine; I can guide step by step)

Not a cloud prompt. Do this before L01 and, ideally, before launching the cloud waves, because it can change the plan (the spikes de-risk M2/M3).

## 1. Tools to install (Windows 11)
| Tool | Why | Check |
|---|---|---|
| **Strawberry Perl** (put it **before** Git's Perl in `PATH`) | The vendored OpenSSL under SQLCipher fails to build with Git's MSYS Perl (observed: `perl ./Configure` exit code 2) | `perl -v` should say `MSWin32`, not `msys` |
| **NASM** | OpenSSL assembly on MSVC | `nasm -v` |
| **Visual Studio 2022 Build Tools** with "Desktop development with C++" and Windows 10/11 SDK | Rust MSVC target, Flutter Windows | `cl` in a Developer prompt |
| **Flutter SDK** (stable; pin the exact version in `app/.fvmrc` or `pubspec` `environment`) | UI | `flutter doctor -v` all green for Windows |
| **Android Studio** + SDK/NDK (for M3 and the Android spike) | Android builds | `flutter doctor` Android toolchain |
| **Rust targets**: `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android` and `cargo install cargo-ndk` | Android core builds | `rustup target list --installed` |
| `cargo install cargo-deny cargo-audit flutter_rust_bridge_codegen` (version pinned to the one the repo uses) | CI parity | `cargo deny --version` |

Confirm that `cargo clippy --workspace --all-targets --locked -- -D warnings` and `cargo test --workspace --locked` pass in `core/` on your machine. (Today they fail locally only because of the Perl issue above; CI on Windows is fine.)

## 2. Spikes (each <= 1 day; report results in `docs/reviews/m2-spikes.md`)
| Spike | Question | Pass criteria |
|---|---|---|
| S1 Flutter + FRB hello | Does a `flutter_rust_bridge` call to a trivial Rust function work on Windows desktop, in debug and release? | Button calls Rust, returns a string; `flutter build windows` OK |
| S2 Core in Flutter | Can the real `arya-vault-session` (after A01) / `vault` crate link into the Flutter Windows build including SQLCipher+OpenSSL? Binary size? | App opens and creates a test vault on disk |
| S3 Android core | Does the core cross-compile for 3 Android ABIs with vendored OpenSSL via `cargo-ndk`, and load in a Flutter Android build? | APK runs on a device/emulator, creates a vault |
| S4 Windows Hello | Can `KeyCredentialManager` (WinRT, via the `windows` crate) create a key, sign a fixed challenge deterministically and survive restarts? Is the signature deterministic (needed to derive a wrapping key, `docs/04` §8)? | Same challenge -> same signature across 20 runs and after reboot; prompt appears; cancel handled |
| S5 Clipboard and capture | `ExcludeClipboardContentFromMonitorProcessing` + `CanIncludeClipboardContentInHistory` formats honored by Win+V history? `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` blocks screenshots/screen share? | Manual test log |

If **S4 fails** (non-deterministic signature or unavailable), the quick-unlock design changes (alternatives: Windows Hello via `UserConsentVerifier` + TPM-bound CNG key with `NCRYPT_UI_POLICY`, or ship M2 without Hello and password-only). Decide before L01.
If **S3 fails** (OpenSSL on Android), consider SQLCipher with a prebuilt OpenSSL or libsodium-based alternative (ADR needed).
