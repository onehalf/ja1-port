"""Differential test: sector battles (strategic.md "Sector battles"; library ops 218 resolve_battle, 201 sector_gained).

usage: .venv/bin/python validate/diff/battle_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [--fan IDS] [-v]
The original: ResolveSectorBattle 0x3E508 (byte s, byte f) or, one case in eight, SectorGained 0x3FA54 (s) runs in
Unicorn on tac60. SetSectorOwned 0x3DDE4, the neighbours 0x404B4, SectorWorkable 0x41E1C, SectorConnectedToHome
0x41178, the attack weight 0x3ECB4, RecalcTerritory 0x50EA8 and TimeCompress 0x11D40 run for real; sprintf_ is
Emu.stub_sprintf. Host ops (stubs, logged): the show = 332 battle_ui (0x4B730 k 0, FormatFxPath 0x24C90 k 1, the
sound name of 0x5C16C k 2 / 3 / 4 with its number, 0x5CCF8 k 5, 0x5CE08 k 6, 0x5C144 k 7, 0x4B764 k 8; SpeechStream
0x5CC6C silent), 0x67F48 = 333 battle_wait (logged random answer), the box 0x3E2D4 = 334 battle_box, ShowMessage
0x27B98 = 335 battle_msg (text by its first words, the number at its end), RecordReplayEvent 0x3FA04 = 89, refresh
0x1CB78 = 316, the time compression leaves 0x67C54 mouse_wait / 0x67C68 mouse_release. Random 0x53538 is a logged
Python RNG.
Not compared: text_buf (the original formats the message text there; in the port the host does).
Fuzzed: the sector table (owned bit, enemies, tappers, guards; the other bytes as the dump), the enemy slots, +0xAA,
mines +0xAF / +0xB0 of the sector, guard_skill, supplies[0], the story bytes of the story sector test, time_compress,
the death counters and the gained / lost / last-gained accounts; s 0..59, f 0 / 1.
--port: ja1_resolve_battle / ja1_sector_gained on the same state; the world and the log are compared. Exits 1 on any
mismatch or when a listed path was never reached.
--fan IDS (with --port; #70): the original side runs the 1.15m bytes of ResolveSectorBattle 0x3E54D..0x3E56B that fix
F27 owns (LAST_KEY 0x95DC3 = 0 after the refresh; a write hook on that byte logs it as battle_ui kind 9), the port
runs with the fan mask IDS ("all" = every fix).
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
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = None
if "--fan" in argv:
    v = argv[argv.index("--fan") + 1]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip().lstrip("Ff")) - 1) for x in v.split(","))
    if "--port" not in argv:
        raise SystemExit("battle_diff: --fan needs --port")
F27_ONLY = [(0x3E54D, 0x3E56B)]
LAST_KEY = 0x95DC3
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv

TBL, SECS, ACC, STORY = ai.G_SECTOR_TBL9, ai.G_SECTORS, 0x229FA0, 0x22A040
GUARD_SKILL, SUPPLIES, TIME_COMPRESS = 0x22A030, 0x22A0BE, 0x9300C
TEXTS = ["Mine explodes", "GUARDS REGAIN", "ENEMY DEFEATED", "ENEMIES GAIN", "NATIVES LOSE", "GUARDS DEFEATED"]
LIB = port_lib.ALL_OPS & ~(1 << 43 | 1 << 63)     # the library runs connected_home and ops 63+ (218, 201)

if not os.path.exists(DUMP):
    print(f"battle_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
if FAN is not None:
    e.put_115m(only=F27_ONLY)
if PORT:
    port_lib.fan_fixes(FAN or 0)
rt = e.rt
log, draws = [], []
case = {"waits": []}
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = ai.s16(eax)
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def ui(k):
    def f(*_):
        log.append(("bui", k, 0))
    return f


def leaf(name):
    def f(*_):
        log.append((name,))
    return f


def sound(emu, eax, *_):
    name = emu.cstr(eax).decode()
    for k, p in ((2, "attack"), (3, "die"), (4, "mine")):
        if name.startswith(p):
            log.append(("bui", k, int(name[len(p):-2]) if k > 2 else 0))
            return
    raise SystemExit(f"battle_diff: sound {name}")


def wait(emu, eax, *_):
    v = case["waits"].pop(0) if case["waits"] else 0
    log.append(("bwait", eax & 0xFFFFFFFF, v))
    return v


def box(emu, eax, edx, ebx, ecx):
    sp = emu.uc.reg_read(UC_X86_REG_ESP)
    log.append(("bbox", eax & 0xFF, edx & 0xFF, ebx & 0xFF, ecx & 0xFF, emu.r32(sp + 4) & 0xFF))


def message(emu, eax, *_):
    text = emu.cstr(eax).decode()
    t = next(i for i, p in enumerate(TEXTS) if text.startswith(p))
    log.append(("bmsg", t, int(text.split()[-1])))


e.stub(rt(0x53538), rnd)
e.stub_sprintf()
for a, k in ((0x4B730, 0), (0x24C90, 1), (0x5CCF8, 5), (0x5CE08, 6), (0x5C144, 7), (0x4B764, 8)):
    e.stub(rt(a), ui(k))
e.stub(rt(0x5C16C), sound)
e.stub(rt(0x5CC6C), lambda *_: None)
e.stub(rt(0x67F48), wait)
e.stub(rt(0x3E2D4), box, 4)
e.stub(rt(0x27B98), message, 0x10)
e.stub(rt(0x3FA04), lambda em, eax, edx, ebx, ecx: log.append(("replay", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF,
                                                               ebx & 0xFFFFFFFF)))
e.stub(rt(0x1CB78), leaf("refresh"))
e.stub(rt(0x67C54), leaf("mouse_wait"))
e.stub(rt(0x67C68), leaf("mouse_release"))
if FAN is not None:
    from unicorn import UC_HOOK_MEM_WRITE
    e.uc.hook_add(UC_HOOK_MEM_WRITE, lambda uc, acc, addr, size, val, ud: log.append(("bui", 9, val & 0xFF)),
                  begin=rt(LAST_KEY), end=rt(LAST_KEY))


def fuzz(r):
    s = r.randrange(60)
    tbl = bytearray(e.uc.mem_read(rt(TBL), 540))
    for n in range(60):
        o = 9 * n
        if r.random() < .3:
            tbl[o] ^= 1
    o = 9 * s
    tbl[o] = (tbl[o] & 0xFE) | r.choice([0, 1])
    big = r.random() < .05
    tbl[o + 1] = r.randrange(40) if big else r.choice([0, 1, 2, 3, 4, 5, 6, 7, 8])
    tbl[o + 3] = r.randrange(60) if big else r.choice([0, 0, 1, 2, 3, 5, 8, 12])
    tbl[o + 4] = r.randrange(60) if big else r.choice([0, 0, 1, 2, 3, 5, 8, 12])
    e.uc.mem_write(rt(TBL), bytes(tbl))
    rec = rt(SECS) + 0xB1 * s
    for k in range(8):
        e.w8(rec + 0x96 + k, r.choice([1, 1, 2, 0xFE, 0xFF, 0, r.randrange(256)]))
    e.w8(rec + 0xAA, r.choice([0, 0, 1, 3]))
    e.w8(rec + 0xAF, r.choice([0, 0, 0, 1, 2, 3, 5, 10]) if not big else r.randrange(256))
    e.w8(rec + 0xB0, r.randrange(256))
    e.w8(rt(GUARD_SKILL), r.randrange(256))
    e.w8(rt(SUPPLIES), r.randrange(256))
    for i in (6, 9):
        e.w8(rt(STORY) + i, r.choice([0, 2, 2, 1]))
    for i in (0xB, 0xD):
        e.w8(rt(STORY) + i, r.choice([0, 1, 0xFF, 2]))
    e.w8(rt(TIME_COMPRESS), r.choice([0, 0, 1]))
    for a in (ai.G_DEAD_NPC, ai.G_DEAD_ALLIES, ai.G_ENEMY_DEATHS):
        e.w32(rt(a), r.choice([0, 3, r.randrange(1 << 32)]))
    for o in (0x78, 0x7C, 0x80):
        e.w32(rt(ACC) + o, r.choice([0, 2, r.randrange(1 << 32)]))
    p = r.choice([0, 0, .1, .3, 1])
    case["waits"] = [1 if r.random() < p else 0 for _ in range(200)]
    kind = "gained" if r.random() < .125 else "battle"
    return kind, s, r.choice([0, 1])


def original(kind, s, f):
    log.clear()
    draws.clear()
    if kind == "battle":
        e.call(rt(0x3E508), s, f)
    else:
        e.call(rt(0x3FA54), s)
    return list(log), list(draws)


def port(kind, s, f, bufs, scalars, dr, waits):
    q, wq = list(dr), list(waits)
    got, plog = [], []

    def random_(n):
        # an empty queue (a port fault) gives changing numbers: Pick a sound must not loop forever on 0
        v = q.pop(0)[1] if q else (len(got) % n if n > 0 else 0)
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "battle_ui":
            plog.append(("bui", a[0], a[1]))
        elif op == "battle_wait":
            v = wq.pop(0) if wq else 0
            plog.append(("bwait", a[0], v))
            return v, None
        elif op == "battle_box":
            plog.append(("bbox",) + tuple(x & 0xFF for x in a[:5]))
        elif op == "battle_msg":
            plog.append(("bmsg", a[0], a[1]))
        elif op == "replay_event":
            plog.append(("replay", a[0], a[1], a[2]))
        elif op in ("w_67c54", "mouse_release", "refresh"):
            plog.append(("mouse_wait" if op == "w_67c54" else op,))
        else:
            raise SystemExit(f"battle_diff --port: unmapped op {op} {a}")
        return 0, None
    if kind == "battle":
        port_lib.sector_run("resolve_battle", bufs, scalars, [s, f], random_, call, host_ops=LIB)
    else:
        port_lib.sector_run("sector_gained", bufs, scalars, [s], random_, call, host_ops=LIB)
    return plog, got


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
        kind, s, f = fuzz(r)
        waits = list(case["waits"])
        pw = port_lib.world_load(e, ai) if PORT else None
        lo, dr = original(kind, s, f)
        cover[kind] += 1
        for k in lo:
            cover[" ".join(str(x) for x in k[:2]) if k[0] in ("bui", "bmsg") else
                  f"replay {k[3]:#x}" if k[0] == "replay" else k[0]] += 1
        if any(k[0] == "bbox" and k[5] == 1 for k in lo):
            cover["guard falls"] += 1
        if any(k[0] == "bbox" and k[5] == 2 for k in lo):
            cover["tapper falls"] += 1
        if any(k[0] == "bwait" and k[2] for k in lo):
            cover["skip"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pw
        bufs_o["text_buf"][:] = bytes(len(bufs_o["text_buf"]))     # 0xEBAC0: the original's message text (the host's)
        bufs["text_buf"][:] = bytes(len(bufs["text_buf"]))
        lp, dp = port(kind, s, f, bufs, scalars, dr, waits)
        bad = []
        if lo != lp:
            bad.append(f"log {first_diff(lo, lp)}")
        if [n for n, _ in dr] != [n for n, _ in dp]:
            bad.append(f"random {first_diff(dr, dp)}")
        for k in bufs_o:
            if k in port_lib.READ_ONLY:
                continue
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} ({kind} s {s} f {f}): " + "; ".join(bad)[:700])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["battle", "gained", "guard falls", "tapper falls", "skip", "mouse_wait", "bbox"]
    need += [f"bui {k}" for k in range(9)] + [f"bmsg {t}" for t in range(6)]
    need += [f"replay {c:#x}" for c in (0x4D, 0x4E, 0x4F, 0x50, 0x52, 0x53, 0x54)]
    if FAN is not None and FAN >> 26 & 1:
        need.append("bui 9")
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"battle_diff: never reached: {missing}")
        fails += 1
    print(f"battle_diff{' (port)' if PORT else ''}{f' --fan {FAN:#x}' if FAN is not None else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
