"""Differential test: the traverse to a neighbour sector (soldier_update.md "Traverse to a neighbour sector").

usage: .venv/bin/python validate/diff/trav_diff.py [ncases=3000] [DUMP] [--port] [--lib SO] [-v]
The original in Unicorn on tac60, four modes per case (chosen in turn):
  edge   CanTraverseEdge 0x30FA0 (EAX soldier, DL dir): its result.
  check  TraverseCheck 0x31344 (TeamNearby? 0x311D8, the edge search 0x310D0 run for real): its result, the calls of
         ShowMercMessage? 0x299B8, StatusMessage? 0x27D24 and MsgMercText 0x15910 (stubbed, logged), and with
         result 1 the entry tile / sector / facing 0x22A474 / 0x22A472 / 0x22A47A.
  prompt the tail of TraversePrompt 0x1CBA8 (edge, then check, else "Can't traverse here!"), composed from the two.
  move   the move 0x315DC with its screen calls stubbed: the calls up to the map load (0x30D6C, RecordReplayEvent,
         the fade 0x52908), the SetMusic 0x5C5EC theme (MusicPlaying 0x5C768 answers the fuzzed `playing`) and the
         world after it.
Fuzzed: the traversing merc (slot 0..7, tile near an edge), cur_sector, dir 9..14, opt_onscreen and the view, the
other mercs 0..7 (+0x182, hp, +0x138, +0x17A, +0x17E, AP, tile near), turn_timer, combat, tile bytes 1 and 5 along
the map edges, the playing music theme, seen_enemies, first_hit, the sector table.
--port: ja1_traverse_edge, ja1_traverse_check, ja1_traverse, ja1_traverse_out + ja1_traverse_in on the same state;
results, host calls and the world are compared. Exits 1 on any mismatch.
"""
import os, random, sys
from collections import Counter
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
if "--lib" in argv:
    os.environ["JA1_PORT_LIB"] = argv[argv.index("--lib") + 1]
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
N = int(args[0]) if args else 3000
DUMP = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = "--port" in argv
VERBOSE = "-v" in argv

SOLDIERS, SECTORS, TILES = ai.G_SOLDIERS, ai.G_SECTOR_TBL9, ai.G_TILEMAP
CUR, COMBAT, SELECTED, ONSCREEN = ai.G_CUR_SECTOR, ai.G_COMBAT, ai.G_SELECTED, 0x92DDF
VIEW = 0x229CF8                      # view_x, view_y (then +0x1C4 / +0x1C8: view_w, view_h at 0x229CBC / 0x229CC0)
TURN_TIMER, SEEN, THEME, FIRST_HIT = 0x22A486, 0x22A478, 0x9635E, 0x22A1F5
OUT = 0x22A472                       # s16 sector, s16 tile (0x22A474), ..., s16 facing (0x22A47A)
TEXTS = {"Can't traverse here!": 11, "Can't traverse during an interrupt!": 12}
LIB_OPS = port_lib.ALL_OPS & ~(1 << 7)   # tile_dist runs in the library

if not os.path.exists(DUMP):
    print(f"trav_diff: {DUMP} missing")
    sys.exit(1)
e = Emu(dump=DUMP, fast=True)
rt = e.rt
log = []
S0 = rt(SOLDIERS)
# t1 values that make a tile not standable (TileStandable 0: class > 5, not a stair or step value)
BLOCK_T1 = [v for v in range(256) if e.r8(rt(ai.G_T1_CLASS) + v) > 5
            and v not in (0x3B, 0x41, 0x5A, 0x60, 0x3D, 0x3E, 0x5C, 0x5D, 0x43, 0x45, 0x62, 0x64)]


def slot_of(p):
    return (p - S0) // 0x200


def text_no(p):
    t = e.cstr(p).decode("latin-1")
    return TEXTS.get(t, t)


