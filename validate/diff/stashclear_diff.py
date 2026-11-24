"""Differential test: emptying the item store at a new game and a load (ClearItemStash? 0x515DC), original vs port.

usage: .venv/bin/python validate/diff/stashclear_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: a sorted item store (0..120 records of types 1..0x70, counts that match the records; sometimes a type with
97..99 records), the current record and Shift (0x284F21: TakeStashItem? takes a whole stack then). The original
ClearItemStash? runs in Unicorn on tac60 with the real TakeStashItem? 0x51A24 (malloc_ / free_ stubbed; the store
is the chain at 0x229C78, tail 0x229C60, current record 0x229C7C, counts 0x93DD4). Result = the counts, the chain
records and the current record; every freed node is checked to be a store node, each freed once.
--port: ja1f_stash_clear (world::clear_stash) on the same records and counts. Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_stash_clear.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_stash_clear.restype = ctypes.c_int32

CLEAR, HEAD, TAIL, CUR, COUNTS, SHIFT = 0x515DC, 0x229C78, 0x229C60, 0x229C7C, 0x93DD4, 0x284F21
e = Emu(dump=dump, fast=True)
HEAP = e.alloc(bytes(8 * 512))
st = {}


def malloc(em, eax, *_):
    a = st["heap"]
    st["heap"] = a + ((eax + 7) & ~7)
    return a


def free(em, eax, *_):
    st["freed"].append(eax)
    return 0


e.stub(e.rt(0x76383), malloc)
e.stub(e.rt(0x76325), free)


def scond(c):
    return c - 256 if c > 127 else c


def fuzz_store(r):
    recs = []
    for _ in range(r.choice([0, 1, 2, 3, 5, 8, 12, 20, 40])):
        t = r.randint(1, 0x70)
        recs.append(bytes([t, r.choice([1, 1, 1, 2, 5, r.randrange(256)]),
                           r.choice([1, 50, 100, r.randrange(110), r.randrange(128, 256)])]))
    if r.random() < .15:
        t = r.randint(1, 0x70)
        recs = [x for x in recs if x[0] != t] + [bytes([t, 1, r.choice([10, 100])]) for _ in range(r.randint(97, 99))]
    recs.sort(key=lambda x: (x[0], -scond(x[2])))
    counts = bytearray(136)
    for x in recs:
        counts[x[0]] += 1
    return counts, recs


r = random.Random(5)
bad, nrec = 0, 0
for i in range(ncases):
    counts, recs = fuzz_store(r)
    nrec += len(recs)
    shift = r.choice([0, 0, 1])
    st.update(heap=HEAP, freed=[])
    nodes = [malloc(e, 7) for _ in recs]
    for j, rec in enumerate(recs):
        e.uc.mem_write(nodes[j], rec + struct.pack("<I", nodes[j + 1] if j + 1 < len(recs) else 0))
    e.w32(e.rt(HEAD), nodes[0] if recs else 0)
    e.w32(e.rt(TAIL), nodes[-1] if recs else 0)
    e.w32(e.rt(CUR), nodes[r.randrange(len(recs))] if recs else 0)
    e.uc.mem_write(e.rt(COUNTS), bytes(counts))
    e.w8(e.rt(SHIFT), shift)
    e.call(e.rt(CLEAR))
    ocounts = bytes(e.uc.mem_read(e.rt(COUNTS), 136))
    orecs, a = [], e.r32(e.rt(HEAD))
    while a and len(orecs) < 1000:
        orecs.append(bytes(e.uc.mem_read(a, 3)))
        a = e.r32(a + 3)
    ocur = e.r32(e.rt(CUR))
    if sorted(st["freed"]) != sorted(nodes):
        print(f"case {i}: freed {len(st['freed'])} nodes of {len(nodes)}")
        bad += 1
    if PORT is None:
        continue
    pc = ctypes.create_string_buffer(bytes(counts), 136)
    pr = ctypes.create_string_buffer(b"".join(recs) or b"\0", max(1, 3 * len(recs)))
    n = PORT.ja1f_stash_clear(pc, pr, len(recs))
    prec = [pr.raw[3 * k:3 * k + 3] for k in range(n)]
    if pc.raw != ocounts or prec != orecs or (ocur != 0) != (n != 0):
        print(f"case {i}: {len(recs)} records, shift {shift}: orig counts {sum(ocounts)} records {len(orecs)} "
              f"cur {ocur:#x}, port counts {sum(pc.raw)} records {n}")
        bad += 1
        if bad > 10:
            break
print(f"stashclear_diff{' --port' if PORT else ''}: {ncases} cases ({nrec} records), {bad} mismatches")
sys.exit(1 if bad else 0)
