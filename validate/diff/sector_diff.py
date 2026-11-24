"""Differential test: spec.py vs the original JA.EXE functions in Unicorn, on JAGGED.DAT sector data + fuzz.

usage: .venv/bin/python validate/diff/sector_diff.py [nfuzz=200]
Covers TileToX/TileToY, SectorDoorLookup?, SectorLockLookup?, CheckSectorTileTriggers (its two callees stubbed
and recorded). Exits 1 on any mismatch.
"""
import os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path[:0] = [HERE, os.path.join(HERE, "..", "..", "tools")]
import spec                          # noqa: E402
from emu import Emu                  # noqa: E402
from common import cd_read           # noqa: E402

G_SECTORS, G_CUR_SECTOR = 0x222B74, 0x22A039
TILE_TO_X, TILE_TO_Y, DOOR, LOCK, TRIGGERS = 0x15E50, 0x15E6C, 0x3AFE0, 0x3B430, 0x19FA0
FIRE, REWARD = 0x19F30, 0x62A68

nfuzz = int(sys.argv[1]) if len(sys.argv) > 1 else 200
rng = random.Random(1)
e = Emu()
fails = 0


def check(what, got, want, ctx):
    global fails
    if got != want:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH {what} {ctx}: original {got!r} spec {want!r}")


def s32(v): return v - (1 << 32) if v & 0x80000000 else v


# --- TileToX / TileToY: every map tile, sentinels and negatives
for t in list(range(-200, 3137 + 200)) + [rng.randrange(-1 << 31, 1 << 31) for _ in range(500)]:
    check("TileToX", s32(e.call(TILE_TO_X, t & 0xFFFFFFFF)), spec.tile_to_x(t), t)
    check("TileToY", s32(e.call(TILE_TO_Y, t & 0xFFFFFFFF)), spec.tile_to_y(t), t)

# --- sector records: the 60 from JAGGED.DAT (blocks A/B/C at 0xB8/0xBF8/0x1558), then random ones
jd = cd_read("DAT/JAGGED.DAT")
recs = []
for i in range(60):
    r = bytearray(spec.SECTOR_SIZE)
    r[0x00:0x30] = jd[0xB8 + i * 0x30:0xB8 + (i + 1) * 0x30]
    r[0x30:0x58] = jd[0xBF8 + i * 0x28:0xBF8 + (i + 1) * 0x28]
    r[0x58:0x80] = jd[0x1558 + i * 0x28:0x1558 + (i + 1) * 0x28]
    recs.append((i, bytes(r)))
for _ in range(nfuzz):
    r = bytearray(rng.randbytes(spec.SECTOR_SIZE))
    pool = [rng.choice([0x0C40, 0x0C41, 0, 0xFFFF, rng.randrange(3136)]) for _ in range(4)]
    for k in range(0, 0x80, 4):                      # reuse a few tiles so lookups hit and repeat
        if rng.random() < 0.5:
            struct.pack_into("<H", r, k, rng.choice(pool))
    recs.append((rng.randrange(60), bytes(r)))

calls = []
e.stub(FIRE, lambda e, a, d, b, c: calls.append(("fire", a, d, b & 0xFF)))
e.stub(REWARD, lambda e, a, d, b, c: calls.append(("reward", a, d, b, c)))
merc = e.alloc(0x200)

for sec, rec in recs:
    e.w8(G_CUR_SECTOR, sec)
    e.uc.mem_write(G_SECTORS + sec * spec.SECTOR_SIZE, rec)
    tiles = {struct.unpack_from("<h", rec, k)[0] for k in range(0, 0x80, 4)}
    tiles |= {0x0C40, 0x0C41, -1, 0, rng.randrange(3136), 70000}
    for t in sorted(tiles):
        check("DoorLookup", s32(e.call(DOOR, t & 0xFFFFFFFF)), spec.sector_door_lookup(rec, t), (sec, t))
        check("LockLookup", s32(e.call(LOCK, t & 0xFFFFFFFF)), spec.sector_lock_lookup(rec, t), (sec, t))
        for level in (1, 2, 5, 10, 0, -3, 127, -128, rng.randrange(-128, 128)):
            e.w16(merc + 0x114, t)
            e.w8(merc + 0x192, level)
            calls.clear()
            e.call(TRIGGERS, merc)
            want = []
            for i, reward in spec.check_sector_tile_triggers(rec, t, level):
                want += [("fire", merc, i, 0), ("reward", merc, 4, reward, 0)]
            check("TileTriggers", calls, want, (sec, t, level))

n = len(recs)
print(f"TileToX/Y: {3537 + 500} tile values; Door/Lock/TileTriggers: {n} records ({n - nfuzz} JAGGED.DAT + {nfuzz} fuzz)")
print("PASS" if not fails else f"FAIL: {fails} mismatches")
sys.exit(1 if fails else 0)
