# M2 Windows evidence (L01)

Evidence for `docs/cloud-tasks/m2/L01-windows-integration.md`, recorded as work lands. Machine: Windows 11
(10.0.26300), Flutter 3.47.7, Visual Studio 2026, release build. Fake canary data only. Items not listed
as verified are **not verified**.

## Verified

| What | Result | Source |
|---|---|---|
| App uses the real core (`FfiVaultApi`), `API_VERSION` asserted at start | API version 2 on both sides; CI job `bindings` fails if they differ | PR #30, #32 |
| docs/14 contract suite against the real core | 17 scenarios pass against `FakeVaultApi` and the real core (Linux in CI, Windows locally) | PR #30, #32 |
| Generated bindings are current | `tools/codegen.sh --check` in CI; Windows-generated files equal Linux-generated ones | PR #32 |
| Recovery-key redraw after a mismatch (`recoveryChallenge`) | ffi tests, contract suite, onboarding widget test | PR #31 |
| `flutter build windows --release` links the core | Builds in about 82 s; CMake installs `arya_vault_ffi.dll` next to the exe and fails if it is missing | PR #33 |
| Bundle sizes | `arya_vault.exe` 88 KB, `flutter_windows.dll` 20.8 MB, `arya_vault_ffi.dll` 9.6 MB, total 38.4 MB | PR #33 |
| First launch with the real core | Starts, window shown, about 208 MB working set after 8 s (single measurement, release build) | local run |
| Manual flow on the real core | Create vault, read and verify recovery key (wrong then right answers), add an item, lock, unlock with wrong and right password: worked as far as the owner tested | owner, local run |
| Vault file at rest | `vault.db` does not start with the SQLite plaintext header; the 336-byte header file holds only key-wrap parameters; the WAL was empty | local scan of the test vault |

| Windows clipboard hygiene, capture exclusion, lifecycle events (runner C++ `PlatformChannel`) | Manual checks run by the owner on a release build: copied secret kept out of Win+V history, auto-clear, capture blocked and unblocked, lock on workstation lock, lock on sleep. Reported as passing; **not itemized**, so per-check detail (for example exact clear timing) is not recorded | PR #36, owner |
| System Save and Open dialogs (runner C++) | Save as file for the recovery key checked by the owner; reported as working. Contents of the saved file not inspected by Claude | PR #36, owner |

