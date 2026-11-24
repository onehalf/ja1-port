"""Differential test: OnScreen 0x1FC34 ("is this soldier's picture in the view", select.md), original vs port.

usage: .venv/bin/python validate/diff/onscreen_diff.py [ncases=3000] [DUMP] [--port]
Per case: a fuzzed soldier record (tile +0x114, picture x +0x116, y +0x118, x offset +0x11A, y offset +0x11C, all
signed words), the option 0x92DDF (opt_onscreen, 0 = always on screen) and the view rectangle x/y/w/h
(0x229CF8 / 0x229CFC / 0x229CBC / 0x229CC0). Half the cases put the point (x - offset) on a corner of the 20-pixel
margin on purpose, with offsets that are not 0, so the four comparisons go both ways and each offset decides the
result. (#39: with offset 0 in the corner cases and fully random points otherwise, dropping an offset stayed green.) The original runs OnScreen 0x1FC34 in Unicorn on tac60;
the port is ja1_on_screen (anim.rs on_screen) through port_lib. Compared: the result. Without --port only the
original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import os
import random
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import ai                            # noqa: E402
from port_lib import world_load, sector_run   # noqa: E402

argv = sys.argv[1:]
args = [a for a in argv if not a.startswith("-")]
ncases = int(args[0]) if args and args[0].isdigit() else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv

OPT = 0x92DDF
VIEW = (ai.G_VIEW_X, ai.G_VIEW_Y, ai.G_VIEW_W, ai.G_VIEW_H)
POINT = (0x114, 0x116, 0x118, 0x11A, 0x11C)

e = Emu(dump=dump, fast=True)
rt = e.rt
rng = random.Random(0x1FC34)
fails = 0


def i16(v):
    return struct.pack("<h", ((v + 0x8000) & 0xFFFF) - 0x8000)


def i32(v):
    return struct.pack("<i", v)


for i in range(ncases):
    slot = rng.randrange(0x30) if i % 5 else i % 0x30
    base = rt(ai.G_SOLDIERS) + 0x200 * slot
    vx, vy = rng.choice([0, -100, 200, 1000, rng.randrange(-3000, 3000)]), \
        rng.choice([0, -100, 200, 1000, rng.randrange(-3000, 3000)])
    vw, vh = rng.choice([0, 1, 50, 320, 2000, rng.randrange(-3000, 3000)]), \
        rng.choice([0, 1, 50, 200, 2000, rng.randrange(-3000, 3000)])
    tile = rng.choice([0, 1, 55, 56, 57, 0x37FF, 0x8000, 0xFFFF, rng.randrange(-0x8000, 0x8000)])
    if i % 2:
        # a picture point on a corner of the 20-pixel margin (delta -21..+1 from the view edge). The offsets are
        # not 0: the point is what the original compares (x - dx, and y - dy from tile 56 up), and it must be in
        # the view on BOTH axes, or a wrong offset on one axis does not change the result. Half of these cases
        # keep the stored y on the corner with the tile below 56, where the original does not use +0x11C.
        dx = rng.choice([0, 1, 20, 21, -20, 300, -300, rng.randrange(-2000, 2000)])
        dy = rng.choice([0, 1, 20, 21, -20, 300, -300, rng.randrange(-2000, 2000)])
        tile = rng.choice([0, 55, 56, 57, 0x37FF, rng.randrange(0, 0x3800)])
        x = vx + rng.choice(list(range(-21, 2)) + [vw - 1, vw, vw + 1, -21, 0]) + dx
        y = vy + rng.choice(list(range(-21, 2)) + [vh - 1, vh, vh + 1, -21, 0]) + (dy if tile >= 56 else 0)
    else:
        x = rng.randrange(-0x8000, 0x8000)
        y = rng.randrange(-0x8000, 0x8000)
        dx = rng.choice([0, 1, 20, 0x7FFF, -0x8000, rng.randrange(-0x8000, 0x8000)])
        dy = rng.choice([0, 1, 20, 0x7FFF, -0x8000, rng.randrange(-0x8000, 0x8000)])
    e.uc.mem_write(base + 0x114, i16(tile) + i16(x) + i16(y) + i16(dx) + i16(dy))
    e.w8(rt(OPT), rng.choice([0, 1, 2, 3, 255]))
    for a, v in zip(VIEW, (vx, vy, vw, vh)):
        e.uc.mem_write(rt(a), i32(v))
    orig = e.call(rt(0x1FC34), base, max_steps=20_000) & 0xFF
    if not PORT:
        continue
    bufs, scalars = world_load(e, ai)
    got = sector_run("on_screen", bufs, scalars, [slot], lambda n: 0) & 0xFF
    if orig != got:
        fails += 1
        if fails <= 10:
            print(f"case {i}: slot {slot} tile {tile} ({x},{y}) d({dx},{dy}) view ({vx},{vy},{vw},{vh}) "
                  f"opt {scalars['opt_onscreen']}: original {orig}, port {got}")
if fails:
    sys.exit(f"onscreen_diff: {fails} mismatches")
print(f"onscreen_diff{'' if PORT else ' (original only)'}: {ncases} cases, 0 mismatches")
