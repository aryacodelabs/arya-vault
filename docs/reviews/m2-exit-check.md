# M2 exit check (draft)

**Scope:** milestone M2 "Windows MVP" (`docs/10-roadmap.md`), tasks A00-A04, B01-B03, L01 · **Evidence date:** 2026-10-15 · **Code state:** `main` after PR #57 (schema 3, list paging) · **Author:** Claude, so this is a **self-assessment by the same agent that wrote most of the code**. It lists evidence; it does not replace the human review `docs/08` asks for (`RV`, `AU`) and shared blind spots are likely. Where the evidence is a manual check, it says **who** ran it (owner or Claude) and what was not recorded.

Status words, used as in `m1-exit-check.md`:

* **Verified**: the stated verification method ran and passes, and the evidence is named so it can be re-run.
* **Partially verified**: part of the requirement or method is evidenced; the gap is stated.
* **Not verified**: no evidence, or the evidence contradicts the requirement.

Per-check detail of the Windows work lives in `docs/reviews/m2-windows-evidence.md` ("evidence doc" below). Machine for all Windows numbers: one Windows 11 laptop (10.0.26300), which is the *owner's development PC*, not a designated reference laptop.

---

## 1. Read this first: what is not verified

| # | Item | Why it matters | Status |
|---|---|---|---|
| 1 | **Usability test with 5 participants** (roadmap exit, L02 part 2) | Not run. No participant, no recording, no `m2-usability.md`. Every statement below about onboarding comprehension is the author's, not a user's. | **Not verified** |
| 2 | **Windows Hello beyond the happy path** (SEC-A02, SEC-A03) | Turning it on and unlocking with it works (owner, release build). Not verified on real hardware: reboot forces the password, cancelling the prompt, a removed/reset Hello credential, Hello not set up, enrolment change. The policy matrix is tested with a fake provider only. The UI shows one message for all quick-unlock failures. | **Partially verified** |
| 3 | **Human review / two-maintainer review** (`RV`, SEC-R05) | `crypto`, `sync`, `ffi` and docs 04/06 changes in M2 (kek-qu in `crypto`, the whole `ffi` crate, docs/04 section 8) were merged on the owner's approval alone; `CODEOWNERS` names one owner. No second reviewer, no external review. | **Not verified** |
| 4 | **No independent security audit** | M9. Nothing here substitutes for it. | **Not verified** |
| 5 | **Crash path of the unlocked app** (SEC-S02, SEC-H06) | The app sets Windows Error Reporting flags, but a machine-wide `LocalDumps` policy (present on the test PC) is not overridden. A crash while unlocked can write a full dump containing keys. No crash was provoked. Locking zeroes keys, so exposure is the unlocked time. | **Not verified** |
| 6 | **Memory of the unlocked app, UI scrolling of a 20,000-item vault, cold start to first frame** (L02 benchmarks) | Core-side numbers exist (section 4). App memory unlocked, widget scroll smoothness and time to first frame were **not** measured. Start-up to "window exists" and lock-screen memory were. | **Partially verified** |
| 7 | **MSIX** (roadmap: "MSIX build (unsigned dev)") | The package builds but **cannot be installed unsigned** on this Windows (0x80073D2C, then 0x80073D2B: unsigned packages may not contain executable activations). What uninstall does to vault data is unanswered. The portable zip is the shippable artefact. | **Not verified** (portable zip: verified to run) |
| 8 | **Screen-share and PrintScreen behaviour of capture exclusion** (SEC-H02), cloud-clipboard check, Print on Windows | Capture exclusion blocking and unblocking was checked by the owner, not itemised. Teams/Zoom share, PrintScreen and the cloud clipboard were not. **Print is deliberately not offered on Windows** (the Print button is hidden when the platform has no print service): sending the recovery key to the print spooler leaves a copy on disk in the spool folder, which sits badly with SEC-S02. "Save as file" and manual copy remain. | **Partially verified** |
| 9 | **Code signing** | None; SmartScreen will warn. Planned for M9. | **Not verified** |
| 10 | **Coverage gate** | CI's coverage job is informational. | **Not enforced** |
| 11 | **`integration_test` in CI** | The Windows integration test passes locally only; CI does not run it (no Windows desktop runner with the built app). | **Verified locally** |
| 12 | **Perf test flake** | `vault_performance_test` "initial list under 100 ms" fails intermittently when the machine is busy; its threshold was left unchanged. | Open |

