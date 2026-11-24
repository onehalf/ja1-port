"""Differential test: tools/morning.py corpse_export (the corpse export question of the morning) vs the original.

usage: .venv/bin/python validate/diff/corpse_diff.py [N=2000] [DUMP ...] [--seed S] [-v] [--port]
--port: the clean-room library (ja1_corpse_export, port_lib) instead of the mirror; its host ops (port_api.md)
are mapped to the original's logged calls, random = the scripted random results.
CorpseExportPrompt? 0x46328 runs with its parts for real (CorpseRefused? 0x462E0, CorpseGrudge? 0x46548,
TeamHasProfile? 0x50AC0, MercLeaves 0x50618, NativeLeaves? 0x505A0, team compaction 0x50750, AddStashItem 0x51D50,
itoa, AddFunds? 0x514A4). Stubbed and logged on both sides: Random 0x53538, the file open 0x5E8E0, StrReplace
0x252B0 (done in Python),
ConfirmDialog? 0x275E4 (scripted answer), 0x28698, 0x36010, 0x67BF0, BlitCopy 0x52AE0, 0x673AC, DrawBassMap?
0x4420C, 0x4508C, the face clear 0x46AC4, 0x21B3C and the stash record 0x51CB0 (scripted success, counted).
Each case fuzzes the 8 team mercs (in sector, health incl. negative, profile, slot byte, items), the AIM profiles'
+0x2F / +0x3B / +0x3C / +0x4D, the balance, the day net and the difficulty. Compared: the ordered call list and the
whole data object afterwards. Exits 1 on any mismatch.
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
SOL = ai.G_SOLDIERS

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 2000
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.PAY = True              # accounts (0xA8), profiles, stash counts
COVER = __import__("collections").Counter()


class Case:
    """the scripted random results, dialog answers and stash results of one case, replayed once per side"""
    def __init__(self, rolls, answer, puts):
        self.rolls0, self.answer, self.puts0 = rolls, answer, puts
        self.reset()

    def reset(self):
        self.rolls = list(self.rolls0)
        self.puts = list(self.puts0)
        self.k = 0
        self.log = []

    def random(self, n):
        self.log.append(("random", ai.s16(n & 0xFFFF)))
        if not self.rolls:               # out of rolls: count up (a redraw loop then ends)
            self.k += 1
            return self.k % n if n > 0 else 0
        r = self.rolls.pop(0)
        return r % n if ai.s16(n & 0xFFFF) > 0 else 0

    def put(self):
        return self.puts.pop(0) if self.puts else 1


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    case = [None]
    for k in range(8):
        assert e.r32(rt(mo.TEAM_PTRS + 4 * k)) == rt(SOL + 0x200 * k), "pointer table"

    def stub(name, nargs, stack=0):
        def f(emu, eax, edx, ebx, ecx):
            sp = emu.uc.reg_read(UC_X86_REG_ESP)
            a = (eax, edx, ebx, ecx)[:nargs] + tuple(emu.r32(sp + 4 + 4 * j) for j in range(stack))
            case[0].log.append((name,) + tuple(ai._i32(x) for x in a))
        return f

    def orig_confirm(emu, eax, edx, ebx, ecx):
        case[0].log.append(("confirm", ebx & 0xFFFFFFFF))
        return case[0].answer

    def orig_put(emu, eax, edx, ebx, ecx):
        case[0].log.append(("stash_put", eax & 0xFF, edx & 0xFF, ebx & 0xFF))
        v = case[0].put()
        if v:
            a = rt(ai.G_STASH_COUNTS) + (eax & 0xFF)
            emu.w8(a, emu.r8(a) + 1)
        return v

    def orig_36010(emu, eax, edx, ebx, ecx):
        sp = emu.uc.reg_read(UC_X86_REG_ESP)
        case[0].log.append(("w_36010", ecx & 0xFF, emu.r32(sp + 4)))

    e.stub(rt(0x53538), lambda emu, eax, *_: case[0].random(ai.s16(eax & 0xFFFF)))
    e.stub(rt(0x5E8E0), lambda emu, eax, *_: case[0].log.append(("w_5e8e0", (eax & 0xFFFFFFFF) - rt(0x90AA0) + 0x90AA0)))
    e.stub(rt(0x275E4), orig_confirm, 12)

    def orig_replace(emu, eax, edx, ebx, ecx):     # StrReplace 0x252B0 (its strstr_ faults in Unicorn): in Python
        src, rep, pat = emu.cstr(eax & 0xFFFFFFFF), emu.cstr(ebx & 0xFFFFFFFF), emu.cstr(ecx & 0xFFFFFFFF)
        case[0].log.append(("str_replace", rep))
        emu.uc.mem_write(edx & 0xFFFFFFFF, src.replace(pat, rep, 1) + b"\0")
    e.stub(rt(0x252B0), orig_replace)
    e.stub(rt(0x28698), stub("w_28698", 1))
    e.stub(rt(0x36010), orig_36010, 4)
    e.stub(rt(0x67BF0), stub("w_67bf0", 0))
    e.stub(rt(0x52AE0), stub("blit", 4, 2), 8)
    e.stub(rt(0x673AC), stub("w_673ac", 0))
    e.stub(rt(0x4420C), stub("bass_map", 0))
    e.stub(rt(0x4508C), stub("w_4508c", 0))
    e.stub(rt(0x46AC4), lambda emu, eax, *_: case[0].log.append(("face_clear", eax & 0xFF)))
    e.stub(rt(0x21B3C), stub("w_21b3c", 0))
    e.stub(rt(0x51CB0), orig_put)

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

        def ext(self, name, *a):
            c = case[0]
            if name == "random":
                return c.random(a[0])
            if name == "confirm":
                c.log.append(("confirm", rt(a[0])))
                return c.answer
            if name == "stash_put":
                c.log.append(("stash_put",) + tuple(x & 0xFF for x in a))
                return c.put()
            if name == "face_clear":
                c.log.append(("face_clear", a[0] & 0xFF))
                return 0
            c.log.append((name,) + tuple(ai._i32(x) if isinstance(x, int) else x for x in a))
            return 0

    w = World()
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        c = Case([rng.randrange(1 << 16) for _ in range(rng.choice([2, 10, 60]))], rng.choice([0, 1, 1, 2]),
                 [rng.choice([1, 1, 1, 0]) for _ in range(20)])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        try:
            e.call(rt(0x46328), max_steps=5_000_000)
        except Exception:
            print("crash; log tail", c.log[-6:])
            raise
        log_o = c.log
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        case[0] = c
        if PORT:
            port_run(e, w, case)
        else:
            mo.corpse_export(w)
        log_m = c.log
        COVER.update({x[0] if x[0] != "confirm" else f"confirm {x[1] - rt(0x90000) + 0x90000:X}" for x in log_o})
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


def port_run(e, w, case):
    """ja1_corpse_export in the library; its host ops give the original's logged calls"""
    rt = e.rt
    bufs, scalars = port_lib.world_load(e, ai)
    texts = []

    def call(op, a):
        a = [ai._i32(x) for x in a]
        c = case[0]
        lg = c.log
        if op == "corpse_ask":
            cost, can_pay = a[0], a[1]
            lg += [("w_5e8e0", 0x90AA0), ("str_replace", b"%d" % cost), ("confirm", rt(0x944A6 if can_pay else 0x944BE))]
            texts.append(b"Dish out $%d for corpse export?\0" % cost)
            return c.answer, None
        if op == "corpse_paid":
            lg += [("w_28698", 0x25), ("w_36010", 0x2B, 0x33), ("w_67bf0",),
                   ("blit", ai._i32(w.g32(0x95678) + 0xB680), ai._i32(w.g32(0x93E60) + 0xB680), 0x140, 0x36, 0x140, 0x140),
                   ("w_673ac",)]
        elif op == "corpse_end":
            lg += [("bass_map",), ("w_36010", 0x2B, 0x33), ("w_4508c",)]
        elif op == "face_clear":
            lg.append(("face_clear", a[0] & 0xFF))
        elif op == "stash_put":
            lg.append(("stash_put",) + tuple(x & 0xFF for x in a[:3]))
            return c.put(), None
        elif op == "face_move":
            to, frm = a[0] & 0xFF, a[1] & 0xFF
            lg.append(("blit", ai._i32(w.g32(0x93428 + 4 * to)), ai._i32(w.g32(0x93428 + 4 * frm)), 0x20, 0x20, 0x20, 0x20))
        elif op == "team_redraw":
            lg.append(("w_21b3c",))
        else:
            raise SystemExit(f"corpse_diff --port: unmapped op {op} {a}")
        return 0, None

    port_lib.sector_run("corpse_export", bufs, scalars, [], lambda n_: case[0].random(n_), call)
    port_lib.world_store(e, ai, bufs, scalars)
    for t in texts:
        for j, b in enumerate(t):
            w.s8(mo.TEXT_OUT + j, b)


