"""Cache of the ORIGINAL side of a diff test, for the mutation runner only (issue #60). Never used by ./t check.

Every mutant reruns the same cases with a changed port; the original side (Unicorn on JA.EXE + dump, scripted
Random) gives the same result each time. With JA1_ORIG_CACHE=DIR a test stores the expected results per case and
runs only the port on a hit. Without that variable this module does nothing.

Key (rule 1 of #60) = sha256 of: the tool name, the contents of every repo Python module the process has loaded (the
test, tools/emu.py, tools/ai.py, port_lib.py ... found in sys.modules, so helpers count), the data files the
test names (JA.EXE, the dump, external assets), the test options (argv, stub lists), the case number and the state of
the case generator at the start of the case (seed + case order). The port library is NOT in the key.
Value (rule 2): the results, not "passed". A case that stopped on a time limit is never stored (rule 3): the test
does not call put() for it. Original routines that run on the PORT's world stay live (rule 4): the tests call them on
the port side, outside the cached part.
JA1_ORIG_CACHE_VERIFY=1: run the original side as well on a hit and exit 3 when the result differs from the stored
bytes (rule 5; validate/orig_cache_proof.py). At exit a line "orig_cache: H hits, M misses, S stored" goes to stderr.
"""
import atexit
import hashlib
import os
import pickle
import sys
import tempfile
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
VERIFY_EXIT = 3


def _sha_file(path):
    with open(path, "rb") as f:
        return hashlib.sha256(f.read()).hexdigest()


def repo_modules():
    """Every Python file of this repo that the process has loaded, the running test first."""
    found = set()
    for m in list(sys.modules.values()):
        p = getattr(m, "__file__", None)
        if p and p.endswith(".py"):
            p = os.path.abspath(p)
            if p.startswith(ROOT + os.sep) and os.sep + ".venv" + os.sep not in p:
                found.add(p)
    return sorted(found)


def pack_frames(frames):
    """(result, log, (bufs, scalars)) per frame -> compact: a buffer that changed is stored as XOR with the frame
    before it (zlib then removes the zero bytes), an equal buffer as None."""
    prev, out = {}, []
    for r, log, (bufs, scalars) in frames:
        row = {}
        for k, v in bufs.items():
            p = prev.get(k)
            if p == v:
                row[k] = None
            elif p is not None and len(p) == len(v):
                row[k] = (int.from_bytes(p, "little") ^ int.from_bytes(v, "little")).to_bytes(len(v), "little")
            else:
                row[k] = (v,)
            prev[k] = v
        out.append((r, log, row, scalars))
    return out


def unpack_frames(packed):
    prev, frames = {}, []
    for r, log, row, scalars in packed:
        bufs = {}
        for k, v in row.items():
            if v is None:
                v = prev[k]
            elif isinstance(v, tuple):
                v = v[0]
            else:
                p = prev[k]
                v = (int.from_bytes(p, "little") ^ int.from_bytes(v, "little")).to_bytes(len(p), "little")
            bufs[k] = v
            prev[k] = v
        frames.append((r, log, (bufs, scalars)))
    return frames


class Cache:
    def __init__(self, tool, files, options):
        self.dir = os.environ["JA1_ORIG_CACHE"]
        self.verify = bool(os.environ.get("JA1_ORIG_CACHE_VERIFY"))
        self.hits = self.misses = self.stored = 0
        h = hashlib.sha256(repr((tool, options)).encode())
        for p in repo_modules():
            h.update(os.path.relpath(p, ROOT).encode() + _sha_file(p).encode())
        for p in files:
            h.update(os.path.basename(p).encode() + _sha_file(p).encode())
        self.run_key = h.hexdigest()
        os.makedirs(self.dir, exist_ok=True)
        atexit.register(lambda: print(f"orig_cache: {self.hits} hits, {self.misses} misses, {self.stored} stored",
                                      file=sys.stderr))

    def _path(self, case, state):
        k = hashlib.sha256(f"{self.run_key} {case} {state!r}".encode()).hexdigest()
        return os.path.join(self.dir, k[:2], k[2:] + ".z")

    def get(self, case, state):
        """The stored value, or None: run the original side (always None in VERIFY mode)."""
        p = self._path(case, state)
        if os.path.exists(p) and not self.verify:
            with open(p, "rb") as f:
                value = pickle.loads(zlib.decompress(f.read()))
            self.hits += 1
            return value
        self.misses += 1
        return None

    def put(self, case, state, value):
        data = zlib.compress(pickle.dumps(value, protocol=4), 1)
        p = self._path(case, state)
        if os.path.exists(p):
            if self.verify:
                with open(p, "rb") as f:
                    if f.read() != data:
                        print(f"orig_cache: case {case}: fresh result differs from the stored bytes", flush=True)
                        sys.exit(VERIFY_EXIT)
                self.hits += 1
            return
        os.makedirs(os.path.dirname(p), exist_ok=True)
        fd, tmp = tempfile.mkstemp(dir=os.path.dirname(p))
        with os.fdopen(fd, "wb") as f:
            f.write(data)
        os.replace(tmp, p)          # (workers share the directory)
        self.stored += 1


def open_cache(tool, files, options):
    """A Cache when JA1_ORIG_CACHE is set, else None."""
    return Cache(tool, files, options) if os.environ.get("JA1_ORIG_CACHE") else None
