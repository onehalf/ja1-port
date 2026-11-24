"""Differential test: host op 99 focus, the view goes to a soldier (world_load.md section 8 "Focus"), original vs port.

usage: .venv/bin/python validate/diff/focus_diff.py [ncases=3000] [DUMP] [--port] [--lib SO]
Per case: a fuzzed soldier record (in-play bytes, +0x9C, tile) and fuzzed view globals (view x / y / w / h,
opt_onscreen, follow_all, focus_slot, shown_slot, show_timer), slot and flag. The original runs FocusOn 0x3F724 in
Unicorn on tac60 (TacticalRedraw 0x176C8 and the mouse calls 0x67368, 0x67E18, 0x67E30, 0x67398 stubbed). A fuzzed
ripple_timer 0x92EF8: a hook on the slide's timer wait at 0x3F6B9 plays one 10 ms timer tick (ripple_timer - 10) per
pass while it waits. The port (port/ja1front ja1f_focus) runs on the same values; the 9 values must match, and for a
slide (world_load.md "The slide in time") its steps, timer ticks, the view at each redraw and ripple_timer = 50 after. Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import UC_X86_REG_EBP    # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_focus.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
                                ctypes.c_void_p]
    PORT.ja1f_focus.restype = None

G_SOLDIERS = 0xFA958
# View field -> (address, size): the order of the port's View struct
FIELDS = [(0x229CF8, 4), (0x229CFC, 4), (0x229CBC, 4), (0x229CC0, 4), (0x92DDF, 1), (0x93458, 1), (0x92E52, 1),
          (0x229CB4, 4), (0x93D54, 4)]

e = Emu(dump=dump, fast=True)
rt = e.rt
RIPPLE = 0x92EF8
ticks = [0]


def tick(uc, *_):
    """cmp ebp, [ripple_timer]; jl back: while it waits, one timer tick per pass."""
    v = s32(e.r32(rt(RIPPLE)))
    if s32(uc.reg_read(UC_X86_REG_EBP)) < v:
        e.w32(rt(RIPPLE), (v - 10) & 0xFFFFFFFF)
        ticks[0] += 1


e.uc.hook_add(UC_HOOK_CODE, tick, None, rt(0x3F6B9), rt(0x3F6B9))
redraws = []
e.stub(rt(0x176C8), lambda em, *a: redraws.append((s32(e.r32(rt(FIELDS[0][0]))), s32(e.r32(rt(FIELDS[1][0]))))))
for a_ in (0x67368, 0x67E18, 0x67E30, 0x67398):
    e.stub(rt(a_), lambda em, *a: 0)


def get(a, k):
    return e.r8(rt(a)) if k == 1 else e.r32(rt(a))


def put(a, k, v):
    (e.w8 if k == 1 else e.w32)(rt(a), v)


def s32(v):
    return v - (1 << 32) if v >> 31 else v


rng = random.Random(int(os.environ.get("FOCUS_SEED", "1")))
fails, moved, slides = 0, 0, 0
save = [get(a, k) for a, k in FIELDS]
for i in range(ncases):
    slot = rng.choice([rng.randrange(8), rng.randrange(8, 48), rng.randrange(0x28, 48)])
    sp = rt(G_SOLDIERS) + 0x200 * slot
    for o in (0x182, 0x183, 0x138):
        e.w8(sp + o, rng.choice([1, 1, 1, 1, 1, 1, 0, 0x50]))
    e.w8(sp + 0x9C, rng.choice([0, 1, 1, 5, 0xFF, 0x80]))
    t = rng.choice([rng.randrange(3136)] * 6 + [0, 55, 3135, 3080])
    e.w16(sp + 0x114, t)
    vw = rng.choice([224, 224, 224, 320, 320, rng.randrange(0x400)])
    vh = rng.choice([168, 168, 182, 182, rng.randrange(0x380)])
    vx = rng.choice([16 * rng.randrange(43), 16 * rng.randrange(43), rng.randrange(-100, 1000)])
    vy = rng.choice([14 * rng.randrange(45), 14 * rng.randrange(45), rng.randrange(-100, 900)])
    if rng.random() < 0.3:              # the tile on the view's left or top edge (the slide and the jump differ there)
        vx, vy = rng.choice([((t % 56) * 16, vy), (vx, (t // 56) * 14), ((t % 56) * 16, (t // 56) * 14)])
    vals = [vx & 0xFFFFFFFF, vy & 0xFFFFFFFF, vw, vh, rng.choice([0, 1, 1, 1, 2, 3]), rng.choice([0, 0, 1]),
            rng.choice([slot, slot, 0x40, rng.randrange(48)]), rng.choice([slot, 0x40, rng.randrange(48)]),
            rng.randrange(1 << 32)]
    flag = rng.choice([0, 0, 1, 0x100])
    for (a, k), v in zip(FIELDS, vals):
        put(a, k, v)
    ripple = rng.choice([-20, 0, 0, 5, 10, 30, 50, 120, 135, rng.randrange(-100, 300)])
    e.w32(rt(RIPPLE), ripple & 0xFFFFFFFF)
    redraws.clear()
    ticks[0] = 0
    e.call(rt(0x3F724), slot, flag)
    ripple_after = s32(e.r32(rt(RIPPLE)))
    got = [get(a, k) for a, k in FIELDS]
    moved += got[:2] != vals[:2]
    slides += bool(redraws)
    if PORT:
        view = (ctypes.c_int32 * 9)(*[s32(v) if k == 4 else v for v, (a, k) in zip(vals, FIELDS)])
        rec = bytes(e.uc.mem_read(sp, 0x200))
        sl = (ctypes.c_int32 * 104)()
        PORT.ja1f_focus(ctypes.addressof(view), rec, slot, flag, ripple, sl)
        want = [view[j] & (0xFF if k == 1 else 0xFFFFFFFF) for j, (a, k) in enumerate(FIELDS)]
        steps = [(sl[2 * j], sl[2 * j + 1]) for j in range(1, sl[0] + 1)]
        if redraws and (steps != redraws or sl[1] != ticks[0] or ripple_after != 50):
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} (slide, ripple {ripple}): original {len(redraws)} steps {ticks[0]} ticks"
                      f" ripple after {ripple_after}, port {sl[0]} steps {sl[1]} ticks; steps differ at"
                      f" {next((j for j, (a, b) in enumerate(zip(redraws, steps)) if a != b), min(len(redraws), len(steps)))}")
        elif not redraws and sl[0]:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i}: the port slides {sl[0]} steps, the original does not")
        if got != want:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i}: slot {slot} flag {flag:#x} in {[s32(v) for v in vals]}\n"
                      f"  original {[s32(v) for v in got]}\n  port     {[s32(v) for v in want]}")
for (a, k), v in zip(FIELDS, save):
    put(a, k, v)
print(f"focus_diff: {ncases} cases ({moved} view moves, {slides} slides){'' if PORT else ' (original only)'}; "
      f"{fails} mismatches")
sys.exit(1 if fails else 0)
