"""Differential test: the strategic screen opening (strategic.md "Strategic screen opening").

usage: .venv/bin/python validate/diff/stratopen_diff.py [ncases=1000] [DUMP] [--port] [--lib SO] [-v]
The original: StrategicScreen's opening calls run one after the other in Unicorn on tac60: 0x4660C (tapper limit),
0x43264 (canteens), DayEndMercUpdate? 0x43F54 (overnight departures), 0x445DC (collect the cash), 0x50BC4 (worker
pool). Random 0x53538 is a logged Python RNG; drawing is stubbed (InitStrategicMap 0x43ECC = op 306 map_show,
0x41C3C = op 262 map_redraw), StatusMessage? 0x27D24 logs its text, 0x5F5AC the guide quote, 0x51CB0 (stash
list) answers 1 and counts the record. Merc leaves, team close-up, cash, natives quit run for real.
Fuzzed: sector table, team (inCurSector, hp, profile), the profiles' dislike bytes +0x3A / +0x3C, +0x2F and
teamStatus, canteens, the worker-pool counters.
--port: ja1_strategic_open on the same state; random answered with the original's draws; the world, the log
(messages as text, guide quotes, map ops, stash records) and the draws are compared. Exits 1 on any mismatch.
"""
import os, random, sys
from unicorn import UC_HOOK_CODE
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP as ESP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 1000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.PAY = True                  # accounts (0xA8), sector_order, profiles, stash_counts, stat_progress, greed
port_lib.STORYDAY = True             # guide_queue (step 1, story events)
port_lib.GUIDE = True                # guides_hired, guide_wait, hard_day, story 43 bytes (step 7, the guide's advice)
VERBOSE = "-v" in argv

STEPS = (0x61DF4, 0x4660C, 0x43264, 0x43F54, 0x445DC, 0x4247C)   # 0x4247C = music, worker pool 0x50BC4, panel fields, guide 0x5F94C
SOLDIERS, PROFILES, SECTORS, STASH = 0xFA958, ai.G_PROFILES, 0x22979C, ai.G_STASH_COUNTS
GONE = b" seems to have disappeared!"

