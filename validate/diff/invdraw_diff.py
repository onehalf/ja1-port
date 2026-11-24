"""Differential test: the inventory screen drawing (tools/invdraw.py) vs the original DrawMercBody? 0x555DC in Unicorn.

usage: .venv/bin/python validate/diff/invdraw_diff.py [ncases=200] [DUMP ...] [--port [--lib SO]] [--png]
Per case: one dump, one merc slot 0..7 (S = record k), [0x92DDF] = 1 as the screen entry 0x54D50 sets it, fuzzed
item entries of S (types 0..0x70, counts, conditions -20..100, attachment), vest type, body number +0x17C,
camouflage +0x1B1, +0x112, the ground item under S, the other mercs' hp / sector / +0x17E / tile, noise in the panel
buffer (case 0 of each slot unfuzzed). Stubs on both sides: the PCC loader 0x532BC (MAIN.DAT member by name, PCX
rows into dst at the pitch; a missing member = -1, nothing drawn), the mouse / screen calls 0x67368 0x67398 0x67BF0
0x673AC 0x682AC 0x67C68, SightLine 0x58760 (a fixed pseudo-random answer per case and argument list; the rule is
tested in ai_diff). Compared: panel buffer, screen buffer, the 48 soldier records, remap table 0, the item place
table 0x949F2, the blink byte 0x96068, [0x955CA], the loader result [0x229D18], the return value (armour).
--port: the clean-room front-end (port/ja1front ja1f_draw_inventory, docs/spec/inventory.md "Drawing the screen")
draws on the second emulator's state instead of the mirror (the sight answers per merc come from the same stub).
--png writes gfx/invdraw_diff.png (original | mirror screen) for the first mismatch. Exits 1 on any mismatch.
"""
import os, random, sys, zlib
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE   # noqa: E402
from unicorn.x86_const import UC_X86_REG_EAX, UC_X86_REG_EBP, UC_X86_REG_EIP, UC_X86_REG_ESP   # noqa: E402
import panel as P                    # noqa: E402
import invdraw as I                  # noqa: E402
import datarc, pcx                   # noqa: E402

