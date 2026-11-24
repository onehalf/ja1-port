"""Differential test: ClearRoomFlags 0x61744 (the Santino room flags, AI op 327), original vs port.

usage: .venv/bin/python validate/diff/clearrooms_diff.py [ncases=50] [DUMP]

Per case: the tactical dump's tile map (3136 x 8 bytes at 0x100A18), either as saved (case 0) or filled with one
random byte (the other cases). The original runs in Unicorn at 0x61744 and must clear tile byte 3 at the two
rectangles (tiles 232 + 56 x row + col for 7 rows x 6 columns, and 625 + 56 x row + col for 3 rows x 5). The port
(ja1f_clear_rooms = sightings::clear_rooms_at, the AI op 327 on the world) runs on a copy of the same bytes; every
tile-map byte must match. The test also needs at least one changed byte per call (a fill of zeros would hide an
over-clear).

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
LIB = os.path.join(ROOT, "port", "ja1logic", "target", "release", "libja1logic.so")
ncases = int(argv[0]) if argv and argv[0].isdigit() else 50
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")

LIBH = ctypes.CDLL(LIB)
LIBH.ja1f_clear_rooms.argtypes = [ctypes.POINTER(ctypes.c_uint8)]
LIBH.ja1f_clear_rooms.restype = None

CLEAR, TILES, N = 0x61744, 0x100A18, 3136 * 8

if not os.path.exists(dump):
    print(f"clearrooms_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
AT = rt(TILES)                       # the dump holds the image at a load delta
base = bytes(e.uc.mem_read(AT, N))

rng = random.Random(int(os.environ.get("CLEARROOMS_SEED", "1")))
fails = 0
for i in range(ncases):
    if i == 0:
        before = base
    else:
        before = bytes([rng.randrange(256)]) * N
    e.uc.mem_write(AT, before)
    e.call(rt(CLEAR))
    got = bytes(e.uc.mem_read(AT, N))
    buf = (ctypes.c_uint8 * N).from_buffer_copy(before)
    LIBH.ja1f_clear_rooms(buf)
    want = bytes(buf)
    bad = []
    if got != want:
        k = next(k for k in range(N) if got[k] != want[k])
        bad.append(f"tile map differs at {k:#x}: original {got[k]:#04x} port {want[k]:#04x}")
    if got == before and i > 0:
        bad.append("no byte changed (the fill hides the rectangles)")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: " + "; ".join(bad))
print(f"clearrooms_diff: {ncases} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
