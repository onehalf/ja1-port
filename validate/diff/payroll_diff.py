"""Differential test: the payroll screen (strategic.md "The payroll screen (host op 301)"), original vs port.

usage: .venv/bin/python validate/diff/payroll_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: 0..8 mercs in the sector (team slots 0..8 from the dump, fuzzed as in assign_diff), random face buffers,
stat_changed bytes, accounts and prev_balance, a noise screen and a noise panel buffer. The original runs
PayrollScreen? 0x44380 in Unicorn on tac60 with the PCC loader 0x532BC stubbed (MAIN.DAT member by name), the
timers answering 1000 ms, the mouse routines stubbed and PlaySfxSlot 0x5CB34 recorded; the port (port/ja1front
ja1f_payroll) runs on the same noise. Compared: the 320 x 200 screen and panel buffer at the end, the sounds.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402
from assign_fuzz import fuzz_soldier  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_payroll.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 4 + [ctypes.c_int32] + \
        [ctypes.c_void_p] * 3
    PORT.ja1f_payroll.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SOLDIERS, FACES, ACC, STATCH, PANEL_P, SCREEN_P = 0xFA958, 0x93428, 0x229FA0, 0x10DD10, 0x93E60, 0x95678
PREV, OTHER_P = 0x229AC0, 0x93E5C
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    return _pics[n]


e = Emu(dump=dump, fast=True)
other = e.r32(e.rt(OTHER_P))
loads, sounds = [], []


def load(em, eax, edx, ebx, ecx):
    loads.append(e.cstr(ebx).decode("latin-1").upper())
    if ecx == other:              # the second BASSMAP.PCC load (nothing reads it here)
        return 0
    w, h, px = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x674B0, 0x47020):
    e.stub(e.rt(a), lambda em, *_: 0)
limits = []                # SetMouseLimit? 0x44445: the window sets the same limit at host op 301
e.stub(e.rt(0x682AC), lambda em, a, d, b, c: limits.append((a, d, b, c)) or 0)
e.stub(e.rt(0x47040), lambda em, *_: 1000)
e.stub(e.rt(0x5CB34), lambda em, a, d, b, c: sounds.append((a, d, b, c)) or 0)
e.stub(e.rt(0x532BC), load, 4)
panel = e.r32(e.rt(PANEL_P))
screen = e.r32(e.rt(SCREEN_P))
faces = [e.r32(e.rt(FACES) + 4 * k) for k in range(8)]
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))


def money(r):
    return r.choice([r.randint(-99999, 999999), r.randint(0, 5000), r.randint(-50, 50), 0])


r = random.Random(1)
bad = 0
for i in range(ncases):
    n = r.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 8])
    sol = bytearray(base_sol)
    for k in range(9):
        s = bytearray(sol[k * 0x200:(k + 1) * 0x200])
        fuzz_soldier(r, s, k, n)
        sol[k * 0x200:(k + 1) * 0x200] = s
    fc = [bytes(r.randrange(256) for _ in range(1024)) for _ in range(8)]
    sc = bytes(1 if r.random() < 0.2 else 0 for _ in range(640))
    acc = bytearray(base_acc)
    for o in (0x10, 0x1C, 0x20, 0x24, 0x2C, 0x38, 0x60, 0x64):
        acc[o:o + 4] = struct.pack("<i", r.randint(0, 999) if o == 0x1C and r.random() < 0.7 else money(r))
    prev = money(r)
    noise_s = bytes(r.randrange(256) for _ in range(64000))
    noise_p = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
    for k in range(8):
        e.uc.mem_write(faces[k], fc[k])
    e.uc.mem_write(e.rt(STATCH), sc)
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.uc.mem_write(e.rt(PREV), struct.pack("<i", prev))
    e.uc.mem_write(panel, noise_p)
    e.uc.mem_write(screen, noise_s)
    loads.clear()
    sounds.clear()
    limits.clear()
    e.call(e.rt(0x44380))
    o_s = bytes(e.uc.mem_read(screen, 64000))
    o_p = bytes(e.uc.mem_read(panel, 64000))
    o_snd = list(sounds)
    if i == 0:
        print("loads", loads, "sounds", o_snd)
    if PORT:
        sbuf = ctypes.create_string_buffer(noise_s, 64000)
        pbuf = ctypes.create_string_buffer(noise_p, 64000)
        snd = (ctypes.c_int32 * 8)()
        rc = PORT.ja1f_payroll(ISO, EXE, bytes(sol), b"".join(fc), sc, bytes(acc), prev, sbuf, pbuf, snd)
        p_snd = [tuple(snd[4 * j:4 * j + 4]) for j in range(2)]
        op_limit = (ctypes.c_int32 * 4)()
        PORT.ja1f_mouse_op_limit(301, op_limit)
        if rc < 0 or sbuf.raw != o_s or pbuf.raw != o_p or p_snd != o_snd or limits != [tuple(op_limit)]:
            msg = []
            if limits != [tuple(op_limit)]:
                msg.append(f"limit {limits} vs {tuple(op_limit)}")
            for nm, a, b in (("screen", o_s, sbuf.raw), ("panel", o_p, pbuf.raw)):
                nb = next((j for j in range(64000) if a[j] != b[j]), -1)
                if nb >= 0:
                    msg.append(f"{nm} @({nb % 320},{nb // 320}) {a[nb]:#x} vs {b[nb]:#x}")
            if p_snd != o_snd:
                msg.append(f"sounds {o_snd} vs {p_snd}")
            print(f"MISMATCH case {i} n {n}: rc {rc}, " + "; ".join(msg))
            bad += 1
            if bad > 10:
                break
print(f"payroll_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
