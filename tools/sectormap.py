"""Render the tactical map (56x56 tiles of 16x14 px) the way the original's redraw does, from the data files.

usage: ./t sectormap SECTOR|live [--view COL ROW COLS ROWS] [--out PNG] [--cmp]
  SECTOR: 1..60 as the UI shows it (TERRAIN.DAT record SECTOR-1); `live` = g_tileMap + view of the running game.
  default: whole map (view 0 0 56 56) -> gfx/sector<NN>.png.  --cmp (live only): diff against the finished frame
  in the back buffer 0x22A4EF (the window goes from there straight to VGA; a single read can land mid-redraw, so
  the most frequent of several frozen reads is used), writes gfx/cmp_live.png (live | ours | same pixels) and prints the match.

Data: TERRAIN.DAT = 60 records of 0x3D41 bytes (LoadTerrainTileset 0x2A9FC): 3136 tiles x 5 bytes (t0..t4 of
g_tileMap) from the last tile down to tile 0, then the tileset number (TILES%d.DAT, MISC\\TILE%d.PAL, MISC\\TILESET%d.LST).
JAGGED.DAT (LoadJaggedDat 0x371BC) from 0x3470: 34 remap tables x 256 (pointers 0xE8C48[]), 157 bytes t0 class
(0x284E58), 256 t1 class (0x108158), 155 t2 class (0x109AD8). Remap table 28 = TILESET%d.LST (0xFF = keep).
Sprite numbers (offsets into the TILES table 0xA8C48): t0 -> t0, t1 -> 157+t1, t2/t3 -> 413+v, shadows -> 1293+k.

Redraw = TacticalRedraw 0x176C8; visible window = cols x rows tiles from the view's top-left tile, tile (c, r) of the
window at back-buffer (32+16c, 28+14r), pitch 0x180. Static passes modelled here (fog flag 0x92DDB = 1, shadows flag
0x1163BC = 1, as in play):
  1  0x169E8 window:       t0 if (cls93B4C[t1] && cls93C4C[t3]) || t3 == 0x73;  t1 if t1 && t1cls < 6 && t3 != 0x73;
                           t2 if t2 && t2cls < 12.  t7 path marks. (ground items, t3 1..9 animations: not modelled)
  2  0x16C48 window+1 up/left (only if the view is not at 0): k = t4 & 0x7F < 99: k < 0x40 -> shadow k at (+2,-2) and
                           k-1, k-2, k-3 at (-16,0), (0,-14), (-16,-14) of that; k >= 0x40 -> shadow k at (0,0).
                           Shadow k = RemapBlit 0x52425 of raw 16x14 bitmap 1293+tbl95E64[k].idx at (+dx,+dy):
                           src 0 = skip, src 0x17 = darken (dst = remap28[dst]), else dst = remap0[src].
  4  0x17028/0x16E3C window+1 on all sides (map-clipped): soldiers (not modelled); t1cls >= 7 and inside the window:
                           t1 if cls93C4C[t3] (t1 > 0x82 at (-2,-1)); t1cls == 6 (any): 2x2 composite 0x164BC = t1 at
                           (+2,-2), then 413+tbl93D60[t1-0x27] .b1 at (+2,-16), .b0 at (-14,-16), .b2 at (-14,-2).
  5  0x17098 window:       413+t2 if t2 > 0x3F; 413+t3 if t3 > 0x50 and t3 != 0x73 (roofs: t3 changes when a merc
                           enters a building).
"""
import os, struct, sys
from common import ROOT, cd_read
from csprite import bank, render_all, palette
from pcx import png

REC, NT, W = 0x3D41, 56 * 56, 56
T1, T2, SH = 157, 413, 1293
BW, BH = 0x180, 28 + 14 * 57 + 28            # back buffer pitch 0x180; tall enough for a whole-map window


