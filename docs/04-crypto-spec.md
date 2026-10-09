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

**SQLCipher pinning (review L4):** defaults differ across SQLCipher versions/platforms, so the core MUST set `cipher_page_size`, `cipher_compatibility` (4), KDF/HMAC algorithms and `cipher_plaintext_header_size = 0` explicitly, and record the values in the DB `meta` table. Opening with unexpected settings is an error, not a fallback.

**AEAD key commitment (review L5):** XChaCha20-Poly1305 is not key-committing. This is acceptable because there is no online decryption oracle (no server), so partitioning-oracle attacks do not apply. Revisit if any online component is ever added.

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
1. Normalize with Unicode NFKD, encode UTF-8. (Same normalization on all platforms; test vectors for accented / CJK / emoji passwords.) The normalization crate and its Unicode version are **pinned**; a vector test fails CI if a dependency update changes any output, because a change would lock users out of correct passwords (review L3).
2. Minimum length 12 characters (soft-warn below 16); strength estimator rejects very weak passwords; passphrases encouraged.
3. Argon2id parameters stored per vault in the header:

| Profile | m (MiB) | t | p | Notes |
|---|---|---|---|---|
| Default floor | 64 | 3 | 1 | Never go below; client refuses lower values |
| Calibrated | ≥64 | ≥3 | 1–4 | At vault creation, calibrate on the device so unlock ≈ 0.5–1.0 s; cap m at 256 MiB for low-RAM devices |
| Salt | 16 bytes random | | | |

Other devices must be able to compute the parameters: the unlock memory requirement is checked against device RAM, and the user is warned if a low-end device cannot comply (never silently reduce).

**Parameter bounds (review H4).** The header is unauthenticated until after the KDF has run, so a hostile provider could set absurd costs to cause out-of-memory or a hang. Clients MUST enforce both floors and ceilings **before** running Argon2:

| Parameter | Floor | Ceiling |
|---|---|---|
| m (MiB) | 64 | 1024 |
| t | 3 | 10 |
| p | 1 | 8 |
| salt | 16 B exactly | |

Values outside the bounds are rejected with an explicit error ("vault parameters are out of range or corrupted"). If the device cannot allocate `m`, the user is told to unlock on a more capable device; the app never silently lowers cost.

## 4. Recovery key
- 160 random bits → 32 characters, Crockford Base32, grouped `XXXXX-XXXXX-XXXXX-...` with a 2-char checksum group to catch typos.
- **Checksum:** the 2-char checksum group encodes the first 10 bits of `SHA-256("aryavault/rk-check/v1" ‖ rk_bytes)` (the 20 raw key bytes) as two Crockford Base32 characters (5 bits each, most significant first). Display layout: six groups of 5 key characters, one group of the remaining 2 key characters, then the 2-character checksum group (`XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XX-CC`); hyphens carry no information. The parser accepts upper/lower case, spaces and hyphens, maps the Crockford substitutions (`I`/`L` → `1`, `O` → `0`), and rejects a mismatching checksum with a typed error before any key derivation.
- Shown once at creation; user must **re-enter** it (or specific groups) to complete onboarding. Offered as printable sheet, PDF and QR. Never stored in the cloud in plaintext; never stored on device (only the wrapped-VK copy derived from it).
- Regenerating the recovery key: re-wrap VK under a new RK, increment `header_version`, publish the new header and delete older headers. **Honest limitation (review M3):** this does not by itself revoke the old RK against anyone who already holds a copy of an old header from the cloud; only VK rotation (§10) does. The UI offers "Regenerate and rotate keys" when compromise is suspected.

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
**Wrap AEAD:** key = KEK, nonce = random 24 B, plaintext = VK, with **separate AADs per wrap** (review H1):

- `wrap_pw` AAD = `"aryavault/wrap-pw/v1"` ‖ vault_id ‖ epoch ‖ canonical_cbor(kdf)
- `wrap_rk` AAD = `"aryavault/wrap-rk/v1"` ‖ vault_id ‖ epoch

`header_version` is deliberately **not** part of any AAD. Otherwise a password change (which bumps `header_version`) would invalidate `wrap_rk`, which can only be re-created with the recovery key the user does not have at that moment. Tampering with KDF params or ids still makes unwrap fail.

