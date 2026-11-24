"""Differential test: the panel buttons of the assignment screen (strategic.md "The panel buttons"), original vs
port.
usage: .venv/bin/python validate/diff/assign_panel_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case as validate/diff/select_diff.py (fuzzed team, picture, store view / records / counts), plus the shown types
0x93DD0, button 14's overlay V 0x934ED, the last listed slot 0x229C20 and up to 8 store records. Script: a press on
a face box selects a merc, then up to 6 presses (each left down, then up at the same place; a button press uses
one more up poll): the item panel's face (deselect, ends the script), its body, buttons 10..14, or a place that
holds nothing. The original runs AssignScreen? 0x4891C in Unicorn (input polls 0x678E8 and 0x6782C scripted)
until the script ends. The port = port/ja1front ja1f_assign_panel. Compared: the screen and the panel buffer, team
slots 0..8, the selected slot, the store view, current record, top type, shown types, V, the loader result and the
last mouse move. Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402
import assign_fuzz as fz             # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_assign_panel.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 6 + [ctypes.c_int32] + \
        [ctypes.c_void_p] * 2 + [ctypes.c_int32] * 2 + [ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_assign_panel.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SOLDIERS, FACES, ACC, STATCH, PANEL_P, SCREEN_P = 0xFA958, 0x93428, 0x229FA0, 0x10DD10, 0x93E60, 0x95678
MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4
MODE, LIST, COUNTS, TOP, SHOWN, SELECTED, LOADRES = 0x93EA4, 0x229C7C, 0x93DD4, 0x229C88, 0x93DD0, 0x229C38, 0x229D18
HEAD, V, LAST, CURSOR = 0x229C78, 0x934ED, 0x229C20, 0x222B4C
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
            _pics[n] = (w, h, px)
        except (KeyError, ValueError):
            _pics[n] = None
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    p = pic(e.cstr(ebx))
    if p is None:
        return 0xFFFFFFFF
    w, h, px = p
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


st = {}


def poll(em, *_):
    if not st:
        return 0
    j = st["polls"]
    st["polls"] = j + 1
    if j >= len(st["ev"]):
        em.stop_reason = "script end"
        em.uc.emu_stop()
        return 0
    x, y, b = st["ev"][j]
    e.w32(e.rt(MOUSE_X), x)
    e.w32(e.rt(MOUSE_Y), y)
    e.w8(e.rt(BUTTONS), b)
    return 0


def mouse_to(em, eax, edx, *_):
    st["warp"] = (eax, edx)
    e.w32(e.rt(MOUSE_X), eax)
    e.w32(e.rt(MOUSE_Y), edx)
    return 0


for a in (0x67BF0, 0x673AC, 0x67E18, 0x67E30, 0x67478, 0x47AE4, 0x5C8A0, 0x5C880, 0x5616C, 0x28698, 0x5C900,
          0x6742C, 0x67444):
    e.stub(e.rt(a), lambda em, *_: 0)
def tip(em, *_):
    st["tip"] = st.get("tip", 0) + 1
    if os.environ.get("PANEL_V"): print("tip at poll", st["polls"], st["ev"])
    return 0
for a in (0x23264, 0x23420):
    e.stub(e.rt(a), tip)
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x6782C), poll)
e.stub(e.rt(0x681D0), mouse_to)
e.stub_sprintf()
e.stub(e.rt(0x532BC), load, 4)
panel = e.r32(e.rt(PANEL_P))
screen = e.r32(e.rt(SCREEN_P))
faces = [e.r32(e.rt(FACES) + 4 * k) for k in range(8)]
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))
e.uc.mem_write(e.rt(0x949F2), bytes(e.uc.mem_read(e.rt(0x94A72), 0x38)))
NODES = e.alloc(bytes(7 * 16))
TYPES = list(range(0, 0x71))


def fuzz_items(r, s):
    for k in range(12):
        a = 12 * k
        if r.random() < .3:
            s[a:a + 3] = bytes(3)
            continue
        t = r.choice(TYPES)
        n = r.choice([0, 1, 2, 5, 9, 10, 50, 98, 99, 100, 0xFF, 0xFE, r.randrange(256)])
        c = r.choice([0, 1, 5, 9, 10, 11, 50, 99, 100, 0xFF, 0xEC, r.randrange(101)])
        s[a:a + 3] = bytes([t, n, c])
    if r.random() < .5:
        s[3:6] = bytes([r.choice(TYPES), r.randrange(256), r.choice([0, 1, 9, 10, 50, 100, 0xFF])])
    if s[3] > 0x70:          # types past the item table (the port reads 0 there)
        s[3] = r.choice(TYPES)
    if 110 <= s[5] <= 127:   # the original's condition bar never ends
        s[5] = 100
    if r.random() < .5:
        s[0x48] = r.choice([0, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36])
    for o, arm in ((0x18, [0x23, 0x24, 0x25, 0x26]), (0x3C, [0x2D, 0x2E, 0x2F, 0x30]), (0x48, [0x31])):
        if r.random() < .5:
            s[o:o + 3] = bytes([r.choice(arm), 1, r.choice([1, 5, 50, 99, 100, r.randrange(1, 101)])])
    s[0x17C] = r.choice([r.randrange(11), r.randrange(11), r.randrange(256)])
    s[0x1B1] = r.choice([0, 0, 1, 7])


def fuzz_store(r):
    counts = bytearray(136)
    for _ in range(r.choice([0, 1, 2, 3, 4, 5, 8, 20])):
        t = r.choice([0, 1, 0x86, 0x87, r.randrange(136), r.randrange(136)])
        counts[t] = r.choice([1, 2, 9, 10, 50, 99, r.randrange(1, 256)])
    recs = []
    for _ in range(r.choice([0, 1, 2, 3, 4, 4, 5, 6, 8, 12])):
        recs.append(bytes([r.choice(TYPES), r.choice([0, 1, 5, 0xFF, 0xFE, r.randrange(256)]),
                           r.choice([0, 1, 9, 10, 11, 50, 100, 0xF6, r.randrange(110), r.randrange(128, 256)])]))   # 110..127: endless bar
    return counts, recs


BUTTONS_XY = [(8, 25), (69, 25), (79, 72), (79, 108), (79, 90)]
PLACES = [(108, 73), (203, 89), (161, 8), (108, 31), (200, 28), (108, 52), (257, 18), (257, 37), (257, 56),
          (257, 75), (257, 94), (257, 113), (108, 92), (159, 115)]
def busy(x, y):
    """The mouse at (x, y) is on something the panel answers (or the exit button)."""
    rects = [(34, 12, 62, 36), (161, 48, 199, 84), (25, 60, 72, 132), (269, 166, 294, 181)]
    rects += [(bx, by, bx + 19, by + 15) for bx, by in BUTTONS_XY]
    rects += [(qx - 4, qy - 4, qx + 45, qy + 19) for qx, qy in PLACES]
    return any(a <= x <= c and b <= y <= d for a, b, c, d in rects)
r = random.Random(int(os.environ.get("PANEL_SEED", 1)))
edge = random.Random(2)
bad = 0
outcomes = {}
for i in range(ncases):
    n = r.choice([1, 2, 3, 4, 5, 6, 7, 8, 8])
    sol = bytearray(base_sol)
    for k in range(9):
        s = bytearray(sol[k * 0x200:(k + 1) * 0x200])
        fz.fuzz_soldier(r, s, k, n)
        fuzz_items(r, s)
        sol[k * 0x200:(k + 1) * 0x200] = s
    if r.random() < 0.1:
        sol[r.randrange(8) * 0x200 + 0x182] ^= 1
    fc = [bytes(r.randrange(256) for _ in range(1024)) for _ in range(8)]
    sc = bytes(1 if r.random() < 0.2 else 0 for _ in range(640))
    acc = bytearray(base_acc)
    for o in (0x24, 0x28, 0x30, 0x34, 0x38):
        acc[o:o + 4] = r.randint(-99999, 999999).to_bytes(4, "little", signed=True)
    mode = 1 if r.random() < 0.6 else 0
    counts, recs = fuzz_store(r)
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
    for k in range(8):
        e.uc.mem_write(faces[k], fc[k])
    e.uc.mem_write(e.rt(STATCH), sc)
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.uc.mem_write(panel, bytes(r.randrange(256) for _ in range(64000)))
    e.w32(e.rt(MODE), mode)
    e.uc.mem_write(e.rt(COUNTS), bytes(counts))
    for j, rec in enumerate(recs):
        nxt = NODES + 7 * (j + 1) if j + 1 < len(recs) else 0
        e.uc.mem_write(NODES + 7 * j, rec + struct.pack("<I", nxt))
    e.w32(e.rt(LIST), NODES if recs else 0)
    e.w32(e.rt(LOADRES), 0x55)
    e.w32(e.rt(HEAD), NODES if recs else 0)
    cur = r.choice([0, 0, 1, 2, len(recs), r.randrange(len(recs) + 1)]) if recs else 0
    cur = min(cur, len(recs))
    e.w32(e.rt(LIST), NODES + 7 * cur if cur < len(recs) else 0)
    shown = bytes(r.choice([0, 1, r.randrange(136), r.choice(recs)[0] if recs else 0]) for _ in range(4))
    e.uc.mem_write(e.rt(SHOWN), shown)
    vb = r.choice([0x38, 0x39, 0x38 if mode else 0x39])
    e.w8(e.rt(V), vb)
    e.w8(e.rt(CURSOR), 0)
    e.w32(e.rt(LAST), r.randrange(8))
    st.clear()
    e.call(e.rt(0x478A4), 99)
    last = e.r32(e.rt(LAST))
    pan = bytearray(e.uc.mem_read(panel, 64000))
    scr = bytearray(pan)
    for y in (0, 143, 144, 145):
        scr[y * 320:(y + 1) * 320] = bytes(r.randrange(256) for _ in range(320))
    e.uc.mem_write(screen, bytes(scr))
    sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    k = r.randrange(n) if r.random() < 0.9 else r.randrange(8)
    fx = 11 + 37 * k
    px, py = r.randint(fx, fx + 30), r.randint(20, 46)
    ev = [(px, py, 1), (r.randint(0, 319), r.randint(0, 199), 0)]
    presses, kinds = [], []
    for _ in range(r.choice([1, 2, 3, 4, 6]) if sol[k * 0x200 + 0x182] else 0):   # 0x182 = inCurSector
        kind = r.choice(["b10", "b11", "b12", "b13", "b14", "b12", "b13", "b14", "face", "body", "none"])
        if kind == "face":
            x, y = r.randint(34, 62), r.randint(12, 36)
        elif kind == "body":
            x, y = r.randint(161, 199), r.randint(48, 84)
        elif kind == "none":
            while True:
                x, y = r.randrange(320), r.randrange(200)
                if not busy(x, y):
                    break
        else:
            bx, by = BUTTONS_XY[int(kind[1:]) - 10]
            x, y = r.randint(bx, bx + 19), r.randint(by, by + 15)
            c = edge.random()                    # own stream: the old cases keep their numbers
            if c < 0.3:                          # a corner: still the button
                x, y = edge.choice([bx, bx + 19]), edge.choice([by, by + 15])
            elif c < 0.6:                        # one pixel outside: nothing, unless something else lies there
                ox, oy = edge.choice([(bx - 1, y), (bx + 20, y), (x, by - 1), (x, by + 16)])
                if not busy(ox, oy):
                    x, y, kind = ox, oy, "none"
        presses.append((x, y))
        kinds.append(kind)
        ev += [(x, y, 1), (x, y, 0)] + ([(x, y, 0)] if kind.startswith("b1") else [])
        if kind == "face":
            break
    st.clear()
    st.update(polls=0, ev=ev, warp=None)
    try:
        e.call(e.rt(0x4891C), max_steps=80_000_000)
        print(f"case {i}: the original returned")
        bad += 1
        continue
    except EmuStop as x:
        if e.stop_reason != "script end":
            print(f"case {i}: original stopped: {x} k {k} n {n} polls {st['polls']} eip {e.uc.reg_read(__import__('unicorn').x86_const.UC_X86_REG_EIP):#x}")
            bad += 1
            continue
    if st.get("tip"):
        print(f"case {i}: the original reached a tooltip (script out of step) {kinds}")
        bad += 1
        continue
    osel = e.r32(e.rt(SELECTED))
    lp = e.r32(e.rt(LIST))
    ocur = (lp - NODES) // 7 if lp else len(recs)
    w = st["warp"]
    o = (osel, e.r32(e.rt(MODE)), ocur, e.r32(e.rt(TOP)), bytes(e.uc.mem_read(e.rt(SHOWN), 4)), e.r8(e.rt(V)),
         struct.unpack("<i", e.uc.mem_read(e.rt(LOADRES), 4))[0] if w else -2, w[0] * 1000 + w[1] if w else -1)
    oscr, opan = bytes(e.uc.mem_read(screen, 64000)), bytes(e.uc.mem_read(panel, 64000))
    osol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    for kd in kinds:
        outcomes[kd] = outcomes.get(kd, 0) + 1
    if PORT:
        ps = ctypes.create_string_buffer(sol, 9 * 0x200)
        pp, psc = ctypes.create_string_buffer(bytes(pan), 64000), ctypes.create_string_buffer(bytes(scr), 64000)
        stt = (ctypes.c_int32 * 12)(mode, cur, 0, *shown, vb, last, 0, 0, 0)
        pr = (ctypes.c_int32 * (2 * len(presses)))(*[c for xy in presses for c in xy])
        rc = PORT.ja1f_assign_panel(ISO, EXE, ps, b"".join(fc), sc, bytes(acc), bytes(counts), b"".join(recs),
                                    len(recs), pp, psc, px, py, pr, len(presses), stt)
        p = (stt[9], stt[0], stt[1], stt[2], bytes(stt[3 + q] & 0xFF for q in range(4)), stt[7],
             stt[10] if stt[11] >= 0 else -2, stt[11])
        msg = []
        if rc != 0 or p != o:
            msg.append(f"rc {rc}, (selected, view, cur, top, shown, V, loader, warp) {o} vs {p}")
        for nm, a, b in (("screen", oscr, psc.raw), ("panel", opan, pp.raw)):
            if a != b:
                nb = next(q for q in range(64000) if a[q] != b[q])
                msg.append(f"{nm} @({nb % 320},{nb // 320}) {a[nb]:#x} vs {b[nb]:#x}")
        if osol != ps.raw:
            nb = next(q for q in range(9 * 0x200) if osol[q] != ps.raw[q])
            msg.append(f"soldier {nb // 0x200} +{nb % 0x200:#x} {osol[nb]:#x} vs {ps.raw[nb]:#x}")
        if msg:
            print(f"MISMATCH case {i} k {k} mode {mode} recs {len(recs)} cur {cur} presses {list(zip(kinds, presses))}: "
                  + "; ".join(msg))
            bad += 1
            if bad > 10:
                break
if os.environ.get("PANEL_V"):
    print("outcomes:", sorted(outcomes.items()))
print(f"assign_panel_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
