"""Differential test: tools/morning.py raise_salary (the morning salary raise questions) vs the original.
usage: .venv/bin/python validate/diff/raise_diff.py [N=2000] [DUMP ...] [--seed S] [-v] [--port]
--port: the clean-room library (ja1_raise_salary, port_lib) instead of the mirror (not written yet).
RaiseSalaryPrompt? 0x448A8 runs with its parts for real (RaiseReason? 0x44634, AimRate 0x1FDEC, the name copy
0x35810, MercLeaves 0x50618, team compaction 0x50750, AddStashItem 0x51D50) and SalaryDialog? 0x29074 for real with
its drawing stubbed (not logged) and its number entry 0x2573C scripted. Stubbed and logged on both sides: PlayQuote
0x24F08, PlayMercVoice 0x24D14, StrReplace 0x252B0 (done in Python), 0x67BF0, 0x467E4, 0x673AC, the text box
0x50908, 0x28698, 0x36010, DrawBassMap? 0x4420C, 0x4508C, the face clear 0x46AC4, 0x21B3C, BlitCopy 0x52AE0 and the
stash record 0x51CB0 (scripted success, counted).
Each case fuzzes the 8 team mercs (in sector, health, profile, salaries, +0x19C / +0x1D9, items), the AIM profiles'
answer bytes (+0x30 / +0x31 / +0x32), limits (+0x36..+0x39, +0x4D), the AIM statistics and the offers. Compared:
the ordered call list and the whole data object afterwards. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import morning as mo                 # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
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
    port_lib.PAY = True
COVER = __import__("collections").Counter()
DIALOG_UI = {0x678E8: 0, 0x358D0: 24, 0x67E18: 0, 0x46B1C: 0, 0x681D0: 0, 0x532BC: 4, 0x3625C: 32, 0x35B60: 4,
             0x2560C: 0, 0x469F0: 4, 0x25414: 0, 0x67C54: 0, 0x46EC4: 0, 0x67E30: 0}


class Case:
    """the scripted offers and stash results of one case, replayed once per side"""
    def __init__(self, offers, puts):
        self.offers0, self.puts0 = offers, puts
        self.reset()

    def reset(self):
        self.offers = list(self.offers0)
        self.puts = list(self.puts0)
        self.log = []

    def offer(self, start):
        self.log.append(("offer", start))
        return self.offers.pop(0) & 0xFFFF if self.offers else 0

    def put(self):
        return self.puts.pop(0) if self.puts else 1


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    case = [None]
    dialog = [False]
    assert e.r8(rt(0x90A60)) == 0

    def stub(name, nargs, stack=0):
        def f(emu, eax, edx, ebx, ecx):
            if dialog[0]:
                return
            sp = emu.uc.reg_read(UC_X86_REG_ESP)
            a = (eax, edx, ebx, ecx)[:nargs] + tuple(emu.r32(sp + 4 + 4 * j) for j in range(stack))
            case[0].log.append((name,) + tuple(ai._i32(x) for x in a))
        return f

    def slot_of(p):
        a = (p & 0xFFFFFFFF) - rt(SOL)
        return a // 0x200 if a % 0x200 == 0 else ("ptr", a)

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

    def orig_replace(emu, eax, edx, ebx, ecx):     # StrReplace 0x252B0 (its strstr_ faults in Unicorn): in Python
        src, rep, pat = emu.cstr(eax & 0xFFFFFFFF), emu.cstr(ebx & 0xFFFFFFFF), emu.cstr(ecx & 0xFFFFFFFF)
        case[0].log.append(("str_replace", rep))
        emu.uc.mem_write(edx & 0xFFFFFFFF, src.replace(pat, rep, 1) + b"\0")

    def orig_offer(emu, eax, *_):
        dialog[0] = False
        return case[0].offer(ai.s16(eax & 0xFFFF))

    def dialog_in(uc, addr, size, user):
        dialog[0] = True

    e.uc.hook_add(UC_HOOK_CODE, dialog_in, None, rt(0x29074), rt(0x29074))
    for a, pop in DIALOG_UI.items():
        e.stub(rt(a), lambda *_: 0, pop)
    e.stub(rt(0x2573C), orig_offer)
    e.stub(rt(0x24F08), lambda emu, eax, edx, ebx, ecx: case[0].log.append(("quote", eax & 0xFF, ai._i32(ebx))), 16)
    e.stub(rt(0x24D14), lambda emu, eax, edx, ebx, ecx: case[0].log.append(("voice", eax & 0xFF, ai._i32(edx))), 4)
    e.stub(rt(0x252B0), orig_replace)
    e.stub(rt(0x67BF0), stub("w_67bf0", 0))
    e.stub(rt(0x467E4), stub("w_467e4", 3))
    e.stub(rt(0x673AC), stub("w_673ac", 0))
    e.stub(rt(0x50908), lambda emu, eax, edx, ebx, ecx: case[0].log.append(("text_box", slot_of(eax), ai._i32(edx),
                                                                          ai._i32(ebx), ai._i32(ecx))))
    e.stub(rt(0x28698), stub("w_28698", 1))
    e.stub(rt(0x36010), orig_36010, 4)
    e.stub(rt(0x52AE0), stub("blit", 4, 2), 8)
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
            if name == "offer":
                return c.offer(a[0])
            if name == "stash_put":
                c.log.append(("stash_put",) + tuple(x & 0xFF for x in a))
                return c.put()
            if name == "face_clear":
                c.log.append(("face_clear", a[0] & 0xFF))
                return 0
            c.log.append((name,) + tuple(ai._i32(x) if isinstance(x, int) else x for x in a))
            return 0

    w = World()
    w.s32(0x229D18, 0)
    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        fuzz(w, rng)
        offers = []
        for _ in range(rng.choice([1, 3, 8])):
            s = rng.randrange(8)
            sal = w.g16(SOL + 0x200 * s + mo.SALARY)
            offers.append(rng.choice([0, -3, sal, sal + 1, sal * 105 // 100, sal * 11 // 10, sal * 3 // 2, sal * 2,
                                      sal // 2, 1, rng.randrange(1, 40000)]))
        c = Case(offers, [rng.choice([1, 1, 1, 0]) for _ in range(20)])
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        c.reset()
        case[0] = c
        dialog[0] = False
        try:
            e.call(rt(0x448A8), max_steps=5_000_000)
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
            mo.raise_salary(w)
        log_m = c.log
        COVER.update({x[0] if x[0] != "quote" else f"quote {x[2]}" for x in log_o})
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
    """ja1_raise_salary in the library; its host ops give the original's logged calls and memory writes"""
    bufs, scalars = port_lib.world_load(e, ai)
    sol, prof = bufs["soldiers"], bufs["profiles"]
    late = []                        # host memory writes, done after world_store

    def cstr(buf, a):
        out = bytearray()
        while buf[a + len(out)]:
            out.append(buf[a + len(out)])
        return bytes(out)

    def text_box(lg, k):
        lg.append(("text_box", k, sol[0x200 * k + mo.SLOT] * 0x25 + 0xB, 0x14, 5))

    def call(op, a):
        a = [ai._i32(x) for x in a]
        c = case[0]
        lg = c.log
        if op in ("raise_voice", "raise_ask", "raise_offer", "raise_answer"):
            k = a[0]
            pid = sol[0x200 * k + mo.PID]
        if op == "raise_voice":
            lg.append(("voice", pid, a[1]))
            late.append((mo.GRUDGE_SHOWN, bytes([a[1] & 0xFF])))
        elif op == "raise_ask":
            late.append((mo.QUOTE_WAIT, b"\1"))
            lg.append(("quote", pid, a[1]))
            if a[1] != 5 and a[2] != -1:
                lg.append(("bad_grudge_arg", a[2]))
            name = cstr(prof, mo.PROFILE_SIZE * a[2]) if a[1] == 5 else b""
            rep = bytes(x + 1 & 0xFF for x in name)
            late.append((mo.TEXT_BUF, rep + b"\0"))
            lg.append(("str_replace", rep))
            txt = mo.cstr(w, mo.QUOTE_TEXT).replace(mo.cstr(w, 0x90B24), rep, 1) + b"\0"
            late.extend([(mo.TEXT_OUT, txt), (mo.QUOTE_TEXT, txt)])
            lg += [("w_67bf0",), ("w_467e4", 0, 0x34, 0x25), ("w_673ac",)]
            text_box(lg, k)
            late.append((mo.OFFER, int.from_bytes(sol[0x200 * k + mo.SALARY:0x200 * k + mo.SALARY + 2], "little")
                         .to_bytes(2, "little")))
        elif op == "raise_offer":
            v = c.offer(ai.s16(a[1] & 0xFFFF))
            late.append((mo.OFFER, v.to_bytes(2, "little")))
            return ai.s16(v), None
        elif op == "raise_answer":
            late.append((mo.QUOTE_WAIT, b"\1"))
            lg += [("quote", pid, 14 if a[1] < 2 else 13), ("w_67bf0",)]
            if not w.g8(mo.NO_WAIT):
                lg.append(("w_28698", 0x25))
            text_box(lg, k)
        elif op == "raise_done":
            lg.append(("w_28698", 0x25))
        elif op == "raise_redraw":
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
            raise SystemExit(f"raise_diff --port: unmapped op {op} {a}")
        return 0, None

    def rnd(n_):
        raise SystemExit("raise_diff --port: the library asked for a random number")
    port_lib.sector_run("raise_salary", bufs, scalars, [], rnd, call)
    port_lib.world_store(e, ai, bufs, scalars)
    for a, b in late:
        for j, x in enumerate(b):
            w.s8(a + j, x)


