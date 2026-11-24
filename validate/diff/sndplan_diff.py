#!/usr/bin/env python3
"""Differential test: the tactical sound bank's load list, LoadOverheadSounds 0x31B44 with 45 samples, original vs port.

usage: .venv/bin/python validate/diff/sndplan_diff.py [ncases=300] [--lib SO]
The original runs 0x31B44 -> 0x31AF8 (EAX 1) -> 0x31A70 per team soldier -> LoadEnemySounds 0x31A10. Fuzzed: the
soldier bytes +0x179 (profile), +0x138, +0x182, +0x188 of slots 0..8 (slot 8 must never load). Stubs that log: FormatFxPath
0x24C90 (archive), LoadFxSound 0x24CB4 (name, slot), PlaySndsVoc 0x24E44 (profile, slot, name). Port: sound::tactical_plan.
Compared: the whole list in its order. On the original side only: the last voice of a soldier closes the archive (stack
flag 1). Not compared: sample bytes (gate_sound plays them). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
PORT = ctypes.CDLL(LIB)
PORT.ja1f_snd_plan.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t]
SOLD, REC = 0xFA958, 0x200

e = Emu(fast=True)
e.stub_sprintf()
log, fx = [], [None]


def fxpath(em, eax, *_):
    fx[0] = em.cstr(eax).decode("latin-1").upper()
    return 0


def fxload(em, eax, edx, *_):
    s = edx & 0xFFFF
    log.append(f"FX {fx[0]} {em.cstr(eax).decode('latin-1').upper()} {s}")
    return 0


flags = []


def voc(em, eax, edx, ebx, ecx):
    last = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    flags.append(last)
    log.append(f"SNDS {eax & 0xFF} {em.cstr(ebx).decode('latin-1').upper()} {edx & 0xFFFF}")
    return 0


e.stub(e.rt(0x24C90), fxpath)
e.stub(e.rt(0x24CB4), fxload)
e.stub(e.rt(0x24E44), voc, 4)
e.stub(e.rt(0x5C144), lambda em, *_: 0)

rng = random.Random(0x31A70)
bad = voices = 0
for case in range(ncases):
    recs = bytearray(REC * 9)
    for k in range(9):
        recs[k * REC + 0x179] = rng.randrange(256)
        recs[k * REC + 0x182] = rng.choice([0, 0, 1, 7, 0x80])
        recs[k * REC + 0x138] = rng.choice([0, 0, 1, 0xFF])
        recs[k * REC + 0x188] = rng.choice([0, 0, 1, 0x40])
    e.uc.mem_write(e.rt(SOLD), bytes(recs))
    log.clear()
    flags.clear()
    e.call(e.rt(0x31B44), 45)
    buf = ctypes.create_string_buffer(8192)
    n = PORT.ja1f_snd_plan(bytes(recs), len(recs), buf, 8192)
    got = buf.raw[:max(n, 0)].decode("latin-1").splitlines()
    voices += sum(a.startswith("SNDS") for a in log)
    if got != log:
        bad += 1
        d = next((i for i, (a, b) in enumerate(zip(log, got)) if a != b), min(len(log), len(got)))
        print(f"case {case}: {len(got)} port lines, {len(log)} original, first at {d}: "
              f"port {got[d:d + 1]} original {log[d:d + 1]}")
    if flags != [0, 0, 0, 1] * (len(flags) // 4):
        bad += 1
        print(f"case {case}: archive flags {flags}")
print(f"sndplan_diff: {ncases} cases, {voices} voices, {bad} mismatches")
sys.exit(1 if bad else 0)
