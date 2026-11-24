"""Differential test: the tile popups (cursor.md "Tile popups"), original vs port.

usage: .venv/bin/python validate/diff/tilepopup_diff.py [ncases=120] [DUMP] [--port] [--lib SO]
Per case on tac60: a tile anywhere on the map with a fuzzed ground entry (type, count, byte 6, also none / 200+) or
the sign object 0x94, fuzzed t1 (raised objects), sector (sign text), view left / top, a noise screen and a noise
cursor save block [0x93E78]. The original runs 0x1D950 (tile, flag) in Unicorn: each step wait (the step timer
spin after the frame call) and the end wait (WaitRightUp 0x67C68 / WaitKeyOrClick 0x67F34, stubbed) take a screen,
then the screen after the call. The PCC loader 0x532BC (MAIN.DAT member) and PlayQuote 0x24F08 (SIGQUOTE.EDT
record) are stubbed. --port: ja1f_tile_popup gives its screens for the same world bytes; compared screen by screen.
Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 120
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_tile_popup.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_char_p] * 2 + [ctypes.c_int32, ctypes.c_void_p,
                                                                                    ctypes.c_int32] + [ctypes.c_char_p] * 2 + [ctypes.c_void_p]
    PORT.ja1f_tile_popup.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SIG = open(os.path.join(ROOT, "rom", "EDT", "SIGQUOTE.EDT"), "rb").read()
SCREEN_P, SAVE_P, TILES, GROUND, SECTOR = 0x95678, 0x93E78, 0x100A18, 0xEB160, 0x22A039
VIEW = [0x229CF8, 0x229CFC, 0x229CB8, 0x229CC4, 0x229CBC, 0x229CC0]
_pics = {}

e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(SCREEN_P))
save = e.r32(e.rt(SAVE_P))
shots = []


def load(em, eax, edx, ebx, ecx):
    n = em.cstr(ebx).decode("latin-1").upper()
    if n not in _pics:
        try:
            w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        except Exception:
            w, h, px = 0, 0, b""
        _pics[n] = (w, h, px)
    w, h, px = _pics[n]
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def quote(em, eax, edx, ebx, ecx):
    buf = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    at = (edx + ebx) * ecx
    em.uc.mem_write(buf, SIG[at:at + ecx] + b"\0")
    return 0


def shot(em, *_):
    shots.append(bytes(em.uc.mem_read(screen, 64000)))
    return 0


for a in (0x67368, 0x67398, 0x678E8, 0x67BF0):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x67C68), shot)
e.stub(e.rt(0x67F34), shot)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x24F08), quote, pop=16)
TIMER = e.rt(0x92EF8)


def spin(uc, addr, size, _):
    shots.append(bytes(uc.mem_read(screen, 64000)))
    uc.mem_write(TIMER, b"\0\0\0\0")


for a in (0x26F84, 0x27048, 0x273E3, 0x2743A):
    e.uc.hook_add(UC_HOOK_CODE, spin, None, e.rt(a), e.rt(a))
e.uc.ctl_flush_tb()

tiles0 = bytes(e.uc.mem_read(e.rt(TILES), 3136 * 8))
ground0 = bytes(e.uc.mem_read(e.rt(GROUND), 2400))
view0 = [struct.unpack("<i", bytes(e.uc.mem_read(e.rt(a), 4)))[0] for a in VIEW]
r = random.Random(11)
bad = items = signs = none = 0
for i in range(ncases):
    tiles, ground = bytearray(tiles0), bytearray(ground0)
    view = list(view0)
    view[2] = r.choice([view0[2], view0[2], 16, 48, 60])
    view[3] = r.choice([view0[3], view0[3], 8, 40])
    vx, vy = view[0] // 16, view[1] // 14
    c = min(55, max(0, vx + r.randrange(-2, view[4] // 16 + 2)))
    w = min(55, max(0, vy + r.randrange(-2, view[5] // 14 + 2)))
    t = 56 * w + c
    kind = r.random()
    tiles[8 * t + 1] = r.choice([r.randrange(256), 0x75, 0x74, 0x10])
    if kind < 0.55:
        g = r.randrange(200)
        tiles[8 * t + 6] = g
        ent = bytearray(12)
        ent[0] = r.randrange(1, 136) if r.random() < 0.95 else 0
        ent[1] = r.choice([1, 2, r.randrange(256)])
        ent[2] = r.randrange(101)
        ent[6] = r.choice([1, 5, 0x7F, r.randrange(256)])
        ground[12 * g:12 * g + 12] = ent
    elif kind < 0.95:
        tiles[8 * t + 6] = r.choice([0xFF, 0xFF, 200 + r.randrange(55)])
        if r.random() < 0.9:
            tiles[8 * t + 1] = 0x94
    else:
        tiles[8 * t + 6] = 0xFF
    sector = r.randrange(60)
    flag = r.randrange(2)
    e.uc.mem_write(e.rt(TILES), bytes(tiles))
    e.uc.mem_write(e.rt(GROUND), bytes(ground))
    for a, v in zip(VIEW, view):
        e.uc.mem_write(e.rt(a), struct.pack("<i", v))
    e.w8(e.rt(SECTOR), sector)
    base = bytes(r.randrange(256) for _ in range(64000))
    sv = bytes(r.randrange(256) for _ in range(224))
    e.uc.mem_write(screen, base)
    e.uc.mem_write(save, sv)
    shots.clear()
    e.call(e.rt(0x1D950), t, flag)
    after = bytes(e.uc.mem_read(screen, 64000))
    orig = shots + [after] if shots else []
    if len(orig) == 17:
        items += 1
    elif len(orig) == 19:
        signs += 1
    elif not orig:
        none += 1
        if after != base:
            print(f"case {i}: no popup but the screen changed")
            bad += 1
    else:
        print(f"case {i}: {len(orig)} screens")
        bad += 1
    if PORT:
        out = ctypes.create_string_buffer(20 * 64000)
        vi = (ctypes.c_int32 * 6)(*view)
        rc = PORT.ja1f_tile_popup(ISO, EXE, bytes(tiles), bytes(ground), t, vi, sector, base, sv, out)
        p = out.raw
        if rc != len(orig):
            print(f"MISMATCH case {i} tile {t}: {rc} screens vs {len(orig)}")
            bad += 1
        else:
            for f in range(rc):
                o, pf = orig[f], p[f * 64000:(f + 1) * 64000]
                if o != pf:
                    n = next(m for m in range(64000) if o[m] != pf[m])
                    if os.environ.get("TP_DEBUG"):
                        open("/tmp/tp_o.bin", "wb").write(o); open("/tmp/tp_p.bin", "wb").write(pf)
                        open("/tmp/tp_b.bin", "wb").write(orig[f - 1])
                    print(f"MISMATCH case {i} tile {t} ({len(orig)} screens) t1 {tiles[8 * t + 1]:#x} g {tiles[8 * t + 6]} "
                          f"view {view}: screen {f} @({n % 320},{n // 320}) "
                          f"{o[n]:#x} vs {pf[n]:#x}")
                    bad += 1
                    break
    if bad > 10:
        break
print(f"tilepopup_diff: {i + 1} cases ({items} item popups, {signs} sign popups, {none} nothing), {bad} mismatches"
      + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
