#!/usr/bin/env python3
"""Differential test: "Exit to DOS?", ExitToDosPrompt 0x38084, original vs port.

usage: .venv/bin/python validate/diff/exitdos_diff.py [ncases=120] [DUMP] [--lib SO]
A case: argument 0 or 1, answer, the day files present, a random seed, the over_map byte. The original runs in Unicorn with
the real question box (the PCC loader and the input 0x2538C are stubbed), the real FileExists test path through
DeleteDaySaves (FileExists and unlink_ stubbed and logged), the real Random and the quit line record. QuickSave and
QuitWithLine are stubbed; QuitWithLine ends the run. The port: question::exit_finish (ja1f_exit_prompt) on a real folder.
Compared: the box pixels over a noise screen, the program ends or not, Quick Save runs or not, which day files go, the quit
line, the final random seed.
On the original side only: the redraw 0x1CB78 runs only for argument 1, before the box; over_map 0x284F13 is 0 in the box and
back to its value in QuickSave and after a no; the screen is back to the noise after the box.
Not compared: the over_map byte in the port (its window shows the pointer for the box; smoke_play logs it). Exits 1 on mismatch.
"""
import ctypes, os, random, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
import datarc, pcx  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 120
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"exitdos_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_exit_prompt.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int] * 2 + [ctypes.c_void_p] * 4 + [ctypes.c_int]
MAIN = game_read("DAT\\MAIN.DAT")
e = Emu(dump=dump, fast=True)
rt = e.rt
screen = e.r32(rt(0x95678))
OVER, SEED, G_DD4 = 0x284F13, 0x9BBBC, 0x92DD4
FILES = ("OFFICE.SAV", "DAY.SAV", "QUICK.SAV")
st = {}


class Quit(Exception):
    pass


def log(s):
    st["log"].append(s)
    return 0


def answer(em, *_):
    st["px"] = bytes(em.uc.mem_read(screen, 64000))
    st["over_box"] = e.r8(rt(OVER))
    return log("ask") or st["answer"]


def exists(em, a, *_):
    log("exists " + e.cstr(a).decode().upper())
    return int(e.cstr(a).decode().upper() in st["present"])


def unlink(em, a, *_):
    n = e.cstr(a).decode().upper()
    st["gone"].add(n)
    return 0


def quick(em, *_):
    st["over_quick"] = e.r8(rt(OVER))
    return log("quick")


def quit_line(em, a, *_):
    st["line"] = e.cstr(a)
    st["seed"] = e.r32(rt(SEED))
    log("quit")
    em.uc.emu_stop()
    st["stopped"] = True
    return 0


_pics = {}


def load(em, eax, edx, ebx, ecx):
    n = e.cstr(ebx).decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    w, h, px = _pics[n]
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for addr in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x67E18, 0x67E30, 0x681D0):
    e.stub(rt(addr), lambda em, *_: 0)
for addr, fn, pop in ((0x532BC, load, 4), (0x2538C, answer, 0), (0x4707C, exists, 0), (0x75D04, unlink, 0), (0x2D880, quick, 0),
                      (0x382AC, quit_line, 0), (0x1CB78, lambda em, a, *_: log(f"redraw {a & 0xFF}"), 0)):
    e.stub(rt(addr), fn, pop=pop)
r = random.Random(1106)
bad = cases = 0
seen = {}


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


for c in range(ncases):
    tactical, ans = (c & 1), (c >> 1 & 1)
    present = {f for f in FILES if r.random() < 0.6}
    seed, over, dd4 = r.getrandbits(32), r.randrange(2), r.randrange(256)
    noise = bytes(r.randrange(256) for _ in range(320)) * 200
    st.update(log=[], answer=ans, present=present, gone=set(), stopped=False)
    for k in ("line", "seed", "over_box", "over_quick", "px"):
        st.pop(k, None)
    e.uc.mem_write(screen, noise)
    e.w8(rt(OVER), over)
    e.w8(rt(G_DD4), dd4)
    e.w32(rt(SEED), seed)
    e.w8(rt(0x92F90), 0)
    try:
        e.call(rt(0x38084), tactical, max_steps=30_000_000)
    except EmuStop as x:
        if not st["stopped"]:
            report(c, f"original stopped: {x}")
            continue
    cases += 1
    lg = st["log"]
    if (lg[:1] == [f"redraw {dd4}"]) != bool(tactical):
        report(c, f"redraw rule: {lg}")
    if st.get("over_box") != 0:
        report(c, f"over_map in the box {st.get('over_box')}")
    if ans and tactical and st.get("over_quick") != over:
        report(c, f"over_map at QuickSave {st.get('over_quick')} want {over}")
    if not ans and e.r8(rt(OVER)) != over:
        report(c, f"over_map after a no {e.r8(rt(OVER))} want {over}")
    if not ans and lg[-1] != "ask":
        report(c, f"steps after a no: {lg}")
    with tempfile.TemporaryDirectory() as d:
        for f in present:
            open(os.path.join(d, f), "wb").close()
        sd = ctypes.c_uint32(seed)
        o, line = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(256)
        ret = PORT.ja1f_exit_prompt(ISO, EXE, d.encode(), tactical, ans, ctypes.byref(sd), noise, o, line, 256)
        left = set(os.listdir(d))
    if ret < 0:
        report(c, f"port returned {ret}")
        continue
    if st["px"] != o.raw:
        q = [j for j in range(64000) if st["px"][j] != o.raw[j]]
        report(c, f"box: {len(q)} pixels, first ({q[0] % 320},{q[0] // 320}) original {st['px'][q[0]]} port {o.raw[q[0]]}")
    if bool(ret & 1) != st["stopped"]:
        report(c, f"ends: original {st['stopped']} port {ret}")
    if bool(ret & 4) != ("quick" in lg):
        report(c, f"quick save: original {lg} port {ret}")
    if st["gone"] != present - left or bool(ret & 2) != ("exists OFFICE.SAV" in lg):
        report(c, f"files gone: original {st['gone']} port {present - left} ({ret})")
    if ans:
        if st.get("line") != line.value:
            report(c, f"line: original {st.get('line')} port {line.value}")
        if st.get("seed") != sd.value:
            report(c, f"seed: original {st.get('seed'):#x} port {sd.value:#x}")
        seen[st.get("line")] = 1
    elif sd.value != seed:
        report(c, "the port draws a random number after a no")
print(f"exitdos_diff: {cases} cases, {len(seen)} quit lines, {bad} mismatches")
sys.exit(1 if bad or cases < ncases else 0)
