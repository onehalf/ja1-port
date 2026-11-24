"""Strings referenced by each procedure (via LE fixups) -- quick naming hints.

usage: ./t strrefs [addr|label ...]      default: all Ghidra procedures
       ./t strrefs --write               -> analysis/strrefs.txt
Procedure ranges = Ghidra starts (analysis/decomp/*.c filenames), each ending at the next start.
A fixup counts if its target lies within 3 bytes of a strings.txt entry (strings_dump prefixes junk bytes).
"""
import bisect, glob, os, re, sys
from le import load
from common import load_labels, parse_addr

ROOT = os.path.join(os.path.dirname(__file__), "..")


def main():
    img, labels = load(), load_labels()
    by_name = {n: a for a, (n, _) in labels.items()}
    starts = sorted({int(os.path.basename(p)[:6], 16) for p in glob.glob(f"{ROOT}/analysis/decomp/*.c")})
    S = {}
    for l in open(f"{ROOT}/analysis/strings.txt"):
        m = re.match(r"([0-9A-F]{8}) \S+ refs=\d+\s+(.*)", l.rstrip("\n"))
        if m:
            S[int(m[1], 16)] = m[2]
    skeys = sorted(S)

    def string_at(a):
        i = bisect.bisect_right(skeys, a) - 1
        return S[skeys[i]] if i >= 0 and a - skeys[i] < 4 else None

    refs = {}
    for f in img.fixups:
        s = string_at(f.addr)
        i = bisect.bisect_right(starts, f.src) - 1
        if s and i >= 0:
            refs.setdefault(starts[i], []).append(s)

    args = [a for a in sys.argv[1:] if a != "--write"]
    want = [by_name[a] if a in by_name else parse_addr(a) for a in args] or sorted(refs)
    out = []
    for p in want:
        name = labels.get(p, (f"FUN_{p:05x}", ""))[0]
        ss = list(dict.fromkeys(refs.get(p, [])))
        out.append(f"{p:06X} {name:24} n={len(ss):<3} " + " | ".join(x[:40] for x in ss[:6]))
    if "--write" in sys.argv:
        open(f"{ROOT}/analysis/strrefs.txt", "w").write("\n".join(out) + "\n")
        print(f"wrote analysis/strrefs.txt ({len(out)} procedures with string refs)")
    else:
        print("\n".join(out))


if __name__ == "__main__":
    main()
