"""Differential test: the office input loop (OfficeScreen 0x4EE64; strategic.md "The office"), original vs port.

usage: .venv/bin/python validate/diff/office_input_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: the A.I.M. link byte 0x22A043, the team size (TeamSize 0x513B8) and a tape of up to about 15 polls
(4 i32 each: mouse x, y, buttons, key; key != 0 = one key, 0x56 = V). The original runs OfficeScreen in Unicorn on
tac60 with InitOfficeScreen 0x4ED1C, TimerStart 0x47020, MousePoll 0x678E8 (the tape), HidePointer 0x67BF0,
ShowMouse 0x673AC, SetMouseShape 0x674B0, MouseSetPos 0x681D0, RestoreScreenArea 0x468B0, SaveScreenArea 0x467E4,
CopyPanelToScreen 0x46BF4, 0x4C26C, FadePalette 0x52908, WaitLeftUp 0x67C54, WaitMs 0x67F48, ClickSound 0x5C880,
SetSequenceVolume 0x5C858 and ToUpper 0x754D7 stubbed; OfficeHighlight 0x4C0C8 (k, on), DrawText 0x358D0 (the label
text, x, stack y), SelectSaveSlot? 0x4D798 (mode; it sets 0x28ECDC = 1 so SaveGame is skipped), SaveGame 0x4EBC8,
RestoreGameDialog? 0x4E700, DrawRoomScreen 0x4C304, DrawOfficeLedger 0x4C41C, SwitchBoxScreen 0x4D310,
AimScreen 0x2011C, StatusMessage? 0x27D24 (its text; the box eats the tape entries until one has a press or a key)
and QuitWithLine 0x382AC logged; KeyPressed 0x5616C / GetKey 0x560F4 answer the tape's key. The port
(port/ja1front ja1f_office_pass) walks the same tape on a world from QICKSTRT.SAV (link, day, team slots in sector).
Compared: the order of highlights, labels, message boxes, actions and the loop's result; a label is compared only
at the tape index where the original drew one (the port draws one at every poll, as its picture does).
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop        # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_office_pass.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32] * 3 + [ctypes.c_void_p,
                                      ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_office_pass.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV").encode()

LINK, MX, MY, BUTTONS, SAVE_FLAG = 0x22A043, 0x229D00, 0x229D04, 0x229CF4, 0x28ECDC
RECTS = [(178, 40, 217, 60), (178, 62, 217, 82), (152, 113, 202, 123), (152, 91, 202, 123),
         (0, 122, 102, 162), (53, 60, 112, 80), (0, 21, 24, 70), (270, 25, 319, 166)]
LABELS = {"Save Game": 0, "Restore Game": 1, "Contact A.I.M.": 2, "Sleep": 4, "View Team": 5,
          "Control Panel": 6, "Leave": 7, "": 8}
LEAVE = (280, 40)


def signed(v):
    return v - (1 << 32) if v >= 1 << 31 else v


e = Emu(dump=dump, fast=True)
tape, pos, cur = [], [0], [0]
key_pending, key_val, team = [0], [0], [0]
orig, orig_labels = [], {}


def feed(ent):
    x, y, btn, key = ent
    e.w32(e.rt(MX), x)
    e.w32(e.rt(MY), y)
    e.w32(e.rt(BUTTONS), btn)
    key_pending[0] = 1 if key else 0
    key_val[0] = key


def poll(em, *_):
    if pos[0] < len(tape):
        cur[0] = pos[0]
        ent = tape[pos[0]]
        pos[0] += 1
        feed(ent)
    else:
        key_pending[0] = 0
    return 0


def getkey(em, *_):
    if key_pending[0]:
        key_pending[0] = 0
        return key_val[0]
    return 0


def toupper(em, eax, *_):
    c = eax & 0xFF
    return c - 32 if 0x61 <= c <= 0x7A else c


def highlight(em, eax, edx, *_):
    orig.append(("h", eax & 0xFFFFFFFF, 1 if edx & 0xFFFFFFFF else 0))
    return 0


def draw_text(em, eax, edx, ebx, ecx):
    txt = em.cstr(eax & 0xFFFFFFFF).decode("latin-1")
    y = signed(em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4))
    orig_labels.setdefault(cur[0], []).append((LABELS.get(txt, 9), signed(ecx & 0xFFFFFFFF), y))
    return 0


def status(em, eax, *_):
    txt = em.cstr(eax & 0xFFFFFFFF).decode("latin-1")
    orig.append(("m", 1 if txt.startswith("You try") else 2))
    while pos[0] < len(tape):
        cur[0] = pos[0]
        ent = tape[pos[0]]
        pos[0] += 1
        feed(ent)
        key_pending[0] = 0                       # the box takes the key that ends it
        if ent[3] or (ent[2] & 1):
            break
    return 0


def action(what, mode=0):
    def fn(em, *_):
        orig.append(("a", what, mode))
        return 0
    return fn


def save_slot(em, eax, *_):
    orig.append(("a", 0, eax & 0xFFFFFFFF))
    e.w32(e.rt(SAVE_FLAG), 1)                    # SaveGame is a stub: keep it out of the log
    return 0


for a in (0x4ED1C, 0x47020, 0x67BF0, 0x673AC, 0x674B0, 0x681D0, 0x468B0, 0x467E4, 0x46BF4, 0x4C26C,
          0x5C880, 0x67C54, 0x67F48, 0x5C858):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x52908), lambda em, *_: 0, 4)       # FadePalette: one stack arg
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x5616C), lambda em, *_: key_pending[0])
e.stub(e.rt(0x560F4), getkey)
e.stub(e.rt(0x754D7), toupper)
e.stub(e.rt(0x513B8), lambda em, *_: team[0])
e.stub(e.rt(0x4C0C8), highlight)
e.stub(e.rt(0x358D0), draw_text, 0x18)
e.stub(e.rt(0x27D24), status)
e.stub(e.rt(0x4D798), save_slot)
e.stub(e.rt(0x4EBC8), action(5))
e.stub(e.rt(0x4E700), action(0, 0))
e.stub(e.rt(0x4C304), action(4))
e.stub(e.rt(0x4C41C), action(2))
e.stub(e.rt(0x4D310), action(3))
e.stub(e.rt(0x2011C), action(1))
e.stub(e.rt(0x382AC), action(6))
ORIG_ADDR = e.rt(0x4EE64)

r = random.Random(7)
bad = 0


def rect_point(k):
    x0, y0, x1, y1 = RECTS[k]
    return (min(max(r.choice([x0, x1, x0 - 1, x1 + 1, r.randrange(x0, x1 + 1)]), 0), 319),      # the mouse stays on
            min(max(r.choice([y0, y1, y0 - 1, y1 + 1, r.randrange(y0, y1 + 1)]), 0), 199))      # the screen


def any_point():
    k = r.random()
    if k < 0.8:
        return rect_point(r.randrange(8))
    return (r.randrange(0, 320), r.randrange(0, 200))


def a_tape():
    ps = []
    for _ in range(r.randrange(0, 5)):
        c = r.random()
        if c < 0.30:
            p = any_point()
            ps.append((p[0], p[1], 0, 0))
        elif c < 0.62:                                    # a click on a hot spot
            p = rect_point(r.randrange(8))
            ps += [(p[0], p[1], 0, 0), (p[0], p[1], 1, 0), (p[0], p[1], 0, 0)]
        elif c < 0.80:                                    # key V opens the version box; a key closes it
            p = any_point()
            ps += [(p[0], p[1], 0, 0), (p[0], p[1], 0, 0x56), (p[0], p[1], 0, 0x1B)]
        elif c < 0.90:                                    # a press on no hot spot (the click latch)
            p = any_point()
            ps += [(p[0], p[1], 0, 0), (p[0], p[1], 1, 0), (p[0], p[1], 0, 0)]
        else:                                             # the label clips at x 295..319
            ps.append((r.randrange(296, 320), r.randrange(0, 200), 0, 0))
    p = LEAVE
    ps += [(p[0], p[1], 0, 0), (p[0], p[1], 1, 0), (p[0], p[1], 0, 0)]
    ps += [(p[0], p[1], 0, 0), (p[0], p[1], 1, 0), (p[0], p[1], 0, 0)]
    return ps


for i in range(ncases):
    link = r.choice([0, 0, 1, 1, 2, 255])
    team[0] = r.choice([0, 0, 1, 2, 3, 8])
    ps = a_tape()
    tape[:] = ps
    pos[0] = cur[0] = 0
    key_pending[0] = key_val[0] = 0
    orig.clear()
    orig_labels.clear()
    e.w8(e.rt(LINK), link)
    e.w32(e.rt(MX), 0xFFFFFFFF)
    e.w32(e.rt(MY), 0xFFFFFFFF)
    e.w32(e.rt(BUTTONS), 0)
    e.w32(e.rt(SAVE_FLAG), 1)
    try:
        ret = e.call(ORIG_ADDR, max_steps=2_000_000)
    except EmuStop as stop:
        print(f"MISMATCH case {i}: the original stopped at {stop} (link {link} team {team[0]} tape {ps})")
        bad += 1
        continue
    orig.append(("r", signed(ret)))
    if PORT:
        ta = (ctypes.c_int32 * (4 * len(ps)))(*[v for q in ps for v in q])
        cap = 5 * len(ps) + 8
        lg = (ctypes.c_int32 * (5 * cap))()
        ct = (ctypes.c_int32 * 1)()
        rc = PORT.ja1f_office_pass(ISO, EXE, SAVE, link, 1, team[0], ta, len(ps), lg, cap, ct)
        got, glabels = [], {}
        for k in range(ct[0]):
            typ, a, b, c, idx = lg[5 * k:5 * k + 5]
            if typ == 1:
                got.append(("h", a, b))
            elif typ == 2:
                glabels.setdefault(idx, []).append((a, b, c))
            elif typ == 3:
                got.append(("m", a))
            elif typ == 4:
                got.append(("a", a, b))
            elif typ == 5:
                got.append(("r", a))
        why = None
        if rc != 0:
            why = f"rc {rc}"
        elif got != orig:
            for k in range(max(len(got), len(orig))):
                g, o = (got[k] if k < len(got) else None), (orig[k] if k < len(orig) else None)
                if g != o:
                    why = f"event {k}: original {o}, port {g}"
                    break
        else:
            for idx in sorted(orig_labels):
                for lab in orig_labels[idx]:
                    if lab not in glabels.get(idx, []):
                        why = f"label at index {idx}: original {lab}, port {glabels.get(idx)}"
                        break
                if why:
                    break
        if why:
            print(f"MISMATCH case {i} (link {link} team {team[0]}, {len(ps)} polls): {why}; tape {ps}")
            bad += 1
            if bad > 10:
                break
print(f"office_input_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
