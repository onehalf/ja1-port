"""Identify library functions (Miles AIL 3 now; Watcom clib by signature: see REVERSE.md) and print labels.csv rows.

usage: ./t libid ail [--write]       AIL public wrappers + the AIL_API_* functions they call
       ./t libid omf LIB.lib.. [--write]  match OMF library modules (Open Watcom clib3r.lib etc.) against the code:
                                    module code with fixup bytes wildcarded must occur exactly once
                                    --fuzzy: also near matches of function starts (labelled with '?')
                                    --tag "Watcom 10.0a": library version named in the label comments
--write appends the rows that labels.csv does not have yet (existing names are never overwritten) and rewrites the
AIL section of defs/cdecl.txt.

AIL 3 wrappers all share one shape: push regs; mov edx,[AIL_indent]; inc edx; ... push "AIL_name(...)\\n" format;
call AIL_fprintf; ... call AIL_API_name; ... dec AIL_indent. The name comes from the format string, the API
function is the first call after the debug block that is not one of the logging helpers.
"""
import os
import re
import struct
import sys
from capstone import CS_OP_IMM
from common import ROOT, disassembler, load_labels
from le import load

AIL_INDENT = 0x3391DC                 # wrapper nesting counter (debug output indents by it)
SIG = b"\x8b\x15" + struct.pack("<I", AIL_INDENT) + b"\x42"      # mov edx,[AIL_indent]; inc edx


def cstr(img, a, n=80):
    b = img.read(a, n)
    return b.split(b"\0")[0].decode("latin1")


