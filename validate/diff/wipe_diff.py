"""Differential test: the view wipes of NewTurn? (turns.md "New turn" step 14 e), original vs port.

usage: .venv/bin/python validate/diff/wipe_diff.py [ncases=60] [DUMP] [--port] [--lib SO]
Per case: a view window (the three view modes' windows, then fuzzed ones with odd sizes) on a noise screen. The
original runs 0x124FC (host op 164 view_save: the view closes to black from both sides) and 0x1259C (op 165
view_restore: the new view opens from the middle row) in Unicorn on tac60; the 1 ms wait 0x67250 is stubbed and
takes a copy of the screen, the view drawing 0x1CB78 is stubbed (the back buffer holds a noise view at (32, 28),
pitch 384). The port (port/ja1front ja1f_wipe = wipe::close / wipe::open) must give the same screens. Without --port
only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_wipe.argtypes = [ctypes.c_int32, ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_int32] * 5 + [ctypes.c_void_p]
    PORT.ja1f_wipe.restype = ctypes.c_int32

SCREEN_P, BACK = 0x95678, 0x22A4EF
LEFT, W, H, TOP, RIGHT = 0x229CB8, 0x229CBC, 0x229CC0, 0x229CC4, 0x229CD8
e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(SCREEN_P))
shots = []
e.stub(e.rt(0x67250), lambda em, *_: shots.append(bytes(em.uc.mem_read(screen, 64000))) or 0)
e.stub(e.rt(0x1CB78), lambda em, *_: 0)

r = random.Random(33)
cases = [(48, 14, 224, 168), (0, 14, 320, 168), (0, 0, 320, 196), (0, 0, 320, 182)]
while len(cases) < ncases:
    w, h = r.randrange(1, 321), r.randrange(1, 197)
    cases.append((r.randrange(0, 321 - w), r.randrange(0, 201 - h), w, h))
bad = steps = 0
for i, (left, top, w, h) in enumerate(cases):
    for a, v in ((LEFT, left), (TOP, top), (W, w), (H, h), (RIGHT, left + w - 1)):
        e.w32(e.rt(a), v)
    base = bytes(r.randrange(256) for _ in range(64000))
    view = bytes(r.randrange(256) for _ in range(w * h))
    for y in range(h):
        e.uc.mem_write(e.rt(BACK) + (28 + y) * 384 + 32, view[y * w:(y + 1) * w])
    for kind, fn in ((0, 0x124FC), (1, 0x1259C)):
        e.uc.mem_write(screen, base)
        shots.clear()
        e.call(e.rt(fn))
        steps += len(shots)
        if PORT:
            n = max(len(shots), 1) + 1
            out = ctypes.create_string_buffer(n * 64000)
            rc = PORT.ja1f_wipe(kind, base, view, left, top, w, h, n, out)
            if rc != len(shots):
                print(f"MISMATCH case {i} kind {kind} ({left}, {top}, {w}, {h}): {len(shots)} screens vs {rc}")
                bad += 1
                continue
            for f in range(rc):
                o, p = shots[f], out.raw[f * 64000:(f + 1) * 64000]
                if o != p:
                    m = next(k for k in range(64000) if o[k] != p[k])
                    print(f"MISMATCH case {i} kind {kind} ({left}, {top}, {w}, {h}): screen {f} @({m % 320},{m // 320})"
                          f" {o[m]:#x} vs {p[m]:#x}")
                    bad += 1
                    break
    if bad > 10:
        break
print(f"wipe_diff: {len(cases)} cases ({steps} wait screens), {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
