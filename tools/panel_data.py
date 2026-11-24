"""Data the merc panel draws from, built from the game files (analysis mirror of the loaders, for panel_data_diff).

The panel slot drawing (tools/panel.py, DrawMercSlot? 0x32EB8) reads five run-time buffers. Their loaders:
- panel sheet [0x93454], 320 x 160 at pitch 320: LoadPanels 0x46EC4 = MAIN.DAT PANELS.PCC (PCX).
- face buffer [0x93428 + 4 slot], 32 x 32 per slot: LoadMercFace 0x286C4 (AL slot, DL face = soldier +0x179).
  Alive (hp > 0): MAIN.DAT FACES.PCX (320 x 200, 11 faces per row, cells 28 wide, 29 high); the face (28 x 25 at
  x = 28 (face % 11), y = 29 (face / 11)) goes to buffer row 2, column 2; the 4 eye rows below it (y + 25) go to
  rows 28..31, column 0. Also soldier +0x1AA = JAGGED.DAT byte [face] (eye strip row). Dead: DEADFACE.PCC (28 x 25)
  to row 2, column 2. A merc who dies in the sector (MercDies? 0x3A4A4) instead gets the 10 frames (28 x 25, side by
  side) of MORF<face>.PCC (CD DAT/MF1.DAT for faces < 30, else MF2.DAT) one after the other at row 2, column 2: the
  buffer keeps the last frame (x = 252).
- font sheets [0x229790 + 4 font], fonts 0..2 = JA_small.fnt, JA_m_ENG.fnt, JA_big.fnt (hard disk), read whole into
  0x22570C one after the other (sizes 0xC16, 0xF72, 0x24FC as built into LoadSmallFont 0x381F8). The char table
  0x9453A (3 fonts x 131 x (u16 offset, u8 width)) holds the widths in the exe; the loader fills each offset with
  the sum of the widths before it.
- remap tables [0xE8C48 + 4 k], k = 0..33 = 0xE8D00 + 0x100 k: LoadJaggedDat 0x371BC reads them (0x2200 bytes) from
  JAGGED.DAT at REMAP_OFS, then sets byte 0x1B of table 29 ([0xE8CBC], the darken table) to 0xC6. Each sector load
  (LoadTerrainTileset 0x2A9FC) rebuilds table 28 from the tileset and sets darken bytes 0x27 = 0x2B, 0xF1 = 0xF2.

usage: ./t panel_data            (prints the sizes it built)
"""
import os, struct, sys
from common import ROOT, cd_read
from datarc import read, member
from pcx import decode

FONT_FILES = ["JA_SMALL.FNT", "JA_M_ENG.FNT", "JA_BIG.FNT"]
FONT_SIZES = [0xC16, 0xF72, 0x24FC]
FONT_BASE = 0x22570C
FONT_CHARS = 0x9453A
# JAGGED.DAT: 0x40 header skip, 60 x 2 sector bytes, 60 x 0x30, 60 x 0x28 locks, 60 x 0x28 triggers, 60 x 2, 0x1540
REMAP_OFS = 0x40 + 60 * 2 + 60 * 0x30 + 60 * 0x28 * 2 + 60 * 2 + 0x1540
REMAP_N = 34
DARKEN = 29


def _main_dat():
    return read(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"))


def panel_sheet():
    """-> 320 x 160 pixels (pitch 320)"""
    w, h, pix, _ = decode(member(_main_dat(), "PANELS.PCC"))
    assert (w, h) == (320, 160)
    return pix


def face(n, alive=True, faces=None, dead=None):
    """-> {buffer offset: bytes} the loader writes into a 32 x 32 face buffer for face number n"""
    out = {}
    if alive:
        w, h, pix, _ = faces or decode(member(_main_dat(), "FACES.PCX"))
        x0, y0 = (n % 11) * 28, (n // 11) * 29
        for r in range(25):
            out[(r + 2) * 32 + 2] = pix[(y0 + r) * w + x0:(y0 + r) * w + x0 + 28]
        for r in range(4):
            out[(r + 28) * 32] = pix[(y0 + 25 + r) * w + x0:(y0 + 25 + r) * w + x0 + 28]
    else:
        w, h, pix, _ = dead or decode(member(_main_dat(), "DEADFACE.PCC"))
        for r in range(h):
            out[(r + 2) * 32 + 2] = pix[r * w:(r + 1) * w]
    return out


def morph_face(n):
    """-> {buffer offset: bytes} after the death morph of face n (its last frame)"""
    arc = read("cd:DAT/MF1.DAT" if n < 30 else "cd:DAT/MF2.DAT")
    w, h, pix, _ = decode(member(arc, f"MORF{n}.PCC"))
    return {(r + 2) * 32 + 2: pix[r * w + 252:r * w + 280] for r in range(25)}


def eye_row(n, jagged=None):
    """soldier +0x1AA after LoadMercFace: JAGGED.DAT byte n"""
    return (jagged or cd_read("DAT/JAGGED.DAT"))[n]


def fonts():
    """-> bytes of the three font sheets, laid out from FONT_BASE"""
    out = b""
    for f, n in zip(FONT_FILES, FONT_SIZES):
        b = open(os.path.join(ROOT, "rom", f), "rb").read()
        assert len(b) == n, f
        out += b
    return out


def font_chars(exe_table):
    """exe char table (3 x 0x189 bytes) -> the table after the loader filled the offsets"""
    t = bytearray(exe_table)
    for f in range(3):
        off = 0
        for c in range(131):
            a = f * 0x189 + c * 3
            struct.pack_into("<H", t, a, off & 0xFFFF)
            off += t[a + 2]
    return bytes(t)


def remap_tables(jagged=None, sector=False):
    """-> 34 x 256 bytes (table k at 0xE8D00 + 0x100 k); sector = after a sector load (table 28 not rebuilt here)"""
    j = jagged or cd_read("DAT/JAGGED.DAT")
    t = bytearray(j[REMAP_OFS:REMAP_OFS + REMAP_N * 0x100])
    t[DARKEN * 0x100 + 0x1B] = 0xC6
    if sector:
        t[DARKEN * 0x100 + 0x27] = 0x2B
        t[DARKEN * 0x100 + 0xF1] = 0xF2
    return bytes(t)


if __name__ == "__main__":
    print(f"panel sheet {len(panel_sheet())} B, fonts {len(fonts())} B, remap {len(remap_tables())} B, "
          f"face 0 rows {len(face(0))}, JAGGED.DAT remap at {REMAP_OFS:#x}")
