"""Differential test: the game-over screen (ending.md "The game-over screen"), original vs port.

usage: .venv/bin/python validate/diff/gameover_diff.py [DUMP] [--port] [--lib SO]
The original GameOverScreen 0x504B4 runs in Unicorn on tac60 over a noise screen, with the PCX loader 0x532BC stubbed
(DAT/OFFICE.DAT member by name, all pixels), the sound, mouse and palette calls stubbed and the key/click wait 0x67E44
answering at once. The port (port/ja1front ja1f_game_over) draws on the same noise. Compared: the 320 x 200 screen; the
port's palette must be VIEWISLE.PCX's (6-bit); the original must fade in over 24 steps and out over 56 steps.
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
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()

e = Emu(dump=dump, fast=True)
fades = []


def load(em, eax, edx, ebx, ecx):
    f = datarc.member(OFFICE, e.cstr(ebx).decode("latin-1").upper())
    w, h, px, _ = pcx.decode(f)
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def fade(em, eax, edx, ebx, ecx):
    fades.append((ebx, ecx, e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)))
    return 0


for a in (0x5C82C, 0x5CE08, 0x67BF0, 0x674B0, 0x5E8E0, 0x53300, 0x52820, 0x67C54, 0x673AC, 0x678E8):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x52908), fade, 4)
e.stub(e.rt(0x67E44), lambda em, *_: 1)
scr = e.r32(e.rt(0x95678))
noise = bytes(random.Random(1).randrange(256) for _ in range(64000))
e.uc.mem_write(scr, noise)
e.call(e.rt(0x504B4))
o = bytes(e.uc.mem_read(scr, 64000))
bad = 0
if fades != [(24, 0, 256), (56, 256, 0)]:
    print("original fades:", fades)
    bad += 1
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_game_over.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 2
    PORT.ja1f_game_over.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    buf, pal = ctypes.create_string_buffer(noise, 64000), ctypes.create_string_buffer(768)
    rc = PORT.ja1f_game_over(ISO, EXE, buf, pal)
    want = bytes(v >> 2 for v in datarc.member(OFFICE, "VIEWISLE.PCX")[-768:])
    p = buf.raw
    if rc != 0 or p != o or pal.raw != want:
        nb = next((q for q in range(64000) if p[q] != o[q]), -1)
        print(f"MISMATCH rc {rc}, palette {'ok' if pal.raw == want else 'differs'}, " +
              (f"screen @({nb % 320},{nb // 320}) {o[nb]:#x} vs {p[nb]:#x}" if nb >= 0 else "screen equal"))
        bad += 1
print(f"gameover_diff: {bad} mismatches" + ("" if "--port" in argv else " (original only)"))
sys.exit(1 if bad else 0)
