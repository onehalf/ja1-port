"""Differential test: the day clock and time compression (turns.md "The day clock", "Time compression").

usage: .venv/bin/python validate/diff/clock_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original: DayClock 0x1F724 (was GuardsArriveMsg; NewTurn's call with 5 minutes) runs in Unicorn on tac60;
SectorConnectedToHome 0x41178, PickNightMerc 0x32828 and TimeCompress 0x11D40 run for real. Host ops (stubs, logged):
the day bar 0x3DB3C = 309 day_bar, the enemies' sector 0x3F45C = 310 enemy_pick, enemy moves 0x3EDE8 = 311
enemy_move, a hired guard walks in 0x30600 = 312 guard_enter, OverviewRedraw 0x23B4C = 313 mode_banner,
TacticalRedraw 0x176C8 = 112 tac_redraw, MsgStrInt 0x27CE0 = 314 guards_msg, RecordReplayEvent 0x3FA04 = 89,
ResolveSectorBattle 0x3E508 = 218, 0x67C54 = 198 mouse_wait, 0x67C68 = 315 mouse_release, 0x1CB78 = 316 refresh,
StopAnim 0x196F8 = 53 stop_anim. Random 0x53538 is a logged Python RNG.
One case in five calls TimeCompress 0x11D40 (on or off) directly instead of the clock.
Fuzzed: the clock (minute, hour, hours, minutes, bar), hard_day, night_warned, day, difficulty, cur_sector,
time_compress, opt_onscreen, the sector table, the guard arrival table (0x10D2B8 s16 minutes, 0x10D0D8 records),
mercs 0..7 (+0x182 +0x138 +0x183 +0x18A), the quote fields.
--port: ja1_day_clock / ja1_time_compress on the same state; the world, the log and the result are compared.
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
port_lib.CLOCK = True
VERBOSE = "-v" in argv

SOLDIERS, SECTORS = 0xFA958, ai.G_SECTOR_TBL9
MINUTE, HOUR, HOURS, MINUTES, BAR = 0x22A0F5, 0x22A0F6, 0x22A0F4, 0x22A0F7, 0x22A0FB
HARD, NIGHT, ETA, REC = 0x22A10C, 0x22A0D3, 0x10D2B8, 0x10D0D8

if not os.path.exists(DUMP):
    print(f"clock_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
e.stub_sprintf()
log, draws = [], []
rng = random.Random(7)
case = {"pick": [], "move": [], "enter": []}


def s8(v):
    v &= 0xFF
    return v - 0x100 if v & 0x80 else v


def rnd(emu, eax, *_):
    n = eax & 0xFFFF
    n = n - 0x10000 if n & 0x8000 else n
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def leaf(name, fn=None):
    def f(emu, eax, edx, ebx, ecx):
        if fn:
            return fn(emu, eax, edx, ebx, ecx)
        log.append((name,))
        return 0
    return f


def pick(emu, *_):
    v = case["pick"].pop(0) if case["pick"] else 0
    log.append(("enemy_pick", v))
    return v


def move(emu, eax, *_):
    v = case["move"].pop(0) if case["move"] else 0
    log.append(("enemy_move", eax & 0xFF, v))
    return v


def enter(emu, eax, *_):
    v = case["enter"].pop(0) if case["enter"] else 0
    log.append(("guard_enter", eax & 0xFF, v))
    return v


def msg(emu, eax, edx, *_):
    t = bytes(emu.uc.mem_read(eax, 40)).split(b"\0")[0]
    n = 1 if t.startswith(b"1 Guard arrives") else int(t.split(b" ")[0])
    # count > 1: EDX is what sprintf_ left (SPEC: an unrelated number); not compared
    log.append(("guards_msg", n, edx & 0xFFFFFFFF if n == 1 else None))
    return 0


e.stub(rt(0x53538), rnd)
e.stub(rt(0x3DB3C), lambda em, eax, *_: log.append(("day_bar", eax & 0xFF)))
e.stub(rt(0x3F45C), pick)
e.stub(rt(0x3EDE8), move)
e.stub(rt(0x30600), enter)
e.stub(rt(0x23B4C), leaf("mode_banner"))
e.stub(rt(0x176C8), lambda em, eax, *_: log.append(("tac_redraw", eax & 0xFFFFFFFF)))
e.stub(rt(0x27CE0), msg)
e.stub(rt(0x3FA04), lambda em, eax, edx, ebx, ecx: log.append(("replay", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF,
                                                               ebx & 0xFFFFFFFF)))
e.stub(rt(0x3E508), lambda em, eax, edx, *_: log.append(("resolve", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF)))
e.stub(rt(0x67C54), leaf("mouse_wait"))
e.stub(rt(0x67C68), leaf("mouse_release"))
e.stub(rt(0x1CB78), leaf("refresh"))
e.stub(rt(0x196F8), lambda em, eax, edx, *_: log.append(("stop_anim", em.r8(eax + 0x17A), s8(edx) & 0xFFFFFFFF)))


def fuzz(r):
    h = r.choice([0, 1, 5, 9, 10, 10, 11, 11, 11, r.randrange(256)])
    m = r.choice([0, 5, 50, 55, 55, 59, r.randrange(60), r.randrange(256)])
    e.w8(rt(HOURS), h)
    e.w8(rt(MINUTE), m)
    e.w8(rt(HOUR), r.choice([(7 + h - 1) % 12 + 1, 12, 12, 1, r.randrange(256)]))
    tot = r.choice([60 * s8(h) + m] * 3 + [r.randrange(-50, 800), r.randrange(1 << 32)])
    e.w32(rt(MINUTES), tot)
    e.w32(rt(BAR), r.choice([0, 3, 19, 20, r.randrange(1 << 32), tot * 100 // 720 // 5 if tot >= 0 else 0]))
    e.w8(rt(HARD), r.choice([0, 0, 1, r.randrange(256)]))
    e.w8(rt(NIGHT), r.choice([0, 0, 1]))
    e.w32(rt(ai.G_DAY), r.choice([0, 1, 2, 3, 4, 5, 10, r.randrange(1 << 16)]))
    e.w8(rt(ai.G_DIFFICULTY), r.randrange(3))
    e.w8(rt(ai.G_CUR_SECTOR), r.choice([0, 0, 59, 58, r.randrange(60)]))
    e.w8(rt(0x9300C), r.choice([0, 1]))
    e.w8(rt(0x92DDF), r.choice([0, 0, 1, 3]))
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 540))
    for s in range(60):
        if r.random() < .5:
            tbl[9 * s] = r.choice([0, 1, 1, 3, r.randrange(256)])
        if r.random() < .1:
            tbl[9 * s + 4] = r.randrange(256)
            tbl[9 * s + 5] = r.randrange(256)
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    eta = bytearray(960)
    rec = bytearray(r.randrange(256) for _ in range(480))
    pe = r.choice([0, 0, .01, .05, .3])
    for i in range(480):
        v = 9999
        if r.random() < pe:
            v = r.choice([tot, tot - 5, tot + 5, 0, r.randrange(-100, 800), r.randrange(1 << 16)]) & 0xFFFF
        eta[2 * i:2 * i + 2] = v.to_bytes(2, "little")
    e.uc.mem_write(rt(ETA), bytes(eta))
    e.uc.mem_write(rt(REC), bytes(rec))
    for k in range(8):
        sl = rt(SOLDIERS) + 0x200 * k
        e.w8(sl + 0x182, r.choice([0, 1, 1]))
        e.w8(sl + 0x138, r.choice([0, 1, 1]))
        e.w8(sl + 0x183, r.choice([0, 10, 14, 15, 50, 0x90]))
        e.w8(sl + 0x18A, r.choice([0, 10, 11, 50, 0x90]))
    e.w8(rt(0x92DEC), r.choice([0x40, 0x40, 3]))
    case["pick"] = [r.choice([0, 3, 58, 59, r.randrange(256)]) for _ in range(2)]
    case["move"] = [r.choice([0, 0, 0, 1, 3, 0x80]) for _ in range(3)]
    case["enter"] = [r.choice([0, 1, 1, 0x100]) for _ in range(500)]
    return r.choice([5] * 8 + [1, 10, 60, 0x105])


def original(kind, n):
    log.clear()
    draws.clear()
    if kind == "clock":
        e.call(rt(0x1F724), n)
        return list(log), list(draws), e.uc.reg_read(ai_reg_eax()) & 0xFFFFFFFF
    e.call(rt(0x11D40), n)
    return list(log), list(draws), None


def ai_reg_eax():
    from unicorn.x86_const import UC_X86_REG_EAX
    return UC_X86_REG_EAX


def port(kind, n, bufs, scalars, dr):
    plog = []
    q = list(dr)
    got = []
    pk, mv, en = list(case["pick0"]), list(case["move0"]), list(case["enter0"])

    def random_(m):
        v = q.pop(0)[1] if q else 0
        got.append((m, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "w_67c54":                      # port_lib's name of op 198 (select.md)
            op = "mouse_wait"
        if op == "day_bar":
            plog.append(("day_bar", a[0] & 0xFF))
        elif op == "enemy_pick":
            v = pk.pop(0) if pk else 0
            plog.append(("enemy_pick", v))
            return v, None
        elif op == "enemy_move":
            v = mv.pop(0) if mv else 0
            plog.append(("enemy_move", a[0] & 0xFF, v))
            return v, None
        elif op == "guard_enter":
            v = en.pop(0) if en else 0
            plog.append(("guard_enter", a[0] & 0xFF, v))
            return v, None
        elif op == "tac_redraw":
            plog.append(("tac_redraw", a[0]))
        elif op == "guards_msg":
            plog.append(("guards_msg", a[0], a[1] if a[0] == 1 else None))
        elif op == "replay_event":
            plog.append(("replay", a[0], a[1], a[2]))
        elif op == "resolve_battle":
            plog.append(("resolve", a[0], a[1]))
        elif op == "stop_anim":
            plog.append(("stop_anim", a[0] & 0xFF, a[1]))
        elif op in ("mode_banner", "mouse_wait", "mouse_release", "refresh"):
            plog.append((op,))
        else:
            raise SystemExit(f"clock_diff --port: unmapped op {op} {a}")
        return 0, None
    host = port_lib.ALL_OPS & ~(1 << 43)          # connected_home: the library's own
    res = port_lib.sector_run("day_clock" if kind == "clock" else "time_compress", bufs, scalars, [n], random_, call,
                              host_ops=host)
    return plog, got, (res & 0xFFFFFFFF if kind == "clock" else None)


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        n = fuzz(r)
        kind = "clock"
        if r.random() < .2:
            kind, n = "compress", r.choice([0, 1, 1, 0x100])
        case["pick0"], case["move0"], case["enter0"] = list(case["pick"]), list(case["move"]), list(case["enter"])
        pre = port_lib.world_load(e, ai) if PORT else None
        night0 = e.r8(rt(NIGHT))
        lo, dr, res = original(kind, n)
        cover[kind] += 1
        if res:
            cover["day over"] += 1
        for k in lo:
            cover[k[0]] += 1
        if e.r8(rt(NIGHT)) and not night0:
            cover["night quote"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp, got, pres = port(kind, n, bufs, scalars, dr)
        bad = []
        if res != pres:
            bad.append(f"result {res} vs {pres}")
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        if dr != got:
            bad.append(f"random {dr} vs {got}")
        for k in bufs_o:
            # other variables share these addresses (as in world_diff): the read-only tables (absorb bytes 0x42..0x45
            # = tick_step, 0xEE = time_compress, dir_steps byte 0x54 = time_compress; scalars, compared there),
            # text_buf, enemy_watch bytes 0..39
            if k in port_lib.READ_ONLY or k == "text_buf":
                continue
            if k == "enemy_watch":
                bufs_o[k] = bufs[k][:40] + bufs_o[k][40:]
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} ({kind} {n}): " + "; ".join(bad)[:500])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ("day over", "day_bar", "enemy_pick", "enemy_move", "guard_enter", "guards_msg", "replay", "resolve",
            "mode_banner", "tac_redraw", "mouse_release", "stop_anim", "refresh", "night quote")
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"clock_diff: never reached: {missing}")
        fails += 1
    print(f"clock_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
