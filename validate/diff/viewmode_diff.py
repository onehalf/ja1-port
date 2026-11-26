"""Differential test: the tactical view modes (panel.md "View modes (keys PgUp, PgDn, Insert, Tab)"), original vs port.

usage: .venv/bin/python validate/diff/viewmode_diff.py [ncases=4000] [DUMP] [--port] [--lib SO] [--fan 45]
--fan 45 (#70 F45, row 80): the original runs the 1.15m Insert branch bytes, the port fan fix F45.
--pfix 15 (#71, #2 row 33): the original with the P15 patch (0x249E4 `je` gone, cave at 0x24B64: the overview holds
mode-1 coordinates), the port at fix level `all`.
Per case: fuzzed view values (mode, view x / y, window, small panel, last wide mode, overview filter, mouse) and an
action: Set mode 0x248E4 (m) with a fuzzed signed byte m, or one key branch of TacticalKeyHandler (PgUp 0x1386A,
PgDn 0x13891, Insert 0x137C8, Tab 0x13F7B; the run ends at the handler's common tail 0x1409B). The original runs in
Unicorn on tac60 with path marks 0x1D8B8, the strips 0x2F928 and the mouse set 0x681D0 stubbed (counted; the strips'
arguments and the mouse point recorded). The port (port/ja1front ja1f_view_mode) runs on the same values; all 22
values must match. Without --port only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import pfix                          # noqa: E402

argv = sys.argv[1:]
PFIX = pfix.parse(argv)
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--fan"))]
FAN = "--fan" in argv
ncases = int(args[0]) if args else 4000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_view_mode.argtypes = [ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32]
    PORT.ja1f_view_mode.restype = ctypes.c_int32
    PORT.ja1_fan_fixes_set.argtypes = [ctypes.c_uint64]
    PORT.ja1_fan_fixes_set((1 << 44) if FAN else 0)

# ViewMode field -> (address, size, signed) in the port's field order; None = an out value of the stubs
FIELDS = [(0x92DDF, 1, 0), (0x229CF8, 4, 1), (0x229CFC, 4, 1), (0x229CC8, 4, 1), (0x229CD0, 4, 1), (0x229CB8, 4, 1),
          (0x229CC4, 4, 1), (0x92E30, 4, 1), (0x92F8A, 1, 1), (0x92DB9, 1, 0), (0x92E34, 1, 0), (0x284F0E, 1, 0),
          (0x93448, 1, 0), (0x92DC1, 1, 0), (0x284F13, 1, 0), (0x229D00, 4, 1), (0x229D04, 4, 1),
          None, None, None, (0x284F14, 1, 0), (0x92E36, 1, 0)]
VIEW_W, VIEW_H = 0x229CBC, 0x229CC0          # written by the window routine too: checked against cols / rows
KEYS = {1: 0x1386A, 2: 0x13891, 3: 0x137C8, 4: 0x13F7B, 5: 0x244C8}
CURSOR_TILE = 0x229D2C                        # the overview click's cursor tile (the port returns it)

e = Emu(dump=dump, fast=True)
rt = e.rt
calls = {"marks": [], "strips": [], "mouse": []}
e.stub(rt(0x1D8B8), lambda em, a, *r: calls["marks"].append(a & 0xFF))
e.stub(rt(0x2F928), lambda em, a, d, *r: calls["strips"].append((a, d)))
e.stub(rt(0x681D0), lambda em, a, d, *r: calls["mouse"].append((a, d)))
e.stub(rt(0x1409B), lambda em, *a: 0)         # the key handler's common tail: end of a key run
if FAN:                                       # #70 F45: the 1.15m Insert branch (0x137D0 jump, 0x137E0..0x137F0)
    e.put_115m(only=[(0x137C8, 0x137F1)])
if PFIX:
    class _FrontFixes:                        # the port side is libja1front: its own copy of the fix masks
        @staticmethod
        def port_fixes(m):
            if PORT:
                PORT.ja1_port_fixes_set.argtypes = [ctypes.c_uint64]
                PORT.ja1_port_fixes_set(m)
    pfix.apply(e, PFIX, _FrontFixes)


def get(f):
    a, k, sg = f
    v = e.r8(rt(a)) if k == 1 else e.r32(rt(a))
    if sg:
        v = v - (1 << 8 * k) if v >> (8 * k - 1) else v
    return v


def put(f, v):
    a, k, _ = f
    (e.w8 if k == 1 else e.w32)(rt(a), v)


rng = random.Random(int(os.environ.get("VIEWMODE_SEED", "1")))
fails, by_kind = 0, {}
save = {f[0]: (f[1], e.uc.mem_read(rt(f[0]), f[1])) for f in FIELDS if f} | {
    VIEW_W: (4, e.uc.mem_read(rt(VIEW_W), 4)), VIEW_H: (4, e.uc.mem_read(rt(VIEW_H), 4))}
for i in range(ncases):
    mode = rng.choice([0, 1, 1, 2, 3, rng.randrange(256)] if rng.random() < 0.1 else [0, 1, 2, 3])
    cols, rows, left, top = rng.choice([(14, 12, 48, 14), (20, 12, 0, 14), (20, 14, 0, 0), (20, 13, 0, 0)])
    if rng.random() < 0.05:
        cols, rows, left, top = rng.randrange(-5, 40), rng.randrange(-5, 40), rng.randrange(-50, 100), rng.randrange(-20, 50)
    vals = [mode, rng.choice([16 * rng.randrange(57), rng.randrange(-200, 1100)]),
            rng.choice([14 * rng.randrange(57), rng.randrange(-200, 1000)]), cols, rows, left, top,
            rng.choice([0, 0, 1, rng.randrange(-3, 3)]), rng.choice([0, 1, rng.randrange(-128, 128)]),
            rng.randrange(256), rng.choice([0, 1]), rng.randrange(256), rng.randrange(256), rng.choice([0, 1, 2, 0xFF]),
            rng.choice([1, 1, 1, 0]), rng.choice([rng.randrange(320), rng.randrange(-50, 400)]),
            rng.choice([rng.randrange(200), rng.randrange(-50, 300)]), 0, 0, 0, 0, rng.choice([0, 1, rng.randrange(256)])]
    kind = rng.choice([0, 0, 0, 1, 2, 3, 4, 5])
    if kind == 5:                             # the overview click 0x244C8 (mode 0)
        vals[0] = 0
    m = rng.choice([0, 1, 2, 3, mode, rng.randrange(-128, 128)])
    for f, v in zip(FIELDS, vals):
        if f:
            put(f, v)
    e.w32(rt(VIEW_W), cols * 16)
    e.w32(rt(VIEW_H), rows * 14)
    e.w32(rt(0x229CDC), top - 1 + rows * 14)  # the window's bottom row and right column, as the window routine sets them
    e.w32(rt(0x229CD8), left - 1 + cols * 16)
    for c in calls.values():
        c.clear()
    e.w32(rt(CURSOR_TILE), 0)
    e.call(rt(0x248E4), m & 0xFF) if kind == 0 else e.call(rt(KEYS[kind]))
    tile = e.r32(rt(CURSOR_TILE))
    got = [get(f) if f else 0 for f in FIELDS]
    got[17], got[18], got[19] = len(calls["mouse"]), len(calls["marks"]), len(calls["strips"])
    if calls["mouse"]:
        got[15], got[16] = [c - (1 << 32) if c >> 31 else c for c in calls["mouse"][-1]]
    bad = [] if all(s == (1, 0) for s in calls["strips"]) and all(k == 0 for k in calls["marks"]) else ["stub args"]
    if (e.r32(rt(VIEW_W)), e.r32(rt(VIEW_H))) != ((got[3] * 16) & 0xFFFFFFFF, (got[4] * 14) & 0xFFFFFFFF):
        bad.append("view_w/h")
    by_kind[kind] = by_kind.get(kind, 0) + 1
    if PORT:
        v = (ctypes.c_int32 * 22)(*vals)
        # the port reads byte fields as bytes: hand it the bytes the original read
        for j, f in enumerate(FIELDS):
            if f and f[1] == 1:
                v[j] = vals[j] & 0xFF if not f[2] else ((vals[j] & 0xFF) ^ 0x80) - 0x80
        v[20] = 0
        ptile = PORT.ja1f_view_mode(ctypes.addressof(v), kind, m)
        if kind == 5 and ptile != tile:
            bad.append(f"cursor tile {tile} port {ptile}")
        want = list(v)
        for j, f in enumerate(FIELDS):
            if f and f[1] == 1:
                want[j] = want[j] & 0xFF if not f[2] else ((want[j] & 0xFF) ^ 0x80) - 0x80
        if got != want:
            bad.append("values")
        if bad:
            fails += 1
            if fails <= 10:
                names = "mode vx vy cols rows left top small lastw old full frame pmoved filter mon mx my mmoves marks strips redraw strip".split()
                diff = {n: (g, w) for n, g, w in zip(names, got, want) if g != w}
                print(f"MISMATCH case {i}: kind {kind} m {m} in {vals[:17]} {bad}\n  (original, port): {diff}")
    elif bad:
        fails += 1
        print(f"HARNESS case {i}: {bad}")
for a, (k, b) in save.items():
    e.uc.mem_write(rt(a), bytes(b))
print(f"viewmode_diff{' --fan 45' if FAN else ''}{' --pfix ' + ','.join(map(str, PFIX)) if PFIX else ''}: {ncases} cases (set {by_kind.get(0, 0)}, PgUp {by_kind.get(1, 0)}, PgDn {by_kind.get(2, 0)}, "
      f"Insert {by_kind.get(3, 0)}, Tab {by_kind.get(4, 0)}, overview click {by_kind.get(5, 0)}){'' if PORT else ' (original only)'}; {fails} mismatches")
sys.exit(1 if fails else 0)
