#!/usr/bin/env python3
"""Gate test of the milestone "1 in 4 of the original game's known functions are diff-tested".

usage: .venv/bin/python validate/gate_diff_tested.py
Runs at the end of a full `./t check`, after every diff test: check.py gives each diff test JA1_EMU_COV, so the
Unicorn harness (tools/emu.py) writes the game functions of the original JA.EXE that ran for real in it (entry
executed, not replaced by a stub) to build/emu_cov/<test>.txt. This test asserts:
(check.py empties build/emu_cov at its start, so no file of an older run counts)
1. every game function whose labels.csv comment says "diff-tested" ran in a diff test of this check (a mark
   without a test behind it fails);
2. at least 1 in 4 game functions (analysis/decomp/procedures.json below the library modules, the count of
   ./t status) carry the mark.
Prints the marked count and how many unmarked functions also ran (candidates for a mark after a review).
Exits 1 on a failed step.
"""
import csv
import glob
import json
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
COV = os.path.join(ROOT, "build", "emu_cov")
LIB_START = 0x754F0          # tools/status.py: first library module
SHARE = 0.25


def share_den(share):
    return round(1 / share)      # 4 or 3: marked x den >= game, exact (339 of 1017 is 1 in 3)


def main(share=SHARE, name="gate_diff_tested"):
    procs = [int(p["address"], 16) for p in json.load(open(os.path.join(ROOT, "analysis/decomp/procedures.json")))]
    game = {a for a in procs if a < LIB_START}
    files = glob.glob(os.path.join(COV, "*.txt"))
    if not files:
        sys.exit("FAIL no coverage files in build/emu_cov (run the full ./t check)")
    ran = set()
    for f in files:
        ran |= {int(l, 16) for l in open(f) if l.strip()}
    marked = {}
    for r in csv.DictReader(open(os.path.join(ROOT, "labels.csv"), errors="replace")):
        try:
            a = int(r["addr"], 16)
        except (KeyError, ValueError, TypeError):
            continue
        if a in game and "diff-tested" in (r.get("comment") or "").lower():
            marked[a] = r["name"]
    bad = sorted(a for a in marked if a not in ran)
    for a in bad:
        print(f"  marked diff-tested, but no diff test ran it: 0x{a:X} {marked[a]}")
    print(f"{len(files)} diff test runs, {len(ran & game)} game functions ran; "
          f"{len(marked)} of {len(game)} marked diff-tested ({len(marked) / len(game):.1%}); "
          f"{len(ran & game - set(marked))} more ran unmarked")
    if bad:
        sys.exit(f"FAIL {len(bad)} marks without a diff test")
    if len(marked) * share_den(share) < len(game):
        sys.exit(f"FAIL {len(marked)} of {len(game)} = {len(marked) / len(game):.1%} < {share:.1%} of game functions diff-tested")
    print(f"{name}: {len(marked)} of {len(game)} game functions diff-tested ({len(marked) / len(game):.1%}), every mark ran")


if __name__ == "__main__":
    main()
