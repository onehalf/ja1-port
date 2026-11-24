#!/usr/bin/env python3
"""target113b_diff.py — port target 1.13b (#24): the game folders and the OpenCDFile reader.

The original's OpenCDFile 0x38304 opens the game-folder file first and the CD file only when it is absent; the
official 1.13b patch works that way (DAT\\XRA.DAT and DAT\\QUESTS.DAT in the game folder, a new JA.EXE). Asserts:
- validate/run/C (the original's run copy) and build/game113b (the port's game folder) hold the 1.13b JA.EXE,
  XRA.DAT and QUESTS.DAT (validate/patch113b.sh puts them there);
- common.game_read gives the 1.13b bytes of XRA.DAT (4 bytes differ from the CD) and QUESTS.DAT (204 bytes);
- common.game_read falls back to the CD for a file no game folder holds (DAT/TILES0.DAT, DAT/JAGGED.DAT).
The port's reader (ja1front iso::game_file) has its own cargo test (game_file_order).
"""
import hashlib, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools"))
from common import PATCH113B, cd_read, game_path, game_read  # noqa: E402

EXE_113B = "043a87f6806a3d3f48152d2f1492d404c5d6304255703b8c82e2b4d743a9f5d0"
DIFFS = {"DAT/XRA.DAT": 4, "DAT/QUESTS.DAT": 204}

subprocess.run([os.path.join(ROOT, "validate", "patch113b.sh")], check=True)
bad = []
for folder in ("validate/run/C", "build/game113b"):
    h = hashlib.sha256(open(os.path.join(ROOT, folder, "JA.EXE"), "rb").read()).hexdigest()
    if h != EXE_113B:
        bad.append(f"{folder}/JA.EXE sha256 {h[:16]} is not 1.13b")
    for f in DIFFS:
        if open(os.path.join(ROOT, folder, f), "rb").read() != open(os.path.join(PATCH113B, f), "rb").read():
            bad.append(f"{folder}/{f} is not the 1.13b file")
for f, n in DIFFS.items():
    a, b = game_read(f), cd_read(f)
    d = sum(x != y for x, y in zip(a, b))
    if a != open(os.path.join(PATCH113B, f), "rb").read() or len(a) != len(b) or d != n:
        bad.append(f"game_read({f}): {d} bytes differ from the CD (want {n}), length {len(a)}")
    if game_read(f.replace("/", "\\")) != a:
        bad.append(f"game_read with a backslash path ({f}) differs")
for f in ("DAT/TILES0.DAT", "DAT/JAGGED.DAT"):
    if game_path(f) is not None or game_read(f) != cd_read(f):
        bad.append(f"game_read({f}) does not fall back to the CD")
for m in bad:
    print("MISMATCH", m)
print(f"target113b_diff: {2 * 3 + 2 * 2 + 2} cases, {len(bad)} mismatches")
sys.exit(1 if bad else 0)
