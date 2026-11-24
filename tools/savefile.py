"""Parse a JA save file into named blocks (layout read from the original's own save code), check it round-trips.

usage: ./t savefile SAVE [--blocks] [--check]
  SAVE = day save (game%d.sav, DAY.SAV, OFFICE.SAV, QICKSTRT.SAV) or mid-day quick save (QUICK.SAV); detected by size.
  default: summary (day, name, funds, difficulty, team, owned sectors, sector files, stash, replay records)
  --blocks: every block with file offset, memory address, size, label
  --check: parse + re-serialise and require identical bytes (exit 1 otherwise)

Layout = the fwrite_ sequence of SaveGame 0x4EBC8 (day) / QuickSave 0x2D880 (quick), taken from the binary at run
time by ioseq.calls(), then the variable tail both share:
  sector files: one 18,080-byte copy of sec%02d.tmp (0xC40 tiles x 5 B + 0x960 B ground items) per sector whose
    g_sectors[i].hasTmp (+0xA9) is set, in sector order (SaveSectorTmps 0x4E740)
  stash: g_stashCounts (0x88 B, one count per item type) + 3 B (type, qty, condition) per counted item
    (SaveItemStash 0x51590)
  quick save only: REPLAY.REC copy, 3 B x the s16 at 0x22A46E (OpenReplayRec 0x2C834)
"""
import struct, sys
from common import load_labels
from ioseq import calls
from jatypes import Types

SAVE_GAME, QUICK_SAVE = 0x4EBC8, 0x2D880
G_SECTORS, SECTOR_SIZE, HAS_TMP = 0x222B74, 0xB1, 0xA9
TMP_SIZE = 0xC40 * 5 + 0x960
STASH_TYPES = 0x88
REPLAY_COUNT = 0x22A46E


def layout(fn):
    """[(addr, size)] of the fixed part, from the fwrite_ calls of the save function."""
    out = []
    for _, kind, buf, size, n in calls(fn):
        if kind != "W":
            continue
        if not all(isinstance(v, int) for v in (buf, size, n)):
            raise SystemExit(f"unresolved fwrite_ in 0x{fn:X}: buf={buf} size={size} n={n}")
        out.append((buf, size * n))
    return out


class Save:
    def __init__(self, data):
        self.data = data
        for kind, fn in (("day", SAVE_GAME), ("quick", QUICK_SAVE)):
            try:
                self._parse(kind, layout(fn))
                return
            except ValueError:
                continue
        raise SystemExit(f"not a day or quick save ({len(data)} bytes)")

    def _parse(self, kind, fixed):
        d, o = self.data, 0
        self.kind, self.blocks = kind, []           # (name, file offset, addr, bytes)
        for addr, size in fixed:
            if o + size > len(d):
                raise ValueError
            self.blocks.append(("", o, addr, d[o:o + size]))
            o += size
        self.mem = {addr: b for _, _, addr, b in self.blocks}
        sectors = self.mem[G_SECTORS]
        self.tmps = []
        for i in range(len(sectors) // SECTOR_SIZE):
            if sectors[i * SECTOR_SIZE + HAS_TMP]:
                if o + TMP_SIZE > len(d):
                    raise ValueError
                self.tmps.append((i, o, d[o:o + TMP_SIZE]))
                o += TMP_SIZE
        if o + STASH_TYPES > len(d):
            raise ValueError
        counts = d[o:o + STASH_TYPES]
        n = sum(counts)
        self.stash_off, self.stash = o, (counts, d[o + STASH_TYPES:o + STASH_TYPES + 3 * n])
        o += STASH_TYPES + 3 * n
        self.replay = b""
        if kind == "quick":
            cnt = struct.unpack("<h", self.mem[REPLAY_COUNT])[0]
            self.replay = d[o:o + 3 * cnt]
            o += 3 * cnt
        if o != len(d) or len(self.stash[1]) != 3 * n:
            raise ValueError

    def serialise(self):
        return (b"".join(b for _, _, _, b in self.blocks) + b"".join(t for _, _, t in self.tmps) +
                self.stash[0] + self.stash[1] + self.replay)

    def reader(self):
        """Bytes-like reader over the fixed blocks at their memory addresses (for jatypes views)."""
        return BlockReader(self.mem)


class BlockReader:
    def __init__(self, mem):
        self.mem = mem

    def read(self, addr, n):
        for a, b in self.mem.items():
            if a <= addr and addr + n <= a + len(b):
                return b[addr - a:addr - a + n]
        raise KeyError(f"0x{addr:X}+{n} not in the save")


def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    if not a:
        sys.exit(__doc__)
    data = open(a[0], "rb").read()
    s = Save(data)
    if "--check" in sys.argv:
        ok = s.serialise() == data
        print(f"{a[0]}: {s.kind} save, {len(s.blocks)} blocks, {len(s.tmps)} sector files, round trip "
              f"{'identical' if ok else 'DIFFERS'}")
        sys.exit(0 if ok else 1)
    labels = load_labels()
    if "--blocks" in sys.argv:
        for _, off, addr, b in s.blocks:
            print(f"  0x{off:05X}  0x{addr:06X} {len(b):6d}  {labels.get(addr, ('',))[0]}")
        for i, off, _ in s.tmps:
            print(f"  0x{off:05X}  sec{i:02d}.tmp {TMP_SIZE:6d}")
        print(f"  0x{s.stash_off:05X}  stash   {STASH_TYPES + len(s.stash[1]):6d}  ({len(s.stash[1]) // 3} items)")
        if s.kind == "quick":
            print(f"  0x{len(data) - len(s.replay):05X}  replay  {len(s.replay):6d}")
        return
    T, r = Types(), s.reader()
    name = s.mem.get(T.addr_of("g_saveName"), b"").split(b"\0")[0].decode("latin-1")
    soldiers = T.view(r, "g_soldiers")
    team = [soldiers[i].nickname for i in range(8) if soldiers[i].hpMax]
    owned = [i + 1 for i, st in enumerate(T.view(r, "g_sectorTbl9")) if st.flags & 1]
    cur = T.view(r, "g_curSector")
    print(f"{a[0]}: {s.kind} save, {len(data)} bytes")
    print(f"  day {T.view(r, 'g_day')}  name {name!r}  funds {T.view(r, 'g_balance')}  "
          f"difficulty {T.view(r, 'g_difficulty')}  current sector {cur + 1 if cur != 0xFF else 'none'}")
    print(f"  team: {', '.join(team)}")
    print(f"  owned sectors (as shown in game): {owned}")
    print(f"  sector files (as shown): {[i + 1 for i, _, _ in s.tmps]}  stash: {len(s.stash[1]) // 3} items  "
          f"replay records: {len(s.replay) // 3}")


if __name__ == "__main__":
    main()
