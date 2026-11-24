"""Differential test: the tactical Control Panel's opening picture (panel.md "The Control Panel (key F10, `opt`
button)"), original vs port.

usage: .venv/bin/python validate/diff/ctlpanel_diff.py [ncases=100] [DUMP] [--port] [--lib SO]
Per case: fuzzed 16 switch bytes (0 or 1; the original's ON / OFF string lookup takes any other value through the
neighbouring strings of its table, which a real game never makes), the in-combat byte 0x92DCB, the quick-save-in-combat
byte g_quickSaveInCombat 0x22A1F9, the Quick Restore presence (FileExists? 0x4707C), and a noise back buffer. The
save-anywhere byte 0x92DC3 stays 0 (the port keeps it 0). The original ControlPanel 0x4B788 runs in Unicorn on
tac60 to an int3 planted at the ControlPanelInput entry 0x4BAA4, with the redraw 0x1CB78, the mouse 0x67BF0 /
0x673AC / 0x682AC and the wait 0x67C54 stubbed; DrawText 0x358D0 runs for real. Compared: the 320 x 200 back
buffer, and the 17 item places (0x28E1A6 entries and the Continue Game place 0x28E1E8). The port (port/ja1front
ja1f_ctlpanel_open) paints on the same noise from a world loaded from QICKSTRT.SAV with the same switches.
The input loop itself (ControlPanelInput 0x4BAA4) is not tested here.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop        # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 100
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_ctlpanel_open.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_void_p, ctypes.c_void_p] + \
        [ctypes.c_int32] * 3 + [ctypes.c_void_p]
    PORT.ja1f_ctlpanel_open.restype = ctypes.c_int32
    PORT.ja1f_ctl_mouse_box.argtypes = [ctypes.c_int32] * 3
    PORT.ja1f_ctl_mouse_box.restype = ctypes.c_int64
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV").encode()

SCREEN_P, OPTS, COMBAT, SAVEANY, QSIC = 0x95678, 0x1163B8, 0x92DCB, 0x92DC3, 0x22A1F9
PANEL_ON, PLACES, CONT = 0x92DCC, 0x28E1A8, 0x28E1E8

e = Emu(dump=dump, fast=True)
screen = e.alloc(bytes(64000))
e.w32(e.rt(SCREEN_P), screen)
for a in (0x1CB78, 0x67BF0, 0x673AC, 0x67C54):
    e.stub(e.rt(a), lambda em, *_: 0)
limits = []                # 0x4B854: the mouse limit is the view window
e.stub(e.rt(0x682AC), lambda em, a, d, b, c: limits.append((a, d, b, c)) or 0)
VIEWS = ((14, 14, 12), (14, 20, 12), (0, 20, 14), (0, 20, 13))   # SetViewWindow 0x24880 (top, cols, rows) of all modes
restore_flag = [0]
e.stub(e.rt(0x4707C), lambda em, *_: restore_flag[0])
e.uc.mem_write(e.rt(0x4BAA4), b"\xCC")       # int3 at the ControlPanelInput call
e.w8(e.rt(PANEL_ON), 1)
e.w8(e.rt(SAVEANY), 0)

r = random.Random(7)
bad = 0
for i in range(ncases):
    opts = bytes(r.choice([0, 1]) for _ in range(16))
    combat = r.choice([0, 1])
    qsic = r.choice([0, 1])
    restore_flag[0] = r.choice([0, 1])
    noise = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(e.rt(OPTS), opts)
    e.w8(e.rt(COMBAT), combat)
    e.w8(e.rt(QSIC), qsic)
    e.uc.mem_write(screen, noise)
    view = r.choice(VIEWS)
    e.call(e.rt(0x24880), *view)
    limits.clear()
    try:
        e.call(e.rt(0x4B788))
    except EmuStop as stop:
        if "INT 0x03" not in str(stop):
            print(f"MISMATCH case {i}: the original stopped at {stop}")
            bad += 1
            continue
    o = bytes(e.uc.mem_read(screen, 64000))
    places = [e.r16(e.rt(PLACES + 4 * k)) | (e.r16(e.rt(PLACES + 4 * k + 2)) << 16) for k in range(16)]
    places.append(e.r32(e.rt(CONT)))
    if o == noise:
        print(f"MISMATCH case {i}: the original drew nothing")
        bad += 1
        continue
    if PORT:
        v = PORT.ja1f_ctl_mouse_box(view[1] * 16, view[2] * 14, 3 if view[0] == 0 else 1)
        got = tuple((v >> (12 * k)) & 0xFFF for k in range(4))
        if limits[:1] != [got]:
            print(f"MISMATCH case {i}: view {view} limit {limits[:1]} vs {got}")
            bad += 1
    if PORT:
        buf = ctypes.create_string_buffer(noise, 64000)
        pl = (ctypes.c_int32 * 34)()
        rc = PORT.ja1f_ctlpanel_open(ISO, EXE, SAVE, buf, opts, combat, qsic, restore_flag[0], pl)
        p = buf.raw
        pp = [pl[2 * k] | (pl[2 * k + 1] << 16) for k in range(17)]
        why = None
        if rc != 0:
            why = f"rc {rc}"
        elif pp != places:
            why = f"places {pp} vs {places}"
        elif p != o:
            nb = next(q for q in range(64000) if p[q] != o[q])
            why = f"panel @({nb % 320},{nb // 320}) {o[nb]:#x} vs {p[nb]:#x}"
        if why:
            print(f"MISMATCH case {i} (combat {combat} qsic {qsic} restore {restore_flag[0]}, "
                  f"opts {opts.hex()}): {why}")
            bad += 1
            if bad > 10:
                break
print(f"ctlpanel_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
