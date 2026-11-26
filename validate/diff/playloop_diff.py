#!/usr/bin/env python3
"""Differential test: the day cycle PlayLoop 0x4FB94, original vs port.

usage: .venv/bin/python validate/diff/playloop_diff.py [ncases=120] [DUMP] [--lib SO] [--fan 62]
--fan 62 (#70 F62, row 72): the original runs the 1.15m PlayLoop bytes (DayEndScreen after an end code, rating + exit
after a lost game; the DayEndScreen stub models cave 0x1EEC1: result = ESI when ESI != 0), the port fan fix F62.
A case: EAX 0 (from the menu: the office first) or 1 (Continue: TacticalLoop(1) first), a list of OfficeScreen results (4 Sleep,
7 Leave) and a list of TacticalLoop results (normal with end code 0..5; Restart; Abort; a mark byte that the tactical day writes
into the world, so a later line shows which world copy came back). The original runs whole in Unicorn; every callee is a stub
that logs (OfficeScreen, DeleteDaySaves, SaveGame, g_day + 1, SunriseScreen, StrategicScreen, TacticalLoop, LoadGame, DayEndScreen,
SabPlayScript's member, GameOverScreen, NewGameInit, the win calls). SaveGame / LoadGame stubs keep and give back (g_day, mark).
The port runs its real steps: ja1f_playloop_run = Play::day_steps, day_enter, abort_day, continue_day, game_end on a real World
with a real save folder; its trace comes from those functions, its files are checked at the end.
Compared: all lines in order, with g_day and the mark at each step; the difficulty and the A.I.M. link after a new game; which
day files exist at the end (the original's from the stub log). Not compared: what the callees do (each has its own diff test),
byte 0x92E4F (no port field: boxes save it, only tactical code clears it), the 2000 ms wait before a win (main.rs win_window; the
script does not wait), the main sound bank after a new game (window: sound::screen_main). The window host in main.rs runs day_steps,
day_enter, abort_day and continue_day too; its game end (end_window, win_window) is its own copy of game_end and does not run here.
Exits 1 on mismatch.
"""
import ctypes, os, random, shutil, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESI  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = "--fan" in argv
ncases = int(args[0]) if args else 120
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
SAVE = os.path.join(ROOT, "build", "DAY_day3.SAV")
for f in (dump, SAVE):
    if not os.path.exists(f):
        print(f"playloop_diff: {f} missing")
        sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_playloop_run.argtypes = [ctypes.c_char_p] * 4 + [ctypes.c_int, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p,
                                                           ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
PORT.ja1_fan_fixes_set.argtypes = [ctypes.c_uint64]
PORT.ja1_fan_fixes_set((1 << 61) if FAN else 0)
e = Emu(dump=dump, fast=True)
rt = e.rt
if FAN:
    e.put_115m(only=[(0x4FC2F, 0x4FC31), (0x4FC47, 0x4FC50), (0x4FCCE, 0x4FCD0)])
DAY, END, ABORT, RESTART, DIFF, LINK = 0x229FA4, 0x93449, 0x92E74, 0x92E75, 0x22A0CC, 0x22A043
st = {}


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def log(s):
    st["log"].append(s)


def at(what):
    log(f"{what} {e.r32(rt(DAY))} {st['mark']}")


def office(em, *_):
    at("office")
    return st["office"].pop(0) if st["office"] else 7


def save(em, a, *_):
    name = e.cstr(a).decode()
    at(f"save {name}")
    st["files"][name] = (e.r32(rt(DAY)), st["mark"])


def load(em, a, *_):
    name = e.cstr(a).decode()
    d, m = st["files"][name]
    e.w32(rt(DAY), d)
    st["mark"] = m
    at(f"load {name}")


def delete(em, *_):
    log("delete")
    st["files"].clear()


def tactical(em, a, *_):
    at(f"tactical {a}")
    for b in (ABORT, RESTART):
        e.w8(rt(b), 0)
    kind, code, dayend, mark = st["tact"].pop(0) if st["tact"] else (0, 0, 0, 0)
    st["mark"] = mark
    st["dayend"] = dayend
    if kind == 1:
        e.w8(rt(RESTART), 1)
    elif kind == 2:
        e.w8(rt(ABORT), 1)
    else:
        e.w8(rt(END), code)


def dayend(em, *_):
    log("dayend")
    esi = e.uc.reg_read(UC_X86_REG_ESI) & 0xFF
    e.w8(rt(END), esi if FAN and esi else st["dayend"])


def quit_line(em, *_):
    log("exit")
    e.stop_reason = "exit"
    e.uc.emu_stop()


def new_game(em, a, d, *_):
    st["newgame"] = (a & 0xFF, d)


def over(em, *_):
    log("over")


def link_after(em, *_):
    # LoadMainSound 0x31B8C comes after the link store: PlayLoop's last call.
    log(f"new {st['newgame'][0]} link {e.r8(rt(LINK))}")


for a, fn in ((0x4EE64, office), (0x37FF0, delete), (0x4EBC8, save), (0x4E964, load), (0x107BC, tactical), (0x65BD4, dayend),
              (0x382AC, quit_line), (0x37A0C, new_game), (0x504B4, over), (0x31B8C, link_after),
              (0x2A268, lambda em, *_: at("sunrise")),
              (0x432F4, lambda em, a, d, *_: at("strategic") if (a, d) == (1, 1) else log(f"strategic args {a} {d}")),
              (0x5D0C8, lambda em, a, d, *_: log(f"show {e.cstr(d).decode()}")),
              (0x5F2F8, lambda em, *_: log("win"))):
    e.stub(rt(a), fn)
for a in (0x67250, 0x62EB4, 0x5CE08, 0x5C82C):
    e.stub(rt(a), lambda em, *_: 0)

r = random.Random(1101)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
tmp = tempfile.mkdtemp(prefix="playloop_diff")
bad = cases = 0
seen = {}


def directed(c):
    """Every branch at least once, then random lists."""
    N = lambda code=0, de=0, m=9: (0, code, de, m)  # noqa: E731
    table = [
        (0, [7], []), (0, [4, 7], []), (0, [4, 4, 7], [N(m=5), N(m=6)]),
        (0, [4, 7], [(1, 0, 0, 5), N(m=6)]), (0, [4, 7], [(1, 0, 0, 5), (1, 0, 0, 6), N(m=7)]),
        (0, [4, 7], [(2, 0, 0, 5)]), (0, [4, 4, 7], [(2, 0, 0, 5), N(m=6)]),
        (0, [4], [N(1)]), (0, [4], [N(2)]), (0, [4], [N(3)]), (0, [4], [N(4)]), (0, [4], [N(5)]),
        (0, [4], [N(0, 2)]), (0, [4], [N(0, 3)]), (0, [4], [N(0, 4)]),
        (1, [7], [N(m=5)]), (1, [4, 7], [(1, 0, 0, 5), N(m=6)]), (1, [7], [(2, 0, 0, 5)]), (1, [], [N(2)]),
    ]
    if FAN:
        # F62: an end code with a verdict that would end the game too (the end code wins).
        table += [(0, [4], [N(1, 2)]), (0, [4], [N(2, 3)]), (0, [4], [N(4, 4)]), (0, [4], [N(5, 2)]), (1, [], [N(3, 4)])]
    if c < len(table):
        return table[c]
    tact, off = [], []
    for _ in range(r.randrange(1, 5)):
        k = r.choice([0, 0, 1, 2])
        tact.append((k, 0, 0, r.randrange(4, 200)))
        off.append(4)
    off += r.choice([[7], [4, 7]])
    return r.randrange(2), off, tact


def run(c):
    global bad, cases
    start, off, tact = directed(c)
    d = os.path.join(tmp, "d")
    shutil.rmtree(d, ignore_errors=True)
    os.mkdir(d)
    fo = (ctypes.c_int32 * max(1, len(off)))(*off)
    ft = (ctypes.c_int32 * max(1, 4 * len(tact)))(*[v for t in tact for v in t])
    buf = ctypes.create_string_buffer(1 << 16)
    k = PORT.ja1f_playloop_run(ISO, EXE, SAVE.encode(), d.encode(), start, fo, len(off), ft, len(tact), buf, 1 << 16)
    assert k > 0, k
    b = buf.raw[:k].decode().split("\n")[:-1]
    # The port's first line gives the save's day and mark; the original starts from them.
    first = b[0].split()
    day0, mark0 = int(first[-2]), int(first[-1])
    for lo, m in snap:
        e.uc.mem_write(lo, m)
    e.w32(rt(DAY), day0)
    e.w8(rt(END), 0)
    e.stop_reason = None
    st.update(log=[], office=list(off), tact=list(tact), mark=mark0, dayend=0, files={}, newgame=None)
    if start:
        # The folder as the port's driver makes it: office file mark 1, day file mark 2, the quick save mark 3.
        st["files"] = {"OFFICE.SAV": (day0, 1), "DAY.SAV": (day0, 2)}
    try:
        ret = e.call(rt(0x4FB94), start, max_steps=2_000_000)
        log("return")
    except EmuStop as x:
        if "exit" not in str(x):
            print(f"case {c}: original stopped: {x}")
            bad += 1
            return
    cases += 1
    a = st["log"]
    for l in a:
        seen[l.split()[0]] = seen.get(l.split()[0], 0) + 1
    if a != b:
        bad += 1
        if bad <= 3:
            i = next((i for i, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            print(f"case {c} start {start} office {off} tact {tact}: line {i}\n  orig {a[max(0, i - 2):i + 3]}\n  port {b[max(0, i - 2):i + 3]}")
        return
    # The port's day files on disk = the files the original's log kept.
    want = set(st["files"])
    have = {f for f in ("OFFICE.SAV", "DAY.SAV", "QUICK.SAV") if os.path.exists(os.path.join(d, f))}
    if not start and "QUICK.SAV" in have and any(l == "delete" for l in a):
        bad += 1
        print(f"case {c}: QUICK.SAV stays after the delete")
    if want - have:
        bad += 1
        print(f"case {c}: files orig {sorted(want)} port {sorted(have)}")


for c in range(ncases):
    run(c)
shutil.rmtree(tmp, ignore_errors=True)
print(f"playloop_diff{' --fan 62' if FAN else ''}: {cases} cases, {bad} mismatches; lines {dict(sorted(seen.items()))}")
for w in ("office", "delete", "save", "sunrise", "strategic", "tactical", "load", "dayend", "show", "over", "new", "win", "exit", "return"):
    if not seen.get(w) and not (FAN and w == "new"):  # F62: a lost game ends the program, no new game
        print(f"playloop_diff: step {w} never ran")
        bad += 1
sys.exit(1 if bad else 0)
