"""Smoke test for tools/emu.py: run one original function and check its effect. .venv/bin/python validate/emu_smoke.py"""
import sys; import os; sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tools"))
from emu import Emu
e = Emu(); e.w32(0x339368 + 12, 7)
r = e.call(0x7C149, 3, 9, cdecl=True)
assert r == 7 and e.r32(0x339368 + 12) == 9, (r, e.r32(0x339368 + 12))
print("ok: swap-slot fn returned old value 7, stored 9;", e.count, "insns")
