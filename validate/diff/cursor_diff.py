"""Differential test (in ./t check): the tile cursor drawing (docs/spec/cursor.md "Drawing") of the clean-room
front-end (port/ja1front ja1f_cursor_draw) vs the original DrawTargetMark? 0x16DD0 + DrawCursorPass? 0x171E8 in
Unicorn on a zeroed back buffer.

usage: .venv/bin/python validate/diff/cursor_diff.py [ncases=3000] [DUMP] [--lib SO]
Per case: the mouse tile (in the view, near it, off it), its occupant, the cursor value (used values + any 0..0xB5),
mode, out of range, combat, AP number + colours, blink timer / phase / no_blink, under_soldier, door wobble clock /
direction / offsets, the hover label (text, place; label_x -1 = none), the three gate flags, and the tile marker
(kind, tile (0xC41 = none), blink clock / phase; FUN_0001626C), and the soldier and tile marks (shown_slot 0x229CB4,
spotted 0x229C94: a merc / enemy slot with a random tile + pixel place, view_tile 0x229C8C with a random object
byte, clock 0x229D38 / phase 0x95F5A). The clock of the cursor pass is switched off (0x92DE5 = 0).
Compared: the back buffer (384 x 854), remap table 0, blink timer / phase, under_soldier, wobble offsets, the
marker's blink clock / phase, the mark clock / phase.
Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import livecast                      # noqa: E402
from csprite import palette          # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args and args[0].isdigit() else 3000
dump = next((a for a in args if not a.isdigit()), os.path.join(ROOT, "build", "mem_fight59.bin"))

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


PORT = ctypes.CDLL(LIB)
PORT.ja1f_cursor_draw.argtypes = [ctypes.c_char_p, ctypes.c_char_p, i32, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p,
                                  ctypes.c_void_p, ctypes.POINTER(CursorState), ctypes.c_void_p]
PORT.ja1f_cursor_draw.restype = i32
PORT.ja1f_cursor_dead_fix.argtypes = [i32, i32, i32, i32]
PORT.ja1f_cursor_dead_fix.restype = i32
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

BACK, BW, BH = 0x22A4EF, 384, 854
TILEMAP = 0x100A18
G = dict(over_map=(0x284F13, "B"), tactical_on=(0x92DCC, "B"), overlay_up=(0x92DCE, "B"), value=(0x284F1D, "B"),
         mode=(0x284F1B, "B"), out_of_range=(0x955C8, "B"), combat=(0x92DCB, "B"), ap_fg=(0x92E5C, "B"),
         ap_shadow=(0x92E60, "B"), blink_phase=(0x95F5C, "B"), no_blink=(0x95E0D, "B"), under_soldier=(0x284F03, "B"),
         wobble_dir=(0x284F05, "b"), tile=(0x229D2C, "i"), ap=(0x229DEC, "i"), blink_timer=(0x229D4C, "i"),
         wobble=(0x229D3C, "i"), wobble_dx=(0x22A47E, "h"), wobble_dy=(0x22A47C, "h"), label_x=(0x22A480, "h"),
         label_y=(0x22A482, "h"), marker_kind=(0x229CA8, "i"), marker_tile=(0x229CAC, "i"),
         marker_blink=(0x229D40, "i"), marker_phase=(0x284F30, "B"), shown_slot=(0x229CB4, "i"),
         spotted=(0x229C94, "i"), view_tile=(0x229C8C, "i"), mark_timer=(0x229D38, "i"), mark_phase=(0x95F5A, "b"))
OUT = ["blink_timer", "blink_phase", "under_soldier", "wobble_dx", "wobble_dy", "marker_blink", "marker_phase",
       "mark_timer", "mark_phase"]
SOLDIERS = 0xFA958
VALUES = [0x8E, 0x8F, 0x90, 0x91, 0xB5, 0xA1, 0xA4, 0x94, 0x77, 0x8C, 0x92, 0x95, 0x96, 0x97, 0x98, 0x74, 0x75, 0x76,
          0x93, 0x8B, 0xA2, 0x9C]
LABELS = [b"DYING", b"CRITICAL", b"WOUNDED", b"HEALTHY", b"SANTINO", b"Ivan", b"Hector", b"", b"Wi^de"]

e = Emu(dump=dump, fast=True)
ts = e.r8(e.rt(0x22A038))
pal = palette(ts)
T0 = e.r32(e.rt(0xE8C48))            # pointer to remap table 0
table0_base = bytes(e.uc.mem_read(T0, 256))
tilemap_base = bytes(e.uc.mem_read(e.rt(TILEMAP), 3136 * 8))
vx, vy, vw, vh = (struct.unpack("<i", e.uc.mem_read(e.rt(a), 4))[0] for a in (0x229CF8, 0x229CFC, 0x229CBC, 0x229CC0))
view = (i32 * 4)(vx, vy, vw, vh)
c0, r0 = vx // 16, vy // 14
soldiers_base = bytes(e.uc.mem_read(e.rt(SOLDIERS), 48 * 512))
e.w8(e.rt(0x92DE5), 0)


def case(r):
    s = {}
    s["over_map"] = r.choice([1, 1, 1, 1, 0])
    s["tactical_on"] = r.choice([1, 1, 1, 1, 0])
    s["overlay_up"] = r.choice([0, 0, 0, 0, 1])
    s["value"] = r.choice(VALUES) if r.random() < .8 else r.randrange(0xB6)
    s["mode"] = r.randrange(4)
    s["out_of_range"] = r.choice([0, 0, 1])
    s["combat"] = r.choice([0, 1, 1])
    s["ap_fg"], s["ap_shadow"] = r.choice([(1, 0), (0xFF, 0xFE)])
    s["blink_phase"] = r.randrange(2)
    s["no_blink"] = r.choice([0, 0, 1])
    s["under_soldier"] = r.choice([0, 0, 1])
    s["wobble_dir"] = r.choice([-2, -2, 0, 1, -1])
    k = r.random()
    if k < .7:
        c, w = c0 + r.randrange(-1, 15), r0 + r.randrange(-1, 13)
    else:
        c, w = r.randrange(56), r.randrange(56)
    s["tile"] = min(max(w, 0), 55) * 56 + min(max(c, 0), 55)
    s["ap"] = r.choice([0, 0, r.randrange(1, 40), r.randrange(-5, 200)])
    s["blink_timer"] = r.choice([200, 50, 1, 0, -10])
    s["wobble"] = r.choice([-1000, -1000, 0, 10, 550])
    s["wobble_dx"], s["wobble_dy"] = r.choice([(0, 0), (-4, 0), (0, -4), (r.randrange(-6, 7), r.randrange(-6, 7))])
    s["label_x"] = r.choice([-1, -1, r.randrange(24, 370), r.randrange(30, 260)])
    s["label_y"] = r.randrange(10, 220)
    s["label"] = r.choice(LABELS)
    if r.random() < .3:
        s["marker_tile"] = 0xC41
    else:
        mc, mw = (c0 + r.randrange(-1, 15), r0 + r.randrange(-1, 13)) if r.random() < .8 else (r.randrange(56), r.randrange(56))
        s["marker_tile"] = min(max(mw, 0), 55) * 56 + min(max(mc, 0), 55)
    s["marker_kind"] = r.choice([0x8A, 0x8A, 0x0F, 0x93, r.randrange(0xB6)])
    s["marker_blink"] = r.choice([200, 50, 1, 0, -10])
    s["marker_phase"] = r.randrange(2)
    occ = r.choice([0x40, 0x40, r.randrange(0x30), r.randrange(0x30, 0x80), r.randrange(0x80, 0x100)])
    # cursor.md "Soldier and tile marks": two soldier slots (merc / enemy / none), a view tile, the clock.
    s["shown_slot"] = r.choice([0x40, 0x40, r.randrange(0x28), r.randrange(0x30), 0x41, 0x100])
    s["spotted"] = r.choice([0x40, 0x40, r.randrange(0x28, 0x30), r.randrange(0x30)])
    if r.random() < .5:
        s["view_tile"] = 0xC41
    else:
        mc, mw = (c0 + r.randrange(-1, 15), r0 + r.randrange(-1, 13)) if r.random() < .8 else (r.randrange(56), r.randrange(56))
        s["view_tile"] = min(max(mw, 0), 55) * 56 + min(max(mc, 0), 55)
    s["mark_timer"] = r.choice([100, 50, 10, 0, -10])
    s["mark_phase"] = r.randrange(5)
    sold = {}
    for k in {s["shown_slot"], s["spotted"]}:
        if k < 0x30:
            t = s["tile"] if r.random() < .2 else r.randrange(3136)
            px = (r.randrange(vx - 40, vx + vw + 40), r.randrange(vy - 40, vy + vh + 40)) if r.random() < .8 \
                else (r.randrange(-100, 1000), r.randrange(-100, 1000))
            sold[k] = (t, px[0], px[1])
    s["sold"] = sold
    s["obj"] = r.choice([None, 0, r.randrange(1, 256)])
    return s, occ


def patch(soldiers, tm, s):
    for k, (t, x, y) in s["sold"].items():
        struct.pack_into("<hhh", soldiers, k * 512 + 0x114, t, x, y)
    if s["obj"] is not None and s["view_tile"] < 3136:
        tm[s["view_tile"] * 8 + 1] = s["obj"]


def original(s, occ):
    for k, (a, f) in G.items():
        e.uc.mem_write(e.rt(a), struct.pack("<" + f, s[k]))
    e.uc.mem_write(e.rt(0x229DE0), str(s["ap"]).encode() + b"\0")
    e.uc.mem_write(e.rt(0x22A444), s["label"] + b"\0")
    e.uc.mem_write(T0, table0_base)
    tm, sd = bytearray(tilemap_base), bytearray(soldiers_base)
    patch(sd, tm, s)
    tm[s["tile"] * 8 + 5] = occ
    e.uc.mem_write(e.rt(TILEMAP), bytes(tm))
    e.uc.mem_write(e.rt(SOLDIERS), bytes(sd))
    e.uc.mem_write(e.rt(BACK), bytes(BW * BH))
    e.call(e.rt(0x16DD0))
    e.call(e.rt(0x171E8))
    out = {k: struct.unpack("<" + G[k][1], e.uc.mem_read(e.rt(G[k][0]), struct.calcsize(G[k][1])))[0] for k in OUT}
    return bytes(e.uc.mem_read(e.rt(BACK), BW * BH)), bytes(e.uc.mem_read(T0, 256)), out


def port(s, occ):
    st = CursorState()
    for k in G:
        setattr(st, k, s[k])
    lab = s["label"][:15]
    st.label[:len(lab)] = list(lab)
    tm, sd = bytearray(tilemap_base), bytearray(soldiers_base)
    patch(sd, tm, s)
    tm[s["tile"] * 8 + 5] = occ
    tmb = (u8 * len(tm)).from_buffer(tm)
    sdb = (u8 * len(sd)).from_buffer(sd)
    t0 = (u8 * 256).from_buffer_copy(table0_base)
    back = (u8 * (BW * BH))()
    rc = PORT.ja1f_cursor_draw(ISO, EXE, ts, tmb, sdb, t0, view, ctypes.byref(st), back)
    return rc, bytes(back), bytes(t0), {k: getattr(st, k) for k in OUT}


r = random.Random(1)
bad = 0
for i in range(ncases):
    s, occ = case(r)
    try:
        ob, ot, oo = original(s, occ)
    except Exception as x:
        print("EMU", i, s, hex(occ), x); bad += 1; continue
    rc, pb, pt, po = port(s, occ)
    livecast.status("cursor_diff", i + 1, ncases, bad, f"value {s['value']:#x} tile {s['tile']}")
    x0, y0 = max(0, 16 * (s["tile"] % 56) - vx), max(0, 14 * (s["tile"] // 56) - vy)
    if ob != pb or ot != pt or oo != po or rc != 0:
        bad += 1
        if bad <= 8:
            n = next((k for k in range(BW * BH) if ob[k] != pb[k]), -1)
            where = f"pixel ({n % BW},{n // BW}) {ob[n]:#x} vs {pb[n]:#x}" if n >= 0 else "buffer ok"
            print(f"MISMATCH case {i} rc {rc} {s} occ {occ:#x}: {where}; table0 {'ok' if ot == pt else 'differs'}; "
                  f"state {oo} vs {po}")
    if ob != pb or i % 50 == 0:
        crop = lambda b: b[:BW * 230]
        livecast.frame("cursor_diff", f"tile cursor value {s['value']:#x}: original | port | differences",
                       crop(ob), crop(pb), BW, 230, pal, force=ob != pb)
saved_value, saved_mode = e.r8(e.rt(0x284F1D)), e.r8(e.rt(0x284F1B))
sp = e.r32(e.rt(0x100958))
saved_soldier = bytes(e.uc.mem_read(sp, 0x200))
fix_cases = 0
for value in (0x91, 0xB5, 0xA4, 0x8F):
    for hp in (0, 1, 15, 255):
        for mode in (0, 1, 2):
            for profile in (0, 7, 8, 0x3F):
                e.w8(e.rt(0x284F1D), value)
                e.w8(e.rt(0x284F1B), mode)
                e.w8(sp + 0x183, hp)
                e.w8(sp + 0x17A, profile)
                e.call(e.rt(0x1D2BC), sp)
                want = e.r8(e.rt(0x284F1D))
                got = PORT.ja1f_cursor_dead_fix(value, hp, mode, profile)
                if want != got:
                    bad += 1
                    if bad <= 8:
                        print(f"MISMATCH dead_fix value {value:#x} hp {hp} mode {mode} profile {profile}: {want:#x} / {got:#x}")
                fix_cases += 1
e.uc.mem_write(sp, saved_soldier)
e.w8(e.rt(0x284F1D), saved_value)
e.w8(e.rt(0x284F1B), saved_mode)
print(f"cursor_diff: {ncases} draw cases + {fix_cases} dead_fix cases, {bad} mismatches")
sys.exit(1 if bad else 0)
