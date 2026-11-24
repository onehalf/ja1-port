"""Differential test: SetViewPose? 0x2B6A0 (the soldier view pose after the inventory closes), original vs port.

usage: .venv/bin/python validate/diff/viewpose_diff.py [ncases=4000] [DUMP]

Per case: a scratch soldier record with fuzzed level byte +0x192 and item 3 type +0x24 (the whole 0..255 range), and
a fill byte over the record. The original runs in Unicorn on tac60 at 0x2B6A0 (EAX = the record) and must write only
+0x19E: 7 when +0x192 <= 2, else 8; +1 when +0x24 = 0x27. The port (ja1f_view_pose = ja1logic sector::view_pose, the
rule soldier_start uses) must give the same byte, and the rest of the record must stay as it was.

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
PORT.ja1f_view_pose.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
PORT.ja1f_view_pose.restype = ctypes.c_int32

SETPOSE, LEVEL, ITEM3, POSE = 0x2B6A0, 0x192, 0x24, 0x19E
REC = 0x200

if not os.path.exists(dump):
    print(f"viewpose_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
rec = e.alloc(REC)

rng = random.Random(int(os.environ.get("VIEWPOSE_SEED", "1")))
fails = 0
fix = [(0, 0), (2, 0), (3, 0), (0, 0x27), (2, 0x27), (3, 0x27), (255, 0x27), (255, 0)]
cases = fix + [(rng.randrange(256), rng.randrange(256)) for _ in range(max(0, ncases - len(fix)))]
for i, (level, item3) in enumerate(cases):
    fill = rng.randrange(256)
    e.uc.mem_write(rec, bytes([fill]) * REC)
    e.w8(rec + LEVEL, level)
    e.w8(rec + ITEM3, item3)
    pre = bytes(e.uc.mem_read(rec, REC))
    e.call(rt(SETPOSE), rec)
    got = e.r8(rec + POSE)
    want = PORT.ja1f_view_pose(level, item3)
    bad = []
    if got != want:
        bad.append(f"pose original {got} port {want}")
    img = bytes(e.uc.mem_read(rec, REC))
    if any(v != pre[k] for k, v in enumerate(img) if k != POSE):
        bad.append("another byte of the record changed")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: level {level} item3 {item3}: " + "; ".join(bad))
print(f"viewpose_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
