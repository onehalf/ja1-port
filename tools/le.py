"""Linear Executable (LE, DOS/4GW) parser + loader for rom/JA.EXE.

usage: ./t le                 # print header, objects, fixup stats
       ./t le --fixups N      # also print the first N fixups

As a module: `load()` returns an LEImage with objects mapped at their preferred
(relocation base) addresses and all internal fixups applied, so linear addresses
match Ghidra's view of build/JA.elf.
"""
import os, struct, sys
from collections import Counter

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
EXE = os.path.join(ROOT, "rom", "JA.EXE")

# fake selectors for 16-bit selector fixups (flat model: one per object)
SEL_BASE = 0x1000

SRC_TYPES = {0: "byte", 2: "sel16", 3: "ptr16:16", 5: "off16", 6: "ptr16:32", 7: "off32", 8: "rel32"}


class Obj:
    def __init__(self, idx, vsize, base, flags, page_idx, npages):
        self.idx, self.vsize, self.base, self.flags = idx, vsize, base, flags
        self.page_idx, self.npages = page_idx, npages
        self.file_ranges = []      # [(obj_off, file_off, size)]
        self.data = bytearray()

    @property
    def kind(self):
        return "code" if self.flags & 0x4 else "data"

    @property
    def perms(self):
        return ("r" if self.flags & 1 else "-") + ("w" if self.flags & 2 else "-") + ("x" if self.flags & 4 else "-")

    def __repr__(self):
        return (f"obj{self.idx} {self.kind} base=0x{self.base:08X} vsize=0x{self.vsize:X} "
                f"flags=0x{self.flags:04X} {self.perms}{' big' if self.flags & 0x2000 else ''} "
                f"pages={self.page_idx}+{self.npages}")


class Fixup:
    __slots__ = ("src", "stype", "obj", "off", "addr")

    def __init__(self, src, stype, obj, off, addr):
        self.src, self.stype, self.obj, self.off, self.addr = src, stype, obj, off, addr

    def __repr__(self):
        return f"0x{self.src:08X} {SRC_TYPES.get(self.stype, self.stype):8} -> obj{self.obj}+0x{self.off:X} (0x{self.addr:08X})"


