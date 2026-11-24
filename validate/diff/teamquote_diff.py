#!/usr/bin/env python3
"""Differential test: the team quote, TeamQuote? 0x61688, original vs port.

usage: .venv/bin/python validate/diff/teamquote_diff.py [ncases=3000] [DUMP] [--lib SO]
A case: the bytes +0x182, +0x138, +0x183, +0x188 and the tile +0x114 of the 8 soldier records, the range, the quote, a seed. The
original runs in Unicorn with the real Random and the real TileDistance; ShowMercMessage 0x299B8 and the redraw 0x1CB78 are
stubbed and logged. The port: sighting::team_quote (ja1f_team_quote).
Compared: which mercs speak and in which order, the final random seed.
On the original side only: each merc gets the quote, near 0x40, the empty text and flag 1; a redraw follows each merc.
Not compared: merc_message itself (balloon_diff). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"teamquote_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_team_quote.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p]
e = Emu(dump=dump, fast=True)
rt = e.rt
SEED, TABLE, DD4 = 0x9BBBC, 0x100958, 0x92DD4
recs = [e.r32(rt(TABLE) + 4 * k) for k in range(8)]
st = {}


def say(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    st["log"].append(("say", recs.index(a & 0xFFFFFFFF), (d ^ 0x80000000) - 0x80000000, b, e.cstr(c), e.r32(sp + 4)))
    return 0


e.stub(rt(0x299B8), say, 4)
e.stub(rt(0x1CB78), lambda em, *_: st["log"].append(("redraw",)) or 0)
r = random.Random(1107)
bad = spoke = none = 0
counts = [0] * 9


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


for c in range(ncases):
    q = r.choice((0x1C, 0x7F, -1, -128, r.randrange(-128, 128)))
    rng = r.choice((0, 3, 0xE, 0xE, 40, 200, 10000))
    seed = r.getrandbits(32)
    near = r.random() < 0.7
    img = bytearray(4096)
    for k in range(8):
        f = bytearray(e.uc.mem_read(recs[k], 512))
        f[0x182] = r.choice((0, 1, 1, 1, 1, r.randrange(256)))
        f[0x138] = r.choice((0, 1, 1, 1, 1, r.randrange(256)))
        f[0x183] = r.choice((14, 15, 16, 60, 0, 255, 128, r.randrange(256)))
        f[0x188] = r.choice((0, 0, 0, 0, 1, r.randrange(256)))
        t = (0x15B + r.randrange(-1000, 1000)) & 0x7FFF if near else r.randrange(0x10000)
        if near and r.random() < 0.5:
            t = (0x15B + r.randrange(-3, 4) * 56 + r.randrange(-3, 4)) & 0xFFFF
        f[0x114:0x116] = t.to_bytes(2, "little")
        e.uc.mem_write(recs[k], bytes(f))
        img[k * 512:(k + 1) * 512] = f
    e.w32(rt(SEED), seed)
    st["log"] = []
    try:
        e.call(rt(0x61688), q & 0xFF, rng, 0, max_steps=2_000_000)
    except EmuStop as x:
        report(c, f"original stopped: {x}")
        continue
    says = [x for x in st["log"] if x[0] == "say"]
    if [x[0] for x in st["log"]] != ["say", "redraw"] * len(says):
        report(c, f"order {st['log']}")
    for x in says:
        if x[2:] != (q, 0x40, b"", 1):
            report(c, f"the original's arguments {x}")
    sd = ctypes.c_uint32(seed)
    o = ctypes.create_string_buffer(8)
    n = PORT.ja1f_team_quote(bytes(img), rng, ctypes.byref(sd), o)
    if n < 0:
        report(c, f"port returned {n}")
        continue
    a, b = [x[1] for x in says], list(o.raw[:n])
    if a != b:
        report(c, f"mercs: original {a} port {b}")
    if e.r32(rt(SEED)) != sd.value:
        report(c, f"seed: original {e.r32(rt(SEED)):#x} port {sd.value:#x}")
    counts[len(a)] += 1
print(f"teamquote_diff: {ncases} cases, mercs per case {counts}, {bad} mismatches")
sys.exit(1 if bad or sum(counts[1:]) < ncases // 4 else 0)
