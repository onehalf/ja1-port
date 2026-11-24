"""Differential test: Quick Save 0x2D880 (quick.sav) vs the port's quick-save writer.

The original QuickSave runs whole in Unicorn on a dump. Stubs: QuestionBox_ 0x278A0 -> 1 (pop 32), ShowMessage_
0x27B98 (pop 16), SaveSectorTmp 0x2F800, OpenMiscFile_ 0x38370 (sets g_datFile 0x92F54 = 1), fwrite_ 0x75FE3
(captures EAX = buf, EDX = size, EBX = n), SaveSectorTmps 0x4E740, SaveItemStash 0x51590, OpenReplayRec 0x2C834,
fclose_ 0x75A77. Its fixed part is 85,402 bytes of the globals it writes.

The port (ja1f_quick_rewrite) restores that file on a world loaded from build/DAY_day3.SAV and writes it again
(quicksave.rs). Every byte a world field holds must equal the original's; the modelled bytes are the ones that do
not move with two fills. The soldiers' +0x90 pointer bytes count too: the port writes the DOS/4GW place of the anim
script table, as the original does, because QuickRestore reads the word back as a pointer (save_xcompat).

The tail writers are stubbed, so the port's input gets a synthesized tail for its parser only: a zero sector file
for every sector the save marks (row byte 169), an empty item stash and the replay records the fixed part counts.
The tail is not compared (the original's tail was not captured).

The round trip cannot see a field at the wrong file place when reader and writer move together. So each dump runs a
second time with noise in the world blocks (0x229FA0, 854 B, not the tile set and the sector bytes) and the options
(0x1163B8, 20 B), neighbours differing, and the port's fields (ja1f_quick_fields, the file as the port reads it) must
equal the memory that the original wrote, at each field's own address (world_fields.py).

usage: .venv/bin/python validate/diff/quicksave_diff.py [DUMP ...]
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                    # noqa: E402
import world_fields                    # noqa: E402

FIXED = 85402                     # quicksave.rs FIXED: the bytes the original wrote (probe: 144 fwrite calls)
TMP_FILE = 18080                  # a saved sector file, sec%02d.tmp
SECTORS, SEC_ROW, SEC_FLAG = 0x222B74, 177, 169
DAY3 = os.path.join(ROOT, "build", "DAY_day3.SAV")
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog"), os.path.join(ROOT, "build", "game113b", "JA.EXE")
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
DUMPS = sys.argv[1:] or [os.path.join(ROOT, "build", "mem_tac60.bin")]

PORT = ctypes.CDLL(LIB)
PORT.ja1f_quick_rewrite.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p,
                                    ctypes.c_size_t, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
PORT.ja1f_quick_rewrite.restype = ctypes.c_int32
PORT.ja1f_quick_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t,
                                   ctypes.c_void_p, ctypes.c_size_t]
PORT.ja1f_quick_fields.restype = ctypes.c_int32
WORLD_BLOCK, OPTIONS, NOISE_KEEP = 0x229FA0, 0x1163B8, (152, 153)
FIELD_FLOOR = 150
ISO, EXE, DAY3 = ISO.encode(), EXE.encode(), DAY3.encode()


def noise(e):
    rng = random.Random(5)
    prev = -1
    for base, n in ((WORLD_BLOCK, 854), (OPTIONS, 20)):
        for i in range(n):
            if base == WORLD_BLOCK and i in NOISE_KEEP:
                prev = e.r8(e.rt(base + i))
                continue
            v = rng.randrange(1, 255)
            while v == prev:
                v = rng.randrange(1, 255)
            e.w8(e.rt(base + i), v)
            prev = v


def original(dump, noisy=False):
    """-> (the fixed part the original wrote, the sector tmp-file flags it wrote with, the written memory, rt)"""
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    if noisy:
        noise(e)
    out, blocks = bytearray(), []

    def fwrite(em, eax, edx, ebx, ecx):
        blocks.append((eax, edx, ebx))
        out.extend(e.uc.mem_read(eax, edx * ebx))
        return ebx

    e.stub(rt(0x278A0), lambda em, *_: 1, 32)          # QuestionBox_ "Save day in progress?" -> yes
    e.stub(rt(0x27B98), lambda em, *_: 0, 16)          # ShowMessage_ "Saving..."
    e.stub(rt(0x2F800), lambda em, *_: 0)              # SaveSectorTmp (file glue)
    e.stub(rt(0x38370), lambda em, *_: e.w32(rt(0x92F54), 1) or 0)   # OpenMiscFile_ -> g_datFile = 1
    e.stub(rt(0x75FE3), fwrite)                        # fwrite_ (EAX buf, EDX size, EBX n)
    e.stub(rt(0x4E740), lambda em, *_: 0)              # SaveSectorTmps (tail)
    e.stub(rt(0x51590), lambda em, *_: 0)              # SaveItemStash (tail)
    e.stub(rt(0x2C834), lambda em, *_: 0)              # OpenReplayRec (tail)
    e.stub(rt(0x75A77), lambda em, *_: 0)              # fclose_
    e.call(rt(0x2D880), max_steps=20_000_000)
    flags = [e.r8(rt(SECTORS + i * SEC_ROW + SEC_FLAG)) for i in range(60)]
    if len(out) != FIXED or len(blocks) < 140:
        raise AssertionError(f"{os.path.basename(dump)}: QuickSave wrote {len(out)} bytes in {len(blocks)} calls")
    segments, at = [], 0
    for buf, size, n in blocks:
        data = bytes(e.uc.mem_read(buf, size * n))
        segments.append((buf, at, data))
        at += size * n
    return bytes(out), flags, segments, rt


def port(payload, fill, cap=2_000_000):
    out = ctypes.create_string_buffer(cap)
    n = PORT.ja1f_quick_rewrite(ISO, EXE, DAY3, payload, len(payload), fill, out, cap)
    if n < 0:
        raise AssertionError(f"ja1f_quick_rewrite returned {n}")
    return out.raw[:n]


bad = 0
for dump in DUMPS:
    name = os.path.basename(dump)
    try:
        q, flags, _, _ = original(dump)
    except AssertionError as err:
        print("FAIL", err)
        bad += 1
        continue
    # the port's parser needs a tail: one zero file per marked sector, an empty stash, the replay records
    n_rec = q[0xC3B2] | q[0xC3B3] << 8
    payload = q + bytes(TMP_FILE * sum(f != 0 for f in flags) + 136 + 3 * n_rec)
    try:
        a, b = port(payload, 0xAA), port(payload, 0x55)
    except AssertionError as err:
        print("FAIL", name, err)
        bad += 1
        continue
    if len(a) != len(b) or len(a) < FIXED:
        print(f"FAIL {name}: port wrote {len(a)} / {len(b)} bytes")
        bad += 1
        continue
    # the bytes a world field holds: the same with two fills. Every one must be the original's.
    modelled = [i for i in range(FIXED) if a[i] == b[i]]
    diffs = [i for i in modelled if a[i] != q[i]]
    if len(modelled) < 84000:
        print(f"FAIL {name}: only {len(modelled)} modelled bytes")
        bad += 1
    if diffs:
        print(f"FAIL {name}: {len(diffs)} of {len(modelled)} modelled bytes differ from the original; first " +
              " ".join(f"@{i:#x}:{a[i]:02x}/{q[i]:02x}" for i in diffs[:10]))
        bad += 1
    if a[0x0E23A:FIXED] != q[0x0E23A:FIXED]:
        n = sum(x != y for x, y in zip(a[0x0E23A:FIXED], q[0x0E23A:FIXED]))
        print(f"FAIL {name}: ground items + tiles: {n} bytes differ")
        bad += 1
    print(f"{name}: {len(q)} fixed bytes, {len(modelled)} modelled, {len(diffs)} differ")
    q, flags, segments, rt = original(dump, noisy=True)
    n_rec = q[0xC3B2] | q[0xC3B3] << 8
    payload = q + bytes(TMP_FILE * sum(f != 0 for f in flags) + 136 + 3 * n_rec)
    raw = ctypes.create_string_buffer(4_000_000)
    n = PORT.ja1f_quick_fields(ISO, EXE, DAY3, payload, len(payload), raw, len(raw))
    if n < 0:
        print(f"FAIL {name} noise: ja1f_quick_fields returned {n}")
        bad += 1
        continue
    nf, nb, fdiffs, _ = world_fields.compare(world_fields.parse(raw.raw[:n]), segments, rt,
                                             lambda addr, i: addr == 0xFA958 and 0x90 <= i % 512 < 0x94)
    if nf < FIELD_FLOOR or fdiffs:
        print(f"FAIL {name} noise: {nf} fields, {len(fdiffs)} bytes differ in {sorted({d[0] for d in fdiffs})[:30]}; "
              f"first {fdiffs[:5]}")
        bad += 1
    else:
        print(f"{name} noise: {nf} fields ({nb} bytes) at their addresses, 0 differ")

print(f"CASES quicksave={len(DUMPS)}")
print(f"quicksave_diff: {len(DUMPS)} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
