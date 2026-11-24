"""Differential test: the port's story-object find scene (ja1_story_find, port/ja1logic/src/storyfind.rs) vs the
original 0x6179C in Unicorn.

usage: .venv/bin/python validate/diff/storyfind_diff.py [N=1500] [DUMP ...] [--seed S] [-v] --port
Each case fuzzes the finder (story_finder), soldiers 0..7 (in sector, duty, hp, in play, pay, task), the selected
merc, story[0x14] / story[0x15], difficulty, the current sector, story tiles (t1 0xAA..0xB1) and the sector's story
slots. Every callee but Forget soldier 0x3A148, Vacate tile 0x2395C, MoveCode 0x2BE54 and First merc 0x2AC60 is a
stub that logs its call; the port runs with every op on the host. Compared: the call list (name + arguments) and
the whole data object. The front-end's lit face slot 0x284F0E is cleared by the harness on the port side as the
front-end does. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
from emu import Emu                  # noqa: E402
import port_lib                      # noqa: E402
port_lib.MORNING = True              # duty7_merc

DUMPS = ["tac60"]
DATA_LO, DATA_HI = 0x90000, 0x33A730
SOL = ai.G_SOLDIERS
FINDER, STORY, LIT, DUTY7 = 0x92DE3, 0x22A040, 0x284F0E, 0x92DDA
TILES, SECTORS, CUR = ai.G_TILEMAP, ai.G_SECTORS, ai.G_CUR_SECTOR
# stubs: address -> (name, stack bytes, argument rule); "s" = soldier pointer -> slot, "i" = int, "b" = byte,
# "x" = a pointer the port passes as 0
STUBS = {0x3F724: ("focus", 0, "bb"), 0x299B8: ("merc_message", 4, "sbbx+"), 0x1CB78: ("redraw", 0, ""),
         0x5F76C: ("brenda_say", 0, "i"), 0x67BF0: ("w_67bf0", 0, ""), 0x52908: ("fade", 4, "x-ii+"),
         0x5AFA4: ("idle", 0, "s"), 0x53660: ("w_53660", 0, ""), 0x3D98C: ("end_task", 0, "si"),
         0x1C438: ("w_1c438", 0, "si"), 0x32EB8: ("w_32eb8", 0, "bb"), 0x3FA04: ("replay_event", 0, "iii"),
         0x1148C: ("w_1148c", 0, "iii"), 0x46A4C: ("w_46a4c", 0, "bi"), 0x331A4: ("w_331a4", 0, "bi"), 0x3A32C: ("squad_lost", 0, ""),
         0x63254: ("radio_result", 0, "")}

args = [a for a in sys.argv[1:] if not a.startswith("-")]
N = int(args[0]) if args and args[0].isdigit() else 1500
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
SEED = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
VERBOSE = "-v" in sys.argv
if "--port" not in sys.argv:
    sys.exit("storyfind_diff: only --port (there is no Python mirror)")


def s32(v):
    v &= 0xFFFFFFFF
    return v - (1 << 32) if v & 0x80000000 else v


def run_dump(path, rng, n):
    e = Emu(dump=path, fast=True)
    rt = e.rt
    log, case = [], {}

    def stub(name, pop, rule):
        def fn(em, eax, edx, ebx, ecx):
            regs, out = [eax, edx, ebx, ecx], []
            for i, c in enumerate(rule):
                v = regs[i] if i < 4 else 0
                if c == "s":
                    out.append((v - rt(SOL)) // 0x200)
                elif c == "b":
                    out.append(v & 0xFF)
                elif c == "i":
                    out.append(s32(v))
                elif c == "x":
                    out.append(0)
                elif c == "+":                   # the stack argument
                    out.append(s32(em.r32(em.uc.reg_read(UC_ESP) + 4)))
            log.append((name, out))
            return case["ret"].get(name, 0)
        return fn

    for a, (name, pop, rule) in STUBS.items():
        e.stub(rt(a), stub(name, pop, rule), pop=pop)
    e.stub(rt(0x53538), lambda em, eax, *_: case["rnd"].pop(0) if case["rnd"] else exhausted())

    data0 = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
    fails = done = 0
    for i in range(n):
        e.uc.mem_write(rt(DATA_LO), data0)
        if not fuzz(e, rt, rng):
            continue
        rnd = [rng.randrange(8) for _ in range(400)]
        case.update(ret={"radio_result": rng.choice([0, 1, 3, 0xFF, rng.randrange(256)])}, rnd=list(rnd))
        pre = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        log.clear()
        try:
            e.call(rt(0x6179C), max_steps=20_000_000)
        except Exception as x:                   # a Random loop that never ends: not a case
            if VERBOSE:
                print("skip", i, x)
            continue
        log_o, post_o = list(log), bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        e.uc.mem_write(rt(DATA_LO), pre)
        case["rnd"] = list(rnd)
        log_p = port_run(e, case)
        post_p = bytes(e.uc.mem_read(rt(DATA_LO), DATA_HI - DATA_LO))
        done += 1
        if log_o != log_p or post_o != post_p:
            fails += 1
            if fails <= 5:
                print(f"MISMATCH case {i}")
                if log_o != log_p:
                    for a_, b_ in zip(log_o + [None] * len(log_p), log_p + [None] * len(log_o)):
                        if a_ != b_:
                            print("  call orig", a_, "port", b_)
                            break
                diffs = [j for j in range(len(post_o)) if post_o[j] != post_p[j]]
                if diffs:
                    print("  data " + ", ".join(f"0x{DATA_LO + j:X} {post_o[j]:02X}/{post_p[j]:02X}" for j in diffs[:8]))
        elif VERBOSE:
            print("ok", i, [c[0] for c in log_o])
    return fails, done


def exhausted():
    raise RuntimeError("random list used up")


UC_ESP = None


def port_run(e, case):
    bufs, scalars = port_lib.world_load(e, ai)
    log = []

    def rnd(n_):
        return case["rnd"].pop(0)

    def call(name, a):
        log.append((name, list(a)))
        return case["ret"].get(name, 0), None
    port_lib.sector_run("story_find", bufs, scalars, [], rnd, call)
    port_lib.world_store(e, ai, bufs, scalars)
    d7 = e.r8(e.rt(DUTY7))
    if log and any(c[0] == "w_32eb8" for c in log) and e.r8(e.rt(LIT)) == d7:
        e.w8(e.rt(LIT), 0x40)                    # the front-end's lit face slot
    return log


def fuzz(e, rt, rng):
    w8 = lambda a, v: e.w8(rt(a), v)            # noqa: E731
    w16 = lambda a, v: e.w16(rt(a), v)          # noqa: E731
    finder = rng.randrange(8)
    w8(FINDER, finder)
    cur = rng.choice([rng.randrange(60), e.r8(rt(CUR))])
    w8(CUR, cur)
    for k in range(8):
        s = SOL + 0x200 * k
        w8(s + 0x17A, k)
        w8(s + 0x182, rng.choice([1, 1, 1, 0]))
        w8(s + 0x17E, rng.choice([0, 0, 0, 1, 7, 8]))
        w8(s + 0x183, rng.choice([100, 40, 15, 14, 0, rng.randrange(256)]))
        w8(s + 0x138, rng.choice([1, 1, 0]))
        w16(s + 0x110, rng.choice([rng.randrange(3000), 1000, rng.randrange(65536)]))
        w8(s + 0x1B9, rng.choice([0, 0, rng.randrange(1, 8)]))
        w16(s + 0x114, rng.randrange(3136))     # VacateTile on 0xC41 writes past the map (port: no write)
        for o in (0x185, 0x17F, 0x181, 0x189, 0x1D5, 0x1BF, 0x1C4, 0x195, 0xE4, 0xE5, 0xE6, 0xE7, 0x116, 0x118):
            w8(s + o, rng.choice([0, rng.randrange(256)]))
    s = SOL + 0x200 * finder
    if e.r8(rt(s + 0x183)) >= 0x80 or e.r8(rt(s + 0x183)) < 15:
        w8(s + 0x183, 50)                        # the caller's test
    w8(ai.G_SELECTED, rng.choice([finder, rng.randrange(8), 0x40]))
    w8(STORY + 0x15, rng.choice([0, 0, 1]))
    w8(STORY + 0x14, rng.choice([0, 5, 8, 10, rng.randrange(256)]))
    w8(ai.G_DIFFICULTY, rng.randrange(3))
    e.w32(rt(0x229CB4), rng.choice([0x40, rng.randrange(8)]))
    w8(LIT, rng.choice([0x40, rng.randrange(8)]))
    w8(ai.G_ACTIVE, rng.choice([0x40, rng.randrange(8)]))
    for _ in range(rng.choice([0, 1, 3, 8])):
        w8(TILES + 8 * rng.randrange(3136) + 1, rng.randrange(0xAA, 0xB2))
    r = SECTORS + 0xB1 * cur
    for k in range(5):
        if rng.random() < 0.4:
            w8(r + 0x9F + k, rng.choice([0xAC, 0xAD, 0xAE, 0xAF, 0xAB, 0xB0, 0x5C]))
    # the first Random loop of the find needs another merc in play that fits, else it never ends
    ready = [k for k in range(8) if e.r8(rt(SOL + 0x200 * k + 0x182)) and not e.r8(rt(SOL + 0x200 * k + 0x17E))
             and 15 <= e.r8(rt(SOL + 0x200 * k + 0x183)) < 0x80]
    if not e.r8(rt(STORY + 0x15)) and len(ready) >= 2:
        if not any(e.r8(rt(SOL + 0x200 * k + 0x138)) for k in ready if k != finder):
            w8(SOL + 0x200 * next(k for k in ready if k != finder) + 0x138, 1)
    return True


def main():
    global UC_ESP
    from unicorn.x86_const import UC_X86_REG_ESP
    UC_ESP = UC_X86_REG_ESP
    rng = random.Random(SEED)
    fails = done = 0
    per = max(N // len(dumps), 1)
    for d in dumps:
        if not os.path.exists(d):
            print("missing", d)
            sys.exit(2)
        f, c = run_dump(d, rng, per)
        fails += f
        done += c
    print(f"storyfind_diff --port: {done} cases on {len(dumps)} dumps, {fails} mismatches")
    sys.exit(1 if fails or done < N // 2 else 0)


main()
