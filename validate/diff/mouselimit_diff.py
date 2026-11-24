#!/usr/bin/env python3
"""Differential test: SetMouseLimit? 0x682AC, original vs port.

usage: .venv/bin/python validate/diff/mouselimit_diff.py [ncases=400] [DUMP] [--lib SO]
Per case: a limit (the rectangles of the original's call sites, then random ones) and mouse positions on and past
each line. The original runs in Unicorn with the DOS call 0x7C80D stubbed: the stub reads the two int 0x33 calls (AX 7,
AX 8; the driver has twice the x resolution) and acts as the driver, so it gives the position that the player sees. The
port: mouselimit::clamp (ja1f_mouse_clamp). Compared: the positions, plus on the original side the call order and the
four stored limit words 0x337CC4.. . Exits 1 on mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 400
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"mouselimit_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_mouse_clamp.argtypes = [ctypes.c_int32] * 6
PORT.ja1f_mouse_clamp.restype = ctypes.c_int64
SITES = [(0, 0, 319, 199), (3, 3, 312, 199), (0x8A, 0xA2, 0xAC, 0xAA), (0, 0x92, 319, 199), (1, 190, 318, 197)]
e = Emu(dump=dump, fast=True)
calls = []


def int86(em, eax, edx, ebx, *_):
    """int386x(0x33, inregs = EDX, outregs = EBX): the words of union REGS: AX +0, CX +8, DX +12."""
    b = bytes(em.uc.mem_read(edx, 16))
    calls.append((eax & 0xFF, struct.unpack_from("<H", b, 0)[0], struct.unpack_from("<H", b, 8)[0],
                  struct.unpack_from("<H", b, 12)[0]))
    return 0


e.stub(e.rt(0x7C80D), int86)
r = random.Random(682)
bad = 0
for c in range(ncases):
    if c < len(SITES):
        x0, y0, x1, y1 = SITES[c]
    else:
        x0, y0 = r.randrange(0, 200), r.randrange(0, 150)
        x1, y1 = r.randrange(x0, 320), r.randrange(y0, 200)
    calls.clear()
    e.call(e.rt(0x682AC), x0, y0, x1, y1, max_steps=20_000)
    why = []
    if [k[:2] for k in calls] != [(0x33, 7), (0x33, 8)]:
        why.append(f"calls {calls}")
    else:
        (_, _, cx, dx), (_, _, cy, dy) = calls
        stored = [e.r32(e.rt(0x337CC4 + 4 * k)) for k in range(4)]
        if stored != [x0, x1, y0, y1]:
            why.append(f"stored {stored}")
        for x, y in [(x0 - 1, y0 - 1), (x0, y0), (x1, y1), (x1 + 1, y1 + 1), (x0 - 1, y1 + 1), (x1 + 1, y0 - 1),
                     (r.randrange(320), r.randrange(200))]:
            driver = (min(max(2 * x, cx), dx) // 2, min(max(y, cy), dy))
            v = PORT.ja1f_mouse_clamp(x0, y0, x1, y1, x, y)
            port = ((v >> 16), (v & 0xFFFF))
            if port != driver:
                why.append(f"mouse {x},{y}: original {driver}, port {port}")
    if why:
        bad += 1
        if bad <= 5:
            print(f"case {c} limit {(x0, y0, x1, y1)}: {why[:3]}")
print(f"mouselimit_diff: {ncases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
