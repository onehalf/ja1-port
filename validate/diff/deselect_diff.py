"""Differential test for DeselectSoldier 0x17F10: the original in Unicorn vs the port's ja1_deselect.

usage: .venv/bin/python validate/diff/deselect_diff.py [DUMP] [n=600]
Reads right behind the call: the selected byte, all 48 soldier records, and the order and arguments of the two calls
(FaceLight 0x46A4C = op 195, RedrawMercSlot 0x331A4 = op 149, both stubbed). The longer callers (merc_dies, EndTurn)
write `selected` again behind the call, so they cannot see its value.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

DESELECT, FACE_LIGHT, REDRAW_SLOT = 0x17F10, 0x46A4C, 0x331A4
args = [a for a in sys.argv[1:] if not a.isdigit()]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["600"])[0])
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
e = Emu(dump=dump, fast=True)
bufs, scalars = port_lib.world_load(e, ai)
soldiers = bufs["soldiers"]
rng = random.Random(0x17F10)
log = []
e.stub(e.rt(FACE_LIGHT), lambda em, eax, edx, ebx, ecx: log.append((195, eax & 0xFF, edx & 0xFF)) or 0)
e.stub(e.rt(REDRAW_SLOT), lambda em, eax, edx, ebx, ecx: log.append((149, eax & 0xFF, edx & 0xFF)) or 0)
sold_addr = e.rt(ai.G_SOLDIERS)
sel_addr = e.rt(ai.G_SELECTED)
fails = done = 0
for i in range(n):
    sel = rng.choice([rng.randrange(8), rng.randrange(8), 8, 12, 0x28, 0x2F, 0x40, 0x7F, 0x80, 0xFF])
    show = rng.choice([0, 1, 1, 0xFF])
    recs = bytes(rng.randrange(256) for _ in range(0x200 * 8))
    # the original
    e.uc.mem_write(sold_addr, recs)
    e.uc.mem_write(sel_addr, bytes([sel]))
    log.clear()
    e.call(e.rt(DESELECT), show)
    got = (e.r8(sel_addr), bytes(e.uc.mem_read(sold_addr, len(recs))), list(log))
    # the port
    soldiers[:len(recs)] = recs
    scalars["selected"] = sel
    calls = []

    def call(op, a):
        calls.append(({"w_46a4c": 195, "w_331a4": 149}[op], a[0] & 0xFF, a[1] & 0xFF))
        return 0, None
    port_lib.ai_run("deselect", bufs, scalars, 0, lambda k: 0, call, show if show < 0x80 else show - 0x100)
    want = (scalars["selected"], bytes(soldiers[:len(recs)]), calls)
    done += sel < 8
    if got != want:
        fails += 1
        if fails <= 10:
            d = [hex(k) for k in range(len(recs)) if got[1][k] != want[1][k]][:6]
            print(f"MISMATCH case {i} selected {sel:#x} show {show}: selected {got[0]:#x}/{want[0]:#x} record {d} "
                  f"calls {got[2]} / {want[2]}")
assert done >= 100, done
print(f"deselect_diff: {n} cases ({done} with a selected merc), {fails} mismatches")
sys.exit(1 if fails else 0)
