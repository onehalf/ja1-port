#!/usr/bin/env python3
"""Differential test: "Say it again" (key L), SpecialNpcSpeech? 0x29D54 with the real PlayQuote 0x24F08, original vs port.

usage: .venv/bin/python validate/diff/sayagain_diff.py [ncases=400] [DUMP] [--lib SO]
A case: the last quote (speaker 0..0x42 or above, category, row, quote), the byte after it (the profile that MEMBER names), nicknames.
Directed cases take records that hold MEMBER. The original runs in Unicorn; OpenCDFile, fseek, fread, fclose read the real EDT file,
so PlayQuote's record arithmetic and the StrReplace are the original's. YesNoPrompt? 0x28C84 is a stub that takes the text (+1
encoded), the speaker, the name. The port: balloon::again_text (ja1f_again_text). Compared: no box / box, speaker, name, text,
and the states the original keeps between runs: the shared text buffer when a record lies past the end of the file (the old text
stays), StrReplace's target (MEMBER at place 128 or later: the signed byte bug, issue #2), g_textBuf as the name when the byte is 60 or
more. Not modelled: g_textBuf after the call (the encode writes the nickname there; again_text takes a read-only world).
Also checked on the original side: the calls in their order (panel picture, save area 0, 0x34, 0x25, 0xC8, quote, prompt with ECX 0,
restore area, path marks off) and byte 0x92E4F put back. Not compared: pixels of the box (question_diff --face), the panel name
(the callers pass it; panel.md Say it again lists them), rows below 0 (no quote has one).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 400
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
SAVE = os.path.join(ROOT, "build", "DAY_day3.SAV")
for f in (dump, SAVE):
    if not os.path.exists(f):
        print(f"sayagain_diff: {f} missing")
        sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_again_text.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_char_p, ctypes.c_ubyte, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p,
                                                         ctypes.c_void_p, ctypes.c_int]
CATS = ["AIM", "DAY", "PST", "JAC", "INV", "SIG", "QST", "BRE", "NAT", "SAN", "INT"]
EDT = {c: game_read(f"EDT\\{c}QUOTE.EDT") for c in CATS}
e = Emu(dump=dump, fast=True)
rt = e.rt
QUOTE, MEMBER, PROF, E4F, SHARED, DST, TEXTBUF = 0x22A1EA, 0x22A1F0, 0xEBB40, 0x92E4F, 0x2299BC, 0x229B1C, 0xEBAC0
st = {}


def log(s):
    st["log"].append(s)


def open_file(em, a, *_):
    st["file"] = e.cstr(a).decode().upper()
    st["pos"] = 0
    log("open " + st["file"])
    e.w32(rt(0x92F54), 1)
    return 1


def fseek(em, a, d, b, *_):
    st["pos"] = d
    return 0


def fread(em, a, d, b, c, *_):
    data = game_read(st["file"]) if st["file"] not in st["cache"] else st["cache"][st["file"]]
    st["cache"][st["file"]] = data
    out = data[st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    return len(out) // max(d, 1)


def prompt(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    st["box"] = (d & 0xFF, e.cstr(b), e.cstr(a))
    log(f"prompt ecx {c} stack {e.r32(sp + 4)} flag {e.r8(rt(0x92EF0))}")


def strstr(em, a, d, *_):
    # strstr_ 0x75BB9 faults in Unicorn (segment pushes); the rest of StrReplace 0x252B0 runs.
    i = e.cstr(a).find(e.cstr(d))
    return a + i if i >= 0 else 0


e.stub(rt(0x75BB9), strstr)
e.stub(rt(0x38304), open_file)
e.stub(rt(0x757B1), fseek)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, *_: 0)
e.stub(rt(0x28C84), prompt, pop=4)
e.stub(rt(0x46B1C), lambda em, *_: log("panel"))
e.stub(rt(0x467E4), lambda em, a, d, b, c: log(f"save area {a} {d} {b} {c}"))
e.stub(rt(0x468B0), lambda em, a, *_: log(f"restore area {a}"))
e.stub(rt(0x1D8A4), lambda em, *_: log("marks off"))
for a in (0x67BF0, 0x673AC):
    e.stub(rt(a), lambda em, *_: 0)

r = random.Random(1101)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
e.uc.mem_write(rt(SHARED), bytes(200))
e.uc.mem_write(rt(DST), bytes(512))
# Records with MEMBER (+1 encoded).
with_member = [(i, k // 200) for i, c in enumerate(CATS) for k in range(0, len(EDT[c]), 200) if b"NFNCFS" in EDT[c][k:k + 200]]


def nick():
    return "".join(r.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh") for _ in range(r.randrange(2, 10))).encode()  # 9 at most: the field holds its NUL


def decode(t):
    return bytes(x if x == 32 else (x - 1) & 0xFF for x in t)


bad = cases = boxes = stale = 0
for c in range(ncases):
    keep = bytes(e.uc.mem_read(rt(SHARED), 200))
    keepd = bytes(e.uc.mem_read(rt(DST), 512))
    for lo, m in snap:
        e.uc.mem_write(lo, m)
    e.uc.mem_write(rt(SHARED), keep)
    e.uc.mem_write(rt(DST), keepd)
    k = r.choice([r.randrange(60), r.randrange(60, 64), 0x40, 0x41, 0x42, r.randrange(0x43, 256)]) if c % 8 else 0x43 + c % 3 * 40
    cat = r.randrange(len(CATS))
    size = len(EDT[CATS[cat]]) // 200
    row, q = r.randrange(0, 40), r.randrange(0, max(size - 40, 1))
    m = r.choice([r.randrange(60), r.randrange(60, 256)])
    if c % 3 == 1 and with_member:
        cat, rec = r.choice(with_member)
        row = r.randrange(0, min(rec, 40) + 1)
        q = rec - row
        m = r.randrange(60)
    if c % 17 == 5:
        q = size + r.randrange(0, 50)        # past the end of the file: the text before stays
    nk, nm, tb = nick(), nick(), nick()
    e.uc.mem_write(rt(TEXTBUF), tb + b"\0")
    qb = bytes([k, cat]) + row.to_bytes(2, "little") + q.to_bytes(2, "little")
    e.uc.mem_write(rt(QUOTE), qb)
    e.w8(rt(MEMBER), m)
    for p, n in ((k, nk), (m, nm)):
        if p < 64:
            e.uc.mem_write(rt(PROF + p * 0x5A), n.ljust(10, b"\0"))
    e.w8(rt(E4F), 0)
    st.update(log=[], box=None, file=None, pos=0, cache=st.get("cache", {}))
    try:
        e.call(rt(0x29D54), max_steps=500_000)
    except EmuStop as x:
        print(f"case {c}: original stopped: {x}")
        bad += 1
        continue
    buf = ctypes.create_string_buffer(1024)
    n = PORT.ja1f_again_text(ISO, EXE, SAVE.encode(), qb, m, nk, nm, tb, buf, 1024)
    assert n >= 0, n
    cases += 1
    port = None if n == 0 else (buf.raw[0], buf.raw[2:2 + buf.raw[1]], buf.raw[2 + buf.raw[1]:n])
    orig = None
    if st["box"]:
        boxes += 1
        orig = (st["box"][0], st["box"][1], decode(st["box"][2]))
        want = ["panel", "save area 0 52 37 200", f"open EDT\\{CATS[cat]}QUOTE.EDT", "prompt ecx 0 stack 0 flag 1", "restore area 0", "marks off"]
        if st["log"] != want or e.r8(rt(E4F)) != 0 or e.r8(rt(0x92EF0)) != 0:
            bad += 1
            print(f"case {c}: original calls {st['log']} 0x92E4F {e.r8(rt(E4F))}")
    if orig != port:
        bad += 1
        if bad <= 4:
            print(f"case {c} k {k:#x} cat {cat} row {row} q {q} m {m}:\n  orig {orig}\n  port {port}")
    stale += c % 17 == 5
print(f"sayagain_diff: {cases} cases ({boxes} boxes, {stale} past the file end), {bad} mismatches")
if not boxes or boxes == cases:
    print("sayagain_diff: a branch never ran")
    bad += 1
sys.exit(1 if bad else 0)
