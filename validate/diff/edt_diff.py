"""Differential test: tools/speech.py text decoding vs the original WrapNextWord 0x36184 in Unicorn.

usage: .venv/bin/python validate/diff/edt_diff.py
Every record of every rom/EDT/*QUOTE.EDT is split into words by the original (encrypted flag set, font 0) and by
speech.decode(); words and their order must match. Exits 1 on any mismatch.
"""
import glob, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import speech                        # noqa: E402
from emu import Emu                  # noqa: E402

WRAP_NEXT_WORD = 0x36184
e = Emu()
src, word, pos = e.alloc(256), e.alloc(256), e.alloc(4)
fails = n = 0
for path in sorted(glob.glob(os.path.join(HERE, "..", "..", "rom", "EDT", "*QUOTE.EDT"))):
    d = open(path, "rb").read()
    for r in range(len(d) // speech.REC):
        rec = d[r * speech.REC:(r + 1) * speech.REC]
        raw = rec.split(b"\0")[0]
        e.uc.mem_write(src, raw + b"\0")
        e.w32(pos, 0)
        got = []
        while e.r8(src + e.r32(pos)) != 0:
            e.call(WRAP_NEXT_WORD, pos, src, word, 0, 1)
            w = bytes(e.uc.mem_read(word, 256)).split(b"\0")[0]
            if w:
                got.append(w)
        want = [w for w in speech.decode(rec).split(b" ") if w]
        n += 1
        if got != want:
            fails += 1
            if fails <= 5:
                print(f"{os.path.basename(path)} #{r}: original {got[:6]} vs speech.py {want[:6]}")
print(f"edt_diff: {n - fails}/{n} records identical")
sys.exit(1 if fails else 0)
