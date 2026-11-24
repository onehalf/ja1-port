#!/usr/bin/env python3
"""Differential test: the top-row rule of ShowMouse? 0x673AC and MousePoll? 0x678E8, original vs port.

usage: .venv/bin/python validate/diff/pointer_diff.py [ncases=600] [DUMP] [--lib SO]
Per case: mouse y, hot spot y 0x229C48, flag 0x92E4F, and the old hide byte 0x92E70. The original runs in Unicorn with the
host calls stubbed: MouseQueuePop? 0x68470 (gives the new position), DrawMousePointer? 0x67A28, MouseRestoreUnder? 0x67848.
ShowMouse runs with the pointer off and the screen ready; MousePoll runs with the pointer on and a moved mouse.
The port: pointer::top_row_hidden (ja1f_top_row_hidden). Compared: the original draws the pointer exactly when the port does
not hide it. A second part runs the rest of MousePoll?: see the comment there. Original side only: the hide byte after the call, and the off byte 0x284F0B after ShowMouse. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 600
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"pointer_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_top_row_hidden.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_uint8]
PORT.ja1f_top_row_hidden.restype = ctypes.c_int32
OFF, READY, HIDE, FLAG, MX, MY, HOTX, HOTY, BUTTONS, SHAPE, LOCK = (0x284F0B, 0x92DCC, 0x92E70, 0x92E4F, 0x229D00, 0x229D04,
                                                                  0x229C70, 0x229C48, 0x229CF4, 0x229C54, 0x229C34)
e = Emu(dump=dump, fast=True)
log = []
pos = [0, 0]


def pop(em, eax, edx, ebx, *_):
    """MouseQueuePop?(&x, &y, &buttons): the new position, no button."""
    em.w32(eax, pos[0])
    em.w32(edx, pos[1])
    em.w32(ebx, 0)
    return 0


e.stub(e.rt(0x68470), pop)
e.stub(e.rt(0x67A28), lambda em, a, d, *_: log.append(("draw", a, d)) or 0)
e.stub(e.rt(0x67848), lambda em, *_: log.append(("restore",)) or 0)
r = random.Random(673)
EDGE = [-9, -8, -1, 0, 1, 7, 8, 14, 15, 16, 17, 23, 24, 25, 40, 199]
HOTS = [0, 0, 0, -8]                     # SetMouseShape?: 0 for the arrows, -8 for a held item
bad = seen = 0
cover = {"show draw": 0, "show hide": 0, "poll draw": 0, "poll hide": 0, "poll hide again": 0}
for c in range(ncases):
    y = EDGE[c % len(EDGE)] if c < 4 * len(EDGE) else r.randrange(0, 200)
    hot = HOTS[(c // len(EDGE)) % 4] if c < 4 * len(EDGE) else r.choice(HOTS)
    flag = (c & 1) if c < 4 * len(EDGE) else r.choice((0, 0, 1, 2))
    hid = r.randrange(2)
    x = r.randrange(320)
    for name in ("show", "poll"):
        log.clear()
        pos[:] = [x, y]
        e.w8(e.rt(READY), 1)
        e.w8(e.rt(FLAG), flag)
        e.w8(e.rt(HIDE), hid)
        e.w32(e.rt(HOTY), hot & 0xFFFFFFFF)
        e.w32(e.rt(HOTX), 0)
        e.w32(e.rt(MX), (x + 1) & 0xFFFFFFFF)     # a moved mouse
        e.w32(e.rt(MY), (y + 1) & 0xFFFFFFFF)
        e.w32(e.rt(LOCK), 0)
        e.w32(e.rt(SHAPE), 0)
        e.w8(e.rt(BUTTONS), 0)
        if name == "show":
            e.w8(e.rt(OFF), 1)
            e.call(e.rt(0x673AC), max_steps=20_000)
        else:
            e.w8(e.rt(OFF), 0)
            e.call(e.rt(0x678E8), 0, max_steps=20_000)
        drawn = any(k[0] == "draw" for k in log)
        hidden = PORT.ja1f_top_row_hidden(y, hot, flag)
        why = []
        if drawn == bool(hidden):
            why.append(f"original draws {drawn}, port hides {hidden}")
        if drawn and [k[1:] for k in log if k[0] == "draw"] != [(x, y + hot & 0xFFFFFFFF)]:
            if [(k[1] & 0xFFFFFFFF, k[2] & 0xFFFFFFFF) for k in log if k[0] == "draw"] != [(x, (y + hot) & 0xFFFFFFFF)]:
                why.append(f"draw place {log}")
        after = e.r8(e.rt(HIDE))
        if hidden and after != 1:
            why.append(f"hide byte {after}")
        if name == "show" and e.r8(e.rt(OFF)) != 0:
            why.append("off byte stays")
        if name == "poll":
            want = [] if hidden and hid else [("restore",)] if hidden else None
            if want is not None and log != want:
                why.append(f"calls {log}, want {want}")
            if not hidden and after != 0:
                why.append(f"hide byte {after}")
            cover["poll hide again" if hidden and hid else "poll hide" if hidden else "poll draw"] += 1
        else:
            cover["show hide" if hidden else "show draw"] += 1
        seen += 1
        if why:
            bad += 1
            if bad <= 5:
                print(f"case {c} {name} y {y} hot {hot} flag {flag} hid {hid}: {'; '.join(why)}")
# MousePoll? 0x678E8, the rest of its body. The shape swap against pointer::poll_shape (the window's pointer_index reads it);
# original side only: the redraw only for a move or the redraw request 0x229C34, none with the pointer off, the request
# cleared by a draw, and the key that a non-zero argument reads.
PORT.ja1f_poll_shape.argtypes = [ctypes.c_int32, ctypes.c_uint8]
PORT.ja1f_poll_shape.restype = ctypes.c_int32
shapes, keys = [], []
e.stub(e.rt(0x674B0), lambda em, a, *_: shapes.append(a & 0xFFFFFFFF) or 0)
e.stub(e.rt(0x5616C), lambda em, *_: 1 if keys else 0)
e.stub(e.rt(0x560F4), lambda em, *_: keys.pop(0))
btn = [0]


def pop_b(em, eax, edx, ebx, *_):
    em.w32(eax, pos[0])
    em.w32(edx, pos[1])
    em.w32(ebx, btn[0])
    return 0


e.stub(e.rt(0x68470), pop_b)
rest = {"swap": 0, "no swap": 0, "moved": 0, "request": 0, "still": 0, "off": 0, "key read": 0}
for c in range(ncases):
    cur = r.choice((0, 0, 200, 200, 201, 202, 202, 203, 203, 1, 57, 199, 204))
    btn[0] = r.randrange(8)
    moved, lock, off, flush = r.randrange(2), r.randrange(2), r.randrange(4) == 0, r.randrange(2)
    x, y = r.randrange(320), r.randrange(40, 200)
    pos[:] = [x, y]
    log.clear(), shapes.clear(), keys.clear()
    keys.extend(r.choice(([], [0x41], [0x41, 0x42])))
    n_keys = len(keys)
    e.w32(e.rt(SHAPE), cur)
    e.w8(e.rt(FLAG), 1)
    e.w8(e.rt(HIDE), 0)
    e.w32(e.rt(HOTY), 0)
    e.w32(e.rt(HOTX), 0)
    e.w32(e.rt(MX), (x + moved) & 0xFFFFFFFF)
    e.w32(e.rt(MY), y)
    e.w32(e.rt(LOCK), lock)
    e.w8(e.rt(OFF), off)
    e.call(e.rt(0x678E8), flush, max_steps=20_000)
    after = PORT.ja1f_poll_shape(cur, btn[0] & 3)
    why = []
    if after != cur and shapes != [after]:
        why.append(f"shape: original {shapes}, port {after}")
    if after == cur and shapes:
        why.append(f"shape: original {shapes}, port keeps {cur}")
    drawn = any(k[0] == "draw" for k in log)
    if drawn != (not off and bool(moved or lock)):
        why.append(f"draw {drawn}")
    if drawn and e.r32(e.rt(LOCK)) != 0:
        why.append("redraw request stays")
    if len(keys) != n_keys - (1 if flush and n_keys else 0):
        why.append(f"keys {n_keys} -> {len(keys)} with argument {flush}")
    rest["swap" if after != cur else "no swap"] += 1
    rest["off" if off else "moved" if moved else "request" if lock else "still"] += 1
    rest["key read"] += flush and n_keys > 0
    seen += 1
    if why:
        bad += 1
        if bad <= 5:
            print(f"rest case {c}: shape {cur} buttons {btn[0]} moved {moved} request {lock} off {off}: {'; '.join(why)}")
cover.update(rest)
# HideMouse? 0x67368 and MouseRedraw? 0x67398 (real ShowMouse? under it) against pointer::hide_mouse and pointer::mouse_redraw:
# the off byte after the call, the restore / draw calls, and the gate bytes over_map 0x284F13 and tactical screen 0x92DCC.
OVER = 0x284F13
PORT.ja1f_hide_mouse.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
PORT.ja1f_mouse_redraw.argtypes = [ctypes.c_uint8, ctypes.c_uint8, ctypes.c_uint8]
e.stub(e.rt(0x68470), pop)
hold = {"hide": 0, "hide over map": 0, "hide already off": 0, "redraw show": 0, "redraw over map": 0, "redraw shown": 0,
        "redraw screen off": 0, "redraw top row": 0}
for c in range(ncases):
    off, over, ready = r.randrange(2), r.choice((0, 0, 1, 2, 0x40)), r.choice((1, 1, 1, 0))
    y = r.choice((r.randrange(0, 16), r.randrange(16, 200)))
    flag, x = r.randrange(2), r.randrange(320)
    for fn in ("hide", "redraw"):
        log.clear()
        pos[:] = [x, y]
        e.w8(e.rt(READY), ready)
        e.w8(e.rt(FLAG), flag)
        e.w8(e.rt(HIDE), 0)
        e.w8(e.rt(OVER), over)
        e.w8(e.rt(OFF), off)
        e.w32(e.rt(HOTY), 0)
        e.w32(e.rt(HOTX), 0)
        e.w32(e.rt(MX), x)
        e.w32(e.rt(MY), y)
        e.call(e.rt(0x67368 if fn == "hide" else 0x67398), max_steps=20_000)
        got = e.r8(e.rt(OFF))
        top = bool(PORT.ja1f_top_row_hidden(y, 0, flag))
        if fn == "hide":
            want = PORT.ja1f_hide_mouse(off, over)
            calls = [("restore",)] if want and not off else []
            hold["hide over map" if over else "hide already off" if off else "hide"] += 1
        else:
            want = PORT.ja1f_mouse_redraw(off, over, ready)
            shows = bool(off) and not want
            calls = [("draw", x, y)] if shows and not top else []
            hold["redraw shown" if not off else "redraw over map" if over else "redraw screen off" if not ready
                 else "redraw top row" if top else "redraw show"] += 1
        why = []
        if got != want:
            why.append(f"off byte: original {got}, port {want}")
        if [tuple(k[0:1]) + tuple(v & 0xFFFFFFFF for v in k[1:]) for k in log] != calls:
            why.append(f"calls: original {log}, port {calls}")
        seen += 1
        if why:
            bad += 1
            if bad <= 5:
                print(f"hold case {c} {fn}: off {off} over {over} ready {ready} y {y} flag {flag}: {'; '.join(why)}")
cover.update(hold)
print(f"pointer_diff: {seen} cases, {bad} mismatches; {cover}")
if min(cover.values()) < 20:
    print("pointer_diff: a branch stays below 20 cases")
    sys.exit(1)
sys.exit(1 if bad else 0)
