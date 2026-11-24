"""Differential test: the merc panel data built from the game files (tools/panel_data.py) vs memory dumps of the original.

usage: .venv/bin/python validate/diff/panel_data_diff.py [DUMP ...] [--all]   (default: DUMPS; --all = every build/mem_*.bin)
Per dump: panel sheet [0x93454] (dumps that loaded it), face buffers [0x93428 + 4 slot] of the mercs in the sector
(+0x182) and their eye row +0x1AA, font sheets [0x229790 + 4 f] and the char table 0x9453A, remap tables
[0xE8C48 + 4 k] (pointers and bytes), and the exe tables the panel reads unchanged (vs JA.EXE). Exits 1 on any
mismatch. Known run-time writes are masked and listed in MASK. Off the tactical screen (no merc in the sector) other
screens use the panel sheet buffer as scratch and reload it after (LoadPanels callers): such dumps count as
"sheet in use".
--port: the face buffers and eye rows come from the clean-room front-end (port/ja1front ja1f_face, docs/spec/panel.md)
instead of tools/panel_data.py. The call starts from noise (the dump's bytes xor 0xA5); afterwards the bytes the
loader writes must equal the dump's and every other byte must still be the noise. The font sheets and char table are
also rebuilt by the port (ja1f_fonts, LoadSmallFont 0x381F8) and compared with the dump's memory. The other checks
stay as they are.
"""
import glob, os, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [os.path.join(ROOT, "tools")]
from memdump import Dump              # noqa: E402
from emu import Emu                   # noqa: E402
from le import load                   # noqa: E402
from common import cd_read            # noqa: E402
from datarc import read, member       # noqa: E402
from pcx import decode                # noqa: E402
import panel_data as PD               # noqa: E402

# the three FNT files LoadSmallFont reads, from the game folder of the target build (byte-identical to rom/'s)
FONT_DIR = os.path.join(ROOT, "build", "game113b")
FONT_BYTES = {n.lower().encode(): open(os.path.join(FONT_DIR, n), "rb").read() for n in PD.FONT_FILES}

PORT = None
if "--port" in sys.argv:
    import ctypes
    LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_face.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p,
                               ctypes.c_void_p]
    PORT.ja1f_face.restype = ctypes.c_int32
    PORT.ja1f_fonts.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_fonts.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()


def region(kind):
    """offsets the loader writes: face rows 2..26 x columns 2..29; alive also eye rows 28..31 x columns 0..27"""
    r = {y * 32 + x for y in range(2, 27) for x in range(2, 30)}
    return r | {y * 32 + x for y in range(28, 32) for x in range(28)} if kind == 0 else r


def port_face(buf, n, kind):
    """-> (1024 bytes after ja1f_face on noise, eye row byte or None, the expected 1024 bytes)"""
    noise = bytes(b ^ 0xA5 for b in buf)
    reg = region(kind)
    want = bytes(buf[i] if i in reg else noise[i] for i in range(1024))
    out = ctypes.create_string_buffer(noise, 1024)
    eye = ctypes.c_uint8(0xEE)
    r = PORT.ja1f_face(ISO, EXE, n, kind, ctypes.addressof(out), ctypes.addressof(eye))
    if r:
        return b"return %d" % r + bytes(1024), None, want
    return out.raw, eye.value, want

# exe tables the panel reads (address, size): slot origins, side offsets, sheet elements, char widths, font sizes,
# dying tint colours, ON DUTY texts
EXE_TABLES = [(0x93686, 32), (0x936A6, 20), (0x93732, 105 * 8), (0x949D6, 12), (0x94BF8, 4), (0x9662A, 64)]
SOLDIERS = 0x100958
# remap table 0 byte 0x1B: DrawMercSlot sets it (tint colour) before each tinted draw; table 28 = the sector's
# tileset table (MISC\TILESET%d.LST, loaded with the sector; the panel does not use it)
MASK = {("remap", 0x1B)} | {("remap", 28 * 0x100 + i) for i in range(0x100)}
DUMPS = ["tac60", "fight1", "combat1", "death1", "f5", "a10", "aim_hired", "office", "eod1", "day3", "kill40", "won1",
         "sector59", "inv1"]

img = load()


def exe(addr, n):
    for o in img.objs:
        if o.base <= addr < o.base + o.vsize:
            return bytes(o.data[addr - o.base:addr - o.base + n])
    raise ValueError(hex(addr))


dumps = [a for a in sys.argv[1:] if not a.startswith("--")] or (
    sorted(glob.glob(os.path.join(ROOT, "build", "mem_*.bin"))) if "--all" in sys.argv
    else [os.path.join(ROOT, "build", f"mem_{d}.bin") for d in DUMPS])
