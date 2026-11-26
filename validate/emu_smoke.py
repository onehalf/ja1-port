"""Smoke test for tools/emu.py: run one original function and check its effect. .venv/bin/python validate/emu_smoke.py"""
import sys; import os; sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tools"))
from emu import Emu
e = Emu(); e.w32(0x339368 + 12, 7)
r = e.call(0x7C149, 3, 9, cdecl=True)
assert r == 7 and e.r32(0x339368 + 12) == 9, (r, e.r32(0x339368 + 12))
print("ok: swap-slot fn returned old value 7, stored 9;", e.count, "insns")
# #70: the fan patch 1.15m exe with its tail code past obj1's vsize: the data-address helper 0x8AC7D (new code)
# turns K 0x2710ED into 0x22A049 (third_party/ja115m/README.md). Skipped without third_party/ja115m.
from le import EXES
if os.path.exists(EXES["115m"]):
    e = Emu(ver="115m")
    r = e.call(0x8AC7D, 0x2710ED)
    assert r == 0x22A049, hex(r)
    print("ok: 1.15m tail helper 0x8AC7D(0x2710ED) = 0x22A049")