DUMPS = ["tac60", "fight1", "combat1", "death1", "f5"]
argv = sys.argv[1:]
PORT = None
if "--port" in argv:
    import ctypes
    LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target",
                                                                             "release", "libja1front.so")
    PORT = ctypes.CDLL(LIB)
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

    class Ja1fPanel(ctypes.Structure):
        _fields_ = [("slot", ctypes.c_uint8), ("selected", ctypes.c_uint8), ("combat", ctypes.c_uint8),
                    ("opt_onscreen", ctypes.c_uint8), ("speaker", ctypes.c_uint8), ("hover", ctypes.c_uint8),
                    ("ap_warned", ctypes.c_uint8 * 8), ("faces", ctypes.c_void_p), ("soldiers", ctypes.c_void_p),
                    ("table0", ctypes.c_void_p)]

    class Ja1fInventory(ctypes.Structure):
        _fields_ = [("panel", Ja1fPanel), ("tileset", ctypes.c_int32), ("sector", ctypes.c_uint8),
                    ("enemies", ctypes.c_uint8), ("blink", ctypes.c_uint8), ("sight", ctypes.c_uint8 * 8),
                    ("small_panel", ctypes.c_int32), ("time_w", ctypes.c_int32), ("tiles", ctypes.c_void_p),
                    ("ground", ctypes.c_void_p)]

    class Ja1fInventoryOut(ctypes.Structure):
        _fields_ = [("armour", ctypes.c_int32), ("load_result", ctypes.c_int32), ("blink", ctypes.c_uint8),
                    ("drawn", ctypes.c_uint8), ("item_rects", ctypes.c_uint8 * 56)]
    PORT.ja1f_draw_inventory.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(Ja1fInventory),
                                         ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(Ja1fInventoryOut)]
    PORT.ja1f_draw_inventory.restype = ctypes.c_int32
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args and args[0].isdigit() else 200
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]
MAIN = datarc.read(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"))
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
            _pics[n] = (w, h, px)
        except KeyError:
            _pics[n] = None
    return _pics[n]


def load(e, name, dst, pitch):
    p = pic(name)
    if p is None:
        return -1
    w, h, px = p
    for y in range(h):
        e.uc.mem_write(dst + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


class Sight:
    seed = 0

    def __call__(self, *a):
        return 1 if random.Random(hash((self.seed,) + tuple(a))).random() < 0.7 else 0


def emu(d, sight):
    e = Emu(dump=d, fast=True)
    e.read = lambda a, n: bytes(e.uc.mem_read(a, n))
    e.write = lambda a, b: e.uc.mem_write(a, bytes(b))

    def orig_load(emu_, eax, edx, ebx, ecx):
        return load(e, e.cstr(ebx), ecx, e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)) & 0xFFFFFFFF

    def orig_sight(emu_, eax, edx, ebx, ecx):
        sp = e.uc.reg_read(UC_X86_REG_ESP)
        st = [I._s32(e.r32(sp + 4 + 4 * i)) for i in range(5)]
        return sight(eax, I._s32(edx), I._s32(ebx), I._s32(ecx), *st)
    e.stub(e.rt(0x532BC), orig_load, 4)
    e.stub(e.rt(0x58760), orig_sight, 20)
    for a in (0x67368, 0x67398, 0x67BF0, 0x673AC, 0x682AC, 0x67C68):
        e.stub(e.rt(a), lambda *_: 0)
    e.stub_sprintf(0x75668)
    e.uc.hook_add(UC_HOOK_CODE, name_copy, e, e.rt(0x32DCE), e.rt(0x32DCE))
    return e


def name_copy(uc, addr, size, e):
    """0x32DCE..0x32E07 (in DrawMercName? 0x32DB8) = an inline strcpy with ds/es pushes (they fault in Unicorn)"""
    src, dst = uc.reg_read(UC_X86_REG_EBP) + 0x16E, e.rt(0xEBAC0)
    e.write(dst, e.cstr(src) + b"\0")
    uc.reg_write(UC_X86_REG_EAX, dst)
    uc.reg_write(UC_X86_REG_EIP, e.rt(0x32E07))


def port_draw(e, k, sight):
    """ja1f_draw_inventory on emulator e's state (merc k); writes everything it outputs back; returns the armour"""
    p = P.Panel(e)
    g = lambda a: e.r8(e.rt(a))
    s = p.soldier(k)
    recs_a = e.r32(e.rt(P.SOLDIERS))
    keep = []
    buf = lambda b: keep.append(ctypes.create_string_buffer(bytes(b), len(b))) or ctypes.addressof(keep[-1])
    recs = buf(e.read(recs_a, 48 * 0x200))
    faces = buf(b"".join(e.read(e.r32(e.rt(P.P_FACES + 4 * j)), 1024) for j in range(8)))
    t0 = p.remap() & ~0xFF
    table0 = buf(e.read(t0, 256))
    scr_a = e.r32(e.rt(I.P_SCREEN))
    panel, screen = buf(e.read(p.panel, 64000)), buf(e.read(scr_a, 64000))
    sv = []
    for j in range(8):
        o = p.soldier(j)
        sv.append(1 if sight(s, P.s16(e.r16(s + 0x116)) + 2, P.s16(e.r16(s + 0x118)) + 4, P.s16(e.r16(o + 0x114)),
                             P.s16(e.r16(o + 0x116)) + 2, P.s16(e.r16(o + 0x118)) + 4, 1, 0xE, 0x40) else 0)
    sec = g(I.CUR_SECTOR)
    pp = Ja1fPanel(k, g(0x284F11), g(0x92DCB), g(I.REDRAW_OFF), g(0x284F0E), g(0x93448),
                   (ctypes.c_uint8 * 8)(*e.read(e.rt(0x22A0EC), 8)), faces, recs, table0)
    v = Ja1fInventory(pp, g(0x22A038), sec, g(I.SECTOR_ENEMIES + 9 * sec), g(I.BLINK), (ctypes.c_uint8 * 8)(*sv),
                      I._s32(e.r32(e.rt(I.SMALL_PANEL_FLAG))), I._s32(e.r32(e.rt(I.TIME_W))),
                      buf(e.read(e.rt(0x100A18), 3136 * 8)), buf(e.read(e.rt(0xEB160), 2400)))
    out = Ja1fInventoryOut()
    r = PORT.ja1f_draw_inventory(ISO, EXE, ctypes.byref(v), panel, screen, ctypes.byref(out))
    if r:
        raise RuntimeError(f"ja1f_draw_inventory returned {r}")
    rd = lambda a, n: ctypes.string_at(a, n)
    e.write(p.panel, rd(panel, 64000))
    e.write(scr_a, rd(screen, 64000))
    e.write(recs_a, rd(recs, 48 * 0x200))
    e.write(t0, rd(table0, 256))
    e.write(e.rt(I.ITEM_RECTS), bytes(out.item_rects))
    e.w8(e.rt(I.BLINK), out.blink)
    e.w8(e.rt(0x955CA), out.drawn)
    e.w32(e.rt(I.LOAD_RESULT), out.load_result)
    return out.armour


TYPES = None


def item_types(e):
    """item types whose picture pointer is set (INV.DAT loaded)"""
    out = []
    for t in range(1, 0x71):
        pi = e.r8(e.rt(I.ITEM_TABLE + 12 * t + 4))
        if e.r32(e.rt(I.PICS + 4 * pi)):
            out.append(t)
    return out


def fuzz(e, slot, r, types):
    p = P.Panel(e)
    if r.random() < .5:
        e.uc.mem_write(p.panel, bytes(r.randrange(256) for _ in range(64000)))
    s = p.soldier(slot)
    for k in range(12):
        a = s + 12 * k
        if r.random() < .3:
            e.write(a, bytes(3))
            continue
        t = r.choice(types)
        n = r.choice([0, 1, 2, 5, 9, 10, 50, 98, 99, 100, 0xFF, 0xFE, r.randrange(256)])
        c = r.choice([0, 1, 5, 9, 10, 11, 50, 99, 100, 0xFF, 0xEC, r.randrange(101)])
        e.write(a, bytes([t, n, c]))
    if r.random() < .5:
        e.write(s + 3, bytes([r.choice(types), r.randrange(256), r.choice([0, 1, 9, 10, 50, 100, 0xFF])]))
    e.w8(s + 0x48, r.choice([0, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36]) if r.random() < .5 else e.r8(s + 0x48))
    for o, arm in ((0x18, [0x23, 0x24, 0x25, 0x26]), (0x3C, [0x2D, 0x2E, 0x2F, 0x30]), (0x48, [0x31])):
        if r.random() < .5:                   # armour in the helmet / vest / leg places, any condition
            e.write(s + o, bytes([r.choice(arm), 1, r.choice([1, 5, 50, 99, 100, r.randrange(1, 101)])]))
    if r.random() < .1:
        e.w8(s + 0x17A, r.choice([8, 0x40, 0xFF]))
    if PORT:                                  # panel.md "The switch view" (the mirror has none)
        for k in range(8):
            if r.random() < .2:
                e.w8(p.soldier(k) + 0x17F, r.choice([1, 1, 2]))
    e.w8(e.rt(I.REDRAW_OFF), r.choice([0, 1, 1, 2, 3]))
    if r.random() < .3:
        e.w32(e.rt(I.SMALL_PANEL_FLAG), r.choice([0, 1, 0xFFFFFFFF]))
    if r.random() < .3:
        e.w32(e.rt(I.TIME_W), r.choice([0, 1, 30, 0xFFFFFFFF]))
    e.w8(s + 0x17C, r.randrange(11))
    e.w8(s + 0x1B1, r.choice([0, 0, 1, 7]))
    e.w16(s + 0x112, r.choice([0, 7, 999, 1000, 9999, 10000, 65535, r.randrange(65536)]))
    tile = P.s16(e.r16(s + 0x114))
    if 0 <= tile < 56 * 56 and r.random() < .7:
        ta = e.rt(0x100A1E + 8 * tile)
        g = r.choice([0xFF, r.randrange(100)])
        e.w8(ta, g)
        if g != 0xFF:
            ga = e.rt(0xEB160 + 12 * g)
            e.write(ga, bytes([r.choice(types + [0]), r.randrange(256), r.choice([0, 5, 50, 100, 0xF0])]))
            e.w8(ga + 6, r.choice([0, 0, 0xFF, 0xD0]))
    for k in range(8):
        o = p.soldier(k)
        if k == slot:
            continue
        if r.random() < .3:
            e.w8(o + 0x183, r.choice([0, 1, 14, 15, 0x80, 50]))
        if r.random() < .2:
            e.w8(o + 0x182, r.choice([0, 1]))
        if r.random() < .2:
            e.w8(o + 0x17E, r.choice([0, 0, 1, 8]))
        if r.random() < .3 and 0 <= tile < 56 * 56:
            e.w16(o + 0x114, max(0, min(3135, tile + r.choice([-57, -1, 1, 2, 56, 112, 3, 200]))))
    e.w8(e.rt(I.BLINK), r.randrange(2))


def state(e, ret):
    p = P.Panel(e)
    t0 = p.remap() & ~0xFF
    return {"panel": e.read(p.panel, 64000), "screen": e.read(e.r32(e.rt(I.P_SCREEN)), 64000),
            "soldiers": e.read(e.r32(e.rt(P.SOLDIERS)), 48 * 0x200), "table0": e.read(t0, 256),
            "rects": e.read(e.rt(I.ITEM_RECTS), 56),
            "misc": bytes([e.r8(e.rt(I.BLINK)), e.r8(e.rt(0x955CA))]) + e.read(e.rt(I.LOAD_RESULT), 4),
            "ret": ret & 0xFFFFFFFF}


def png(path, a, b, e):
    from csprite import palette
    pal = palette(0)
    import struct
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    raw = bytearray()
    for y in range(200):
        raw.append(0)
        for v in a[y * 320:(y + 1) * 320] + b[y * 320:(y + 1) * 320]:
            raw += bytes(pal[3 * v:3 * v + 3])
    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 640, 200, 8, 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(bytes(raw))) + chunk(b"IEND", b""))
    open(path, "wb").write(data)


