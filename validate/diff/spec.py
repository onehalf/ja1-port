"""Reference reimplementations of JA.EXE functions, traced from the asm (address in each docstring).

Plain Python with the original's integer semantics (C truncating division, i16/i8 sign extension, loop bounds
as in the binary, quirks included). Each one is checked against the original running in Unicorn by the
*_diff.py scripts next to this file; the port translates these, not the other way round.
"""
import struct

SECTOR_SIZE = 0xB1      # g_sectors record (0x222B74, 60 records)
MAP_W = 56              # sector map is 56 x 56 tiles


def _s16(b, o): return struct.unpack_from("<h", b, o)[0]
def _u16(b, o): return struct.unpack_from("<H", b, o)[0]


def _cdiv(a, b):
    """C/idiv division: truncates toward zero; returns (quotient, remainder)."""
    q = abs(a) // abs(b) * (1 if (a < 0) == (b < 0) else -1)
    return q, a - q * b


def tile_to_x(tile):
    """0x15E50 TileToX: screen x of a tile = (tile % 56) * 16 + 5."""
    return _cdiv(tile, MAP_W)[1] * 16 + 5


def tile_to_y(tile):
    """0x15E6C TileToY: screen y of a tile = (tile / 56) * 14 + 5."""
    return _cdiv(tile, MAP_W)[0] * 14 + 5


def _lookup(rec, base, tile):
    # 12 (i16 tile, i16 value) pairs from rec+base; first match wins, 0 if none
    for i in range(12):
        if _s16(rec, base + 4 * i) == tile:
            return _s16(rec, base + 4 * i + 2)
    return 0


def sector_door_lookup(rec, tile):
    """0x3AFE0 SectorDoorLookup?(EAX=tile), rec = g_sectors[g_curSector]: block A at +0x00 (12 pairs)."""
    return _lookup(rec, 0x00, tile)


def sector_lock_lookup(rec, tile):
    """0x3B430 SectorLockLookup?(EAX=tile): block B at +0x30. Quirk: the loop also runs 12 times, but B holds only
    10 pairs (0x28 B), so pairs 10-11 are the first two tile triggers of block C (+0x58)."""
    return _lookup(rec, 0x30, tile)


def check_sector_tile_triggers(rec, merc_tile, level):
    """0x19FA0 CheckSectorTileTriggers(EAX=merc). For each of the 10 trigger pairs in block C (+0x58) whose tile
    equals the merc's tile (u16 merc+0x114), compare the merc's experience level (i8 merc+0x192) with the value:
      value >= 0: fires if level >= value, reward 10;  value < 0: fires if level <= -value, reward 5.
    A firing calls FireSectorTrigger(merc, i, 0) then 0x62A68(merc, 4, reward, 0). Returns [(i, reward), ...]."""
    out = []
    for i in range(10):
        o = 0x58 + 4 * i
        if _u16(rec, o) != merc_tile & 0xFFFF:
            continue
        v = _s16(rec, o + 2)
        if v < 0:
            if level <= -v:
                out.append((i, 5))
        elif level >= v:
            out.append((i, 10))
    return out
