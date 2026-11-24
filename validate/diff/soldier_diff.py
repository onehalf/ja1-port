"""Differential test: tools/sectormap.py's soldier layer (Renderer.soldier) vs the original DrawSoldier 0x165E8, called by
the object pass 0x17028 / DrawTileObjects 0x16E3C, replayed in Unicorn on machine snapshots.

usage: .venv/bin/python validate/diff/soldier_diff.py [ncases=300] DUMP... [--port [--lib PATH]]
For every dump the snapshot's own map, soldiers and occupants stay; ground items and t7 marks are cleared (a third of the fuzzed cases puts random t7 path marks into the window) and the
text calls 0x358D0 (names, floating numbers) are stubbed out. Each case puts the view near one soldier and, after
the first plain case per soldier, fuzzes that soldier's drawing fields (facing, frame override +0x136 and script
word, +0x10A, colour +0x17B, +0x1AD, +0x9C, +0x188, position +0x116 / +0x118, animation code, the player's sight
table, the selected merc, the show-all flag). The original passes 0x169E8, 0x16C48, 0x17028, 0x17098 run on a
zeroed back buffer; the window and the anchor fields +0x11A / +0x11C must match. --port: also the clean-room
front-end (port/ja1front, ja1f_render_scene) on the same state must match the original. Exits 1 on any mismatch.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
from emu import Emu                  # noqa: E402
from common import cd_read, load_labels  # noqa: E402
from csprite import bank             # noqa: E402
import sectormap as sm               # noqa: E402
import livecast                      # noqa: E402
from csprite import palette          # noqa: E402

VIEW_TILE, P1, P2, P4, P5, TEXT = 0x46F3C, 0x169E8, 0x16C48, 0x17028, 0x17098, 0x358D0
BACK, REMAP_PTRS, SOLDIERS, COLS, ROWS, TSBYTE = 0x22A4EF, 0xE8C48, 0x100958, 0x229CC8, 0x229CD0, 0x22A038
VIEWER, SEEN, SELECTED, DARK, SHOW_ALL = 0x92DC6, 0xFAA6A, 0x284F11, 0x284F08, 0x93458
lab = {n: a for a, (n, _) in load_labels().items()}
TILEMAP, VIEWX, VIEWY = lab["g_tileMap"], lab["g_viewX"], lab["g_viewY"]
GROUND = lab["g_groundItems"]

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(HERE, "..", "..", "port", "ja1front", "target",
                                                                         "release", "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
PORT = None
if "--port" in argv:
    import ctypes

    class Scene(ctypes.Structure):
        _fields_ = [("tilemap", ctypes.c_void_p), ("soldiers", ctypes.c_void_p), ("script_words", ctypes.c_void_p),
                    ("seen", ctypes.c_void_p), ("show_all", ctypes.c_uint8), ("viewer", ctypes.c_uint8),
                    ("selected", ctypes.c_uint8), ("shade", ctypes.c_uint8), ("ground_items", ctypes.c_void_p)]
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_render_scene.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.POINTER(Scene)] + \
        [ctypes.c_int32] * 4 + [ctypes.c_void_p]
    PORT.ja1f_render_scene.restype = ctypes.c_int32
    ROOT = os.path.join(HERE, "..", "..")
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()


def port_render(e, S, ts, c0, r0, cols, rows):
    """-> (window, 48 soldier records after the call) from ja1f_render_scene on the state S.snapshot() took"""
    tm = ctypes.create_string_buffer(bytes(e.uc.mem_read(e.rt(TILEMAP), 8 * sm.NT)), 8 * sm.NT)
    recs = ctypes.create_string_buffer(b"".join(S.recs.get(k, bytes(0x200)) for k in range(48)), 48 * 0x200)
    words = (ctypes.c_uint16 * 48)(*[S.words.get(k, 0) for k in range(48)])
    seen = ctypes.create_string_buffer(S.seen_tbl, len(S.seen_tbl))
    gi = ctypes.create_string_buffer(bytes(e.uc.mem_read(e.rt(GROUND), 2400)), 2400)
    sc = Scene(ctypes.addressof(tm), ctypes.addressof(recs), ctypes.addressof(words), ctypes.addressof(seen),
               S.show_all, S.viewer, S.selected, S.dark, ctypes.addressof(gi))
    out = (ctypes.c_uint8 * (16 * cols * 14 * rows))()
    r = PORT.ja1f_render_scene(ISO, EXE, ts, ctypes.byref(sc), c0, r0, cols, rows, out)
    return (bytes(out) if r == 0 else b"return %d" % r), recs.raw
ncases = int(args[0]) if args and args[0].isdigit() else 300
dumps = [a for a in args if not a.isdigit()] or [os.path.join(HERE, "..", "..", "build", "mem_combat1.bin")]
sh = cd_read("DAT/SH.DAT")
SH_N = struct.unpack_from("<I", sh)[0]
SH_OFFS = struct.unpack_from(f"<{SH_N}I", sh, 4)


class Soldiers:
    """sectormap.Renderer.sol: soldier records and the globals DrawSoldier reads, taken from the emulator"""
    def __init__(self, e, tb):
        self.e, self.tb, self.recs, self.anchor = e, tb, {}, {}

    def ptr(self, slot):
        return self.e.r32(self.e.rt(SOLDIERS) + 4 * slot)

    def snapshot(self):
        e = self.e
        self.recs = {s: bytes(e.uc.mem_read(self.ptr(s), 0x200)) for s in range(0x30) if self.ptr(s)}
        self.words = {s: e.r16(struct.unpack_from("<I", r, 0x90)[0]) for s, r in self.recs.items()}
        self.view_x, self.view_y = e.r32(e.rt(VIEWX)), e.r32(e.rt(VIEWY))
        self.viewer, self.selected = e.r8(e.rt(VIEWER)), e.r8(e.rt(SELECTED))
        self.dark, self.show_all = e.r8(e.rt(DARK)), e.r8(e.rt(SHOW_ALL))
        self.seen_tbl = bytes(e.uc.mem_read(e.rt(SEEN), 0x40 * 0x200))
        self.anchor = {}

    def rec(self, slot): return self.recs[slot]
    def script_word(self, slot): return self.words[slot]
    def seen(self, viewer, slot): return self.seen_tbl[viewer * 0x200 + slot]
    def set_anchor(self, slot, ox, oy): self.anchor[slot] = (ox, oy)
    def remap(self, k):
        assert 0 <= k < 34, f"remap table {k}"
        return self.tb.remap[k]

    def bitmap(self, n, size):
        assert 0 <= n < SH_N, f"SH.DAT entry {n}"
        o = 4 + 4 * SH_N + SH_OFFS[n]
        return sh[o:o + size]


def run(dump, ncases, rng):
    e = Emu(dump=dump)
    e.stub(e.rt(TEXT), lambda e, a, d, b, c: 0)
    cols, rows = e.r32(e.rt(COLS)), e.r32(e.rt(ROWS))
    ts = e.r8(e.rt(TSBYTE))
    raw = bytearray(e.uc.mem_read(e.rt(TILEMAP), 8 * sm.NT))
    for i in range(sm.NT):
        raw[8 * i + 6], raw[8 * i + 7] = 0xFF, 0                 # no ground items, no t7 marks
    e.uc.mem_write(e.rt(TILEMAP), bytes(raw))
    tiles = [bytes(raw[8 * i:8 * i + 8]) for i in range(sm.NT)]
    tb = sm.Tables(ts)
    pal = palette(ts)
    for k in range(34):                                          # the file tables are what the port loads
        live = bytes(e.uc.mem_read(e.r32(e.rt(REMAP_PTRS) + 4 * k), 256))
        assert k == 28 or live == bytes(tb.remap[k]), f"remap table {k} differs from JAGGED.DAT"
    R = sm.Renderer(tiles, ts)
    S = Soldiers(e, tb)
    R.sol = S
    occ = [(i % sm.W, i // sm.W, raw[8 * i + 5] & 0x7F) for i in range(sm.NT) if (raw[8 * i + 5] & 0x7F) < 0x30]
    saved = {s: bytes(e.uc.mem_read(S.ptr(s), 0x200)) for _, _, s in occ}
    gsave = {a: e.r8(e.rt(a)) for a in (VIEWER, SELECTED, DARK, SHOW_ALL)}
    seen_save = bytes(e.uc.mem_read(e.rt(SEEN), 0x40 * 0x200))
    fails = 0
    for n in range(ncases):
        for s, r in saved.items():
            e.uc.mem_write(S.ptr(s), r)
        for a, v in gsave.items():
            e.w8(e.rt(a), v)
        e.uc.mem_write(e.rt(SEEN), seen_save)
        e.uc.mem_write(e.rt(TILEMAP), bytes(raw))
        mc, mr, slot = occ[n % len(occ)]
        dc, dr = rng.randrange(-1, cols + 1), rng.randrange(-1, rows + 1)
        edge = rng.random() < 0.2                               # the soldier in the extra column / row -1
        if edge:
            dc, dr = rng.choice([(-1, dr), (dc, -1)])
        c0 = min(max(mc - dc, 0), 56 - cols)
        r0 = min(max(mr - dr, 0), 56 - rows)
        what = "plain" if n < len(occ) else fuzz(e, S.ptr(slot), slot, rng, mr * sm.W + mc, edge)
        if n >= len(occ) and rng.random() < 0.3:              # path marks (tile byte 7) on tiles of the window
            for _ in range(24):
                i = (r0 + rng.randrange(rows)) * sm.W + c0 + rng.randrange(cols)
                e.w8(e.rt(TILEMAP) + 8 * i + 7, rng.choice([rng.randrange(256), rng.randrange(1, 9) * 17]))
            what += " marks"
        items = PORT is not None and n >= len(occ) and rng.random() < 0.4
        if items:                                               # ground items (tile byte 6) on window tiles
            gtab = bytearray(2400)
            for g in range(200):
                gtab[12 * g] = rng.randrange(136)
                gtab[12 * g + 6] = rng.choice([0, 1, 1, 1, 0xFF, rng.randrange(256)])
            e.uc.mem_write(e.rt(GROUND), bytes(gtab))
            tall = [i for i in range((r0 - 1) * sm.W, (r0 + rows + 1) * sm.W) if 0 <= i < sm.NT and raw[8 * i + 1] >= 0x75]
            for _ in range(30):
                i = rng.choice(tall) if tall and rng.random() < 0.3 else \
                    (r0 + rng.randrange(-1, rows + 1)) * sm.W + c0 + rng.randrange(-1, cols + 1)
                if 0 <= i < sm.NT:
                    e.w8(e.rt(TILEMAP) + 8 * i + 6, rng.choice([rng.randrange(200), rng.randrange(256)]))
                    if rng.random() < 0.2:                      # objects at the t1 < 0x75 edge
                        e.w8(e.rt(TILEMAP) + 8 * i + 1, rng.choice([0, 0x73, 0x74, 0x75, 0x76]))
            what += " items"
        tm = bytes(e.uc.mem_read(e.rt(TILEMAP), 8 * sm.NT))
        R.tiles = [tm[8 * i:8 * i + 8] for i in range(sm.NT)]
        e.w32(e.rt(VIEWX), 16 * c0); e.w32(e.rt(VIEWY), 14 * r0)
        S.snapshot()
        R.buf = bytearray(sm.BW * sm.BH)
        R.draw(c0, r0, cols, rows)
        want_anchor = {s: S.anchor[s] for s in S.anchor}
        e.uc.mem_write(e.rt(BACK), bytes(sm.BW * sm.BH))
        t = e.call(e.rt(VIEW_TILE), 16 * c0, 14 * r0)
        e.call(e.rt(P1), t); e.call(e.rt(P2), t); e.call(e.rt(P4), c0, r0); e.call(e.rt(P5), t)
        b = bytes(e.uc.mem_read(e.rt(BACK), sm.BW * sm.BH))
        orig = b"".join(b[(28 + y) * sm.BW + 32:(28 + y) * sm.BW + 32 + 16 * cols] for y in range(14 * rows))
        ours = R.window(cols, rows)
        got_anchor = {s: (e.r16(S.ptr(s) + 0x11A), e.r16(S.ptr(s) + 0x11C)) for s in want_anchor}
        if PORT is not None:
            pw, precs = port_render(e, S, ts, c0, r0, cols, rows)
            orecs = b"".join(bytes(e.uc.mem_read(S.ptr(k), 0x200)) if k in S.recs else bytes(0x200) for k in range(48))
            if items:                                           # the reference renderer draws no ground items
                ours = pw
            if pw != orig or precs != orecs:
                bad = sorted({i // 0x200 for i in range(len(orecs)) if precs[i] != orecs[i]})
                what += f" PORT ({sum(a != b for a, b in zip(pw, orig))} px; records differ in slots {bad})"
                ours = None
        shown = pw if PORT is not None else R.window(cols, rows)
        livecast.status("soldier_diff", n + 1, ncases, fails, f"{os.path.basename(dump)} case {n} slot {slot}")
        livecast.frame("soldier_diff", f"soldiers, {os.path.basename(dump)} case {n} ({what}): original | "
                   + ("port" if PORT is not None else "reference") + " | differences",
                   orig, shown, 16 * cols, 14 * rows, pal, force=shown != orig)
        if orig != ours or got_anchor != want_anchor:
            fails += 1
            if fails <= 8:
                diff = [i for i in range(len(orig)) if ours is not None and orig[i] != ours[i]]
                where = f", first at window ({diff[0] % (16 * cols)},{diff[0] // (16 * cols)})" if diff else ""
                print(f"MISMATCH {os.path.basename(dump)} case {n} slot {slot} view ({c0},{r0}) {what}: "
                      f"{len(diff)} px{where}; anchors original {got_anchor} ours {want_anchor}")
    return fails


def fuzz(e, p, slot, rng, tile, edge):
    """change 1..3 drawing inputs of soldier p (edge: also a visible soldier with a wall piece); -> description"""
    done = []
    if edge and tile >= 55:
        e.w8(e.rt(TILEMAP) + 8 * (tile - 55) + 1, rng.choice([0x41, 0x60])); e.w8(p + 0x9C, 1)
        done.append("edge-wall")
    for _ in range(rng.randint(1, 3)):
        k = rng.randrange(12)
        if k == 0:
            v = rng.choice([1, 2, 3, 4, 5, 6, 7, 8]); e.w8(p + 0x1A1, v)
        elif k == 1:
            v = rng.choice([0, 1, 0x30, 0x31, 0x42, 0x43, 0x48, 0x50, 0xB5, 0xB6, rng.randrange(0xB6)])
            e.w16(p + 0x136, v)
        elif k == 2:
            v = rng.choice([0x42, 0x43, 0x48, 0x50, rng.randrange(0xB6)])
            e.w16(p + 0x136, 0); e.w16(e.r32(p + 0x90), v)
        elif k == 3:
            v = rng.randrange(0, 0x20); e.w16(p + 0x10A, v); e.w16(p + 0x136, rng.choice([0x48, 0x50]))
        elif k == 4:
            v = rng.choice([0, 0, rng.randrange(0, 0x14)]); e.w8(p + 0x17B, v)   # table 0 often (box 1 #45 c13004)
            if v == 0 and rng.random() < 0.7: e.w8(p + 0x1AD, 0)
        elif k == 5:
            v = rng.choice([0, 1]); e.w8(p + 0x1AD, v)
        elif k == 6:
            v = rng.choice([0, 1, 0x80, 0xFF]); e.w8(p + 0x9C, v)
            if rng.random() < 0.5: e.w8(e.rt(SHOW_ALL), rng.choice([0, 1]))
        elif k == 7:
            v = rng.choice([0, 1]); e.w8(p + 0x188, v)
        elif k == 8:
            dx, dy = rng.randrange(-40, 41), rng.randrange(-40, 41)
            e.w16(p + 0x116, (e.r16(p + 0x116) + dx) & 0xFFFF); e.w16(p + 0x118, (e.r16(p + 0x118) + dy) & 0xFFFF)
            v = (dx, dy)
        elif k == 9:
            if rng.random() < 0.5: e.w8(p + 0x1F3, rng.randrange(5))
            v = rng.choice([0, 1, 2, 3, 4, 0xD, 0x2B, 0x2C]); e.w16(p + 0x1E8 + 2 * e.r8(p + 0x1F3), v)
            e.w16(p + 0x136, rng.choice([0x30, 0x31, 0x60]))
        elif k == 11:                                         # a wall piece up-right of the soldier's tile
            v = rng.choice([0x41, 0x60, 0x40, 0x61])
            i = tile - 55 + rng.choice([0, 0, 0, -1, 1, -56])
            if i >= 0: e.w8(e.rt(TILEMAP) + 8 * i + 1, v)
        else:
            viewer = e.r8(e.rt(VIEWER))
            if rng.random() < 0.3: e.w8(e.rt(VIEWER), viewer := rng.choice([0, 1, 0x2F, 0x3F, 0x40]))
            if viewer < 0x30:      # rows 48..63 of the sight table lie over g_tileMap: never written here
                e.w8(e.rt(SEEN) + viewer * 0x200 + slot, rng.choice([0, 1, 2]))
            e.w8(e.rt(SELECTED), rng.choice([slot, slot ^ 1]))
            v = viewer
        done.append(f"{['facing', 'frame136', 'script', 'f10A', 'colour', 'f1AD', 'f9C', 'dead', 'pos', 'code', 'sight', 'wall'][k]}={v}")
    return " ".join(done)


def main():
    rng = random.Random(1)
    fails = total = 0
    for d in dumps:
        f = run(d, ncases, rng)
        print(f"{os.path.basename(d)}: {ncases - f}/{ncases} cases identical")
        fails += f; total += ncases
    print(f"soldier_diff: {total - fails}/{total} cases identical")
    livecast.status("soldier_diff", total, total, fails, "done", force=True)
    sys.exit(1 if fails else 0)


main()
