"""Variable map of a global memory range: which code reads/writes which bytes, with Watcom's byte-in-dword idiom undone.

usage: ./t varmap <addr|label> <len> [--funcs] [--unlabeled]
Every absolute reference (LE fixup) into [addr, addr+len) is decoded: access size from the operand (byte/word/dword
ptr), R/W from the instruction (mov to memory, inc/dec/add... = W; immediate use = &, address taken), and a dword
load followed within 4 instructions by `sar/shr reg, 0x18` (0x10) on the same register is counted as the byte
(word) at +3 (+2) — Watcom's way of reading a char global. Output per effective address: sizes, R/W/& counts,
idx = how many of them are indexed ([reg+addr], array/struct access), label, and with --funcs the functions involved. --unlabeled hides addresses that already have a label.
"""
import sys
from collections import defaultdict
from capstone.x86 import X86_OP_MEM, X86_OP_IMM, X86_OP_REG
from le import load
from common import load_labels, load_funcs, parse_addr, disassembler
from xref import containing_func

WRITES = {"inc", "dec", "add", "sub", "and", "or", "xor", "neg", "not", "shl", "shr", "sar", "adc", "sbb", "pop",
          "setne", "sete", "setg", "setl", "setge", "setle", "seta", "setb", "setae", "setbe", "xchg"}


def insn_at(img, md, src):
    for back in range(1, 11):
        ins = next(md.disasm(img.read(src - back, 16), src - back), None)
        if ins and ins.address < src and ins.address + ins.size >= src + 4:
            return ins
    return None


def main():
    a = [x for x in sys.argv[1:] if not x.startswith("--")]
    if len(a) < 2:
        sys.exit(__doc__)
    img, labels = load(), load_labels()
    by_name = {n: x for x, (n, _) in labels.items()}
    lo = by_name[a[0]] if a[0] in by_name else parse_addr(a[0])
    hi = lo + int(a[1], 0)
    md = disassembler()
    md.detail = True
    fnames = load_funcs()
    funcs = sorted(fnames)
    var = defaultdict(lambda: {"sz": set(), "R": 0, "W": 0, "&": 0, "i": 0, "f": set()})
    for f in img.fixups:
        if not (lo <= f.addr < hi) or f.stype not in (5, 6, 7):
            continue
        o = img.obj_at(f.src)
        if not o or o.kind != "code":
            continue
        ins = insn_at(img, md, f.src)
        if ins is None:
            continue
        eff, size, kind = f.addr, None, "R"
        mem = [op for op in ins.operands if op.type == X86_OP_MEM and lo - 0x10000 <= (op.mem.disp & 0xFFFFFFFF) < hi]
        if mem:
            size = mem[0].size
            if mem[0].mem.base or mem[0].mem.index:
                var[eff]["i"] += 1
            if ins.mnemonic.startswith("mov") and ins.operands[0].type == X86_OP_MEM:
                kind = "W"
            elif ins.mnemonic in WRITES and ins.operands[0].type == X86_OP_MEM:
                kind = "W"
            elif ins.mnemonic == "lea":
                kind = "&"
            if size == 4 and kind == "R" and ins.operands[0].type == X86_OP_REG:
                reg = ins.operands[0].reg
                nxt = ins.address + ins.size
                for _ in range(4):
                    j = next(md.disasm(img.read(nxt, 16), nxt), None)
                    if j is None:
                        break
                    if (j.mnemonic in ("sar", "shr") and len(j.operands) == 2 and j.operands[0].type == X86_OP_REG
                            and j.operands[0].reg == reg and j.operands[1].type == X86_OP_IMM
                            and j.operands[1].imm in (0x18, 0x10)):
                        sh = j.operands[1].imm
                        eff, size = f.addr + (3 if sh == 0x18 else 2), (1 if sh == 0x18 else 2)
                        break
                    if any(op.type == X86_OP_REG and op.reg == reg for op in j.operands[:1]):
                        break
                    nxt = j.address + j.size
        else:
            kind = "&"
        v = var[eff]
        if size:
            v["sz"].add(size)
        v[kind] += 1
        fn = containing_func(ins.address, funcs)
        v["f"].add(labels[fn][0] if fn in labels else fnames.get(fn, f"{fn:X}" if fn else "?"))
    for eff in sorted(var):
        if "--unlabeled" in sys.argv and eff in labels:
            continue
        v = var[eff]
        lab = labels[eff][0] if eff in labels else ""
        line = (f"+0x{eff - lo:03X} 0x{eff:X} {'/'.join(str(s) for s in sorted(v['sz'])) or '-':5s} "
                f"R{v['R']:<3d} W{v['W']:<3d} &{v['&']:<2d} idx{v['i']:<3d} {lab}")
        if "--funcs" in sys.argv:
            line += "  [" + ", ".join(sorted(v["f"]))[:150] + "]"
        print(line)


if __name__ == "__main__":
    main()
