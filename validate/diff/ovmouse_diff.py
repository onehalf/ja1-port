"""Differential test: the overview mouse step (panel.md "The overview mouse step"), original vs port.

usage: .venv/bin/python validate/diff/ovmouse_diff.py [ncases=3000] [DUMP] [--port] [--lib SO]
Per case: fuzzed mouse point and buttons, cursor tile, ap_timer, the selected soldier's bytes, the occupant of the
mouse tile and its hp, and the path cost. The original 0x24630 runs in Unicorn on tac60 with its calls stubbed and
logged: show mouse 0x67BF0, Hide path marks 0x1D8B8, TileOccupant 0x23944 (answers the fuzzed occupant), SelectMerc?
0x1148C, StatusMessage? 0x27D24, AllMoveTraverse? 0x1CFEC, PathApCost 0x1D32C (answers the fuzzed cost), the release
wait 0x67C68 and the overview click 0x244C8. The port (port/ja1front ja1f_ov_mouse) runs on the same values; the 17
values and the call log must match. Without --port only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_ov_mouse.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_ov_mouse.restype = ctypes.c_int32

SOLDIERS, REC, PTRS = 0xFA958, 0x200, 0x100958
SEL = 0x284F11
# OvMouse field -> (address or record offset "r", size, signed) in the port's field order
FIELDS = [(0x284F0B, 1, 0), (0x284F13, 1, 0), (0x229D00, 4, 1), (0x229D04, 4, 1), (0x229CF4, 1, 0), (0x229D2C, 4, 1),
          (0x229DD4, 4, 1), (0x92DD4, 1, 0), (0x229DEC, 4, 1), (0x92E5C, 4, 1), (SEL, 1, 0), ("r", 0x1B8, 0),
          ("r", 0x183, 1), ("r", 0x1A5, 1), ("r", 0x177, 0), (0x284F21, 1, 0), (0x284F20, 1, 0)]
NAMES = "shown mouse_on x y buttons tile ap_timer colour cost over sel 1b8 hp ap 177 shift alt".split()

e = Emu(dump=dump, fast=True)
rt = e.rt
log, ans, bad = [], [0x40, 0, 0], []
sel_now = [0]


def rec(k):
    return rt(SOLDIERS + REC * k)


def put_log(k, a):
    log.append((k, a))


def cost(em, eax, edx, ebx, ecx):
    sp = em.uc.reg_read(__import__("unicorn").x86_const.UC_X86_REG_ESP)
    st = [em.r32(sp + 4 * j) & 0xFF for j in (1, 2, 3)]
    s = sel_now[0]
    want = [em.r8(rt(0x284F21)), em.r8(rec(s) + 0x177), em.r8(rt(0x284F20))]
    if eax != rec(s) or ebx & 0xFF != 1 or ecx & 0xFF != 0 or st != want:
        bad.append(f"cost args {eax:#x} {ebx & 0xFF} {ecx & 0xFF} {st} want {want}")
    put_log(6, edx - (1 << 32) if edx >> 31 else edx)
    return ans[2]


def walk(em, eax, edx, ebx, ecx):
    if eax != rec(sel_now[0]) or ebx & 0xFF != 0:
        bad.append("walk args")
    put_log(5, edx - (1 << 32) if edx >> 31 else edx)


def select(em, eax, edx, ebx, ecx):
    if edx & 0xFF != 0 or ebx & 0xFF != 1:
        bad.append("select args")
    put_log(3, eax & 0xFF)


def status(em, eax, *r):
    if eax != rt(0x9ABFD):
        bad.append("status text")
    put_log(4, 0)


def marks(em, eax, *r):                      # Hide path marks: AP number 0, cursor tile 0xC41 first (cursor.md)
    em.w32(rt(0x229DEC), 0)
    em.w32(rt(0x229D2C), 0xC41)
    put_log(2, eax & 0xFF)


e.stub(rt(0x67BF0), lambda em, *r: put_log(1, 0))
e.stub(rt(0x1D8B8), marks)
e.stub(rt(0x23944), lambda em, *r: ans[0])
e.stub(rt(0x1148C), select)
e.stub(rt(0x27D24), status)
e.stub(rt(0x1CFEC), walk)
e.stub(rt(0x1D32C), cost, pop=12)
e.stub(rt(0x67C68), lambda em, *r: None)
e.stub(rt(0x244C8), lambda em, *r: put_log(7, 0))


def addr(f, s):
    return rec(s) + f[1] if f[0] == "r" else rt(f[0])


def size(f):
    return 1 if f[0] == "r" else f[1]


def get(f, s):
    k = size(f)
    v = e.r8(addr(f, s)) if k == 1 else e.r32(addr(f, s))
    if f[2]:
        v = v - (1 << 8 * k) if v >> (8 * k - 1) else v
    return v


def put(f, s, v):
    (e.w8 if size(f) == 1 else e.w32)(addr(f, s), v & (0xFF if size(f) == 1 else 0xFFFFFFFF))


rng = random.Random(int(os.environ.get("OVMOUSE_SEED", "1")))
fails, by_kind = 0, {}
keep = [(rt(PTRS), 4 * 8)] + [(rec(k), REC) for k in range(64)] + [
    (rt(f[0]), f[1]) for f in FIELDS if f[0] != "r"]
saved = [(a, bytes(e.uc.mem_read(a, n))) for a, n in keep]
for i in range(ncases):
    sel = rng.randrange(8)
    occ = rng.choice([0x40, 0x40, rng.randrange(8), sel, rng.randrange(8, 0x40), rng.randrange(0x40, 0x80),
                      rng.randrange(256)])
    tile0 = rng.choice([rng.randrange(3136), rng.randrange(-100, 3300), 0xC41])
    x, y = rng.randrange(48, 272), rng.randrange(14, 182)
    if rng.random() < 0.15:
        x, y = rng.randrange(-100, 400), rng.randrange(-100, 300)
    if rng.random() < 0.3:                    # often the mouse stays on the cursor tile
        tile0 = min(int((y - 14) / 3) * 56 + int((x - 48) / 4), 3135)
    vals = [rng.choice([1, 1, 0, rng.randrange(256)]), rng.choice([0, 1]), x, y,
            rng.choice([0, 0, 0, 1, 2, 3, rng.randrange(256)]), tile0,
            rng.choice([0, 10, 20, 800, rng.randrange(-50, 900), rng.randrange(-(1 << 31), 1 << 31)]),
            rng.randrange(256), rng.randrange(-5, 100), rng.choice([1, 0xFF, rng.randrange(256)]), sel,
            rng.choice([1, 1, 0, rng.randrange(256)]), rng.choice([30, 15, 14, 0, rng.randrange(-128, 128)]),
            rng.choice([rng.randrange(0, 40), rng.randrange(-128, 128)]), rng.randrange(256), rng.choice([0, 1]),
            rng.choice([0, 1])]
    ans[:] = [occ, 0, rng.choice([0, 5, 20, 40, rng.randrange(-10, 200)])]
    if occ < 64:
        e.w8(rec(occ) + 0x183, rng.choice([30, 15, 14, 0, rng.randrange(256)]))
    e.w32(rt(PTRS + 4 * sel), rec(sel))
    for f, v in zip(FIELDS, vals):
        put(f, sel, v)
    if occ < 64:
        b = e.r8(rec(occ) + 0x183)
        ans[1] = b - 256 if b > 127 else b
    sel_now[0] = sel
    log.clear()
    bad.clear()
    e.call(rt(0x24630))
    got = [get(f, sel) for f in FIELDS]
    olog = list(log)
    kind = "left" if vals[4] & 1 else "right" if vals[4] & 2 else "hover"
    by_kind[kind] = by_kind.get(kind, 0) + 1
    if PORT:
        v = (ctypes.c_int32 * 17)()
        for j, f in enumerate(FIELDS):
            if size(f) == 1:
                v[j] = vals[j] & 0xFF if not f[2] else ((vals[j] & 0xFF) ^ 0x80) - 0x80
            else:
                v[j] = vals[j]
        a = (ctypes.c_int32 * 3)(*ans)
        lg = (ctypes.c_int32 * 32)()
        n = PORT.ja1f_ov_mouse(ctypes.addressof(v), ctypes.addressof(a), ctypes.addressof(lg))
        plog = [(lg[2 * j], lg[2 * j + 1]) for j in range(n)]
        want = list(v)
        for j, f in enumerate(FIELDS):
            if size(f) == 1:
                want[j] = want[j] & 0xFF if not f[2] else ((want[j] & 0xFF) ^ 0x80) - 0x80
        if got != want:
            bad.append("values")
        if olog != plog:
            bad.append(f"calls {olog} port {plog}")
        if bad:
            fails += 1
            if fails <= 10:
                diff = {nm: (g, w) for nm, g, w in zip(NAMES, got, want) if g != w}
                print(f"MISMATCH case {i}: in {vals} ans {ans} {bad}\n  (original, port): {diff}")
    elif bad:
        fails += 1
        print(f"HARNESS case {i}: {bad}")
for a, b in saved:
    e.uc.mem_write(a, b)
print(f"ovmouse_diff: {ncases} cases ({', '.join(f'{k} {n}' for k, n in sorted(by_kind.items()))})"
      f"{'' if PORT else ' (original only)'}; {fails} mismatches")
sys.exit(1 if fails else 0)
