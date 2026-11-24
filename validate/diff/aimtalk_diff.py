#!/usr/bin/env python3
"""Differential test: the A.I.M. quote box 0x28FB8, original vs port.

usage: .venv/bin/python validate/diff/aimtalk_diff.py [ncases=2000] [DUMP] [--lib SO]
The original runs in Unicorn; YesNoPrompt 0x28C84, MousePoll 0x678E8, the picture load 0x532BC and SpeechWait 0x28034 are stubbed
and logged. The 5 call sites are read from the image: all pass ECX 8, stack 20, BL 0. The port: aim::talk_plan (ja1f_aim_talk).
Compared with Subtitles off: no box, the face member name, the wait's point, its time, its hold. With Subtitles on: the box, the
member (the box loads the profile in DL as the name).
On the original side only: the call order, the box's arguments (text, nickname = the profile record, ECX 0, stack 1), the wait's
stack arguments (1, 0x90494, 0), the picture's buffer and size, the pointer poll 0.
Not compared: the box itself (question_diff), the wait (speechwait_diff), the face drawing (faceanim_diff).
Exits 1 on mismatch.
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
ncases = int(args[0]) if args else 2000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"aimtalk_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_aim_talk.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p]
e = Emu(dump=dump, fast=True)
rt = e.rt
SUBS, PROFILES, STRIP, TEXT = 0x1163BD, 0xEBB40, 0x93E70, 0x229B1C
bad = 0


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


# The 5 callers (xref): push 0x14, mov ecx 8, xor ebx ebx in the 40 bytes before the call.
for site in (0x20638, 0x206EF, 0x207B6, 0x20902, 0x220B5):
    code = bytes(e.uc.mem_read(rt(site - 40), 40))
    if b"\x6a\x14\xb9\x08\x00\x00\x00" not in code or b"\x31\xdb" not in code:
        report(site, "a caller passes other numbers")
log = []


def sp(k):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + k)


e.stub(rt(0x28C84), lambda em, a, d, b, c, *_: log.append(("box", a, d & 0xFF, b, c, sp(4))) or 1, 4)
e.stub(rt(0x678E8), lambda em, a, *_: log.append(("poll", a)) or 0)
e.stub(rt(0x532BC), lambda em, a, d, b, c, *_: log.append(("load", e.cstr(b), c, sp(4))) or 77, 4)
e.stub(rt(0x28034), lambda em, a, d, b, c, *_: log.append(("wait", a, d, b, c, sp(4), sp(8), sp(12))) or 0, 12)
r = random.Random(1111)
kinds = [0, 0]
for c in range(ncases):
    sub, prof = r.randrange(2), r.choice((0, 1, 9, 10, 63, r.randrange(64)))
    n = r.choice((0, 1, 2, 199, r.randrange(200)))
    e.uc.mem_write(rt(TEXT), bytes(r.randrange(1, 256) for _ in range(n)) + b"\0")
    e.w8(rt(SUBS), sub)
    e.w32(rt(0x229D18), 0)
    log.clear()
    try:
        e.call(rt(0x28FB8), rt(TEXT), prof, 0, 8, 20, max_steps=100_000)
    except (EmuStop, TypeError) as x:
        report(c, f"original stopped: {x}")
        continue
    o, nm = (ctypes.c_int * 5)(), ctypes.create_string_buffer(8)
    if PORT.ja1f_aim_talk(sub, prof, n, o, nm):
        report(c, "port error")
        continue
    kinds[sub] += 1
    boxed, x, y, ms, hold = o
    face = nm.value
    if sub:
        want = [("box", rt(TEXT), prof, rt(PROFILES) + 90 * prof, 0, 1)]
        if log != want:
            report(c, f"original box {log}")
        if not boxed or face != b"%d" % log[0][2]:
            report(c, f"port boxed {boxed} face {face}")
    else:
        if [k[0] for k in log] != ["poll", "load", "wait"] or log[0][1] != 0:
            report(c, f"original order {log}")
            continue
        _, name, buf, size = log[1]
        _, wx, wy, wms, whold, s1, s2, s3 = log[2]
        if buf != e.r32(rt(STRIP)) or size != 0x94 or (s1, s2, s3) != (1, rt(0x90494), 0) or e.r32(rt(0x229D18)) != 77:
            report(c, f"original arguments {log}")
        if boxed or name != face or (wx, wy, wms, whold) != (x, y, ms, hold):
            report(c, f"original {log[1:]} port {list(o)}")
print(f"aimtalk_diff: {ncases} cases, {kinds[1]} with Subtitles, {bad} mismatches")
sys.exit(1 if bad or not all(kinds) else 0)
