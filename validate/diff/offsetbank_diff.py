#!/usr/bin/env python3
"""Differential test: the picture bank loader LoadOffsetBank 0x52E00 + OpenTilesDat 0x36EA0, original vs port.

usage: .venv/bin/python validate/diff/offsetbank_diff.py [ncases=600] [--lib SO]
The original runs in Unicorn with the real FileSize 0x5273C and the real ReadWholeFile 0x52768. Stubs: FileExists 0x4707C,
open_, filelength_, close_, fopen_, fgetc_, fclose_ (they serve the case's bytes). The files of over 64 KB use a stub of
ReadWholeFile with the same rule: one Python call per byte is too slow for them, and the small files run the real one.
The port: offsetbank::load, open_tiles, tiles_name (ja1f_offset_bank, ja1f_tiles_name). Tileset::load, sinv, InvData::load
and SoldierBank call these.
Files: every real bank (SH, SINV, INV, TILES0..5 at their real buffers and sizes) and random banks: counts 0, negative and
in the table, offsets that wrap at 32 bits, file short of max (-1), file over max (-2), no file, tile sets 0..255, tile files
at the 0x40000 limit and over it.
Compared, case by case: the result; on success the buffer without its last 4 bytes (table of pointers and data); the tile
file name (case and slash folded).
On the original side only: the path order (the local file first, then drive + ":\\" + name), the last 4 bytes come from
just after the buffer, the bytes after the buffer stay, a failed load leaves the partial read, -2 leaves the buffer as it
was, INV.DAT byte 0x44C4 is the byte of record 255 that screen.rs reads.
Not compared: a file under 4 bytes, a count past the file end (no game file does this; offsetbank.rs lists them). Exits 1
on mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 600
PORT = ctypes.CDLL(LIB)
PORT.ja1f_offset_bank.argtypes = [ctypes.c_char_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_uint32, ctypes.c_void_p,
                                  ctypes.c_int]
PORT.ja1f_tiles_name.argtypes = [ctypes.c_ubyte, ctypes.c_void_p, ctypes.c_int]
LOADER, OPEN_TILES, READ_WHOLE, NAME_DIGIT = 0x52E00, 0x36EA0, 0x52768, 0x90819
TILE_BUF, SH_BUF, SINV_BUF, INV_BUF = 0xA8C48, 0x123B08, 0xA6C48, 0x1163CC
PATH = 0x2F0000 - 0x400                                     # a free spot for the name the test passes
RECORD_255_BYTE = 0xFA958 + 255 * 0x200 + 0x138
e = Emu(fast=True)
drive = bytes(e.uc.mem_read(0x92F8C, 1)).decode()
st = {}
BULK = 0x10000


def stack_path():
    return e.cstr(e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)).decode()


def on_exists(em, a, *_):
    # A file on the CD: its full name exists (OpenTilesDat tests a name, the loader tests the name it built).
    p = e.cstr(a).decode()
    st["log"].append(("exists", p))
    return 1 if st["file"] is not None and (st["local"] or p.startswith(drive + ":")) else 0


def on_open(em, *_):
    st["log"].append(("open", stack_path()))
    return 7 if st["file"] is not None else 0xFFFFFFFF


def on_length(em, a, *_):
    assert a == 7
    return len(st["file"])


def on_fopen(em, a, *_):
    st["log"].append(("fopen", e.cstr(a).decode()))
    st["pos"] = 0
    return 9 if st["file"] is not None else 0


def on_fgetc(em, a, *_):
    assert a == 9
    if st["pos"] >= len(st["file"]):
        return 0xFFFFFFFF
    st["pos"] += 1
    return st["file"][st["pos"] - 1]


def bulk_read(em, a, d, b, *_):
    # ReadWholeFile's rule (the real one runs on the small files).
    st["log"].append(("fopen", e.cstr(a).decode()))
    if st["file"] is None:
        return 0xFFFFFFFF
    got = st["file"][:b]
    e.uc.mem_write(d, bytes(got))
    return 0xFFFFFFFF if len(got) < b else 0


e.stub(0x4707C, on_exists)
e.stub(0x76693, on_open)
e.stub(0x768E1, on_length)
e.stub(0x7691E, lambda em, *_: 0)
e.stub(0x75EA0, on_fopen)
e.stub(0x7648C, on_fgetc)
e.stub(0x75A77, lambda em, *_: 0)
r = random.Random(1103)


def rnd_bank(n, data_len):
    """count n (signed), n offsets, data"""
    table = max(n, 0)
    w = [r.randrange(data_len + 1) if r.randrange(8) else r.randrange(1 << 32) for _ in range(table)]
    return struct.pack("<i", n) + struct.pack(f"<{table}I", *w) + r.randbytes(data_len)


def port_result(file, max_, tiles, base):
    out = ctypes.create_string_buffer(0x101000)
    n = -1 if file is None else len(file)
    got = PORT.ja1f_offset_bank(None if file is None else bytes(file), n, max_, tiles, base, out, len(out))
    return got, out


bad = cases = 0
seen = {"ok": 0, "short": 0, "big": 0, "missing": 0, "real_read": 0, "nocount": 0, "tiles": 0, "limit": 0}
NOISE = bytes(r.randrange(256) for _ in range(8))


def report(tag, msg):
    global bad
    bad += 1
    if bad <= 20:
        print(f"{tag}: {msg}")


def run(tag, file, max_, buf, local, tiles=None, name=b"DAT\\TEST.DAT"):
    """one case; tiles = the tile set number for OpenTilesDat, else LoadOffsetBank with name"""
    global cases
    size = max_ if tiles is None else (len(file) if file is not None and len(file) <= 0x40000 else 0)
    span = max(size, len(file) if file is not None and len(file) <= 0x100000 else 0) + 16
    before = (NOISE * (span // 8 + 1))[:span]
    e.uc.mem_write(buf, before)
    st.update(log=[], file=file, local=local, pos=0)
    big = file is not None and len(file) > BULK
    if big:
        e.stub(READ_WHOLE, bulk_read)
    else:
        e.stubs.pop(READ_WHOLE, None)
        seen["real_read"] += 1
    try:
        if tiles is None:
            e.uc.mem_write(PATH, name + b"\0")
            got = e.call(LOADER, PATH, buf, max_, max_steps=8_000_000)
        else:
            got = e.call(OPEN_TILES, tiles, max_steps=8_000_000)
    except EmuStop as x:
        report(tag, f"original stopped: {x}")
        return
    cases += 1
    got = got - (1 << 32) if got >= 1 << 31 else got
    want, out = port_result(file, max_, 0 if tiles is None else 1, buf)
    if got != want:
        report(tag, f"result original {got} port {want} (len {None if file is None else len(file)} max {max_})")
        return
    now = bytes(e.uc.mem_read(buf, span))
    if tiles is not None:
        digit = e.r8(NAME_DIGIT)
        if digit != 0x30 + tiles % 10:
            report(tag, f"digit {digit:#x} for tile set {tiles}")
        nb = ctypes.create_string_buffer(32)
        PORT.ja1f_tiles_name(tiles, nb, 32)
        name = nb.value.replace(b"/", b"\\").upper()
        if e.cstr(0x90810).upper() != name:
            report(tag, f"name {e.cstr(0x90810)} port {name}")
    # The original side: the path order.
    plain = st["log"][0][1]
    use = plain if local and file is not None else f"{drive}:\\{plain}"
    if file is not None and any(x[1] != use for x in st["log"] if x[0] != "exists"):
        report(tag, f"paths {st['log']} (wanted {use})")
    if got == 0:
        seen["ok"] += 1
        m = len(file)
        if now[:m - 4] != bytes(out[:m - 4]):
            at = next(i for i in range(m - 4) if now[i] != out[i][0])
            report(tag, f"buffer differs at {at:#x}: original {now[at]:#x} port {out[at][0]:#x}")
        if now[m - 4:m] != before[m:m + 4] or now[m:] != before[m:]:
            report(tag, "the buffer tail is not the old bytes after the buffer")
        if int.from_bytes(file[:4], "little", signed=True) <= 0:
            seen["nocount"] += 1
    elif got == -2:
        seen["big"] += 1
        if now != before:
            report(tag, "-2 changed the buffer")
    elif file is None:
        seen["missing"] += 1
        if now != before:
            report(tag, "no file changed the buffer")
    elif len(file) <= 0x40000 or tiles is None:
        seen["short"] += 1
        if tiles is None and (now[:len(file)] != bytes(file) or now[len(file):] != before[len(file):]):
            report(tag, "a short read did not leave the partial read")


# Every real bank at its own buffer and size.
for nm, buf, mx in (("SH", SH_BUF, 0xFF044), ("SINV", SINV_BUF, 0x1E98), ("INV", INV_BUF, 0xD73C)):
    f = game_read(f"DAT\\{nm}.DAT")
    run(nm, f, mx, buf, True, name=f"DAT\\{nm}.DAT".encode())
    if nm == "INV" and e.r8(RECORD_255_BYTE) != 0xDF:
        report(nm, f"record 255's byte = {e.r8(RECORD_255_BYTE):#x}, wanted 0xDF")
for t in range(6):
    run(f"TILES{t}", game_read(f"DAT\\TILES{t}.DAT"), 0, TILE_BUF, t % 2 == 0, tiles=t)
    seen["tiles"] += 1

for c in range(ncases):
    k = c % 8
    n = r.choice((0, -1, -r.randrange(1, 1 << 30), r.randrange(1, 40), r.randrange(1, 300)))
    f = rnd_bank(n, r.randrange(4, 1200))
    local = r.randrange(2) == 0
    if k <= 2:
        run(f"case {c} exact", f, len(f), TILE_BUF, local)
    elif k == 3:
        run(f"case {c} short", f, len(f) + r.randrange(1, 300), TILE_BUF, local)
    elif k == 4:
        run(f"case {c} big", f, r.randrange(4, len(f)), TILE_BUF, local)
    elif k == 5:
        run(f"case {c} none", None, r.randrange(4, 4000), TILE_BUF, local)
    elif k == 6:
        t = r.randrange(256)
        run(f"case {c} tiles {t}", f, 0, TILE_BUF, local, tiles=t)
        seen["tiles"] += 1
    else:
        t = r.randrange(256)
        if c % 40 == 7:
            f2 = None
        elif c % 40 == 15:
            # The size limit: 0x40000 loads, one byte more fails, both as bulk reads.
            f2 = f + bytes(0x40000 - len(f))
            seen["limit"] += 1
        elif c % 40 == 23:
            f2 = f + bytes(0x40001 - len(f))
            seen["limit"] += 1
        else:
            f2 = f
        run(f"case {c} tiles {t}", f2, 0, TILE_BUF, local, tiles=t)
        seen["tiles"] += 1
print("offsetbank_diff:", " ".join(f"{a}={b}" for a, b in seen.items()))
for a, b in seen.items():
    if not b:
        report("coverage", f"no case of kind {a}")
print(f"offsetbank_diff: {cases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
