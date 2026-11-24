#!/usr/bin/env python3
"""Differential test: the edge drag and ALL-move loop, AllMoveTraverse? 0x1CFEC, original vs port.

usage: .venv/bin/python validate/diff/allmove_diff.py [ncases=1500] [DUMP] [--lib SO]
A case fuzzes the combat byte 0x92DCB, the round lock 0x92DE0, redraw_off 0x92DDF, the press point, cursor value and tile,
the Shift and Alt bytes, the caller's BL (confirm: UseItemAt 1, OverviewMouse 0), the answer of MoveMercTo and a tape of
polls (x, y, buttons, the left button after WaitRightUp, the edge answer). The original runs in Unicorn on tac60 from its
first line with real ClearTask. Stubs that log: MousePoll, CanTraverseEdge, TacticalRedraw, OverviewRedraw, WaitRightUp,
StatusMessage?, TraverseCheck, TraverseMove?, MoveMercTo. The port: ja1f_drag_run = drag.rs.
Compared: the call log in order with the value, tile and point at each redraw, polls used, the tail (end kind, direction,
the arguments of MoveMercTo), the final value and tile, the ALL marker, and the soldier record after the press's resets.
TraverseCheck answers -1 or 2 here: the tail after it (0, 1) belongs to ja1_traverse, which trav_diff compares.
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"allmove_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_drag_run.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]

e = Emu(dump=dump, fast=True)
rt = e.rt
VALUE, TILE, MX, MY, MB, COMBAT, LOCK, RDO, ALT, SHIFT = (0x284F1D, 0x229D2C, 0x229D00, 0x229D04, 0x229CF4, 0x92DCB, 0x92DE0,
                                                          0x92DDF, 0x284F20, 0x284F21)
MARK = (0x229CA8, 0x229CAC, 0x229D40, 0x229D20)
base = e.r32(rt(0x100958))
st = {}


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def log(*a):
    st["log"].append(a)


def poll(em, *_):
    log(6, e.r8(rt(VALUE)), 0, 0)
    t = st["tape"][min(st["pos"], len(st["tape"]) - 1)]
    st["pos"] += 1
    if st["pos"] > len(st["tape"]):
        st["overrun"] = True
    e.w32(rt(MX), t[0] & 0xFFFFFFFF)
    e.w32(rt(MY), t[1] & 0xFFFFFFFF)
    e.w32(rt(MB), t[2])
    st["row"] = t
    return 0


def wait_right(em, *_):
    log(4, 0, 0, 0)
    e.w32(rt(MB), st["row"][3] & 1)
    return 0


def edge(em, a, d, *_):
    log(1, d & 0xFF, 0, 0)
    return st["row"][4]


def tac(em, a, *_):
    log(2, e.r8(rt(VALUE)), None, None)
    log(5, s32(e.r32(rt(TILE))), 0, 0)
    assert a & 0xFF == 1
    return 0


def over(em, a, d, b, c, *_):
    assert a & 0xFF == 1
    assert d & 0xFF == e.r8(rt(VALUE))
    log(2, d & 0xFF, s32(b), s32(c))
    log(5, s32(e.r32(rt(TILE))), 0, 0)
    return 0


def check(em, a, d, *_):
    st["tail"] = ("check", d & 0xFF)
    return st["check"] & 0xFFFFFFFF


def status(em, a, *_):
    t = bytes(e.cstr(a))
    if st.get("tail") or t == b"Can't traverse here!":
        st["status"] = t
    else:
        log(3, 0, 0, 0)
    return 0


def move(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    args = [ctypes.c_int8(e.r8(sp + 4 + 4 * i)).value for i in range(4)]
    st["tail"] = ("move", s32(d), b & 0xFF, ctypes.c_int8(c & 0xFF).value, *args)
    return st["move"] & 0xFFFFFFFF


e.stub(rt(0x678E8), poll)
e.stub(rt(0x67C68), wait_right)
e.stub(rt(0x30FA0), edge)
e.stub(rt(0x176C8), tac)
e.stub(rt(0x23B4C), over)
e.stub(rt(0x27D24), status)
e.stub(rt(0x31344), check)
e.stub(rt(0x315DC), lambda em, *_: st.__setitem__("moved", True) or 0)
e.stub(rt(0x1C608), move, pop=16)

r = random.Random(1103)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
bad = cases = ends = 0
kinds = [0, 0, 0, 0]
alls = marks = redraws = waits = 0
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    cases += 1
    combat, lock, rdo = r.choice([0, 0, 0, 1]), r.choice([0, 0, 0, 0, 1]), r.randrange(2)
    px, py = r.randrange(40, 280), r.randrange(40, 160)
    value, tile0 = r.choice([0x8C, 0x8E, 0xA1, 0x91, 0x0F]), r.choice([0xC41, 500, 1500])
    tile = r.choice([302, 1500, 2990, 28])
    shift, alt, bl = r.choice([0, 1, 0x80]), r.choice([0, 1, 0x80]), r.randrange(2)
    move_res = r.choice([0, 1, 1, 2, -1])
    n = r.randrange(1, 10)
    tape = []
    x, y = px, py
    for i in range(n):
        last = i == n - 1
        c = r.randrange(5)
        if c == 0:
            x, y = px + r.choice([-18, -17, -16, 0, 16, 17, 18]), py + r.choice([-18, -17, -16, 0, 16, 17, 18])
        elif c == 1:
            x, y = x + r.randrange(-20, 21), y + r.randrange(-20, 21)
        elif c == 2:
            x, y = px, py
        b = (0 if last else 1) if r.random() < .8 else r.choice([1, 3, 2, 0] if not last else [0, 2])
        tape.append((x, y, b, 0 if last else r.choice([1, 1, 1, 0]), r.choice([1, 1, 0])))
    if r.random() < .15:
        # An edge test fails, a later one passes, the mouse comes back: the failed flag must be off at the release.
        fy = py - 20
        tape = [(px, fy, 1, 1, 0), (px, fy - 20, 1, 1, 1), (px, fy, 1, 1, 1), (px, fy, 0, 0, 1)]
        n = 4
    sbase = base
    noise = bytes(r.randrange(256) for _ in range(512))
    e.uc.mem_write(sbase, noise)
    e.w8(rt(COMBAT), combat)
    e.w8(rt(LOCK), lock)
    e.w8(rt(RDO), rdo)
    e.w32(rt(MX), px)
    e.w32(rt(MY), py)
    e.w8(rt(VALUE), value)
    e.w32(rt(TILE), tile0)
    e.w8(rt(ALT), alt)
    e.w8(rt(SHIFT), shift)
    e.w32(rt(0x229CA8), 0)
    e.w32(rt(0x229CAC), 0)
    e.w32(rt(0x229D40), 0)
    e.w32(rt(0x229D20), 0)
    chk = r.choice([-1, 2])
    st.clear()
    st.update(log=[], tape=tape, pos=0, overrun=False, check=chk, move=move_res, row=tape[0])
    errs = []
    try:
        e.call(rt(0x1CFEC), sbase, tile, bl)
    except Exception as ex:
        print(f"case {case}: original stopped: {ex}")
        bad += 1
        continue
    # The port.
    state = (ctypes.c_int32 * 27)(tile, px, py, value, tile0, combat, lock, ctypes.c_int8(shift).value, ctypes.c_int8(alt).value,
                                  bl, move_res)
    flat = (ctypes.c_int32 * (n * 5))(*[v for t in tape for v in t])
    rec = ctypes.create_string_buffer(noise, 512)
    cap = 200
    out = (ctypes.c_int32 * (cap * 4))()
    used = PORT.ja1f_drag_run(state, flat, n, rec, out, cap)
    plog = [tuple(out[i * 4:i * 4 + 4]) for i in range(used)]
    if st["overrun"]:
        errs.append("the original ran past the tape")
    if st["pos"] != state[26]:
        errs.append(f"polls orig {st['pos']} port {state[26]}")
    olog = st["log"]
    if len(olog) != len(plog):
        errs.append(f"log length orig {olog} port {plog}")
    else:
        for a, b in zip(olog, plog):
            if a[0] == 2 and a[2] is None:
                a, b = a[:2], b[:2]
            if tuple(a) != tuple(b):
                errs.append(f"log orig {olog} port {plog}")
                break
    redraws += sum(a[0] == 2 for a in olog)
    waits += sum(a[0] == 4 for a in olog)
    value1, tile1 = e.r8(rt(VALUE)), s32(e.r32(rt(TILE)))
    if (value1, tile1) != (state[11], state[12]):
        errs.append(f"value/tile orig {value1:#x},{tile1} port {state[11]:#x},{state[12]}")
    tail = st.get("tail")
    msg = st.get("status")
    code = state[13]
    if code < 0:
        errs.append("the port did not end")
    else:
        kinds[code] += 1
    if code == 0 and (tail or msg):
        errs.append(f"refused: tail {tail} {msg}")
    elif code == 1 and (tail != ("check", state[14]) or msg or st.get("moved")):
        errs.append(f"traverse: orig {tail} {msg} port {state[14]}")
    elif code == 2 and (tail or msg != b"Can't traverse here!"):
        errs.append(f"no edge: orig {tail} {msg}")
    elif code == 3:
        want = ("move", tile, state[20], state[21], state[22], state[23], bl, state[24])
        if tail != want or msg:
            errs.append(f"walk: orig {tail} {msg} port {want}")
        alls += state[15]
    mk = tuple(s32(e.r32(rt(a))) for a in MARK)
    if mk != tuple(state[16:20]):
        errs.append(f"marker orig {mk} port {tuple(state[16:20])}")
    marks += mk[0] != 0
    orec = bytes(e.uc.mem_read(sbase, 512))
    if orec != rec.raw[:512]:
        j = next(j for j in range(512) if orec[j] != rec.raw[j])
        errs.append(f"record +{j:#x} orig {orec[j]:02x} port {rec.raw[j]:02x}")
    if errs:
        bad += 1
        if bad <= 8:
            print(f"case {case} combat {combat} lock {lock} rdo {rdo} press {px, py} tape {tape}: {errs}")
print(f"allmove_diff: {cases} cases, ends refused/traverse/no edge/walk {kinds}, {alls} ALL walks, {marks} markers, "
      f"{redraws} redraws, {waits} right waits, {bad} mismatches")
sys.exit(1 if bad or min(kinds) < ncases // 50 or alls < ncases // 50 or marks < ncases // 100 else 0)