e.stub(rt(0x299B8), lambda em, a, d, b, c: log.append(("merc_message", slot_of(a), d & 0xFF, b & 0xFF)) and 0, pop=4)
e.stub(rt(0x27D24), lambda em, a, *_: log.append(("message", text_no(a))) and 0)
e.stub(rt(0x15910), lambda em, a, d, b, c: log.append(("merc_text", slot_of(a), e.cstr(d).decode("latin-1"), b)) and 0)
# the move's calls
s8 = lambda v: (v & 0xFF) - 256 * (v & 0x80 != 0)
e.stub(rt(0x30D6C), lambda em, a, *_: log.append(("leave_sector", a & 0xFF)) and 0)
e.stub(rt(0x3FA04), lambda em, a, d, b, c: log.append(("replay_event", s8(a), s8(d), b)) and 0)
e.stub(rt(0x52908), lambda em, a, d, b, c: log.append(("fade", d, b, c, e.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4))) and 0, pop=4)
e.stub(rt(0x2A9FC), lambda em, a, *_: log.append(("terrain", a & 0xFF)) and 0)
e.stub(rt(0x2E918), lambda em, a, d, *_: log.append(("setup", a & 0xFFFF, d & 0xFF)) and 0)
for adr in (0x3F490, 0x681D0, 0x2F928, 0x1CB78, 0x33768):
    e.stub(rt(adr), lambda em, *_: 0)
playing = [0]
e.stub(rt(0x5C768), lambda em, *_: playing[0])
e.stub(rt(0x5C724), lambda em, *_: 1)
e.stub(rt(0x5C5EC), lambda em, a, *_: log.append(("music", a)) and 0)

CARD = [(10, lambda r: r.randrange(56) + 56 * r.choice([0, 1, 2, 4, 5, 6, 9, r.randrange(56)])),
        (11, lambda r: 56 * r.randrange(56) + r.choice([55, 54, 51, 50, 49, r.randrange(56)])),
        (12, lambda r: r.randrange(56) + 56 * r.choice([55, 54, 51, 50, 49, r.randrange(56)])),
        (13, lambda r: 56 * r.randrange(56) + r.choice([0, 1, 5, 6, 9, r.randrange(56)]))]


