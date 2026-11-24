"""Printable strings in the loaded image with the number of absolute refs (LE fixups) to each.

usage: ./t strings_dump [-g REGEX] [-n MINLEN=5] [--write]   (--write -> analysis/strings.txt)
"""
import re, sys
from collections import Counter
from le import load


def main():
    img = load()
    minlen = int(sys.argv[sys.argv.index("-n") + 1]) if "-n" in sys.argv else 5
    pat = re.compile(sys.argv[sys.argv.index("-g") + 1], re.I) if "-g" in sys.argv else None
    refs = Counter(f.addr for f in img.fixups)
    lines = []
    for o in img.objs:
        for m in re.finditer(rb"[\x20-\x7e\t\r\n]{%d,}\x00" % minlen, bytes(o.data)):
            a = o.base + m.start()
            s = m.group()[:-1].decode("latin-1").replace("\n", "\\n").replace("\r", "\\r")
            if pat and not pat.search(s):
                continue
            lines.append(f"{a:08X} {o.kind:4} refs={refs.get(a, 0):<3} {s}")
    if "--write" in sys.argv:
        open(__import__("os").path.join(__import__("le").ROOT, "analysis", "strings.txt"), "w").write("\n".join(lines) + "\n")
        print(f"{len(lines)} strings -> analysis/strings.txt")
    else:
        print("\n".join(lines))


if __name__ == "__main__":
    main()
