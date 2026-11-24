"""Differential test: the slide-in of the assignment screen, AssignPanelFlipIn? 0x47B68, original vs port.

usage: .venv/bin/python validate/diff/flipin_diff.py [ncases=30] [DUMP] [--port] [--lib SO]
The original runs whole in Unicorn on tac60: PlayFlipAnim 0x51264, DrawPanelPccFields 0x36010 (ASSPAN.PCC and the
fields 27..30), RestoreMapTextStrip? 0x41608 and the blits are real. The picture loader 0x532BC reads MAIN.DAT. The
clock is virtual: each read of TimerElapsedMs 0x47040 adds 1 ms, and the screen is noted at each read.
Per case: a noise screen, back buffer and panel buffer (rows 0..145 of the panel buffer are the slide's new picture),
fuzzed accounts, the mouse row.
Compared with MapUi::flipin_test (ja1f_flipin, the real assign_flip_steps and assign_slide_steps): each picture, the
time it shows (at most 1 ms of difference: a read of the clock after a wait), the pointer while it shows (hidden in the
flip, shape 0xC9 in the slide: HidePointer?, ShowMouse?, SetMouseShape?, 0x67478 logged), the sounds with the picture
they start on, the panel buffer. Then PlayFlipAnim 0x51264 alone with the arguments of its 3 other callers (finance page open and close,
assignment leave, payroll screen) against MapUi::flip_test (ja1f_flip_steps). On the original side also: the shapes 0xC9 then 0 (the 0 at the end: the window's shape after the slide). Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
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
ncases = int(args[0]) if args else 30
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_flip_steps.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p, ctypes.c_int32] + [ctypes.c_void_p] * 3 + [ctypes.c_size_t]
    PORT.ja1f_flipin.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 3 + [ctypes.c_size_t] + [ctypes.c_void_p] * 3 + [ctypes.c_size_t,
                                                                                                                 ctypes.c_void_p]
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
ACC, BACK, SHOT, CAP = 0x229FA0, 0x22A4EF, 64000, 64
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen, panel = e.r32(e.rt(0x95678)), e.r32(e.rt(0x93E60))
clock = [0]
starts = {}
frames, sounds, shapes = [], [], []
ptr = {"hidden": False, "shape": 0}   # the pointer as HidePointer?, ShowMouse?, SetMouseShape? and 0x67478 leave it


def ptr_now():
    return -1 if ptr["hidden"] else ptr["shape"]


def load(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    w, h, px, _ = pcx.decode(datarc.member(MAIN, name))
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def note(em):
    s = bytes(em.uc.mem_read(screen, SHOT))
    if not frames or frames[-1][0] != s:
        frames.append((s, clock[0], ptr_now()))


def t_start(em, eax, *_):
    starts[eax] = clock[0]
    return 0


def t_elapsed(em, eax, *_):
    note(em)
    clock[0] += 1
    return clock[0] - starts[eax]


def sfx(em, a, d, b, c):
    note_len = len(frames)
    sounds.append((note_len, a, d, b, c))
    return 0


def set_ptr(hidden=None, shape=None):
    if hidden is not None:
        ptr["hidden"] = hidden
    if shape is not None:
        shapes.append(shape)
        ptr["shape"] = shape
    return 0


e.stub(e.rt(0x678E8), lambda em, *_: 0)
e.stub(e.rt(0x67BF0), lambda em, *_: set_ptr(hidden=True))
e.stub(e.rt(0x673AC), lambda em, *_: set_ptr(hidden=False))
e.stub(e.rt(0x674B0), lambda em, a, *_: set_ptr(shape=a))
e.stub(e.rt(0x67478), lambda em, a, *_: set_ptr(hidden=False, shape=a))
e.stub(e.rt(0x47020), t_start)
e.stub(e.rt(0x47040), t_elapsed)
e.stub(e.rt(0x5CB34), sfx)
e.stub(e.rt(0x532BC), load, 4)
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))
r = random.Random(47)
bad = 0
out = ctypes.create_string_buffer(CAP * SHOT) if PORT else None
ms = (ctypes.c_int32 * CAP)()
pt = (ctypes.c_int32 * CAP)()
fx = (ctypes.c_int32 * (4 * CAP))()
for i in range(ncases):
    sc0 = r.randbytes(SHOT)
    pa0 = r.randbytes(SHOT)
    e.uc.mem_write(screen, sc0)
    e.uc.mem_write(panel, pa0)
    # as FUN_47AE4: the back buffer holds the screen's rows 0..142; the rest is noise
    e.uc.mem_write(e.rt(BACK), sc0[:143 * 320] + r.randbytes(SHOT - 143 * 320))
    acc = bytearray(base_acc)
    for o in range(0, 0xA8, 4):
        acc[o:o + 4] = struct.pack("<i", r.choice([r.randint(-99999, 999999), r.randint(0, 5000), r.randint(-50, 50)]))
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.w32(e.rt(0x229D04), r.choice([0, 100, 141, 142, 199]))
    clock[0] = 0
    frames.clear(), sounds.clear(), shapes.clear()
    ptr.update(hidden=False, shape=0)
    e.call(e.rt(0x47B68), max_steps=40_000_000)
    note(e)
    if shapes != [0xC9, 0]:
        print(f"case {i}: pointer shapes {shapes}")
        bad += 1
    op = bytes(e.uc.mem_read(panel, SHOT))
    if not PORT:
        continue
    pb = ctypes.create_string_buffer(pa0, SHOT)
    n = PORT.ja1f_flipin(ISO, EXE, sc0, pb, bytes(acc), len(acc), out, ms, fx, CAP, pt)
    if n != len(frames):
        print(f"case {i}: steps port {n} original {len(frames)}")
        bad += 1
    for k in range(min(n, len(frames))):
        a = out.raw[k * SHOT:(k + 1) * SHOT]
        if a != frames[k][0]:
            d = [q for q in range(SHOT) if a[q] != frames[k][0][q]]
            print(f"case {i} step {k}: {len(d)} pixels, first ({d[0] % 320},{d[0] // 320}), last row {d[-1] // 320}")
            bad += 1
            break
        if pt[k] != frames[k][2]:
            print(f"case {i} step {k}: pointer port {pt[k]} original {frames[k][2]} (-1 = hidden)")
            bad += 1
            break
        if k + 1 < len(frames):
            shown = frames[k + 1][1] - frames[k][1]
            if abs(shown - ms[k]) > 1:
                print(f"case {i} step {k}: shows port {ms[k]} ms original {shown} ms")
                bad += 1
                break
    psounds = [(k, *fx[k * 4:k * 4 + 4]) for k in range(max(n, 0)) if fx[k * 4] >= 0]
    if psounds != sounds:
        print(f"case {i}: sounds port {psounds} original {sounds}")
        bad += 1
    if pb.raw != op:
        d = [q for q in range(SHOT) if pb.raw[q] != op[q]]
        print(f"case {i}: panel {len(d)} pixels, first ({d[0] % 320},{d[0] // 320})")
        bad += 1
    if bad > 10:
        break
# PlayFlipAnim 0x51264 alone: the other callers (0x43757, 0x43800, 0x4A3E2, 0x443C4; 0x47B68 is above).
FLIPS = ((0, 0x90ABC, 1, 1, "finance open"), (1, 0x90ABC, 3, -1, "finance close"), (2, 0x90B98, 3, -1, "assign leave"),
         (3, 0x90B10, 1, 1, "payroll"))
for i in range(ncases):
    for kind, name, first, step, what in FLIPS:
        sc0 = r.randbytes(SHOT)
        e.uc.mem_write(screen, sc0)
        e.uc.mem_write(panel, r.randbytes(SHOT))
        clock[0] = 0
        frames.clear(), sounds.clear()
        e.call(e.rt(0x51264), e.rt(name), first, 3, step & 0xFFFFFFFF, max_steps=40_000_000)
        note(e)
        if len(frames) != 4 or [x[0] for x in sounds] != [1]:
            print(f"case {i} {what}: original {len(frames)} steps, sounds {sounds}")
            bad += 1
        if not PORT:
            continue
        n = PORT.ja1f_flip_steps(ISO, EXE, sc0, kind, out, ms, fx, CAP)
        if n != len(frames):
            print(f"case {i} {what}: steps port {n} original {len(frames)}")
            bad += 1
        for k in range(min(n, len(frames))):
            if out.raw[k * SHOT:(k + 1) * SHOT] != frames[k][0]:
                print(f"case {i} {what} step {k}: the picture differs")
                bad += 1
                break
            if k + 1 < len(frames) and abs(frames[k + 1][1] - frames[k][1] - ms[k]) > 1:
                print(f"case {i} {what} step {k}: shows port {ms[k]} ms original {frames[k + 1][1] - frames[k][1]} ms")
                bad += 1
                break
        psounds = [(k, *fx[k * 4:k * 4 + 4]) for k in range(max(n, 0)) if fx[k * 4] >= 0]
        if psounds != sounds:
            print(f"case {i} {what}: sounds port {psounds} original {sounds}")
            bad += 1
    if bad > 10:
        break
print(f"flipin_diff: {ncases} cases, {len(frames)} steps, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
