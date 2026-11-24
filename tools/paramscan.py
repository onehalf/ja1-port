"""Candidate soldier* parameters, to seed defs/functions.toml (review each one in the asm before adding it).

usage: ./t paramscan [--min N]     (--min: least evidence count to list a function, default 2)
       ./t paramscan --toml          defs/functions.toml lines for the candidates (soldier* s + the other register
                                     arguments the function reads before writing them, typed "?"; return kept "?")
Two independent signals per (function, argument register EAX/EDX/EBX/ECX):
  calls  call sites that load the register with &g_soldiers[k] (mov/lea imm), [g_soldiersPtr], or idx*0x200 + g_soldiers
         within the last 8 instructions before the call (same straight-line run)
  deref  the function itself reads/writes [reg + off] at a soldier field offset >= 0x100 (types.toml fields) while reg
         still holds the incoming argument: a linear walk in address order (branches ignored, so a candidate only),
         copies via mov reg2, reg followed, a call clobbers EAX only (Watcom callees save the rest)
Game code only (below the library region 0x754F0).

       ./t paramscan --index [--toml]  index/pointer parameters into g_profiles (profile), g_sectors / g_sectorTbl9
                                     (sector), g_tileMap (tile): --toml prints the candidate signatures, merged with
                                     the existing defs/functions.toml entry ("?" slots filled, typed slots kept)
  index  an incoming argument, or a register computed from it by arithmetic (Watcom's lea/shl/sub multiplications,
         mov reg, g_X + add reg, idx), is the base/index of a memory operand inside the global (loads don't propagate)
  pass   the unchanged argument is passed in a register whose parameter defs/functions.toml (or an earlier finding,
         iterated to a fixpoint) names tile*/profile/sector
  ptr    call sites load &g_X[k] (or a register holding g_X + index) into the argument register -> T* parameter
"""
import sys
from capstone import CS_ARCH_X86, CS_MODE_32, Cs
from capstone.x86 import X86_OP_IMM, X86_OP_MEM, X86_OP_REG
from common import load_funcs, load_labels
from jatypes import Types
from le import load

LIB = 0x754F0
ARGS = ["eax", "edx", "ebx", "ecx"]
FAMILY = {r: "e" + x + "x" for x in "abcd" for r in ("e" + x + "x", x + "x", x + "l", x + "h")}


def fam(name):
    return FAMILY.get(name, name)


def flow(listing, init, transfer, meet):
    """Forward dataflow over one function: {address: in-state}. Successors are the fall-through and direct branch
    targets inside the function (a jump table or a jump out of it ends the path); at a merge, meet() keeps what all
    predecessors agree on, so the body of a loop entered by a jump to its test sees the loop's own writes."""
    idx = {ins.address: i for i, ins in enumerate(listing)}
    state = {listing[0].address: init} if listing else {}
    work = [0] if listing else []
    while work:
        i = work.pop()
        ins = listing[i]
        out = transfer(state[ins.address], ins)
        m, ops = ins.mnemonic, ins.operands
        succ = [] if m in ("jmp", "ret", "retf", "iretd") or i + 1 >= len(listing) else [i + 1]
        if (m.startswith("j") or m.startswith("loop")) and ops and ops[0].type == X86_OP_IMM and ops[0].imm in idx:
            succ.append(idx[ops[0].imm])
        for j in succ:
            a = listing[j].address
            old = state.get(a)
            new = out if old is None else meet(old, out)
            if new != old:
                state[a] = new
                work.append(j)
    return state


def self_zero(ins):
    ops = ins.operands
    return ins.mnemonic in ("xor", "sub") and len(ops) == 2 and ops[0].type == ops[1].type == X86_OP_REG \
        and ops[0].reg == ops[1].reg


def arity(md, code, base, f, end):
    """Number of register arguments: Watcom fills EAX EDX EBX ECX in order, so 1 + the highest of them read while
    still untouched on every path from the entry (push of a callee-saved register is not a read, xor r, r no read)."""
    listing = list(md.disasm(code[f - base:end - base], f))

    def keys(name):                            # mov dh, x leaves DL holding the argument; mov dx, x clobbers both
        k = rkey(name)
        return {k, k[1] + "h"} if k in ARGS and name[-1] != "l" else {k}

    def transfer(untouched, ins):
        wr = set().union(*(keys(ins.reg_name(r)) for r in ins.regs_access()[1]))
        return untouched - wr - ({"eax", "ah"} if ins.mnemonic == "call" else set())   # callees save the rest
    state = flow(listing, frozenset(ARGS) | HIGH, transfer, lambda a, b: a & b)
    read = set()
    for ins in listing:
        if ins.address in state and ins.mnemonic != "push" and not self_zero(ins):
            read |= {fam(n) for n in (ins.reg_name(r) for r in ins.regs_access()[0]) if rkey(n) in state[ins.address]}
    return max((ARGS.index(r) + 1 for r in read), default=0)


