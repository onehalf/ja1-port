#!/usr/bin/env python3
"""Differential test: the opening of the day replay, ReplayScreenInit? 0x64274, original vs port.

usage: .venv/bin/python validate/diff/replayinit_diff.py [ncases=100] [DUMP] [--lib SO]
A case: a fuzzed replay table, sector and plant state, day counters and day-start copies. The original runs from its
first line. FormatFxPath, LoadFxSound, HidePointer?, MouseSetPos? and FadePalette? are logged stubs. LoadArchivePcx? and
LoadArchivePalette? read MAIN.DAT in Python. The rest is real: PaletteCopy?, LoadPanelPcc, DrawStrategicMap, the accounts,
DrawMapTable, DrawPanelPccFields. The port: ja1f_replay_init = ReplayPic::new + panel_numbers + fade_in, with the accounts
of the original after the call (verdict_diff compares the accounts).
Compared: the screen, rows 0..145 of the clean map (no replay picture reads lower rows), the palette, the fade levels.
Then the sound bank: the slot names from LoadFxSound, the replay animations with PlaySfxSlot logged, against
ja1f_replay_sounds (name and volume per sound). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from datarc import read, member      # noqa: E402
from pcx import decode               # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP as ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 100
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"replayinit_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
P = ctypes.CDLL(LIB)
P.ja1f_replay_init.argtypes = [ctypes.c_char_p, ctypes.c_char_p] + [ctypes.c_void_p] * 7
P.ja1f_replay_init.restype = ctypes.c_int32
P.ja1f_replay_sounds.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                 ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32]
P.ja1f_replay_sounds.restype = ctypes.c_int32

R, ACC, SCREEN_P, CLEAN_P, MAPBUF_P, DAC = 0x2254F0, 0x229FA0, 0x95678, 0x93E60, 0x93E5C, 0xEDA08
DEAD = (0x229AE0, 0x229AE4, 0x229AE8, 0x229AF0)
STARTS = (0x229AD8, 0x229AC4, 0x229ACC)
main = read(os.path.join(ROOT, "build", "game113b", "DAT", "MAIN.DAT"))
pics = {}


def pic(name):
    if name not in pics:
        pics[name] = decode(member(main, name))
    return pics[name]


e = Emu(dump=dump, fast=True)
rt = e.rt
log = []


def cstr(emu, a):
    return bytes(emu.uc.mem_read(a, 16)).split(b"\0")[0].decode()


def load_pcx(emu, arc, hdr, name, buf):
    pitch = emu.r32(emu.uc.reg_read(ESP) + 4)
    w, h, px, _ = pic(cstr(emu, name).upper())
    log.append(("pcx", cstr(emu, name).upper(), buf, pitch))
    for y in range(h):
        emu.uc.mem_write(buf + y * pitch, px[y * w:(y + 1) * w])
    return 0


def load_pal(emu, arc, hdr, name, out):
    pal = pic(cstr(emu, name).upper())[3]
    emu.uc.mem_write(out, bytes(v >> 2 for v in pal))
    log.append(("pal", cstr(emu, name).upper()))
    return 0


e.stub(rt(0x532BC), load_pcx, pop=4)
e.stub(rt(0x53300), load_pal)
e.stub(rt(0x24C90), lambda em, a, *_: log.append(("fxdir", cstr(em, a))))
e.stub(rt(0x24CB4), lambda em, a, d, *_: log.append(("fx", cstr(em, a), d & 0xFF)))
e.stub(rt(0x67BF0), lambda em, *_: log.append(("hide",)))
e.stub(rt(0x681D0), lambda em, a, d, *_: log.append(("mouse", a, d)))
fades = []
e.stub(rt(0x52908), lambda em, a, d, b, c, *_: fades.append((a, d & 0xFFFF, b & 0xFFFF, c & 0xFFFF)), pop=4)
e.stub(rt(0x67250), lambda em, *_: 0)
e.stub(rt(0x67EEC), lambda em, *_: 0)
e.stub(rt(0x67300), lambda em, *_: 0)
sfx = []
e.stub(rt(0x5CB34), lambda em, a, d, b, c, *_: sfx.append((a, d, b & 0xFF, c)))
e.stub(rt(0x53538), lambda em, n, *_: 0)
snap = [(lo, bytes(e.uc.mem_read(lo, hi - lo + 1))) for lo, hi, _ in e.uc.mem_regions() if lo < 0x1000000]
screen, clean = e.r32(rt(SCREEN_P)), e.r32(rt(CLEAN_P))


def i32(a):
    return int.from_bytes(e.uc.mem_read(rt(a), 4), "little", signed=True)


def table(r):
    t = bytearray()
    for s in range(60):
        rec = [r.randrange(4) if r.random() < .8 else r.randrange(256)] + [r.choice([0, 0, 1, 2, 5, 10, r.randrange(16)]) for _ in range(8)]
        rec[7] = r.randrange(rec[6] + 1)
        rec[8] = r.randrange(rec[6] + 1)
        t += bytes(rec)
    return bytes(t)


bad = 0
seen = set()
r = random.Random(0x64274)
for case in range(ncases):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    log.clear()
    fades.clear()
    t = table(r)
    e.uc.mem_write(rt(R), t)
    e.uc.mem_write(screen, bytes(r.randrange(256) for _ in range(64)) * 1000)
    e.uc.mem_write(clean, bytes(r.randrange(256) for _ in range(64)) * 1000)
    big = case % 4 == 3
    for a in DEAD + STARTS:
        v = r.randrange(-2000, 2000) if big else r.randrange(0, 30)
        e.uc.mem_write(rt(a), v.to_bytes(4, "little", signed=True))
    v7 = [i32(0x229AE0), i32(0x229AE4), i32(0x229AE8), i32(0x229AF0), i32(0x229AD8), i32(0x229AC4), i32(0x229ACC)]
    e.call(rt(0x64274))
    o_scr, o_cln = bytes(e.uc.mem_read(screen, 64000)), bytes(e.uc.mem_read(clean, 64000))
    o_dac4 = bytes(e.uc.mem_read(rt(DAC), 1024))
    acc = bytes(e.uc.mem_read(rt(ACC), 0xA8))
    seen.add(("sign", v7[0] < 0))
    v = (ctypes.c_int32 * 7)(*v7)
    s, c, p, f = [ctypes.create_string_buffer(n) for n in (64000, 64000, 768, 25 * 768)]
    n = P.ja1f_replay_init(ISO, EXE, t, acc, v, s, c, p, f)
    msgs = []
    if n != 25:
        msgs.append(f"fade levels {n}")
    if s.raw != o_scr:
        d = [i for i in range(64000) if s.raw[i] != o_scr[i]]
        msgs.append(f"screen: {len(d)} pixels, first x {d[0] % 320} y {d[0] // 320}, orig {o_scr[d[0]]} port {s.raw[d[0]]}, last y {d[-1] // 320}")
    if c.raw[:146 * 320] != o_cln[:146 * 320]:
        d = [i for i in range(146 * 320) if c.raw[i] != o_cln[i]]
        msgs.append(f"clean: {len(d)} pixels, first x {d[0] % 320} y {d[0] // 320}, orig {o_cln[d[0]]} port {c.raw[d[0]]}")
    o_rgb = bytes(o_dac4[i * 4 + c_] for i in range(256) for c_ in range(3))
    want = bytes(b * 255 // 63 for b in o_rgb)
    if p.raw != want:
        i = next(i for i in range(768) if p.raw[i] != want[i])
        msgs.append(f"palette: first byte {i}, orig {want[i]} port {p.raw[i]}")
    if f.raw[24 * 768:] != p.raw:
        msgs.append("the last fade level is not the palette")
    if any(f.raw[:768]):
        msgs.append("the first fade level is not black")
    if fades != [(rt(DAC), 0, 0x18, 0)]:
        msgs.append(f"fade args {fades}")
    if msgs:
        bad += 1
        if bad <= 5:
            print(f"case {case}: " + "; ".join(msgs))

# The sound bank: every replay animation plays the slots of the opening's bank.
names = {x[2]: x[1].upper() for x in log if x[0] == "fx"}
if [x for x in log if x[0] == "fxdir"] != [("fxdir", "replay")] or sorted(names) != list(range(6)):
    bad += 1
    print("sound bank", names)
ANIM = {0: 0x64658, 1: 0x64D20, 3: 0x643F8, 4: 0x664FC}
snd_cases = 0
for case in range(max(ncases // 2, 24)):
    for lo, b in snap:
        e.uc.mem_write(lo, b)
    t = table(r)
    e.uc.mem_write(rt(R), t)
    k = (0, 1, 3, 4)[case % 4]
    s = r.randrange(60)
    # The second and third arguments as replaypic_diff uses them.
    if k == 1:
        s = r.randrange(11, 49)
        a = [s, r.choice([s - 10, s + 10, s - 1, s + 1, s]), r.choice([1, 2]), r.randrange(256), 0]
    else:
        a = [s, r.choice([1, 2]), r.randrange(2), r.randrange(256), 0]
    e.uc.mem_write(rt(0x9562D), bytes([a[3]]))
    sfx.clear()
    e.call(rt(ANIM[k]), *a[:3])
    orig = [f"{names[x[0] & 0xFF]} {x[2]}" for x in sfx]
    bad_arg = [x for x in sfx if x[1] != 0x2B11 or x[3] != 1]
    aa = (ctypes.c_int32 * 5)(*a)
    out = ctypes.create_string_buffer(4096)
    n = P.ja1f_replay_sounds(ISO, EXE, t, k, aa, a[3], out, 4096)
    port = out.raw[:n].decode().split("\n")[:-1] if n > 0 else []
    snd_cases += 1
    seen.add(("snd", k, bool(orig)))
    if orig != port or bad_arg or n < 0:
        bad += 1
        if bad <= 5:
            print(f"sound case {case} k {k} a {a}: orig {orig[:6]} port {port[:6]} args {bad_arg[:2]} n {n}")
for k in (0, 1, 3, 4):
    if ("snd", k, True) not in seen:
        bad += 1
        print(f"no sound in animation {k}")
print(f"replayinit_diff: {ncases} + {snd_cases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
