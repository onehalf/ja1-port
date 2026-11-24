"""Strategic sector table g_sectorTbl9 (0x22979C, 60 x 9 B) from a DOSBox-X dump or a day/game save.

usage: ./t sectors build/mem_NAME.bin|live [--check]
b0: bit0 owned, bits 1-4 = per-side barrier (2 N, 4 E, 8 S, 0x10 W; set in pairs with the neighbour by 0x3DE20);
b1 enemies, b2 our mercs, b3/b4 guard counts, b6 resource total, b7 workable today (0x41E1C), b8 used/lost.
--check recomputes b7 with SectorConnectedToHome 0x41178 + SectorWorkable 0x41E1C and reports mismatches.
"""
import sys
from jatypes import Types
from memdump import source
import ai

G_SECTOR_TBL9 = 0x22979C


def connected(d, s):
    """0x41178 exactly (the wall-follow walk of tools/ai.py; a plain search differs on rare maps)."""
    return ai.connected_home(_Mem(d), G_SECTOR_TBL9, s)


class _Mem:
    def __init__(self, d):
        self.d = d

    def g8(self, a):
        return self.d.read(a, 1)[0]


def workable(d, t, s):
    """0x41E1C: 25 % security per safe side (map edge, owned neighbour or barrier bit), b7 = min(b6-b8, b6*sec/100)."""
    b = t[s]
    if not b[0] & 1 or not connected(d, s):
        return 0
    own = lambda n: t[n][0] & 1
    sec = 0
    sec += 25 if s < 10 or own(s - 10) or b[0] & 2 else 0
    sec += 25 if s % 10 == 9 or own(s + 1) or b[0] & 4 else 0
    sec += 25 if s > 49 or own(s + 10) or b[0] & 8 else 0
    sec += 25 if s % 10 == 0 or own(s - 1) or b[0] & 0x10 else 0
    return min((b[6] - b[8]) & 0xFF, b[6] * sec // 100 & 0xFF)


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    if a[0].upper().endswith(".SAV"):
        sys.exit("save files: TODO (offset of g_sectorTbl9 in the day-save layout)")
    d = source(a[0])
    t = [s.raw for s in Types().view(d, "g_sectorTbl9")]
    for i, b in enumerate(t):
        if any(b):
            line = f"{i:2d} (UI {i + 1:2d}) " + " ".join(f"{x:3d}" for x in b) + (" owned" if b[0] & 1 else "")
            if "--check" in a:
                w = workable(d, t, i)
                line += "" if w == b[7] else f"   b7 MISMATCH calc {w}"
            print(line)


if __name__ == "__main__":
    main()