def fuzz(w, rng):
    w.s32(mo.BALANCE, rng.choice([0, 599, 600, 800, 1000, 999, 5000, -100, rng.randrange(-2000, 30000)]))
    w.s32(mo.DAY_NET, rng.choice([0, 100, -500, rng.randrange(-5000, 5000)]))
    w.s8(mo.DIFFICULTY, rng.choice([0, 1, 2]))
    listed = [w.g8(ai.G_LISTED_ITEMS + j) for j in range(32)]
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + mo.IN_SECTOR, rng.choice([1, 1, 1, 1, 0]))
        w.s8(s + mo.HP, rng.choice([50, 100, 1, 0, 0, -5 & 0xFF, 0x80, rng.randrange(256)]))
        w.s8(s + mo.PID, rng.choice([rng.randrange(60), rng.randrange(60), 59, 60, 61, rng.randrange(60, 64)]))
        w.s8(s + mo.SLOT, rng.choice([k, k, k, rng.randrange(8)]))
        for j in range(12):
            r = s + 12 * j
            t = rng.choice([0, 0, rng.randrange(1, 136), rng.choice(listed)])
            w.s8(r, t)
            w.s8(r + 1, rng.choice([1, 1, 2, 3]))
            w.s8(r + 3, rng.choice([0, 0, 0, rng.choice(listed), rng.randrange(1, 136)]))
    for p in range(60):
        a = mo.PROFILES + mo.PROFILE_SIZE * p
        w.s8(a + 0x2F, rng.choice([0, 1, 3, rng.randrange(256)]))
        for o in (0x3B, 0x4D):
            w.s8(a + o, rng.choice([99, 99, 99, rng.randrange(64)]))
        w.s8(a + 0x3C, rng.choice([99, 99, rng.randrange(64)]))


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
    print(f"corpse_diff{' --port' if PORT else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
