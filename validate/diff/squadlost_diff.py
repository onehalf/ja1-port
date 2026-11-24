#!/usr/bin/env python3
"""Differential test: the squad lost move, SquadLost? 0x3A32C, original vs port (Play::squad_lost_move).

usage: .venv/bin/python validate/diff/squadlost_diff.py [--lib SO] [--only CASE]
Each case enters a sector as loadsector_diff does (original 0x2AD20, port enter_world + sector_start), puts the same
noise in the state the function sets, and runs the move to sector 59. Every field of quicksave::field_table and the
cursor state must equal the original's memory, and the end seed must match. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
_argv = sys.argv[1:]
sys.argv = sys.argv[:1] + (["--lib", _argv[_argv.index("--lib") + 1]] if "--lib" in _argv else [])
sys.path.insert(0, HERE)
import loadsector_diff as L          # noqa: E402
import world_fields                  # noqa: E402

ONLY = _argv[_argv.index("--only") + 1] if "--only" in _argv else None
SELECTED = 0x284F11
TILEMAP, GROUND = 0x100A18, 0xEB160
PORT = L.PORT
PORT.ja1f_squadlost_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t,
                                       ctypes.c_uint32, ctypes.c_uint8, ctypes.c_void_p, ctypes.c_size_t,
                                       ctypes.POINTER(ctypes.c_uint32)]
E = L.EMU
# Music, panel and screen work inside the move (register arguments, BlitCopy pops 8).
E.stub(E.rt(0x5C724), lambda em, *_: 1)  # MusicBarCheck: the original waits for a true result
for a_, pop_ in ((0x52AE0, 8), (0x331A4, 0), (0x33768, 0)):
    E.stub(E.rt(a_), lambda em, *_: 0, pop_)
LIGHTS = []                              # FaceLight? 0x46A4C (AL slot, DL element): 0x3A419 lights the selected merc's face


def face_light(em, eax, edx, *_):
    LIGHTS.append((eax & 0xFF, edx & 0xFF))
    return 0


E.stub(E.rt(0x46A4C), face_light)
PORT.ja1f_take_face_lights.argtypes = [ctypes.c_char_p]
PORT.ja1f_take_face_lights.restype = None
STRIP = []                               # (0x10398 = up, 0x10454 = shade): the shade that stays in the panel buffer


def strip_up(em, *_):
    STRIP.append(0)
    return 0


def strip_shade(em, eax, *_):
    STRIP.append(eax & 0xFF)
    return 0


E.stub(E.rt(0x10398), strip_up)
E.stub(E.rt(0x10454), strip_shade)
PORT.ja1f_frame_shade.argtypes = [ctypes.c_uint8, ctypes.c_uint8]
TMP = {}                                 # sec%02d.tmp of this case (tmpsave_diff proves the writer's layout)


def save_tmp(em, *_):
    """SaveSectorTmp 0x2F800: tile bytes 1, 2, 3, 4, 6 and the ground items, as the original's fwrite calls."""
    cur = em.r8(em.rt(L.CUR))
    if cur != 59:                        # only sector 59 reads its tmp file again in this function
        return 0
    m = bytes(em.uc.mem_read(em.rt(TILEMAP), 3136 * 8))
    b = bytearray()
    for t in range(3136):
        b += bytes(m[t * 8 + o] for o in (1, 2, 3, 4, 6))
    b += bytes(em.uc.mem_read(em.rt(GROUND), 0x960))
    TMP[f"SEC{cur:02d}.TMP"] = bytes(b)
    return 0


def fopen(em, eax, *_):
    name = em.cstr(eax).decode().upper().replace("\\", "/").split("/")[-1]
    if name not in TMP:
        return 0
    L.FILE["data"], L.FILE["pos"] = TMP[name], 0
    return 0x1235


E.stub(E.rt(0x2F800), save_tmp)
E.stub(E.rt(0x75EA0), fopen)


def run_original(patch, sel):
    TMP.clear()
    STRIP.clear()
    e, seed = L.run_original(patch)
    e.w8(e.rt(SELECTED), sel)
    for a, v in ((0x92DCB, 1), (0x92DCC, 0), (0x92DCE, 1), (0x92DCA, 1), (0x284F13, 0), (0x284F1B, 3)):
        e.w8(e.rt(a), v)
    LIGHTS.clear()
    e.call(e.rt(0x3A32C), 0, max_steps=400_000_000)
    return e, seed


