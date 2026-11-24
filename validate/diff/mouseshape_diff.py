#!/usr/bin/env python3
"""Differential test: SetMouseShape? 0x674B0 + DrawMousePointer? 0x67A28 and MouseSavePos2? 0x67E18, original vs port.

usage: .venv/bin/python validate/diff/mouseshape_diff.py [DUMP] [--lib SO]
The original runs in Unicorn with nothing stubbed: the bodies, BlitCopy, BlitRemap, PictureSize? and the sheet and pictures
of the dump. Shapes 0, 200..203: width, height, hot spot and the 42 x 16 buffer 0x93E74 (colours 0 and 0x17 = not drawn, as the
draw's table) against pointer::shape. Held items: DrawMousePointer? at the mouse + hot spot into the screen, against
pointer::held_item + held_picture on the same clear screen, at inner and edge mouse places. Original side only: a repeat of the
shape changes nothing; each change sets the redraw request 0x229C34 and keeps the old shape in 0x229C3C.
MouseSavePos2? + MouseRestorePos2? + MouseSetPos? with player moves between them, against pointer::turn_mouse. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"mouseshape_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
PORT.ja1f_pointer_shape.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p]
PORT.ja1f_pointer_held.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_size_t,
                                   ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p]
HOTX, HOTY, W, H, SHAPE, OLD, REDRAW = 0x229C70, 0x229C48, 0x229C30, 0x229C58, 0x229C54, 0x229C3C, 0x229C34
BUF, TARGET, SAVE, TABLE, TYPES = 0x93E74, 0x95678, 0x93E7C, 0xE8C48, 0x94DF8
e = Emu(dump=dump, fast=True)
rd = lambda a, n: bytes(e.uc.mem_read(a, n))    # a = runtime address
i32 = lambda a: int.from_bytes(rd(e.rt(a), 4), "little", signed=True)
buf, target = e.r32(e.rt(BUF)), e.r32(e.rt(TARGET))
if buf == 0 or target == 0:
    print("mouseshape_diff: no pointer buffer in the dump")
    sys.exit(1)
bad = seen = 0
cover = {"plain": 0, "repeat": 0, "held inner": 0, "held clipped": 0, "pic 42": 0, "pic 30": 0, "pic 16": 0, "turn": 0}


def fail(msg):
    global bad
    bad += 1
    if bad <= 8:
        print(msg)


def set_shape(s, old=-1):
    """Run the body from another shape; `old` is not a real shape, so the body always runs."""
    e.w32(e.rt(SHAPE), old & 0xFFFFFFFF)
    e.w32(e.rt(REDRAW), 0)
    e.call(e.rt(0x674B0), s, max_steps=400_000)
    if i32(SHAPE) != s or i32(OLD) != old or i32(REDRAW) != 1:
        fail(f"shape {s}: state {i32(SHAPE)} {i32(OLD)} {i32(REDRAW)}")


# the plain shapes
for s in (0, 200, 201, 202, 203):
    set_shape(s)
    w, h, hx, hy = i32(W), i32(H), i32(HOTX), i32(HOTY)
    o = rd(buf, 42 * 16)
    out, pix = (ctypes.c_int32 * 4)(), (ctypes.c_uint8 * 672)()
    if PORT.ja1f_pointer_shape(ISO, EXE, s, out, pix) != 1:
        fail(f"shape {s}: the port has no such shape")
        continue
    seen += 1
    cover["plain"] += 1
    if (out[0], out[1], out[2], out[3]) != (w, h, -hx, -hy):
        fail(f"shape {s}: size + hot spot original {w, h, -hx, -hy}, port {tuple(out)}")
    want = [0 if o[y * 42 + x] == 0x17 else o[y * 42 + x] for y in range(h) for x in range(w)]
    got = list(pix[:w * h]) if (out[0], out[1]) == (w, h) else []
    if want != got:
        diff = sum(a != b for a, b in zip(want, got))
        fail(f"shape {s}: pixels differ ({diff} of {len(want)}, port {len(got)})")
    outside = sum(1 for y in range(16) for x in range(42) if (x >= w or y >= h) and o[y * 42 + x])
    if outside:     # the rows that the draw does not read are clear, so no hidden picture part exists
        fail(f"shape {s}: {outside} pixels outside the drawn size")
    # a repeat of the shape does nothing
    e.w32(e.rt(REDRAW), 0)
    e.uc.mem_write(buf, b"\x55" * 4)
    e.call(e.rt(0x674B0), s, max_steps=400_000)
    cover["repeat"] += 1
    if i32(REDRAW) != 0 or rd(buf, 4) != b"\x55" * 4:
        fail(f"shape {s}: a repeat changes the state")
    # the shape that a button press swaps in has the same place in the port: only the shape number differs

# held items: the original draws through DrawMousePointer? into the back buffer
types = rd(e.rt(TYPES), 1632)
PLACES = [(100, 80), (0, 0), (1, 7), (23, 8), (24, 9), (319, 199), (300, 190), (278, 184), (277, 183), (10, 199), (319, 3)]
r = random.Random(674)
tt = (ctypes.c_uint8 * len(types)).from_buffer_copy(types)
for item in range(1, 136):
    pic = types[12 * item + 4]
    for mx, my in PLACES[:2] + [r.choice(PLACES[2:]), (r.randrange(320), r.randrange(200))]:
        set_shape(item)
        hx, hy = i32(HOTX), i32(HOTY)
        e.uc.mem_write(target, bytes(64000))
        e.call(e.rt(0x67A28), (mx + hx) & 0xFFFFFFFF, (my + hy) & 0xFFFFFFFF, 0, 0, max_steps=600_000)
        o = rd(target, 64000)
        p = (ctypes.c_uint8 * 64000)()
        got_pic = PORT.ja1f_pointer_held(ISO, EXE, tt, len(types), item, mx, my, p)
        seen += 1
        if got_pic != pic:
            fail(f"item {item}: picture original {pic}, port {got_pic}")
        g = bytes(p)
        if o != g:
            at = [i for i in range(64000) if o[i] != g[i]]
            fail(f"item {item} pic {pic} mouse {mx, my}: {len(at)} pixels differ, first ({at[0] % 320},{at[0] // 320}) "
                 f"original {o[at[0]]:#x} port {g[at[0]]:#x}")
        clipped = not (mx >= 24 and my >= 8 and mx + 18 <= 320 and my + 8 <= 200)
        cover["held clipped" if clipped else "held inner"] += 1
        cover["pic 42" if pic <= 20 else "pic 30" if pic <= 90 else "pic 16"] += 1
    if i32(TABLE) and rd(e.r32(e.rt(TABLE)) + 0x17, 1) != b"\x17":
        fail("the draw leaves the table's colour 0x17 changed")

# A turn change: MouseSetPos? 0x681D0 (the int 33h call stubbed), MouseSavePos2? 0x67E18, MouseRestorePos2? 0x67E30, and the player's
# moves between them (the queue writes 0x229D00 / 0x229D04). Against pointer::turn_mouse.
PORT.ja1f_turn_mouse.argtypes = [ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p]
e.stub(e.rt(0x7C80D), lambda em, *_: 0)
for c in range(60):
    x, y = r.randrange(320), r.randrange(200)
    tape = [(147, 0, 0)] if c % 2 else []     # a tape can start with the save, as at the start of combat
    for _ in range(12):
        tape.append(r.choice([(0, r.randrange(320), r.randrange(200)), (146, 0x50, 8), (147, 0, 0), (157, 0, 0), (0, r.randrange(320), r.randrange(200))]))
    e.w32(e.rt(0x229D00), x)
    e.w32(e.rt(0x229D04), y)
    e.w32(e.rt(0x229C40), 0)
    e.w32(e.rt(0x229C44), 0)
    flat = (ctypes.c_int32 * (3 * len(tape)))(*[v for row in tape for v in row])
    out = (ctypes.c_int32 * (2 * len(tape)))()
    PORT.ja1f_turn_mouse(x, y, flat, len(tape), out)
    for i, (op, a, b) in enumerate(tape):
        if op == 0:
            e.w32(e.rt(0x229D00), a)
            e.w32(e.rt(0x229D04), b)
        elif op == 146:
            e.call(e.rt(0x681D0), a, b, max_steps=2000)
        elif op == 147:
            e.call(e.rt(0x67E18), max_steps=1000)
        else:
            e.call(e.rt(0x67E30), max_steps=2000)
        seen += 1
        cover["turn"] += 1
        if (i32(0x229D00), i32(0x229D04)) != (out[2 * i], out[2 * i + 1]):
            fail(f"turn tape {c} row {i} op {op}: original {i32(0x229D00), i32(0x229D04)}, port {out[2 * i], out[2 * i + 1]}")
            break

print(f"mouseshape_diff: {seen} cases, {bad} mismatches; {cover}")
empty = [k for k, v in cover.items() if v == 0]
if empty:
    print(f"mouseshape_diff: no case reached {empty}")
    sys.exit(1)
sys.exit(1 if bad else 0)
