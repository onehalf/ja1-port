"""Differential test: Quick Restore 0x2C898 (quick.sav -> globals) vs the port's quick-restore reader.

The original QuickRestore runs whole in Unicorn on a dump with a real quick.sav fed through fread_ 0x758AD
(captured: dest, size*n; the bytes come from the file in order). Its other callees are stubs: QuestionBox_ 0x278A0
-> 1 (pop 32), ShowMessage_ 0x27B98 (pop 16), OpenCDFile 0x38304, StopMusic 0x5C82C, StopAllSounds_ 0x5CE08,
fclose_ 0x75A77, FadePalette_ 0x52908 (pop 4), memset_ 0x756A0, SetMusic 0x5C5EC and the setup steps 0x4E93C,
InitTempFiles 0x38038, CopySaveChunks_ 0x4E840, ClearItemStash_ 0x515DC, LoadItemStash 0x51508,
CopyReplayRec_ 0x2C7D0, FixNewProfiles_ 0x503F8, 0x2884C, ResetSectorsFromDat 0x3712C, LoadTerrainTileset
0x2A9FC, 0x46A4C, 0x67BF0, LoadStripPccs_ 0x2F928, 0x681D0, 0x1CB78. So this test covers the file layout and the
read itself; the setup steps are their own functions with their own tests.

The read must be the QuickSave 0x2D880 layout: 144 fread calls, 85,402 bytes (quicksave.rs FIXED), and every
byte it writes into memory must be the file's byte. Three bytes are the function's own post-read changes and
are left out of that compare: the playing theme 0x9635E/0x9635F -> 0xFFFF, and 0x284F13 -> 1. They are compared on
their own (box 1 review #45 c13004): over_map 0x284F13, the theme word and the SetMusic 0x5C5EC call (the file's
theme when it is above 0, else none) vs quicksave::after_restore of the file's theme (ja1f_quick_after).

The port side is ja1f_quick_rewrite (quicksave.rs restore + write) on the same file over a world loaded from
build/DAY_day3.SAV. Every byte it models (equal under two fills, like quicksave_diff) must equal the original
memory after the restore, at the same file offset. The soldiers' +0x90 pointer word is left out: the restore's own
loop rewrites it in memory from the offset at +0x98, with the base of the dump's Emu.

The real files hold many equal neighbours (zeros), so a field read one byte off can still get the right value. Each
pair therefore runs twice: with the real file, and with the world blocks (0x0A0B2, 854 B) and the options (0x0A408,
20 B) filled with noise in which neighbours always differ. The noise is only file data for this read: the setup
steps are stubs in the original, and the port needs only the sector and tile set bytes, which keep their values.

The round trip cannot see a field in the wrong place (the writer undoes the reader; #39). So the test also reads the
port's world field by field (ja1f_quick_fields, quicksave::field_table) and compares each field with the original's
memory at that field's own address (world_fields.py). This part is what catches a swap of two fields.

usage: .venv/bin/python validate/diff/quickrestore_diff.py [PAYLOAD DUMP ...]   (pairs; default set below)
"""
import ctypes
import os
import random
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                    # noqa: E402
import world_fields                    # noqa: E402

FIXED = 85402                     # quicksave.rs FIXED: QuickSave's fixed part (144 writes)
READS = 144                       # fread_ calls in QuickRestore (probe)
NOISE_KEEP = (152, 153)           # block 5: the tile set and the current sector (the port's loader opens their files)
# Blocks 10..141 under noise, but not: the replay record count (the port cuts the records off the file tail with it),
# and quicksave::fixed_state, the bytes that the port writes as constants and does not read (the plain file runs compare
# them), and the volume 0x963A4 (always 127 in the file). The noise runs do not compare these bytes.
NOISE_FIXED = {0xC3B2, 0xC3B3, 0xBDC9, 0xBDCA, 0xBDCB, 0xC9C7, 0xE0B7, 0xE0F2, 0xE0F6, 0xE220,
               *range(0xE0C2, 0xE0C6), *range(0xE0D2, 0xE0F2), *range(0xE22E, 0xE232), *range(0xE236, 0xE23A)}
