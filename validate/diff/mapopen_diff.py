"""Differential test: the opening of the island map, InitStrategicMap 0x43ECC, original vs port.

usage: .venv/bin/python validate/diff/mapopen_diff.py [ncases=40] [DUMP] [--port] [--lib SO]
The original runs whole in Unicorn on tac60: the screen clear, LoadPanelPcc 0x46B1C, DrawStrategicMap 0x41FA0,
PanelTopToScreen? 0x46C1C, the 60 cells and markers, LoadArchivePalette? 0x53300, PaletteCopy? 0x52820. MAIN.DAT comes
through a stubbed file layer (the picture loader 0x532BC, ArcSeek 0x534A4, fgetc_ and fread_); LoadInvDat 0x38190 is
stubbed (the port reads INV.DAT on its own). Per case: a fuzzed sector table. Compared with ja1f_map_open: the
320 x 200 screen and the palette at 0xFA608 and 0xEDA08. Checked on the original side: 0x284F0C = 0 and 0x284F08 = 0x20.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
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
ncases = int(args[0]) if args else 40
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_map_open.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 4
    PORT.ja1f_map_open.restype = ctypes.c_int32
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SECTORS, DAC, PAL_IN = 0x22979C, 0xEDA08, 0xFA608
e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(0x95678))
cur = {"data": b"", "pos": 0}
loaded = []


def load(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    loaded.append(name)
    w, h, px, _ = pcx.decode(datarc.member(MAIN, name))
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def arcseek(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    loaded.append(name)
    cur["data"], cur["pos"] = datarc.member(MAIN, name), 0
    em.w32(em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4), len(cur["data"]))
    return 0


def fseek(em, eax, edx, ebx, ecx):
    cur["pos"] = {0: edx, 1: cur["pos"] + (edx - (1 << 32) if edx >> 31 else edx), 2: len(cur["data"]) + edx}[ebx]
    return 0


def fgetc(em, *_):
    cur["pos"] += 1
    return cur["data"][cur["pos"] - 1]


def fread(em, eax, edx, ebx, ecx):
    b = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(b)
    em.uc.mem_write(eax, b)
    return len(b) // edx


for a_, pop_, f_ in ((0x532BC, 4, load), (0x534A4, 4, arcseek), (0x757B1, 0, fseek), (0x7648C, 0, fgetc), (0x758AD, 0, fread),
                     (0x38190, 0, lambda em, *_: 0)):
    e.stub(e.rt(a_), f_, pop_)
snap = bytes(e.uc.mem_read(0, 16 << 20))
orig_tbl = bytes(e.uc.mem_read(e.rt(SECTORS), 540))


def count(r):
    return r.choice([0, 0, 1, 2, 3, 5, 8, 10, r.randrange(16), r.randrange(256)])


def fuzz(r, i):
    if i == 0:
        return orig_tbl
    t = bytearray()
    for s in range(60):
        rec = [r.randrange(256) if r.random() < .3 else r.randrange(2)] + [count(r) for _ in range(8)]
        t += bytes(rec)
    return bytes(t)


r = random.Random(4394)
fails, drawn = 0, 0
first_names = None
for i in range(ncases):
    tbl = fuzz(r, i)
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(SECTORS), tbl)
    e.uc.mem_write(e.rt(0x229D8C), bytes(60))
    e.w8(e.rt(0x284F0C), r.choice([0, 1, 2]))
    e.w8(e.rt(0x284F08), r.choice([0, 0x20, 0x40]))
    e.uc.mem_write(screen, bytes(r.randrange(256) for _ in range(64)) * 1000)   # the clear must clear it
    loaded.clear()
    e.call(e.rt(0x43ECC), max_steps=200_000_000)
    a = bytes(e.uc.mem_read(screen, 64000))
    if first_names is None:
        first_names = list(loaded)
        print("files", first_names)
    if e.r8(e.rt(0x284F0C)) != 0 or e.r8(e.rt(0x284F08)) != 0x20:
        raise AssertionError(f"case {i}: the original bytes 0x284F0C / 0x284F08 {e.r8(e.rt(0x284F0C))} {e.r8(e.rt(0x284F08))}")
    pal_in = bytes(e.uc.mem_read(e.rt(PAL_IN), 768))
    dac4 = bytes(e.uc.mem_read(e.rt(DAC), 1024))     # PaletteCopy? 0x52820: 3 bytes per colour -> 4-byte entries
    dac = b"".join(dac4[k * 4:k * 4 + 3] for k in range(256))
    drawn += any(a)
    if not PORT:
        continue
    scr, pal = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(768)
    st, mk = ctypes.create_string_buffer(tbl, 540), ctypes.create_string_buffer(bytes(60), 60)
    rc = PORT.ja1f_map_open(ISO, EXE, ctypes.addressof(st), ctypes.addressof(mk), ctypes.addressof(scr), ctypes.addressof(pal))
    if rc:
        raise RuntimeError(f"ja1f_map_open returned {rc}")
    bad = []
    b = scr.raw
    if a != b:
        n = next(j for j in range(64000) if a[j] != b[j])
        rows = sorted({j // 320 for j in range(64000) if a[j] != b[j]})
        bad.append(f"screen @({n % 320},{n // 320}) {a[n]:#x} vs {b[n]:#x}, rows {rows[0]}..{rows[-1]} ({len(rows)})")
    if pal_in != pal.raw:
        bad.append("palette 0xFA608")
    if dac != pal.raw:
        bad.append("palette 0xEDA08")
    if bad:
        fails += 1
        if fails <= 5:
            print(f"MISMATCH case {i}: " + "; ".join(bad))
print(f"mapopen_diff: {ncases} cases, {drawn} drawn, {fails} mismatches")
sys.exit(1 if fails or drawn != ncases else 0)
