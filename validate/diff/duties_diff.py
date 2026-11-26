"""Differential test: tools/duties.py (the morning duties) vs 0x4B48C in Unicorn.

usage: .venv/bin/python validate/diff/duties_diff.py [N=2000] [DUMP ...] [--seed S] [-v] [--port]
--fan IDS (with --port; #70): the original side runs the fan patch 1.15m code (Emu.put_115m), the port runs with
the fan fixes IDS on (`all` or `F02,F07`); the plain --port run checks the fixes off against 1.13b.
--port: the clean-room library (ja1_morning_duties, port_lib) instead of the mirror; host op 281 practice is
logged like the original's StatPractice call, random = the scripted random results.
Each case fuzzes the 8 team mercs (hp, hpMax, the healed byte +0x19A, duty +0x17E, training +0x199, the stats,
in sector / dead flags, the slot byte) and their items (first aid kits, medical kits, toolkits, repairable items,
stacks with unit conditions, negative stacks). Random 0x53538 and StatPractice 0x62A68 are stubbed on both sides
(scripted random results, StatPractice logged with its arguments); DropItem 0x470B0 runs for real. Compared: the
ordered call list and the whole data object afterwards. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import duties as du                  # noqa: E402
from emu import Emu, EmuStop                  # noqa: E402

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
RANDOM, PRACTICE = 0x53538, 0x62A68

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 2000
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
FAN = None
if "--fan" in sys.argv:
    v = sys.argv[sys.argv.index("--fan") + 1]
    args = [a for a in args if a != v]
    dumps = [a for a in dumps if a != v] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip()[1:]) - 1) for x in v.split(","))
    if not PORT:
        raise SystemExit("duties_diff: --fan needs --port")
SOL = ai.G_SOLDIERS
COVER = __import__("collections").Counter()
STOP_EVERY = 500
STOPS = [0]


class Case:
    """the scripted random results of one case, replayed once per side"""
    def __init__(self, rolls):
        self.rolls0 = rolls
        self.reset()

    def reset(self):
        self.rolls = list(self.rolls0)
        self.log = []

    def random(self, n):
        self.log.append(("random", ai.s16(n & 0xFFFF)))
        r = self.rolls.pop(0) if self.rolls else 0
        return r % n if ai.s16(n & 0xFFFF) > 0 else 0


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    if FAN is not None:
        e.put_115m()
    port_lib.fan_fixes(FAN or 0) if PORT else None
    rt = e.rt
    dd = rt(0x90000) - 0x90000
    case = [None]

    def slot_of(p):
        a = p - SOL
        return a // 0x200 if a % 0x200 == 0 else ("ptr", a)

    def orig_random(emu, eax, *_):
        return case[0].random(eax & 0xFFFF)

    def orig_practice(emu, eax, edx, ebx, ecx):
        case[0].log.append(("practice", slot_of((eax & 0xFFFFFFFF) - dd), edx & 0xFF, ai.s16(ebx & 0xFFFF), ecx & 0xFF))
        return 0

    e.stub(rt(RANDOM), orig_random)
    e.stub(rt(PRACTICE), orig_practice)

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

        def ext(self, name, *a):
            if name == "random":
                return case[0].random(a[0] & 0xFFFF)
            if name == "practice":
                case[0].log.append(("practice", slot_of(a[0]), a[1] & 0xFF, ai.s16(a[2] & 0xFFFF), a[3] & 0xFF))
                return 0
            raise AssertionError(name)

    w = World()

    def port_run():
        bufs, scalars = port_lib.world_load(e, ai)

        def call(name, a):
            op = port_lib.ALL_OPS.get(name, name) if isinstance(name, int) else name
            a = [ai._i32(x) for x in a]
            if op in (47, "div_zero"):          # host call 47 = stop point
                case[0].log.append(("stop point",))
                return 0, None
            if op != "practice":
                raise SystemExit(f"duties_diff --port: unmapped op {op} {a}")
            case[0].log.append(("practice", a[0], a[1] & 0xFF, ai.s16(a[2] & 0xFFFF), a[3] & 0xFF))
            return 0, None

        port_lib.sector_run("morning_duties", bufs, scalars, [], lambda n_: case[0].random(n_ & 0xFFFF), call,
                            sync=True)
        port_lib.world_store(e, ai, bufs, scalars)
    repairable = [t for t in range(1, 136) if w.g8(du.ITEM_TBL + 12 * t + 9)]
    stacked = [t for t in repairable if w.g8(du.ITEM_TBL + 12 * t + 6) > 1]
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng, repairable, stacked)
        stop = i % STOP_EVERY == STOP_EVERY - 1
        if stop:
            stop_case(w, rng, repairable, stacked, i // STOP_EVERY % 3)
        c = Case([rng.randrange(1 << 16) for _ in range(40)])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        try:
            e.call(rt(0x4B48C), max_steps=5_000_000)
            orig_stop = None
        except EmuStop as x:
            orig_stop = str(x)
        if stop or orig_stop:
            # a loop of about 2^32 rounds (strategic.md "Doctor" / "Repair"): the original is still in DoctorHeal /
            # RepairDuty at the step limit, the port must make its stop point (host call 47)
            e.uc.mem_write(rt(DATA_LO), pre)
            c.reset()
            if PORT:
                port_run()
            eip = int(orig_stop.rsplit("0x", 1)[1], 16) if orig_stop else 0
            in_loop = any(rt(lo) <= eip < rt(hi) for lo, hi in ((0x48418, 0x48684), (0x48684, 0x4882C)))
            ok = in_loop and (not PORT or ("stop point",) in c.log)
            STOPS[0] += 1
            if not ok:
                fails += 1
                print(f"MISMATCH {os.path.basename(path)} case {i}: stop case {stop}, original {orig_stop}, "
                      f"port {'stop point' if ('stop point',) in c.log else 'no stop point'}")
            continue
        log_o = c.log
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        case[0] = c
        if PORT:
            port_run()
        else:
            du.morning_duties(w)
        log_m = c.log
        COVER.update({x[0] if x[0] == "random" else f"practice {x[2]}" for x in log_o})
        post_m = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        bad = []
        if log_o != log_m:
            k = next((j for j, (a, b) in enumerate(zip(log_o, log_m)) if a != b), min(len(log_o), len(log_m)))
            bad.append(f"call {k}: original {log_o[k] if k < len(log_o) else None}, "
                       f"mirror {log_m[k] if k < len(log_m) else None}")
        if post_o != post_m:
            diffs = [j for j in range(len(post_o)) if post_o[j] != post_m[j]]
            bad.append("memory: " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_m[j]:02X}" for j in diffs[:6]))
        if bad:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} case {i}: " + "; ".join(bad))
                if VERBOSE:
                    print("  log", log_o[:30])
                    print("  mirror", log_m[:30])
    return fails


def fuzz(w, rng, repairable, stacked):
    for k in range(8):
        s = SOL + 0x200 * k
        hpm = rng.choice([100, 80, 45, 9, 10, 99, rng.randrange(101)])    # sane values: hp <= hpMax, healed <=
        hp = rng.choice([hpm, hpm - 1, hpm // 2, 15, 14, 0, 1, rng.randrange(hpm + 1)])   # the gap (else a doctor with
        w.s8(s + du.HP_MAX, hpm)                                          # a negative budget loops 2^32 times)
        hp = min(max(hp, 0), hpm)
        w.s8(s + du.HP, hp)
        w.s8(s + du.HEALED, rng.choice([0, 0, 0, min(1, hpm - hp), rng.randrange(hpm - hp + 1)]))
        w.s8(s + du.DUTY, rng.choice([0, 1, 2, 3, 3, 4, 4, 5, 5, 6, 7, rng.randrange(256)]))
        w.s8(s + du.TRAIN, rng.choice([0, 1, 2, 3, 4, rng.randrange(256)]))
        w.s8(s + du.SLOT, rng.choice([k, k, k, k, rng.randrange(256)]))
        w.s8(s + 0x182, rng.choice([1, 1, 0]))
        w.s8(s + 0x188, rng.choice([0, 0, 0, 1]))
        w.s8(s + 0x19B, rng.choice([0, 1, 2]))
        for o in (du.AGI, du.DEX, du.WIS, du.MED, du.EXPL, du.MECH, du.MARKS):
            w.s8(s + o, rng.choice([0, 30, 50, 80, 99, 100, rng.randrange(101)]))
        w.s8(s + du.LEVEL, rng.choice([0, 1, 3, 10, rng.randrange(11)]))
        for j in range(12):
            r = s + 12 * j
            kind = rng.random()
            if j == 0 and kind < 0.6:
                t = du.TOOLKIT
            elif kind < 0.3:
                t = rng.choice([du.FIRST_AID, du.MEDKIT])
            elif kind < 0.6 and repairable:
                t = rng.choice(stacked if stacked and rng.random() < 0.4 else repairable)
            else:
                t = rng.choice([0, 0, rng.randrange(1, 136)])
            w.s8(r, t)
            w.s8(r + 1, rng.choice([0, 1, 2, 3, 5, -1 & 0xFF, -3 & 0xFF]))
            for b in range(2, 12):
                w.s8(r + b, rng.choice([0, 1, 10, 11, 50, 99, 100, rng.randrange(101)]))


def stop_case(w, rng, repairable, stacked, kind):
    """One merc gets an input that the original loops on about 2^32 times: kind 0 = doctor with a negative budget
    and hp above hpMax, 1 = repair with negative points on a condition above 100, 2 = the same on a clip."""
    k = rng.randrange(8)
    s = SOL + 0x200 * k
    if kind == 0:
        w.s8(s + du.DUTY, 3)
        hpm = rng.randrange(1, 100)
        w.s8(s + du.HP_MAX, hpm)
        w.s8(s + du.HP, rng.randrange(hpm + 1, 128))
        w.s8(s + du.WIS, 0)                    # cap = ((0 - (100 - dex) x 20 / 100) x med / 100) x 2 < 0
        w.s8(s + du.DEX, rng.randrange(50))
        w.s8(s + du.MED, rng.randrange(50, 101))
        return
    w.s8(s + du.DUTY, 5)
    w.s8(s + du.MECH, rng.randrange(50, 101))
    w.s8(s + du.DEX, rng.randrange(50, 101))
    w.s8(s, du.TOOLKIT)
    w.s8(s + 2, rng.randrange(0x80, 0xF0))     # toolkit condition < 0: points < 0
    r = s + 12
    for o in (1, 7, 8, 9, 10, 11):             # only place 1 is repairable
        if o != 1:
            w.s8(s + 12 * o, 0)
    if kind == 1:
        w.s8(r, rng.choice(repairable))
        w.s8(r + 1, 1)
        w.s8(r + 2, rng.randrange(101, 128))
    else:
        w.s8(r, rng.choice(stacked))
        w.s8(r + 1, 3)
        w.s8(r + 2, 100)
        w.s8(r + 8, rng.choice([100, rng.randrange(101, 128)]))
        w.s8(r + 9, rng.randrange(101, 128))


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
    if VERBOSE:
        print(" ".join(f"{k}:{v}" for k, v in sorted(COVER.items())))
    print(f"duties_diff{' --port' if PORT else ''}{f' --fan {FAN:#x}' if FAN is not None else ''}: {total} cases on {len(dumps)} dumps ({STOPS[0]} stop cases), "
          f"{fails} mismatches")
    sys.exit(1 if fails else 0)


main()
