#!/usr/bin/env python3
"""Differential test: a merc's morning quotes, AimComplainQuote? 0x44D28 and the quote part of HatedCheck? 0x44C10, original vs port.

usage: .venv/bin/python validate/diff/complainquote_diff.py [ncases=600] [DUMP] [--lib SO]
The original runs in Unicorn with the real PlayQuote 0x24F08, StrReplace 0x252B0 and EncodeStringPlus1 0x35810; the file
calls read the real PSTQUOTE.EDT. Stubs: the voice file calls (sprintf_, itoa_, OpenArchiveForSpeech?, ArcSeek: no voice
found), strstr_ (faults in Unicorn), the box 0x50908 (takes the text of 0x2299BC and its arguments), PlayMercVoice 0x24D14.
The port: balloon::complain_texts / hate_text + play::merc_quote + play::encode_name (ja1f_complain_quote).
Compared, case by case with the states kept from case to case as in the game: the boxes (quote, text), the last quote
0x22A1EA, the member byte 0x22A1F0, g_textBuf (complaints), the shared buffer 0x2299BC, the StrReplace target 0x229B1C.
On the original side only: the order of the calls, the voice requests (speaker, row, quote, category, play flag), the box
arguments (x = slot x 37 + 11, y 20, 5), byte 0x92F8B = 1. The port's op dispatch and the x are in smoke step morning_complain.
Not compared: pixels (balloon_diff, question_diff), the voice bytes. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
from unicorn import UC_HOOK_CODE  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 600
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"complainquote_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_complain_quote.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_ubyte, ctypes.c_ubyte, ctypes.c_int, ctypes.c_int, ctypes.c_int,
                                                             ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
EDT = game_read("EDT\\PSTQUOTE.EDT")
e = Emu(dump=dump, fast=True)
rt = e.rt
QUOTE, MEMBER, PROF, SHARED, DST, TEXTBUF, SOL = 0x22A1EA, 0x22A1F0, 0xEBB40, 0x2299BC, 0x229B1C, 0xEBAC0, 0xFA958
PTRS = 0x100958
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


def text_box(em, a, d, b, c, *_):
    st["log"].append(("box", (a & 0xFFFFFFFF), d, b, c & 0xFF, e.r8(rt(0x92F8B))))
    st["boxes"].append(bytes(e.uc.mem_read(rt(SHARED), 200)))


def voice(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    st["log"].append(("name", a & 0xFF, d & 0xFF, e.cstr(b), c, e.r32(sp + 4)))


def on_quote(uc, addr, size, _):
    sp = uc.reg_read(UC_X86_REG_ESP)
    st["log"].append(("quote", st_reg("eax") & 0xFF, st_reg("edx"), st_reg("ebx"), e.r32(sp + 8) & 0xFF, e.r32(sp + 12) & 0xFF, e.r32(sp + 16)))


def st_reg(n):
    from unicorn import x86_const as x
    return e.uc.reg_read(getattr(x, "UC_X86_REG_" + n.upper()))


e.stub(rt(0x75BB9), strstr)
e.stub(rt(0x38304), open_file)
e.stub(rt(0x757B1), fseek)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, *_: 0)
e.stub(rt(0x75668), lambda em, *_: 0)
e.stub(rt(0x7574F), lambda em, *_: 0)
e.stub(rt(0x5C0EC), lambda em, *_: 0)
e.stub(rt(0x534A4), lambda em, *_: 0, pop=4)
e.stub(rt(0x50908), text_box)
e.stub(rt(0x24D14), voice, pop=4)
for a in (0x67BF0, 0x673AC, 0x467E4):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x28698), lambda em, *_: 0)
e.uc.hook_add(UC_HOOK_CODE, on_quote, begin=rt(0x24F08), end=rt(0x24F08))

r = random.Random(1102)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
for a, n in ((SHARED, 200), (DST, 512), (TEXTBUF, 128)):
    e.uc.mem_write(rt(a), bytes(n))
with_member = [k // 200 for k in range(0, len(EDT), 200) if b"NFNCFS" in EDT[k:k + 200]]
with_member = [(n // 15, n % 15) for n in with_member if n % 15 in (6, 8, 9, 10, 11) and n // 15 < 60]
print(f"complainquote_diff: {len(with_member)} PST records of quotes 6, 8..11 hold MEMBER")


def nick():
    return "".join(r.choice("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh ") for _ in range(r.randrange(2, 10))).strip().encode() or b"Xy"


def decode(t):
    t = t[:t.index(0)] if 0 in t else t
    return bytes(x if x == 32 else (x - 1) & 0xFF for x in t)


bad = cases = named = hates = 0
state = bytearray(7)
tb = bytearray(128)
KEEP = ((SHARED, 200), (DST, 512), (TEXTBUF, 128), (QUOTE, 7))
for c in range(ncases):
    keep = [(a, bytes(e.uc.mem_read(rt(a), n))) for a, n in KEEP]
    for lo, m in snap:
        e.uc.mem_write(lo, m)
    for a, m in keep:
        e.uc.mem_write(rt(a), m)
    hate = c % 4 == 3
    if c % 3 == 0 and with_member:
        p, q = r.choice(with_member)
        hate = q == 6
    else:
        p, q = r.randrange(60), 6 if hate else r.choice((8, 9, 10, 11))
    m = r.randrange(60)
    slot = r.randrange(8)
    first = 0 if hate else r.randrange(2)
    n = nick()
    e.uc.mem_write(rt(PROF + m * 0x5A), n.ljust(10, b"\0"))
    rec = rt(SOL + 0x200 * slot)
    e.w32(rt(PTRS + 4 * slot), rec)
    e.w8(rec + 0x179, p)
    e.w8(rec + 0x17A, slot)
    st.update(log=[], boxes=[], pos=0)
    try:
        if hate:
            # 0x44C10 runs from its first record: only this record speaks.
            for k in range(8):
                r2 = rt(SOL + 0x200 * k)
                e.w8(r2 + 0x182, 1 if k <= 1 else 0)
                e.w8(r2 + 0x183, 0 if k != slot % 2 else 1)
                e.w8(r2 + 0x179, m if k != slot % 2 else p)
                e.w8(r2 + 0x17A, k)
            slot %= 2
            if m == p:
                e.w8(rt(SOL + 0x200 * (1 - slot) + 0x179), m)
            e.w8(rt(PROF + p * 0x5A + 0x3A), m)
            e.call(rt(0x44C10), max_steps=800_000)
        else:
            flag = e.alloc(4) if "flag" not in st else st["flag"]
            st["flag"] = flag
            e.w8(flag, 1 - first)
            e.call(rt(0x44D28), slot, flag, q, rt(PROF + m * 0x5A), m, max_steps=800_000)
    except EmuStop as x:
        print(f"case {c}: original stopped: {x}")
        bad += 1
        continue
    cases += 1
    log = st["log"]
    # The original's calls.
    x = slot * 0x25 + 0xB
    q1 = ("quote", p, p * 15)
    boxes = [(7 if first else q, b) for b in st["boxes"][:1]] + [(q, b) for b in st["boxes"][1:]]
    kinds = [t[0] for t in log]
    want = (["quote", "box"] if hate else (["quote", "box"] if first else []) + ["name", "quote", "box"])
    if kinds != want:
        bad += 1
        print(f"case {c}: original calls {log}")
    for t in log:
        if t[0] == "box" and (t[2:] != (x, 20, 5, 1) or t[1] != rt(SOL + 0x200 * slot)):
            bad += 1
            print(f"case {c}: box arguments {t}")
        if t[0] == "name" and t[1:] != (p, m, b"", 1, 1):
            bad += 1
            print(f"case {c}: name voice {t}")
        if t[0] == "quote" and (t[1:3] != q1[1:3] or t[4:] != (2, p, 1)):
            bad += 1
            print(f"case {c}: quote call {t}")
    # The port.
    pst = (ctypes.c_ubyte * 7)(*state)
    ptb = (ctypes.c_ubyte * 128)(*tb)
    buf = ctypes.create_string_buffer(4096)
    got = PORT.ja1f_complain_quote(ISO, EXE, n, p, m, q, first, int(hate), pst, ptb, buf, 4096)
    assert got > 0, got
    state = bytearray(pst)
    tb = bytearray(ptb)
    raw = buf.raw[:got]
    i = 1
    port = []
    for _ in range(raw[0]):
        ln = int.from_bytes(raw[i + 1:i + 3], "little")
        port.append((raw[i], raw[i + 3:i + 3 + ln]))
        i += 3 + ln
    shared, target = raw[i:i + 200], raw[i + 200:]
    orig = [(a, decode(b)) for a, b in boxes]
    ot = e.cstr(rt(DST))
    results = {"boxes": (orig, port), "shared": (bytes(e.uc.mem_read(rt(SHARED), 200)), shared), "target": (ot, target),
               "last quote": (bytes(e.uc.mem_read(rt(QUOTE), 7)), bytes(state)),
               "text_buf": (bytes(e.uc.mem_read(rt(TEXTBUF), 128)), bytes(tb))}
    for name, (o, g) in results.items():
        if o != g:
            bad += 1
            if bad <= 6:
                print(f"case {c} hate {int(hate)} p {p} q {q} m {m} first {first} nick {n}: {name}\n  orig {o}\n  port {g}")
    named += b"NFNCFS" in EDT[(15 * p + q) * 200:(15 * p + q + 1) * 200]
    hates += hate
print(f"complainquote_diff: {cases} cases ({hates} hate, {named} with MEMBER), {bad} mismatches")
if cases < ncases or not named or not hates:
    print("complainquote_diff: a branch never ran")
    bad += 1
sys.exit(1 if bad else 0)
