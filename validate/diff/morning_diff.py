"""Differential test: tools/morning.py (the small steps at the end of the morning chain) vs the original in Unicorn.

usage: .venv/bin/python validate/diff/morning_diff.py [N=2000] [DUMP ...] [--seed S] [-v] [--only NAME]
Sections: wake (0x42378: duty 7 merc 0x92DDA, patients and doctors back on duty, g_teamOnDuty), sync (0x65B40:
salary and stats back into the profiles). Each case fuzzes the 8 team soldiers (duty, hp, hpMax, wounds byte,
salaries, stats, in sector, profile) and compares the whole data object afterwards. Exits 1 on any mismatch.
--port: the clean-room library (ja1_duty_wake / ja1_profile_sync, port_lib) instead of the mirror.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import morning as mo                 # noqa: E402
from emu import Emu                  # noqa: E402

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
SOL = ai.G_SOLDIERS
SECTIONS = {"wake": (0x42378, mo.duty_wake), "sync": (0x65B40, mo.profile_sync)}

args = [a for a in sys.argv[1:] if not a.startswith("-")]
if "--only" in sys.argv:
    only = sys.argv[sys.argv.index("--only") + 1]
    args.remove(only)
    SECTIONS = {only: SECTIONS[only]}
N = int(args[0]) if args and args[0].isdigit() else 2000
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.MORNING = True          # profiles, duty7_merc


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

        def ext(self, name, *a):
            raise AssertionError(name)

    w = World()
    for k in range(8):                   # the steps walk the pointer table 0x100958 (= the team records)
        assert w.g32(0x100958 + 4 * k) == rt(SOL + 0x200 * k), "pointer table"
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        name = rng.choice(sorted(SECTIONS))
        addr, mirror = SECTIONS[name]
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.call(rt(addr), max_steps=2_000_000)
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        if PORT:
            port_run(e, name)
        else:
            mirror(w)
        post_m = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        if post_o != post_m:
            fails += 1
            if fails <= 5:
                diffs = [j for j in range(len(post_o)) if post_o[j] != post_m[j]]
                print(f"MISMATCH {name} case {i}: " + ", ".join(
                    f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_m[j]:02X}" for j in diffs[:6]))
    return fails


def port_run(e, name):
    bufs, scalars = port_lib.world_load(e, ai)

    def rnd(n_):
        raise SystemExit("morning_diff --port: the library asked for a random number")

    def call(op, a):
        raise SystemExit(f"morning_diff --port: unexpected host op {op} {a}")
    port_lib.sector_run(name.replace("wake", "duty_wake").replace("sync", "profile_sync"), bufs, scalars, [], rnd, call)
    port_lib.world_store(e, ai, bufs, scalars)


def fuzz(w, rng):
    g = rng.choice([0x40, 0x40, rng.randrange(8), rng.randrange(0x40), rng.randrange(256)])
    if PORT and 0x30 <= g < 0x40:     # strategic.md Back on duty: records past the 48 of the world
        g = rng.randrange(0x30)
    w.s8(mo.DUTY7_MERC, g)
    w.s32(mo.TEAM_ON_DUTY, rng.choice([0, 1, 5, rng.randrange(1 << 32)]))
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + mo.DUTY, rng.choice([0, 1, 2, 3, 3, 4, 4, 5, 7, rng.randrange(256)]))
        w.s8(s + mo.IN_SECTOR, rng.choice([1, 1, 0]))
        hpm = rng.choice([100, 50, rng.randrange(256)])
        hp = rng.choice([hpm, hpm // 2, rng.randrange(256)])
        w.s8(s + mo.HP_MAX, hpm)
        w.s8(s + mo.HP, hp)
        w.s8(s + mo.WOUNDS, rng.choice([hpm - hp, hpm - hp, 0, rng.randrange(256)]))
        w.s16(s + mo.SALARY, rng.randrange(65536))
        w.s16(s + mo.SALARY2, rng.randrange(65536))
        w.s8(s + mo.PID, rng.choice([rng.randrange(60), rng.randrange(64)]))
        for so, _ in mo.PROFILE_COPY:
            w.s8(s + so, rng.randrange(256))
    for g in range(8, 0x40):             # the duty 7 merc may be any soldier record
        if rng.random() < 0.1:
            w.s8(SOL + 0x200 * g + mo.DUTY, rng.choice([7, 0, rng.randrange(256)]))


def main():
    rng = random.Random(SEED)
    total = fails = 0
    per = max(N // len(dumps), 1)
    for d in dumps:
        if not os.path.exists(d):
            print("missing", d)
            sys.exit(2)
        fails += run_dump(d, rng, per)
        total += per
    print(f"morning_diff{' --port' if PORT else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
