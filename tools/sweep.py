"""Game functions without a diff test, sorted by risk to the port (issue #33).

usage: ./t sweep [--tier N] [--all]
- tier 1: called by game code; the port and docs/spec do not cite the address
- tier 2: called by game code; the port or docs/spec cites the address
- tier 3: called only by startup code (main_, _cstart_, __*, Init*), or no caller found
Each tier is sorted by size, biggest first. "Done" = the labels.csv comment holds
"diff-tested" or "not diff-testable:". Callers come from the Ghidra decomp
(analysis/decomp), so calls through a pointer table are not seen.
Without --all, each tier prints its first 20 rows.
"""
import collections, csv, json, os, re, sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")
LIB_START = 0x754F0   # first library module (tools/status.py)
DONE = ("diff-tested", "not diff-testable:")


def read_tree(d, exts):
    out = []
    for root, _, files in os.walk(os.path.join(ROOT, d)):
        if "/target" in root:
            continue
        out += [open(os.path.join(root, f), errors="replace").read() for f in files if f.endswith(exts)]
    return "\n".join(out)


def callers():
    """address -> set of caller addresses, from the decomp text"""
    ddir = os.path.join(ROOT, "analysis/decomp")
    files = {int(f[:6], 16): (f[:-2].split("_", 1)[1], open(os.path.join(ddir, f), errors="replace").read())
             for f in os.listdir(ddir) if f.endswith(".c")}
    named = {n.replace("?", "_"): a for a, (n, _) in files.items() if not n.startswith("FUN_")}
    call_rx = re.compile(r"\b(FUN_[0-9a-f]{8}|[A-Za-z_]\w*)\s*\(")
    out = collections.defaultdict(set)
    for me, (_, text) in files.items():
        for tok in set(call_rx.findall(text)):
            a = int(tok[4:], 16) if tok.startswith("FUN_") else named.get(tok)
            if a is not None and a != me:
                out[a].add(me)
    return out


def tiers():
    procs = sorted((int(p["address"], 16), int(p["size"]))
                   for p in json.load(open(os.path.join(ROOT, "analysis/decomp/procedures.json"))))
    labels = {int(r["addr"], 16): r for r in csv.DictReader(open(os.path.join(ROOT, "labels.csv"), errors="replace"))}
    port, spec = read_tree("port", (".rs",)), read_tree("docs/spec", (".md",))
    calls = callers()
    name = lambda a: (labels.get(a) or {}).get("name") or ""
    startup = lambda c: re.match(r"(main_|_cstart_|__|Init)", name(c) or "FUN_")
    cited = lambda a: re.search(r"0x0*%x\b" % a, port + spec, re.I) is not None
    out = {1: [], 2: [], 3: []}
    for a, size in procs:
        if a >= LIB_START:
            continue
        comment = ((labels.get(a) or {}).get("comment") or "").lower()
        if any(d in comment for d in DONE):
            continue
        cs = sorted(c for c in calls[a] if c < LIB_START)
        t = 3 if not cs or all(startup(c) for c in cs) else 2 if cited(a) else 1
        out[t].append((a, size, name(a) or "(unnamed)", [name(c) or "FUN_%05X" % c for c in cs]))
    for rows in out.values():
        rows.sort(key=lambda r: -r[1])
    return out


def main():
    only = int(sys.argv[sys.argv.index("--tier") + 1]) if "--tier" in sys.argv else None
    for t, rows in tiers().items():
        if only and t != only:
            continue
        unnamed = sum(1 for r in rows if r[2] == "(unnamed)")
        print("tier %d: %d functions, %d bytes, %d unnamed" % (t, len(rows), sum(r[1] for r in rows), unnamed))
        for a, size, n, cs in rows if "--all" in sys.argv else rows[:20]:
            more = " +%d" % (len(cs) - 4) if len(cs) > 4 else ""
            print("  0x%05X %5d  %-28s <- %s%s" % (a, size, n, ", ".join(cs[:4]) or "-", more))


if __name__ == "__main__":
    main()