# File ranges that the port must model (equal under both fills): the options (block 6, 20 B: the 16 Control Panel
# switches and the 4 bytes after them; the switch box draws a lever for all 20) and the sector order (block 7, 61 B).
# These live in the world's day-save image, not in a field, so a byte that the restore forgets keeps the fill and
# would otherwise count as "not modelled" (#45 qr_f: 19 of the 20 option bytes copied stayed green).
MUST_MODEL = (*range(0x0A408, 0x0A408 + 20), *range(0x0A41C, 0x0A41C + 61))
SKIP_IMAGE = (0x9635E, 0x9635F, 0x284F13)   # post-read: theme = -1, 0x284F13 = 1
DAY3 = os.path.join(ROOT, "build", "DAY_day3.SAV")
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog"), os.path.join(ROOT, "build", "game113b", "JA.EXE")
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
DEFAULT = [("build/QUICK_t10.SAV", "build/mem_tac60.bin"),
           ("build/QUICK_t2.SAV", "build/mem_combat1.bin"),
           ("build/QUICK_s59start.SAV", "build/mem_fight20.bin")]
PAIRS = list(zip(sys.argv[1::2], sys.argv[2::2])) or DEFAULT

STUBS = ((0x278A0, 32), (0x5C82C, 0), (0x5CE08, 0), (0x27B98, 16), (0x38304, 0), (0x4E93C, 0),
         (0x38038, 0), (0x4E840, 0), (0x515DC, 0), (0x51508, 0), (0x2C7D0, 0), (0x75A77, 0),
         (0x503F8, 0), (0x2884C, 0), (0x3712C, 0), (0x2A9FC, 0), (0x52908, 4), (0x756A0, 0),
         (0x46A4C, 0), (0x67BF0, 0), (0x2F928, 0), (0x681D0, 0), (0x1CB78, 0), (0x5C5EC, 0))

PORT = ctypes.CDLL(LIB)
PORT.ja1f_quick_rewrite.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p,
                                    ctypes.c_size_t, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
PORT.ja1f_quick_rewrite.restype = ctypes.c_int32
PORT.ja1f_quick_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t,
                                   ctypes.c_void_p, ctypes.c_size_t]
PORT.ja1f_quick_fields.restype = ctypes.c_int32
PORT.ja1f_quick_after.argtypes = [ctypes.c_int32, ctypes.c_void_p]
PORT.ja1f_quick_after.restype = None
POST = {}
PORT.ja1f_sound_volumes.argtypes = [ctypes.c_void_p]
PORT.ja1f_sound_volumes.restype = None
ISO, EXE, DAY3 = ISO.encode(), EXE.encode(), DAY3.encode()


def original(dump, payload):
    """-> [(dest dump address, file offset, memory bytes after the restore)] and the skipped dump addresses.
    The memory comes from a read-back after the call, so the function's own changes show up."""
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    reads, pos = [], 0

    def fread(em, eax, edx, ebx, ecx):
        nonlocal pos
        n = edx * ebx
        reads.append((eax, pos, n))
        if pos + n > len(payload):
            raise AssertionError(f"{os.path.basename(dump)}: fread past the file at {pos} + {n}")
        em.uc.mem_write(eax, payload[pos:pos + n])
        pos += n
        return ebx

    e.stub(rt(0x758AD), fread)
    for a, pop in STUBS:
        e.stub(rt(a), (lambda em, *_: 1) if a == 0x278A0 else (lambda em, *_: 0), pop)
    mouse = []
    e.stub(rt(0x681D0), lambda em, eax, edx, *_: (mouse.append((eax, edx)), 0)[1], 0)
    music = []
    e.stub(rt(0x5C5EC), lambda em, eax, edx, *_: (music.append((eax, edx)), 0)[1], 0)
    e.call(rt(0x2C898), 1, max_steps=20_000_000)
    at = next((off for dest, off, n in reads if dest == rt(0x9635E)), None)
    theme = struct.unpack_from("<h", payload, at)[0] if at is not None else None
    POST.update(theme=theme, got=(e.r8(rt(0x284F13)), struct.unpack("<h", bytes(e.uc.mem_read(rt(0x9635E), 2)))[0],
                                  music[0][0] if music else -1, len(music)))
    if mouse != [(230, 110)]:                     # quicksave.rs RESTORE_MOUSE; smoke window_quick_mouse tests the port
        raise AssertionError(f"{os.path.basename(dump)}: MouseSetPos calls {mouse}")
    if pos != FIXED or len(reads) != READS:
        raise AssertionError(f"{os.path.basename(dump)}: QuickRestore read {pos} bytes in {len(reads)} calls")
    return [(dest, off, bytes(e.uc.mem_read(dest, n))) for dest, off, n in reads], {rt(a) for a in SKIP_IMAGE}, rt


