"""Differential test: the sunrise (strategic.md "Sunrise").

usage: .venv/bin/python validate/diff/sunrise_diff.py [ncases=2000] [DUMP] [--port] [--lib SO] [-v]
The original: the overnight healing 0x4B3F0 runs in Unicorn on tac60 (Random 0x53538 stubbed: a Python RNG, each
draw logged); the other sunrise steps are plain stores of PlayLoop / SunriseScreen? (g_day + 1, memset of
g_statChanged 0x10DD10, the i16 at 0x22A02C = 0) and are applied here to the original's memory as the spec says.
Fuzzed: soldiers 0..7 (inCurSector, hp, hpMax), the day, stat_changed, the arrivals count.
--port: ja1_sunrise of port/ja1logic on the same state, random answered with the original's draws (the n of each
draw is compared): world (port_lib world_load), the host op log and the draws are compared. Exits 1 on mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 2000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.PAY = True                  # accounts (0xA8)
port_lib.STATS = True                # stat_changed
VERBOSE = "-v" in argv

HEAL, PTRS, DAY, ARRIVALS, STAT_CHANGED = 0x4B3F0, 0x100958, 0x229FA4, 0x22A02C, 0x10DD10

if not os.path.exists(DUMP):
    print(f"sunrise_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
draws = []
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = eax & 0xFFFF
    n = n - 0x10000 if n & 0x8000 else n
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


e.stub(rt(0x53538), rnd)


def fuzz(r):
    for k in range(8):
        p = e.r32(rt(PTRS) + 4 * k)
        e.w8(p + 0x182, r.choice([0, 1, 1, 1, r.randrange(256)]))
        m = r.choice([100, 75, 50, 30, 15, 14, 1, 0, 127, 128, 200, r.randrange(256)])
        e.w8(p + 0x184, m)
        e.w8(p + 0x183, r.choice([m, (m - r.randrange(1, 40)) & 0xFF, (m + r.randrange(1, 20)) & 0xFF, 0,
                                  r.randrange(256)]))
    e.w32(rt(DAY), r.choice([0, 1, 5, 30, 0x7FFFFFFF, 0xFFFFFFFF, r.randrange(1 << 32)]))
    e.w16(rt(ARRIVALS), r.choice([0, 1, 3, r.randrange(1 << 16)]))
    if r.random() < .5:
        e.uc.mem_write(rt(STAT_CHANGED), bytes(r.choice([0, 1]) for _ in range(640)))


def original():
    draws.clear()
    e.w32(rt(DAY), (e.r32(rt(DAY)) + 1) & 0xFFFFFFFF)
    e.uc.mem_write(rt(STAT_CHANGED), bytes(640))
    e.w16(rt(ARRIVALS), 0)
    e.call(rt(HEAL))
    return list(draws)


def port(bufs, scalars, dr):
    q = list(dr)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else 0
        got.append((n, v))
        return v

    def call(op, a):
        if op == "sunrise_show":
            plog.append(op)
            return 0, None
        raise SystemExit(f"sunrise_diff --port: unmapped op {op} {list(a)}")
    port_lib.sector_run("sunrise", bufs, scalars, [], random_, call)
    return got, plog


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        pre = port_lib.world_load(e, ai) if PORT else None
        dr = original()
        cover["draws"] += len(dr)
        cover["cases with draws"] += bool(dr)
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        got, plog = port(bufs, scalars, dr)
        bad = []
        if got != dr:
            bad.append(f"draws {dr} vs {got}")
        if plog != ["sunrise_show"]:
            bad.append(f"ops {plog}")
        for k in bufs_o:
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i}: " + "; ".join(bad)[:400])
    if VERBOSE or not PORT:
        print("cover:", dict(cover))
    if not cover["draws"]:
        print("sunrise_diff: no random draw reached")
        fails += 1
    print(f"sunrise_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
