#!/usr/bin/env python3
"""Differential test: entering a sector, LoadTacticalSector? 0x2AD20 with EAX 0, original vs port (enter::enter_world).

usage: .venv/bin/python validate/diff/loadsector_diff.py [--lib SO] [--only CASE]
The original runs whole in Unicorn on mem_wl_pre with the real SetupSectorFromXra, LoadTerrainTileset, FirstMerc,
TimeCompress, JumpViewToTile, CursorModeSet and the rest. Only file, picture and sound work is stubbed; the file stubs
serve the real game files. The port: ja1f_enter_fields on the day save the original wrote at the same moment. A case
changes the same bytes on both sides (sector, duties, merc flags, hp, the sector's merc byte). The entry must overwrite
the noise in the fields it starts again (front-end state too: Play::sector_start). Then every field of
quicksave::field_table must equal the original's memory at that field's address, and the end seed must match.
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from common import game_read         # noqa: E402
from emu import Emu                  # noqa: E402
import world_fields                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
ONLY = argv[argv.index("--only") + 1] if "--only" in argv else None
DUMP = os.path.join(ROOT, "build", "mem_wl_pre.bin")
SAVE = os.path.join(ROOT, "build", "DAY_before_restart.SAV")
ISO_B = os.path.join(ROOT, "rom", "JA1.gog").encode()
EXE_B = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
for f in (DUMP, SAVE):
    if not os.path.exists(f):
        print(f"loadsector_diff: {f} missing")
        sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_enter_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_uint32,
                                   ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(ctypes.c_uint32)]
BASE_SAVE = open(SAVE, "rb").read()
FILES = {}

SEED, SOLD, CUR = 0x9BBBC, 0x100958, 0x22A039
SAVE_SOLD, SAVE_CUR = 28, 20837
# file, picture and sound work: (address, stack bytes the callee pops)
STUBS = ((0x38190, 0), (0x5E8E0, 0), (0x4707C, 0), (0x75D04, 0), (0x3FA04, 0), (0x11D3C, 0), (0x5C4C8, 0), (0x2C898, 0),
         (0x31B44, 0), (0x52908, 4), (0x5C5EC, 0), (0x67F90, 0), (0x532BC, 4), (0x52E00, 0), (0x381C0, 0), (0x2F928, 0),
         (0x1CB78, 0), (0x36EA0, 0), (0x2A990, 0), (0x75A77, 0), (0x75EA0, 0))


EMU = Emu(dump=DUMP, fast=True)
START = [(lo, bytes(EMU.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in EMU.uc.mem_regions()]
FILE = {"data": b"", "pos": 0}


def open_cd(em, eax, *_):
    name = em.cstr(eax).decode().upper().replace("\\", "/")
    if name not in FILES:
        FILES[name] = game_read(name)
    FILE["data"], FILE["pos"] = FILES[name], 0
    return 0x1234


def fread(em, eax, edx, ebx, ecx):
    d = FILE["data"][FILE["pos"]:FILE["pos"] + edx * ebx]
    FILE["pos"] += len(d)
    em.uc.mem_write(eax, d)
    return len(d) // max(edx, 1)


def fseek(em, eax, edx, ebx, ecx):
    FILE["pos"] = edx if ebx == 0 else FILE["pos"] + edx if ebx == 1 else len(FILE["data"]) + edx
    return 0


for a_, pop_ in STUBS:
    EMU.stub(EMU.rt(a_), lambda em, *_: 0, pop_)
EMU.stub(EMU.rt(0x38304), open_cd)
EMU.stub(EMU.rt(0x758AD), fread)
EMU.stub(EMU.rt(0x757B1), fseek)
EMU.stub_sprintf()


def run_original(patch):
    e = EMU
    for lo, data in START:
        e.uc.mem_write(lo, data)
    seed = e.r32(e.rt(SEED))
    patch["mem"](e)
    noise(e)
    e.call(e.rt(0x2AD20), 0, max_steps=400_000_000)
    return e, seed


def run_port(patch, seed):
    save = bytearray(BASE_SAVE)
    patch["save"](save)
    buf = ctypes.create_string_buffer(4_000_000)
    end = ctypes.c_uint32()
    n = PORT.ja1f_enter_fields(ISO_B, EXE_B, bytes(save), len(save), seed, buf, len(buf), ctypes.byref(end))
    if n < 0:
        raise AssertionError(f"ja1f_enter_fields returned {n}")
    return world_fields.parse(buf.raw[:n]), end.value


def writes(items):
    """items: (image address, save offset or None, value bytes)"""
    def mem(e):
        base = e.r32(e.rt(SOLD))
        for a, o, v in items:
            if a:
                e.uc.mem_write(e.rt(a), v)
            else:                        # a soldier byte: the same offset as in the save
                e.uc.mem_write(base + o - SAVE_SOLD, v)

    def save(s):
        for _, o, v in items:
            if o is not None:
                s[o:o + len(v)] = v
    return {"mem": mem, "save": save}


def noise(e):
    """The original's copies of the noise in ja1f_enter_fields: the entry must overwrite each one."""
    for a, n in NOISE:
        e.uc.mem_write(e.rt(a), b"\x55" + bytes(n - 1) if n > 1 else b"\x55")


