"""Differential test: the office picture (strategic.md "The office"), original vs port.

usage: .venv/bin/python validate/diff/office_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: a fuzzed A.I.M. link byte (0x22A043), day (g_day 0x229FA4), dark flag, highlight hot spot k (0 = none)
and frame j, and a label at a fuzzed mouse position, on a noise panel buffer. The original runs DrawRoomScreen
0x4C304(dark) (with the calendar number 0x4C2B4), then the highlight frame 0x4BFFC(k, j), then DrawText 0x358D0 with
the label as OfficeScreen 0x4EE64 calls it (x = min(mouse x, 295) by its rule at 0x4F13D, y = mouse y - 10, fg 0xFE,
shadow 1, font 1), in Unicorn on tac60, with the PCX loader 0x532BC stubbed (DAT/OFFICE.DAT member by name, all
pixels) and OFFBRUSH.PCX in a fresh buffer at the pointer 0x93E5C. The port (port/ja1front ja1f_office) draws on the
same noise. Compared: the 320 x 200 panel buffer; the port's palette must be the room file's (ROOM / ROOMDARK).
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_office.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_uint8] + [ctypes.c_int32] * 4 + \
        [ctypes.c_char_p] + [ctypes.c_int32] * 2 + [ctypes.c_void_p] * 2
    PORT.ja1f_office.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()
LINK, DAY, PANEL_P, BRUSH_P = 0x22A043, 0x229FA4, 0x93E60, 0x93E5C
LABELS = ["Save Game", "Restore Game", "Contact A.I.M.", "Sleep", "View Team", "Control Panel", "Leave", "",
          "99", "Q"]
_pics = {}


def pic(name):
    n = (name.decode("latin-1") if isinstance(name, bytes) else name).upper()
    if n not in _pics:
        f = datarc.member(OFFICE, n)
        w, h, px, _ = pcx.decode(f)
        _pics[n] = (w, h, px, bytes(v >> 2 for v in f[-768:]))
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    w, h, px, _ = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x46BF4, 0x4C26C, 0x47020, 0x53300, 0x52820, 0x52870):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x47040), lambda em, *_: 100)          # TimerElapsedMs: the 90 ms frame wait ends at once
e.stub(e.rt(0x532BC), load, 4)
bw, bh, bpx, _ = pic("OFFBRUSH.PCX")
e.w32(e.rt(BRUSH_P), e.alloc(bytes(bpx)))
panel = e.r32(e.rt(PANEL_P))
text = e.alloc(64)

r = random.Random(1)
bad = 0
for i in range(ncases):
    link = r.choice([0, 0, 1, r.randrange(256)])
    day = r.choice([0, 1, 2, 9, 10, 31, 99, 100, r.randrange(65536)])
    dark = r.random() < 0.3
    k = r.choice([0, 0, 1, 2, 3, 4, 5, 6, 7])
    j = r.randrange(4)
    lab = r.choice(LABELS + [None])
    mx, my = r.choice([r.randrange(320), r.choice([0, 295, 296, 297, 319])]), r.choice([r.randrange(200), 0, 9, 10])
    noise = bytes(r.randrange(256) for _ in range(64000))
    e.w8(e.rt(LINK), link)
    e.w32(e.rt(DAY), day)
    e.uc.mem_write(panel, noise)
    e.call(e.rt(0x4C304), int(dark))
    if k:
        e.call(e.rt(0x4BFFC), k, j)
    if lab is not None:
        e.uc.mem_write(text, lab.encode() + b"\0")
        x = 0x127 if mx > 0x128 else mx
        e.call(e.rt(0x358D0), text, panel, 0x140, x, (my - 10) & 0xFFFFFFFF, 0, 0xFE, 1, 1, 300)
    o = bytes(e.uc.mem_read(panel, 64000))
    if PORT:
        buf = ctypes.create_string_buffer(noise, 64000)
        pal = ctypes.create_string_buffer(768)
        rc = PORT.ja1f_office(ISO, EXE, link, day, int(dark), k, j,
                              None if lab is None else lab.encode(), mx, my, buf, pal)
        p = buf.raw
        want_pal = pic("ROOMDARK.PCX" if dark else "ROOM.PCX")[3]
        if rc != 0 or p != o or pal.raw != want_pal:
            nb = next((q for q in range(64000) if p[q] != o[q]), -1)
            print(f"MISMATCH case {i} link {link} day {day} dark {dark} k {k} j {j} label {lab!r} at ({mx},{my}): "
                  f"rc {rc}, palette {'ok' if pal.raw == want_pal else 'differs'}, " +
                  (f"panel @({nb % 320},{nb // 320}) {o[nb]:#x} vs {p[nb]:#x}" if nb >= 0 else "panel equal"))
            bad += 1
            if bad > 10:
                break
print(f"office_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
