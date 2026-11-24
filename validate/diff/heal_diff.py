"""Differential test for the Alt+L debug key body (cursor.md "Debug keys (level 4)" 2, heal): the original's arm of
TacticalKeyHandler 0x13950..0x1399C in Unicorn vs the port's ja1_debug_heal (port/ja1logic turns.rs).

usage: .venv/bin/python validate/diff/heal_diff.py [DUMP] [n=600]
The arm runs from 0x13950 (behind the level-4 test) with ESI = record S and DL = 0 until it jumps to the common tail
0x1409B: +0x183 = +0x184, +0x185 = +0x186 = 0, Rest? 0x32754 (S, fatigue i16 +0x10C, 1; real, its DrawEnergyBar?
0x32610 = op energy_bar and CopySlotToScreen 0x33204 = op w_33204 stubbed and logged), MercMaxAP 0x56210 (real) into
+0x1A5, then RedrawMercSlot? 0x331A4 (+0x17A, 1) (stubbed: the port's caller redraws the strip).
Compared: all 48 soldier records behind the call, the host calls of Rest in order, and the one RedrawMercSlot? call.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu, RET_SENTINEL    # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

ARM, TAIL = 0x13950, 0x1409B
ENERGY, PANEL, REDRAW = 0x32610, 0x33204, 0x331A4
args = [a for a in sys.argv[1:] if not a.isdigit()]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["600"])[0])
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
e = Emu(dump=dump, fast=True)
bufs, scalars = port_lib.world_load(e, ai)
soldiers = bufs["soldiers"]
base_recs = bytes(soldiers)
sold_addr, sel_addr = e.rt(ai.G_SOLDIERS), e.rt(ai.G_SELECTED)
rng = random.Random(0x13950)
log = []
e.stub(e.rt(ENERGY), lambda em, eax, edx, *a: log.append(("energy_bar", (eax - sold_addr) // 512, edx & 0xFF)) or 0)
e.stub(e.rt(PANEL), lambda em, eax, *a: log.append(("w_33204", eax & 0xFF)) or 0)
redraws = []
e.stub(e.rt(REDRAW), lambda em, eax, edx, *a: redraws.append((eax & 0xFF, edx & 0xFF)) or 0)


def tail(em, *a):                    # the arm ends at the common tail: return to the sentinel
    em.w32(em.uc.reg_read(UC_X86_REG_ESP), RET_SENTINEL)
    return 0


e.stub(e.rt(TAIL), tail)
# the trampoline reads S from data (Unicorn keeps translated code: a rewritten trampoline would run stale), DL = 0
targs = e.alloc(4)
tramp = e.alloc(b"\x8B\x35" + struct.pack("<I", targs) + b"\x31\xD2"
                + b"\xB8" + struct.pack("<I", e.rt(ARM)) + b"\xFF\xE0")
fails = 0
for i in range(n):
    slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(48)])
    recs = bytearray(base_recs)
    r = slot * 0x200
    recs[r + 0x17A] = rng.choice([slot, rng.randrange(8)])
    for off in (0x183, 0x184, 0x185, 0x186, 0x18A, 0x1BA, 0x1A5, 0x10C, 0x10D):
        recs[r + off] = rng.randrange(256)
    if rng.random() < 0.5:
        recs[r + 0x10C:r + 0x10E] = struct.pack("<h", rng.choice([0, 50, 99, 100, 101, 5000, 9999, 10000, 10001, -1, -100]))
    for off in rng.sample(range(0x100, 0x200), 12):  # other bytes MercMaxAP may read
        recs[r + off] = rng.randrange(256)
    rec_addr = sold_addr + r
    # the original
    e.uc.mem_write(sold_addr, bytes(recs))
    e.w32(targs, rec_addr)
    log.clear()
    redraws.clear()
    e.call(tramp)
    got_recs = bytes(e.uc.mem_read(sold_addr, len(recs)))
    got_log, got_redraws = list(log), list(redraws)
    # the port
    soldiers[:] = recs
    pcalls = []

    def call(op, a):
        pcalls.append((op, a[0], a[1] & 0xFF) if op == "energy_bar" else (op, a[0] & 0xFF))
        return 0, None
    port_lib.ai_run("debug_heal", bufs, scalars, slot, lambda k: 0, call)
    bad = []
    if got_recs != bytes(soldiers):
        bad.append("record " + " ".join(hex(k % 0x200) + "@" + str(k // 0x200) + f"={got_recs[k]:#x}/{soldiers[k]:#x}"
                                        for k in range(len(recs)) if got_recs[k] != soldiers[k])[:160])
    if got_log != pcalls:
        bad.append(f"calls {got_log} / port {pcalls}")
    if got_redraws != [(recs[r + 0x17A], 1)]:
        bad.append(f"RedrawMercSlot? {got_redraws}")
    if bad:
        fails += 1
        if fails <= 8:
            print(f"MISMATCH case {i} slot {slot}: {'; '.join(bad)}")
print(f"heal_diff: {n} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
