"""List soldier slots (g_soldiers, 48 x soldier) from a DOSBox-X dump or the running game (live).

usage: ./t soldiers build/mem_NAME.bin|live [--all] [--raw OFF LEN]
Shows occupied slots (tile < 0xC40 or health > 0); --all shows every slot; --raw adds bytes [OFF, OFF+LEN) per slot.
Fields: defs/types.toml (struct soldier).
"""
import sys
from jatypes import Types
from memdump import source


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    d = source(a[0])
    T = Types()
    show_all = "--all" in a
    raw = None
    if "--raw" in a:
        i = a.index("--raw")
        raw = (int(a[i + 1], 0), int(a[i + 2], 0))
    print(f"g_curSector = {T.view(d, 'g_curSector')}")
    for k, s in enumerate(T.view(d, "g_soldiers")):
        if not show_all and s.tile >= 0xC40 and s.hp == 0:
            continue
        stats = [s.energy, s.agility, s.dexterity, s.wisdom, s.medical, s.explosives, s.mechanical, s.marksmanship,
                 s.level]
        line = (f"{k:2d} {s.nickname:10s} tile {s.tile:4d} ({s.tile % 56:2d},{s.tile // 56:2d}) xy {s.x:4d},{s.y:4d} "
                f"hp {s.hp:3d}/{s.hpMax:3d} stats {' '.join(f'{b:3d}' for b in stats)} "
                f"slot {s.slot:2d} prof {s.profileId:2d} st {s.status}")
        if raw:
            line += " | " + s.raw[raw[0]:raw[0] + raw[1]].hex(" ")
        print(line)


if __name__ == "__main__":
    main()
