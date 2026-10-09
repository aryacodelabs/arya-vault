#!/usr/bin/env python3
"""Independent cross-check decryptor for AryaVault golden vaults (docs/11 §2, docs/04 §13).

Written from docs/04-crypto-spec.md (incl. §16) ONLY; it shares no code with, and was not
derived from, the Rust implementation. It opens a golden vault directory with the master
password and with the recovery key, checks both yield the same vault key, decrypts every
segment / snapshot / manifest envelope, verifies padding, path binding and hash chain, and
compares plaintexts with expected.json. Exit code 0 only if everything verifies.

Run:  python -I tools/crosscheck/crosscheck.py core/testdata/golden/v1
      python -I tools/crosscheck/crosscheck.py core/testdata/golden      (all v*/ dirs)
Keys are never printed.
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
import unicodedata
from pathlib import Path

from argon2.low_level import Type, hash_secret_raw
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.kdf.hkdf import HKDF
from nacl.bindings import crypto_aead_xchacha20poly1305_ietf_decrypt
from nacl.exceptions import CryptoError


class VerifyError(Exception):
    """Any failure to parse or authenticate; always a non-zero exit."""


# ---------------------------------------------------------------- strict canonical CBOR
# RFC 8949 §4.2.1 subset (docs/04 §16): uint, nint, bstr, tstr, array, map, bool, null.
MAX_DEPTH = 16


class _Reader:
    def __init__(self, data: bytes, max_len: int):
        self.d, self.i, self.max_len = data, 0, max_len

    def take(self, n: int) -> bytes:
        if n < 0 or self.i + n > len(self.d):
            raise VerifyError("cbor: truncated")
        out = self.d[self.i : self.i + n]
        self.i += n
        return out

    def head(self):
        b = self.take(1)[0]
        major, ai = b >> 5, b & 31
        if ai < 24:
            return major, ai, ai
        widths = {24: 1, 25: 2, 26: 4, 27: 8}
        if ai not in widths:
            raise VerifyError("cbor: reserved or indefinite head")
        arg = int.from_bytes(self.take(widths[ai]), "big")
        minimum = {24: 24, 25: 0x100, 26: 0x10000, 27: 0x100000000}[ai]
        if major != 7 and arg < minimum:
            raise VerifyError("cbor: non-shortest head")
        return major, ai, arg

    def value(self, depth: int):
        major, ai, arg = self.head()
        if major == 0:
            return arg
        if major == 1:
            return ("nint", arg)
        if major in (2, 3):
            if arg > self.max_len:
                raise VerifyError("cbor: string too long")
            raw = self.take(arg)
            if major == 2:
                return raw
            try:
                return raw.decode("utf-8")
            except UnicodeDecodeError as e:
                raise VerifyError("cbor: bad utf-8") from e
        if major == 4:
            if depth >= MAX_DEPTH or arg > 256 or arg > len(self.d) - self.i:
                raise VerifyError("cbor: bad array")
            return [self.value(depth + 1) for _ in range(arg)]
        if major == 5:
            if depth >= MAX_DEPTH or arg > 256 or 2 * arg > len(self.d) - self.i:
                raise VerifyError("cbor: bad map")
            out, prev = {}, None
            for _ in range(arg):
                start = self.i
                k = self.value(depth + 1)
                kb = self.d[start : self.i]
                if prev is not None and not prev < kb:
                    raise VerifyError("cbor: map keys duplicated or unsorted")
                prev = kb
                if not isinstance(k, (str, int, bytes)) or isinstance(k, bool):
                    raise VerifyError("cbor: unsupported map key")
                out[k] = self.value(depth + 1)
            return out
        if major == 7 and ai in (20, 21, 22):
            return {20: False, 21: True, 22: None}[ai]
        raise VerifyError("cbor: unsupported item")


def cbor_decode(data: bytes, max_len: int = 4096):
    r = _Reader(data, max_len)
    v = r.value(0)
    if r.i != len(data):
        raise VerifyError("cbor: trailing bytes")
    return v


def _head(major: int, n: int) -> bytes:
    if n < 24:
        return bytes([major << 5 | n])
    for ai, w in ((24, 1), (25, 2), (26, 4), (27, 8)):
        if n < 1 << (8 * w):
            return bytes([major << 5 | ai]) + n.to_bytes(w, "big")
    raise VerifyError("cbor: integer too large")


def cbor_encode(v) -> bytes:
    if isinstance(v, bool) or v is None:
        return bytes([0xF6 if v is None else 0xF5 if v else 0xF4])
    if isinstance(v, int):
        return _head(0, v)
    if isinstance(v, bytes):
        return _head(2, len(v)) + v
    if isinstance(v, str):
        raw = v.encode("utf-8")
        return _head(3, len(raw)) + raw
    if isinstance(v, list):
        return _head(4, len(v)) + b"".join(cbor_encode(x) for x in v)
    if isinstance(v, dict):
        items = sorted((cbor_encode(k), cbor_encode(x)) for k, x in v.items())
        return _head(5, len(items)) + b"".join(k + x for k, x in items)
    raise VerifyError("cbor: cannot encode")


# ------------------------------------------------------------------------ spec constants
# docs/04 §3 bounds
M_MIN, M_MAX, T_MIN, T_MAX, P_MIN, P_MAX = 64 * 1024, 1024 * 1024, 3, 10, 1, 8
MAX_HEADER = 2048
KIB = 1024
MAX_PLAINTEXT = {1: 1 << 20, 2: 64 << 20, 3: 256 << 10}  # segment, snapshot, manifest (§16)
KIND_LABEL = {1: "log/v1", 2: "snapshot/v1", 3: "manifest/v1"}
FIXED = 111

HEX = "0-9a-f"
RE_HEADER = re.compile(rf"header-([{HEX}]{{8}})-([{HEX}]{{8}})-([{HEX}]{{32}})\.bin")
RE_SEG = re.compile(rf"devices/([{HEX}]{{32}})/([{HEX}]{{16}})\.seg")
RE_MAN = re.compile(rf"devices/([{HEX}]{{32}})/manifest-([{HEX}]{{16}})\.bin")
RE_SNAP = re.compile(rf"snapshots/([{HEX}]{{16}})-([{HEX}]{{32}})\.snap")


def u(name: str, v, bits: int) -> int:
    if isinstance(v, bool) or not isinstance(v, int) or not 0 <= v < 1 << bits:
        raise VerifyError(f"field {name}: not a u{bits}")
    return v


def bstr(name: str, v, n: int) -> bytes:
    if not isinstance(v, bytes) or len(v) != n:
        raise VerifyError(f"field {name}: expected {n} bytes")
    return v


# ---------------------------------------------------------------------------- primitives
def aead_open(key: bytes, nonce: bytes, ct: bytes, aad: bytes) -> bytes:
    try:
        return crypto_aead_xchacha20poly1305_ietf_decrypt(ct, aad, nonce, key)
    except CryptoError as e:
        raise VerifyError("authentication failed") from e


def hkdf(salt: bytes, ikm: bytes, info: bytes) -> bytes:
    return HKDF(algorithm=hashes.SHA256(), length=32, salt=salt, info=info).derive(ikm)


_MK_CACHE: dict = {}


def master_key(password: str, kdf: dict) -> bytes:
    """Argon2id v1.3 over the NFKD-normalised UTF-8 password; bounds checked first (§3)."""
    if not (M_MIN <= kdf["m_kib"] <= M_MAX and T_MIN <= kdf["t"] <= T_MAX and P_MIN <= kdf["p"] <= P_MAX):
        raise VerifyError("vault parameters are out of range or corrupted")
    if len(kdf["salt"]) != 16:
        raise VerifyError("salt must be 16 bytes")
    key = (password, kdf["m_kib"], kdf["t"], kdf["p"], kdf["salt"])
    if key not in _MK_CACHE:  # memoised only so the byte-flip tests stay fast
        pw = unicodedata.normalize("NFKD", password).encode("utf-8")
        _MK_CACHE[key] = hash_secret_raw(
            pw, kdf["salt"], time_cost=kdf["t"], memory_cost=kdf["m_kib"],
            parallelism=kdf["p"], hash_len=32, type=Type.ID, version=19,
        )
    return _MK_CACHE[key]


CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ"


def parse_recovery_key(text: str) -> bytes:
    """§4 / §16: 32 Crockford chars + 2 checksum chars; tolerant of case, spaces, hyphens, I/L/O."""
    chars = [c for c in text.upper() if c != "-" and not c.isspace()]
    chars = ["1" if c in "IL" else "0" if c == "O" else c for c in chars]
    if len(chars) != 34 or any(c not in CROCKFORD for c in chars):
        raise VerifyError("recovery key: bad length or character")
    n = 0
    for c in chars[:32]:
        n = n << 5 | CROCKFORD.index(c)
    rk = n.to_bytes(20, "big")
    d = hashlib.sha256(b"aryavault/rk-check/v1" + rk).digest()
    want = (d[0] << 2) | (d[1] >> 6)  # first 10 bits
    got = CROCKFORD.index(chars[32]) << 5 | CROCKFORD.index(chars[33])
    if want != got:
        raise VerifyError("recovery key: checksum mismatch")
    return rk


# ------------------------------------------------------------------------------ header
def parse_header(data: bytes, name: str) -> dict:
    if len(data) > MAX_HEADER:
        raise VerifyError("header too large")
    m = RE_HEADER.fullmatch(name)
    if not m:
        raise VerifyError("header file name is not canonical")
    h = cbor_decode(data, 64)
    if not isinstance(h, dict) or h.get("format_version") != 1:
        raise VerifyError("unsupported or missing format_version")
    if set(h) != {"format_version", "vault_id", "header_version", "epoch", "kdf", "wrap_pw", "wrap_rk", "created_at"}:
        raise VerifyError("header: unexpected field set")
    kdf = h["kdf"]
    if not isinstance(kdf, dict) or set(kdf) != {"alg", "version", "m_kib", "t", "p", "salt"}:
        raise VerifyError("kdf: unexpected field set")
    if kdf["alg"] != "argon2id" or kdf["version"] != 0x13:
        raise VerifyError("kdf: unsupported algorithm")
    out = {
        "vault_id": bstr("vault_id", h["vault_id"], 16),
        "header_version": u("header_version", h["header_version"], 32),
        "epoch": u("epoch", h["epoch"], 32),
        "created_at": u("created_at", h["created_at"], 64),
        "kdf": {k: u(k, kdf[k], 32) for k in ("m_kib", "t", "p")} | {"salt": bstr("salt", kdf["salt"], 16)},
        "kdf_raw": kdf,
    }
    for w in ("wrap_pw", "wrap_rk"):
        if not isinstance(h[w], dict) or set(h[w]) != {"nonce", "ct"}:
            raise VerifyError(f"{w}: unexpected field set")
        out[w] = (bstr(w + ".nonce", h[w]["nonce"], 24), bstr(w + ".ct", h[w]["ct"], 48))
    if (int(m[1], 16), int(m[2], 16)) != (out["epoch"], out["header_version"]):
        raise VerifyError("header name does not match body")
    out["device_id"] = bytes.fromhex(m[3])
    return out


def unwrap_vault_key(hd: dict, password: str | None, recovery_key: str | None):
    """Returns (VK via password, VK via recovery key); either may be None if not supplied."""
    vid, epoch = hd["vault_id"], hd["epoch"].to_bytes(4, "big")
    vk_pw = vk_rk = None
    if password is not None:
        kek = hkdf(vid, master_key(password, hd["kdf"]), b"aryavault/kek-pw/v1")
        aad = b"aryavault/wrap-pw/v1" + vid + epoch + cbor_encode(
            {"alg": "argon2id", "version": 19, **{k: hd["kdf"][k] for k in ("m_kib", "t", "p", "salt")}}
        )
        vk_pw = aead_open(kek, hd["wrap_pw"][0], hd["wrap_pw"][1], aad)
    if recovery_key is not None:
        kek = hkdf(vid, parse_recovery_key(recovery_key), b"aryavault/kek-rk/v1")
        aad = b"aryavault/wrap-rk/v1" + vid + epoch  # no header_version, no kdf (SEC-C12)
        vk_rk = aead_open(kek, hd["wrap_rk"][0], hd["wrap_rk"][1], aad)
    for vk in (vk_pw, vk_rk):
        if vk is not None and len(vk) != 32:
            raise VerifyError("unwrapped vault key has the wrong length")
    return vk_pw, vk_rk


# ---------------------------------------------------------------------------- envelope
def unpad(p: bytes) -> bytes:
    if not p or len(p) % KIB:
        raise VerifyError("padding: bad length")
    stripped = p.rstrip(b"\x00")
    if not stripped or stripped[-1] != 0x80:
        raise VerifyError("padding: missing 0x80 marker")
    pt = stripped[:-1]
    if (len(pt) // KIB + 1) * KIB != len(p):
        raise VerifyError("padding: not minimal")
    return pt


def open_envelope(vk: bytes, vault_id: bytes, epoch: int, data: bytes, rel_path: str) -> dict:
    """Parse, path-bind, authenticate and unpad one envelope (docs/04 §6, §16)."""
    if len(data) < 6 or data[:4] != b"AVLT":
        raise VerifyError("envelope: bad magic")
    if int.from_bytes(data[4:6], "big") != 1:
        raise VerifyError("envelope: unsupported format_version")
    if len(data) < FIXED:
        raise VerifyError("envelope: truncated")
    kind = data[6]
    if kind not in KIND_LABEL:
        raise VerifyError("envelope: bad kind")
    f = {
        "vault_id": data[7:23], "epoch": int.from_bytes(data[23:27], "big"), "device_id": data[27:43],
        "seq": int.from_bytes(data[43:51], "big"), "prev_hash": data[51:83],
    }
    nonce, ct_len = data[83:107], int.from_bytes(data[107:111], "big")
    ct = data[FIXED:]
    max_ct = (MAX_PLAINTEXT[kind] // KIB + 1) * KIB + 16
    if ct_len > max_ct or ct_len < KIB + 16 or (ct_len - 16) % KIB:
        raise VerifyError("envelope: bad ct_len")
    if len(ct) != ct_len:
        raise VerifyError("envelope: length mismatch")
    zero = bytes(32)
    if kind == 1 and (f["seq"] < 1 or (f["seq"] == 1 and f["prev_hash"] != zero)):
        raise VerifyError("envelope: bad segment seq/prev_hash")
    if kind == 2 and (f["seq"] != 0 or f["prev_hash"] != zero):
        raise VerifyError("envelope: bad snapshot seq/prev_hash")
    if kind == 3 and f["prev_hash"] != zero:
        raise VerifyError("envelope: bad manifest prev_hash")
    if f["vault_id"] != vault_id or f["epoch"] != epoch:
        raise VerifyError("envelope: wrong vault or epoch")
    # path binding (§6 / §16)
    if kind == 1:
        m = RE_SEG.fullmatch(rel_path)
        ok = bool(m) and bytes.fromhex(m[1]) == f["device_id"] and int(m[2], 16) == f["seq"]
    elif kind == 3:
        m = RE_MAN.fullmatch(rel_path)
        ok = bool(m) and bytes.fromhex(m[1]) == f["device_id"] and int(m[2], 16) == f["seq"]
    else:
        m = RE_SNAP.fullmatch(rel_path)
        ok = bool(m) and bytes.fromhex(m[2]) == f["device_id"]
    if not ok:
        raise VerifyError("envelope: does not match its path")
    aad = cbor_encode({
        "magic": b"AVLT", "format_version": 1, "kind": kind, "vault_id": f["vault_id"],
        "epoch": f["epoch"], "device_id": f["device_id"], "seq": f["seq"], "prev_hash": f["prev_hash"],
    })
    key = hkdf(vault_id, vk, KIND_LABEL[kind].encode() + epoch.to_bytes(4, "big"))
    pt = unpad(aead_open(key, nonce, ct, aad))
    if len(pt) > MAX_PLAINTEXT[kind]:
        raise VerifyError("envelope: plaintext over limit")
    return {"kind": kind, "plaintext": pt, "hash": hashlib.sha256(data).digest(), **f}


# ------------------------------------------------------------------------------ driver
def check_chain(segs: list) -> None:
    """Hash chain (docs/04 §7) over one device's (seq, prev_hash, envelope_sha256) tuples:
    the first must be seq 1 with a zero prev_hash; each next seq is +1 and its prev_hash is
    the SHA-256 of the previous envelope's bytes."""
    segs = sorted(segs)
    if segs[0][0] != 1 or segs[0][1] != bytes(32):
        raise VerifyError("segment hash chain does not start at seq 1 with a zero prev_hash")
    for (s0, _, h0), (s1, p1, _) in zip(segs, segs[1:]):
        if s1 != s0 + 1 or p1 != h0:
            raise VerifyError("segment hash chain broken")