def fuzz(r, edge_only):
    """returns (slot, dir)"""
    dir_, tile = r.choice(CARD)
    if r.random() < .05 and edge_only:   # TraverseCheck with another dir reads unset locals (the prompt passes 10..13)
        dir_ = r.choice([9, 14, 0])
    slot = r.choice([0, 0, 1, 2, 3, 4, 5, 6, 7])
    t0 = tile(r)
    e.w8(rt(CUR), r.choice([r.randrange(60), r.randrange(60), 0, 9, 10, 49, 50, 59, r.randrange(256)]))
    e.w8(rt(ONSCREEN), r.choice([0, 0, 1, 2]))
    e.w32(rt(VIEW), r.randrange(0, 0x380 - 0xE0, 16))
    e.w32(rt(VIEW) + 4, r.randrange(0, 0x310 - 0xA8, 14))
    e.w16(rt(TURN_TIMER), r.choice([0, 0, 0, -5, 3, 100]))
    e.w8(rt(COMBAT), r.choice([0, 0, 1]))
    e.w8(rt(SELECTED), r.choice([slot, slot, 8, 0x40]))
    e.w16(rt(SEEN), r.choice([0, 1, 3]))
    e.w16(rt(THEME), r.choice([0, 1, 2, 3, 4, 5, 6, 0xFFFF, r.randrange(1 << 16)]))
    e.w8(rt(FIRST_HIT), r.choice([0, 1, 2, 3, 0xFF]))
    playing[0] = r.choice([0, 1])
    tbl = bytearray(e.uc.mem_read(rt(SECTORS), 80 * 9))
    for s in range(80):
        if r.random() < .3:
            tbl[9 * s + 1] = r.choice([0, 0, 2, 5])
        if r.random() < .3:
            tbl[9 * s + 2] = r.choice([0, 1, 3, 4])
    e.uc.mem_write(rt(SECTORS), bytes(tbl))
    clean = r.random() < .5          # a squad that passes Team nearby
    near = r.choice([1, 2, 3]) if clean else r.choice([1, 2, 3, 5, 7, 8, 10])
    if clean:
        e.w16(rt(TURN_TIMER), r.choice([0, 0, -5]))
    for k in range(8):
        p = S0 + 0x200 * k
        e.w8(p + 0x17A, k if r.random() < .95 else r.randrange(8))
        e.w8(p + 0x182, 1 if k == slot or r.random() < .7 else 0)
        e.w8(p + 0x138, 1 if k == slot or r.random() < .9 else 0)
        e.w8(p + 0x183, r.choice([40, 40, 30, 20, 16, 15, 14, 12, 10, 9, 0, -3 & 0xFF]) if k != slot else r.choice([40, 20, 16]))
        e.w8(p + 0x17E, r.choice([0, 0, 0, 0, 1] if clean else [0, 0, 0, 0, 8, 1]))
        e.w8(p + 0x1A5, r.choice([20, 20, 10, 5, 4, 0]))
        if r.random() < .1:
            e.w8(p + 0x1B2, r.choice([0x10, 0x6C, 0x70]))
        if clean and k != slot and e.r8(p + 0x183) < 0x80:
            e.w8(p + 0x183, max(e.r8(p + 0x183), r.choice([15, 20, 40])))
        if k == slot:
            tk = t0
        else:
            dr, dc = r.randint(-near, near), r.randint(-near, near)
            tk = max(0, min(55, t0 // 56 + dr)) * 56 + max(0, min(55, t0 % 56 + dc))
        e.w16(p + 0x114, tk)
    # tile bytes along the edges: occupant (byte 5) and t1 (byte 1) vary standable
    for _ in range(r.choice([0, 10, 40, 120])):
        side = r.randrange(4)
        i = r.randrange(56)
        t = [i, 56 * i + 55, 0xC08 + i, 56 * i][side] if r.random() < .8 else r.randrange(3136)
        tp = rt(TILES) + 8 * t
        if r.random() < .5:
            e.w8(tp + 5, r.choice([0x40, 0x7F, r.randrange(8), 0x28 + r.randrange(8), 8 + r.randrange(32)]))
        else:
            e.w8(tp + 1, r.choice([0, 3, 0x3B, 0x3D, 0x43, 0x50, 0x73, r.randrange(256)]))
    # a whole edge not standable (BLOCK_T1, no occupant): the edge search runs off both ends (ja1-cov 2026-11-06)
    if r.random() < .08:
        side = r.randrange(4)
        for i in range(56):
            tp = rt(TILES) + 8 * [i, 56 * i + 55, 0xC08 + i, 56 * i][side]
            e.w8(tp + 1, r.choice(BLOCK_T1))
            e.w8(tp + 5, 0x40)
            if e.r8(tp + 3) == 0x73:
                e.w8(tp + 3, 0)
    return slot, dir_


def out_words():
    return [int.from_bytes(e.uc.mem_read(rt(OUT) + o, 2), "little", signed=True) for o in (2, 0, 8)]


def original(mode, slot, dir_):
    sp = S0 + 0x200 * slot
    del log[:]
    if mode != "move":
        for o in (0, 2, 8):
            e.w16(rt(OUT) + o, 0)
    if mode == "edge":
        return e.call(rt(0x30FA0), sp, dir_) & 0xFFFFFFFF, list(log), None
    if mode == "check":
        v = e.call(rt(0x31344), sp, dir_)
        v = v - (1 << 32) if v & 0x80000000 else v
        return v, list(log), out_words() if v == 1 else None
    if mode == "prompt":
        v = 0
        if e.call(rt(0x30FA0), sp, dir_) & 0xFFFFFFFF:
            v = e.call(rt(0x31344), sp, dir_)
            v = v - (1 << 32) if v & 0x80000000 else v
        if v == 0:
            log.append(("message", 11))
        return v, list(log), out_words() if v == 1 else None
    # move: a dest sector in the table (the check's sector)
    e.call(rt(0x315DC), sp)
    return None, list(log), None


def port_calls(name, bufs, scalars, args):
    plog = []

    def call(op, a):
        if op == "merc_message":
            plog.append((op, a[0], a[1], a[2]))
        elif op == "merc_text":
            plog.append((op, a[0], {10: "doesn't have enough Action Points to traverse!"}.get(a[1], a[1]), a[2]))
        elif op == "message":
            plog.append((op, a[0]))
        elif op in ("leave_sector",):
            plog.append((op, a[0]))
        elif op == "replay_event":
            plog.append((op, a[0], a[1], a[2]))
        elif op == "fade":
            plog.append((op, a[0], a[1], a[2], a[3]))
        else:
            plog.append((op,) + tuple(a))
        return 0, None
    r = port_lib.sector_run(name, bufs, scalars, args, None, call, host_ops=LIB_OPS, sync=True)
    return r, plog


def main():
    r = random.Random(7)
    snap = bytes(e.uc.mem_read(0, 16 << 20))
    fails = 0
    cover = Counter()
    modes = ["edge", "check", "prompt", "move"]
    for i in range(N):
        e.uc.mem_write(0, snap)
        mode = modes[i % 4]
        slot, dir_ = fuzz(r, mode == "edge")
        if mode == "move":
            e.w16(rt(OUT), r.choice([r.randrange(60), r.randrange(60), 0, 59]))
            e.w8(rt(CUR), r.randrange(60))   # a tactical sector (past the 80-entry table the original reads other data)
        pre = port_lib.world_load(e, ai) if PORT else None
        dest = int.from_bytes(e.uc.mem_read(rt(OUT), 2), "little", signed=True)
        res, olog, oout = original(mode, slot, dir_)
        if mode == "move":
            mus = [x[1] for x in olog if x[0] == "music"]
            cover[f"move music {mus[0] if mus else 0}"] += 1
            k = next(j for j, x in enumerate(olog) if x[0] == "terrain")
            want_log, want_m = olog[:k], (mus[0] if mus else 0)
        else:
            cover[f"{mode} {res}"] += 1
            for x in olog:
                cover[f"{mode} call {x[0]} {x[2] if x[0] == 'merc_message' else ''}"] += 1
        if not PORT:
            continue
        bufs_o, sc_o = port_lib.world_load(e, ai)
        bufs, scalars = pre
        out = bytearray(6)
        bad = []
        if mode == "move":
            theme = int.from_bytes(e.uc.mem_read(rt(THEME), 2), "little")
            _, plog = port_calls("traverse_out", bufs, scalars, [dest])
            m, plog2 = port_calls("traverse_in", bufs, scalars, [theme, playing[0]])
            if plog != want_log or plog2:
                bad.append(f"calls {want_log} vs {plog + plog2}")
            if m != want_m:
                bad.append(f"music {want_m} vs {m}")
        else:
            name = {"edge": "traverse_edge", "check": "traverse_check", "prompt": "traverse"}[mode]
            pa = [slot, dir_] + ([out] if mode != "edge" else [])
            v, plog = port_calls(name, bufs, scalars, pa)
            if v != res:
                bad.append(f"result {res} vs {v}")
            if plog != olog:
                bad.append(f"calls {olog} vs {plog}")
            if oout is not None:
                pout = [int.from_bytes(out[2 * j:2 * j + 2], "little", signed=True) for j in range(3)]
                if pout != oout:
                    bad.append(f"out {oout} vs {pout}")
        for k in bufs_o:
            if bufs_o[k] != bufs[k]:
                j = next(j for j in range(len(bufs[k])) if bufs[k][j] != bufs_o[k][j])
                bad.append(f"{k} @{j:#x} {bufs_o[k][j]:#x} vs {bufs[k][j]:#x}")
        for k in sc_o:
            if sc_o[k] != scalars[k]:
                bad.append(f"{k} {sc_o[k]} vs {scalars[k]}")
        if bad:
            fails += 1
            if fails <= 10:
                print(f"MISMATCH case {i} {mode} slot {slot} dir {dir_}: " + "; ".join(bad)[:700])
    if VERBOSE or not PORT:
        print("cover:", dict(sorted(cover.items())))
    need = ["edge 0", "edge 1", "check -1", "check 0", "check 1", "prompt 1", "prompt 0", "prompt -1",
            "check call merc_message 9", "check call merc_message 17", "check call merc_message 7",
            "check call message ", "check call merc_text ", "move music 0", "move music 1", "move music 2",
            "move music 3", "move music 4"]
    missing = [k for k in need if not cover[k]]
    if missing:
        print(f"trav_diff: never reached: {missing}")
        fails += 1
    print(f"trav_diff{' (port)' if PORT else ''}: {N} cases, {fails} mismatches")
    sys.exit(1 if fails else 0)


main()