class Tables:
    def __init__(self, tileset):
        from le import load
        img = load()
        j = cd_read("DAT/JAGGED.DAT")
        o = 0x3470
        self.remap = [bytearray(j[o + 256 * k:o + 256 * (k + 1)]) for k in range(34)]; o += 0x2200
        self.t0cls = j[o:o + 0x9D]; o += 0x9D
        self.t1cls = j[o:o + 0x100]; o += 0x100
        self.t2cls = j[o:o + 0x9B] + bytes(0x100 - 0x9B)
        self.remap[29][0x1B] = 0xC6
        lst = cd_read(f"MISC/TILESET{tileset}.LST")
        for i, v in enumerate(lst[:256]):            # LoadTerrainTileset's overrides
            if v == 0xFF:
                continue
            if i in (0xA2, 0xA3): v = 0xA3
            if tileset == 7 and i == 0xB9: v = 0xBD
            if tileset == 9:
                if i == 0xF9: v = 0xFA
                elif i == 0xB9: v = 0xBD
            if i == 0x2E: v = 0x2F
            if i in (0xF5, 0xF7, 0xF9): v = 0xF9
            self.remap[28][i] = v
        self.remap[29][0x27] = 0x2B; self.remap[29][0xF1] = 0xF2
        self.c93b4c = img.read(0x93B4C, 256)
        self.c93c4c = img.read(0x93C4C, 256)
        self.shadow = img.read(0x95E64, 3 * 128)    # read as dword>>24 at 0x95E61+1.. = bytes from 0x95E64
        self.comp = img.read(0x93D60, 3 * 256)


def terrain(sector):
    d = cd_read("DAT/TERRAIN.DAT")[(sector - 1) * REC:sector * REC]
    tiles = [None] * NT
    for k in range(NT):
        tiles[NT - 1 - k] = d[k * 5:k * 5 + 5] + bytes([0x40, 0xFF, 0])
    return tiles, d[NT * 5]


def s8(v):
    return v - 256 if v > 127 else v


