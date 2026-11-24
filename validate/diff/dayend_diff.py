#!/usr/bin/env python3
"""Differential test: the wait loop of the day-end screen (DayEndScreen? 0x65BD4 + DrawTeamSlotMarks? 0x4508C), original vs port.

usage: .venv/bin/python validate/diff/dayend_diff.py [ncases=1500] [DUMP] [--lib SO]
A case: 8 soldier records (present byte, profile, slot, nickname; holes in the team), a tape of polls (mouse on the OK buttons,
their edges, the faces, the face edges, elsewhere; left and right button as levels; keys L, T, l, t, Alt-X, other extended keys,
others) and the answers of the question boxes, with the mouse after the box. The original runs from the function's first line
with real ButtonAt?, MouseInRect?, ToUpper?, DrawTeamSlotMarks?. Stubs log the key actions, the button sounds,
PressPanelButton?, ShowTooltip with its rectangle, HideTooltip, YesNoPrompt? text + profile + nickname, FireMerc arguments,
TeamCompact?, the marks of DrawPanelElem. The port: ja1f_dayend_run = dayend.rs, the code the window runs.
Compared: all lines per poll. A fire stub shifts the records up on both sides (the real FireMerc + compaction: ai_diff).
Not compared: pixels (payroll_diff, smoke window_day), the steps before the loop and their order (verdict_diff, replay tests),
a verdict other than 0 (the loop does not run), the morning's inner order (morning_chain_diff; the original's list is
asserted here), the tail's LoadPanels and DayEndBankrupt? (verdict_diff).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"dayend_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_dayend_run.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]

e = Emu(dump=dump, fast=True)
rt = e.rt
MX, MY, MB, SOLD = 0x229D00, 0x229D04, 0x229CF4, 0xFA958
MORNING = [0x44380, 0x44740, 0x445DC, 0x4B48C, 0x45100, 0x45BBC, 0x42378, 0x46328, 0x448A8, 0x44C10, 0x45010, 0x65B40]
st = {}


def stack(i):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def log(s):
    st["log"].append(s)


def row():
    return st["tape"][st["pos"] - 1]


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
    st["keys"] = ([t[3], t[4]] if t[3] == 0 else [t[3]]) if t[3] >= 0 else []
    return 0


def key_pressed(em, *_):
    return 1 if st["keys"] else 0


def get_key(em, *_):
    return st["keys"].pop(0) if st["keys"] else 0


def exit_prompt(em, a, *_):
    assert a == 0
    log("exit prompt")
    if row()[5]:
        log("exit")
        e.stop_reason = "exit"
        e.uc.emu_stop()
    return 0


def toggle(em, a, d, *_):
    assert e.cstr(a) == b"Text " and d == rt(0x1163BD), (e.cstr(a), hex(d))
    log("toggle")
    return 0


def press(em, a, *_):
    log(f"press {a & 0xFF}")
    e.w32(rt(MB), e.r32(rt(MB)) & ~1)
    return 0


def tip(em, a, d, b, c, *_):
    assert e.cstr(stack(1)) == b"fire", e.cstr(stack(1))
    log(f"tip {s32(a)} {s32(d)} {s32(b)} {s32(c)} {s32(stack(0))}")
    return 0


def ask(em, a, d, b, c, *_):
    log(f"ask {e.cstr(a).decode()} {d & 0xFF} {e.cstr(b).decode()}")
    assert c == 1 and stack(0) == 0
    t = row()
    e.w32(rt(MX), t[6])
    e.w32(rt(MY), t[7])
    return 1 if t[5] else 0


def fire(em, a, d, b, *_):
    log(f"fire {a & 0xFF} {s32(d)} {s32(b)}")
    # Which record: the pass's x (11 + 37 k) gives k.
    st["fired"] = (s32(d) - 11) // 37
    return 0


def compact(em, a, *_):
    assert a & 0xFF == 0
    log("compact")
    k = st["fired"]
    base = rt(SOLD)
    rest = bytes(e.uc.mem_read(base + (k + 1) * 0x200, (7 - k) * 0x200))
    e.uc.mem_write(base + k * 0x200, rest + bytes(0x200))
    return 0


def draw_elem(em, a, d, b, c, *_):
    assert a & 0xFF == 0x67 and stack(0) == 0x14
    st["marks"].append((s32(c) - 13) // 37)
    return 0


def bass(em, *_):
    st["marks"] = []
    return 0


def copy_panel(em, *_):
    log(f"redraw {st['marks']}")
    return 0


def sprintf(em, *_):
    assert e.cstr(stack(1)) == b"%s%s?"
    e.uc.mem_write(stack(0), e.cstr(stack(2)) + e.cstr(stack(3)) + b"?\0")
    return 0


def morning(i):
    def f(em, *_):
        st["morning"].append(i)
        if i == len(MORNING) - 1:
            assert st["morning"] == list(range(len(MORNING))), st["morning"]
            log("morning")
        return 0
    return f


def bankrupt(em, *_):
    log("day end")
    return 0


def nothing(em, *_):
    return 0


def restore_area(em, a, *_):
    # 0x65E27 and 0x65E69: the question box goes; with Subtitles on, a yes keeps it during FireMerc.
    if a & 0xFF == 0x25:
        log("close")
    return 0


e.stub(rt(0x678E8), poll)
e.stub(rt(0x5616C), key_pressed)
e.stub(rt(0x560F4), get_key)
e.stub(rt(0x38084), exit_prompt)
e.stub(rt(0x29D54), lambda em, *_: log("again"))
e.stub(rt(0x3F994), toggle)
e.stub(rt(0x35D44), press)
e.stub(rt(0x23264), tip, pop=20)
e.stub(rt(0x23420), lambda em, *_: log("hide"))
e.stub(rt(0x28C84), ask, pop=4)
e.stub(rt(0x5097C), fire)
e.stub(rt(0x50750), compact)
e.stub(rt(0x75668), sprintf)
e.stub(rt(0x4420C), bass)
e.stub(rt(0x469F0), draw_elem, pop=4)
e.stub(rt(0x46BF4), copy_panel)
e.stub(rt(0x62304), bankrupt)
for i, a in enumerate(MORNING):
    e.stub(rt(a), morning(i))
e.stub(rt(0x52908), nothing, pop=4)
e.stub(rt(0x5C8A0), lambda em, *_: log("sound 0"))
e.stub(rt(0x5C900), lambda em, *_: log("sound 1"))
# Before the loop: the verdict answers 0.
for a in (0x5C4C8, 0x64274, 0x5C5EC, 0x455B4, 0x652B8, 0x673AC, 0x31B8C, 0x65AB0, 0x60680, 0x615DC, 0x67BF0, 0x467E4,
          0x46EC4):
    e.stub(rt(a), nothing)
e.stub(rt(0x28698), restore_area)

r = random.Random(1101)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
NICKS = [b"BUD", b"IVAN", b"RAVEN", b"", b"LONGNICK", b"a b"]
OK = {15: (266, 174), 16: (279, 166)}


def point():
    c = r.randrange(10)
    if c <= 2:
        x, y = r.choice(list(OK.values()))
        return x + r.choice([-1, 0, 5, 25, 26]), y + r.choice([-1, 0, 7, 15, 16])
    if c <= 7:
        k = r.randrange(8)
        return r.choice([10, 11, 12, 25, 42, 43, 47, 48]) + 37 * k, r.choice([19, 20, 30, 45, 46, 47])
    if c == 8:
        return r.randrange(320), r.randrange(60)
    return r.randrange(320), r.randrange(200)


def stage_tape():
    tape = []
    for i in range(r.randrange(4, 30)):
        x, y = point()
        if tape and r.random() < .4:
            x, y = tape[-1][0], tape[-1][1]
        b = r.choice([0, 0, 0, 0, 1, 2, 3])
        c = r.random()
        if c < .05:
            k = (0, 0x2D)
        elif c < .08:
            k = (0, r.choice([0x48, 0x2C, 0x2E, 0]))
        elif c < .2:
            k = (r.choice([0x4C, 0x6C, 0x54, 0x74]), 0)
        elif c < .25:
            k = (r.choice([13, 27, 65, 0x4B, 0x4D, 0x53, 0x55, 0xCC, 0xF4]), 0)
        else:
            k = (-1, 0)
        ans = 1 if r.random() < .25 else 0
        mx, my = (x, y) if r.random() < .6 else point()
        tape.append((x, y, b, k[0], k[1], ans, mx, my))
    return tape


bad = polls = 0
seen = dict.fromkeys(["press 15", "press 16", "tip", "hide", "ask", "fire", "exit", "again", "toggle", "day end", "redraw", "close"], 0)
first = None
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    base = rt(SOLD)
    n_team = r.randrange(0, 9)
    for k in range(8):
        a = base + k * 0x200
        present = (k < n_team) if r.random() < .8 else r.randrange(2)
        e.w8(a + 0x182, r.choice([1, 2, 255]) if present else 0)
        e.w8(a + 0x179, r.choice([0, 5, 59, 60, 61, 200]))
        e.w8(a + 0x17A, r.randrange(64))
        e.uc.mem_write(a + 0x16E, r.choice(NICKS).ljust(9, b"\0"))
    # Answers to Exit are mostly no: a yes ends the case.
    tape = stage_tape()
    if r.random() < .85:
        tape = [(t[0], t[1], t[2], t[3], t[4], 0 if t[3] == 0 and t[4] == 0x2D else t[5], t[6], t[7]) for t in tape]
    if r.random() < .6:
        # Reach the payroll screen soon.
        i = r.randrange(min(3, len(tape)))
        t = tape[i]
        tape[i] = (OK[15][0] + 2, OK[15][1] + 2, 1, t[3], t[4], t[5], t[6], t[7])
    n = len(tape)
    sold = bytes(e.uc.mem_read(base, 8 * 0x200))
    subtitles = r.randrange(2)
    e.w8(rt(0x1163BD), subtitles)
    st.update(log=[], tape=tape, pos=0, keys=[], morning=[], marks=[], fired=0)
    try:
        e.call(rt(0x65BD4), max_steps=2_000_000)
    except EmuStop as x:
        if "tape end" not in str(x) and "exit" not in str(x):
            print(f"case {case}: original stopped: {x}")
            bad += 1
            continue
    flat = (ctypes.c_int32 * (n * 8))(*[v for t in tape for v in t])
    buf = ctypes.create_string_buffer(1 << 18)
    k = PORT.ja1f_dayend_run(sold, flat, n, subtitles, buf, 1 << 18)
    assert k > 0, k
    a = st["log"]
    b = buf.raw[:k].decode().splitlines()
    polls += st["pos"]
    for l in a:
        for w in seen:
            if l.startswith(w):
                seen[w] += 1
    if a != b:
        bad += 1
        j = next((j for j in range(min(len(a), len(b))) if a[j] != b[j]), min(len(a), len(b)))
        if bad <= 5:
            print(f"case {case} line {j}: orig {a[max(0, j - 3):j + 3]} port {b[max(0, j - 3):j + 3]}")
print(f"dayend_diff: {ncases} cases, {polls} polls, {seen}, {bad} mismatches")
sys.exit(1 if bad or min(seen.values()) < ncases // 100 else 0)
