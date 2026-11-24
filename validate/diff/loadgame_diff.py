"""Differential test: LoadGame 0x4E964 (a day save -> the globals) vs the port's day-save reader.

The original runs whole in Unicorn on a dump with a real day save fed through fread_ 0x758AD. OpenCDFile
0x38304 picks the file by the name it receives: the day save (LoadGame's path argument) or DAT/JAGGED.DAT
(ResetSectorsFromDat 0x3712C, which runs real at the end of LoadGame). fseek_ 0x757B1 moves the read position,
fclose_ 0x75A77 does nothing. The display and stash steps are stubs (each is its own diff test): ShowMessage_
0x27B98 (pop 16), InitTempFiles 0x38038, CopySaveChunks_ 0x4E840, ClearItemStash_ 0x515DC, LoadItemStash
0x51508 and the face loads FUN_2884c 0x2884C.

LoadGame reads at file offset 0x1C (after the day/save-name header): 8 x 0x200 soldiers to g_soldiers 0xFA958,
0x3C x 0x5A profiles to 0xEBB40, 9 x 0x3C to g_sectorTbl9 0x22979C, 0xB1 x 0x3C sectors to g_sectors 0x222B74,
0x356 to g_mercDeathsTotal 0x229FA0, 0x14 options to g_options 0x1163B8, 0x3D to g_sectorOrder 0x92DF0, 0x1E
to g_notesFound 0x92ED2. Then the three profile fixes, ResetSectorsFromDat and the restore of the two option
bytes g_options[0] / g_options[1] that it kept on entry (savegame.md "Sound FX and Music at a load"). Each case
sets those two bytes before the call (the four on/off pairs in turn), and the port gets the same pair as the
running program's switches (ja1f_loadgame `keep`, ctlpanel::put_sound_switches): both must come out equal.

The port side is ja1f_loadgame (World::from_parts + World::save_bytes_fill): the same save length, the same
fixed part (SAVE_FIXED_END 21649). A byte that comes out the same under two fills is a byte the port holds in
a field; it must equal the original's memory after the call. Bytes the original left in memory but the port
keeps only as the raw file copies are not compared (the fill filter).

Cases: real day saves (build/*.SAV) on three dumps, the three profile-fix triggers, and deterministic fuzz of
the read ranges (the sector rows' tmp-file selector byte 169 is left alone: a set value needs a sector file
in the tail, and the load would rightly refuse a save without one).

The round trip cannot see a field in the wrong place (the writer undoes the reader; #39). So the test also reads the
port's world field by field (ja1f_day_fields, quicksave::field_table) and compares each field with the original's
memory at that field's own address (world_fields.py). This part is what catches a swap of two fields.

usage: .venv/bin/python validate/diff/loadgame_diff.py [SAVE DUMP ...]   (pairs; default set below)
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from common import cd_read           # noqa: E402
from emu import Emu                 # noqa: E402
import world_fields                  # noqa: E402

ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog"), os.path.join(ROOT, "build", "game113b", "JA.EXE")
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
JAGGED = cd_read("DAT/JAGGED.DAT")

LOAD, OPEN, FSEEK, FREAD, FCLOSE = 0x4E964, 0x38304, 0x757B1, 0x758AD, 0x75A77
G_DATFILE = 0x92F54
# (global, bytes); LoadGame's freads in order, starting at file offset 0x1C
BLOCKS = [(0xFA958, 4096), (0xEBB40, 5400), (0x22979C, 540), (0x222B74, 10620),
          (0x229FA0, 854), (0x1163B8, 20), (0x92DF0, 61), (0x92ED2, 30)]
START, FIXED = 0x1C, 21649
# the two option bytes LoadGame keeps from before the load (g_options[0] and [1]): compared against `keep`
G_OPTIONS = 0x1163B8
OPT_AT = 21538
STUBS = ((0x27B98, 16), (0x38038, 0), (0x4E840, 0), (0x515DC, 0), (0x51508, 0), (0x2884C, 0))
DUMPS = ["build/mem_tac60.bin", "build/mem_combat1.bin", "build/mem_death1.bin"]
REAL = [("build/DAY_day3.SAV", 0), ("build/DAY_fight48.SAV", 0), ("build/day_loop.SAV", 1),
        ("build/eod_port.SAV", 2), ("build/DAY_before_restart.SAV", 0)]
# profile-fix triggers: profile 41 +0x3A = 99, profile 24 +0x39 = 2, profile 26 +0x39 = 101
FIX_OFFS = [4124 + 41 * 90 + 0x3A, 4124 + 24 * 90 + 0x39, 4124 + 26 * 90 + 0x39]
FUZZ, SEED = 12, 20261101

PORT = ctypes.CDLL(LIB)
PORT.ja1f_loadgame.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_size_t,
                               ctypes.c_int32, ctypes.c_int32, ctypes.c_void_p, ctypes.c_size_t]
PORT.ja1f_loadgame.restype = ctypes.c_int32
PORT.ja1f_day_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p,
                                 ctypes.c_size_t]
PORT.ja1f_day_fields.restype = ctypes.c_int32
ISO_B, EXE_B = ISO.encode(), EXE.encode()


def original(dump, payload, pre):
    """-> [(dest, file offset, memory bytes after the call)] for the save's 8 reads."""
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    cur = ["save"]
    pos = {"save": 0, "jagged": 0}
    reads = []

    def open_(em, eax, edx, ebx, ecx):
        name = bytes(em.uc.mem_read(eax, 64)).split(b"\0")[0].upper()
        cur[0] = "jagged" if b"JAGGED" in name else "save"
        em.w32(rt(G_DATFILE), 1)                 # the file handle the reads use
        return 1

    def fseek(em, eax, edx, ebx, ecx):
        if ebx == 0:
            pos[cur[0]] = edx
        return 0

    def fread(em, eax, edx, ebx, ecx):
        n = edx * ebx
        src = payload if cur[0] == "save" else JAGGED
        at = pos[cur[0]]
        if at + n > len(src):
            raise AssertionError(f"fread past {cur[0]} at {at} + {n}")
        em.uc.mem_write(eax, src[at:at + n])
        if cur[0] == "save":
            reads.append((eax, at, n))
        pos[cur[0]] = at + n
        return ebx

    e.stub(rt(FREAD), fread)
    e.stub(rt(OPEN), open_)
    e.stub(rt(FSEEK), fseek)
    e.stub(rt(FCLOSE), lambda em, *_: 0)
    for a, pop in STUBS:
        e.stub(rt(a), lambda em, *_: 0, pop)
    e.uc.mem_write(rt(G_OPTIONS), bytes(pre))    # the running program's Sound FX and Music
    path = e.alloc(b"C:\\JA\\DAY.SAV\0")
    e.call(rt(LOAD), path, 1, max_steps=20_000_000)
    if len(reads) != len(BLOCKS) or [n for _, _, n in reads] != [n for _, n in BLOCKS]:
        raise AssertionError(f"{os.path.basename(dump)}: reads {[(a, n) for a, _, n in reads]}")
    if reads[0][1] != START or sum(n for _, _, n in reads) != FIXED - START:
        raise AssertionError(f"{os.path.basename(dump)}: the reads start at {reads[0][1]}")
    return [(dest, off, bytes(e.uc.mem_read(dest, n))) for dest, off, n in reads], rt


