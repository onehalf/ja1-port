"""Differential test (in ./t check): the hover step of the tile cursor (docs/spec/cursor.md "Hover") of the
clean-room front-end (port/ja1front ja1f_hover, on the library world) vs the original UseItemAt? 0x1D9B8 with no
mouse button, in Unicorn on mem_fight59.

usage: .venv/bin/python validate/diff/hover_diff.py [ncases=1500] [DUMP] [--lib SO]
Stubbed in the original (drawing only): mouse pointer hide / show 0x67BF0 0x67C54 0x67C68, strip slot redraw 0x331A4,
portrait draws 0x31C98 0x32EB8 0x335F8. Light 0x3349C, unlight 0x33570, SetSpeakerName 0x336E8, PathApCost,
ActionApCost and HidePathMarks run for real.
Per case: the mouse tile (soldiers, doors, ground items, the view), 1 / 3 / 85 hover steps, the selected merc, cursor
mode / value / tile / AP / wobble, the hover state (delays, lit, named, light delay, target, redraw_off), the gate
flags, path_preview, Shift / Alt, and the selected merc's hp, +0x1B8, +0x1BE, +0x1BF, +0x16A, apLeft, hand item.
Compared: the cursor and hover state, the label text, and the whole library world (tile marks, marks list ...).
Exits 1 on any mismatch.
"""
import ctypes, itertools, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
import livecast                      # noqa: E402
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args and args[0].isdigit() else 1500
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


class HoverState(ctypes.Structure):
    _fields_ = [("ap_timer", i32), ("item_timer", i32), ("light_timer", i32), ("target", i32), ("lit", u8),
                ("named", u8), ("light_wait", u8), ("redraw_off", u8)]


PORT = ctypes.CDLL(LIB)
PORT.ja1f_hover.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host), ctypes.c_char_p,
                            ctypes.POINTER(CursorState), ctypes.POINTER(HoverState)] + [i32] * 4
PORT.ja1f_hover.restype = i32
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

C = dict(over_map=(0x284F13, "B"), tactical_on=(0x92DCC, "B"), overlay_up=(0x92DCE, "B"), value=(0x284F1D, "B"),
         mode=(0x284F1B, "B"), out_of_range=(0x955C8, "B"), ap_fg=(0x92E5C, "i"), ap_shadow=(0x92E60, "i"),
         wobble_dir=(0x284F05, "b"), tile=(0x229D2C, "i"), ap=(0x229DEC, "i"), wobble=(0x229D3C, "i"))
H = dict(ap_timer=(0x229DD4, "i"), item_timer=(0x229DDC, "i"), light_timer=(0x93D50, "i"), target=(0x229C4C, "i"),
         lit=(0x284F0E, "B"), named=(0x95E0C, "B"), light_wait=(0x93D4C, "B"), redraw_off=(0x92DDF, "B"))
SEL, SHIFT, ALT, PREVIEW, BUTTONS, LABEL, STRIP = 0x284F11, 0x284F21, 0x284F20, 0x229DD8, 0x229CF4, 0x22A444, 0x93448
SOLDIERS, TILEMAP, GROUND = 0xFA958, 0x100A18, None
VALUES = [0x8E, 0x8F, 0x91, 0xB5, 0xA1, 0xA4, 0x94, 0x77, 0x8C, 0x92, 0x95, 0x96, 0x97, 0x98, 0x74, 0x75, 0x76,
          0x93, 0x8B, 0x8D, 0xA2]

e = Emu(dump=dump, fast=True)
for a in (0x67BF0, 0x67C54, 0x67C68, 0x331A4, 0x31C98, 0x32EB8, 0x335F8):
    e.stub(e.rt(a), lambda em, *r: 0)
OBJ2 = (e.rt(0x90000), 0x33A730 - 0x90000)
obj2_base = bytes(e.uc.mem_read(*OBJ2))
base_bufs, base_sc = port_lib.world_load(e, ai)
base_bufs = {k: bytes(v) for k, v in base_bufs.items()}
tiles = base_bufs["tiles"]
vx, vy = (struct.unpack("<i", e.uc.mem_read(e.rt(a), 4))[0] for a in (0x229CF8, 0x229CFC))
c0, r0 = vx // 16, vy // 14
occupied = [t for t in range(3136) if (tiles[8 * t + 5] & 0x7F) < 0x30]
doors = [t for t in range(3136) if base_bufs["t1_class"][tiles[8 * t + 1]] > 5 and tiles[8 * t + 1] in
         (0x3D, 0x3E, 0x5C, 0x5D, 0x43, 0x45, 0x62, 0x64)]
