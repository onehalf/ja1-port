"""Differential test: tools/sectormap.py's renderer vs the original tactical draw passes, replayed in Unicorn on a
machine snapshot (Emu dump mode).

usage: .venv/bin/python validate/diff/tiles_diff.py [DUMP [nviews=40]] [--sectors 1-60] [--png] (gfx/tiles_diff.png: original | ours, first mismatch)
DUMP = a snapshot taken in tactical (./t jactl dump NAME -> build/mem_NAME.bin; default build/mem_tac60.bin, from
save state tac60). ./t check runs `build/mem_tac60.bin 1 --sectors 1-60` (3 views per sector, ~45 s). For every sector (default: only the
snapshot's own map) g_tileMap is replaced by the TERRAIN.DAT record and its TILES%d.DAT bank, remap table 28 and
tileset byte are installed the way OpenTilesDat 0x36EA0 / LoadTerrainTileset 0x2A9FC do; then for random views the
original passes run (0x46F3C view tile, 0x169E8, 0x16C48, 0x17028, 0x17098; soldiers 0x165E8 stubbed out; ground
items and t7 marks cleared) on a zeroed back buffer, and the window is compared byte for byte with sectormap.Renderer.
Exits 1 on any mismatch.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
from emu import Emu                  # noqa: E402
from common import cd_read           # noqa: E402
import sectormap as sm               # noqa: E402

TILEMAP, TILESBUF, VIEWX, VIEWY, COLS, ROWS, TSBYTE = 0x229D50, 0xA8C48, None, None, 0x229CC8, 0x229CD0, 0x22A038
BACK, REMAP_PTRS = 0x22A4EF, 0xE8C48
VIEW_TILE, P1, P2, P4, P5, SOLDIER = 0x46F3C, 0x169E8, 0x16C48, 0x17028, 0x17098, 0x165E8

args = [a for a in sys.argv[1:] if not a.startswith("--")]
dump = args[0] if args else os.path.join(HERE, "..", "..", "build", "mem_tac60.bin")
nviews = int(args[1]) if len(args) > 1 else 40
e = Emu(dump=dump)
from common import load_labels       # noqa: E402
lab = {n: a for a, (n, _) in load_labels().items()}
TILEMAP, VIEWX, VIEWY = lab["g_tileMap"], lab["g_viewX"], lab["g_viewY"]
e.stub(e.rt(SOLDIER), lambda e, a, d, b, c: 0)
cols, rows = e.r32(e.rt(COLS)), e.r32(e.rt(ROWS))
rng = random.Random(1)


def install(sector):
    """TERRAIN.DAT record + its tileset into the emulated game (mirrors the loaders); -> tiles list for sectormap"""
    tiles, ts = sm.terrain(sector)
    for i, t in enumerate(tiles):
        t = t[:5] + b"\x40\xff\x00"                      # occupant none, no ground item, t7 0 (as the loader sets)
        tiles[i] = t
        e.uc.mem_write(e.rt(TILEMAP) + 8 * i, t)
    data = cd_read(f"DAT/TILES{ts}.DAT")
    n = struct.unpack_from("<I", data)[0]
    buf = bytearray(data[4:])                           # loader 0x52E00: drop the count, offsets -> pointers
    base = e.rt(TILESBUF)
    for k in range(n):
        struct.pack_into("<I", buf, 4 * k, base + 4 * n + struct.unpack_from("<I", buf, 4 * k)[0])
    e.uc.mem_write(base, bytes(buf))
    e.uc.ctl_remove_cache(base, base + 0x40000)        # sprites are code: drop Unicorn's stale translations
    e.w8(e.rt(TSBYTE), ts)
    tb = sm.Tables(ts)
    t28 = e.r32(e.rt(REMAP_PTRS) + 4 * 28)              # runtime pointer, flat in dump mode
    e.uc.mem_write(t28, bytes(tb.remap[28]))
    return tiles, ts


def original(c0, r0, roofs=True):
    e.w8(e.rt(lab["g_roofsOn"]), int(roofs))
    e.w32(e.rt(VIEWX), 16 * c0); e.w32(e.rt(VIEWY), 14 * r0)
    e.uc.mem_write(e.rt(BACK), bytes(sm.BW * sm.BH))
    t = e.call(e.rt(VIEW_TILE), 16 * c0, 14 * r0)
    e.call(e.rt(P1), t); e.call(e.rt(P2), t); e.call(e.rt(P4), c0, r0); e.call(e.rt(P5), t)
    b = bytes(e.uc.mem_read(e.rt(BACK), sm.BW * sm.BH))
    return b"".join(b[(28 + y) * sm.BW + 32:(28 + y) * sm.BW + 32 + 16 * cols] for y in range(14 * rows)), t


def main():
    sectors = [None]
    if "--sectors" in sys.argv:
        a, b = map(int, sys.argv[sys.argv.index("--sectors") + 1].split("-"))
        sectors = list(range(a, b + 1))
    fails = total = 0
    for sec in sectors:
        if sec is None:
            raw = bytes(e.uc.mem_read(e.rt(TILEMAP), 8 * sm.NT))
            tiles = [raw[8 * i:8 * i + 5] + b"\x40\xff\x00" for i in range(sm.NT)]
            for i, t in enumerate(tiles):
                e.uc.mem_write(e.rt(TILEMAP) + 8 * i, t)
            ts = e.r8(e.rt(TSBYTE))
        else:
            tiles, ts = install(sec)
        R = sm.Renderer(tiles, ts)
        views = [(0, 0), (56 - cols, 56 - rows)] + [(rng.randrange(57 - cols), rng.randrange(57 - rows))
                                                    for _ in range(nviews)]
        bad = 0
        for k, (c0, r0) in enumerate(views):
            R.roofs = k % 2 == 0                        # every second view with g_roofsOn = 0 (debug key Alt+Q)
            want, t = original(c0, r0, R.roofs)
            assert t == r0 * 56 + c0, (t, c0, r0)
            R.buf = bytearray(sm.BW * sm.BH); R.draw(c0, r0, cols, rows)
            got = R.window(cols, rows)
            total += 1
            if got != want:
                bad += 1
                diff = [i for i, (x, y) in enumerate(zip(got, want)) if x != y]
                if fails + bad <= 8:
                    i = diff[0]; x, y = i % (16 * cols), i // (16 * cols)
                    print(f"MISMATCH sector {sec} view ({c0},{r0}): {len(diff)} px, first at window ({x},{y}) "
                          f"= tile ({c0 + x // 16},{r0 + y // 14}) original {want[i]:#04x} ours {got[i]:#04x}")
                if "--png" in sys.argv and fails + bad == 1:
                    from pcx import png
                    w, h = 16 * cols, 14 * rows
                    sheet = bytearray(2 * (w + 4) * h)
                    for yy in range(h):
                        for k, src in enumerate((want, got)):
                            o = yy * 2 * (w + 4) + k * (w + 4); sheet[o:o + w] = src[yy * w:(yy + 1) * w]
                    png(os.path.join(HERE, "..", "..", "gfx", "tiles_diff.png"), 2 * (w + 4), h, bytes(sheet),
                        sm.palette(ts))
        fails += bad
        print(f"sector {sec if sec else 'dump'} tileset {ts}: {len(views) - bad}/{len(views)} views identical")
    print(f"{total - fails}/{total} views identical")
    sys.exit(1 if fails else 0)


main()
