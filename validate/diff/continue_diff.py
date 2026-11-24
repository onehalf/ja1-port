#!/usr/bin/env python3
"""Differential test: the prompt at the program start, ContinueDayPrompt 0x4FF00 + DeleteDaySaves 0x37FF0, original vs port.

usage: .venv/bin/python validate/diff/continue_diff.py [ncases=24] [DUMP] [--lib SO]
The original runs in Unicorn with the real AIMSCRN palette read (fread_, fseek_, fgetc_ serve MAIN.DAT), the real
question boxes (the PCC loader and the input 0x2538C are stubbed; the input takes the case's answers) and the real
DeleteDaySaves (FileExists and unlink_ are stubbed and logged). The port: quicksave::continue_prompt
(ja1f_continue_prompt) on a real folder that holds the same day files.
Compared: the answer, the order of fades, key flush, pointer, questions and delete, the fade palettes (each fade call,
4-byte entries), the 64000 pixels of each question box, which day files go.
On the original side only: the screen is back to black after each box; the pointer calls inside a box (the port's window
has its own pointer, so these calls are left out of the compare); the shape 0.
Not compared: the byte 0x229D18 (the palette read's result; the port has no field); the box's pressed button pictures
(question_diff). Exits 1 on mismatch.
"""
import ctypes, os, random, shutil, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from common import game_read  # noqa: E402
import datarc, pcx  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 24
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"continue_diff: {dump} missing")
    sys.exit(1)
ISO, EXE = (os.path.join(ROOT, *p).encode() for p in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
PORT = ctypes.CDLL(LIB)
PORT.ja1f_continue_prompt.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_int,
                                                              ctypes.c_void_p, ctypes.c_int]
MAIN = game_read("DAT\\MAIN.DAT")
e = Emu(dump=dump, fast=True)
rt = e.rt
SCREEN_PTR = 0x95678
FILES = ("OFFICE.SAV", "DAY.SAV", "QUICK.SAV")
st = {"pos": 0}
screen = e.r32(rt(SCREEN_PTR))


def log(s):
    st["log"].append(s)
    return 0


def fread(em, a, d, b, c, *_):
    out = MAIN[st["pos"]:st["pos"] + d * b]
    if out:
        e.uc.mem_write(a, bytes(out))
    st["pos"] += len(out)
    st["reads"] += 1
    return len(out) // max(d, 1)


def fseek(em, a, d, b, *_):
    st["pos"] = {0: d, 1: st["pos"] + d, 2: len(MAIN) + d}[b]
    return 0


def fgetc(em, *_):
    if st["pos"] >= len(MAIN):
        return 0xFFFFFFFF
    st["pos"] += 1
    return MAIN[st["pos"] - 1]


def fade(em, a, d, b, c, *_):
    sp = e.uc.reg_read(UC_X86_REG_ESP)
    st["fades"].append(bytes(e.uc.mem_read(a, 0x400)))
    if d != 0:
        log(f"fade dest {d}")
    return log(f"fade {b} {c} {e.r32(sp + 4)}")


def answer(em, *_):
    st["shown"].append(bytes(em.uc.mem_read(screen, 64000)))
    log("ask")
    st["at"] += 1
    return st["tape"][st["at"] - 1]


def exists(em, a, *_):
    return int(e.cstr(a).decode().upper() in st["present"])


def unlink(em, a, *_):
    st["gone"].append(e.cstr(a).decode().upper())
    return 0


_pics = {}


def load(em, eax, edx, ebx, ecx):
    n = e.cstr(ebx).decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    w, h, px = _pics[n]
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for addr in (0x678E8, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x67E18, 0x67E30, 0x681D0):
    e.stub(rt(addr), lambda em, *_: 0)
