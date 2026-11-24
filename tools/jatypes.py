"""Typed access to JA.EXE data, driven by defs/types.toml (struct layouts + typed globals) and defs/functions.toml
(function signatures).

usage: ./t jatypes [--check]          validate both files (sizes, overlaps, labels.csv names, signature types)
       ./t jatypes --header [FILE]    C header for Ghidra / the port (default build/ja_types.h)
       ./t jatypes --doc              field tables (markdown) per struct
       ./t jatypes --json [FILE]      structs, typed globals, signatures with addresses (build/ja_types.json, Ghidra)
As a module:
    T = Types()
    s = T.view(reader, "g_soldiers")[3]     # reader: anything with .read(addr, n) (memdump.Dump, jactl.Live, Bytes)
    s.hp, s.nickname, s.items[0].type, s.raw
    T.decode("soldier", b)                   # plain dict from bytes
"""
import os
import re
import struct
import sys
import tomllib
from common import ROOT, load_labels

DEFS = os.path.join(ROOT, "defs", "types.toml")
FUNCS = os.path.join(ROOT, "defs", "functions.toml")
PRIM = {"u8": "<B", "i8": "<b", "u16": "<H", "i16": "<h", "u32": "<I", "i32": "<i", "ptr": "<I"}
CTYPE = {"u8": "uint8_t", "i8": "int8_t", "u16": "uint16_t", "i16": "int16_t", "u32": "uint32_t",
         "i32": "int32_t", "ptr": "uint32_t", "char": "char"}


class Type:
    """Parsed type string: base name + array dims, e.g. 'item[12]', 'char[15][136]' (outermost dim last).
    'T*' is a 32-bit pointer to T (decoded like ptr; .target = T); 'void' only as a pointer target or return."""
    def __init__(self, s, types):
        m = re.fullmatch(r"(\w+)\s*(\*?)((?:\[\d+\])*)", s.strip())
        if not m:
            raise ValueError(f"bad type {s!r}")
        self.base = m.group(1)
        self.target = None
        self.dims = [int(d) for d in re.findall(r"\[(\d+)\]", m.group(3))]
        self.types = types
        if self.base not in PRIM and self.base not in ("char", "void") and self.base not in types.structs:
            raise ValueError(f"unknown type {self.base!r} in {s!r}")
        if m.group(2):
            self.target, self.base = self.base, "ptr"
        elif self.base == "void":
            raise ValueError(f"void is only a pointer target or a return type: {s!r}")

    def base_size(self):
        if self.base in PRIM:
            return struct.calcsize(PRIM[self.base])
        if self.base == "char":
            return 1
        return self.types.structs[self.base]["size"]

    def size(self):
        n = self.base_size()
        for d in self.dims:
            n *= d
        return n

    def elem(self):
        """Type of one element of the outermost array (char[N] counts as a scalar string)."""
        t = Type.__new__(Type)
        t.base, t.dims, t.types = self.base, self.dims[:-1], self.types
        return t

    def is_string(self):
        return self.base == "char" and len(self.dims) == 1


class Bytes:
    """Reader over a bytes object at a base address (save files, struct buffers)."""
    def __init__(self, data, base=0):
        self.data, self.base = data, base

    def read(self, addr, n):
        o = addr - self.base
        return self.data[o:o + n]


class View:
    """Lazy typed view of memory at addr: attribute access for struct fields, indexing for arrays."""
    def __init__(self, reader, addr, typ):
        self._r, self._a, self._t = reader, addr, typ

    def __len__(self):
        return self._t.dims[-1]

    def __getitem__(self, i):
        t = self._t
        if not t.dims:
            raise TypeError("not an array")
        if i < 0:
            i += t.dims[-1]
        if not 0 <= i < t.dims[-1]:
            raise IndexError(i)
        e = t.elem()
        return value(self._r, self._a + i * e.size(), e)

    def __iter__(self):
        return (self[i] for i in range(len(self)))

    def __getattr__(self, name):
        st = self._t.types.structs.get(self._t.base)
        if self._t.dims or st is None or name not in st["by_name"]:
            raise AttributeError(name)
        off, ft = st["by_name"][name]
        return value(self._r, self._a + off, ft)

    @property
    def raw(self):
        return self._r.read(self._a, self._t.size())

    @property
    def addr(self):
        return self._a

    def dict(self):
        return self._t.types.decode_type(self._t, self.raw)

    def __repr__(self):
        return f"<{self._t.base}{''.join(f'[{d}]' for d in self._t.dims)} @0x{self._a:X}>"