---

## 2. Roadmap M2 scope checklist (`docs/10` M2)

| Item | Status | Evidence |
|---|---|---|
| Flutter shell, FFI bindings | Verified | `app/`, `core/crates/ffi`; `API_VERSION` check at start; CI job `bindings` (codegen is current, contract suite against the real core on Linux). |
| Lock screen, onboarding with recovery-key verification | Verified | `onboarding_flow_test.dart`, `routing_guard_test.dart`, ffi `sec_a01_*`; owner ran the full flow on the real core. Comprehension by new users: see item 1. |
| Vault CRUD, notes, search, generator, health | Verified (automated) | `vault_*_test.dart`, `generator_test.dart`, contract suite (17 scenarios, fake and real core). Owner exercised the main flows manually; per-flow detail not itemised. |
| Windows Hello unlock | Partially verified | Item 2 above; `core/crates/platform-windows` (13 fake-Hello tests, `tests/hello_manual.rs` real-Hello test run by hand), spike S4 (5/5 fresh prompts), docs/04 section 8. |
| Clipboard hygiene, capture exclusion, auto-lock | Partially verified | Section 5 (SEC-H01, H02, A04). |
| MSIX build (unsigned dev) | Not verified as installable | Item 7. |
| Packaging | Verified for the zip | `tools/package/package-windows.ps1`, `docs/windows-install.md`, `SHA256SUMS.txt`; extracted zip runs from a fresh folder. |

---

## 3. Exit criteria (`docs/10` M2 **Exit**)

| Criterion | Status | Evidence |
|---|---|---|
| US-01 create vault + recovery key | Verified | `sec_a01_create_confirm_lock_unlock`, onboarding widget tests, owner's manual run. |
| US-02 unlock; enable OS unlock | Partially verified | Password unlock verified; Hello enable + unlock verified by the owner; failure paths not (item 2). |
| US-03 logins: add/edit/delete/favorite/tag, URLs, notes, custom fields | Verified (automated) | `vault_edit_test`, `vault_detail_test`, `vault_folders_test`, vault tests `create_get_list_every_type`, `custom_fields_are_individually_addressable_registers`. |
| US-04 Markdown secure notes, searchable | Verified (automated) | `markdown_test.dart`, vault `search_*` tests. |
| US-05 generator | Verified | `generator_test.dart`, generator crate tests. |
| US-06 strength, reused/weak | Verified (automated) | `health_reused_weak_and_old`, strength tests. |
| US-07 copy with auto-clear | Partially verified | Logic: `clipboard_manager_test.dart`; native: owner's manual checks (Win+V exclusion, auto-clear), not itemised. |
| US-08 instant offline search | Verified for the core; UI not measured | Search 31-60 ms on 20,000 items (section 4). |
| US-09 import CSV / Bitwarden JSON / KDBX; export | Partially verified | CSV, Bitwarden, AryaVault import/export verified in `vault` tests and the CLI e2e; **KDBX is not implemented** (deferred since M1). The export dialogs on Windows: save dialog checked by the owner for the recovery key only. |
| US-13 change master password without re-encrypting | Verified | `sec_a05_change_password_without_the_recovery_key`, `sec_c12_*`. |
| US-15 auto-lock on timeout, sleep, screen lock | Partially verified | Logic tests; owner checked lock on workstation lock and sleep (not itemised). Lock on minimise: switch added in Settings (docs/14 section 7.2, API v5; default off), tested with the fake and the settings record; not yet seen on a real minimise. |
| Usability test, 5 users | **Not verified** | Item 1. |
| SEC-A*, SEC-S*, SEC-H01-H03 on Windows | See section 5 | |

