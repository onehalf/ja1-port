"""Differential test: the view move at the end of a merc message, original vs port (sighting::merc_message + run_view_move).

usage: .venv/bin/python validate/diff/balloonview_diff.py [ncases=1500] [DUMP] [--lib SO]
The original runs ShowMercMessage? 0x299B8 in Unicorn on tac60, with FocusOn? 0x3F724 and JumpViewToTile? 0x3F490 real.
The balloon, PlayQuote and the loader are stubbed; the stub of SpeechWait? 0x28034 writes byte 0x92F65 as the right button
does, and it records the view at the wait: the view must still be the old view there. Fuzzed: merc slot, health, how,
near, the near soldier's +0x9C, follow_all, opt_onscreen, the view, the right button. Compared: the 12 view values
after the call, the slide's steps, and the case with no balloon (health below the limit; how < 0).
Exits 1 on a mismatch, or when a kind of case did not run.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import UC_X86_REG_EBP    # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 1500
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_balloon_view.argtypes = [ctypes.c_void_p, ctypes.c_void_p] + [ctypes.c_int32] * 4 + [ctypes.c_void_p]
PORT.ja1f_balloon_view.restype = ctypes.c_int32

SOLDIERS, F65, RIPPLE = 0xFA958, 0x92F65, 0x92EF8
# The order of ja1f_balloon_view's values.
FIELDS = [(0x229CF8, 4), (0x229CFC, 4), (0x229CBC, 4), (0x229CC0, 4), (0x92DDF, 1), (0x93458, 1), (0x92E52, 1),
          (0x229CB4, 4), (0x93D54, 4), (0x229C8C, 4), (0x229C94, 4), (0x92E53, 1)]
NAMES = "view_x view_y view_w view_h onscreen follow_all focus_slot shown_slot show_timer view_tile spotted focus_last".split()

e = Emu(dump=dump, fast=True)
rt = e.rt


def s32(v):
    return v - (1 << 32) if v >> 31 else v


def tick(uc, *_):
    """cmp ebp, [ripple_timer]; jl back: while the slide waits, one timer tick per pass (focus_diff)."""
    v = s32(e.r32(rt(RIPPLE)))
    if s32(uc.reg_read(UC_X86_REG_EBP)) < v:
        e.w32(rt(RIPPLE), (v - 10) & 0xFFFFFFFF)


e.uc.hook_add(UC_HOOK_CODE, tick, None, rt(0x3F6B9), rt(0x3F6B9))
redraws = []
e.stub(rt(0x176C8), lambda em, *a: redraws.append(1) or 0)
for a_ in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x11D40, 0x67368, 0x67398, 0x67C54, 0x1CB78, 0x320A0,
           0x1D8A4, 0x681D0, 0x67478, 0x674B0, 0x467E4, 0x46EC4, 0x67250, 0x67E18, 0x67E30, 0x5C9C0):
    e.stub(rt(a_), lambda em, *_: 0)
e.stub(rt(0x24E44), lambda em, *_: 0, 4)
e.stub(rt(0x532BC), lambda em, *_: 0, 4)
e.stub(rt(0x24F08), lambda em, *_: 0, pop=16)
cur = {}


def wait(em, *_):
    cur["waits"] += 1
    cur["at_wait"] = tuple(s32(e.r32(rt(a))) for a, k in FIELDS[:2])
    e.w8(rt(F65), cur["right"])
    return 0


e.stub(rt(0x28034), wait, pop=12)
TEXT = e.alloc(b"\0" * 16)
base = bytes(e.uc.mem_read(rt(SOLDIERS), 48 * 512))


def get(a, k):
    return e.r8(rt(a)) if k == 1 else s32(e.r32(rt(a)))


def put(a, k, v):
    (e.w8 if k == 1 else e.w32)(rt(a), v & 0xFF if k == 1 else v & 0xFFFFFFFF)


r = random.Random(int(os.environ.get("BV_SEED", "5")))
bad = 0
seen = {}
for i in range(ncases):
    recs = bytearray(base)
    slot = r.randrange(8)
    near = r.choice([0x40, r.randrange(48), r.randrange(8, 48), slot, 0x7F])  # 48..63: past the records, no caller passes it
    how = r.choice([3, 8, 0, r.randrange(33), r.randrange(33), 9, -1, -1 & 0xFF])
    how = how - 256 if how > 127 else how
    hp = r.choice([40, 20, 15, 14, 12, 10, 9])
    right = r.randrange(2)
    for k in (slot, near if near < 48 else slot):
        o = k * 512
        recs[o + 0x182], recs[o + 0x138] = r.choice([1, 1, 1, 0]), r.choice([1, 1, 1, 0])
        recs[o + 0x183] = r.choice([30, 30, 30, 0])
        recs[o + 0x114:o + 0x116] = r.randrange(56 * 56).to_bytes(2, "little")
        recs[o + 0x9C] = r.choice([0, 1, 5, 0xFF])
    o = slot * 512
    recs[o + 0x17A], recs[o + 0x179], recs[o + 0x180] = slot, r.randrange(64), r.randrange(2)
    recs[o + 0x183], recs[o + 0x30], recs[o + 0x32] = hp, 0x2A, r.choice([10, 50, 50, 9])
    if near == slot:
        recs[o + 0x183] = hp
    vals = [r.randrange(0, 56) * 16, r.randrange(0, 40) * 14, 320, 112, r.randrange(2), r.randrange(2), r.choice([0x40, slot, near & 0xFF]),
            r.randrange(8), r.randrange(100), r.randrange(3000), r.randrange(48), r.randrange(48)]
    tile = r.randrange(56 * 56) if how == 3 else near
    arg = tile
    e.uc.mem_write(rt(SOLDIERS), bytes(recs))
    for (a, k), v in zip(FIELDS, vals):
        put(a, k, v)
    e.w8(rt(F65), 0)
    e.w32(rt(RIPPLE), r.randrange(0, 80))
    e.w8(rt(0x1163BD), 0)
    cur.update(right=right, waits=0, at_wait=None)
    redraws.clear()
    view0 = (vals[0], vals[1])
    e.call(rt(0x299B8), rt(SOLDIERS + o), how, arg, TEXT, 1)
    orig = [get(a, k) for a, k in FIELDS]
    pbuf = ctypes.create_string_buffer(bytes(recs))
    pv = (ctypes.c_int32 * 12)(*vals)
    slide = ctypes.c_int32(0)
    res = PORT.ja1f_balloon_view(pbuf, pv, slot, how, arg, right, ctypes.byref(slide))
    port = list(pv)
    kind = (res, right if res == 3 else 0, how == 3, orig[:2] != vals[:2])
    seen[kind] = seen.get(kind, 0) + 1
    diffs = [f"{n} {o_} port {p}" for n, o_, p in zip(NAMES, orig, port) if o_ != p]
    if (cur["waits"] > 0) != (res == 3):
        diffs.append(f"waits {cur['waits']} port result {res}")
    if cur["at_wait"] is not None and cur["at_wait"] != view0:
        diffs.append("the view moved before the wait")
    if len(redraws) != slide.value:
        diffs.append(f"slide steps {len(redraws)} port {slide.value}")
    if diffs:
        bad += 1
        if bad <= 8:
            print(f"case {i}: slot {slot} how {how} near {arg} hp {hp} right {right}: " + ", ".join(diffs[:5]))
need = [(3, 0, False, True), (3, 1, False, False), (3, 0, True, True), (1, 0, False, True)]
for k in need:
    if not seen.get(k):
        print("kind of case did not run:", k, seen)
        bad += 1
print(f"balloonview_diff: {ncases} cases, {bad} mismatches; moved with a balloon {sum(v for k, v in seen.items() if k[0] == 3 and k[3])}, "
      f"stayed after right {seen.get((3, 1, False, False), 0)}")
sys.exit(1 if bad else 0)