def port(payload, fill, pre, cap=1 << 20):
    out = ctypes.create_string_buffer(cap)
    n = PORT.ja1f_loadgame(ISO_B, EXE_B, payload, len(payload), fill, pre[0] | pre[1] << 8, out, cap)
    if n < 0:
        raise AssertionError(f"ja1f_loadgame returned {n}")
    return out.raw[:n]


G_SOLDIERS = 0xFA958
FIELD_FLOOR = 20                  # the fields with an address that the read reaches


def field_skip(addr, i):
    """the soldiers' +0x90 pointer"""
    return addr == G_SOLDIERS and 0x90 <= i % 512 < 0x94


def cases():
    """(name, payload, dump)"""
    out = []
    for save, k in REAL:
        out.append((os.path.basename(save), open(os.path.join(ROOT, save), "rb").read(),
                    os.path.join(ROOT, DUMPS[k])))
    base = out[1][1]
    for fix, off in enumerate(FIX_OFFS):
        buf = bytearray(base)
        buf[off] = (99, 2, 101)[fix]
        out.append((f"fix{fix + 1}", bytes(buf), os.path.join(ROOT, DUMPS[0])))
    rng = random.Random(SEED)
    sel = {10064 + i * 177 + 169 for i in range(60)}     # the tmp-file selector of each sector row
    allowed = [i for i in range(START, FIXED) if i not in sel]
    for k in range(FUZZ):
        buf = bytearray(out[k % len(REAL)][1])
        for at in rng.sample(allowed, 40):
            buf[at] = rng.randrange(256)
        out.append((f"fuzz{k + 1}", bytes(buf), os.path.join(ROOT, DUMPS[k % len(DUMPS)])))
    # A real save holds many equal neighbours and 40 fuzzed bytes rarely touch one field: a field read one byte off
    # still gets the right value. Here the world block and the options hold noise in which neighbours differ (not
    # the tile set and the sector, which the port's reader needs).
    for k in range(len(DUMPS)):
        buf = bytearray(out[k % len(REAL)][1])
        prev = -1
        for at in list(range(20684, 20684 + 854)) + list(range(21538, 21538 + 20)):
            if at in (20836, 20837):
                prev = buf[at]
                continue
            v = rng.randrange(1, 255)
            while v == prev:
                v = rng.randrange(1, 255)
            buf[at] = prev = v
        out.append((f"noise{k + 1}", bytes(buf), os.path.join(ROOT, DUMPS[k])))
    return out


