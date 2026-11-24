#!/usr/bin/env python3
"""Every read and write of the 43-byte story block in the Rust port (docs/story_map.md).

usage: python3 tools/story_refs.py            print the table: byte, R/W, file:line, function
       python3 tools/story_refs.py --check    exit 1 when a byte that the port reads or writes has no row in
                                              docs/story_map.md, or when a story access is not resolved
The scanner knows the accessor forms of the port (s / s8 / s_put / s_add, story / story_put, m_story, sh_story,
self.story, get / put / get8 / put8 / buf8 with STORY_LEN, Self::table / buf_put with 32, story[i] = v). A literal
index or a file constant resolves; a table or a variable index (STORY_PICKUPS, wal_story 5 + a) resolves through
RESOLVED below, which a person keeps. Any other line that names the story block is listed as UNRESOLVED.
Each file's STORY_LEN is shown: an index at or past it reads 0 and a write there is dropped in that file.
"""
import os
import re
import sys
from collections import defaultdict

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = [os.path.join(ROOT, "port", "ja1logic", "src"), os.path.join(ROOT, "port", "ja1front", "src")]
MAP = os.path.join(ROOT, "docs", "story_map.md")

IDX = r"(0x[0-9A-Fa-f]+|\d+|[A-Z_][A-Z0-9_]*)"
W = r"(?:w|self\.w|e\.w)"
READ = [
    rf"\b(?:s|s8)\(\s*{W}\s*,\s*{IDX}\s*\)",
    rf"\bstory\(\s*{W}\s*,\s*{IDX}\s*\)",
    rf"\b(?:m_story|sh_story)\(\s*{IDX}\s*\)",
    rf"\b(?:self|a)\.story\(\s*{IDX}\s*\)",
    rf"\bstory\(\s*{IDX}\s*\)",
    rf"\b(?:get8?|buf8)\(\s*[\w.()*\s]*story[\w)]*\s*,\s*STORY_LEN\s*,\s*{IDX}(?:\s+as\s+i64)?\s*\)",
    rf"\bSelf::table\(\s*[\w.()*{{}}\s]*story[\w)}}\s]*,\s*32\s*,\s*{IDX}\s*\)",
]
WRITE = [
    rf"\b(?:s_put|s_add|story_put)\(\s*{W}\s*,\s*{IDX}\s*,",
    rf"\bput8?\(\s*[\w.()*\s]*story[\w)]*\s*,\s*STORY_LEN\s*,\s*{IDX}(?:\s+as\s+i64)?\s*,",
    rf"\bSelf::buf_put\(\s*story\s*,\s*32\s*,\s*{IDX}\s*,",
    rf"\bstory\[\s*{IDX}\s*\]\s*=[^=]",
]
# Accesses whose index is a table entry or a computed value: (file, line text fragment) -> [(kind, bytes)].
RESOLVED = {
    ("usetile.rs", "get(story, STORY_LEN, flag)"): [("R", [0x1A, 0x1B, 0x1C, 0x1D])],      # STORY_PICKUPS
    ("usetile.rs", "put(story, STORY_LEN, flag, 1)"): [("W", [0x1A, 0x1B, 0x1C, 0x1D])],
    ("usetile.rs", "self.story(5 + a)"): [("R", [5, 6, 7, 10, 12, 14])],                  # wal_story a = 0,1,2,5,7,9
    ("guide.rs", "s8(w, 5 + e[1] as i64)"): [("R", [5, 6])],                                # TIPS e1 = 0, 1
    ("guide.rs", "s(w, 5 + x as i64)"): [("R", [5, 6, 8, 9, 12])],                          # TIPS e2, e3 = 0, 1, 3, 4, 7
    ("guide.rs", "s8(w, 5 + k)"): [("R", [5, 6, 7, 8, 9])],                                 # tip k = 0..4
    ("guide.rs", "s_put(w, 5 + k, 1)"): [("W", [5, 6, 7, 8, 9])],
}
# Lines that name the story block but do not access a byte (declarations, copies, pointers, comments).
IGNORE = re.compile(r"^\s*(//|///|//!|\*)|story_finder|story_scene|story_day|story_find|story_step|story_hit|storyday|"
                    r"storyfind|story_sector|STORY_PICKUPS|STORY_LEN|fn (?:s|s8|s_put|s_add|story|story_put|"
                    r"m_story|sh_story)\(|story: \*mut u8|story: Vec<u8>|story: vec!|story: self\.story|"
                    r"story: w\.story|w\.story\.as_mut_ptr|copy\(&mut self\.story|&self\.story|bf!\(w\.story\)|"
                    r"put\(&mut o, 20844|let st = unsafe|let story = |\(\*w\)\.story \}, STORY_LEN, i|"
                    r"\(\*self\.w\)\.story \}, STORY_LEN, k|pub story|story: Vec|story,|wal_story|"
                    r"let p = unsafe \{ \(\*self\.w\)\.story \}|^\s*s\(w, i\) as i8 as i32|"
                    r"let v = s\(w, i\)\.wrapping_add|^\s*s_put\(w, i, v\);|eprintln!\(\"story find")


