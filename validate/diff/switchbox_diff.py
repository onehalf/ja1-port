"""Differential test: the office's switch box (strategic.md "The switch box (Control Panel)"), original vs port.

usage: .venv/bin/python validate/diff/switchbox_diff.py [ncases=100] [DUMP] [--port] [--lib SO]
Per case: fuzzed 20 switch bytes (0, 1 or other values), sound / music driver bytes, playing / wanted theme words,
a noise office picture (g_panelBuf at entry) and a noise plain room (what DrawRoomScreen 0x4C304 draws, stubbed),
and 1..12 left presses (on switches 0..19, at rectangle edges, anywhere) that end with one on the close lever. The
original SwitchBoxScreen? 0x4D310 runs in Unicorn on tac60 with the PCX loader 0x532BC stubbed (DAT/OFFICE.DAT
member by name), MousePoll 0x678E8 = the next press, the timer 0x47040 always past, PlaySfxSlot 0x5CB34 / SetMusic
0x5C5EC / StopMusic 0x5C82C logged. Logged pictures: g_panelBuf at every 0x46BF4 (panel to the screen), the screen
at every close-lever frame (TimerStart 0x47020 from the input loop 0x4D114) and after the dissolve 0x52C98. The port
(port/ja1front ja1f_switchbox) gives its steps on the same inputs. Compared: the order of pictures, sounds and
music calls, every picture, the 20 bytes at the end.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
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
ncases = int(args[0]) if args else 100
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_switchbox.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_char_p] * 2 + [ctypes.c_void_p] + \
        [ctypes.c_int32] * 4 + [ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_switchbox.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()
OPTS, PANEL_P, SCREEN_P, BUTTONS, MX, MY = 0x1163B8, 0x93E60, 0x95678, 0x229CF4, 0x229D00, 0x229D04
SND_DRV, MID_DRV, PLAYING, WANTED = 0x96399, 0x9639A, 0x9635E, 0x96360
_pics = {}


def pic(name):
    n = (name.decode("latin-1") if isinstance(name, bytes) else name).upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(OFFICE, n))
        _pics[n] = (w, h, px)
    return _pics[n]


e = Emu(dump=dump, fast=True)
log, presses, plain_room = [], [], [b""]


def load(em, eax, edx, ebx, ecx):
    w, h, px = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def ret():
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP))


def show(em, *_):
    px = bytes(e.uc.mem_read(panel, 64000))
    e.uc.mem_write(screen, px)
    log.append(("pic", px))


def timer_start(em, *_):
    if e.rt(0x4D114) <= ret() < e.rt(0x4D310):
        log.append(("pic", bytes(e.uc.mem_read(screen, 64000))))


def show_mouse(em, *_):
    if ret() == e.rt(0x4D708):          # after the dissolve 0x52C98 (call at 0x4D6FE)
        log.append(("pic", bytes(e.uc.mem_read(screen, 64000))))


def poll(em, *_):
    if presses:
        x, y = presses.pop(0)
        e.w32(e.rt(MX), x)
        e.w32(e.rt(MY), y)
        e.w32(e.rt(BUTTONS), 1)
    else:
        log.append(("extra poll",))
        e.w32(e.rt(MX), 250)
        e.w32(e.rt(MY), 40)
        e.w32(e.rt(BUTTONS), 1)
    return 0


def room(em, *_):
    e.uc.mem_write(panel, plain_room[0])


for a in (0x67BF0, 0x67C54, 0x67F90, 0x53300, 0x52820, 0x52870, 0x4C26C):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x47040), lambda em, *_: 100)          # TimerElapsedMs: always past
e.stub(e.rt(0x47020), timer_start)
e.stub(e.rt(0x673AC), show_mouse)
e.stub(e.rt(0x46BF4), show)
e.stub(e.rt(0x678E8), poll)
e.stub(e.rt(0x4C304), room)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x5CB34), lambda em, a, d, b, c: log.append(("sfx", a, d, b & 0xFF)))
e.stub(e.rt(0x5C5EC), lambda em, *_: log.append(("music", 1, 1)))
e.stub(e.rt(0x5C82C), lambda em, *_: log.append(("music", 0, 1)))
screen = e.alloc(bytes(64000))
e.w32(e.rt(SCREEN_P), screen)
panel = e.r32(e.rt(PANEL_P))
r = random.Random(1)


def place(i):
    return (28, 26 + 15 * i) if i < 10 else (137, 26 + 15 * (i - 10))


def a_press():
    c = r.random()
    if c < 0.7:
        x, y = place(r.choice([0, 0, 5, 5, 1] + list(range(20))))
        return (x + r.choice([0, 26, 27, -1, r.randrange(27)]), y + r.choice([0, 7, 8, -1, r.randrange(8)]))
    if c < 0.8:
        return (r.choice([243, 244, 266, 267]), r.choice([30, 31, 96, 97, r.randrange(31, 97)]))
    return (r.randrange(320), r.randrange(200))


bad = 0
for i in range(ncases):
    opts = bytes(r.choice([0, 1, 1, 0, 2, 255, r.randrange(256)]) for _ in range(20))
    e.uc.mem_write(e.rt(OPTS), opts)
    snd, mid = r.choice([1, 1, 0]), r.choice([1, 1, 0])
    e.w8(e.rt(SND_DRV), snd)
    e.w8(e.rt(MID_DRV), mid)
    playing, wanted = r.choice([-1, 0, 1, 5, -1]), r.choice([-1, 0, 2, 7])
    e.w16(e.rt(PLAYING), playing)
    e.w16(e.rt(WANTED), wanted)
    ps = [a_press() for _ in range(r.randrange(0, 12))] + [(244 + r.randrange(23), 31 + r.randrange(66))]
    presses[:] = ps
    noise_s = bytes(r.randrange(256) for _ in range(64000))
    plain_room[0] = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(panel, noise_s)
    e.uc.mem_write(screen, bytes(64000))
    log.clear()
    e.w32(e.rt(BUTTONS), 0)
    e.call(e.rt(0x4D310), max_steps=200_000_000)
    want_opts = bytes(e.uc.mem_read(e.rt(OPTS), 20))
    if PORT:
        op = ctypes.create_string_buffer(opts, 20)
        xy = (ctypes.c_int32 * (2 * len(ps)))(*[v for q in ps for v in q])
        cap = 400
        lg = (ctypes.c_int32 * (4 * cap))()
        pics = ctypes.create_string_buffer(64000 * 80)
        ct = (ctypes.c_int32 * 2)()
        rc = PORT.ja1f_switchbox(ISO, EXE, noise_s, plain_room[0], op, snd, mid, wanted, playing, xy, len(ps), lg,
                                 cap, pics, 80, ct)
        got = []
        for k in range(ct[0]):
            t, a, b, c = lg[4 * k:4 * k + 4]
            if t == 0:
                got.append(("pic", pics.raw[a * 64000:(a + 1) * 64000]))
            elif t == 1:
                got.append(("sfx", a, b, c))
            elif t == 2 and b:
                got.append(("music", a, 1))
        why = None
        if rc != 0:
            why = f"rc {rc}"
        elif op.raw != want_opts:
            why = f"switch bytes {op.raw.hex()} vs {want_opts.hex()}"
        else:
            for k in range(max(len(got), len(log))):
                g, o = (got[k] if k < len(got) else None), (log[k] if k < len(log) else None)
                if g != o:
                    if g and o and g[0] == o[0] == "pic":
                        q = next(q for q in range(64000) if g[1][q] != o[1][q])
                        why = f"step {k} picture @({q % 320},{q // 320}) {o[1][q]:#x} vs {g[1][q]:#x}"
                    else:
                        why = f"step {k}: original {o and o[:4] if o and o[0] != 'pic' else o and 'pic'}, " \
                              f"port {g and g[:4] if g and g[0] != 'pic' else g and 'pic'}"
                    break
        if why:
            print(f"MISMATCH case {i} ({len(ps)} presses, {len(log)} steps): {why}")
            bad += 1
            if bad > 10:
                break
print(f"switchbox_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
