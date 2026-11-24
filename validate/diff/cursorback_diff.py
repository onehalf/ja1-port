#!/usr/bin/env python3
"""Differential test: the item on the cursor goes back to its strip box, CursorItemBack? 0x53660, original vs port.

usage: .venv/bin/python validate/diff/cursorback_diff.py [ncases=3000] [DUMP] [--lib SO]
Per case: 0x92DCC (screen_drawn), the flag 0x955C9, the cursor item, the selected merc, the strip box 0..6 and noise in all 48
records. The original runs in Unicorn on tac60 with the real CopyBytes12 0x47090 and DropItem 0x470B0; HidePointer, ShowMouse,
SetMouseShape 0x674B0 and the mouse limit 0x682AC (it answers 0) are stubbed. The port: play::cursor_back_raw (ja1f_cursor_back).
Compared: all soldier records, the 12 cursor bytes, the flag, drop_answer 0x229D18 (the old value when nothing happens).
On the original side only: the shape call (0, 0, 0x13F) and the limit call with 0: the port's pointer comes from the cursor item.
Not compared: strip boxes above 6 (StripClick finds only 7 boxes).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import port_lib  # noqa: E402
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"cursorback_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_cursor_back.argtypes = [ctypes.c_void_p]
e = Emu(dump=dump, fast=True)
rt = e.rt
DRAWN, FLAG, HELD, ANSWER, SELECTED, BOX, SOLDIERS = 0x92DCC, 0x955C9, 0x222B50, 0x229D18, 0x284F11, 0x92E73, 0xFA958
SIZE = 48 * 0x200
log = []
bad = 0
e.stub(rt(0x674B0), lambda em, a, d, b, c, *_: log.append(("shape", a & 0xFF, d, b)) or 0)
e.stub(rt(0x682AC), lambda em, a, *_: log.append(("limit", a)) or 0)
e.stub(rt(0x67BF0), lambda em, *_: 0)
e.stub(rt(0x673AC), lambda em, *_: 0)


def report(c, msg):
    global bad
    bad += 1
    if bad <= 12:
        print(f"case {c}: {msg}")


rng = random.Random(53)
acted = 0
for c in range(ncases):
    drawn = rng.choice((0, 1, 1, 1))
    flag = rng.choice((0, 1, 1, 1, 7))
    item = bytes(rng.randrange(256) for _ in range(12))
    if rng.random() < 0.2:
        item = b"\0" + item[1:]
    sel, box = rng.randrange(48), rng.randrange(7)
    old = rng.randrange(1, 0x7FFFFFFF)
    recs = bytes(rng.randrange(256) for _ in range(64)) * (SIZE // 64)
    log.clear()
    e.w8(rt(DRAWN), drawn)
    e.w8(rt(FLAG), flag)
    e.uc.mem_write(rt(HELD), item)
    e.uc.mem_write(rt(SOLDIERS), recs)
    e.w32(rt(ANSWER), old)
    e.w8(rt(SELECTED), sel)
    e.w8(rt(BOX), box)
    e.call(rt(0x53660))
    o = (bytes(e.uc.mem_read(rt(SOLDIERS), SIZE)), bytes(e.uc.mem_read(rt(HELD), 12)), e.r8(rt(FLAG)), e.r32(rt(ANSWER)))
    did = bool(drawn and flag and item[0])
    acted += did
    if log != ([("shape", 0, 0, 0x13F), ("limit", 0)] if did else []):
        report(c, f"the original's calls {log}")
    w = port_lib.World()
    s_buf = ctypes.create_string_buffer(recs, SIZE)
    h_buf = ctypes.create_string_buffer(item, 12)
    w.soldiers = ctypes.cast(s_buf, type(w.soldiers))
    w.held = ctypes.cast(h_buf, type(w.held))
    w.screen_drawn, w.cursor_drop, w.drop_answer, w.selected, w.strip_box = drawn, flag, old, sel, box
    r = PORT.ja1f_cursor_back(ctypes.byref(w))
    p = (s_buf.raw[:SIZE], h_buf.raw[:12], w.cursor_drop, w.drop_answer & 0xFFFFFFFF)
    if bool(r) != did or o != p:
        first = next((i for i in range(SIZE) if o[0][i] != p[0][i]), None)
        report(c, f"drawn {drawn} flag {flag} type {item[0]} sel {sel} box {box}: soldier byte {first}, held {o[1].hex()} {p[1].hex()}, "
                  f"flag {o[2]} {p[2]}, answer {o[3]:#x} {p[3]:#x}, port acted {r}")
print(f"cursorback_diff: {ncases} cases, {acted} acted: {bad} mismatches")
sys.exit(1 if bad or acted < ncases // 4 else 0)