IDX_GLOBALS = {"g_profiles": ("profile", "profile"), "g_sectors": ("sector", "sector"),
               "g_sectorTbl9": ("sector", "sectorState"), "g_tileMap": ("tile", "tile")}   # global -> (kind, struct)
WIDTH = {1: "u8", 2: "u16", 4: "i32"}


def param_kind(t, name):
    """Kind of an existing defs/functions.toml parameter: index by name, pointer by type."""
    for kind, struct in set(IDX_GLOBALS.values()):
        if t == struct + "*":
            return "ptr:" + struct
    n = name.lower()
    return "tile" if n.endswith("tile") else "profile" if n == "profile" else "sector" if n == "sector" else None


HIGH = {"ah", "bh", "ch", "dh"}


def rkey(name):
    """Dataflow key of a register: the high byte registers are their own; the rest is the 32-bit family."""
    return name if name in HIGH else fam(name)


def index_scan(md, code, base, starts, T):
    """Returns ({(f, reg): {kind: [evidence]}}, {(f, reg): first-read width}); kind = tile/profile/sector (index)
    or ptr:<struct>. Dataflow state per register (flow()): lin = {arg: c} for c * arg + constant (linear in the
    incoming arguments), pure = the argument held unchanged, gb = (global, constant byte offset) for g_X + off + lin.
    An index hit needs c == the struct size, so an index into a sub-array (trigger k of a sector) is not a sector."""
    ranges = []
    for g, (kind, struct) in IDX_GLOBALS.items():
        ranges.append((T.addr_of(g), T.addr_of(g) + T.globals[g].size(), kind, struct, T.structs[struct]["size"]))

    def inglobal(a):
        a &= 0xFFFFFFFF
        return next(((lo, kind, struct, sz) for lo, hi, kind, struct, sz in ranges if lo <= a < hi), None)

    def gl(a):
        g = inglobal(a)
        return (g, (a & 0xFFFFFFFF) - g[0]) if g else None

    known = {}                                  # (f, reg) -> kind, from defs/functions.toml
    for name, (ret, params, ev, note) in T.functions.items():
        try:
            f = T.addr_of(name)
        except KeyError:
            continue
        for i, (t, pn) in enumerate(params[:4]):
            if param_kind(t, pn):
                known[(f, ARGS[i])] = param_kind(t, pn)
    ends = dict(zip(starts, starts[1:] + [LIB]))
    listing = {f: list(md.disasm(code[f - base:ends[f] - base], f)) for f in starts}
    nargs = {f: arity(md, code, base, f, ends[f]) for f in starts}

    def transfer(st, ins):
        lin, pure, gb = (dict(d) for d in st)

        def kill(k):
            for d in (lin, pure, gb):
                d.pop(k, None)
        m, ops = ins.mnemonic, ins.operands
        if m == "call":
            kill("eax")
            return lin, pure, gb
        dst = rkey(ins.reg_name(ops[0].reg)) if ops and ops[0].type == X86_OP_REG else None
        wr = ins.regs_access()[1]
        if dst is None or m in ("push", "cmp", "test") or m.startswith("j"):
            for r in wr:                        # implicit writes (idiv, mul, cdq, string ops)
                kill(rkey(ins.reg_name(r)))
            return lin, pure, gb
        src = ops[1:]
        sreg = [rkey(ins.reg_name(op.reg)) for op in src if op.type == X86_OP_REG]
        imm = [op.imm for op in src if op.type == X86_OP_IMM]
        new, g, p = None, None, None            # None = not argument-derived / unknown
        if self_zero(ins):
            pass
        elif m in ("mov", "movzx", "movsx"):
            if sreg:
                new, g, p = dict(lin.get(sreg[0], {})) or None, gb.get(sreg[0]), pure.get(sreg[0])
            elif imm:
                g = gl(imm[0])
                new = {} if g else None
            # a memory load is data, not an index: contributes nothing
        elif m == "lea" and src[0].type == X86_OP_MEM:
            mo = src[0].mem
            new = {}
            for r, s in ((mo.base, 1), (mo.index, mo.scale)):
                if r:
                    for a, c in lin.get(rkey(ins.reg_name(r)), {}).items():
                        new[a] = new.get(a, 0) + c * s
                    g = g or gb.get(rkey(ins.reg_name(r)))
            g = (g[0], g[1] + mo.disp) if g else gl(mo.disp)
            new = new if new or g else None
        elif m in ("add", "sub") and dst in lin or (m == "add" and (sreg and (sreg[0] in lin or sreg[0] in gb)
                                                                      or imm and inglobal(imm[0]))):
            new = dict(lin.get(dst, {}))
            g = gb.get(dst)
            if sreg:
                for a, c in lin.get(sreg[0], {}).items():
                    new[a] = new.get(a, 0) + (c if m == "add" else -c)
                g = g or gb.get(sreg[0])
            elif imm:
                g = (g[0], g[1] + (imm[0] if m == "add" else -imm[0])) if g else gl(imm[0]) if m == "add" else None
        elif dst in lin and m == "shl" and imm:
            new = {a: c << imm[0] for a, c in lin[dst].items()}
        elif m == "imul" and len(ops) == 3 and imm and sreg and sreg[0] in lin:
            new = {a: c * imm[0] for a, c in lin[sreg[0]].items()}
        elif dst in lin and (m in ("inc", "dec") or m == "and" and imm and imm[0] in (0xFF, 0xFFFF)):
            new, g, p = lin[dst], gb.get(dst), pure.get(dst) if m == "and" else None
            if g and m != "and":
                g = (g[0], g[1] + (1 if m == "inc" else -1))
        elif dst in lin and m == "neg":
            new = {a: -c for a, c in lin[dst].items()}
        if ops[0].size < 4 and dst not in HIGH and m not in ("mov", "movzx", "movsx") and new is None:
            new, g, p = lin.get(dst), gb.get(dst), None
        kill(dst)
        if new is not None:
            new = {a: c for a, c in new.items() if c}
            if new or g:
                lin[dst] = new
        if g:
            gb[dst] = g
        if p and (ops[0].size == 4 or m in ("movzx", "movsx") or p == dst):
            pure[dst] = p
        for r in wr:
            k = rkey(ins.reg_name(r))
            if k != dst and k in ARGS + ["esi", "edi", "ebp"]:
                kill(k)
        return lin, pure, gb

    def meet(a, b):
        return tuple({k: v for k, v in x.items() if y.get(k) == v} for x, y in zip(a, b))

    init = ({r: {r: 1} for r in ARGS}, {r: r for r in ARGS}, {})
    states = {f: flow(listing[f], init, transfer, meet) for f in starts}

    def scan(f, found):
        hits, width = {}, {}

        def hit(key, kind, what):
            hits.setdefault(key, {}).setdefault(kind, []).append(what)
        for ins in listing[f]:
            if ins.address not in states[f]:
                continue                        # not reached by direct control flow
            lin, pure, gb = states[f][ins.address]
            m, ops = ins.mnemonic, ins.operands
            for op in ops:                      # first read of an argument register: its width
                if op.type == X86_OP_REG and m != "push":
                    k = rkey(ins.reg_name(op.reg))
                    if pure.get(k) == k and k not in width and not (op is ops[0] and len(ops) == 2 and m in (
                            "mov", "movzx", "movsx", "lea")) and not self_zero(ins):
                        width[k] = WIDTH.get(op.size, "i32")
            for op in ops:                      # index signal
                if op.type != X86_OP_MEM:
                    continue
                b = rkey(ins.reg_name(op.mem.base)) if op.mem.base else None
                x = rkey(ins.reg_name(op.mem.index)) if op.mem.index else None
                g = inglobal(op.mem.disp) or (gb.get(b) or gb.get(x) or (None,))[0]
                if not g:
                    continue
                tot = {}
                for r, s in ((b, 1), (x, op.mem.scale)):
                    for a, c in lin.get(r, {}).items():
                        tot[a] = tot.get(a, 0) + c * s
                for a, c in tot.items():
                    if c == g[3] and a in ARGS[:nargs[f]]:
                        hit((f, a), g[1], f"index {ins.address:X}")
            if m == "call" and ops and ops[0].type == X86_OP_IMM:
                tgt = ops[0].imm
                for r in ARGS:
                    k = known.get((tgt, r)) or found_kind(found, (tgt, r))
                    if k and not k.startswith("ptr:") and r in pure and pure[r] in ARGS[:nargs[f]]:
                        hit((f, pure[r]), k, f"pass {ins.address:X}")
                    g, off = gb.get(r, (None, 0))   # &g_X[k] or g_X + c * arg: a T* argument
                    if g and off % g[3] == 0 and list(lin.get(r, {}).values()) in ([], [g[3]]) \
                            and ARGS.index(r) < nargs.get(tgt, 4):
                        hit((tgt, r), "ptr:" + g[2], f"ptr {ins.address:X}")
        return hits, width

    found, widths = {}, {}
    for _ in range(5):                          # fixpoint over the "pass" signal
        new = {}
        for f in starts:
            hits, width = scan(f, found)
            for key, kinds in hits.items():
                for k, ev in kinds.items():
                    new.setdefault(key, {}).setdefault(k, []).extend(ev)
            for r, w in width.items():
                widths[(f, r)] = w
        if new == found:
            break
        found = new
    return found, widths