def main():
    pairs = list(zip(sys.argv[1::2], sys.argv[2::2]))
    todo = [(os.path.basename(a), open(a, "rb").read(), b) for a, b in pairs] if pairs else cases()
    bad = 0
    kept = 0
    for k, (name, payload, dump) in enumerate(todo):
        # all four pairs in turn: off/off, on/off, off/on, on/on (a real save holds on/on mostly)
        pre = [k & 1, (k >> 1) & 1]
        dname = f"{name}/{os.path.basename(dump)}"
        try:
            segments, rt = original(dump, payload, pre)
        except AssertionError as err:
            print("FAIL", err)
            bad += 1
            continue
        except Exception as err:                  # noqa: BLE001 - an EmuStop is a failed case too
            print("FAIL", dname, err)
            bad += 1
            continue
        try:
            a, b = port(payload, 0xAA, pre), port(payload, 0x55, pre)
        except AssertionError as err:
            print("FAIL", dname, err)
            bad += 1
            continue
        compared, diffs = 0, []
        for dest, off, mem in segments:
            for i in range(len(mem)):
                at = off + i
                if a[at] != b[at]:  # not modelled
                    continue
                compared += 1
                if at in (OPT_AT, OPT_AT + 1):
                    kept += 1
                    if mem[i] != pre[at - OPT_AT]:
                        diffs.append((at, pre[at - OPT_AT], mem[i]))   # the original did not keep it
                if a[at] != mem[i]:
                    diffs.append((at, a[at], mem[i]))
        raw = ctypes.create_string_buffer(4_000_000)
        n = PORT.ja1f_day_fields(ISO_B, EXE_B, payload, len(payload), raw, len(raw))
        if n < 0:
            print(f"FAIL {dname}: ja1f_day_fields returned {n}")
            bad += 1
            continue
        nf, nb, fdiffs, unmapped = world_fields.compare(world_fields.parse(raw.raw[:n]), segments, rt, field_skip)
        if nf < FIELD_FLOOR or fdiffs:
            by = sorted({d[0] for d in fdiffs})
            print(f"FAIL {dname}: {nf} fields, {len(fdiffs)} bytes differ in {by[:40]}; first {fdiffs[:6]}")
            bad += 1
        last = f"{nf} fields ({nb} bytes) at their addresses; no address: {' '.join(unmapped)}"
        if diffs:
            print(f"FAIL {dname}: {len(diffs)} of {compared} modelled bytes differ from the original; first " +
                  " ".join(f"@{i:#x}:{x:02x}/{y:02x}" for i, x, y in diffs[:10]))
            bad += 1
        else:
            print(f"{dname}: {len(segments)} reads, {FIXED} bytes, {compared} compared, 0 differ; {last}")
    if kept != 2 * len(todo):
        print(f"FAIL kept option bytes compared {kept} times, want {2 * len(todo)}")
        bad += 1
    print(f"CASES loadgame={len(todo)}")
    print(f"loadgame_diff: {len(todo)} cases, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
