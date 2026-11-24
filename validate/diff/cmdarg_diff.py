#!/usr/bin/env python3
"""Differential test: one command line word, ParseCmdArg 0x1008C, original vs port (world::cmd_word, ja1f_cmd_word).
The real body runs in Unicorn with the real ToUpper and strcmp_; only printf_ and exit_ are stubbed (they log).
Compared: the kind of word (from the flags 0x229E14, 0x229E10, 0x955C6 + 0x955C7 and the exit call), the printed line
and the upper case word. Words: the 3 known words in mixed case, near misses, random bytes 1..255.

usage: .venv/bin/python validate/diff/cmdarg_diff.py [N=1500] [DUMP] [--lib PATH]
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"cmdarg_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(LIB)
P.ja1f_cmd_word.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32]
P.ja1f_cmd_word.restype = ctypes.c_int32
TEXT, NOCRIT, RESET, ARG1, ARG2 = 0xEBAC0, 0x229E14, 0x229E10, 0x955C6, 0x955C7
KNOWN = (b"NOCRITICAL", b"RESET", b"ALLOWSAVE")

e = Emu(dump=dump, fast=True)
rt = e.rt
log = []


def printf(em, *_):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    fmt = em.cstr(em.r32(sp + 4))
    if b"%s" in fmt:
        fmt = fmt.replace(b"%s", em.cstr(em.r32(sp + 8)))
    log.append(("line", bytes(fmt)))


def exit_(em, a, *_):
    log.append(("exit", a))


e.stub(rt(0x75591), printf)
e.stub(rt(0x755B4), exit_)


def word(rng):
    k = rng.randrange(8)
    w = bytearray(rng.choice(KNOWN))
    if k < 4:           # mixed case
        return bytes(c | 0x20 if rng.random() < 0.5 else c for c in w)
    if k == 4:          # one letter off
        w[rng.randrange(len(w))] ^= rng.choice((1, 0x40, 0x80))
        return bytes(c or 1 for c in w)
    if k == 5:          # longer or shorter
        return bytes(w[:rng.randrange(len(w))]) if rng.random() < 0.5 else bytes(w) + b"X"
    return bytes(rng.randrange(1, 256) for _ in range(rng.randrange(0, 40)))


rng = random.Random(1)
bad = 0
kinds = [0] * 4
for n in range(ncases):
    w = word(rng)
    e.uc.mem_write(rt(TEXT), w + b"\0")
    for a in (NOCRIT, RESET):
        e.w32(rt(a), 7)
    e.w8(rt(ARG1), 0)
    e.w8(rt(ARG2), 0)
    log.clear()
    e.call(rt(0x1008C), max_steps=20_000)
    up = e.cstr(rt(TEXT))
    lines = b"".join(x[1] for x in log if x[0] == "line")
    if e.r32(rt(NOCRIT)) == 0:
        kind = 0
    elif e.r32(rt(RESET)) == 1:
        kind = 1
    elif e.r8(rt(ARG1)) == 1 and e.r8(rt(ARG2)) == 1:
        kind = 2
    elif ("exit", 0) in log:
        kind = 3
    else:
        kind = -1
    out = ctypes.create_string_buffer(256)
    got = P.ja1f_cmd_word(w, out, 256)
    kinds[max(kind, 0)] += 1
    if (kind, lines) != (got, out.value):
        bad += 1
        if bad <= 5:
            print(f"MISMATCH case {n}: word {w!r} upper {up!r}: original {kind} {lines!r}, port {got} {out.value!r}")
print(f"cmdarg_diff: {ncases} cases (nocritical {kinds[0]}, reset {kinds[1]}, allowsave {kinds[2]}, unknown {kinds[3]}); {bad} mismatches")
sys.exit(1 if bad else 0)
