"""Differential test: ToggleMessage 0x3F994 (panel.md "Toggle switch", strategic.md Key T), original vs port.

usage: .venv/bin/python validate/diff/toggle_diff.py [ncases=4000] [DUMP]

Per case: fuzzed word bytes (the callers' words such as "Enemies" / "Show Roofs " plus empty, one byte, odd and even
lengths, up to 27 ASCII bytes: the game's words are ASCII literals) and switch byte values. Words stay at 27 bytes or less: the original builds the text in a
32-byte stack buffer, so a longer word (27 + " off" + NUL = 32) would overwrite its return address (no caller passes one). The original runs in Unicorn on tac60 with StatusMessage_ 0x27D24
stubbed (the text is captured): it flips the switch byte at EDX and builds the word + " on" / " off" from
0x9A04F / 0x9A069. The port (ja1f_toggle_text = play::toggle_text) gets the same word and switch; the switch byte
and the text must match. The switch byte is fuzzed 0..255: the original xors 1 into it (2 -> 3, 0xFF -> 0xFE).

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
PORT.ja1f_toggle_text.argtypes = [ctypes.c_char_p, ctypes.POINTER(ctypes.c_uint8), ctypes.c_char_p,
                                  ctypes.c_int32]
PORT.ja1f_toggle_text.restype = ctypes.c_int32

TOGGLE, STATUS = 0x3F994, 0x27D24
WORDS = (b"Enemies", b"Show Roofs ", b"Text ", b"Subtitles", b"", b"A", b"ab", b"x" * 19, b"key T", b"y" * 23, b"z" * 24, b"w" * 25,
         b"v" * 27)

if not os.path.exists(dump):
    print(f"toggle_diff: {dump} missing")
    sys.exit(1)
e = Emu(dump=dump, fast=True)
rt = e.rt
seen = []


def status(emu, eax, *_):
    seen.append(bytes(emu.cstr(eax)))
    return 0


e.stub(rt(STATUS), status)

rng = random.Random(int(os.environ.get("TOGGLE_SEED", "1")))
fails = 0
buf = e.alloc(256)          # the word + NUL
sw = e.alloc(16)            # the switch byte
for i in range(ncases):
    w = rng.choice(WORDS)
    if rng.random() < .2:
        w = bytes(rng.randrange(1, 128) for _ in range(rng.randrange(0, 28)))
    word = w + b"\0"
    e.uc.mem_write(buf, word)
    sw0 = rng.choice([0, 1, 0xFF, 2, 3, rng.randrange(256)])
    e.w8(sw, sw0)
    seen.clear()
    e.call(rt(TOGGLE), buf, sw)
    got = (e.r8(sw), seen[0] if seen else None)
    out = ctypes.create_string_buffer(4096)
    s = ctypes.c_uint8(sw0)
    n = PORT.ja1f_toggle_text(word, ctypes.byref(s), out, 4096)
    want = (s.value, out.value[:n])
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: word {w!r} switch {sw0}: (switch, text) original {got} port {want}")
print(f"toggle_diff: {ncases} cases; {fails} mismatches")
sys.exit(1 if fails else 0)
