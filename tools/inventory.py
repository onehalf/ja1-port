"""Inventory of game data: every file in rom/ and inside the CD image rom/JA1.gog, with size and magic.

usage: ./t inventory      -> analysis/inventory.txt (needs bsdtar for the ISO)
"""
import os, subprocess, tarfile
from le import ROOT

ROM = os.path.join(ROOT, "rom")


def magic(b):
    if b[:2] == b"MZ": return "MZ exe"
    if b[:1] == b"\x0a" and len(b) > 3 and b[2] == 1: return "PCX"
    if b[:4] == b"FORM": return "IFF/XMI"
    if b[:4] == b"RIFF": return "RIFF"
    if b[:4] == b"%PDF": return "PDF"
    return b[:8].hex()


def main():
    rows = []
    for dp, _, fs in os.walk(ROM):
        for f in sorted(fs):
            p = os.path.join(dp, f)
            rel = os.path.relpath(p, ROM)
            if rel.startswith("__support") or rel == "JA1.gog":
                continue
            with open(p, "rb") as fh:
                rows.append(("rom", rel, os.path.getsize(p), magic(fh.read(16))))
    with subprocess.Popen(
            ["bsdtar", "cf", "-", "--format", "ustar", "@" + os.path.join(ROM, "JA1.gog")],
            stdout=subprocess.PIPE) as p:
        with tarfile.open(fileobj=p.stdout, mode="r|") as t:
            for m in t:
                if m.isfile():
                    head = t.extractfile(m).read(16)
                    rows.append(("cd", m.name, m.size, magic(head)))
    rows.sort(key=lambda r: (r[0], r[1].upper()))
    out = os.path.join(ROOT, "analysis", "inventory.txt")
    with open(out, "w") as f:
        for src, name, size, mg in rows:
            f.write(f"{src:3} {size:>10}  {mg:<18} {name}\n")
    print(f"{len(rows)} files -> {out}")


if __name__ == "__main__":
    main()
