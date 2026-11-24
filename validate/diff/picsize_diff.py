"""Differential test: PictureSize? 0x46AE4 (an item picture's size class), original vs port.

usage: .venv/bin/python validate/diff/picsize_diff.py [ncases=4000] [DUMP]
Per case the original runs in Unicorn on tac60: EAX = picture id (masked to a byte), EDX = width out, EBX = offset
out. id <= 20 -> 42 / 0, <= 90 -> 30 / 6, else 16 / 13 (the width and x offset of the picture in the 42-column
cursor buffer, used when a held item is drawn). The port (ja1f_picture_size = invdraw::picture_size) must give the
same width and offset for the same id. Exits 1 on any mismatch.
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
dump = next((a for a in argv if not a.isdigit()), os.path.join(ROOT, "build", "mem_tac60.bin"))

PORT = ctypes.CDLL(LIB)
PORT.ja1f_picture_size.argtypes = [ctypes.c_int32]
PORT.ja1f_picture_size.restype = ctypes.c_int32

SIZE = 0x46AE4

if not os.path.exists(dump):
    print(f"picsize_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
w_out = e.alloc(4)
dx_out = e.alloc(4)

rng = random.Random(int(os.environ.get("PICSIZE_SEED", "1")))
# every byte id, then fuzz (the original masks EAX to a byte)
cases = list(range(256))
while len(cases) < ncases:
    cases.append(rng.randrange(0, 1 << 31))

fails = 0
for i, pic in enumerate(cases):
    e.uc.mem_write(w_out, b"\0\0\0\0")
    e.uc.mem_write(dx_out, b"\0\0\0\0")
    e.call(rt(SIZE), pic, w_out, dx_out)
    w = int.from_bytes(bytes(e.uc.mem_read(w_out, 4)), "little")
    dx = int.from_bytes(bytes(e.uc.mem_read(dx_out, 4)), "little")
    got = PORT.ja1f_picture_size(pic)
    want = ((w & 0xFF) << 8) | (dx & 0xFF)
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: pic {pic:#x} original width {w} offset {dx}, port {got >> 8} / {got & 0xFF}")
print(f"picsize_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
