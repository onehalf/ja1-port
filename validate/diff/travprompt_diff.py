#!/usr/bin/env python3
"""Differential test: the traverse popup, TraversePrompt 0x1CBA8, original vs port.

usage: .venv/bin/python validate/diff/travprompt_diff.py [ncases=1500] [DUMP] [--lib SO]
A case fuzzes the gate (selected 0x284F11, sector mercs byte, hp), the path (T key EDX 0 or button EDX 1), the view
window (SetViewWindow's rectangle), over_map 0x284F13, the tile 0x229D2C, the g_redrawOff byte and a tape of polls
(mouse biased to the word rectangles and their edges; buttons; keys). The original runs in Unicorn on tac60 with real
DrawPanelElemAt, DrawText and MouseInRect on the VGA window; the stubs log. Tail stubs: CanTraverseEdge, TraverseCheck
(answers from the case), TraverseMove, StatusMessage (trav_diff compares those with the port).
The port: ja1f_trav_prompt = travprompt.rs, ja1f_trav_picture = strips::trav_popup. Compared at each poll: the lit word and
redraw flag, the 320 x 200 picture after a redraw; at the end: corner, pick code, tile, over_map, ShowMouse, the call order
in the original's tail, byte 0x92E4F restored. Exits 1 on mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402,F401

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"travprompt_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_trav_picture.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p]
PORT.ja1f_trav_prompt.argtypes = [ctypes.c_void_p] * 2 + [ctypes.c_int, ctypes.c_void_p]

e = Emu(dump=dump, fast=True)
rt = e.rt
SEL, CUR, TAB, TILE, OVER, E4F, RDO, MX, MY, MB = (0x284F11, 0x22A039, 0x22979E, 0x229D2C, 0x284F13, 0x92E4F, 0x92DDF,
                                                   0x229D00, 0x229D04, 0x229CF4)
VGA = e.r32(rt(0x95678))
base = e.r32(rt(0x100958))
FILL = bytes((i * 7 + (i >> 8)) & 0x7F | 0x40 for i in range(64000))
st = {}


def log(*a):
    st["log"].append(a)


def poll(em, *_):
    # Before the next poll: the picture now on the screen belongs to the poll before.
    if st["pos"] < len(st["tape"]):
        st["pics"].append(bytes(e.uc.mem_read(VGA, 64000)))
    t = st["tape"][min(st["pos"], len(st["tape"]) - 1)]
    st["pos"] += 1
    if st["pos"] > len(st["tape"]):
        st["overrun"] = True
    e.w32(rt(MX), t[0] & 0xFFFFFFFF)
    e.w32(rt(MY), t[1] & 0xFFFFFFFF)
    e.w32(rt(MB), t[2])
    st["key"] = t[3]
    return 0


def key_pressed(em, *_):
    return 1 if st["key"] >= 0 else 0


def get_key(em, *_):
    k = st["key"]
    st["key"] = -1
    return k & 0xFF


def hide(em, *_):
    log("hide")
    return 0


def show(em, *_):
    log("show", e.r8(rt(OVER)))
    return 0


def up(mask, name):
    def f(em, *_):
        e.w32(rt(MB), e.r32(rt(MB)) & ~mask)
        log(name)
        return 0
    return f


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


e.stub(rt(0x678E8), poll)
e.stub(rt(0x5616C), key_pressed)
e.stub(rt(0x560F4), get_key)
e.stub(rt(0x67BF0), hide)
e.stub(rt(0x673AC), show)
e.stub(rt(0x681D0), lambda em, a, d, *_: log("warp", s32(a), s32(d)))
e.stub(rt(0x176C8), lambda em, a, *_: log("redraw", a, s32(e.r32(rt(TILE))), e.r8(rt(E4F))))
e.stub(rt(0x23B4C), lambda em, a, *_: log("redraw", a, s32(e.r32(rt(TILE))), e.r8(rt(E4F))))
e.stub(rt(0x467E4), lambda em, a, d, b, *_: log("save", a, s32(d), s32(b)))
e.stub(rt(0x468B0), lambda em, a, d, b, *_: log("restore", a, s32(d), s32(b)))
e.stub(rt(0x67C54), up(1, "wait_l"))
e.stub(rt(0x67C68), up(2, "wait_r"))
e.stub(rt(0x30FA0), lambda em, a, d, *_: (log("edge", d & 0xFF), st["edge"])[1])
e.stub(rt(0x31344), lambda em, a, d, *_: (log("check", d & 0xFF), st["check"] & 0xFFFFFFFF)[1])
e.stub(rt(0x315DC), lambda em, *_: log("move"))
e.stub(rt(0x27D24), lambda em, a, *_: log("status", bytes(e.cstr(a))))

WORDS = [(28, 1), (16, 11), (42, 11), (28, 21)]
r = random.Random(1101)
pic_cache = {}


def picture(x0, y0, lit):
    k = (x0, y0, lit)
    if k not in pic_cache:
        b = ctypes.create_string_buffer(FILL, 64000)
        assert PORT.ja1f_trav_picture(ISO, EXE, x0, y0, lit, b) == 0
        pic_cache[k] = b.raw
    return pic_cache[k]


def point(x0, y0):
    c = r.randrange(6)
    if c <= 2:
        dx, dy = r.choice(WORDS)
        return x0 + dx + r.choice([-11, -10, -9, 0, 9, 10, 11]), y0 + dy + r.choice([3, 4, 5, 7, 9, 10, 11])
    if c == 3:
        return x0 + r.randrange(-5, 80), y0 + r.randrange(-5, 50)
    if c == 4:
        return r.choice([47, 48, 49, 271, 272, 0, 319, 160]), r.choice([0, 13, 14, 181, 182, 195, 196, 199, 100])
    return r.randrange(320), r.randrange(200)


snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
KEYS = [ord(c) for c in "NEWSnews"] + [0x1B, 0x41, 0x4F, 0x52, 0x54, 0x58, 0x1A, 0x1C, 0x57 + 1, 0xE0, 0x80]
bad = cases = opened = picks = pixels = 0
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    cases += 1
    sel = r.choice([0, 1, 2, 3, 7, 0, 1, 8, 9, 0x40, 0xFF])
    cur = e.r8(rt(CUR))
    mercs = r.choice([1, 1, 1, 2, 8, 0, 0x80, 0xFF])
    hp = r.choice([40, 40, 16, 16, 15, 14, 0, 0x7F, 0x80, 0xFF])
    pslot = sel if sel < 48 else 0
    button = r.randrange(2)
    cols, rows = r.choice([(14, 12), (20, 12), (20, 14), (20, 13)])
    top = 14 if rows == 12 else 0
    left = 48 if cols < 20 else 0
    over, tile = r.choice([0, 0, 1]), r.choice([0xC41, 500, 1500])
    rdo = r.randrange(2)
    e4f0 = r.randrange(2)
    e.w8(rt(SEL), sel)
    e.w8(rt(TAB) + cur * 9, mercs)
    e.w8(base + pslot * 512 + 0x183, hp)
    e.w8(rt(RDO), rdo)
    e.w8(rt(E4F), e4f0)
    e.w8(rt(OVER), over)
    e.w32(rt(TILE), tile)
    e.call(rt(0x24880), top, cols, rows)
    e.uc.mem_write(VGA, FILL)
    x0, y0 = (76, 11) if button else (126, 60)
    n = r.randrange(1, 12)
    tape = []
    for i in range(n):
        last = i == n - 1
        x, y = point(x0, y0)
        if last and r.random() < .6:
            x, y = x0 + 28, y0 + 8
        elif r.random() < .35 and tape:
            x, y = tape[-1][0], tape[-1][1]
        b = r.choice([0] * 12 + [1, 2, 3]) if not last else r.choice([1, 1, 1, 2, 3])
        k = r.choice(KEYS) if r.random() < .15 else -1
        tape.append((x, y, b, k))
    st.update(log=[], tape=tape, pos=0, key=-1, pics=[], overrun=False, edge=r.randrange(2), check=r.choice([1, 0, -1, 2]))
    try:
        e.call(rt(0x1CBA8), base + pslot * 512, button)
    except Exception as ex:
        print(f"case {case}: original stopped: {ex}")
        bad += 1
        continue
    # The port.
    state = (ctypes.c_int32 * 14)(sel, mercs, hp, button, over, tile, cols, rows, left, top)
    flat = (ctypes.c_int32 * (n * 4))(*[v for t in tape for v in t])
    out = (ctypes.c_int32 * (n * 3 + 2))()
    used = PORT.ja1f_trav_prompt(state, flat, n, out)
    errs = []
    gate = st["pos"] > 0 or any(a[0] == "hide" for a in st["log"])
    if gate != (used > 0):
        errs.append(f"gate orig {gate} port {used}")
    elif used > 0:
        opened += 1
        if st["pos"] != used:
            errs.append(f"polls orig {st['pos']} port {used}")
        if (state[10], state[11]) != (x0, y0):
            errs.append(f"corner port {state[10], state[11]} case {x0, y0}")
        saves = [a for a in st["log"] if a[0] == "save"]
        if saves != [("save", 12, state[10], state[11])]:
            errs.append(f"save {saves}")
        rest = [a for a in st["log"] if a[0] == "restore"]
        if rest != [("restore", 12, state[10], state[11])]:
            errs.append(f"restore {rest}")
        warps = [a for a in st["log"] if a[0] == "warp"]
        want = [] if button else [("warp", x0 + 45, y0 + 35)]
        if warps != want:
            errs.append(f"warp {warps}")
        # Each redraw's tile: the T key's and the close redraw see the tile off the map, the T key's with the flag old.
        rds = [a for a in st["log"] if a[0] == "redraw"]
        want = ([("redraw", 1, 0xC41, e4f0)] if not button else []) + [("redraw", 1, 0xC41, e4f0)]
        if rds != want:
            errs.append(f"redraws {rds}")
        for i in range(used):
            lit, red, done = out[i * 3], out[i * 3 + 1], out[i * 3 + 2]
            # pics[i] = the screen at the start of poll i. The port's lit word of poll i-1 is on it.
            if i < len(st["pics"]):
                shown = out[(i - 1) * 3] if i else -1
                pixels += 1
                if st["pics"][i] != picture(x0, y0, shown):
                    a, b = st["pics"][i], picture(x0, y0, shown)
                    j = next(j for j in range(64000) if a[j] != b[j])
                    errs.append(f"poll {i} lit {shown} pixel ({j % 320},{j // 320}) orig {a[j]:02x} port {b[j]:02x}")
                    break
        code, show_flag = out[used * 3], out[used * 3 + 1]
        tail = [a for a in st["log"] if a[0] in ("edge", "check", "move", "status")]
        if code >= 0:
            picks += 1
            want = [("edge", code)]
            if st["edge"]:
                want.append(("check", code))
                if st["check"] == 1:
                    want.append(("move",))
                elif st["check"] == 0:
                    want.append(("status", b"Can't traverse here!"))
            else:
                want.append(("status", b"Can't traverse here!"))
            if tail != want:
                errs.append(f"tail orig {tail} want {want}")
        elif tail:
            errs.append(f"tail with no pick {tail}")
        tile1, over1 = s32(e.r32(rt(TILE))), e.r8(rt(OVER))
        if (tile1, over1) != (state[12], state[13]):
            errs.append(f"tile/over orig {tile1:#x},{over1} port {state[12]:#x},{state[13]}")
        if e.r8(rt(E4F)) != e4f0:
            errs.append("0x92E4F not restored")
        shows_after = 0
        wl = next(i for i, a in enumerate(st["log"]) if a[0] == "wait_r")
        shows_after = sum(a[0] == "show" for a in st["log"][wl:])
        if shows_after != show_flag:
            errs.append(f"show orig {shows_after} port {show_flag}")
    if errs:
        bad += 1
        if bad <= 8:
            print(f"case {case} sel {sel} mercs {mercs} hp {hp} button {button} tape {tape}: {errs}")
print(f"travprompt_diff: {cases} cases, {opened} opened, {picks} picks, {pixels} pictures, {bad} mismatches")
sys.exit(1 if bad or opened < ncases // 8 or picks < ncases // 20 else 0)
