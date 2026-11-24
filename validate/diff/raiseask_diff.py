#!/usr/bin/env python3
"""Differential test: the quote boxes of a salary raise, RaiseSalaryPrompt? 0x448A8, original vs port.

usage: .venv/bin/python validate/diff/raiseask_diff.py [ncases=300] [DUMP] [--lib SO]
The original runs in Unicorn with the real RaiseReason? 0x44634, AimRate, PlayQuote 0x24F08, EncodeStringPlus1 0x35810 and StrReplace
0x252B0; the file calls read the real PSTQUOTE.EDT. Stubs: the voice file calls, strstr_ (faults in Unicorn), the box 0x50908
(takes the text of 0x2299BC), the offer dialog 0x29074 (scripted answers), the map redraw and the merc's leave.
The port: balloon::say_boxes + play::say_args + play::merc_quote (ja1f_raise_quote), one call per box. Compared, with the states kept
from case to case as in the game: the texts of the boxes in order, the last quote 0x22A1EA, the member byte 0x22A1F0, g_textBuf,
the shared buffer 0x2299BC, the StrReplace target 0x229B1C. The decisions (reason, offers, answers) stay in raise_diff.
PST rows 620 and 635 hold MEMBER past place 127 (reason 5 of profiles 41 and 42). The run fails if it sees neither.
The name voices (PlayMercVoice 0x24D14 calls vs play::name_voice) too.
Not compared: pixels (talkbox_diff), the box point (raise_diff), voice bytes. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from common import game_read  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"raiseask_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_raise_quote.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_ubyte] + [ctypes.c_int] * 3 + [ctypes.c_void_p] * 3 + [ctypes.c_int]
EDT = game_read("EDT\\PSTQUOTE.EDT")
e = Emu(dump=dump, fast=True)
rt = e.rt
QUOTE, MEMBER, PROF, SHARED, DST, TEXTBUF, SOL = 0x22A1EA, 0x22A1F0, 0xEBB40, 0x2299BC, 0x229B1C, 0xEBAC0, 0xFA958
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
    assert e.cstr(a).upper() == b"EDT\\PSTQUOTE.EDT", e.cstr(a)
    e.w32(rt(0x92F54), 1)
    return 1


def strstr(em, a, d, *_):
    i = e.cstr(a).find(e.cstr(d))
    return a + i if i >= 0 else 0


def text_box(em, *_):
    st["boxes"].append(e.cstr(rt(SHARED)))


def dialog(em, *_):
    return st["answers"].pop(0)


e.stub(rt(0x75BB9), strstr)
e.stub(rt(0x38304), open_file)
e.stub(rt(0x757B1), fseek)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, *_: 0)
e.stub(rt(0x75668), lambda em, *_: 0)
e.stub(rt(0x7574F), lambda em, *_: 0)
e.stub(rt(0x5C0EC), lambda em, *_: 0)
e.stub(rt(0x534A4), lambda em, *_: 0, pop=4)
voices = []
# PlayMercVoice 0x24D14 (profile EAX, DL = remark or member): the name voices the original plays, in order.
e.stub(rt(0x24D14), lambda em, eax, edx, *_: voices.append(((edx & 0xFF) ^ 0x80) - 0x80) or 0, pop=4)
e.stub(rt(0x50908), text_box)
e.stub(rt(0x29074), dialog, pop=8)
for a in (0x67BF0, 0x673AC, 0x467E4, 0x28698, 0x4420C, 0x4508C, 0x46AC4, 0x21B3C, 0x50618, 0x50750):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x36010), lambda em, *_: 0, pop=4)
e.stub(rt(0x52AE0), lambda em, *_: 0, pop=8)


def decode(t):
    return bytes(x if x == 32 else (x - 1) & 0xFF for x in t)


for a, n in ((SHARED, 200), (DST, 512), (TEXTBUF, 128)):
    e.uc.mem_write(rt(a), bytes(n))
LATE = {41, 42}
assert all(EDT[(15 * p + 5) * 200:(15 * p + 5) * 200 + 200].index(b"NFNCFS") > 127 for p in LATE)
FIELD = {1: 0x36, 2: 0x37, 3: 0x38, 4: 0x39}
r = random.Random(1103)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
KEEP = ((SHARED, 200), (DST, 512), (TEXTBUF, 128), (QUOTE, 7))
state = bytearray(e.uc.mem_read(rt(QUOTE), 7))
tb = bytearray(128)
out = ctypes.create_string_buffer(4096)
bad = cases = late = boxes = 0
for c in range(ncases):
    keep = [(a, bytes(e.uc.mem_read(rt(a), n))) for a, n in KEEP]
    for lo, m in snap:
        e.uc.mem_write(lo, m)
    for a, m in keep:
        e.uc.mem_write(rt(a), m)
    reason = r.randrange(1, 6)
    p = r.choice((41, 42)) if reason == 5 and c % 2 == 0 else r.randrange(60)
    g = r.randrange(60) if reason == 5 else -1
    name = "".join(r.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh ") for _ in range(r.randrange(2, 10))).strip().encode() or b"Xy"
    rec, pr = rt(SOL), rt(PROF + p * 0x5A)
    for k in range(8):
        e.w8(rt(SOL + 0x200 * k + 0x182), 0)
    e.w8(rec + 0x182, 1)
    e.w8(rec + 0x183, 40)        # hp
    e.w8(rec + 0x179, p)
    e.w8(rec + 0x17A, 0)
    e.w8(rec + 0x19C, 0)
    for off in (0x36, 0x37, 0x39):
        e.w8(pr + off, 101)
    e.w8(pr + 0x38, 255)
    e.w8(pr + 0x4D, g if reason == 5 else 99)
    if reason < 5:
        e.w8(pr + FIELD[reason], 0)
    e.w8(pr + 0x32, 3)
    if g >= 0:
        e.uc.mem_write(rt(PROF + g * 0x5A), name.ljust(10, b"\0"))
    answers = r.choice(([-1], [0], [2], [1, 2], [1, 1, 0]))
    st.update(pos=0, boxes=[], answers=list(answers))
    voices.clear()
    e.call(rt(0x448A8), max_steps=2_000_000)
    if e.stop_reason:
        print(f"case {c}: original stopped: {e.stop_reason}")
        bad += 1
        continue
    want = [decode(b) for b in st["boxes"]]
    got = []
    nick = name if g >= 0 else b""
    tb_c, st_c = (ctypes.c_ubyte * 128).from_buffer(tb), (ctypes.c_ubyte * 7).from_buffer(state)
    shared = target = b""
    pvoices = []
    for op, a in [(296, reason)] + [(298, x) for x in answers if x >= 0]:
        n = PORT.ja1f_raise_quote(ISO, EXE, nick, p, op, a, g, st_c, tb_c, out, len(out))
        if n < 0:
            got.append(b"port error")
            continue
        v = out.raw[:n]
        tl = int.from_bytes(v[2:4], "little")
        m = PORT.ja1f_raise_name_voice(p, op, a, g, v[1])
        pvoices += [m] if m >= 0 else []
        got.append(v[4:4 + tl])
        shared, target = v[4 + tl:204 + tl], v[204 + tl:]
    cases += 1
    boxes += len(want)
    late += p in LATE and reason == 5
    checks = (("boxes", want, got), ("last quote", bytes(e.uc.mem_read(rt(QUOTE), 6)), bytes(state[:6])),
              ("g_textBuf", bytes(e.uc.mem_read(rt(TEXTBUF), 128)), bytes(tb)),
              ("shared", bytes(e.uc.mem_read(rt(SHARED), 200)), shared), ("target", e.cstr(rt(DST)), target))
    checks += (("name voices", voices, pvoices),)
    if reason == 5:
        checks += (("member", e.r8(rt(MEMBER)), state[6]),)
    for label, a, b in checks:
        if a != b:
            bad += 1
            if bad <= 6:
                print(f"case {c}: p {p} reason {reason} g {g} answers {answers}: {label}\n  orig {a!r}\n  port {b!r}"[:700])
            break
print(f"raiseask_diff: {cases} cases, {boxes} boxes, {late} late MEMBER, {bad} mismatches")
if bad or cases < ncases or not late:
    sys.exit(1)
print("raiseask_diff: all equal")