main_dat = read(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"))
FACES = decode(member(main_dat, "FACES.PCX"))
DEAD = decode(member(main_dat, "DEADFACE.PCC"))
JAG = cd_read("DAT/JAGGED.DAT")
SHEET, FONTS, REMAP, REMAP_S = PD.panel_sheet(), PD.fonts(), PD.remap_tables(JAG), PD.remap_tables(JAG, True)
CHARS = PD.font_chars(exe(PD.FONT_CHARS, 3 * 0x189))

checks = fails = 0
counts = {}


def cmp(name, dname, a, b, mask=()):
    global checks, fails
    checks += 1
    counts[name] = counts.get(name, 0) + 1
    bad = [i for i in range(len(a)) if a[i] != b[i] and i not in mask]
    if bad:
        fails += 1
        if fails <= 20:
            print(f"MISMATCH {dname} {name}: {len(bad)} bytes, first @{bad[0]:#x} {a[bad[0]]:#x} vs {b[bad[0]]:#x}")


def orig_fonts(D, fp):
    """LoadSmallFont 0x381F8 run for real in Unicorn on this dump, with its two file helpers replaced: FileSize
    0x5273C and ReadWholeFile 0x52768 read the three FNT files from build/game113b (both return the same as the
    original: size, and 0 for a whole read). The sheet pointers, the sheets, the char table and the last-read global
    start as noise (dump byte xor 0xA5), so the function must write the dump's bytes back: a wrong file size, sheet
    offset or char width shows. -> the emulator, ready to compare."""
    e = Emu(dump=D.mem, fast=True)

    def fsize(em, path, *_):
        b = FONT_BYTES.get(em.cstr(path).lower())
        return len(b) if b else 0xFFFFFFFF

    def readw(em, path, dest, size, *_):
        b = FONT_BYTES.get(em.cstr(path).lower())
        if not b:
            return 0xFFFFFFFF
        em.uc.mem_write(dest, b[:size])
        return 0

    e.stub(e.rt(0x5273C), fsize)
    e.stub(e.rt(0x52768), readw)
    e.uc.mem_write(e.rt(0x229790), bytes(x ^ 0xA5 for x in D.read(0x229790, 12)))
    # the char table's 3-byte entries hold a width at +2 (exe data, read and not rewritten) and the x-offset word at
    # +0 (what the function computes): noise only the offset words, so an untouched width byte still compares
    chars = bytearray(D.read(PD.FONT_CHARS, 3 * 0x189))
    chars[0::3] = bytes(x ^ 0xA5 for x in chars[0::3])
    chars[1::3] = bytes(x ^ 0xA5 for x in chars[1::3])
    e.uc.mem_write(e.rt(PD.FONT_CHARS), bytes(chars))
    e.uc.mem_write(e.rt(0x229D18), bytes(x ^ 0xA5 for x in D.read(0x229D18, 4)))
    e.uc.mem_write(fp[0], bytes(x ^ 0xA5 for x in D.mem[fp[0]:fp[0] + sum(PD.FONT_SIZES)]))
    e.call(e.rt(0x381F8))
    return e


for d in dumps:
    dn = os.path.basename(d)[4:-4]
    if not os.path.exists(d):
        print(f"MISSING {d}")
        fails += 1
        continue
    try:
        D = Dump(d)
        D.delta(0x93454)
    except Exception as ex:
        print(f"SKIP {dn}: {ex}")
        continue
    m = D.mem
    g32 = lambda a: struct.unpack_from("<I", D.read(a, 4))[0]
    for a, n in EXE_TABLES:
        cmp(f"exe {a:#x}", dn, D.read(a, n), exe(a, n))
    # char table: widths from the exe, offsets as the loader fills them
    cmp("font chars", dn, D.read(PD.FONT_CHARS, 3 * 0x189), CHARS)
    fp = [g32(0x229790 + 4 * f) for f in range(3)]
    if fp[0]:
        base = D.read(PD.FONT_BASE, 1) and PD.FONT_BASE + D.delta(PD.FONT_BASE)
        cmp("font ptrs", dn, struct.pack("<3I", *fp),
            struct.pack("<3I", base, base + PD.FONT_SIZES[0], base + PD.FONT_SIZES[0] + PD.FONT_SIZES[1]))
        cmp("fonts", dn, m[fp[0]:fp[0] + len(FONTS)], FONTS)
        # the original LoadSmallFont 0x381F8 runs for real here (file helpers stubbed), from noise
        e = orig_fonts(D, fp)
        cmp("orig font ptrs", dn, bytes(e.uc.mem_read(e.rt(0x229790), 12)), struct.pack("<3I", *fp))
        cmp("orig fonts", dn, bytes(e.uc.mem_read(fp[0], sum(PD.FONT_SIZES))),
            m[fp[0]:fp[0] + sum(PD.FONT_SIZES)])
        cmp("orig font chars", dn, bytes(e.uc.mem_read(e.rt(PD.FONT_CHARS), 3 * 0x189)),
            D.read(PD.FONT_CHARS, 3 * 0x189))
        cmp("orig font read", dn, bytes(e.uc.mem_read(e.rt(0x229D18), 4)), bytes(4))
        if PORT:
            # LoadSmallFont 0x381F8's memory image from the port: the three sheets (0xC16 + 0xF72 + 0x24FC bytes) and
            # the computed char table (3 x 0x189: running x offset, width).
            sheets = (ctypes.c_uint8 * sum(PD.FONT_SIZES))()
            chars = (ctypes.c_uint8 * (3 * 0x189))()
            rc = PORT.ja1f_fonts(ISO, EXE, sheets, chars)
            if rc != 0:
                fails += 1
                print(f"MISMATCH {dn} port fonts: rc {rc}")
            else:
                cmp("port fonts", dn, bytes(sheets), m[fp[0]:fp[0] + sum(PD.FONT_SIZES)])
                cmp("port font chars", dn, bytes(chars), D.read(PD.FONT_CHARS, 3 * 0x189))
    rp = [g32(0xE8C48 + 4 * k) for k in range(PD.REMAP_N)]
    if rp[0]:
        r0 = 0xE8D00 + D.delta(0xE8D00)
        cmp("remap ptrs", dn, struct.pack(f"<{PD.REMAP_N}I", *rp),
            struct.pack(f"<{PD.REMAP_N}I", *[r0 + 0x100 * k for k in range(PD.REMAP_N)]))
        got = m[r0:r0 + len(REMAP)]
        sector = got[28 * 0x100:29 * 0x100] != REMAP[28 * 0x100:29 * 0x100]   # a tileset rebuilt table 28
        cmp("remap sector" if sector else "remap", dn, got, REMAP_S if sector else REMAP,
            {i for n, i in MASK if n == "remap"})
    sp = g32(0x93454)
    if not sp:
        continue
    insector = any(m[g32(SOLDIERS + 4 * k) + 0x182] for k in range(8) if g32(SOLDIERS + 4 * k))
    # aim_fired: AIM screen, a merc record still has +0x182 set, and a 59 x 53 box at the sheet's top left is in use
    if (insector and not dn.startswith("aim_")) or m[sp:sp + len(SHEET)] == SHEET:
        cmp("panel sheet", dn, m[sp:sp + len(SHEET)], SHEET)
    else:   # off the tactical screen the sheet is scratch (sunrise picture, money, yes/no dialog, speech balloons)
        counts["sheet in use"] = counts.get("sheet in use", 0) + 1
    for slot in range(8):
        s = g32(SOLDIERS + 4 * slot)
        fb = g32(0x93428 + 4 * slot)
        if not s or not fb or not m[s + 0x182]:
            continue
        n, hp = m[s + 0x179], m[s + 0x183]
        alive = 0 < hp < 0x80
        if PORT:
            buf = m[fb:fb + 1024]
            kind = 0 if alive else 1
            if not alive and port_face(buf, n, 1)[0] != port_face(buf, n, 1)[2]:
                kind = 2
            out, eye, want = port_face(buf, n, kind)
            cmp(f"port face kind {kind}", f"{dn} slot {slot} face {n}", out[:1024], want)
            if alive:
                cmp("port eye row", f"{dn} slot {slot} face {n}", bytes([eye if eye is not None else 0xEE]),
                    m[s + 0x1AA:s + 0x1AB])
            continue
        got, want = b"", b""
        rows = PD.face(n, alive, FACES, DEAD)
        if not alive and any(m[fb + o:fb + o + len(r)] != r for o, r in rows.items()):
            rows, kind = PD.morph_face(n), "face morph"      # died in this sector: last frame of the death morph
        else:
            kind = "face alive" if alive else "face dead"
        for off, row in rows.items():
            got += m[fb + off:fb + off + len(row)]
            want += row
        cmp(kind, f"{dn} slot {slot} face {n}", got, want)
        if not alive and "--v" in sys.argv:
            print(f"dead face: {dn} slot {slot}")
        if alive:
            cmp("eye row", f"{dn} slot {slot} face {n}", m[s + 0x1AA:s + 0x1AB], bytes([PD.eye_row(n, JAG)]))
print("checked: " + ", ".join(f"{k} {v}" for k, v in counts.items()))
print(f"panel_data_diff{' (port)' if PORT else ''}: {len(dumps)} dumps, {checks} checks, {fails} mismatches")
sys.exit(1 if fails else 0)
