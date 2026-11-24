"""Differential test: tools/combat.py projectile_hit_test vs the original ProjectileHitTest 0x577B4 in Unicorn.

usage: .venv/bin/python validate/diff/flight_diff.py [DUMP] [n=20000] [--port]
Runs on a memory snapshot (class / window / posture tables, soldier records). Each case fuzzes one tile's layers
(objects from every class: trees, walls, low objects, roofs), optionally a soldier standing on it (tile, position,
animation), and the projectile state (height, position inside the tile, pixels flown, direction, cap height,
target tile, skip byte, roll flag). Random 0x53538 is scripted on both sides; result, skip byte, write-back chance
and the Random calls must match. --port: the clean-room port's ja1_hit_test (shooting.md batch 5) on the world
loaded from the same memory instead of tools/combat.py. Exits 1 on any mismatch.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import combat                        # noqa: E402
from emu import Emu                  # noqa: E402

HIT_TEST, RANDOM = 0x577B4, 0x53538
PORT = "--port" in sys.argv
args = [a for a in sys.argv[1:] if not a.isdigit() and a != "--port"]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["20000"])[0])
dump = args[0] if args else os.path.join(HERE, "..", "..", "build", "mem_tac60.bin")
rng = random.Random(4)
e = Emu(dump=dump)
rt = e.rt
if PORT:
    import ai                        # noqa: E402
    import port_lib                  # noqa: E402
read = lambda a, k: bytes(e.uc.mem_read(rt(a), k))
script, calls = [], []


def stub_random(emu, eax, *_):
    k = combat.s16(eax)
    calls.append(k)
    return script[len(calls) - 1] % k if k > 0 else 0


e.stub(rt(RANDOM), stub_random)
skip_p, state_p = e.alloc(4), e.alloc(4)
t1class = read(combat.G_T1CLASS, 256)
by_class = {}
for o in range(256):
    by_class.setdefault(t1class[o], []).append(o)
t0_roof = [v for v in range(157) if read(combat.G_T0CLASS + v, 1)[0] == 3] or [0]
SLOT_FREE = 0x2C                     # an unused enemy slot we can overwrite
fails = 0
results = {}

# Forced part: the last 1 in 5 cases reach the window branch (a wall object and a direction with a table byte >= 0x62, no
# soldier, skip 0, height below 9000), with every entry chance. The random fuzz meets this branch about 1 time in 500.
window = read(combat.WINDOW_TBL, 2056)
wall_dirs = [(o, d) for o in range(256) if t1class[o] in (7, 8) for d in range(8)
             if o * 8 + d < 2056 and combat.s8(window[o * 8 + d]) >= 0x62]
FORCED_FROM = n - n // 5
forced = {"stop": 0, "held by chance": 0, "pass": 0}

for i in range(n):
    tile = rng.randrange(56 * 2, 3136 - 56 * 2)
    a = rt(combat.G_TILEMAP) + 8 * tile
    cls = rng.choice([0, 1, 6, 6, 7, 8, 9, 9, rng.randrange(256)])
    obj = rng.choice(by_class.get(cls, [0]) + ([0xE2, 0xE3] if cls == 9 else []))
    t0 = rng.choice(t0_roof + [rng.randrange(157)])
    t3 = rng.choice([0, 0x51, 0x52, 0x73, rng.randrange(256)])
    occ = 0x40
    if rng.random() < 0.35:
        occ = SLOT_FREE
        s = bytearray(read(combat.G_SOLDIERS + occ * combat.SLOT, combat.SLOT))
        cx, cy = combat.tile_xy(tile)
        struct.pack_into("<Hhh", s, 0x114, rng.choice([tile, tile, rng.randrange(3136)]), cx - 2 + rng.randrange(-2, 3),
                         cy - 4 + rng.randrange(-2, 3))
        s[0x1F3] = rng.randrange(5)
        struct.pack_into("<H", s, 0x1E8 + 2 * s[0x1F3], rng.choice([0, 2, 3, 4, 0x15, 0x37, 0x38, 0x3D, rng.randrange(0x60)]))
        e.uc.mem_write(rt(combat.G_SOLDIERS + occ * combat.SLOT), bytes(s))
    e.uc.mem_write(a, bytes([t0, obj, 0, t3, 0, occ, 0xFF, 0]))
    cx, cy = combat.tile_xy(tile)
    x, y = cx + rng.randrange(-6, 9), cy + rng.randrange(-5, 7)
    if combat.view_tile(x, y) != tile:
        x, y = cx, cy
    height = rng.choice([0, 1, 999, 1000, 3000, 4500, 5500, 6000, 7000, 9000, 9001, 10000, 10001, 12000,
                         rng.randrange(-100, 23000)])
    flown = rng.choice([0, 15, 29, 30, 74, 75, rng.randrange(0, 400)])
    direction = rng.randrange(8)
    cap = rng.choice([0, 0, 9000, 22000])
    target = rng.choice([tile, rng.randrange(3136)])
    roll = rng.choice([0, 1])
    skip = rng.choice([0, 0, rng.randrange(1, 256)])
    script[:] = [rng.randrange(0x7FFF) for _ in range(4)]
    chance0 = rng.choice([0, 0, 100, rng.randrange(1, 100)])   # (LineOfFire starts it at 100, Projectile step at 0)
    if i >= FORCED_FROM and wall_dirs:
        obj, direction = rng.choice(wall_dirs)
        cls = t1class[obj]
        occ, skip, height = 0x40, 0, rng.choice([1, 1000, 4500, 7000, 8999])
        flown = rng.choice([15, 30, 75, 200, rng.randrange(0, 400)])
        e.uc.mem_write(a, bytes([t0, obj, 0, t3, 0, occ, 0xFF, 0]))
    e.w8(skip_p, skip); e.w32(state_p, chance0)
    calls.clear()
    got = combat.s16(e.call(rt(HIT_TEST), height & 0xFFFFFFFF, x, y, flown, direction, skip_p, state_p, cap, target, roll)
                     & 0xFFFF)
    got_full = (got, e.r8(skip_p), e.r32(state_p), calls[:])
    seen = []
    def r(k):
        seen.append(k)
        return script[len(seen) - 1] % k if k > 0 else 0
    if PORT:
        bufs, scalars = port_lib.world_load(e, ai)
        out = bytearray(struct.pack("<BxxxI", skip, chance0))
        res = port_lib.sector_run("hit_test", bufs, scalars, [height, x, y, flown, direction, skip, cap, target, roll, out],
                                  r, lambda op, a: (_ for _ in ()).throw(SystemExit(f"port called host op {op} {a}")))
        sk, ch = out[0], struct.unpack_from("<I", out, 4)[0]
        res = combat.s16(res & 0xFFFF)
    else:
        res, sk, ch = combat.projectile_hit_test(height, x, y, flown, direction, skip, cap, target, roll, r, read, chance0)
    want = (res, sk, ch, seen)
    if i >= FORCED_FROM:
        forced["stop" if got_full[0] == 1 else "held by chance" if calls and calls[0] == 100 and script[0] % 100 >= got_full[2]
               and chance0 else "pass"] += 1
    results[res] = results.get(res, 0) + 1
    if got_full != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH: original {got_full} port {want} | class {cls} obj {obj:#x} t0 {t0:#x} t3 {t3:#x} occ {occ:#x} "
                  f"h {height} xy {x},{y} flown {flown} cap {cap} target {target == tile} roll {roll} skip {skip}")
print(f"flight_diff: {n} cases (results {dict(sorted(results.items()))}), {fails} mismatches")
print(f"flight_diff forced window cases: {forced}")
if n >= 1000 and min(forced.values()) < 5:         # the forced part must reach all three ends of the branch
    print("flight_diff: the forced window cases do not reach all ends")
    fails += 1
sys.exit(1 if fails else 0)
