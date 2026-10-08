# 04 — Cryptography Specification (v1 draft)

**Rule: no custom primitives, no custom protocols.** Everything below composes standard, audited building blocks. Any change to this document requires review by two maintainers and a version bump (see CONTRIBUTING).

## 1. Primitives
| Purpose | Primitive | Reference | Rust crate (candidate) |
|---|---|---|---|
| Password hashing | Argon2id v1.3 | RFC 9106 | `argon2` (RustCrypto) |
| AEAD | XChaCha20-Poly1305 (192-bit nonce) | draft-irtf-cfrg-xchacha, RFC 8439 | `chacha20poly1305` |
| KDF (key expansion) | HKDF-SHA-256 | RFC 5869 | `hkdf`, `sha2` |
| Hash (chain, IDs) | SHA-256 | FIPS 180-4 | `sha2` |
| RNG | OS CSPRNG | — | `getrandom` / `rand::rngs::OsRng` |
| Zeroization | `zeroize`, `secrecy` | — | `zeroize`, `secrecy` |
| Local DB encryption | SQLCipher 4 (AES-256-CBC + HMAC-SHA512, raw key) | SQLCipher docs | `rusqlite` (bundled-sqlcipher) |
| Constant-time compare | `subtle` | — | `subtle` |

AES-256-GCM is an acceptable fallback for hardware-accelerated paths but is **not** used in v1, to keep one AEAD.

## 2. Key hierarchy

```
Master password (user)
   │  NFKD-normalize, UTF-8
   ▼  Argon2id(salt, m, t, p) → 32 bytes
Master Key (MK)
   │  HKDF-SHA256(salt=vault_id, info="aryavault/kek-pw/v1")
   ▼
KEK_pw ──unwrap──┐
                 ▼
Recovery key (RK, 160-bit random)        Vault Key (VK, 256-bit random)  ◄── generated once at vault creation
   │  HKDF-SHA256(salt=vault_id, info="aryavault/kek-rk/v1")      │
   ▼                                                               │ HKDF-SHA256(salt=vault_id, info=label‖epoch)
KEK_rk ──unwrap──► VK                                              ▼
                                                  ┌── K_db        ("db/v1")        → SQLCipher raw key
                                                  ├── K_log       ("log/v1")       → op-log segments
                                                  ├── K_snap      ("snapshot/v1")  → snapshots
                                                  ├── K_manifest  ("manifest/v1")  → device manifests
                                                  └── K_item_hist ("history/v1")   → reserved
```

- VK is random, **not** derived from the password. Changing the master password re-wraps VK; no data re-encryption.
- Two independent wraps of the same VK exist in the header: one under `KEK_pw`, one under `KEK_rk`.
- RK has ≥128 bits of entropy, so a single HKDF (no Argon2) is sufficient.

## 3. Master password handling
1. Normalize with Unicode NFKD, encode UTF-8. (Same normalization on all platforms; test vectors for accented / CJK / emoji passwords.)
2. Minimum length 12 characters (soft-warn below 16); strength estimator rejects very weak passwords; passphrases encouraged.
3. Argon2id parameters stored per vault in the header:

| Profile | m (MiB) | t | p | Notes |
|---|---|---|---|---|
| Default floor | 64 | 3 | 1 | Never go below; client refuses lower values |
| Calibrated | ≥64 | ≥3 | 1–4 | At vault creation, calibrate on the device so unlock ≈ 0.5–1.0 s; cap m at 256 MiB for low-RAM devices |
| Salt | 16 bytes random | | | |

Other devices must be able to compute the parameters: the unlock memory requirement is checked against device RAM, and the user is warned if a low-end device cannot comply (never silently reduce).

## 4. Recovery key
- 160 random bits → 32 characters, Crockford Base32, grouped `XXXXX-XXXXX-XXXXX-...` with a 2-char checksum group to catch typos.
- Shown once at creation; user must **re-enter** it (or specific groups) to complete onboarding. Offered as printable sheet, PDF and QR. Never stored in the cloud in plaintext; never stored on device (only the wrapped-VK copy derived from it).
- Regenerating the recovery key: re-wrap VK under a new RK, increment `header_version`, invalidate the old wrap.

## 5. Vault header (plaintext, integrity-bound)
Stored as `header-<n>.bin` (CBOR) in the sync location and as a row in local DB.

```
Header {
  format_version: u16,
  vault_id: 16 bytes (random UUID),
  header_version: u32,           // monotonically increasing
  epoch: u32,                    // key epoch (rotation counter)
  kdf: { alg:"argon2id", version:0x13, m_kib:u32, t:u32, p:u32, salt:16B },
  wrap_pw: { nonce:24B, ct:48B },   // AEAD(KEK_pw, VK)
  wrap_rk: { nonce:24B, ct:48B },   // AEAD(KEK_rk, VK)
  created_at: u64
}
```
**Wrap AEAD:** key = KEK, nonce = random 24 B, plaintext = VK, **AAD = "aryavault/wrap/v1" ‖ vault_id ‖ epoch ‖ header_version ‖ canonical_cbor(kdf)**. Tampering with KDF params or ids makes unwrap fail.

## 6. Encrypted container formats
All containers share an envelope:

