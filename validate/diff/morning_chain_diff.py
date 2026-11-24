"""Differential test: the whole morning (strategic.md "The morning") of the original vs the clean-room library.

usage: .venv/bin/python validate/diff/morning_chain_diff.py [N=300] [DUMP ...] [--seed S] [-v]
The original side calls the 12 morning functions in the order of DayEndScreen? 0x65CE2..0x65D19 on a fuzzed world;
the port side runs ja1_morning (port_lib) on the same world. Only the world buffers and scalars are compared
(the drawing, the quotes and the texts are the host's job and are stubbed or ignored on both sides). Both sides get
the same scripted answers: random results, item store results, the corpse export answer, the salary offers.
StatPractice 0x62A68 runs for real on both sides (the port's host op 281 calls it on the port's world) and so does
StatCountUp 0x45624 (its timers stubbed). The balance stays high, so the Not enough funds dialog does not open
(payday_diff tests it).
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path.insert(0, os.path.join(ROOT, "tools"))
sys.path.insert(0, HERE)
import ai                            # noqa: E402
import morning as mo                 # noqa: E402
import duties as du                  # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

port_lib.PAY = port_lib.STATS = port_lib.MORNING = True
DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
SOL = ai.G_SOLDIERS
CHAIN = (0x44380, 0x44740, 0x445DC, 0x4B48C, 0x45100, 0x45BBC, 0x42378, 0x46328, 0x448A8, 0x44C10, 0x45010, 0x65B40)
NOP = {0x44380: 0, 0x24F08: 16, 0x24D14: 4, 0x67BF0: 0, 0x467E4: 0, 0x673AC: 0, 0x50908: 0, 0x28698: 0, 0x36010: 4,
       0x52AE0: 8, 0x4420C: 0, 0x4508C: 0, 0x46AC4: 0, 0x21B3C: 0, 0x5E8E0: 0, 0x44D28: 4, 0x35810: 0, 0x46BF4: 0,
       0x46C44: 12, 0x36090: 24, 0x46C1C: 0, 0x67250: 0, 0x5C880: 0, 0x67EEC: 0, 0x67300: 0, 0x47020: 0,
       0x47144: 4, 0x27D24: 0, 0x67478: 0, 0x682AC: 0, 0x674B0: 0, 0x23420: 0, 0x23264: 20,
       # salary dialog drawing (raise_diff DIALOG_UI)
       0x678E8: 0, 0x358D0: 24, 0x67E18: 0, 0x46B1C: 0, 0x681D0: 0, 0x532BC: 4, 0x3625C: 32, 0x35B60: 4,
       0x2560C: 0, 0x469F0: 4, 0x25414: 0, 0x67C54: 0, 0x46EC4: 0, 0x67E30: 0}
QUIET = {"payroll_show", "face_clear", "face_move", "team_redraw", "runs_off", "island_map", "runs_off_end",
         "xp_stat_begin", "xp_count", "xp_health_bar", "xp_salary_msg", "xp_stat_end", "xp_end", "cp_begin", "cp_quote",
         "cp_end", "hate_quote", "corpse_paid", "corpse_end", "raise_voice", "raise_ask", "raise_answer", "raise_done",
         "raise_redraw", "pd_end", "pd_face_frame", "pd_panel_show", "pd_message", "pd_clear", "pd_blink",
         "pd_timer_start", "mouse_off", "mouse_show", "tip_merc", "tip_item"}

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 300
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
COVER = __import__("collections").Counter()


class Case:
    """the scripted answers of one case, replayed once per side"""
    def __init__(self, rng):
        self.r0 = [rng.randrange(1 << 30) for _ in range(400)]
        self.puts0 = [rng.choice([1, 1, 1, 0]) for _ in range(60)]
        self.offers0 = [rng.choice([0, 1, 100, 300, 1000, 2000, 5000, rng.randrange(1, 40000)]) for _ in range(40)]
        self.answers0 = [rng.choice([0, 1]) for _ in range(4)]
        self.reset()

    def reset(self):
        self.r, self.puts, self.offers, self.answers = list(self.r0), list(self.puts0), list(self.offers0), \
            list(self.answers0)
        self.count = __import__("collections").Counter()

    def random(self, n):
        i = self.count["random"]
        self.count["random"] += 1
        v = (self.r0[i % len(self.r0)] + (i // len(self.r0)) * 2654435761) & 0x3FFFFFFF
        return v % n if n > 0 else 0

    def put(self):
        self.count["stash_put"] += 1
        return self.puts.pop(0) if self.puts else 1

    def offer(self):
        self.count["offer"] += 1
        return self.offers.pop(0) & 0xFFFF if self.offers else 0

    def answer(self):
        self.count["confirm"] += 1
        return self.answers.pop(0) if self.answers else 0


def fuzz(w, rng, repairable):
    w.s32(mo.BALANCE, rng.randrange(5_000_000, 50_000_000))
    w.s32(mo.DAY_NET, rng.randrange(-5000, 5000))
    w.s8(mo.DIFFICULTY, rng.choice([0, 1, 2]))
    for a, vals in ((mo.DEATHS, [0, 0, 1, 3, 9]), (mo.DAY, [0, 1, 5, 10, 20, 60]), (mo.HIRINGS, [0, 2, 8, 30]),
                    (mo.RESIGNATIONS, [0, 1, 4]), (mo.TERMINATIONS, [0, 1, 3]), (mo.NON_PAYMENTS, [0, 0, 2, 6]),
                    (mo.BAD_DAYS, [0, 1, 3, 8]), (mo.NO_WAIT, [0, 0, 1])):
        w.s8(a, rng.choice(vals + [rng.randrange(256)]))
    listed = [w.g8(ai.G_LISTED_ITEMS + j) for j in range(32)]
    for k in range(8):
        s = SOL + 0x200 * k
        hpm = rng.choice([100, 80, 45, 9, 99, rng.randrange(1, 101)])
        hp = min(max(rng.choice([hpm, hpm - 1, hpm // 2, 15, 14, 0, 0, 1, rng.randrange(hpm + 1)]), 0), hpm)
        w.s8(s + du.HP_MAX, hpm)
        w.s8(s + du.HP, hp)
        w.s8(s + du.HEALED, rng.choice([0, 0, 0, rng.randrange(hpm - hp + 1)]))
        w.s8(s + du.DUTY, rng.choice([0, 1, 2, 3, 3, 4, 5, 6, 7, 8]))
        w.s8(s + du.TRAIN, rng.choice([0, 1, 2, 3, 4]))
        w.s8(s + mo.SLOT, k)
        w.s8(s + mo.IN_SECTOR, rng.choice([1, 1, 1, 1, 0]))
        w.s8(s + mo.PID, rng.choice([rng.randrange(60), rng.randrange(60), 59, 60, rng.randrange(60, 64)]))
        w.s16(s + mo.SALARY, rng.choice([100, 250, 1000, rng.randrange(1, 5000)]))
        w.s16(s + mo.SALARY2, rng.choice([0, 100, 250, rng.randrange(5000)]))
        w.s8(s + 0x1D9, rng.choice([0, 0, 1]))
        w.s8(s + 0x19C, rng.choice([0, 0, 1, 1, 3, 7, 8]))
        for o in (du.AGI, du.DEX, du.WIS, du.MED, du.EXPL, du.MECH, du.MARKS):
            w.s8(s + o, rng.choice([0, 30, 50, 80, 99, rng.randrange(101)]))
        w.s8(s + du.LEVEL, rng.choice([1, 3, 10, rng.randrange(1, 11)]))
        for j in range(12):
            r = s + 12 * j
            kind = rng.random()
            if j == 0 and kind < 0.4:
                t = du.TOOLKIT
            elif kind < 0.3:
                t = rng.choice([du.FIRST_AID, du.MEDKIT, 0x6F])
            elif kind < 0.5 and repairable:
                t = rng.choice(repairable)
            else:
                t = rng.choice([0, 0, rng.randrange(1, 136), rng.choice(listed)])
            w.s8(r, t)
            w.s8(r + 1, rng.choice([1, 1, 2, 3]))
            for b in range(2, 12):
                w.s8(r + b, rng.choice([0, 1, 10, 50, 99, rng.randrange(101)]))
            w.s8(r + 3, rng.choice([0, 0, 0, rng.choice(listed), rng.randrange(1, 136)]))
    for k in range(8):                     # xp_diff's progress rows: 8 stats (i16), then the level progress
        row = ai.G_STAT_PROGRESS + 18 * k
        lv = w.g8(SOL + 0x200 * k + du.LEVEL)
        for j in range(8):
            w.s16(row + 2 * j, rng.choice([0, 0, 99, 100, 150, -50, -150, rng.randrange(-3000, 3000)]))
        lim = 1000 * lv * (lv + 1) // 2            # a lower level progress loops (nearly) forever in the original
        w.s16(row + 16, rng.choice([0, 999, 1000, lv * 1000, rng.randrange(32768), -rng.randrange(min(lim, 32768))]))
    for p in range(60):
        a = mo.PROFILES + mo.PROFILE_SIZE * p
        w.s8(a + 0x2F, rng.choice([0, 1, 3]))
        w.s8(a + 0x30, rng.choice([0, 1, 5, 10, 25]))
        w.s8(a + 0x31, rng.choice([0, 1, 3, 10]))
        w.s8(a + 0x32, rng.choice([0, 1, 2, 3]))
        for o in (0x36, 0x37, 0x39):
            w.s8(a + o, rng.choice([5, 30, 101, 101, rng.randrange(256)]))
        w.s8(a + 0x38, rng.choice([3, 10, 99]))
        for o in (0x3A, 0x3B, 0x4D):
            w.s8(a + o, rng.choice([99, 99, 99, rng.randrange(64)]))
        w.s8(a + 0x3C, rng.choice([99, 99, rng.randrange(64)]))


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    case = [None]
    for a, pop in NOP.items():
        e.stub(rt(a), lambda *_: 0, pop)
    e.stub(rt(0x47040), lambda *_: 0x7FFFFFFF)                     # TimerElapsedMs: the pause is always over
    e.stub(rt(0x53538), lambda emu, eax, *_: case[0].random(eax & 0xFFFF))
    e.stub(rt(0x275E4), lambda *_: case[0].answer(), 12)
    e.stub(rt(0x2573C), lambda *_: case[0].offer())
    e.stub_sprintf()

    def orig_put(emu, eax, *_):
        v = case[0].put()
        if v:
            a = rt(ai.G_STASH_COUNTS) + (eax & 0xFF)
            emu.w8(a, emu.r8(a) + 1)
        return v

    def orig_replace(emu, eax, edx, ebx, ecx):     # StrReplace 0x252B0 (its strstr_ faults in Unicorn): in Python
        src, rep, pat = emu.cstr(eax & 0xFFFFFFFF), emu.cstr(ebx & 0xFFFFFFFF), emu.cstr(ecx & 0xFFFFFFFF)
        emu.uc.mem_write(edx & 0xFFFFFFFF, src.replace(pat, rep, 1) + b"\0")
    e.stub(rt(0x51CB0), orig_put)
    e.stub(rt(0x252B0), orig_replace)

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

    w = World()
    w.s32(0x229D18, 0)
    repairable = [t for t in range(1, 136) if w.g8(du.ITEM_TBL + 12 * t + 9)]
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng, repairable)
        c = Case(rng)
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        case[0] = c
        for a in CHAIN:
            try:
                e.call(rt(a), max_steps=300_000_000 if a == 0x45BBC else 20_000_000)
            except Exception:
                print(f"original 0x{a:X} stopped (case {i}); rt(0x10000)=0x{rt(0x10000):X}")
                raise
        bufs_o, sc_o = port_lib.world_load(e, ai)
        cnt_o = dict(c.count)
        e.uc.mem_write(rt(DATA_LO), pre)
        c.reset()
        bufs_p, sc_p = port_lib.world_load(e, ai)
        unknown = []

        def call(op, a):
            a = [ai._i32(x) for x in a]
            if op == "practice":
                port_lib.bufs_store(e, ai, bufs_p)
                e.call(rt(0x62A68), rt(SOL) + 0x200 * a[0], a[1] & 0xFF, a[2] & 0xFFFF, a[3] & 0xFF,
                       max_steps=2_000_000)
                port_lib.bufs_load(e, ai, bufs_p)
            elif op == "stash_put":
                return c.put(), None
            elif op == "corpse_ask":
                return c.answer(), None
            elif op == "raise_offer":
                return ai.s16(c.offer()), None
            elif op not in QUIET:
                unknown.append((op, a))
            return 0, None

        r = port_lib.sector_run("morning", bufs_p, sc_p, [], lambda n_: c.random(n_ & 0xFFFF), call)
        cnt_p = dict(c.count)
        COVER.update(cnt_o)
        bad = []
        if unknown:
            bad.append(f"unmapped ops {unknown[:3]}")
        if r != 0:
            bad.append(f"result {r}")
        if cnt_o != cnt_p:
            bad.append(f"answers used: original {cnt_o}, port {cnt_p}")
        for k in bufs_o:
            if k in port_lib.READ_ONLY:
                continue
            # views of one byte in two fields: story +0..7 = accounts +0xA0..0xA7 (the library writes accounts),
            # accounts +0x0C..0x0F = the scalar team_on_duty (the library writes the scalar)
            skip = range(8) if k == "story" else range(0xC, 0x10) if k == "accounts" else ()
            diff = [j for j in range(len(bufs_o[k])) if bufs_o[k][j] != bufs_p[k][j] and j not in skip]
            if not diff:
                continue
            j = diff[0]
            bad.append(f"{k} +0x{j:X}: original {bufs_o[k][j]:02X}, port {bufs_p[k][j]:02X}")
        for k in sc_o:
            if sc_o[k] != sc_p[k]:
                bad.append(f"{k}: original {sc_o[k]}, port {sc_p[k]}")
        if bad:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH {os.path.basename(path)} case {i}: " + "; ".join(bad[:6]))
    return fails


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
    print(f"morning_chain_diff: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