class Renderer:
    def __init__(self, tiles, tileset):
        self.tiles, self.ts = tiles, tileset
        self.tb = Tables(tileset)
        self.spr = render_all(bank(cd_read(f"DAT/TILES{tileset}.DAT")))
        self.raw = bank(cd_read(f"DAT/TILES{tileset}.DAT"))
        self.buf = bytearray(BW * BH)
        self.sol = None        # Soldiers (below) to draw soldiers in pass 4; None = the static scene only
        self.roofs = True      # g_roofsOn 0x92DDB: False = every t0 and tall t1 drawn, no roofs (debug key Alt+Q)

    def soldier(self, slot, c0, r0, cols, rows):
        """DrawSoldier 0x165E8 without its text (names, floating numbers): SH.DAT raw bitmap (facing-1)*182 + frame,
        20x20 (frame < 0x43, anchor 9, 12) or 32x28 (0x43..0xB5, anchor 15, 16), RemapBlit keys 0x18 skip /
        0x1C darken. Writes the anchor to +0x11A / +0x11C like the original."""
        S = self.sol
        rec = S.rec(slot)
        u16 = lambda o: rec[o] | rec[o + 1] << 8
        s16 = lambda o: u16(o) - 0x10000 if u16(o) & 0x8000 else u16(o)
        if rec[0x188] == 1:
            return
        fi = (s8(rec[0x1A1]) - 1) & 0xFFFF
        fr = S.script_word(slot)
        if s8(rec[0x9C]) <= 0 and not S.show_all:
            return
        if rec[0x17A] >= 0x28 and S.viewer < 0x40 and S.seen(S.viewer, rec[0x17A]) != 1:
            return
        if u16(0x136):
            fr = u16(0x136)
        if fr >= 0x43:
            if fr >= 0xB6:
                return
            w, h, ox, oy = 32, 28, 15, 16
        else:
            w, h, ox, oy = 20, 20, 9, 12
        S.set_anchor(slot, ox, oy)
        x = s16(0x116) - ox - S.view_x + 32
        y = s16(0x118) - oy - S.view_y + 28
        code = u16(0x1E8 + 2 * rec[0x1F3])
        if (1 <= code <= 3 or code in (0xD, 0x2B)) and fr > 0x30:
            x += 1; y += 4
        if 32 - w > x or 16 * cols + 32 < x or y < 28 - h or y > 14 * rows + 28:
            return
        if rec[0x1AD]:
            sel = 0x1B
        elif rec[0x17A] < 8 and rec[0x17A] == S.selected:
            sel = (rec[0x17B] + 8) & 0xFF
        else:
            sel = rec[0x17B]
        if fr in (0x48, 0x50):
            fr = (fr + u16(0x10A)) & 0xFFFF
        src = S.bitmap(fi * 182 + fr, w * h)
        a, d, b = S.remap(s8(sel)), S.remap(S.dark), self.buf
        for r in range(h):
            for c in range(w):
                v = src[r * w + c]
                o = (y + r) * BW + x + c
                if v == 0x18:
                    continue
                b[o] = d[b[o]] if v == 0x1C else a[v]

    def blit(self, i, x, y):
        b, s = self.buf, self.spr[i]
        for (dx, dy), v in s.items():
            o = (y + dy) * BW + x + dx
            if 0 <= o < len(b):
                b[o] = v

    def shadow(self, x, y, k):
        idx, dx, dy = self.tb.shadow[3 * k], s8(self.tb.shadow[3 * k + 1]), s8(self.tb.shadow[3 * k + 2])
        x += dx; y += dy
        src = self.raw[SH + idx]
        if y < 0:                                     # quirk of 0x163F8: rows skipped, height stays 14
            src = self.raw[SH + idx][-y * 16:] + self.raw[SH + idx + 1]; y = 0
        a, d, b = self.tb.remap[0], self.tb.remap[28], self.buf
        for r in range(14):
            for c in range(16):
                v = src[r * 16 + c]
                o = (y + r) * BW + x + c
                if v == 0 or o >= len(b):
                    continue
                b[o] = d[b[o]] if v == 0x17 else a[v]

    def draw(self, c0, r0, cols, rows):
        tb, T = self.tb, self.tiles
        def tile(c, r): return T[r * W + c] if 0 <= c < W and 0 <= r < W else None
        for r in range(rows):                         # pass 1
            for c in range(cols):
                t0, t1, t2, t3 = T[(r0 + r) * W + c0 + c][:4]
                x, y = 32 + 16 * c, 28 + 14 * r
                if (tb.c93b4c[t1] and tb.c93c4c[t3]) or t3 == 0x73 or not self.roofs:
                    self.blit(t0, x, y)
                if t1 and tb.t1cls[t1] < 6 and t3 != 0x73:
                    self.blit(T1 + t1, x, y)
                if t2 and tb.t2cls[t2] < 12:
                    self.blit(T2 + t2, x, y)
                m = T[(r0 + r) * W + c0 + c][7] if len(T[0]) > 7 else 0
                if m >> 4:                            # path marks (tile byte 7): DrawGroundPass 0x16B42
                    self.blit(644 + (m >> 4) if m >> 4 < 10 else 595, x, y)
                if m & 15:
                    self.blit(652 + (m & 15), x, y)
        for r in range(-1 if r0 else 0, rows + 1):    # pass 2 (shadows)
            for c in range(-1 if c0 else 0, cols):
                i = (r0 + r) * W + c0 + c
                if i >= NT:
                    break
                k = T[i][4] & 0x7F
                x, y = 32 + 16 * c, 28 + 14 * r
                if k < 99:
                    if k < 0x40:
                        self.shadow(x + 2, y - 2, k)
                        for n, (dx, dy) in enumerate(((-16, 0), (0, -14), (-16, -14))):
                            if x + 2 + dx >= 0 and y - 2 + dy >= 0:
                                self.shadow(x + 2 + dx, y - 2 + dy, (k - 1 - n) & 0xFF)
                    else:
                        self.shadow(x, y, k)
        for r in range(-1, rows + 1):                 # pass 4 (soldiers when self.sol is set, tall t1, composites)
            for c in range(-1, cols + 1):
                t = tile(c0 + c, r0 + r)
                if t is None:
                    continue
                t1, t3 = t[1], t[3]
                x, y = 32 + 16 * c, 28 + 14 * r
                inside = c >= 0 and r >= 0
                if self.sol is not None and (t[5] & 0x7F) < 0x30:
                    self.soldier(t[5] & 0x7F, c0, r0, cols, rows)
                    i = (r0 + r) * W + c0 + c - 55       # up-right neighbour's wall piece goes back over the soldier
                    if inside and 0 <= i and T[i][1] in (0x41, 0x60):
                        self.blit(T1 + T[i][1], x + 16, y - 14)
                if tb.t1cls[t1] >= 7 and inside:
                    if tb.c93c4c[t3] or not self.roofs:
                        self.blit(T1 + t1, x - 2 if t1 > 0x82 else x, y - 1 if t1 > 0x82 else y)
                elif tb.t1cls[t1] == 6:
                    e = 3 * (t1 - 0x27)
                    X, Y = x + 2, y - 2
                    self.blit(T1 + t1, X, Y)
                    if Y - 14 >= 0: self.blit(T2 + tb.comp[e + 1], X, Y - 14)
                    if X - 16 >= 0 and Y - 14 >= 0: self.blit(T2 + tb.comp[e], X - 16, Y - 14)
                    if X - 16 >= 0: self.blit(T2 + tb.comp[e + 2], X - 16, Y)
        for r in range(rows):                         # pass 5 (t2 > 0x3F, roofs)
            for c in range(cols):
                t2, t3 = T[(r0 + r) * W + c0 + c][2:4]
                x, y = 32 + 16 * c, 28 + 14 * r
                if t2 > 0x3F:
                    self.blit(T2 + t2, x, y)
                if t3 > 0x50 and t3 != 0x73 and self.roofs:
                    self.blit(T2 + t3, x, y)

    def window(self, cols, rows):
        return b"".join(bytes(self.buf[(28 + y) * BW + 32:(28 + y) * BW + 32 + 16 * cols]) for y in range(14 * rows))


