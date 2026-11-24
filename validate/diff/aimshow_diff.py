"""Differential test: the whole A.I.M. picture at its opening, ShowAimScreen 0x1FEB0, original vs port.

usage: .venv/bin/python validate/diff/aimshow_diff.py [ncases=100] [DUMP] [--port] [--lib SO]
The original runs whole in Unicorn on an A.I.M. dump: AimDrawFunds? 0x513E4, the team faces 0x21B3C and 0x21198,
AimBrowse 0x221B0 with the page 0x21648, the labels 0x35AB8 and the palette copy 0x52820 are real. MAIN.DAT comes
through a stubbed file layer: the picture loader 0x532BC is stubbed, the palette loader 0x53300 runs with fseek_,
fgetc_ and fread_ on the archive. LoadMercFace 0x286C4 (real, its files stubbed) fills the face buffers of the team; the buffers of empty slots hold 0. Per case: a team of 0..8 mercs (name, on duty byte), balance, page profile, view, argument EAX.
Compared with aim::show_test (ja1f_aim_show, the real Aim::show and Aim::picture): the 320 x 200 screen, the palette,
the mode byte 0x95F6C and profile P. The original also shows: EAX = 1 starts theme 3 and EAX = 0 keeps the music
(checked here on the original side, the window's Aim::new starts theme 3). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx, common           # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 100
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_aim_hired.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
BIO = common.game_read("EDT/BIOQUOTE.EDT")
PROFILES, SOLDIERS, PROFILE, SOLDIER, ACC, FACES = 0xEBB40, 0xFA958, 0x5A, 0x200, 0x229FA0, 0x93428
SHOT = 64000
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen = e.r32(e.rt(0x95678))
FP = 0x7F00
cur = {"data": b"", "pos": 0}
music = []
files = {}


def load(em, eax, edx, ebx, ecx):
    name = em.cstr(ebx).decode("latin-1").upper()
    try:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, name))
    except Exception:
        return 0
    pitch = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        em.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def arcseek(em, eax, edx, ebx, ecx):
    """ArcSeek 0x534A4: the member's size goes to the out pointer (argument on the stack); the file stands at its start."""
    name = em.cstr(ebx).decode("latin-1").upper()
    cur["data"], cur["pos"] = datarc.member(MAIN, name), 0
    out = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4)
    em.w32(out, len(cur["data"]))
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


def bio(em, eax, *_):
    em.uc.mem_write(em.rt(0xED1C0), BIO[eax * 320:eax * 320 + 320])
    return 0


def open_cd(em, eax, *_):
    name = em.cstr(eax).decode("latin-1").upper()
    files.setdefault(name, None)
    if files[name] is None:
        files[name] = common.game_read(name)
    cur["data"], cur["pos"] = files[name], 0
    em.w32(em.rt(0x92F54), FP)
    return FP


for a in (0x67BF0, 0x673AC, 0x67C0C, 0x67478, 0x67F90, 0x38190):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x52908), lambda em, *_: 0, 4)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x534A4), arcseek, 4)
e.stub(e.rt(0x38304), open_cd)
e.stub(e.rt(0x75A77), lambda em, *_: 0)
e.stub(e.rt(0x757B1), fseek)
e.stub(e.rt(0x7648C), fgetc)
e.stub(e.rt(0x758AD), fread)
e.stub(e.rt(0x234E0), bio)
e.stub(e.rt(0x5C82C), lambda em, *_: music.append("stop") or 0)
e.stub(e.rt(0x5C5EC), lambda em, eax, edx, *_: music.append(("play", eax, edx)) or 0)
r = random.Random(41)
bad = teams = 0
ascii_ = lambda n: bytes(r.randint(0x41, 0x7A) for _ in range(n))
for i in range(ncases):
    n = r.choice([0, 1, 2, 3, 4, 5, 6, 7, 8])
    teams += n
    so = bytearray(8 * SOLDIER)
    e.w8(e.rt(0x9344B), 0)      # FACES.PCX is not in the panel buffer (ShowAimScreen clears the flag too)
    for k in range(8):
        e.uc.mem_write(e.r32(e.rt(FACES + 4 * k)), bytes(1024))
        s = bytearray(e.uc.mem_read(e.rt(SOLDIERS + k * SOLDIER), SOLDIER))
        s[0x182] = 0
        if k < n:
            s[0x182] = 1
            s[0x179] = r.randrange(60)
            s[0x16E:0x177] = ascii_(r.randint(1, 8)).ljust(9, b"\0")
            s[0x19D] = r.randrange(2)
        s[0x183] = 50
        e.uc.mem_write(e.rt(SOLDIERS + k * SOLDIER), bytes(s))
        so[k * SOLDIER:(k + 1) * SOLDIER] = s
        if k < n:
            e.call(e.rt(0x286C4), k, s[0x179])
    p = r.randrange(60)
    e.w8(e.rt(ACC + 0xA4), p)
    balance = r.choice([0, 7, 1234, 99999, r.randrange(100000)])
    e.w32(e.rt(ACC + 0x24), balance)
    view = i & 1
    e.w8(e.rt(0x93D9F), 1 - view)
    e.uc.mem_write(e.rt(0x966D5), e.cstr(e.rt(0x99D5D if view else 0x99D43)) + b"\0")   # as ButtonAt? 0x67CD8
    e.w8(e.rt(0x93EA8), 0)
    arg = i % 3 == 0
    music.clear()
    e.call(e.rt(0x1FEB0), int(arg), max_steps=80_000_000)
    assert music == ([("stop"), ("play", 3, 0)] if arg else []), music
    o = bytes(e.uc.mem_read(screen, SHOT))
    opal = bytes(e.uc.mem_read(e.rt(0xFA608), 768))
    mode, op = e.r8(e.rt(0x95F6C)), e.r8(e.rt(ACC + 0xA4))
    if not PORT:
        continue
    b, bp = ctypes.create_string_buffer(SHOT), ctypes.create_string_buffer(768)
    profs = bytes(e.uc.mem_read(e.rt(PROFILES), 60 * PROFILE))
    acc = bytes(e.uc.mem_read(e.rt(ACC), 0x200))
    m = PORT.ja1f_aim_show(ISO, EXE, profs, bytes(so), acc, len(acc), view, b, bp)
    if m != mode | op << 8:
        print(f"case {i}: mode/P port {m & 255}/{m >> 8} original {mode}/{op}")
        bad += 1
    if bp.raw != opal:
        print(f"case {i}: palette differs")
        bad += 1
    if b.raw != o:
        d = [q for q in range(SHOT) if b.raw[q] != o[q]]
        k = d[0]
        print(f"case {i} team {n} view {view} p {op}: {len(d)} pixels, first ({k % 320},{k // 320}): port {b.raw[k]} "
              f"original {o[k]}, box x {min(q % 320 for q in d)}..{max(q % 320 for q in d)} y {d[0] // 320}..{d[-1] // 320}")
        bad += 1
    if bad > 10:
        break
print(f"aimshow_diff: {ncases} cases, {teams} team mercs, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
