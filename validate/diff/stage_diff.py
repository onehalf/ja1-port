"""Differential test: timed effects and the home-sector stages (turns.md "Start a timed effect", "Home-sector stages").

usage: .venv/bin/python validate/diff/stage_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original in Unicorn on tac60, three modes per case (chosen in turn):
  effect  0x157BC (EAX tile, DL kind, BL timer, ECX power, stack: owner, flag): its result, the 5 x 13 B effect table
          0x22A488 and the fuse beep timer 0x229E0C.
  stage   StageEffects? 0x635DC with 0x157BC real: the effect table; at stage 0 the stat-loss message globals
          (timer 0x92DE8, merc 0x22A1F1, amount 0x22A02F), seen by the port as host op 183 loss_msg (0, 0xFE, 4).
  five    KillStragglers? 0x74C44 with ForgetSoldier 0x3A148 real and MercDies? 0x3A4A4 stubbed (logged).
  mission 0x1AB60 (EAX power; combat.md Mission blast) with the ground blast 0x355D0 stubbed (logged): mission_done.
  doors   ToggleDoorsById? 0x35774 (AL mode, DL value; combat.md Toggling doors by value) with door lookup and
          TileStandable real, 0x3ACB8 / 0x3ADB4 stubbed (logged): fuzzed door pairs, door objects, occupants.
  call    CallEnemies? 0x72568 (AX tile): the call bytes.
Fuzzed: the effect table (free -1, other negative, used entries), the arguments, stage 0..5 and 0xFF, difficulty, the
enemy records 0x28..0x30 (+0x182, +0x138, hp, +0x9C, +0x1C4). The stage table 0x95E0E stays the game's own (the port
holds it as data).
--port: ja1_effect_start, ja1_stage_effects, ja1_stage_five on the same state; results, host calls and the world are
compared. Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv

SOLDIERS = ai.G_SOLDIERS
FX, BEEP, STAGE, DIFF = 0x22A488, 0x229E0C, ai.G_STAGE, ai.G_DIFFICULTY
LOSS = [(ai.G_LOSS_TIMER, 4), (ai.G_LOSS_SLOT, 1), (ai.G_LOSS_AMOUNT, 1)]

if not os.path.exists(DUMP):
    print(f"stage_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
log = []
S0 = rt(SOLDIERS)
e.stub(rt(0x3A4A4), lambda em, a, d, *_: log.append(("merc_dies", (a - S0) // 0x200, d)) and 0)
e.stub(rt(0x355D0), lambda em, a, d, b, c: log.append(("w_355d0", ai._i32(a), ai._i32(d), ai._i32(b), ai._i32(c),
                                                       ai._i32(em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4))))
       and 0, 4)
e.stub(rt(0x3ACB8), lambda em, a, d, b, c: log.append(("break_obj", ai._i32(a), d & 0xFF, b & 0xFF)) and 0)
e.stub(rt(0x3ADB4), lambda em, a, d, b, c: log.append(("close_door", ai._i32(a), d & 0xFF, b & 0xFF)) and 0)
DOOR_OBJS = [0x3B, 0x41, 0x5A, 0x60, 0x3D, 0x3E, 0x5C, 0x5D, 0x43, 0x45, 0x62, 0x64, 0x50, 0x3C]


def fuzz(r, mode):
    tbl = bytearray(e.uc.mem_read(rt(FX), 65))
    for f in range(5):
        b = 13 * f
        free = r.choice([-1, -1, -1, -2, 0, 5, 3000, -0x80000000])
        tbl[b:b + 4] = (free & 0xFFFFFFFF).to_bytes(4, "little")
        for o in range(4, 13):
            if r.random() < .5:
                tbl[b + o] = r.randrange(256)
    if r.random() < .2:
        for f in range(5):
            tbl[13 * f:13 * f + 4] = b"\0\0\0\0"
    e.uc.mem_write(rt(FX), bytes(tbl))
    e.w32(rt(BEEP), r.choice([0, 0, 50, 700, -3 & 0xFFFFFFFF]))
    e.w8(rt(STAGE), r.choice([0, 0, 1, 2, 3, 4, 5, 0xFF]))
    e.w8(rt(DIFF), r.choice([0, 1, 2, 0xFF, 0x80]))
    for a_, k_ in LOSS:
        e.uc.mem_write(rt(a_), bytes(r.randrange(256) for _ in range(k_)))
    for k in range(0x28, 0x30):
        p = S0 + 0x200 * k
        e.w8(p + 0x182, r.choice([1, 1, 1, 0]))
        e.w8(p + 0x138, r.choice([1, 1, 1, 0]))
        e.w8(p + 0x183, r.choice([40, 20, 15, 14, 0, 0x90]))
        e.w8(p + 0x9C, r.choice([0, 0, 1, 0xFF, 0x80, 5]))
        e.w8(p + 0x1C4, r.choice([0, 0, 0, 1]))
    if mode == "mission":
        e.w8(rt(ai.G_CUR_SECTOR), r.choice([11, 11, 0, 5, 59, 70]))
        e.uc.mem_write(rt(ai.G_MISSION_DONE), bytes(r.choice([0, 0, 1, 2]) for _ in range(80)))
        return [r.choice([1000, 0, 50, -1, r.randrange(1 << 32) - (1 << 31)])]
    if mode == "call":
        e.uc.mem_write(rt(ai.G_CALL_M), bytes(r.randrange(256) for _ in range(8)))
        return [r.choice([0, 0x15B, 3135, r.randrange(1 << 16), r.randrange(1 << 32) - (1 << 31)]), 0x44]
    if mode == "doors":
        cur = r.choice([0, 3, 11, 25, 59])
        e.w8(rt(ai.G_CUR_SECTOR), cur)
        row = rt(ai.G_SECTORS) + 0xB1 * cur
        tiles = [r.randrange(3136) for _ in range(6)]
        for k in range(12):
            t = r.choice(tiles + [r.randrange(3136), 0xFFFF])
            e.w16(row + 4 * k, t)
            e.w16(row + 4 * k + 2, r.choice([0, 1, 2, 3, 0x81, 0x101]))
        for t in tiles:
            m = rt(ai.G_TILEMAP) + 8 * t
            e.w8(m + 1, r.choice(DOOR_OBJS))
            e.w8(m + 3, r.choice([0, 0x73]))
            e.w8(m + 5, r.choice([0x40, 0x40, 0x41, 0xC0, 3, 0x2A, 0x83]))
        return [r.choice([0, 1, 1, 2, 0x101]), r.choice([0, 1, 2, 3, 0x81])]
    if mode == "effect":
        tile = r.choice([0x15B, 0xC41, 0, r.randrange(3136), r.randrange(1 << 16)])
        return [tile, r.randrange(256), r.choice([0, 0, 1, 2, 0xFF]),
                r.choice([0, 800, -5, r.randrange(1 << 32) - (1 << 31)]), r.choice([0x41, 3, 0x28]),
                r.choice([0, 0, 1, 0x80])]
    return []


def original(mode, a):
    del log[:]
    if mode == "effect":
        v = e.call(rt(0x157BC), a[0], a[1], a[2], a[3] & 0xFFFFFFFF, a[4], a[5]) & 0xFFFFFFFF
        return v, list(log)
    if mode == "mission":
        e.call(rt(0x1AB60), a[0] & 0xFFFFFFFF)
        return None, list(log)
    if mode == "call":
        e.call(rt(0x72568), a[0] & 0xFFFFFFFF, a[1])
        return None, list(log)
    if mode == "doors":
        e.call(rt(0x35774), a[0], a[1])
        return None, list(log)
    pre = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in LOSS]
    e.call(rt(0x635DC if mode == "stage" else 0x74C44))
    post = [bytes(e.uc.mem_read(rt(a_), k_)) for a_, k_ in LOSS]
    if post != pre:
        log.append(("loss_msg", e.r8(rt(ai.G_LOSS_SLOT)), e.r8(rt(ai.G_LOSS_AMOUNT)),
                    4 if e.r32(rt(ai.G_LOSS_TIMER)) == 0x1D4C else -1))
        for (a_, k_), b_ in zip(LOSS, pre):
            e.uc.mem_write(rt(a_), b_)
    return None, list(log)


def port_calls(name, bufs, scalars, a):
    plog = []

    def call(op, x):
        plog.append((op,) + tuple(x))
        return 0, None
    r = port_lib.sector_run(name, bufs, scalars, a, None, call, sync=True)
    return r, plog


def main():
    r = random.Random(11)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    modes = ["effect", "stage", "five", "mission", "doors", "call"]
    for i in range(N):
        e.uc.mem_write(0, snap)
        mode = modes[i % len(modes)]
        a = fuzz(r, mode)
        pre = port_lib.world_load(e, ai) if PORT else None
        res, olog = original(mode, a)
        if mode == "effect":
            cover[f"effect {res} beep {a[5] != 0}"] += 1
        elif mode == "doors":
            cover[f"doors {sorted(set(c_[0] for c_ in olog))}"] += 1
        else:
            cover[f"{mode} calls {min(len(olog), 3)}"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        bad = []
        name = {"effect": "effect_start", "stage": "stage_effects", "five": "stage_five", "mission": "mission_blast",
                "doors": "toggle_doors", "call": "call_enemies"}[mode]
        v, plog = port_calls(name, bufs, scalars, a)
        if mode == "effect" and v != res:
            bad.append(f"result {res} vs {v}")
        if plog != olog:
            bad.append(f"calls {olog} vs {plog}")
        for k in bufs_o:
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} {mode} {a}: " + "; ".join(bad)[:700])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["effect 0 beep False", "effect 1 beep False", "effect 1 beep True", "stage calls 0", "stage calls 1",
            "five calls 0", "five calls 1", "five calls 3", "mission calls 1", "call calls 0", "doors []",
            "doors ['break_obj']", "doors ['close_door']"]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"stage_diff: never reached: {missing}")
        fails += 1
    print(f"stage_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
