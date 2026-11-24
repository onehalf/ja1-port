#!/usr/bin/env python3
"""Differential test: the office entry, InitOfficeScreen? 0x4ED1C, original vs port.

usage: .venv/bin/python validate/diff/officeinit_diff.py [ncases=60] [DUMP] [--lib SO]
The original runs in Unicorn with the real archive open, member search, picture and palette reads (fopen_, fread_, fseek_,
fgetc_, fclose_ serve the real OFFICE.DAT), the real DrawRoomScreen, calendar text and CopyPanelToScreen. Logged stubs:
PlayXmiMusic, LoadMainSound, LoadPanelPcc, SetMouseShape, SetMusic, FadePalette, FlushKeys, MousePoll, SaveScreenArea, Hide + Show.
The port: office::Office::new (ja1f_office_init). Compared: the 64000 screen pixels, the palette of the fade (EAX 0), the
theme. A case: EAX 0 or 1, link byte, day.
On the original side only: the order of the calls, EAX 1 has no music, no fade and no key flush, the shape 0xCA, the brush
picture reaches its buffer, 0x284F0C = 1, the save area 13 at (mouse x, mouse y - 10).
Not compared: EAX 1 against the port (Restore Game keeps the office music through music::screen and has no fade line);
the brush bytes (office_diff draws them). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
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
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"officeinit_diff: {dump} missing")
    sys.exit(1)
ISO, EXE, SAVE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE"), ("rom", "__support", "save", "QICKSTRT.SAV")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_office_init.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int] * 2 + [ctypes.c_void_p] * 3
ARC = game_read("DAT\\OFFICE.DAT")
e = Emu(dump=dump, fast=True)
rt = e.rt
LINK, DAY, SCREEN_PTR, BRUSH_PTR, MX, MY, KIND = 0x22A043, 0x229FA4, 0x95678, 0x93E5C, 0x229D00, 0x229D04, 0x284F0C
DAC = 0xEDA08
st = {"pos": 0}


def log(s):
    st["log"].append(s)
    return 0


def fread(em, a, d, b, c, *_):
    out = ARC[st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    st["pos"] += len(out)
    return len(out) // max(d, 1)


def fseek(em, a, d, b, *_):
    st["pos"] = {0: d, 1: st["pos"] + d, 2: len(ARC) + d}[b]
    return 0


def fgetc(em, *_):
    if st["pos"] >= len(ARC):
        return 0xFFFFFFFF
    st["pos"] += 1
    return ARC[st["pos"] - 1]


def fopen(em, a, *_):
    assert e.cstr(a).upper() == b"DAT\\OFFICE.DAT", e.cstr(a)
    st["pos"] = 0
    return 5


def fade(em, a, d, b, c, *_):
    sp_arg = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    st["pal"] = bytes(e.uc.mem_read(a, 0x400))
    return log(f"fade {d} {b} {c} {sp_arg}")


for addr, fn, pop in ((0x75EA0, fopen, 0), (0x758AD, fread, 0), (0x757B1, fseek, 0), (0x7648C, fgetc, 0), (0x75A77, lambda em, *_: 0, 0),
                      (0x4707C, lambda em, *_: 1, 0), (0x52908, fade, 4),
                      (0x5C4C8, lambda em, a, *_: log(f"xmi {e.cstr(a).decode()}"), 0),
                      (0x31B8C, lambda em, *_: log("mainsnd"), 0),
                      (0x46B1C, lambda em, a, d, b, *_: log(f"panelpcc {d & 0xFF} {e.cstr(b).decode()}"), 0),
                      (0x674B0, lambda em, a, *_: log(f"shape {a}"), 0),
                      (0x5C5EC, lambda em, a, d, b, *_: log(f"music {a} {d} {b}"), 0),
                      (0x67F90, lambda em, *_: log("flush"), 0),
                      (0x678E8, lambda em, a, *_: log(f"poll {a}"), 0),
                      (0x67BF0, lambda em, *_: log("hide"), 0),
                      (0x673AC, lambda em, *_: log("show"), 0),
                      (0x467E4, lambda em, a, d, b, *_: log(f"save {a} {d} {(b ^ 0x80000000) - 0x80000000}"), 0)):
    e.stub(rt(addr), fn, pop=pop)
r = random.Random(1104)
bad = cases = 0


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


screen = e.r32(rt(SCREEN_PTR))
brush = e.r32(rt(BRUSH_PTR))
for c in range(ncases):
    quiet = c % 3 == 2
    link = r.choice((0, 0, 1, r.randrange(256)))
    day = r.choice((1, 2, 9, 10, 31, 99, 100, r.randrange(1, 400)))
    mx, my = r.randrange(320), r.randrange(200)
    e.w8(rt(0x92F90), 0)
    e.w8(rt(LINK), link)
    e.w32(rt(DAY), day)
    e.w8(rt(KIND), 0)
    e.w32(rt(MX), mx)
    e.w32(rt(MY), my)
    e.uc.mem_write(screen, b"\x55" * 64000)
    e.uc.mem_write(brush, b"\x55" * 0x100)
    st["log"] = []
    st.pop("pal", None)
    try:
        e.call(rt(0x4ED1C), int(quiet), max_steps=30_000_000)
    except EmuStop as x:
        report(c, f"original stopped: {x}")
        continue
    cases += 1
    tail = ["show", "poll 0", "hide", f"save 13 {mx} {my - 10}", "show"]
    head = [] if quiet else ["xmi MISC\\menus", "mainsnd"]
    middle = [] if quiet else ["music 1 0 24", "fade 0 24 0 256", "flush"]
    want = head + ["panelpcc 1 OFFBRUSH.PCX", "shape 202", "hide"] + middle + tail
    if st["log"] != want:
        report(c, f"calls {st['log']}")
    if e.r8(rt(KIND)) != 1:
        report(c, f"0x284F0C = {e.r8(rt(KIND))}")
    if bytes(e.uc.mem_read(brush, 0x100)) == b"\x55" * 0x100:
        report(c, "no brush picture")
    o, p, m = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(768), ctypes.c_int(-5)
    ret = PORT.ja1f_office_init(ISO, EXE, SAVE, link, day, o, p, ctypes.byref(m))
    if ret:
        report(c, f"port returned {ret}")
        continue
    px = bytes(e.uc.mem_read(screen, 64000))
    if px != o.raw:
        d = [i for i in range(64000) if px[i] != o.raw[i]]
        report(c, f"link {link} day {day}: {len(d)} pixels, first ({d[0] % 320},{d[0] // 320}) original {px[d[0]]} port {o.raw[d[0]]}")
    if not quiet:
        rgb = bytes(st["pal"][k] for k in range(0x400) if k % 4 != 3)
        if rgb != p.raw:
            report(c, "palette differs")
        if m.value != 1:
            report(c, f"port theme {m.value}")
print(f"officeinit_diff: {cases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
