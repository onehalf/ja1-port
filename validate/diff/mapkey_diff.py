#!/usr/bin/env python3
"""Differential test: the map screens' key handler MapKeyHandler? 0x426D0, original vs port (mapui key, guard_key).

usage: .venv/bin/python validate/diff/mapkey_diff.py [ncases=400] [--lib SO]
StrategicScreen passes DL 1 (0x4FC08) and GuardAssignScreen? passes DL 0 (0x42921). A run walks a list of keys: letters
of both cases, Enter, Escape and other extended keys. KeyPressed and GetKey answer the list, the real ToggleMessage
0x3F994 runs on the Subtitles switch 0x1163BD, StatusMessage? 0x27D24 takes the text. SpecialNpcSpeech? 0x29D54 and
ExitToDosPrompt 0x38084 are stubs that log the call (sayagain_diff and exittodos cover their bodies).
Compared per key: nothing, Enter (return 1), the message text, Alt-X, key L; and the switch after the run.
On the original side only: g_ctrlHeld 0x284F1F = flags 0x22A46C & 0x104 != 0 on each key but Enter (the port's window
reads the Ctrl state itself). Enter on the guard screen is not pressed (guard_diff tests its exit).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 400
PORT = ctypes.CDLL(LIB)
PORT.ja1f_map_key.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p,
                                                       ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int]
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
SAVE = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV").encode()
SUBS, CTRL, FLAGS = 0x1163BD, 0x284F1F, 0x22A46C

e = Emu(fast=True)
queue, log = [], []
e.stub(e.rt(0x5616C), lambda em, *_: 1 if queue else 0)
e.stub(e.rt(0x560F4), lambda em, *_: queue.pop(0))
e.stub(e.rt(0x29D54), lambda em, *_: log.append((4, None)) or 0)
e.stub(e.rt(0x38084), lambda em, eax, *_: log.append((3, eax)) or 0)
e.stub(e.rt(0x27D24), lambda em, eax, *_: log.append((2, em.cstr(eax).decode("latin-1"))) or 0)


def letters(rng):
    pick = rng.random()
    if pick < 0.45:
        return ord(rng.choice("tTlLpPgGmM"))
    if pick < 0.60:
        return 27
    if pick < 0.80:
        return rng.randrange(1, 128)
    return ord(rng.choice("abcdefghijknoqrsuvwxyzABCDEFGHIJKNOQRSUVWXYZ0123456789"))


rng = random.Random(0x426D0)
bad = counts = 0
seen = [0] * 5
for case in range(ncases):
    guard = case % 4 == 3
    dl = 0 if guard else 1
    subs0 = rng.randrange(2)
    keys = []
    for _ in range(rng.randrange(1, 9)):
        r = rng.random()
        if r < 0.15:
            keys.append((0, 0x2D if rng.random() < 0.6 else rng.choice([0x2C, 0x2E, 0x3B, 0x48, 0x50, 0x0D, 0x4C])))
        else:
            k = 13 if not guard and r < 0.25 else letters(rng)
            while guard and k == 13:
                k = letters(rng)
            keys.append((k, 0))
    # the original
    e.w8(e.rt(SUBS), subs0)
    want = []
    for a, scan in keys:
        queue[:] = [a, scan] if a == 0 else [a]
        log.clear()
        flags = rng.choice([0, 0x4, 0x100, 0x104, 0x1, 0x2, 0x8])
        e.w32(e.rt(FLAGS), flags)
        e.w8(e.rt(CTRL), 0x55)
        ret = e.call(e.rt(0x426D0), 0, dl) & 0xFF
        if ret == 1:
            kind, txt = 1, None
        elif log:
            kind, txt = log[0]
            if kind != 2:
                txt = None
        else:
            kind, txt = 0, None
        c = e.r8(e.rt(CTRL))
        if ret == 1:
            if c != 0x55:
                print(f"case {case}: Enter wrote g_ctrlHeld")
                bad += 1
        elif c != (1 if flags & 0x104 else 0):
            print(f"case {case}: g_ctrlHeld {c} flags {flags:#x}")
            bad += 1
        want.append((kind, txt))
    subs_want = e.r8(e.rt(SUBS))
    # the port
    n = len(keys)
    kb = (ctypes.c_int * (2 * n))(*[v for p in keys for v in p])
    out = (ctypes.c_int * (2 * n))()
    tx = ctypes.create_string_buffer(4096)
    su = ctypes.c_int(subs0)
    rc = PORT.ja1f_map_key(ISO, EXE, SAVE, int(guard), kb, n, ctypes.byref(su), out, tx, 4096)
    if rc != 0:
        print(f"case {case}: port returned {rc}")
        bad += 1
        continue
    for i in range(n):
        kind, off = out[2 * i], out[2 * i + 1]
        txt = None if off < 0 else tx.raw[off:tx.raw.index(b"\0", off)].decode("latin-1")
        counts += 1
        seen[want[i][0]] += 1
        if (kind, txt) != want[i]:
            print(f"case {case} guard {guard} key {keys[i]}: port {(kind, txt)} original {want[i]}")
            bad += 1
    if su.value != subs_want:
        print(f"case {case}: Subtitles port {su.value} original {subs_want}")
        bad += 1
print(f"mapkey_diff: {ncases} runs, {counts} keys, nothing/Enter/box/Alt-X/L = {seen}, {bad} mismatches")
sys.exit(1 if bad else 0)
