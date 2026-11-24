#!/usr/bin/env python3
"""Differential test: 0x156B8 (a free effect record of the table 0xEAFF0, used by Explode, LandItem?, ProjectileImpact),
original vs the port (play::puff, host op 178).

usage: .venv/bin/python validate/diff/puff_diff.py [ncases=500] [DUMP] [--lib SO]
A case: a table with random used and free records, then random arguments. Compared: the whole table and the result.
Exits 1 on mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"puff_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(LIB)
P.ja1f_puff.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
P.ja1f_puff.restype = ctypes.c_int32
TABLE, REC, N = 0xEB002 - 0x12, 0x12, 16     # the search starts at record 1 (0xEB002)
e = Emu(dump=dump, fast=True)
r = random.Random(5)
bad, seen = 0, {}
for case in range(ncases):
    used = r.choice([0.0, 0.5, 0.9, 1.0])
    t = bytearray(r.randrange(256) for _ in range(N * REC))
    for k in range(N):
        t[k * REC] = r.randrange(1, 256) if r.random() < used else 0
    e.uc.mem_write(e.rt(TABLE), bytes(t))
    tile, x, y = r.randrange(-32768, 32768), r.randrange(-32768, 32768), r.randrange(-32768, 32768)
    look = r.randrange(1 << 32)
    count, kind, power = r.randrange(256), r.randrange(256), r.randrange(256)
    got = e.call(e.rt(0x156B8), tile & 0xFFFFFFFF, x & 0xFFFFFFFF, y & 0xFFFFFFFF, look, count, kind, power)
    got = struct.unpack("<i", struct.pack("<I", got & 0xFFFFFFFF))[0]
    o = bytes(e.uc.mem_read(e.rt(TABLE), N * REC))
    fx = ctypes.create_string_buffer(bytes(t), N * REC)
    a = (ctypes.c_int32 * 7)(tile, x, y, struct.unpack("<i", struct.pack("<I", look))[0], count, kind, power)
    res = P.ja1f_puff(fx, a)
    seen[res] = seen.get(res, 0) + 1
    if res != got or fx.raw != o:
        bad += 1
        if bad <= 5:
            d = next((i for i in range(N * REC) if fx.raw[i] != o[i]), -1)
            print(f"case {case}: result {got} vs {res}, first byte {d} (record {d // REC} +{d % REC:#x})")
if -1 not in seen or len(seen) < 3:
    print(f"puff_diff: the cases do not reach all results: {seen}")
    bad += 1
print(f"puff_diff: {ncases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
