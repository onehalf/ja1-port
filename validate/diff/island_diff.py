"""Differential test: the island map picture (cells + markers) of the original vs the clean-room front-end.

usage: .venv/bin/python validate/diff/island_diff.py [ncases=200] [DUMP] [--port] [--lib SO] [--png]
Per case: a fuzzed sector table (60 x 9 bytes, sectorState) and marks (60 signed bytes, 0x229D8C; mostly 0). The
original runs in Unicorn on tac60: the screen *0x95678 cleared, the 640-wide map buffer *0x93E5C filled with
MAPDARK.PCC | MAP.PCC (MAIN.DAT, tools/pcx.py), then per sector 0x41640 (cell) and 0x419BC (markers, element
draw 0x46920 = BlitKey from g_panelSheet). The port (port/ja1front ja1f_island_map, strategic.md "The island map
picture") draws from the same table; the 320 x 200 screen and the 6-bit MAP.PCC palette are compared.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from datarc import read, member      # noqa: E402
from pcx import decode, png          # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
import pfix                       # noqa: E402  #71 --pfix IDS (P04): the map column cap on both sides
PFIX = pfix.parse(argv)
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_island_map.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 4
    PORT.ja1f_island_map.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

SECTORS, MARKS, SCREEN_P, MAPBUF_P = 0x22979C, 0x229D8C, 0x95678, 0x93E5C
CELL, MARKERS = 0x41640, 0x419BC

if not os.path.exists(dump):
    print(f"island_diff: {dump} missing")
    sys.exit(1)
main = read("rom/DAT/MAIN.DAT")
dw, dh, dark, _ = decode(member(main, "MAPDARK.PCC"))
mw, mh, bright, mpal = decode(member(main, "MAP.PCC"))
assert (dw, mw) == (320, 320) and dh == mh <= 200
mapbuf = b"".join(dark[y * 320:(y + 1) * 320] + bright[y * 320:(y + 1) * 320] for y in range(dh)) + bytes(640 * (200 - dh))
pal6 = bytes(v >> 2 for v in mpal)

e = Emu(dump=dump, fast=True)
screen, mapp = e.r32(e.rt(SCREEN_P)), e.r32(e.rt(MAPBUF_P))
e.uc.mem_write(mapp, mapbuf)
snap = bytes(e.uc.mem_read(0, 16 << 20))
orig_tbl = bytes(e.uc.mem_read(e.rt(SECTORS), 540))


def count(r):
    return r.choice([0, 0, 1, 2, 3, 5, 8, 10, r.randrange(16), r.randrange(256)])


def fuzz(r, i):
    """(sectors 540 bytes, marks 60 bytes); case 0 = the dump's own table"""
    if i == 0:
        return orig_tbl, bytes(60)
    t = bytearray()
    for s in range(60):
        rec = [r.randrange(256) if r.random() < .3 else r.randrange(2)] + [count(r) for _ in range(8)]
        if r.random() < .5:                   # game-like: workable / unhealthy within trees
            rec[7] = r.randrange(rec[6] + 1)
            rec[8] = r.randrange(rec[6] + 1)
        t += bytes(rec)
    marks = bytes(r.choice([0] * 8 + [1, 2, 0xFF, 0xFE, r.randrange(256)]) if r.random() < .3 else 0
                  for _ in range(60))
    return bytes(t), marks


def original(tbl, marks):
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(SECTORS), tbl)
    e.uc.mem_write(e.rt(MARKS), marks)
    e.uc.mem_write(screen, bytes(64000))
    for s in range(60):
        e.call(e.rt(CELL), e.rt(SECTORS), s)
        e.call(e.rt(MARKERS), e.rt(SECTORS), s)
    return bytes(e.uc.mem_read(screen, 64000))