**Header selection (reviews M4, M7).** When several headers exist, the active one is the highest `(epoch, header_version, device_id)` that is well-formed. Headers with `epoch` lower than the highest epoch this device has ever seen are rejected. A newer header is only *adopted* after it unwraps successfully with the user's credentials; an unverifiable newer header raises a warning instead of replacing the current one. Headers older than the active one are deleted promptly once all known devices have acknowledged the new header (they remain brute-force targets otherwise).

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
- **Path binding (review M1):** the envelope's `device_id` and `seq` MUST equal the values in the file's path. Mismatches are rejected and quarantined. (Otherwise a provider could copy a valid segment into another device's directory and confuse its hash chain.)

## 7. Integrity chain
- `prev_hash` = SHA-256 of the previous segment *envelope bytes* from the same device (zero for seq 1).
- Manifest (per device, encrypted with `K_manifest`): `{ own_head: (seq, hash), seen: { device_id → (seq, hash) } }`.
- A reader verifies: AEAD OK → `prev_hash` matches stored hash of seq-1 → seq strictly increasing → peers' manifests do not claim a higher seq than found (otherwise: **rollback / deletion warning**).

## 8. Biometric / quick unlock
- On enable, VK is encrypted under a **hardware-backed, non-exportable key** requiring user presence: iOS/macOS Keychain `SecAccessControl` with `.biometryCurrentSet`; Android Keystore key with `setUserAuthenticationRequired(true)`, `setInvalidatedByBiometricEnrollment(true)`, StrongBox if available; Windows: `KeyCredentialManager` (Hello-bound TPM key; derive the wrapping key from a deterministic signature over a fixed per-vault challenge). `UserConsentVerifier` alone and plain DPAPI are **not** acceptable: they are UI gates or session-bound, so malware running as the user could skip them (review M2); Linux: Secret Service (no biometrics, optional PIN). Linux quick-unlock is **lower assurance** (anything that can read the unlocked login keyring can read the key); it is opt-in and off by default, and the UI says so.
- Master password required: after reboot (policy configurable), after 5 failed biometrics, after 72 h (default), or after biometrics enrollment change.
- An optional **app PIN** is only a convenience gate on top of keystore, never a replacement for master password.

## 9. Master-password change and recovery
- **Change password:** unwrap VK (old pw) → derive new KEK_pw with new salt/params → new `wrap_pw` → publish `header-<n+1>`. Other devices detect the new header version and re-prompt on next unlock.
- **Reset via recovery key:** unwrap VK via RK → user sets new master password → new wrap_pw; optionally regenerate RK.
- Old header versions are retained locally for 30 days to survive races, but **deleted from the cloud** as soon as every known device has acknowledged the newer header. Anyone who already copied an old header can still brute-force the old password against it, and an old password plus old header still yields the same VK. Changing the password does **not** revoke a compromised password; **VK rotation** does (review M3). The UI offers "Change password and rotate keys" as the recommended option after a suspected leak.

## 10. Vault key rotation (epoch bump)
Triggered by: suspected compromise, device revocation, user request.
1. Generate VK'. Increment `epoch`.
2. Take snapshot of current state, encrypt under `K_snap(VK', epoch+1)`.
3. Publish new header with `wrap_pw'/wrap_rk'`, new snapshot, and mark old epoch segments obsolete.
4. Other devices on next sync see higher epoch, prompt for master password, adopt snapshot.
5. Old-epoch cloud files are deleted after all known devices acknowledge (or after 30 days).

6. **Local DB re-key:** each device re-keys its SQLCipher file (`PRAGMA rekey` with the new `K_db`) after adopting the new epoch.
7. **Late and offline devices (review M4):** a device that wrote segments or holds unsent ops from the old epoch, which are not covered by the rotation snapshot's `covers` map, re-emits those ops under the new epoch once the user has entered the master password. Re-emitting is idempotent (ops are keyed by `(device_id, seq, index)`; see doc 06 §8.1).
8. **Rejected inputs:** headers and segments with an epoch lower than the highest known are ignored. A revoked device keeps the old VK and can still read old-epoch files, which is unavoidable, but it cannot read anything written under the new epoch.

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

## 16. Encoding details (v1, normative)
This section pins the byte-level choices that §2-§7 leave open, so an independent implementation (e.g. `tools/crosscheck`) can read every v1 file from this document alone. It documents what the v1 golden files (`core/testdata/golden/v1/`) already contain; any change is a `format_version` bump (§14).

**Integers.** Where an integer is concatenated into a hash/KDF/AAD input (`epoch` in the §2 sub-key `info`, in the §5 wrap AADs, and in the envelope fixed header), it is **big-endian, fixed width** (`epoch` u32 = 4 bytes, `seq` u64 = 8 bytes, `format_version` u16 = 2 bytes). `vault_id`, `device_id` and `prev_hash` are the raw bytes.

**Sub-key `info` (§2).** `info = ASCII(label) ‖ epoch_be32`, e.g. `"db/v1" ‖ 00 00 00 01`. Salt is `vault_id` (16 bytes), IKM is VK, output is 32 bytes. `KEK_pw` uses IKM = MK (32 B) and `KEK_rk` uses IKM = RK (the 20 raw bytes), each with `salt = vault_id` and the `info` strings in §2.

**Canonical CBOR (RFC 8949 §4.2.1).** Used for the header, the `kdf` struct in the `wrap_pw` AAD and the envelope AAD. Shortest-form integer and length heads, definite lengths only, no tags/floats; maps have unique keys sorted by the bytewise lexicographic order of their **encoded** keys. A reader MUST reject any non-canonical encoding, duplicate keys, nesting deeper than 16, trailing bytes, and lengths above its limits.

**`kdf` struct.** A CBOR map with text keys `alg` (`"argon2id"`), `version` (unsigned `19`), `m_kib`, `t`, `p` (unsigned), `salt` (byte string of 16). This exact canonical encoding is the `canonical_cbor(kdf)` in the `wrap_pw` AAD (§5). Hex example for `m_kib=65536, t=3, p=1, salt=ab×16`:
`a6 6170 01 6174 03 63616c67 686172676f6e326964 6473616c74 50 abab…ab 656d5f6b6962 1a00010000 6776657273696f6e 13`.

**Header file.** One canonical CBOR map with text keys `format_version` (unsigned), `vault_id` (bstr 16), `header_version` (u32), `epoch` (u32), `kdf` (map above), `wrap_pw` and `wrap_rk` (maps `{ "nonce": bstr 24, "ct": bstr 48 }`), `created_at` (unsigned, seconds since the Unix epoch). No other keys. Maximum size 2048 bytes. `device_id` is **not** in the body; it appears only in the file name (§16 file names), and the epoch and version in the name MUST equal the body.

**Envelope wire layout (v1, fixed width).**
```
offset  size  field
     0     4  magic = "AVLT"
     4     2  format_version (u16, = 1)
     6     1  kind (1 = segment, 2 = snapshot, 3 = manifest)
     7    16  vault_id
    23     4  epoch
    27    16  device_id
    43     8  seq   (segment seq / manifest counter; 0 for snapshots)
    51    32  prev_hash (SHA-256 of the previous segment envelope bytes; zero otherwise)
    83    24  nonce
   107     4  ct_len (u32)
   111 ct_len  ciphertext ‖ 16-byte tag
```
The total file length is exactly `111 + ct_len`. `format_version` is checked immediately after the magic, before any other validation, so a newer format is reported as "unsupported" rather than as a malformed file. Per-kind rules: segments have `seq >= 1` and a zero `prev_hash` when `seq = 1`; snapshots have `seq = 0` and a zero `prev_hash`; manifests have a zero `prev_hash` (their `seq` is the manifest counter). `ct_len - 16` is a non-zero multiple of 1024 and at most the padded maximum plaintext (segment 1 MiB, manifest 256 KiB, snapshot 64 MiB, before padding).

**Envelope AAD (§6).** The canonical CBOR map with text keys `magic` (bstr `"AVLT"`), `format_version` (unsigned), `kind` (unsigned 1-3), `vault_id` (bstr 16), `epoch` (unsigned), `device_id` (bstr 16), `seq` (unsigned), `prev_hash` (bstr 32). `prev_hash` is present for every kind (all zero where unused). The AEAD key is `K_log` for segments, `K_snap` for snapshots and `K_manifest` for manifests, each derived at the envelope's `epoch`.

**Padding (§6).** ISO/IEC 7816-4: append `0x80` then `0x00` bytes to the next multiple of 1024; there is always at least one padding byte, so a plaintext whose length is already a multiple of 1024 gains a full 1024-byte bucket. Unpadding strips trailing zeros, requires `0x80`, and requires that the padding is the minimal form (no extra buckets). It is performed only after authentication.

**File names (§6, docs/06 §3).** Relative to `<root>/AryaVault/<vault_id>/`, lowercase hex, fixed width, and nothing else is accepted:
`header-<epoch:8>-<header_version:8>-<device_id:32>.bin`; `devices/<device_id:32>/<seq:16>.seg`; `devices/<device_id:32>/manifest-<counter:16>.bin`; `snapshots/<hlc:16>-<device_id:32>.snap`. A segment's or manifest's envelope `device_id` and `seq` MUST equal the path's. For snapshots only `device_id` is bound (the file name's HLC is not repeated in the envelope).

**Recovery key text (§4).** 20 key bytes are written as 32 Crockford Base32 characters (alphabet `0123456789ABCDEFGHJKMNPQRSTVWXYZ`, most significant bits first, 5 bits per character), followed by 2 checksum characters, grouped as `XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XXXXX-XX-CC`.
