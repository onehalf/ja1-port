"""Differential test: tools/xpchange.py (the morning stat changes) vs ExperienceChangeMsg? 0x45BBC in Unicorn.

usage: .venv/bin/python validate/diff/xp_diff.py [N=1000] [DUMP ...] [--seed S] [-v] [--port] [--fan IDS]
--fan IDS (with --port; #70): the original side runs the fan patch 1.15m code (Emu.put_115m), the port runs with
the fan fixes IDS on (`all` or `F03`); the plain --port run checks the fixes off against 1.13b.
--port: the clean-room library (ja1_stat_changes, port_lib) instead of the mirror; its host ops 282..287
(port_api.md) are mapped to the original's logged calls.
First the salary step 0x62DA8 (x 1.5 / x 2/3 on the x87, then rounded) for 3000 salaries each way, then N cases:
the team (members in the sector, hp, hpMax, healed, stats, level, duty, salary, effective salary, profile) and the
practice progress rows are fuzzed. Stubbed on the original side and logged: TimerStart / TimerElapsedMs, the hpbar
0x47144, StatusMessage (the text), 0x67478 / 0x682AC; the counting display 0x45624 runs for real (its entry is
logged) with its drawing calls (BlitCopy, DrawFrame, DrawNumber, 0x46C1C, 0x67250, 0x5C880, 0x67EEC) and the key
check 0x67300 as no-ops. Compared: the ordered call list and the whole data object. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import xpchange as xp                # noqa: E402
from emu import Emu                  # noqa: E402

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
SOL = ai.G_SOLDIERS

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 1000
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    port_lib.STATS = True            # stat_progress, stat_changed
FAN = None
if "--fan" in sys.argv:
    v = sys.argv[sys.argv.index("--fan") + 1]
    args = [a for a in args if a != v]
    dumps = [a for a in dumps if a != v] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip()[1:]) - 1) for x in v.split(","))
    if not PORT:
        raise SystemExit("xp_diff: --fan needs --port")


def salary_test(e, rng):
    rt = e.rt
    bad = 0
    sals = list(range(0, 1200)) + [rng.randrange(65536) for _ in range(1800)]
    for up in (0, 1):
        for sal in sals:
            o = e.call(rt(0x62DA8), sal, up) & 0xFFFF
            m = xp.salary_step(sal, up)
            if o != m:
                bad += 1
                if bad <= 5:
                    print(f"MISMATCH salary_step({sal}, {up}): original {o}, mirror {m}")
    return bad


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    if FAN is not None:
        e.put_115m()
    port_lib.fan_fixes(FAN or 0) if PORT else None
    rt = e.rt
    dd = rt(0x90000) - 0x90000
    log = []

    def nop(*_):
        return 0

    for a, pop in ((0x52AE0, 8), (0x46C44, 12), (0x36090, 24), (0x46C1C, 0), (0x67250, 0), (0x5C880, 0),
                   (0x67EEC, 0), (0x67300, 0)):
        e.stub(rt(a), nop, pop)
    e.stub(rt(0x47020), lambda *_: log.append(("stat_begin",)))
    e.stub(rt(0x47040), lambda *_: log.append(("stat_end",)) or 0x7FFFFFFF)
    e.stub(rt(0x47144), lambda em, eax, edx, ebx, ecx: log.append(
        ("hpbar", ((eax & 0xFFFFFFFF) - dd - SOL) / 0x200, edx, ebx, ecx)), 4)    # 5th argument (stack) = 1
    e.stub(rt(0x27D24), lambda em, eax, *_: log.append(("msg", em.cstr(eax))))
    e.stub(rt(0x67478), lambda em, eax, edx, ebx, ecx: log.append(("w_67478", eax, ebx, ecx)))
    e.stub(rt(0x682AC), lambda em, eax, edx, ebx, ecx: log.append(("w_682ac", eax, edx, ebx, ecx) if PORT else ("w_682ac", eax, edx)))
    e.stub_sprintf()

    def count_real(rng, n):
        """0x45624 for real (drawing as no-ops) vs xpchange.count: the final store, deltas of at most 300"""
        bad = 0
        for _ in range(n):
            k, stat, d = rng.randrange(8), rng.randrange(10), rng.randrange(-300, 301)
            s = SOL + 0x200 * k
            w.s8(s + xp.SLOT, k)
            for o in xp.FIELDS.values():
                w.s8(s + o[0], rng.randrange(256))
            w.s16(s + xp.SALARY2, rng.randrange(65536))
            pre = bytes(e.uc.mem_read(rt(s), 0x200))
            flag = rt(0x22A404 + 0x100)
            e.call(rt(0x45624), k, stat, rt(s + (xp.SALARY2 if stat == 0 else xp.FIELDS[stat][0])), d & 0xFFFF,
                   flag, max_steps=5_000_000)
            post_o = bytes(e.uc.mem_read(rt(s), 0x200))
            e.uc.mem_write(rt(s), pre)
            log.clear()
            xp.count(w, k, stat, d)
            if post_o != bytes(e.uc.mem_read(rt(s), 0x200)):
                bad += 1
                if bad <= 3:
                    print(f"MISMATCH count({k}, {stat}, {d})")
        return bad

    def stub_count(em, eax, edx, ebx, ecx):     # 0x45624 after count_real: the mirror's store
        log.append(("count", eax & 0xFF, edx & 0xFF, ebx - dd, ai.s16(ecx & 0xFFFF)))
        k = (ebx - dd - SOL) // 0x200
        log_len = len(log)
        xp.count(w, k, edx & 0xFF, ai.s16(ecx & 0xFFFF))
        del log[log_len:]

    class World:
        def g8(self, a): return e.r8(rt(a))
        def g16(self, a): return e.r16(rt(a))
        def g32(self, a): return e.r32(rt(a))
        def s8(self, a, v): e.w8(rt(a), v & 0xFF)
        def s16(self, a, v): e.w16(rt(a), v & 0xFFFF)
        def s32(self, a, v): e.w32(rt(a), v & 0xFFFFFFFF)

        def ext(self, name, *a):
            if name == "hpbar":
                log.append(("hpbar", a[0], a[1], 0x14, 1))
            else:
                log.append((name,) + a)
            return 0

    w = World()

    def port_run():
        bufs, scalars = port_lib.world_load(e, ai)
        sol = bufs["soldiers"]

        def cstr(a):
            return xp.cstr(w, a)

        def call(op, a):
            a = [ai._i32(x) for x in a]
            if op == "xp_stat_begin":
                log.append(("stat_begin",))
            elif op == "xp_stat_end":
                log.append(("stat_end",))
            elif op == "xp_count":
                k, stat, v, d = a
                o = xp.SALARY2 if stat == 0 else xp.FIELDS[stat][0]
                b = 0x200 * k + o
                want = int.from_bytes(sol[b:b + 2], "little") if stat == 0 else ai.s8(sol[b])
                if v != want:
                    log.append(("bad value", k, stat, v, want))
                log.append(("count", sol[0x200 * k + xp.SLOT], stat, SOL + b, ai.s16(d & 0xFFFF)))
            elif op == "xp_health_bar":
                log.append(("hpbar", a[0], sol[0x200 * a[0] + xp.SLOT] * 0x25 + 0x2C, 0x14, 1))
            elif op == "xp_salary_msg":
                k, up, lv, sal = a
                nick = bytes(sol[0x200 * k + xp.NICK:0x200 * k + xp.NICK + 10]).split(b"\0")[0]
                log.append(("msg", nick + cstr(xp.S_RISEN if up else xp.S_FALLEN) + b" %d!  " % lv
                            + cstr(xp.S_AIM_UP if up else xp.S_AIM_DOWN) + b" $%d " % sal + cstr(xp.S_TAIL)))
            elif op == "xp_end":
                log.append(("w_67478", 0, 0x13F, 199))
                log.append(("w_682ac", *port_lib.mouse_op_limit(287)))
            else:
                raise SystemExit(f"xp_diff --port: unmapped op {op} {a}")
            return 0, None

        def rnd(n_):
            raise SystemExit("xp_diff --port: the library asked for a random number")
        port_lib.sector_run("stat_changes", bufs, scalars, [], rnd, call, sync=True)
        port_lib.world_store(e, ai, bufs, scalars)

    fails = salary_test(e, rng) + count_real(rng, 1500)
    e.stub(rt(0x45624), stub_count, 4)
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        log.clear()
        try:
            e.call(rt(0x45BBC), max_steps=20_000_000)
        except Exception:
            print("crash; log tail", log[-4:])
            raise
        log_o = list(log)
        post_o = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        log.clear()
        if PORT:
            port_run()
        else:
            xp.stat_changes(w)
        log_m = list(log)
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


def fuzz(w, rng):
    team = rng.choice([0, 1, 3, 5, 8, 8, rng.randrange(9)])
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + xp.IN_SECTOR, 1 if k < team else rng.choice([0, 0, 1]) if k > team else 0)
        hpm = rng.choice([100, 80, 45, 15, 99, rng.randrange(101)])
        hp = rng.choice([hpm, hpm - 1, hpm // 2, 15, 14, 0, 1, rng.randrange(hpm + 1)])
        hp = min(max(hp, 0), hpm)
        w.s8(s + xp.HP_MAX, hpm)
        w.s8(s + xp.HP, hp)
        w.s8(s + xp.HEALED, rng.choice([0, 0, rng.randrange(hpm - hp + 1)]))
        for o in (0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191):
            w.s8(s + o, rng.choice([0, 1, 30, 50, 99, 100, rng.randrange(101)]))
        lv = rng.choice([1, 2, 5, 9, 10, rng.randrange(1, 11)])
        w.s8(s + 0x192, lv)
        w.s8(s + xp.SLOT, k)
        w.s8(s + xp.PID, rng.randrange(64))
        w.s8(s + xp.DUTY, rng.choice([0, 1, 2, 3, 4, 5, 7]))
        sal = rng.choice([0, 5, 100, 499, 500, 1000, 1500, 2500, 5000, 10000, 30000, rng.randrange(65536)])
        w.s16(s + xp.SALARY, sal)
        w.s16(s + xp.SALARY2, rng.choice([sal, sal // 2, (sal + 1) // 2, rng.randrange(65536)]))
        row = xp.PROG + 18 * k
        for j in range(8):
            w.s16(row + 2 * j, rng.choice([0, 0, 99, 100, 150, -50, -150, 9999, rng.randrange(-3000, 3000),
                                           rng.randrange(-32768, 32768)]))
        lim = 1000 * lv * (lv + 1) // 2            # at or below -lim the first loop runs until p wraps (16-bit)
        w.s16(row + 16, rng.choice([0, 999, 1000, lv * 1000, -lv * 1000, -lv * 1000 + 1, rng.randrange(32768),
                                    -rng.randrange(min(lim, 32768)), -min(lim, 32768), -min(lim + 1, 32768), -32768,
                                    -rng.randrange(32769)]))


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
    print(f"xp_diff{' --port' if PORT else ''}{f' --fan {FAN:#x}' if FAN is not None else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
