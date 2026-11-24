#!/usr/bin/env python3
"""Differential test: InitMainDatMouse 0x36F90, the program start before the main menu, original vs port (startup.rs).

usage: .venv/bin/python validate/diff/startup_diff.py [ncases=40] [DUMP] [--lib SO]
The original runs in Unicorn with every callee stubbed and logged: DOS set-up, music, title part, panel file, mouse. Fuzzed:
word 0x95DC0 and byte 0x9639B. Compared with startup::START and startup::set_music_zero: the music file, whether SetMusic(0, 0)
runs, the title argument, the mouse limit, place and shape, and the pointer off byte. Original side only: the order of the
calls, the 64000-byte clear of the screen before the title part, and the panel file. Exits 1 on mismatch.
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
ncases = int(args[0]) if args else 40
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"startup_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_start_plan.argtypes = [ctypes.c_uint16, ctypes.c_uint8, ctypes.POINTER(ctypes.c_int32)]
PORT.ja1f_start_music.restype = ctypes.c_char_p
OFF, SHAPE, DRIVER, MUSIC, MX, MY, SCREEN = 0x284F0B, 0x229C54, 0x95DC0, 0x9639B, 0x229D00, 0x229D04, 0x95678
e = Emu(dump=dump, fast=True)
log = []


def text(em, a):
    return bytes(em.uc.mem_read(a, 24)).split(b"\0")[0].decode("latin-1")


def named(name, *regs):
    def f(em, eax, edx, ebx, ecx, *_):
        vals = dict(a=eax, d=edx, b=ebx, c=ecx)
        log.append((name, *[vals[k] & 0xFFFFFFFF for k in regs]))
        return 0
    return f


for addr, name, regs in ((0x68594, "driver", ""), (0x381F8, "font", ""), (0x5C5EC, "setmusic", "ad"), (0x524E0, "video", "a"),
                         (0x66604, "title", "a"), (0x46EC4, "panels", "bc"), (0x68568, "driverptr", ""),
                         (0x682AC, "limit", "adbc"), (0x681D0, "setpos", "ad"), (0x674B0, "shape", "a"), (0x46EF0, "fail", ""),
                         (0x75619, "atexit", "")):
    e.stub(e.rt(addr), named(name, *regs))
e.stub(e.rt(0x5C4C8), lambda em, a, *_: log.append(("music", text(em, a))) or 0)
e.stub(e.rt(0x756A0), lambda em, a, d, b, *_: log.append(("memset", a & 0xFFFFFFFF, d & 0xFF, b & 0xFFFFFFFF)) or 0)
e.stub(e.rt(0x53338), lambda em, a, *_: log.append(("open", text(em, a))) or 1, 4)
screen = e.r32(e.rt(SCREEN))
r = random.Random(36)
bad = 0
cover = {"zero": 0, "no zero": 0}
ORDER = ["atexit", "driver", "fail", "font", "music", "video", "memset", "title", "open", "fail", "panels", "driverptr", "limit",
         "setpos", "shape"]
for c in range(ncases):
    drv = c % 3 if c < 9 else r.choice((0, 1, 2, 0x100, 0xFFFF))
    mus = (c // 3) % 3 if c < 9 else r.choice((0, 1, 2, 0x80))
    log.clear()
    e.w16(e.rt(DRIVER), drv)
    e.w8(e.rt(MUSIC), mus)
    e.w8(e.rt(OFF), 0)
    e.w32(e.rt(SHAPE), 7)
    e.w32(e.rt(MX), 3)
    e.w32(e.rt(MY), 4)
    e.call(e.rt(0x36F90), max_steps=20_000)
    out = (ctypes.c_int32 * 12)()
    PORT.ja1f_start_plan(drv, mus, out)
    title_arg, x0, y0, x1, y1, px, py, shape, off, zero = out[:10]
    by = {k[0]: k[1:] for k in log}
    why = []
    names = [k[0] for k in log if k[0] != "setmusic"]
    if names != ORDER:
        why.append(f"order {names}")
    if ("setmusic" in by) != bool(zero):
        why.append(f"SetMusic: original {by.get('setmusic')}, port {zero}")
    if "setmusic" in by and by["setmusic"] != (0, 0):
        why.append(f"SetMusic arguments {by['setmusic']}")
    if by.get("music", ("",))[0] != "MISC\\" + PORT.ja1f_start_music().decode():
        why.append(f"music {by.get('music')}")
    if by.get("title") != (title_arg,):
        why.append(f"title {by.get('title')}, port {title_arg}")
    if by.get("limit", (0,))[0] != x0 or by.get("limit", (0, 0))[1] != y0:
        why.append(f"limit {by.get('limit')}, port {x0, y0}")
    if by.get("driverptr") is None or by.get("panels") != (x1, y1):
        why.append(f"panel rectangle {by.get('panels')}, port {x1, y1}")
    if by.get("setpos") != (px, py) or (e.r32(e.rt(MX)), e.r32(e.rt(MY))) != (px, py):
        why.append(f"mouse {by.get('setpos')}, port {px, py}")
    if by.get("shape") != (shape,):
        why.append(f"shape {by.get('shape')}, port {shape}")
    if e.r8(e.rt(OFF)) != off:
        why.append(f"off byte {e.r8(e.rt(OFF))}, port {off}")
    if e.r32(e.rt(SHAPE)) != 0xFFFFFFFF:
        why.append("the old shape stays (the first SetMouseShape? must load)")
    if by.get("video") != (0x13,) or by.get("memset", (0, 1, 0))[1:] != (0, 64000) or by["memset"][0] != screen:
        why.append(f"screen clear {by.get('video')} {by.get('memset')}")
    if by.get("open") != ("dat\\main.dat",):
        why.append(f"panel file {by.get('open')}")
    cover["zero" if "setmusic" in by else "no zero"] += 1
    if why:
        bad += 1
        if bad <= 5:
            print(f"case {c} driver {drv} music {mus}: {'; '.join(why)}")
print(f"startup_diff: {ncases} cases, {bad} mismatches; {cover}")
if min(cover.values()) < 5:
    print("startup_diff: a branch stays below 5 cases")
    sys.exit(1)
sys.exit(1 if bad else 0)
