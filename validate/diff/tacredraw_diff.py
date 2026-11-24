#!/usr/bin/env python3
"""Differential test: one whole tactical frame, TacticalRedraw 0x176C8 with EAX 0, original vs port.

usage: .venv/bin/python validate/diff/tacredraw_diff.py [ncases=40] [DUMP ...] [--lib SO]
The original runs whole in Unicorn on a dump: every draw pass, DrawText and RemapRect are real. A QuickSave 0x2D880 run
on the same dump gives the port the same state (ja1f_tactical_frame: quicksave::restore, then
enter::render_world_cursor). Each case sets on both sides: the view, the banner timers and their texts, time compression,
the darken flag 0x93DBE, the "sees enemies" line, the show-all byte, the tile cursor, the view mode (1, 2, 3), the lit and
named slots, soldier text and flight bytes, the effect records, and dying or bleeding mercs (their portraits go over the
full view). Compared: the view window of the back buffer, the cursor bytes that the frame changes, the eye bytes +0x1AC, the
darken flag and the frame's flag 0x22A02E (opt_reload_msg). Exits 1 on mismatch.
Not compared: the copy to VGA (EAX 1): the port's screen::draw does that job with its own buffers. Not fuzzed: the roofs
flag (1), the hit step and the fire of the effect records (+0x10 = 1; ai_diff runs 0x173B4), smoke counts above 10
(the original reads past the sheet there, render.rs SPEC GAP).
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
NCASES = int(args[0]) if args and args[0].isdigit() else 100
EDGE = int(os.environ.get("TACREDRAW_EDGE", "40"))
DUMPS = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", n) for n in ("mem_tac60.bin", "mem_fight59.bin")]
ISO, EXE, DAY3 = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE"),
                                                             ("build", "DAY_day3.SAV")))
u8, i8, i16, i32 = ctypes.c_uint8, ctypes.c_int8, ctypes.c_int16, ctypes.c_int32
FIXED, TMP = 85402, 18080


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


class Frame(ctypes.Structure):
    _fields_ = [(n, i32) for n in ("tileset", "c0", "r0", "cols", "rows", "turn_timer", "turn_timer2", "time_compress",
                                   "darken", "stop_timer", "screen_drawn", "sees_slot", "lit", "named", "roofs",
                                   "show_all", "view_mode", "banner_colour")] + \
               [("banner", ctypes.c_char_p), ("sees_text", ctypes.c_char_p), ("patch", ctypes.c_void_p), ("patch_n", i32),
                ("fx", ctypes.c_char_p), ("dmg_text", ctypes.c_char_p), ("faces", ctypes.c_char_p), ("combat", i32),
                ("ap_warned", ctypes.c_char_p), ("eyes_out", ctypes.c_void_p)]


PORT = ctypes.CDLL(LIB)
PORT.ja1f_tactical_frame.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_void_p, ctypes.c_size_t, ctypes.POINTER(Frame),
                                                              ctypes.POINTER(CursorState), ctypes.c_void_p, ctypes.c_void_p]
PORT.ja1f_tactical_frame.restype = i32

BACK, PITCH, WIN = 0x22A4EF, 384, 0x22CF0F
# the cursor fields with their addresses (cursor_diff G)
G = dict(over_map=(0x284F13, "B"), tactical_on=(0x92DCC, "B"), overlay_up=(0x92DCE, "B"), value=(0x284F1D, "B"),
         mode=(0x284F1B, "B"), out_of_range=(0x955C8, "B"), combat=(0x92DCB, "B"), ap_fg=(0x92E5C, "B"),
         ap_shadow=(0x92E60, "B"), blink_phase=(0x95F5C, "B"), no_blink=(0x95E0D, "B"), under_soldier=(0x284F03, "B"),
         wobble_dir=(0x284F05, "b"), tile=(0x229D2C, "i"), ap=(0x229DEC, "i"), blink_timer=(0x229D4C, "i"),
         wobble=(0x229D3C, "i"), wobble_dx=(0x22A47E, "h"), wobble_dy=(0x22A47C, "h"), label_x=(0x22A480, "h"),
         label_y=(0x22A482, "h"), marker_kind=(0x229CA8, "i"), marker_tile=(0x229CAC, "i"),
         marker_blink=(0x229D40, "i"), marker_phase=(0x284F30, "B"), shown_slot=(0x229CB4, "i"),
         spotted=(0x229C94, "i"), view_tile=(0x229C8C, "i"), mark_timer=(0x229D38, "i"), mark_phase=(0x95F5A, "b"))
OUT = ("blink_timer", "blink_phase", "under_soldier", "wobble_dx", "wobble_dy", "marker_blink", "marker_phase",
       "mark_timer", "mark_phase", "label_x")
SEES, BANNER, DARKEN, RELOAD = 0x10DA50, 0x10E380, 0x93DBE, 0x22A02E
SOLD, FX, DMG, APW = 0xFA958, 0xEAFF0, 0x10E290, 0x22A0EC
MODES = (1, 1, 2, 3)
FLIGHT = list(range(16)) + list(range(40, 48))
SECTORS, SEC_ROW, SEC_FLAG = 0x222B74, 177, 169


def quick_payload(dump):
    """the quick save that the original writes on the dump: the fixed part + a synthesized tail (quicksave_diff)"""
    e = Emu(dump=dump, fast=True)
    rt, out = e.rt, bytearray()

    def fwrite(em, eax, edx, ebx, ecx):
        out.extend(e.uc.mem_read(eax, edx * ebx))
        return ebx

    e.stub(rt(0x278A0), lambda em, *_: 1, 32)
    e.stub(rt(0x27B98), lambda em, *_: 0, 16)
    e.stub(rt(0x2F800), lambda em, *_: 0)
    e.stub(rt(0x38370), lambda em, *_: e.w32(rt(0x92F54), 1) or 0)
    e.stub(rt(0x75FE3), fwrite)
    for a in (0x4E740, 0x51590, 0x2C834, 0x75A77):
        e.stub(rt(a), lambda em, *_: 0)
    e.call(rt(0x2D880), max_steps=20_000_000)
    assert len(out) == FIXED, f"QuickSave wrote {len(out)} bytes"
    flags = [e.r8(rt(SECTORS + i * SEC_ROW + SEC_FLAG)) for i in range(60)]
    n_rec = out[0xC3B2] | out[0xC3B3] << 8
    return bytes(out) + bytes(TMP * sum(f != 0 for f in flags) + 136 + 3 * n_rec)


def s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def run(dump, rng0):
    q = quick_payload(dump)
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    start = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions()]
    ts = e.r8(rt(0x22A038))
    mode0 = e.r8(rt(0x92DDF))
    soldiers = bytes(e.uc.mem_read(rt(SOLD), 48 * 512))
    vx0, vy0 = s32(e.r32(rt(0x229CF8))), s32(e.r32(rt(0x229CFC)))
    faces = b"".join(bytes(e.uc.mem_read(e.r32(rt(0x93428 + 4 * k)), 1024)) for k in range(8))
    bad = cases = 0
    for case in range(NCASES + EDGE):
        # made cases after the old stream: flight sprites and damage texts beyond each limit of the view
        edge = case >= NCASES
        rng = random.Random(f"edge{case}") if edge else rng0
        for lo, b in start:
            e.uc.mem_write(lo, b)
        first = case == 0
        mode = mode0 if first else rng.choice(MODES)
        cols, rows = {1: (14, 12), 2: (20, 12), 3: (20, 14)}[mode]
        c0 = vx0 // 16 if first else rng.randrange(0, 57 - cols)
        r0 = vy0 // 14 if first else rng.randrange(0, 57 - rows)
        patches, fx, dmg = [], None, None
        if edge or (not first and rng.random() < .7):
            # the view on a soldier, with fuzzed text and flight bytes
            k = rng.randrange(48)
            t = struct.unpack_from("<H", soldiers, k * 512 + 0x114)[0]
            if t < 3136:
                c0 = max(0, min(56 - cols, t % 56 - rng.randrange(cols)))
                r0 = max(0, min(56 - rows, t // 56 - rng.randrange(rows)))
                if edge:
                    # the soldier on the bottom or right tiles of the view, the text offset past the limit
                    c0 = max(0, min(56 - cols, t % 56 - rng.choice([0, cols - 1, cols - 2])))
                    r0 = max(0, min(56 - rows, t // 56 - rng.choice([0, rows - 1, rows - 2])))
                    patches += [(k, 0x1D2, 4), (k, 0xD8, rng.randrange(0, 60)), (k, 0xDA, rng.randrange(0, 60))]
            for s in (k, rng.randrange(8)):
                patches.append((s, 0x195, rng.choice([0, 1])))
                if rng.random() < .3:
                    patches += [(s, 0x1D2, 4), (s, 0xD8, rng.randrange(0, 20)), (s, 0xDA, rng.randrange(0, 20))]
            if case % 2 and not edge:
                # every flight slot in turn; some sprites on the limits of DrawSpriteAtWorld 0x1698C
                s = FLIGHT[(case // 2) % len(FLIGHT)]
                x, y = (c0 * 16 + rng.randrange(-20, cols * 16 + 20), r0 * 14 + rng.randrange(-20, rows * 14 + 20))
                if rng.random() < .3:
                    x = c0 * 16 + rng.choice([-16, -15, cols * 16 - 1, cols * 16])
                if rng.random() < .3:
                    y = r0 * 14 + rng.choice([-14, -13, rows * 14 - 1, rows * 14])
                patches += [(s, 0x1C7, 1), (s, 0x1C5, rng.randrange(0x78, 0x80)), (s, 0xF6, x & 255), (s, 0xF7, x >> 8 & 255),
                            (s, 0x9E, y & 255), (s, 0x9F, y >> 8 & 255)]
            if edge:
                for s in rng.sample(FLIGHT, 6):
                    x = c0 * 16 + rng.choice([rng.randrange(-40, -10), rng.randrange(cols * 16 - 20, cols * 16 + 40), rng.randrange(cols * 16)])
                    y = r0 * 14 + rng.choice([rng.randrange(-40, -10), rng.randrange(rows * 14 - 20, rows * 14 + 40), rng.randrange(rows * 14)])
                    n = rng.choice(list(range(0x78, 0x80)) + list(range(0xD2, 0xD8)) + [0])
                    patches += [(s, 0x1C7, 1), (s, 0x1C5, n), (s, 0xF6, x & 255), (s, 0xF7, x >> 8 & 255),
                                (s, 0x9E, y & 255), (s, 0x9F, y >> 8 & 255)]
            if mode >= 2 and rng.random() < .6:
                # portraits over the full view: a dying or bleeding merc (TacticalRedraw 0x17759)
                s = rng.randrange(8)
                patches.append((s, rng.choice((0x189, 0x1DC)), 1))
                for s in (s, rng.randrange(8)):
                    patches += [(s, 0x181, rng.choice([0, 0, 1, 3])), (s, 0x183, rng.choice([10, 15, 40])),
                                (s, 0x1A5, rng.choice([0, 5, 12])), (s, 0xEF, rng.choice([0, 8])),
                                (s, 0x189, rng.choice([0, 0, 1, 2])), (s, 0x1AC, rng.choice([0, 1, 3]))]
            if edge or rng.random() < .5:
                dmg = bytes(b for k2 in range(48) for b in b"12\0\0\0")
        fx_port = None
        if not first and rng.random() < .4:
            # effect records: no hit step and no fire (+0x10 = 1 and no kind 3 count 5: ai_diff tests those)
            fx, fx_port = bytearray(288), bytearray(288)
            for k in range(1, 16):
                if rng.random() < .3:
                    kind = rng.choice([0, 0, 2, 3, 4])
                    count = rng.randrange(1, 5) if kind == 0 else rng.choice([1, 2, 3, 6, 9, 12, 19]) if kind != 4 else rng.randrange(1, 11)
                    x, y = (c0 * 16 + rng.randrange(-20, cols * 16 + 20), r0 * 14 + rng.randrange(-20, rows * 14 + 20))
                    look = rng.randrange(2)
                    for b, ptr in ((fx, rt(0x9566B if look else 0x95670)), (fx_port, look)):
                        struct.pack_into("<bBBBBhhBBBi", b, k * 0x12, count, 0, 0, 0, 0, x, y, 0, 0, kind, ptr)
                        b[k * 0x12 + 0x10] = 1
        fr = Frame(tileset=ts, c0=c0, r0=r0, cols=cols, rows=rows, view_mode=mode)
        arr = (i32 * (3 * len(patches) + 1))(*[v for t in patches for v in t])
        fr.patch, fr.patch_n = ctypes.cast(arr, ctypes.c_void_p), len(patches)
        fr.fx, fr.dmg_text = bytes(fx_port) if fx_port else None, dmg
        fr.faces, fr.combat = faces, rng.choice([0, 1])
        ap_warned = bytes(rng.choice([0, 0, 1]) for _ in range(8))
        fr.ap_warned = ap_warned
        fr.turn_timer = 0 if first else rng.choice([0, 0, 1, 5])
        fr.turn_timer2 = 0 if first else rng.choice([0, 0, 0, 1, 40])
        fr.time_compress = 0 if first else rng.choice([0, 0, 1])
        fr.darken = 0 if first else rng.choice([0, 0, 1])
        if case % 8 == 5:
            # the darken flag with time compression and no timer: the text stays off
            fr.time_compress, fr.darken, fr.turn_timer, fr.turn_timer2 = 1, 1, 0, 0
        fr.stop_timer = 0 if first else rng.choice([0, 0, 3])
        fr.screen_drawn = e.r8(rt(0x92DCC)) if first else rng.choice([0, 1, 1])
        fr.sees_slot = e.r8(rt(0x92DC6)) if first else rng.choice([0x40, 0x40, rng.randrange(8)])
        fr.lit = 0x40 if first else rng.choice([0x40, 0x40, rng.randrange(8), rng.randrange(48)])
        fr.named = 0x40 if first else rng.choice([0x40, 0x40, rng.randrange(48)])
        fr.roofs = 1
        fr.show_all = 0 if first else rng.choice([0, 0, 1])
        fr.banner_colour = rng.choice([0x72, 0xA2, 0x16])
        banner, sees = b"ENEMY TURN", b"Grunty sees 2 enemies!"
        fr.banner, fr.sees_text = banner, sees
        st = CursorState()
        st.tactical_on = fr.screen_drawn
        st.combat = fr.combat
        st.mode, st.value = 1, 0x8E
        st.tile, st.ap = 0xC41, 0
        st.label_x, st.label_y = -1, 0
        st.marker_tile, st.shown_slot, st.spotted, st.view_tile = 0xC41, 0x40, 0x40, 0xC41
        st.ap_fg, st.ap_shadow, st.wobble = 1, 0, -1000
        st.blink_timer, st.marker_blink, st.mark_timer = 200, 200, 100
        st.wobble_dir = -2
        if not first and rng.random() < .5:
            st.tile = (r0 + rng.randrange(rows)) * 56 + c0 + rng.randrange(cols)
            st.value = rng.choice([0x8E, 0x8F, 0x90, 0x91, 0xA4])
            st.ap = rng.choice([0, 7, 25])
        # the original's side
        for k, (a, f) in G.items():
            e.uc.mem_write(rt(a), struct.pack("<" + f, getattr(st, k)))
        e.w32(rt(0x229CC8), cols)
        e.w32(rt(0x229CD0), rows)
        e.w32(rt(0x229CBC), cols * 16)
        e.w32(rt(0x229CC0), rows * 14)
        e.w8(rt(0x92DDF), mode)
        e.w8(rt(0x284F0E), fr.lit)
        e.w8(rt(0x95E0C), fr.named)
        e.uc.mem_write(rt(APW), ap_warned)
        for s, off, v in patches:
            e.w8(rt(SOLD + s * 512 + off), v)
        if fx:
            e.uc.mem_write(rt(FX), bytes(fx))
        if dmg:
            e.uc.mem_write(rt(DMG), dmg)
        e.uc.mem_write(rt(0x229DE0), str(st.ap).encode() + b"\0")
        e.uc.mem_write(rt(0x22A444), b"\0")
        e.w32(rt(0x229CF8), c0 * 16)
        e.w32(rt(0x229CFC), r0 * 14)
        e.uc.mem_write(rt(0x22A484), struct.pack("<hh", fr.turn_timer2, fr.turn_timer))
        e.w8(rt(0x9300C), fr.time_compress)
        e.w8(rt(DARKEN), fr.darken)
        e.w32(rt(0x95DC8), fr.stop_timer)
        e.w8(rt(0x92DC6), fr.sees_slot)
        e.w8(rt(0x93458), fr.show_all)
        e.w8(rt(0x92DDE), fr.banner_colour)
        e.uc.mem_write(rt(BANNER), banner + b"\0")
        e.uc.mem_write(rt(SEES), sees + b"\0")
        e.w8(rt(RELOAD), 0)
        e.uc.mem_write(rt(BACK), bytes(PITCH * 250))  # the effect sheet follows the back buffer
        e.call(rt(0x176C8), 0, max_steps=30_000_000)
        win = e.uc.mem_read(rt(WIN), PITCH * rows * 14)
        o = b"".join(bytes(win[y * PITCH:y * PITCH + cols * 16]) for y in range(rows * 14))
        # the port's side
        out, rel = ctypes.create_string_buffer(cols * 16 * rows * 14), u8(0)
        eyes = (u8 * 8)()
        fr.eyes_out = ctypes.cast(eyes, ctypes.c_void_p)
        rc = PORT.ja1f_tactical_frame(ISO, EXE, DAY3, q, len(q), ctypes.byref(fr), ctypes.byref(st), out, ctypes.byref(rel))
        cases += 1
        msg = []
        if rc != 0:
            msg.append(f"port returned {rc}")
        elif o != out.raw:
            d = [i for i in range(len(o)) if o[i] != out.raw[i]]
            i = d[0]
            xs, ys = [k % (cols * 16) for k in d], [k // (cols * 16) for k in d]
            msg.append(f"{len(d)} pixels differ, x {min(xs)}..{max(xs)} y {min(ys)}..{max(ys)}, first ({i % (cols * 16)},{i // (cols * 16)}) "
                       f"orig {o[i]:#x} port {out.raw[i]:#x}")
            if os.environ.get("TACREDRAW_DUMP"):
                open("/tmp/tacredraw_o.bin", "wb").write(o)
                open("/tmp/tacredraw_p.bin", "wb").write(out.raw)
        for k in OUT:
            a, f = G[k]
            v = struct.unpack("<" + f, e.uc.mem_read(rt(a), struct.calcsize(f)))[0]
            if v != getattr(st, k):
                msg.append(f"{k} orig {v} port {getattr(st, k)}")
        if e.r8(rt(RELOAD)) != rel.value:
            msg.append(f"reload flag orig {e.r8(rt(RELOAD))} port {rel.value}")
        oe = [e.r8(rt(SOLD + k * 512 + 0x1AC)) for k in range(8)]
        if oe != list(eyes):
            msg.append(f"eye bytes orig {oe} port {list(eyes)}")
        if e.r8(rt(DARKEN)) != 0:
            msg.append("the original kept the darken flag")
        if msg:
            bad += 1
            if bad <= 8:
                print(f"FAIL {os.path.basename(dump)} case {case} view ({c0},{r0}) tt {fr.turn_timer} tt2 {fr.turn_timer2} tc {fr.time_compress} "
                      f"dark {fr.darken} stop {fr.stop_timer} drawn {fr.screen_drawn} sees {fr.sees_slot} all {fr.show_all} tile {st.tile}: " + "; ".join(msg))
                if os.environ.get("TACREDRAW_DUMP") and fx:
                    print(" fx", [(fx[k * 0x12], fx[k * 0x12 + 0xB], struct.unpack_from("<hh", fx, k * 0x12 + 5)) for k in range(1, 16) if fx[k * 0x12]], "view px", c0 * 16, r0 * 14, "mode", mode)
                if os.environ.get("TACREDRAW_STOP"):
                    sys.exit(1)
    return cases, bad


rng = random.Random(1)
total = fails = 0
for d in DUMPS:
    n, b = run(d, rng)
    total, fails = total + n, fails + b
print(f"tacredraw_diff: {total} cases, {fails} mismatches")
sys.exit(1 if fails or not total else 0)