def settled_back_buffer(g, n=12):
    """Most frequent back-buffer content over n frozen reads ~40 ms apart (= a frame not being redrawn)."""
    import collections, time
    seen = collections.Counter()
    for _ in range(n):
        with g.frozen():
            seen[g.mem(0x22A4EF, BW * BH)] += 1
        time.sleep(0.04)
    buf, k = seen.most_common(1)[0]
    print(f"back buffer: {len(seen)} distinct of {n} reads, using one seen {k}x")
    return buf


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__); return
    live = argv[0] == "live"
    if live:
        from jactl import Game
        g = Game()
        with g.frozen():
            tm = g.mem("g_tileMap", 8 * NT)
            vx, vy, cols, rows = g.v.g_viewX, g.v.g_viewY, g.num("<i", 0x229CC8), g.num("<i", 0x229CD0)
            ts = g.mem(0x22A038, 1)[0]
        back = settled_back_buffer(g) if "--cmp" in argv else None
        tiles = [tm[8 * i:8 * i + 8] for i in range(NT)]
        view = (vx // 16, vy // 14, cols, rows)
        name = "live"
    else:
        tiles, ts = terrain(int(argv[0]))
        view = (0, 0, W, W)
        name = f"sector{int(argv[0]):02d}"
    if "--view" in argv:
        i = argv.index("--view"); view = tuple(map(int, argv[i + 1:i + 5]))
    R = Renderer(tiles, ts)
    R.draw(*view)
    w, h = 16 * view[2], 14 * view[3]
    pix = R.window(view[2], view[3])
    pal = palette(ts)
    out = argv[argv.index("--out") + 1] if "--out" in argv else os.path.join(ROOT, "gfx", name + ".png")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    if live and "--cmp" in argv:
        ref = b"".join(bytes(back[(28 + y) * BW + 32:(28 + y) * BW + 32 + w]) for y in range(h))
        same = sum(a == b for a, b in zip(ref, pix))
        print(f"view {view}: {same}/{len(ref)} = {100 * same / len(ref):.2f}% pixels equal the back buffer")
        G = w + 4
        sheet = bytearray(3 * G * h)
        for y in range(h):
            for k, src in enumerate((ref, pix, bytes(a if a == b else 0 for a, b in zip(ref, pix)))):
                sheet[y * 3 * G + k * G:y * 3 * G + k * G + w] = src[y * w:(y + 1) * w]
        out = os.path.join(ROOT, "gfx", "cmp_live.png")
        png(out, 3 * G, h, bytes(sheet), pal)
    else:
        png(out, w, h, pix, pal)
    print(f"tileset {ts} view {view} -> {out}")


if __name__ == "__main__":
    main(sys.argv[1:])
