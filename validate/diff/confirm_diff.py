"""Differential test: ConfirmDialog? 0x275E4 (panel.md "The travel animation"), original vs port.

usage: .venv/bin/python validate/diff/confirm_diff.py [ncases=240] [DUMP] [--port] [--lib SO]
The original runs whole in Unicorn on day3: FormatSanimName, PlaySanim, the text, the buttons, the save and the
restore of the area are real. The PCC loader 0x532BC reads MAIN.DAT and the row's archive; 0x2538C is stubbed (it
takes the screen and gives the case's answer); the frame timer 0x47040 and KeyOrClick 0x67E44 read a tape.
Per case: a text, a wait, yes and no row (a row can be none), the panel flag 0x284F0C, the flags of the callers
(restore 1 or 0, mouse 1 or 0), a mouse place in or out of the box, a noise screen.
Compared with port confirm::run (ja1f_confirm): the call order (sounds with slot + loop, mouse moves, the input,
every timer and KeyOrClick pass, StopAllSounds), the screen at the input, after the answer's box, after each frame
step and at the end, and the answer.
Also: the original must put back 0x92E4F, clear 0x93DCC, and clear 0x9344B when a row ran. The wait 0x2538C runs
real for every key byte, against question::key_answer; its button order is checked too. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 240
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_day3.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_confirm.restype = ctypes.c_int32
    PORT.ja1f_go_screen.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32] * 2 + [ctypes.c_char_p, ctypes.c_void_p]
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

DAT = lambda n: open(os.path.join(ROOT, "rom", "DAT", n), "rb").read()
MAIN = DAT("MAIN.DAT")
ARCS = {1: DAT("SANIM.DAT"), 2: DAT("SANIM2.DAT")}
ROWS, TEXTS = 0x9443A, 0x9681F
# the rows of each archive: (arc, wait, yes, no) as the callers pass them, plus crossed ones below
CALLS = [(1, 0, 1, None), (1, 2, 3, None), (1, 4, 5, None), (1, 8, 9, 10), (1, 8, 11, 10), (2, 6, 7, None),
         (2, 12, 13, None), (2, 14, 15, None), (2, 16, 17, None)]
QUESTIONS = [b"Get to work?", b"Terminate communication with A.I.M.?", b"Raise natives' salary? \xb1 (don't expect to lower it later)",
             b"Dish out $1500 for corpse export?", b"There is a $1000 helicopter charge. Authorize payment?"]
SHOT = 64000
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen = e.r32(e.rt(0x95678))
arc = [ARCS[1]]
log, shots, due_tape, stop_tape, answer = [], [], [], [], [0]
first_timer = [False]


def shot():
    shots.append(bytes(e.uc.mem_read(screen, SHOT)))


def load(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    src = arc[0] if name.startswith("SANIM") else MAIN
    try:
        w, h, px, _ = pcx.decode(datarc.member(src, name))
    except Exception:
        return 0
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


go_shot = [None]


def timer(em, *_):
    if first_timer[0]:            # the first pass of the go row: the box as the blit left it
        first_timer[0] = False
        log.append((4, 0, 0))
        shot()
        go_shot[0] = shots[-1]
    v = due_tape.pop(0) if due_tape else 1
    log.append((6, v, 0))
    return 0x7FFFFFFF if v else -1


def stopped(em, *_):
    v = stop_tape.pop(0) if stop_tape else 1
    if log[-1] == (6, 1, 0):      # the frame step ran in this pass: its picture
        log.append((4, 0, 0))
        shot()
    log.append((7, v, 0))
    return v


def ask(em, *_):
    log.append((3, 0, 0))
    shot()
    first_timer[0] = True
    return answer[0]


def stop_sounds(em, *_):
    log.append((5, 0, 0))
    return 0


def warp(em, eax, edx, ebx, ecx):
    log.append((2, eax, edx))
    return 0


def sound(em, eax, edx, ebx, ecx):
    assert edx == 0x2B11 and ebx == 20
    log.append((1, eax, ecx))
    return 0


mouse_at = [(0, 0)]


def mouse_back(em, *_):
    log.append((2,) + mouse_at[0])
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x47020, 0x67C54, 0x67E18, 0x67C0C, 0x5CCF8):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x47040), timer)
e.stub(e.rt(0x67E44), stopped)
e.stub(e.rt(0x2538C), ask)
e.stub(e.rt(0x5CE08), stop_sounds)
e.stub(e.rt(0x681D0), warp)
e.stub(e.rt(0x5CB34), sound)
e.stub(e.rt(0x67E30), mouse_back)
TEXT = e.alloc(1024)


def fuzz(r):
    words = []
    for _ in range(r.choice([1, 2, 3, 5, 8, 12])):
        k = r.random()
        if k < 0.05:
            words.append("\xb1")
        else:
            words.append("".join(chr(r.randint(0x21, 0x7E)) for _ in range(r.choice([1, 2, 3, 4, 6, 9]))))
    return " ".join(words).encode("latin-1")


r = random.Random(33)
row_ptr = lambda k: 0 if k is None else e.rt(ROWS + 12 * k)
TIMER_KEEP = e.r32(e.rt(0x92E4F)) & 0xFF
bad = 0
n_yes = n_no = n_steps = n_warp = n_stop = n_none = 0
if PORT:
    shots_buf = ctypes.create_string_buffer(SHOT * 80)
    log_buf = (ctypes.c_int32 * 900)()
n_go = 0
for i in range(ncases):
    a, wr, yr, nr = CALLS[i % len(CALLS)]
    if i % 11 == 10:                       # the original's null checks: no callers use these, the code has them
        wr = None
    if i % 13 == 12:
        yr = None
    if i % 7 == 6 and nr is None:
        nr = r.choice([k for c in CALLS if c[0] == a for k in c[1:3]])
    arc[0] = ARCS[a]
    s = QUESTIONS[i] if i < len(QUESTIONS) else fuzz(r)
    # rows 7, 13, 15, 17 hold a sector number in characters 22 and 23 (the host writes it)
    number = r.randint(1, 60)
    for k in (7, 13, 15, 17):
        e.uc.mem_write(e.rt(TEXTS + 40 * k + 22), b"%2d" % number)
    office = r.randrange(2)
    restore, mouse = r.choice([(1, 1), (1, 1), (0, 0), (1, 0), (0, 1)])
    mx, my = r.choice([(100, 60), (0x34, 0x25), (0x10B, 0x74), (0x33, 60), (0x10C, 60), (100, 0x24), (100, 0x75), (5, 190)])
    answer[0] = r.randrange(2)
    go = yr if answer[0] else nr
    if go is None:
        n_none += 1
    frames = 60 if go is None else bytes(e.uc.mem_read(e.rt(ROWS + 12 * go), 12))[3]
    # the tapes: most frames step at once; a timer that is not due leaves the picture; a stop ends the row early
    due = [int(r.random() < 0.8) for _ in range(frames * 2)]
    stop = [0] * (frames * 2)
    if i % 3 == 0:
        stop[r.randrange(max(1, frames))] = 1
        n_stop += 1
    log.clear(); shots.clear()
    go_shot[0] = None
    due_tape[:] = due; stop_tape[:] = stop
    first_timer[0] = False
    mouse_at[0] = (mx, my)
    base = bytes(r.randrange(256) for _ in range(SHOT))
    e.uc.mem_write(screen, base)
    e.uc.mem_write(TEXT, s + b"\0")
    e.w8(e.rt(0x284F0C), office)
    e.w32(e.rt(0x229D00), mx)
    e.w32(e.rt(0x229D04), my)
    e.w8(e.rt(0x9344B), 1)
    keep = r.randrange(2)
    e.w8(e.rt(0x92E4F), keep)
    # 3 stack flags in the order save, restore, mouse (the save flag is 1 in every caller)
    rc = e.call(e.rt(0x275E4), TEXT, row_ptr(wr), row_ptr(yr), row_ptr(nr), 1, restore, mouse) & 0xFF
    end = bytes(e.uc.mem_read(screen, SHOT))
    if rc != answer[0]:
        print(f"case {i}: the original answered {rc}, not {answer[0]}"); bad += 1
    if e.r8(e.rt(0x92E4F)) != keep:
        print(f"case {i}: the original did not put back 0x92E4F"); bad += 1
    ran = wr is not None or go is not None
    if e.r32(e.rt(0x93DCC)) != 0 or e.r8(e.rt(0x9344B)) != (0 if ran else 1):
        print(f"case {i}: 0x93DCC {e.r32(e.rt(0x93DCC)):#x} 0x9344B {e.r8(e.rt(0x9344B))} (row ran {ran})"); bad += 1
    n_yes += answer[0]; n_no += 1 - answer[0]
    n_steps += sum(1 for x in log if x == (6, 1, 0))
    n_warp += sum(1 for x in log if x[0] == 2)
    if not PORT:
        continue
    b = ctypes.create_string_buffer(base, SHOT)
    rows = (ctypes.c_int32 * 3)(*[-1 if k is None else k for k in (wr, yr, nr)])
    iv = (ctypes.c_int32 * 7)(number, office, restore, mouse, mx, my, answer[0])
    n = PORT.ja1f_confirm(ISO, EXE, {1: b"SANIM.DAT", 2: b"SANIM2.DAT"}[a], s, rows, iv, bytes(due), bytes(stop), len(due), b,
                          shots_buf, 80, log_buf, 300)
    if n < 0:
        print(f"case {i}: port error {n}"); bad += 1
        continue
    plog = [tuple(log_buf[3 * j:3 * j + 3]) for j in range(n)]
    if plog != log:
        for j, (x, y) in enumerate(zip(plog + [None] * 9, log + [None] * 9)):
            if x != y:
                print(f"case {i} rows {wr},{yr},{nr} answer {answer[0]}: log row {j}: port {x} original {y}")
                break
        bad += 1
        continue
    for j, o in enumerate(shots):
        p = shots_buf.raw[j * SHOT:(j + 1) * SHOT]
        if p != o:
            k = next(q for q in range(SHOT) if p[q] != o[q])
            print(f"case {i} rows {wr},{yr},{nr} answer {answer[0]} office {office}: shot {j}/{len(shots)}: "
                  f"first difference ({k % 320},{k // 320}): port {p[k]} original {o[k]}")
            bad += 1
            break
    # the island map's box after yes (question::go_screen, panel picture PANEL_P) = the go row's first box
    if go_shot[0] is not None and not office:
        g = ctypes.create_string_buffer(SHOT)
        rc_go = PORT.ja1f_go_screen(ISO, EXE, {1: b"SANIM.DAT", 2: b"SANIM2.DAT"}[a], go, number if go in (7, 13, 15, 17) else -1, base, g)
        if rc_go != 0 or g.raw != go_shot[0]:
            k_ = next((q for q in range(SHOT) if g.raw[q] != go_shot[0][q]), -1); print(f"case {i} row {go} answer {answer[0]}: the map's go box differs (rc {rc_go}) at ({k_ % 320},{k_ // 320}): {g.raw[k_]} vs {go_shot[0][k_]}"); bad += 1
        n_go += 1
    if b.raw != end:
        print(f"case {i}: the screen at the end differs (restore {restore}, answer {answer[0]})"); bad += 1
    if iv[6] != answer[0]:
        print(f"case {i}: port answer {iv[6]}"); bad += 1
    if bad > 10:
        break

# the wait 0x2538C itself: every key byte, then both buttons. Its own frame steps are PlaySanim(1), stubbed here.
tape = []
e2 = Emu(dump=dump, fast=True)
for a_ in (0x678E8, 0x258D0):
    e2.stub(e2.rt(a_), lambda em, *_: 0)
buttons, polls = [0, 0], [0]


def button_at(em, eax, *_):
    polls[0] += 1
    if polls[0] > 8:           # no answer came: end the wait with button 6 and say so
        tape.append("none")
        return int(eax & 0xFF == 6)
    return buttons[(eax & 0xFF) - 5]


keys = []
e2.stub(e2.rt(0x67CD8), button_at)
e2.stub(e2.rt(0x5616C), lambda em, *_: int(bool(keys)))
e2.stub(e2.rt(0x560F4), lambda em, *_: keys.pop(0))
n_keys = 0
for c in range(256):
    keys[:] = [c]
    tape.clear()
    polls[0] = 0
    v = e2.call(e2.rt(0x2538C)) & 0xFF
    want = -1 if tape else v
    if PORT:
        got = PORT.ja1f_question_key(c)
        n_keys += 1
        if got != want:
            print(f"key {c:#x}: port {got} original {want}"); bad += 1
keys.clear()
for b5, b6, want in ((1, 0, 0), (0, 1, 1), (1, 1, 0)):
    buttons[:] = [b5, b6]
    polls[0] = 0
    v = e2.call(e2.rt(0x2538C)) & 0xFF
    if v != want:
        print(f"buttons {b5},{b6}: the original answered {v}"); bad += 1

print(f"confirm_diff: {ncases} cases, {n_yes} yes, {n_no} no, {n_none} no row, {n_steps} frame steps, {n_warp} mouse moves, "
      f"{n_stop} stops, {n_keys} keys, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