def port(payload, fill, cap=2_000_000):
    out = ctypes.create_string_buffer(cap)
    n = PORT.ja1f_quick_rewrite(ISO, EXE, DAY3, payload, len(payload), fill, out, cap)
    if n < 0:
        raise AssertionError(f"ja1f_quick_rewrite returned {n}")
    return out.raw[:n]


G_SOLDIERS = 0xFA958
FIELD_FLOOR = 150                 # the fields with an address that the read reaches


def field_skip(addr, i):
    """the function's own changes after the read, and the soldiers' +0x90 pointer"""
    if addr == G_SOLDIERS and 0x90 <= i % 512 < 0x94:
        return True
    return addr + i in SKIP_IMAGE


def ptr(i, q):
    """a soldier's +0x90 pointer word in the file (skipped as in quicksave_diff)"""
    return (i >= 2 and 0x90 <= (i - 2) % 512 < 0x94 and q[2 + (i - 2) // 512 * 512 + 0x182] != 0
            and q[2 + (i - 2) // 512 * 512 + 0x138] != 0)


def noised(payload):
    """the payload with the world blocks and the options full of noise; neighbours differ"""
    q = bytearray(payload)
    rng = random.Random(len(payload))
    prev = -1
    for off in list(range(0x0A0B2, 0x0A0B2 + 854)) + list(range(0x0A408, 0x0A408 + 20)):
        if off - 0x0A0B2 in NOISE_KEEP:
            prev = q[off]
            continue
        v = rng.randrange(1, 255)
        while v == prev:
            v = rng.randrange(1, 255)
        q[off] = v
        prev = v
    for off in range(0xA497, 0xE23A):             # blocks 10..141, the scalars after the options
        if off in NOISE_FIXED:
            prev = q[off]
            continue
        v = rng.randrange(1, 255)
        while v == prev:
            v = rng.randrange(1, 255)
        q[off] = v
        prev = v
    q[0:2] = q[0x0A0B2 + 4:0x0A0B2 + 6]           # the first read (g_day) lies under the later world block
    return bytes(q)


bad = 0
for pay_name, dump_name, noise in [(p, d, n) for p, d in PAIRS for n in (False, True)]:
    dump = os.path.join(ROOT, dump_name)
    payload = open(os.path.join(ROOT, pay_name), "rb").read()
    if noise:
        payload = noised(payload)
    name = f"{os.path.basename(pay_name)}/{os.path.basename(dump_name)}{' noise' if noise else ''}"
    try:
        segments, skipped, rt = original(dump, payload)
    except AssertionError as err:
        print("FAIL", err)
        bad += 1
        continue
    except Exception as err:                      # noqa: BLE001 - an EmuStop is a failed case too
        print("FAIL", name, err)
        bad += 1
        continue
    want = (ctypes.c_int32 * 3)()
    PORT.ja1f_quick_after(POST["theme"] if POST["theme"] is not None else -999, want)
    got = POST["got"]
    if POST["theme"] is None or got[:3] != tuple(want) or got[3] > 1:
        print(f"FAIL {name}: after the read (file theme {POST['theme']}): original over_map / theme / SetMusic "
              f"{got[:3]} ({got[3]} calls), port {tuple(want)}")
        bad += 1
    a, b = port(payload, 0xAA), port(payload, 0x55)
    if len(a) != len(payload) or len(a) < FIXED:
        print(f"FAIL {name}: the port wrote {len(a)} bytes, the file has {len(payload)}")
        bad += 1
        continue
    compared, diffs, bad_read = 0, [], []
    lost = [i for i in MUST_MODEL if a[i] != b[i]]
    if lost:
        print(f"FAIL {name}: {len(lost)} option / sector-order bytes are not restored (they keep the old value); "
              f"first {[hex(i) for i in lost[:8]]}")
        bad += 1
    last = {dest + i: off + i for dest, off, mem in segments for i in range(len(mem))}
    for dest, off, mem in segments:
        for i in range(len(mem)):
            if dest + i in skipped:               # the function's own change after the read
                continue
            if last[dest + i] != off + i:         # a later read fills this byte again (0x284F24: blocks 57 and 66)
                continue
            if mem[i] != payload[off + i]:        # the read did not give the file's byte back
                bad_read.append((off + i, mem[i], payload[off + i]))
            if noise and off + i in NOISE_FIXED:  # fixed_state follows the noised view size; the plain run compares it
                continue
            if a[off + i] != b[off + i] or ptr(off + i, payload):
                continue                          # the port does not model this byte (or it is a pointer)
            compared += 1
            if a[off + i] != mem[i]:
                diffs.append((off + i, a[off + i], mem[i]))
    if bad_read:
        print(f"FAIL {name}: {len(bad_read)} bytes of the file did not come back from the read; first " +
              " ".join(f"@{i:#x}:{x:02x}/{y:02x}" for i, x, y in bad_read[:10]))
        bad += 1
    if diffs:
        print(f"FAIL {name}: {len(diffs)} of {compared} modelled bytes differ from the original; first " +
              " ".join(f"@{i:#x}:{x:02x}/{y:02x}" for i, x, y in diffs[:10]))
        bad += 1
    raw = ctypes.create_string_buffer(4_000_000)
    n = PORT.ja1f_quick_fields(ISO, EXE, DAY3, payload, len(payload), raw, len(raw))
    if n < 0:
        print(f"FAIL {name}: ja1f_quick_fields returned {n}")
        bad += 1
        continue
    # The volume numbers are outside the world (sound.rs): the restore must store the file's numbers.
    vols = (ctypes.c_int32 * 2)()
    PORT.ja1f_sound_volumes(vols)
    mem_vol = {dest: int.from_bytes(mem, "little", signed=True) for dest, _, mem in segments if len(mem) == 4}
    want = (mem_vol[rt(0x963A0)], mem_vol[rt(0x963A8)])
    if tuple(vols) != want:
        print(f"FAIL {name}: volumes after the restore: port {tuple(vols)}, original {want}")
        bad += 1
    fields = world_fields.parse(raw.raw[:n])
    nf, nb, fdiffs, unmapped = world_fields.compare(fields, segments, rt, field_skip)
    if nf < FIELD_FLOOR or fdiffs:
        by = sorted({d[0] for d in fdiffs})
        print(f"FAIL {name}: {nf} fields, {len(fdiffs)} bytes differ in {by[:40]}; first {fdiffs[:6]}")
        bad += 1
    if not bad_read and not diffs:
        print(f"{name}: {len(segments)} reads, {FIXED} bytes, {compared} compared, 0 differ; "
              f"{nf} fields ({nb} bytes) at their addresses; no address: {' '.join(unmapped)}")

print(f"CASES quickrestore={2 * len(PAIRS)}")
print(f"quickrestore_diff: {2 * len(PAIRS)} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
