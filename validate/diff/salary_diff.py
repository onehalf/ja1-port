"""Differential test: the A.I.M. salary panel (strategic.md "The salary panel"), original vs port.

usage: .venv/bin/python validate/diff/salary_diff.py [ncases=80] [DUMP] [--port] [--lib SO]
Per case on an office dump (day3): a random team record k (0..7) with fuzzed salary, hp and stats +0x18B .. +0x192,
and a random screen. The original runs ShowSalaryPanel 0x221E8 in Unicorn: the picture loader 0x532BC writes
PANEL_S.PCC (MAIN.DAT), the mouse calls (0x67BF0, WaitRightUp 0x67C68, 0x673AC) and the screen part save / restore
(0x467E4, 0x468B0) are stubbed, so the panel stays on the screen. DrawNumber and the text run for real.
--port: ja1f_aim_salary draws the panel over the same screen; the screens are compared byte by byte. Then the
pressed faces of the A.I.M. buttons 0..4 (strategic.md "Button faces"): DrawPanelButtonDown? 0x35DA4 vs ja1f_aim_button.
Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
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
ncases = int(args[0]) if args else 80
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_day3.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_aim_salary.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_char_p, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_aim_salary.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SCREEN_P, SOL = 0x95678, 0xFA958

e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(SCREEN_P))
loads = []


def load(em, eax, edx, ebx, ecx):
    # 0x532BC also writes the picture's PCX header to 0x936BE (BlitCopy reads its x / y maxima at 0x936C6 / 0x936C8).
    m = datarc.member(MAIN, em.cstr(ebx).decode("latin-1").upper())
    w, h, px, _ = pcx.decode(m)
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    em.uc.mem_write(em.rt(0x936BE), m[:128])
    loads.append(em.cstr(ebx))
    return 0


for a in (0x67BF0, 0x67C68, 0x673AC, 0x467E4, 0x468B0):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)

snap = e.uc.mem_read(e.rt(0x90000), 0x33A730 - 0x90000)
r = random.Random(11)
bad = 0
for i in range(ncases):
    e.uc.mem_write(e.rt(0x90000), bytes(snap))
    k = i % 8 if i < 16 else r.randrange(8)
    s = SOL + 0x200 * k
    e.uc.mem_write(e.rt(s + 0x110), struct.pack("<H", r.choice([0, 1, 450, 800, r.randrange(5000),
                                                                r.randrange(65536)])))
    e.w8(e.rt(s + 0x183), r.choice([0, 1, 50, 100, r.randrange(256)]))
    for o in range(0x18B, 0x193):
        e.w8(e.rt(s + o), r.choice([0, 7, 10, r.randrange(101), r.randrange(256)]))
    team = bytes(e.uc.mem_read(e.rt(SOL), 8 * 512))
    before = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, before)
    e.call(e.rt(0x221E8), k)
    orig = bytes(e.uc.mem_read(screen, 64000))
    assert sum(a != b for a, b in zip(orig, before)) > 5000, "the original drew no panel"
    if PORT:
        out = ctypes.create_string_buffer(before, 64000)
        rc = PORT.ja1f_aim_salary(ISO, EXE, team, k, out)
        if rc != 0:
            print(f"MISMATCH case {i}: port returned {rc}")
            bad += 1
        elif out.raw != orig:
            n = next(m for m in range(64000) if orig[m] != out.raw[m])
            print(f"MISMATCH case {i} k {k}: @({n % 320},{n // 320}) {orig[n]:#x} vs {out.raw[n]:#x}")
            bad += 1
    if bad > 10:
        break
assert loads and all(x == b"panel_s.pcc" for x in loads), loads[:3]
# strategic.md "Button faces": DrawPanelButtonDown? 0x35DA4 (b) for the A.I.M. buttons 0..4 (button table 0x9345A;
# the label of button 2 at 0x966D5 = the view after the press: "BIO" when the view was 0 (bio), else "GEAR").
nb = 0
if PORT:
    PORT.ja1f_aim_button.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_aim_button.restype = ctypes.c_int32
for b in range(5):
    for view in (0, 1):
        e.uc.mem_write(e.rt(0x90000), bytes(snap))
        e.uc.mem_write(e.rt(0x966D5), b"BIO\0" if view == 0 else b"GEAR\0")
        before = bytes(r.randrange(256) for _ in range(64000))
        e.uc.mem_write(screen, before)
        e.call(e.rt(0x35DA4), b)
        orig = bytes(e.uc.mem_read(screen, 64000))
        assert orig != before, "the original drew no pressed face"
        nb += 1
        if PORT:
            out = ctypes.create_string_buffer(before, 64000)
            rc = PORT.ja1f_aim_button(ISO, EXE, b, view, out)
            if rc != 0 or out.raw != orig:
                n = next((m for m in range(64000) if orig[m] != out.raw[m]), 0)
                print(f"MISMATCH button {b} view {view}: rc {rc} @({n % 320},{n // 320}) {orig[n]:#x} vs {out.raw[n]:#x}")
                bad += 1
print(f"salary_diff: {i + 1} cases + {nb} button faces, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