def ail(img):
    code = bytes(img.objs[0].data)
    base = img.objs[0].base
    starts = []
    i = 0
    while (i := code.find(SIG, i)) >= 0:
        s = i
        while s > 0 and 0x50 <= code[s - 1] <= 0x57:          # single-byte pushes of the prologue
            s -= 1
        starts.append(base + s)
        i += 1
    md = disassembler()
    md.detail = True
    rows = []
    helpers = {}
    for k, s in enumerate(starts):
        end = starts[k + 1] if k + 1 < len(starts) else s + 0x200
        name, calls = None, []
        for ins in md.disasm(img.read(s, end - s), s):
            if ins.mnemonic == "push" and ins.operands and ins.operands[0].type == CS_OP_IMM:
                v = ins.operands[0].imm
                if name is None and img.obj_at(v) is not None and img.obj_at(v).kind == "data":
                    m = re.match(r"(AIL_\w+)\(", cstr(img, v))
                    if m:
                        name = m.group(1)
            if ins.mnemonic == "call" and ins.operands[0].type == CS_OP_IMM:
                calls.append((ins.address, ins.operands[0].imm, name is not None))
            if ins.mnemonic in ("ret", "jmp") and ins.address > s + 0x40 and ins.mnemonic == "ret":
                break
        if not name:
            continue
        for _, t, _ in calls:
            helpers[t] = helpers.get(t, 0) + 1
        rows.append((s, name, calls))
    # helpers = call targets shared by (almost) every wrapper: logging / reentrancy checks
    common = {t for t, n in helpers.items() if n > len(rows) // 2}
    out = []
    for s, name, calls in rows:
        api = next((t for _, t, after in calls if after and t not in common), None)
        out.append((s, name, "AIL 3 public wrapper (cdecl; logs the call when AIL debug is on) [libid]"))
        if api is not None and not name.startswith("AIL_API_"):
            out.append((api, name.replace("AIL_", "AIL_API_", 1), f"AIL 3 implementation behind {name} (cdecl) [libid]"))
    return out, common


def omf_modules(path):
    """Yield (module name, code bytes, set of fixup byte offsets, {public: offset}) for each module's code segment(s)
    of an OMF library (Watcom: segment class CODE). Several code segments are concatenated in definition order."""
    d = open(path, "rb").read()
    page = struct.unpack_from("<H", d, 1)[0] + 3 if d[0] == 0xF0 else 16
    pos = page if d[0] == 0xF0 else 0
    while pos < len(d) and d[pos] != 0xF1:
        name, lnames, segs, data, fix, pubs = "", [""], [], {}, {}, {}
        last = None                               # (segidx, offset) of the last LEDATA, for FIXUPP
        while True:
            typ = d[pos]
            ln = struct.unpack_from("<H", d, pos + 1)[0]
            body = d[pos + 3:pos + 3 + ln - 1]
            pos += 3 + ln
            big = typ & 1
            if typ == 0x80:
                name = body[1:1 + body[0]].decode("latin1")
            elif typ == 0x96:
                i = 0
                while i < len(body):
                    lnames.append(body[i + 1:i + 1 + body[i]].decode("latin1"))
                    i += 1 + body[i]
            elif typ in (0x98, 0x99):
                acbp = body[0]
                i = 1
                size = struct.unpack_from("<I" if big else "<H", body, i)[0]
                i += 4 if big else 2
                if acbp >> 5 == 0:                # absolute: frame + offset
                    i += 3
                segname, clsname = idx(body, i)
                i2 = idx_len(body, i)
                cls, _ = idx(body, i2)
                segs.append((lnames[segname], lnames[cls], size))
            elif typ in (0x90, 0x91):
                i = 0
                grp, i = idx(body, i)
                seg, i = idx(body, i)
                if seg == 0:
                    i += 2
                try:
                    while i < len(body):
                        n = body[i]
                        pname = body[i + 1:i + 1 + n].decode("latin1")
                        i += 1 + n
                        off = struct.unpack_from("<I" if big else "<H", body, i)[0]
                        i += 4 if big else 2
                        _, i = idx(body, i)
                        pubs[pname] = (seg, off)
                except (struct.error, IndexError):
                    pass                          # odd PUBDEF variant (EMU387): keep what was read
            elif typ in (0xA0, 0xA1):
                seg, i = idx(body, 0)
                off = struct.unpack_from("<I" if big else "<H", body, i)[0]
                i += 4 if big else 2
                buf = data.setdefault(seg, bytearray())
                chunk = body[i:]
                if len(buf) < off + len(chunk):
                    buf.extend(b"\0" * (off + len(chunk) - len(buf)))
                buf[off:off + len(chunk)] = chunk
                last = (seg, off)
            elif typ in (0x9C, 0x9D):
                i = 0
                while i < len(body):
                    b = body[i]
                    if b & 0x80:                  # FIXUP subrecord
                        locat = (b << 8) | body[i + 1]
                        loc = (locat >> 10) & 0xF
                        drec = locat & 0x3FF
                        i += 2
                        fixdat = body[i]
                        i += 1
                        if not fixdat & 0x80:     # frame by datum
                            if (fixdat >> 4) & 7 < 3:
                                _, i = idx(body, i)
                        if not fixdat & 0x08:     # target by datum
                            _, i = idx(body, i)
                        if not fixdat & 0x04:     # displacement present
                            i += 4 if big else 2
                        n = {0: 1, 1: 2, 2: 2, 3: 4, 4: 1, 5: 2, 9: 4, 11: 6, 13: 4}.get(loc, 4)
                        if last:
                            fix.setdefault(last[0], set()).update(range(last[1] + drec, last[1] + drec + n))
                    else:                         # THREAD subrecord
                        method = (b >> 2) & 7
                        i += 1
                        if not (b & 0x40 and method >= 4) and method < 3 or (not b & 0x40 and method < 4):
                            if method < 3 or not b & 0x40:
                                _, i = idx(body, i)
            elif typ in (0x8A, 0x8B):
                break
        for si, (sname, cls, size) in enumerate(segs, 1):
            if cls.upper().endswith("CODE") and si in data:
                p = {n: o for n, (sg, o) in pubs.items() if sg == si}
                yield name, bytes(data[si]), fix.get(si, set()), p
        pos = (pos + page - 1) // page * page


def idx(b, i):
    """OMF index field -> (value, next position)"""
    if b[i] & 0x80:
        return ((b[i] & 0x7F) << 8) | b[i + 1], i + 2
    return b[i], i + 1


def idx_len(b, i):
    return i + (2 if b[i] & 0x80 else 1)


def omf_match(img, libs, prefix=48, min_fixed=20):
    """[(addr, public, module, lib, kind)]: a module whose masked code occurs exactly once in obj1 ('module'),
    else each public whose first `prefix` masked bytes (to the next public at most) occur exactly once ('prefix')."""
    code = bytes(img.objs[0].data)
    base = img.objs[0].base
    out, stats = [], {"modules": 0, "module": 0, "prefix": 0}

    def find(b, fix, lo, hi):
        if sum(1 for i in range(lo, hi) if i not in fix) < min_fixed:
            return None
        pat = b"".join(b"." if i in fix else re.escape(bytes([b[i]])) for i in range(lo, hi))
        it = re.finditer(pat, code, re.DOTALL)
        first = next(it, None)
        return first.start() if first and next(it, None) is None else None

    for lib in libs:
        for mod, b, fix, pubs in omf_modules(lib):
            if not pubs:
                continue
            stats["modules"] += 1
            h = find(b, fix, 0, len(b))
            if h is not None:
                stats["module"] += 1
                out += [(base + h + o, p, mod, os.path.basename(lib), "module") for p, o in pubs.items()]
                continue
            offs = sorted(set(pubs.values())) + [len(b)]
            for p, o in pubs.items():
                end = min(offs[offs.index(o) + 1], o + prefix)
                h = find(b, fix, o, end)
                if h is not None:
                    stats["prefix"] += 1
                    out.append((base + h, p, mod, os.path.basename(lib), "prefix"))
    # one address claimed by prefix matches from different modules = ambiguous (e.g. open/uopen): drop those
    mods = {}
    for r in out:
        mods.setdefault(r[0], set()).add(r[2])
    out = [r for r in out if r[4] == "module" or len(mods[r[0]]) == 1]
    return out, stats


def omf_fuzzy(img, libs, taken, n=64, min_fixed=24, accept=0.85, margin=0.15):
    """Publics not matched exactly: compare their first n masked bytes with every Ghidra function start
    (analysis/decomp/procedures.json); best score >= accept and ahead of the runner-up by margin -> candidate.
    taken = addresses already identified (skipped). Returns [(addr, public, module, lib, score)]."""
    import json
    procs = [int(p["address"], 16) for p in json.load(open(os.path.join(ROOT, "analysis", "decomp", "procedures.json")))]
    procs = [p for p in procs if p not in taken]
    window = {p: img.read(p, n) for p in procs}
    best = {}
    for lib in libs:
        for mod, b, fix, pubs in omf_modules(lib):
            offs = sorted(set(pubs.values())) + [len(b)]
            for pub, o in pubs.items():
                end = min(offs[offs.index(o) + 1], o + n)
                idxs = [i for i in range(o, end) if i not in fix]
                if len(idxs) < min_fixed:
                    continue
                scores = sorted(((sum(window[p][i - o] == b[i] for i in idxs if i - o < len(window[p])) / len(idxs), p)
                                 for p in procs), reverse=True)
                (s1, p1), (s2, _) = scores[0], scores[1]
                if s1 >= accept and s1 - s2 >= margin:
                    if p1 not in best or best[p1][4] < s1:
                        best[p1] = (p1, pub, mod, os.path.basename(lib), s1)
    return sorted(best.values())


def main():
    a = sys.argv[1:]
    if a and a[0] == "omf":
        img = load()
        tag = a[a.index("--tag") + 1] if "--tag" in a else "Watcom"
        libs = [x for x in a[1:] if not x.startswith("--") and x != tag]
        rows, st = omf_match(img, libs)
        labels = load_labels()
        for addr, p, mod, lib, kind in sorted(rows):
            have = labels.get(addr)
            print(f"0x{addr:X},{p},{lib}:{mod} {kind}" + (f"   (labels.csv: {have[0]})" if have else ""))
        print(f"# {st['modules']} modules with code: {st['module']} matched whole, {st['prefix']} publics by prefix "
              f"-> {len(rows)} publics", file=sys.stderr)
        fz = omf_fuzzy(img, libs, {r[0] for r in rows} | set(labels)) if "--fuzzy" in a else []
        for addr, p, mod, lib, sc in fz:
            print(f"0x{addr:X},{p}?,{lib}:{mod} fuzzy {sc:.0%}")
        if fz:
            print(f"# fuzzy: {len(fz)} more", file=sys.stderr)
        if "--write" in a:
            # one row per address: aliases (stricmp_/strcmpi_/_stricmp_) and every library that matched go in the comment
            by = {}
            for addr, p, mod, lib, kind in rows:
                e = by.setdefault(addr, {"names": [], "src": [], "kind": kind, "fuzzy": None})
                e["names"].append(p)
                e["src"].append(f"{lib}:{mod}")
                if kind == "module":
                    e["kind"] = "module"
            for addr, p, mod, lib, sc in fz:
                if addr not in by:
                    by[addr] = {"names": [p], "src": [f"{lib}:{mod}"], "kind": "fuzzy", "fuzzy": sc}
            new = []
            for addr in sorted(by):
                if addr in labels:
                    continue
                e = by[addr]
                names = sorted(set(e["names"]), key=lambda n: (n.startswith("_"), len(n), n))
                src = ", ".join(sorted(set(e["src"])))
                alias = f"; aliases {' '.join(names[1:])}" if len(names) > 1 else ""
                if e["kind"] == "fuzzy":
                    new.append(f'0x{addr:X},{names[0]}?,"Watcom clib? {tag} {src}: first bytes {e["fuzzy"]:.0%} similar, '
                               f'unique best (fuzzy){alias} [libid]"')
                else:
                    how = "whole module" if e["kind"] == "module" else "first bytes"
                    new.append(f'0x{addr:X},{names[0]},"Watcom clib: {tag} {src}, {how} match (fixups masked, '
                               f'unique){alias} [libid]"')
            with open(os.path.join(ROOT, "labels.csv"), "a") as f:
                f.write("".join(r + "\n" for r in new))
            print(f"wrote {len(new)} labels", file=sys.stderr)
        return
    if not a or a[0] != "ail":
        sys.exit(__doc__)
    img = load()
    rows, common = ail(img)
    seen, uniq = set(), []
    for r in rows:
        if r[0] not in seen:
            seen.add(r[0])
            uniq.append(r)
    labels = load_labels()
    new = [r for r in uniq if r[0] not in labels]
    for addr, name, cmt in uniq:
        flag = "" if addr not in labels else f"   (labels.csv: {labels[addr][0]})"
        print(f"0x{addr:X},{name},{cmt}{flag}")
    print(f"# {len(uniq)} functions, {len(new)} new; logging helpers: " + " ".join(f"0x{t:X}" for t in sorted(common)),
          file=sys.stderr)
    if "--write" in a:
        with open(os.path.join(ROOT, "labels.csv"), "a") as f:
            for addr, name, cmt in new:
                f.write(f'0x{addr:X},{name},"{cmt}"\n')
        lo = min(r[0] for r in uniq)
        hi = max(r[0] for r in uniq)
        path = os.path.join(ROOT, "defs", "cdecl.txt")
        keep = []
        if os.path.exists(path):
            skip = False
            for line in open(path):
                if line.startswith("# --- AIL"):
                    skip = True
                elif line.startswith("# ---"):
                    skip = False
                if not skip:
                    keep.append(line)
        keep += ["# --- AIL (./t libid ail --write): every identified wrapper and API function\n"]
        keep += [f"0x{r[0]:X}  # {r[1]}\n" for r in sorted(uniq)]
        keep += [f"0x{t:X}  # AIL logging helper\n" for t in sorted(common)]
        open(path, "w").write("".join(keep))
        print(f"wrote {len(new)} labels, defs/cdecl.txt AIL section (0x{lo:X}..0x{hi:X})", file=sys.stderr)


if __name__ == "__main__":
    main()
