"""Test of the stub fast path and the coverage hook in tools/emu.py.

usage: .venv/bin/python validate/diff/emureg_diff.py

reg_get / reg_set against Unicorn's public reg_read / reg_write: same values. A stub
gets the four argument registers, sets EAX (32 bits), returns to the caller and pops its bytes. Coverage: an entry
counts only when it runs for real; a later real run after a stub counts, also through e.stubs edits without stub(),
in stub -> real -> stub order, and in dump mode (relocated entries). Runs on the small library function 0x7C149 as a
stand-in game function, with its own coverage file, so the check's coverage gates do not see it. Exits 1 on any mismatch.
"""
import os
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path[:0] = [os.path.join(ROOT, "tools")]
os.environ["JA1_EMU_COV"] = os.path.join(tempfile.mkdtemp(), "cov.txt")
import emu                           # noqa: E402
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP  # noqa: E402

FN = 0x7C149                         # swap-slot: (slot, new) -> old (validate/emu_smoke.py)
SLOTS = 0x339368
DUMP = os.path.join(ROOT, "build", "mem_tac60.bin")
emu._game_functions = lambda: [FN]
bad, cases = 0, 0


def expect(what, got, want):
    global bad, cases
    cases += 1
    if got != want:
        bad += 1
        print(f"FAIL {what}: {got!r} != {want!r}")


def registers():
    uc = emu.Emu(fast=True).uc
    for reg in (UC_X86_REG_EAX, UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESP):
        for v in (0, 1, 0x7FFFFFFF, 0x80000000, 0xFFFFFFFF):
            uc.reg_write(reg, v)
            expect(f"reg_get {reg} {v:#x}", emu.reg_get(uc, reg), uc.reg_read(reg))
            emu.reg_set(uc, reg, v ^ 0xFFFFFFFF)
            expect(f"reg_set {reg} {v:#x}", uc.reg_read(reg), v ^ 0xFFFFFFFF)


def stub_call():
    for fast in (True, False):
        e = emu.Emu(fast=fast)
        seen = []
        e.stub(FN, lambda em, a, d, b, c: seen.append((a, d, b, c)) or -1, pop=8)
        got = e.call(FN, 0xFFFFFFFF, 2, 0x80000000, 4, 0x11, 0x22)   # args 5, 6 on the stack: the stub pops them
        expect(f"fast={fast} stub arguments", seen, [(0xFFFFFFFF, 2, 0x80000000, 4)])
        expect(f"fast={fast} stub result to 32 bits", got, 0xFFFFFFFF)
        expect(f"fast={fast} stack after the pop", e.uc.reg_read(UC_X86_REG_ESP), emu.STACK_TOP - 0x100)


def real_works(e, a):
    e.w32(e.rt(SLOTS + 12), 7)
    return e.call(a, 3, 9, cdecl=True) == 7 and e.r32(e.rt(SLOTS + 12)) == 9


def coverage(dump):
    tag = "dump" if dump else "image"

    def fresh():
        emu.COV_SEEN.clear()
        return emu.Emu(dump=dump, fast=True)

    e = fresh()
    a = e.rt(FN)
    e.stub(a, lambda *_: 5)
    expect(f"{tag} stub runs", e.call(a, 3, 9, cdecl=True), 5)
    expect(f"{tag} stubbed entry does not count", FN in emu.COV_SEEN, False)
    expect(f"{tag} stubbed entry waits", e.cov_wait.get(a), FN)
    e.call(a, 3, 9, cdecl=True)
    expect(f"{tag} second stub call does not count", FN in emu.COV_SEEN, False)
    del e.stubs[a]
    expect(f"{tag} real run after the stub", real_works(e, a), True)
    expect(f"{tag} real run counts", FN in emu.COV_SEEN, True)
    expect(f"{tag} wait is empty", e.cov_wait, {})
    e.stub(a, lambda *_: 6)
    expect(f"{tag} stub again", e.call(a, 3, 9, cdecl=True), 6)
    expect(f"{tag} still counted", FN in emu.COV_SEEN, True)

    e = fresh()
    a = e.rt(FN)
    expect(f"{tag} real first", real_works(e, a), True)
    expect(f"{tag} real first counts", FN in emu.COV_SEEN, True)
    e.stub(a, lambda *_: 5)
    expect(f"{tag} then stub", e.call(a, 3, 9, cdecl=True), 5)

    e = fresh()
    a = e.rt(FN)
    e.stub(a, lambda *_: 5)
    e.call(a, 3, 9, cdecl=True)
    fn_pop = e.stubs.pop(a)
    e.stubs[a] = fn_pop                  # direct edits, as ai_diff.py does
    e.call(a, 3, 9, cdecl=True)
    expect(f"{tag} direct edit: still not counted", FN in emu.COV_SEEN, False)
    e.stubs.pop(a)
    expect(f"{tag} direct edit: real run", real_works(e, a), True)
    expect(f"{tag} direct edit: counts", FN in emu.COV_SEEN, True)

    e = fresh()                          # an entry in e.stubs with no stub() hook runs for real, uncounted, as before
    a = e.rt(FN)
    e.stubs[a] = (lambda *_: 5, 0)
    expect(f"{tag} no hook: runs for real", real_works(e, a), True)
    expect(f"{tag} no hook: not counted", FN in emu.COV_SEEN, False)
    expect(f"{tag} no hook: no wait", e.cov_wait, {})
    del e.stubs[a]
    expect(f"{tag} no hook: real run", real_works(e, a), True)
    expect(f"{tag} no hook: counts", FN in emu.COV_SEEN, True)


def main():
    registers()
    stub_call()
    coverage(None)
    coverage(DUMP)
    print(f"emureg_diff: {cases} cases, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
