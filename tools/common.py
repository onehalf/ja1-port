"""Shared helpers: labels.csv, address parsing, Capstone setup (32-bit flat, linear addresses)."""
import csv, os
from capstone import Cs, CS_ARCH_X86, CS_MODE_32

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LABELS = os.path.join(ROOT, "labels.csv")


def cd_read(path):
    """Bytes of a file inside the CD image rom/JA1.gog, e.g. cd_read('DAT/JAGGED.DAT') (bsdtar reads ISO 9660)."""
    import subprocess
    return subprocess.check_output(["bsdtar", "-xOf", os.path.join(ROOT, "rom", "JA1.gog"), path])


# optional: the official 1.13b patch files (JA.EXE, DAT/XRA.DAT, DAT/QUESTS.DAT) in patch113b/ (README "Setup")
PATCH113B = os.environ.get("JA1_PATCH113B") or os.path.join(ROOT, "patch113b")


def game_path(path):
    """Path of a game-folder file that overrides the CD (port target 1.13b, #24): the official 1.13b patch files
    first, then the GOG 1.13 game folder rom/; None when neither has it."""
    rel = path.replace("\\", "/")
    for base in (PATCH113B, os.path.join(ROOT, "rom")):
        p = os.path.join(base, rel)
        if os.path.isfile(p):
            return p
    return None


def game_read(path):
    """Bytes of a file as OpenCDFile 0x38304 opens it: the game-folder file first (game_path: 1.13b patch, then
    rom/), the CD file (cd_read) only when it is absent. Use it for every read the game makes through OpenCDFile."""
    p = game_path(path)
    if p is not None:
        return open(p, "rb").read()
    return cd_read(path.replace("\\", "/"))


def parse_addr(s):
    """'0x66F6C' / '66F6C' = linear address; 'F:0x37D4C' = JA.EXE file offset."""
    s = s.strip()
    if s.upper().startswith("F:"):
        from le import load
        lin = load().file_to_lin(int(s[2:], 16))
        if lin is None:
            raise ValueError(f"file offset {s} is not inside a loaded page")
        return lin
    return int(s, 16)


def load_labels():
    """{linear_addr: (name, comment)}"""
    out = {}
    if not os.path.exists(LABELS):
        return out
    with open(LABELS, newline="") as f:
        for row in csv.reader(f):
            if not row or row[0].startswith("#") or row[0] == "addr":
                continue
            out[parse_addr(row[0])] = (row[1] if len(row) > 1 else "", row[2] if len(row) > 2 else "")
    return out


def disassembler():
    md = Cs(CS_ARCH_X86, CS_MODE_32)
    md.detail = False
    return md


def load_funcs():
    """{linear_addr: name} — Ghidra procedures (analysis/decomp/procedures.json, ./t ghidra) overlaid with labels.csv
    code names."""
    import json
    out = {}
    p = os.path.join(ROOT, "analysis", "decomp", "procedures.json")
    if os.path.exists(p):
        for it in json.load(open(p)):
            out[int(it["address"], 16)] = it["name"]
    from le import load
    img = load()
    for a, (n, _) in load_labels().items():
        o = img.obj_at(a)
        if o is not None and o.kind == "code" and (a in out or not out):
            out[a] = n
    return out
