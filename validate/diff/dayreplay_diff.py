"""Differential test: the day replay (strategic.md "The day replay"; library op day_replay).

usage: .venv/bin/python validate/diff/dayreplay_diff.py [ncases=1500] [DUMP] [--port] [--lib SO] [-v]
The original: DayReplay 0x652B8 runs in Unicorn on tac60 with a fuzzed day (events, the replay table 0x2254F0,
Day Replays 0x1163C4, the carry 0x9562D). REPLAY.REC: fopen_ / fclose_ stubs, fread_ hands out the events.
EscPressed? 0x67300 answers from a fuzzed queue (the port: bit 24 of host op 354 replay_record, and the answer of
each battle frame). The animations 0x64658 (k 0), 0x64D20 (k 1), 0x643F8 (k 3), 0x664FC (k 4), DrawMorphFrame?
0x65FB0 (k 5) are stubs that log their register arguments when the show byte 0x963BC is 1 and answer from a second
fuzzed queue (Escape in it: 0x963BC = 0) = host op 355 replay_anim; the battle 0x6494C runs for real (its
pictures stubbed: BlitCopy, BlitKey, 0x41788, 0x41640, 0x419BC), each of its frames logs the 30 ms wait 0x67250 with
its bursts (stack locals) = host op 355 (2, ...). The map draws 0x41C3C (k 0), 0x41B24 (k 1, EDX), 0x41BB8 (k 2, EAX), 0x41C60 (k 3, EDX) =
host op 356 replay_draw. SetSectorOwned, SectorWorkable, SectorConnectedToHome run for real on the replay table.
Random 0x53538 is a logged Python RNG.
--port: ja1_day_replay on the same state; the log, the random draws and the world are compared. Exits 1 on any
mismatch or when a listed path was never reached.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
port_lib.DAYSTART = True             # sector_replay
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 1500
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv

TBL, COUNT, SHOW, OPT, CARRY = 0x2254F0, 0x22A46E, 0x963BC, 0x1163C4, 0x9562D
LIB = port_lib.ALL_OPS & ~(1 << 43 | 1 << 63)     # the library runs connected_home and ops 63+

if not os.path.exists(DUMP):
    print(f"dayreplay_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
log, draws = [], []
case = {"events": [], "esc": [], "aesc": []}
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = ai.s16(eax)
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def fread(emu, eax, edx, ebx, ecx):
    emu.uc.mem_write(eax, bytes(case["events"].pop(0)))
    return 1


def esc(*_):
    return case["esc"].pop(0) if case["esc"] else 0


def anim(k, nargs):
    def f(emu, eax, edx, ebx, ecx):
        if emu.r8(rt(SHOW)) != 1:
            return
        regs = [eax, edx, ebx][:nargs]
        log.append(("anim", k) + tuple(x & 0xFF for x in regs))   # arguments as their low bytes
        if case["aesc"].pop(0) if case["aesc"] else 0:
            emu.w8(rt(SHOW), 0)
    return f


def draw(k, reg):
    def f(emu, eax, edx, ebx, ecx):
        log.append(("draw", k, {"eax": eax, "edx": edx, "none": 0}[reg] & 0xFF))
    return f


e.stub(rt(0x53538), rnd)
e.stub(rt(0x75EA0), lambda *_: 0x1234)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda *_: 0)
e.stub(rt(0x532BC), lambda *_: 0, 4)        # picture load (one stack argument)
e.stub(rt(0x67300), esc)
for a, k, n in ((0x64658, 0, 3), (0x64D20, 1, 3), (0x643F8, 3, 2), (0x664FC, 4, 1), (0x65FB0, 5, 3)):
    e.stub(rt(a), anim(k, n))
for a, k, reg in ((0x41C3C, 0, "none"), (0x41B24, 1, "edx"), (0x41BB8, 2, "eax"), (0x41C60, 3, "edx")):
    e.stub(rt(a), draw(k, reg))
e.stub(rt(0x52AE0), lambda *_: 0, 8)          # BlitCopy
e.stub(rt(0x522A0), lambda *_: 0)             # BlitKey (cdecl)
for a in (0x41788, 0x41640, 0x419BC):
    e.stub(rt(a), lambda *_: 0)
def bframe(emu, *_):
    """the 30 ms wait of a battle frame: the bursts (z, m, y, x arrays at the frame's esp + 0, 0xC, 0x18, 0x24)"""
    sp = emu.uc.reg_read(UC_X86_REG_ESP) + 4
    v = [emu.r32(sp + 4 * i) for i in range(12)]
    log.append(("bframe",) + tuple(v[k] | v[3 + k] << 8 | v[9 + k] << 16 | v[6 + k] << 24 for k in range(3)))


e.stub(rt(0x67250), bframe)
e.stub(rt(0x5CB34), lambda *_: 0)             # PlaySfxSlot (the battle sound: the host plays it at frame 0)

CODES = [ord(c) for c in "JKLMNOPQRSTUVWXYZ[\\]"]


def event(r):
    f = r.randrange(60)
    k = r.random()
    if k < .75:
        c = r.choice(CODES)
    elif k < .87:
        c = r.randrange(64)
    elif k < .95:
        c = r.randrange(101, 128)
    else:
        c = r.choice([0x40, 0x49, 0x5E, 0x64, 0x80, 0xFF, r.randrange(256)])
    if c in (ord("J"), ord("K"), ord("L")):
        t = r.choice([f, (f + 1) % 60, (f + 59) % 60, (f + 10) % 60, (f + 50) % 60, r.randrange(60)])
    elif c < 64:
        t = r.randrange(8)
    else:
        t = r.choice([0, 1, 2, 3, 5, 8, 0xFF, 0xFE, r.randrange(256)])
    if c == ord("M") and r.random() < .7:      # the battle uses ~300 random numbers: fewer of them
        c = ord("T")
    return [f, t, c]


def fuzz(r):
    tbl = bytearray(e.uc.mem_read(rt(TBL), 540))
    for s in range(60):
        o = 9 * s
        if r.random() < .5:
            tbl[o] = r.randrange(256) & 0x1F
            for b in range(1, 9):
                tbl[o + b] = r.choice([0, 0, 1, 2, 3, 5, 8, 9, r.randrange(256)])
    e.uc.mem_write(rt(TBL), bytes(tbl))
    n = r.choice([0, 1, 2, 5, 10, 20, 40])
    case["events"] = [event(r) for _ in range(n)]
    e.w16(rt(COUNT), n)
    e.w8(rt(OPT), r.choice([0, 1, 1, 1]))
    e.w8(rt(CARRY), r.choice([0, 0, 1, 3, r.randrange(256)]))
    p = r.choice([0, 0, .02, .1])
    case["esc"] = [1 if r.random() < p else 0 for _ in range(400)]
    case["aesc"] = [1 if r.random() < p else 0 for _ in range(200)]
    return [list(x) for x in case["events"]]


def original():
    log.clear()
    draws.clear()
    e.call(rt(0x652B8))
    return list(log), list(draws)


def port(bufs, scalars, events, dr, escq, aescq):
    q, eq, aq = list(dr), list(escq), list(aescq)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else (len(got) % n if n > 0 else 0)
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "replay_count":
            return len(events) & 0xFFFF, None
        if op == "replay_record":
            f, t, c = events[a[0]]
            return f | t << 8 | c << 16 | (eq.pop(0) if eq else 0) << 24, None
        if op == "replay_anim":
            if a[0] == 2:
                plog.append(("bframe",) + tuple(a[3:6]))
                return (eq.pop(0) if eq else 0), None
            n = {0: 3, 1: 3, 3: 2, 4: 1, 5: 3}[a[0]]
            plog.append(("anim", a[0]) + tuple(x & 0xFF for x in a[1:1 + n]))
            return (aq.pop(0) if aq else 0), None
        if op == "replay_draw":
            plog.append(("draw", a[0], a[1] & 0xFF))
            return 0, None
        raise SystemExit(f"dayreplay_diff --port: unmapped op {op} {a}")
    port_lib.sector_run("day_replay", bufs, scalars, [], random_, call, host_ops=LIB)
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
        events = fuzz(r)
        escq, aescq = list(case["esc"]), list(case["aesc"])
        pw = port_lib.world_load(e, ai) if PORT else None
        lo, dr = original()
        cover["case"] += 1
        for k in lo:
            cover[k[0] if k[0] == "bframe" else " ".join(str(x) for x in k[:2])] += 1
        if any(x[2] == ord("M") for x in events) and any(k[0] == "bframe" for k in lo):
            cover["battle"] += 1
        if escq[:len(events)].count(1) or (aescq and 1 in aescq[:5]):
            cover["escape"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pw
        lp, dp = port(bufs, scalars, events, dr, escq, aescq)
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
                print(f"MISMATCH case {i} ({len(events)} events {events[:6]}): " + "; ".join(bad)[:700])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["battle", "escape", "bframe"] + [f"anim {k}" for k in (0, 1, 3, 4, 5)] + [f"draw {k}" for k in range(4)]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"dayreplay_diff: never reached: {missing}")
        fails += 1
    print(f"dayreplay_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
