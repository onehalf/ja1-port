"""Compiled sprites: TILES?.DAT (tactical tiles) and SINV.DAT are banks of x86 routines, not bitmaps.

usage: ./t csprite FILE [--pal N|FILE] [--out PNG] [--info]
  FILE: cd:DAT/TILES0.DAT, rom/DAT/..., or a local path.  --pal: tileset number (cd:MISC/TILE%d.PAL, 6-bit VGA)
  default output gfx/<name>.png = contact sheet (32 per row, 0..N-1 left to right), transparent = palette 0 swapped
  for magenta.  --info prints per-sprite size and bounding box.

Container (read by the loader 0x52E00, which OpenTilesDat 0x36EA0 calls): u32 N, N x u32 offsets relative to the end
of the table; the loader drops the count and turns each offset into a pointer in place. Drawing = DrawCompiledSprite
0x52DC8(EAX=dest, EDX=sprite, EBX=pitch, ECX=x, [esp]=y): EDI = dest + y*pitch + x, `call` the sprite. Each sprite
writes its opaque pixels with mov byte/word/dword [edi+k], imm and steps rows with add edi, ebx; ret, then a
nop + u16 w + u16 h trailer (not always present). Here the sprite is *executed* in Unicorn and its memory writes are
recorded, so every pixel comes from the original code path.
"""
import os, struct, sys
from unicorn import Uc, UcError, UC_ARCH_X86, UC_MODE_32, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_EDI, UC_X86_REG_EBX, UC_X86_REG_ESP
from common import ROOT, cd_read

CODE, CANVAS, STACK, STOP = 0x100000, 0x400000, 0x300000, 0x280000
PITCH, OX, OY, CW, CH = 0x400, 0x200, 0x200, 0x400, 0x400   # scratch canvas, sprite origin in its middle


def read_file(spec):
    if spec.startswith("cd:"):
        return cd_read(spec[3:])
    return open(spec if os.path.isabs(spec) else os.path.join(ROOT, spec), "rb").read()


def bank(data):
    """-> list of sprite code blobs (each up to the next offset / EOF)"""
    n = struct.unpack_from("<I", data)[0]
    offs = struct.unpack_from(f"<{n}I", data, 4)
    base = 4 + 4 * n
    ends = list(offs[1:]) + [len(data) - base]
    return [data[base + a:base + b] for a, b in zip(offs, ends)]


def palette(spec):
    """6-bit VGA palette file -> 768 bytes 8-bit"""
    raw = read_file(f"cd:MISC/TILE{int(spec)}.PAL" if str(spec).isdigit() else spec)[:768]
    return bytes(min(255, v << 2 | v >> 4) for v in raw)


class Runner:
    def __init__(self):
        self.mu = Uc(UC_ARCH_X86, UC_MODE_32)
        for a, s in ((CODE, 0x100000), (STOP & ~0xFFF, 0x1000), (STACK - 0x10000, 0x10000), (CANVAS, CW * CH)):
            self.mu.mem_map(a, s)
        self.mu.mem_write(STOP, b"\xf4")
        self.writes = {}
        self.mu.hook_add(UC_HOOK_MEM_WRITE, self._w, begin=CANVAS, end=CANVAS + CW * CH - 1)

    def _w(self, mu, access, addr, size, value, data):
        for i in range(size):
            self.writes[addr - CANVAS + i] = (value >> (8 * i)) & 0xFF

    def run(self, blob):
        """-> {(x, y): colour} relative to the draw origin"""
        self.writes = {}
        mu = self.mu
        mu.mem_write(CODE, blob + b"\xf4")
        mu.ctl_remove_cache(CODE, CODE + len(blob) + 1)   # else Unicorn reruns the previous sprite's TB
        mu.reg_write(UC_X86_REG_EDI, CANVAS + OY * PITCH + OX)
        mu.reg_write(UC_X86_REG_EBX, PITCH)
        mu.reg_write(UC_X86_REG_ESP, STACK - 4)
        mu.mem_write(STACK - 4, struct.pack("<I", STOP))
        mu.emu_start(CODE, STOP, count=200000)
        return {(o % PITCH - OX, o // PITCH - OY): v for o, v in self.writes.items()}


def trailer(blob):
    """(w, h) from the nop+u16+u16 trailer after the last ret, if it looks sane"""
    if len(blob) >= 6 and blob[-5] == 0x90 and blob[-6] == 0xC3:
        w, h = struct.unpack_from("<HH", blob, len(blob) - 4)
        if 0 < w <= 320 and 0 < h <= 200:
            return w, h
    return None


def render_all(blobs):
    """Entries that are not code (TILES?.DAT 1293.. = raw 16x14 bitmaps, drawn by the remap blitter in 0x163F8)
    are shown as raw 16-wide bitmaps."""
    r = Runner()
    out = []
    for b in blobs:
        try:
            out.append(r.run(b))
        except UcError:
            out.append({(i % 16, i // 16): v for i, v in enumerate(b)})
    return out


def sheet(sprites, cell=None, cols=32, bg=0):
    """-> (w, h, pixels) contact sheet; cell = (w, h) or the max bbox"""
    boxes = [(min(x for x, _ in s), min(y for _, y in s), max(x for x, _ in s), max(y for _, y in s))
             if s else (0, 0, 0, 0) for s in sprites]
    x0 = min(0, *[b[0] for b in boxes]); y0 = min(0, *[b[1] for b in boxes])
    cw = cell[0] if cell else max(b[2] for b in boxes) - x0 + 1
    ch = cell[1] if cell else max(b[3] for b in boxes) - y0 + 1
    rows = (len(sprites) + cols - 1) // cols
    W, H = cols * (cw + 1), rows * (ch + 1)
    pix = bytearray([bg]) * (W * H)
    for k, s in enumerate(sprites):
        bx, by = (k % cols) * (cw + 1) - x0, (k // cols) * (ch + 1) - y0
        for (x, y), v in s.items():
            if 0 <= x - x0 < cw and 0 <= y - y0 < ch:
                pix[(by + y) * W + bx + x] = v
    return W, H, bytes(pix)


def main(argv):
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__); return
    spec = argv[0]
    pal = None
    out = None
    if "--pal" in argv:
        pal = palette(argv[argv.index("--pal") + 1])
    if "--out" in argv:
        out = argv[argv.index("--out") + 1]
    blobs = bank(read_file(spec))
    sprites = render_all(blobs)
    if "--info" in argv:
        for k, (b, s) in enumerate(zip(blobs, sprites)):
            bb = (min(x for x, _ in s), min(y for _, y in s), max(x for x, _ in s), max(y for _, y in s)) if s else None
            print(f"{k:4d} code {len(b):5d} px {len(s):4d} bbox {bb} trailer {trailer(b)}")
        return
    from pcx import png
    if pal is None:
        pal = bytes(v for i in range(256) for v in (i, i, i))
    pal = bytearray(pal); pal[0:3] = b"\xff\x00\xff"      # sheet background (never written: sprites skip it)
    w, h, pix = sheet(sprites)
    out = out or os.path.join(ROOT, "gfx", os.path.splitext(os.path.basename(spec))[0].lower() + ".png")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    png(out, w, h, pix, bytes(pal))
    print(f"{len(sprites)} sprites -> {out} ({w}x{h})")


if __name__ == "__main__":
    main(sys.argv[1:])
