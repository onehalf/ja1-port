"""Differential test: Random 0x53538 = (rand_()*n/0x7FFF) % n over the Watcom seed at 0x9BBBC,
original vs the port's front-end rng (host::Rng through ja1f_random).

usage: .venv/bin/python validate/diff/rng_diff.py [ncases=3000] [DUMP] [--port] [--lib SO]
Per case: a random 32-bit seed, n (1..0x7FFF; the modulus is a signed 16-bit value), and 1..8 draws.
The original runs in Unicorn (no dump needed: the function touches only the seed); the seed is written
before the first draw and read after the last. --port: ja1f_random (host::Rng) from the same seed must
give the same last value and the same final seed. Exits 1 on any mismatch.
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(
    ROOT, "port", "ja1front", "target", "release", "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
n = int(args[0]) if args else 3000
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_random.argtypes = [ctypes.c_uint32, ctypes.c_int32, ctypes.c_uint32,
                                 ctypes.POINTER(ctypes.c_uint32)]
    PORT.ja1f_random.restype = ctypes.c_int32

RANDOM, SEED = 0x53538, 0x9BBBC
e = Emu(fast=True)          # no dump: the function touches only the seed at 0x9BBBC
rng = random.Random(53538)
fails = 0
seen = {}
for i in range(n):
    seed = rng.getrandbits(32)
    mod = rng.choice([1, 2, 3, 5, 7, 10, 20, 50, 100, 250, 1000, 0x7FFF, -1, -7, -100, -0x8000, rng.randrange(1, 0x8000)])
    draws = rng.randrange(1, 9)
    e.w32(SEED, seed)
    last = 0
    for _ in range(draws):
        last = e.call(e.rt(RANDOM), mod)
        last -= (last & 0x80000000) << 1      # EAX as a signed number
    oseed = e.r32(SEED)
    if PORT:
        fs = ctypes.c_uint32(seed)
        pv = PORT.ja1f_random(seed, mod, draws, ctypes.byref(fs))
        if pv != last or fs.value != oseed:
            fails += 1
            if fails <= 10:
                print(f"  case {i}: seed {seed:08X} n {mod} draws {draws}: "
                      f"original {last} seed {oseed:08X}, port {pv} seed {fs.value:08X}")
    seen[i] = last
# r = 0x7FFF: the one draw where (r * n) / 0x7FFF = n, and the original wraps it to 0. Random seeds see it once in 32768 draws.
INV = pow(0x41C64E6D, -1, 1 << 32)
tops = 0
for mod in [2, 3, 5, 7, 10, 20, 50, 100, 250, 1000, 0x7FFF, -2, -7, -100, -0x8000]:
    for low in (0, 0x1234, 0xFFFF):
        seed = (((0x7FFF0000 | low) - 0x3039) * INV) & 0xFFFFFFFF
        e.w32(SEED, seed)
        last = e.call(e.rt(RANDOM), mod)
        last -= (last & 0x80000000) << 1
        tops += 1
        if PORT:
            fs = ctypes.c_uint32(seed)
            pv = PORT.ja1f_random(seed, mod, 1, ctypes.byref(fs))
            if pv != last or fs.value != e.r32(SEED):
                fails += 1
                if fails <= 10:
                    print(f"  top draw: seed {seed:08X} n {mod}: original {last}, port {pv}")
print(f"rng_diff: {tops} draws at r = 0x7FFF")
# n = 0: the original's idiv stops the program; the port gives 0. Both must draw from the seed first.
if PORT:
    e.w32(SEED, 0x12345678)
    try:
        e.call(e.rt(RANDOM), 0)
        print("  n 0: the original did not stop")
        fails += 1
    except Exception:                       # noqa: BLE001 - the divide fault of Unicorn or an EmuStop
        pass
    fs = ctypes.c_uint32(0)
    if PORT.ja1f_random(0x12345678, 0, 1, ctypes.byref(fs)) != 0 or fs.value != e.r32(SEED):
        print("  n 0: the port's value or seed differs")
        fails += 1
print(f"{n} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
