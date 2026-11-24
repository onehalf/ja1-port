#!/usr/bin/env python3
"""Differential test: the first picture of the guard screen and the picture after clicks, GuardAssignScreen? 0x427B4, original vs port.

usage: .venv/bin/python validate/diff/guardpic_diff.py [ncases=30] [DUMP] [--lib SO]
The original runs in Unicorn on tac60 for 1..9 passes of the loop, with fuzzed times around the 500 ms limit of the water rotation (real PaletteRotate? 0x528AC;
PaletteSet? 0x52870 logged; the mouse on the current sector). Real: InitStrategicMap 0x43ECC,
CountSectorGuards? 0x42664, DrawPanelPccFields 0x36010, the hover in the loop. MAIN.DAT comes through the file layer stubs of mapopen_diff. Stubbed: the
fade, the mouse, the key handler and the start button. Per case: a fuzzed sector table, cur_sector and old guard counts 0x229D8C. Compared with ja1f_guard_picture: the 320 x 200 screen and the
display palette at 0xEDA08 after the last pass. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop         # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 30
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_guard_picture.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_void_p,
                                   ctypes.c_void_p, ctypes.c_int32]
PORT.ja1f_guard_picture.restype = ctypes.c_int32
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SECTORS, CUR, MARKS, PAL_DAC = 0x22979C, 0x22A039, 0x229D8C, 0xEDA08
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen = e.r32(e.rt(0x95678))
cur = {"data": b"", "pos": 0}
loaded = []
passes = {"n": 0, "stop": 2, "times": [0], "sets": []}


def load(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    loaded.append(name)
    w, h, px, _ = pcx.decode(datarc.member(MAIN, name))
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def arcseek(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    loaded.append(name)
    cur["data"], cur["pos"] = datarc.member(MAIN, name), 0
    em.w32(em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4), len(cur["data"]))
    return 0


def fseek(em, eax, edx, ebx, ecx):
    cur["pos"] = {0: edx, 1: cur["pos"] + (edx - (1 << 32) if edx >> 31 else edx), 2: len(cur["data"]) + edx}[ebx]
    return 0


def fgetc(em, *_):
    cur["pos"] += 1
    return cur["data"][cur["pos"] - 1]


def fread(em, eax, edx, ebx, ecx):
    b = cur["data"][cur["pos"]:cur["pos"] + edx * ebx]
    cur["pos"] += len(b)
    em.uc.mem_write(eax, b)
    return len(b) // edx


def elapsed(em, *_):
    # TimerElapsedMs(0x22A3FC) of the pass that runs now.
    return passes["times"][passes["n"] - 1]


def pal_set(em, eax, edx, ebx, ecx):
    passes["sets"].append((eax, edx, ebx))
    return 0


def poll(em, *_):
    if em.r32(em.uc.reg_read(UC_X86_REG_ESP)) == em.rt(0x42918):
        passes["n"] += 1
        if passes["n"] == passes["stop"]:
            em.stop_reason = "last pass"
            em.uc.emu_stop()
    return 0


for a_, pop_, f_ in ((0x532BC, 4, load), (0x534A4, 4, arcseek), (0x757B1, 0, fseek), (0x7648C, 0, fgetc), (0x758AD, 0, fread),
                     (0x38190, 0, lambda em, *_: 0), (0x678E8, 0, poll), (0x681D0, 0, lambda em, *_: 0),
                     (0x52908, 4, lambda em, *_: 0), (0x67BF0, 0, lambda em, *_: 0), (0x673AC, 0, lambda em, *_: 0),
                     (0x67C54, 0, lambda em, *_: 0), (0x67C68, 0, lambda em, *_: 0), (0x682AC, 0, lambda em, *_: 0),
                     (0x47040, 0, elapsed), (0x47020, 0, lambda em, *_: 0), (0x52870, 0, pal_set),
                     (0x770EB, 0, lambda em, *_: 0), (0x426D0, 0, lambda em, *_: 0), (0x67CD8, 0, lambda em, *_: 0)):
    e.stub(e.rt(a_), f_, pop_)
snap = bytes(e.uc.mem_read(0, 16 << 20))
orig_tbl = bytes(e.uc.mem_read(e.rt(SECTORS), 540))
orig_cur = e.r8(e.rt(CUR))


def count(r):
    return r.choice([0, 0, 1, 2, 3, 5, 8, 10, r.randrange(16), r.randrange(100)])


def fuzz(r, i):
    if i == 0:
        return orig_tbl, orig_cur
    t = bytearray()
    for s in range(60):
        t += bytes([r.randrange(2)] + [count(r) for _ in range(8)])
    return bytes(t), r.randrange(60)


r = random.Random(4427)
fails = 0
for i in range(ncases):
    tbl, c = fuzz(r, i)
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(SECTORS), tbl)
    e.w8(e.rt(CUR), c)
    # Old counts from the last guard screen: InitStrategicMap draws them (0x41AE2), and the clear at 0x4283B comes after it.
    old = bytes(r.randrange(4) if r.random() < .3 else 0 for _ in range(60))
    e.uc.mem_write(e.rt(MARKS), old)
    e.uc.mem_write(screen, bytes(r.randrange(256) for _ in range(64)) * 1000)
    loaded.clear()
    # Times per loop pass: the first one is old (the screen does not start this timer), then around the 500 ms limit.
    times = [r.choice([501, 501, 501, 500, 0]) for _ in range(r.randrange(1, 10))]
    passes.update(n=0, stop=len(times) + 1, times=times, sets=[])
    e.w32(e.rt(0x229D00), (c % 10) * 31 + 3)     # the mouse stands where the screen puts it
    e.w32(e.rt(0x229D04), (c // 10) * 23 + 3)
    e.w8(e.rt(0x229CF4), 0)
    e.w8(e.rt(0x284F1F), 0)
    e.stop_reason = None
    try:
        e.call(e.rt(0x427B4), max_steps=200_000_000)
    except EmuStop:
        pass
    assert e.stop_reason == "last pass", (i, e.stop_reason)
    rotations = sum(t > 500 for t in times)
    assert passes["sets"] == [(e.rt(PAL_DAC), 0xD7, 0xDE)] * rotations, (i, passes["sets"])
    a = bytes(e.uc.mem_read(screen, 64000))
    dac = bytes(e.uc.mem_read(e.rt(PAL_DAC), 1024))
    if any(dac[3::4]):
        raise RuntimeError("pad bytes of the palette entries")
    pal_in = bytes(b for k in range(256) for b in dac[k * 4:k * 4 + 3])
    if i == 0:
        print("files", list(loaded))
    scr, pal = ctypes.create_string_buffer(64000), ctypes.create_string_buffer(768)
    st, mk = ctypes.create_string_buffer(tbl, 540), ctypes.create_string_buffer(old, 60)
    rc = PORT.ja1f_guard_picture(ISO, EXE, ctypes.addressof(st), ctypes.addressof(mk), c, ctypes.addressof(scr), ctypes.addressof(pal),
                                 (ctypes.c_int32 * len(times))(*times), len(times))
    if rc:
        raise RuntimeError(f"ja1f_guard_picture returned {rc}")
    b = scr.raw
    bad = []
    if a != b:
        d = [j for j in range(64000) if a[j] != b[j]]
        rows = sorted({j // 320 for j in d})
        bad.append(f"screen {len(d)} bytes @({d[0] % 320},{d[0] // 320}) {a[d[0]]:#x} vs {b[d[0]]:#x}, rows {rows[0]}..{rows[-1]} ({len(rows)})")
    if pal_in != bytes(v >> 2 for v in pal.raw):
        bad.append(f"palette after {rotations} rotations")
    if bad:
        fails += 1
        if fails <= 5:
            print(f"MISMATCH case {i} cur {c}: " + "; ".join(bad))

# Part 2: clicks. The script takes guards and sends them, so GuardCellRedraw? 0x42500 draws cells over the old counts of the first
# picture. Real: the whole click code; a case with a message box (0x27D24) counts for nothing on both sides.
PORT.ja1f_guard_clicks.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p,
                                   ctypes.c_void_p, ctypes.c_int32]
PORT.ja1f_guard_clicks.restype = ctypes.c_int32
script = {"ev": [], "j": 0, "box": False}


def poll2(em, *_):
    if em.r32(em.uc.reg_read(UC_X86_REG_ESP)) != em.rt(0x42918):
        return 0
    j = script["j"]
    if j >= len(script["ev"]):
        em.stop_reason = "last pass"
        em.uc.emu_stop()
        return 0
    script["j"] = j + 1
    x, y, b, ctrl = script["ev"][j]
    em.w32(em.rt(0x229D00), x)
    em.w32(em.rt(0x229D04), y)
    em.w8(em.rt(0x229CF4), b)
    em.w8(em.rt(0x284F1F), ctrl)
    return 0


def msg2(em, *_):
    script["box"] = True
    em.stop_reason = "box"
    em.uc.emu_stop()
    return 0


e.stub(e.rt(0x678E8), poll2, 0)
e.stub(e.rt(0x27D24), msg2, 0)
passes["times"] = [0] * 4
elapsed2 = lambda em, *_: 0
e.stub(e.rt(0x47040), elapsed2, 0)


def at(s):
    return (s % 10) * 31 + 15, (s // 10) * 23 + 11


def near(r, c):
    n = [s for s in (c - 1, c + 1, c - 10, c + 10) if 0 <= s < 60 and (s // 10 == c // 10 or s % 10 == c % 10)]
    return r.choice(n)


def click_script(r, c):
    ev, a, b = [], c, near(r, c)
    ev.append((*at(a), 0, 0))
    for _ in range(r.randrange(1, 4)):
        for s, btn, ct in ((a, 2, 0), (b, 1, 0)):
            ev += [(*at(s), 0, 0), (*at(s), btn, ct), (*at(s), 0, 0)]
    if r.random() < .5:
        ev += [(*at(b), 0, 1), (*at(b), 2, 1), (*at(b), 0, 1), (*at(b), 0, 0)]
    # Hovers with guards in hand or none: another cell, the panel (the hover ends, its fields clear) and back.
    if r.random() < .5:
        ev += [(*at(a), 0, 0), (*at(a), 2, 0), (*at(a), 0, 0), (*at(near(r, a)), 0, 0)]
    if r.random() < .5:
        ev += [(150, 170, 0, 0), (150, 170, 0, 0)]
        if r.random() < .5:
            ev.append((*at(b), 0, 0))
    ev.append((*at(near(r, b)), 0, 0))
    return ev


ncl, nbox = 0, 0
cfails = 0
for i in range(ncases * 3):
    tbl = bytearray(fuzz(r, i + 1)[0])
    c = r.randrange(60)
    for s_ in range(60):
        # Most counts are of play. GuardCellRedraw? restores one cell only: a row of more than 8 marks stays over the next cell until
        # that cell's redraw. The strategic screen allows 8 guards with b5 guards on the way, so such a row is possible.
        tbl[s_ * 9 + 1:s_ * 9 + 9] = bytes(r.randrange(3) for _ in range(8))
        if i % 2 and r.random() < .3:
            tbl[s_ * 9 + r.choice((4, 5, 6))] = r.randrange(8, 13)
        if r.random() < .9:     # every sector owned: linked to home, so few cases end in a box
            tbl[s_ * 9] = 1
    tbl[c * 9] = 1
    tbl[c * 9 + 4] = r.randrange(2, 6)
    ev = click_script(r, c)
    tbl = bytes(tbl)
    e.uc.mem_write(0, snap)
    e.uc.mem_write(e.rt(SECTORS), tbl)
    e.w8(e.rt(CUR), c)
    old = bytes(r.randrange(2) for _ in range(60))
    e.uc.mem_write(e.rt(MARKS), old)
    e.uc.mem_write(screen, bytes(64000))
    e.w32(e.rt(0x229D00), (c % 10) * 31 + 3)
    e.w32(e.rt(0x229D04), (c // 10) * 23 + 3)
    e.w8(e.rt(0x229CF4), 0)
    e.w8(e.rt(0x284F1F), 0)
    e.stop_reason = None
    script.update(ev=ev, j=0, box=False)
    try:
        e.call(e.rt(0x427B4), max_steps=200_000_000)
    except EmuStop:
        pass
    a = bytes(e.uc.mem_read(screen, 64000))
    scr = ctypes.create_string_buffer(64000)
    st, mk = ctypes.create_string_buffer(tbl, 540), ctypes.create_string_buffer(old, 60)
    sc = (ctypes.c_int32 * (4 * len(ev)))(*[v for x in ev for v in x])
    rc = PORT.ja1f_guard_clicks(ISO, EXE, ctypes.addressof(st), ctypes.addressof(mk), c, ctypes.addressof(scr), ctypes.addressof(sc), len(ev))
    if script["box"] or rc == 2:
        nbox += 1
        if script["box"] != (rc == 2):
            cfails += 1
            if cfails <= 5:
                print(f"MISMATCH click case {i}: box original {script['box']} port {rc}")
        continue
    assert e.stop_reason == "last pass" and rc == 0, (i, e.stop_reason, rc)
    ncl += 1
    b = scr.raw
    if a != b:
        cfails += 1
        d = [j for j in range(64000) if a[j] != b[j]]
        rows = sorted({j // 320 for j in d})
        if cfails <= 5:
            print(f"MISMATCH click case {i} cur {c}: {len(d)} bytes @({d[0] % 320},{d[0] // 320}) {a[d[0]]:#x} vs {b[d[0]]:#x}, rows {rows[0]}..{rows[-1]}")
assert ncl >= ncases // 2, (ncl, nbox)
fails += cfails
print(f"guardpic_diff clicks: {ncl} cases, {nbox} with a box, {cfails} mismatches")
print(f"guardpic_diff: {ncases} cases, {fails} mismatches")
sys.exit(1 if fails else 0)
