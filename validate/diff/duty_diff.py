"""Differential test: the duty menu of the assignment screen (strategic.md "The duty menu"), original vs port.

usage: .venv/bin/python validate/diff/duty_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: fuzzed team slots 0..8 (as assign_diff), the assignment picture built by the original DrawAssignMap?
0x478A4(99) in the panel buffer and on the screen (rows 0 and 143..145 noise), then a scripted mouse: poll 0 =
a left press on a duty name box of slot k, then moves with the left held / up, a press (or a right press), and for
TRAIN the same for the subject menu. The original runs AssignScreen? 0x4891C in Unicorn with the input poll
0x678E8 replaced by the script (the screen is kept at each poll), the start 0x47AE4, keys, sounds, the mouse move
0x681D0, CalcLedgerAll 0x51178 and YesNoPrompt? 0x28C84 (answer no) stubbed; it stops at the first poll of the main
loop after poll 0. The port = port/ja1front ja1f_duty_script. Compared: the screen after every poll, the polls used,
the mouse y of the open, duty and trainWhat of slots 0..8, the ledger and FIRE question calls.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
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
    PORT.ja1f_duty_script.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 7 + [ctypes.c_int32] + \
        [ctypes.c_void_p] * 2
    PORT.ja1f_duty_script.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SOLDIERS, FACES, ACC, STATCH, PANEL_P, SCREEN_P = 0xFA958, 0x93428, 0x229FA0, 0x10DD10, 0x93E60, 0x95678
MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    w, h, px = pic(e.cstr(ebx))
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
    if j >= 1:
        st["shots"].append(bytes(e.uc.mem_read(screen, 64000)))
    if j >= len(st["ev"]) or (j >= 1 and st["arg0"] is not None and _[0] == 0):
        em.stop_reason = "script end"
        em.uc.emu_stop()
        return 0
    st["arg0"] = _[0]
    x, y, b = st["ev"][j]
    e.w32(e.rt(MOUSE_X), x)
    e.w32(e.rt(MOUSE_Y), y)
    e.w8(e.rt(BUTTONS), b)
    return 0


def mouse_to(em, eax, edx, *_):
    st["warp"] = edx
    e.w32(e.rt(MOUSE_X), eax)
    e.w32(e.rt(MOUSE_Y), edx)
    return 0


def flag(name, r=0):
    def f(em, *_):
        st[name] = 1
        return r
    return f


for a in (0x67BF0, 0x673AC, 0x67E18, 0x67E30, 0x67478, 0x47AE4, 0x5C8A0, 0x5C880, 0x5616C, 0x67CD8, 0x28698):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x681D0), mouse_to)
e.stub(e.rt(0x51178), flag("ledger"))
e.stub_sprintf()


def fire_ask(em, eax, *_):
    st["fire"] = e.cstr(eax)
    return 0


e.stub(e.rt(0x28C84), fire_ask, 4)
e.stub(e.rt(0x532BC), load, 4)
panel = e.r32(e.rt(PANEL_P))
screen = e.r32(e.rt(SCREEN_P))
faces = [e.r32(e.rt(FACES) + 4 * k) for k in range(8)]
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))


def menu_moves(r, x0, y0, w, h, n):
    out = []
    for _ in range(n):
        if r.random() < 0.15:
            out.append((r.randint(0, 319), r.randint(0, 199)))
        else:
            out.append((r.randint(x0 - 3, x0 + w + 3), r.randint(y0 - 3, y0 + h + 3)))
    return out


def phase(r, x0, y0, w, h):
    ev = [(x, y, 1) for x, y in menu_moves(r, x0, y0, w, h, r.randint(0, 2))]
    ev += [(x, y, 0) for x, y in menu_moves(r, x0, y0, w, h, r.randint(1, 4))]
    x, y = menu_moves(r, x0, y0, w, h, 1)[0]
    ev.append((x, y, 2 if r.random() < 0.12 else r.choice([1, 1, 1, 3])))
    return ev


def script(r, k, sol):
    bx = 11 + 37 * k
    ev = [(r.randint(bx, bx + 36), r.randint(7, 15), 1)]
    x0 = bx - 3
    ev += phase(r, x0, 12, 42, 62)
    # the subject menu (used only when TRAIN is chosen; extra polls are not used otherwise)
    sx = x0 + 10 if k < 7 else x0 - 8
    q = r.random()
    if q < 0.7:   # aim the press at TRAIN or at a random row
        x, _, b = ev[-1]
        row = 2 if q < 0.35 else r.randrange(7)
        ev[-1] = (r.randint(x0, x0 + 41), r.randint(20, 26) + 7 * row, b)
    ev += phase(r, sx, 30, 57, 60)
    ev += [(r.randint(0, 319), r.randint(0, 199), 0)]
    return ev


r = random.Random(1)
bad = 0
outcomes = {}
for i in range(ncases):
    n = r.choice([1, 2, 3, 4, 5, 6, 7, 8, 8])
    sol = bytearray(base_sol)
    for k in range(9):
        s = bytearray(sol[k * 0x200:(k + 1) * 0x200])
        fz.fuzz_soldier(r, s, k, n)
        sol[k * 0x200:(k + 1) * 0x200] = s
    if r.random() < 0.1:   # a slot after the list end in the sector / a slot in the list not in the sector
        sol[r.randrange(8) * 0x200 + 0x182] ^= 1
    fc = [bytes(r.randrange(256) for _ in range(1024)) for _ in range(8)]
    sc = bytes(1 if r.random() < 0.2 else 0 for _ in range(640))
    acc = bytearray(base_acc)
    for o in (0x24, 0x28, 0x30, 0x34, 0x38):
        acc[o:o + 4] = r.randint(-99999, 999999).to_bytes(4, "little", signed=True)
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
    for k in range(8):
        e.uc.mem_write(faces[k], fc[k])
    e.uc.mem_write(e.rt(STATCH), sc)
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.uc.mem_write(panel, bytes(r.randrange(256) for _ in range(64000)))
    st.clear()
    try:
        e.call(e.rt(0x478A4), 99)
    except EmuStop as x:
        print("draw", i, n, x, hex(e.uc.reg_read(__import__("unicorn").x86_const.UC_X86_REG_EIP))); raise
    pan = bytearray(e.uc.mem_read(panel, 64000))
    scr = bytearray(pan)
    for y in (0, 143, 144, 145):
        scr[y * 320:(y + 1) * 320] = bytes(r.randrange(256) for _ in range(320))
    e.uc.mem_write(screen, bytes(scr))
    sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    k = r.randrange(n) if r.random() < 0.9 else r.randrange(8)
    ev = script(r, k, sol)
    st.clear()
    st.update(polls=0, shots=[], ev=ev, arg0=None, warp=-1, ledger=0, fire=0)
    try:
        e.call(e.rt(0x4891C), max_steps=50_000_000)
        print(f"case {i}: the original returned")
        bad += 1
        continue
    except EmuStop as x:
        if e.stop_reason != "script end":
            print(f"case {i}: original stopped: {x} k {k} n {n} polls {st['polls']} ev {ev}")
            bad += 1
            continue
    osol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    o = (st["polls"] - 1, st["warp"], st["ledger"], st["fire"],
         [osol[q * 0x200 + 0x17E] for q in range(9)], [osol[q * 0x200 + 0x199] for q in range(9)])
    dk = [sol[q * 0x200 + 0x17E] != o[4][q] or sol[q * 0x200 + 0x199] != o[5][q] for q in range(9)]
    key = ("fire" if st["fire"] else "ledger" if st["ledger"] else "none") + ("+train" if any(dk) and o[4][k] == 2 else "")
    outcomes[key] = outcomes.get(key, 0) + 1
    if PORT:
        ps = ctypes.create_string_buffer(sol, 9 * 0x200)
        pp, psc = ctypes.create_string_buffer(bytes(pan), 64000), ctypes.create_string_buffer(bytes(scr), 64000)
        flat = [v for t in ev for v in t]
        evb = (ctypes.c_int32 * len(flat))(*flat)
        shots = ctypes.create_string_buffer(64000 * len(ev))
        res = (ctypes.c_int32 * 5)()
        rc = PORT.ja1f_duty_script(ISO, EXE, ps, b"".join(fc), sc, bytes(acc), pp, psc, evb, len(ev), shots, res)
        p = (res[0], res[4], res[3], (b"Fire " + bytes(ps.raw[k * 0x200 + 0x16E:k * 0x200 + 0x177]).split(b"\0")[0] + b"?") if res[1] == 6 else 0,
             [ps.raw[q * 0x200 + 0x17E] for q in range(9)], [ps.raw[q * 0x200 + 0x199] for q in range(9)])
        msg = []
        if rc != 0 or p != o:
            msg.append(f"rc {rc}, (polls, warp y, ledger, fire, duties, trainWhat) {o} vs {p}")
        for j in range(min(o[0], p[0])):
            a, b = st["shots"][j], shots.raw[64000 * j:64000 * (j + 1)]
            if a != b:
                nb = next(q for q in range(64000) if a[q] != b[q])
                msg.append(f"screen after poll {j} @({nb % 320},{nb // 320}) {a[nb]:#x} vs {b[nb]:#x}")
                break
        if msg:
            print(f"MISMATCH case {i} k {k} ev {ev[:o[0] + 1]}: " + "; ".join(msg))
            bad += 1
            if bad > 10:
                break
if os.environ.get("DUTY_V"):
    print("outcomes:", sorted(outcomes.items()))
print(f"duty_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
