#!/usr/bin/env python3
"""Differential test of the program main, main_ 0x10150, original vs world::main_words_text (first_word, cmd_word, the notice).
The real body and the real ParseCmdArg run in Unicorn; every callee of the start chain is stubbed and logged. Fuzzed: the command
line, 0 to 5 words. Compared: the words it reads (only the first 3), the lines it prints, the end at an unknown word, the
ALLOWSAVE notice.

Original side only, checked against a fixed list: the order of the 21 calls of the start chain (every callee has its own test or
its own mark), Int24Install only without NOCRITICAL, the key driver's argument (1 after RESET), NewGameInit's argument 0, byte
0x22A043 = 0, GetKey only with the notice. The port has no text screen, so the two banner lines and the key wait of the notice
stay out of it (world_load.md "The program option allow save"): the banner is only compared with its fixed text.

usage: .venv/bin/python validate/diff/main_diff.py [ncases=1500] [DUMP]
Exits 1 on a mismatch or when a road is not reached.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
ncases = int(argv[0]) if argv else 1500
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"main_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
e = Emu(dump=dump, fast=True)
rt = e.rt
calls, lines = [], []
ended = [False]
BANNER = [b"\nLoading Jagged Alliance Release  v1.13  Sep 05-95\n", b"Copyright (c) 1995 Sir-tech Software, Inc.\n\n"]


def printf(em, *_):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    fmt = bytes(em.cstr(em.r32(sp + 4)))
    if b"%s" in fmt:
        fmt = fmt.replace(b"%s", bytes(em.cstr(em.r32(sp + 8))))
    lines.append(fmt)


def exit_(em, *_):
    # The stub returns, so the body goes on: everything after this point is ignored.
    if not ended[0]:
        ended[0] = (len(calls), len(lines))


def named(name, arg=False):
    def f(em, eax, *_):
        calls.append((name, eax & 0xFF) if arg else name)
        return 0
    return f


e.stub(rt(0x75591), printf)
e.stub(rt(0x755B4), exit_)
for a, name, arg in ((0x374B0, "lock buffers", 0), (0x68784, "int24", 0), (0x75619, "atexit", 0), (0x373F0, "cd", 0),
                     (0x5C0B8, "lock callback", 0), (0x66EF8, "lock globals", 0), (0x56188, "key driver", 1), (0x560F4, "getkey", 0),
                     (0x53564, "srand", 0), (0x52C10, "dissolve", 0), (0x50440, "disk", 0), (0x38038, "temp", 0),
                     (0x377B0, "world", 0), (0x37A0C, "new game", 1), (0x67294, "cpu", 0), (0x5CED4, "sound", 0),
                     (0x36F90, "main dat", 0), (0x31B8C, "main sound", 0), (0x4FFEC, "game main", 0)):
    e.stub(rt(a), named(name, arg))
SCRATCH = rt(0x339000)
KNOWN = (b"NOCRITICAL", b"RESET", b"ALLOWSAVE", b"allowsave", b"Reset", b"nocritical")
r = random.Random(0x10150)
bad = 0
seen = {"ended": 0, "notice": 0, "past three": 0, "plain": 0}


def fail(case, text):
    global bad
    bad += 1
    if bad <= 10:
        print(f"MISMATCH case {case}: {text}")


for case in range(ncases):
    n = r.randrange(6)
    words = [r.choice(KNOWN) if r.random() < 0.9 else r.choice((b"X", b"RESETS", b"ALLOW", b"bogus")) for _ in range(n)]
    # argv: the program name, then the words.
    p, ptrs = SCRATCH + 64, []
    for w in [b"JA"] + words:
        e.uc.mem_write(p, w + b"\0")
        ptrs.append(p)
        p += len(w) + 1
    for k, q in enumerate(ptrs):
        e.w32(SCRATCH + 4 * k, q)
    e.w8(rt(0x955C6), 0)
    e.w8(rt(0x955C7), 0)
    e.w8(rt(0x22A043), 0x55)
    calls.clear()
    lines.clear()
    ended[0] = False
    e.call(rt(0x10150), n + 1, SCRATCH)
    # The port.
    buf = ctypes.create_string_buffer(4096)
    ln = ctypes.c_int(0)
    end = P.ja1f_main_words(b"".join(w + b"\0" for w in words) + b"\0", n, buf, 4096, ctypes.byref(ln))
    port = buf.raw[:ln.value]
    orig = lines[:ended[0][1]] if ended[0] else lines[:]
    if not ended[0]:
        ban = [k for k, t in enumerate(orig) if b"Loading Jagged" in t or b"Copyright" in t]
        if [orig[k] for k in ban] != BANNER:
            fail(case, f"banner {[orig[k] for k in ban]}")
        orig = [t for k, t in enumerate(orig) if k not in ban]
    o = b"".join(orig)
    if o != port or int(bool(ended[0])) != end:
        fail(case, f"words {words}: original {o[:60]!r} end {ended[0]}; port {port[:60]!r} end {end}")
    first = [w.upper() for w in words[:3]]
    seen["ended" if ended[0] else "notice" if b"ALLOWSAVE" in first else "plain"] += 1
    seen["past three"] += n > 3 and not ended[0]
    if ended[0]:
        if ended[0][0]:
            fail(case, f"calls before the end: {calls[:ended[0][0]]}")
        continue
    # Original side: the chain.
    crit = b"NOCRITICAL" not in first
    want = ["lock buffers"] + (["int24", "atexit"] if crit else []) + ["cd", "lock callback", "lock globals", ("key driver", int(b"RESET" in first))]
    want += (["getkey"] if b"ALLOWSAVE" in first else []) + ["srand", "dissolve", "disk", "temp", "world", ("new game", 0), "cpu", "sound",
                                                             "main dat", "main sound", "game main"]
    if calls != want:
        fail(case, f"chain {calls}")
    if e.r8(rt(0x22A043)) != 0:
        fail(case, f"byte 0x22A043 = {e.r8(rt(0x22A043))}")
for k, v in seen.items():
    if not v:
        fail("end", f"road {k} not reached")
print(f"main_diff: {ncases} cases {seen}, {bad} mismatches")
sys.exit(1 if bad else 0)
