#!/usr/bin/env python3
"""Differential test of the stat-loss message, original vs the port (play::loss_set, loss_timer_tick, loss_pass). Parts:
  set : real HitLocationDamage 0x38D80 with scripted Random against the library's ja1_hit_location, whose host op 183
        goes to play::loss_set: g_lossSlot 0x22A1F1, g_lossTimer 0x92DE8, g_lossAmount 0x22A02F, g_lossPart 0xFA908,
        g_lossStat 0xFA918 (all 16 bytes each, the old bytes behind a shorter text too).
  stage: StageEffects? 0x6362F..0x6364A, the arm of stage 0, against op 183 (0, 0xFE, 4).
  tick: TacticalTimer? 0x5EFDE..0x5EFFD.
  pass: TacticalLoop 0x10800..0x108D8 with the real MsgMercText: timer after, the box text and its arguments, TeamQuote?.

usage: .venv/bin/python validate/diff/lossmsg_diff.py [ncases=1500] [DUMP]
Exits 1 on a mismatch or when a part does not reach its cases.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, RET_SENTINEL  # noqa: E402
import ai  # noqa: E402
import port_lib  # noqa: E402
from unicorn import UC_HOOK_CODE  # noqa: E402
from unicorn.x86_const import UC_X86_REG_EIP, UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
ncases = int(argv[0]) if argv else 1500
dump = argv[1] if len(argv) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"lossmsg_diff: {dump} missing")
    sys.exit(1)
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
ISO, EXE = (os.path.join(ROOT, *q).encode() for q in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
e = Emu(dump=dump, fast=True)
rt = e.rt
r = random.Random(0x107BC)
HIT, RANDOM, SOLD, NSOLD = 0x38D80, 0x53538, 0xFA958, 48
SLOT, TIMER, AMOUNT, PART, STAT = 0x22A1F1, 0x92DE8, 0x22A02F, 0xFA908, 0xFA918
bad = 0
seen = {}


class Io(ctypes.Structure):
    _fields_ = [("slot", ctypes.c_int), ("amount", ctypes.c_int), ("timer", ctypes.c_int),
                ("part", ctypes.c_ubyte * 16), ("stat", ctypes.c_ubyte * 16)]


def fail(part, case, text):
    global bad
    bad += 1
    if bad <= 10:
        print(f"MISMATCH {part} case {case}: {text}")


def soldiers():
    return bytes(e.uc.mem_read(rt(SOLD), NSOLD * 512))


def orig_state():
    return (e.r8(rt(SLOT)), e.r8(rt(AMOUNT)), e.r32(rt(TIMER)) & 0xFFFFFFFF,
            bytes(e.uc.mem_read(rt(PART), 16)), bytes(e.uc.mem_read(rt(STAT), 16)))


def put_state(st):
    e.w8(rt(SLOT), st[0])
    e.w8(rt(AMOUNT), st[1])
    e.w32(rt(TIMER), st[2])
    e.uc.mem_write(rt(PART), st[3])
    e.uc.mem_write(rt(STAT), st[4])


def to_io(st):
    io = Io(st[0], st[1], struct.unpack("<i", struct.pack("<I", st[2]))[0])
    io.part[:] = list(st[3])
    io.stat[:] = list(st[4])
    return io


def io_state(io):
    return (io.slot & 0xFF, io.amount & 0xFF, io.timer & 0xFFFFFFFF, bytes(io.part), bytes(io.stat))


def step(io, sol, mode, a=0, b=0, c=0):
    out = ctypes.create_string_buffer(300)
    n, ms = ctypes.c_int(0), ctypes.c_int(0)
    v = P.ja1f_loss_step(ISO, EXE, sol, ctypes.byref(io), mode, a, b, c, out, 300, ctypes.byref(n), ctypes.byref(ms))
    if v < 0:
        print(f"lossmsg_diff: ja1f_loss_step error {v}")
        sys.exit(1)
    return v, out.raw[:n.value], ms.value


def fuzz_state():
    garbage = lambda: bytes(r.randrange(256) for _ in range(16))
    t = r.choice([garbage(), b"\0" * 16, b"elbowelbowelbow\0"])
    return (r.randrange(256), r.randrange(256), r.choice([0xFFFFFFFF, 0, 1, 500, 7500]), t, garbage())


# --- set: HitLocationDamage on a merc or an enemy record
script, calls = [], []


def stub_random(em, eax, *_):
    k = eax & 0xFFFF
    k = k - 0x10000 if k >= 0x8000 else k
    v = script[len(calls)] % k if k > 0 else 0
    calls.append(k)
    return v


e.stub(rt(RANDOM), stub_random)
base = soldiers()
for case in range(ncases):
    k = r.choice([0, 1, 2, 3, 0x28, 0x29])
    rec = bytearray(base[k * 512:k * 512 + 512])
    rec[0x17A] = k
    for o, v in ((0x18, 0), (0x48, 0), (0x3C, 0)):       # no armour: the texts come from the loss itself
        if r.random() < 0.9:
            rec[o] = v
    for o in (0x18B, 0x18C, 0x18D):
        rec[o] = r.randrange(1, 101)
    rec[0x1F3] = 0
    struct.pack_into("<H", rec, 0x1E8, r.choice([0, 2, 5, 0x15, 0x39]))
    height = r.choice([5500, 5000, 6000, 3000, 2500, 800, 2000, 100])
    dmg = r.choice([10, 30, 60, 120, 200])
    script[:] = [r.randrange(0x7FFF) for _ in range(12)]
    st = fuzz_state()
    # original
    put_state(st)
    tgt = rt(SOLD) + k * 512
    e.uc.mem_write(tgt, bytes(rec))
    calls.clear()
    e.call(rt(HIT), tgt, height, dmg)
    want = orig_state()
    # library on the same start, op 183 to play::loss_set
    e.uc.mem_write(tgt, bytes(rec))
    bufs, scalars = port_lib.world_load(e, ai)
    scalars["knockdown"] = 0
    ops = []
    idx = [0]

    def prng(n):
        v = script[idx[0]] % n if n > 0 else 0
        idx[0] += 1
        return v

    def call(op, a):
        if op != "loss_msg":
            raise SystemExit(f"lossmsg_diff: host op {op} {a}")
        ops.append(tuple(a[:3]))
        return 0, None
    port_lib.ai_run("hit_location", bufs, scalars, k, prng, call, (height, dmg))
    io = to_io(st)
    sol = bytes(bufs["soldiers"])
    for a in ops:
        step(io, sol, 0, *a)
    got = io_state(io)
    key = "set%d" % (ops[-1][2] if ops else -1)
    seen[key] = seen.get(key, 0) + 1
    if got != want:
        fail("set", case, f"k {k} h {height} d {dmg} ops {ops}\n  original {want}\n  port     {got}")
    e.uc.mem_write(tgt, base[k * 512:k * 512 + 512])

# --- stage: StageEffects? 0x6362F..0x6364A, the arm of stage 0
stage_end = e.uc.hook_add(UC_HOOK_CODE, lambda uc, a, s, u: (uc.reg_write(UC_X86_REG_EIP, RET_SENTINEL), uc.emu_stop()),
                          begin=rt(0x6364A), end=rt(0x6364A))
st = fuzz_state()
put_state(st)
e.call(rt(0x6362F))
e.uc.hook_del(stage_end)
io = to_io(st)
step(io, base, 0, 0, 0xFE, 4)
if io_state(io) != orig_state():
    fail("stage", 0, f"original {orig_state()} port {io_state(io)}")
seen["stage"] = 1

# --- tick
tick_end = e.uc.hook_add(UC_HOOK_CODE, lambda uc, a, s, u: (uc.reg_write(UC_X86_REG_EIP, RET_SENTINEL), uc.emu_stop()),
                         begin=rt(0x5EFFD), end=rt(0x5EFFD))
for case, t in enumerate([0, 1, 5, 9, 10, 11, 20, 500, 7500, 0xFFFFFFFF, 0xFFFFFFF6, 0x7FFFFFFF]):
    st = (0, 0, t, b"\0" * 16, b"\0" * 16)
    put_state(st)
    e.call(rt(0x5EFDE))
    io = to_io(st)
    step(io, base, 2)
    seen["tick"] = seen.get("tick", 0) + 1
    if io.timer & 0xFFFFFFFF != e.r32(rt(TIMER)) & 0xFFFFFFFF:
        fail("tick", case, f"t {t:#x} original {e.r32(rt(TIMER)):#x} port {io.timer & 0xFFFFFFFF:#x}")
e.uc.hook_del(tick_end)

# --- pass
boxes, quotes = [], []


def show(em, eax, edx, ebx, ecx):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    boxes.append((em.cstr(eax), edx, ebx, ecx, [em.r32(sp + 4 + 4 * i) for i in range(4)]))
    return 0


e.stub(rt(0x27B98), show, 16)
e.stub(rt(0x61688), lambda em, eax, edx, ebx, *_: quotes.append((eax & 0xFF, edx)) or 0, 0)
e.stub_sprintf()
e.stubs.pop(rt(0x15910), None)
for a in (0x2AD20, 0x47020, 0x5F2B0):
    e.stub(rt(a), lambda em, *a_: 0, 0)
pass_end = e.uc.hook_add(UC_HOOK_CODE, lambda uc, a, s, u: (uc.reg_write(UC_X86_REG_EIP, RET_SENTINEL), uc.emu_stop()),
                         begin=rt(0x108D8), end=rt(0x108D8))
frame, tramp = e.alloc(16), e.alloc(16)
e.uc.mem_write(tramp, b"\xBD" + struct.pack("<I", frame + 8) + b"\xE9" + struct.pack("<i", rt(0x107BC) - (tramp + 10)))
NAMES = [b"Nick", b"Hitman", b"Grunty", b"", b"Buns", b"A", b"Ninebytes"]
TEXTS = [b"head", b"arm", b"leg", b"", b"elbow"]
for case in range(ncases):
    amount = r.choice([0, 1, 2, 5, 0x7F, 0x80, 0xFF, 0xFF, 0xFF, 0xFE, 0xFE, r.randrange(256)])
    k = r.choice([r.randrange(8), r.randrange(8), r.randrange(0x28, 0x30), 0x3F, 0x40, 0x80, 0xFF])
    if k < NSOLD:
        nm = r.choice(NAMES)
        e.uc.mem_write(rt(SOLD) + k * 512 + 0x16E, nm.ljust(9, b"\0") + b"\0")
        e.w8(rt(SOLD) + k * 512 + 0x17A, r.choice([k, k, 0, 7, 8, 0x40]))
    timer = r.choice([0, 0, 0, 0, 500, 5, 0xFFFFFFF6])
    st = (k, amount, timer, r.choice(TEXTS).ljust(16, b"\0"), r.choice(TEXTS).ljust(16, b"\0"))
    if k == 0x3F:         # the records stop at 48 in the port; the original's slots stop there too (0x17A, stage 0)
        st = (0x2F, *st[1:])
    put_state(st)
    boxes.clear()
    quotes.clear()
    e.call(tramp)
    want = orig_state()
    io = to_io(st)
    v, text, ms = step(io, soldiers(), 1)
    got = io_state(io)
    kind = "box" if boxes else "quote" if quotes else "none"
    seen["pass_" + kind] = seen.get("pass_" + kind, 0) + 1
    if boxes:
        b0 = boxes[0]
        if b0[1:4] != (0, 0, 1) or b0[4][:2] != [1, 0] or b0[4][3] != 1:
            fail("pass", case, f"box arguments {b0[1:]}")
    wv = 1 if boxes else 2 if quotes else 0
    wt = boxes[0][0] if boxes else b""
    wt = wt.encode("latin-1") if isinstance(wt, str) else wt
    wms = boxes[0][4][2] if boxes else 0
    if (v, text, ms, got) != (wv, wt, wms, want) or (quotes and quotes[0] != (0x1C, 0xE)):
        fail("pass", case, f"slot {st[0]} amount {amount} timer {timer:#x}\n  original {wv} {wt} {wms} {want} {quotes}"
                           f"\n  port     {v} {text} {ms} {got}")
e.uc.hook_del(pass_end)

for need in ("set0", "set1", "set2", "set3", "pass_box", "pass_quote", "pass_none"):
    if seen.get(need, 0) < 3:
        print(f"lossmsg_diff: part {need} reached only {seen.get(need, 0)} cases")
        bad += 1
print(f"lossmsg_diff: {sum(seen.values())} cases {seen}, {bad} mismatches")
sys.exit(1 if bad else 0)
