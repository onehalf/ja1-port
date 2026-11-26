#!/usr/bin/env python3
"""Differential test: the mouse on the merc slots (SlotMouse 0x11714 + its frame in TacticalKeyHandler 0x1463A .. 0x14721,
and the on-map path 0x14225), original vs port.

usage: .venv/bin/python validate/diff/slotmouse_diff.py [ncases=2000] [DUMP] [--port] [--lib SO] [--fan 61]
--fan 61 (#70 F61, row 87): the original runs the 1.15m call 0x11B24 -> cave 0x8AF04 (CursorItemBack? 0x53660,
stubbed and logged as call 28, then SelectMerc?); the port runs with fan fix F61 on.
A case fuzzes the bytes 0x92DC8, 0x93448, 0x284F11 and Alt, the record fields SlotMouse reads, the mouse point (biased to
rows, bars, faces, switches and the edges) and the buttons. The original runs in Unicorn with real SlotMouse,
ClearStripHover, SetSlotView and hit tests, entered mid-function; the stubs log the calls in order and MousePoll reads
the tape. WaitLeftUp / WaitRightUp only log. Cases with the byte 0x92DCC at 0 enter at the gate 0x141EC, and part of the
on-map cases enter there with the byte at 1 and the real MouseInRect; all others hold it at 1. g_redrawOff stays 0 or 1. The port:
ja1f_slot_mouse = slotmouse.rs. Compared: the calls with their arguments, the four state bytes, all 8 records.
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN_IDS = sorted(int(x.lstrip("F")) for x in argv[argv.index("--fan") + 1].split(",")) if "--fan" in argv else []
ncases = int(args[0]) if args else 2000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"slotmouse_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB) if "--port" in argv else None
if PORT:
    PORT.ja1_fan_fixes_set.argtypes = [ctypes.c_uint64]
    PORT.ja1_fan_fixes_set(sum(1 << (k - 1) for k in FAN_IDS))

CONT, HOVER, LAST, OVER, SEL, ALT, MX, MY, MB, SOLD = (0x92DC8, 0x93448, 0x92E58, 0x284F16, 0x284F11, 0x284F20, 0x229D00,
                                                       0x229D04, 0x229CF4, 0x100958)
e = Emu(dump=dump, fast=True)
rt = e.rt
FAN_RUNS = {61: [(0x11B24, 0x11B29)]}
if FAN_IDS:
    e.put_115m(only=[r for k in FAN_IDS for r in FAN_RUNS[k]])
base = e.r32(rt(SOLD))
st = {}


def slot_of(p):
    return ((p - base) // 512) if p else -1


def stack(i):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def log(*a):
    st["log"].append(tuple(a))


def poll(em, *_):
    t = st["tape"][min(st["pos"], len(st["tape"]) - 1)]
    st["pos"] += 1
    e.w32(rt(MX), t[0])
    e.w32(rt(MY), t[1])
    e.w32(rt(MB), t[2])
    return 0


def up(mask, op):
    def f(em, *_):
        e.w32(rt(MB), e.r32(rt(MB)) & ~mask)
        log(op)
        return 0
    return f


e.stub(rt(0x331A4), lambda em, a, d, *_: log(1, a & 0xFF, d & 0xFF))
e.stub(rt(0x46C44), lambda em, a, d, b, c, *_: log(2, s32(b), s32(c), s32(stack(0)), s32(stack(1))), pop=12)


def copy(em, a, *_):
    # SetSlotView's own copies are its animation: the port's log has only its end.
    ret = e.r32(e.uc.reg_read(UC_X86_REG_ESP))
    if not rt(0x332E4) <= ret < rt(0x333B8):
        log(3, a & 0xFF)
    return 0


e.stub(rt(0x33204), copy)
e.stub(rt(0x358D0), lambda em, a, d, b, c, *_: log(4, s32(c), s32(stack(0))), pop=24)
e.stub(rt(0x3FF3C), lambda em, a, *_: log(10, a & 0xFF))
e.stub(rt(0x4B730), lambda em, a, *_: log(11, a))
e.stub(rt(0x4B764), lambda em, *_: log(12))
e.stub(rt(0x67C54), up(1, 13))
e.stub(rt(0x67C68), up(2, 22))
e.stub(rt(0x1D8B8), lambda em, *_: log(14))
e.stub(rt(0x59AE8), lambda em, a, d, b, c, *_: (log(15, slot_of(a), s32(d), s32(b), c & 0xFF, stack(0)), st["cost"])[1], pop=4)
e.stub(rt(0x15960), lambda em, a, d, b, *_: (log(16, slot_of(a), d & 0xFF, b), st["enough"])[1])
e.stub(rt(0x1C608), lambda em, a, d, b, c, *_: log(17, slot_of(a), s32(d), b, s32(c), s32(stack(0)), s32(stack(1)), stack(2) | stack(3)), pop=16)
e.stub(rt(0x59C14), lambda em, a, d, b, *_: log(18, slot_of(a), s32(d), b))
e.stub(rt(0x1148C), lambda em, a, d, b, *_: log(19, s32(a), s32(d), b))
e.stub(rt(0x53660), lambda em, *_: log(28))     # CursorItemBack? (1.15m cave 0x8AF04 only)
e.stub(rt(0x1CB78), lambda em, *_: log(20))
e.stub(rt(0x1FC34), lambda em, *_: st["onscr"])
e.stub(rt(0x682AC), lambda em, a, d, b, c, *_: log(23, s32(a), s32(d), s32(b), s32(c)))
e.stub(rt(0x32FFC), lambda em, a, d, b, *_: log(24, a & 0xFF, s32(d), s32(b)))
e.stub(rt(0x5CB34), lambda em, a, d, b, c, *_: log(25, a, d, b, c))
e.stub(rt(0x678E8), poll)
# ReserveAp, then SetSlotView's inner calls.
e.stub(rt(0x17C88), lambda em, a, *_: log(26, slot_of(a)))
e.stub(rt(0x330C0), lambda em, a, *_: log(21, e.r8(a + 0x17F), 1))
e.stub(rt(0x32EB8), lambda em, a, *_: log(21, e.r8(base + (a & 0xFF) * 512 + 0x17F), 0))
for a in (0x67BF0, 0x673AC, 0x67250, 0x24630, 0x32E74):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x14721), lambda em, *_: 0)
e.stub(rt(0x141F5), lambda em, *_: st.update(gate_closed=1) or 0)   # the return of the gate 0x141EC

FIELDS = (0x138, 0x17E, 0x180, 0x182, 0x183, 0x194, 0x16A, 0x16B, 0x16C, 0x16D, 0x17F, 0x95, 0x1A6, 0x177, 0x196, 0x1A8)
ORIGIN = [(0 if k < 4 else 272, 14 + 42 * (k % 4)) for k in range(8)]
r = random.Random(33)
for k in range(8):
    if e.r32(rt(SOLD) + 4 * k) != base + k * 512:
        print("slotmouse_diff: the soldier table is not in order")
        sys.exit(1)
OFFM = bytes(e.uc.mem_read(base, 8 * 512))


def point(k):
    x0, y0 = ORIGIN[k]
    col = e.r8(base + k * 512 + 0x180)
    c = r.randrange(9)
    if c == 0:
        return x0 + r.randrange(48), y0 + r.randrange(42)
    if c <= 2:                            # the bar area with its edges
        bx = x0 + (3 if col == 0 else 35)
        return bx + r.choice((-1, 0, 1, 5, 10, 11)), y0 + 4 + r.choice((-1, 0, 13, 26, 27))
    if c <= 5:                            # the face
        return x0 + (20 if col == 0 else 10), y0 + r.randrange(42)
    if c == 6:                            # a screen edge
        return r.choice((0, 1, 318, 319, 47, 48, 271, 272)), y0 + r.randrange(42)
    if c == 7:                            # a row edge
        return x0 + 20, r.choice((y0 - 1, y0, y0 + 41, y0 + 42))
    return r.randrange(320), r.randrange(200)


def switch_point(k):
    x0, y0 = ORIGIN[k]
    i = r.randrange(4)
    return x0 + r.choice((0x22, 0x23, 0x27, 0x2B, 0x2C)), y0 + 5 + 9 * i + r.choice((-1, 0, 2, 5, 6))


calls_seen = {}
gate_cases = [0, 0]
RECT = (0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC)
bad = 0
for case in range(ncases):
    e.uc.mem_write(base, OFFM)
    k = r.randrange(8)
    sel = k if r.random() < 0.6 else r.randrange(8)
    for j in range(8):
        a = base + j * 512
        e.w8(a + 0x17A, j)
        e.w8(a + 0x180, r.randrange(2))
        e.w8(a + 0x182, 0 if r.random() < 0.06 else 1)
        e.w8(a + 0x194, 1 if r.random() < 0.06 else 0)
        e.w8(a + 0x138, 0 if r.random() < 0.06 else 1)
        e.w8(a + 0x183, r.choice((0, 1, 30, 30, 30, 30, 0x90, 0xFF)))
        e.w8(a + 0x17E, 1 if r.random() < 0.1 else 0)
        e.w8(a + 0x17F, r.choice((0, 0, 0, 1, 5)))
        e.w8(a + 0x16D, r.randrange(2))
        for o in (0x16A, 0x16B, 0x16C):
            e.w8(a + o, r.randrange(2))
        e.w8(a + 0x95, r.choice((0, 0, 3, 7, 0xFD, 0xFE, 0xFF)))
        t = r.randrange(0, 3000)
        e.w16(a + 0x114, t)
        e.w16(a + 0x120, r.choice((t, 0xC41, r.randrange(3000))))
        e.w16(a + 0x96, r.randrange(3000))
        for o in (0x1A8, 0x177, 0x196):
            e.w8(a + o, r.randrange(3))
    cont = r.choice((0x40, 0x40, k, k, sel, r.randrange(8)))
    hover = r.choice((0x40, 0x40, k, k, r.randrange(8), 0x20))
    alt = r.randrange(2)
    on_map = r.random() < 0.08
    x, y = point(k)
    buttons = r.choice((0, 0, 1, 1, 2, 2, 3))
    tape = [(x, y, buttons)]
    if buttons:
        for _ in range(r.randrange(6)):
            tape.append(switch_point(k) + (r.choice((0, 1, 1)),))
        tape.append((x, y, 2))
    st.update(log=[], tape=tape[1:] or [(x, y, 2)], pos=0, onscr=r.randrange(2), cost=r.randrange(1, 40), enough=r.randrange(2))
    st["before"] = bytes(e.uc.mem_read(base, 8 * 512))
    st["ground"] = {}
    a = base + sel * 512
    t2 = e.r16(a + 0x96)
    st["ground"][sel] = e.r8(rt(0x100A1E) + t2 * 8)
    e.w8(rt(CONT), cont)
    e.w8(rt(HOVER), hover)
    e.w8(rt(SEL), sel)
    e.w8(rt(ALT), alt)
    e.w8(rt(LAST), 0x40)
    e.w8(rt(OVER), 0)
    screen_off = r.random() < 0.1
    e.w8(rt(0x92DCC), 0 if screen_off else 1)
    st["gate_closed"] = 0
    e.w32(rt(MX), x)
    e.w32(rt(MY), y)
    e.w32(rt(MB), buttons)
    mode2 = not on_map and r.random() < 0.05
    if screen_off:
        e.w8(rt(0x92DDF), 0)
        e.call(rt(0x141EC))
        assert st["gate_closed"], "the gate 0x141EC did not close"
        gate_cases[0] += 1
    elif on_map:
        e.w8(rt(0x92DDF), 0)
        if r.random() < 0.5:                  # through the gate: the whole screen is the map rectangle
            rect = [e.r32(rt(a)) for a in RECT]
            for a, v in zip(RECT, (0, 0, 319, 199)):
                e.w32(rt(a), v)
            e.call(rt(0x141EC))
            for a, v in zip(RECT, rect):      # the slot columns at 0x1463A read this rectangle too
                e.w32(rt(a), v)
            assert not st["gate_closed"], "the gate 0x141EC closed with the byte at 1"
            gate_cases[1] += 1
        else:
            e.call(rt(0x14225))
    else:
        e.w8(rt(0x92DDF), 2 if mode2 else r.randrange(2))
        e.call(rt(0x1463A))
    orig_log = [tuple(c) for c in st["log"]]
    orig_state = (e.r8(rt(CONT)), e.r8(rt(HOVER)), e.r8(rt(LAST)), e.r8(rt(OVER)))
    orig_rec = bytes(e.uc.mem_read(base, 8 * 512))
    for c in orig_log:
        calls_seen[c[0]] = calls_seen.get(c[0], 0) + 1
    if not PORT:
        continue
    rec = (ctypes.c_ubyte * (8 * 512)).from_buffer_copy(st["before"])
    state = (ctypes.c_int * 7)(cont, hover, 0x40, 0, sel, alt, 3 if screen_off else (1 if on_map else (2 if mode2 else 0)))
    ptape = [v for t in tape for v in t]
    tp = (ctypes.c_int * len(ptape))(*ptape)
    ans = (ctypes.c_int * 4)(st["onscr"], st["cost"], st["enough"], st["ground"].get(sel, 0))
    out = (ctypes.c_int * 8 * 256)()
    n = PORT.ja1f_slot_mouse(rec, state, tp, len(tape), ans, out, 256)
    if n < 0:
        print(f"case {case}: port error {n}")
        sys.exit(1)
    port_log = [tuple(out[i][:{1: 3, 2: 5, 3: 2, 4: 3, 10: 2, 11: 2, 12: 1, 13: 1, 14: 1, 15: 6, 16: 4, 17: 8, 18: 4, 19: 4, 20: 1, 21: 3, 22: 1,
                               23: 5, 24: 4, 25: 5, 26: 2, 28: 1}.get(out[i][0], 8)]) for i in range(n)]
    port_state = (state[0], state[1], state[2], state[3])
    port_rec = bytes(rec)
    diffs = []
    if orig_log != port_log:
        i = next((i for i, (a, b) in enumerate(zip(orig_log, port_log)) if a != b), min(len(orig_log), len(port_log)))
        diffs.append(f"call {i}: orig {orig_log[i:i + 3]} port {port_log[i:i + 3]} (orig {len(orig_log)} port {len(port_log)})")
    if orig_state != port_state:
        diffs.append(f"state orig {orig_state} port {port_state}")
    if orig_rec != port_rec:
        o = next(i for i in range(len(orig_rec)) if orig_rec[i] != port_rec[i])
        diffs.append(f"record {o // 512} +{o % 512:#x}: orig {orig_rec[o]} port {port_rec[o]}")
    if diffs:
        bad += 1
        if bad <= 8:
            print(f"case {case}: k {k} sel {sel} cont {cont:#x} hover {hover:#x} on_map {on_map} tape {tape[:4]}: " + "; ".join(diffs) + (f" ORIG {orig_log} PORT {port_log}" if len(orig_log) < 12 else ""))
print("calls seen", sorted(calls_seen.items()), "gate closed", gate_cases[0], "gate open", gate_cases[1])
print(f"slotmouse_diff: {ncases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
