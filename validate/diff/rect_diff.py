"""Differential test: RectVisible? 0x1FBDC (x0, y0, x1, y1 in EAX, EDX, EBX, ECX) -> 1 unless the rectangle lies
wholly outside the view (g_viewX 0x229CF8, g_viewY 0x229CFC, size 0x229CBC / 0x229CC0), original vs the port
(blast::rect_visible, which Explode uses) and the mirror tools/ai.py rect_visible.

usage: .venv/bin/python validate/diff/rect_diff.py [ncases] [DUMP]
Per case: fuzzed signed view position / size and rectangle corners (inverted and negative corners included), plus
the exact boundary values. The original runs in Unicorn on the dump (view globals written per case); the mirror
reads the same bytes. Exits 1 on any mismatch.
"""
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import ai                           # noqa: E402
import port_lib                     # noqa: E402

argv = sys.argv[1:]
n = int(argv[0]) if argv else 2000
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print("missing", dump)
    sys.exit(2)

RECT = 0x1FBDC
GX, GY, GW, GH = ai.G_VIEW_X, ai.G_VIEW_Y, ai.G_VIEW_W, ai.G_VIEW_H
e = Emu(dump=dump, fast=True)


class W:
    def g32(self, a):
        return e.r32(e.rt(a))


def s32(v):
    return v - 0x100000000 if v & 0x80000000 else v


w = W()
rng = random.Random(1)
fails = 0
for i in range(n):
    vx, vy = rng.randrange(-50, 700), rng.randrange(-50, 500)
    vw, vh = rng.randrange(1, 400), rng.randrange(1, 300)
    if i % 4 == 0:
        # exact edges: just inside and just outside each side of the view
        xs = [vx - 1, vx, vx + vw - 1, vx + vw]
        ys = [vy - 1, vy, vy + vh - 1, vy + vh]
        x0, x1 = rng.choice(xs), rng.choice(xs)
        y0, y1 = rng.choice(ys), rng.choice(ys)
    else:
        x0, x1 = rng.randrange(vx - 200, vx + vw + 200), rng.randrange(vx - 200, vx + vw + 200)
        y0, y1 = rng.randrange(vy - 200, vy + vh + 200), rng.randrange(vy - 200, vy + vh + 200)
    e.w32(e.rt(GX), vx & 0xFFFFFFFF)
    e.w32(e.rt(GY), vy & 0xFFFFFFFF)
    e.w32(e.rt(GW), vw & 0xFFFFFFFF)
    e.w32(e.rt(GH), vh & 0xFFFFFFFF)
    got = s32(e.call(e.rt(RECT), x0 & 0xFFFFFFFF, y0 & 0xFFFFFFFF, x1 & 0xFFFFFFFF, y1 & 0xFFFFFFFF))
    want = ai.rect_visible(w, x0, y0, x1, y1)
    port = port_lib.lib().ja1_rect_visible(vx, vy, vw, vh, x0, y0, x1, y1)
    if got != want or got != port:
        fails += 1
        if fails <= 8:
            print(f"MISMATCH view {vx},{vy} {vw}x{vh} rect {x0},{y0} {x1},{y1}: original {got} mirror {want} port {port}")
print(f"rect_diff: {n} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
