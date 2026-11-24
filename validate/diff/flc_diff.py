#!/usr/bin/env python3
"""flc_diff.py [--port] [--lib SO] [--seed S] [--only NAME] — the FLC film decoder of the original vs the port.

Every film of the game (141: the door films F_/I_/S_ DR/PD KEY/PCK/HAN of the tileset archives DAT\\0..9.DAT, the
title films of DAT\\TITLE.DAT, the end films of DAT\\ENDGRAPH.DAT), each frame and then the ring frame.
The original: the frame reader 0x5B990 (FlcPlay 0x5BC58 calls it once per frame) in Unicorn on the plain image, with
its whole chunk path unstubbed (0x5B8D0 read, 0x5B810 chunk loop, COLOR 0x5B74C + DAC writers 0x5B248 / 0x5B294,
BYTE_RUN 0x5B2D0, DELTA_FLI 0x5B3A4, DELTA_FLC 0x5B470, BLACK 0x5B628, FLI_COPY 0x5B6E4, clip 0x5B094 + line
writers 0x5B0FC / 0x5B16C / 0x5B1C4). The player record is set up as PlayFilm 0x5BFF8 does (128-byte FLC header,
+0x88 x, +0x8C y, +0x9C screen 320 x 200, +0xA8 = the film flag: colours only with it); FlcOpen 0x5BB30 runs first
(open callback 0x5BA24 and seek callback 0x5BA74 stubbed: the member starts at 0) and reads the header and seeks to
the first frame; fread_ 0x758AD reads from the film bytes. --port also compares FlcOpen's speed (+0x10) and first
frame place with ja1f_flc_head (Flc::new). The screen and
the DAC (ports 0x3C8 / 0x3C9, 6-bit values) start random. Positions: the title / end films at their real places
(0, 0) / INTRO05 (0, 15) with flag 1; door films at random places that reach past the right and bottom edges, flag 0
(as PlayDoorFx 0x3B03C) or 1.
Without --port the screen after each frame is checked only for the return value 0 (each frame read).
--port: ja1f_flc (flc.rs Flc::next_frame + show + set_dac) on the same bytes; screen + DAC compared per frame.
No game film has a BLACK chunk (0x5B628), so after the game films (not with --only) come 8 real films with one spliced in
and 40 made films of fuzzed size, odd widths too, at places that also cross the left and top edges. No game film has
a DELTA_FLI chunk at a place with y != 0: the made films hold such chunks.
Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import (UC_X86_REG_GDTR, UC_X86_REG_CS, UC_X86_REG_SS, UC_X86_REG_DS,  # noqa: E402
                               UC_X86_REG_ES, UC_X86_REG_GS)
from common import cd_read           # noqa: E402
import datarc                        # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 1
only = argv[argv.index("--only") + 1].upper() if "--only" in argv else None
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_flc.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
                              ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_flc.restype = ctypes.c_int32
    PORT.ja1f_flc_head.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_flc_head.restype = ctypes.c_int32
READ_FRAME, FREAD, BUF_PTR = 0x5B990, 0x758AD, 0x93E64
# FlcOpen 0x5BB30 and the callbacks PlayFilm 0x5BFF8 puts in the record: +0xB5 = 0x5BA24 (ArcSeek to the member),
# +0xBD = 0x5BA74 (fseek to member + EDX)
FLC_OPEN, CB_OPEN, CB_SEEK = 0x5BB30, 0x5BA24, 0x5BA74
PLACES = {"SIRTECH.FLC": (0, 0), "TITLE1.FLC": (0, 0), "TITLE2.FLC": (0, 0), "INTRO05.FLC": (0, 15),
          "HELILIFT.FLC": (0, 0), "HELILEAV.FLC": (0, 0)}


class DacEmu(Emu):
    """OUT 0x3C8 = DAC write index, OUT 0x3C9 = the next 6-bit component (VGA)."""
    def _io(self, uc, port, size, value, *rest):
        if not hasattr(self, "dac"):
            self.dac, self.dac_i = bytearray(1024), 0
        if port == 0x3C8:
            self.dac_i = 4 * (value & 0xFF)
        elif port == 0x3C9:
            self.dac[self.dac_i] = value & 0x3F
            self.dac_i += 1
            if self.dac_i % 4 == 3:
                self.dac_i += 1
            self.dac_i %= 1024
        else:
            return super()._io(uc, port, size, value, *rest)


def films():
    names = subprocess.check_output(["bsdtar", "-tf", os.path.join(ROOT, "rom", "JA1.gog")]).decode().split("\n")
    for p in sorted(n for n in names if n.upper().endswith(".DAT")):
        d = cd_read(p)
        try:
            ms = datarc.members(d)
        except Exception:
            continue
        for n, o, s in ms:
            b = d[o:o + s]
            if len(b) > 128 and struct.unpack_from("<H", b, 4)[0] in (0xAF11, 0xAF12):
                if only is None or n.upper() == only:
                    yield p, n.upper(), b


def black_chunk():
    return struct.pack("<IH", 6, 13)


def splice_black(data, r):
    """`data` with a BLACK chunk at a random place in the chunk list of a random frame after the first."""
    pos, frames = struct.unpack_from("<I", data, 80)[0], []
    while pos + 16 <= len(data) and struct.unpack_from("<H", data, pos + 4)[0] == 0xF1FA:
        frames.append(pos)
        pos += struct.unpack_from("<I", data, pos)[0]
    if len(frames) < 3:
        return None
    f = r.randrange(1, len(frames))
    fp = frames[f]
    size, _, nch = struct.unpack_from("<IHH", data, fp)
    q = fp + 16
    for _ in range(r.randrange(nch + 1)):
        q += struct.unpack_from("<I", data, q)[0]
    out = bytearray(data[:q] + black_chunk() + data[q:])
    struct.pack_into("<I", out, fp, size + 6)
    struct.pack_into("<H", out, fp + 6, nch + 1)
    struct.pack_into("<I", out, 0, len(out))
    if struct.unpack_from("<I", out, 84)[0] > fp:     # the second frame's place
        struct.pack_into("<I", out, 84, struct.unpack_from("<I", out, 84)[0] + 6)
    return bytes(out)


def delta_fli_chunk(r, w, h):
    """A DELTA_FLI chunk (0x5B3A4): first row, row count, per row packets (skip, count i8, bytes or one byte)."""
    y0, rows = r.randrange(h), r.randrange(1, min(h, 5) + 1)
    rows = min(rows, h - y0)
    body = bytearray(struct.pack("<hh", y0, rows))
    for _ in range(rows):
        x, packets = 0, bytearray()
        count = 0
        while x < w and count < 3:
            skip, n = r.randrange(0, 5), r.randrange(1, 9)
            if x + skip + n > w:
                break
            packets.append(skip)
            if r.random() < .5:
                packets += bytes([n]) + bytes(r.randrange(1, 256) for _ in range(n))
            else:
                packets += bytes([256 - n, r.randrange(1, 256)])
            x += skip + n
            count += 1
        body += bytes([count]) + packets
    body += bytes(len(body) & 1)
    return struct.pack("<IH", 6 + len(body), 12) + bytes(body)


def color_chunk(r):
    """A COLOR chunk (0x5B74C), type 11 (6-bit) or 4 (8-bit), with 2..5 packets of (skip, count (0 = 256), RGB x count);
    the colour index wraps past 255. No game film has more than one packet in a COLOR chunk."""
    kind = r.choice([4, 11])
    n = r.randrange(2, 6)
    body = bytearray(struct.pack("<H", n))
    for _ in range(n):
        skip, count = r.randrange(0, 120), r.choice([r.randrange(1, 40), r.randrange(1, 256), 0])
        body += bytes([skip, count]) + bytes(r.randrange(64 if kind == 11 else 256) for _ in range(3 * (count or 256)))
    body += bytes(len(body) & 1)
    return struct.pack("<IH", 6 + len(body), kind) + bytes(body)


def made_film(r, rc=None):
    """A film of FLI_COPY, DELTA_FLI and BLACK chunks in a fuzzed order, size w x h, ring frame included; with `rc`
    (its own generator: the other chunks stay as they were) a frame may start with a COLOR chunk of many packets."""
    w, h = r.choice([1, 2, 3, 7, 16, 33, r.randrange(1, 321)]), r.choice([1, 2, 5, r.randrange(1, 201)])
    n = r.randrange(2, 5)
    out = bytearray(128)
    struct.pack_into("<IHHHHHHI", out, 0, 0, 0xAF12, n, w, h, 8, 0, 10)
    second = 0
    for k in range(n + 1):
        chunks = []
        for _ in range(r.randrange(1, 4)):
            kind = r.random()
            if kind < .3:
                chunks.append(black_chunk())
            elif kind < .6:
                chunks.append(delta_fli_chunk(r, w, h))
            else:
                body = bytes(r.randrange(1, 256) for _ in range(w * h))
                pad = len(body) & 1
                chunks.append(struct.pack("<IH", 6 + len(body) + pad, 16) + body + bytes(pad))
        if rc is not None and rc.random() < .5:
            chunks.insert(0, color_chunk(rc))
        body = b"".join(chunks)
        if k == 1:
            second = len(out)
        out += struct.pack("<IHH", 16 + len(body), 0xF1FA, len(chunks)) + bytes(8) + body
    struct.pack_into("<I", out, 0, len(out))
    struct.pack_into("<II", out, 80, 128, second)
    return bytes(out)


def black_films(r):
    real = [(a, n, d) for a, n, d in films() if n not in PLACES]
    r.shuffle(real)
    made = 0
    for a, n, d in real:
        d2 = splice_black(d, r)
        if d2:
            yield a, "SPLICE " + n, d2
            made += 1
        if made == 8:
            break
    rc = random.Random(5274)
    for k in range(40):
        yield "made", f"MADE{k}", made_film(r, rc)


def all_films(r):
    yield from films()
    if only is None:
        yield from black_films(r)


e = DacEmu(fast="--slow" not in argv)
SCREEN, REC = e.alloc(64000), e.alloc(0x100)
FRAMEBUF = e.alloc(0x40000)
e.w32(BUF_PTR, FRAMEBUF)
stream = {"data": b"", "pos": 0}


def fread(em, buf, size, n, fp):
    want = size * n
    d, p = stream["data"], stream["pos"]
    if p + want > len(d):
        return 0
    em.uc.mem_write(buf, d[p:p + want])
    stream["pos"] = p + want
    return n


e.stub(FREAD, fread)


def cb_open(em, rec, *_):
    stream["pos"] = 0
    return 0


def cb_seek(em, rec, off, *_):
    stream["pos"] = off
    return 0


e.stub(CB_OPEN, cb_open)
e.stub(CB_SEEK, cb_seek)
# The decoder reloads GS from DS before its gs: accesses. Unicorn starts with no descriptor table, and a segment load
# there leaves the stack 16-bit; so the harness gives it a flat GDT as DOS/4GW has (code 0x08, data 0x10, base 0,
# 4 GB, 32-bit) and loads the selectors before each call.
GDT = e.alloc(24)
e.uc.mem_write(GDT, struct.pack("<QQQ", 0, 0x00CF9A000000FFFF, 0x00CF92000000FFFF))
e.uc.reg_write(UC_X86_REG_GDTR, (0, GDT, 23, 0))
for reg, sel in ((UC_X86_REG_CS, 0x08), (UC_X86_REG_SS, 0x10), (UC_X86_REG_DS, 0x10), (UC_X86_REG_ES, 0x10),
                 (UC_X86_REG_GS, 0x10)):
    e.uc.reg_write(reg, sel)
r = random.Random(seed)
fails = nfilms = nframes = 0
for arc, name, data in all_films(r):
    nfilms += 1
    w, h = struct.unpack_from("<HH", data, 8)
    count = struct.unpack_from("<H", data, 6)[0] + 1
    if name in PLACES:
        (x, y), flag = PLACES[name], 1
    elif name.startswith(("MADE", "SPLICE")):
        x, y, flag = r.randrange(-w, 320), r.randrange(-h, 200), r.choice((0, 1))
    else:
        x, y, flag = r.randrange(0, 320 - w // 2), r.randrange(0, 200 - h // 2), r.choice((0, 0, 1))
    screen, dac = bytes(r.randrange(256) for _ in range(64000)), bytearray(r.randrange(64) for _ in range(1024))
    for i in range(3, 1024, 4):
        dac[i] = 0
    e.uc.mem_write(SCREEN, screen)
    e.uc.mem_write(FRAMEBUF, bytes(0x40000))
    e.dac = bytearray(dac)
    # FlcOpen (EDX = 1 pass) reads the header into the record and seeks to the first frame; then x, y
    rec = bytearray(0x100)
    struct.pack_into("<IIIIIIIB", rec, 0x80, 1, 0, 0, 0, 0, 0, 0, 0)
    struct.pack_into("<IIIB", rec, 0x9C, SCREEN, 320, 200, flag)
    struct.pack_into("<I", rec, 0xB5, CB_OPEN)
    struct.pack_into("<I", rec, 0xBD, CB_SEEK)
    e.uc.mem_write(REC, bytes(rec))
    stream["data"], stream["pos"] = data, 0
    res = e.call(FLC_OPEN, REC, 1)
    if res != 0:
        print(f"MISMATCH {arc} {name}: original FlcOpen result {res:#x}")
        fails += 1
        continue
    e.w32(REC + 0x88, x)
    e.w32(REC + 0x8C, y)
    got = None
    if PORT:
        head = (ctypes.c_int32 * 3)()
        hr = PORT.ja1f_flc_head(data, len(data), head)
        want = (e.r32(REC + 0x10), stream["pos"], e.r32(REC + 0x84))
        if hr != 0 or (head[0], head[1]) != want[:2] or want[2] != 1:
            print(f"MISMATCH {arc} {name}: FlcOpen speed / first frame / passes {want}, port {hr} {list(head)}")
            fails += 1
            continue
        out = ctypes.create_string_buffer(count * 65024)
        n = PORT.ja1f_flc(data, len(data), x, y, flag, screen, bytes(dac), out, count)
        got = out.raw
        if n != count:
            print(f"MISMATCH {arc} {name}: port frames {n} / {count}")
            fails += 1
            continue
    for k in range(count):
        res = e.call(READ_FRAME, REC)
        nframes += 1
        if res != 0:
            print(f"MISMATCH {arc} {name} frame {k}: original read result {res:#x}")
            fails += 1
            break
        if got is None:
            continue
        want_s, want_d = bytes(e.uc.mem_read(SCREEN, 64000)), bytes(e.dac)
        o = k * 65024
        gs, gd = got[o:o + 64000], got[o + 64000:o + 65024]
        if gs != want_s or gd != want_d:
            bad = [i for i in range(64000) if gs[i] != want_s[i]]
            badd = [i for i in range(1024) if gd[i] != want_d[i]]
            first = f"pixel ({bad[0] % 320}, {bad[0] // 320}) {gs[bad[0]]} / {want_s[bad[0]]}" if bad else ""
            print(f"MISMATCH {arc} {name} at ({x}, {y}) flag {flag} frame {k}/{count}: {len(bad)} pixels {first}, "
                  f"{len(badd)} DAC bytes" + (f" first {badd[0]} {gd[badd[0]]} / {want_d[badd[0]]}" if badd else ""))
            fails += 1
            break
print(f"flc_diff{' --port' if PORT else ''}: {nfilms} films, {nframes} frames, {fails} mismatches")
sys.exit(1 if fails or not nfilms else 0)
