"""Differential test: FadePalette? 0x52908 (a palette fade), original vs port.

usage: .venv/bin/python validate/diff/fade_diff.py [ncases=200] [DUMP]

Per case: a 768-byte palette (6-bit colours), a step count n (1..24) and levels from / to (-20..256; the original
clamps a negative value to 0). The original runs in Unicorn on tac60 at 0x52908 (EAX source palette of 256
four-byte RGB entries, EDX destination palette, EBX n, ECX from, stack to): it waits for the vertical retrace
(port 0x3DA) and writes one 768-byte palette per level to the DAC (port 0x3C8 index, then 18-bit bytes to 0x3C9,
also through the OUTSB loop), level l = n*from/256 .. n*to/256, each colour c -> c*l/n (8-bit wrap). The port
(ja1f_fade = ja1front::office::fade; the office ledger and the screen fades) must give the same levels.
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
ncases = int(argv[0]) if argv and argv[0].isdigit() else 200
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")

PORT = ctypes.CDLL(LIB)
PORT.ja1f_fade.argtypes = [ctypes.POINTER(ctypes.c_ubyte), ctypes.c_int32, ctypes.c_int32,
                           ctypes.c_int32, ctypes.POINTER(ctypes.c_ubyte), ctypes.c_int32]
PORT.ja1f_fade.restype = ctypes.c_int32

FADE = 0x52908
PAL4, PAL3 = 1024, 768
MAX_LEVELS = 64

if not os.path.exists(dump):
    print(f"fade_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
src = e.alloc(PAL4)
dst = e.alloc(PAL4)
# The retrace poll waits for bit 0 = 0, then bit 8 = 1.
e.port_in = lambda p: 0x08
out = []
e.port_out = lambda p, size, v: out.append((p, v))

rng = random.Random(int(os.environ.get("FADE_SEED", "1")))
fixed = [(24, 256, 0), (24, 0, 256), (56, 256, 0), (1, 0, 256), (2, 0, 256), (16, 128, 0),
         (24, 256, 256), (24, 0, 0), (24, 5, 7), (24, -8, 300), (3, -1, -2), (24, 300, -5),
         (1, 256, 0), (24, 192, 64), (24, 129, 127)]
cases = fixed[:]
while len(cases) < ncases:
    n = rng.randint(1, 24)
    from_ = rng.choice([rng.randint(-20, 256), rng.randint(0, 256), rng.choice([0, 256, 128, 192, 255])])
    to = rng.choice([rng.randint(-20, 256), rng.randint(0, 256), rng.choice([0, 256, 128, 192, 255])])
    cases.append((n, from_, to))

fails = 0
for i, (n, frm, to) in enumerate(cases):
    hi = rng.random() < 0.5
    pal = bytes(rng.randrange(64 if not hi else 256) for _ in range(PAL3))
    e.uc.mem_write(src, b"".join(pal[k * 3:k * 3 + 3] + b"\xAA" for k in range(256)))
    out.clear()
    e.call(rt(FADE), src, dst, n, frm, to)
    levels, cur = [], None
    for port, v in out:
        if port == 0x3C8:
            cur = []
            levels.append(cur)
        elif port == 0x3C9:
            if cur is None:
                break
            cur.append(v)
        else:
            cur = None
            break
    bad = []
    if any(len(l) != PAL3 for l in levels):
        bad.append(f"original wrote {[len(l) for l in levels]} bytes per level")
    got = (ctypes.c_ubyte * (PAL3 * MAX_LEVELS))()
    src3 = (ctypes.c_ubyte * PAL3).from_buffer_copy(pal)
    cnt = PORT.ja1f_fade(src3, n, frm, to, got, MAX_LEVELS)
    if cnt != len(levels):
        bad.append(f"level count original {len(levels)} port {cnt}")
    for k in range(min(cnt, len(levels))):
        o = bytes(levels[k])
        p = bytes(got[k * PAL3:(k + 1) * PAL3])
        if o != p:
            for j in range(PAL3):
                if o[j] != p[j]:
                    bad.append(f"level {k} byte {j}: original {o[j]} port {p[j]}")
                    break
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i} n {n} from {frm} to {to}: " + "; ".join(bad))
print(f"fade_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
