# 03 — Threat Model

Method: asset → adversary → attack surface → mitigation, with STRIDE categories. Review at every minor release and whenever the sync or crypto spec changes.

## 1. Assets
| ID | Asset | Sensitivity |
|---|---|---|
| A1 | Item contents (passwords, notes, TOTP seeds, cards) | Critical |
| A2 | Master password | Critical |
| A3 | Recovery key | Critical |
| A4 | Vault key (VK) and derived subkeys | Critical |
| A5 | Item metadata (titles, URLs, counts, timestamps) | High |
| A6 | Cloud OAuth tokens | Medium (grants access only to ciphertext) |
| A7 | Vault integrity / availability (no silent loss) | High |

## 2. Adversaries
| ID | Adversary | Capability |
|---|---|---|
| T1 | **Cloud provider / account attacker** | Reads, modifies, deletes, rolls back, replays any file in the sync location |
| T2 | **Network attacker** | MITM on provider traffic |
| T3 | **Thief with powered-off device** | Full disk image |
| T4 | **Thief with locked device** | Physical access, tries unlock/biometric bypass, cold boot, memory scraping while locked |
| T5 | **Casual local snooper** | Shoulder surfing, screen share, clipboard history, screenshots |
| T6 | **Malicious app on same device (non-root)** | Clipboard sniffing, accessibility abuse, screen recording |
| T7 | **Malicious cloud-folder peer / compromised second device** | Can write valid-looking files into the sync location |
| T8 | **Supply-chain attacker** | Malicious dependency, build tool, or update |
| T9 | **Offline brute-forcer** | Has stolen the ciphertext (from cloud or disk), attacks the master password |
| T10 | **Malware with user privileges / root / keylogger** | Out of scope (see §5) |

## 3. Trust boundaries
```
[User] — [App process + Rust core] — [OS keystore] — [Disk] — (boundary) — [Network] — [Cloud provider]
```
Everything beyond the app process/OS keystore is **untrusted for confidentiality and integrity**. Cloud data is treated as attacker-controlled input.

## 4. Threats and mitigations

| # | Threat (STRIDE) | Adversary | Mitigation | Residual risk |
|---|---|---|---|---|
| 1 | Read vault from cloud (I) | T1 | XChaCha20-Poly1305 with VK; all names opaque IDs; padded sizes | File count / sizes / timing leak activity pattern |
| 2 | Tamper with segment (T) | T1,T7 | AEAD with AAD binding (vault_id, device_id, seq, epoch, prev_hash); reject on failure | None for content |
| 3 | Delete / truncate segments (D) | T1 | Per-device hash chain; other devices' manifests ack last seen seq → gap/rollback detected, user warned; snapshots + local copy | Cannot prevent deletion, only detect; local DB remains |
| 4 | Roll back to older state / replay (T) | T1 | Monotonic seq + manifest acks; reject lower-seq re-uploads; device remembers highest seen | Brand-new device cannot detect rollback of the entire vault |
| 5 | Downgrade KDF params in header (T) | T1 | Params are AAD of wrapped VK; client enforces floors **and ceilings** (anti-DoS) before running the KDF; new params only via user action | Attacker who has the header can brute-force at *original* params anyway |
| 6 | Brute-force master password (I) | T9 | Argon2id (≥64 MiB, calibrated), password-strength meter, minimum length, zxcvbn-style check, **recovery key is high entropy (≥128 bit)** | Weak chosen password remains the weakest link |
| 7 | MITM provider traffic (T/I) | T2 | TLS via provider SDK/OS stack; payload E2E encrypted anyway | Availability only |
| 8 | Disk image of powered-off device (I) | T3 | SQLCipher DB; keystore-wrapped biometric key; full-disk encryption recommended in onboarding | Unencrypted disk + weak password |
| 9 | Biometric bypass on locked device (S/E) | T4 | VK released only by hardware-bound key requiring biometric (`biometryCurrentSet` / Keystore auth-bound); master password required after N failures / reboot / 72h | OS biometric flaws |
| 10 | Memory scraping (I) | T4,T6 | Zeroize keys on lock; `mlock`/`VirtualLock` best-effort; no secrets in logs/crash dumps; disable core dumps | Unlocked-session memory is readable by privileged attacker |
| 11 | Clipboard sniffing / history (I) | T5,T6 | Clear after 30 s (configurable); mark sensitive (Android `EXTRA_IS_SENSITIVE`, Windows history exclusion, macOS concealed type, iOS expiry + local-only) | Apps with clipboard permission may read within the window |
| 12 | Screenshots / screen share (I) | T5,T6 | `FLAG_SECURE`, `WDA_EXCLUDEFROMCAPTURE`, macOS `sharingType`, app-switcher blur | Camera pointed at screen |
| 13 | Shoulder surfing (I) | T5 | Passwords masked by default; reveal needs tap; reveal auto-hides | — |
| 14 | Malicious import file (T/E) | T8,T7 | Rust parsers, fuzzed, size and depth limits | Parser bugs |
| 15 | Malicious dependency / build (T) | T8 | Pinned lockfiles, `cargo-vet`/`cargo-deny`, `cargo audit`, dependabot, minimal deps, CI provenance, reproducible builds goal, signed releases | Residual supply-chain risk |
| 16 | Phishing OAuth consent / token theft (S) | T1 | `drive.appdata` scope only; tokens in OS keystore; PKCE; user can revoke at Google | Tokens grant only ciphertext access |
| 17 | Forged device joins sync (S) | T7 | Any writer needs VK (AEAD). A peer *with* VK is a fully trusted device. Device list visible + "revoke → rotate VK" action | Revocation requires key rotation (cost: re-encrypt snapshot) |
| 18 | Lost master password + recovery key (D) | user | Mandatory recovery key flow with confirmation; reminder prompts; exportable emergency sheet | By design unrecoverable |
| 19 | Silent data loss through bad merge (T) | bug | Field-level LWW + item history + conflict copies for notes; property tests; never hard-delete for 30 days | — |
| 20 | Clock skew corrupting ordering (T) | device | HLC; reject remote HLC > now + 24 h and flag; tiebreak by device_id | Wrong clock on one device may win LWW once; history preserves loser |
| 21 | Autofill to wrong site / phishing (S) | T6 | Match on registrable domain / app package + signature; no fuzzy match; prompt on mismatch | — |
| 22 | Update tampering (T) | T8 | Store-signed mobile; code-signed + notarized desktop installers; signature verification on any in-app update | — |

## 5. Explicitly out of scope
- Malware with root / admin / kernel privileges or a keylogger on the user's device while the vault is unlocked (T10).
- Hardware attacks on secure enclaves / TPMs.
- Coercion ("rubber-hose") and legal compulsion.
- Side-channels beyond constant-time operations provided by vetted libraries.
- Compromise of the user's OS vendor.

## 6. Security invariants (must always hold)
- I1: Plaintext secrets never touch disk outside SQLCipher and never leave the process except by explicit user action.
- I2: Nothing is uploaded unless AEAD-encrypted under a key derived from VK.
- I3: Keys are zeroized on lock and never logged.
- I4: Every external byte (cloud file, import file, IPC message) is parsed by bounded, fuzzed Rust code and authenticated before use.
- I5: No network traffic other than to the user-selected provider (and optional update check, off by default).

## 7. Review checklist per release
- [ ] New data leaving device? Is it encrypted and padded?
- [ ] New parser? Fuzz target added?
- [ ] New permission / entitlement? Justified in docs?
- [ ] New dependency? Reviewed and pinned?
- [ ] Crypto spec diff reviewed by two maintainers?
