"""Differential test: loading a merc face into a slot's face buffer (panel.md), original vs port.

usage: .venv/bin/python validate/diff/loadface_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: a slot 0..7, a face number 0..65 (the 11 x 6 grid of FACES.PCX), the slot soldier's hp (+0x183, signed:
alive above 0), the faces-loaded flag 0x9344B and a noise face buffer. The original LoadMercFace 0x286C4 runs in
Unicorn on tac60: the PCC loader 0x532BC loads MAIN.DAT members by name (FACES.PCX into the panel buffer *0x93E60
once, DEADFACE.PCC straight into the face buffer at (2, 2) with pitch 32), OpenCDFile 0x38304 / fseek_ / fread_ /
fclose_ read DAT\\JAGGED.DAT through common.game_read. Result = the 1024 face bytes and the soldier's eye row +0x1AA.
--port: ja1f_face (kind 0 alive, 1 dead) fills the same noise buffer; the eye byte is compared when alive.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402
from common import game_read         # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_face.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 2 + [ctypes.c_void_p] * 2
    PORT.ja1f_face.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
LOAD_FACE, FACE_BUFS, SOLDIERS, LOADED, DATFILE = 0x286C4, 0x93428, 0xFA958, 0x9344B, 0x92F54
FAKE_FP = 0x7777
e = Emu(dump=dump, fast=True)
loads, cur, files = [], {}, {}


def load(em, eax, edx, ebx, ecx):
    name = e.cstr(ebx).decode("latin-1").upper()
    loads.append(name)
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    w, h, px, _ = pcx.decode(datarc.member(MAIN, name))
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def open_cd(em, eax, edx, ebx, ecx):
    name = e.cstr(eax).decode("latin-1").upper()
    if name not in files:
        files[name] = game_read(name)
    cur["data"], cur["pos"] = files[name], 0
    e.w32(e.rt(DATFILE), FAKE_FP)
    return FAKE_FP


def fseek(em, eax, edx, ebx, ecx):
    assert eax == FAKE_FP and ebx == 0
    cur["pos"] = edx
    return 0


def fread(em, eax, edx, ebx, ecx):
    assert ecx == FAKE_FP
    d = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(d)
    e.uc.mem_write(eax, d)
    return len(d) // edx if edx else 0


e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x38304), open_cd)
e.stub(e.rt(0x757B1), fseek)
e.stub(e.rt(0x758AD), fread)
e.stub(e.rt(0x75A77), lambda em, *_: 0)
r = random.Random(11)
bad, kinds, seen = 0, [0, 0], set()
for i in range(ncases):
    slot, n = r.randrange(8), r.randrange(66)
    hp = r.choice([r.randint(1, 127), r.randint(-128, 0), 1, 0, -1, 127, -128])
    # flag 1 only once a case loaded FACES.PCX into the panel buffer (the dump's panel buffer holds other pictures)
    flag = r.choice([0, 1]) if "FACES.PCX" in seen else 0
    noise = bytes(r.randrange(256) for _ in range(1024))
    soldier = e.rt(SOLDIERS) + slot * 512
    buf = e.r32(e.rt(FACE_BUFS) + slot * 4)
    e.w8(soldier + 0x183, hp & 0xFF)
    eye0 = r.randrange(256)
    e.w8(soldier + 0x1AA, eye0)
    e.w8(e.rt(LOADED), flag)
    e.uc.mem_write(buf, noise)
    loads.clear()
    e.call(e.rt(LOAD_FACE), slot, n)
    face, eye = bytes(e.uc.mem_read(buf, 1024)), e.r8(soldier + 0x1AA)
    alive = hp > 0
    kinds[0 if alive else 1] += 1
    seen.update(loads)
    want = ["FACES.PCX"] if alive and not flag else (["DEADFACE.PCC"] if not alive else [])
    if loads != want:
        print(f"case {i}: loads {loads} want {want}")
        bad += 1
    if PORT is None:
        continue
    out = ctypes.create_string_buffer(noise, 1024)
    peye = ctypes.c_uint8(eye0)
    rc = PORT.ja1f_face(ISO, EXE, n, 0 if alive else 1, out, ctypes.byref(peye))
    if rc != 0 or out.raw != face or peye.value != eye:
        diff = [k for k in range(1024) if out.raw[k] != face[k]]
        print(f"case {i}: slot {slot} face {n} hp {hp} rc {rc} eye orig {eye} port {peye.value}, "
              f"{len(diff)} bytes differ, first {diff[:6]}")
        bad += 1
        if bad > 10:
            break
print(f"loadface_diff{' --port' if PORT else ''}: {ncases} cases (alive {kinds[0]}, dead {kinds[1]}), {bad} mismatches")
sys.exit(1 if bad else 0)
