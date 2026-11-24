"""Differential test: the sector reset after a load (ResetSectorsFromDat 0x3712C), original vs port.

usage: .venv/bin/python validate/diff/sectorreset_diff.py [DUMP ...]

ResetSectorsFromDat runs whole in Unicorn on each dump (default build/mem_tac60.bin, mem_combat1.bin,
mem_death1.bin, mem_f3.bin) with the file calls of JAGGED.DAT handled in Python: OpenCDFile 0x38304 returns a
fake handle, fseek_ 0x757B1 moves the read position to its offset, fread_ 0x758AD copies from the real
DAT/JAGGED.DAT inside rom/JA1.gog, fclose_ 0x75A77 does nothing. The original reads 60 x 48 bytes from offset
184 and then 60 x 40 bytes, into sector record bytes 0..47 and 48..87 (world_load.md section 3 "Sector reset
after loading").

The port side is ja1f_sector_reset (world::reset_sectors) on the same 60 x 177 sector records, which start as
the dump's bytes. Every byte of every record must equal the original's after the call.

Exits 1 on any mismatch.
"""
import ctypes, random
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from common import cd_read          # noqa: E402
from emu import Emu                 # noqa: E402

LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_sector_reset.argtypes = [ctypes.c_char_p, ctypes.c_size_t, ctypes.c_void_p]
PORT.ja1f_sector_reset.restype = ctypes.c_int32

RESET, OPEN, FSEEK, FREAD, FCLOSE = 0x3712C, 0x38304, 0x757B1, 0x758AD, 0x75A77
SECTORS, COUNT, ROW = 0x222B74, 60, 177
DUMPS = ["build/mem_tac60.bin", "build/mem_combat1.bin", "build/mem_death1.bin", "build/mem_f3.bin"]
args = [a for a in sys.argv[1:] if not a.startswith("-")]
dumps = args or [os.path.join(ROOT, d) for d in DUMPS]
JAGGED = cd_read("DAT/JAGGED.DAT")


def original(dump):
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    pos = [0]
    calls = []

    def open_(em, eax, edx, ebx, ecx):
        calls.append(("open", edx))
        return 1

    def fseek(em, eax, edx, ebx, ecx):
        calls.append(("seek", edx, ebx))
        pos[0] = edx if ebx == 0 else pos[0]
        return 0

    def fread(em, eax, edx, ebx, ecx):
        n = edx * ebx
        if pos[0] + n > len(JAGGED):
            raise AssertionError(f"fread past JAGGED.DAT at {pos[0]} + {n}")
        em.uc.mem_write(eax, JAGGED[pos[0]:pos[0] + n])
        pos[0] += n
        calls.append(("read", eax, n))
        return ebx

    def fclose(em, eax, edx, ebx, ecx):
        calls.append(("close",))
        return 0

    e.stub(rt(OPEN), open_)
    e.stub(rt(FSEEK), fseek)
    e.stub(rt(FREAD), fread)
    e.stub(rt(FCLOSE), fclose)
    # Noise: the dump's rows already hold the reset values, so a row without its reset would not show.
    noise = random.Random(COUNT)
    e.uc.mem_write(rt(SECTORS), bytes(noise.randrange(256) for _ in range(COUNT * ROW)))
    before = bytes(e.uc.mem_read(rt(SECTORS), COUNT * ROW))
    e.call(rt(RESET))
    after = bytes(e.uc.mem_read(rt(SECTORS), COUNT * ROW))
    return before, after, calls


bad = 0
for dump in dumps:
    name = os.path.basename(dump)
    try:
        before, after, calls = original(dump)
    except Exception as err:                          # noqa: BLE001 - an EmuStop is a failed case too
        print(f"FAIL {name}: the original stopped: {err}")
        bad += 1
        continue
    nread = sum(1 for c in calls if c[0] == "read")
    if nread != 120 or any(c[0] != "read" or c[2] != (48 if i < 60 else 40)
                           for i, c in enumerate([c for c in calls if c[0] == "read"])):
        print(f"FAIL {name}: {nread} reads, not 60 x 48 + 60 x 40")
        bad += 1
        continue
    sectors = ctypes.create_string_buffer(before, COUNT * ROW)
    r = PORT.ja1f_sector_reset(JAGGED, len(JAGGED), sectors)
    if r != 0:
        print(f"FAIL {name}: ja1f_sector_reset returned {r}")
        bad += 1
        continue
    if sectors.raw != after:
        d = next(i for i in range(COUNT * ROW) if sectors.raw[i] != after[i])
        rec, off = divmod(d, ROW)
        print(f"FAIL {name}: sector {rec} byte {off}: port {sectors.raw[d]:#04x} original {after[d]:#04x}")
        bad += 1
        continue
    # the reset leaves the record bytes outside 0..87 as the load had them
    kept = all(before[i * ROW + j] == after[i * ROW + j]
               for i in range(COUNT) for j in range(88, ROW))
    if not kept:
        print(f"FAIL {name}: the original changed a record byte above 87")
        bad += 1

print(f"{len(dumps)} dumps, {COUNT * ROW} bytes each, {bad} failed")
sys.exit(1 if bad else 0)
