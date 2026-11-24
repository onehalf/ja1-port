"""Differential test: OfficeHighlight 0x4C0C8 + OfficeHighlightFrame 0x4BFFC for the office's hot spots 1..7.

usage: .venv/bin/python validate/diff/officehl_diff.py [ncases=60] [DUMP]
Per case: a noise panel picture, a noise palette and a chain of highlight on / off calls with fuzzed pauses (0..250
ms). The original runs in Unicorn on tac60 with the real wait loop and the real OfficePaletteCycle 0x4C26C, on a virtual
clock: MousePoll 0x678E8 adds 1 ms, TimerElapsedMs 0x47040 / TimerStart 0x47020 use it. OFFBRUSH.PCX is decoded into
buffer 0x93E5C. Stubs: CopyPanelToScreen 0x46BF4 (log the panel), PlaySfxSlot 0x5CB34 (log), PaletteSet 0x52870 (log),
HidePointer 0x67BF0, ShowMouse 0x673AC. The port is office::highlight_on / highlight_off / play_frames.
Compared: pictures, sounds with rate / volume / loops, palette rotations, each with its clock; the palette at the end.
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import datarc, pcx                   # noqa: E402

args = [a for a in sys.argv[1:] if not a.startswith("--")]
ncases = int(args[0]) if args else 60
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
P.ja1f_office_highlight.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                    ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p]
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
PANEL_P, BRUSH_P, PAL = 0x93E60, 0x93E5C, 0xEDA08
OFFICE = open(os.path.join(ROOT, "rom", "DAT", "OFFICE.DAT"), "rb").read()

e = Emu(dump=dump, fast=True)
log, clock, timers = [], [0], {}
w, h, px, _ = pcx.decode(datarc.member(OFFICE, "OFFBRUSH.PCX"))
brush = bytearray(640 * 320)
for y in range(h):
    brush[y * 640:y * 640 + w] = bytes(px[y * w:(y + 1) * w])
e.w32(e.rt(BRUSH_P), e.alloc(bytes(brush)))
panel = e.r32(e.rt(PANEL_P))


def elapsed(em, eax, *_):
    return clock[0] - timers.get(eax, -100000)


def start(em, eax, *_):
    timers[eax] = clock[0]


def tick(em, *_):
    clock[0] += 1
    return 0


e.stub(e.rt(0x47040), elapsed)
e.stub(e.rt(0x47020), start)
e.stub(e.rt(0x678E8), tick)
e.stub(e.rt(0x46BF4), lambda em, *_: log.append(("pic", bytes(e.uc.mem_read(panel, 64000)), clock[0])))
e.stub(e.rt(0x5CB34), lambda em, a, d, b, c: log.append(("sfx", a, d, b & 0xFF, c, clock[0])))
e.stub(e.rt(0x52870), lambda em, *_: log.append(("cyc", clock[0])))
for a in (0x67BF0, 0x673AC):
    e.stub(e.rt(a), lambda em, *_: 0)

r = random.Random(11)
PAL_A, TIMER_A = e.rt(PAL), None
bad = seen = 0
hits = set()
for i in range(ncases):
    noise = bytes(r.randrange(256) for _ in range(64000))
    pal4 = bytes(r.randrange(64) for _ in range(1024))    # the original's entries have 4 bytes, the port's 3
    pal = b"".join(pal4[q * 4:q * 4 + 3] for q in range(256))
    e.uc.mem_write(panel, noise)
    e.uc.mem_write(PAL_A, pal4)
    seq, on = [], 0
    for _ in range(r.randrange(1, 7)):
        gap = r.choice([0, 0, 1, 30, 89, 90, 91, 250, r.randrange(250)])
        if on:
            seq.append((on, 0, gap))
            on = 0
        else:
            on = r.randrange(1, 8)
            seq.append((on, 1, gap))
    log.clear()
    timers.clear()
    clock[0] = 1000
    timers[e.rt(0x22A404)] = 1000
    for k, o, gap in seq:
        clock[0] += gap
        hits.add((k, o))
        e.call(e.rt(0x4C0C8), k, o, max_steps=50_000_000)
    m = bytes(e.uc.mem_read(PAL_A, 1024))
    pal_o = b"".join(m[q * 4:q * 4 + 3] for q in range(256))
    base = ctypes.create_string_buffer(noise, 64000)
    pp = ctypes.create_string_buffer(pal, 768)
    sq = (ctypes.c_int32 * (3 * len(seq)))(*[v for t in seq for v in t])
    cap, picap = 2000, 40
    lg = (ctypes.c_int32 * (6 * cap))()
    pics = ctypes.create_string_buffer(64000 * picap)
    ct = (ctypes.c_int32 * 2)()
    rc = P.ja1f_office_highlight(EXE, base, sq, len(seq), pp, lg, cap, pics, picap, ct)
    got = []
    for q in range(ct[0]):
        t, a, b, c, d, tm = lg[6 * q:6 * q + 6]
        if t == 0:
            got.append(("pic", pics.raw[a * 64000:(a + 1) * 64000], tm))
        elif t == 1:
            got.append(("sfx", a, b, c, d, tm))
        else:
            got.append(("cyc", tm))
    why = None
    if rc != 0:
        why = f"rc {rc}"
    else:
        for q in range(max(len(got), len(log))):
            g, o = (got[q] if q < len(got) else None), (log[q] if q < len(log) else None)
            if g != o:
                sg = g and (g[0], g[-1]) if g and g[0] == "pic" else g
                so = o and (o[0], o[-1]) if o and o[0] == "pic" else o
                why = f"step {q}: original {so}, port {sg}"
                if g and o and g[0] == o[0] == "pic" and g[1] != o[1]:
                    z = next(z for z in range(64000) if g[1][z] != o[1][z])
                    why += f" picture @({z % 320},{z // 320}) {o[1][z]:#x} vs {g[1][z]:#x}"
                break
        if why is None and pp.raw != pal_o:
            why = "palette at the end"
    seen += len(log)
    if why:
        print(f"MISMATCH case {i} {seq}: {why}")
        bad += 1
        if bad > 10:
            break
missing = [(k, o) for k in range(1, 8) for o in (0, 1) if (k, o) not in hits]
if missing and ncases >= 30:
    print("not reached:", missing)
    bad += 1
print(f"officehl_diff: {ncases} cases, {seen} steps, {bad} mismatches")
sys.exit(1 if bad else 0)
