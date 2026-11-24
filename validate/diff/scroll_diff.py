"""Differential test: the arrow-key view scroll of TacticalKeyHandler 0x13490 (panel.md "Arrow-key scroll"),
original vs port.

usage: .venv/bin/python validate/diff/scroll_diff.py [ncases=4000] [DUMP] [--port] [--lib SO]
Per case: fuzzed steps dy 0x9344C / dx 0x9344D, hover timer 0x229D28, delay 0x229D44, switches Fast Scroll 0x1163BA /
Alt Scroll 0x1163BB, Alt 0x284F20, view x / y + window (ClampView 0x24830 reads view w / h 0x229CBC / 0x229CC0) and
the cursor tile 0x229D2C; then maybe one arrow key case (up 0x137F1, left 0x1382F, right 0x13850, down 0x13815, run to the handler's common
tail 0x1409B; it sets the handler's local "key scroll" byte at [esp]), then the handler's end 0x14721..0x147A4 (with
0x1EDAC, StepTile 0x46FF4 and ClampView unstubbed). The original runs in Unicorn on tac60, the port (port/ja1front
ja1f_scroll = viewmode::Scroll) on the same values; the 4 scroll values, view x / y, cursor tile, the redraw byte
0x284F14 and "moved" (0x1EDAC ran: it writes 1 to 0x229D08) must match. Without --port only the original runs.
Mouse cases (panel.md "Mouse-edge scroll"): the mouse x / y 0x229D00 / 0x229D04 (many at the screen edges), the view
mode 0x92DDF and the window rect 0x229CB8 / 0x229CC4 / 0x229CD8 / 0x229CDC of that mode; the mouse part
0x14201..0x14721 runs between the key case and the end, with every call in it stubbed (return 0: no button, no
popup) but MouseInRect 0x67C0C and MouseYInRows? 0x67C38; tactical_on 0x92DCC = 1, 0x92E35 = 0, buttons 0.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_ESP, UC_X86_REG_EIP
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, STACK_TOP       # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 4000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_scroll.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(ctypes.c_int32), ctypes.c_int32,
                                 ctypes.c_void_p]
    PORT.ja1f_scroll.restype = ctypes.c_int32

DY, DX, HOVER, DELAY = 0x9344C, 0x9344D, 0x229D28, 0x229D44
FAST, ALT_SCROLL, ALT = 0x1163BA, 0x1163BB, 0x284F20
VIEW_X, VIEW_Y, VIEW_W, VIEW_H, CURSOR, REDRAW, MOVED = 0x229CF8, 0x229CFC, 0x229CBC, 0x229CC0, 0x229D2C, 0x284F14, 0x229D08
KEYS = {0x48: 0x137F1, 0x4B: 0x1382F, 0x4D: 0x13850, 0x50: 0x13815}
TAIL, END_START, END = 0x1409B, 0x14721, 0x147A4
MOUSE_START = 0x14201
MOUSE_X, MOUSE_Y, MODE, TAC_ON, SMALL_END, BUTTONS, OVER_MAP = 0x229D00, 0x229D04, 0x92DDF, 0x92DCC, 0x92E35, 0x229CF4, 0x284F13
RECT = (0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC)
WINDOWS = {0: (14, 12, 48, 14), 1: (14, 12, 48, 14), 2: (20, 12, 0, 14), 3: (20, 14, 0, 0)}
STUBBED = (0x331A4, 0x1D9B8, 0x24630, 0x33570, 0x1D2BC, 0x1D8B8, 0x681D0, 0x673AC, 0x67BF0, 0x159CC, 0x67CD8, 0x121D8,
           0x1CBA8, 0x54D50, 0x1328C, 0x1002C, 0x4B788, 0x11DC0, 0x54F3C, 0x11714, 0x1CB58)

e = Emu(dump=dump, fast=True)
rt = e.rt
uc = e.uc
for a in STUBBED:
    e.stub(rt(a), lambda em, *a: 0)
# The mouse part falls through to the end block inside one translated block, where emu_start's `until` is not seen.
STOP = [False]
uc.hook_add(UC_HOOK_CODE, lambda u, *_: STOP[0] and u.emu_stop(), None, rt(END_START), rt(END_START))


def s8(v):
    return v - 256 if v >= 128 else v


def s32(v):
    return v - (1 << 32) if v >> 31 else v


rng = random.Random(int(os.environ.get("SCROLL_SEED", "1")))
fails, by_kind, moved_n = 0, {}, 0
keep = [(a, n) for a, n in ((DY, 1), (DX, 1), (HOVER, 4), (DELAY, 4), (FAST, 1), (ALT_SCROLL, 1), (ALT, 1),
                            (VIEW_X, 4), (VIEW_Y, 4), (VIEW_W, 4), (VIEW_H, 4), (CURSOR, 4), (REDRAW, 1), (MOVED, 4),
                            (MOUSE_X, 4), (MOUSE_Y, 4), (MODE, 1), (TAC_ON, 1), (SMALL_END, 1), (BUTTONS, 4),
                            (OVER_MAP, 1)) + tuple((r, 4) for r in RECT)]
save = {a: bytes(uc.mem_read(rt(a), n)) for a, n in keep}
sp = STACK_TOP - 0x200
for i in range(ncases):
    step = lambda big: rng.choice([0, 0, big, -big, rng.randrange(-128, 128)])
    dy, dx = step(14), step(16)
    timer = lambda: rng.choice([rng.randrange(-60, 260), 0, 10, -10, 70, 200, rng.randrange(-(1 << 31), 1 << 31)])
    hover, delay = timer(), timer()
    fast, alt_scroll, alt = (rng.choice([0, 0, 1, rng.randrange(256)]) for _ in range(3))
    mouse = rng.random() < 0.6
    mode = rng.randrange(4)
    cols, rows, left, top = WINDOWS[mode]
    if mode == 3 and rng.random() < 0.3:
        rows = 13
    if not mouse:
        cols, rows = rng.choice([(14, 12), (20, 12), (20, 14), (20, 13)])
    edge = lambda hi: rng.choice([0, 0, 1, hi - 1, hi, hi, rng.randrange(-3, 3), rng.randrange(hi - 2, hi + 3),
                                  rng.randrange(hi + 1)])
    mx, my = edge(319), edge(199)
    over_map = 0      # 1 = the mouse just left the map: cursor tile 0xC41, path marks off (the cursor code, not this test)
    vx = rng.choice([16 * rng.randrange(57), rng.randrange(-200, 1100)])
    vy = rng.choice([14 * rng.randrange(57), rng.randrange(-200, 1000)])
    cur = rng.choice([rng.randrange(0xC40), 0xC41, rng.choice([0, 55, 56, 0xC07, 0xC08, 0xC3F]), rng.randrange(-60, 0xC80)])
    key = rng.choice([0, 0, 0x48, 0x4B, 0x4D, 0x50])
    for a, n, v in ((DY, 1, dy), (DX, 1, dx), (HOVER, 4, hover), (DELAY, 4, delay), (FAST, 1, fast),
                    (ALT_SCROLL, 1, alt_scroll), (ALT, 1, alt), (VIEW_X, 4, vx), (VIEW_Y, 4, vy),
                    (VIEW_W, 4, cols * 16), (VIEW_H, 4, rows * 14), (CURSOR, 4, cur), (REDRAW, 1, 0), (MOVED, 4, 0)):
        (e.w8 if n == 1 else e.w32)(rt(a), v & 0xFFFFFFFF)
    e.w32(sp, 0)
    uc.reg_write(UC_X86_REG_ESP, sp)
    if key:
        uc.emu_start(rt(KEYS[key]), rt(TAIL), timeout=2_000_000)
    if mouse:
        for a, n, v in ((MOUSE_X, 4, mx), (MOUSE_Y, 4, my), (MODE, 1, mode), (TAC_ON, 1, 1), (SMALL_END, 1, 0),
                        (BUTTONS, 4, 0), (OVER_MAP, 1, over_map), (RECT[0], 4, left), (RECT[1], 4, top),
                        (RECT[2], 4, left + 16 * cols - 1), (RECT[3], 4, top + 14 * rows - 1)):
            (e.w8 if n == 1 else e.w32)(rt(a), v & 0xFFFFFFFF)
        uc.reg_write(UC_X86_REG_ESP, sp)
        STOP[0] = True
        uc.emu_start(rt(MOUSE_START), rt(END_START), timeout=2_000_000)
        STOP[0] = False
        if uc.reg_read(UC_X86_REG_EIP) != rt(END_START):
            raise SystemExit(f"case {i}: the mouse part did not reach the end block (eip {uc.reg_read(UC_X86_REG_EIP) - rt(0x10000) + 0x10000:#x}; mouse ({mx}, {my}) mode {mode} over {over_map} key {key})")
    uc.reg_write(UC_X86_REG_ESP, sp)
    uc.emu_start(rt(END_START), rt(END), timeout=2_000_000)
    got = (s8(e.r8(rt(DY))), s8(e.r8(rt(DX))), s32(e.r32(rt(HOVER))), s32(e.r32(rt(DELAY))), s32(e.r32(rt(VIEW_X))),
           s32(e.r32(rt(VIEW_Y))), s32(e.r32(rt(CURSOR))), e.r8(rt(REDRAW)), e.r32(rt(MOVED)))
    moved_n += got[-1]
    kind = (f"key {key:#x}" if key else "no key") + (" + mouse" if mouse else "")
    by_kind[kind] = by_kind.get(kind, 0) + 1
    bad = []
    if PORT:
        s = (ctypes.c_int32 * 4)(dy, dx, hover, delay)
        v = (ctypes.c_int32 * 22)()
        v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[11], v[12] = mode if mouse else 1, vx, vy, cols, rows, left, top, 0x40, 0x40
        c = ctypes.c_int32(cur)
        o = (ctypes.c_int32 * 6)(fast, alt_scroll, alt, int(mouse), mx, my)
        r = PORT.ja1f_scroll(ctypes.addressof(s), ctypes.addressof(v), ctypes.byref(c), key, ctypes.addressof(o))
        want = (s8(s[0] & 0xFF), s8(s[1] & 0xFF), s[2], s[3], v[1], v[2], c.value, v[20], r)
        if got != want:
            bad.append(f"(dy, dx, hover, delay, view x, view y, cursor, redraw, moved) original {got} port {want}")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: {kind} dy {dy} dx {dx} hover {hover} delay {delay} fast {fast} alt_scroll "
                  f"{alt_scroll} alt {alt} view ({vx}, {vy}) {cols}x{rows} cursor {cur} mouse ({mx}, {my}) mode {mode}: {bad}")
for a, b in save.items():
    uc.mem_write(rt(a), b)
print(f"scroll_diff: {ncases} cases ({', '.join(f'{k} {n}' for k, n in sorted(by_kind.items()))}; {moved_n} moved)"
      f"{'' if PORT else ' (original only)'}; {fails} mismatches")
sys.exit(1 if fails else 0)