def value(reader, addr, t):
    if t.is_string():
        return reader.read(addr, t.dims[0]).split(b"\0")[0].decode("latin1")
    if t.dims or t.base in t.types.structs:
        return View(reader, addr, t)
    return struct.unpack(PRIM[t.base], reader.read(addr, t.base_size()))[0]


class Types:
    def __init__(self, path=DEFS):
        d = tomllib.load(open(path, "rb"))
        self.structs = {}
        for name, s in d.get("struct", {}).items():
            self.structs[name] = {"size": s["size"], "doc": s.get("doc", ""), "fields": s["fields"], "by_name": {}}
        for name, s in self.structs.items():
            for off, ts, fname, ev, note in s["fields"]:
                s["by_name"][fname] = (off, Type(ts, self))
        self.globals = {n: Type(ts, self) for n, ts in d.get("global", {}).items()}
        self._labels = None
        self.functions = {}                     # name -> (ret type str, [(type str, name)], evidence, note)
        if os.path.exists(FUNCS):
            for name, (sig, ev, note) in tomllib.load(open(FUNCS, "rb")).get("function", {}).items():
                self.functions[name] = self.parse_sig(sig) + (ev, note)

    def parse_sig(self, sig):
        """'ret (type name, ...)' -> (ret, [(type, name)]); types checked."""
        m = re.fullmatch(r"\s*(\w+\s*\*?|\?)\s*\((.*)\)\s*", sig)
        if not m:
            raise ValueError(f"bad signature {sig!r}")
        ret = m.group(1).replace(" ", "")
        if ret not in ("void", "?"):
            Type(ret, self)
        params = []
        for p in filter(None, (x.strip() for x in m.group(2).split(","))):
            pm = re.fullmatch(r"(\w+\s*\*?|\?)\s*(\w+)", p)
            if not pm:
                raise ValueError(f"bad parameter {p!r} in {sig!r}")
            t = pm.group(1).replace(" ", "")
            if t != "?":
                Type(t, self)
            params.append((t, pm.group(2)))
        return ret, params

    def addr_of(self, name):
        if re.fullmatch(r"0x[0-9A-Fa-f]+", name):
            return int(name, 16)
        if self._labels is None:
            self._labels = {n: a for a, (n, _) in load_labels().items()}
        if name not in self._labels:
            raise KeyError(f"{name} not in labels.csv")
        return self._labels[name]

    def view(self, reader, name):
        """Typed value of a global by its labels.csv name (View for arrays/structs, int/str otherwise)."""
        return value(reader, self.addr_of(name), self.globals[name])

    def decode_type(self, t, b):
        if t.is_string():
            return b[:t.dims[0]].split(b"\0")[0].decode("latin1")
        if t.dims:
            e = t.elem()
            n = e.size()
            return [self.decode_type(e, b[i * n:(i + 1) * n]) for i in range(t.dims[-1])]
        if t.base in self.structs:
            return {f: self.decode_type(ft, b[off:off + ft.size()]) for f, (off, ft) in self.structs[t.base]["by_name"].items()}
        return struct.unpack(PRIM[t.base], b[:t.base_size()])[0]

    def decode(self, sname, b):
        return self.decode_type(Type(sname, self), b)

    # ---- checks / outputs ----
    def check(self):
        errs = []
        for name, s in self.structs.items():
            end = 0
            for off, ts, fname, ev, note in s["fields"]:
                t = s["by_name"][fname][1]
                if ev not in "VTG" or len(ev) != 1:
                    errs.append(f"{name}.{fname}: evidence {ev!r} not V/T/G")
                if off < end:
                    errs.append(f"{name}.{fname} @0x{off:X} overlaps the previous field (ends 0x{end:X}) or is out of order")
                end = off + t.size()
                if end > s["size"]:
                    errs.append(f"{name}.{fname} ends at 0x{end:X} > size 0x{s['size']:X}")
            if len(s["by_name"]) != len(s["fields"]):
                errs.append(f"{name}: duplicate field names")
        labels = {n: a for a, (n, _) in load_labels().items()}
        spans = []
        for g, t in self.globals.items():
            if g not in labels:
                errs.append(f"global {g} not in labels.csv")
                continue
            spans.append((labels[g], labels[g] + t.size(), g))
        spans.sort()
        for (a0, e0, g0), (a1, e1, g1) in zip(spans, spans[1:]):
            if a1 < e0:
                errs.append(f"globals overlap: {g0} 0x{a0:X}..0x{e0:X} and {g1} @0x{a1:X}")
        for f, (ret, params, ev, note) in self.functions.items():
            if f not in labels and not re.fullmatch(r"0x[0-9A-Fa-f]+", f):
                errs.append(f"function {f} not in labels.csv")
            if ev not in ("V", "T", "G"):
                errs.append(f"function {f}: evidence {ev!r} not V/T/G")
            if len({n for _, n in params}) != len(params):
                errs.append(f"function {f}: duplicate parameter names")
        return errs

    def header(self):
        out = ["/* generated by ./t jatypes --header from defs/types.toml -- do not edit */", "#pragma pack(push, 1)",
               "#include <stdint.h>", ""]
        for name, s in self.structs.items():
            out.append(f"/* {s['doc']} */")
            out.append(f"typedef struct {name} {{")
            pos = 0
            for off, ts, fname, ev, note in s["fields"]:
                t = s["by_name"][fname][1]
                if off > pos:
                    out.append(f"    uint8_t _pad{pos:03X}[0x{off - pos:X}];")
                ct = f"uint32_t /* {t.target}* */" if t.target else CTYPE.get(t.base, t.base)
                dims = "".join(f"[{d}]" for d in reversed(t.dims))
                c = f" /* +0x{off:X} {ev}{': ' + note if note else ''} */"
                out.append(f"    {ct} {fname}{dims};{c}")
                pos = off + t.size()
            if pos < s["size"]:
                out.append(f"    uint8_t _pad{pos:03X}[0x{s['size'] - pos:X}];")
            out.append(f"}} {name}; /* 0x{s['size']:X} */")
            out.append("")
        out.append("#pragma pack(pop)")
        return "\n".join(out) + "\n"

    def json(self):
        """{structs: {name: {size, doc, fields: [[off, type, name, ev, note]]}}, globals: [[addr, name, type]]}"""
        return {"structs": {n: {"size": s["size"], "doc": s["doc"], "fields": s["fields"]} for n, s in self.structs.items()},
                "globals": [[self.addr_of(g), g, ts] for g, ts in
                            ((g, tomllib.load(open(DEFS, "rb"))["global"][g]) for g in self.globals)],
                "labels": [[a, n, c] for a, (n, c) in sorted(load_labels().items())],
                "functions": [[self.addr_of(f), f, ret, [list(p) for p in params], ev]
                              for f, (ret, params, ev, note) in self.functions.items()]}

    def doc(self):
        out = []
        for name, s in self.structs.items():
            out += [f"### {name} (0x{s['size']:X} B)", s["doc"], "", "| Off | Type | Name | Ev | Note |", "|---|---|---|---|---|"]
            for off, ts, fname, ev, note in s["fields"]:
                out.append(f"| 0x{off:X} | {ts} | {fname} | {ev} | {note} |")
            out.append("")
        return "\n".join(out)

    def coverage(self):
        """{struct: (bytes covered by V fields, by T, by G, size)}"""
        res = {}
        for name, s in self.structs.items():
            c = {"V": 0, "T": 0, "G": 0}
            for off, ts, fname, ev, note in s["fields"]:
                c[ev] += s["by_name"][fname][1].size()
            res[name] = (c["V"], c["T"], c["G"], s["size"])
        return res


def main():
    a = sys.argv[1:]
    T = Types()
    if "--header" in a:
        i = a.index("--header")
        path = a[i + 1] if len(a) > i + 1 else os.path.join(ROOT, "build", "ja_types.h")
        open(path, "w").write(T.header())
        print(path)
        return
    if "--json" in a:
        import json
        i = a.index("--json")
        path = a[i + 1] if len(a) > i + 1 else os.path.join(ROOT, "build", "ja_types.json")
        json.dump(T.json(), open(path, "w"), indent=1)
        print(path)
        return
    if "--doc" in a:
        print(T.doc())
        return
    errs = T.check()
    for e in errs:
        print("ERROR", e)
    for name, (v, t, g, size) in T.coverage().items():
        print(f"{name:12s} 0x{size:3X} B  V {v:4d}  T {t:4d}  G {g:4d}  unmapped {size - v - t - g:4d}")
    ev = [e for _, _, e, _ in T.functions.values()]
    print(f"signatures   {len(ev)}  V {ev.count('V')}  T {ev.count('T')}  G {ev.count('G')}")
    sys.exit(1 if errs else 0)


if __name__ == "__main__":
    main()