class LEImage:
    def __init__(self, path=EXE):
        self.path = path
        d = self.raw = open(path, "rb").read()
        # bound DOS/4G stub: walk the MZ chain until the 'LE' header
        self.le, self.mz_base = self._find_le(d)
        h = self.le
        u32 = lambda o: struct.unpack_from("<I", d, h + o)[0]
        self.hdr = {
            "cpu": struct.unpack_from("<H", d, h + 0x08)[0], "os": struct.unpack_from("<H", d, h + 0x0A)[0],
            "flags": u32(0x10), "npages": u32(0x14), "eip_obj": u32(0x18), "eip": u32(0x1C),
            "esp_obj": u32(0x20), "esp": u32(0x24), "page_size": u32(0x28), "last_page": u32(0x2C),
            "fixup_size": u32(0x30), "obj_tab": u32(0x40), "nobjs": u32(0x44), "page_map": u32(0x48),
            "iter_pages": u32(0x4C), "res_names": u32(0x58), "entry_tab": u32(0x5C),
            "fixup_pages": u32(0x68), "fixup_recs": u32(0x6C), "imp_mods": u32(0x70), "nimp_mods": u32(0x74),
            "data_pages": u32(0x80) + self.mz_base, "auto_ds": u32(0x94) if h + 0x98 <= len(d) else 0,
        }
        H = self.hdr
        self.objs = []
        for i in range(H["nobjs"]):
            vs, base, fl, pi, np_, _ = struct.unpack_from("<6I", d, h + H["obj_tab"] + 24 * i)
            self.objs.append(Obj(i + 1, vs, base, fl, pi, np_))
        # page map: LE = 3-byte big-endian page number + flags byte
        self.pages = []
        for i in range(H["npages"]):
            b = d[h + H["page_map"] + 4 * i: h + H["page_map"] + 4 * i + 4]
            self.pages.append(((b[0] << 16) | (b[1] << 8) | b[2], b[3]))
        self._load_pages()
        self.fixups = self._parse_fixups()
        self._apply_fixups()

    @staticmethod
    def _find_le(d):
        """Returns (le_off, mz_base). Bound DOS/4G: DOS/16M 'BW' stub, then an inner MZ whose
        e_lfanew points at the LE header. LE 'data pages' offset is relative to that inner MZ."""
        le = d.find(b"LE\x00\x00")
        while le >= 0:
            i = d.rfind(b"MZ", 0, le)
            while i >= 0:
                if i + struct.unpack_from("<I", d, i + 0x3C)[0] == le:
                    return le, i
                i = d.rfind(b"MZ", 0, i)
            le = d.find(b"LE\x00\x00", le + 1)
        raise ValueError("no LE header with a matching MZ stub")

    def _load_pages(self):
        H, d = self.hdr, self.raw
        ps = H["page_size"]
        for o in self.objs:
            o.data = bytearray(o.vsize)
            for k in range(o.npages):
                pidx = o.page_idx + k               # 1-based logical page
                num, fl = self.pages[pidx - 1]
                size = H["last_page"] if pidx == H["npages"] else ps
                obj_off = k * ps
                size = min(size, o.vsize - obj_off) if obj_off < o.vsize else 0
                if fl == 0 and size > 0:
                    fo = H["data_pages"] + (num - 1) * ps
                    o.data[obj_off:obj_off + size] = d[fo:fo + size]
                    o.file_ranges.append((obj_off, fo, size))
                elif fl not in (0, 3):
                    raise NotImplementedError(f"page {pidx} flags {fl} (iterated/invalid)")

    def _parse_fixups(self):
        H, d, h = self.hdr, self.raw, self.le
        fpt = [struct.unpack_from("<I", d, h + H["fixup_pages"] + 4 * i)[0] for i in range(H["npages"] + 1)]
        rec0 = h + H["fixup_recs"]
        out = []
        for o in self.objs:
            for k in range(o.npages):
                pidx = o.page_idx + k
                page_lin = o.base + k * H["page_size"]
                p, end = rec0 + fpt[pidx - 1], rec0 + fpt[pidx]
                while p < end:
                    src, fl = d[p], d[p + 1]; p += 2
                    stype = src & 0x0F
                    cnt = srcoff = 0
                    if src & 0x20:
                        cnt = d[p]; p += 1
                    else:
                        srcoff = struct.unpack_from("<h", d, p)[0]; p += 2
                    if fl & 3 != 0:
                        raise NotImplementedError(f"non-internal fixup target type {fl & 3} at page {pidx}")
                    if fl & 0x40:
                        tobj = struct.unpack_from("<H", d, p)[0]; p += 2
                    else:
                        tobj = d[p]; p += 1
                    toff = 0
                    if stype != 2:
                        if fl & 0x10:
                            toff = struct.unpack_from("<I", d, p)[0]; p += 4
                        else:
                            toff = struct.unpack_from("<H", d, p)[0]; p += 2
                    if fl & 0x04:     # additive
                        if fl & 0x20:
                            toff += struct.unpack_from("<I", d, p)[0]; p += 4
                        else:
                            toff += struct.unpack_from("<H", d, p)[0]; p += 2
                    srcs = []
                    if src & 0x20:
                        for _ in range(cnt):
                            srcs.append(struct.unpack_from("<h", d, p)[0]); p += 2
                    else:
                        srcs = [srcoff]
                    tgt = self.objs[tobj - 1].base + toff
                    for so in srcs:
                        out.append(Fixup(page_lin + so, stype, tobj, toff, tgt))
        return out

    def _apply_fixups(self):
        for f in self.fixups:
            o = self.obj_at(f.src)
            if o is None:
                continue
            i = f.src - o.base
            sel = SEL_BASE + 8 * f.obj
            def put(fmt, v):
                n = struct.calcsize(fmt)
                if 0 <= i and i + n <= len(o.data):
                    struct.pack_into(fmt, o.data, i, v)
            if f.stype == 7:
                put("<I", f.addr & 0xFFFFFFFF)
            elif f.stype == 8:
                put("<i", f.addr - (f.src + 4))
            elif f.stype == 5:
                put("<H", f.addr & 0xFFFF)
            elif f.stype == 2:
                put("<H", sel)
            elif f.stype == 6:
                put("<I", f.addr & 0xFFFFFFFF)
                if i + 6 <= len(o.data):
                    struct.pack_into("<H", o.data, i + 4, sel)
            elif f.stype == 3:
                put("<H", f.addr & 0xFFFF)
                if i + 4 <= len(o.data):
                    struct.pack_into("<H", o.data, i + 2, sel)
            elif f.stype == 0:
                put("<B", f.addr & 0xFF)

    # --- address helpers -------------------------------------------------
    @property
    def entry(self):
        return self.objs[self.hdr["eip_obj"] - 1].base + self.hdr["eip"]

    def obj_at(self, lin):
        for o in self.objs:
            if o.base <= lin < o.base + o.vsize:
                return o
        return None

    def read(self, lin, n):
        o = self.obj_at(lin)
        if o is None:
            raise ValueError(f"0x{lin:X} not mapped")
        i = lin - o.base
        return bytes(o.data[i:i + n])

    def lin_to_file(self, lin):
        """Linear address -> byte offset in JA.EXE (None for bss/zero-fill)."""
        o = self.obj_at(lin)
        if o is None:
            return None
        i = lin - o.base
        for oo, fo, sz in o.file_ranges:
            if oo <= i < oo + sz:
                return fo + i - oo
        return None

    def file_to_lin(self, fo):
        for o in self.objs:
            for oo, f0, sz in o.file_ranges:
                if f0 <= fo < f0 + sz:
                    return o.base + oo + fo - f0
        return None


_cache = None


def load():
    global _cache
    if _cache is None:
        _cache = LEImage()
    return _cache


def main():
    img = load()
    H = img.hdr
    print(f"{img.path}  inner MZ @ 0x{img.mz_base:X}, LE header @ file 0x{img.le:X}")
    print(f"cpu={H['cpu']} os={H['os']} flags=0x{H['flags']:X} pages={H['npages']} page_size=0x{H['page_size']:X} "
          f"last_page=0x{H['last_page']:X} data_pages @ 0x{H['data_pages']:X}")
    print(f"entry = obj{H['eip_obj']}+0x{H['eip']:X} = 0x{img.entry:08X}   stack = obj{H['esp_obj']}+0x{H['esp']:X}"
          f"   auto_ds obj{H['auto_ds']}   import modules={H['nimp_mods']}")
    for o in img.objs:
        print(" ", o)
    c = Counter(SRC_TYPES.get(f.stype, f.stype) for f in img.fixups)
    print(f"fixups: {len(img.fixups)}  " + "  ".join(f"{k}={v}" for k, v in c.most_common()))
    if "--fixups" in sys.argv:
        n = int(sys.argv[sys.argv.index("--fixups") + 1])
        for f in img.fixups[:n]:
            print(" ", f)


if __name__ == "__main__":
    main()
