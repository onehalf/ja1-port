"""Differential test: the assignment picture (strategic.md "The assignment picture"), original vs port.

usage: .venv/bin/python validate/diff/assign_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: 0..8 mercs in the sector (team slots 0..8 from the dump with fuzzed duties, training subjects, health,
stats, pay, place 0 / medical items, slot bytes, profiles, nicknames), random face buffers, stat_changed bytes and
accounts, and a noise panel buffer. The original runs DrawAssignMap? 0x478A4(99) in Unicorn on tac60 with the PCC
loader 0x532BC stubbed (MAIN.DAT member by name); the port (port/ja1front ja1f_assign_map) draws on the same noise.
Compared: the 320 x 200 panel buffer and the duty bytes of slots 0..8.
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
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_assign_map.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 5
    PORT.ja1f_assign_map.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SOLDIERS, FACES, ACC, STATCH, PANEL_P = 0xFA958, 0x93428, 0x229FA0, 0x10DD10, 0x93E60
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    w, h, px = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x67E18, 0x67E30, 0x681D0):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
panel = e.r32(e.rt(PANEL_P))
faces = [e.r32(e.rt(FACES) + 4 * k) for k in range(8)]
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
base_acc = bytearray(e.uc.mem_read(e.rt(ACC), 0xA8))


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
    for o in (0x24, 0x28, 0x30, 0x34, 0x38):
        acc[o:o + 4] = struct.pack("<i", r.choice([r.randint(-99999, 999999), r.randint(0, 5000), r.randint(-50, 50)]))
    noise = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
    for k in range(8):
        e.uc.mem_write(faces[k], fc[k])
    e.uc.mem_write(e.rt(STATCH), sc)
    e.uc.mem_write(e.rt(ACC), bytes(acc))
    e.uc.mem_write(panel, noise)
    e.call(e.rt(0x478A4), 99)
    o = bytes(e.uc.mem_read(panel, 64000))
    osol = bytes(e.uc.mem_read(e.rt(SOLDIERS), 9 * 0x200))
    oduty = [osol[k * 0x200 + 0x17E] for k in range(9)]
    if PORT:
        buf = ctypes.create_string_buffer(noise, 64000)
        ps = ctypes.create_string_buffer(bytes(sol), 9 * 0x200)
        rc = PORT.ja1f_assign_map(ISO, EXE, ps, b"".join(fc), sc, bytes(acc), buf)
        p = buf.raw
        pduty = [ps.raw[k * 0x200 + 0x17E] for k in range(9)]
        if rc != 0 or p != o or pduty != oduty:
            nb = next((j for j in range(64000) if p[j] != o[j]), -1)
            print(f"MISMATCH case {i} n {n}: rc {rc}, duties {oduty} vs {pduty}, " +
                  (f"panel @({nb % 320},{nb // 320}) {o[nb]:#x} vs {p[nb]:#x}" if nb >= 0 else "panel equal"))
            bad += 1
            if bad > 10:
                break
print(f"assign_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
