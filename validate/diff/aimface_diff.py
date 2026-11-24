#!/usr/bin/env python3
"""Differential test: a left click on P's face in the A.I.M. screen (AimScreen 0x204B3 .. 0x20E31), original vs port.

usage: .venv/bin/python validate/diff/aimface_diff.py [ncases=600] [DUMP] [--lib SO]
The original runs in Unicorn from the balance test 0x204B3 (balance <= 0: nothing) to an int3 at 0x20E31, with page mode
M = 1..5 (M = 0 is the interview: aim_diff). The jump table 0x2010C: M 1 AimAlreadyHiredMsg? 0x2306C, M 2 salary x 2,
+0x4C = 100 and AimHire(P, 10), M 3 AddFunds?(-3 x signed word +0x3E, 1), +0x3E = 0, then MercRefusalReason 0x219CC
(real): 0 = AimHire(P, 10), else PlayQuote(P, 13), +0x3D = 1 and AimShowMercBio? 0x21648 (its page-mode rule:
+0x2A = 0 -> 4, +0x4B = 0 -> 0, else 1). M 4 and 5: nothing. Stubbed and logged: AimAlreadyHiredMsg?, AimHire 0x22014,
PlayQuote 0x24F08, AddFunds? 0x514A4 (applied to g_balance), AimShowMercBio?. Stubbed: WaitLeftUp 0x67C54, HideTooltip
0x23420, AimQuoteBox? 0x28FB8, RestoreSaveArea0 0x28698, PlayMercVoice 0x24D14 and the pointer / team-face drawing of
MercRefusalReason.
The port: aim::face_click_test (ja1f_aim_face_click) = Aim::click on (30, 40) with X = 3 x debt from the interview, a
funds count run to its end. Compared: the call list (msg P / quote P q / hire P), the balance, the profile bytes
+0x28/+0x29/+0x3D/+0x3E/+0x3F/+0x4C of P, the page mode after the click (not after a hire: AimHire is stubbed; aim_diff
tests it). The debt stays below 0x8000 (the original's sar is signed; the port reads it unsigned at the interview).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 600
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"aimface_diff: {dump} missing")
    sys.exit(1)
ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
PORT.ja1f_aim_face_click.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_char_p, ctypes.c_void_p,
                                     ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
e = Emu(dump=dump, fast=True)
rt = e.rt
PROFILES, PSIZE, SOL, ACC, BALANCE, MODE, PSEL, SUBS = 0xEBB40, 0x5A, 0xFA958, 0x229FA0, 0x229FC4, 0x95F6C, 0x22A044, 0x1163BD
FIELDS = (0x28, 0x29, 0x3D, 0x3E, 0x3F, 0x4C)
st = {}


def s32(v):
    v &= 0xFFFFFFFF
    return v - (1 << 32) if v & 0x80000000 else v


def log(tag, quote=False):
    def f(em, a, d=0, b=0, *_):
        st["log"].append((tag, a & 0xFF, (b if quote else d) & 0xFF))
        return 0
    return f


def add_funds(em, a, d, *_):
    a = s32(a)
    st["funds"].append(a)
    if not ((d & 0xFF) == 1 and a >= 0):
        em.w32(rt(BALANCE), (em.r32(rt(BALANCE)) + a) & 0xFFFFFFFF)


def bio(em, a, *_):
    o = rt(PROFILES + PSIZE * (a & 0xFF))
    st["log"].append(("bio", a & 0xFF, 0))
    em.w8(rt(MODE), 4 if em.r8(o + 0x2A) == 0 else 0 if em.r8(o + 0x4B) == 0 else 1)
    return 0


e.stub(rt(0x2306C), log("msg"))
e.stub(rt(0x22014), log("hire"))
e.stub(rt(0x24F08), log("quote", True), 16)   # EAX P, EDX 14 P (the row), EBX q
e.stub(rt(0x514A4), add_funds)
e.stub(rt(0x21648), bio)
e.stub(rt(0x28FB8), lambda em, *_: 0, 4)
e.stub(rt(0x24D14), lambda em, *_: 0, 4)
for a in (0x67C54, 0x23420, 0x28698, 0x67BF0, 0x21198, 0x673AC):
    e.stub(rt(a), lambda em, *_: 0)
e.uc.mem_write(rt(0x20E31), b"\xCC")

rnd = random.Random(0x204B3)
pbase = bytes(e.uc.mem_read(rt(PROFILES), 60 * PSIZE))
sbase = bytes(e.uc.mem_read(rt(SOL), 8 * 0x200))
fails = checks = 0
seen = {}


def norm_port(lines):
    out = []
    for ln in lines:
        w = ln.split()
        if ln.startswith("aim msg status "):
            out.append(("msg", int(w[3])))
        elif ln.startswith(("aim quote ", "aim talk ")):
            out.append(("quote", int(w[2]), int(w[3])))
        elif ln.startswith("aim hire "):
            out.append(("hire", int(w[2])))
        elif ln.startswith("aim msg "):
            out.append(("box", ln[8:]))
    return out


def norm_orig(lg):
    out = []
    for tag, a, d in lg:
        if tag == "msg":
            out.append(("msg", a))
        elif tag == "quote":
            out.append(("quote", a, d))
        elif tag == "hire":
            out += [("quote", a, d), ("hire", a)]
    return out


for case in range(ncases):
    e.uc.mem_write(rt(PROFILES), pbase)
    e.uc.mem_write(rt(SOL), sbase)
    p = rnd.randrange(60)
    o = rt(PROFILES + PSIZE * p)
    m = rnd.choice((1, 2, 3, 3, 3, 3, 4, 5))
    debt = rnd.choice((0, 1, 7, 100, 333, rnd.randrange(0x8000), 0x7FFF))
    e.w16(o + 0x3E, debt)
    e.w8(o + 0x3D, rnd.choice((0, 0, 1)))
    if rnd.random() < 0.4:
        e.w8(o + 0x2A, rnd.choice((0, 1, 50, 100)))
    if rnd.random() < 0.5:
        e.w8(o + 0x4B, rnd.choice((0, 0, 1, 2, 3)))
    if rnd.random() < 0.3:
        e.w16(o + 0x28, rnd.choice((0, 1, 500, 0x7FFF, 0x8000, 0xFFFF)))
    for k in range(8):
        if rnd.random() < 0.3:
            e.w8(rt(SOL + 0x200 * k + 0x182), rnd.randrange(2))
    x = 3 * debt
    bal = rnd.choice((0, -5, 1, x - 1, x, x + 1, x * 2, rnd.randrange(-1000, 200000)))
    e.w32(rt(BALANCE), bal & 0xFFFFFFFF)
    e.w8(rt(PSEL), p)
    e.w8(rt(MODE), m)
    sub = rnd.randrange(2)
    e.w8(rt(SUBS), sub)
    prof = bytearray(e.uc.mem_read(rt(PROFILES), 60 * PSIZE))
    sol = bytes(e.uc.mem_read(rt(SOL), 8 * 0x200))
    acc = bytearray(e.uc.mem_read(rt(ACC), 0xA8))
    st.update(log=[], funds=[])
    try:
        e.call(rt(0x204B3), max_steps=2_000_000)
        if fails < 3:
            print(f"case {case}: original returned without the int3")
    except EmuStop as stop:
        if "INT 0x03" not in str(stop) and fails < 3:
            print(f"case {case}: original stopped: {stop}")
    want_calls = norm_orig(st["log"])
    want_bal = s32(e.r32(rt(BALANCE)))
    want_f = [e.r8(o + f) for f in FIELDS]
    want_mode = e.r8(rt(MODE))
    hired = any(t == "hire" for t, *_ in st["log"])

    pb = (ctypes.c_uint8 * len(prof)).from_buffer(prof)
    ab = (ctypes.c_uint8 * len(acc)).from_buffer(acc)
    buf = ctypes.create_string_buffer(8192)
    got_mode = PORT.ja1f_aim_face_click(ISO, EXE, pb, sol, ab, m, sub, buf, len(buf))
    lines = buf.value.decode("latin-1").splitlines()
    got_calls = [c for c in norm_port(lines) if c[0] != "box"]
    got_bal = s32(int.from_bytes(bytes(acc[0x24:0x28]), "little"))
    got_f = [prof[PSIZE * p + f] for f in FIELDS]
    checks += 1
    route = ("none" if bal <= 0 else f"m{m}:" + "+".join(c[0] for c in want_calls)) or "-"
    seen[route] = seen.get(route, 0) + 1
    errs = []
    if got_mode < 0:
        errs.append(f"port error {got_mode}")
    if got_calls != want_calls:
        errs.append(f"calls original {want_calls} port {got_calls}")
    if got_bal != want_bal:
        errs.append(f"balance original {want_bal} port {got_bal}")
    if got_f != want_f:
        errs.append(f"fields original {want_f} port {got_f}")
    if not hired and got_mode != want_mode:
        errs.append(f"mode original {want_mode} port {got_mode}")
    if errs:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {case}: P {p} M {m} debt {debt} balance {bal}: " + "; ".join(errs))

print("routes: " + "; ".join(f"{k} x{v}" for k, v in sorted(seen.items())))
print(f"CASES aimface={checks}")
print(f"aimface_diff (port): {checks} checks, {fails} mismatches")
sys.exit(1 if fails else 0)