def found_kind(found, key):
    ks = [k for k in found.get(key, {}) if not k.startswith("ptr:")]
    return ks[0] if len(ks) == 1 else None


def index_main(md, code, base, starts, names, T):
    found, width = index_scan(md, code, base, starts, T)
    ends = dict(zip(starts, starts[1:] + [LIB]))
    byf = {}
    for (f, r), kinds in found.items():
        if f < LIB and f in ends:
            byf.setdefault(f, {})[r] = kinds
    if "--toml" not in sys.argv:
        print(f"{'function':26s} addr  arg  kind: evidence")
        for f in sorted(byf):
            for r in ARGS:
                if r in byf[f]:
                    ks = "; ".join(f"{k} {len(v)}x ({', '.join(v[:3])})" for k, v in byf[f][r].items())
                    print(f"{names.get(f, f'FUN_{f:05X}'):26s} {f:05X} {r}  {ks}")
        return
    STRUCT_IDX = {"tile": "i32", "profile": "u8", "sector": "u8"}
    for f in sorted(byf):
        key = names[f] if f in names and not names[f].startswith("FUN_") else f"0x{f:X}"
        old = T.functions.get(key)
        ret, params = (old[0], list(old[1])) if old else ("?", [])
        n = max([arity(md, code, base, f, ends[f]), len(params)] + [ARGS.index(r) + 1 for r in byf[f]])
        params += [("?", r) for r in ARGS[len(params):n]]
        notes = []
        for i, r in enumerate(ARGS[:n]):
            kinds = byf[f].get(r)
            if not kinds or params[i][0] != "?":
                continue
            if len(kinds) > 1:
                notes.append(f"{r} ambiguous: {' / '.join(kinds)}")
                continue
            (k, ev), = kinds.items()
            if k.startswith("ptr:"):
                params[i] = (k[4:] + "*", {"sectorState": "tbl"}.get(k[4:], k[4:][0]))
            else:
                w = width.get((f, r), STRUCT_IDX[k])
                params[i] = (w if k != "tile" or w != "u8" else "i32", k)
            notes.append(f"{r} {k}: {', '.join(ev[:4])}")
        if not notes:
            continue
        names_used = {}
        out = []
        for t, pn in params:                    # unique parameter names (two tiles -> tile, tile2)
            c = names_used[pn] = names_used.get(pn, 0) + 1
            out.append(f"{t} {pn}{c if c > 1 else ''}")
        ev = old[2] if old else "G"
        note = (old[3] + "; " if old and old[3] else "") + "idxscan: " + "; ".join(notes)
        print(f'"{key}" = ["{ret} ({", ".join(out)})", "{ev}", "{note}"]')


