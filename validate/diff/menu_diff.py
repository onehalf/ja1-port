#!/usr/bin/env python3
"""Differential test: the main menu loop with Set Difficulty and Set Quick-Save Mode (GameMain 0x4FFEC, NewGameSetup
0x4F548, SetQuickSaveMode 0x4F898), original vs port.

usage: .venv/bin/python validate/diff/menu_diff.py [ncases=500] [DUMP] [--lib SO]
A case: Continue allowed or not (AnyGameSaveExists), which of GAME0.SAV / QICKSTRT.SAV exist, a tape of polls (mouse on rows,
edges of rows and elsewhere; left button; keys Up, Down, Enter, others). The original runs from GameMain's first line;
MainMenu 0x4FDF4 is real, without its file loads (mainmenu_diff has them). Stubs log the row draw 0x4FD48, DrawText, FadePalette, ClickSound,
MouseSetPos, rename, and the game: NewGameInit + PlayLoop. The port: ja1f_menu_run = menu.rs. Compared: all lines per poll,
which files exist at the end, g_difficulty and the quick-save byte at each game start. The tape's end stops both.
Not compared: pixels (gate_new_game), the Continue prompt at the program start, the black fade before Set Difficulty
(the screen is black: the port does not run it), the intro and sound calls of START NEW GAME (filmcue, gate_music).
Exits 1 on mismatch.
"""
import ctypes, os, random, shutil, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"menu_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_menu_run.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]

e = Emu(dump=dump, fast=True)
rt = e.rt
MX, MY, MB, DIFF = 0x229D00, 0x229D04, 0x229CF4, 0x22A0CC
st = {}


def stack(i):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def log(s):
    st["log"].append(s)


class TapeEnd(Exception):
    pass


def poll(em, *_):
    i = st["pos"]
    if i >= len(st["tape"]):
        e.stop_reason = "tape end"
        e.uc.emu_stop()
        return 0
    t = st["tape"][i]
    st["pos"] = i + 1
    log(f"poll {i}")
    e.w32(rt(MX), t[0])
    e.w32(rt(MY), t[1])
    e.w32(rt(MB), t[2])
    st["keys"] = [t[3], t[4]] if t[3] == 0 else ([t[3]] if t[3] >= 0 else [])
    return 0


def key_pressed(em, *_):
    return 1 if st["keys"] else 0


def get_key(em, *_):
    return st["keys"].pop(0) if st["keys"] else 0


def warp(em, a, d, *_):
    e.w32(rt(MX), a)
    e.w32(rt(MY), d)
    log(f"warp {s32(a)} {s32(d)}")
    return 0


def wait_left(em, *_):
    e.w32(rt(MB), e.r32(rt(MB)) & ~1)
    return 0


def text(em, a, d, b, c, *_):
    log(f"text {e.cstr(a).decode()} {s32(c)} {s32(stack(0))} {stack(1)} {stack(2) & 0xFF} {stack(3) & 0xFF} {stack(4) & 0xFF}")
    return 0


def fade(em, a, d, b, c, *_):
    log(f"fade {s32(b)} {s32(c)} {s32(stack(0))}")
    return 0


def exists(em, a, *_):
    n = e.cstr(a).decode().upper()
    if n == "QUICK.SAV":
        return 0
    if n == "GAME0.SAV":
        return int(st["game0"])
    if n == "QICKSTRT.SAV":
        return int(st["qs"])
    return 0


def rename(em, a, d, *_):
    log("rename")
    st["game0"], st["qs"] = True, False
    return 0


def playloop(em, a, *_):
    log("game")
    return 0


def new_game(em, a, d, *_):
    st["game_o"].append(f"game {e.r8(rt(DIFF))} {e.r8(rt(0x22A1F9))}")
    return 0


def quit_line(em, *_):
    log("exit")
    e.stop_reason = "exit"
    e.uc.emu_stop()
    return 0


def nothing(em, *_):
    return 0


e.stub(rt(0x678E8), poll)
e.stub(rt(0x5616C), key_pressed)
e.stub(rt(0x560F4), get_key)
e.stub(rt(0x681D0), warp)
e.stub(rt(0x67C54), wait_left)
e.stub(rt(0x358D0), text, pop=24)
e.stub(rt(0x52908), fade, pop=4)
e.stub(rt(0x4707C), exists)
e.stub(rt(0x762CD), rename)
e.stub(rt(0x4FB94), playloop)
e.stub(rt(0x37A0C), new_game)
e.stub(rt(0x382AC), quit_line)
e.stub(rt(0x5C880), lambda em, *_: log("sound"))
e.stub(rt(0x4FD48), lambda em, a, d, *_: log(f"row {s32(a)} {d}"))
e.stub(rt(0x4FDB8), lambda em, *_: st["cont"])


def memset(em, *_):
    # The key byte of both choice screens starts uninitialized in the original (stale stack). Here a stale 13 could only
    # come from the stubs; the game's intro and file loads run over that stack first. The first screen line clears it.
    sp = e.uc.reg_read(UC_X86_REG_ESP) + 4
    for i in range(0x40):
        if e.r8(sp + i) == 13:
            e.w8(sp + i, 0)
    return 0


