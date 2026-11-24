"""Differential test for QueueTwoAnims 0x17F5C (op 181 knockdown_anim): the original in Unicorn vs the port's
ja1_knockdown_anim (engine: cur into the animation queue, animIdx + 1, next, +0x1FE = 0).

usage: .venv/bin/python validate/diff/qanim_diff.py [DUMP] [n=2000]
Compares the animation queue 0x1E8..0x200 of the soldier record afterwards. The function makes no calls.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

QUEUE_TWO_ANIMS = 0x17F5C
args = [a for a in sys.argv[1:] if not a.isdigit()]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["2000"])[0])
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
e = Emu(dump=dump, fast=True)
bufs, scalars = port_lib.world_load(e, ai)
soldiers = bufs["soldiers"]
rng = random.Random(0x17F5C)
fails = 0
for i in range(n):
    slot = rng.randrange(0x30)
    base = 0x200 * slot
    rec = bytearray(rng.randrange(256) for _ in range(0x200))
    for k in range(10):                       # the queue 0x1E8..0x1FA and the bytes around it
        ai.w16(rec, 0x1E8 + 2 * k, rng.randrange(0x10000))
    idx = rng.choice([0, 0, 1, 2, 3, 4, 0xFF])
    rec[0x1F3] = idx
    rec[0x1FE] = rng.choice([0, 0, 1, 0xFF])
    nxt, cur = rng.randrange(0x10000), rng.randrange(0x10000)
    # the original
    p = e.rt(ai.G_SOLDIERS) + base
    e.uc.mem_write(p, bytes(rec))
    e.call(e.rt(QUEUE_TWO_ANIMS), p, nxt, cur)
    got = bytes(e.uc.mem_read(p + 0x1E8, 0x18))
    # the port
    soldiers[base:base + 0x200] = rec
    port_lib.ai_run("knockdown_anim", bufs, scalars, slot, lambda k: 0, lambda op, a: (0, None), (nxt, cur))
    want = bytes(soldiers[base + 0x1E8:base + 0x200])
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i} slot {slot} idx {idx}: original {got.hex()} port {want.hex()}")
print(f"qanim_diff: {n} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