def fuzz(w, rng):
    for a, vals in ((mo.DEATHS, [0, 0, 1, 3, 9]), (mo.DAY, [0, 1, 5, 10, 20, 60, 255]), (mo.HIRINGS, [0, 0, 2, 8, 30]),
                    (mo.RESIGNATIONS, [0, 1, 4]), (mo.TERMINATIONS, [0, 1, 3]), (mo.NON_PAYMENTS, [0, 0, 2, 6]),
                    (mo.BAD_DAYS, [0, 1, 3, 8, 12]), (mo.NO_WAIT, [0, 0, 1])):
        w.s8(a, rng.choice(vals + [rng.randrange(256)]))
    listed = [w.g8(ai.G_LISTED_ITEMS + j) for j in range(32)]
    for k in range(8):
        s = SOL + 0x200 * k
        w.s8(s + mo.IN_SECTOR, rng.choice([1, 1, 1, 1, 1, 0]))
        w.s8(s + mo.HP, rng.choice([50, 100, 1, 0, -5 & 0xFF, rng.randrange(256)]))
        w.s8(s + mo.PID, rng.choice([rng.randrange(60), rng.randrange(60), 59, 60, rng.randrange(60, 100)]))
        w.s8(s + mo.SLOT, k)
        w.s16(s + mo.SALARY, rng.choice([100, 250, 1000, 3, rng.randrange(1, 5000), rng.randrange(1, 65536)]))
        w.s16(s + mo.SALARY2, rng.choice([0, 100, 250, rng.randrange(65536)]))
        w.s8(s + mo.B19C, rng.choice([0, 8, 8, 3, rng.randrange(256)]))
        w.s8(s + 0x1D9, rng.choice([0, 0, 1, rng.randrange(256)]))
        w.s8(s + 0x18D, rng.randrange(256))
        for j in range(12):
            r = s + 12 * j
            t = rng.choice([0, 0, rng.randrange(1, 136), rng.choice(listed)])
            w.s8(r, t)
            w.s8(r + 1, rng.choice([1, 1, 2, 3]))
            w.s8(r + 3, rng.choice([0, 0, 0, rng.choice(listed), rng.randrange(1, 136)]))
    s8 = SOL + 0x200 * 8                   # record 8 (not a team slot) ends the loop: make it look like a merc
    w.s8(s8 + mo.IN_SECTOR, rng.choice([0, 1]))
    w.s8(s8 + mo.HP, rng.choice([0, 50]))
    w.s8(s8 + mo.PID, rng.randrange(60))
    for p in range(60):
        a = mo.PROFILES + mo.PROFILE_SIZE * p
        w.s8(a + 0x30, rng.choice([0, 1, 2, 5, 10, 25, rng.randrange(256)]))
        w.s8(a + 0x31, rng.choice([0, 1, 3, 10, rng.randrange(256)]))
        w.s8(a + 0x32, rng.choice([0, 1, 2, 3, rng.randrange(256)]))
        for o in (0x36, 0x37, 0x39):
            w.s8(a + o, rng.choice([0, 1, 5, 30, 101, 0x65, rng.randrange(256)]))
        w.s8(a + 0x38, rng.choice([0, 3, 10, 99, rng.randrange(256)]))
        w.s8(a + 0x4D, rng.choice([99, 99, 60, rng.randrange(60), rng.randrange(256)]))


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
    print(f"raise_diff{' --port' if PORT else ''}: {total} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