---

## 4. Benchmarks

One Windows 11 development laptop, not a reference device (`docs/11` section 6). Core numbers come from `core/crates/ffi/tests/l02_bench.rs` with an optimised **test** profile (the release profile cannot build the test-only fake provider), Argon2id **Low** profile, fake canary items, 20,000 items. Timings vary by tens of percent between runs.

| Measure | Result | Target / note |
|---|---|---|
| `list`, page of 50, first page | 4.9 ms (was 75-122 ms before schema 3) | under 16 ms (owner decision, docs/05 5.1): **met** |
| `list`, page of 50, last page (offset 19,950) | 13-15 ms by offset; **3.7 ms by cursor** | the app's list pages by cursor since API v4 |
| `list`, 1 / 10 / 200 rows | 0.3 / 1.1 / 26 ms | |
| Search: exact title / prefix / no match | 56 / 60 / 1.5 ms (earlier quiet run: 34 / 31 / 0.9 ms) | under 100 ms: met on this machine |
| Unlock with 20,000 items (Low profile, includes Argon2id) | 155-216 ms | **Default** profile unlock on this machine not measured in this run |
| `item_count` | 11 ms (was 125 ms; index count of `visible = 1`) | measured while a package build was running, so likely a little pessimistic |
| Build 20,000 items one by one | 65 s | write path, not a target |
| Packaged app start (extracted zip, 20,000-item vault): window exists | 3.3 s first launch after extracting, 1.6-1.8 s warm | "window exists", not first frame: a lower bound |
| Working set on the lock screen | about 205-219 MB | unlocked memory **not measured** |
| Bundle | 38.4 MB extracted, 16.5 MB zip | |

Not measured: widget scroll frame times, unlocked memory, Default-profile unlock on the app, a low-end laptop.

---

## 5. Requirements for Windows

`Evidence` names tests (re-run with `cargo test --workspace --locked` and `flutter test`) or rows of the evidence doc.

### SEC-A (authentication and lock)

| ID | Requirement (short) | Status | Evidence and gap |
|---|---|---|---|
| SEC-A01 | Recovery key >= 128-bit, shown after password creation, verified by re-entry | **Verified** | ffi `sec_a01_create_confirm_lock_unlock`; session `sec_a01_confirmation_normalises_and_rejects_everything_else`; app `routing_guard_test`, `onboarding_flow_test`; owner's run on Windows. Gap: how real users cope with it (usability test). |
| SEC-A02 | Biometric unlock hardware-backed, non-exportable, invalidated on enrolment change | **Partially verified** | Provider uses `KeyCredentialManager` (spike S4: deterministic signature, fresh prompt per signature); `sec_a02_invalidation_disables_quick_unlock_and_deletes_the_blob` with a fake. Whether the key is TPM-backed on the test PC, and real enrolment-change invalidation, were not checked. |
| SEC-A03 | Password after reboot, 72 h, or 5 failed attempts | **Partially verified** | `sec_a03_*` tests (session, with fake provider and clock). Real reboot on Windows not verified (`boot_id` is implemented, never exercised across a reboot). |
| SEC-A04 | Auto-lock on timeout, screen lock, sleep; lock wipes keys and closes DB | **Partially verified** | `sec_a04_lock_closes_the_database_and_with_vault_refuses_afterwards`, `sec_a04_lock_during_unlock_quick_leaves_a_locked_session_with_no_keys`, `auto_lock_test.dart`, `windows_platform_test.dart` (events); owner's manual lock-on-screen-lock and sleep (not itemised). Timeout path on the real build not itemised. |
| SEC-A05 | Password change does not re-encrypt data | **Verified** | `sec_a05_change_password_without_the_recovery_key` (ffi, session, CLI). |
| SEC-A06 | VK rotation available and tested | **Verified** (local) | `sec_a06_*` (crypto, session x6, storage backups, ffi, CLI); crash tests `sec_s06_*` for rotation. Cloud rotation steps are M4. App checkbox comprehension: usability test. |
| SEC-A07 | Master password length >= 12, strength check | **Verified** | `sec_a07_weak_master_passwords_are_rejected_everywhere`, generator `strength.rs`, create-screen tests. |

