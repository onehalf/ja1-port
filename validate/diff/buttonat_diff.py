"""Differential test: ButtonAt? 0x67CD8 for the salary box buttons (strategic.md "Salary raise"), original vs port.

usage: .venv/bin/python validate/diff/buttonat_diff.py [--lib SO]
The original runs ButtonAt? (EAX button, DL 1: button table 0x93458, the pressed element's w, h from 0x93734) in
Unicorn on the plain image with the left button down (0x229CF4 = 1) and the press steps stubbed (ButtonPressSound?
0x5C8A0, PressPanelButton? 0x35D44, PressedFieldBox? 0x35ED4, ButtonReleaseSound? 0x5C900), for the buttons of the
salary box 0x2573C: 5 NO, 6 OK, 0x11 down, 0x12 up. The mouse (0x229D00, 0x229D04) goes to every point of the
button's place widened by 3 pixels on each side. The port side is salary::button_hit (ja1f_salary_button_hit), the
place test of SalaryBox::button_at. Compared: hit or not. Box 1 review (#45 c13004: an edge fault in button_at was
GREEN). Exits 1 on any mismatch.
"""
import ctypes, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_salary_button_hit.argtypes = [ctypes.c_int32] * 3
PORT.ja1f_salary_button_hit.restype = ctypes.c_int32
BUTTON_AT, MOUSE_X, MOUSE_Y, MOUSE_B = 0x67CD8, 0x229D00, 0x229D04, 0x229CF4
IDS = [5, 6, 0x11, 0x12]

e = Emu(fast=True)
for a in (0x5C8A0, 0x35D44, 0x35ED4, 0x5C900):
    e.stub(a, lambda *_: 0)
cases = fails = hits = 0
for k, b in enumerate(IDS):
    rec = 0x93458 + 10 * b
    x0, y0 = e.r16(rec + 2), e.r16(rec + 4)
    el = e.r8(rec + 7)
    w, h = e.r16(0x93734 + 8 * el + 2), e.r16(0x93734 + 8 * el + 4)
    for y in range(y0 - 3, y0 + h + 4):
        for x in range(x0 - 3, x0 + w + 4):
            e.w8(MOUSE_B, 1)
            e.w32(MOUSE_X, x)
            e.w32(MOUSE_Y, y)
            want = e.call(BUTTON_AT, b, 1) & 0xFF
            port = PORT.ja1f_salary_button_hit(k, x, y)
            cases += 1
            hits += want != 0
            if (want != 0) != (port != 0):
                fails += 1
                if fails <= 10:
                    print(f"MISMATCH button {b:#x} at ({x}, {y}): original {want}, port {port}")
print(f"buttonat_diff: {len(IDS)} buttons, {cases} points ({hits} hits), {fails} mismatches")
sys.exit(1 if fails else 0)
