"""Differential test: the item info box (panel.md "Item info box"), original vs port.

usage: .venv/bin/python validate/diff/iteminfo_diff.py [ncases=400] [DUMP] [--port] [--lib SO]
Per case: a profile, an item type (every type, money 0x6F and the picture items often), a count, a value, the
numbers flag a, the info flag, the options Item Info and Subtitles and a noise screen. The original runs 0x25D20
(the item info box) in Unicorn on tac60 with the voice stubbed (ArcSeek finds nothing, the sound calls do nothing),
PlayQuote 0x24F08 reading EDT\\INVQUOTE.EDT and the PCC loader 0x532BC reading MAIN.DAT by name; the screen is
taken at the first poll 0x678E8 of the wait (which then presses the left button). The box must be gone after the
call (the screen back to the noise) unless it was a picture. The port (port/ja1front ja1f_item_info) draws on the
same noise; the 320 x 200 screens and the results are compared.
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
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 400
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_item_info.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 7 + [ctypes.c_void_p]
    PORT.ja1f_item_info.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
QUOTES = open(os.path.join(ROOT, "rom", "EDT", "INVQUOTE.EDT"), "rb").read()
SCREEN_P, BUTTONS, OPT_INFO, OPT_SUB = 0x95678, 0x229CF4, 0x1163BE, 0x1163BD
PICTURES = [89, 90, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 115, 116, 135]
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
            _pics[n] = (w, h, px)
        except Exception:
            _pics[n] = None
    return _pics[n]


e = Emu(dump=dump, fast=True)
loads = []


def load(em, eax, edx, ebx, ecx):
    name = e.cstr(ebx)
    p = pic(name)
    loads.append(name)
    if p is None:
        return 1
    w, h, px = p
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def quote(em, eax, edx, ebx, ecx):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    buf = e.r32(sp + 4)
    at = ((edx & 0xFFFFFFFF) + ebx) * ecx
    e.uc.mem_write(buf, QUOTES[at:at + ecx].ljust(ecx, b"\0"))
    return 0


screen = e.r32(e.rt(SCREEN_P))
held = []


MOUSE_X, MOUSE_Y = 0x229D00, 0x229D04
mouse_log, mouse_step = [], [0, 0]


def poll(em, *_):
    if not held:
        held.append(bytes(em.uc.mem_read(screen, 64000)))
        e.w32(e.rt(MOUSE_X), (e.r32(e.rt(MOUSE_X)) + mouse_step[0]) & 0xFFFFFFFF)
        e.w32(e.rt(MOUSE_Y), (e.r32(e.rt(MOUSE_Y)) + mouse_step[1]) & 0xFFFFFFFF)
    e.w8(e.rt(BUTTONS), 1)
    return 0


def mouse_set(em, eax, edx, *_):
    mouse_log.append((1, eax, edx))
    e.w32(e.rt(MOUSE_X), eax)
    e.w32(e.rt(MOUSE_Y), edx)
    return 0


saved_place = [0, 0]


def mouse_save(em, *_):
    saved_place[:] = [e.r32(e.rt(MOUSE_X)), e.r32(e.rt(MOUSE_Y))]
    mouse_log.append((0, *saved_place))
    return 0


def mouse_back(em, *_):
    mouse_log.append((2, *saved_place))
    return 0


for a in (0x67BF0, 0x673AC, 0x67C54, 0x67C68, 0x47020, 0x46EC4,
          0x5C0EC, 0x5CC6C, 0x5C960, 0x5C9C0, 0x5CCF8, 0x5CE08, 0x5C144, 0x25CA4, 0x25BDC):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x681D0), mouse_set)
e.stub(e.rt(0x67E18), mouse_save)
e.stub(e.rt(0x67E30), mouse_back)
e.stub(e.rt(0x534A4), lambda em, *_: 1, 4)          # ArcSeek: no voice sample
e.stub(e.rt(0x5CA74), lambda em, *_: 1)             # the voice still plays: no next voice step
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x24F08), quote, 16)
e.stub(e.rt(0x678E8), poll)
e.stub_sprintf()


def strstr(em, eax, edx, *_):
    k = e.cstr(eax).find(e.cstr(edx))
    return 0 if k < 0 else eax + k


e.stub(e.rt(0x75BB9), strstr)                       # its segment pushes fault in Unicorn
e.stub(e.rt(0x756A0), lambda em, eax, edx, ebx, ecx: e.uc.mem_write(eax, bytes([edx & 0xFF]) * ebx) or eax)  # memset_


def case(r, i):
    k = r.random()
    if i < 136:
        t = i or 0x6F
    elif k < .2:
        t = r.choice(PICTURES)
    elif k < .35:
        t = 0x6F
    else:
        t = r.randrange(1, 136)
    count = r.choice([1, 1, 2, 5, 99, -1, -3, r.randrange(-128, 128)])
    value = r.choice([r.randrange(0, 101), r.randrange(0, 12), r.randrange(-5, 140), r.randrange(0, 32768)])
    if t == 0x6F and r.random() < .5:
        value = r.choice([0, 1, 50, 250, 1000, 32767, r.randrange(0, 5000)])
    if 110 <= value & 0xFF < 128:        # the condition bar of the original then never ends (crash)
        value = 100
    return dict(profile=r.randrange(0, 64), t=t, count=count, value=value, a=r.choice([1, 1, 0]), b=r.choice([0, 1]),
                info=r.choice([0, 1]), opt_info=1 if i < 136 else r.choice([1, 1, 1, 0]), opt_sub=r.choice([0, 1]),
                # the mouse: at the box edges (x 52..267, y 37..116), and a move in the wait around the limit of 10
                mx=r.choice([51, 52, 267, 268, r.randrange(0, 320)]), my=r.choice([36, 37, 116, 117, r.randrange(0, 200)]),
                dx=r.choice([0, -9, 9, -10, 10, r.randrange(-40, 40)]), dy=r.choice([0, -9, 9, -10, 10, r.randrange(-40, 40)]))


# A picture item loads its picture into the panel sheet; the original then reloads the sheet (LoadPanels), which the
# loader stub can not do (its files are not in MAIN.DAT): put the sheet back before each case instead.
SHEET = e.r32(e.rt(0x93454))
sheet = bytes(e.uc.mem_read(SHEET, 0xC800))
r = random.Random(1)
bad = 0
cases = [case(r, i) for i in range(ncases)]
for i, c in enumerate(cases):
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    e.uc.mem_write(SHEET, sheet)
    e.w8(e.rt(OPT_INFO), c["opt_info"])
    e.w8(e.rt(OPT_SUB), c["opt_sub"])
    e.w8(e.rt(BUTTONS), 0)
    held.clear()
    loads.clear()
    mouse_log.clear()
    e.w32(e.rt(MOUSE_X), c["mx"])
    e.w32(e.rt(MOUSE_Y), c["my"])
    mouse_step[:] = c["dx"], c["dy"]
    if "-v" in argv: print("CASE", i, c, flush=True)
    rc = e.call(e.rt(0x25D20), c["profile"], c["t"], c["count"] & 0xFF, c["value"] & 0xFFFF, c["a"], c["b"],
                c["info"]) & 0xFF
    after = bytes(e.uc.mem_read(screen, 64000))
    if len(held) != 1:
        print(f"case {i}: the wait polled {len(held)} times")
        bad += 1
        continue
    o = held[0]
    if rc == 0 and after != base:
        print(f"case {i} {c}: the screen after the box is not the screen before")
        bad += 1
    if PORT:
        buf = ctypes.create_string_buffer(base, 64000)
        prc = PORT.ja1f_item_info(ISO, EXE, c["profile"], c["t"], c["count"], c["value"], c["a"],
                                  c["opt_info"] | c["opt_sub"] << 1, c["info"], buf)
        p = buf.raw
        out = (ctypes.c_int32 * 12)()
        nm = PORT.ja1f_item_mouse(c["opt_info"], c["info"], c["t"] in PICTURES, c["mx"], c["my"], c["dx"], c["dy"], out)
        pm = [tuple(out[3 * k:3 * k + 3]) for k in range(nm)]
        # A picture item has no save and no move, and its end compares the mouse with two locals that nothing wrote
        # (stack contents; accepted difference, #2): the port never moves the mouse there.
        if c["t"] in PICTURES:
            mouse_log[:] = [x for x in mouse_log if x[0] != 2]
        if pm != mouse_log:
            print(f"MISMATCH case {i} {c}: mouse {mouse_log} vs {pm}")
            bad += 1
        if prc != rc or p != o:
            n = next((k for k in range(64000) if p[k] != o[k]), -1)
            print(f"MISMATCH case {i} {c}: rc {rc} vs {prc}" + (f", screen @({n % 320},{n // 320}) {o[n]:#x} vs "
                                                                 f"{p[n]:#x}" if n >= 0 else ""))
            bad += 1
            if "-d" in argv and n >= 0:
                y0, x0 = n // 320, n % 320
                for yy in range(y0 - 1, y0 + 4):
                    print(yy, o[yy * 320 + x0 - 2:yy * 320 + x0 + 8].hex(), p[yy * 320 + x0 - 2:yy * 320 + x0 + 8].hex())
            if bad > 10:
                break
    elif "-v" in argv:
        print(i, c, rc, loads, sum(a != b for a, b in zip(o, base)))
print(f"iteminfo_diff: {len(cases)} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
