"""Differential test: the office's team ledger page (strategic.md "The team ledger"), original vs port.

usage: .venv/bin/python validate/diff/ledger_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: a fuzzed team (0..8 mercs in team slots 0..7: inCurSector, hp, hpMax, wounds, salary2, level, kills,
nickname), fuzzed face buffers, accounts fields (day, sectors won / lost / owned, last sector gained (59 = none),
factory capacity, trees, guards / tappers / team pay, income, funds) and difficulty, on a noise screen and a noise
room buffer (g_panelBuf). The original DrawOfficeLedger 0x4C41C runs in Unicorn on tac60 with CalcLedgerAll
0x51178 stubbed (the port's count is ai_diff's), the PCX loader 0x532BC stubbed (DAT/OFFICE.DAT member by name, all
pixels) and the screen pointer 0x95678 on a fresh buffer; the first mouse poll 0x678E8 takes the screen and the room
buffer and presses the left button (the page closes; DrawRoomScreen 0x4C304 is stubbed). The port (port/ja1front
ja1f_ledger) draws on the same noise. Compared: the screen, the room buffer, the palette (VIEWISLE.PCX's).
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
    PORT.ja1f_ledger.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_char_p] * 3 + [ctypes.c_uint8] + \
        [ctypes.c_void_p] * 3
    PORT.ja1f_ledger.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()
SOLDIERS, ACC, DIFF, PANEL_P, SCREEN_P, FACES, BUTTONS = 0xFA958, 0x229FA0, 0x22A0CC, 0x93E60, 0x95678, 0x93428, \
    0x229CF4
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


shot = {}


def poll(em, *_):
    if "screen" not in shot:
        shot["screen"] = bytes(e.uc.mem_read(screen, 64000))
        shot["room"] = bytes(e.uc.mem_read(panel, 64000))
    e.w32(e.rt(BUTTONS), 1)
    return 0


for a in (0x67BF0, 0x673AC, 0x67C54, 0x674B0, 0x53300, 0x52820, 0x51178, 0x4C304, 0x46BF4, 0x29D54):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x5616C), lambda em, *_: 0)            # KeyPressed: no key
e.stub(e.rt(0x52908), lambda em, *_: 0, 4)         # palette fade
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x532BC), load, 4)
screen = e.alloc(bytes(64000))
e.w32(e.rt(SCREEN_P), screen)
panel = e.r32(e.rt(PANEL_P))
faces = [e.r32(e.rt(FACES + 4 * k)) for k in range(8)]
r = random.Random(1)
LETTERS = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz .-'"


def i32(v):
    return v & 0xFFFFFFFF


bad = 0
for i in range(ncases):
    n = r.choice([0, 1, 2, 3, 4, 5, 7, 8, 8, r.randrange(9)])
    for k in range(9):
        b = e.rt(SOLDIERS + k * 0x200)
        e.w8(b + 0x182, (r.randrange(1, 256) if r.random() < 0.2 else 1) if k < n else 0)
        # The bar stays inside the buffers (hp >= -3, 0 <= wounds <= hpMax - hp + 5): else the original draws
        # rows above the screen, or (fewer than 0 rows) copies 4 G rows.
        hm = r.choice([r.randrange(1, 101), 100, 1, r.randrange(128)])
        hp = r.choice([hm, r.randrange(-3, hm + 1), 0])
        e.w8(b + 0x183, hp)
        e.w8(b + 0x184, hm)
        e.w8(b + 0x185, r.choice([0, 0, r.randrange(0, hm - hp + 6)]))
        e.w16(b + 0x112, r.choice([r.randrange(3000), 0, 65535, r.randrange(65536)]))
        e.w8(b + 0x192, r.choice([r.randrange(1, 11), r.randrange(256)]))
        e.w8(b + 0x1D9, r.choice([0, r.randrange(50), r.randrange(256)]))
        nick = bytes(r.choice(LETTERS) for _ in range(r.randrange(10)))
        e.uc.mem_write(b + 0x16E, nick + b"\0" * (10 - len(nick)))
    for k in range(8):
        e.uc.mem_write(faces[k], bytes(r.randrange(256) for _ in range(1024)))
    big = lambda: r.choice([0, r.randrange(100), r.randrange(100000), r.randrange(-1000, 0), r.randrange(1 << 32),
                            9999, 10000, 99999, 100000, 999999, 1000000])
    e.w32(e.rt(ACC + 4), r.choice([1, 2, 9, 99, 100, 999, 1000, big()]))
    for o in (0x18, 0x20, 0x24, 0x38, 0x58, 0x5C, 0x74, 0x78, 0x7C, 0x84):
        e.w32(e.rt(ACC + o), i32(big()))
    e.w32(e.rt(ACC + 0x80), i32(r.choice([59, 59, r.randrange(60), -1, big()])))
    diff = r.choice([0, 1, 2, 3, 255, r.randrange(256)])
    e.w8(e.rt(DIFF), diff)
    noise_s = bytes(r.randrange(256) for _ in range(64000))
    noise_r = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, noise_s)
    e.uc.mem_write(panel, noise_r)
    shot.clear()
    e.w32(e.rt(BUTTONS), 0)
    e.call(e.rt(0x4C41C))
    if "screen" not in shot:
        print(f"case {i}: the original did not poll the mouse")
        bad += 1
        break
    if PORT:
        so = bytes(e.uc.mem_read(e.rt(SOLDIERS), 8 * 0x200))
        fa = b"".join(bytes(e.uc.mem_read(faces[k], 1024)) for k in range(8))
        ac = bytes(e.uc.mem_read(e.rt(ACC), 0xA8))
        sb = ctypes.create_string_buffer(noise_s, 64000)
        rb = ctypes.create_string_buffer(noise_r, 64000)
        pal = ctypes.create_string_buffer(768)
        rc = PORT.ja1f_ledger(ISO, EXE, so, fa, ac, diff, rb, sb, pal)
        want_pal = pic("VIEWISLE.PCX")[3]
        if rc != 0 or sb.raw != shot["screen"] or rb.raw != shot["room"] or pal.raw != want_pal:
            def first(p, o):
                q = next((q for q in range(64000) if p[q] != o[q]), -1)
                return f"@({q % 320},{q // 320}) {o[q]:#x} vs {p[q]:#x}" if q >= 0 else "equal"
            print(f"MISMATCH case {i} team {n} difficulty {diff}: rc {rc}, palette "
                  f"{'ok' if pal.raw == want_pal else 'differs'}, screen {first(sb.raw, shot['screen'])}, "
                  f"room {first(rb.raw, shot['room'])}")
            bad += 1
            if bad > 10:
                break
print(f"ledger_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
