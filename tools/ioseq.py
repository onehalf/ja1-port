"""Ordered fread_/fwrite_ calls in a function, with the register args set just before each call.

usage: ./t ioseq <addr|label> [max_insns=3000]
Watcom register args: EAX=buf EDX=size EBX=n. Values are the last `mov reg, imm` (or a copy of a
resolved register) seen within 10 insns (else '?').
Stops at the first top-level `ret` after max_insns or at the next Ghidra procedure start.
"""
import glob, os, bisect, sys
from le import load
from common import load_labels, parse_addr, disassembler

IO = {0x758AD: "R", 0x75FE3: "W"}
ROOT = os.path.join(os.path.dirname(__file__), "..")


def calls(a, mx=3000, img=None):
    """[(insn addr, 'R'|'W', buf, size, n)] in code order; values are ints when resolved (`mov reg, imm`, or
    `mov reg, reg2` copying a resolved register), else the operand text / '?'."""
    img = img or load()
    starts = sorted(int(os.path.basename(p)[:6], 16) for p in glob.glob(f"{ROOT}/analysis/decomp/*.c"))
    i = bisect.bisect_right(starts, a)
    end = starts[i] if i < len(starts) else a + 0x10000
    o = img.obj_at(a)
    code = bytes(o.data[a - o.base:end - o.base])
    win, total, out = [], 0, []
    for ins in disassembler().disasm(code, a):
        win = (win + [ins])[-10:]
        if ins.mnemonic == "call":
            try:
                t = int(ins.op_str, 16)
            except ValueError:
                t = None
            if t in IO:
                regs = {}
                for y in win[:-1]:
                    if y.mnemonic == "mov" and ", " in y.op_str:
                        r, v = y.op_str.split(", ", 1)
                        try:
                            regs[r] = int(v, 16)
                        except ValueError:
                            regs[r] = regs.get(v, v)
                out.append((ins.address, IO[t], regs.get("eax", "?"), regs.get("edx", "?"), regs.get("ebx", "?")))
        total += 1
        if total > mx:
            break
    return out


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    img, labels = load(), load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    a = by_name[sys.argv[1]] if sys.argv[1] in by_name else parse_addr(sys.argv[1])
    mx = int(sys.argv[2]) if len(sys.argv) > 2 else 3000

    def g(v):
        return (labels[v][0] if v in labels else f"0x{v:x}") if isinstance(v, int) else v
    for addr, kind, buf, sz, n in calls(a, mx, img):
        tot = f"= 0x{sz * n:X}" if isinstance(sz, int) and isinstance(n, int) else ""
        print(f"{addr:06X} {kind} buf={g(buf):22} size={g(sz) if not isinstance(sz, int) else hex(sz):8} "
              f"n={hex(n) if isinstance(n, int) else n:6} {tot}")


if __name__ == "__main__":
    main()
