"""Differential test: LoadTerrainTileset 0x2A9FC, original vs port (front-end data::fill_map_record + table28_from_lst).

usage: .venv/bin/python validate/diff/terrainload_diff.py [ncases=120] [DUMP]

The original runs in Unicorn on a tactical dump (mem_tac60): EAX = 0-based sector, DL = 0. Its file calls are
served by Python: OpenCDFile 0x38304 records the name, fseek_ 0x757B1 moves the read position, fread_ 0x758AD
copies from DAT\\TERRAIN.DAT of the case record or from MISC\\tileset<n>.lst (one byte per call, as the game does),
fclose_ 0x75A77 returns 0. OpenTilesDat 0x36EA0, LoadTilePalette 0x2A990 (sets the shade byte 0x284F08 = 0x1C the
real one sets) and FormatSectorDatName 0x2F7C8 are stubbed; sprintf_ is the harness stub_sprintf. Before each run
the base table 28 that JAGGED.DAT gives is written to the dump's g_remapTables[28] target, so both sides start
equal, and table 29's two fixed bytes are zeroed before each run; the original writes them, the port applies them
once at startup (data::Common, ja1f_common_remap): the two bytes the original wrote must equal the port's table 29
(box 1 review #45 c13004: they were asserted on the original side only).

Cases: every one of the 60 real sector records with its real .lst, then `ncases` fuzz cases with a random record,
a random base table and an .lst that is 0xFF at random places (the keep-the-base byte rule, which no shipped .lst
uses) and forced values at the special indices 0x2E 0xA2 0xA3 0xB9 0xF5 0xF7 0xF9.

Port side: ja1f_terrain_load (data::fill_map_record, data::table28_from_lst) on the same record, base table and
.lst. The 3136 x 8 map, the tileset byte and table 28 (256 bytes) must match byte for byte.
Exits 1 on any mismatch.
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from common import cd_read          # noqa: E402
from emu import Emu                 # noqa: E402

LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_terrain_load.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_size_t,
                                   ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
PORT.ja1f_terrain_load.restype = ctypes.c_int32

LOAD, OPEN, FSEEK, FREAD, FCLOSE = 0x2A9FC, 0x38304, 0x757B1, 0x758AD, 0x75A77
SPRINTF, TILESDAT, PALETTE, FMTNAME = 0x75668, 0x36EA0, 0x2A990, 0x2F7C8
TILEMAP, TILESET, REMAPS, SHADE = 0x100A18, 0x22A038, 0xE8C48, 0x284F08
RECORD, TILES, SPECIAL = 15681, 3136, (0x2E, 0xA2, 0xA3, 0xB9, 0xF5, 0xF7, 0xF9)
args = [a for a in sys.argv[1:] if not a.startswith("-")]
NCASES = int(args[0]) if args else 120
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
TERRAIN = cd_read("DAT/TERRAIN.DAT")
LSTS = {t: cd_read("MISC/TILESET%d.LST" % t) for t in range(10)}
JAGGED = cd_read("DAT/JAGGED.DAT")
BASE28 = JAGGED[13424 + 28 * 256:13424 + 29 * 256]

e = Emu(dump=DUMP, fast=True)
e.stub_sprintf()
rt = e.rt
addr28 = e.r32(rt(REMAPS) + 28 * 4)
addr29 = e.r32(rt(REMAPS) + 29 * 4)
cur = {"data": b"", "pos": 0, "terrain": b"", "lst": b""}
names = []


def cstr(a):
    b = bytes(e.uc.mem_read(a, 64))
    return b.split(b"\0")[0].decode("latin-1")


def open_cd(em, eax, *_):
    name = cstr(eax)
    names.append(name)
    if name.upper().startswith("DAT"):
        cur["data"], cur["pos"] = cur["terrain"], 0
    else:
        cur["data"], cur["pos"] = cur["lst"], 0
    em.w32(rt(0x92F54), 1)          # g_datFile
    return 1


def fseek(em, eax, edx, ebx, ecx):
    cur["pos"] = edx
    return 0


def fread(em, eax, edx, ebx, ecx):
    n = edx * ebx
    d = cur["data"][cur["pos"]:cur["pos"] + n]
    if len(d) != n:
        raise AssertionError("fread past the file: pos %d n %d of %d" % (cur["pos"], n, len(cur["data"])))
    em.uc.mem_write(eax, d)
    cur["pos"] += n
    return ebx


def palette(em, *_):
    em.w8(rt(SHADE), 0x1C)
    return 0


e.stub(rt(OPEN), open_cd)
e.stub(rt(FSEEK), fseek)
e.stub(rt(FREAD), fread)
e.stub(rt(FCLOSE), lambda em, *_: 0)
e.stub(rt(TILESDAT), lambda em, *_: 0)
e.stub(rt(PALETTE), palette)
e.stub(rt(FMTNAME), lambda em, *_: 0)

PORT.ja1f_common_remap.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_size_t, ctypes.c_int32,
                                   ctypes.c_char_p]
_t29 = ctypes.create_string_buffer(256)
assert PORT.ja1f_common_remap(TERRAIN, len(TERRAIN), JAGGED, len(JAGGED), 29, _t29) == 0
PORT29 = (_t29.raw[0x27], _t29.raw[0xF1])
fails = 0


def one(case, sector, record, base28, lst, want_names):
    """Run the original on `record` (15,681 B) + `lst` + `base28`; compare with the port."""
    global fails
    e.uc.mem_write(addr28, base28)
    e.w8(addr29 + 0x27, 0)
    e.w8(addr29 + 0xF1, 0)
    cur["terrain"], cur["lst"], cur["pos"] = bytes(sector * RECORD) + record, lst, sector * RECORD
    names.clear()
    e.call(rt(LOAD), sector, 0)
    got_map = bytes(e.uc.mem_read(rt(TILEMAP), TILES * 8))
    got_ts = e.r8(rt(TILESET))
    got_tab = bytes(e.uc.mem_read(addr28, 256))
    out = ctypes.create_string_buffer(TILES * 8)
    ts = ctypes.c_ubyte()
    tab = ctypes.create_string_buffer(256)
    r = PORT.ja1f_terrain_load(record, base28, lst, len(lst), out, ctypes.byref(ts), tab)
    problems = []
    if r != 0:
        problems.append("ja1f_terrain_load returned %d" % r)
    if out.raw != got_map:
        d = next(i for i in range(TILES * 8) if out.raw[i] != got_map[i])
        problems.append("map byte %d (tile %d +%d): port %#04x original %#04x"
                        % (d, d // 8, d % 8, out.raw[d], got_map[d]))
    if ts.value != got_ts:
        problems.append("tileset: port %d original %d" % (ts.value, got_ts))
    if tab.raw != got_tab:
        d = next(i for i in range(256) if tab.raw[i] != got_tab[i])
        problems.append("table 28 byte %#04x: port %#04x original %#04x (base %#04x, lst %#04x)"
                        % (d, tab.raw[d], got_tab[d], base28[d], lst[d] if d < len(lst) else -1))
    if names != want_names:
        problems.append("files: %r, wanted %r" % (names, want_names))
    if (e.r8(addr29 + 0x27), e.r8(addr29 + 0xF1)) != PORT29:
        problems.append("table 29 bytes 0x27 / 0xF1: original %#04x %#04x, port %#04x %#04x"
                        % (e.r8(addr29 + 0x27), e.r8(addr29 + 0xF1), *PORT29))
    if problems:
        fails += 1
        if fails <= 8:
            print("FAIL case %d (sector %d): %s" % (case, sector, "; ".join(problems)))


case = 0
for sector in range(60):
    record = TERRAIN[sector * RECORD:(sector + 1) * RECORD]
    ts = record[TILES * 5]
    if ts > 9:
        print("FAIL sector %d has tileset byte %d" % (sector, ts))
        fails += 1
        continue
    one(case, sector, record, BASE28, LSTS[ts], ["DAT\\TERRAIN.DAT", "MISC\\tileset%d.lst" % ts])
    case += 1

rng = random.Random(0x2A9FC)
for _ in range(NCASES):
    sector = rng.randrange(60)
    record = bytearray(rng.randrange(256) for _ in range(TILES * 5)) + bytes([rng.randrange(10)])
    lst = bytearray(0xFF if rng.random() < 0.4 else rng.randrange(256) for _ in range(256))
    for i in SPECIAL:
        lst[i] = rng.choice([0xFF, 0xFF, 0x00, 0x2E, 0xA3, 0xBD, 0xFA, 0xF9, rng.randrange(256)])
    base28 = bytes(rng.randrange(256) for _ in range(256)) if rng.random() < 0.5 else BASE28
    ts = record[TILES * 5]
    one(case, sector, bytes(record), base28, bytes(lst), ["DAT\\TERRAIN.DAT", "MISC\\tileset%d.lst" % ts])
    case += 1

print("terrainload_diff: %d cases, %d mismatches" % (case, fails))
sys.exit(1 if fails else 0)
