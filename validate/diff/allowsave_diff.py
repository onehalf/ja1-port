"""Differential test: 0x4E93C (QuickRestore's allow-save mode step), original vs port.

usage: .venv/bin/python validate/diff/allowsave_diff.py [ncases=4000] [DUMP]

Per case: fuzzed quick-save-mode-chosen byte 0x22A1FA, mode byte 0x22A1F9 (the day save's base bytes 21286 / 21285),
the program option "allow save" byte 0x955C6 (0/1) and the flag 0x955C7 it copies. The original runs in Unicorn on
tac60: when the mode is not chosen (0x22A1FA = 0) and allow-save is on, it sets 0x22A1FA = 1 and 0x22A1F9 = 0x955C7.
The port (ja1f_allow_save_mode = world::set_allow_save_mode) gets the same bytes and the allow flag; both base bytes
must match. 0x955C7 is 1 whenever 0x955C6 is 1 (both come from the command-line word ALLOWSAVE), so the test keeps
that pair together; with allow off the flag is fuzzed and must not matter.

Exits 1 on any mismatch.
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
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
ncases = int(argv[0]) if argv and argv[0].isdigit() else 4000
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")

PORT = ctypes.CDLL(LIB)
PORT.ja1f_allow_save_mode.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int32]
PORT.ja1f_allow_save_mode.restype = ctypes.c_int32

FN, CHOSEN, MODE, ALLOW, FLAG = 0x4E93C, 0x22A1FA, 0x22A1F9, 0x955C6, 0x955C7
BASE, BASE_LEN = 0x224ED4, 21648          # the port's base window; 21285/21286 = 0x22A1F9/0x22A1FA

if not os.path.exists(dump):
    print(f"allowsave_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
buf = e.alloc(BASE_LEN)

rng = random.Random(int(os.environ.get("ALLOWSAVE_SEED", "1")))
fails = 0
cases = [(0, 0, 0), (1, 0, 0), (2, 0, 0), (255, 0, 1), (0, 1, 1), (0, 0, 1), (3, 0, 1)]
cases += [(rng.choice([0, 1, rng.randrange(256)]), rng.randrange(256), rng.choice([0, 1]))
          for _ in range(max(0, ncases - len(cases)))]
for i, (chosen, mode, allow) in enumerate(cases):
    flag = 1 if allow else rng.randrange(256)
    e.w8(rt(CHOSEN), chosen)
    e.w8(rt(MODE), mode)
    e.w8(rt(ALLOW), allow)
    e.w8(rt(FLAG), flag)
    e.call(rt(FN))
    got = (e.r8(rt(MODE)), e.r8(rt(CHOSEN)))
    base = bytearray(BASE_LEN)
    base[21285] = mode
    base[21286] = chosen
    b = ctypes.create_string_buffer(bytes(base), BASE_LEN)
    PORT.ja1f_allow_save_mode(b, BASE_LEN, allow)
    want = (b.raw[21285], b.raw[21286])
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: chosen {chosen} mode {mode} allow {allow} flag {flag}: "
                  f"(mode, chosen) original {got} port {want}")
print(f"allowsave_diff: {len(cases)} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
