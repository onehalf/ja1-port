#!/usr/bin/env python3
"""Gate test of the milestone "1 in 2 of the original game's known functions are diff-tested".

usage: .venv/bin/python validate/gate_diff_tested_half.py
Same asserts as gate_diff_tested.py (every "diff-tested" mark in labels.csv ran in a diff test of this full check,
read from build/emu_cov), with the share 1 in 2: marked x 2 >= game functions (509 of 1017).
Exits 1 on a failed step.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gate_diff_tested  # noqa: E402

if __name__ == "__main__":
    gate_diff_tested.main(1 / 2, "gate_diff_tested_half")