e.stub(rt(0x756A0), memset)
for a in (0x67BF0, 0x673AC, 0x674B0, 0x5C82C, 0x5D0C8, 0x46EC4, 0x5CE08, 0x31B8C, 0x5E8E0, 0x53300, 0x52820):
    e.stub(rt(a), nothing)


# MainMenu 0x4FDF4 runs for real; its file loads are stubs here, mainmenu_diff runs them on the real file.
e.stub(rt(0x5E8E0), nothing)
e.stub(rt(0x532BC), nothing, pop=4)

ROWS = [43, 65, 87, 109]
r = random.Random(1101)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]


def point():
    c = r.randrange(7)
    if c <= 1:
        return r.randrange(60, 251), r.choice(ROWS) + r.choice([-1, 0, 1, 10, 21, 22, 23])
    if c == 2:
        return r.choice([59, 60, 250, 251]), r.randrange(43, 110)
    if c <= 4:
        return r.randrange(125, 196), r.choice([80, 90, 100, 110, 120]) + r.choice([-1, 0, 2, 6, 12, 13])
    if c == 5:
        return r.choice([124, 125, 195, 196]), r.choice([85, 95, 105, 115, 125])
    return r.randrange(320), r.randrange(200)


tmp = tempfile.mkdtemp(prefix="menu_diff")
bad = cases = games = exits = polls = 0
first = None
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    cont = r.randrange(2)
    game0, qs = r.randrange(2) == 1, r.randrange(2) == 1
    n = r.randrange(4, 40)
    tape = []
    for i in range(n):
        x, y = point()
        if tape and r.random() < .5:
            x, y = tape[-1][0], tape[-1][1]
        left = 1 if r.random() < .12 else 0
        c = r.random()
        if c < .2:
            k = (0, r.choice([0x48, 0x50]))
        elif c < .27:
            k = (13, -1)
        elif c < .31:
            k = r.choice([(0, 0x47), (0, 0x49), (0, 0x4F), (27, -1), (65, -1), (32, -1)])
        else:
            k = (-1, -1)
        tape.append((x, y, left, k[0], k[1]))
    d = os.path.join(tmp, "d")
    shutil.rmtree(d, ignore_errors=True)
    os.mkdir(d)
    for f, on in (("GAME0.SAV", game0), ("QICKSTRT.SAV", qs)):
        if on:
            open(os.path.join(d, f), "w").close()
    st.update(game_o=[], log=[], tape=tape, pos=0, keys=[], cont=cont, game0=game0, qs=qs)
    e.w8(rt(0x9635E), 0xFF)
    e.w8(rt(0x9635F), 0xFF)
    try:
        e.call(rt(0x4FFEC), max_steps=2_000_000)
    except EmuStop as x:
        if "tape end" not in str(x) and "exit" not in str(x):
            print(f"case {case}: original stopped: {x}")
            bad += 1
            continue
    flat = (ctypes.c_int32 * (n * 5))(*[v for t in tape for v in t])
    buf = ctypes.create_string_buffer(1 << 18)
    k = PORT.ja1f_menu_run(ISO, EXE, d.encode(), cont, flat, n, buf, 1 << 18)
    assert k > 0, k
    a = st["log"]
    # 'game' in the original comes as one line; the port's line holds d q: compare the numbers apart.
    b = []
    games_p = []
    for l in buf.raw[:k].decode().splitlines():
        if l.startswith("game "):
            games_p.append(l)
            b.append("game")
        else:
            b.append(l)
    polls += st["pos"]
    games += a.count("game")
    exits += a.count("exit")
    # The black fade after START NEW GAME: only the original runs it.
    a2 = []
    for i, l in enumerate(a):
        if l == "fade 24 256 0" and i + 1 < len(a) and a[i + 1].startswith("text Set Difficulty"):
            continue
        if l == "rename":
            continue
        a2.append(l)
    a = a2
    errs = []
    if st["game_o"] != games_p:
        errs.append(f"games orig {st['game_o']} port {games_p}")
    if a != b:
        j = next((j for j in range(min(len(a), len(b))) if a[j] != b[j]), min(len(a), len(b)))
        errs.append(f"line {j}: orig {a[max(0, j - 3):j + 3]} port {b[max(0, j - 3):j + 3]}")
    have = (os.path.exists(os.path.join(d, "GAME0.SAV")), os.path.exists(os.path.join(d, "QICKSTRT.SAV")))
    if have != (st["game0"], st["qs"]):
        errs.append(f"files orig {st['game0'], st['qs']} port {have}")
    if errs:
        bad += 1
        if bad <= 5:
            print(f"case {case} cont {cont} tape {tape[:12]}: {errs}")
shutil.rmtree(tmp, ignore_errors=True)
print(f"menu_diff: {ncases} cases, {polls} polls, {games} games, {exits} exits, {bad} mismatches")
sys.exit(1 if bad or games < ncases // 10 or exits < ncases // 10 else 0)
