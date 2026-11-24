"""Differential test: the A.I.M. statistics page (strategic.md "The A.I.M. statistics page"), original vs port.

usage: .venv/bin/python validate/diff/aimstats_diff.py [ncases=60] [DUMP] [--port] [--lib SO]
Per case on an office dump (day3): fuzzed accounts bytes (day, sorties, success points, hirings, fatalities,
terminations, resignations, non-payments, balance) and a fuzzed team (size, health / agility / marksmanship / wisdom,
salary). The original runs AimStatsScreen? 0x224C4 in Unicorn: the PCX palette loader 0x53300, palette set 0x52820
and the fades 0x52908 are stubbed, the picture loader 0x532BC writes STATBGND.PCX (MAIN.DAT), the mouse calls are
stubbed, KeyPressed = 1 and GetKey = Enter end the page at once. CalcLedgerAll and PerformanceRating run for real.
--port: ja1f_aim_stats draws the page from the same team, the accounts as they are after the call and the rating
(PerformanceRating 0x62EB4 run again on the state after the call); the screens are compared byte by byte.
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
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_day3.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_aim_stats.argtypes = [ctypes.c_char_p] * 4 + [ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_aim_stats.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SCREEN_P, SOL, ACC = 0x95678, 0xFA958, 0x229FA0

e = Emu(dump=dump, fast=not os.environ.get("AS_TRACE"))
e.stub_sprintf()
screen = e.r32(e.rt(SCREEN_P))


def load(em, eax, edx, ebx, ecx):
    w, h, px, _ = pcx.decode(datarc.member(MAIN, em.cstr(ebx).decode("latin-1").upper()))
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


e.stub(e.rt(0x52908), lambda em, *_: 0, 4)
for a in (0x53300, 0x52820, 0x678E8, 0x673AC, 0x67BF0, 0x67C54, 0x67C68):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x5616C), lambda em, *_: 1)
e.stub(e.rt(0x560F4), lambda em, *_: 13)

snap = e.uc.mem_read(e.rt(0x90000), 0x33A730 - 0x90000)
r = random.Random(7)
bad = 0
for i in range(ncases):
    e.uc.mem_write(e.rt(0x90000), bytes(snap))

    def s8(a, v):
        e.w8(e.rt(a), v & 0xFF)
    for o, vals in ((0x00, [0, 0, 1, 3, r.randrange(256)]), (0x9A, [0, 1, 5, 20, r.randrange(256)]),
                    (0x9C, [0, 0, 1, r.randrange(256)]), (0x9D, [0, 0, 1, 4, 10, r.randrange(256)]),
                    (0x9F, [0, 3, 30, r.randrange(256)]), (0xA0, [0, 0, 1, r.randrange(256)]),
                    (0xA1, [0, 0, 1, 2, r.randrange(256)])):
        s8(ACC + o, r.choice(vals))
    e.uc.mem_write(e.rt(ACC + 4), struct.pack("<i", r.choice([0, 1, 2, 5, 20, 40, 255, r.randrange(-3, 400)])))
    size = r.choice([0, 1, 3, 5, 8, r.randrange(9)])
    for k in range(8):
        s = SOL + 0x200 * k
        s8(s + 0x182, r.randrange(1, 3) if k < size else 0)
        if r.random() < 0.7:
            for o in (0x184, 0x18B, 0x191, 0x18D):
                s8(s + o, r.choice([r.randrange(101), r.randrange(256)]))
            e.uc.mem_write(e.rt(s + 0x110), struct.pack("<H", r.choice([r.randrange(5000), r.randrange(65536)])))
    team = bytes(e.uc.mem_read(e.rt(SOL), 8 * 512))
    e.uc.mem_write(screen, bytes(r.randrange(256) for _ in range(64000)))
    e.trace = bool(os.environ.get("AS_TRACE"))
    e.call(e.rt(0x224C4))
    orig = bytes(e.uc.mem_read(screen, 64000))
    if PORT:
        acc = bytes(e.uc.mem_read(e.rt(ACC), 0xA8))
        rating = e.call(e.rt(0x62EB4)) & 0xFF
        out, pal = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(768)
        rc = PORT.ja1f_aim_stats(ISO, EXE, team, acc, rating, out, pal)
        if rc != 0:
            print(f"MISMATCH case {i}: port returned {rc}")
            bad += 1
        elif out.raw != orig:
            n = next(m for m in range(64000) if orig[m] != out.raw[m])
            print(f"MISMATCH case {i} size {size} day {struct.unpack('<i', acc[4:8])[0]} rating {rating}: "
                  f"@({n % 320},{n // 320}) {orig[n]:#x} vs {out.raw[n]:#x}")
            if os.environ.get("AS_DEBUG"):
                open("/tmp/as_o.bin", "wb").write(orig); open("/tmp/as_p.bin", "wb").write(out.raw)
            bad += 1
    if bad > 10:
        break
print(f"aimstats_diff: {i + 1} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
