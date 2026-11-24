"""Differential test: CursorModeSet? 0x1C438 (cursor.md "Mode cursor"), original vs port.

usage: .venv/bin/python validate/diff/cursormode_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
The original 0x1C438 runs whole in Unicorn on a dump: the mode setter 0x284F1B (a mode above 2 becomes 1), then
MoveCursorState 0x1C464 with its HidePathMarks? 0x1D8B8 and WeaponCursor? 0x1C39C. The port side is
cursor::click_panel (port/ja1front ja1f_cursor_mode_set) on the library world built from the same dump.
Per case: a fuzzed soldier (hp 0x183, team slot 0x17A, hand weapon type +0, condition +2, water bytes
0x1B2/0x1B4, anim index 0x1F3 and the anim word 0x1E8..), a fuzzed item table entry for that weapon (kind +2,
the two kind bytes +7 / +9) and the mode (0..255); both the mode byte 0x284F1B and the cursor value 0x284F1D
start from the same noise. Compared: mode and value after the call. Without --port only the original runs
(a smoke test of the harness). Every 3rd case comes from its own stream: all conditions of the mode pass but one,
which stands at its edge. Half of those enter at MoveCursorState 0x1C464 with the mode byte as it stands (mode 3
comes only that way: key handler 0x14112, no setter). Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
u8, i8, i16, i32 = ctypes.c_uint8, ctypes.c_int8, ctypes.c_int16, ctypes.c_int32


class CursorState(ctypes.Structure):
    _fields_ = [("over_map", u8), ("tactical_on", u8), ("overlay_up", u8), ("value", u8), ("mode", u8),
                ("out_of_range", u8), ("combat", u8), ("ap_fg", u8), ("ap_shadow", u8), ("blink_phase", u8),
                ("no_blink", u8), ("under_soldier", u8), ("wobble_dir", i8), ("pad", u8 * 3), ("tile", i32),
                ("ap", i32), ("blink_timer", i32), ("wobble", i32), ("wobble_dx", i16), ("wobble_dy", i16),
                ("label_x", i16), ("label_y", i16), ("label", u8 * 16),
                ("marker_kind", i32), ("marker_tile", i32), ("marker_life", i32), ("marker_blink", i32),
                ("marker_phase", u8), ("pad2", u8 * 3),
                ("shown_slot", i32), ("spotted", i32), ("view_tile", i32), ("mark_timer", i32),
                ("mark_phase", i8), ("pad3", u8 * 3)]


PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_cursor_mode_set.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host),
                                          ctypes.POINTER(CursorState), i32, i32]
    PORT.ja1f_cursor_mode_set.restype = i32
    PORT.ja1f_cursor_move_state.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host),
                                            ctypes.POINTER(CursorState), i32]
    PORT.ja1f_cursor_move_state.restype = i32

SOLDIERS, MODE, VALUE = 0xFA958, 0x284F1B, 0x284F1D
REC, NSOL = 0x200, 0x30
ITEM = ai.G_ITEM_KIND - 2
ITEM_N = 136 * 12

e = Emu(dump=dump, fast=True)
base_sol = bytes(e.uc.mem_read(e.rt(SOLDIERS), NSOL * REC))
base_item = bytes(e.uc.mem_read(e.rt(ITEM), ITEM_N))


def rd(a):
    return e.uc.mem_read(e.rt(a), 1)[0]


def wr(a, v):
    e.uc.mem_write(e.rt(a), bytes([v & 0xFF]))


def port_call(slot, mode, noise, direct=False, apn=0, tilen=0):
    bufs, sc = port_lib.world_load(e, ai)
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = port_lib.World(**{k: ctypes.cast(a, port_lib.u8p) for k, a in arrays.items()}, **sc)
    host = port_lib.Host(None, port_lib.RANDOM_FN(lambda _c, k: 0), port_lib.CALL_FN(lambda *_a: 0), 0)
    st = CursorState()
    st.mode = noise
    st.value = noise
    st.ap, st.tile = apn, tilen
    if direct:
        st.mode = mode
        rc = PORT.ja1f_cursor_move_state(ctypes.byref(w), ctypes.byref(host), ctypes.byref(st), slot)
    else:
        rc = PORT.ja1f_cursor_mode_set(ctypes.byref(w), ctypes.byref(host), ctypes.byref(st), slot, mode)
    if rc < 0:
        raise SystemExit("port call failed: %d" % rc)
    return (rc >> 8) & 0xFF, rc & 0xFF, st.ap, st.tile


MODE_POOL = [0, 1, 2, 3, 4, 0x40, 0x7F, 0x80, 0xFF]
HP_POOL = [0, 1, 14, 15, -1, 0x7F, -0x80, 0x40]
SLOTB_POOL = [0, 1, 6, 7, 8, 9, 0xFF, 0x40]


def edge_case(r, sol, it, rec, ty):
    """All conditions of a mode pass; one term stands at its edge. Returns (mode, direct entry, noise)."""
    ent = 12 * ty
    sol[rec + 0x183], sol[rec + 2] = r.choice([15, 16, 60]), r.choice([10, 11, 90])
    sol[rec + 0x1B2], sol[rec + 0x1B4] = 0, 0
    sol[rec + 0x1F3] = 0
    sol[rec + 0x1E8:rec + 0x1EA] = (r.choice([4, 5, 0x100, 0xFFFF])).to_bytes(2, "little")
    it[ent + 2] = r.choice([0x74, 0x77, 0x92, 0x95])
    it[ent + 7], it[ent + 9] = 0, 0
    kind = r.randrange(9)
    if kind == 0:
        sol[rec + 0x183] = r.choice([13, 14, 15, 16])
    elif kind == 1:
        sol[rec + 0x1B2] = r.choice([0x6B, 0x6C, 0x6D, 0x70])
        sol[rec + 0x1B4] = r.choice([0, 0x1B, 0x1C, 0x1D, 0x1E, 0x1F, 0x20])
    elif kind == 2:
        sol[rec + 0x1E8:rec + 0x1EA] = (r.choice([2, 3, 4])).to_bytes(2, "little")
    elif kind in (3, 4, 5):
        sol[rec + 2] = r.choice([8, 9, 9, 10])
        it[ent + 7] = r.choice([0, 0, 1, 2, 0xFF])
        it[ent + 9] = r.choice([0, 0, 1, 2, 0xFF])
    elif kind == 6:                      # mode 0: hp 0, team slot at the edge of 8
        sol[rec + 0x183] = 0
        sol[rec + 0x17A] = r.choice([6, 7, 8, 9])
        return 0, r.choice([False, True]), r.randrange(256)
    elif kind == 7:                      # mode 1: hp at the edge of 15, hp 0, hp below 0
        sol[rec + 0x183] = r.choice([0, 1, 13, 14, 15, 16, 0xFF, 0x80])
        return 1, r.choice([False, True]), r.randrange(256)
    else:                                # mode 3 and past the table: only the direct entry, the setter maps them
        return r.choice([3, 3, 3, 4, 0xFF]), True, r.randrange(256)
    return 2, r.choice([False, True]), r.randrange(256)


fail = 0
rng = random.Random(0xC438)
edge = random.Random(0x1C39C)
try:
    for case in range(ncases):
        slot = rng.randrange(NSOL)
        sol = bytearray(base_sol)
        it = bytearray(base_item)
        rec = slot * REC
        hp = rng.choice(HP_POOL) if case % 2 else rng.randrange(256)
        ty = rng.choice([0, 1, 2, 5, 0x10, rng.randrange(136)])
        sol[rec + 0x183] = hp & 0xFF
        sol[rec + 0x17A] = rng.choice(SLOTB_POOL) if case % 2 else rng.randrange(256)
        sol[rec + 0] = ty
        sol[rec + 2] = rng.choice([0, 9, 10, 0x7F, 0x80, rng.randrange(256)])
        sol[rec + 0x1B2] = rng.choice([0, 0x6B, 0x6C, rng.randrange(256)])
        sol[rec + 0x1B4] = rng.choice([0x1C, 0x1F, 0, rng.randrange(256)])
        sol[rec + 0x1F3] = rng.randrange(4)
        for k in range(0x1E8, 0x1F0):
            sol[rec + k] = rng.randrange(256)
        ent = 12 * ty
        it[ent + 2] = rng.randrange(256)
        it[ent + 7] = rng.choice([0, 1, 0xFF])
        it[ent + 9] = rng.choice([0, 1, 0xFF])
        e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
        e.uc.mem_write(e.rt(ITEM), bytes(it))
        mode = rng.choice(MODE_POOL) if case % 2 else rng.randrange(256)
        noise = rng.randrange(256)
        direct = False
        if case % 3 == 2:
            mode, direct, noise = edge_case(edge, sol, it, rec, ty)
            ent = 12 * sol[rec]
            e.uc.mem_write(e.rt(SOLDIERS), bytes(sol))
            e.uc.mem_write(e.rt(ITEM), bytes(it))
        wr(MODE, noise)
        wr(VALUE, noise)
        apn, tilen = rng.randrange(1, 60), rng.randrange(3136)      # MoveCursorState clears both at its start
        e.uc.mem_write(e.rt(0x229DEC), struct.pack("<i", apn))
        e.uc.mem_write(e.rt(0x229D2C), struct.pack("<i", tilen))
        if direct:
            wr(MODE, mode)
            e.call(e.rt(0x1C464), e.rt(SOLDIERS) + rec)
        else:
            e.call(e.rt(0x1C438), e.rt(SOLDIERS) + rec, mode)
        om, ov = rd(MODE), rd(VALUE)
        oa = struct.unpack("<i", e.uc.mem_read(e.rt(0x229DEC), 4))[0]
        ot = struct.unpack("<i", e.uc.mem_read(e.rt(0x229D2C), 4))[0]
        if PORT is not None:
            pm, pv, pa, pt = port_call(slot, mode, noise, direct, apn, tilen)
            if (om, ov, oa, ot) != (pm, pv, pa, pt):
                fail += 1
                if fail <= 10:
                    print("case %d slot %d mode %d hp %d ty %d: original %02X/%02X ap %d tile %d port %02X/%02X ap %d tile %d"
                          % (case, slot, mode, hp, ty, om, ov, oa, ot, pm, pv, pa, pt))
        elif om > 2:
            # The setter maps a mode above 2 to 1; no run may leave a mode above 2 (a harness smoke check).
            fail += 1
            if fail <= 10:
                print("case %d slot %d mode %d: mode left at %02X" % (case, slot, mode, om))
finally:
    e.uc.mem_write(e.rt(SOLDIERS), base_sol)
    e.uc.mem_write(e.rt(ITEM), base_item)

print("%d cases, %d mismatches" % (ncases, fail))
sys.exit(1 if fail else 0)
