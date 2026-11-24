"""Differential test: the map panel of the strategic screen (strategic.md "The map panel"), original vs port.

usage: .venv/bin/python validate/diff/mappanel_diff.py [ncases=300] [DUMP] [--port] [--lib SO] [--png]
Per case: fuzzed Day accounts (0x229FA0, 0xA8 bytes: small, large, negative values) and g_teamOnDuty. The original
runs in Unicorn on tac60: the screen *0x95678 cleared, MAIN.DAT MAPPAN.PCC (tools/pcx.py) at row 146, then
0x42020 or 0x42090 (raise the tab of map mode a), sometimes 0x468B0 or 0x42114 (lower it) + the raise of tab b, then
0x36010 (number fields 0..14), then number field 15 (0x35F64) and, in some cases, field 26 (the pressed wage
number) or 0x35E84 (field 1 with its box). The port (port/ja1front ja1f_map_panel + ja1f_map_field) draws the same;
the 320 x 200 screen is compared. Then ncases // 3 finance cases (strategic.md "The finance page": FINANCE.PCC +
0x36010 fields 16..25, the back button 0x35DA4 / 0x35C0C (8), the field 4 / 0 box 0x35F0C / 0x35E84, MAPFLIPn.PCC;
port ja1f_map_field 18..27) on a noise screen, and ncases // 3 processing cases (DrawProcessingUsage? 0x42148 with
fuzzed plants, sector owner bits and g_treesTapped; the map buffer built from MAPDARK / MAP.PCC; the screen is
taken at the wait for the button 0x67C54 and must be back to the noise after; port ja1f_processing), and ncases // 3
replay panel cases (0x36010 fields 31..42 with fuzzed day values; port ja1f_replay_fields).
Without --port only the original runs (a smoke test of the harness).
Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from datarc import read, member      # noqa: E402
from pcx import decode, png          # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_map_tab_switch.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] + [ctypes.c_int] * 3 + [ctypes.c_void_p]
    for f in (PORT.ja1f_map_panel, PORT.ja1f_map_field):
        f.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32,
                      ctypes.c_void_p]
        f.restype = ctypes.c_int32
    PORT.ja1f_processing.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 4
    PORT.ja1f_processing.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

ACC, ON_DUTY, SCREEN_P, EMPTY = 0x229FA0, 0x229FAC, 0x95678, 0x90A60
PLANTS, SECTORS, MAPBUF_P = 0x22A12A, 0x22979C, 0x93E5C
TABS = 0x93EDA

if not os.path.exists(dump):
    print(f"mappanel_diff: {dump} missing")
    sys.exit(1)
MAIN = read("rom/DAT/MAIN.DAT")
pw, ph, pan, pal = decode(member(MAIN, "MAPPAN.PCC"))
assert (pw, ph) == (320, 53)
fin = decode(member(MAIN, "FINANCE.PCC"))[2]
flips = [decode(member(MAIN, f"MAPFLIP{n}.PCC"))[2] for n in (1, 2, 3)]
mapbuf = bytearray(640 * 200)
for col, name in ((0, "MAPDARK.PCC"), (320, "MAP.PCC")):
    w_, h_, px, _ = decode(member(MAIN, name))
    for y in range(h_):
        mapbuf[y * 640 + col:y * 640 + col + w_] = px[y * w_:(y + 1) * w_]

e = Emu(dump=dump, fast=True)
for a in (0x67BF0, 0x673AC, 0x5C900, 0x5C8A0):   # mouse hide / show, sounds
    e.stub(e.rt(a), lambda *_: 0)
e.stub_sprintf()
held = []                                         # the screen while DrawProcessingUsage? waits for the button
e.stub(e.rt(0x67C54), lambda em, *_: held.append(bytes(em.uc.mem_read(screen, 64000))))
screen = e.r32(e.rt(SCREEN_P))
snap = bytes(e.uc.mem_read(0, 16 << 20))
orig_acc = bytes(e.uc.mem_read(e.rt(ACC), 0xA8))


def tab(m):
    a = e.rt(TABS + 8 * m)
    return struct.unpack("<hh", e.uc.mem_read(a, 4))


def val(r):
    return r.choice([0, 1, 5, 9, 10, 42, 99, 100, 999, 1000, 12345, 99999, 999999, 1000000, 1234567, -1, -5, -99,
                     -12345, -999999, -2147483648, 2147483647, r.randrange(-3000, 3000), r.randrange(1 << 32) - (1 << 31)])


def fuzz(r, i):
    if i == 0:
        return orig_acc, e.r32(e.rt(ON_DUTY))
    acc = bytearray(orig_acc)
    for off in range(0, 0x98, 4):
        if r.random() < .6:
            struct.pack_into("<i", acc, off, val(r))
    if r.random() < .3:                       # the money colour edge: g_balanceAfterDay = g_balance (+/- 1)
        struct.pack_into("<I", acc, 0x28, (struct.unpack_from("<i", acc, 0x24)[0] + r.choice([-1, 0, 0, 1])) & 0xFFFFFFFF)
    return bytes(acc), val(r) & 0xFFFFFFFF


raises = [0]


def original(acc, duty, a, b, extra):
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(ACC), acc)
    e.w32(e.rt(ON_DUTY), duty)
    e.uc.mem_write(screen, bytes(64000))
    e.uc.mem_write(screen + 146 * 320, pan)
    # 0x42020 draws on the panel buffer's default place, 0x42090 takes buffer and pitch (StrategicScreen 0x4382B,
    # AssignScreen? 0x4A413) and 0x42114 is the lower with the pointer hidden (the port has one raise_tab for all).
    raises[0] += 1
    by_buffer = raises[0] & 1
    def raise_(m):
        if by_buffer:
            e.call(e.rt(0x42090), m, screen, 320)
        else:
            e.call(e.rt(0x42020), m)
    raise_(a)
    if b != a:
        if by_buffer:
            e.call(e.rt(0x42114), a)
        else:
            x, y = tab(a)
            e.call(e.rt(0x468B0), 10, x, y)
        raise_(b)
    e.call(e.rt(0x36010), e.rt(EMPTY), screen, 320, 0, 14)
    for k in extra:
        if k == 101:
            e.call(e.rt(0x35E84), 1)
        else:
            mode = struct.unpack("<h", e.uc.mem_read(e.rt(0x93FD8 + 19 * k + 0xD), 2))[0]
            e.call(e.rt(0x35F64), screen, 320, k, mode)
    return bytes(e.uc.mem_read(screen, 64000))


def port(acc, duty, a, b, extra):
    scr = ctypes.create_string_buffer(bytes(64000), 64000)
    ab = ctypes.create_string_buffer(acc, 0xA8)
    duty = duty - (1 << 32) if duty & 0x80000000 else duty
    if a == b:
        rc = PORT.ja1f_map_panel(ISO, EXE, ctypes.addressof(ab), duty, b, ctypes.addressof(scr))
    else:
        rc = PORT.ja1f_map_tab_switch(ISO, EXE, ctypes.addressof(ab), duty, a, b, ctypes.addressof(scr))
    if rc:
        raise RuntimeError(f"ja1f_map_panel returned {rc}")
    for k in extra:
        pk = {26: 16, 101: 17}.get(k, k)          # 16 = the pressed wage number, 17 = the wage box + field 1
        rc = PORT.ja1f_map_field(ISO, EXE, ctypes.addressof(ab), duty, pk, ctypes.addressof(scr))
        if rc:
            raise RuntimeError(f"ja1f_map_field {pk} returned {rc}")
    return scr.raw


r = random.Random(1)
fails = 0
for i in range(ncases):
    acc, duty = fuzz(r, i)
    a = r.randrange(3)
    b = a if r.random() < .5 else r.randrange(3)
    extra = [15] + ([r.choice([26, 101])] if r.random() < .4 else [])
    if i:
        acc = bytearray(acc)
        struct.pack_into("<i", acc, 0x70, r.choice([1, 9, 10, 33, 60, r.randrange(-5, 200)]))
        acc = bytes(acc)
    o = original(acc, duty, a, b, extra)
    if i == 0 and o[146 * 320:] == bytes(pan) + bytes(320):
        print("mappanel_diff: the original drew nothing on the panel")
        sys.exit(1)
    if not PORT:
        continue
    p = port(acc, duty, a, b, extra)
    if o != p:
        fails += 1
        n = next(j for j in range(64000) if o[j] != p[j])
        if fails <= 10:
            print(f"MISMATCH case {i} mode {a}->{b} extra {extra}: screen @({n % 320},{n // 320}) {o[n]:#x} vs "
                  f"{p[n]:#x} ({sum(x != y for x, y in zip(o, p))} bytes)")
        if fails == 1 and "--png" in argv:
            pix = b"".join(o[y * 320:(y + 1) * 320] + p[y * 320:(y + 1) * 320] for y in range(200))
            png(os.path.join(ROOT, "gfx", "mappanel_diff.png"), 640, 200, pix, pal)


def pscr(k, base):
    scr = ctypes.create_string_buffer(base, 64000)
    ab = ctypes.create_string_buffer(acc, 0xA8)
    rc = PORT.ja1f_map_field(ISO, EXE, ctypes.addressof(ab), 0, k, ctypes.addressof(scr))
    if rc:
        raise RuntimeError(f"ja1f_map_field {k} returned {rc}")
    return scr.raw


def report(i, what, o, p):
    global fails
    if o != p:
        fails += 1
        n = next(j for j in range(64000) if o[j] != p[j])
        if fails <= 10:
            print(f"MISMATCH {what} case {i}: screen @({n % 320},{n // 320}) {o[n]:#x} vs {p[n]:#x} "
                  f"({sum(x != y for x, y in zip(o, p))} bytes)")


# The finance page, the field boxes, the back button and the flip pictures (strategic.md "The finance page").
nfin = max(1, ncases // 3)
for i in range(nfin):
    acc, duty = fuzz(r, i)
    base = bytes(r.randrange(256) for _ in range(64)) * 1000
    what = r.choice(["page", "back0", "back1", "box4p", "box4u", "box0p", "box0u", "flip", "strip"])
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(ACC), acc)
    e.w32(e.rt(ON_DUTY), duty)
    e.uc.mem_write(screen, base)
    if what == "page":
        e.uc.mem_write(screen + 146 * 320, fin)
        e.call(e.rt(0x36010), e.rt(EMPTY), screen, 320, 16, 25)
        k = 18
    elif what.startswith("back"):
        e.call(e.rt(0x35DA4 if what == "back1" else 0x35C0C), 8)
        k = 20 if what == "back1" else 19
    elif what.startswith("box"):
        fk = int(what[3])
        e.call(e.rt(0x35F0C if what[4] == "p" else 0x35E84), fk)
        k = {"box4p": 21, "box4u": 22, "box0p": 23, "box0u": 24}[what]
    elif what == "strip":                         # the top strip's buttons 0x13..0x18 up / pressed
        b, pr = r.randrange(6), r.randrange(2)
        e.call(e.rt(0x35DA4 if pr else 0x35C0C), 0x13 + b)
        k = 28 + 2 * b + pr
    else:
        n = r.randrange(3)
        e.uc.mem_write(screen + 146 * 320, flips[n])
        k = 25 + n
    o = bytes(e.uc.mem_read(screen, 64000))
    if i == 0 and o == base and what != "flip":
        print("mappanel_diff: the original drew nothing (finance)")
        sys.exit(1)
    if PORT:
        report(i, what, o, pscr(k, base))

# The replay screen's panel (strategic.md "The day-end accounts"): 0x36010 fields 31..42 on a noise screen with
# fuzzed accounts and the day values 0x229AE0, 0x229AEC, 0x229AF0, 0x229ADC, 0x229AC8, 0x229AD0; port ja1f_replay_fields.
DAYV = (0x229AE0, 0x229AEC, 0x229AF0, 0x229ADC, 0x229AC8, 0x229AD0)
if PORT:
    PORT.ja1f_replay_fields.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 3
    PORT.ja1f_replay_fields.restype = ctypes.c_int32
for i in range(max(1, ncases // 3)):
    acc, duty = fuzz(r, i)
    day = [val(r) for _ in range(6)]
    base = bytes(r.randrange(256) for _ in range(64)) * 1000
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(ACC), acc)
    for a, v in zip(DAYV, day):
        e.w32(e.rt(a), v & 0xFFFFFFFF)
    e.uc.mem_write(screen, base)
    e.call(e.rt(0x36010), e.rt(EMPTY), screen, 320, 31, 42)
    o = bytes(e.uc.mem_read(screen, 64000))
    if i == 0 and o == base:
        print("mappanel_diff: the original drew nothing (replay panel)")
        sys.exit(1)
    if PORT:
        scr = ctypes.create_string_buffer(base, 64000)
        ab = ctypes.create_string_buffer(acc, 0xA8)
        db = (ctypes.c_int32 * 6)(*[(v + 2**31) % 2**32 - 2**31 for v in day])
        rc = PORT.ja1f_replay_fields(ISO, EXE, ctypes.addressof(ab), ctypes.addressof(db), ctypes.addressof(scr))
        if rc:
            raise RuntimeError(f"ja1f_replay_fields returned {rc}")
        report(i, "replay", o, scr.raw)

# The processing plants (tab 3): the screen while the button is held, and the screen after (= before).
nproc = max(1, ncases // 3)
orig_plants = bytes(e.uc.mem_read(e.rt(PLANTS), 48))
orig_sec = bytes(e.uc.mem_read(e.rt(SECTORS), 540))
for i in range(nproc):
    acc, duty = fuzz(r, i)
    plants, sec = bytearray(orig_plants), bytearray(orig_sec)
    if i:
        for k in range(4):
            struct.pack_into("<iiB", plants, 12 * k, r.choice([0, 1, 3, 5, 8, 10, 99, 100, 150, 220, 255, -1, r.randrange(-50, 300)]),
                             r.choice([r.randrange(60), r.randrange(60), 59, 0]), r.choice([2, 2, 0, 1, 3]))
        for s_ in range(60):
            if r.random() < .3:
                sec[s_ * 9] ^= 1
        acc = bytearray(acc)
        struct.pack_into("<i", acc, 0x1C, r.choice([0, 3, 7, 20, 230, 255, 256, 300, -1, r.randrange(-1000, 1000)]))
        acc = bytes(acc)
    base = bytes(r.randrange(256) for _ in range(64)) * 1000
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(ACC), acc)
    e.uc.mem_write(e.rt(PLANTS), bytes(plants))
    e.uc.mem_write(e.rt(SECTORS), bytes(sec))
    e.uc.mem_write(e.r32(e.rt(MAPBUF_P)), bytes(mapbuf))
    e.uc.mem_write(screen, base)
    held.clear()
    e.call(e.rt(0x42148))
    after = bytes(e.uc.mem_read(screen, 64000))
    if len(held) != 1 or after != base:
        fails += 1
        print(f"processing case {i}: held {len(held)}, screen after {'=' if after == base else '!='} before")
        continue
    if PORT:
        scr = ctypes.create_string_buffer(base, 64000)
        bufs = [ctypes.create_string_buffer(bytes(b), len(b)) for b in (acc, plants, sec)]
        rc = PORT.ja1f_processing(ISO, EXE, *[ctypes.addressof(b) for b in bufs], ctypes.addressof(scr))
        if rc:
            raise RuntimeError(f"ja1f_processing returned {rc}")
        report(i, "processing", held[0], scr.raw)
ncases += nfin + nproc
print(f"mappanel_diff{' (port)' if PORT else ''}: {ncases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
