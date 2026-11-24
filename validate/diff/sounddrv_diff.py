#!/usr/bin/env python3
"""Differential test of the sound start, InitSound 0x5CED4 + InitAil 0x5C1DC, original vs startup::SoundSetup and
World::apply_sound_cfg. The original runs in Unicorn with the AIL, DPMI and file calls stubbed; the file reads give a fuzzed
SOUND.CFG (and the real GOG file). Compared: the driver bytes 0x96399 / 0x9639A, the options 0, 1 and 5 (0x1163B8, 0x1163B9,
0x1163BD). The GOG file also compares the start volumes 0x963A0 / 0x963A8 with sound::volumes(), and the original side checks
that a missing file ends the program with the SETSOUND text.

Original side only (the port has its own mixer, FM synth and no Miles driver): driver file names, the sample handles, the music
forms of device 8, 9, 0xB (bytes 0x95DC0, 0x9639B, volume 80 for device 9).

usage: .venv/bin/python validate/diff/sounddrv_diff.py [ncases=400] [DUMP]
Exits 1 on a mismatch or when a road is not reached.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
ncases = int(argv[0]) if argv else 400
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"sounddrv_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
ISO, EXE = (os.path.join(ROOT, *q).encode() for q in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
GOG = open(os.path.join(ROOT, "build", "game113b", "SOUND.CFG"), "rb").read()
e = Emu(dump=dump, fast=True)
rt = e.rt
AIL = {}
BAD = 0
log = []


class Quit(Exception):
    pass


def named(name, res=0):
    def f(em, *_):
        log.append(name)
        return res
    return f


cfg = [GOG]
reads = [0]


def fread(em, buf, size, n, *_):
    k = (size & 0xFFFF) * (n & 0xFFFF)
    em.uc.mem_write(buf & 0xFFFFFFFF, cfg[0][reads[0]:reads[0] + k])
    reads[0] += k
    return n & 0xFFFF


# Every call target of the two routines: find the AIL stubs from the listed labels.
LABELS = {}
for line in open(os.path.join(ROOT, "labels.csv"), errors="replace"):
    p = line.split(",", 2)
    if len(p) >= 2 and p[1].startswith("AIL_") and p[0].startswith("0x"):
        LABELS[p[1].rstrip("?")] = int(p[0], 16)
for name, res in (("AIL_startup", 0), ("AIL_set_GTL_filename_prefix", 0), ("AIL_install_MDI_driver_file", 0x1000),
                  ("AIL_allocate_sequence_handle", 0x2000), ("AIL_set_preference", 0), ("AIL_install_DIG_driver_file", 0x3000),
                  ("AIL_allocate_sample_handle", 0x4000), ("AIL_shutdown", 0)):
    e.stub(rt(LABELS[name]), named(name, res))
e.stub(rt(0x637C4), named("lock"))
e.stub(rt(0x63834), named("lockcall"))
exists = [1]
e.stub(rt(0x4707C), lambda em, *_: exists[0])
e.stub(rt(0x524E0), named("video"))
e.stub(rt(0x75591), named("printf"))


def do_exit(em, *_):
    log.append("exit")
    em.uc.emu_stop()
    return 0


e.stub(rt(0x755B4), do_exit)
e.stub(rt(0x38304), named("open", 1))
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), named("close"))
r = random.Random(0x5CED4)
OPTS = (0x1163B8, 0x1163B9, 0x1163BD)
bad = 0
seen = {"no sound": 0, "no music": 0, "both": 0, "subtitles": 0}


def one(c, case):
    global bad
    opts = [r.choice((0, 1)) for _ in OPTS]
    for a, v in zip(OPTS, opts):
        e.w8(rt(a), v)
    cfg[0] = c
    reads[0] = 0
    log.clear()
    e.uc.mem_write(rt(0x96398), b"\0" * 4)
    e.call(rt(0x5CED4))
    o = (e.r8(rt(0x96399)), e.r8(rt(0x9639A)), e.r8(rt(OPTS[0])), e.r8(rt(OPTS[1])), e.r8(rt(OPTS[2])))
    out = (ctypes.c_int * 7)()
    if P.ja1f_sound_setup(ISO, EXE, c, bytes(opts), out):
        bad += 1
        print("port error")
        return out
    if tuple(out[:5]) != o:
        bad += 1
        if bad <= 10:
            print(f"MISMATCH case {case}: devices {struct.unpack_from('<h', c, 0x18)[0]} {struct.unpack_from('<h', c, 0x40)[0]}: original {o} port {tuple(out[:5])}")
    seen["no sound" if not o[0] else "no music" if not o[1] else "both"] += 1
    seen["subtitles"] += c[80] != 0
    return out


# The real file: the start volumes too.
e.w32(rt(0x963A0), 0x7777)
e.w32(rt(0x963A8), 0x7777)
out = one(GOG, "GOG")
vol = (e.r32(rt(0x963A0)), e.r32(rt(0x963A8)))
if vol != (out[5], out[6]) or e.r8(rt(0x95DC0)) or e.r8(rt(0x9639B)):
    bad += 1
    print(f"MISMATCH GOG file: original volumes {vol}, form bytes {e.r8(rt(0x95DC0))} {e.r8(rt(0x9639B))}; port {out[5]}, {out[6]}")
# A fuzzed file.
for case in range(ncases):
    c = bytearray(GOG)
    struct.pack_into("<h", c, 0x18, r.choice((-1, 0, 1, 2, 3, 0x7FFF, -32768)))
    struct.pack_into("<h", c, 0x40, r.choice((-1, 0, 1, 2, 3, 8, 9, 0xB, 0x7FFF, -32768)))
    c[80] = r.choice((0, 1, 2, 0xFF))
    e.w8(rt(0x95DC0), 0)
    one(bytes(c), case)
# No file: the text and the end of the program.
exists[0] = 0
log.clear()
try:
    e.call(rt(0x5CED4))
except Exception:
    pass
if "printf" not in log or "exit" not in log:
    bad += 1
    print(f"no-file road: {log}")
for k, v in seen.items():
    if not v:
        bad += 1
        print(f"sounddrv_diff: road {k} not reached")
print(f"sounddrv_diff: {ncases + 1} cases {seen}, {bad} mismatches")
sys.exit(1 if bad else 0)
