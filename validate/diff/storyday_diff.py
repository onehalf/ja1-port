"""Differential test: the story events at the day start (strategic.md "Story events at the day start").

usage: .venv/bin/python validate/diff/storyday_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original: StoryDayEvents? 0x61DF4 runs in Unicorn on tac60 (Blight start 0x61FF4, Blight spread 0x620EC,
FindSquadItem? 0x61528 and SectorWorkable run for real). LoadQuestRecord 0x614D4 = host op 307 quest_record: writes
250 fuzzed bytes to 0x22A302 and logs n. The store probe 0x51A24 = host op 308 stash_take: logs (type, take),
answers a logged random 0 / 1; on yes with take, stash_counts[type] - 1 (what the store does).
--fan (with --port; #70 F40): the original side runs the 1.15m StoryDayEvents? sites (Emu.put_115m), the port runs
with fix F40; story 0x06 / 0x09 / 0x0B and sector 27 get extra fuzz from their own RNG (the plain stream stays).
Fuzzed: story bytes 0x0A 0x0B 0x0E 0x15 0x1E 0x20..0x22, guide_queue, sectors owned (accounts +0x74), difficulty
0..2, sector table (flags, trees, unhealthy), sector records +0xAB..+0xAE, squad items (type 0x51), inCurSector,
the quest record (sector pairs 0..59 with some out of a valid state).
--port: ja1_story_day on the same state; the world and the host log are compared. Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from common import game_read          # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.PAY = True                  # accounts, stash_counts
port_lib.STORYDAY = True             # guide_queue
VERBOSE = "-v" in argv
FAN = "--fan" in argv
FAN_RNG = random.Random(40)
if FAN and not PORT:
    raise SystemExit("storyday_diff: --fan needs --port")

SOLDIERS, SECTORS, RECORDS, STASH = 0xFA958, ai.G_SECTOR_TBL9, ai.G_SECTORS, ai.G_STASH_COUNTS
STORY, QUEUE, QUEST, ACC = 0x22A040, 0x22A4C9, 0x22A302, 0x229FA0

if not os.path.exists(DUMP):
    print(f"storyday_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
if FAN:
    e.put_115m(only=[(0x61E38, 0x61E3E), (0x61EA7, 0x61EAD), (0x61F2B, 0x61F31)])
port_lib.fan_fixes((1 << 39) if FAN else 0)
log = []
case = {"quest": b"", "answers": []}


QUESTS = game_read("DAT/QUESTS.DAT")    # the 1.13b file (port target, #24): a case with "real" uses its records


def quest_bytes(n):
    """the record op 307 gives for n: the fuzzed one, or in a "real" case QUESTS.DAT record n - 5 (zeros outside)"""
    if not case.get("real"):
        return case["quest"]
    return QUESTS[250 * (n - 5):250 * (n - 4)] if 5 <= n <= 7 else bytes(250)


def quest(emu, eax, *_):
    log.append(("quest", eax & 0xFFFFFFFF))
    emu.uc.mem_write(rt(QUEST), quest_bytes(eax & 0xFFFFFFFF))
    return 0


def probe(emu, eax, edx, ebx, ecx):
    t = emu.r8(eax)
    take = 1 if ecx & 0xFFFFFFFF else 0
    a = case["answers"].pop(0) if case["answers"] else 0
    log.append(("stash", t, take, a))
    if a and take:
        emu.w8(rt(STASH) + t, emu.r8(rt(STASH) + t) - 1)
    return a


e.stub(rt(0x614D4), quest)
e.stub(rt(0x51A24), probe)


def fuzz(r):
    st = bytearray(e.uc.mem_read(rt(STORY), 35))
    st[0x0A] = r.choice([0, 0, 1, 1, 1, 2, r.randrange(256)])
    st[0x0B] = r.choice([0, 1, 1, 2, r.randrange(256)])
    st[0x0E] = r.choice([0, 1, 2, 3, r.randrange(256)])
    st[0x15] = r.choice([0, 0, 1, r.randrange(256)])
    st[0x1E] = r.choice([0, 0, 1, r.randrange(256)])
    st[0x20] = r.choice([0, 0, 1, r.randrange(256)])
    st[0x21] = r.choice([0, 1, 1, r.randrange(256)])
    st[0x22] = r.choice([0, 1, 2, 3, 0x80, 0x81, r.randrange(256)])
    e.uc.mem_write(rt(STORY), bytes(st))
    e.uc.mem_write(rt(QUEUE), bytes(r.randrange(256) for _ in range(7)))
    e.w32(rt(ACC) + 0x74, r.choice([29, 30, 31, 45, 60, -5, r.randrange(1 << 32)]) & 0xFFFFFFFF)
    e.w8(rt(ai.G_DIFFICULTY), r.randrange(3))
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 720))
    pt = r.choice([.1, .5, .9])
    for s in range(60):
        o = 9 * s
        if r.random() < .7:
            tbl[o] = r.choice([1, 1, 1, 0, 3, r.randrange(256)])
        if r.random() < pt:
            tbl[o + 6] = r.choice([0, 1, 3, 5, 10, r.randrange(256)])
            tbl[o + 8] = r.choice([0, 0, 0, 1, 2, tbl[o + 6], r.randrange(256)])
            tbl[o + 7] = r.randrange(256)
    for s in range(60, 80):   # entries past the map: a neighbour test that reaches them reads them
        tbl[9 * s:9 * s + 9] = bytes([r.choice([0, 1]), 0, 0, 0, 0, 0, r.choice([0, 5]), 0, 0])
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    for s in range(60):
        a = rt(RECORDS) + 0xB1 * s
        for b in (0xAB, 0xAC, 0xAD, 0xAE):
            e.w8(a + b, r.choice([0, 0, 1, 2, 3, 4, r.randrange(256)]))
    for k in range(8):
        sl = rt(SOLDIERS) + 0x200 * k
        e.w8(sl + 0x182, r.choice([0, 1, 1]))
        for j in range(12):
            if r.random() < .03:
                e.w8(sl + 12 * j, 0x51)
            elif e.r8(sl + 12 * j) == 0x51 and r.random() < .5:
                e.w8(sl + 12 * j, 0)
    q = bytearray(r.randrange(256) for _ in range(250))
    for i in range(25):
        for o in (0, 6):
            v = r.randrange(60) if r.random() < .97 else r.choice([60, 70, 79])
            q[10 * i + o:10 * i + o + 2] = v.to_bytes(2, "little")
    case["quest"] = bytes(q)
    case["real"] = r.random() < .25
    case["answers"] = [r.choice([0, 1, 1, 2]) for _ in range(4)]
    if FAN:   # F40: the kidnapping test (story 0x06 = 2, 0x09 = 2, 0x0B < 1, sector 27 not owned)
        f = FAN_RNG
        for b, vals in ((0x06, [2, 2, 2, 0, 1]), (0x09, [2, 2, 2, 1, 3]), (0x0B, [0, 0, 0xFF, 1, 2])):
            e.w8(rt(STORY) + b, f.choice(vals))
        if f.random() < .6:
            e.w8(rt(SECTORS) + 9 * 27, e.r8(rt(SECTORS) + 9 * 27) & 0xFE)


def original():
    log.clear()
    e.call(rt(0x61DF4))
    return list(log)


def port(bufs, scalars, answers):
    plog = []
    q = list(answers)

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "quest_record":
            plog.append(("quest", a[0]))
            return 0, quest_bytes(a[0])
        if op == "stash_take":
            v = q.pop(0) if q else 0
            plog.append(("stash", a[0], 1 if a[1] else 0, v))
            return v, None
        raise SystemExit(f"storyday_diff --port: unmapped op {op} {a}")
    port_lib.sector_run("story_day", bufs, scalars, [], lambda n: 0, call)
    return plog


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        answers = list(case["answers"])
        pre = port_lib.world_load(e, ai) if PORT else None
        st0 = bytes(e.uc.mem_read(rt(STORY), 35))
        lo = original()
        st1 = bytes(e.uc.mem_read(rt(STORY), 35))
        cover[f"stage {st0[0x0A] if st0[0x0A] < 3 else 'x'}->{st1[0x0A] if st1[0x0A] < 3 else 'x'}"] += 1
        for k in lo:
            cover[k[0]] += 1
            if k == ("quest", 6):
                cover["quest 6"] += 1   # F40 kidnapping test
        if st1[0x20] and not st0[0x20]:
            cover["cured"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp = port(bufs, scalars, answers)
        bad = []
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        for b in (bufs_o, bufs):
            b["item_buf"][:] = bytes(12)   # 0x222B68: the original's probe argument (the store probe is the host)
            b["story"][:8] = bytes(8)   # views of one byte in two fields: story +0..7 = accounts +0xA0..0xA7
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
                print(f"MISMATCH case {i}: " + "; ".join(bad)[:500])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    missing = [k for k in ("quest", "stash", "stage 0->1", "stage 1->2", "stage 1->1", "cured") + (("quest 6",) if FAN else ())
               if not cover[k]]
    if missing:
        print(f"storyday_diff: never reached: {missing}")
        fails += 1
    print(f"storyday_diff{' (port)' if PORT else ''}{' --fan 40' if FAN else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