### SEC-S (storage)

| ID | Requirement (short) | Status | Evidence and gap |
|---|---|---|---|
| SEC-S01 | DB is SQLCipher-encrypted with a VK-derived key | **Verified** | storage tests; CLI/ffi disk scans; evidence doc: real vault does not start with the SQLite header. |
| SEC-S02 | No plaintext secrets in files, logs, crash reports, temp files, OS backups | **Partially verified** | ffi `sec_s02_nothing_secret_is_on_disk_after_a_session`; 20,000-item scan of the vault folder (no match); scan of `%TEMP%`, `%LOCALAPPDATA%`, `%APPDATA%`, `%PROGRAMDATA%`, WER folders after an integration-test run (363 files, 942 MB, none; 268 locked files not searched). Not covered: pagefile, hibernation, crash dumps of AryaVault itself (item 5), OS backups, the release build. |
| SEC-S03 | OAuth tokens in OS keystore | **Not applicable in M2** | No provider yet (M5). |
| SEC-S04 | Android/iOS backup flags | **Not applicable in M2** | M3/M6. |
| SEC-S05 | Search index only inside the encrypted DB | **Verified** (`RV` pending) | storage tests (`SEC-S05` scan); the new `visible` column and `item_visible_updated` index hold only ids, flags and HLCs. Human review not done. |
| SEC-S06 | Atomic writes, crash safety with fault injection | **Verified** | `sec_s06_*` (storage create/rekey 200 kills each, rotation, vault `every_mutation_is_all_or_nothing`). New in M2: migration to schema 3 is one transaction with a pre-migration encrypted backup. |

### SEC-H and the others the task names

| ID | Requirement (short) | Status | Evidence and gap |
|---|---|---|---|
| SEC-H01 | Clipboard cleared after <= 30 s; sensitive flag set | **Partially verified** | `clipboard_manager_test.dart`, `windows_platform_test.dart` (flag passed); runner sets `ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeClipboardContentInHistory = 0`, `CanUploadToCloudClipboard = 0`; owner's manual check (Win+V, auto-clear). Cloud-clipboard behaviour and exact timings were not recorded. |
| SEC-H02 | Screen capture blocked for vault screens | **Partially verified** | `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`; owner confirmed capture blocked and unblocked. PrintScreen, Teams/Zoom share not checked. |
| SEC-H03 | Secrets masked by default; reveal hides <= 15 s | **Verified** | `create_and_key_screens_test.dart`, `vault_detail_test.dart` (`hidden within 15 s`), ffi prefs clamp 1-15 s. |
| SEC-C06 | Zeroize keys on lock and drop | **Partially verified** | `sec_c06_*` (crypto drops, session lock, rotation, quick unlock). No memory-inspection test (`MT`), no `mlock`; stale copies in the Dart layer, allocator reuse and swap are not ruled out (`crates/ffi/README.md`). |
| SEC-H04 | No analytics, no network except provider | **Verified** (app source) | `policy_test.dart`; no network capture test was run. |

---

## 6. Deferred items and spec questions raised during M2

Taken from the PR descriptions of A00-A04, B01-B03 and L01. None is silently resolved; "Open" means nobody has decided.

