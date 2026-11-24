"""Differential test: the tactical interface strips (tools/invdraw.py tac_strips) vs the original LoadStripPccs?
0x2F928 in Unicorn.

usage: .venv/bin/python validate/diff/strips_diff.py [ncases=500] [DUMP ...] [--port [--lib SO]] [--png]
Per case: one dump, AL top 0/1, DL keep 0/1, [0x92DDF] 0..3, combat 0x92DCB, the sector's mercs byte, the strip flag
0x92E36, the selected slot 0x284F11 and the turn slot 0x284F0F (0..7, 8, 0xFF), inPlay of the mercs, item entries
of the mercs, the time meter width, the small panel flag, the blink byte, noise in the panel and screen buffers
(case 0 of each dump unfuzzed). Stubs on both sides: the PCC loader 0x532BC (as invdraw_diff), the mouse / screen
calls 0x67368 0x67398 0x67BF0 0x673AC 0x682AC 0x67C68 0x68404 0x67330. Records with +0x17F = 1 (the switch view)
are set to 0 for the mirror; --port gives a quarter of the mercs +0x17F = 1 (or 2). Compared: panel buffer, screen buffer, the 48 soldier records, remap table 0, the
blink byte, [0x229CF0], [0x92DCA], the loader result. --port: the clean-room front-end (port/ja1front
ja1f_draw_strips, docs/spec/panel.md "The interface strips") instead of the mirror. Exits 1 on any mismatch.
"""
import os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import panel as P                    # noqa: E402
import invdraw as I                  # noqa: E402
import invdraw_diff as D             # noqa: E402  (argv parsing there sees our args: only the loader / emu are used)

DUMPS = ["tac60", "fight1", "combat1", "death1", "f5"]
argv = sys.argv[1:]
PORT = None
if "--port" in argv:
    import ctypes
    LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target",
                                                                             "release", "libja1front.so")
    PORT = ctypes.CDLL(LIB)
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    u8, i32 = ctypes.c_uint8, ctypes.c_int32

    class Ja1fStrips(ctypes.Structure):
        _fields_ = [("panel", D.Ja1fPanel), ("tileset", i32), ("top", u8), ("keep", u8), ("sector", u8),
                    ("mercs", u8), ("enemies", u8), ("strip_flag", u8), ("turn_slot", u8), ("blink", u8),
                    ("combat_seen", u8), ("small_panel", i32), ("time_w", i32), ("sel_in_play", u8)]

    class Ja1fStripsOut(ctypes.Structure):
        _fields_ = [("load_result", i32), ("blink", u8), ("combat_seen", u8), ("mouse_x", i32)]
    PORT.ja1f_draw_strips.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.POINTER(Ja1fStrips), i32,
                                      ctypes.c_void_p, ctypes.c_void_p, ctypes.POINTER(Ja1fStripsOut)]
    PORT.ja1f_draw_strips.restype = i32
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args and args[0].isdigit() else 500
dumps = [a for a in args if not a.isdigit()] or [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS]


def emu(d):
    e = D.emu(d, D.Sight())
    for a in (0x68404, 0x67330):
        e.stub(e.rt(a), lambda *_: 0)
    return e


