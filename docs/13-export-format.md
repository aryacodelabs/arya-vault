# 13 — AryaVault encrypted export format (v1)

> **Status: Draft for owner review.** The owner approves this format before the implementation merges (task T07). Once released, the format is frozen; changes need a new `format_version` and a new test vector, and readers keep supporting old versions (docs/04 §14).

This document is written so that a third party can read and write exports without the AryaVault source. It specifies the **password-protected export** (§1-§8) and, for completeness, the **plaintext CSV export** (§9). Importers for other products' files (CSV shapes, Bitwarden JSON) are described in docs/05 §9 and are not a format owned by AryaVault.

Primitives are exactly those of docs/04 §1 (Argon2id, HKDF-SHA-256, XChaCha20-Poly1305, SHA-256); there is no new construction. "Canonical CBOR" means RFC 8949 §4.2.1 deterministic encoding as pinned in docs/04 §16.

## 1. Purpose and threat model
A user exports their vault to move it to another app or keep an offline backup. The file may be stored anywhere (cloud drive, e-mail, USB stick), so an attacker is assumed to read, copy and modify it. Goals: confidentiality of all vault contents under the export password, detection of any modification, and bounded resource use for a reader parsing a hostile file. Non-goals: hiding that the file is an AryaVault export, hiding its approximate size (it is padded to 1 KiB steps), protection against someone who knows the password, and rollback protection (an older export is a valid export).

## 2. File layout
```
file = "AVEX" ‖ cbor-map            ; 4 ASCII bytes, then exactly one canonical CBOR value, no trailing bytes

cbor-map = {
  "format_version": 1,                      ; unsigned
  "kdf": {                                  ; the same struct as the vault header (docs/04 §5)
    "alg": "argon2id", "version": 19,
    "m_kib": u32, "t": u32, "p": u32,
    "salt": bstr(16)
  },
  "export_id": bstr(16),                    ; random, per export
  "nonce": bstr(24),                        ; random, per export
  "ct": bstr                                ; AEAD ciphertext ‖ 16-byte tag
}
```
Keys appear in canonical order (sorted by encoded key), so on the wire `ct`, `kdf`, `nonce`, `export_id`, `format_version` come in that order. A reader MUST reject a map with missing or extra keys.

**Reader checks before any cryptography**, in this order:
1. total size ≤ the reader's limit (default 64 MiB);
2. the 4-byte magic is `AVEX` (else "not an AryaVault export");
3. the CBOR decodes strictly (canonical form, no duplicate keys, depth ≤ 4, no trailing bytes);
4. `format_version` is understood. A larger (or unknown) version MUST be reported as "unsupported format version N, update required", never as corruption, and MUST NOT be parsed further;
5. `kdf.alg` is `argon2id`, `kdf.version` is 19, and the parameters are inside the bounds of docs/04 §3: **64 MiB ≤ m ≤ 1024 MiB, 3 ≤ t ≤ 10, 1 ≤ p ≤ 8, salt exactly 16 bytes**. Out-of-range values are rejected *before* Argon2 runs (a hostile file must not cause an out-of-memory condition or a hang);
6. `ct` has length `16 + 1024·k` for some `k ≥ 1` (a padded plaintext plus the tag).

A reader MAY expose the parameters (`m_kib`, `t`, `p`, ciphertext size) before asking for the password so a device that cannot afford the KDF can say so instead of failing later.

## 3. Key derivation
1. Normalise the export password: Unicode NFKD, encoded as UTF-8, **no trimming** (docs/04 §3).
2. `MK = Argon2id(password, salt = kdf.salt, m = m_kib KiB, t, p, version 0x13, output 32 bytes)`.
3. `KEK = HKDF-SHA256(salt = export_id, ikm = MK, info = "aryavault/kek-pw/v1", L = 32)`.

