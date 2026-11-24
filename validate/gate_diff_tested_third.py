#!/usr/bin/env python3
"""Gate test of the milestone "1 in 3 of the original game's known functions are diff-tested".

usage: .venv/bin/python validate/gate_diff_tested_third.py
Same asserts as gate_diff_tested.py (every "diff-tested" mark in labels.csv ran in a diff test of this full check,
read from build/emu_cov), with the share 1 in 3: marked x 3 >= game functions (339 of 1017).
Exits 1 on a failed step.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gate_diff_tested  # noqa: E402

if __name__ == "__main__":
    gate_diff_tested.main(1 / 3, "gate_diff_tested_third")
