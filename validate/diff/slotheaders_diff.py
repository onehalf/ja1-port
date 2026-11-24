#!/usr/bin/env python3
"""Differential test: the save list headers, ReadSaveSlotHeaders 0x4DB34, original vs port.

usage: .venv/bin/python validate/diff/slotheaders_diff.py [ncases=300] [DUMP] [--lib SO]
The original runs in Unicorn. Stubs: __STOSD, sprintf_ (Emu's), fopen_, fread_, fclose_ (they serve the case's files).
The port: saveslots::Headers::read (ja1f_slot_headers) on the same files in a temporary folder.
Compared: the count, used slots, the day of each slot (0xFFFF when free), the name of a used slot, byte 0 of a free name.
On the original side only: the name asked for (game<n>.sav, mode rb), the list 0x28E060 of used slots in order, -1 after them.
Not compared: the rest of a free name (the original keeps old bytes; the list draws byte 0 only: slots_diff), a file under 28 bytes
(no game file has that size). Exits 1 on mismatch.
"""
import ctypes, os, random, shutil, struct, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"slotheaders_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_slot_headers.argtypes = [ctypes.c_char_p] + [ctypes.c_void_p] * 3
e = Emu(dump=dump, fast=True)
rt = e.rt
LIST, DAYS, NAMES = 0x28E060, 0x28ECE4, 0x28ECF8
st = {}


def fopen(em, a, d, *_):
    name = e.cstr(a).decode()
    st["asked"].append((name, e.cstr(d).decode()))
    k = int(name[4])
    if k not in st["files"]:
        return 0
    st["cur"], st["pos"] = st["files"][k], 0
    return 0x100 + k


def fread(em, a, d, b, c, *_):
    out = st["cur"][st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, out)
    st["pos"] += len(out)
    return len(out) // max(d, 1)


def stosd(em, a, d, b, c, *_):
    # __STOSD (faults in Unicorn: its segment push): ECX dwords of EDX at EAX.
    e.uc.mem_write(a, struct.pack("<I", d) * c)
    return a


e.stub(rt(0x75F77), stosd)
e.stub_sprintf()
e.stub(rt(0x75EA0), fopen)
e.stub(rt(0x758AD), fread)
e.stub(rt(0x75A77), lambda em, *_: 0)
r = random.Random(1106)
bad = cases = 0
tmp = tempfile.mkdtemp(prefix="slothead")


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


try:
    for c in range(ncases):
        p = r.choice((0.0, 0.2, 0.5, 0.9, 1.0))
        files = {}
        for k in range(10):
            if r.random() < p:
                day = r.choice((0, 1, 7, 0xFFFF, r.randrange(65536)))
                name = r.randbytes(r.randrange(27)).replace(b"\0", b"A")
                files[k] = struct.pack("<H", day) + name.ljust(26, b"\0") + r.randbytes(r.randrange(40))
        for f in os.listdir(tmp):
            os.remove(os.path.join(tmp, f))
        for k, b in files.items():
            open(os.path.join(tmp, f"GAME{k}.SAV"), "wb").write(b)
        e.uc.mem_write(rt(DAYS), r.randbytes(20))
        e.uc.mem_write(rt(NAMES), r.randbytes(260))
        e.uc.mem_write(rt(LIST), r.randbytes(40))
        st.update(files=files, asked=[], cur=b"", pos=0)
        try:
            got = e.call(rt(0x4DB34), max_steps=400_000)
        except EmuStop as x:
            report(c, f"original stopped: {x}")
            continue
        cases += 1
        used, days, names = ctypes.create_string_buffer(10), (ctypes.c_uint16 * 10)(), ctypes.create_string_buffer(260)
        n = PORT.ja1f_slot_headers(tmp.encode(), used, days, names)
        if got != n:
            report(c, f"count original {got} port {n}")
        if st["asked"] != [(f"game{k}.sav", "rb") for k in range(10)]:
            report(c, f"names {st['asked']}")
        order = [k for k in sorted(files)] + [0xFFFFFFFF] * (10 - len(files))
        if list(struct.unpack("<10I", bytes(e.uc.mem_read(rt(LIST), 40)))) != order:
            report(c, "list of used slots")
        od = struct.unpack("<10H", bytes(e.uc.mem_read(rt(DAYS), 20)))
        on = bytes(e.uc.mem_read(rt(NAMES), 260))
        for k in range(10):
            if used.raw[k] != (k in files):
                report(c, f"slot {k} used: port {used.raw[k]}")
            if od[k] != days[k]:
                report(c, f"slot {k} day: original {od[k]} port {days[k]}")
            a, b = on[26 * k:26 * k + 26], names.raw[26 * k:26 * k + 26]
            if (a != b) if k in files else (a[0] != b[0]):
                report(c, f"slot {k} name: original {a} port {b}")
finally:
    shutil.rmtree(tmp, ignore_errors=True)
print(f"slotheaders_diff: {cases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
