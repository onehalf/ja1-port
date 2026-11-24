"""Differential test for the Ctrl+T debug key body (cursor.md "Debug keys (level 4)" 5): the original's arm of
TacticalKeyHandler 0x13C98..0x13CE3 in Unicorn vs the port's ja1_teleport (port/ja1logic update.rs).

usage: .venv/bin/python validate/diff/teleport_diff.py [DUMP] [n=600]
The original arm runs from 0x13C98 (behind the level-4 test) with EDI = tile T and ESI = record S, until it jumps to
the common tail 0x1409B: TileToY 0x15E6C / TileToX 0x15E50 / PlaceAt 0x3F8A4 (VacateTile, ViewTile, StopAnim,
OccupyTile, the tile bytes and their classes, all real; StopAnim's RedrawMercPortrait? 0x336B4 = the port's host op
step_sound, stubbed and compared), then TurnSightings 0x11E2C and NextTurn 0x12D58 (stubbed,
logged). ReplaceSoldier 0x2FB98 is stubbed and logged: the port's caller (play.rs debug_key) runs
ja1_replace_soldier, ja1_turn_sightings and ja1_next_turn itself behind ja1_teleport.
Compared: all 48 soldier records and the whole tile map behind the call, and that the original made exactly the
calls ReplaceSoldier (S), TurnSightings (1) and, with the turn flag 0x92DD0 set, NextTurn (1) with the kept merc
byte 0x284F0F = selected when selected < 8 (the rule play.rs copies).
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu, RET_SENTINEL    # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

ARM, TAIL = 0x13C98, 0x1409B
REPLACE, SIGHTINGS, NEXT_TURN, PORTRAIT = 0x2FB98, 0x11E2C, 0x12D58, 0x336B4
TURN_FLAG, KEPT = 0x92DD0, 0x284F0F
args = [a for a in sys.argv[1:] if not a.isdigit()]
n = int(([a for a in sys.argv[1:] if a.isdigit()] or ["600"])[0])
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
e = Emu(dump=dump, fast=True)
bufs, scalars = port_lib.world_load(e, ai)
soldiers, tiles = bufs["soldiers"], bufs["tiles"]
base_recs = bytes(soldiers)
base_tiles = bytes(tiles)
rng = random.Random(0x13C98)
log = []
e.stub(e.rt(REPLACE), lambda em, eax, *a: log.append(("replace", eax)) or 0)
e.stub(e.rt(SIGHTINGS), lambda em, eax, *a: log.append(("sightings", eax & 0xFF)) or 0)
e.stub(e.rt(NEXT_TURN), lambda em, eax, *a: log.append(("next_turn", eax & 0xFF)) or 0)
e.stub(e.rt(PORTRAIT), lambda em, eax, edx, *a: log.append(("step_sound", eax & 0xFF, edx & 0xFF)) or 0)


def tail(em, *a):                    # the arm ends at the common tail: return to the sentinel
    em.w32(em.uc.reg_read(UC_X86_REG_ESP), RET_SENTINEL)
    return 0


e.stub(e.rt(TAIL), tail)
# the trampoline reads T and S from data (Unicorn keeps translated code: a rewritten trampoline would run stale)
targs = e.alloc(8)
tramp = e.alloc(b"\x8B\x3D" + struct.pack("<I", targs) + b"\x8B\x35" + struct.pack("<I", targs + 4)
                + b"\xB8" + struct.pack("<I", e.rt(ARM)) + b"\xFF\xE0")
sold_addr, tile_addr, sel_addr = e.rt(ai.G_SOLDIERS), e.rt(ai.G_TILEMAP), e.rt(ai.G_SELECTED)
fails = 0
for i in range(n):
    slot = rng.choice([rng.randrange(8), rng.randrange(8), rng.randrange(48)])
    sel = rng.choice([slot, slot, rng.randrange(8), 8, 0x20])
    t = rng.randrange(3136)
    flag = rng.choice([0, 1])
    recs = bytearray(base_recs)
    tl = bytearray(base_tiles)
    r = slot * 0x200
    old = rng.randrange(3136)
    recs[r + 0x114:r + 0x116] = struct.pack("<H", old)
    recs[r + 0x17A] = slot
    for off in (0x12A, 0x124, 0x1F6, 0x1A6, 0x95, 0x96, 0x97, 0x134, 0x135, 0x116, 0x118, 0x120, 0x126, 0x128,
                0x1B2, 0x1B3, 0x1B4, 0x1B5, 0x1B6, 0x1B7):
        recs[r + off] = rng.randrange(256)
    for k in rng.sample(range(3136), 40) + [old, t]:
        tl[8 * k + 5] = rng.choice([0, 0x40, 0xFF, slot, slot | 0x80, rng.randrange(256)])
        b = rng.choice([0, 1, 2])          # t0 < 157: g_t0Class holds 157 entries (JAGGED.DAT); past it lie globals
        tl[8 * k + b] = rng.randrange(157 if b == 0 else 256)
    rec_addr = sold_addr + r
    # the original
    e.uc.mem_write(sold_addr, bytes(recs))
    e.uc.mem_write(tile_addr, bytes(tl))
    e.w8(sel_addr, sel)
    e.w8(e.rt(TURN_FLAG), flag)
    e.w8(e.rt(KEPT), 0x77)
    e.w32(targs, t)
    e.w32(targs + 4, rec_addr)
    log.clear()
    e.call(tramp)
    got_recs = bytes(e.uc.mem_read(sold_addr, len(recs)))
    got_tiles = bytes(e.uc.mem_read(tile_addr, len(tl)))
    got_log, got_kept = list(log), e.r8(e.rt(KEPT))
    want_log = [("replace", rec_addr), ("sightings", 1)] + ([("next_turn", 1)] if flag else [])
    pcalls = []
    want_kept = sel if flag and sel < 8 else 0x77
    # the port
    soldiers[:] = recs
    tiles[:] = tl
    scalars["selected"] = sel

    def call(op, a):
        pcalls.append((op, a[0] & 0xFF, a[1] & 0xFF))
        return 0, None
    port_lib.ai_run("teleport", bufs, scalars, slot, lambda k: 0, call, t)
    bad = []
    if got_recs != bytes(soldiers):
        bad.append("record " + " ".join(hex(k % 0x200) + "@" + str(k // 0x200) + f"={got_recs[k]:#x}/{soldiers[k]:#x}" for k in range(len(recs))
                                        if got_recs[k] != soldiers[k])[:120])
    if got_tiles != bytes(tiles):
        bad.append("tiles " + " ".join(f"{k // 8}.{k % 8}:{got_tiles[k]:#x}/{tiles[k]:#x}" for k in range(len(tl))
                                       if got_tiles[k] != tiles[k])[:120])
    olog = [c for c in got_log if c[0] == "step_sound"]
    rest = [c for c in got_log if c[0] != "step_sound"]
    if olog != pcalls:
        bad.append(f"StopAnim calls {olog} / port {pcalls}")
    if rest != want_log or got_kept != want_kept:
        bad.append(f"calls {rest} kept {got_kept:#x} (want {want_log} kept {want_kept:#x})")
    if bad:
        fails += 1
        if fails <= 8:
            print(f"MISMATCH case {i} slot {slot} selected {sel:#x} tile {t} old {old}: {'; '.join(bad)}")
print(f"teleport_diff: {n} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
