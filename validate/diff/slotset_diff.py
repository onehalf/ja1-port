"""Differential test: SetSlotView 0x332E4 (panel.md "The switch view"), original vs port.

usage: .venv/bin/python validate/diff/slotset_diff.py [ncases=4000] [DUMP]

Per case: a scratch soldier record (fill 0xAA; slot byte +0x17A and view byte +0x17F set), the view byte in DL and
fuzzed g_redrawOff 0x92DDF. The original runs in Unicorn on tac60 at 0x332E4 with its five callees stubbed:
DrawSlotBack? 0x32E74 (slot AL, element DL), CopySlotToScreen 0x33204 (slot AL), BusyWait? 0x67250 (ms EAX),
DrawMercSlot? 0x32EB8 (slot AL, copy DL) and DrawSwitchView 0x330C0 (soldier EAX, copy DL). The test captures the
call log and the stored view byte +0x17F.

The port (ja1f_slot_view = panel::slot_view_calls, which runs panel::slot_view_frame, the function the window's
animation runs) gives the stored byte and its own call log. The test compares the log captured from the original
with the port's log, call by call: nothing about the order is written here (#39: the test built the expected log in
Python before, so it checked itself). Only the fixed arguments that the port does not hold are added: the slot, the
record pointer, and copy = 1 of the last draw. g_redrawOff >= 2: the original returns before it stores the byte (the
port's callers never ask in that mode: screen::switch_can_open uses the same panel::slot_view_runs). Every other
record byte must stay at the fill byte.

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
PORT.ja1f_slot_view.argtypes = [ctypes.c_uint32, ctypes.c_uint32, ctypes.POINTER(ctypes.c_ubyte),
                                ctypes.POINTER(ctypes.c_uint32), ctypes.c_int32]
PORT.ja1f_slot_view.restype = ctypes.c_int32

SETSLOTVIEW = 0x332E4
DRAW_BACK = 0x32E74
COPY_SLOT = 0x33204
BUSY_WAIT = 0x67250
DRAW_MERC_SLOT = 0x32EB8
DRAW_SWITCH = 0x330C0
REDRAW_OFF = 0x92DDF
REC = 512
FILL = 0xAA

if not os.path.exists(dump):
    print(f"slotset_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
rec = e.alloc(REC)
redraw = rt(REDRAW_OFF)

calls = []
e.stub(rt(DRAW_BACK), lambda em, ax, dx, bx, cx: (calls.append(("back", ax & 0xFF, dx & 0xFF)), 0)[1])
e.stub(rt(COPY_SLOT), lambda em, ax, dx, bx, cx: (calls.append(("copy", ax & 0xFF)), 0)[1])
e.stub(rt(BUSY_WAIT), lambda em, ax, dx, bx, cx: (calls.append(("wait", ax)), 0)[1])
e.stub(rt(DRAW_MERC_SLOT), lambda em, ax, dx, bx, cx: (calls.append(("slot", ax & 0xFF, dx & 0xFF)), 0)[1])
e.stub(rt(DRAW_SWITCH), lambda em, ax, dx, bx, cx: (calls.append(("switchview", ax, dx & 0xFF)), 0)[1])

rng = random.Random(int(os.environ.get("SLOTSET_SEED", "1")))
# every view byte with the two ends of the slot range and every redraw mode, then fuzz; the fuzz takes 0/1 often
# so that the open and the close paths both get many cases
cases = [(v, s, r) for v in range(256) for s in (0, 7) for r in (0, 1, 2, 3)]
while len(cases) < ncases:
    view = rng.choice((0, 1, 2, 3, 255, rng.randrange(256), rng.randrange(256)))
    off = rng.choice((0, 1, 2, 3, rng.randrange(256)))
    cases.append((view, rng.randrange(256), off))

fails = 0
for i, (view, slot, off) in enumerate(cases):
    e.uc.mem_write(rec, bytes([FILL]) * REC)
    e.w8(rec + 0x17A, slot)
    e.w8(rec + 0x17F, view)
    e.w8(redraw, off)
    pre = bytes(e.uc.mem_read(rec, REC))
    calls.clear()
    e.call(rt(SETSLOTVIEW), rec, view)
    got = list(calls)
    got_byte = e.r8(rec + 0x17F)
    post = bytes(e.uc.mem_read(rec, REC))

    stored = ctypes.c_ubyte()
    out = (ctypes.c_uint32 * 64)()
    n = PORT.ja1f_slot_view(view, off, ctypes.byref(stored), out, 32)
    want = []
    for j in range(max(n, 0)):
        code, arg = out[j * 2], out[j * 2 + 1]
        want.append({1: ("back", slot, arg), 2: ("copy", slot), 3: ("wait", arg), 4: ("slot", slot, 1),
                     5: ("switchview", rec, 1)}[code])
    want_byte = stored.value

    bad = []
    if n < 0:
        bad.append(f"port returned {n}")
    if got_byte != want_byte:
        bad.append(f"view byte original {got_byte} port {want_byte}")
    if got != want:
        bad.append(f"calls original {got} port {want}")
    changed = [j for j in range(REC) if post[j] != pre[j] and j != 0x17F]
    if changed:
        bad.append(f"record bytes changed after +0x17F: {changed[:8]}")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: view {view} slot {slot} redrawOff {off}: " + "; ".join(bad))
print(f"slotset_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
