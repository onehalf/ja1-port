#!/usr/bin/env python3
"""Differential test: the second click of a confirm, WaitConfirmClick? 0x1C514, original vs port.

usage: .venv/bin/python validate/diff/confirmclick_diff.py [ncases=3000] [DUMP] [--lib SO]
The original runs in Unicorn. Stubs: WaitLeftUp, WaitRightUp, TacticalRedraw (logged), MouseQueueNext 0x6782C (the tape sets
the mouse place and buttons), KeyPressed + GetKey (the tape's key).
The port: cursor::confirm_poll + confirm_end (ja1f_confirm_click) and viewmode::tactical_tile. Compared: the result, the cursor tile
0x229D2C, the item timer 0x229DDC, the count of polls. A case: view place, map window of mode 1 or 3, the tile under the
mouse or next to it, tapes with idle polls, moves off, off-map places, both buttons, Escape and other keys.
On the original side only: TacticalRedraw(1) runs when g_redrawOff is set; the other keys are read and dropped.
Not compared: overview mode (the original uses this tile rule there too; click_diff has no case). click_diff gives the answer
to the cursor's other steps. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"confirmclick_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_confirm_click.argtypes = [ctypes.c_int] * 3 + [ctypes.c_void_p] + [ctypes.c_int] * 2 + [ctypes.c_void_p]
e = Emu(dump=dump, fast=True)
rt = e.rt
MX, MY, OX, OY, VX, VY, BUTTONS, CUR, TIMER, REDRAW_OFF = 0x229D00, 0x229D04, 0x229CB8, 0x229CC4, 0x229CF8, 0x229CFC, 0x229CF4, 0x229D2C, 0x229DDC, None
st = {}


def poll(em, *_):
    k = st["i"]
    st["i"] += 1
    if k >= len(st["tape"]):
        raise EmuStop("tape end")
    mx, my, b, key = st["tape"][k]
    e.w32(rt(MX), mx)
    e.w32(rt(MY), my)
    e.w8(rt(BUTTONS), b)
    st["key"] = key
    return 0


def key_read(em, *_):
    k = st["key"]
    st["key"] = 0
    st["keys_read"] += 1
    return k


def redraw(em, a, *_):
    st["redraws"].append(a)
    return 0


e.stub(rt(0x6782C), poll)
e.stub(rt(0x5616C), lambda em, *_: 1 if st["key"] else 0)
e.stub(rt(0x560F4), key_read)
e.stub(rt(0x67C54), lambda em, *_: 0)
e.stub(rt(0x67C68), lambda em, *_: 0)
e.stub(rt(0x176C8), redraw)
r = random.Random(1105)
bad = cases = 0
seen = {0: 0, 1: 0}
ends = {"escape": 0, "off": 0, "right": 0, "left": 0, "other_key": 0}
for c in range(ncases):
    ox, oy, cols, rows = r.choice(((48, 14, 14, 12), (0, 0, 20, 14)))
    c0, r0 = r.randrange(0, 43), r.randrange(0, 44)
    vx0, vy0 = r.randrange(16 * cols), r.randrange(14 * rows)
    tile = (r0 + vy0 // 14) * 56 + c0 + vx0 // 16

    def on_tile():
        return ox + (vx0 // 16) * 16 + r.randrange(16), oy + (vy0 // 14) * 14 + r.randrange(14)

    tape = []
    for _ in range(r.randrange(0, 4)):
        x, y = on_tile()
        tape.append((x, y, 0, r.choice((0, 0, 65, 13))))
    k = r.randrange(7)
    x, y = on_tile()
    if k == 0:
        tape.append((x, y, r.choice((0, 1, 2)), 27))
        ends["escape"] += 1
    elif k == 1:
        tape.append((x, y, 1, 0))
    elif k == 2:
        tape.append((x, y, 2, 0))
    elif k == 3:
        tape.append((x, y, 3, 0))
    elif k == 4:
        # The next tile: one pixel past an edge.
        dx, dy = r.choice(((-((x - ox) % 16) - 1, 0), (16 - (x - ox) % 16, 0), (0, -((y - oy) % 14) - 1), (0, 14 - (y - oy) % 14)))
        nx, ny = x + dx, y + dy
        if ox <= nx < ox + 16 * cols and oy <= ny < oy + 14 * rows:
            tape.append((nx, ny, r.choice((0, 1)), 0))
        else:
            tape.append((x, y, 1, 0))
    else:
        tape.append((x, y, 1 if k == 5 else 2, 0))
    window = []
    for mx, my, b, key in tape:
        window += [mx - ox, my - oy, b, key]
    e.w32(rt(OX), ox)
    e.w32(rt(OY), oy)
    e.w32(rt(VX), c0 * 16)
    e.w32(rt(VY), r0 * 14)
    e.w32(rt(TIMER), 77)
    e.w32(rt(CUR), 5)
    st.update(tape=tape, i=0, key=0, keys_read=0, redraws=[])
    try:
        got = e.call(rt(0x1C514), tile, max_steps=200_000) & 0xFF
    except EmuStop as x:
        print(f"case {c}: original stopped: {x}")
        bad += 1
        continue
    cases += 1
    seen[got] += 1
    arr = (ctypes.c_int * len(window))(*window)
    out = (ctypes.c_int * 4)()
    rc = PORT.ja1f_confirm_click(tile, c0, r0, arr, len(tape), 77, out)
    mine = (out[0], out[1], out[2], out[3])
    theirs = (got, e.r32(rt(CUR)), e.r32(rt(TIMER)), st["i"])
    if rc or mine != theirs:
        bad += 1
        if bad <= 15:
            print(f"case {c} tile {tile} view ({c0},{r0}) tape {tape}: original {theirs} port {mine}")
print(f"confirmclick_diff: results 0 x {seen[0]}, 1 x {seen[1]}")
if not seen[0] or not seen[1]:
    bad += 1
print(f"confirmclick_diff: {cases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
