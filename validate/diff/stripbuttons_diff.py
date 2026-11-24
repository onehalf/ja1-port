#!/usr/bin/env python3
"""Differential test: the top strip buttons, 0x10398 (all up) and 0x10454 (shade), original vs port.

usage: .venv/bin/python validate/diff/stripbuttons_diff.py [ncases=300] [DUMP] [--lib SO]
A case: noise in the panel buffer and the screen, a show flag, the redraw-off byte 0x92DDF. Up, or a shade with last 0x12..0x18
(the callers pass 0x15 and 0x18; 0x12 = no button). The original runs in Unicorn with its real element draw 0x469F0, label
0x35AB8 and shade 0x254F0. The port: strips::buttons_up / buttons_shade / buttons_copy (ja1f_strip_buttons).
StopWalk 0x104C8 + 0x10310 (the busy display of a walk): strips::stop_walk and the frame's shade_of, see the part at the end.
Compared: the panel buffer, the screen, remap table 0 afterwards. buttons_up and buttons_shade are the code of draw_strips
(strips_diff --port). Not compared here: when the front-end shades (the smoke step end_turn looks at the picture).
Exits 1 on mismatch.
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
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"stripbuttons_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
PORT.ja1f_strip_buttons.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 3 + [ctypes.c_int] * 3
e = Emu(dump=dump, fast=True)
rt = e.rt
P_PANEL, P_SCREEN, P_TABLE0, REDRAW_OFF = 0x93E60, 0x95678, 0xE8C48, 0x92DDF
panel, screen, table0 = e.r32(rt(P_PANEL)), e.r32(rt(P_SCREEN)), e.r32(rt(P_TABLE0))
r = random.Random(1110)
bad = shown = 0
kinds = {}
for c in range(ncases):
    last = r.choice((0, 0, 0x12, 0x13, 0x14, 0x15, 0x15, 0x16, 0x17, 0x18, 0x18))
    show, off = r.randrange(2), r.randrange(5)
    # Noise with colours that the remap table moves, and bytes the shade leaves alone.
    pan = bytes(r.randrange(256) for _ in range(64000)) if r.random() < .5 else bytes(e.uc.mem_read(panel, 64000))
    scr = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(panel, pan)
    e.uc.mem_write(screen, scr)
    e.w8(rt(REDRAW_OFF), off)
    tab = bytes(e.uc.mem_read(table0, 256))
    try:
        if last == 0:
            e.call(rt(0x10398), show, max_steps=8_000_000)
        else:
            e.call(rt(0x10454), last, show, max_steps=8_000_000)
    except EmuStop as x:
        print(f"case {c}: original stopped: {x}")
        bad += 1
        continue
    op, os_, ot = (bytes(e.uc.mem_read(a, n)) for a, n in ((panel, 64000), (screen, 64000), (table0, 256)))
    pp, ps, pt = (ctypes.create_string_buffer(b, len(b)) for b in (pan, scr, tab))
    rc = PORT.ja1f_strip_buttons(ISO, EXE, pt, pp, ps, last, show, off)
    kinds[last] = kinds.get(last, 0) + 1
    shown += os_ != scr
    msg = []
    if rc:
        msg.append(f"port returned {rc}")
    for name, a, b in (("panel", op, pp.raw), ("screen", os_, ps.raw), ("table0", ot, pt.raw)):
        if a != b:
            d = [i for i in range(len(a)) if a[i] != b[i]]
            msg.append(f"{name}: {len(d)} bytes, first {d[0]} ({d[0] % 320},{d[0] // 320}) {a[d[0]]:#x} / {b[d[0]]:#x}")
    if msg:
        bad += 1
        if bad <= 10:
            print(f"case {c} last {last:#x} show {show} off {off}: " + "; ".join(msg))
    # Back to a clean table: a fault in one case does not run on.
    e.uc.mem_write(table0, tab)
# StopWalk 0x104C8 with 0x10310: calls in a row on the panel with its six buttons up; the real shade and element draws.
# The port draws the strip of each frame: buttons_up, then buttons_shade with shade_of (ja1f_stop_walk).
PTRS, SELECTED, COMBAT, HOLD, TURN = 0x100958, 0x284F11, 0x92DCB, 0x92DCE, 0x92DCA
walks = {"on": 0, "off": 0, "none": 0}
for c in range(max(30, ncases // 10)):
    pan = bytes(r.randrange(256) for _ in range(64000))
    scr = bytes(64000)
    e.uc.mem_write(panel, pan)
    e.uc.mem_write(screen, scr)
    tab = bytes(e.uc.mem_read(table0, 256))
    e.w8(rt(REDRAW_OFF), 0)
    e.call(rt(0x10398), 0, max_steps=8_000_000)
    up = bytes(e.uc.mem_read(panel, 64000))
    hold, turn = 0, r.randrange(2)
    e.w8(rt(HOLD), 0), e.w8(rt(TURN), turn)
    sel, combat = r.choice((0, 2, 7)), r.choice((1, 1, 1, 0))
    e.w8(rt(SELECTED), sel), e.w8(rt(COMBAT), combat)
    for step in range(5):
        slot = r.choice((sel, sel, sel, (sel + 1) % 8, 8, 41))
        flag = r.choice((0, 1, 1, 0x80))
        rec = rt(0xFA958) + 0x200 * slot
        e.w8(rec + 0x17A, slot)
        e.call(rt(0x104C8), rec, flag, max_steps=8_000_000)
        io = (ctypes.c_uint8 * 2)(hold, turn)
        last = PORT.ja1f_stop_walk(slot, flag, combat, sel, io)
        walks["on" if io[0] > hold else "off" if io[0] < hold else "none"] += 1
        hold, turn = io[0], io[1]
        pp, ps, pt = (ctypes.create_string_buffer(b, len(b)) for b in (pan, scr, tab))
        PORT.ja1f_strip_buttons(ISO, EXE, pt, pp, ps, 0, 0, 0)
        if last:
            PORT.ja1f_strip_buttons(ISO, EXE, pt, pp, ps, last, 0, 0)
        o = bytes(e.uc.mem_read(panel, 64000))
        fields = (e.r8(rt(HOLD)), e.r8(rt(TURN)))
        if fields != (hold, turn) or o != pp.raw or bytes(e.uc.mem_read(screen, 64000)) != scr:
            bad += 1
            if bad <= 10:
                d = [i for i in range(64000) if o[i] != pp.raw[i]]
                print(f"walk case {c} step {step}: slot {slot} flag {flag} sel {sel} combat {combat}: flags {fields} vs {(hold, turn)}, panel {len(d)} bytes {d[:2]}")
if 0 in walks.values():
    bad += 1
    print(f"stop_walk branches missing: {walks}")
print(f"stop_walk: {walks}")
print(f"stripbuttons_diff: {ncases} cases, {len(kinds)} kinds, {shown} copied to the screen, {bad} mismatches")
sys.exit(1 if bad or not shown or len(kinds) < 7 else 0)
