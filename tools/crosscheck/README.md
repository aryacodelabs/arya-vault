# crosscheck: independent golden-vault decryptor

A second implementation of the AryaVault v1 read path, in Python, used to catch drift
between the written spec (`docs/04-crypto-spec.md`, incl. §16) and the Rust core
(docs/11 §2, SEC-C08). It shares no code with the Rust crates and is written from the
spec text only.

## What it checks
For every `core/testdata/golden/v*/` directory (new version directories are picked up
automatically; each needs an `expected.json`):

1. strict canonical-CBOR parse of the header (bounds, alg, name/body agreement);
2. NFKD + Argon2id (`argon2-cffi`) -> MK -> HKDF-SHA256 (`cryptography`) -> `KEK_pw`;
   unwrap VK with XChaCha20-Poly1305 (`PyNaCl`) using the spec's AAD;
3. the same via the recovery key (`KEK_rk`); both VKs must be identical;
4. `K_log` / `K_snap` / `K_manifest` sub-keys; every envelope is parsed per §16,
   path-bound, authenticated, unpadded, and its plaintext compared with `expected.json`;
5. hash chain, file hashes (pinned in `expected.json`), and that no unexpected file exists.

Exit code 0 only if everything verifies. Keys are never printed.

## Run
```sh
python3 -m venv .venv
.venv/bin/pip install --require-hashes --no-deps -r tools/crosscheck/requirements.txt
.venv/bin/python -I tools/crosscheck/crosscheck.py core/testdata/golden          # all versions
.venv/bin/python -I tools/crosscheck/crosscheck.py core/testdata/golden/v1       # one version
.venv/bin/python -I tools/crosscheck/test_negative.py core/testdata/golden       # tamper tests
```
`test_negative.py` flips every byte of every golden file (with file-hash pinning turned
off, so parsing and authentication must reject it on their own), runs end-to-end
tampering through the real script, and unit-tests the strict parsers, padding, path
binding, hash-chain linking and KDF bounds directly.

## Dependencies
`requirements.txt` is generated with
`pip-compile --generate-hashes --strip-extras --allow-unsafe -o requirements.txt requirements.in`
and must be installed with `--require-hashes`. Direct deps: `argon2-cffi` (Argon2id
reference C library), `cryptography` (HKDF-SHA256), `PyNaCl` (libsodium XChaCha20-Poly1305).

## Adding a golden version
Add `core/testdata/golden/vN/` (append-only, see its README) with an `expected.json`
(same shape as v1). If `vN` changes a format, update `crosscheck.py` from the updated spec,
not from the Rust code.
