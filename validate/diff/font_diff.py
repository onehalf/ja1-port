#!/usr/bin/env python3
"""Differential test: the drawn pixels of the three fonts, DrawText 0x358D0 of the original vs the port's text_mode.

usage: .venv/bin/python validate/diff/font_diff.py [ncases=1500] [DUMP] [--lib SO]
The original: LoadSmallFont 0x381F8 gives the font sheets and the (pitch, height) table 0x949D6; the dump holds them, and
DrawText, TextWidth 0x35880, BlitRemap 0x522E4 and BlitRemapKey 0x5232D run for real in Unicorn. The port: Painter::text_mode
through ja1f_draw_text, with the sheets and the sizes of panel.rs.
Part "glyphs": every glyph of every font, alone, in rows away from the edges: this part sees each row and column of a sheet.
Part "cases": random text, place, colours, shadow, mode, centre, the right edge, the top rows, an unknown glyph in the text.
Compared: all 64000 bytes of a buffer filled with noise, and the table bytes 0xFF, 0xFE, 0 after the call.
Part "width": the centred place of each of the 256 byte values in each font, the only effect of TextWidth 0x35880's read
outside the tables, with a glyph that shows the place.
Not in the cases: glyphs of width 0 (the copy of 0 columns runs 2^32 columns in the original, see the stop point in the port).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"font_diff: {dump} missing")
    sys.exit(1)
ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
e = Emu(dump=dump, fast=True)
rt = e.rt
MARGIN = 320 * 20
BUF = e.alloc(bytes(MARGIN + 64000 + MARGIN)) + MARGIN
TEXT = e.alloc(bytes(512))
REMAP = e.r32(rt(0xE8C48))
TAB = bytes(e.uc.mem_read(REMAP, 256))
WIDTHS = [[e.r8(rt(0x9453C + f * 0x189 + 3 * c)) for c in range(131)] for f in range(3)]
rnd = random.Random(0xF047)
NOISE = bytes(rnd.randrange(256) for _ in range(64000))
fails = checks = 0
seen = {}


def run(text, x, y, centred, fg, shadow, font, mode, what):
    global fails, checks
    e.uc.mem_write(BUF, NOISE)
    e.uc.mem_write(TEXT, text + b"\0")
    e.uc.mem_write(REMAP, TAB)
    try:
        e.call(rt(0x358D0), TEXT, BUF, 320, x & 0xFFFFFFFF, y & 0xFFFFFFFF, centred, fg, shadow, font, mode)
    except EmuStop as err:
        fails += 1
        print(f"MISMATCH {what}: original stopped: {err} text {text.hex()} x {x} y {y} font {font} mode {mode}")
        return
    want = bytes(e.uc.mem_read(BUF, 64000))
    wleft = bytes(e.r8(REMAP + i) for i in (0xFF, 0xFE, 0))
    scr = ctypes.create_string_buffer(NOISE, 64000)
    left = ctypes.create_string_buffer(3)
    rc = PORT.ja1f_draw_text(ISO, EXE, text, x, y, centred, fg, shadow, font, mode, scr, left)
    checks += 1
    seen[what] = seen.get(what, 0) + 1
    if rc != 0 or scr.raw[:64000] != want or left.raw[:3] != wleft:
        fails += 1
        if fails <= 12:
            bad = [i for i in range(64000) if scr.raw[i] != want[i]]
            at = f"first (x {bad[0] % 320}, y {bad[0] // 320}), {len(bad)} pixels" if bad else "no pixel"
            print(f"MISMATCH {what}: rc {rc} text {text.hex()} x {x} y {y} centred {centred} fg {fg} shadow {shadow} "
                  f"font {font} mode {mode}: {at}; left {left.raw[:3].hex()} want {wleft.hex()}")


# every glyph of every font: every row and column of the sheets, plain and in the other two blits
for font in range(3):
    for c in range(131):
        if not WIDTHS[font][c]:
            continue
        ch = bytes([c + 0x20])
        run(ch, 100, 90, 0, 0x2A, 0, font, 300, "glyphs plain")
        run(ch, 100, 90, 0, 0x2A, 3, font, 300, "glyphs shadow")
        run(ch, 100, 90, 0, 0x2A, 3, font, 0x41, "glyphs mode")

for n in range(ncases):
    font = rnd.randrange(3)
    glyphs = [c for c in range(131) if WIDTHS[font][c]]
    text = bytes(rnd.choice(glyphs) + 0x20 for _ in range(rnd.choice((0, 1, 2, 5, 12, 40))))
    what = "cases"
    k = rnd.randrange(8)
    x, y = rnd.randrange(0, 320), rnd.randrange(0, 185)
    if k == 0:    # the right edge
        x, what = rnd.randrange(290, 320), "cases right edge"
    elif k == 1:  # the top rows (font 0 draws one row higher)
        y, what = rnd.randrange(0, 3), "cases top"
    elif k == 2 and text:  # an unknown glyph after some text: the two table bytes stay changed
        text = text[:rnd.randrange(len(text) + 1)] + bytes([rnd.choice((1, 0x1F, 0xA3, 0xFF))]) + text
        what = "cases unknown glyph"
    centred = rnd.randrange(2)
    fg, shadow = rnd.randrange(256), rnd.choice((0, 0, 1, rnd.randrange(256)))
    mode = rnd.choice((300, 300, 300, rnd.randrange(256), 0))
    run(text, x, y, centred, fg, shadow, font, mode, what)

for font in range(3):
    for b in range(1, 256):
        if 0x20 <= b < 0xA3 and not WIDTHS[font][b - 0x20]:
            continue
        run(bytes([b]) + b"A", 160, 90, 1, 0x2A, 0, font, 300, "width")
        run(b"A" + bytes([b, b]), 160, 90, 1, 0x2A, 0, font, 300, "width")   # twice: the half of an odd sum hides a 1

print("checked: " + ", ".join(f"{k} {v}" for k, v in seen.items()))
print(f"CASES font={checks}")
print(f"font_diff (port): {checks} checks, {fails} mismatches")
sys.exit(1 if fails else 0)
