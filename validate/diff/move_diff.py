"""Differential test: tools/combat.py move_code / step_ap_cost vs the original MoveCode 0x2BE54 and StepApCost 0x39B30.

usage: .venv/bin/python validate/diff/move_diff.py [DUMP] [nfuzz=20000] [--port]
--port compares the clean-room port (port/ja1logic via validate/diff/port_lib.py) instead of tools/combat.py.
StepApCost: every move code the builder produces plus random u16 codes x Shift / +0x177 / Alt flag values.
MoveCode: every tile of all 60 TERRAIN.DAT sectors (with random previous codes) plus fuzzed t0/t1/t3 bytes, run on
a memory snapshot (the class tables come from JAGGED.DAT at run time). Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import combat                        # noqa: E402
import sectormap as sm               # noqa: E402
from emu import Emu                  # noqa: E402

PORT = "--port" in sys.argv
if PORT:
    import port_lib                  # noqa: E402
    mirror = port_lib
else:
    mirror = combat

MOVE_CODE, STEP_COST, G_TILEMAP = 0x2BE54, 0x39B30, 0x100A18
args = [a for a in sys.argv[1:] if not a.isdigit() and a != "--port"]
nfuzz = int(([a for a in sys.argv[1:] if a.isdigit()] or ["20000"])[0])
dump = args[0] if args else os.path.join(HERE, "..", "..", "build", "mem_tac60.bin")
rng = random.Random(3)
e = Emu(dump=dump)
rt = e.rt
fails = 0


def check(what, got, want, detail):
    global fails
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH {what}: original {got} port {want} | {detail}")


codes = sorted(combat.STEP_EXTRA) + [combat.MOVE_WALL, 0, 999, 1001, 1199, 1202, 1499, 1599, 1999, 2001, 3599, 4999,
                                     5001, 25299, 25401, 25499, 25501, 65535]
nstep = 0
for code in codes + [rng.randrange(0x10000) for _ in range(300)]:
    for flags in [(a, b, c) for a in (0, 1) for b in (0, 1) for c in (0, 1)] + [(rng.randrange(-3, 3), 1, 0)]:
        tile = rng.randrange(3136)
        e.w16(rt(combat.G_MOVECODE) + 2 * tile, code)
        got = combat.s8(e.call(rt(STEP_COST), tile, flags[0] & 0xFFFFFFFF, flags[2], flags[1]) & 0xFF)
        want = mirror.step_ap_cost(code, flags[0], flags[1], flags[2])
        nstep += 1
        check("StepApCost", got, want, f"code {code} shift {flags[0]} b177 {flags[1]} alt {flags[2]}")

t0c = bytes(e.uc.mem_read(rt(combat.G_T0CLASS), 256))
t1c = bytes(e.uc.mem_read(rt(combat.G_T1CLASS), 256))
olds = [0, 1000, 1200, 25300, 25400, 25500, 3600]
nmove = 0


def move_case(tile, t0, t1, t3, old):
    global nmove
    a = rt(G_TILEMAP) + 8 * tile
    e.w8(a, t0); e.w8(a + 1, t1); e.w8(a + 3, t3)
    e.w16(rt(combat.G_MOVECODE) + 2 * tile, old)
    e.call(rt(MOVE_CODE), tile)
    got = e.r16(rt(combat.G_MOVECODE) + 2 * tile)
    want = mirror.move_code(t0, t1, t3, old, t0c, t1c)
    nmove += 1
    check("MoveCode", got, want, f"t0 {t0:#x} t1 {t1:#x} t3 {t3:#x} old {old}")


for sector in range(1, 61):
    tiles, _ = sm.terrain(sector)
    for i, t in enumerate(tiles):
        move_case(i, t[0], t[1], t[3], rng.choice(olds))
for _ in range(nfuzz):
    move_case(rng.randrange(3136), rng.randrange(157), rng.choice([0, 0, rng.randrange(256)]),
              rng.choice([0, 0x73, rng.randrange(256)]), rng.choice(olds + [rng.randrange(0x10000)]))
print(f"move_diff{' (port)' if PORT else ''}: {nstep} StepApCost + {nmove} MoveCode cases (60 sectors + fuzz), {fails} mismatches")
sys.exit(1 if fails else 0)
