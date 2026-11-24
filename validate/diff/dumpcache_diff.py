"""Test of the dump offset cache (tools/memdump.py): the cached offsets must equal a fresh search.

usage: .venv/bin/python validate/diff/dumpcache_diff.py [DUMP ...]     (default: DUMPS below; --all = every build/mem_*.bin)

Per dump: a fresh search (no cache), then a cache file written and read back must give the same offsets. A cache
file that is corrupt, of the wrong shape, or names a place that does not hold the object's bytes must cause a new
search, not a wrong offset. Another dump or another algorithm version must get another cache file.
Runs in a temporary cache folder, so it never reads what other check jobs wrote. Exits 1 on any mismatch.
"""
import glob
import json
import os
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
sys.path[:0] = [os.path.join(ROOT, "tools")]
import memdump                       # noqa: E402
from le import load                  # noqa: E402

DUMPS = ["build/mem_tac60.bin"]


def main():
    args = sys.argv[1:]
    paths = sorted(glob.glob(os.path.join(ROOT, "build", "mem_*.bin"))) if "--all" in args else \
        [a for a in args if a != "--all"] or [os.path.join(ROOT, d) for d in DUMPS]
    img = load()
    bad, cases = 0, 0

    def expect(what, got, want):
        nonlocal bad, cases
        cases += 1
        if got != want:
            bad += 1
            print(f"FAIL {what}: {got} != {want}")

    with tempfile.TemporaryDirectory() as tmp:
        memdump.CACHE_DIR = tmp
        for path in paths:
            mem = open(path, "rb").read()
            tag = os.path.basename(path)
            fresh = [memdump.locate(mem, o) for o in img.objs]
            cache = memdump.cache_path(mem, img)
            expect(f"{tag} one changed byte gets a new file",
                   memdump.cache_path(mem[:-1] + bytes([mem[-1] ^ 1]), img) != cache, True)
            expect(f"{tag} first use searches", memdump.locate_all(mem, img), fresh)
            expect(f"{tag} cache file written", os.path.exists(cache), True)
            expect(f"{tag} cache read", memdump.read_cache(cache, mem, img), fresh)
            expect(f"{tag} second use", memdump.locate_all(mem, img), fresh)
            for label, text in (("not json", "{"), ("wrong shape", "[1, 2]"), ("short", "[]"),
                                ("wrong place", json.dumps([[d + 4 if d is not None else 4, p if p is not None else 0]
                                                            for d, p in fresh]))):
                with open(cache, "w") as f:
                    f.write(text)
                expect(f"{tag} {label}: read refuses", memdump.read_cache(cache, mem, img), None)
                expect(f"{tag} {label}: search again", memdump.locate_all(mem, img), fresh)
                expect(f"{tag} {label}: file repaired", memdump.read_cache(cache, mem, img), fresh)
            d = memdump.Dump(path)
            expect(f"{tag} Dump", [x[2] for x in d.objs], [x[0] for x in fresh])
            os.environ["JA1_DUMP_CACHE"] = "0"
            expect(f"{tag} Dump, no cache", [x[2] for x in memdump.Dump(path).objs], [x[0] for x in fresh])
            del os.environ["JA1_DUMP_CACHE"]
        mem = open(paths[0], "rb").read()
        one = memdump.cache_path(mem, img)
        memdump.DELTA_ALGO += 1
        expect("a new algorithm gets a new file", memdump.cache_path(mem, img) != one, True)
        memdump.DELTA_ALGO -= 1
    print(f"dumpcache_diff: {len(paths)} dumps, {cases} cases, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
