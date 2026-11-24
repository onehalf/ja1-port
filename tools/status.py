"""Progress report: how much of JA.EXE is identified, named, and verified.

usage: ./t status [--md | --readme]
  --readme rewrites the block between <!-- status:start --> and <!-- status:end --> in README.md (run it at each
           session end; never edit that block by hand)
Functions = Ghidra's procedures (analysis/decomp/procedures.json, run ./t ghidra first), sizes in code bytes.
  library   = identified clib / AIL / Watcom runtime (labels tagged [libid], names AIL_*, *_ or __*)
  library?  = unnamed functions at or above LIB_START: the linker put all library modules (Watcom clib, math, AIL)
              after the game's objects; every function identified there is library code, none is game code
  named     = game functions with a labels.csv name (a trailing '?' = tentative)
  diff-tested = labels.csv comment says diff-tested (spec vs original in Unicorn, validate/diff/)
Data = struct bytes per evidence level from defs/types.toml.
"""
import datetime
import json
import os
import sys
from common import ROOT, load_labels
from jatypes import Types


LIB_START = 0x754F0          # strcmp_, first library module after the game's own (C and assembly) objects


def is_lib(name, comment):
    return "[libid]" in comment or name.startswith(("AIL_", "__", "_cstart")) or name.rstrip("?").endswith("_")


def main():
    md = "--md" in sys.argv
    p = os.path.join(ROOT, "analysis", "decomp", "procedures.json")
    if not os.path.exists(p):
        sys.exit("run ./t ghidra first (analysis/decomp/procedures.json)")
    procs = {int(x["address"], 16): x["size"] for x in json.load(open(p))}
    labels = load_labels()
    total_n, total_b = len(procs), sum(procs.values())
    cats = {"library": [0, 0], "library?": [0, 0], "game, named": [0, 0], "game, named?": [0, 0],
            "game, unnamed": [0, 0]}
    tested = [0, 0]
    for a, size in procs.items():
        name, cmt = labels.get(a, ("", ""))
        if name and is_lib(name, cmt):
            k = "library"
        elif name:
            k = "game, named?" if name.endswith("?") else "game, named"
            if "diff-tested" in cmt.lower():
                tested[0] += 1
                tested[1] += size
        elif a >= LIB_START:
            k = "library?"
        else:
            k = "game, unnamed"
        cats[k][0] += 1
        cats[k][1] += size
    game_n = total_n - cats["library"][0] - cats["library?"][0]
    game_b = total_b - cats["library"][1] - cats["library?"][1]
    rows = [(k, n, b, n / total_n, b / total_b) for k, (n, b) in cats.items()]
    rows.append(("game, diff-tested", tested[0], tested[1], tested[0] / game_n, tested[1] / game_b))
    if "--readme" in sys.argv:
        T = Types()
        ev = [e for _, _, e, _ in T.functions.values()]
        named, tent = cats["game, named"][0], cats["game, named?"][0]
        block = (f"As of {datetime.date.today().isoformat()} (`./t status`):\n\n```\n"
                 f"game functions    {game_n:4d}   named {named} + tentative {tent}, unnamed {cats['game, unnamed'][0]}\n"
                 f"diff-tested       {tested[0]:4d}   {tested[0] / game_n:.1%} of game functions "
                 f"({tested[1] / game_b:.1%} of game code bytes)\n"
                 f"signatures        {len(ev):4d}   verified {ev.count('V')}, traced {ev.count('T')}"
                 + (f", scanner guess {ev.count('G')}" if ev.count('G') else "") + "\n```\n")
        rp = os.path.join(ROOT, "README.md")
        text = open(rp).read()
        a, b = "<!-- status:start -->\n", "<!-- status:end -->"
        i, j = text.find(a), text.find(b)
        if i < 0 or j < i:
            sys.exit("README.md: no <!-- status:start --> ... <!-- status:end --> block")
        new = text[:i + len(a)] + block + text[j:]
        if new != text:
            open(rp, "w").write(new)
        print(block, end="")
        return
    if md:
        print("| Functions | count | code bytes | % count | % bytes |\n|---|---|---|---|---|")
        for k, n, b, pn, pb in rows:
            print(f"| {k} | {n} | {b} | {pn:.1%} | {pb:.1%} |")
    else:
        print(f"functions: {total_n} ({total_b} code bytes); game {game_n} ({game_b} B)")
        for k, n, b, pn, pb in rows:
            print(f"  {k:18s} {n:5d} {pn:6.1%}   {b:7d} B {pb:6.1%}")
        print("  (diff-tested % is of game functions)")
    T = Types()
    ev = [e for _, _, e, _ in T.functions.values()]
    sig = f"{len(ev)} (verified {ev.count('V')}, traced {ev.count('T')}, scanner guess {ev.count('G')})"
    print(f"\n| Signatures (defs/functions.toml) | {sig} |\n|---|---|" if md else f"  signatures {sig}")
    print()
    if md:
        print("| Struct | size | verified | traced | guess | unmapped |\n|---|---|---|---|---|---|")
    else:
        print("structs (bytes):      size   verified   traced    guess  unmapped")
    for name, (v, t, g, size) in T.coverage().items():
        u = size - v - t - g
        if md:
            print(f"| {name} | {size} | {v} | {t} | {g} | {u} |")
        else:
            print(f"  {name:14s} {size:9d} {v:10d} {t:8d} {g:8d} {u:9d}")


if __name__ == "__main__":
    main()
