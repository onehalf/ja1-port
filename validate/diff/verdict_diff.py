"""Differential test: the day's verdict (strategic.md "The day's verdict").

usage: .venv/bin/python validate/diff/verdict_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
First the day-end accounts (strategic.md "The day-end accounts"): ReplayScreenInit? 0x642EE..0x64346 (death totals,
CalcProcessCap, RecalcTerritory, ApplyDayLedger; a stub at 0x64346 returns), 0x455B4 (rest of the team) and 0x65AB0
(tappers on lost trees). Then DayEndVerdict 0x60680 runs in Unicorn on tac60, then (result 0 only) the bonus 0x615DC. The crate check
0x62698, CrateLost 0x62780, DayScore 0x61264, 0x61488, FindSquadItem? 0x61528, DropItem 0x470B0, CalcProcessCap and
SectorConnectedToHome run for real. Host ops: guide_say 0x5F5AC (302), BrendaSpeech? 0x5F76C (317), the store probe
0x51A24 (308 stash_take: a logged random answer), StatusMessage 0x27D24 (330 home_lost_msg) and DrawText 0x358D0 of
the rating word (331 rating_word: score = (text - 0x965D6) / 12 - 3). Random 0x53538 is a logged Python RNG.
Fuzzed: story bytes, item_told, the day accounts and day-start copies, deaths, streak, plants, crate states and the
day (crate days 9, 17, 26, 31), the sector table, the five bytes R(S[0x17], 0x9F..0xA3), soldiers 0..7 (present,
story items in pockets), difficulty 0..2, idle_day, team_on_duty, bonus.
After the bonus: the day-end steps (DayEndScreen? 0x65F6C..0x65F88: 0x62304 and the first_hit count), "end" = its result.
--port: ja1_day_end_accounts, ja1_day_verdict (+ ja1_day_bonus + ja1_day_end) on the same state; result, host log and world are compared. Exits 1 on any
mismatch.
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
port_lib.PAY = True                  # accounts, stash_counts
port_lib.VERDICT = True              # story 43, item_told, start_*, bonus, idle_day
VERBOSE = "-v" in argv

SOLDIERS, SECTORS, RECORDS = 0xFA958, ai.G_SECTOR_TBL9, ai.G_SECTORS
STORY, ACC, PLANTS, STASH = 0x22A040, 0x229FA0, ai.G_PLANTS, ai.G_STASH_COUNTS
CRATE_STATE, CRATE_DAYS = 0x22A06B, [9, 17, 26, 31]
CAPS = [0, 20, 21, 40, 45, 50, 55, 60, 75, 80, 85, 100, 110, 120]
ITEMS = [0x86, 0x7A, 0x5C, 0x79, 0x5E, 0x51, 0x6E, 0x7C, 0x77]

if not os.path.exists(DUMP):
    print(f"verdict_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
log, draws = [], []
case = {"answers": []}
rng = random.Random(7)


def rnd(emu, eax, *_):
    n = eax & 0xFFFF
    n = n - 0x10000 if n & 0x8000 else n
    v = rng.randrange(n) if n > 0 else 0
    draws.append((n, v))
    return v


def say(emu, eax, edx, *_):
    log.append(("say", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF))


def brenda(emu, eax, *_):
    log.append(("brenda", eax & 0xFFFFFFFF))


def probe(emu, eax, edx, ebx, ecx):
    t = emu.r8(eax)
    take = 1 if ecx & 0xFFFFFFFF else 0
    a = case["answers"].pop(0) if case["answers"] else 0
    log.append(("stash", t, take, a))
    if a and take:
        emu.w8(rt(STASH) + t, (emu.r8(rt(STASH) + t) - 1) & 0xFF)
    return a


def home_lost(emu, eax, *_):
    log.append(("home_lost",))


def draw_text(emu, eax, *_):
    log.append(("rating", ((eax & 0xFFFFFFFF) - rt(0x965D6)) // 12 - 3))


e.stub(rt(0x53538), rnd)
e.stub(rt(0x5F5AC), say)
e.stub(rt(0x5F76C), brenda)
e.stub(rt(0x51A24), probe)
e.stub(rt(0x27D24), home_lost)
e.stub(rt(0x358D0), draw_text, 0x18)
e.stub(rt(0x65F89), lambda *_: None)
e.stub(rt(0x64346), lambda *_: None)


def i16(r, lo, hi):
    return r.randint(lo, hi) & 0xFFFFFFFF


def fuzz(r):
    st = bytearray(e.uc.mem_read(rt(STORY), 43))
    for i in (2, 5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xE):
        st[i] = r.choice([0, 0, 1, 1, 1, 2, r.randrange(256)])
    st[0xF] = r.choice([0, 0, 1, 2, 3, r.randrange(256)])
    for i in (0x12, 0x13, 0x15, 0x16, 0x1E, 0x20, 0x21):
        st[i] = r.choice([0, 0, 1, r.randrange(256)])
    st[0x14] = r.choice([0, 1, 4, 5, 6, 7, 8, 9, 10, 11, r.randrange(256)])
    st[0x17] = r.randrange(60) if r.random() < .95 else r.randrange(256)
    st[0x18] = r.choice([0, 1, 2, 3, r.randrange(256)])
    st[0x22] = r.choice([0, 1, 2, 3, 0x80, r.randrange(256)])
    st[0x24] = r.randrange(256)
    if r.random() < .05:            # the ransom paid with the cure at hand (Brenda 0xD / 0xF)
        st[0xB], st[0x16], st[0x15], st[0xA], st[0x20], st[0x21] = 1, 1, 0, 1, 0, 0
        st[0xE], st[0x1E] = r.choice([(2, 0), (1, 0), (2, 1)])
    e.uc.mem_write(rt(STORY), bytes(st))
    e.uc.mem_write(rt(0x22A1F6), bytes(r.choice([0, 0, 1]) for _ in range(3)))
    acc = rt(ACC)
    base = {0x18: r.randrange(400), 0x14: r.randrange(300), 0x1C: r.randrange(120), 0x74: r.randrange(60),
            0x88: r.randrange(5), 0x84: r.choice(CAPS)}
    snap = {0x18: 0x229AC4, 0x14: 0x229ACC, 0x1C: 0x229AD4, 0x74: 0x229AD8, 0x88: 0x229AF8, 0x84: 0x229AF4}
    for o, v in base.items():
        e.w32(acc + o, v)
        dv = r.choice([0, 0, r.randint(-3, 3), r.randint(-20, 20), r.randint(-100, 100)])
        if o == 0x84:
            dv = r.choice([0, 0, -5, 5, r.randint(-60, 60)])
        e.w32(rt(snap[o]), (v - dv) & 0xFFFFFFFF)
    if r.random() < .02:
        e.w32(acc + 0x18, r.randrange(1 << 32))
    for a in (ai.G_DEAD_MERCS, ai.G_DEAD_ALLIES, ai.G_DEAD_NPC, ai.G_ENEMY_DEATHS):
        e.w32(rt(a), r.choice([0, 0, 0, 1, 2, 3, 4, 5, 6, 15, 16, 30]) if r.random() < .97 else r.randrange(1 << 32))
    e.w8(acc + 0x9B, r.choice([0, 0, 1, 2, 3, 4, 5, 6, r.randrange(256)]))
    e.w8(acc + 0x9A, r.randrange(256))
    e.w8(acc + 0x90, r.randrange(256))
    for o in (0x92, 0x94):
        e.uc.mem_write(acc + o, r.randrange(1 << 16).to_bytes(2, "little"))
    for o in (0x20, 0x24, 0x2C, 0x64):
        e.w32(acc + o, r.randrange(-50000, 200000) & 0xFFFFFFFF)
    e.w32(rt(ai.G_PREV_BALANCE), r.randrange(-50000, 200000) & 0xFFFFFFFF)
    e.w32(rt(0x229FAC), r.choice([0, 0, 1, 3]))
    day = r.choice(CRATE_DAYS + [1, 1, 2, 3, 4, 8, 10, 12, 13, 49, 50, 100, 250, 251, 300])
    e.w32(rt(ai.G_DAY), day)
    e.w8(rt(ai.G_DIFFICULTY), r.randrange(3))
    e.w8(rt(0x22A10A), r.choice([0, 0, 1]))
    e.uc.mem_write(rt(CRATE_STATE), bytes(r.choice([0, 0, 1, 1, 2, 3, 4, 5]) for _ in range(4)))
    e.w8(rt(0x92DD6), r.choice([0, 0, 1]))
    e.uc.mem_write(rt(0x92DD8), r.choice([0, 0, 1000, r.randrange(1 << 16)]).to_bytes(2, "little"))
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 540))
    po = r.choice([.3, .7, .95, 1.0])
    for s in range(60):
        o = 9 * s
        tbl[o] = (tbl[o] & 0xFE) | (1 if r.random() < po else 0)
        if r.random() < .3:
            tbl[o + 6] = r.choice([0, 0, 5, 20])
        if r.random() < .3:
            tbl[o + 8] = r.choice([0, 0, 0, 1])
        if r.random() < .5:            # tappers (byte 3) against 3 x workable (byte 7)
            tbl[o + 3] = r.choice([0, 1, 5, 20, 60, 90, r.randrange(256)])
        if r.random() < .3:
            tbl[o + 7] = r.choice([0, 1, 2, 30, 90, r.randrange(256)])
    tbl[9 * 0x3B] |= 1
    if r.random() < .1:
        tbl[9 * 0x3B] &= 0xFE
    if r.random() < .1:
        for s in range(60):
            if tbl[9 * s] & 1:
                tbl[9 * s + 6] = 0
    if r.random() < .3:
        for s in range(60):
            tbl[9 * s + 8] = 0
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    n = st[0x17] if st[0x17] < 60 else 0
    rec = rt(RECORDS) + 0xB1 * n + 0x9F
    e.uc.mem_write(rec, bytes(r.choice([0xAE, 0xAF, 0xAC, 0, r.randrange(256)]) for _ in range(5)))
    for i in range(4):
        p = rt(PLANTS) + 12 * i
        e.w32(p, r.choice([0, 10, 25, 35, 45]))
        e.w32(p + 4, r.randrange(60))
        e.w8(p + 8, r.choice([0, 1, 2, 3, 3, r.randrange(256)]))
        e.w8(p + 0xB, r.randrange(256))
    for o in (0x00, 0x50, 0x6C):    # the death totals (strategic.md "The day-end accounts")
        e.w32(acc + o, r.choice([0, 3, 40, r.randrange(1 << 32)]))
    for k in range(8):
        sl = rt(SOLDIERS) + 0x200 * k
        e.w8(sl + 0x182, r.choice([0, 1, 1, 1]))
        e.w8(sl + 0x185, r.choice([0, 3, r.randrange(256)]))
        e.w8(sl + 0x1B1, r.choice([0, 0, 1, 2, 0xFF, r.randrange(256)]))
        e.w8(sl + 0x18A, r.randrange(256))
        e.uc.mem_write(sl + 0x10C, r.randrange(1 << 16).to_bytes(2, "little"))
        for j in range(12):
            if r.random() < .06:
                e.w8(sl + 12 * j, r.choice(ITEMS))
            elif e.r8(sl + 12 * j) in ITEMS and r.random() < .5:
                e.w8(sl + 12 * j, 0)
    case["answers"] = [r.choice([0, 0, 1]) for _ in range(8)]
    # strategic.md "The day-end screen", day-end steps 1 and 2: the balance sign, first_hit, accounts +0x91.
    e.w8(rt(0x22A1F5), r.choice([0, 1, 1, 2, 2, 3, 0xFF, r.randrange(256)]))
    e.w8(rt(ACC + 0x91), r.choice([0, 1, 5, 0xFF, r.randrange(256)]))
    if r.random() < .5:
        e.w32(rt(ACC + 0x24), r.choice([-1, -5000, -(1 << 31), 0, 1, r.randrange(1 << 32)]) & 0xFFFFFFFF)


def original():
    log.clear()
    draws.clear()
    e.call(rt(0x642EE))
    e.call(rt(0x455B4))
    e.call(rt(0x65AB0))
    res = e.call(rt(0x60680)) & 0xFF
    log.append(("result", res))
    if res == 0:
        e.call(rt(0x615DC))
        # DayEndScreen? 0x65F6C..0x65F88: 0x62304 (result -> end code byte 0x93449), first_hit < 2 -> inc 0x22A031;
        # the stub at 0x65F89 (the fade) returns to the sentinel.
        keep = e.r8(rt(0x93449))
        e.call(rt(0x65F6C))
        log.append(("end", e.r8(rt(0x93449))))
        e.w8(rt(0x93449), keep)
    return list(log), list(draws)


def port(bufs, scalars, dr, answers):
    q, a_ = list(dr), list(answers)
    got, plog = [], []

    def random_(n):
        v = q.pop(0)[1] if q else 0
        got.append((n, v))
        return v

    def call(op, a):
        a = [x & 0xFFFFFFFF for x in a]
        if op == "guide_say":
            plog.append(("say", a[0], a[1]))
        elif op == "brenda_say":
            plog.append(("brenda", a[0]))
        elif op == "stash_take":
            v = a_.pop(0) if a_ else 0
            plog.append(("stash", a[0] & 0xFF, 1 if a[1] else 0, v))
            return v, None
        elif op == "home_lost_msg":
            plog.append(("home_lost",))
        elif op == "rating_word":
            plog.append(("rating", a[0] - (1 << 32) if a[0] & 0x80000000 else a[0]))
        else:
            raise SystemExit(f"verdict_diff --port: unmapped op {op} {a}")
        return 0, None
    port_lib.sector_run("day_end_accounts", bufs, scalars, [], random_, call, sync=True)
    res = port_lib.sector_run("day_verdict", bufs, scalars, [], random_, call, sync=True) & 0xFF
    plog.append(("result", res))
    if res == 0:
        port_lib.sector_run("day_bonus", bufs, scalars, [], random_, call, sync=True)
        plog.append(("end", port_lib.sector_run("day_end", bufs, scalars, [], random_, call, sync=True) & 0xFF))
    return plog, got


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
        lo, dr = original()
        for k in lo:
            cover[k[0]] += 1
            if k[0] in ("say", "brenda", "result", "rating", "end"):
                cover[f"{k[0]} {k[1]:#x}" if k[0] != "rating" else f"rating {k[1]}"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp, got = port(bufs, scalars, dr, answers)
        bad = []
        if lo != lp:
            j = next((j for j in range(min(len(lo), len(lp))) if lo[j] != lp[j]), min(len(lo), len(lp)))
            bad.append(f"log from #{j}: {lo[j:j + 4]} vs {lp[j:j + 4]}")
        if [n for n, _ in dr] != [n for n, _ in got]:
            bad.append(f"random {dr} vs {got}")
        for b in (bufs_o, bufs):        # views of one byte in two fields (the scalar / the story byte is compared):
            b["accounts"][0xA0:0xA8] = bytes(8)     # accounts +0xA0..0xA7 = story +0..7
            b["accounts"][0xC:0x10] = bytes(4)      # accounts +0xC = team_on_duty
            b["accounts"][0x90] = 0                 # accounts +0x90 = guard_skill
            b["item_buf"][:] = bytes(12)            # 0x222B68: the original's probe argument (the store probe is the host)
            b["story"][0x19] = 0                    # story +0x19 = merc_tire
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
                print(f"MISMATCH case {i}: " + "; ".join(bad)[:600])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["end 0x0", "end 0x3", "say 0xb3", "say 0xb4", "home_lost", "stash", "result 0x2", "result 0x3", "result 0x4", "result 0x0"]
    need += [f"rating {v}" for v in range(-3, 4)]
    need += [f"brenda {q:#x}" for q in (0xB, 0xD, 0xE, 0xF, 0x10, 0x13, 0x14)]
    need += [f"say {q:#x}" for q in [0x94, 0x95, 0xB2, 0xAB, 0xA0, 0x9F, 0xA5, 0xAC, 0xAD, 0xAE, 0x26, 0x2D, 0x34,
                                     0x3B, 0x42, 0x49, 0x4A, 0x4B, 0x46, 0x47, 0x48, 0x91, 0xAF, 0xB0, 0xB1, 0xB7,
                                     0xB8, 0x50, 0x5F, 0x61, 0x66, 0x9D, 0x92, 0x9B, 0x9E, 0xB6, 0xB5, 0x9A, 0x93,
                                     0x99, 0x9C]]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"verdict_diff: never reached: {missing}")
        fails += 1
    print(f"verdict_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
