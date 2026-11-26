"""Differential test: the credits after the win (docs/spec/ending.md "The credits (after the win)"), original vs port.

usage: .venv/bin/python validate/diff/credits_diff.py [DUMP] [--port] [--png DIR]
The original PlayTitleIntro 0x66604 runs in Unicorn on tac60 with argument 0 (the credits path). Stubs: the sound
archive open/close, OpenFileOrCD, ArcSeek and the read (DAT\\TITLE.DAT members from the CD), the PCX palette read
(fadepal.pcx, 6-bit), the VGA palette set, the music volume and SetMusic (logged), the fade 0x52908 (logged), the
key wait 0x67F48 and the sound stops. EscPressed 0x67300 is patched to `xor eax, eax; ret` and the busy delay
0x67250 to `ret` (native speed: EscPressed runs once per byte of every fade). IN from the VGA status port answers
retrace / no retrace in turn. At each copy of the brightness buffer to the screen (0x66BCD in the fade, 0x66CB6 in
the scroll step) the harness logs `frame CRC` (zlib CRC-32 of the screen). --port: `ja1front ISO EXE --credits` must
print the same frame, fade and music lines. --png DIR writes every 40th frame.
--title: argument 1 (the title part at the program start, ending.md "The title part (program start)"): PlayFilm
0x5BFF8 is stubbed (logged `flc NAME X Y FLAG`, NAME in lower case: the dump holds the names upper-cased by the
run at the program start; the member search ignores case); FilmCueLoad 0x5EDE0 runs with OpenCDFile 0x38304 (logged
`open_cd NAME`), fread_ and fclose_ stubbed on the hard-disk file; at the end the cue rate, the voice volume and the
CRC of the 200-byte cue table are logged (`cue_state`). The port runs `ja1front ISO EXE --title`.
"""
import os, subprocess, sys, zlib
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                                  # noqa: E402
from common import cd_read                           # noqa: E402
from unicorn import UC_HOOK_CODE                     # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP         # noqa: E402
import datarc, pcx                                   # noqa: E402

argv = sys.argv[1:]
PNG = argv[argv.index("--png") + 1] if "--png" in argv else None
args = [a for a in argv if not a.startswith("--") and a != PNG]
dump = args[0] if args else os.path.join(ROOT, "build", "mem_tac60.bin")
FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1-port")
ISO, EXE = os.path.join(ROOT, "rom/JA1.gog"), os.path.join(ROOT, "build/game113b/JA.EXE")
TITLE = cd_read("DAT/TITLE.DAT")

e = Emu(dump=dump, fast=True)
rt = e.rt
log = []
scr = e.r32(rt(0x95678))
arg = lambda k: e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4 * k)  # noqa: E731


def arcseek(em, eax, edx, ebx, ecx):
    data = datarc.member(TITLE, e.cstr(ebx).decode("latin-1").upper())
    e.w32(ecx, 0)
    e.w32(arg(1), len(data))
    return 0


def read(em, eax, edx, ebx, ecx):
    e.uc.mem_write(edx, datarc.member(TITLE, "CREDITS.BIN")[:ebx])
    return 0


def palette(em, eax, edx, ebx, ecx):
    e.uc.mem_write(ecx, bytes(v >> 2 for v in datarc.member(TITLE, "FADEPAL.PCX")[-768:]))
    return 0


def fade(em, eax, edx, ebx, ecx):
    log.append(f"fade {edx} {ebx} {ecx} {arg(1)} {zlib.crc32(bytes(e.uc.mem_read(eax, 1024))):08x}")
    return 0


def music(em, eax, edx, ebx, ecx):
    log.append(f"music {eax} {edx}")
    return 0


frames = []


def frame(uc, addr, size, _):
    p = bytes(uc.mem_read(scr, 64000))
    frames.append(p)
    log.append(f"frame {zlib.crc32(p):08x}")


def wait_ms(em, eax, edx, ebx, ecx):
    log.append(f"wait_ms {eax}")
    return 0


