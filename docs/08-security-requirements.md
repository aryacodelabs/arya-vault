# 08 — Security Requirements

Each requirement is numbered, testable, and traced to a threat (doc 03) and a verification method. **MUST** = release blocker.

Verification keys: **UT** unit test · **PT** property test · **FZ** fuzz · **IT** integration test · **MT** manual platform test · **RV** code/spec review · **AU** external audit.

## 1. Cryptography
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-C01 | MUST use only primitives listed in doc 04; no custom crypto | all | RV, CI lint (`cargo-deny` allowlist) |
| SEC-C02 | MUST derive MK with Argon2id ≥ 64 MiB, t ≥ 3; client refuses weaker params | 5,6 | UT |
| SEC-C03 | MUST generate all keys/nonces/salts from OS CSPRNG | 6 | RV, UT (source check) |
| SEC-C04 | MUST use unique random 192-bit nonce per AEAD message | 2 | UT, PT |
| SEC-C05 | MUST bind vault_id, epoch, device_id, seq, kind in AAD | 2,4 | UT |
| SEC-C06 | MUST zeroize keys on lock and on drop | 10 | UT, MT (memory inspection) |
| SEC-C07 | MUST pass published test vectors (RFC 9106, 8439/xchacha, 5869) | 6 | UT |
| SEC-C08 | MUST open all golden vault files from previous releases | 19 | IT (CI) |
| SEC-C09 | Password generation MUST be unbiased (rejection sampling) | 6 | PT (chi-square), UT |
| SEC-C10 | NFKD normalization MUST be identical on all platforms | 6 | UT vectors |

## 2. Authentication, unlock, recovery
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-A01 | Recovery key MUST be ≥128-bit entropy, shown only after master password creation, and verified by re-entry before onboarding completes | 18 | IT, MT |
| SEC-A02 | Biometric unlock MUST use hardware-backed, non-exportable, biometric-bound keys; invalidated on enrollment change | 9 | MT per platform |
| SEC-A03 | Master password MUST be required after reboot (default), 72 h (default), or 5 failed biometrics | 9 | IT |
| SEC-A04 | Auto-lock MUST lock on timeout, screen lock, sleep; lock MUST wipe keys & close DB | 4,10 | IT, MT |
| SEC-A05 | Changing master password MUST NOT require re-encrypting data; old wrap superseded | 6 | UT |
| SEC-A06 | VK rotation MUST be available and tested | 17 | IT |
| SEC-A07 | Minimum master password length 12; strength check enforced | 6 | UT |

## 3. Storage
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-S01 | Vault DB MUST be SQLCipher-encrypted with key derived from VK | 8 | UT, MT (inspect file) |
| SEC-S02 | No plaintext secrets in files, logs, crash reports, temp files, or OS backups | 8,10 | MT (grep disk), CI log scan |
| SEC-S03 | OAuth refresh tokens MUST be in OS keystore, never in DB or prefs | 16 | MT |
| SEC-S04 | Android: `allowBackup=false`, no `android:debuggable`; iOS: exclude from iCloud/iTunes backup | 8 | MT |
| SEC-S05 | Search index MUST live only inside the encrypted DB | 8 | RV |
| SEC-S06 | Atomic writes (SQLite WAL / temp+rename); crash-safety tested with fault injection | 19 | IT |

## 4. Sync
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-Y01 | Nothing uploaded unless AEAD-encrypted | 1 | IT (fake provider asserts ciphertext only) |
| SEC-Y02 | Remote file names/sizes MUST NOT reveal item content; padding ≥1 KiB buckets | 1 | UT |
| SEC-Y03 | Hash chain & manifest verification MUST detect truncation/rollback and warn; MUST NOT auto-overwrite local state | 3,4 | IT (adversarial provider) |
| SEC-Y04 | Merge MUST be commutative/associative/idempotent | 19 | PT |
| SEC-Y05 | All remote bytes parsed by bounded parsers; fuzzed | 14 | FZ |
| SEC-Y06 | Drive scope MUST be `drive.appdata` only | 16 | RV |
| SEC-Y07 | HLC skew guard flags remote clocks > 24 h ahead | 20 | UT |
| SEC-Y08 | Provider conformance suite MUST pass for every provider | 19 | IT |
| SEC-Y09 | TLS cert validation never disabled; no custom trust stores | 7 | RV |

## 5. Client hygiene
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-H01 | Clipboard cleared after ≤30 s default; sensitive flag set on every platform that supports it | 11 | MT |
| SEC-H02 | Screen capture blocked for vault screens (Android FLAG_SECURE, Windows exclude-from-capture, macOS sharingType none, iOS blur) | 12 | MT |
| SEC-H03 | Secrets masked by default; reveal auto-hides ≤15 s | 13 | IT |
| SEC-H04 | No analytics SDKs; no network except chosen provider; update check opt-in | I5 | RV, network capture test |
| SEC-H05 | Crash reporting opt-in, scrubs all vault data | 10 | RV, UT |
| SEC-H06 | Core dumps disabled / secrets excluded where OS permits | 10 | MT |
| SEC-H07 | Autofill matches registrable domain / signed package ID only | 21 | IT |
| SEC-H08 | iOS AutoFill extension MUST NOT run Argon2 (memory); uses keystore-held VK | — | RV, MT |

## 6. Supply chain & release
| ID | Requirement | Threat | Verify |
|---|---|---|---|
| SEC-R01 | Lockfiles committed; `cargo audit`, `cargo-deny`, `cargo-vet` (or equivalent) in CI | 15 | CI |
| SEC-R02 | Dart/Flutter deps pinned; reviewed on update; minimal count | 15 | CI, RV |
| SEC-R03 | Release artifacts signed (Authenticode, Apple notarization, Android signing, GPG/minisign checksums) | 22 | MT |
| SEC-R04 | CI builds from tagged commits on ephemeral runners; SLSA-style provenance; goal: reproducible Rust core builds | 15 | CI |
| SEC-R05 | Two-person review for `crypto`, `sync`, and `ffi` crates (CODEOWNERS) | 15 | repo settings |
| SEC-R06 | Public vulnerability-disclosure process (SECURITY.md) | — | RV |
| SEC-R07 | External audit completed and published before 1.0; critical/high findings fixed | all | AU |

## 7. Privacy
| ID | Requirement |
|---|---|
| SEC-P01 | No user identifiers collected; no accounts. |
| SEC-P02 | Store privacy labels declare "Data not collected". |
| SEC-P03 | Diagnostics export is manual, scrubbed, user-reviewed. |
