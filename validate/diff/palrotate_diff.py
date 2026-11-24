"""Differential test: PaletteRotate? 0x528AC (the palette cycle), original vs port.

usage: .venv/bin/python validate/diff/palrotate_diff.py [ncases=4000] [DUMP]

Per case: a 1024-byte VGA palette (256 entries of 4 bytes, random) and a first / last entry pair, 0 <= first <=
last <= 255. The original runs in Unicorn on tac60 at 0x528AC (EAX palette, EDX first, EBX last): the entry at
first goes to last, every entry first+1..last moves down one (4-byte entries). The port (ja1f_pal_rotate =
ja1front::eod::rotate, op 206 pal_cycle) keeps 3-byte RGB entries; it must rotate the same visible bytes.
The original has no bounds check and an inverted range overwrites last with the first entry; the port ignores an
empty or inverted range. The game only passes ascending ranges, so the cases stay inside 0 <= first <= last <= 255.

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
PORT.ja1f_pal_rotate.argtypes = [ctypes.POINTER(ctypes.c_ubyte), ctypes.c_int32, ctypes.c_int32]
PORT.ja1f_pal_rotate.restype = None

ROTATE = 0x528AC
PAL4, PAL3 = 1024, 768

if not os.path.exists(dump):
    print(f"palrotate_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
buf = e.alloc(PAL4)

rng = random.Random(int(os.environ.get("PALROTATE_SEED", "1")))
fixed = [(f, l) for (f, l) in
         [(0, 1), (0, 255), (1, 255), (0, 0), (255, 255), (254, 255), (3, 254), (0xAC, 0xB2),
          (0xA5, 0xAB), (0xD7, 0xDA), (0xDB, 0xDE), (0xE1, 0xEF), (2, 5), (0x3C, 0xFF), (0xFE, 0xFE)]
         if f <= l]
cases = fixed[:]
while len(cases) < ncases:
    f = rng.randrange(256)
    l = rng.randrange(f, 256)
    cases.append((f, l))

fails = 0
for i, (first, last) in enumerate(cases):
    raw = bytes(rng.randrange(256) for _ in range(PAL4))
    e.uc.mem_write(buf, raw)
    pre = raw
    e.call(rt(ROTATE), buf, first, last)
    post = bytes(e.uc.mem_read(buf, PAL4))
    p3 = bytes(v for k in range(256) for v in raw[k * 4:k * 4 + 3])
    out = (ctypes.c_ubyte * PAL3).from_buffer_copy(p3)
    PORT.ja1f_pal_rotate(out, first, last)
    got = bytes(out)
    bad = []
    if got != bytes(v for k in range(256) for v in post[k * 4:k * 4 + 3]):
        for k in range(256):
            if got[k * 3:k * 3 + 3] != post[k * 4:k * 4 + 3]:
                bad.append(f"entry {k}: original {list(post[k * 4:k * 4 + 3])} port {list(got[k * 3:k * 3 + 3])}")
                if len(bad) >= 3:
                    break
    if post[last * 4:last * 4 + 4] != pre[first * 4:first * 4 + 4]:
        bad.append("original did not put the saved first entry at last")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i} first {first} last {last}: " + "; ".join(bad))
# The island map's water (mapui::rotate_water, ja1f_map_water): the original's two calls (0xD7, 0xDA), (0xDB, 0xDE)
# (strategic.md "Water on the island map"), then the port's one call, on the same palettes; 3 rotations per case.
PORT.ja1f_map_water.argtypes = [ctypes.POINTER(ctypes.c_ubyte)]
PORT.ja1f_map_water.restype = None
nwater = max(50, ncases // 20)
for i in range(nwater):
    raw = bytes(rng.randrange(256) for _ in range(PAL4))
    e.uc.mem_write(buf, raw)
    out = (ctypes.c_ubyte * PAL3).from_buffer_copy(bytes(v for k in range(256) for v in raw[k * 4:k * 4 + 3]))
    for _ in range(3):
        e.call(rt(ROTATE), buf, 0xD7, 0xDA)
        e.call(rt(ROTATE), buf, 0xDB, 0xDE)
        PORT.ja1f_map_water(out)
    post = bytes(e.uc.mem_read(buf, PAL4))
    if bytes(out) != bytes(v for k in range(256) for v in post[k * 4:k * 4 + 3]):
        fails += 1
        if fails <= 10:
            k = next(k for k in range(256) if bytes(out)[k * 3:k * 3 + 3] != post[k * 4:k * 4 + 3])
            print(f"MISMATCH water case {i}: entry {k:#x} original {list(post[k * 4:k * 4 + 3])} port {list(bytes(out)[k * 3:k * 3 + 3])}")
print(f"palrotate_diff: {len(cases)} cases + {nwater} map water cases; {fails} mismatches")
sys.exit(1 if fails else 0)
