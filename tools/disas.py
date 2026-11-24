"""Targeted 32-bit disassembler over the loaded (fixed-up) LE image.

usage: ./t disas <addr> [count=40]        addr = linear (0x66F6C), label name, or F:<file offset>
Annotates labels.csv names on targets and marks operands that came from LE fixups (*).
"""
import sys
from le import load
from common import load_labels, parse_addr, disassembler


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    img, labels = load(), load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    a = sys.argv[1]
    start = by_name[a] if a in by_name else parse_addr(a)
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 40
    fix = {f.src: f for f in img.fixups}
    o = img.obj_at(start)
    if o is None:
        sys.exit(f"0x{start:X} not mapped")
    code = bytes(o.data[start - o.base:])
    md = disassembler()
    for n, ins in enumerate(md.disasm(code, start)):
        if n >= count:
            break
        if ins.address in labels:
            name, cm = labels[ins.address]
            print(f"\n{name}:{'  ; ' + cm if cm else ''}")
        fx = [fix[x] for x in range(ins.address, ins.address + ins.size) if x in fix]
        note = []
        for f in fx:
            note.append("*" + (labels[f.addr][0] if f.addr in labels else f"obj{f.obj}+0x{f.off:X}"))
        if ins.mnemonic in ("call", "jmp") or ins.mnemonic.startswith("j"):
            try:
                t = int(ins.op_str, 16)
                if t in labels:
                    note.append(labels[t][0])
            except ValueError:
                pass
        fo = img.lin_to_file(ins.address)
        print(f"  {ins.address:08X} [F:{fo:06X}]  {ins.bytes.hex():<20} {ins.mnemonic:<7} {ins.op_str}"
              + (f"    ; {' '.join(note)}" if note else ""))


if __name__ == "__main__":
    main()
