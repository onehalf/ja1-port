"""Screen positions of soldiers in tactical (for driving the original), from a dump or the running game (live).

usage: ./t tacview build/mem_NAME.bin|live [--center WX WY]
View origin = g_viewX/g_viewY (world px, one arrow key = 16 / 14 px); the 320x200 game screen shows world
(x, y) at (x - vx + 48, y - vy + 12) (sprite foot), play window x 48..271, y 20..189.
--center prints the arrow keys that put world (WX, WY) near (160, 105).
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
    vx, vy = T.view(d, "g_viewX"), T.view(d, "g_viewY")
    print(f"view {vx},{vy}  sector {T.view(d, 'g_curSector')}")
    S = T.view(d, "g_soldiers")
    for k in list(range(16)) + list(range(0x28, 0x30)):
        s = S[k]
        if s.tile >= 0xC40:
            continue
        sx, sy = s.x - vx + 48, s.y - vy + 12
        vis = "on " if 48 <= sx < 272 and 20 <= sy < 190 else "off"
        print(f"{k:2d} {s.nickname:10s} ({s.tile % 56:2d},{s.tile // 56:2d}) hp {s.hp:3d}/{s.hpMax:3d} "
              f"screen {sx:4d},{sy:4d} {vis} item0 {s.items[0].type:02x}/{s.items[0].qty}")
    if "--center" in a:
        i = a.index("--center")
        wx, wy = int(a[i + 1]), int(a[i + 2])
        dx = round((wx - vx + 48 - 160) / 16)
        dy = round((wy - vy + 12 - 105) / 14)
        keys = (["Right"] * dx if dx > 0 else ["Left"] * -dx) + (["Down"] * dy if dy > 0 else ["Up"] * -dy)
        print("KEYS " + " ".join(keys))


if __name__ == "__main__":
    main()
