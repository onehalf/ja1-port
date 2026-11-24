"""Sir-tech .DAT archive container (MAIN.DAT, OFFICE.DAT, ISLAND.DAT, CD DAT/0.DAT ...): list / extract members.

usage: ./t datarc <file|cd:PATH> [--x OUTDIR]     e.g. ./t datarc rom/DAT/OFFICE.DAT, ./t datarc cd:DAT/0.DAT
Layout (from the files; every archive's last member ends exactly at EOF):
  0x00 char name[16] (the archive's own name)   0x10 u32 cap   0x14 u32 count
  0x20 + 0x20*i: char name[16], u32 offset (absolute), u32 size, 8 zero bytes; data starts at (cap+1)*0x20
"""
import os, struct, sys
from common import ROOT, cd_read


def read(path):
    if path.startswith("cd:"):
        return cd_read(path[3:])
    return open(path if os.path.isabs(path) else os.path.join(ROOT, path), "rb").read()


def members(data):
    """[(name, offset, size)] (count may include empty-named trailing slots, skipped)"""
    n = struct.unpack_from("<I", data, 20)[0]
    out = []
    for i in range(n):
        e = 0x20 + 0x20 * i
        name = data[e:e + 16].split(b"\0")[0].decode("latin-1")
        off, size = struct.unpack_from("<II", data, e + 16)
        if name:
            out.append((name, off, size))
    return out


def member(data, name):
    for n, o, s in members(data):
        if n.upper() == name.upper():
            return data[o:o + s]
    raise KeyError(name)


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    data = read(sys.argv[1])
    out = sys.argv[sys.argv.index("--x") + 1] if "--x" in sys.argv else None
    print(f"{data[:16].split(bytes(1))[0].decode('latin-1')}: cap {struct.unpack_from('<I', data, 16)[0]}")
    for n, o, s in members(data):
        print(f"  {n:16} 0x{o:08X} {s:9}  {data[o:o + 8].hex()}")
        if out:
            os.makedirs(out, exist_ok=True)
            open(os.path.join(out, n), "wb").write(data[o:o + s])


if __name__ == "__main__":
    main()