def consts(text):
    return {m.group(1): int(m.group(2), 0) for m in re.finditer(r"const ([A-Z_][A-Z0-9_]*): \w+ = (0x[0-9A-Fa-f]+|\d+);", text)}


def fn_at(lines, i):
    for j in range(i, -1, -1):
        m = re.search(r"\bfn ([a-z_0-9]+)", lines[j])
        if m:
            return m.group(1)
    return "?"


def scan():
    refs, unresolved, lens = defaultdict(list), [], {}
    for d in SRC:
        for name in sorted(os.listdir(d)):
            if not name.endswith(".rs"):
                continue
            path = os.path.join(d, name)
            text = open(path).read()
            if "story" not in text:
                continue
            cs = consts(text)
            if "STORY_LEN" in cs:
                lens[name] = cs["STORY_LEN"]
            lines = text.split("\n")
            own_s = bool(re.search(r"fn s\(w: \*mut Ja1World, i: i64\) -> u8 \{\s*get8\(unsafe \{ \(\*w\)\.story", text))
            for i, line in enumerate(lines):
                if line.startswith("#[cfg(test)]"):
                    break                                   # the tests module: fixtures, not game code
                if "story" not in line and not (own_s and re.search(r"\b(?:s|s8|s_put|s_add)\(", line)):
                    continue
                code = line.split("//")[0]
                where = f"{os.path.relpath(path, ROOT)}:{i + 1}"
                hit = False
                for frag, kinds in RESOLVED.items():
                    if frag[0] == name and frag[1] in code:
                        for kind, bs in kinds:
                            for b in bs:
                                refs[b].append((kind, where, fn_at(lines, i)))
                        hit = True
                for kind, pats in (("R", READ), ("W", WRITE)):
                    for p in pats:
                        for m in re.finditer(p, code):
                            v = m.group(1)
                            b = int(v, 0) if v[0].isdigit() else cs.get(v)
                            if b is None:
                                unresolved.append(f"{where}: index {v} unknown: {line.strip()}")
                            else:
                                refs[b].append((kind, where, fn_at(lines, i)))
                            hit = True
                if not hit and not IGNORE.search(line):
                    unresolved.append(f"{where}: {line.strip()}")
    return refs, unresolved, lens


def mapped():
    if not os.path.exists(MAP):
        return set()
    return {int(m.group(1), 16) for m in re.finditer(r"^\| 0x([0-9A-F]{2}) \|", open(MAP).read(), re.M)}


def main():
    refs, unresolved, lens = scan()
    if "--check" in sys.argv:
        bad = [f"byte 0x{b:02X}: no row in docs/story_map.md" for b in sorted(set(refs) - mapped())]
        bad += [f"UNRESOLVED {u}" for u in unresolved]
        for x in bad:
            print(x)
        print("story_refs: " + ("FAILED" if bad else f"ok ({len(refs)} bytes accessed)"))
        sys.exit(1 if bad else 0)
    print("STORY_LEN per file: " + ", ".join(f"{k} {v}" for k, v in sorted(lens.items())))
    for b in range(43):
        rs = refs.get(b, [])
        short = sorted({(k, os.path.basename(w.split(":")[0]), f) for k, w, f in rs})
        print(f"0x{b:02X} " + ("; ".join(f"{k} {f}:{fn}" for k, f, fn in short) or "-"))
    for b in sorted(set(refs) - set(range(43))):
        print(f"OUT OF BLOCK 0x{b:02X}: {refs[b]}")
    for u in unresolved:
        print("UNRESOLVED " + u)


if __name__ == "__main__":
    main()
