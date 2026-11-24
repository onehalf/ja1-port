"""Read a DOSBox-X guest memory dump (validate/xdump.sh) by image (linear, labels.csv) address.

usage: ./t memdump <build/mem_NAME.bin> [addr|label [len]]      (no addr: print the object relocation)
DOS/4GW loads the LE objects elsewhere than their preferred bases (seen: obj1 +0x1F6000, obj2 +0x1F1000), so each
object's delta is found by matching its initialized data (obj2) or, for code, a fixup-free byte run.
As a module: d = Dump(path); d.read(0xFA958, 0x200); d.delta(0xFA958)
"""
import hashlib
import json
import os
import sys
import tempfile
from le import load
from common import ROOT, load_labels, parse_addr

# The object search below takes ~2.3 s per dump and was 14 % of the full check's test time (2026-11-01). Its result
# depends only on the bytes hashed in cache_path, so it is kept in build/dump_deltas. JA1_DUMP_CACHE=0 turns the cache
# off (validate/diff/dumpcache_diff.py compares both). Raise DELTA_ALGO when locate() changes.
DELTA_ALGO = 1
CACHE_DIR = os.path.join(ROOT, "build", "dump_deltas")


def locate(mem, o):
    """(delta, probe) of one LE object in guest memory; (None, None) when no probe is found exactly once"""
    data = bytes(o.data)
    for probe in range(0, len(data) - 64, 0x400):   # first 64-byte run found exactly once
        p = data[probe:probe + 64]
        if len(set(p)) < 8:
            continue
        i = mem.find(p)
        if i >= 0 and mem.find(p, i + 1) < 0:
            return i - probe - o.base, probe
    return None, None


def cache_path(mem, img):
    """the cache file of these exact bytes: algorithm version, each object's layout and data, the whole dump"""
    h = hashlib.blake2b(digest_size=20)
    h.update(f"algo {DELTA_ALGO} objs {len(img.objs)}\n".encode())
    for o in img.objs:
        h.update(f"{o.base} {o.vsize} {len(o.data)}\n".encode())
        h.update(bytes(o.data))
    h.update(mem)
    return os.path.join(CACHE_DIR, h.hexdigest() + ".json")


def read_cache(path, mem, img):
    """[(delta, probe), ...] from the cache file, or None when it is absent or does not fit these bytes"""
    try:
        with open(path) as f:
            rows = [(d, p) for d, p in json.load(f)]
    except (OSError, ValueError, TypeError):
        return None
    if len(rows) != len(img.objs):
        return None
    for o, (d, p) in zip(img.objs, rows):
        if d is None and p is None:
            continue
        if not isinstance(d, int) or not isinstance(p, int) or p < 0:
            return None
        run = bytes(o.data[p:p + 64])
        a = o.base + d + p
        if len(run) != 64 or a < 0 or bytes(mem[a:a + 64]) != run:     # the stored place must hold the object's bytes
            return None
    return rows


def write_cache(path, rows):
    """atomic: parallel check jobs read and write the same file; a failed write only costs the next search"""
    try:
        os.makedirs(CACHE_DIR, exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=CACHE_DIR, suffix=".tmp")
        with os.fdopen(fd, "w") as f:
            json.dump(rows, f)
        os.replace(tmp, path)
    except OSError:
        pass


def locate_all(mem, img):
    if os.environ.get("JA1_DUMP_CACHE") == "0":
        return [locate(mem, o) for o in img.objs]
    path = cache_path(mem, img)
    rows = read_cache(path, mem, img)
    if rows is None:
        rows = [locate(mem, o) for o in img.objs]
        write_cache(path, rows)
    return rows


class Dump:
    def __init__(self, path):
        """path = dump file, or the 16 MB guest memory itself (bytes, e.g. jactl.Live.snapshot())."""
        self.mem = path if isinstance(path, (bytes, bytearray)) else open(path, "rb").read()
        img = load()
        self.objs = [(o.base, o.base + o.vsize, d) for o, (d, _) in zip(img.objs, locate_all(self.mem, img))]

    def delta(self, addr):
        for lo, hi, d in self.objs:
            if lo <= addr < hi:
                if d is None:
                    raise ValueError(f"object at 0x{lo:X} not located in dump")
                return d
        raise ValueError(f"0x{addr:X} is outside the image")

    def read(self, addr, n):
        a = addr + self.delta(addr)
        return self.mem[a:a + n]


def source(arg):
    """Reader for a tool argument: 'live' = the running game (tools/jactl.py), else a dump file."""
    if arg == "live":
        from jactl import Live
        return Live()
    return Dump(arg)


def main():
    if len(sys.argv) < 2:
        print(__doc__); return
    d = Dump(sys.argv[1])
    for lo, hi, dl in d.objs:
        print(f"object 0x{lo:08X}-0x{hi:08X}: " + (f"delta +0x{dl:X} (loaded at 0x{lo + dl:X})" if dl is not None else "not found"))
    if len(sys.argv) > 2:
        by_name = {n: a for a, (n, _) in load_labels().items()}
        a = by_name.get(sys.argv[2]) or parse_addr(sys.argv[2])
        n = int(sys.argv[3], 0) if len(sys.argv) > 3 else 0x100
        b = d.read(a, n)
        for i in range(0, len(b), 16):
            row = b[i:i + 16]
            print(f"{a + i:08X}  {row.hex(' '):47}  {''.join(chr(c) if 32 <= c < 127 else '.' for c in row)}")


if __name__ == "__main__":
    main()