def port(tbl, marks):
    scr, pal = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(768)
    st, mk = ctypes.create_string_buffer(tbl, 540), ctypes.create_string_buffer(marks, 60)
    rc = PORT.ja1f_island_map(ISO, EXE, ctypes.addressof(st), ctypes.addressof(mk), ctypes.addressof(scr),
                              ctypes.addressof(pal))
    if rc:
        raise RuntimeError(f"ja1f_island_map returned {rc}")
    return scr.raw, pal.raw


r = random.Random(1)
fails = 0
for i in range(ncases):
    tbl, marks = fuzz(r, i)
    a = original(tbl, marks)
    if i == 0 and not any(a):
        print("island_diff: the original drew nothing")
        sys.exit(1)
    if not PORT:
        continue
    b, pal = port(tbl, marks)
    bad = []
    if a != b:
        n = next(j for j in range(64000) if a[j] != b[j])
        bad.append(f"screen @({n % 320},{n // 320}) {a[n]:#x} vs {b[n]:#x} ({sum(x != y for x, y in zip(a, b))} bytes)")
    if pal != pal6:
        bad.append("palette")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: " + "; ".join(bad))
        if fails == 1 and "--png" in argv:
            pix = b"".join(a[y * 320:(y + 1) * 320] + b[y * 320:(y + 1) * 320] for y in range(200))
            png(os.path.join(ROOT, "gfx", "island_diff.png"), 640, 200, pix, mpal)
# the mouse: sector under the mouse 0x41410 (its number drawing 0x35F64 stubbed) for every point of the map part,
# and the frame 0x4147C (sound stubbed) of every sector in the three colours, on the case 0 picture
e.uc.mem_write(0, snap)
if PFIX is not None:

    class _FrontFixes:              # the port side is libja1front: its own copy of the fix masks
        @staticmethod
        def port_fixes(m):
            PORT.ja1_port_fixes_set.argtypes = [ctypes.c_uint64]
            PORT.ja1_port_fixes_set(m)
    pfix.apply(e, PFIX, _FrontFixes)
e.stub(e.rt(0x35F64), lambda *_: 0)
e.stub(e.rt(0x5CB34), lambda *_: 0)
mouse_bad = frame_bad = 0
if PORT:
    PORT.ja1f_map_sector.argtypes = [ctypes.c_int32, ctypes.c_int32]
    PORT.ja1f_map_sector.restype = ctypes.c_int32
    PORT.ja1f_map_frame.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_map_frame.restype = ctypes.c_int32
    for y in range(141):
        for x in range(320):
            a = e.call(e.rt(0x41410), x, y) & 0xFF
            a = a - 256 if a > 127 else a
            b = PORT.ja1f_map_sector(x, y)
            if a != b:
                mouse_bad += 1
                if mouse_bad <= 5:
                    print(f"MISMATCH mouse ({x},{y}): {a} vs {b}")
    if PORT.ja1f_map_sector(100, 141) != -1:
        mouse_bad += 1
        print("MISMATCH mouse (100,141): not -1")
    pic = original(*fuzz(random.Random(2), 0))
    for sec in range(60):
        for col in (0x6E, 0x6B, 0xFF):
            e.uc.mem_write(screen, pic)
            e.call(e.rt(0x4147C), sec, col)
            a = bytes(e.uc.mem_read(screen, 64000))
            scr = ctypes.create_string_buffer(pic, 64000)
            rc = PORT.ja1f_map_frame(ISO, EXE, sec, col, ctypes.addressof(scr))
            if rc or a != scr.raw:
                frame_bad += 1
                if frame_bad <= 5:
                    n = next((j for j in range(64000) if a[j] != scr.raw[j]), -1)
                    print(f"MISMATCH frame sector {sec} colour {col:#x}: rc {rc}, first @({n % 320},{n // 320})")
    print(f"island_diff mouse: {141 * 320 + 1} points, {mouse_bad} mismatches; frames: 180, {frame_bad} mismatches")
    fails += mouse_bad + frame_bad
print(f"island_diff{' (port)' if PORT else ''}: {ncases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
