"""Differential test: ArcSeek 0x534A4 (speech.md "Archives"), original vs port.

usage: .venv/bin/python validate/diff/arcseek_diff.py [ncases=2000] [DUMP] [--lib SO]
The original runs in Unicorn on a dump with fseek 0x757B1 stubbed (recorded). The port side is sab::find_member, which
panel::archive_member and so every archive read of the port use (ja1f_find_member). Per case: a crafted directory
(count near the record list, upper- and lower-case names, a 16-byte name that runs on into its offset bytes, empty
records, duplicates) and a wanted name (exact, other case, shorter, longer, the record at count, the header's name,
empty). Compared: the result, offset, size, and the fseek place. Exits 1 on any mismatch.
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
ncases = int(args[0]) if args else 2000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_find_member.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_char_p, ctypes.POINTER(ctypes.c_int32)]
PORT.ja1f_find_member.restype = ctypes.c_int32

ARC_SEEK, FSEEK = 0x534A4, 0x757B1
CAP = 0x20 * 24
e = Emu(dump=dump, fast=True)
seeks = []


def fseek(_em, eax, edx, ebx, _ecx):
    seeks.append((edx, ebx))
    return 0


e.stub(e.rt(FSEEK), fseek)
dir_buf = e.alloc(bytes(CAP + 64))
name_buf = e.alloc(bytes(64))
out_off = e.alloc(bytes(4))
out_size = e.alloc(bytes(4))
PLAIN = [b"A", b"AB", b"QUOTE.8", b"BANK01.PCX", b"MAP_12.SAB", b"X" * 15]
r = random.Random(0x534A4)


def directory():
    """(archive bytes, the names of the records 1..)"""
    entries = r.randrange(0, 12)
    count = entries + r.choice([1, 1, 1, 0, 2])
    arc = bytearray(CAP)
    arc[:16] = r.choice([b"HEADER.DAT", b"QUOTE.8", b"A"]).ljust(16, b"\0")
    struct.pack_into("<II", arc, 0x10, r.randrange(1 << 16), count)
    names = []
    for i in range(1, entries + 2):
        k = r.randrange(12)
        if k == 0:
            nm = b""
        elif k == 1:
            nm = r.choice(PLAIN).lower()
        elif k == 2:
            nm = (r.choice(PLAIN) + b"Z" * 16)[:16]
        elif k == 3 and names:
            nm = r.choice(names)
        else:
            nm = r.choice(PLAIN) + b"%d" % r.randrange(10)
        names.append(nm)
        off, size = r.randrange(1 << 20) | 0x41000000 if k == 2 else r.randrange(1 << 20), r.randrange(1 << 16)
        arc[0x20 * i:0x20 * i + 16] = nm.ljust(16, b"\0")
        struct.pack_into("<II", arc, 0x20 * i + 16, off, size)
    return arc, names


def wanted(names):
    k = r.randrange(9)
    nm = r.choice(names) if names else b"NONE"
    if k == 0:
        return b""
    if k == 1:
        return b"HEADER.DAT"
    if k == 2:
        return nm.lower()
    if k == 3:
        return nm[:-1]
    if k == 4:
        return nm + b"Q"
    if k == 5:
        return b"NOT.THERE"
    return nm


bad = found = 0
for case in range(ncases):
    arc, names = directory()
    want = wanted(names)
    e.uc.mem_write(dir_buf, bytes(arc))
    e.uc.mem_write(name_buf, want.ljust(40, b"\0"))
    e.w32(out_off, 0xAAAAAAAA)
    e.w32(out_size, 0xBBBBBBBB)
    seeks.clear()
    rc = e.call(e.rt(ARC_SEEK), 0x1234, dir_buf, name_buf, out_off, out_size)
    rc = struct.unpack("<i", struct.pack("<I", rc & 0xFFFFFFFF))[0]
    orig = (rc, e.r32(out_off), e.r32(out_size), list(seeks))
    out = (ctypes.c_int32 * 2)(-1, -1)
    prc = PORT.ja1f_find_member(bytes(arc), len(arc), want, out)
    if prc == 0:
        found += 1
        port = (0, out[0] & 0xFFFFFFFF, out[1] & 0xFFFFFFFF, [(out[0] & 0xFFFFFFFF, 0)])
    else:
        port = (prc, 0xAAAAAAAA, 0xBBBBBBBB, [])
    if orig != port:
        bad += 1
        if bad <= 8:
            print("case %d count %d want %r names %r: original %r port %r"
                  % (case, struct.unpack_from("<I", arc, 0x14)[0], want, names, orig, port))
if found < ncases // 4:
    print("too few found: %d" % found)
    bad += 1
print("arcseek_diff: %d cases, %d found, %d mismatches" % (ncases, found, bad))
sys.exit(1 if bad else 0)
