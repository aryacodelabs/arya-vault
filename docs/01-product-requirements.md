# 01 — Product Requirements

## 1. Vision
A password and notes manager a person can trust because it has no server to breach, its code is public, and its crypto follows published designs. Data is yours, stored on your devices, optionally replicated to a cloud account you already own.

## 2. Goals
- G1: Store logins, secure notes, cards, identities and TOTP secrets.
- G2: Generate strong passwords and passphrases.
- G3: Work 100% offline on all five platforms.
- G4: Optional encrypted backup and multi-device sync via Google Drive, iCloud (Apple devices) or a user-chosen folder.
- G5: Native-feeling apps (not a web app, not a browser wrapper).
- G6: Zero-knowledge: no party except the user's own devices can read the vault.

## 3. Non-goals (v1)
- Hosted server, accounts, or web vault.
- Team / family sharing and organizations.
- Passkey storage (planned v1.x, see roadmap).
- File attachments (planned v1.x).
- Emergency access by a third party.
- Telemetry (none by default; crash reports are opt-in only).

## 4. Target platforms
| Platform | Priority | Min version (proposed) |
|---|---|---|
| Windows | P0 (first) | Windows 10 22H2 / 11 |
| Android | P0 (first) | Android 8.0 (API 26); autofill APIs 8.0+, Credential Manager 14+ |
| macOS | P1 | macOS 12 |
| iOS | P1 | iOS 16 |
| Linux | P2 | glibc distros with libsecret; Flatpak build |

## 5. Personas
- **Privacy-conscious individual:** does not want a third-party server holding their vault.
- **Multi-device user:** Windows laptop + Android phone, or Mac + iPhone.
- **Developer / power user:** wants import/export, CLI later, auditable source.

## 6. User stories (MVP unless marked)
| ID | Story |
|---|---|
| US-01 | As a user I create a vault with a master password and receive a recovery key. |
| US-02 | I unlock with master password; on mobile/desktop I may enable biometrics / OS unlock. |
| US-03 | I add, edit, delete, favorite and tag logins (title, username, password, URL(s), notes, custom fields). |
| US-04 | I write secure notes in Markdown; they are searchable. |
| US-05 | I generate passwords (length, classes, exclude ambiguous) and passphrases (word count, separator, capitalization). |
| US-06 | I see a strength estimate, plus reused / weak password reports. |
| US-07 | I copy a field; the clipboard clears automatically. |
| US-08 | I search across all items instantly, offline. |
| US-09 | I import from CSV, Bitwarden JSON, KeePass (KDBX) and export to encrypted JSON and CSV (with warning). |
| US-10 | I enable cloud sync with Google Drive (all platforms) or iCloud (Apple devices) or a folder. |
| US-11 | I add a second device by installing the app, choosing the same provider and entering my master password. |
| US-12 | I restore from backup / snapshot, including after losing every device (needs master password or recovery key). |
| US-13 | I change my master password without re-encrypting everything. |
| US-14 | I view and restore previous versions of an item. |
| US-15 | The app auto-locks on timeout, sleep, screen lock, or app-switch (configurable). |
| US-16 (P1) | Autofill on Android / iOS. |
| US-17 (P1) | TOTP codes generated in-app. |
| US-18 (P2) | Browser extension on desktop via native messaging. |

## 7. Feature scope by release
| Release | Contents |
|---|---|
| **0.1 alpha** | Windows + Android, local vault, generator, import/export |
| **0.2 beta** | Folder sync + Google Drive sync, history, recovery flows |
| **0.3 beta** | macOS, iOS, iCloud (CloudKit), biometrics everywhere |
| **0.4 beta** | Linux, autofill (Android, iOS), TOTP |
| **1.0** | External audit complete, signed installers, store releases, browser extension optional |

## 8. Quality attributes
| Attribute | Target |
|---|---|
| Unlock time | < 1.5 s on mid-range phone (Argon2 calibrated) |
| Search | < 100 ms on 5,000 items |
| Vault size | Up to 20,000 items without UI jank |
| Cold start | < 2 s desktop, < 3 s mobile |
| Offline | Every feature except sync works offline |
| Accessibility | Screen-reader labels, dynamic text, contrast AA |
| Localization | English first; i18n-ready from day one |

## 9. Constraints and honest limitations
- **No password reset exists.** Lose master password and recovery key → data is unrecoverable.
- **iCloud sync works only among Apple devices** (no iCloud API for Windows/Linux/Android). Google Drive or a shared folder is the cross-ecosystem option.
- Cloud sync is eventually consistent; changes appear after the next poll / change notification, not instantly.
- A device compromised by malware (keylogger, root) is out of scope for protection (see threat model).

## 10. Success metrics (open-source project)
- Zero known critical vulnerabilities at 1.0; external audit report published.
- Sync convergence tests: 100% pass across the simulator matrix.
- Data-loss bugs: zero tolerated; each is a release blocker.
