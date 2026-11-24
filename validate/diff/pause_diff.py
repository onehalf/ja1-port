"""Differential test: Lock? 0x4B730 and Unlock? 0x4B764 (panel.md "Other users of Lock"), original vs port.

usage: .venv/bin/python validate/diff/pause_diff.py [ncases=4000] [DUMP]

Per case: fuzzed round_lock 0x92DE0, lock_owned 0x92DE2, clock switch 0x92DE5, blink byte 0x284F06 and blink timer
0x229D48, and one of the two functions: Lock (EAX = clock, 0 or 1) or Unlock (no argument). Both run in Unicorn on
tac60 and write only those five places. The port (port/ja1front ja1f_pause_op = pause::Pause::lock / unlock) gets the
same fields; after the call all five must equal the original's.

The blink byte and the clock switch are fuzzed 0/1: the original stores AL raw in the clock byte and the game's
callers pass 0/1 only; the blink byte is 1 at a lock and flips 0/1 at each blink step. round_lock and lock_owned are
fuzzed 0/1 plus other nonzero round_lock values (the original holds on any nonzero).

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
PORT.ja1f_pause_op.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.POINTER(ctypes.c_uint8),
                               ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_uint8),
                               ctypes.POINTER(ctypes.c_uint8), ctypes.POINTER(ctypes.c_int32)]
PORT.ja1f_pause_op.restype = ctypes.c_int32

LOCK, UNLOCK = 0x4B730, 0x4B764
ROUND_LOCK, LOCK_OWNED, CLOCK, BLINK, TIMER = 0x92DE0, 0x92DE2, 0x92DE5, 0x284F06, 0x229D48
FIELDS = (ROUND_LOCK, LOCK_OWNED, CLOCK, BLINK)

if not os.path.exists(dump):
    print(f"pause_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt


def state():
    return (e.r8(rt(ROUND_LOCK)), e.r8(rt(LOCK_OWNED)), e.r8(rt(CLOCK)), e.r8(rt(BLINK)),
            e.r32(rt(TIMER)) - (1 << 32) if e.r32(rt(TIMER)) >> 31 else e.r32(rt(TIMER)))


def put(st):
    for a, v in zip(FIELDS, st[:4]):
        e.w8(rt(a), v)
    e.w32(rt(TIMER), st[4] & 0xFFFFFFFF)


rng = random.Random(int(os.environ.get("PAUSE_SEED", "1")))
fails, by_op = 0, {LOCK: 0, UNLOCK: 0}
for i in range(ncases):
    unlock = i % 2 == 1
    st = (rng.choice([0, 0, 1, rng.randrange(2, 256)]), rng.choice([0, 1]), rng.choice([0, 1, 1]),
          rng.choice([0, 1, 1]), rng.choice([0, 800, 400, rng.randrange(-2000, 2000)]))
    clock = st[2]
    put(st)
    if unlock:
        e.call(rt(UNLOCK))
    else:
        e.call(rt(LOCK), clock)
    got = state()
    by_op[UNLOCK if unlock else LOCK] += 1
    rl, co, lo, bl, tm = (ctypes.c_uint8(st[0]), ctypes.c_uint8(st[2]), ctypes.c_uint8(st[1]),
                          ctypes.c_uint8(st[3]), ctypes.c_int32(st[4]))
    PORT.ja1f_pause_op(1 if unlock else 0, clock, ctypes.byref(rl), ctypes.byref(co), ctypes.byref(lo),
                       ctypes.byref(bl), ctypes.byref(tm))
    want = (rl.value, lo.value, co.value, bl.value, tm.value)
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: {'unlock' if unlock else 'lock'} from {st}: "
                  f"(round_lock, lock_owned, clock, blink, timer) original {got} port {want}")
# Pause? 0x1002C with the real Lock and Unlock: the redraw 0x1CB78 is logged, the key flush and the two waits for a
# mouse button are stubs (the window has no such wait for a key press).
PAUSED, PAUSE = 0x22A129, 0x1002C
redraws = []
e.stub(rt(0x1CB78), lambda em, *_: redraws.append(1) or 0)
for a in (0x67F90, 0x67C54, 0x67C68):
    e.stub(rt(a), lambda em, *_: 0)
PORT.ja1f_pause_set.argtypes = [ctypes.c_int32] + [ctypes.POINTER(ctypes.c_uint8)] * 5 + [ctypes.POINTER(ctypes.c_int32)]
PORT.ja1f_pause_set.restype = ctypes.c_int32
for i in range(ncases):
    on, was = rng.choice([0, 1]), rng.choice([0, 1])
    st = (rng.choice([0, 0, 1, rng.randrange(2, 256)]), rng.choice([0, 1]), rng.choice([0, 1, 1]),
          rng.choice([0, 1, 1]), rng.choice([0, 800, 400, rng.randrange(-2000, 2000)]))
    put(st)
    e.w8(rt(PAUSED), was)
    redraws.clear()
    e.call(rt(PAUSE), on)
    got = state() + (e.r8(rt(PAUSED)), len(redraws))
    pa, rl, co, lo, bl, tm = (ctypes.c_uint8(was), ctypes.c_uint8(st[0]), ctypes.c_uint8(st[2]), ctypes.c_uint8(st[1]),
                              ctypes.c_uint8(st[3]), ctypes.c_int32(st[4]))
    r_ = PORT.ja1f_pause_set(on, ctypes.byref(pa), ctypes.byref(rl), ctypes.byref(co), ctypes.byref(lo),
                             ctypes.byref(bl), ctypes.byref(tm))
    want = (rl.value, lo.value, co.value, bl.value, tm.value, pa.value, r_)
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH pause case {i}: on {on} paused {was} from {st}: original {got} port {want}")
print(f"pause_diff: {ncases} cases (lock {by_op[LOCK]}, unlock {by_op[UNLOCK]}); {fails} mismatches")
sys.exit(1 if fails else 0)