| Source | Question or deferral | State |
|---|---|---|
| A01 / A03 (PR #20) | docs/04 section 10 says `PRAGMA rekey`; the implementation re-keys a copy and swaps it in behind the header commit | Open: needs a one-line spec note |
| A03 | Whole-vault rollback to a consistent old header + database is not detectable locally | Open: accepted limit, M4 sync adds detection |
| A03 | `highest_epoch` is written at the first unlock of an older vault; read-only media would fail | Open |
| A03 | A second unlocked session can lose writes during a rotation | Open; mitigated by single-instance on Windows |
| A03 | No secure erase of the old database blocks after rotation | Open: inherent |
| A03 | `regenerateRecoveryKey(rotateKeys)` maps to the session's rotate call; the contract has no KDF profile argument | Open |
| A04 (PR #21) | 62 functions (61 + a bootstrap `initCore`), listed deviations | Contract updated by #26 |
| B01 (PR #24, #26, #31) | `recoveryChallenge` missing from the contract and ffi | **Resolved** (docs/14, ffi, app; `API_VERSION` 2) |
| B02 (PR #25) | The count line shows the loaded count, `itemCount` unused | Open (minor) |
| B03 (PR #27) | `changePassword` returns nothing; after rotation the app fetches a fresh key rather than the rotation's | Spec change proposed (docs/14 §7.3, `API_VERSION` 6); code follows once approved |
| B03 | Export asks for the master password first (docs/14 says re-auth; the task prompt did not) | Accepted |
| B03 | "View recovery key" cannot show it (never stored); offers regeneration | Accepted |
| B03 | If the clipboard fingerprint is null after our write, the manager clears unconditionally | Accepted; Windows now supplies the sequence number |
| B03 | `windowMinimized`/`windowRestored` events exist; lock-on-minimise had no settings UI | **Resolved** in code (`lockOnMinimize`, docs/14 section 7.2, API v5); needs a manual check on Windows |
| B03 | docs/07 lists an "immediately" auto-lock; docs/14 allows 1-60 min only | Spec change proposed: drop "immediately" from docs/07 §7 |
| B03 | Import warnings are shown as written, assumed secret-free | **Tested** (`import_reports_never_carry_file_content`: hostile Bitwarden JSON and CSV with canaries in names, values, folder names and broken rows; no report or error contains them) |
| B03 | Whether generator calls work while locked is unspecified (fake allows it) | Spec change proposed (docs/14 §4.3, §7.4): allowed while locked |
| L01 | Hello failures all show one message | **Resolved** in code (docs/14 section 7.1, `API_VERSION` 3, PR #60): cancel is silent, invalidated says quick unlock was turned off. Still only tested with the fake provider; two-maintainer review pending |
| L01 | Print on Windows | Decided against: the spooler would keep a copy of the recovery key; the button is hidden (PR for `PrintService.isAvailable`) |
| L01 | Default vault location is `%LOCALAPPDATA%\AryaVault\vault`; File History and OneDrive are not touched by the app | Documented (`dir_security.rs`) |
| L01 | Rust crate licence text and artwork are placeholders; MSIX tiles are generic | Open |
| L02 | docs/05 section 5.1 open questions: debug assertion on every write for `item.visible`; should search and trash use it | Open |
| L02 | Keyset cursor for the list (deep offsets) | **Built** (docs/14 section 4.2.1, API v4); search still pages by offset |
| M1 carry-over | KDBX import, 1 h fuzz runs in CI, coverage gate | Unchanged from `m1-exit-check.md` unless noted there |

---

## 7. Release gate for `v0.1.0-alpha.1` (L02 part 3)

| Gate | State |
|---|---|
| Exit check has no unaccepted "not verified" MUST item | **No.** SEC-A02/A03/A04/S02/H01/H02/C06 are partial; the owner must accept or close each. |
| Usability blockers fixed | **Cannot say**: the test has not run. |
| Unsigned MSIX/zip attached to a prerelease with checksums | **Zip only**: the MSIX cannot be installed unsigned, so it should not be offered (this edits the L02 gate text). |

My reading: a clearly labelled **technical preview** (zip, checksums, "not audited, unsigned, back up your data") is defensible. A public "alpha" that implies readiness is not, until items 1, 2 and 5 are closed.
