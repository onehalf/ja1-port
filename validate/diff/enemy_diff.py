"""Differential test: the enemy commander's day-clock moves (strategic.md "Enemy moves"; ops 310 / 311).

usage: .venv/bin/python validate/diff/enemy_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original: enemy_pick 0x3F45C (one case in four) or enemy_move 0x3EDE8 (byte src) runs in Unicorn on tac60;
SectorValue 0x3E03C (SectorConnectedToHome 0x41178, 0x633D8), the shuffle 0x3ED94, take / put an enemy 0x3DFDC /
0x3DF4C and TimeCompress 0x11D40 run for real. Host ops (stubs, logged): the walk-in 0x306BC = 329 enemy_walk_in
(logged random answer 0 / 1), RecordReplayEvent 0x3FA04 = 89, ResolveSectorBattle 0x3E508 = 218, the time
compression leaves 0x67C54 mouse_wait, 0x67C68 mouse_release, 0x1CB78 refresh. Random 0x53538 is a logged Python RNG.
The mirror: tools/enemy.py on the same fuzzed state (its SectorValue callees run in Unicorn), the same draws.
Fuzzed: the sector table (flags, enemies, mercs, tappers, guards, unhealthy), the enemy slots and +0xAA of every
sector record, Santino's reserve 0x22A008, guard_skill, supplies[0], the story bytes of the story condition, the pick
counter 0x22A0BD, time_compress, difficulty, day.
--port: ja1_enemy_pick / ja1_enemy_move on the same state; the world, the log and the result are compared.
--fan IDS (with --port; #70): the original side runs the 1.15m bytes of EnemyMoves? / SectorValue that fix F16 owns
(0x3EF76, cave 0x3E0CF, 0x3F0A8), the port runs with the fan fixes IDS on (`all` or `F16`); the mirror is not
compared (it is 1.13b).
Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import enemy                         # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = None
if "--fan" in argv:
    v = argv[argv.index("--fan") + 1]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip().lstrip("Ff")) - 1) for x in v.split(","))
    if "--port" not in argv:
        raise SystemExit("enemy_diff: --fan needs --port")
F16_ONLY = [(0x3EF76, 0x3EF96), (0x3E0CF, 0x3E0DE), (0x3F0A8, 0x3F0A9)]
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.ENEMY = True                # sector_order, enemy_counter
VERBOSE = "-v" in argv

TBL, SECS, ORDER = ai.G_SECTOR_TBL9, ai.G_SECTORS, ai.G_SECTOR_ORDER
COUNTER, SANTINO = enemy.G_ENEMY_COUNTER, enemy.G_SANTINO
LIB = port_lib.ALL_OPS & ~(1 << 9 | 1 << 43 | 1 << 44)   # the library runs sector_value, connected_home, quest_check

if not os.path.exists(DUMP):
    print(f"enemy_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
if FAN is not None:
    e.put_115m(only=F16_ONLY)
if PORT:
    port_lib.fan_fixes(FAN or 0)
rt = e.rt
log, draws = [], []
case = {"walk": []}
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = ai.s16(eax)
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def walk(emu, eax, edx, *_):
    v = case["walk"].pop(0) if case["walk"] else 0
    log.append(("walk_in", eax & 0xFF, edx & 0xFF, v))
    return v


def leaf(name):
    def f(*_):
        log.append((name,))
        return 0
    return f


e.stub(rt(0x53538), rnd)
e.stub(rt(0x306BC), walk)
e.stub(rt(0x3FA04), lambda em, eax, edx, ebx, ecx: log.append(("replay", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF,
                                                               ebx & 0xFFFFFFFF)))
e.stub(rt(0x3E508), lambda em, eax, edx, *_: log.append(("resolve", eax & 0xFFFFFFFF, edx & 0xFF)))
e.stub(rt(0x67C54), leaf("mouse_wait"))
e.stub(rt(0x67C68), leaf("mouse_release"))
e.stub(rt(0x1CB78), leaf("refresh"))


class W:
    """the mirror's world: emulator memory; Random replays the original's draws"""
    def __init__(self, dr):
        self.q, self.got = list(dr), []

    def g8(self, a):
        return e.r8(rt(a))

    def g16(self, a):
        return e.r16(rt(a))

    def g32(self, a):
        return e.r32(rt(a))

    def s8(self, a, v):
        e.w8(rt(a), v & 0xFF)

    def s32(self, a, v):
        e.w32(rt(a), v & 0xFFFFFFFF)

    def random(self, k):
        k = ai.s16(k)
        v = self.q.pop(0)[1] if self.q else 0
        self.got.append((k, v))
        return v

    def ext(self, name, *a):
        if name == "connected_home":
            return e.call(rt(0x41178), rt(a[0]), a[1])
        if name == "quest_check":
            return e.call(rt(0x633D8), *a)
        if name == "walk_in":
            return walk(e, a[0], a[1])
        if name == "replay":
            log.append(("replay", a[0] & 0xFFFFFFFF, a[1] & 0xFFFFFFFF, a[2] & 0xFFFFFFFF))
            return 0
        if name == "resolve":
            log.append(("resolve", a[0] & 0xFFFFFFFF, a[1] & 0xFF))
            return 0
        if name == "time_compress":
            e.call(rt(0x11D40), a[0])
            return 0
        raise SystemExit(f"enemy_diff: mirror ext {name}")


def fuzz(r):
    tbl = bytearray(e.uc.mem_read(rt(TBL), 540))
    wild = r.random() < .05
    for s in range(60):
        o = 9 * s
        tbl[o] = (1 if r.random() < .3 else 0) | (r.randrange(32) & 0x1E if r.random() < .3 else tbl[o] & 0x1E)
        tbl[o + 1] = r.randrange(256) if wild and r.random() < .2 else r.choice([0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 8])
        tbl[o + 2] = r.choice([0] * 8 + [1, 2, 4, r.randrange(256) if wild else 3])
        tbl[o + 3] = r.choice([0, 0, 1, 3, 5, 9])
        tbl[o + 4] = r.choice([0, 0, 0, 1, 2, 4, r.randrange(256) if wild else 3])
        tbl[o + 8] = r.choice([0] * 9 + [1])
    e.uc.mem_write(rt(TBL), bytes(tbl))
    for s in range(60):
        b = SECS + 0xB1 * s
        n = tbl[9 * s + 1]
        sl = bytes((r.choice([1, 2]) if k < n else r.choice([0xFF, 0xFE])) if r.random() < .9
                   else r.randrange(256) for k in range(8))
        e.uc.mem_write(rt(b + 0x96), sl)
        e.w8(rt(b + 0xAA), r.choice([0, 0, 1]))
    e.w32(rt(SANTINO), r.choice([0, 1, 3, 7, 8, 10, 20, -1, -200]) & 0xFFFFFFFF)
    e.w8(rt(enemy.G_GUARD_SKILL), r.randrange(256))
    e.w8(rt(enemy.G_SUPPLIES), r.randrange(256))
    if r.random() < .2:
        e.w8(rt(0x22A049), 2)
        e.w8(rt(0x22A046), 2)
        e.w8(rt(0x22A04B), r.choice([0, 0, 1]))
    e.w8(rt(0x22A04D), r.choice([0, 0, 1]))
    e.w8(rt(COUNTER), r.choice([0, 5, 59, 60, 60, 61, 255, r.randrange(256)]))
    e.w8(rt(enemy.G_TIME_COMPRESS), r.choice([0, 1]))
    e.w8(rt(ai.G_DIFFICULTY), r.randrange(3))
    e.w32(rt(ai.G_DAY), r.choice([1, 2, 3, 5, 9, 20]))
    case["walk"] = [r.choice([0, 1, 1]) for _ in range(20)]
    case["zero"] = False
    kind = "pick" if r.random() < .25 else "move"
    src = r.choice([0, 0, r.randrange(60), r.randrange(60), r.randrange(60)])
    if kind == "move" and r.random() < .06:
        src = zero_score(r) or src
    if FAN is not None and kind == "move" and r.random() < .5:
        # F16: an attack on the plant 0x35 from a neighbour (the 1.15m change matters only there)
        src = r.choice([0x2B, 0x34, 0x36])
        e.w8(rt(0x22A04D), 1 if r.random() < .8 else 0)
        o = 9 * 0x35
        tbl[o + 2] = r.choice([1, 2, 4, 40, 120, 255])
        tbl[o + 4] = r.choice([0, 3, 60, 200])
        tbl[9 * src + 1] = r.choice([3, 5, 8])
        e.uc.mem_write(rt(TBL), bytes(tbl))
    elif FAN is not None and kind == "move" and r.random() < .5:
        # F16: an attack from a story sector (its 1.13b attackers' weight is 4x, the 1.15m one is the target's)
        src = r.choice([0x11, 0x1B])
        e.w8(rt(0x22A049), 2)
        e.w8(rt(0x22A046), 2)
        e.w8(rt(0x22A04B), 0)
        tbl[9 * src + 1] = r.choice([3, 5, 8])
        for nb in (src - 1, src + 1, src - 10, src + 10):
            if r.random() < .6:
                tbl[9 * nb + 2] = r.choice([1, 2, 4])
                tbl[9 * nb + 1] = r.choice([0, 1, 3])
        e.uc.mem_write(rt(TBL), bytes(tbl))
    return kind, src


def zero_score(r):
    """an attack whose defence + attack sum wraps to exactly 0 (16-bit): the score division by zero (enemy.rs
    de + att == 0: skipped, or +infinity = taken). Picks src + one neighbour, then the neighbour's defenders."""
    src = r.randrange(1, 60)
    ds = [d for d in range(4) if not (d == 0 and src % 10 == 9 or d == 1 and src > 49 or d == 2 and src < 10
                                      or d == 3 and src % 10 == 0)]
    d = r.choice(ds)
    nb = src + enemy.OFFSET[d]
    t = lambda s, k: rt(TBL + 9 * s + k)     # noqa: E731
    e.w8(t(src, 0), e.r8(t(src, 0)) & ~1)
    e.w8(t(src, 2), 0)
    keep = 3 if not e.r8(rt(SECS + 0xB1 * src + 0xAA)) else 1
    surplus = r.randrange(2, 9 - keep)
    e.w8(t(src, 1), keep + surplus)
    e.w8(t(nb, 1), r.randrange(0, 7))
    if r.random() < .5:
        e.w8(t(nb, 0), e.r8(t(nb, 0)) | 1)
    for s in (src, nb):
        n = e.r8(t(s, 1))
        e.uc.mem_write(rt(SECS + 0xB1 * s + 0x96), bytes(r.choice([1, 2]) if k < n else 0xFF for k in range(8)))
    w = W([])
    f = 1 if not e.r8(t(src, 0)) & enemy.WALL[d] or ai._story_cond(w, nb) else 0
    cnt = min(8 - e.r8(t(nb, 1)), surplus, 7)
    att = ai.s16(enemy.attack_weight(w, src, f) * cnt + enemy.attack_weight(w, nb, 1) * e.r8(t(nb, 1)))
    if att == 0 or att == -0x8000:
        return None
    m, g = enemy.defend_merc(w), enemy.defend_guard(w)
    want = -att & 0xFFFF
    sols = []
    for a in range(1, 256):
        for b in (0, r.randrange(256), r.randrange(256)):
            c2 = (want - a * m - b * g) & 0xFFFF
            if c2 % 2 == 0 and c2 // 2 < 256:
                sols.append((a, b, c2 // 2))
    if not sols:
        return None
    a, b, c = r.choice(sols)
    e.w8(t(nb, 2), a)
    e.w8(t(nb, 4), b)
    e.w8(t(nb, 3), c)
    case["zero"] = ai.s16(ai.sector_value(w, nb, f)) != -100
    return src


SEEN = [(TBL, 540), (SECS, 60 * 0xB1), (ORDER, 61), (COUNTER, 1), (SANTINO, 4), (enemy.G_TIME_COMPRESS, 1),
        (0x92F60, 4)]


def state():
    return [bytes(e.uc.mem_read(rt(a), n)) for a, n in SEEN]


def original(kind, src):
    log.clear()
    draws.clear()
    v = e.call(rt(0x3F45C)) if kind == "pick" else e.call(rt(0x3EDE8), src)
    return v & 0xFF, list(log), list(draws)


def mirror(kind, src, dr):
    log.clear()
    w = W(dr)
    v = enemy.enemy_pick(w) if kind == "pick" else enemy.enemy_move(w, src)
    return v & 0xFF, list(log), w.got


def port(kind, src, bufs, scalars, dr, walks):
    q, wq = list(dr), list(walks)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else 0
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "enemy_walk_in":
            v = wq.pop(0) if wq else 0
            plog.append(("walk_in", a[0] & 0xFF, a[1] & 0xFF, v))
            return v, None
        if op == "replay_event":
            plog.append(("replay", a[0], a[1], a[2]))
        elif op == "resolve_battle":
            plog.append(("resolve", a[0], a[1] & 0xFF))
        elif op in ("w_67c54", "mouse_release", "refresh"):
            plog.append(("mouse_wait" if op == "w_67c54" else op,))
        else:
            raise SystemExit(f"enemy_diff --port: unmapped op {op} {a}")
        return 0, None
    if kind == "pick":
        v = port_lib.sector_run("enemy_pick", bufs, scalars, [], random_, call, host_ops=LIB)
    else:
        v = port_lib.sector_run("enemy_move", bufs, scalars, [src], random_, call, host_ops=LIB)
    return v & 0xFF, plog, got


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
        kind, src = fuzz(r)
        walks = list(case["walk"])
        fz = bytes(e.uc.mem_read(0, 16 << 20))
        pre = state()
        pw = port_lib.world_load(e, ai) if PORT else None
        vo, lo, dr = original(kind, src)
        so = state()
        bufs_o, sc_o = port_lib.world_load(e, ai) if PORT else (None, None)
        cover[kind] += 1
        if case["zero"]:
            cover["zero sum"] += 1
        for k in lo:
            cover[k[0] if k[0] != "replay" else f"replay {k[3] if k[3] < 100 or k[3] > 108 else '100+n'}"] += 1
        if vo:
            cover["arrivals"] += 1
        if kind == "pick" and so[3] == b"\0" and pre[3] != b"\0":
            cover["shuffle"] += 1
        if kind == "move" and src == 0 and so[4] != pre[4]:
            cover["santino sends"] += 1
        if sum(1 for k in lo if k[0] == "replay" and 100 < k[3] <= 108) > 1:
            cover["two moves"] += 1
        bad = []
        e.uc.mem_write(0, fz)
        case["walk"] = list(walks)
        vm, lm, dm = mirror(kind, src, dr)
        sm = state()
        if FAN is not None:
            sm = so = ()
            vm, lm, dm = vo, lo, dr
        if (vo, lo) != (vm, lm):
            bad.append(f"mirror result {vo} vs {vm}, log {first_diff(lo, lm)}")
        if [n for n, _ in dr] != [n for n, _ in dm]:
            bad.append(f"mirror random {first_diff(dr, dm)}")
        for (a, n), x, y in zip(SEEN, so, sm):
            if x != y:
                j = next(j for j in range(n) if x[j] != y[j])
                bad.append(f"mirror {a:#x}+{j:#x}: {x[j]:#x} vs {y[j]:#x}")
        if PORT:
            bufs, scalars = pw
            vp, lp, dp = port(kind, src, bufs, scalars, dr, walks)
            if (vo, lo) != (vp, lp):
                bad.append(f"port result {vo} vs {vp}, log {first_diff(lo, lp)}")
            if [n for n, _ in dr] != [n for n, _ in dp]:
                bad.append(f"port random {first_diff(dr, dp)}")
            for k in bufs_o:
                # the read-only tables share addresses with scalars (as in clock_diff: absorb 0x42..0x45 = tick_step,
                # 0xEE and dir_steps 0x54 = time_compress; compared as scalars)
                if k in port_lib.READ_ONLY:
                    continue
                if bufs_o[k] != bufs[k]:
                    j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                    bad.append(f"port {k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
            for k in sc_o:
                if sc_o[k] != scalars[k]:
                    bad.append(f"port {k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} ({kind} {src}): " + "; ".join(bad)[:600])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ("walk_in", "resolve", "replay 75", "replay 76", "replay 84", "replay 87", "replay 100+n", "shuffle",
            "santino sends", "two moves", "arrivals", "mouse_wait", "zero sum")
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"enemy_diff: never reached: {missing}")
        fails += 1
    print(f"enemy_diff{' (port)' if PORT else ''}{f' --fan {FAN:#x}' if FAN is not None else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
