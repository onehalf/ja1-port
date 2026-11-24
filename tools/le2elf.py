"""Convert rom/JA.EXE (LE) into build/JA.elf: an ELF32/i386 image Ghidra/objdump can load.

usage: ./t le2elf [out.elf]

Each LE object becomes a PT_LOAD segment + section at its preferred base, with all
fixups pre-applied (so addresses in Ghidra == linear addresses in tools/le.py).
labels.csv entries with linear addresses become .symtab symbols (FUNC in code, OBJECT in data).
Re-run after editing labels.csv so Ghidra picks up the names.
"""
import os, struct, sys
from le import load, ROOT
from common import load_labels

EM_386, ET_EXEC = 3, 2


def strtab(names):
    blob, offs = b"\0", {}
    for n in names:
        if n not in offs:
            offs[n] = len(blob)
            blob += n.encode() + b"\0"
    return blob, offs


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(ROOT, "build", "JA.elf")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    img = load()
    labels = load_labels()

    segs = []   # (name, obj, filesz)
    for o in img.objs:
        filesz = max((oo + sz for oo, _, sz in o.file_ranges), default=0)
        segs.append((".text" if o.kind == "code" else ".data", o, filesz))
        if o.kind == "data" and o.vsize > filesz:
            segs.append((".bss", o, None))

    syms = []   # (name, value, size, info, shndx)
    for addr, (name, _c) in sorted(labels.items()):
        o = img.obj_at(addr)
        if o is None or not name:
            continue
        shndx = 1 + [i for i, s in enumerate(segs) if s[1] is o and
                     (s[2] is None or addr < o.base + s[2])][0]
        typ = 2 if o.kind == "code" else 1           # STT_FUNC / STT_OBJECT
        syms.append((name, addr, 0, (1 << 4) | typ, shndx))   # STB_GLOBAL

    shnames = [s[0] for s in segs] + [".symtab", ".strtab", ".shstrtab"]
    shstr, shoff_names = strtab(shnames)
    symstr, symoff = strtab([s[0] for s in syms])
    symtab = b"\0" * 16 + b"".join(struct.pack("<IIIBBH", symoff[n], v, sz, info, 0, sh)
                                   for n, v, sz, info, sh in syms)

    nph = len(img.objs)
    pos = 52 + 32 * nph
    body = bytearray()
    sec = []    # (name, type, flags, addr, off, size)
    phdrs = []
    for name, o, filesz in segs:
        if filesz is None:
            sec.append((name, 8, 3, o.base + max(oo + sz for oo, _, sz in o.file_ranges or [(0, 0, 0)]),
                        pos + len(body), o.vsize - max(oo + sz for oo, _, sz in o.file_ranges or [(0, 0, 0)])))
            continue
        while (pos + len(body)) % 0x1000 != o.base % 0x1000:
            body += b"\0"
        off = pos + len(body)
        body += o.data[:filesz]
        flags = 6 if o.kind == "code" else 3                 # SHF_ALLOC|EXECINSTR / ALLOC|WRITE
        sec.append((name, 1, flags, o.base, off, filesz))
        pf = (4 if o.flags & 1 else 0) | (2 if o.flags & 2 else 0) | (1 if o.flags & 4 else 0)
        phdrs.append(struct.pack("<8I", 1, off, o.base, o.base, filesz, o.vsize, pf, 0x1000))
    for name, data, typ in ((".symtab", symtab, 2), (".strtab", symstr, 3), (".shstrtab", shstr, 3)):
        sec.append((name, typ, 0, 0, pos + len(body), len(data)))
        body += data
    while (pos + len(body)) % 4:
        body += b"\0"
    shoff = pos + len(body)
    n_sec = len(sec) + 1
    symtab_idx = 1 + [s[0] for s in sec].index(".symtab")
    shdrs = b"\0" * 40
    for name, typ, flags, addr, off, size in sec:
        link = symtab_idx + 1 if name == ".symtab" else 0
        info = 1 if name == ".symtab" else 0
        entsize = 16 if name == ".symtab" else 0
        shdrs += struct.pack("<10I", shoff_names[name], typ, flags, addr, off, size, link, info,
                             0x1000 if typ == 1 else 1, entsize)
    ident = b"\x7fELF" + bytes([1, 1, 1, 0]) + b"\0" * 8
    ehdr = ident + struct.pack("<HHIIIIIHHHHHH", ET_EXEC, EM_386, 1, img.entry, 52, shoff, 0,
                               52, 32, nph, 40, n_sec, n_sec - 1)
    with open(out, "wb") as f:
        f.write(ehdr + b"".join(phdrs) + body + shdrs)
    print(f"wrote {out}: {len(img.objs)} segments, {len(syms)} symbols, entry 0x{img.entry:08X}")


if __name__ == "__main__":
    main()
