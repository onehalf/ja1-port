"""Differential test: OverviewTileAtMouse 0x24488 (the overview tile under the mouse), original vs port.

usage: .venv/bin/python validate/diff/vtile_diff.py [ncases=2000] [DUMP]

Per case: an overview screen point (x, y), x 48..271 and y 14..181 (the 56 x 56 map is 4 x 3 pixels per tile at
(48, 14)). The original runs in Unicorn on tac60 at 0x24488 with the mouse position globals 0x229D00 / 0x229D04
written first; it returns 56 * ((y - 14) / 3) + (x - 48) / 4. The port (ja1f_overview_tile =
ja1front::viewmode::overview_tile, panel.md "The overview mouse step" step 2) must return the same tile.
The original has no upper clamp; the port clamps to tile 3135. The caller only passes points inside the overview,
so the cases stay inside it.

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
ncases = int(argv[0]) if argv and argv[0].isdigit() else 2000
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")

PORT = ctypes.CDLL(LIB)
PORT.ja1f_overview_tile.argtypes = [ctypes.c_int32, ctypes.c_int32]
PORT.ja1f_overview_tile.restype = ctypes.c_int32

TILE = 0x24488
MX, MY = 0x229D00, 0x229D04
LEFT, TOP, WIDTH, HEIGHT = 48, 14, 224, 168

if not os.path.exists(dump):
    print(f"vtile_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt

rng = random.Random(int(os.environ.get("VTILE_SEED", "1")))
# every tile origin and its last pixel, then random points of the overview
cases = [(LEFT + 4 * c + dx, TOP + 3 * r + dy) for r in range(HEIGHT // 3) for c in range(WIDTH // 4)
         for (dx, dy) in ((0, 0), (3, 2), (c % 4, r % 3))]
while len(cases) < ncases:
    cases.append((LEFT + rng.randrange(WIDTH), TOP + rng.randrange(HEIGHT)))

fails = 0
for i, (x, y) in enumerate(cases):
    e.uc.mem_write(rt(MX), x.to_bytes(4, "little"))
    e.uc.mem_write(rt(MY), y.to_bytes(4, "little"))
    want = e.call(rt(TILE))
    got = PORT.ja1f_overview_tile(x, y)
    if want != got:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: ({x}, {y}) original {want} port {got}")
print(f"vtile_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
