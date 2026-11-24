"""Differential test: the item store's load and save (LoadItemStash 0x51508, SaveItemStash 0x51590), original vs port.

usage: .venv/bin/python validate/diff/stashload_diff.py [ncases=200] [DUMP]

The original LoadItemStash runs whole in Unicorn on a dump (default build/mem_tac60.bin) with fread_ 0x758AD
fed the tail bytes and malloc_/free_ stubbed by a bump allocator (the Watcom heap in a dump does not run; the
stubs are those of stashclear_diff.py). A case is a save's stash tail: 136 count bytes, then `count[i]` 3-byte
records (type, quantity, condition) for each type i. The store is emptied first (head 0x229C78, tail 0x229C60,
current record 0x229C7C and the counts 0x93DD4 all zero; counts are filled with a pattern so a missing read
shows). Results = the counts and the store chain (7-byte nodes: type, quantity, condition, next dword).

Then the original SaveItemStash runs on that store with fwrite_ 0x75FE3 captured: its bytes must be the counts
and the chain's records.

The port side is ja1f_stash_load (world::read_stash, the reader of the save's tail) and ja1f_stash_save
(world::write_stash, the writer of the same tail). Every count and every record, in order, must equal the
original's; the port's tail must equal SaveItemStash's bytes.

Types 1..0x87 only (types above 0x87 would index past the 136 counts). A stackable type (item table 0x94DFC
+ 12 * type, byte +2 > 1) gets quantity 1: AddStashItem? 0x51D50 splits a stack into singles, so SaveItemStash
0x51590 never writes another quantity there. Every other type gets a fuzzed quantity byte (1, 2, 3, 7, any): the
original keeps it as it is (box 1 review #45 c13004: a quantity fault in read_stash / write_stash was GREEN). Counts stay 0..99, where the original's room test lets every record in. The port's SPEC GAP
covers the rest (a stackable type with quantity > 1 is split by the original, a type-0 record is refused there).

Exits 1 on any mismatch, and prints the cases, the records and the compared bytes.
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
ncases = int(argv[0]) if argv and argv[0].isdigit() else 200
dump = argv[1] if len(argv) > 1 and not argv[1].startswith("-") else os.path.join(ROOT, "build", "mem_tac60.bin")

PORT = ctypes.CDLL(LIB)
PORT.ja1f_stash_load.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_void_p,
                                 ctypes.c_int32]
PORT.ja1f_stash_load.restype = ctypes.c_int32
PORT.ja1f_stash_save.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                 ctypes.c_size_t]
PORT.ja1f_stash_save.restype = ctypes.c_int32

LOAD, SAVE, FREAD, FWRITE, MALLOC, FREE = 0x51508, 0x51590, 0x758AD, 0x75FE3, 0x76383, 0x76325
HEAD, TAIL, CUR, COUNTS = 0x229C78, 0x229C60, 0x229C7C, 0x93DD4
MAXT, MAXREC, CAP = 0x87, 120, 200

e = Emu(dump=dump, fast=True)
STACK = {t for t in range(1, MAXT + 1) if e.r8(e.rt(0x94DFE + 12 * t)) > 1}   # AddStashItem? 0x51D78: stackable types


def qty(r, t):
    return 1 if t in STACK else r.choice([1, 1, 2, 3, 7, r.randrange(256)])
HEAP = e.alloc(bytes(8 * 512))
st = {}


def malloc(em, eax, *_):
    a = st["heap"]
    st["heap"] = a + ((eax + 7) & ~7)
    return a


def free(em, eax, *_):
    return 0


e.stub(e.rt(MALLOC), malloc)
e.stub(e.rt(FREE), free)



def fuzz_tail(r):
    """a tail SaveItemStash can write: counts 0..99, quantity-1 records, types 1..0x87"""
    recs = []
    for _ in range(r.choice([0, 0, 1, 2, 3, 5, 8, 12, 20, 40])):
        t = r.randint(1, MAXT)
        recs.append(bytes([t, qty(r, t), r.choice([1, 50, 100, r.randrange(256), r.randrange(128, 256)])]))
    if r.random() < .15 and len(recs) < MAXREC:       # a nearly full type (the room test is 99)
        t = r.randint(1, MAXT)
        recs = [x for x in recs if x[0] != t] + [bytes([t, 1, r.choice([10, 100])])
                                                for _ in range(r.randint(97, 99))]
    counts = bytearray(136)
    for x in recs:
        counts[x[0]] += 1
    # SaveItemStash writes the records grouped by type; the loader reads count[i] of them in that order.
    return counts, sorted(recs, key=lambda x: x[0])


def fixed_tails():
    """edge cases first: empty, one, canteen, equals, signed conditions, a full type, every type once"""
    out = []
    out.append((bytearray(136), []))
    c = bytearray(136); c[5] = 1
    out.append((c, [bytes([5, 1, 7])]))
    c = bytearray(136); c[0x41] = 2
    out.append((c, [bytes([0x41, 1, 5]), bytes([0x41, 1, 200])]))
    c = bytearray(136); c[9] = 4
    out.append((c, [bytes([9, 1, 5]), bytes([9, 1, 5]), bytes([9, 1, 4]), bytes([9, 1, 250])]))
    c = bytearray(136); c[3] = 2; c[4] = 1
    out.append((c, [bytes([3, 1, 0x80]), bytes([3, 1, 0x7F]), bytes([4, 1, 1])]))
    c = bytearray(136); c[20] = 99
    out.append((c, [bytes([20, 1, 1]) for _ in range(99)]))
    c = bytearray(136)
    for t in range(1, MAXT + 1):
        c[t] = 1
    out.append((c, [bytes([t, 1, t]) for t in range(1, MAXT + 1)]))
    return out


def original(tail):
    st["heap"] = HEAP
    pos, written = 0, []

    def fread(em, eax, edx, ebx, ecx):
        nonlocal pos
        n = edx * ebx
        em.uc.mem_write(eax, tail[pos:pos + n])
        pos += n
        return ebx

    def fwrite(em, eax, edx, ebx, ecx):
        n = edx * ebx
        written.append(bytes(em.uc.mem_read(eax, n)))
        return ebx

    e.stub(e.rt(FREAD), fread)
    e.stub(e.rt(FWRITE), fwrite)
    e.w32(e.rt(HEAD), 0)
    e.w32(e.rt(TAIL), 0)
    e.w32(e.rt(CUR), 0)
    e.uc.mem_write(e.rt(COUNTS), bytes([0xAA]) * 136)
    e.call(e.rt(LOAD))
    counts = bytes(e.uc.mem_read(e.rt(COUNTS), 136))
    recs, a = [], e.r32(e.rt(HEAD))
    while a and len(recs) < 1000:
        recs.append(bytes(e.uc.mem_read(a, 3)))
        a = e.r32(a + 3)
    e.call(e.rt(SAVE))
    return counts, recs, pos, b"".join(written)


def port(tail):
    counts = ctypes.create_string_buffer(136)
    out = ctypes.create_string_buffer(3 * CAP)
    n = PORT.ja1f_stash_load(bytes(tail), len(tail), counts, out, CAP)
    if n < 0:
        return None, None, None
    recs = [out.raw[3 * k:3 * k + 3] for k in range(n)]
    flat = b"".join(recs)
    wout = ctypes.create_string_buffer(136 + 3 * CAP)
    m = PORT.ja1f_stash_save(counts, flat, n, wout, len(wout))
    if m < 0:
        return counts.raw, recs, None
    return counts.raw, recs, wout.raw[:m]


r = random.Random(11)
cases = fixed_tails() + [fuzz_tail(r) for _ in range(ncases)]
bad = nrec = 0
for k, (counts, recs) in enumerate(cases):
    tail = bytes(counts) + b"".join(recs)
    try:
        ocounts, orecs, pos, osaved = original(tail)
    except Exception as err:                          # noqa: BLE001 - an EmuStop is a failed case too
        print(f"FAIL case {k}: the original stopped: {err}")
        bad += 1
        continue
    pcounts, precs, psaved = port(tail)
    nrec += len(recs)
    errs = []
    if pos != 136 + 3 * len(recs):
        errs.append(f"read {pos} bytes, the tail holds {136 + 3 * len(recs)}")
    if ocounts != bytes(counts):
        errs.append("the counts do not come back from the file")
    # the store the original must build: sorted (type up, signed condition down), a canteen's condition 100
    want = sorted((bytes([x[0], x[1], 100 if x[0] == 0x41 else x[2]]) for x in recs),
                  key=lambda x: (x[0], -(x[2] - 256 if x[2] > 127 else x[2])))
    if orecs != want:
        errs.append(f"the original's store is not the sorted records ({len(orecs)} of {len(recs)})")
    if pcounts != ocounts:
        d = [i for i in range(136) if pcounts[i] != ocounts[i]]
        errs.append(f"counts differ at {d[:6]}")
    if precs != orecs:
        d = next((i for i, (a, b) in enumerate(zip(precs, orecs)) if a != b), min(len(precs), len(orecs)))
        errs.append(f"records differ at {d}: port {precs[d:d + 1]} original {orecs[d:d + 1]}")
    if osaved != bytes(ocounts) + b"".join(orecs):
        errs.append(f"SaveItemStash wrote {len(osaved)} bytes, not the counts and the chain")
    if psaved != osaved:
        d = next((i for i, (a, b) in enumerate(zip(psaved or b"", osaved)) if a != b), None)
        errs.append(f"the port's tail differs from SaveItemStash at {d}")
    if errs:
        bad += 1
        print(f"FAIL case {k} ({len(recs)} records): " + "; ".join(errs))
        if bad >= 10:
            print("stopping after 10 failed cases")
            break

print(f"{len(cases)} cases, {nrec} records, {bad} failed")
sys.exit(1 if bad else 0)