The AEAD key is `KEK`. (Step 3 reuses the vault's `KEK_pw` derivation with a fresh random 16-byte `export_id` as the salt. It cannot collide with a vault header, whose wrap uses a different salt, nonce and AAD label. See the open question in §10.)

## 4. Encryption
```
padded     = plaintext ‖ 0x80 ‖ 0x00…     ; ISO/IEC 7816-4, to the next multiple of 1024; ALWAYS at least one padding byte
aad        = "aryavault/export/v1" ‖ canonical-CBOR({ "format_version": 1, "kdf": <as in the file>, "export_id": <as in the file> })
ct         = XChaCha20-Poly1305-Encrypt(key = KEK, nonce = nonce, aad = aad, plaintext = padded)   ; ciphertext ‖ tag
```
`nonce` is 24 random bytes from the OS CSPRNG, fresh for every export. The AAD binds the version, the whole KDF struct and the export id, so changing any of them (even to another valid value) makes decryption fail. After authentication succeeds, padding is removed strictly: strip trailing zero bytes, require `0x80`, and require that the padding is the minimal form (no extra whole 1024-byte block). Padding errors are reported only after authentication, so they cannot act as an oracle.

A wrong password and a modified file are indistinguishable by design ("wrong password or damaged export").

## 5. Payload
`plaintext` is one canonical CBOR map; every map below has *exactly* the listed keys (a reader rejects unknown or missing keys):

```
payload = {
  "format": "aryavault-export",
  "include_history": bool,
  "folders": [ folder… ],       ; parents before children
  "items":   [ item… ]
}
folder = { "id": bstr(16), "name": tstr, "parent": bstr(16) / null }
item = {
  "id": bstr(16),               ; informational; importers create new ids
  "type": "login" / "note" / "card" / "identity",
  "title": tstr,
  "folder": bstr(16) / null,    ; a folder id from "folders"
  "favorite": bool,
  "fields": { <field-key>: tstr, … },   ; only fields that have a value, never "title"
  "urls": [ tstr… ],            ; logins only
  "tags": [ tstr… ],
  "custom": [ { "kind": "text"/"hidden"/"url"/"date", "label": tstr, "value": tstr }… ],
  "history": { <field-key>: [ tstr… ], … }   ; present IFF include_history; see below
}
```
**Field keys per type** (docs/05 §2): `login`: `username`, `password`, `totp_seed`, `notes`; `note`: `body`; `card`: `holder`, `number`, `expiry`, `cvv`, `pin`, `notes`; `identity`: `first_name`, `middle_name`, `last_name`, `email`, `phone`, `address`, `ids`, `notes`.

**History.** `history[key]` lists the *previous* values of a field that currently has a value, **oldest first**, excluding the current value and excluding cleared versions. It carries no timestamps or device ids; an importer replays the values as successive edits, so the importing vault's own retention limits (docs/05 §8) apply. Concurrent versions are exported like any other retained version.

**What is exported:** every visible item (docs/06 §5.2); items in the trash, deleted folders, tombstones, `created_at`, HLC timestamps and device ids are **not** exported. **Limits** (docs/05 §10) apply to values: a field ≤ 64 KiB (a note body ≤ 1 MiB), ≤ 100 custom fields and ≤ 50 tags per item. Readers also bound the number of records (default 100,000), string length (default 1 MiB), nesting depth (16) and total number of CBOR items.

## 6. Reading an export (informative)
Parse and authenticate the container (§2-§4), decode the payload strictly, then convert to the importer's neutral model. A malformed *item* is skipped and reported; a malformed container, payload map or folder entry rejects the whole file. Importing is separate from parsing and atomic: the vault is either fully updated or not at all. Imported items receive new ids and creation times; folders are matched by name and parent so repeated imports do not duplicate them; logins with the same case-insensitive title, username and URL as an existing login are skipped by default.

## 7. Test vector
Everything here is fake. The repository fixture `core/testdata/import/aryavault-v1.avex` is this file; `aryavault-v1.payload.cbor` is its plaintext payload.

| item | value |
|---|---|
| password | `CANARY-export-password-v1` |
| KDF | argon2id v19, m = 65536 KiB, t = 3, p = 1 |
| salt (16 B) | `000102030405060708090a0b0c0d0e0f` |
| export_id (16 B) | `101112131415161718191a1b1c1d1e1f` |
| nonce (24 B) | `202122232425262728292a2b2c2d2e2f3031323334353637` |
| MK | `131e0daf50e7c4e14da7526d5735fc20f9cf77af1924808f3a10b39d75730656` |
| KEK | `53ff9a371445bd0383e9bb6b930c692352cd8744602f9de453724934943ee79f` |
| payload length | 299 bytes (padded to 1024; ciphertext 1040 bytes) |
| file length | 1192 bytes |
| SHA-256 of the file | `7cca965dcbb0560f9f6225b413758746fe153298bc7449e8a1125bb95f2498bf` |

The payload (hex):
```
a4656974656d7381a962696450111111111111111111111111111111116474616773816663616e6172796474797065656c6f67696e6475726c73817468747470733a2f2f6578616d706c652e74657374657469746c656e43414e415259204578616d706c6566637573746f6d80666669656c6473a26870617373776f72647343414e4152592d6b61742d70617373776f726468757365726e616d656b63616e6172792d7573657266666f6c6465725022222222222222222222222222222222686661766f72697465f566666f726d617470617279617661756c742d6578706f727467666f6c6465727381a36269645022222222222222222222222222222222646e616d656d43414e41525920466f6c64657266706172656e74f66f696e636c7564655f686973746f7279f4
```
It decodes to one folder ("CANARY Folder") and one login ("CANARY Example", user `canary-user`, password `CANARY-kat-password`, URL `https://example.test`, tag `canary`, favorite). These values were produced by the Rust implementation and independently reproduced and decrypted by a separate Python program (`argon2-cffi`, `cryptography`, `PyNaCl`) that follows only this document.

## 8. Security notes
- **Password strength is the only protection** of a copied file. Argon2id at the floor (64 MiB, t = 3) slows guessing but cannot save a weak password; exports SHOULD use calibrated, higher parameters and the UI SHOULD enforce the master-password policy (docs/04 §3).
- **No integrity beyond the password:** anyone who knows the password can forge an export. The authentication tag protects against everyone else.
- **Size leakage:** the file reveals its size to the nearest KiB, the KDF parameters and the format version.
- **No compression** (docs/04 §6).
- A reader MUST wipe the derived keys and decrypted buffers when finished and MUST NOT log passwords, keys or payload content.

## 9. Plaintext CSV export
Unencrypted, for moving data to tools that cannot read §2. The UI MUST warn that every password is written in clear text and require explicit confirmation (the library enforces this with a token type). UTF-8, LF line endings, header row:

`name,url,username,password,note,totp,type,folder,tags,favorite`

`type` is `login` or `note` (a note's body is in `note`). Cards and identities cannot be represented and are skipped; only a login's first URL is written; `tags` are joined with `, `; `favorite` is `1` or `0`. **CSV-injection neutralisation:** any cell that begins with `=`, `+`, `-`, `@`, a tab or a carriage return is prefixed with a single quote `'`, because a spreadsheet may execute such a cell as a formula. This intentionally alters those values and is not reversed on import.

## 10. Open questions for the owner
1. **Key derivation label.** The export reuses the `aryavault/kek-pw/v1` HKDF label with `export_id` as salt because T07 may not add primitives to the crypto crate. A dedicated label (e.g. `aryavault/export-key/v1`) would be cleaner domain separation and needs a small crypto-crate addition plus a new test vector; decide before release.
2. **Container magic and CBOR body.** Is `AVEX` + CBOR the wanted wrapper (vs. a fully self-describing CBOR file)? It allows file-type sniffing and is trivial to parse.
3. **History without timestamps.** Replaying history as fresh edits loses the original times and device ids. Acceptable, or should the format carry them?
4. **Placeholder limits** (64 MiB file, 100,000 records) mirror docs/05 §10 and the task; confirm.
