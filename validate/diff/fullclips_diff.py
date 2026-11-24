"""Differential test for the Ctrl+R debug key body (cursor.md "Debug keys (level 4)" 1, full clips): the original's arm
of TacticalKeyHandler 0x13AED..0x13B39 in Unicorn vs the port's debugkeys::full_clips (ja1f_full_clips).

usage: .venv/bin/python validate/diff/fullclips_diff.py [DUMP] [n=600] [--lib SO]
The arm runs from 0x13AED (behind the level-4 test) over the 8 records at the pointer 0x100958 until it jumps to the
common tail 0x1409B (stubbed: return). Per case the 8 records get fuzzed hand item type (+0x00), count (+0x01) and the
three gate bytes +0x182, +0x183, +0x138; the item table 0x94DF8 (136 x 12 bytes) gets fuzzed bytes 5 (clip).
Compared: the 8 records and the item table behind the call.
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
dump = next((a for a in args if not a.isdigit()), os.path.join(ROOT, "build", "mem_tac60.bin"))
n = int(next((a for a in args if a.isdigit()), "600"))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_full_clips.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
PORT.ja1f_full_clips.restype = ctypes.c_int32

ARM, TAIL, RECS_PTR, ITEMS = 0x13AED, 0x1409B, 0x100958, 0x94DF8
e = Emu(dump=dump, fast=True)
e.stub(e.rt(TAIL), lambda em, *a: 0)
recs_at = e.r32(e.rt(RECS_PTR))
items_at = e.rt(ITEMS)
base_recs = bytes(e.uc.mem_read(recs_at, 8 * 512))
base_items = bytes(e.uc.mem_read(items_at, 136 * 12))
rng = random.Random(0x13ADC)
fails = changed = 0
for i in range(n):
    recs = bytearray(base_recs)
    items = bytearray(base_items)
    for k in range(8):
        r = 512 * k
        recs[r] = rng.choice([0, 1, 14, 15, rng.randrange(1, 15), rng.randrange(1, 15), rng.randrange(256)])
        recs[r + 1] = rng.randrange(256)
        for off in (0x182, 0x183, 0x138):
            recs[r + off] = rng.choice([0, 1, 1, 1, rng.randrange(256)])
    for t in range(136):
        items[12 * t + 5] = rng.randrange(256)
    e.uc.mem_write(recs_at, bytes(recs))
    e.uc.mem_write(items_at, bytes(items))
    e.call(e.rt(ARM))
    got = bytes(e.uc.mem_read(recs_at, 8 * 512))
    got_items = bytes(e.uc.mem_read(items_at, 136 * 12))
    buf = ctypes.create_string_buffer(bytes(recs), 8 * 512)
    rc = PORT.ja1f_full_clips(buf, bytes(items))
    pr = buf.raw[:8 * 512]
    changed += got != bytes(recs)
    if rc != 0 or got != pr or got_items != bytes(items):
        fails += 1
        if fails <= 8:
            diff = " ".join(f"{k % 512:#x}@{k // 512}={got[k]:#x}/{pr[k]:#x}" for k in range(len(got)) if got[k] != pr[k])
            print(f"MISMATCH case {i}: rc {rc} {diff[:200]}" + (" (item table changed)" if got_items != bytes(items) else ""))
print(f"fullclips_diff: {n} cases ({changed} with a changed clip), {fails} mismatches")
sys.exit(1 if fails or changed < n // 4 else 0)
