#!/usr/bin/env python3
"""Negative checks for crosscheck.py: tampered golden files MUST be rejected.

Run:  python -I tools/crosscheck/test_negative.py [golden-root]
Exit code 0 only if (a) every untampered golden dir passes, and (b) every tampered copy
fails. Two layers:
  * in-process: flip EVERY byte of every file with SHA-256 pinning disabled, so rejection
    must come from parsing / path binding / AEAD / padding, not from the pin;
  * end-to-end: run the real script as a subprocess on tampered copies (flip first / middle
    / last byte, truncate, append, swap files, wrong secrets) and require a non-zero exit.
"""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import crosscheck as cc  # noqa: E402

SCRIPT = HERE / "crosscheck.py"
failures: list[str] = []


def check(cond: bool, what: str) -> None:
    if not cond:
        failures.append(what)
        print(f"  FAIL: {what}")


def rejected(fn) -> bool:
    try:
        fn()
    except (cc.VerifyError, OSError, KeyError, ValueError, UnicodeDecodeError):
        return True
    return False


def run_script(d: Path) -> int:
    return subprocess.run([sys.executable, "-I", str(SCRIPT), str(d)], capture_output=True).returncode


def copy(src: Path, tmp: Path) -> Path:
    dst = tmp / src.name
    shutil.copytree(src, dst)
    return dst


def data_files(d: Path) -> list[Path]:
    return sorted(p for p in d.rglob("*") if p.is_file() and p.name not in ("README.md", "expected.json"))