def fuzz(e, r, types):
    p = P.Panel(e)
    if r.random() < .5:
        e.uc.mem_write(p.panel, bytes(r.randrange(256) for _ in range(64000)))
    if r.random() < .3:
        e.uc.mem_write(e.r32(e.rt(I.P_SCREEN)), bytes(r.randrange(256) for _ in range(64000)))
    e.w8(e.rt(I.REDRAW_OFF), r.choice([0, 0, 1, 1, 2, 3]))
    e.w8(e.rt(I.COMBAT), r.choice([0, 1]))
    e.w8(e.rt(I.COMBAT_SEEN), r.choice([0, 1]))
    sec = e.r8(e.rt(I.CUR_SECTOR))
    if r.random() < .3:
        e.w8(e.rt(I.SECTOR_MERCS + 9 * sec), r.choice([0, 1, 5]))
    if r.random() < .3:
        e.w8(e.rt(I.SECTOR_ENEMIES + 9 * sec), r.choice([0, 1, 5]))
    e.w8(e.rt(I.STRIP_ON), r.choice([0, 1, 1, 7]))
    e.w8(e.rt(I.SELECTED), r.choice([0, 1, 2, 3, 4, 5, 6, 7, 8, 0xFF]))
    e.w8(e.rt(I.TURN_SLOT), r.choice([0, 3, 7, 8, 0xFF]))
    if r.random() < .3:
        e.w32(e.rt(I.SMALL_PANEL_FLAG), r.choice([0, 1, 0xFFFFFFFF]))
    if r.random() < .3:
        e.w32(e.rt(I.TIME_W), r.choice([0, 1, 30, 0xFFFFFFFF]))
    e.w8(e.rt(I.BLINK), r.randrange(2))
    for k in range(8):
        s = p.soldier(k)
        if r.random() < .3:
            e.w8(s + 0x138, r.choice([0, 1]))
        if r.random() < .1:
            e.w8(s + 0x17A, r.choice([8, 0xFF, r.randrange(8)]))
        for j in range(12):
            if r.random() < .4:
                t = r.choice(types + [0])
                e.write(s + 3 * j, bytes([t, r.choice([0, 1, 9, 99, 0xFF, r.randrange(256)]),
                                          r.choice([0, 1, 9, 10, 50, 100, 0xFF, 0xEC])]))


def port_draw(e, top, keep):
    p = P.Panel(e)
    g = lambda a: e.r8(e.rt(a))
    recs_a = e.r32(e.rt(P.SOLDIERS))
    keep_ = []
    buf = lambda b: keep_.append(ctypes.create_string_buffer(bytes(b), len(b))) or ctypes.addressof(keep_[-1])
    recs = buf(e.read(recs_a, 48 * 0x200))
    faces = buf(b"".join(e.read(e.r32(e.rt(P.P_FACES + 4 * j)), 1024) for j in range(8)))
    t0 = p.remap() & ~0xFF
    table0 = buf(e.read(t0, 256))
    scr_a = e.r32(e.rt(I.P_SCREEN))
    panel, screen = buf(e.read(p.panel, 64000)), buf(e.read(scr_a, 64000))
    sec, sel = g(I.CUR_SECTOR), g(I.SELECTED)
    pp = D.Ja1fPanel(0, sel, g(I.COMBAT), g(I.REDRAW_OFF), g(0x284F0E), g(0x93448),
                     (ctypes.c_uint8 * 8)(*e.read(e.rt(0x22A0EC), 8)), faces, recs, table0)
    v = Ja1fStrips(pp, g(0x22A038), top, keep, sec, g(I.SECTOR_MERCS + 9 * sec), g(I.SECTOR_ENEMIES + 9 * sec),
                   g(I.STRIP_ON), g(I.TURN_SLOT), g(I.BLINK), g(I.COMBAT_SEEN),
                   I._s32(e.r32(e.rt(I.SMALL_PANEL_FLAG))), I._s32(e.r32(e.rt(I.TIME_W))),
                   g(0xFAA90 + 0x200 * sel))   # sel 64 / 255: a tile map / INV.DAT byte (panel.md)
    out = Ja1fStripsOut()
    r = PORT.ja1f_draw_strips(ISO, EXE, ctypes.byref(v), I._s32(e.r32(e.rt(I.MOUSE_TILE))), panel, screen,
                              ctypes.byref(out))
    if r:
        raise RuntimeError(f"ja1f_draw_strips returned {r}")
    rd = lambda a, n: ctypes.string_at(a, n)
    e.write(p.panel, rd(panel, 64000))
    e.write(scr_a, rd(screen, 64000))
    e.write(recs_a, rd(recs, 48 * 0x200))
    e.write(t0, rd(table0, 256))
    e.w8(e.rt(I.BLINK), out.blink)
    e.w8(e.rt(I.COMBAT_SEEN), out.combat_seen)
    e.w32(e.rt(I.LOAD_RESULT), out.load_result & 0xFFFFFFFF)
    e.w32(e.rt(I.MOUSE_TILE), out.mouse_x & 0xFFFFFFFF)