if not os.path.exists(DUMP):
    print(f"stratopen_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
e.stub_sprintf()
log, draws = [], []
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = eax & 0xFFFF
    n = n - 0x10000 if n & 0x8000 else n
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def stash(emu, eax, edx, ebx, ecx):
    log.append(("stash", eax & 0xFF, edx & 0xFF, ebx & 0xFF))
    emu.w8(rt(STASH) + (eax & 0xFF), emu.r8(rt(STASH) + (eax & 0xFF)) + 1)
    return 1


e.stub(rt(0x53538), rnd)
e.stub(rt(0x51CB0), stash)
QUEST = bytes(range(250))           # LoadQuestRecord 0x614D4 = op 307: a fixed record; fuzz() varies its pairs
quest_rec = bytearray(QUEST)


def quest(emu, eax, *_):
    log.append(("quest", eax & 0xFFFFFFFF))
    emu.uc.mem_write(rt(0x22A302), bytes(quest_rec))
    return 0


def probe(emu, eax, *_):   # the store probe 0x51A24 = op 308 stash_take: always "none in the store"
    log.append(("probe", emu.r8(eax)))
    return 0


e.stub(rt(0x614D4), quest)
e.stub(rt(0x51A24), probe)
e.stub(rt(0x43ECC), lambda *_: log.append(("map_show",)))
e.stub(rt(0x41C3C), lambda *_: log.append(("map_redraw",)))
e.stub(rt(0x27D24), lambda emu, eax, *_: log.append(("msg", emu.cstr(eax))))
e.stub(rt(0x5F5AC), lambda emu, eax, edx, *_: log.append(("guide", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF)))
offer_ans = [0]                     # the guide's hire offer 0x28C84 = op 319: the case's answer


def offer(emu, eax, edx, *_):
    log.append(("offer", edx & 0xFF, offer_ans[0]))
    return offer_ans[0]


e.stub(rt(0x5F76C), lambda emu, eax, *_: log.append(("brenda", eax & 0xFFFFFFFF)))
in_guide = [0]                      # 0x36010 = op 318 panel_draw only inside 0x5F94C (the steps before draw on their own)
pre_guide = []


def panel_fields(emu, eax, edx, ebx, ecx):
    k = (ecx & 0xFFFFFFFF, emu.r32(emu.uc.reg_read(ESP) + 4))
    if in_guide[0]:
        log.append(("panel",) + k)
    else:
        pre_guide.append(k)


e.stub(rt(0x36010), panel_fields, 4)
e.uc.hook_add(UC_HOOK_CODE, lambda uc, a, n, u: in_guide.__setitem__(0, 1), None, rt(0x5F94C), rt(0x5F94C))
e.uc.ctl_flush_tb()
e.stub(rt(0x5C5EC), lambda emu, eax, edx, ebx, ecx: log.append(("music", eax, edx, ebx)))
e.stub(rt(0x28C84), offer, 4)
e.stub(rt(0x21FC0), lambda emu, eax, *_: log.append(("join", eax & 0xFF)))
for a, pop in {0x67BF0: 0, 0x673AC: 0, 0x674B0: 0, 0x42020: 0, 0x46AC4: 0, 0x21B3C: 0, 0x532BC: 4, 0x52908: 4,
               0x467E4: 0, 0x28698: 0}.items():
    e.stub(rt(a), lambda *_: 0, pop)


def fuzz(r):
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 540))
    pf = r.choice([0, .05, .5])
    for s in range(60):
        if r.random() < pf:
            o = 9 * s
            tbl[o] = r.choice([1, 1, 0, 3, 0x1F, r.randrange(256)])
            tbl[o + 3] = r.choice([0, 3, 4, 5, 9, 30, 255, r.randrange(256)])
            tbl[o + 6] = r.choice([0, 3, 10, r.randrange(256)])
            tbl[o + 7] = r.randrange(256)
            tbl[o + 8] = r.choice([0, 0, 1, 5, r.randrange(256)])
    tbl[59 * 9] |= 1
    for i in range(25):
        quest_rec[10 * i:10 * i + 2] = r.randrange(60).to_bytes(2, "little")
        quest_rec[10 * i + 6:10 * i + 8] = r.randrange(60).to_bytes(2, "little")
    st = rt(0x22A040)
    e.w8(st + 0x0A, r.choice([0, 1, 1, 2]))
    e.w8(st + 0x20, r.choice([0, 0, 1]))
    e.w32(rt(0x229FA0) + 0x74, r.choice([10, 30, 40]))
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    big = r.random() < .3   # canteen stacks above 5 (bytes past the place): no departures (Merc leaves reads past its copy)
    nteam = r.randrange(9)
    team = [r.randrange(60) for _ in range(8)]
    for k in range(8):
        s = rt(SOLDIERS) + 0x200 * k
        e.w8(s + 0x182, 1 if k < nteam else r.choice([0, 0, 0, 1]))
        e.w8(s + 0x183, r.choice([0, 30, 30, r.randrange(256)]))
        e.w8(s + 0x179, team[k])
        e.w8(s + 0x17A, k)
        for j in range(12):
            p = s + 12 * j
            if r.random() < .15:
                e.w8(p, 0x41)
                c = r.choice([0, 1, 2, 3, 5, 0x80, r.randrange(256) if big else r.choice([4, 0x90])])
                if c < 0x80 and 12 * j + c + 6 >= 0x90:   # +0x90 is a pointer in the original, an offset in the port
                    c = 0x90 - 12 * j - 7
                e.w8(p + 1, c)
            elif e.r8(p + 1) > 5 and e.r8(p + 1) < 0x80:   # stacks above 5 read past the item copy (Merc leaves)
                e.w8(p + 1, 5)
    for p in range(64):
        a = rt(PROFILES) + ai.PROFILE_SIZE * p
        e.w8(a + 0x4B, r.choice([0, 1, 2, 49, 50, 51, 100, r.randrange(256)]))
        if r.random() < .5:
            d = r.choice(team + [99, r.randrange(64)])
            e.w8(a + 0x3A, d)
            e.w8(a + 0x3C, r.choice([d, d, 99, r.choice(team), r.randrange(256)]))
            e.w8(a + 0x2F, r.choice([3, 3, 0, 1, 2, r.randrange(256)]))
        if big:
            e.w8(a + 0x3A, 99)


