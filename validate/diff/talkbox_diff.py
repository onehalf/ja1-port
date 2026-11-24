#!/usr/bin/env python3
"""Differential test: a merc talks, 0x50908 (balloon::talk_box, ja1f_talk_box), original vs port.

usage: .venv/bin/python validate/diff/talkbox_diff.py [ncases=60] [DUMP] [--lib SO]
The original runs in Unicorn with the real LoadFaceSheet?, SpeechBalloon? and YesNoPrompt?. Stubs: the PCC loader 0x532BC (the
MAIN.DAT member), the frame delay 0x67250 and SpeechWait 0x28034 (they take the screen), the mouse calls.
Compared per case (Subtitles on and off, face point x 11 + 37 k, y as the callers pass, profile, text, noise screen): the number
of screens, all their pixels, the wait in ms. The calls in the eight callers: edx 11 + 37 k,
ebx 20, cl 5; FireMerc passes its own point. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from common import game_read  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402
import datarc, pcx  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"talkbox_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(LIB)
P.ja1f_talk_box.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int] * 3 + [ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_char_p,
                                                                          ctypes.c_void_p, ctypes.c_void_p]
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
MAIN = game_read("DAT\\MAIN.DAT")
e = Emu(dump=dump, fast=True)
rt = e.rt
screen = e.r32(rt(0x95678))
shots, names, waits, cache = [], [], [], {}


def load(em, eax, edx, ebx, ecx):
    n = em.cstr(ebx).decode("latin-1").upper()
    names.append(n)
    if n not in cache:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        cache[n] = (w, h, px)
    w, h, px = cache[n]
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x11D40, 0x67368, 0x67398, 0x67C54,
          0x1CB78, 0x320A0, 0x3F724, 0x1D8A4, 0x3F490, 0x681D0, 0x67478, 0x674B0, 0x467E4, 0x46EC4):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x67250), lambda em, *_: shots.append(bytes(em.uc.mem_read(screen, 64000))) or 0)


def speech_wait(em, eax, edx, ebx, ecx):
    waits.append(ebx)
    shots.append(bytes(em.uc.mem_read(screen, 64000)))
    return 0


e.stub(rt(0x28034), speech_wait, pop=12)
e.stub(rt(0x532BC), load, 4)
REC = rt(0xFA958)
TEXT = rt(0x2299BC)
WORDS = b"I need more money or I go home right now . you pay us well , boss !".split()
r = random.Random(50908)
bad = 0
seen = {"box": 0, "balloon": 0}
out = ctypes.create_string_buffer(9 * 64000)
for case in range(ncases):
    sub = case % 2
    k, y0 = r.randrange(8), r.choice([20, 20, 20, 60])
    p = r.choice([0, 1, 8, 16, 25, 32, 40])
    x0 = 11 + 37 * k
    text = b" ".join(r.choice(WORDS) for _ in range(r.randrange(1, 28)))
    nick = r.choice([b"Ivan", b"Grunty", b"Lynx", b"Hector"])
    e.w8(rt(0x1163BD), sub)
    e.w8(REC + 0x179, p)
    e.uc.mem_write(REC + 0x16E, nick + b"\0")
    # The quote as stored (+1 encoding, the space stays): the text-only box decodes it, the port's callers pass the decoded text.
    e.uc.mem_write(TEXT, bytes(c if c == 0x20 else c + 1 for c in text) + b"\0")
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    shots.clear()
    names.clear()
    waits.clear()
    e.call(rt(0x50908), REC, x0, y0, 5, max_steps=60_000_000)
    wait = ctypes.c_uint32(0)
    n = P.ja1f_talk_box(ISO, EXE, x0, y0, p, nick, sub, text, base, out, ctypes.byref(wait))
    seen["box" if sub else "balloon"] += 1
    if n < 0 or n != len(shots):
        print(f"case {case} sub {sub}: {len(shots)} original screens, port {n} (names {names})")
        bad += 1
        continue
    if waits != [wait.value]:
        print(f"case {case} sub {sub}: wait original {waits}, port {wait.value}")
        bad += 1
    for j in range(n):
        o = out.raw[j * 64000:(j + 1) * 64000]
        if o != shots[j]:
            d = [i for i in range(64000) if o[i] != shots[j][i]]
            print(f"case {case} sub {sub} screen {j}: {len(d)} pixels, first ({d[0] % 320},{d[0] // 320}) orig {shots[j][d[0]]} port {o[d[0]]}")
            bad += 1
            break
print(f"talkbox_diff: {ncases} cases, {bad} mismatches ({seen})")
sys.exit(1 if bad or 0 in seen.values() else 0)
