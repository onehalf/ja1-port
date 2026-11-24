"""Cross-references to a linear address.

usage: ./t xref <addr|label> [--range N]
- absolute refs: exact, from LE off32 fixups (every absolute code/data pointer in the image is one)
- relative refs: E8/E9 call/jmp and 0F 8x jcc rel32 found by scanning code (rel8 short jumps not scanned)
--range N also reports fixups pointing into [addr, addr+N) (struct/array field accesses).
"""
import struct, sys
from le import load
from common import load_labels, load_funcs, parse_addr, disassembler


def containing_func(addr, funcs):
    best = None
    for f in funcs:
        if f <= addr:
            best = f
        else:
            break
    return best


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    img, labels = load(), load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    a = sys.argv[1]
    tgt = by_name[a] if a in by_name else parse_addr(a)
    rng = int(sys.argv[sys.argv.index("--range") + 1], 0) if "--range" in sys.argv else 1
    fnames = load_funcs()
    lab = lambda x: labels[x][0] if x in labels else fnames.get(x, "")
    md = disassembler()
    funcs = sorted(fnames)

    def show(src, kind):
        f = containing_func(src, funcs)
        ins = next(md.disasm(img.read(src, 16), src), None)
        txt = f"{ins.mnemonic} {ins.op_str}" if ins and ins.address == src else ""
        print(f"  {kind:5} {src:08X}  {txt:<40} in {lab(f) or '?'}")

    hits = 0
    for f in img.fixups:
        if tgt <= f.addr < tgt + rng and f.stype in (7, 6, 5):
            # find instruction start: scan back up to 10 bytes for an insn that covers the fixup
            o = img.obj_at(f.src)
            start = f.src
            if o and o.kind == "code":
                for back in range(1, 11):
                    ins = next(md.disasm(img.read(f.src - back, 16), f.src - back), None)
                    if ins and ins.address + ins.size > f.src >= ins.address + 1 and ins.address + ins.size >= f.src + 4:
                        start = ins.address
                        break
                show(start, "abs" if f.addr == tgt else f"+{f.addr - tgt:X}")
            else:
                print(f"  data  {f.src:08X}  (pointer in obj{o.idx if o else '?'})")
            hits += 1
    for o in img.objs:
        if o.kind != "code":
            continue
        d = o.data
        for i in range(len(d) - 5):
            b = d[i]
            if b in (0xE8, 0xE9):
                if o.base + i + 5 + struct.unpack_from("<i", d, i + 1)[0] == tgt:
                    show(o.base + i, "call" if b == 0xE8 else "jmp"); hits += 1
            elif b == 0x0F and 0x80 <= d[i + 1] <= 0x8F:
                if o.base + i + 6 + struct.unpack_from("<i", d, i + 2)[0] == tgt:
                    show(o.base + i, "jcc"); hits += 1
    print(f"{hits} refs to 0x{tgt:X} {lab(tgt)}")


if __name__ == "__main__":
    main()
