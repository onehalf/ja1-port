"""Differential test: the day start after "Get to work?" (strategic.md "Day start").

usage: .venv/bin/python validate/diff/day_diff.py [ncases=2000] [DUMP] [--port] [--lib SO] [-v]
The original: StrategicScreen's block 0x43C3D..0x43E07 runs in Unicorn on tac60 as a code block (made-up stack
frame, the go flag [esp+0x18] = 1 as after the start checks), then, when the go flag is still set, the block
0x43E5E..0x43E97 (the copy of the sector table to the replay copy 0x2254F0). Fuzzed: team_on_duty, cur_sector,
sorties 0x22A03F, soldiers 0..7 (inCurSector, hp, profileId, duty), the sector table, the dialog answer.
The sanim load 0x5E8E0, ConfirmDialog? 0x275E4 and the yes/no box 0x278A0 are stubbed and logged; sprintf_ runs
as Python (Emu.stub_sprintf) and the 2 characters it writes into the go animation's text line are compared with
"%2d" of the number the port passes to host op 303 day_ask. GuideMerc 0x62A00 runs for real.
--port: ja1_day_start of port/ja1logic on the same state: the world (port_lib world_load, with sector_replay),
the host op log and the answer are compared. Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu, RET_SENTINEL, STACK_TOP   # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 2000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
port_lib.PAY = True                  # the world gets accounts (0xA8)
port_lib.DAYSTART = True             # and sector_replay
VERBOSE = "-v" in argv

START, JOIN, COPY, LOOP, COPY_END = 0x43C3D, 0x43E07, 0x43E5E, 0x43352, 0x43E97
SECTORS, REPLAY, CUR, TEAM, SORTIES, SPEAKER = 0x22979C, 0x2254F0, 0x22A039, 0x229FAC, 0x22A03F, 0x92DD5
PTRS, ROWS, SANIM_TEXT = 0x100958, 0x9443A, 0x9681F
NO_DUTY_TEXT = 0x98007

if not os.path.exists(DUMP):
    print(f"day_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
e.stub_sprintf()
log = []
answer = [0]


def confirm(emu, eax, edx, ebx, ecx):
    """ConfirmDialog?(text, wait row ptr, go row ptr, 0): rows are 12-byte records of the table 0x9443A"""
    wait, go = (edx - rt(ROWS)) // 12, (ebx - rt(ROWS)) // 12
    line = rt(SANIM_TEXT) + 40 * emu.r8(ebx)
    log.append(("day_ask", 1, wait, go, bytes(emu.uc.mem_read(line + 22, 2))))
    return answer[0]


def yesno(emu, eax, *_):
    log.append(("day_ask", 0, 0, 0, 0) if eax == rt(NO_DUTY_TEXT) else ("yesno?", eax))
    return answer[0]


e.stub(rt(0x5E8E0), lambda *_: log.append(("sanim",)))
e.stub(rt(0x275E4), confirm, 12)
e.stub(rt(0x278A0), yesno, 0x20)
e.stub(rt(0x67BF0), lambda *_: 0)
for a in (JOIN, LOOP, COPY_END):         # join points: "return" to the sentinel pushed below the frame
    e.stub(rt(a), lambda *_: 0)
SP = STACK_TOP - 0x400


def fuzz(r):
    tbl = bytes(r.randrange(256) for _ in range(540))
    e.uc.mem_write(rt(SECTORS), tbl)
    e.uc.mem_write(rt(REPLAY), bytes(r.randrange(256) for _ in range(540)))
    e.w8(rt(CUR), r.choice([59, 0, 9, 98, 254, 255, r.randrange(60), r.randrange(256)]))
    e.w32(rt(TEAM), r.choice([0, 0, 1, 2, 3, 4, 5, 8, 0xFFFFFFFF, r.randrange(256), r.randrange(1 << 32)]))
    e.w8(rt(SORTIES), r.choice([0, 1, 5, 255, r.randrange(256)]))
    e.w8(rt(SPEAKER), r.randrange(256))
    for k in range(8):
        p = e.r32(rt(PTRS) + 4 * k)
        e.w8(p + 0x182, r.choice([0, 1, 1, r.randrange(256)]))
        e.w8(p + 0x183, r.choice([0, 30, 30, r.randrange(256)]))
        e.w8(p + 0x179, r.choice([59, 60, 61, 62, 63, 64, 65, 3, r.randrange(256)]) if r.random() < .4 else
             r.randrange(60))
        e.w8(p + 0x17E, r.choice([0, 0, 1, 4, 7, r.randrange(256)]))
    answer[0] = r.choice([0, 1, 1, 2, 0xFFFFFFFF])


def run(start, go):
    e.uc.mem_write(SP, bytes(0x20))
    e.w32(SP, RET_SENTINEL)
    e.w8(SP + 0x18, go)
    e.uc.reg_write(UC_X86_REG_ESP, SP)
    e.uc.emu_start(rt(start), RET_SENTINEL + 1, timeout=5_000_000)
    if e.uc.reg_read(UC_X86_REG_EIP) != RET_SENTINEL:
        raise RuntimeError(f"original stopped at {e.uc.reg_read(UC_X86_REG_EIP):#x}")
    return e.r8(SP + 0x18)


def original():
    log.clear()
    go = run(START, 1)
    if go:
        run(COPY, go)
    return list(log), 1 if go else 0


def port(bufs, scalars):
    plog = [("sanim",)]

    def call(op, a):
        if op == "day_ask":
            kind, wait, go, num = (x & 0xFFFFFFFF for x in a)
            plog.append(("day_ask", kind, wait, go, (b"%2d" % num)[:2] if kind == 1 else num))   # first 2 chars
            return answer[0], None
        raise SystemExit(f"day_diff --port: unmapped op {op} {list(a)}")
    r = port_lib.sector_run("day_start", bufs, scalars, [], lambda n: 0, call)
    return plog, r


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        fuzz(r)
        pre = port_lib.world_load(e, ai) if PORT else None
        lo, go = original()
        team = e.r32(rt(TEAM))
        cover[f"go{go}"] += 1
        cover[f"kind{lo[1][1] if len(lo) > 1 else '?'}"] += 1
        if go and e.r8(rt(SPEAKER)) < 0x40:
            cover["guide"] += 1
        if len(lo) != 2 or lo[1][0] != "day_ask":
            print(f"case {i}: odd log {lo}")
            fails += 1
            continue
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp, gp = port(bufs, scalars)
        bad = []
        if lo != lp:
            bad.append(f"log {lo} vs {lp}")
        if go != gp:
            bad.append(f"answer {go} vs {gp}")
        for b in (bufs_o, bufs):            # accounts +0xC = team_on_duty, +0x99 = cur_sector (compared as scalars)
            b["accounts"][0xC:0x10] = bytes(4)
            b["accounts"][0x99] = 0
            b["text_buf"][:] = bytes(len(b["text_buf"]))   # sprintf scratch of the original: host text
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
                print(f"MISMATCH case {i} team {team:#x} answer {answer[0]:#x}: " + "; ".join(bad)[:400])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items(), key=str)))
    missing = [k for k in ("go0", "go1", "kind0", "kind1", "guide") if not cover[k]]
    if missing:
        print(f"day_diff: never reached: {missing}")
        fails += 1
    print(f"day_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
