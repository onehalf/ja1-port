#!/usr/bin/env python3
"""Differential test: the picture of the main menu, MainMenu 0x4FDF4 + the row draw 0x4FD48, original vs port.

usage: .venv/bin/python validate/diff/mainmenu_diff.py [DUMP] [--lib SO]
The original runs in Unicorn with the real archive open, member search, palette read, PCX read and row draw, on the real
OFFICE.DAT. Stubs: fopen_, fread_, fseek_, fgetc_, fclose_ (they serve the file), FadePalette 0x52908 (logs its arguments and
the screen and palette then), mouse show, MouseSetPos (logged), memset is real.
The port: menu::Menu::open (ja1f_main_menu). Compared for each row 0..2: the 64000 screen pixels at the fade in, the palette
of both fades, the fade arguments, the mouse place. menu_diff compares the calls with the loop. Exits 1 on mismatch.
"""
import ctypes, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"mainmenu_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_main_menu.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int] + [ctypes.c_void_p] * 4
ARC = game_read("DAT\\OFFICE.DAT")
e = Emu(dump=dump, fast=True)
rt = e.rt
SCREEN_PTR, DAC = 0x95678, 0xEDA08
st = {}
counts = {}


def note(n):
    counts[n] = counts.get(n, 0) + 1


def fopen(em, a, *_):
    note(("fopen", e.cstr(a)))
    st["pos"] = 0
    return 5


def fread(em, a, d, b, c, *_):
    note("fread")
    out = ARC[st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    st["pos"] += len(out)
    return len(out) // max(d, 1)


def fseek(em, a, d, b, *_):
    st["pos"] = {0: d, 1: st["pos"] + d, 2: len(ARC) + d}[b]
    return 0


def fgetc(em, *_):
    note("fgetc")
    if st["pos"] >= len(ARC):
        return 0xFFFFFFFF
    st["pos"] += 1
    return ARC[st["pos"] - 1]


def fade(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    st["fades"].append(((a & 0xFFFFFFFF) == rt(DAC), d, b, c, e.r32(sp + 4), bytes(e.uc.mem_read(a, 0x400))))
    return 0


def warp(em, a, d, *_):
    st["warp"] = (a, d)
    return 0


for addr, fn, pop in ((0x75EA0, fopen, 0), (0x758AD, fread, 0), (0x757B1, fseek, 0), (0x7648C, fgetc, 0), (0x75A77, lambda em, *_: 0, 0),
                      (0x52908, fade, 4), (0x681D0, warp, 0), (0x673AC, lambda em, *_: 0, 0), (0x4707C, lambda em, *_: 1, 0)):
    e.stub(rt(addr), fn, pop=pop)
bad = 0
for row in range(3):
    st.update(fades=[], warp=None, pos=0)
    e.w8(rt(0x92F90), 0)
    try:
        e.call(rt(0x4FDF4), row, max_steps=30_000_000)
    except EmuStop as x:
        print(f"row {row}: original stopped: {x}")
        bad += 1
        continue
    screen = e.r32(rt(SCREEN_PTR))
    px = bytes(e.uc.mem_read(screen, 64000))
    o, p, f, w = (ctypes.create_string_buffer(n) for n in (64000, 768, 768, 8))
    ret = PORT.ja1f_main_menu(ISO, EXE, row, o, p, f, w)
    if ret:
        print(f"row {row}: port returned {ret}")
        bad += 1
        continue
    if px != o.raw:
        d = [i for i in range(64000) if px[i] != o.raw[i]]
        print(f"row {row}: {len(d)} pixels differ, first ({d[0] % 320},{d[0] // 320}) original {px[d[0]]} port {o.raw[d[0]]}")
        bad += 1
    fades = st["fades"]
    if [x[:5] for x in fades] != [(True, 0, 24, 256, 0), (True, 0, 24, 0, 256)]:
        print(f"row {row}: fades {[x[:5] for x in fades]}")
        bad += 1
    # The palette in the fade call has 4-byte entries (0x52820).
    for i, name, mine in ((0, "first", f.raw), (1, "last", p.raw)):
        rgb = bytes(fades[i][5][k] for k in range(0x400) if k % 4 != 3)
        if rgb != mine:
            k = next(j for j in range(768) if rgb[j] != mine[j])
            print(f"row {row}: {name} palette differs at {k}: original {rgb[k]} port {mine[k]}")
            bad += 1
    mx = int.from_bytes(w.raw[:4], "little")
    my = int.from_bytes(w.raw[4:], "little")
    if st["warp"] != (mx, my):
        print(f"row {row}: mouse original {st['warp']} port {(mx, my)}")
        bad += 1
print("mainmenu_diff: calls", {k: v for k, v in counts.items()})
print(f"mainmenu_diff: 3 rows, {bad} mismatches")
sys.exit(1 if bad else 0)