def main():
    if "--index" in sys.argv:
        img = load()
        o = img.objs[0]
        md = Cs(CS_ARCH_X86, CS_MODE_32)
        md.detail = True
        funcs = load_funcs()
        index_main(md, bytes(o.data), o.base, sorted(a for a in funcs if a < LIB), funcs, Types())
        return
    mn = int(sys.argv[sys.argv.index("--min") + 1]) if "--min" in sys.argv else 2
    img = load()
    o = img.objs[0]
    code = bytes(o.data)
    labels = {n: a for a, (n, _) in load_labels().items()}
    S0, SPTR = labels["g_soldiers"], labels["g_soldiersPtr"]
    T = Types()
    soff = {off for off, ts, fn, ev, note in T.structs["soldier"]["fields"] if off >= 0x100}
    funcs = load_funcs()
    starts = sorted(a for a in funcs if a < LIB)
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = True

    def insns(a, end):
        return md.disasm(code[a - o.base:end - o.base], a)

    calls, deref = {}, {}
    for i, f in enumerate(starts):
        end = starts[i + 1] if i + 1 < len(starts) else LIB
        hold = {r: r for r in ARGS}           # register -> incoming arg register it still holds
        recent = []                            # (insn) since the last branch/call
        for ins in insns(f, end):
            m = ins.mnemonic
            ops = ins.operands
            # (c) dereference of an incoming argument
            if hold:
                for op in ops:
                    if op.type == X86_OP_MEM and op.mem.base and op.mem.disp in soff:
                        r = ins.reg_name(op.mem.base)
                        if r in hold:
                            deref.setdefault((f, hold[r]), set()).add(op.mem.disp)
                if m == "mov" and len(ops) == 2 and ops[0].type == X86_OP_REG:
                    dst = fam(ins.reg_name(ops[0].reg))
                    src = ins.reg_name(ops[1].reg) if ops[1].type == X86_OP_REG else None
                    if src in hold and src == fam(src):
                        hold[dst] = hold[src]
                    else:
                        hold.pop(dst, None)
                elif ops and ops[0].type == X86_OP_REG and m not in ("cmp", "test", "push"):
                    hold.pop(fam(ins.reg_name(ops[0].reg)), None)
                if m == "call":
                    hold.pop("eax", None)
                elif m == "pop" and ops and ops[0].type == X86_OP_REG:
                    hold.pop(fam(ins.reg_name(ops[0].reg)), None)
            # (b) call sites loading a soldier pointer
            if m == "call" and ops and ops[0].type == X86_OP_IMM:
                tgt = ops[0].imm
                for r in ARGS:
                    for p in reversed(recent[-8:]):
                        po = p.operands
                        if not po or po[0].type != X86_OP_REG or p.reg_name(po[0].reg) != r:
                            continue
                        hit = False
                        if p.mnemonic in ("mov", "lea") and len(po) == 2:
                            if po[1].type == X86_OP_IMM:
                                v = po[1].imm
                                hit = S0 <= v < S0 + 48 * 0x200 and (v - S0) % 0x200 == 0
                            elif po[1].type == X86_OP_MEM and not po[1].mem.base:
                                d = po[1].mem.disp
                                hit = d == SPTR or (p.mnemonic == "lea" and S0 <= d < S0 + 48 * 0x200
                                                    and (d - S0) % 0x200 == 0)
                        elif p.mnemonic == "add" and len(po) == 2 and po[1].type == X86_OP_IMM:
                            hit = po[1].imm == S0
                        if hit:
                            calls.setdefault((tgt, r), []).append(ins.address)
                        break                  # the last write to r decides
            if m.startswith("j") or m in ("call", "ret"):
                recent = []
            else:
                recent.append(ins)

    names = funcs
    keys = sorted(k for k in set(calls) | set(deref)
                  if k[0] < LIB and len(deref.get(k, ())) + min(1, len(calls.get(k, []))) >= mn)
    if "--toml" in sys.argv:
        ends = dict(zip(starts, starts[1:] + [LIB]))
        byf = {}
        for f, r in keys:
            byf.setdefault(f, []).append(r)
        for f, regs in byf.items():
            if f not in ends:
                continue
            n = max([arity(md, code, o.base, f, ends[f])] + [ARGS.index(r) + 1 for r in regs])
            sn = {r: "s" if i == 0 else f"s{i + 1}" for i, r in enumerate(sorted(regs, key=ARGS.index))}
            params = ", ".join(f"soldier* {sn[a]}" if a in sn else f"? {a}" for a in ARGS[:n])
            ds = [deref.get((f, r), set()) for r in regs]
            ncs = [len(calls.get((f, r), [])) for r in regs]
            ev = "T" if all(len(d) >= 3 or (d and nc) for d, nc in zip(ds, ncs)) else "G"
            key = names[f] if f in names and not names[f].startswith("FUN_") else f"0x{f:X}"
            note = "paramscan: " + "; ".join(f"{r} deref {' '.join(f'+{x:X}' for x in sorted(d))}"
                                             + (f", {nc} call sites" if nc else "") for r, d, nc in zip(regs, ds, ncs))
            print(f'"{key}" = ["? ({params})", "{ev}", "{note}"]')
        return
    print(f"{'function':32s} arg  calls  deref offsets")
    for f, r in keys:
        nc, d = len(calls.get((f, r), [])), deref.get((f, r), set())
        print(f"{names.get(f, f'FUN_{f:05X}'):26s} {f:05X} {r}  {nc:5d}  {' '.join(f'+{x:X}' for x in sorted(d))}")


if __name__ == "__main__":
    main()
