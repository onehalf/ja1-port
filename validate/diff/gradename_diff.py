"""Differential test: InvGradeName 0x25BDC (the item voice's grade name), original vs port.

usage: .venv/bin/python validate/diff/gradename_diff.py [ncases=4000] [DUMP]

Per case: a scratch text buffer (fill byte 0xAA, then a base text of 0..23 ASCII bytes and a NUL), a prefix code
0..255 and a count byte 0..255. The original runs in Unicorn on tac60 at 0x25BDC (EAX = count, EDX = prefix code,
EBX = buffer): code 2 / 3 / 4 writes "C" / "L" / "U" at the buffer start (dropping the base text), any other code
keeps it; then the grade letter of the signed count (A..F, panel.md "The item's voice") and ".8" go at the end.
The port (ja1f_inv_grade_name = iteminfo::inv_grade_name, the item voice's member name) must give the same text,
and every byte after the new terminator must stay at the fill byte.

Exits 1 on any mismatch.
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
ncases = int(argv[0]) if argv and argv[0].isdigit() else 4000
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")

PORT = ctypes.CDLL(LIB)
PORT.ja1f_inv_grade_name.argtypes = [ctypes.c_char_p, ctypes.c_uint32, ctypes.c_int32,
                                     ctypes.POINTER(ctypes.c_char), ctypes.c_int32]
PORT.ja1f_inv_grade_name.restype = ctypes.c_int32

GRADE = 0x25BDC
BUF = 64
FILL = 0xAA

if not os.path.exists(dump):
    print(f"gradename_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
buf = e.alloc(BUF)

rng = random.Random(int(os.environ.get("GRADENAME_SEED", "1")))
bases = [b"", b"X", b"abc", b"item", b"a" * 23, b"0123456789abcdefghijklmn"]
# every count byte against every prefix code, then fuzz
fix = [(code, v, bases[i % len(bases)]) for code in range(256) for v in (0, 10, 11, 25, 26, 50, 51, 75, 76, 99, 100, 127, 128, 200, 255)
       for i in range(1)]
cases = fix[:]
while len(cases) < ncases:
    code = rng.randrange(256)
    v = rng.randrange(256)
    n = rng.randrange(24)
    base = bytes(rng.randrange(0x20, 0x7F) for _ in range(n))
    cases.append((code, v, base))

fails = 0
for i, (code, v, base) in enumerate(cases):
    e.uc.mem_write(buf, bytes([FILL]) * BUF)
    e.uc.mem_write(buf, base + b"\0")
    pre = bytes(e.uc.mem_read(buf, BUF))
    e.call(rt(GRADE), v, code, buf)
    got = e.cstr(buf)
    out = (ctypes.c_char * BUF)()
    n = PORT.ja1f_inv_grade_name(base + b"\0", code, v, out, BUF)
    want = bytes(out[:n])
    bad = []
    if got != want:
        bad.append(f"text original {got!r} port {want!r}")
    tail = bytes(e.uc.mem_read(buf + len(got) + 1, BUF - len(got) - 1))
    if tail != pre[len(got) + 1:]:
        bad.append("a byte after the terminator changed")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: code {code} v {v} base {base!r}: " + "; ".join(bad))
print(f"gradename_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
