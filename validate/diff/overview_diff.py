"""Differential test (in ./t check): the overview picture of view mode 0 (panel.md "The overview picture").

usage: .venv/bin/python validate/diff/overview_diff.py [ncases=12] [DUMP] [--lib SO]
The original OverviewRedraw 0x23B4C runs in Unicorn on a tactical snapshot (default build/mem_tac60.bin) from its
start to 0x241A1 (after the clock, step 3, and the overview cursor, step 4; the texts and blits that follow are
not part of the test). The cursor's values (picture value, given point, mouse, window, cost, combat, over,
tactical_on, overlay_up) and the clock's (clock switch 0x92DE5, blink byte 0x284F06, clock_hour / clock_minute) are
fuzzed too. Per case: a sector's TERRAIN.DAT record and tile bank are installed (as tiles_diff.py), tile bytes 4, 6, 7
and some whole tiles are fuzzed, the ground items' byte 6, the soldiers' fields the marks read, sees_slot, the
filter, the roofs flag, "show all" and the effect records (kinds 1..4; byte 0x10 = 1, StartTileAnim stubbed: the hit
step is game logic) too; the back buffer starts with random bytes. Then port/ja1front ja1f_overview draws the same
into a copy of the buffer, and the 384 x 253 buffer rows are compared byte for byte. Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from common import cd_read, load_labels  # noqa: E402
import sectormap as sm               # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 12
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

lab = {n: a for a, (n, _) in load_labels().items()}
TILEMAP, TILESBUF, TSBYTE, BACK = lab["g_tileMap"], 0xA8C48, 0x22A038, 0x22A4EF
ITEMS, SOLDIERS, FX = 0xEB160, 0xFA958, 0xEAFF0
SEES, FILTER, ROOFS, SHOWALL, MODE = 0x92DC6, 0x92DC1, 0x92DDB, 0x93458, 0x92DDF
START, STOP, START_ANIM = 0x23B4C, 0x241A1, 0x15720
# the overview cursor (step 4): mouse, window rectangle, cost, combat, over, tactical_on, overlay_up, clock switch
MOUSE_X, MOUSE_Y, RECT, COST, COMBAT, OVER, TAC_ON, OVL_UP, CLOCK = (0x229D00, 0x229D04, (0x229CB8, 0x229CC4, 0x229CD8,
    0x229CDC), 0x229DEC, 0x92DCB, 0x92E5C, 0x92DCC, 0x92DCE, 0x92DE5)
BLINK, HOUR, MINUTE = 0x284F06, 0x22A0F6, 0x22A0F5
T0PTR = 0xE8C48
ROWS = 253
NB = 384 * ROWS

if not os.path.exists(dump):
    print(f"SKIP: {dump} missing")
    sys.exit(0)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_overview.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] + [ctypes.c_char_p] * 4 + [ctypes.c_int32] * 2 \
    + [ctypes.c_void_p] * 2 + [ctypes.c_char_p]
PORT.ja1f_overview.restype = ctypes.c_int32

e = Emu(dump=dump)
from unicorn import UC_HOOK_CODE     # noqa: E402
e.uc.hook_add(UC_HOOK_CODE, lambda uc, *_: uc.emu_stop(), None, e.rt(STOP), e.rt(STOP))
e.stub(e.rt(START_ANIM), lambda e, a, d, b, c: 0)
e.stub_sprintf()
rng = random.Random(27)


def install(sector):
    """TERRAIN.DAT record + its tile bank into the emulated game (as tiles_diff.py) -> tiles, tileset"""
    tiles, ts = sm.terrain(sector)
    data = cd_read(f"DAT/TILES{ts}.DAT")
    n = struct.unpack_from("<I", data)[0]
    buf = bytearray(data[4:])
    base = e.rt(TILESBUF)
    for k in range(n):
        struct.pack_into("<I", buf, 4 * k, base + 4 * n + struct.unpack_from("<I", buf, 4 * k)[0])
    e.uc.mem_write(base, bytes(buf))
    e.uc.ctl_remove_cache(base, base + 0x40000)
    e.w8(e.rt(TSBYTE), ts)
    return tiles, ts


def fuzz_tiles(tiles):
    out = bytearray()
    t0max = 157
    for t in tiles:
        b = bytearray(t[:5]) + bytearray(3)
        if rng.random() < 0.04:                        # a whole random tile (composites, roofs, 0x73)
            b[0] = rng.randrange(t0max)
            b[1] = rng.choice([0, rng.randrange(256), rng.randrange(0x27, 0x100)])
            b[2] = rng.choice([0, rng.randrange(1, 155)])
            b[3] = rng.choice([0, 0x73, rng.randrange(1, 0x50), rng.randrange(0x50, 155)])
        b[4] = (b[4] & 0x7F) | (0x80 if rng.random() < 0.7 else 0)
        b[5] = 0x40
        b[6] = rng.randrange(200) if rng.random() < 0.05 else rng.choice([0xFF, 0xFF, rng.randrange(200, 256)])
        b[7] = (rng.randrange(1, 16) << 4 if rng.random() < 0.03 else 0) | rng.randrange(16)
        out += b
    return bytes(out)


def fuzz_soldiers(sol):
    s = bytearray(sol)
    for k in range(48):
        r = k * 512
        for off, p in ((0x182, 0.8), (0x183, 0.85), (0x138, 0.85)):
            s[r + off] = rng.choice([1, rng.randrange(1, 256)]) if rng.random() < p else 0
        s[r + 0x9C] = rng.choice([0, 1, 2, 0x80, 0xFF, rng.randrange(256)])
        struct.pack_into("<h", s, r + 0x116, rng.choice([rng.randrange(0, 896), rng.randrange(-40, 0)]))
        struct.pack_into("<h", s, r + 0x118, rng.choice([rng.randrange(0, 784), 47 * rng.randrange(17),
                                                         rng.randrange(-200, 0)]))
        s[r + 0x1BD] = rng.choice([0x6A, 0x6B, 0x6C, 0x6D, 0x6E])
        for v in range(48):
            s[r + 0x112 + v] = rng.choice([0, 1, 1, 2])
    return bytes(s)


def fuzz_fx():
    f = bytearray(16 * 18)
    for k in range(1, 16):
        if rng.random() < 0.5:
            continue
        r = k * 18
        kind = rng.choice([1, 2, 3, 4])
        f[r] = rng.randrange(1, 22 if kind in (2, 3) else 11)
        f[r + 0xB] = kind
        struct.pack_into("<h", f, r + 5, rng.randrange(-40, 900))
        struct.pack_into("<h", f, r + 7, rng.randrange(-40, 790))
        f[r + 0x10] = 1
    return bytes(f)


def fuzz_cursor():
    """-> OvCursor values p px py mouse_x mouse_y left top right bottom cost over combat tactical_on overlay_up,
    then the clock switch, blink byte, hour, minute (the port gets clock = switch and blink)"""
    p = rng.choice([0xAF, 0x4F, rng.randrange(256)])
    px, py = (0, 0) if rng.random() < 0.7 else (rng.randrange(1, 330), rng.randrange(-10, 210))
    # Mode 0's window is 14 x 12 at (48, 14); a left edge below 21 puts the number at x < 0 (spec gap: not described).
    left, top, w, h = rng.choice([(48, 14, 224, 168), (21, 14, 280, 168), (32, 0, 288, 196), (48, 14, 224, 182)])
    mx, my = rng.randrange(0, 320), rng.randrange(0, 200)
    cost = rng.choice([0, 0, rng.randrange(1, 100), rng.randrange(100, 300)])
    return [p, px, py, mx, my, left, top, left + w - 1, top + h - 1, cost, rng.choice([1, 0xFF]),
            rng.randrange(2), rng.choice([0, 1, 1]), rng.choice([0, 0, 1]),
            rng.choice([0, 1, 1, rng.randrange(256)]), rng.choice([0, 1, 1, rng.randrange(256)]),
            rng.choice([7, 12, 1, rng.randrange(-128, 128)]), rng.choice([0, 5, 59, rng.randrange(-128, 128)])]


def main():
    bad = 0
    sectors = [rng.randrange(1, 61) for _ in range(ncases)]
    for case, sec in enumerate(sectors):
        tiles, ts = install(sec)
        tm = fuzz_tiles(tiles)
        items = bytes(rng.choice([0, 1, 5, 0x80, 0xFF]) if i % 12 == 6 else rng.randrange(256) for i in range(2400))
        sol = fuzz_soldiers(bytes(e.uc.mem_read(e.rt(SOLDIERS), 48 * 512)))
        fx = fuzz_fx()
        if os.environ.get("OV_ONLY") == "soldiers":
            fx = bytes(len(fx))
        if os.environ.get("OV_ONLY") == "tiles":     # debugging aid: no marks of soldiers, no effect records
            sol = bytes(0 if (i % 512) == 0x182 else v for i, v in enumerate(sol)); fx = bytes(len(fx))
        sees = rng.choice([rng.randrange(8), 0x40, rng.randrange(8, 48)])
        flt, roofs, show = rng.randrange(2), rng.choice([1, 1, 0]), rng.randrange(2)
        bb = bytes(rng.randrange(256) for _ in range(NB))
        for a, v in ((TILEMAP, tm), (ITEMS, items), (SOLDIERS, sol), (FX, fx), (BACK, bb)):
            e.uc.mem_write(e.rt(a), v)
        for a, v in ((SEES, sees), (FILTER, flt), (ROOFS, roofs), (SHOWALL, show), (MODE, 0)):
            e.w8(e.rt(a), v)
        cur = fuzz_cursor()
        p, px, py, mx, my, l, t, r, b, cost, over, combat, tac, ovl, clk, blink, hour, minute = cur
        for a, v in zip((MOUSE_X, MOUSE_Y) + RECT + (COST, OVER), (mx, my, l, t, r, b, cost, over)):
            e.w32(e.rt(a), v)
        for a, v in ((COMBAT, combat), (TAC_ON, tac), (OVL_UP, ovl), (CLOCK, clk), (BLINK, blink), (HOUR, hour & 0xFF),
                     (MINUTE, minute & 0xFF)):
            e.w8(e.rt(a), v)
        t0 = e.r32(e.rt(T0PTR))
        table0 = bytes(e.uc.mem_read(t0, 256))
        e.call(e.rt(START), 1, p, px & 0xFFFFFFFF, py & 0xFFFFFFFF, max_steps=50_000_000)
        assert e.uc.reg_read(__import__("unicorn").x86_const.UC_X86_REG_EIP) == e.rt(STOP), "did not reach the stop"
        want = bytes(e.uc.mem_read(e.rt(BACK), NB))
        got = ctypes.create_string_buffer(bb + bytes(384 * 854 - NB), 384 * 854)
        oc = (ctypes.c_int32 * 17)(*cur[:14], int(clk != 0 and blink != 0), hour, minute)
        rc = PORT.ja1f_overview(ISO, EXE, ts, tm, items, sol, fx, sees, flt | roofs << 1 | show << 2, got,
                                ctypes.addressof(oc), table0)
        assert rc == 0, f"ja1f_overview returned {rc}"
        got = got.raw[:NB]
        if got != want:
            bad += 1
            diff = [i for i in range(NB) if got[i] != want[i]]
            i = diff[0]
            inwin = [j for j in diff if 32 <= j % 384 < 256 and 28 <= j // 384 < 196]
            if inwin:
                j = inwin[0]
                print(f"  in the picture: {len(inwin)} px, first at ({j % 384},{j // 384}) original {want[j]:#04x} port {got[j]:#04x}")
            print(f"MISMATCH case {case} sector {sec} (filter {flt} roofs {roofs} show {show} sees {sees} cursor {cur}): "
                  f"{len(diff)} px, first at ({i % 384},{i // 384}) original {want[i]:#04x} port {got[i]:#04x}")
    print(f"overview: {ncases - bad}/{ncases} cases identical")
    sys.exit(1 if bad else 0)


main()