items = [t for t in range(3136) if tiles[8 * t + 6] < 200]
itypes = base_bufs["item_types"]
hand = [k for k in range(1, len(itypes) // 12) if itypes[12 * k + 2] in (0x92, 0x95, 0x96)] or [0]
own = {tiles[8 * t + 5] & 0x7F: t for t in occupied if (tiles[8 * t + 5] & 0x7F) < 8}
foes = [t for t in occupied if (tiles[8 * t + 5] & 0x7F) >= 8] or occupied
open_tiles = [min(max(r0 + y, 0), 55) * 56 + min(max(c0 + x, 0), 55) for y in range(12) for x in range(14)]
open_tiles = [t for t in open_tiles if (tiles[8 * t + 5] & 0x7F) >= 0x30] or [0]
print(f"hover_diff: view ({c0},{r0}), {len(occupied)} soldier tiles, {len(doors)} doors, {len(items)} item tiles, "
      f"{len(hand)} weapon types")


# Every health word: the plain table, the next table (hp 105..127), and past it into the item names (hp -15..-128 as bytes).
# The past-table words are one hp each, every second turn, so that each one lands in a case that draws the name.
FOE_HPS = [1, 14, 15, 16, 29, 30, 31, 32, 47, 48, 60, 63, 64, 75, 79, 80, 90, 95, 96, 99, 100, 104, 105, 112, 119, 120, 127,
           242, 255]
PAST_HPS = [120, 128, 137, 152, 167, 182, 197, 212, 227]
ALL_WORDS = [b"DYING", b"CRITICAL", b"WOUNDED", b"POOR", b"HEALTHY", b"STRONG", b"EXCELLENT", b"FAINT", b"DEFINITE", b"   ",
             b" Ammo   ", b"etonator     ", b"n       ", b"niper Scope  ", b"e  ", b"Of Steel", b"SANTINO"]
seen_words = set()


def foe_hp():
    k = next(foe_turn)
    return PAST_HPS[(k // 2) % len(PAST_HPS)] if k % 2 else FOE_HPS[(k // 2) % len(FOE_HPS)]


foe_turn = itertools.count()

def wr(a, f, v):
    e.uc.mem_write(e.rt(a), struct.pack("<" + f, v))


def rd(a, f):
    return struct.unpack("<" + f, e.uc.mem_read(e.rt(a), struct.calcsize(f)))[0]


def case(r):
    k = r.random()
    if k < .3 and occupied:
        t = r.choice(occupied)
    elif k < .45 and doors:
        t = r.choice(doors)
    elif k < .55 and items:
        t = r.choice(items)
    else:
        t = min(max(r0 + r.randrange(-1, 13), 0), 55) * 56 + min(max(c0 + r.randrange(-1, 15), 0), 55)
    c = dict(over_map=0, tactical_on=r.choice([1] * 12 + [0]), overlay_up=r.choice([0] * 12 + [1]),
             value=r.choice(VALUES), mode=r.choice([1, 1, 1, 2, 2, 3, 0]), out_of_range=r.randrange(2),
             ap_fg=0xFF, ap_shadow=0xFE, wobble_dir=r.choice([0, -2, 1]),
             tile=r.choice([t, t, 0xC41, r.randrange(3136)]), ap=r.choice([0, 0, r.randrange(1, 30)]),
             wobble=r.choice([-1000, -1000, 550, 10, 0, -5]))
    h = dict(ap_timer=r.choice([800, 800, 10, 20, 0, r.randrange(-20, 900)]), item_timer=r.choice([2000, 10, 0, 20]),
             light_timer=r.choice([300, 5, 0, -10]), target=r.choice([0x40, r.randrange(48)]),
             lit=r.choice([0x40, 0x40, r.randrange(8)]), named=r.choice([0x40, 0x40, r.randrange(48)]),
             light_wait=r.randrange(2), redraw_off=r.choice([0, 0, 0, 1, 2]))
    sel = r.choice(list(range(8)) * 8 + [8])
    m = dict(hp=r.choice([None, None, None, 10, 0, 14, 15]), b1B8=r.choice([None, None, 0, 1]),
             b1BE=r.choice([0, 0, 0, 1]), b1BF=r.choice([0, 0, 0, 1]), b16A=r.randrange(2),
             apLeft=r.choice([None, r.randrange(0, 30)]), hand=r.choice([None, None, r.choice(hand)]),
             cond=r.choice([None, 5, 50]))
    g = dict(shift=r.choice([0, 0, 1]), alt=r.choice([0, 0, 1]), preview=r.choice([0, 0, 1]))
    n = r.choice([1, 1, 3, 85])
    return t, c, h, sel, m, g, n


def edge_case(r, base, wobble=False):
    """Own stream (so the main cases do not move): the rare terms stand at their edges.
    Kinds: reach (WeaponReach? 0x58AB8), own tile in mode 2 (0x93), the enemy's name (SetSpeakerName? 0x336E8)."""
    t, c, h, sel, m, g, n = base
    sel = r.choice(list(own)) if own else sel
    c.update(over_map=0, tactical_on=1, overlay_up=0, out_of_range=r.randrange(2), wobble=-1000)
    h.update(redraw_off=0, ap_timer=800, named=0x40)
    m.update(hp=None, b1B8=None, b1BE=0, b1BF=0, hand=None, cond=None, apLeft=20)
    g.update(shift=0, alt=0, preview=0)
    n = 1
    kind = 3 if wobble else r.randrange(6)
    m["edge"] = {"kind": kind}
    if kind == 0:
        # A distance one pixel around the reach. The formula here only picks the place; the compare is the original.
        rng_item, hp, cond = r.choice([0, 1, 2, 5, 12]), r.choice([0, 15, 30, 60, 99]), r.choice([0, 1, 40, 73, 100])
        reach = rng_item * 15 + hp - 50
        reach -= (100 - cond) * reach // 200 if reach >= 0 else -((100 - cond) * -reach // 200)
        reach = max(reach, 0)
        d = max(reach + r.choice([-1, 0, 1, 2]), 0)
        t = r.choice(open_tiles)
        c.update(mode=2, value=r.choice([0x95, 0x8E]), tile=t)
        px = (t % 56) * 16 + 5
        m["edge"].update(rng_item=rng_item, hp=hp, cond=cond, x=px + d * 16 // 15 + r.randrange(2), y=(t // 56) * 14 + 5)
    elif kind == 1:
        t = own[sel]
        c.update(mode=2, value=r.choice([0x93, 0x93, 0x94, 0xB5, 0x8E, 0x95]), tile=t)
        m["hp"] = r.choice([60, 60, 14])
    elif kind == 4:
        # Unlight? 0x33570 with a lit byte around its edge of 8: at 8 and above the byte stays.
        t = r.choice(open_tiles)
        c.update(mode=1, value=r.choice([0x8E, 0x91]), tile=t)
        h.update(lit=r.choice([7, 8, 9, 0x3F]))
        m["hp"] = 60
    elif kind == 5:
        # TargetApNumber? 0x1D690 calls ActionApCost with flag 1: a gun in the aim animations adds 1 when the soldier
        # does not face the tile. The delay runs out in this step.
        t = r.choice(open_tiles)
        c.update(mode=2, value=r.choice([0x92, 0x95]), tile=t)
        h.update(ap_timer=r.choice([10, 20]))
        m.update(hp=60, b1B8=1, apLeft=r.choice([0, 5, 20, 40]), b16A=r.randrange(2))
        m["edge"].update(ty=r.randrange(1, 0x11), anim=r.choice([0x1B, 0x1F, 0x23, 0x23, 0x1C]), facing=r.randrange(8))
    else:
        t = r.choice(foes)
        c.update(mode=1, value=r.choice([0x8E, 0xA4, 0x91]), tile=t)
        m["hp"] = 60
        m["edge"].update(side=r.choice([3, 3, 3, 4, 5, 5]), foe_hp=foe_hp(),
                         foe=tiles[8 * t + 5] & 0x7F)
    if kind == 3:
        # The empty-tile branch: tile_standable (class edge, t1 words, t3 0x73) and the wobble rule (t3 <= 0x50 on this
        # tile, the tile left of it, or 2 rows up). TileByte3? 0x23A78 and the occupant edge 0x30.
        t = r.choice([k for k in open_tiles if k >= 112 and k % 56 > 0] or open_tiles)
        c.update(mode=r.choice([1, 1, 2, 3]), value=r.choice([0x8E, 0xB5, 0xA4, 0x94, 0x91]), tile=t,
                 wobble=r.choice([-1000, -1000, 550, 10, 0, -5]), wobble_dir=r.choice([0, -2, 1]))
        m["hp"] = 60
        cls = base_bufs["t1_class"]
        words = [0x3B, 0x41, 0x5A, 0x60, 0x3D, 0x3E, 0x5C, 0x5D, 0x43, 0x45, 0x62, 0x64]
        at = lambda f: [k for k in range(len(cls)) if f(cls[k])] or [0]
        t1 = r.choice([r.choice(words), r.choice(words), r.choice(at(lambda v: v == 5)), r.choice(at(lambda v: v == 6)),
                       r.choice(at(lambda v: v > 5)), r.choice(at(lambda v: v < 5))])
        m["edge"].update(t=t, t1=t1, t3=r.choice([0, 0x50, 0x51, 0x72, 0x73, 0x74]),
                         left=r.choice([0x50, 0x51, 0x60]), up=r.choice([0x50, 0x51, 0x60]),
                         occ=r.choice([0x40, 0x40, 0x30, 0x2F, 0x2E]), occ_b9c=r.choice([0, 1, 40, 0xFF]))
        if wobble:
            # tile_standable = -2 (these t1 words) on a high tile: only the tile 2 rows up counts, not the left one
            c.update(wobble=-1000)
            m["edge"].update(t1=r.choice([0x3D, 0x3E, 0x5C, 0x5D]), t3=r.choice([0x51, 0x60]), occ=0x40,
                             left=r.choice([0x50, 0x50, 0x51]), up=r.choice([0x50, 0x51, 0x60]))
    return t, c, h, sel, m, g, n


def setup(t, c, h, sel, m, g):
    e.uc.mem_write(*OBJ2[:1], obj2_base)
    port_lib.world_store(e, ai, {k: bytearray(v) for k, v in base_bufs.items()}, dict(base_sc))
    for k, (a, f) in C.items():
        wr(a, f, c[k])
    for k, (a, f) in H.items():
        wr(a, f, h[k])
    wr(SEL, "B", sel)
    wr(SHIFT, "B", g["shift"])
    wr(ALT, "B", g["alt"])
    wr(PREVIEW, "i", g["preview"])
    wr(BUTTONS, "B", 0)
    wr(STRIP, "B", 0x40)
    e.uc.mem_write(e.rt(LABEL), b"xx\0" + bytes(13))
    if sel < 8:
        rec = SOLDIERS + 512 * sel
        for k, off in (("hp", 0x183), ("b1B8", 0x1B8), ("b1BE", 0x1BE), ("b1BF", 0x1BF), ("b16A", 0x16A),
                       ("apLeft", 0x1A5), ("hand", 0), ("cond", 2)):
            if m[k] is not None:
                wr(rec + off, "B", m[k])
        ed = m.get("edge")
        if ed and ed["kind"] == 0:
            ty = rd(rec, "B") or 1
            wr(rec, "B", ty)
            wr(ai.G_ITEM_KIND - 2 + 12 * ty + 2, "B", 0x95)
            wr(ai.G_ITEM_KIND - 2 + 12 * ty + 11, "B", ed["rng_item"])
            wr(rec + 0x183, "B", ed["hp"])
            wr(rec + 0x18A, "B", ed["cond"])
            wr(rec + 0x116, "h", ed["x"])
            wr(rec + 0x118, "h", ed["y"])
        elif ed and ed["kind"] == 5:
            wr(rec, "B", ed["ty"])
            wr(ai.G_ITEM_KIND - 2 + 12 * ed["ty"] + 2, "B", 0x92)
            wr(rec + 0x1F3, "B", 0)
            wr(rec + 0x1E8, "h", ed["anim"])
            wr(rec + 0x1A1, "B", ed["facing"])
        elif ed and ed["kind"] == 2:
            wr(SOLDIERS + 512 * ed["foe"] + 0xBA, "B", ed["side"])
            wr(SOLDIERS + 512 * ed["foe"] + 0x183, "B", ed["foe_hp"])
        elif ed and ed["kind"] == 3:
            base, q = ai.G_TILEMAP, ed["t"]
            wr(base + 8 * q + 1, "B", ed["t1"])
            wr(base + 8 * q + 3, "B", ed["t3"])
            wr(base + 8 * (q - 1) + 3, "B", ed["left"])
            wr(base + 8 * (q - 112) + 3, "B", ed["up"])
            wr(base + 8 * q + 5, "B", ed["occ"])
            if ed["occ"] < 0x30:
                wr(SOLDIERS + 512 * ed["occ"] + 0x9C, "B", ed["occ_b9c"])


def original(t, n):
    for _ in range(n):
        e.call(e.rt(0x1D9B8), t)
    c = {k: rd(a, f) for k, (a, f) in C.items()}
    h = {k: rd(a, f) for k, (a, f) in H.items()}
    lab = bytes(e.uc.mem_read(e.rt(LABEL), 16)).split(b"\0")[0]
    bufs, sc = port_lib.world_load(e, ai)
    return c, h, lab, bufs, sc


def port(t, n, g):
    bufs, sc = port_lib.world_load(e, ai)
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = port_lib.World(**{k: ctypes.cast(a, port_lib.u8p) for k, a in arrays.items()}, **sc)
    ops = []

    def rnd(_ctx, k):
        ops.append(("random", k))
        return 0

    def cb(_ctx, op, a, nargs, out, size):
        ops.append(op)
        return 0
    host = port_lib.Host(None, port_lib.RANDOM_FN(rnd), port_lib.CALL_FN(cb), 0)
    st, hs = CursorState(), HoverState()
    for k, (a, f) in C.items():
        setattr(st, k, rd(a, f) if k not in ("ap_fg", "ap_shadow") else rd(a, f) & 0xFF)
    st.label[:] = list(e.uc.mem_read(e.rt(LABEL), 16))
    for k, (a, f) in H.items():
        setattr(hs, k, rd(a, f))
    rc = PORT.ja1f_hover(ctypes.byref(w), ctypes.byref(host), EXE, ctypes.byref(st), ctypes.byref(hs), t,
                         g["shift"], g["alt"], n)
    for k in sc:
        sc[k] = getattr(w, k)
    c = {k: getattr(st, k) for k in C}
    h = {k: getattr(hs, k) for k in H}
    lab = bytes(st.label).split(b"\0")[0]
    return rc, c, h, lab, bufs, sc, ops


r = random.Random(1)
edge = random.Random(0x58AB8)
bad = 0
WOBBLE = 40      # made cases after the old streams: the wobble rule of a tile with tile_standable = -2


def all_cases():
    for i in range(ncases):
        base = case(r)
        yield edge_case(edge, base) if i % 5 == 4 else base
    xr = random.Random("wobble")
    for i in range(WOBBLE):
        yield edge_case(xr, case(xr), wobble=True)


for i, (t, c, h, sel, m, g, n) in enumerate(all_cases()):
    setup(t, c, h, sel, m, g)
    pc, ph, plab, pbufs, psc, ops = None, None, None, None, None, None
    rc, pc, ph, plab, pbufs, psc, ops = port(t, n, g)
    setup(t, c, h, sel, m, g)
    try:
        oc, oh, olab, obufs, osc = original(t, n)
    except Exception as x:
        print("EMU", i, t, c, x)
        bad += 1
        continue
    oc["ap_fg"] &= 0xFF
    oc["ap_shadow"] &= 0xFF
    diffs = [k for k in oc if oc[k] != pc[k]] + [k for k in oh if oh[k] != ph[k]]
    diffs += ["label"] if olab != plab else []
    seen_words.add(olab)
    diffs += [k for k in obufs if k not in port_lib.READ_ONLY and obufs[k] != pbufs[k]]
    diffs += [k for k in osc if osc[k] != psc[k] and k != "marked_slot"]   # = named (0x95E0C), compared above
    diffs += ["host ops"] if ops else []
    diffs += ["rc"] if rc else []
    livecast.status("hover_diff", i + 1, ncases, bad, f"tile {t} mode {c['mode']} value {c['value']:#x}")
    if diffs:
        bad += 1
        if bad <= 8:
            print(f"MISMATCH case {i}: tile {t} n {n} sel {sel} {c} {h} {m} {g}: {diffs}")
            for k in diffs[:4]:
                if k in oc:
                    print(f"  {k}: {oc[k]} vs {pc[k]}")
                elif k in oh:
                    print(f"  {k}: {oh[k]} vs {ph[k]}")
                elif k == "label":
                    print(f"  label: {olab} vs {plab}")
                elif k in osc:
                    print(f"  {k}: {osc[k]} vs {psc[k]}")
                elif k == "host ops":
                    print(f"  host ops: {ops[:10]}")
                elif k in obufs:
                    j = next(j for j in range(len(obufs[k])) if obufs[k][j] != pbufs[k][j])
                    print(f"  {k}[{j}]: {obufs[k][j]} vs {pbufs[k][j]}")
if ncases >= 1500 and set(ALL_WORDS) - seen_words:     # a word that no case shows proves nothing
    print("hover_diff: words never shown:", sorted(set(ALL_WORDS) - seen_words))
    bad += 1
print(f"hover_diff: {ncases} cases + {WOBBLE} made wobble cases, {bad} mismatches")
sys.exit(1 if bad else 0)