# Front-end state of the port with its address in the original (the map has no name for these).
PLAY = {"c.marker_kind": (0x229CA8, 4), "c.marker_tile": (0x229CAC, 4), "c.marker_blink": (0x229D40, 4),
        "c.blink_timer": (0x229D4C, 4), "c.mark_timer": (0x229D38, 4), "c.over_map": (0x284F13, 1),
        "c.mode": (0x284F1B, 1), "c.value": (0x284F1D, 1), "c.tile": (0x229D2C, 4), "c.ap": (0x229DEC, 4),
        "c.scroll_dy": (0x9344C, 1), "c.scroll_dx": (0x9344D, 1), "c.scroll_hover": (0x229D28, 4),
        "c.scroll_delay": (0x229D44, 4), "c.lit": (0x284F0E, 1), "c.slot_hover": (0x93448, 1),
        "c.strip_box": (0x92E73, 1), "c.quit_tactical": (0x92E74, 1), "c.strip_flag": (0x92E36, 1)}
NOISE = list(PLAY.values())
NOISE += [(0x22A1F5, 1), (0x92DC9, 1), (0x229D1C, 4), (0x92DA4, 4), (0x22A10B, 1), (0x22A0CF, 1), (0x229AE0, 4),
          (0x229AE4, 4), (0x229AE8, 4), (0x229AF0, 4), (0x22A1F3, 1), (0x22A100, 4), (0x92E30, 4)]


def skip(name, i):
    # copies that only Quick Save and Quick Restore fill (dayloop.rs): the play state keeps these values elsewhere
    if name.split(".")[-1] in ("cursor_mode", "cursor_value", "sight_queue", "music_wanted", "mouse_saved_x", "mouse_saved_y"):
        return True
    """Bytes that the two sides keep for another reason."""
    if name == "soldiers":              # +0x90: script pointer (address / offset); +0x11A: anchors of the port's draw
        return 0x90 <= i % 0x200 < 0x94 or 0x11A <= i % 0x200 < 0x11E
    if name == "enemy_watch":           # bytes 0..39 are guard_eta bytes 920..959 in the original (one variable)
        return i < 40
    if name == "accounts":              # the map's size runs over opt_reload_msg 0x22A02E and tileset 0x22A038
        return i in (142, 152)
    if name == "sector_state":          # past the 540 bytes of the table
        return i >= 540
    # opt_reload_msg: 0x22A02E gets 1 from the first draw (the port draws in enter_world, the original after the call).
    # guide_queue: not in the day save, set by the morning duties of the dump's day (the function does not touch it).
    return name in ("opt_reload_msg", "guide_queue")


BASE = writes([])
CASES = [("base", BASE)]
for s in range(60):
    CASES.append((f"sector{s}", writes([(CUR, SAVE_CUR, bytes([s]))])))
for k in range(8):                       # first merc k: mercs before it off duty
    CASES.append((f"first{k}", writes([(0, SAVE_SOLD + 0x200 * i + 0x17E, b"\x01") for i in range(k)])))
CASES.append(("nomerc", writes([(0, SAVE_SOLD + 0x200 * i + 0x17E, b"\x01") for i in range(8)])))
CASES.append(("nomerc_empty", writes([(CUR, SAVE_CUR, bytes([5]))] + [(0, SAVE_SOLD + 0x200 * i + 0x17E, b"\x01") for i in range(8)])))
SAVE_STATE, STATE = 9524, world_fields.addresses()["sector_state"][0]
REPLAY = world_fields.addresses()["sector_replay"][0]
RNG = random.Random(11)
for c in range(40):                      # merc fields FirstMerc and the cursor read; a sector with or without mercs
    s = RNG.randrange(60)
    m = bytes([RNG.choice((0, 1, 3))])
    # A loaded day save gives the sector table and its replay copy the same bytes (World::from_parts).
    items = [(CUR, SAVE_CUR, bytes([s])), (STATE + 9 * s + 2, SAVE_STATE + 9 * s + 2, m), (REPLAY + 9 * s + 2, None, m)]
    for i in range(8):
        r = SAVE_SOLD + 0x200 * i
        items += [(0, r + 0x17E, bytes([RNG.choice((0, 0, 0, 1, 8))])), (0, r + 0x182, bytes([RNG.choice((0, 1, 1))])),
                  (0, r + 0x183, bytes([RNG.choice((0, 5, 14, 15, 60))])), (0, r + 0x138, bytes([RNG.randrange(2)]))]
    CASES.append((f"fuzz{c}", writes(items)))


def main():
    bad = total = 0
    addr = dict(world_fields.addresses(), **PLAY)
    for name, patch in CASES:
        if ONLY and name != ONLY:
            continue
        e, seed = run_original(patch)
        fields, end = run_port(patch, seed)
        nf, nb, unmapped, diffs = 0, 0, [], []
        for full, value in fields.items():
            n = world_fields.short(full)
            n = full if full in addr else n if n in addr else n.split(".")[-1]
            if n not in addr:
                unmapped.append(n)
                continue
            a, size = addr[n]
            at = a(e) if callable(a) else e.rt(a)
            mem = bytes(e.uc.mem_read(at, min(size, len(value))))
            d = [(i, value[i], mem[i]) for i in range(len(mem)) if value[i] != mem[i] and not skip(n, i)]
            nf, nb = nf + 1, nb + len(mem)
            if d:
                diffs.append((n, len(d), d[:4]))
        real = e.r32(e.rt(SEED))
        if real != end or diffs:
            print(f"{name}: seed {'ok' if real == end else f'{end:#x} want {real:#x}'}")
            for d in diffs:
                print("  ", d)
        bad += len(diffs) + (real != end)
        total += 1
    print(f"loadsector_diff: {total} cases, last {nf} fields, {nb} bytes, no address: {' '.join(unmapped)}; {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
