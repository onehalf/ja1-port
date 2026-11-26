#!/usr/bin/env python3
"""Differential test: the text of an A.I.M. refusal box, AimScreen 0x2011C sites 0x20583..0x2078D, original vs port.

usage: .venv/bin/python validate/diff/aimquote_diff.py [ncases=500] [DUMP] [--lib SO] [--fan IDS]
--fan IDS (#70): the original side runs the fan patch 1.15m code (Emu.put_115m), the port runs with the fan fixes IDS
on (`all` or `F04`); without it the fixes are off and the original is 1.13b.
The original runs in Unicorn from the call of MercRefusalReason 0x20583 (stub: returns the reason of the case) with the real
PlayQuote 0x24F08, EncodeStringPlus1 0x35810 and StrReplace 0x252B0; the file calls read the real AIMQUOTE.EDT. Stubs: the
voice file calls, strstr_ (faults in Unicorn), the box 0x28FB8 (takes the text, then ends the run).
The port: aim::quote_text (ja1f_aim_quote_text). Compared, with the states kept from case to case as in the game: the box text
as the player reads it, the last quote 0x22A1EA, g_textBuf, the shared buffer 0x2299BC, the StrReplace target 0x229B1C.
Records 362, 580 and 678 hold MEMBER past place 127 (issue #2 row 44); the run counts them and fails if it sees none.
Not compared: pixels and the box point (aimtalk_diff), reason 8 with a hire (aim_diff). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, RET_SENTINEL  # noqa: E402
from common import game_read  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = None
if "--fan" in argv:
    v = argv[argv.index("--fan") + 1]
    FAN = (1 << 128) - 1 if v == "all" else sum(1 << (int(x.strip()[1:]) - 1) for x in v.split(","))
ncases = int(args[0]) if args else 500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"aimquote_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_aim_quote_text.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 3 + [ctypes.c_int] * 2 + [ctypes.c_void_p, ctypes.c_int]
EDT = game_read("EDT\\AIMQUOTE.EDT")
e = Emu(dump=dump, fast=True)
if FAN is not None:
    e.put_115m()
PORT.ja1_fan_fixes_set.argtypes = [ctypes.c_uint64]
PORT.ja1_fan_fixes_set((FAN or 0) & ((1 << 64) - 1))
PORT.ja1_fan_fixes_set_hi.argtypes = [ctypes.c_uint64]
PORT.ja1_fan_fixes_set_hi((FAN or 0) >> 64)
rt = e.rt
QUOTE, PROF, SHARED, DST, TEXTBUF, AIMP = 0x22A1EA, 0xEBB40, 0x2299BC, 0x229B1C, 0xEBAC0, 0x22A044
st = {}


def fread(em, a, d, b, c, *_):
    out = EDT[st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    return len(out) // max(d, 1)


def fseek(em, a, d, b, *_):
    st["pos"] = d
    return 0


def open_file(em, a, *_):
    assert e.cstr(a).upper() == b"EDT\\AIMQUOTE.EDT", e.cstr(a)
    e.w32(rt(0x92F54), 1)
    return 1


def strstr(em, a, d, *_):
    i = e.cstr(a).find(e.cstr(d))
    return a + i if i >= 0 else 0


def box(em, a, *_):
    st["box"] = e.cstr(a)
    e.w32(e.uc.reg_read(UC_X86_REG_ESP), RET_SENTINEL)


e.stub(rt(0x219CC), lambda em, *_: st["r"])
e.stub(rt(0x75BB9), strstr)
e.stub(rt(0x38304), open_file)
e.stub(rt(0x757B1), fseek)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, *_: 0)
e.stub(rt(0x75668), lambda em, *_: 0)
e.stub(rt(0x7574F), lambda em, *_: 0)
e.stub(rt(0x5C0EC), lambda em, *_: 0)
e.stub(rt(0x534A4), lambda em, *_: 0, pop=4)
e.stub(rt(0x24D14), lambda em, *_: 0, pop=4)
for a in (0x67BF0, 0x673AC, 0x467E4, 0x28698):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x28FB8), box, pop=4)


def decode(t):
    return bytes(x if x == 32 else (x - 1) & 0xFF for x in t)


for a, n in ((SHARED, 200), (DST, 512), (TEXTBUF, 128)):
    e.uc.mem_write(rt(a), bytes(n))
LATE = [362, 580, 678]
rows = [k // 200 for k in range(0, len(EDT), 200) if b"NFNCFS" in EDT[k:k + 200]]
rows = [n for n in rows if n % 14 in (5, 6, 12) and n // 14 < 60]
for n in LATE:
    assert n in rows and EDT[n * 200:n * 200 + 200].index(b"NFNCFS") > 127, n
print(f"aimquote_diff: {len(rows)} AIM records of reasons 6, 7, 13 hold MEMBER")
r = random.Random(1103)
bad = cases = named = late = 0
lq, tb = bytearray(6), bytearray(128)
lq[:] = e.uc.mem_read(rt(QUOTE), 6)
tb[:] = e.uc.mem_read(rt(TEXTBUF), 128)
out = ctypes.create_string_buffer(2048)
for c in range(ncases):
    k = c % 5
    if k == 0:
        row = r.choice(LATE)
    elif k in (1, 2):
        row = r.choice(rows)
    else:
        row = None
    if row is not None:
        p, q = row // 14, row % 14
    else:
        p, q = r.randrange(60), r.randrange(14)
    reason = q + 1
    # The member bytes: the fields that the sites read, a real profile or a test name.
    m = r.randrange(60)
    for off in (0x4C, 0x3B, 0x4D):
        e.w8(rt(PROF + p * 90 + off), m)
    if c % 2:
        name = "".join(r.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh ") for _ in range(r.randrange(2, 10))).strip().encode() or b"Xy"
        e.uc.mem_write(rt(PROF + m * 90), name.ljust(10, b"\0"))
    if reason == 8:
        e.w8(rt(PROF + p * 90 + 0x2F), 0)    # not the hire road 0x206BA
    e.w8(rt(AIMP), p)
    e.w32(rt(0x22A041), p << 24)
    st.update(r=reason, pos=0, box=None)
    e.call(rt(0x20583), p, max_steps=200_000)
    if st["box"] is None or e.stop_reason:
        print(f"case {c}: p {p} reason {reason}: no box, {e.stop_reason}")
        bad += 1
        continue
    profiles = bytes(e.uc.mem_read(rt(PROF), 64 * 90))
    lq_c, tb_c = (ctypes.c_ubyte * 6).from_buffer(lq), (ctypes.c_ubyte * 128).from_buffer(tb)
    n = PORT.ja1f_aim_quote_text(ISO, EXE, profiles, lq_c, tb_c, p, q, out, len(out))
    if n < 0:
        print(f"case {c}: port error {n}")
        bad += 1
        continue
    v = out.raw[:n]
    tl = int.from_bytes(v[:2], "little")
    text, shared, target = v[2:2 + tl], v[2 + tl:202 + tl], v[202 + tl:]
    cases += 1
    named += reason in (6, 7, 13) and b"NFNCFS" in EDT[row * 200:row * 200 + 200] if row is not None else 0
    late += row in LATE
    want = (decode(st["box"]), bytes(e.uc.mem_read(rt(QUOTE), 6)), bytes(e.uc.mem_read(rt(TEXTBUF), 128)),
            bytes(e.uc.mem_read(rt(SHARED), 200)), e.cstr(rt(DST)))
    got = (text, bytes(lq), bytes(tb), shared, target)
    for label, a, b in zip(("box text", "last quote", "g_textBuf", "shared", "target"), want, got):
        if a != b:
            bad += 1
            if bad <= 8:
                print(f"case {c}: p {p} reason {reason} m {m}: {label}\n  orig {a[:200]!r}\n  port {b[:200]!r}")
            break
print(f"aimquote_diff{f' --fan {FAN:#x}' if FAN is not None else ''}: {cases} cases, {named} with MEMBER, {late} of them late, {bad} mismatches")
if bad or cases < ncases or not late:
    sys.exit(1)
print("aimquote_diff: all equal")
