"""Differential test: tools/morning.py complain_pick / complain (the AIM rate complaints of the morning) vs the original.

usage: .venv/bin/python validate/diff/complain_diff.py [N=2000] [DUMP ...] [--seed S] [-v] [--port]
--port: the clean-room library (ja1_complain_pick / ja1_complain, port_lib) instead of the mirror; host ops 288..290
(port_api.md) are mapped to the original's logged calls, random = the scripted random results.
Sections: pick (0x45010: random team slots until an informer (+0x19C = 1) speaks), speak (0x44E7C with a fuzzed speaker slot),
hate (0x44C10: quote 6 about a disliked merc in the team; 0x35810, PlayQuote, StrReplace, 0x50908 stubbed too).
AimRate 0x1FDEC runs for real. Stubbed and logged on both sides: Random 0x53538, 0x67BF0, 0x467E4, 0x673AC,
0x28698 and the quote 0x44D28 (speaker slot, quote, the other merc's profile, the "said" flag before). Each case
fuzzes the 8 team mercs (in sector, health, state byte +0x19C, profile, slot byte), their profiles' AIM limits
(+0x36..+0x39) and the AIM counters of the accounts block. Compared: the ordered call list and the whole data
object afterwards. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import morning as mo                 # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
RANDOM = 0x53538
SOL = ai.G_SOLDIERS

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 2000
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.PAY = True              # accounts (0xA8), profiles
COVER = __import__("collections").Counter()


class Case:
    """the scripted random results of one case, replayed once per side"""
    def __init__(self, rolls):
        self.rolls0 = rolls
        self.reset()

    def reset(self):
        self.rolls = list(self.rolls0)
        self.k = 0
        self.log = []

    def random(self, n):
        self.log.append(("random", ai.s16(n & 0xFFFF)))
        if not self.rolls:               # out of rolls: count up (the pick loop then ends)
            self.k += 1
            return self.k % n
        r = self.rolls.pop(0)
        return r % n if ai.s16(n & 0xFFFF) > 0 else 0


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    case = [None]
    for k in range(8):
        assert e.r32(rt(mo.TEAM_PTRS + 4 * k)) == rt(SOL + 0x200 * k), "pointer table"

    def stub(name, nargs):
        def f(emu, eax, edx, ebx, ecx):
            case[0].log.append((name,) + tuple(ai._i32(x) for x in (eax, edx, ebx)[:nargs]))
        return f

    def orig_quote(emu, eax, edx, ebx, ecx):
        sp = emu.uc.reg_read(UC_X86_REG_ESP)
        pid = emu.r32(sp + 4) & 0xFF
        assert (ecx & 0xFFFFFFFF) == rt(mo.PROFILES + mo.PROFILE_SIZE * pid), "profile pointer"
        case[0].log.append(("complain", eax & 0xFF, ebx & 0xFF, pid, emu.r8(edx & 0xFFFFFFFF)))
        emu.w8(edx & 0xFFFFFFFF, 1)

    e.stub(rt(RANDOM), lambda emu, eax, *_: case[0].random(eax & 0xFFFF))
    e.stub(rt(0x67BF0), stub("w_67bf0", 3))
    e.stub(rt(0x467E4), stub("w_467e4", 1))
    e.stub(rt(0x673AC), stub("w_673ac", 0))
    e.stub(rt(0x28698), stub("w_28698", 1))
    e.stub(rt(0x44D28), orig_quote, 4)

    def slot_of(p):
        a = (p & 0xFFFFFFFF) - rt(SOL)
        return a // 0x200 if a % 0x200 == 0 else ("ptr", a)

    def orig_name(emu, eax, *_):
        a = (eax & 0xFFFFFFFF) - rt(mo.PROFILES)
        case[0].log.append(("name", a // mo.PROFILE_SIZE if a % mo.PROFILE_SIZE == 0 else ("ptr", a)))

    e.stub(rt(0x35810), orig_name)
    e.stub(rt(0x24F08), lambda emu, eax, edx, ebx, ecx: case[0].log.append(("quote", eax & 0xFF, ai._i32(edx), ebx & 0xFF)), 16)
    e.stub(rt(0x252B0), lambda emu, *_: case[0].log.append(("replace",)))
    e.stub(rt(0x50908), lambda emu, eax, edx, ebx, ecx: case[0].log.append(("text_box", slot_of(eax), ai._i32(edx),
                                                                          ai._i32(ebx), ai._i32(ecx))))

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

        def ext(self, name, *a):
            if name == "random":
                return case[0].random(a[0])
            case[0].log.append((name,) + a)
            return 0

    w = World()

    def port_run(name, slot):
        bufs, scalars = port_lib.world_load(e, ai)

        def call(op, a):
            a = [ai._i32(x) for x in a]
            lg = case[0].log
            if op == "cp_begin":
                lg += [("w_67bf0", a[0] & 0xFF, 0x34, 0x25), ("w_467e4", 0), ("w_673ac",)]
            elif op == "cp_quote":
                lg.append(("complain", a[0] & 0xFF, a[1] & 0xFF, a[2] & 0xFF, 0 if a[3] else 1))
            elif op == "cp_end":
                lg.append(("w_28698", 0x25))
            elif op == "hate_quote":         # the host's quote (the original's calls; its two quote globals)
                k, d = a[0], a[1] & 0xFF
                pid = bufs["soldiers"][0x200 * k + mo.PID]
                lg += [("name", d), ("quote", pid, pid * 15, 6), ("replace",), ("w_67bf0", d, 0x34, 0x25),
                       ("w_467e4", 0), ("text_box", k, bufs["soldiers"][0x200 * k + mo.SLOT] * 0x25 + 0xB, 0x14, 5),
                       ("w_28698", 0x25)]
                late.append(d)
            else:
                raise SystemExit(f"complain_diff --port: unmapped op {op} {a}")
            return 0, None

        late = []
        entry = {"pick": "complain_pick", "speak": "complain", "hate": "hated_check"}[name]
        port_lib.sector_run(entry, bufs, scalars, [slot] if name == "speak" else [],
                            lambda n_: case[0].random(n_ & 0xFFFF), call)
        port_lib.world_store(e, ai, bufs, scalars)
        for d in late:
            w.s8(mo.QUOTE_SHOWN, 1)
            w.s8(mo.QUOTE_PROFILE, d)

    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        name = rng.choice(["pick", "pick", "speak", "hate"])
        slot = rng.choice([0, 1, 2, 3, 4, 5, 6, 7, rng.randrange(256)])
        c = Case([rng.randrange(1 << 16) for _ in range(rng.choice([3, 10, 60]))])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        if name == "pick":
            e.call(rt(0x45010), max_steps=2_000_000)
        elif name == "hate":
            e.call(rt(0x44C10), max_steps=2_000_000)
        else:
            e.call(rt(0x44E7C), slot, max_steps=2_000_000)
        log_o = c.log
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        case[0] = c
        if PORT:
            port_run(name, slot)
        elif name == "pick":
            mo.complain_pick(w)
        elif name == "hate":
            mo.hated_check(w)
        else:
            mo.complain(w, slot)
        log_m = c.log
        COVER.update({f"{name} quote {x[2]}" for x in log_o if x[0] == "complain"} | {name} |
                     {f"{name} quote 6" for x in log_o if x[0] == "quote"})
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
                print(f"MISMATCH {os.path.basename(path)} {name} case {i}: " + "; ".join(bad))
                if VERBOSE:
                    print("  log", log_o[:30])
                    print("  mirror", log_m[:30])
    return fails


def fuzz(w, rng):
    for o, vals in ((mo.DAY, [1, 2, 5, 6, 20, 40, 255, rng.randrange(256)]), (mo.DEATHS, [0, 0, 1, 3, rng.randrange(256)]),
                    (mo.HIRINGS, [0, 0, 1, 4, 10, rng.randrange(256)]), (mo.RESIGNATIONS, [0, 0, 1, rng.randrange(256)]),
                    (mo.TERMINATIONS, [0, 0, 1, rng.randrange(256)]), (mo.NON_PAYMENTS, [0, 0, 1, 2, rng.randrange(256)]),
                    (mo.BAD_DAYS, [0, 0, 1, 2, 4, rng.randrange(256)])):
        w.s8(o, rng.choice(vals))
    w.s8(mo.DAY + 1, rng.choice([0, 0, 1]))
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + mo.IN_SECTOR, rng.choice([1, 1, 1, 1, 0]))
        w.s8(s + mo.HP, rng.choice([50, 100, 1, 1, 0, rng.randrange(256)]))
        w.s8(s + mo.B19C, rng.choice([1, 1, 1, 0, 7, rng.randrange(256)]))
        w.s8(s + mo.PID, rng.choice([rng.randrange(60), rng.randrange(60), 59, 60, rng.randrange(64)]))
        w.s8(s + mo.SLOT, rng.choice([k, k, k, k, rng.randrange(8), rng.randrange(256)]))
    for p in range(60):
        a = mo.PROFILES + mo.PROFILE_SIZE * p
        for o in (0x36, 0x37, 0x38, 0x39):
            w.s8(a + o, rng.choice([0, 1, 2, 5, 10, 20, 30, 50, 100, 101, 255, rng.randrange(256)]))
    team = [w.g8(SOL + 0x200 * k + mo.PID) for k in range(8)]
    for k in range(8):                   # the disliked merc: often one of the team (also the merc itself)
        p = w.g8(SOL + 0x200 * k + mo.PID)
        if p < 64:
            a = mo.PROFILES + mo.PROFILE_SIZE * p
            w.s8(a + 0x3A, rng.choice([rng.choice(team), rng.choice(team), 99, rng.randrange(256)]))
            w.s8(a + 0x3C, rng.choice([99, rng.randrange(256)]))


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
        print("cover", dict(sorted(COVER.items())))
    print(f"complain_diff{' --port' if PORT else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
