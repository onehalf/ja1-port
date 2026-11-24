#!/usr/bin/env python3
"""Differential test: the wrapper of the inventory screen, FUN_00054d50 0x54D50, original vs port.

usage: .venv/bin/python validate/diff/invwrap_diff.py [ncases=1500] [DUMP] [--lib SO]
A case: the merc that opens the screen, the hand items, action points and +0x16A of the 8 mercs, the bytes +0x1BB and +0x130, the view mode, panel ends. The original
runs from its first line with real ReserveAp? and ActionApCost. The screen 0x53A9C, DrawMercBody?, LoadStripPccs, DrawStripBoxes,
the mouse limit, MouseSetPos? are stubs (inv_diff, strips_diff, invdraw_diff test them); the redraw 0x1CB78 is a stub that sets
the flag 0x22A02E, as TacticalRedraw and OverviewRedraw do.
The port: ja1f_inv_after = invscreen::after_screen + wrapper_end, the code the window runs, for the same merc pair.
Compared: all 48 soldier records, view mode, panel ends, the flag 0x22A02E, the second argument of the screen.
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"invwrap_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(LIB)
i32 = ctypes.c_int32
P.ja1f_inv_after.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host), i32, i32, ctypes.c_void_p]
P.ja1f_inv_after.restype = i32
P.ja1f_cursor_after.argtypes = [ctypes.c_uint8, ctypes.c_uint8, i32, i32, i32]
P.ja1f_cursor_after.restype = ctypes.c_int64
SOL, REC, MODE, ENDS, FLAG, STRIPM = 0xFA958, 0x200, 0x92DDF, 0x92E35, 0x22A02E, 0x92E4F
e = Emu(dump=dump, fast=True)
rt = e.rt
log = []


def read_u8(a):
    return e.uc.mem_read(rt(a), 1)[0]


def screen(em, a, d, *_):
    log.append(("screen", a, d & 0xFF, read_u8(MODE), read_u8(STRIPM)))
    # The screen itself does not change the mercs here.


def redraw(em, *_):
    log.append(("redraw", read_u8(MODE)))
    em.uc.mem_write(rt(FLAG), b"\x01")


e.stub(rt(0x53A9C), screen)
e.stub(rt(0x1CB78), redraw)
for a in (0x555DC, 0x2F928, 0x55FD8):
    e.stub(rt(a), lambda em, *_: 0)
tail = []                  # the mouse limit and the mouse move at the end of 0x54D50
e.stub(rt(0x682AC), lambda em, a, d, b, c: tail.append(("limit", a, d, b, c)) or 0)
e.stub(rt(0x681D0), lambda em, a, d, *_: tail.append(("mouse", a, d)) or 0)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
r = random.Random(0x54D50)
bad = 0
seen = set()
HAND = [0, 1, 5, 10, 14, 15, 0x20, 0x4D, 0x85, 0x86]
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    log.clear()
    opener = r.randrange(8)
    for k in range(8):
        base = rt(SOL) + k * REC
        rec = bytearray(e.uc.mem_read(base, REC))
        rec[0] = r.choice(HAND)
        rec[0x17A] = k
        rec[0x182] = r.randrange(2)
        rec[0x130] = r.choice((0, 0, 1, 5))
        rec[0x1BB] = r.choice((0, 1, 7))
        rec[0x177] = r.randrange(2)
        rec[0x16A] = r.randrange(2)
        rec[0xEF] = r.randrange(40)
        rec[0xEE] = r.randrange(40)
        rec[0x188] = 0
        e.uc.mem_write(base, bytes(rec))
    mode = r.randrange(4)
    ends = r.randrange(2)
    info = r.randrange(2)
    e.uc.mem_write(rt(MODE), bytes([mode]))
    e.uc.mem_write(rt(ENDS), bytes([ends]))
    e.uc.mem_write(rt(FLAG), bytes([r.randrange(2)]))
    e.uc.mem_write(rt(0x222B50), bytes([r.randrange(2)]))
    stripm = r.randrange(2)
    e.uc.mem_write(rt(STRIPM), bytes([stripm]))
    # The world of the port, from the same state before the call.
    bufs, sc = port_lib.world_load(e, ai)
    tail.clear()
    mouse_x = r.randrange(320)
    e.w32(rt(0x229D00), mouse_x)
    e.call(rt(0x54D50), rt(SOL) + opener * REC, info)
    v = P.ja1f_cursor_after(e.r8(rt(0x222B50)), 0, mouse_x, 1, 0)
    want_tail = [("limit", 1, 0xBE, 0x13E, 0xC5), ("mouse", v >> 16, 0xBF)] if v >> 8 & 1 else []
    if tail != want_tail:
        bad += 1
        if bad <= 5:
            print(f"case {case}: tail of the screen: orig {tail} port {want_tail}")
    want_log = [("screen", rt(SOL) + opener * REC, info, 1, 1), ("redraw", mode)]
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = port_lib.World(**{k: ctypes.cast(a, port_lib.u8p) for k, a in arrays.items()}, **sc)
    host = port_lib.Host(None, port_lib.RANDOM_FN(lambda _c, k: 0), port_lib.CALL_FN(lambda *_a: 0), 0)
    pe = ctypes.c_uint8(ends)
    rc = P.ja1f_inv_after(ctypes.byref(w), ctypes.byref(host), opener, mode, ctypes.byref(pe))
    msgs = []
    if rc < 0:
        msgs.append(f"rc {rc}")
    if log != want_log:
        msgs.append(f"log {log}")
    o = bytes(e.uc.mem_read(rt(SOL), 48 * REC))
    pp = bytes(bufs["soldiers"])
    d = [i for i in range(48 * REC) if o[i] != pp[i] and (i % REC) not in range(0x90, 0x94)]
    if d:
        msgs.append(f"soldier {d[0] // REC} +{d[0] % REC:#x}: orig {o[d[0]]} port {pp[d[0]]} ({len(d)} bytes)")
    if read_u8(ENDS) != pe.value:
        msgs.append(f"panel ends orig {read_u8(ENDS)} port {pe.value}")
    if read_u8(MODE) != mode or read_u8(STRIPM) != stripm:
        msgs.append("mode bytes not kept")
    if read_u8(FLAG) != w.opt_reload_msg:
        msgs.append(f"flag orig {read_u8(FLAG)} port {w.opt_reload_msg}")
    if msgs:
        bad += 1
        if bad <= 5:
            print(f"case {case} opener {opener} mode {mode}: " + "; ".join(msgs))

# CursorItemToSoldier? 0x54EC8 and the tail of 0x54D50: limit + mouse row 191 + flag 0x955C9 with an item left on the
# cursor. The port: invscreen::cursor_after. 0x54D50 is a stub here: it leaves the scripted item on the cursor.
CURSOR, DROPF = 0x222B50, 0x955C9
calls, after_screen = [], [0]
e2 = Emu(dump=dump, fast=True)
e2.stub(e2.rt(0x674B0), lambda em, *_: 0)
e2.stub(e2.rt(0x54D50), lambda em, *_: em.w8(em.rt(CURSOR), after_screen[0]) or 0)
e2.stub(e2.rt(0x682AC), lambda em, a, d, b, c: calls.append(("limit", a, d, b, c)) or 0)
e2.stub(e2.rt(0x681D0), lambda em, a, d, *_: calls.append(("mouse", a, d)) or 0)
r2 = random.Random(11)
cases2 = max(200, ncases // 5)
bad2 = 0
for case in range(cases2):
    before, after_screen[0] = r2.choice([0, 7, 0x4D]), r2.choice([0, 0, 9, 0x51])
    drop, mx = r2.choice([0, 1]), r2.randrange(320)
    e2.w8(e2.rt(CURSOR), before)
    e2.w8(e2.rt(DROPF), drop)
    e2.w32(e2.rt(0x229D00), mx)
    calls.clear()
    e2.call(e2.rt(0x54EC8), e2.rt(SOL))
    o_flag = e2.r8(e2.rt(DROPF))
    held = after_screen[0] if before else 0
    v = P.ja1f_cursor_after(held, drop, mx, 0, 1)
    p_calls = [("limit", 1, 0xBE, 0x13E, 0xC5), ("mouse", mx, 0xBF)] if v >> 8 & 1 else []
    if v & 0xFF != o_flag or calls != p_calls or (p_calls and v >> 16 != mx):
        bad2 += 1
        if bad2 <= 5:
            print(f"cursor case {case}: before {before} after {after_screen[0]} drop {drop}: orig {calls} flag {o_flag}, port {p_calls} {v:#x}")
print(f"invwrap_diff: {ncases} cases, {bad} mismatches; cursor: {cases2} cases, {bad2} mismatches")
bad += bad2
sys.exit(1 if bad else 0)
