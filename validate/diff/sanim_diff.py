"""Differential test: the travel animation rows (panel.md "The travel animation"), original vs port.

usage: .venv/bin/python validate/diff/sanim_diff.py [DUMP] [--port] [--lib SO]
For every row of the table (0x9443A, 18 rows) the original runs FormatSanimName 0x27494 (row pointer) and then
PlaySanim? 0x258D0 once per frame step in Unicorn: the PCX loader 0x532BC reads the member from DAT/SANIM.DAT (rows whose pictures are all
below 42) or DAT/SANIM2.DAT,
TimerElapsedMs 0x47040 always says the time passed, the sound (0x5CB34, 0x5CCF8) and mouse calls are stubbed. Taken:
frame 0 in the box buffer (*0x93E6C + 0xBA0, pitch 228) and the screen block at +0x3EC0 (pitch 320) after each
step: n − 1 steps without loop (ECX 0), and for rows of at most 20 frames 2 (n − 1) + 1 steps with loop (1).
--port: ja1f_sanim gives the frames and the text; step j must show frame j (loop: j mod (n − 1)); the text must be
the 40 bytes at 0x9681F + 40 x byte 0 (but characters 22 and 23). Exits 1 on any mismatch.
"""
import ctypes, os, sys
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
dump = args[0] if args else os.path.join(ROOT, "build", "mem_day3.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_sanim.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 3 + [ctypes.c_void_p] * 2
    PORT.ja1f_sanim.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

ARCS = {k: open(os.path.join(ROOT, "rom", "DAT", n), "rb").read() for k, n in ((1, "SANIM.DAT"), (2, "SANIM2.DAT"))}
ARC = ARCS[2]
ROWS, TEXTS, NROWS = 0x9443A, 0x9681F, 18
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen = e.r32(e.rt(0x95678))
box = e.r32(e.rt(0x93E6C))


def load(em, eax, edx, ebx, ecx):
    global ARC
    try:
        w, h, px, _ = pcx.decode(datarc.member(ARC, em.cstr(ebx).decode("latin-1").upper()))
    except Exception:
        return 0
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x5CB34, 0x5CCF8, 0x47020, 0x67C0C, 0x67BF0, 0x673AC):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x47040), lambda em, *_: 0x7FFFFFFF)
e.stub(e.rt(0x532BC), load, 4)


def block(base, pitch, w, h):
    return b"".join(bytes(e.uc.mem_read(base + y * pitch, w)) for y in range(h))


bad = steps = 0
for r in range(NROWS):
    row = bytes(e.uc.mem_read(e.rt(ROWS + 12 * r), 12))
    n, w, h = row[3], row[4], row[5]
    arc = 1 if row[1] + (n - 1) // max(1, row[2]) <= 41 else 2   # rows 0..5 are SANIM.DAT's (0..41), the rest SANIM2.DAT's
    ARC = ARCS[arc]
    runs = [(0, n - 1)] + ([(1, 2 * (n - 1) + 1)] if n <= 20 else [])
    port = None
    if PORT:
        out, text = ctypes.create_string_buffer(64 * 3000), ctypes.create_string_buffer(40)
        pn = PORT.ja1f_sanim(ISO, EXE, arc, r, -1, out, text)
        orig_text = bytes(e.uc.mem_read(e.rt(TEXTS + 40 * row[0]), 40))
        if pn != n:
            print(f"MISMATCH row {r}: port {pn} frames vs {n}")
            bad += 1
            continue
        # characters 22 and 23 hold the sector number of the dump's last day start (the host writes them)
        if text.raw[:22] + text.raw[24:] != orig_text[:22] + orig_text[24:]:
            print(f"MISMATCH row {r}: text {text.raw!r} vs {orig_text!r}")
            bad += 1
        port = [out.raw[k * w * h:(k + 1) * w * h] for k in range(n)]
    for loop, count in runs:
        e.call(e.rt(0x27494), e.rt(ROWS + 12 * r))
        f0 = block(box + 0xBA0, 0xE4, w, h)
        if port and f0 != port[0]:
            print(f"MISMATCH row {r}: frame 0 in the box")
            bad += 1
        for j in range(count):
            e.call(e.rt(0x258D0), loop)
            got = block(screen + 0x3EC0, 320, w, h)
            steps += 1
            if port:
                k = j % (n - 1) if loop else j
                if got != port[k]:
                    print(f"MISMATCH row {r} loop {loop} step {j}: not frame {k}")
                    bad += 1
                    break
    if bad > 10:
        break
print(f"sanim_diff: {NROWS} rows, {steps} frame steps, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
