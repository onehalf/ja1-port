"""Differential test: the performance rating and the win variables (ending.md "The performance rating", "Game end"
win sequence step 1).

usage: .venv/bin/python validate/diff/rating_diff.py [ncases=2000] [DUMP] [--port] [--lib SO] [-v]
The original: PerformanceRating? 0x62EB4 runs in Unicorn on tac60 (FindSquadItem? 0x61528, CalcLedgerAll 0x51178 and
sprintf_ run for real); its result (the score, low byte), the text in g_textBuf 0xEBAC0 and the world after it are
kept. The win variables: WinGameSequence? 0x5F2F8 runs up to 0x5F3A7 (after v3, before
the music calls; v0..v3 at 0x337798). Fuzzed: story bytes, the sector table (owned flag, trees, sector 0x1B), plants, the accounts
inputs (deaths, streak, success points, sectors gained / lost, balance, day), soldiers 0..7 (present, items 0x5F /
0x7B in pockets), leave_reason 0..5.
--port: ja1_performance_rating and ja1_win_vars on the same state; score, text, v0..v3 and world are compared.
Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu, STACK_TOP, RET_SENTINEL   # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 2000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.PAY = True                  # accounts, stash_counts
port_lib.VERDICT = True              # story 43
VERBOSE = "-v" in argv

SOLDIERS, SECTORS, PLANTS = 0xFA958, ai.G_SECTOR_TBL9, ai.G_PLANTS
STORY, ACC, TEXT, VARS = 0x22A040, 0x229FA0, 0xEBAC0, 0x337798
WORDS = ["TERRIBLE", "LOUSY", "POOR", "FAIR", "GOOD", "EXCELLENT", "OUTSTANDING"]

if not os.path.exists(DUMP):
    print(f"rating_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
e.stub_sprintf()


def run_until(start, stop):
    """the original from `start` until it reaches `stop` (the win sequence's variables, before its music call)"""
    sp = STACK_TOP - 0x100
    e.w32(sp, RET_SENTINEL)
    e.uc.reg_write(UC_X86_REG_ESP, sp)
    e.uc.emu_start(rt(start), rt(stop), timeout=2_000_000)


def fuzz(r):
    st = bytearray(e.uc.mem_read(rt(STORY), 43))
    for i in (5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xE):
        st[i] = r.choice([0, 0, 1, 1, 2, 2, r.randrange(256)])
    for i in (0x15, 0x1E):
        st[i] = r.choice([0, 0, 0, 1, r.randrange(256)])
    st[0x18] = r.randrange(256)
    st[0x24] = r.randrange(256)
    e.uc.mem_write(rt(STORY), bytes(st))
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 540))
    po = r.choice([.1, .5, .9, 1.0])
    for s in range(60):
        o = 9 * s
        tbl[o] = (tbl[o] & 0xFE) | (1 if r.random() < po else 0)
        if r.random() < .4:
            tbl[o + 6] = r.choice([0, 0, 5, 20, 60, r.randrange(256)])
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    for i in range(4):
        p = rt(PLANTS) + 12 * i
        e.w32(p, r.choice([0, 10, 25, 35, 45]))
        e.w32(p + 4, r.randrange(60))
        e.w8(p + 8, r.choice([0, 1, 2, 3, 3, r.randrange(256)]))
    acc = rt(ACC)
    big = lambda: r.randrange(1 << 32)
    e.uc.mem_write(acc, r.choice([0, 0, 1, 3, r.randrange(1 << 16)]).to_bytes(2, "little"))
    e.w32(acc + 4, r.choice([1, 2, 3, 4, 10, 49, 50, 51, 100, 300, big()]))
    e.w32(acc + 0x24, r.choice([-60000, -5000, 0, 4999, 5000, 12345, 49999, 50000, 50001, 200000, big()]) & 0xFFFFFFFF)
    for o in (0x50, 0x6C):
        e.w32(acc + o, r.choice([0, 3, 9, 10, 17, 80, -9 & 0xFFFFFFFF, -17 & 0xFFFFFFFF, big()]))
    for o in (0x78, 0x7C):
        e.uc.mem_write(acc + o, r.choice([0, 1, 4, 12, r.randrange(1 << 16)]).to_bytes(2, "little"))
    for o in (0x9A, 0x9B, 0xA0):
        e.w8(acc + o, r.choice([0, 1, 3, 9, 10, r.randrange(256)]))
    e.w8(rt(0x93449), r.choice([0, 1, 1, 2, 3, 4, 5, r.randrange(256)]))
    for k in range(8):
        sl = rt(SOLDIERS) + 0x200 * k
        e.w8(sl + 0x182, r.choice([0, 1, 1, 1]))
        for j in range(12):
            if r.random() < .04:
                e.w8(sl + 12 * j, r.choice([0x5F, 0x7B]))
            elif e.r8(sl + 12 * j) in (0x5F, 0x7B) and r.random() < .5:
                e.w8(sl + 12 * j, 0)


def text():
    b = bytes(e.uc.mem_read(rt(TEXT), 80))
    return b[:b.index(0)].decode("latin-1")


def original():
    e.uc.mem_write(rt(VARS), bytes(8))
    run_until(0x5F2F8, 0x5F3A7)
    v = [int.from_bytes(e.uc.mem_read(rt(VARS) + 2 * i, 2), "little", signed=True) for i in range(4)]
    res = e.call(rt(0x62EB4)) & 0xFF
    return res, text(), v


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        pre = port_lib.world_load(e, ai) if PORT else None
        res, txt, v = original()
        word = txt.split(" ")[-2] if txt.startswith("Your overall performance rating is ") else "?"
        cover[f"word {word}"] += 1
        cover[f"score {'0' if res == 0 else '100' if res == 100 else 'mid'}"] += 1
        for k in range(4):
            cover[f"v{k} {v[k]}"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        out = bytearray(8)
        port_lib.sector_run("win_vars", bufs, scalars, [out], None, None, sync=True)
        pv = [int.from_bytes(out[2 * k:2 * k + 2], "little", signed=True) for k in range(4)]
        ps = port_lib.sector_run("performance_rating", bufs, scalars, [], None, None, sync=True)
        bad = []
        if ps != res:
            bad.append(f"score {res} vs {ps}")
        want = f"Your overall performance rating is {WORDS[[30, 50, 60, 70, 80, 90, 1 << 30].index(next(t for t in [30, 50, 60, 70, 80, 90, 1 << 30] if ps < t))]} ({ps}%)"
        if txt != want:
            bad.append(f"text {txt!r} vs {want!r}")
        if pv != v:
            bad.append(f"vars {v} vs {pv}")
        for b in (bufs_o, bufs):
            b["accounts"][0xA0:0xA8] = bytes(8)     # accounts +0xA0..0xA7 = story +0..7
            b["accounts"][0xC:0x10] = bytes(4)      # accounts +0xC = team_on_duty
            b["text_buf"][:] = bytes(len(b["text_buf"]))   # the rating text is compared above
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
    need = [f"word {w}" for w in WORDS] + ["score 0", "score 100", "score mid"]
    need += [f"v{k} {b}" for k in range(4) for b in (0, 1)]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"rating_diff: never reached: {missing}")
        fails += 1
    print(f"rating_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