def prepare(e, seed):
    """the mirror has no switch view: records with +0x17F = 1 get 0; --port: some mercs get the switch view"""
    rr = random.Random(seed ^ 0x17F)
    for k in range(48):
        s = P.Panel(e).soldier(k) if k < 8 else e.rt(0xFA958 + 0x200 * k)
        if PORT and k < 8 and rr.random() < .25:
            e.w8(s + 0x17F, rr.choice([1, 1, 1, 2]))
        elif not PORT and e.r8(s + 0x17F) == 1:
            e.w8(s + 0x17F, 0)


def state(e):
    p = P.Panel(e)
    t0 = p.remap() & ~0xFF
    return {"panel": e.read(p.panel, 64000), "screen": e.read(e.r32(e.rt(I.P_SCREEN)), 64000),
            "soldiers": e.read(e.r32(e.rt(P.SOLDIERS)), 48 * 0x200), "table0": e.read(t0, 256),
            "misc": bytes([e.r8(e.rt(I.BLINK)), e.r8(e.rt(I.COMBAT_SEEN))]) + e.read(e.rt(I.LOAD_RESULT), 4)
            + e.read(e.rt(I.MOUSE_TILE), 4)}


fails = cases = 0
reached = set()
r = random.Random(1)
for d in dumps:
    if not os.path.exists(d):
        print(f"missing {d}")
        sys.exit(2)
    A, B = emu(d), emu(d)
    types = D.item_types(A)
    snap = bytes(A.uc.mem_read(0, 16 << 20))
    for i in range(max(4, ncases // len(dumps))):
        A.uc.mem_write(0, snap)
        B.uc.mem_write(0, snap)
        seed = r.randrange(1 << 30)
        top, keep = (1, 0) if i == 0 else (r.choice([0, 1, 1]), r.choice([0, 0, 1]))
        for e in (A, B):
            if i:
                fuzz(e, random.Random(seed), types)
            prepare(e, seed)
        g = lambda a: A.r8(A.rt(a))
        sel, turn = g(I.SELECTED), g(I.TURN_SLOT)
        on = bool(top and g(I.REDRAW_OFF) < 3)
        reached.add(("top", on, keep if on else 0))
        if g(I.STRIP_ON):
            reached.add(("strip", "sel" if sel < 8 and A.r8(A.rt(0xFAA90 + 0x200 * sel)) else
                         "turn" if turn < 8 and A.r8(A.rt(0xFAA90 + 0x200 * sel)) else "copy"))
        try:
            A.call(A.rt(0x2F928), top, keep)
        except Exception as x:
            print(f"original stopped: {os.path.basename(d)} case {i}: {x}")
            fails += 1
            continue
        if PORT:
            port_draw(B, top, keep)
        else:
            I.tac_strips(B, top, keep, lambda n, dst, pitch: D.load(B, n, dst, pitch))
        cases += 1
        sa, sb = state(A), state(B)
        bad = []
        for k in sa:
            if sa[k] != sb[k]:
                n = next(j for j in range(len(sa[k])) if sa[k][j] != sb[k][j])
                where = f"({n % 320},{n // 320})" if k in ("panel", "screen") else (
                    f"rec {n // 512} +{n % 512:#x}" if k == "soldiers" else f"+{n}")
                bad.append(f"{k} @{where} {sa[k][n]:#x} vs {sb[k][n]:#x} "
                           f"({sum(x != y for x, y in zip(sa[k], sb[k]))} bytes)")
        if bad:
            fails += 1
            if fails == 1 and "--png" in argv:
                D.png(os.path.join(ROOT, "gfx", "strips_diff.png"), sa["screen"], sb["screen"], A)
            if fails <= 10:
                print(f"MISMATCH {os.path.basename(d)} case {i} top {top} keep {keep}: " + "; ".join(bad))
want = {("top", True, 0), ("top", True, 1), ("top", False, 0), ("strip", "sel"), ("strip", "turn"), ("strip", "copy")}
missing = want - reached
if missing:
    print(f"strips_diff: never reached: {sorted(missing, key=str)}")
    fails += 1
print(f"strips_diff{' (port)' if PORT else ''}: {cases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
