#!/usr/bin/env python3
"""Differential test: the questions when the player leaves the A.I.M. screen, AimScreen 0x20F2E..0x210FD vs aim.rs.

usage: .venv/bin/python validate/diff/aimleave_diff.py [ncases=2000] [DUMP] [--lib SO]
The original's exit branch runs in Unicorn. Stubs: ConfirmDialog? 0x275E4 (the answers of the case), StatusMessage? 0x27D24,
AimCancelPending 0x20060, ConcatTextAndNumbers? 0x36C5C (gives the charge in the text), AddFunds 0x514A4, RestoreSaveArea
0x28698, time 0x7576A. Compared: the list of
steps ("cannot afford", ask terminate, ask charge, pay, cancel, leave or stay) and the charge in the question. A case has 0..8
mercs with byte +0x19D = 1, other values in that byte, a balance below, at and above the charge (also negative), the difficulty
and two answers. aim_diff tests the paid path after yes; this test tests which path runs. The port: aim::leave_first and
ASK_AFTER_HELI_NO through aim::leave_route; Aim::leave and Aim::answer use the same rules in the window.
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai  # noqa: E402
from emu import Emu, EmuStop, STACK_TOP  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 2000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"aimleave_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
e = Emu(dump=dump, fast=True)
rt = e.rt
TERMINATE, BALANCE, DIFF, CHARGES = 0x975B7, 0x229FC4, 0x22A0CC, 0x95C28
SOL = ai.G_SOLDIERS
st = {}


def confirm(em, a, *_):
    a &= 0xFFFFFFFF
    st["log"].append(2 if a == rt(TERMINATE) else 3 if a == rt(0xEBAC0) else 99)
    ans = st["answers"]
    v = ans[st["n"]] if st["n"] < len(ans) else 0
    st["n"] += 1
    return v


def pay(em, a, *_):
    if 4 not in st["log"]:
        st["log"].append(4)
    em.w32(rt(BALANCE), (em.r32(rt(BALANCE)) + a) & 0xFFFFFFFF)


def concat(em, a, d, *_):
    st["charge"] = d if d < 0x80000000 else d - (1 << 32)


e.stub(rt(0x275E4), confirm, 12)
e.stub(rt(0x27D24), lambda em, *_: st["log"].append(1) or 0)
e.stub(rt(0x20060), lambda em, *_: st["log"].append(5) or 0)
e.stub(rt(0x36C5C), concat, 20)
e.stub(rt(0x514A4), pay)
for a in (0x28698, 0x7576A):   # paid path, tested by aim_diff: the save area, the C library's time
    e.stub(rt(a), lambda em, *_: 0)
e.uc.mem_write(rt(0x210FD), b"\xCC")
rnd = random.Random(0x20F2E)
base = bytes(e.uc.mem_read(rt(SOL), 8 * 0x200))
fails = checks = 0
seen = {}
for case in range(ncases):
    e.uc.mem_write(rt(SOL), base)
    n = rnd.choice((0, 0, 1, 1, 2, 3, 8))
    hired = rnd.sample(range(8), n)
    for k in range(8):
        e.w8(rt(SOL + 0x200 * k + 0x19D), 1 if k in hired else rnd.choice((0, 0, 2, 0xFF)))
    d = rnd.randrange(3)
    e.w8(rt(DIFF), d)
    h = e.r16(rt(CHARGES + 2 + 2 * d))
    bal = rnd.choice((h - 1, h, h + 1, 0, -1, -h, h * 2, rnd.randrange(-100000, 100000)))
    e.w32(rt(BALANCE), bal & 0xFFFFFFFF)
    answers = [rnd.randrange(2), rnd.randrange(2)]
    st.update(log=[], answers=answers, n=0, charge=None)
    sp = STACK_TOP - 0x104
    e.w8(sp + 0x8C, 1)                    # "the player leaves" (the branch runs only with this byte set)
    e.w32(sp + 0x68, rt(0x229FA0 + 0xA3))  # the pointer that the paid path reads, as in aim_diff
    sb = bytes(e.uc.mem_read(rt(SOL), 8 * 0x200))   # before the call: the paid path changes +0x19D
    stay = None
    try:
        e.call(rt(0x20F2E), max_steps=200_000)
    except EmuStop as stop:
        if "INT 0x03" in str(stop):
            stay = e.r8(e.uc.reg_read(UC_X86_REG_ESP) + 0x8C) == 0
        elif fails < 3:
            print(f"original stopped: {stop}")
    want = list(st["log"]) + [None if stay is None else 7 if stay else 6]
    out = (ctypes.c_int32 * 8)()
    cnt = PORT.ja1f_aim_leave(sb, bal, d, bytes(answers), out)
    got = list(out[:max(cnt, 0)])
    checks += 1
    key = ",".join(str(x) for x in want)
    seen[key] = seen.get(key, 0) + 1
    charge_ok = st["charge"] is None or st["charge"] == h
    if got != want or not charge_ok or (3 in want) != (st["charge"] is not None):
        fails += 1
        if fails <= 12:
            print(f"MISMATCH case {case}: hired {n} balance {bal} charge {h} difficulty {d} answers {answers}: "
                  f"original {want} port {got} text charge {st['charge']}")

print("routes (1 cannot afford, 2 terminate, 3 charge, 4 pay, 5 cancel, 6 leave, 7 stay): "
      + "; ".join(f"{k} x{v}" for k, v in sorted(seen.items())))
print(f"CASES aimleave={checks}")
print(f"aimleave_diff (port): {checks} checks, {fails} mismatches")
sys.exit(1 if fails else 0)
