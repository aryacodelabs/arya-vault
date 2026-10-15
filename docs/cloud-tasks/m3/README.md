# M3 Task Plan: Android

M3 goal (`docs/10-roadmap.md`): the M2 user stories pass on Android. Keystore biometrics, `FLAG_SECURE`, clipboard sensitive flag, responsive layouts, backup rules. Exit: SEC-* mobile items verified on 3 devices (low/mid/high RAM), Argon2 low-RAM handling validated. Release: 0.1 alpha.

Status: **draft, started 2026-10-15** alongside the M2 manual tests. M2's exit check is still a draft; nothing here changes its open items.

## Slices

| ID | Slice | Spec | State |
|---|---|---|---|
| C01 | Platform services on Android: backup rules, `FLAG_SECURE`, sensitive clipboard, screen-off events (shared `aryavault/platform` channel) | SEC-S04, SEC-H01, SEC-H02 | In progress (this branch) |
| C02 | Rust core for Android: `cargo ndk` build of `ffi`, load from the APK, vault dir under app-private storage | docs/09 (targets), docs/14 | Not started |
| C03 | Keystore quick unlock (biometric-bound key) behind the existing quick-unlock seam; failure codes per docs/14 7.1 | docs/04 section 8, SEC-A02, SEC-A03 | Not started; needs a spec note on StrongBox and key invalidation |
| C04 | Responsive layouts (phone widths, back navigation, keyboard) | docs/07 | Not started |
| C05 | File save / pick through the storage access framework | docs/13 | Not started |
| C06 | Argon2 low-RAM handling (calibration cap, memory failure message) | docs/04 | Not started |
| C07 | Real application id, signing config, release build (ADR-0009 pending) | docs/12 | Blocked on owner decision |

## Open points
* `applicationId` and package are still `invalid.placeholder.arya_vault`; renaming waits for ADR-0009.
* The clipboard fingerprint on Android is a salted hash of the text (no sequence number exists); Android 10+ lets only the focused app read the clipboard, so after the app goes to the background the manager cannot confirm "still ours" and will not clear. To be checked on a device, then decided: clear unconditionally on background, or accept.
* Nothing in C01 has run on a device or emulator yet.
