"""Hex+ASCII dump of the loaded (fixed-up) LE image.

usage: ./t peek <addr|label> [len=64]
"""
import sys
from le import load
from common import load_labels, parse_addr


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    img, labels = load(), load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    a = by_name[sys.argv[1]] if sys.argv[1] in by_name else parse_addr(sys.argv[1])
    n = int(sys.argv[2], 0) if len(sys.argv) > 2 else 64
    o = img.obj_at(a)
    if o is None:
        sys.exit(f"0x{a:X} not mapped")
    b = bytes(o.data[a - o.base:a - o.base + n])
    for i in range(0, len(b), 16):
        row = b[i:i + 16]
        print(f"{a + i:08X}  {row.hex(' '):47}  " + "".join(chr(c) if 32 <= c < 127 else "." for c in row))


if __name__ == "__main__":
    main()