def run_port(patch, seed, sel):
    save = bytearray(L.BASE_SAVE)
    patch["save"](save)
    buf = ctypes.create_string_buffer(4_000_000)
    end = ctypes.c_uint32()
    n = PORT.ja1f_squadlost_fields(L.ISO_B, L.EXE_B, bytes(save), len(save), seed, sel, buf, len(buf),
                                   ctypes.byref(end))
    if n < 0:
        raise AssertionError(f"ja1f_squadlost_fields returned {n}")
    lights = ctypes.create_string_buffer(8)
    PORT.ja1f_take_face_lights(lights)
    return world_fields.parse(buf.raw[:n]), end.value, lights.raw


def skip(name, i):
    # copies that only Quick Save and Quick Restore fill (dayloop.rs): the play state keeps these values elsewhere
    if name.split(".")[-1] in ("cursor_mode", "cursor_value", "sight_queue", "music_wanted"):
        return True
    if name == "accounts" and i == 153:  # the map's size runs over cur_sector 0x22A039 (the field cur_sector compares it)
        return True
    return L.skip(name, i)


RNG = random.Random(13)
CASES = [("base", L.BASE, 0)]
for s in range(8):
    CASES.append((f"sel{s}", L.BASE, s))
CASES.append(("sel_enemy", L.BASE, 8))
for s in range(0, 59, 4):
    CASES.append((f"sector{s}", L.writes([(L.CUR, L.SAVE_CUR, bytes([s]))]), RNG.randrange(8)))
for c in range(20):
    s = RNG.randrange(59)
    m = bytes([RNG.choice((0, 1, 3))])
    items = [(L.CUR, L.SAVE_CUR, bytes([s])), (L.STATE + 9 * s + 2, L.SAVE_STATE + 9 * s + 2, m),
             (L.REPLAY + 9 * s + 2, None, m)]
    for i in range(8):
        r = L.SAVE_SOLD + 0x200 * i
        items += [(0, r + 0x17E, bytes([RNG.choice((0, 0, 0, 1, 8))])), (0, r + 0x182, bytes([RNG.choice((0, 1, 1))])),
                  (0, r + 0x183, bytes([RNG.choice((5, 14, 15, 60))])), (0, r + 0x138, bytes([RNG.randrange(2)]))]
    CASES.append((f"fuzz{c}", L.writes(items), RNG.choice((0, 1, 2, 3, 4, 5, 6, 7, 8, 8, 8))))
# The real scene: the last merc of the sector is dead, no merc can stand in sector 59. MercDies? writes the dead merc's slot; 8 is a forced value.
for sel in (0, 3, 7, 8):
    for sec in (5, 59):
        dead = [(0, L.SAVE_SOLD + 0x200 * i + 0x183, b"\0") for i in range(8)]
        CASES.append((f"alldead{sel}_{sec}", L.writes([(L.CUR, L.SAVE_CUR, bytes([sec]))] + dead), sel))
        out = [(0, L.SAVE_SOLD + 0x200 * i + 0x138, b"\0") for i in range(8)]
        CASES.append((f"allout{sel}_{sec}", L.writes([(L.CUR, L.SAVE_CUR, bytes([sec]))] + out), sel))


def main():
    bad = total = 0
    addr = dict(world_fields.addresses(), **L.PLAY)
    nf = nb = 0
    unmapped = []
    for name, patch, sel in CASES:
        if ONLY and name != ONLY:
            continue
        e, seed = run_original(patch, sel)
        fields, end, lights = run_port(patch, seed, sel)
        nf, nb, unmapped, diffs = 0, 0, [], []
        # 0x3A419: the portrait frame of the selected merc (the last element per slot; slots >= 8 have no face).
        want = [0] * 8
        for k, el in LIGHTS:
            if k < 8:
                want[k] = el
        if list(lights) != want:
            diffs.append(("face light", list(lights), LIGHTS[:4]))
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
        # The frame after the move: combat and the view hold of the original, the shade of its last strip call.
        shade = PORT.ja1f_frame_shade(e.r8(e.rt(0x92DCB)), e.r8(e.rt(0x92DCE)))
        if not STRIP or shade != STRIP[-1]:
            diffs.append(("strip shade", shade, list(STRIP)))
        # A later fight's strip reload ends it.
        PORT.ja1f_frame_shade(1, 0)
        if PORT.ja1f_frame_shade(0, 0) != 0:
            diffs.append(("strip shade after a fight", 1, []))
        real = e.r32(e.rt(L.SEED))
        if real != end or diffs:
            print(f"{name}: seed {'ok' if real == end else f'{end:#x} want {real:#x}'}")
            for d in diffs[:12]:
                print("  ", d)
        bad += len(diffs) + (real != end)
        total += 1
    print(f"squadlost_diff: {total} cases, last {nf} fields, {nb} bytes, no address: {' '.join(unmapped)}; {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