| Windows Hello quick unlock in the app (PR #43 fix) | Owner turned it on in Settings and unlocked with Hello on a release build: **works**. Cancel, credential removal and reboot not reported, so not recorded as verified. Claude saw the resulting files: `quick-unlock.blob` 121 bytes and `quick-unlock.policy` 52 bytes next to `vault.db` | owner, local run |
| Vault folder ACL on a real vault (PR #44 build) | `icacls` shows only `<PC>\<user>:(OI)(CI)(F)`; the folder and `vault.db` carry the NotContentIndexed attribute | local run (Claude) |
| Portable zip and unsigned MSIX (`tools/package/package-windows.ps1`) | Zip 16.5 MB and MSIX 16.5 MB, checksums in `SHA256SUMS.txt`; the extracted zip runs from a fresh folder (title "AryaVault"). The MSIX was **built** (`makeappx` succeeded) but **not installed or run** | local run (Claude) |

The test vault lived in a temp profile (`LOCALAPPDATA` overridden) and was deleted afterwards.

## Hardening (PR for goals 7 and 8)

| What | Result |
|---|---|
| Single instance (named mutex `Local\AryaVault.SingleInstance.v1`) | A second launch exited within seconds and left one process (Claude, release build). Bringing the running window to the front was not observed by Claude |
| Window title and product name | Title reads "AryaVault" (Claude, `MainWindowTitle`) |
| Vault folder ACL (current user only, no inheritance) and not-content-indexed flag | Two automated Windows tests on a temp folder check the real `icacls` and `attrib` output. **Not yet checked on the folder of a real vault**, and not checked from a second Windows account |
| Crash reports | `SetErrorMode` plus `WerSetFlags(NOHEAP, NO_UI)`: no dialog and no heap in reports made by this process. **Limit:** a machine-wide WER LocalDumps policy is not overridden, so a crash while unlocked can still write a full dump with keys in it. Locking zeroes the keys, so the exposure is the unlocked time only. A crash was **not** provoked to confirm what is written |

## Not verified yet (L01 goals still open)

- Windows Hello: the reboot requirement of the policy (password required after a reboot), cancelling the prompt, a removed or reset credential and "Hello not set up" are not verified. The app shows one message for every quick-unlock failure.
- Print on Windows (the recovery sheet's Print button still reports "unavailable").
- PrintScreen and Teams/Zoom share results for capture exclusion, and the cloud-clipboard check from spike S5.
- Default vault location ACL (current user only), exclusion from indexing and File History, WER dump exclusion, single-instance mutex.
- Installing and running the MSIX (and what uninstalling does to the vault data); a DLL search-path hijack test; code signing.
- Disk scan with canary secrets over `%LOCALAPPDATA%`, temp and crash-dump folders; the scan above looked at file headers only, not at the text of the owner's items.
- Memory with the 20,000-item vault *unlocked*, and scrolling that list in the UI (the core-side numbers and the lock-screen start numbers are in the next sections).
- Window title still `arya_vault`.

## 20,000-item core measurements and text-level canary scan (2026-10-14)

`cargo test -p arya-vault-ffi --test l02_bench -- --ignored --nocapture` with an optimised test
profile (`CARGO_PROFILE_TEST_OPT_LEVEL=3`; the real release profile cannot build the test-only
fake provider). Same Windows 11 machine as above; fake canary data only; Argon2id Low profile.

| Measure | Result |
|---|---|
| Create 20,000 items (one at a time) | 64.9 s |
| List, first page of 50 | 79 ms |
| List, last page of 50 | 73 ms |
| Search, one exact title | 34 ms |
| Search, prefix matching 20,000 items | 31 ms |
| Search, no match | 0.9 ms |
| Unlock with 20,000 items (Low profile, includes Argon2id) | 155 ms |
| Item count | 126 ms |
| Vault folder after the run | 2 files, 42.9 MB |

Canary scan: after locking, all files of the vault folder were searched for the item text
(`CANARY-L02-zq7`, `password-`, a URL host) and the master password, as UTF-8 and UTF-16LE.
No match (4 needles x 2 files).

Limits: this is the core through its public API, not the Flutter UI, so it says nothing about
widget scroll speed, app cold start or app memory. The scan covered the vault folder only, not
`%TEMP%`, crash dumps or the pagefile. List pages cost about 75 ms each, above the 50 ms an
interactive list would like; the cause (decrypting 50 rows versus a fixed per-call cost) is not
yet investigated.

## Integration test and start-up (2026-10-14)

- `flutter test integration_test -d windows` (new `app/integration_test/app_test.dart`) builds the
  Windows runner and drives the real core in a temporary folder: create and confirm a vault, add an
  item, show the lock screen, unlock with the master password through the UI, see the item in
  the list, press "Lock now", see the lock screen and no item text. Passed.
- Start-up of the packaged portable zip (extracted from `dist/`, release build) pointed at a
  folder holding the 20,000-item vault: time from process start until the main window exists,
  first launch after extracting 3.3 s; three warm launches 1.6, 1.6 and 1.8 s. Working set on the
  lock screen about 205-219 MB.
- Not measured: memory after unlocking the 20,000-item vault and scrolling it. A scripted
  keystroke attempt did not reach the window, so the working set did not change; it needs a person
  at the keyboard. "Window exists" is not "first frame drawn", so these times are a lower bound.

## Temp and crash-dump scan (2026-10-14)

After one run of the integration test (the debug build against the real core: unlock, item
shown, lock), every file modified since the run began under `%TEMP%`, `%LOCALAPPDATA%`,
`%APPDATA%`, `%PROGRAMDATA%` (including the WER report folders), `C:\Windows\Temp`, `Minidump` and
`LiveKernelReports` was searched for the item title, the user name and the master password, as
UTF-8 and UTF-16LE. 363 files, 942 MB: no match.

Limits, stated plainly:
- 268 files could not be opened (locked by running programs) and were not searched.
- The pagefile, hibernation file and live process memory were not searched.
- No dump of AryaVault itself existed, because the app did not crash. This is not a test of the
  crash path: the machine has a machine-wide `LocalDumps` policy (`%LOCALAPPDATA%\CrashDumps`
  holds dumps of other programs), and a crash of an *unlocked* AryaVault would be dumped in full by
  that policy, which the app's own WER flags cannot override (see the hardening section).
- It was the debug build, not the packaged release build, which nobody could unlock by script.