def original():
    log.clear()
    draws.clear()
    for a in STEPS:
        in_guide[0] = 0
        pre_guide.clear()
        e.call(rt(a))
    in_guide[0] = 0
    # 0x4247C on the original side only: SetMusic first (the port: dayloop::play(2)), then the worker pool, the field draw (map_diff).
    if ("music", 2, 0, 0x140) not in log or log.count(("music", 2, 0, 0x140)) != 1:
        raise SystemExit("stratopen_diff: 0x4247C music calls")
    if not pre_guide or pre_guide[-1] != (0, 14):
        raise SystemExit(f"stratopen_diff: 0x4247C panel fields {pre_guide}")
    kept = [k for k in log if k != ("music", 2, 0, 0x140)]
    return kept, list(draws)


def port(bufs, scalars, dr):
    q = list(dr)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else 0
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "guide_say":
            plog.append(("guide", a[0], a[1]))
        elif op == "merc_gone":
            p = bufs["profiles"][ai.PROFILE_SIZE * a[0]:]
            plog.append(("msg", bytes(p[:p.index(0)]) + GONE))
        elif op == "natives_quit_msg":
            n = a[0] - (1 << 32) if a[0] & 0x80000000 else a[0]
            plog.append(("msg", b"%d %s quit this morning!" % (n, (b"guard", b"guards", b"tapper", b"tappers")[a[1]])))
        elif op == "quest_record":
            plog.append(("quest", a[0]))
            return 0, bytes(quest_rec)
        elif op == "stash_take":
            plog.append(("probe", a[0] & 0xFF))
            return 0, None
        elif op in ("map_show", "map_redraw"):
            plog.append((op,))
        elif op == "stash_put":
            plog.append(("stash", a[0] & 0xFF, a[1] & 0xFF, a[2] & 0xFF))
            return 1, None
        elif op == "brenda_say":
            plog.append(("brenda", a[0]))
        elif op == "panel_draw":
            plog.append(("panel", a[0], a[1]))
        elif op == "guide_offer":
            plog.append(("offer", a[0] & 0xFF, offer_ans[0]))
            return offer_ans[0], None
        elif op == "merc_join":
            plog.append(("join", a[0] & 0xFF))
        elif op == "team_redraw":
            # strategic.md: the opening closes up the team with redraw 0 only, so op 266 never comes on the
            # original's side; logged here so that one from the port is a mismatch
            plog.append(("team_redraw",))
        elif op not in ("face_clear", "face_move"):
            raise SystemExit(f"stratopen_diff --port: unmapped op {op} {a}")
        return 0, None
    port_lib.sector_run("strategic_open", bufs, scalars, [], random_, call)
    return plog, got


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        offer_ans[0] = i % 3 == 1        # the hire offer (when asked): yes in a third of the cases
        pre = port_lib.world_load(e, ai) if PORT else None
        lo, dr = original()
        for k in lo:
            cover[k[0] if k[0] != "msg" else "gone" if k[1].endswith(GONE) else "quit"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp, got = port(bufs, scalars, dr)
        bad = []
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        if got != dr:
            bad.append(f"draws {dr} vs {got}")
        for b in (bufs_o, bufs):
            b["accounts"][0xC:0x10] = bytes(4)          # team_on_duty, cur_sector: compared as scalars
            b["accounts"][0x99] = 0
            b["text_buf"][:] = bytes(len(b["text_buf"]))   # sprintf scratch of the original: host text
            b["item_buf"][:] = bytes(12)   # 0x222B68: the store probe's argument in the original
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
        print("cover:", dict(cover))
    missing = [k for k in ("map_show", "guide", "gone", "stash", "quest") if not cover[k]]
    if missing:
        print(f"stratopen_diff: never reached: {missing}")
        fails += 1
    print(f"stratopen_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
