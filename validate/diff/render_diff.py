"""Differential test (in ./t check): the clean-room front-end's static map (port/ja1front, ja1f_render_view) vs tools/sectormap.py's
renderer, which tiles_diff.py checks byte for byte against the original draw passes.

usage: .venv/bin/python validate/diff/render_diff.py [nviews=3] [--sectors 1-60] [--lib PATH]
For every sector, nviews random 14 x 12 windows (plus the four map corners on the first sector of each tileset) are
drawn by both and compared byte for byte. The front-end reads rom/JA1.gog and rom/JA.EXE itself. Exits 1 on any
mismatch. Build first: (cd port/ja1front && ~/.cargo/bin/cargo build --release).
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import sectormap as sm               # noqa: E402
import livecast                      # noqa: E402
from csprite import palette          # noqa: E402

argv = sys.argv[1:]
lib_path = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(
    ROOT, "port", "ja1front", "target", "release", "libja1front.so")
lo, hi = (1, 60)
if "--sectors" in argv:
    lo, hi = map(int, argv[argv.index("--sectors") + 1].split("-"))
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--sectors"))]
nviews = int(args[0]) if args else 3

lib = ctypes.CDLL(lib_path)
lib.ja1f_render_view.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_int32] * 5 + [ctypes.c_void_p]
lib.ja1f_render_view.restype = ctypes.c_int32
lib.ja1f_set_roofs.argtypes = [ctypes.c_uint8]
ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
rng = random.Random(1)
COLS, ROWS = 14, 12
fails = views = 0
seen_ts = set()
renderers = {}
pals = {}
for sector in range(lo, hi + 1):
    tiles, ts = sm.terrain(sector)
    windows = [(rng.randrange(57 - COLS), rng.randrange(57 - ROWS)) for _ in range(nviews)]
    if ts not in seen_ts:
        seen_ts.add(ts)
        windows += [(0, 0), (56 - COLS, 0), (0, 56 - ROWS), (56 - COLS, 56 - ROWS)]
    if ts not in renderers:
        renderers[ts] = sm.Renderer(tiles, ts)
    R = renderers[ts]
    R.tiles = tiles
    for k, (c0, r0) in enumerate(windows):
        # Every second view with the roofs flag off (tactical_map.md "Drawing order", debug key Alt+Q).
        R.roofs = k % 2 == 0
        lib.ja1f_set_roofs(int(R.roofs))
        R.buf = bytearray(sm.BW * sm.BH)
        R.draw(c0, r0, COLS, ROWS)
        want = R.window(COLS, ROWS)
        out = (ctypes.c_uint8 * (16 * COLS * 14 * ROWS))()
        r = lib.ja1f_render_view(ISO, EXE, sector, c0, r0, COLS, ROWS, out)
        got = bytes(out)
        views += 1
        livecast.status("render_diff", views, None, fails, f"sector {sector}, view ({c0}, {r0})")
        livecast.frame("render_diff", f"static map, sector {sector}, view ({c0}, {r0}): reference | port | differences",
                   want, got, 16 * COLS, 14 * ROWS, pals.setdefault(ts, palette(ts)), force=got != want)
        if r != 0 or got != want:
            fails += 1
            if fails <= 10:
                diff = [i for i in range(len(want)) if got[i] != want[i]]
                first = diff[0] if diff else None
                print(f"MISMATCH sector {sector} (tileset {ts}) view ({c0}, {r0}): return {r}, {len(diff)} pixels differ"
                      + (f", first at x {first % (16 * COLS)} y {first // (16 * COLS)}: ours {got[first]:#x}"
                         f" reference {want[first]:#x}" if diff else ""))
print(f"render_diff: {views} views over sectors {lo}-{hi}, {fails} mismatches")
livecast.status("render_diff", views, views, fails, "done", force=True)
sys.exit(1 if fails else 0)
