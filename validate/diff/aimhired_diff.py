#!/usr/bin/env python3
"""Differential test: the "not available" message of the A.I.M. screen, AimAlreadyHiredMsg? 0x2306C, original vs port.

usage: .venv/bin/python validate/diff/aimhired_diff.py [ncases=1500] [DUMP]
The original runs in Unicorn with ShowMessage? 0x27B98 stubbed and logged. The port: ja1f_hired_text = aim::hired_text.
Compare the text, the x (0x42 at the left, 0 centred) and the time limit. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
NCASES = int(argv[0]) if argv and argv[0].isdigit() else 1500
DUMP = next((a for a in argv if not a.isdigit()), os.path.join(ROOT, "build", "mem_tac60.bin"))
if not os.path.exists(DUMP):
    print(f"aimhired_diff: {DUMP} missing")
    sys.exit(1)
PORT = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
PORT.ja1f_hired_text.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p]
PORT.ja1f_hired_text.restype = ctypes.c_int32
PROFILES, REC, FN = 0xEBB40, 90, 0x2306C
e = Emu(dump=DUMP, fast=True)
log = []


def show_msg(em, eax, edx, ebx, ecx):
    """ShowMessage? 0x27B98: text EAX, x EBX, 5 more arguments; +12 from ESP = the time limit (0xFA0 in the call)."""
    sp = em.uc.reg_read(__import__("unicorn.x86_const", fromlist=["UC_X86_REG_ESP"]).UC_X86_REG_ESP)
    log.append((bytes(em.uc.mem_read(eax, 200)).split(b"\0")[0], ebx & 0xFFFFFFFF,
                [em.r32(sp + k) for k in (4, 8, 12, 16)]))
    return 0


e.stub(e.rt(0x27B98), show_msg, 0x10)
rng = random.Random(77)
bad = 0
seen = set()
STATUS = [0, 1, 2, 3, 49, 50, 51, 52, 100, 127, 128, 200, 255]
for c in range(NCASES):
    p = rng.randrange(60)
    t = STATUS[c] if c < len(STATUS) else rng.randrange(256)
    name = bytes(rng.randrange(0x41, 0x5B) for _ in range(rng.randrange(0, 10))).ljust(10, b"\0")
    at = e.rt(PROFILES + p * REC)
    e.uc.mem_write(at, name)
    e.w8(at + 0x4B, t)
    pr = bytes(e.uc.mem_read(at, REC))
    log.clear()
    e.call(e.rt(FN), p, max_steps=200_000)
    out = ctypes.create_string_buffer(300)
    info = (ctypes.c_int32 * 2)()
    n = PORT.ja1f_hired_text(pr, out, 300, info)
    got = (out.raw[:n], info[0] & 0xFFFFFFFF, info[1])
    if len(log) != 1:
        print(f"case {c}: {len(log)} ShowMessage calls")
        bad += 1
        continue
    text, x, stack = log[0]
    if c == 0:
        print("stack args", stack)
    seen.add(x)
    if (text, x) != got[:2] or stack[2] != got[2]:
        if bad < 8:
            print(f"case {c} t={t}: orig {text!r} x={x} stack {stack}; port {got}")
        bad += 1
print(f"aimhired_diff: {NCASES} cases, x values {sorted(seen)}, {bad} mismatches")
sys.exit(1 if bad else 0)
