"""Differential test: a sector click on the island map (strategic.md "Sector click on the island map").

usage: .venv/bin/python validate/diff/map_diff.py [ncases=2000] [DUMP] [--port] [--lib SO] [-v]
The original: StrategicScreen's click block 0x43987..0x43C32 runs in Unicorn on tac60 with a made-up stack frame
(sector at [esp+4], the process warning flag at [esp+0x1C]), the map mode byte 0x93DA0 and the mouse buttons
0x229CF4 fuzzed, together with the sector table, cur_sector, team_on_duty, the worker pool, the idle pool and the
plants. StatusMessage? 0x27D24 gives the message number, 0x5F5AC(0x2F, 6) the guide quote; drawing is stubbed.
CalcLedgerAll and SectorConnectedToHome run for real. The start checks (ja1_map_start) are compared with the
rule of the spec on the same states (the original's start test is inline in the screen loop).
--port: ja1_map_click / ja1_map_start of port/ja1logic on the same state: the world (port_lib world_load), the
message, the guide quotes and the warning flag are compared. Without --port only the original runs (harness smoke
test; it fails when a message never shows up). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
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
port_lib.PAY = True                  # the world gets accounts (0xA8), prev_balance, sector_order
VERBOSE = "-v" in argv

START, END = 0x43987, 0x43C32
SECTORS, CUR, TEAM, MODE, BUTTONS = 0x22979C, 0x22A039, 0x229FAC, 0x93DA0, 0x229CF4
POOL, IDLE, PLANTS, BAL_AFTER = 0x229FDC, 0x229FEC, 0x22A12A, 0x229FC8
MSG = {0x97B57: 1, 0x97ADF: 2, 0x99DAB: 3, 0x99D91: 4, 0x97DAF: 5, 0x99DC5: 6, 0x97E27: 7, 0x97F8F: 8, 0x97F17: 9,
       0x97E9F: 10, 0x99E13: 11, 0x99DDF: 12, 0x99DF9: 13, 0x97CBF: 14, 0x97D37: 15, 0x990E7: 16, 0x9915F: 17}
KEY_START, KEY_END, BUTTON_START, HINTED = 0x4336E, 0x433B2, 0x43899, 0x22A1F4
NOP = {0x41640: 0, 0x419BC: 0, 0x67C54: 0, 0x67C68: 0, 0x36010: 4}

if not os.path.exists(DUMP):
    print(f"map_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
log = []
for a, pop in NOP.items():
    e.stub(rt(a), lambda *_: 0, pop)
msg_of = {rt(a): n for a, n in MSG.items()}
e.stub(rt(0x27D24), lambda emu, eax, *_: log.append(("msg", msg_of.get(eax & 0xFFFFFFFF, f"?{eax:#x}"))))
e.stub(rt(0x5F5AC), lambda emu, eax, edx, *_: log.append(("guide", eax & 0xFFFFFFFF, edx & 0xFFFFFFFF)))
e.stub(rt(END), lambda *_: 0)          # the join point: "return" to the sentinel pushed below the frame
e.stub(rt(KEY_END), lambda *_: 0)
SP = STACK_TOP - 0x400


def g8(a): return e.r8(rt(a))
def s8(a, v): e.w8(rt(a), v)
def s32(a, v): e.w32(rt(a), v)


def fuzz(r):
    """one case: (sector, mode, buttons, warned) after the world is fuzzed in emulator memory"""
    tbl = bytearray()
    owned_p = r.choice([.5, .8, 1.0])
    for s in range(60):
        f = 1 if r.random() < owned_p else 0
        if r.random() < .1:
            f |= r.choice([2, 4, 8, 16, 0x1E])
        trees = r.choice([0, 0, 1, 3, 5, 8, 10, r.randrange(256)])
        rec = [f, r.randrange(9), r.choice([0, 0, 1, 4, 254, 255, r.randrange(256)]),
               r.choice([0, 1, 2, 3, 6, 9, 12, 252, 253, 254, 255, r.randrange(256)]),
               r.choice([0, 1, 7, 8, 9, 255, r.randrange(256)]), r.choice([0, 0, r.randrange(256)]), trees,
               r.randrange(trees + 1) if r.random() < .7 else r.randrange(256), r.randrange(trees + 1)]
        tbl += bytes(rec)
    tbl[59 * 9] |= 1                                        # home owned (most cases)
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    sector = r.randrange(60)
    if r.random() < .1:                                     # no tappers, nothing workable (messages 8, 9)
        tbl[9 * sector + 3] = tbl[9 * sector + 7] = 0
        e.uc.mem_write(rt(SECTORS), bytes(tbl))
    s8(CUR, r.choice([0xFF, 0xFF, sector, r.randrange(60), r.randrange(256)]))
    s32(TEAM, r.choice([0, 1, 2, 3, 8, r.randrange(256), r.randrange(1 << 32)]))
    s32(POOL, r.choice([0, 3, 15, 30, 100, r.randrange(1 << 32)]))
    s32(IDLE, r.choice([0, 1, 1, 2, 3, 3, 4, 10, r.randrange(1 << 32)]))
    if r.random() < .5:                                     # plants: capacity, sector, state 2 = working
        for k in range(4):
            p = rt(PLANTS) + 12 * k
            e.w32(p, r.choice([0, 1, 3, 10, 100, r.randrange(1 << 32)]))
            e.w32(p + 4, r.randrange(60))   # >= 60: Connected to home of the original is undefined (Plants SPEC GAP)
            e.w8(p + 8, r.choice([2, 2, 0, 1, r.randrange(256)]))
    mode = r.choice([0, 0, 1, 1, 1, 2, 2, 3, r.randrange(256)])
    if mode == 1 and r.random() < .3:                       # the process warning edge: one plant at home, its
        q = sum(tbl[9 * k + 3] for k in range(60) if tbl[9 * k] & 1) // 3     # capacity near tappers div 3
        e.uc.mem_write(rt(PLANTS), bytes(48))
        e.w32(rt(PLANTS), max(0, q + r.choice([-2, -1, 0, 1, 2])))
        e.w32(rt(PLANTS) + 4, 59)
        e.w8(rt(PLANTS) + 8, 2)
    buttons = r.choice([1, 1, 2, 2, 3, 0, r.randrange(256)])
    s8(MODE, mode)
    s8(BUTTONS, buttons)
    return sector, mode, buttons, r.choice([0, 0, 1, r.randrange(256)])


def run(start, frame):
    """one code block of StrategicScreen from start to its join point (stubbed as a return) -> the log"""
    log.clear()
    e.uc.mem_write(SP, bytes(0x20))
    e.w32(SP, RET_SENTINEL)
    for off, v, n in frame:
        e.uc.mem_write(SP + off, v.to_bytes(n, "little"))
    e.uc.reg_write(UC_X86_REG_ESP, SP)
    e.uc.emu_start(rt(start), RET_SENTINEL + 1, timeout=5_000_000)
    if e.uc.reg_read(UC_X86_REG_EIP) != RET_SENTINEL:
        raise RuntimeError(f"original stopped at {e.uc.reg_read(UC_X86_REG_EIP):#x}")
    return list(log)


def original(sector, warned):
    """the click block on the current emulator state -> (log, warned after)"""
    lg = run(START, [(4, sector, 4), (0x1C, warned, 1)])
    return lg, e.r8(SP + 0x1C)


def original_start(button, hinted):
    """the start test after the start key (MapKeyHandler? answer != 0) or the start button -> (answer, hinted)"""
    s8(HINTED, hinted)
    lg = run(BUTTON_START if button else KEY_START, [(0x1C, 0, 1)])
    go = e.r8(SP + 0x18)
    msgs = [m for _, m in lg]
    ans = 0 if go else 16 if msgs == [16, 17] else msgs[0] if len(msgs) == 1 else f"?{msgs}"
    return ans, g8(HINTED)


def port(sector, mode, buttons, warned, bufs, scalars):
    plog = []

    def call(op, a):
        if op == "guide_say":
            plog.append(("guide", a[0] & 0xFFFFFFFF, a[1] & 0xFFFFFFFF))
            return 0, None
        raise SystemExit(f"map_diff --port: unmapped op {op} {list(a)}")
    wf = bytearray([warned])
    r = port_lib.sector_run("map_click", bufs, scalars, [sector, mode, buttons, wf], lambda n: 0, call)
    if r:                                   # the message shows before All accounts and the guide quote
        plog.insert(0, ("msg", r))
    return plog, wf[0]


def main():
    r = random.Random(1)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    for i in range(N):
        e.uc.mem_write(0, snap)
        sector, mode, buttons, warned = fuzz(r)
        pre = port_lib.world_load(e, ai) if PORT else None
        lo, wo = original(sector, warned)
        for k in lo:
            cover[k[0] if k[0] == "guide" else f"msg{k[1]}"] += 1
        if not lo:
            cover["none"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        lp, wp = port(sector, mode, buttons, warned, bufs, scalars)
        bad = []
        if lo != lp:
            bad.append(f"log {lo} vs {lp}")
        if wo != wp:
            bad.append(f"warned {wo} vs {wp}")
        for b in (bufs_o, bufs):            # accounts +0xC = team_on_duty, +0x99 = cur_sector (compared as scalars)
            b["accounts"][0xC:0x10] = bytes(4)
            b["accounts"][0x99] = 0
        for k in bufs_o:
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        # the start checks on the state after the click: the key form, then the button form with a random hint flag
        lib = port_lib.lib()
        lib.ja1_map_start.restype = ctypes.c_int32
        arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
        w = port_lib.World(**{k: ctypes.cast(a, port_lib.u8p) for k, a in arrays.items()}, **scalars)
        if r.random() < .3:                     # the funds edge
            bal = r.choice([-1, 0, 1]) & 0xFFFFFFFF
            e.w32(rt(BAL_AFTER), bal)
            bufs["accounts"][0x28:0x2C] = bal.to_bytes(4, "little")
        for button in (0, 1):
            h0 = r.choice([0, 1, 0xFF])
            want = original_start(button, h0)
            hp = ctypes.c_uint8(h0)
            got = (lib.ja1_map_start(ctypes.byref(w), button, ctypes.byref(hp)), hp.value)
            cover[f"start{button}_{want[0]}"] += 1
            if got != want:
                bad.append(f"start(button {button}, hinted {h0}) {want} vs {got}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} sector {sector} mode {mode} buttons {buttons:#x}: " + "; ".join(bad)[:400])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items(), key=str)))
    missing = [f"msg{n}" for n in range(1, 14) if not cover[f"msg{n}"]] + ([] if cover["guide"] else ["guide"])
    if missing:
        print(f"map_diff: never reached: {missing}")
        fails += 1
    print(f"map_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