def verify_dir(vdir: Path, password: str | None = None, recovery_key: str | None = None,
               check_pins: bool = True) -> list[str]:
    """Verifies one golden version directory; returns report lines or raises VerifyError.

    check_pins=False skips the SHA-256 file pins from expected.json so that tests can prove
    the parsing/authentication layers reject tampering on their own.
    """
    exp = json.loads((vdir / "expected.json").read_text(encoding="utf-8"))
    password = exp["password"] if password is None else password
    recovery_key = exp["recovery_key"] if recovery_key is None else recovery_key
    lines = []

    # discover files strictly by the §16 grammar; anything unexpected is an error
    files = sorted(p.relative_to(vdir).as_posix() for p in vdir.rglob("*") if p.is_file())
    known = {"README.md", "expected.json"}
    headers = [f for f in files if f.startswith("header-")]
    if len(headers) != 1:
        raise VerifyError(f"expected exactly one header, found {len(headers)}")
    for f in files:
        if f not in known and not (f in headers or RE_SEG.fullmatch(f) or RE_MAN.fullmatch(f) or RE_SNAP.fullmatch(f)):
            raise VerifyError(f"unexpected file {f!r}")

    hdata = (vdir / headers[0]).read_bytes()
    hd = parse_header(hdata, headers[0])
    lines.append(f"header ok: format 1, epoch {hd['epoch']}, version {hd['header_version']}")
    for field, want in (("vault_id", bytes.fromhex(exp["vault_id"])), ("epoch", exp["epoch"]),
                        ("header_version", exp["header_version"]), ("created_at", exp["created_at"]),
                        ("device_id", bytes.fromhex(exp["device_id"]))):
        if hd[field] != want:
            raise VerifyError(f"header {field} differs from expected.json")
    k = exp["kdf"]
    if (hd["kdf"]["m_kib"], hd["kdf"]["t"], hd["kdf"]["p"], hd["kdf"]["salt"].hex()) != (k["m_kib"], k["t"], k["p"], k["salt"]):
        raise VerifyError("kdf parameters differ from expected.json")

    vk_pw, vk_rk = unwrap_vault_key(hd, password, recovery_key)
    if vk_pw != vk_rk:
        raise VerifyError("password and recovery key yield different vault keys")
    lines.append("wrap_pw and wrap_rk both unwrap; vault keys match")

    seen_kinds = {}
    chain: dict[bytes, list] = {}
    for f in files:
        if f in headers or f in known:
            continue
        data = (vdir / f).read_bytes()
        env = open_envelope(vk_pw, hd["vault_id"], hd["epoch"], data, f)
        seen_kinds[env["kind"]] = seen_kinds.get(env["kind"], 0) + 1
        if env["kind"] == 1:
            chain.setdefault(env["device_id"], []).append((env["seq"], env["prev_hash"], env["hash"]))
        name = {1: "segment", 2: "snapshot", 3: "manifest"}[env["kind"]]
        want = exp["files"][name]
        if f != want["path"]:
            raise VerifyError(f"{name} path differs from expected.json")
        if env["plaintext"].decode("utf-8", "strict") != want["plaintext"]:
            raise VerifyError(f"{name} plaintext differs from expected.json")
        if check_pins and hashlib.sha256(data).hexdigest() != want["sha256"]:
            raise VerifyError(f"{name} file hash differs from expected.json")
        lines.append(f"{name} ok: {f} ({len(env['plaintext'])} plaintext bytes)")
    if check_pins and hashlib.sha256(hdata).hexdigest() != exp["files"]["header"]["sha256"]:
        raise VerifyError("header file hash differs from expected.json")
    for kind in (1, 2, 3):
        if seen_kinds.get(kind, 0) != 1:
            raise VerifyError(f"expected exactly one envelope of kind {kind}")
    for segs in chain.values():
        check_chain(segs)
    lines.append("hash chain ok")
    return lines


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if not a.startswith("--")]
    if len(args) != 1:
        print("usage: python -I crosscheck.py <golden-version-dir | golden-root>", file=sys.stderr)
        return 2
    root = Path(args[0])
    dirs = [root] if (root / "expected.json").is_file() else sorted(
        d for d in root.glob("v*") if d.is_dir()
    )
    if not dirs:
        print(f"no golden directories found under {root}", file=sys.stderr)
        return 2
    failed = 0
    for d in dirs:
        try:
            lines = verify_dir(d)
        except (VerifyError, OSError, KeyError, ValueError) as e:
            print(f"FAIL {d.name}: {e}")
            failed += 1
        else:
            print(f"PASS {d.name}")
            for line in lines:
                print("  " + line)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