```
Envelope {
  magic: "AVLT", format_version: u16, kind: u8 (1=segment, 2=snapshot, 3=manifest),
  vault_id, epoch: u32, device_id: 16B, seq: u64 (segments/manifests), prev_hash: 32B (segments),
  nonce: 24B random,
  ciphertext, tag
}
AAD = canonical_cbor(all fields except nonce/ciphertext/tag)
```
- Nonce: random 192-bit per message (birthday bound safe for lifetime volumes; never counter-based across devices).
- Plaintext is **padded** to a multiple of 1 KiB (ISO/IEC 7816-4 style `0x80 00…`) before encryption to limit size leakage.
- **No compression** in v1 (avoids length-leak classes; revisit with analysis).
- File names in the cloud are opaque: `<device_id-hex>/<seq-hex>.seg`; no item names, no titles.

## 7. Integrity chain
- `prev_hash` = SHA-256 of the previous segment *envelope bytes* from the same device (zero for seq 1).
- Manifest (per device, encrypted with `K_manifest`): `{ own_head: (seq, hash), seen: { device_id → (seq, hash) } }`.
- A reader verifies: AEAD OK → `prev_hash` matches stored hash of seq-1 → seq strictly increasing → peers' manifests do not claim a higher seq than found (otherwise: **rollback / deletion warning**).

## 8. Biometric / quick unlock
- On enable, VK is encrypted under a **hardware-backed, non-exportable key** requiring user presence: iOS/macOS Keychain `SecAccessControl` with `.biometryCurrentSet`; Android Keystore key with `setUserAuthenticationRequired(true)`, `setInvalidatedByBiometricEnrollment(true)`, StrongBox if available; Windows Hello-gated DPAPI/TPM key; Linux: Secret Service (no biometrics, optional PIN).
- Master password required: after reboot (policy configurable), after 5 failed biometrics, after 72 h (default), or after biometrics enrollment change.
- An optional **app PIN** is only a convenience gate on top of keystore, never a replacement for master password.

## 9. Master-password change and recovery
- **Change password:** unwrap VK (old pw) → derive new KEK_pw with new salt/params → new `wrap_pw` → publish `header-<n+1>`. Other devices detect the new header version and re-prompt on next unlock.
- **Reset via recovery key:** unwrap VK via RK → user sets new master password → new wrap_pw; optionally regenerate RK.
- Old header versions are retained for 30 days locally to survive races, then purged (the old wrap_pw remains a brute-force target for anyone who copied it; this is inherent and noted for users: changing password after a suspected leak should be accompanied by **VK rotation**).

## 10. Vault key rotation (epoch bump)
Triggered by: suspected compromise, device revocation, user request.
1. Generate VK'. Increment `epoch`.
2. Take snapshot of current state, encrypt under `K_snap(VK', epoch+1)`.
3. Publish new header with `wrap_pw'/wrap_rk'`, new snapshot, and mark old epoch segments obsolete.
4. Other devices on next sync see higher epoch, prompt for master password, adopt snapshot.
5. Old-epoch cloud files are deleted after all known devices acknowledge (or after 30 days).

Note: rotation cannot protect data an attacker already decrypted. It protects future data.

## 11. Memory and process hygiene
- Keys held in `Zeroizing<[u8; 32]>`; secret strings in `secrecy::SecretString`.
- Dart side receives secrets only upon explicit reveal/copy; zero buffers after use (`Uint8List.fillRange`).
- Disable core dumps (Linux `prctl(PR_SET_DUMPABLE,0)`, Windows WER exclusion) and exclude app data dir from OS backups where possible (Android `allowBackup=false`).
- `mlock`/`VirtualLock` best-effort for key pages.

## 12. Password generator
- Source: OS CSPRNG; uniform selection by **rejection sampling** (no modulo bias).
- Character classes: lower, upper, digits, symbols (configurable subset), exclude ambiguous (`Il1O0`).
- Guarantee at least one char from each enabled class by constrained placement then Fisher–Yates shuffle (CSPRNG).
- Passphrases: EFF large wordlist (7776 words, ≈12.9 bits/word); default 6 words; separator and capitalization options.
- Show entropy estimate in bits.

## 13. Test vectors and verification
- Argon2id: RFC 9106 §5.3 vectors + project-specific vectors for NFKD cases.
- XChaCha20-Poly1305: draft-irtf-cfrg-xchacha vectors.
- HKDF: RFC 5869 vectors.
- **Golden files:** `core/testdata/` contains a sample vault header + segments + snapshot that every release must open; format changes must keep reading old versions.
- A second, independent implementation (Python script in `tools/`) decrypts golden files to catch spec/implementation drift.

## 14. Versioning and agility
- `format_version` in every container. Readers support all prior versions; writers upgrade on explicit user action when needed.
- Algorithm identifiers are stored (`alg` fields) to permit future migration (e.g., Argon2 param raises, post-quantum KEM wrap added later) without ambiguity.

## 15. Open questions
- Whether to add an optional "Secret Key" (1Password-style, device-held extra entropy) in v2.
- Whether to add per-device Ed25519 signatures for attribution (requires revocation story).
- Padding bucket sizes vs storage overhead.
