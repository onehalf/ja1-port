#!/usr/bin/env python3
"""Differential test: the way to the guard screen and back FUN_1328C (key M), original vs port.

usage: .venv/bin/python validate/diff/guardopen_diff.py [DUMP] [--lib SO]
The original runs in Unicorn on tac60 with every callee stubbed and logged. The port: guardopen::ENTER, LEAVE and mouse_off
(ja1f_guard_steps). Compared: the order of the calls, each fade (steps, from, to), HidePathMarks' force, the mouse wobble, over_map and
the cursor tile at the screen. The key test: TacticalKeyHandler 0x13E78 tests 0x92DCC before the call (read from the image; guardopen::key_m_opens).
On the original side only: 0x92DE1 = 1 during the screen and 0 after (the port has no turn pass during the screen), the palette
0xEDA08, LoadStripPccs (1, 0), the view mode argument of the redraw.
Not compared: what the steps do inside (cursorback_diff, fade_diff, hover_diff, guard_diff, tiles).
Exits 1 on mismatch.
"""
import ctypes, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"guardopen_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_guard_steps.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
e = Emu(dump=dump, fast=True)
rt = e.rt
FLAG, DX, DY, OVER, TILE, VIEW = 0x92DE1, 0x22A47E, 0x22A47C, 0x284F13, 0x229D2C, 0x92DD4
bad = []
log = []


def sp(k):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + k)


def fade(em, a, d, b, c, *_):
    log.append(f"Fade({b}, {c}, {sp(4)})")
    if a != rt(0xEDA08) or d != 0:
        bad.append(f"fade source {a:#x} {d}")
    return 0


def screen(em, *_):
    log.append("screen")
    state = (e.r8(rt(FLAG)), e.r16(rt(DX)), e.r16(rt(DY)), e.r8(rt(OVER)), e.r32(rt(TILE)))
    log.append(("at screen", state))
    return 0


def marks(em, a, *_):
    log.append("HideMarks")
    if a & 0xFF != 0:
        bad.append(f"HidePathMarks force {a}")
    return 0


def strips(em, a, d, *_):
    log.append("Strips")
    if (a, d) != (1, 0):
        bad.append(f"LoadStripPccs {a} {d}")
    return 0


def redraw(em, a, d, b, c, *_):
    log.append("Redraw")
    if a & 0xFF != 3 or (d, b, c) != (0, 0x18, 0):
        bad.append(f"redraw {a} {d} {b:#x} {c}")
    return 0


e.stub(rt(0x53660), lambda em, *_: log.append("CursorBack") or 0)
e.stub(rt(0x67368), lambda em, *_: log.append("HideMouse") or 0)
e.stub(rt(0x52908), fade, 4)
e.stub(rt(0x1D8B8), marks)
e.stub(rt(0x427B4), screen)
e.stub(rt(0x2A990), lambda em, *_: log.append("Palette") or 0)
e.stub(rt(0x2F928), strips)
e.stub(rt(0x1CB78), redraw)
e.w8(rt(FLAG), 0)
e.w16(rt(DX), 3)
e.w16(rt(DY), 4)
e.w8(rt(OVER), 1)
e.w32(rt(TILE), 500)
e.w8(rt(VIEW), 3)
e.call(rt(0x1328C))
if e.r8(rt(FLAG)) != 0:
    bad.append("0x92DE1 stays set")
at = next(x for x in log if isinstance(x, tuple))[1]
if at[0] != 1:
    bad.append(f"0x92DE1 at the screen {at[0]}")
orig = [x for x in log if isinstance(x, str)]
state = (ctypes.c_int * 4)(3, 4, 1, 500)
out = ctypes.create_string_buffer(1000)
n = PORT.ja1f_guard_steps(state, out, 1000)
port = out.raw[:n].decode().split("\n")[:-1]
# MouseOff has no call in the original: it is 4 writes between HideMarks and the screen.
if "MouseOff" not in port or port[port.index("MouseOff") + 1] != "screen":
    bad.append(f"MouseOff is not right before the screen: {port}")
port = [x for x in port if x != "MouseOff"]
if orig != port:
    bad.append(f"steps\n  orig {orig}\n  port {port}")
if list(state) != list(at[1:]):
    bad.append(f"state at the screen: orig {at[1:]} port {list(state)}")
# The two callers: the key's test of 0x92DCC.
code = bytes(e.uc.mem_read(rt(0x13E78), 16))
if code[:2] != b"\x80\x3d" or int.from_bytes(code[2:6], "little") != rt(0x92DCC):
    bad.append(f"key M does not test 0x92DCC: {code.hex()}")
for b in bad[:12]:
    print(b)
print(f"guardopen_diff: {len(orig)} calls: {len(bad)} mismatches")
sys.exit(1 if bad else 0)
