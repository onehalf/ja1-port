"""Differential test: the sector tmp file (op 219 save_tmp), original SaveSectorTmp 0x2F800 vs port.

usage: .venv/bin/python validate/diff/tmpsave_diff.py [ncases=40] [DUMP] [--port] [--lib SO]
Per case on a tactical dump (tac60): fuzzed g_tileMap (3136 x 8 B), g_groundItems (0x960 B) and g_curSector.
The original runs in Unicorn: sprintf_ by the harness (stub_sprintf), OpenMiscFile? 0x38370 logs the file name and returns a fake FILE,
fwrite_ 0x75FE3 collects the bytes, fclose_ 0x75A77 returns 0. Without --port the bytes are checked against the
layout of world_load.md section 8 (per tile bytes 1, 2, 3, 4, 6, then the ground items) and the name sec%02d.tmp;
--port: ja1f_tmp_file (port/ja1front host::make_tmp_file) makes the file from the same tiles and ground items.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 40
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_tmp_file.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p]
    PORT.ja1f_tmp_file.restype = ctypes.c_int32
TILEMAP, GROUND, CUR_SECTOR, FAKE_FP = 0x100A18, 0xEB160, 0x22A039, 0x7F00

e = Emu(dump=dump, fast=True)
e.stub_sprintf()
rt = e.rt
names, out = [], bytearray()


def cstr(a):
    b = bytes(e.uc.mem_read(a, 32))
    return b[:b.index(0)].decode("latin-1")


def open_misc(em, eax, *_):
    names.append(cstr(eax))
    return FAKE_FP


def fwrite(em, eax, edx, ebx, ecx):
    out.extend(e.uc.mem_read(eax, edx * ebx))
    return ebx


e.stub(rt(0x38370), open_misc)
e.stub(rt(0x75FE3), fwrite)
e.stub(rt(0x75A77), lambda em, *_: 0)
rng = random.Random(219)
fails = 0
for case in range(ncases):
    tiles = bytes(rng.randrange(256) for _ in range(3136 * 8)) if case % 4 else bytes(e.uc.mem_read(rt(TILEMAP), 3136 * 8))
    ground = bytes(rng.randrange(256) for _ in range(0x960)) if case % 3 else bytes(e.uc.mem_read(rt(GROUND), 0x960))
    sector = rng.choice([0, 5, 59, rng.randrange(256)])
    e.uc.mem_write(rt(TILEMAP), tiles)
    e.uc.mem_write(rt(GROUND), ground)
    e.w8(rt(CUR_SECTOR), sector)
    names.clear()
    out.clear()
    e.call(rt(0x2F800))
    got = bytes(out)
    if PORT:
        buf = ctypes.create_string_buffer(18080)
        n = PORT.ja1f_tmp_file(tiles, ground, buf)
        want = buf.raw[:n]
    else:
        want = b"".join(bytes(tiles[8 * t + k] for k in (1, 2, 3, 4, 6)) for t in range(3136)) + ground
    want_names = ["sec%02d.tmp" % sector]
    if (got, names) != (want, want_names):
        fails += 1
        if fails <= 5:
            d = [i for i in range(min(len(got), len(want))) if got[i] != want[i]][:8]
            print(f"MISMATCH case {case} sector {sector}: names {names} / {want_names}, len {len(got)} / {len(want)}, "
                  f"first bytes off {d}")
print(f"tmpsave_diff{' --port' if PORT else ''}: {ncases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
