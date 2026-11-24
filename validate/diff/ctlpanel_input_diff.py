"""Differential test: the tactical Control Panel's input loop (ControlPanelInput 0x4BAA4; panel.md "The Control
Panel"), original vs port.

usage: .venv/bin/python validate/diff/ctlpanel_input_diff.py [ncases=100] [DUMP] [--port] [--lib SO]
Per case: 16 switch bytes (0, 1, 2, 255), the in-combat byte 0x92DCB, the quick-save-in-combat byte 0x22A1F9, the
Quick Restore presence, the sound / music driver bytes 0x96399 / 0x9639A, the playing / wanted music themes
0x9635E / 0x96360, and a tape of up to 8 mouse passes (4 i32 each: x, y, left press, escape) that stays inside the
map view MOUSE_BOX (48, 14, 271, 181) and ends with Escape or a Continue Game click. The original runs in Unicorn
on tac60 with MousePoll 0x678E8 = the next pass, EscPressed 0x67300 = its escape, WaitLeftUp 0x67C54, HidePointer
0x67BF0 and ShowMouse 0x673AC stubbed, DrawText / BlitCopy real; PlaySfxSlot 0x5CB34, SetMusic 0x5C5EC,
StopMusic 0x5C82C and PathMarksOff 0x1D8A4 logged. The port (port/ja1front ja1f_ctlpanel_pass) walks the same tape
on a world from QICKSTRT.SAV. Compared: the order of sounds, music calls and Show Paths offs, the close result,
the 16 switch bytes at the end, and every DrawText 0x358D0 and BlitCopy 0x52AE0 call of the input loop (text, place,
colours, font, mode; the copy's place and size). The opening picture is ctlpanel_diff's work. A switch value above 1
never occurs in play (only the toggle writes it): the original then indexes past its ON / OFF texts, so the text of
such a value is not compared, its place and colours are.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop        # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 100
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_ctlpanel_pass.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_void_p] + [ctypes.c_int32] * 7 + \
        [ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p,
         ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_ctlpanel_pass.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV").encode()

SCREEN_P, OPTS, COMBAT, QSIC = 0x95678, 0x1163B8, 0x92DCB, 0x22A1F9
PANEL_ON, SAVEANY = 0x92DCC, 0x92DC3
MX, MY, BUTTONS, SND_DRV, MID_DRV, PLAYING, WANTED = 0x229D00, 0x229D04, 0x229CF4, 0x96399, 0x9639A, 0x9635E, 0x96360
BOX = (48, 14, 271, 181)
CONTINUE = (120, 142)
BUTTONS_XY = [(55, 165), (170, 165)]
VALUE_X = (55 + 75, 170 + 75)                   # the ON / OFF column of each switch row


def place(k):
    return (55, 25 + 13 * k) if k < 8 else (170, 25 + 13 * (k - 8))


e = Emu(dump=dump, fast=True)
screen = e.alloc(bytes(64000))
e.w32(e.rt(SCREEN_P), screen)
log, tape, esc_state = [], [], [0]


def poll(em, *_):
    if tape:
        x, y, left, esc = tape.pop(0)
        e.w32(e.rt(MX), x)
        e.w32(e.rt(MY), y)
        e.w32(e.rt(BUTTONS), left)
        esc_state[0] = esc
    else:
        log.append(("extra poll",))        # the tape must always close the panel
        e.w32(e.rt(MX), 250)
        e.w32(e.rt(MY), 40)
        e.w32(e.rt(BUTTONS), 1)
        esc_state[0] = 1
    return 0


e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x67300), lambda em, *_: esc_state[0])
for a in (0x67C54, 0x67BF0, 0x673AC):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x5CB34), lambda em, a, d, b, c: log.append(("sfx", a, d, b & 0xFF)))
e.stub(e.rt(0x5C5EC), lambda em, a, d, *_: log.append(("music", 1, a & 0xFFFF)))
e.stub(e.rt(0x5C82C), lambda em, *_: log.append(("music", 0, 0)))
e.stub(e.rt(0x1D8A4), lambda em, *_: log.append(("marks",)))
restore_flag = [0]
e.stub(e.rt(0x4707C), lambda em, *_: restore_flag[0])
for a in (0x1CB78, 0x682AC):                    # ControlPanel's redraw and mouse helpers
    e.stub(e.rt(a), lambda em, *_: 0)
draws, drawing = [], [False]


def draw_text(em, text, dst, pitch, x):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    y, centre, fg, shadow, font, mode = (e.r32(sp + 4 * k) for k in range(1, 7))
    if drawing[0]:
        s = e.cstr(text)
        draws.append(("T", x, y, fg & 0xFF, shadow & 0xFF, s, dst == screen, pitch, centre, font, mode))
    return 0


def blit(em, dst, src, w, h):
    if drawing[0]:
        d = dst - screen
        draws.append(("C", d % 320, d // 320, w, h))
    return 0


e.stub(e.rt(0x358D0), draw_text, pop=24)
e.stub(e.rt(0x52AE0), blit, pop=8)
ORIG = bytes(e.uc.mem_read(e.rt(0x4BAA4), 1))

r = random.Random(11)
r2 = random.Random(12)                          # own stream: the old tapes stay the same
bad = 0
ndraw = [0]


def an_entry():
    c = r.random()
    if c < 0.55:                                   # a switch press (edges and outside edges too)
        x, y = place(r.choice([0, 0, 5, 5, 1, 7] + list(range(16))))
        dx = r.choice([0, 0, 95, 96, -1, r.randrange(96)])
        dy = r.choice([-4, 0, 9, 10, -5, r.randrange(-4, 10)])
        return (min(max(x + dx, BOX[0]), BOX[2]), min(max(y + dy, BOX[1]), BOX[3]), 1, 0)
    if c < 0.70:                                   # a press anywhere in the map view
        return (r.randrange(BOX[0], BOX[2] + 1), r.randrange(BOX[1], BOX[3] + 1), r.choice([0, 1]), 0)
    if c < 0.80:                                   # a Continue Game press
        x, y = CONTINUE
        return (x + r.choice([0, 10, 95, -1]), y + r.choice([-4, 0, 9, -5]), 1, 0)
    if c < 0.88:                                   # a Quick Save / Restore press
        x, y = r.choice(BUTTONS_XY)
        return (x + r.choice([0, 10, 80, -1]), y + r.choice([-4, 0, 5, -5]), 1, 0)
    return (r.randrange(BOX[0], BOX[2] + 1), r.randrange(BOX[1], BOX[3] + 1), 0, 0)


for i in range(ncases):
    opts = bytes(r.choice([0, 1, 1, 0, 2, 255]) for _ in range(16))
    combat, qsic, restore = r.choice([0, 1]), r.choice([0, 1]), r.choice([0, 1])
    snd, mid = r.choice([1, 1, 0]), r.choice([1, 1, 0])
    wanted, playing = r.choice([-1, 0, 1, 5, 7]), r.choice([-1, 0, 2, 3])
    ps = [an_entry() for _ in range(r.randrange(0, 9))]
    if r.random() < 0.5:
        ps.append((r.randrange(BOX[0], BOX[2] + 1), r.randrange(BOX[1], BOX[3] + 1), 0, 1))   # Escape
    else:
        x, y = CONTINUE
        ps.append((x, y, 1, 0))                                                               # Continue Game
    shown = 1 if (combat == 0 or qsic) else 0
    extra = r2.choice([0, 1, 8, 0x41])          # byte 21554: the shadow of a hovered Continue Game

    # ControlPanel 0x4B788 first: it fills the 17 item places the input loop's hover search reads.
    e.w8(e.rt(PANEL_ON), 1)
    e.w8(e.rt(SAVEANY), 0)
    e.uc.mem_write(e.rt(OPTS), opts + bytes([extra]))
    e.w8(e.rt(COMBAT), combat)
    e.w8(e.rt(QSIC), qsic)
    e.w8(e.rt(SND_DRV), snd)
    e.w8(e.rt(MID_DRV), mid)
    e.w16(e.rt(PLAYING), playing)
    e.w16(e.rt(WANTED), wanted)
    restore_flag[0] = restore
    e.uc.mem_write(e.rt(0x4BAA4), b"\xCC")
    e.uc.ctl_flush_tb()                         # the previous case's restored 0x4BAA4 block is still cached
    try:
        e.call(e.rt(0x4B788))
    except EmuStop as stop:
        if "INT 0x03" not in str(stop):
            print(f"MISMATCH case {i}: the original panel stop at {stop}")
            bad += 1
            continue
    e.uc.mem_write(e.rt(0x4BAA4), ORIG)
    e.uc.ctl_flush_tb()
    tape[:] = ps
    if os.environ.get("CP_DEBUG") and i == 2:
        print("DBG before opening", bytes(e.uc.mem_read(e.rt(OPTS), 16)).hex(), "g_options after opening",
              bytes(e.uc.mem_read(e.rt(OPTS), 16)).hex(), "places",
              bytes(e.uc.mem_read(e.rt(0x28E1A8), 8)).hex(" "))
    esc_state[0] = 0
    log.clear()
    draws.clear()
    drawing[0] = True
    try:
        ret = e.call(e.rt(0x4BAA4), shown, restore, max_steps=5_000_000)
    except EmuStop as stop:
        print(f"MISMATCH case {i}: the original stopped at {stop}")
        bad += 1
        continue
    drawing[0] = False
    ret = ret if ret < 0x80000000 else ret - 0x100000000
    log.append(("close", ret))
    want_opts = bytes(e.uc.mem_read(e.rt(OPTS), 16))
    if os.environ.get("CP_DEBUG") and i == 2:
        print("DBG after input", want_opts.hex(), "ret", ret, "log", log, "tape left", tape)
    ndraw[0] += len(draws)
    if PORT:
        op = ctypes.create_string_buffer(opts + bytes([extra]), 17)
        dbuf = ctypes.create_string_buffer(4096)
        ta = (ctypes.c_int32 * (4 * len(ps)))(*[v for q in ps for v in q])
        cap = 5 * len(ps) + 4
        lg = (ctypes.c_int32 * (4 * cap))()
        sw = ctypes.create_string_buffer(16)
        ct = (ctypes.c_int32 * 2)()
        rc = PORT.ja1f_ctlpanel_pass(ISO, EXE, SAVE, op, combat, qsic, restore, snd, mid, wanted, playing, ta,
                                     len(ps), lg, cap, sw, ct, dbuf, len(dbuf))
        got = []
        for k in range(ct[0]):
            t, a, b, c = lg[4 * k:4 * k + 4]
            if t == 1:
                got.append(("sfx", a, b, c))
            elif t == 2:
                got.append(("music", a, b))
            elif t == 3:
                got.append(("marks",))
            elif t == 4:
                got.append(("close", a))
        why = None
        if rc != 0:
            why = f"rc {rc}"
        elif sw.raw != want_opts:
            why = f"switch bytes {sw.raw.hex()} vs {want_opts.hex()}"
        else:
            for k in range(max(len(got), len(log))):
                g, o = (got[k] if k < len(got) else None), (log[k] if k < len(log) else None)
                if g != o:
                    why = f"step {k}: original {o}, port {g}"
                    break
        if not why:
            pd = dbuf.raw[:ct[1]].split(b"\n")[:-1]
            for k in range(max(len(pd), len(draws))):
                g = pd[k] if k < len(pd) else None
                o = draws[k] if k < len(draws) else None
                if o is not None and o[0] == "T":
                    good = o[6] and o[7] == 320 and o[8] == 0 and o[9] == 1 and o[10] == 300
                    head = b"T %d %d %d %d " % o[1:5]
                    want = head + o[5]
                    if not good or g != want:
                        if not (g is not None and g.startswith(head) and o[1] in VALUE_X and o[4] > 1):
                            why = f"draw {k}: original {o}, port {g}"
                            break
                elif o is not None:
                    if o[3:] != (20, 13) or g != b"C %d %d" % o[1:3]:
                        why = f"draw {k}: original {o}, port {g}"
                        break
                else:
                    why = f"draw {k}: original none, port {g}"
                    break
        if why:
            print(f"MISMATCH case {i} (combat {combat} qsic {qsic} restore {restore} snd {snd} mid {mid} "
                  f"wanted {wanted} playing {playing}, opts {opts.hex()}, {len(ps)} passes): {why}")
            bad += 1
            if bad > 10:
                break
print(f"ctlpanel_input_diff: {ncases} cases, {ndraw[0]} draw calls, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
