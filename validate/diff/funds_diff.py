#!/usr/bin/env python3
"""Differential test: the money count-down of the A.I.M. screen, AddFunds? 0x514A4 mode 1 + AimDrawFunds? 0x513E4, original vs port.

usage: .venv/bin/python validate/diff/funds_diff.py [ncases=40] [DUMP] [--lib SO]
The original runs in Unicorn on the A.I.M. dump aim_fired (real DrawNumber 0x36090, real BlitCopy; BusyWait? 0x67250 logged).
Per case: amount and start balance. Compared with ja1f_aim_count: the balance at each redraw, which redraws have a 30 ms
wait after them (the wait argument too), and the 40 x 12 screen part at (280, 183) round the 32 x 7 funds box at each redraw. Big amounts
(the 16-bit step rule) stub the redraw, so they compare the balances and waits only. Also amount 0 and mode 2: no redraw.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 40
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_aim_fired.bin")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_aim_count.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
                                ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
PORT.ja1f_aim_count.restype = ctypes.c_int32
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
BALANCE, ADD, DRAW, WAIT, BLIT = 0x229FC4, 0x514A4, 0x513E4, 0x67250, 0x52AE0
e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(0x95678))
log = []        # per redraw: [balance, box, wait argument or None]


PART = 40 * 12      # the 40 x 12 screen part at (280, 183) round the 32 x 7 box: digit rows outside the box stay as they were


def box():
    return b"".join(bytes(e.uc.mem_read(screen + (183 + y) * 320 + 280, 40)) for y in range(12))



def wait(em, eax, *_):
    if log:
        log[-1][2] = eax & 0xFFFFFFFF
    return 0


def draw_stub(em, *_):
    log.append([em.r32(em.rt(BALANCE)), None, None])
    return 0


e.stub(e.rt(WAIT), wait)


def original(amount, balance, pics, mode=1):
    global before
    before = box()
    """The log of a call. With pictures: a redraw ends at its last BlitCopy return, read at each wait and at the end."""
    log.clear()
    e.w32(e.rt(BALANCE), balance & 0xFFFFFFFF)
    e.stubs.pop(e.rt(DRAW), None)
    if pics:
        def wait_pic(em, eax, *_):
            log.append([em.r32(em.rt(BALANCE)), box(), eax & 0xFFFFFFFF])
            return 0
        e.stub(e.rt(WAIT), wait_pic)
        e.call(e.rt(ADD), -amount & 0xFFFFFFFF, mode, max_steps=400_000_000)
        end = e.r32(e.rt(BALANCE))
        if not log or log[-1][0] != end:   # the last redraw has no wait after it
            if amount > 0 and mode == 1:
                log.append([end, box(), None])
    else:
        e.stub(e.rt(DRAW), draw_stub)
        e.stub(e.rt(WAIT), wait)
        e.call(e.rt(ADD), -amount & 0xFFFFFFFF, mode, max_steps=400_000_000)
    return [list(x) for x in log], e.r32(e.rt(BALANCE))


def port(amount, balance, pics):
    cap = amount // 10 + 2
    vals, waits = (ctypes.c_int32 * cap)(), (ctypes.c_uint8 * cap)()
    boxes = ctypes.create_string_buffer(before, cap * PART) if pics else None
    n = PORT.ja1f_aim_count(ISO, EXE, balance, amount, cap, vals, waits, boxes)
    if n < 0:
        raise RuntimeError(f"ja1f_aim_count returned {n}")
    return [[vals[k], boxes.raw[k * PART:(k + 1) * PART] if pics else None, 30 if waits[k] else None] for k in range(n)]


r = random.Random(5120)
SMALL = [10, 11, 20, 299, 300, 301, 309, 310, 311, 319, 320, 800, 1000, 1500, 1234, 5, 9, 100, 101]
BIG = [327670, 327679, 327680, 327689, 327690, 655360, 655369, 655370, 400000]
fails = 0
total = 0
cases = [(a, True) for a in SMALL] + [(a, False) for a in BIG]
while len(cases) < ncases:
    cases.append((r.randrange(1, 4000), True))
for a, pics in cases[:max(ncases, len(SMALL) + len(BIG))]:
    bal = r.randrange(a, 120000) if pics else a + 1000
    got, end = original(a, bal, pics)
    want = port(a, bal, pics)
    total += 1
    bad = None
    if end != bal - a:
        bad = f"balance end {end} vs {bal - a}"
    elif len(got) != len(want):
        bad = f"redraws {len(got)} vs {len(want)}"
    else:
        for k, (x, y) in enumerate(zip(got, want)):
            if x != y:
                bad = f"redraw {k}: balance {x[0]} vs {y[0]}, wait {x[2]} vs {y[2]}, box {'same' if x[1] == y[1] else 'differs'}"
                break
    if bad:
        fails += 1
        if fails <= 5:
            print(f"MISMATCH amount {a} balance {bal}: {bad}")
# Nothing to count: amount 0, and any other mode adds at once.
for a, mode in ((0, 1), (800, 2), (800, 0)):
    got, end = original(a, 5000, False, mode)
    total += 1
    if got or end != 5000 - a:
        fails += 1
        print(f"MISMATCH no-count case amount {a} mode {mode}: {len(got)} redraws, balance {end}")
print(f"funds_diff: {total} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