e.stub(rt(0x67F48), wait_ms)
for a in (0x5C0EC, 0x53498, 0x52870, 0x5C858, 0x5C144, 0x5CE08, 0x5C82C):
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x53338), lambda em, *_: 0, 4)
e.stub(rt(0x534A4), arcseek, 4)
e.stub(rt(0x526BC), read)
e.stub(rt(0x53300), palette)
e.stub(rt(0x52908), fade, 4)
e.stub(rt(0x5C5EC), music)
e.uc.mem_write(rt(0x67300), b"\x31\xc0\xc3")
e.uc.mem_write(rt(0x67250), b"\xc3")
e.uc.ctl_flush_tb()
e.uc.mem_write(rt(0xEDA08), bytes(1024))             # the X bytes of the RGBX palette (0x52820 copies R, G, B)
e.w32(rt(0x9635C), 0xFFFF0000 | (e.r32(rt(0x9635C)) & 0xFFFF))   # no theme plays (as after the win)
vs = [0]


def port_in(port):
    vs[0] ^= 8
    return vs[0]


e.port_in = port_in
for a in (0x66BCD, 0x66CB6):
    e.uc.hook_add(UC_HOOK_CODE, frame, None, rt(a), rt(a))
TITLE_MODE = "--title" in argv
if TITLE_MODE:
    cd = {"pos": 0, "data": b""}

    def open_cd(em, eax, edx, ebx, ecx):
        name = e.cstr(eax).decode("latin-1")
        log.append(f"open_cd {name}")
        cd["data"], cd["pos"] = open(os.path.join(ROOT, "rom", name.replace("\\", "/")), "rb").read(), 0
        return 0

    def fread(em, eax, edx, ebx, ecx):
        n = edx * ebx
        e.uc.mem_write(eax, cd["data"][cd["pos"]:cd["pos"] + n])
        cd["pos"] += n
        return ebx

    def film(em, eax, edx, ebx, ecx):
        log.append(f"flc {e.cstr(eax).decode('latin-1').lower()} {edx} {ebx} {ecx}")
        return 0

    e.stub(rt(0x38304), open_cd)
    e.stub(rt(0x758AD), fread)
    e.stub(rt(0x75A77), lambda em, *_: 0)
    e.stub(rt(0x5BFF8), film, 4)
e.call(rt(0x66604), 1 if TITLE_MODE else 0, max_steps=10**12)
log.append(f"end {e.r8(rt(0x33796C))}")
if TITLE_MODE:
    log.append(f"cue_state {e.r16(rt(0x96370))} {e.r16(rt(0x337948))} "
               f"{zlib.crc32(bytes(e.uc.mem_read(rt(0xA5448), 200))):08x}")
if e.stop_reason:
    log.append(f"STOP {e.stop_reason}")
if PNG:
    os.makedirs(PNG, exist_ok=True)
    pal = bytes(min(255, (v >> 2) * 4 + 0) for v in datarc.member(TITLE, "FADEPAL.PCX")[-768:])
    for i in range(0, len(frames), 40):
        pcx.png(os.path.join(PNG, f"credits_{i:04d}.png"), 320, 200, frames[i], pal)
keep = ("frame", "fade", "music", "end", "STOP", "flc", "open_cd", "cue_state", "wait_ms")
o = [x for x in log if x.startswith(keep)]
bad = 0
if not any(x.startswith("end 0") for x in o) or sum(x.startswith("frame") for x in o) < 1000:
    print("credits_diff: the original did not run to the end:", o[-1], len(o))
    bad += 1
elif "--port" in argv:
    p = [x for x in subprocess.run([FRONT, ISO, EXE, "--title" if TITLE_MODE else "--credits"], capture_output=True, text=True).stdout.split("\n")
         if x.startswith(keep)]
    n = next((i for i, (x, y) in enumerate(zip(o, p)) if x != y), None)
    if n is None and len(o) != len(p):
        n = min(len(o), len(p))
    if n is not None:
        bad += 1
        print(f"line {n}: orig {o[n] if n < len(o) else '-'} | port {p[n] if n < len(p) else '-'}")
print(f"credits_diff: {sum(x.startswith('frame') for x in o)} frames, {bad} mismatches" +
      ("" if "--port" in argv else " (original only)"))
sys.exit(1 if bad else 0)