def test_primitives() -> None:
    """Direct tests of the strict parsers, which tampering of authenticated files cannot reach."""
    print("strict parsers:")
    corpus = {
        "non-shortest uint": "1800", "non-shortest 2-byte": "1900ff", "non-shortest length": "5800",
        "indefinite array": "9fff", "indefinite map": "bfff", "tag": "c001", "float": "f93c00",
        "undefined": "f7", "invalid utf-8": "62c328", "duplicate keys": "a2616101616102",
        "unsorted keys": "a2616201616102", "longer key first": "a2626161016162" + "02",
        "trailing bytes": "0100", "truncated": "19", "empty": "",
    }
    for name, h in corpus.items():
        check(rejected(lambda h=h: cc.cbor_decode(bytes.fromhex(h))), f"non-canonical CBOR accepted: {name}")
    deep = b"\x81" * 17 + b"\x00"
    check(rejected(lambda: cc.cbor_decode(deep)), "17-deep nesting accepted")
    check(not rejected(lambda: cc.cbor_decode(b"\x81" * 15 + b"\x00")), "15-deep nesting rejected")
    check(cc.cbor_decode(bytes.fromhex("a2616101616202")) == {"a": 1, "b": 2}, "canonical map not decoded")
    for v in ({"b": 1, "a": 2}, {"alg": "argon2id", "salt": b"\x00" * 16, "t": 3}):
        check(cc.cbor_decode(cc.cbor_encode(v)) == v, "encode/decode round trip")

    bad_pads = {
        "empty": b"", "short": b"\x80", "not a multiple": b"\x80" + bytes(1023 - 0) + b"\x00",
        "all zeros": bytes(1024), "wrong marker": b"abc\x81" + bytes(1020),
        "junk after marker": b"abc\x80" + bytes(1019) + b"\x01",
        "non-minimal (extra bucket)": b"abc\x80" + bytes(1020 + 1024),
    }
    for name, p in bad_pads.items():
        check(rejected(lambda p=p: cc.unpad(p)), f"bad padding accepted: {name}")
    for n in (0, 1, 1023, 1024, 1025, 2047):
        padded = bytes(range(256))[:1] * n + b"\x80" + bytes(((n // 1024 + 1) * 1024) - n - 1)
        check(len(padded) == (n // 1024 + 1) * 1024 and cc.unpad(padded) == bytes(range(256))[:1] * n, f"valid padding rejected at n={n}")

    # hash chain (§7). The v1 golden set has one segment, so exercise the linking logic here.
    z, h1, h2, h3 = bytes(32), bytes([1]) * 32, bytes([2]) * 32, bytes([3]) * 32
    check(not rejected(lambda: cc.check_chain([(1, z, h1)])), "single segment chain rejected")
    check(not rejected(lambda: cc.check_chain([(2, h1, h2), (1, z, h1), (3, h2, h3)])), "valid 3-chain rejected")
    check(rejected(lambda: cc.check_chain([(1, z, h1), (2, h3, h2)])), "wrong prev_hash accepted")
    check(rejected(lambda: cc.check_chain([(1, z, h1), (3, h1, h2)])), "sequence gap accepted")
    check(rejected(lambda: cc.check_chain([(2, h1, h2)])), "chain not starting at seq 1 accepted")
    check(rejected(lambda: cc.check_chain([(1, h1, h1)])), "seq 1 with non-zero prev_hash accepted")

    # hostile KDF parameters must be rejected BEFORE any Argon2 call (spec 3, SEC-C11)
    calls = []
    real = cc.hash_secret_raw
    cc.hash_secret_raw = lambda *a, **k: calls.append(1) or real(*a, **k)
    try:
        for m, tt, pp in ((1 << 31, 3, 1), (65535, 3, 1), (1048577, 3, 1), (65536, 2, 1), (65536, 11, 1), (65536, 3, 0), (65536, 3, 9)):
            err = None
            try:
                cc.master_key("CANARY-x", {"m_kib": m, "t": tt, "p": pp, "salt": bytes(16)})
            except cc.VerifyError as e:
                err = str(e)
            check(err is not None and "out of range" in err, f"kdf ({m},{tt},{pp}) not rejected as out of range")
        check(not calls, "Argon2 ran for out-of-range parameters")
    finally:
        cc.hash_secret_raw = real


def test_dir(src: Path) -> None:
    print(f"{src.name}:")
    check(run_script(src) == 0, "untampered directory must pass (exit 0)")
    files = data_files(src)
    check(len(files) >= 4, "expected a header and three envelopes")

    # --- in-process: every byte of every file, pins off
    with tempfile.TemporaryDirectory() as t:
        work = copy(src, Path(t))
        flips = 0
        for f in data_files(work):
            orig = f.read_bytes()
            for i in range(len(orig)):
                mut = bytearray(orig)
                mut[i] ^= 0x01
                f.write_bytes(bytes(mut))
                flips += 1
                if not rejected(lambda: cc.verify_dir(work, check_pins=False)):
                    check(False, f"flip of byte {i} in {f.relative_to(work)} was ACCEPTED")
                    break
            f.write_bytes(orig)
            # truncation and extension
            for name, mut in (("truncated", orig[:-1]), ("extended", orig + b"\x00"), ("empty", b"")):
                f.write_bytes(mut)
                check(rejected(lambda: cc.verify_dir(work, check_pins=False)), f"{name} {f.relative_to(work)} accepted")
            f.write_bytes(orig)
        check(not rejected(lambda: cc.verify_dir(work, check_pins=False)), "restored copy must pass again")
        print(f"  {flips} single-byte flips rejected (pins disabled)")

    # --- in-process: wrong secrets and hostile KDF params
    check(rejected(lambda: cc.verify_dir(src, password="CANARY-wrong")), "wrong password accepted")
    check(rejected(lambda: cc.verify_dir(src, recovery_key="00000-00000-00000-00000-00000-00000-00-00")), "bad recovery key accepted")
    exp = json.loads((src / "expected.json").read_text())
    good_rk = exp["recovery_key"]
    typo = good_rk[:-1] + ("0" if good_rk[-1] != "0" else "1")
    check(rejected(lambda: cc.verify_dir(src, recovery_key=typo)), "recovery key with wrong checksum accepted")
    hdr = next(p for p in files if p.name.startswith("header-"))
    with tempfile.TemporaryDirectory() as t:
        work = copy(src, Path(t))
        h = work / hdr.relative_to(src)
        parsed = cc.cbor_decode(h.read_bytes())
        for field, value in (("m_kib", 1 << 31), ("m_kib", 65535), ("t", 2), ("t", 11), ("p", 0), ("p", 9)):
            parsed["kdf"][field] = value
            h.write_bytes(cc.cbor_encode(parsed))
            check(rejected(lambda: cc.verify_dir(work, check_pins=False)), f"kdf {field}={value} accepted")
            parsed["kdf"] = cc.cbor_decode(hdr.read_bytes())["kdf"]
        # a header written with a non-canonical (indefinite-length) map must be refused
        h.write_bytes(b"\xbf" + hdr.read_bytes()[1:] + b"\xff")
        check(rejected(lambda: cc.verify_dir(work, check_pins=False)), "indefinite-length header accepted")

    # --- end-to-end subprocess on tampered copies (pins on, as users run it)
    with tempfile.TemporaryDirectory() as t:
        for f in files:
            rel = f.relative_to(src)
            orig = f.read_bytes()
            for label, mut in (
                ("first byte", bytes([orig[0] ^ 1]) + orig[1:]),
                ("middle byte", orig[: len(orig) // 2] + bytes([orig[len(orig) // 2] ^ 0x80]) + orig[len(orig) // 2 + 1 :]),
                ("last byte", orig[:-1] + bytes([orig[-1] ^ 1])),
                ("truncated", orig[:-1]),
                ("appended", orig + b"\x00"),
            ):
                work = copy(src, Path(t)) if not (Path(t) / src.name).exists() else Path(t) / src.name
                (work / rel).write_bytes(mut)
                code = run_script(work)
                check(code != 0, f"{label} of {rel} -> exit {code}, expected non-zero")
                (work / rel).write_bytes(orig)
        print(f"  end-to-end tampering of {len(files)} files rejected")

    # --- path binding in isolation (SEC-Y10): call open_envelope directly with wrong paths,
    # so the expected.json path comparison cannot mask a missing binding check
    hd = cc.parse_header(hdr.read_bytes(), hdr.name)
    vk, _ = cc.unwrap_vault_key(hd, exp["password"], None)
    seg_f = next(p for p in files if p.suffix == ".seg")
    man_f = next(p for p in files if p.name.startswith("manifest-"))
    snap_f = next(p for p in files if p.suffix == ".snap")
    seg_rel, man_rel, snap_rel = (f.relative_to(src).as_posix() for f in (seg_f, man_f, snap_f))
    dev = seg_rel.split("/")[1]
    def op(f, rel):
        return cc.open_envelope(vk, hd["vault_id"], hd["epoch"], f.read_bytes(), rel)
    check(not rejected(lambda: op(seg_f, seg_rel)), "segment at its own path must open")
    check(not rejected(lambda: op(man_f, man_rel)), "manifest at its own path must open")
    check(not rejected(lambda: op(snap_f, snap_rel)), "snapshot at its own path must open")
    other_dev = "e2" * 16
    for what, f, rel in (
        ("segment at another device's path", seg_f, seg_rel.replace(dev, other_dev)),
        ("segment at another seq", seg_f, seg_rel.replace("0000000000000001.seg", "0000000000000002.seg")),
        ("segment at a manifest path", seg_f, man_rel),
        ("manifest at another device's path", man_f, man_rel.replace(dev, other_dev)),
        ("manifest at another counter", man_f, man_rel.replace("0000000000000001", "0000000000000002")),
        ("manifest at a segment path", man_f, seg_rel),
        ("snapshot at another device's path", snap_f, snap_rel.replace(dev, other_dev)),
        ("snapshot at a segment path", snap_f, seg_rel),
        ("segment at a non-canonical path", seg_f, seg_rel.upper()),
    ):
        check(rejected(lambda f=f, rel=rel: op(f, rel)), f"{what} accepted")

    # --- files moved to the wrong path (path binding, SEC-Y10)
    with tempfile.TemporaryDirectory() as t:
        work = copy(src, Path(t))
        seg = next(p for p in data_files(work) if p.suffix == ".seg")
        other = seg.with_name("0000000000000002.seg")
        seg.rename(other)
        check(run_script(work) != 0, "segment renamed to another seq accepted")
        other.rename(seg)
        dev = seg.parent
        moved = dev.parent / ("e2" * 16)
        dev.rename(moved)
        check(run_script(work) != 0, "device directory renamed (other device) accepted")


def main() -> int:
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else HERE.parent.parent / "core" / "testdata" / "golden"
    dirs = [root] if (root / "expected.json").is_file() else sorted(d for d in root.glob("v*") if d.is_dir())
    if not dirs:
        print(f"no golden directories under {root}", file=sys.stderr)
        return 2
    test_primitives()
    for d in dirs:
        test_dir(d)
    if failures:
        print(f"\n{len(failures)} negative check(s) FAILED")
        return 1
    print("\nall negative checks passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
