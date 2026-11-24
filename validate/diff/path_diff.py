"""Differential test: tools/combat.py find_path vs the original FindPath 0x40810 in Unicorn.

usage: .venv/bin/python validate/diff/path_diff.py [DUMP] [n=300] [--port]
--port compares the clean-room port (port/ja1logic via validate/diff/port_lib.py) instead of tools/combat.py.
Runs on a memory snapshot (sector map, move codes, occupants). Cases: random start/dest pairs on the real map, then
the same with fuzzed move codes (every special code: 1500, 1201, 0x6338, 0x62D4, 0x639C, water, walls), fuzzed
occupants, soldier bytes +0x177 / +0xB9, slot (+0x17A, player or enemy) and the combat flag [0x92DCB]. Long paths
recycle closed nodes; constructed maps (a walled-off destination on an open field: the search runs out of trail
records; a serpentine maze: paths longer than the 80-byte buffer) cover the overflow ends. Fixed maps at the end
(path_maps.txt + two equal corridors) sit exactly on the node-pool, trail-record and occupied-cost limits, so a
limit one off fails. Compared: return value, the path
buffer 0x109BD8 + slot * 0x50, soldier +0xD4 (step count) and +0x1AF. Exits 1 on any mismatch.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import combat                        # noqa: E402
from emu import Emu                  # noqa: E402

FIND_PATH, G_TILEMAP, G_COMBAT = 0x40810, 0x100A18, 0x92DCB
args = [a for a in sys.argv[1:] if not a.isdigit() and a != "--port"]
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["300"])[0])
dump = args[0] if args else os.path.join(HERE, "..", "..", "build", "mem_tac60.bin")
rng = random.Random(5)
e = Emu(dump=dump, fast=True)
rt = e.rt
MC = rt(combat.G_MOVECODE)
base_mc = bytes(e.uc.mem_read(MC, 2 * 3136))
base_map = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
SPECIAL = [1000, 1200, 1201, 1500, 1600, 2000, 3000, 3600, 5000, 0x62D4, 0x6338, 0x639C, 25300, 25400, 25500]
fails, found, steps_max, recycled = 0, 0, 0, 0
ends = {}


def run_case(i, start, dest, slot, b177, bB9, flag):
    global fails, found, steps_max, recycled
    sp = rt(combat.G_SOLDIERS) + slot * combat.SLOT
    e.w16(sp + 0x114, start)
    e.w8(sp + 0x177, b177); e.w8(sp + 0xB9, bB9); e.w8(sp + 0x17A, slot)
    e.w16(sp + 0xD4, 0x5555); e.w8(sp + 0x1AF, 0x77)
    e.w8(rt(G_COMBAT), flag)
    buf = rt(combat.PATH_BUF) + slot * 0x50
    e.uc.mem_write(buf, b"\xEE" * 0x50)
    got = e.call(rt(FIND_PATH), sp, dest, 1, max_steps=200_000_000)
    got_buf, got_n, got_af = bytes(e.uc.mem_read(buf, 0x50)), e.r16(sp + 0xD4), e.uc.mem_read(sp + 0x1AF, 1)[0]

    mc = [e.r16(MC + 2 * t) for t in range(3136)]
    tmap = bytes(e.uc.mem_read(rt(G_TILEMAP), 8 * 3136))
    t0 = tmap[0::8]
    occ = lambda t: 0x40 if (tmap[8 * t + 5] & 0x7F) >= 0x30 else tmap[8 * t + 5] & 0x7F
    st = {}
    if PORT:
        want = port_lib.find_path(start, dest, mc, t0, occ, b177, bB9, slot, flag)
        want = (want[0], want[1], want[2], 0 if want[0] else 0x77)   # +0x1AF is the caller's record, not the port's
        ends[want[0]] = ends.get(want[0], 0) + 1
        found += want[0] == 1
    else:
        dirs = combat.find_path(start, dest, mc, t0, occ, b177, bB9, slot, flag, st)
        recycled += st.get("recycled", 0) > 0
        ends[st["end"]] = ends.get(st["end"], 0) + 1
    if PORT:
        pass
    elif dirs is None:
        want = (0, b"\xEE" * 0x50, 0x5555, 0x77)
    else:
        k = min(len(dirs), 0x50)
        want = (1, bytes(dirs[:k]) + b"\xEE" * (0x50 - k), k, 0)
        found += 1
        steps_max = max(steps_max, len(dirs))
    if (got & 0xFFFFFFFF, got_buf, got_n, got_af) != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: start {start} dest {dest} slot {slot} b177 {b177} bB9 {bB9} combat {flag}\n"
                  f"  original {got} n {got_n} {got_buf.hex()}\n  port     {want[0]} n {want[2]} {want[1].hex()}")

for i in range(n):
    e.uc.mem_write(MC, base_mc)
    e.uc.mem_write(rt(G_TILEMAP), base_map)
    fuzz = i >= n // 3
    kind = i % 10
    if fuzz and kind in (0, 1):                   # constructed maps: open field / serpentine maze, few occupants
        e.uc.mem_write(MC, struct.pack("<3136H", *([rng.choice([1000, 1200, 3000])] * 3136)))
        for t in range(3136):
            e.w8(rt(G_TILEMAP) + 8 * t + 5, 0x40)
        if kind == 1:
            for r in range(2, 56, 2):
                gap = 54 if (r // 2) % 2 else 1
                for c in range(56):
                    if c != gap:
                        e.w16(MC + 2 * (r * 56 + c), 0x639C)
        else:                                     # wall off a random destination: explores everything
            d = rng.randrange(57, 3136 - 57)
            for st in combat.DIR_STEP:
                e.w16(MC + 2 * (d + st), 0x639C)
    elif fuzz:
        for _ in range(rng.choice([20, 200, 1500])):
            t = rng.randrange(3136)
            e.w16(MC + 2 * t, rng.choice(SPECIAL + [rng.randrange(0x10000)]))
        for _ in range(rng.choice([0, 10, 100])):
            t = rng.randrange(3136)
            e.w8(rt(G_TILEMAP) + 8 * t + 5, rng.choice([rng.randrange(48), 0x40, 0x80 | rng.randrange(48),
                                                        rng.randrange(256)]))
            e.w8(rt(G_TILEMAP) + 8 * t, rng.randrange(157))
    slot = rng.choice([0, 3, 7, 9, 0x28, 0x2D])
    start, dest = rng.randrange(3136), rng.randrange(3136)
    if fuzz and kind == 0:
        dest = d
    elif fuzz and kind == 1:
        start, dest = rng.randrange(56), 55 * 56 + rng.randrange(56)
    if rng.random() < 0.15:
        dest = start + rng.choice(combat.DIR_STEP + (0, 2, 112)) if 0 <= start + 112 < 3136 else start
    b177, bB9 = rng.choice([0, 0, 1]), rng.choice([0, 1])
    flag = rng.choice([0, 1])
    run_case(i, start, dest, slot, b177, bB9, flag)

# Fixed maps where a pool / record / occupied-cost limit one off changes the result (random maps almost never do).
VALS = [1000, 1200, 3000, 20000, 30000, 65000]
fixed = []
for line in open(os.path.join(HERE, "path_maps.txt")):
    if not line.startswith("#"):
        name, s, d, codes = line.split()
        fixed.append((int(s), int(d), [VALS[int(c)] for c in codes], {}))
corr = [0x639C] * 3136                            # two equal corridors: one through an occupant, one through code 20000
for t in (570, 574, 515, 516, 517, 627, 628, 629):
    corr[t] = 1000
corr[628] = 20000
fixed.append((570, 574, corr, {516: 1}))
for j, (start, dest, codes, occupants) in enumerate(fixed):
    e.uc.mem_write(MC, struct.pack("<3136H", *codes))
    e.uc.mem_write(rt(G_TILEMAP), base_map)
    for t in range(3136):
        e.w8(rt(G_TILEMAP) + 8 * t, 0)
        e.w8(rt(G_TILEMAP) + 8 * t + 5, occupants.get(t, 0x40))
    run_case(n + j, start, dest, 9, 0, 0, 0)
print(f"path_diff{' (port)' if PORT else ''}: {n + len(fixed)} FindPath cases (ends {ends}, {recycled} recycling nodes, longest {steps_max} steps), "
      f"{fails} mismatches")
sys.exit(1 if fails else 0)
