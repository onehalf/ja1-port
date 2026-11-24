"""Differential test: stat practice (combat.md "Stat practice", exact steps).

usage: .venv/bin/python validate/diff/practice_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original: StatPractice 0x62A68 (EAX soldier, DL stat, BX rolls, CL flag) runs in Unicorn on tac60; Random
0x53538 is a logged Python RNG. Fuzzed: the slot (mostly a merc), the soldier's row byte +0x17A, practice mode
+0x19B, state +0x19C, the nine stat bytes, the row's nine progress counters, stat 0..10, rolls (negative, small,
rarely large) and flag.
--port: ja1_stat_practice on the same state; the random draws and the world are compared. Exits 1 on any mismatch.
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
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv
port_lib.STATS = True                # stat_progress (and stat_changed)
SOLDIERS, PROGRESS = 0xFA958, ai.G_STAT_PROGRESS
FIELDS = (0x184, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191, 0x192)

if not os.path.exists(DUMP):
    print(f"practice_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
draws = []
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = ai.s16(eax)
    # edge values often (n - 1, n - 2 = the chance caps 99 / 98, 0), else uniform
    v = min(max(rng.choice([rng.randrange(n)] * 3 + [n - 1, n - 2, 0]), 0), n - 1) if n > 0 else 0
    draws.append((n, v))
    return v


e.stub(rt(0x53538), rnd)


def fuzz(r):
    slot = r.randrange(8) if r.random() < .85 else r.randrange(48)
    s = rt(SOLDIERS) + 0x200 * slot
    e.w8(s + 0x17A, slot if slot < 8 and r.random() < .9 else r.choice([r.randrange(8), 8, r.randrange(256)]))
    e.w8(s + 0x19B, r.choice([0, 1, 1, 1, 2, 2, 3, r.randrange(256)]))
    e.w8(s + 0x19C, r.choice([0, 6, 6, r.randrange(256)]))
    stat = r.choice(list(range(1, 10)) * 3 + [0, 10, r.randrange(256)])
    for k, o in enumerate(FIELDS):
        v = r.choice([r.randrange(1, 101), r.randrange(256), 0, 1, 99, 100, 128])
        if k == 8 and v == 0:
            v = 1                    # level 0 divides by zero in the original (combat.md); not fuzzed
        e.w8(s + o, v)
    row = e.r8(s + 0x17A)
    if row < 8:
        for k in range(9):
            e.w16(rt(PROGRESS) + 0x12 * row + 2 * k,
                  r.choice([0, r.randrange(100), r.randrange(10000), r.randrange(1 << 16), 0x7FFF, 0x8000]))
    if row < 8 and 1 <= stat <= 9 and r.random() < .2:
        # the chance near its cap: a low stat, little progress, a wise merc
        e.w8(s + FIELDS[stat - 1], r.choice([1, 2]) if stat == 9 else r.choice([0, 1, 2]) if stat != 4 else 60)
        e.w16(rt(PROGRESS) + 0x12 * row + 2 * (stat - 1), r.randrange(100))
        e.w8(s + 0x18D, r.choice([60, 100, 127, 200]))
        if stat != 4:
            e.w16(rt(PROGRESS) + 0x12 * row + 6, r.randrange(100))
    n = r.choice([0, 1, 2, 3, 5, 10, 20, -1, -5, r.randrange(200), 0x5556 if r.random() < .05 else 4])
    flag = r.choice([0, 0, 0, 1, 2, r.randrange(256)])
    return slot, stat, n & 0xFFFF, flag


def port(slot, stat, n, flag, bufs, scalars, dr):
    q, got = list(dr), []

    def random_(m):
        v = q.pop(0)[1] if q else 0
        got.append((m, v))
        return v

    def call(op, a):
        raise SystemExit(f"practice_diff --port: unexpected op {op} {a}")
    port_lib.sector_run("stat_practice", bufs, scalars, [slot, stat, ai.s16(n), flag], random_, call)
    return got


def first_diff(a, b):
    j = next((j for j in range(min(len(a), len(b))) if a[j] != b[j]), min(len(a), len(b)))
    return f"from #{j}: {a[j:j + 4]} vs {b[j:j + 4]}"


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        slot, stat, n, flag = fuzz(r)
        s = rt(SOLDIERS) + 0x200 * slot
        row, mode = e.r8(s + 0x17A), e.r8(s + 0x19B)
        pw = port_lib.world_load(e, ai) if PORT else None
        before = bytes(e.uc.mem_read(rt(PROGRESS), 8 * 0x12))
        draws.clear()
        e.call(rt(0x62A68), s, stat, n, flag)
        dr = list(draws)
        after = bytes(e.uc.mem_read(rt(PROGRESS), 8 * 0x12))
        if dr:
            cover[f"mode {'gain' if mode == 1 else 'loss'} stat {stat}"] += 1
            if after != before:
                cover["changed"] += 1
            if stat == 8 and e.r8(s + 0x19C) == 6 and mode == 1:
                cover["marksmanship state 6"] += 1
            if flag:
                cover["flagged"] += 1
        else:
            cover["no roll"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pw
        dp = port(slot, stat, n, flag, bufs, scalars, dr)
        bad = []
        if dr != dp:
            bad.append(f"random {first_diff(dr, dp)}")
        for k in bufs_o:
            if k in port_lib.READ_ONLY:
                continue
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} (slot {slot} row {row} mode {mode} stat {stat} n {ai.s16(n)} flag {flag}): "
                      + "; ".join(bad)[:700])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["no roll", "changed", "marksmanship state 6", "flagged"]
    need += [f"mode {m} stat {k}" for m in ("gain", "loss") for k in range(1, 10)]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"practice_diff: never reached: {missing}")
        fails += 1
    print(f"practice_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
