"""Differential test for TacticalKeyHandler's pixel-to-tile helper 0x159CC."""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path.insert(0, os.path.join(ROOT, "tools"))
from emu import Emu

n = int(sys.argv[1]) if len(sys.argv) > 1 else 2000
dump = os.path.join(ROOT, "build", "mem_tac60.bin")
lib = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
port = lib.ja1f_tactical_tile
port.argtypes = [ctypes.c_int] * 4
port.restype = ctypes.c_int
emu = Emu(dump=dump, fast=True)
LEFT, TOP, VIEW_X, VIEW_Y = 0x229CB8, 0x229CC4, 0x229CF8, 0x229CFC
emu.w32(emu.rt(LEFT), 48)
emu.w32(emu.rt(TOP), 14)
rng = random.Random(0x159CC)
fails = 0
for i in range(n):
    c0, r0 = rng.randrange(43), rng.randrange(45)
    # The mouse reaches the whole screen: with the view at (48, 14), x 0..319 and y 0..199 (no test of the place before
    # the calls 0x134C4 and 0x1444F).
    vx = [0, 15, 16, 223, 224, -1, -16, -17, -48, 271, rng.randrange(-48, 272)][i % 11]
    vy = [0, 13, 14, 167, 168, -1, -14, -15, 185, rng.randrange(-14, 186)][i % 10]
    emu.w32(emu.rt(VIEW_X), c0 * 16)
    emu.w32(emu.rt(VIEW_Y), r0 * 14)
    original = emu.call(emu.rt(0x159CC), 48 + vx, 14 + vy)
    original -= (original & 0x80000000) << 1             # EAX as a signed number
    actual = port(c0, r0, vx, vy)
    if original != actual:
        print(f"case {i}: view=({c0},{r0}) pixel=({vx},{vy}) original={original} port={actual}")
        fails += 1
        if fails == 10:
            break
if fails:
    sys.exit(f"viewtile_diff: {fails} mismatches")
print(f"viewtile_diff: {n} cases, 0 mismatches")
