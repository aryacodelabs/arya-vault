#!/usr/bin/env python3
"""Independent check of the Windows quick-unlock KEK (docs/04 section 8).

Run:  python -I tools/crosscheck/test_kek_qu.py

`KEK_qu = HKDF-SHA256(salt = blob salt, ikm = platform signature, info = "aryavault/kek-qu/v1")`,
32 bytes. The Rust function `arya_vault_crypto::hkdf::kek_quick_unlock` has a known-answer test
with the same vector; this script recomputes it with a different implementation (the
`cryptography` package, through crosscheck.py's `hkdf`, plus a hand-written RFC 5869 using only
`hmac` and `hashlib`), so a mistake in either side shows up as a mismatch.

The signature is a fake 256-byte pattern, not a real Hello signature.
"""
from __future__ import annotations

import hashlib
import hmac
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import crosscheck as cc  # noqa: E402

INFO = b"aryavault/kek-qu/v1"
SIGNATURE = bytes((i * 7 + 3) % 256 for i in range(256))
SALT = bytes(range(0x10, 0x20))
EXPECTED = "e0747f3958f7a982e4852354b075cb11969b6a0b148b024f96d17d6fc44c09c1"


def rfc5869(salt: bytes, ikm: bytes, info: bytes) -> bytes:
    prk = hmac.new(salt, ikm, hashlib.sha256).digest()
    return hmac.new(prk, info + b"\x01", hashlib.sha256).digest()


def main() -> int:
    via_cryptography = cc.hkdf(SALT, SIGNATURE, INFO).hex()
    via_hmac = rfc5869(SALT, SIGNATURE, INFO).hex()
    ok = True
    for name, got in (("cryptography", via_cryptography), ("hmac", via_hmac)):
        if got != EXPECTED:
            print(f"FAIL: {name} gives {got}, expected {EXPECTED}")
            ok = False
    # Domain separation: the password-KEK label must give a different key.
    if cc.hkdf(SALT, SIGNATURE, b"aryavault/kek-pw/v1").hex() == EXPECTED:
        print("FAIL: kek-qu equals kek-pw for the same inputs")
        ok = False
    print("ok: KEK_qu vector matches" if ok else "FAILED")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
