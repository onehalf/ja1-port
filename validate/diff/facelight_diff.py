#!/usr/bin/env python3
"""Differential test: the light frame of a merc face, 0x46A4C, original vs port.

usage: .venv/bin/python validate/diff/facelight_diff.py [ncases=300] [DUMP] [--lib SO]
Part 1, one call: a slot 0..7, frame picture 5 or 0xC, a noise face buffer, a noise remap table 0. The original runs in
Unicorn with its real BlitRemap 0x522E4. The port: panel::face_light (ja1f_face_light). Compared: the 1024 face bytes on
both sides and, on the original side, table byte 0x1B and the other buffers (the call writes only this slot's face).
Part 2, the state after a load: the face buffers that the original holds in a dump, against the buffers of a new port
Screen on the same state (the quick save that the original writes on the dump, as tacredraw_diff). The compare covers
the frame rows 0..27 and the face. Exits 1 on mismatch.
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
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"facelight_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
PORT.ja1f_face_light.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int]
PORT.ja1f_screen_faces.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_size_t,
                                   ctypes.c_void_p]
P_FACES, P_TABLE0 = 0x93428, 0xE8C48
e = Emu(dump=dump, fast=True)
rt = e.rt
table = e.r32(rt(P_TABLE0))
real_table = bytes(e.uc.mem_read(table, 256))
faces = [e.r32(rt(P_FACES + 4 * k)) for k in range(8)]
r = random.Random(4646)
bad = 0
kinds = {5: 0, 0xC: 0}
for c in range(ncases):
    slot, el = c % 8, 5 if c % 2 == 0 else 0xC
    face = bytes(r.randrange(256) for _ in range(1024))
    tab = real_table if c % 4 < 2 else bytes(r.randrange(256) for _ in range(256))
    e.uc.mem_write(table, tab)
    e.uc.mem_write(faces[slot], face)
    others = [bytes(e.uc.mem_read(faces[k], 1024)) for k in range(8) if k != slot]
    e.call(rt(0x46A4C), slot, el, max_steps=200_000)
    orig = bytes(e.uc.mem_read(faces[slot], 1024))
    port = (ctypes.c_uint8 * 1024)(*face)
    rc = PORT.ja1f_face_light(ISO, EXE, tab, port, el)
    why = []
    if rc != 0:
        why.append(f"rc {rc}")
    if orig != bytes(port):
        n = sum(a != b for a, b in zip(orig, bytes(port)))
        why.append(f"{n} face bytes")
    if bytes(e.uc.mem_read(table, 256)) != tab:
        why.append("table changed in the original")
    if others != [bytes(e.uc.mem_read(faces[k], 1024)) for k in range(8) if k != slot]:
        why.append("other face changed in the original")
    if orig == face:
        why.append("no change at all")
    kinds[el] += 1
    if why:
        bad += 1
        if bad <= 5:
            print(f"case {c}: slot {slot} element {el:#x}: {', '.join(why)}")

# Part 2: the buffers that the original holds after its load.
src = open(os.path.join(HERE, "tacredraw_diff.py")).read()
ns = {"Emu": Emu, "FIXED": 85402, "TMP": 18080, "SECTORS": 0x222B74, "SEC_ROW": 177, "SEC_FLAG": 169}
exec(src[src.index("def quick_payload"):src.index("def s32")], ns)
q = ns["quick_payload"](dump)
DAY3 = os.path.join(ROOT, *('build', 'DAY_day3.SAV')).encode()
e = Emu(dump=dump, fast=True)   # part 1 wrote noise into the buffers
rt = e.rt
out = (ctypes.c_uint8 * 8192)()
rc = PORT.ja1f_screen_faces(ISO, EXE, DAY3, q, len(q), out)
part2 = 0
if rc != 0:
    print(f"part 2: rc {rc}")
    part2 = 1
else:
    pb = bytes(out)
    sel = e.r8(rt(0x284F11))
    for k in range(8):
        o = bytes(e.uc.mem_read(faces[k], 1024))
        p = pb[k * 1024:(k + 1) * 1024]
        n = sum(o[i] != p[i] for i in range(0x380))
        alive = e.r8(rt(0xFA958 + k * 512 + 0x182))
        # A slot with no merc: only the frame counts (the original may keep old bytes in the middle).
        frame = [i for i in range(0x380) if i // 32 in (0, 1, 27) or i % 32 in (0, 1, 30, 31)]
        nf = sum(o[i] != p[i] for i in frame)
        if nf or (alive and n):
            part2 += 1
            print(f"part 2: slot {k} (selected {sel}): frame {nf} bytes, all {n} bytes")
print(f"facelight_diff: {ncases} calls ({kinds[5]} off, {kinds[0xC]} on): {bad} mismatches; "
      f"load state: {part2} slots differ")
sys.exit(1 if bad or part2 else 0)
