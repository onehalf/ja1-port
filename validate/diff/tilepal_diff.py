"""Differential test: loading the palette of a tileset (LoadTilePalette 0x2A990), original vs port.

usage: .venv/bin/python validate/diff/tilepal_diff.py [DUMP] [--port] [--lib SO]
Per tileset 0..9 (g_tileset 0x22A038) and a noise start: the original runs in Unicorn on tac60, OpenCDFile 0x38304 /
fread_ / fclose_ read MISC\\TILE<n>.PAL through common.game_read. Result = the 768 bytes at 0xFA608, the DAC copy
0xEDA08 (4 bytes per colour: 0x52820 writes R G B, the 4th byte stays) and the shade byte 0x284F08 (always 0x1C).
--port: ja1f_palette gives the 8-bit colours of the same tileset; compared with the original's 6-bit bytes
through the port's VGA scale (v * 4 + v / 16). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from common import game_read         # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_palette.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_palette.restype = ctypes.c_int32
    ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()

LOAD, TILESET, RAW, DAC, SHADE, DATFILE = 0x2A990, 0x22A038, 0xFA608, 0xEDA08, 0x284F08, 0x92F54
FAKE_FP = 0x7777
e = Emu(dump=dump, fast=True)
cur, names = {}, []


def open_cd(em, eax, edx, ebx, ecx):
    name = e.cstr(eax).decode("latin-1").upper()
    names.append(name)
    cur["data"], cur["pos"] = game_read(name), 0
    e.w32(e.rt(DATFILE), FAKE_FP)
    return FAKE_FP


def fread(em, eax, edx, ebx, ecx):
    assert ecx == FAKE_FP
    d = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(d)
    e.uc.mem_write(eax, d)
    return len(d) // edx if edx else 0


e.stub_sprintf()
e.stub(e.rt(0x38304), open_cd)
e.stub(e.rt(0x758AD), fread)
e.stub(e.rt(0x75A77), lambda em, *_: 0)
r = random.Random(3)
bad = 0
for t in range(10):
    names.clear()
    e.w8(e.rt(TILESET), t)
    e.uc.mem_write(e.rt(RAW), bytes(r.randrange(256) for _ in range(768)))
    dac0 = bytes(r.randrange(256) for _ in range(1024))
    e.uc.mem_write(e.rt(DAC), dac0)
    e.w8(e.rt(SHADE), r.randrange(256))
    e.call(e.rt(LOAD))
    raw, dac = bytes(e.uc.mem_read(e.rt(RAW), 768)), bytes(e.uc.mem_read(e.rt(DAC), 1024))
    want_dac = bytes(raw[3 * (k // 4) + k % 4] if k % 4 < 3 else dac0[k] for k in range(1024))
    if names != [f"MISC\\TILE{t}.PAL"] or dac != want_dac or e.r8(e.rt(SHADE)) != 0x1C:
        print(f"tileset {t}: files {names}, DAC copy ok {dac == want_dac}, shade {e.r8(e.rt(SHADE)):#x}")
        bad += 1
    if PORT is None:
        continue
    out = ctypes.create_string_buffer(768)
    rc = PORT.ja1f_palette(ISO, t, out)
    want = bytes(min(255, (v & 63) * 4 + (v & 63) // 16) for v in raw)
    if rc != 0 or out.raw != want:
        print(f"tileset {t}: rc {rc}, {sum(a != b for a, b in zip(out.raw, want))} colour bytes differ")
        bad += 1
print(f"tilepal_diff{' --port' if PORT else ''}: 10 tilesets, {bad} mismatches")
sys.exit(1 if bad else 0)