if __name__ == "__main__":
    fails = cases = offmap = 0
    r = random.Random(1)
    for d in dumps:
        if not os.path.exists(d):
            print(f"missing {d}")
            sys.exit(2)
        sa_, sb_ = Sight(), Sight()
        A, B = emu(d, sa_), emu(d, sb_)
        types = item_types(A)
        snap = bytes(A.uc.mem_read(0, 16 << 20))
        for i in range(max(8, ncases // len(dumps))):
            slot = i % 8
            A.uc.mem_write(0, snap)
            B.uc.mem_write(0, snap)
            seed = r.randrange(1 << 30)
            sa_.seed = sb_.seed = seed
            for e in (A, B):
                e.w8(e.rt(I.REDRAW_OFF), 1)
                if i >= 8:
                    fuzz(e, slot, random.Random(seed), types)
            s = P.Panel(A).soldier(slot)
            try:
                ra = A.call(A.rt(0x555DC), s)
            except Exception as x:
                print(f"original stopped: {os.path.basename(d)} case {i}: {x}")
                fails += 1
                continue
            if PORT and not 0 <= P.s16(B.r16(s + 0x114)) < 3136:
                offmap += 1                   # the original reads the bytes after the tile map: not in the spec
                continue
            if PORT:
                rb = port_draw(B, slot, sb_)
            else:
                rb = I.draw_body(B, s, lambda n, dst, pitch: load(B, n, dst, pitch), sb_)
            cases += 1
            sa, sb = state(A, ra), state(B, rb)
            bad = []
            for k in sa:
                if sa[k] != sb[k]:
                    if k == "ret":
                        bad.append(f"ret {sa[k]} vs {sb[k]}")
                        continue
                    n = next(j for j in range(len(sa[k])) if sa[k][j] != sb[k][j])
                    where = f"({n % 320},{n // 320})" if k in ("panel", "screen") else (
                        f"rec {n // 512} +{n % 512:#x}" if k == "soldiers" else f"+{n}")
                    bad.append(f"{k} @{where} {sa[k][n]:#x} vs {sb[k][n]:#x} "
                               f"({sum(x != y for x, y in zip(sa[k], sb[k]))} bytes)")
            if bad:
                fails += 1
                if fails == 1 and "--png" in argv:
                    png(os.path.join(ROOT, "gfx", "invdraw_diff.png"), sa["screen"], sb["screen"], A)
                if fails <= 10:
                    print(f"MISMATCH {os.path.basename(d)} case {i} slot {slot}: " + "; ".join(bad))
    print(f"invdraw_diff{' (port)' if PORT else ''}: {cases} cases{f' ({offmap} off-map left out)' if offmap else ''}, "
          f"{fails} mismatches")
    sys.exit(1 if fails else 0)
