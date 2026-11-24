"""PCX (8-bit, RLE) decoder for the game's .PCX/.PCC members -> PNG.

usage: ./t pcx <archive|cd:PATH> <MEMBER|all> [--pal ARCHIVE:MEMBER] [--out DIR]   (default out: gfx/)
  ./t pcx rom/DAT/OFFICE.DAT ROOM.PCX     ./t pcx rom/DAT/MAIN.DAT all --pal rom/DAT/OFFICE.DAT:ROOM.PCX
Members without a trailing 0x0C+768-byte palette (or with --pal) are drawn with the given palette, else grayscale.
VGA palettes in these files are 8-bit (0..255) as stored in standard PCX.
"""
import os, struct, sys, zlib
from common import ROOT
from datarc import read, members, member


def decode(b):
    """-> (w, h, pixels bytes w*h, palette 768 bytes or None)"""
    if b[0] != 0x0A or b[3] != 8 or b[65] != 1:
        raise ValueError(f"not an 8-bit 1-plane PCX (hdr {b[:4].hex()}, planes {b[65]})")
    x0, y0, x1, y1 = struct.unpack_from("<4H", b, 4)
    bpl = struct.unpack_from("<H", b, 66)[0]
    w, h = x1 - x0 + 1, y1 - y0 + 1
    out, i = bytearray(), 128
    need = bpl * h
    while len(out) < need and i < len(b):
        c = b[i]; i += 1
        if c >= 0xC0:
            out += bytes([b[i]]) * (c & 0x3F); i += 1
        else:
            out.append(c)
    rows = b"".join(bytes(out[y * bpl:y * bpl + w]) for y in range(h))
    pal = b[-768:] if len(b) >= 769 and b[-769] == 0x0C else None
    return w, h, rows, pal


def png(path, w, h, pix, pal):
    if pal is None:
        pal = bytes(v for i in range(256) for v in (i, i, i))
    raw = b"".join(b"\0" + pix[y * w:(y + 1) * w] for y in range(h))
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    with open(path, "wb") as f:
        f.write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 3, 0, 0, 0))
                + chunk(b"PLTE", bytes(pal)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def main():
    if len(sys.argv) < 3:
        print(__doc__); return
    data = read(sys.argv[1])
    out = sys.argv[sys.argv.index("--out") + 1] if "--out" in sys.argv else os.path.join(ROOT, "gfx")
    forced = None
    if "--pal" in sys.argv:
        a, m = sys.argv[sys.argv.index("--pal") + 1].rsplit(":", 1)
        forced = decode(member(read(a), m))[3]
    names = [n for n, _, _ in members(data)] if sys.argv[2] == "all" else [sys.argv[2]]
    os.makedirs(out, exist_ok=True)
    for n in names:
        try:
            w, h, pix, pal = decode(member(data, n))
        except ValueError as e:
            print(f"  {n}: skipped ({e})"); continue
        p = os.path.join(out, os.path.splitext(n)[0] + ".png")
        png(p, w, h, pix, forced or pal)
        print(f"  {n}: {w}x{h} {'own palette' if pal and not forced else 'forced palette' if forced else 'grayscale'} -> {os.path.relpath(p, ROOT)}")


if __name__ == "__main__":
    main()
