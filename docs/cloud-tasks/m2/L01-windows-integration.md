# L01: Windows integration (local task: owner's machine with Claude's help)

Not a cloud prompt: it needs Windows, a display, Windows Hello hardware/PIN, and iterative UI work. Start after A04 and B01-B03 are merged and L00 is complete.

## Goals
1. **Wire the app to the real core.** Implement `FfiVaultApi` (Dart) against the generated bindings, keep `FakeVaultApi` for tests, and choose the implementation at startup (`--dart-define=FAKE_API=true` for demos). Assert `API_VERSION` equality at start. Make the `docs/14` contract tests (a shared suite run against both implementations) pass for the FFI one.
2. **Windows build**: `flutter build windows` links `arya_vault_ffi.dll` (+ SQLCipher/OpenSSL statically); installer layout; the DLL is loaded only from the app directory (no search-path hijack); release build with `panic = "abort"`; record binary sizes.
3. **Windows Hello quick unlock (Rust)**: crate `core/crates/platform-windows` (cfg(windows)) implementing `QuickUnlockProvider` with `KeyCredentialManager` per spike S4 (deterministic signature over a fixed per-vault challenge -> HKDF -> wrapping key -> AEAD seal of the VK; blob stored by the session). Registered through A04's hook. Handle: cancelled prompt, Hello not set up, credential deleted, TPM unavailable (provider reports `Unavailable` and the UI hides the option). If S4 failed, implement the fallback chosen in L00 or ship password-only and record it as a deviation.
4. **Clipboard hygiene** (Flutter Windows plugin or `windows` crate FFI via platform channel): write with `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeClipboardContentInHistory = 0`, `CanUploadToCloudClipboard = 0`; `contentFingerprint()` for B03's "clear only if still ours"; verify in Win+V and cloud-clipboard-off behavior (S5).
5. **Capture exclusion**: `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` on the main window, toggled by the setting; verify with Snipping Tool, Teams/Zoom screen share, and PrintScreen.
6. **Lifecycle events** for `PlatformEvents`: workstation lock/unlock (`WTSRegisterSessionNotification`), system suspend/resume (`WM_POWERBROADCAST`), window minimize; feed `AutoLockController`.
7. **Default vault location**: `%LOCALAPPDATA%\AryaVault\vault\` with restrictive ACL (current user only); not in OneDrive-known-folder redirection; excluded from Windows Search indexing and (if applicable) File History.
8. **Process hardening**: disable WER crash dumps for the process (`WerAddExcludedApplication` or registry-free equivalent), no secrets in crash reports; single-instance (named mutex) so two processes cannot open the same vault.
9. **MSIX (unsigned) and a portable zip**: package identity placeholder; capabilities minimal; document how to sideload; note signing is M9.

## Verification (record evidence in `docs/reviews/m2-windows-evidence.md`)
Manual checklist from `docs/11` §9 for Windows: biometric/Hello unlock incl. invalidation when Hello credentials are reset; clipboard auto-clear and history exclusion; capture blocked; auto-lock on timeout/sleep/lock screen; token/keystore storage; install/upgrade/uninstall leaves no plaintext (disk scan with canary secrets over `%LOCALAPPDATA%`, temp, crash dump folders, MSIX data); 20,000-item scroll/search; cold start time; memory use.
Automated: contract suite against FFI; Flutter `integration_test` on Windows (`flutter test integration_test -d windows`) covering onboarding -> add item -> lock -> quick unlock/password unlock -> search -> change password.

## Exit
Everything above verified or explicitly listed as a deviation with owner sign-off; hand over to L02.