for addr, fn, pop in ((0x758AD, fread, 0), (0x757B1, fseek, 0), (0x7648C, fgetc, 0), (0x52908, fade, 4), (0x532BC, load, 4), (0x2538C, answer, 0),
                      (0x4707C, exists, 0), (0x75D04, unlink, 0),
                      (0x67F90, lambda em, *_: log("flush"), 0),
                      (0x674B0, lambda em, a, *_: log(f"shape {a}"), 0),
                      (0x67BF0, lambda em, *_: log("hide"), 0),
                      (0x673AC, lambda em, *_: log("show"), 0)):
    e.stub(rt(addr), fn, pop=pop)
r = random.Random(1105)
bad = cases = deletes = 0


def report(c, msg):
    global bad
    bad += 1
    if bad <= 15:
        print(f"case {c}: {msg}")


TAPES = ([1], [0, 1], [0, 0, 1], [0, 0, 0, 1], [0, 0, 0, 0, 0, 0, 1], [0, 0, 0, 0, 1])
for c in range(ncases):
    tape = TAPES[c % len(TAPES)]
    present = {f for f in FILES if r.random() < 0.7}
    st.update(log=[], fades=[], shown=[], at=0, tape=tape, present=present, gone=[], reads=0, pos=0)
    e.uc.mem_write(screen, b"\x55" * 64000)
    e.w8(rt(0x92F90), 0)
    try:
        ret = e.call(rt(0x4FF00), max_steps=60_000_000)
    except EmuStop as x:
        report(c, f"original stopped: {x}")
        continue
    cases += 1
    if not st["reads"]:
        report(c, "the original read no palette")
    if st["at"] != len(tape):
        report(c, f"the original took {st['at']} answers of {len(tape)}")
    if bytes(e.uc.mem_read(screen, 64000)) != bytes(64000):
        report(c, "the original screen is not black at the end")
    lg = st["log"]
    # The pointer calls inside the boxes stay on the original side.
    if lg[:4] != ["fade 24 0 256", "flush", "shape 0", "show"] or lg[-2:] != ["hide", "fade 24 0 256"]:
        report(c, f"calls {lg}")
    orig = lg[:2] + lg[3:4] + [x for x in lg[4:-2] if x not in ("hide", "show")] + lg[-2:]
    with tempfile.TemporaryDirectory() as d:
        for f in present:
            open(os.path.join(d, f), "wb").close()
        arr = (ctypes.c_int * len(tape))(*tape)
        pal, boxes, text = ctypes.create_string_buffer(768), ctypes.create_string_buffer(64000 * 8), ctypes.create_string_buffer(4096)
        pret = PORT.ja1f_continue_prompt(ISO, EXE, d.encode(), arr, len(tape), pal, boxes, 8, text, 4096)
        left = set(os.listdir(d))
    if pret not in (0, 1):
        report(c, f"port returned {pret}")
        continue
    if pret != ret:
        report(c, f"answer: original {ret} port {pret}")
    # The port logs the delete as a call; the original side shows it by its unlink_ calls.
    orig_del = list(orig)
    port = [x for x in text.value.decode().split("\n") if x != "delete"]
    if orig_del != port:
        report(c, f"steps: original {orig_del} port {port}")
    if set(st["gone"]) != present - left:
        report(c, f"files gone: original {st['gone']} port {present - left}")
    deletes += bool(st["gone"])
    rgb = bytes(st["fades"][0][k] for k in range(0x400) if k % 4 != 3)
    if len(st["fades"]) != 2 or st["fades"][0] != st["fades"][1]:
        report(c, "the original fades use other colours")
    if rgb != pal.raw:
        k = next(j for j in range(768) if rgb[j] != pal.raw[j])
        report(c, f"palette differs at {k}: original {rgb[k]} port {pal.raw[k]}")
    for i, px in enumerate(st["shown"]):
        o = boxes.raw[i * 64000:(i + 1) * 64000]
        if px != o:
            q = [j for j in range(64000) if px[j] != o[j]]
            report(c, f"box {i}: {len(q)} pixels, first ({q[0] % 320},{q[0] // 320}) original {px[q[0]]} port {o[q[0]]}")
print(f"continue_diff: {cases} cases, {deletes} with a delete, {bad} mismatches")
sys.exit(1 if bad or cases < ncases else 0)
