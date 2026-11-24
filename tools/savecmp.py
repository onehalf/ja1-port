"""Compare a day save (game%d.sav / DAY.SAV / OFFICE.SAV, written by SaveGame 0x4EBC8) with a DOSBox-X dump, block by block.

usage: ./t savecmp SAVE build/mem_NAME.bin [--tmpdir validate/run/C]
Layout: day 2, name 0x1A, 8 soldiers, profiles, g_sectorTbl9, g_sectors, 0x229FA0 0x356, 0x1163B8 0x14, 0x92DF0 0x3D,
0x92ED2 0x1E (= 0x5491 B), then one sec%02d.tmp copy (18,080 B) per sector with g_sectors+0xA9 set (0x4E740), then
the item stash: 0x88 counts (g_stashCounts 0x93DD4) + 3 B per item (SaveItemStash 0x51590).
"""
import os
import sys
from memdump import Dump

BLOCKS = [("day", 0x229FA4, 2), ("name", 0x28EDFE, 0x1A), ("soldiers", 0xFA958, 0x1000), ("profiles", 0xEBB40, 0x1518),
          ("g_sectorTbl9", 0x22979C, 0x21C), ("g_sectors", 0x222B74, 0x297C), ("0x229FA0", 0x229FA0, 0x356),
          ("0x1163B8", 0x1163B8, 0x14), ("0x92DF0", 0x92DF0, 0x3D), ("0x92ED2", 0x92ED2, 0x1E)]
TMP = 3136 * 5 + 0x960


def main():
    a = sys.argv[1:]
    if len(a) < 2:
        sys.exit(__doc__)
    f = open(a[0], "rb").read()
    d = Dump(a[1])
    tmpdir = a[a.index("--tmpdir") + 1] if "--tmpdir" in a else None
    o = 0
    for name, addr, size in BLOCKS:
        m, s = d.read(addr, size), f[o:o + size]
        diff = [i for i in range(size) if m[i] != s[i]]
        print(f"{name:13s} file 0x{o:04X} +0x{size:X}: {len(diff)} diffs " + " ".join(f"+0x{i:X}:{m[i]:02x}/{s[i]:02x}" for i in diff[:10]))
        o += size
    secs = f[0x2750:0x2750 + 0x297C]           # the save's own g_sectors decides which sector files follow
    for i in range(60):
        if secs[i * 0xB1 + 0xA9]:
            blob = f[o:o + TMP]
            note = ""
            if tmpdir:
                p = os.path.join(tmpdir, f"SEC{i:02d}.TMP")
                note = (" == " if os.path.exists(p) and open(p, "rb").read() == blob else " != ") + p
            print(f"sec{i:02d}.tmp     file 0x{o:04X} +0x{TMP:X}{note}")
            o += TMP
    counts = f[o:o + 0x88]
    n = sum(counts)
    print(f"stash         file 0x{o:04X}: {n} items, counts {'==' if counts == d.read(0x93DD4, 0x88) else '!='} memory; "
          f"end 0x{o + 0x88 + 3 * n:X} vs file size 0x{len(f):X}")


if __name__ == "__main__":
    main()
