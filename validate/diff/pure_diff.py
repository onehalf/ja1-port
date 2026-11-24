"""Differential test of small pure functions: the original in Unicorn vs tools/ai.py (or the port with --port).

usage: .venv/bin/python validate/diff/pure_diff.py [N] [--port]
  direction  DirectionTo? 0x52560 (pixel points -> 1..8)   port: ja1_direction
  tile_dist  TileDistance 0x15A24 (two tiles -> distance)   port: ja1_tile_dist
  aim_dir    DirectionAtan? 0x5260C (pixel points -> 1..8 + the +0x10A word): every tile pair + N fuzzed points
             port: ja1_aim_dir
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import ai                            # noqa: E402
from emu import Emu                  # noqa: E402

PORT = "--port" in sys.argv
args = [a for a in sys.argv[1:] if a != "--port"]
n = int(args[0]) if args else 20000
e = Emu(dump=os.path.join(HERE, "..", "..", "build", "mem_tac60.bin"))
if PORT:
    import port_lib
    lib = port_lib.lib()
    for f in ("ja1_direction", "ja1_tile_dist"):
        getattr(lib, f).restype = ctypes.c_int32
    lib.ja1_direction.argtypes = [ctypes.c_int32] * 4
    lib.ja1_tile_dist.argtypes = [ctypes.c_int32] * 2
    lib.ja1_aim_dir.restype = ctypes.c_int32
    lib.ja1_aim_dir.argtypes = [ctypes.c_int32] * 4 + [ctypes.POINTER(ctypes.c_uint16)]
    direction = lib.ja1_direction
    tile_dist = lib.ja1_tile_dist

    def aim_dir(x1, y1, x2, y2):
        w = ctypes.c_uint16(0x5555)
        return lib.ja1_aim_dir(x1, y1, x2, y2, ctypes.byref(w)), w.value
else:
    direction, tile_dist, aim_dir = ai.direction_to, ai.tile_distance, ai.aim_dir
OUT = e.alloc(bytes(4))
rng = random.Random(3)
fails = {"direction": 0, "tile_dist": 0, "aim_dir": 0}


def check_aim(x1, y1, x2, y2):
    e.w16(OUT, 0x5555)
    d = e.call(e.rt(0x5260C), x1 & 0xFFFFFFFF, y1 & 0xFFFFFFFF, x2 & 0xFFFFFFFF, y2 & 0xFFFFFFFF, OUT) & 0xFF
    got, want = (d, e.r16(OUT)), aim_dir(x1, y1, x2, y2)
    if got != want:
        fails["aim_dir"] += 1
        if fails["aim_dir"] <= 5:
            print(f"MISMATCH aim_dir ({x1}, {y1}) -> ({x2}, {y2}): original {got}, test {want}")


n_aim = 0
for dc in range(-55, 56):                        # every tile-to-tile step on the 56-column map (pixel points 16 x 14)
    for dr in range(-60, 61):
        check_aim(485, 425, 485 + 16 * dc, 425 + 14 * dr)
        n_aim += 1
for i in range(n):
    r = rng.choice([1, 3, 20, 200, 900, 30000])
    x1, y1 = rng.randrange(-50, 900), rng.randrange(-50, 800)
    x2, y2 = x1 + rng.randrange(-r, r + 1), y1 + rng.randrange(-r, r + 1)
    if rng.random() < 0.2:                       # exact diagonals and axes
        d = rng.randrange(-300, 300)
        x2, y2 = rng.choice([(x1 + d, y1 + d), (x1 + d, y1 - d), (x1, y1 + d), (x1 + d, y1)])
    got = ai._i32(e.call(e.rt(0x52560), x1 & 0xFFFFFFFF, y1 & 0xFFFFFFFF, x2 & 0xFFFFFFFF, y2 & 0xFFFFFFFF))
    want = direction(x1, y1, x2, y2)
    if got != want:
        fails["direction"] += 1
        if fails["direction"] <= 5:
            print(f"MISMATCH direction ({x1}, {y1}) -> ({x2}, {y2}): original {got}, test {want}")
    a, b = rng.choice([rng.randrange(3136), rng.randrange(-3136, 6272), 0xC41]), rng.randrange(-200, 3400)
    got = ai._i32(e.call(e.rt(0x15A24), a & 0xFFFFFFFF, b & 0xFFFFFFFF))
    want = tile_dist(a, b)
    if got != want:
        fails["tile_dist"] += 1
        if fails["tile_dist"] <= 5:
            print(f"MISMATCH tile_dist {a} {b}: original {got}, test {want}")
    check_aim(x1, y1, x2, y2)
    n_aim += 1
print(f"pure_diff{' --port' if PORT else ''}: direction {n}, tile_dist {n}, aim_dir {n_aim}; {sum(fails.values())} mismatches")
sys.exit(1 if any(fails.values()) else 0)
