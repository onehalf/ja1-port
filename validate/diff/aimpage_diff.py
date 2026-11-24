"""Differential test: the A.I.M. merc page (strategic.md "The A.I.M. screen"), original vs port.

usage: .venv/bin/python validate/diff/aimpage_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
AimShowMercBio? 0x21648 runs whole in Unicorn on an A.I.M. dump: AimDrawStats 0x212D8, AimTextPanel 0x214E8,
AimDrawMercItems? 0x2186C, AimDrawTextPanel 0x215BC, the remap, the blits and the texts are real. The loader 0x532BC
reads MAIN.DAT, and 0x234E0 reads the record of EDT/BIOQUOTE.EDT.
Per case: a profile with fuzzed name, nickname, salary, stats, health (0 = dead), teamStatus and kit; a team record
that holds this profile or not; view 0x93D9F (bio or gear). The screen starts as AIMSCRN.PCX.
Compared with aim::page_test (ja1f_aim_page, the page code of the window): the 320 x 200 screen and the mode byte
0x95F6C. Exits 1 on any mismatch.
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
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_aim_hired.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
BIO = common.game_read("EDT/BIOQUOTE.EDT")
PROFILES, SOLDIERS, PROFILE, SOLDIER = 0xEBB40, 0xFA958, 0x5A, 0x200
_w, _h, AIMSCRN, _ = pcx.decode(datarc.member(MAIN, "AIMSCRN.PCX"))
assert (_w, _h) == (320, 200)
SHOT = 64000
e = Emu(dump=dump, fast=True)
screen = e.r32(e.rt(0x95678))


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


def bio(em, eax, *_):
    em.uc.mem_write(em.rt(0xED1C0), BIO[eax * 320:eax * 320 + 320])
    return 0


for a in (0x67BF0, 0x673AC, 0x67C0C):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x5CB34), lambda em, *_: 0)          # PlaySfxSlot (the tooltip sound)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x234E0), bio)
r = random.Random(39)
bad = dead = away = gear = tips = 0
STOCK_NAMES = [bytes(e.uc.mem_read(e.rt(PROFILES + k * PROFILE), 10)) for k in range(60)]
TIP_TEXT = 0x229B1C    # a free text buffer of the original (AimQuoteBox_ text): only for this case
ascii_ = lambda n: bytes(r.randint(0x41, 0x7A) for _ in range(n))
for i in range(ncases):
    pi = r.randrange(60)
    a = e.rt(PROFILES + pi * PROFILE)
    pr = bytearray(e.uc.mem_read(a, PROFILE))
    pr[0:10] = ascii_(r.randint(1, 9)).ljust(10, b"\0")
    pr[0x0A:0x28] = ascii_(r.randint(1, 24)).ljust(30, b"\0")
    pr[0x28:0x2A] = r.randrange(30000).to_bytes(2, "little")    # the original reads it signed; no merc asks above this
    for off in (0x2A, 0x2C, 0x42, 0x2E, 0x40, 0x43, 0x41, 0x2D, 0x44):
        pr[off] = r.randint(1, 100)
    if i % 5 == 1:
        pr[0x2A] = 0
    pr[0x4B] = r.choice([0, 0, 1, 2])
    kit = [0] * 10
    for k in range(r.choice([0, 1, 3, 5, 7, 10])):
        kit[r.randrange(10)] = r.randint(1, 120)
    pr[0x50:0x5A] = bytes(kit)
    e.uc.mem_write(a, bytes(pr))
    # team records: slot 0 holds this profile in a third of the cases; byte 0x19D says it is on duty or not
    so = bytearray(8 * SOLDIER)
    if i % 3 == 0:
        so[0x182] = 1
        so[0x179] = pi
        so[0x19D] = r.randrange(2)
    for k in range(8):
        for off in (0x182, 0x179, 0x19D):
            e.w8(e.rt(SOLDIERS + k * SOLDIER + off), so[k * SOLDIER + off])
    view = i & 1                # the port's view: 0 = bio, 1 = gear; 0x93D9F: 0 = gear
    e.w8(e.rt(0x93D9F), 1 - view)
    e.w8(e.rt(0x93EA8), 0)
    e.uc.mem_write(screen, bytes(AIMSCRN))
    e.call(e.rt(0x467E4), 2, 0x50, 0xF)       # the save areas of AimScreen's first picture
    e.call(e.rt(0x467E4), 5, 0x10, 0x4E)
    e.call(e.rt(0x467E4), 4, 0x118, 0xB9)     # after area 5, as ShowAimScreen: their back buffer rects share a row
    # the page turns while a tooltip of the last page is open (HideTooltip inside 0x21648) and with the modes that only
    # AimScreen writes as the old value: the port draws the whole screen again and must get the same pixels
    e.w8(e.rt(0x95F6C), i % 5)
    if i % 4 == 3:
        # the old page keeps its stock name: a fuzzed wide name leaves pixels outside save area 5 at (16,78), in the
        # original only; all 60 stock nicknames leave none (build/probe run 2026-11-04)
        last = r.randrange(60)
        if last != pi:
            e.uc.mem_write(e.rt(PROFILES + last * PROFILE), STOCK_NAMES[last])
        e.call(e.rt(0x21648), last, max_steps=40_000_000)
        e.w8(e.rt(0x95F6C), r.choice([2, 3]))
        e.uc.mem_write(e.rt(TIP_TEXT), b"Hire at\0\0\0\0\0\0\0\0\0double\0\0\0\0\0\0\0\0\0\0salary\0")
        tips += 1
        if i % 8 == 3:
            e.call(e.rt(0x23264), 9, 9, 0x15, 0x41, 0x47, e.rt(TIP_TEXT), e.rt(TIP_TEXT) + 16, e.rt(TIP_TEXT) + 32, 1)
        else:
            k = r.randrange(8)
            fx, fy = (240 if k % 2 == 0 else 284), 21 + 40 * (k // 2)       # aim.rs face_xy
            e.call(e.rt(0x23264), k + 1, fx, fy, fx + 28, fy + 25, e.rt(TIP_TEXT), e.rt(TIP_TEXT) + 16, e.rt(TIP_TEXT) + 32, 0)
        assert e.r8(e.rt(0x93EA8)) != 0, "the tooltip did not open"
    e.call(e.rt(0x21648), pi, max_steps=40_000_000)
    assert e.r8(e.rt(0x93EA8)) == 0, "0x21648 left the tooltip open"
    o = bytes(e.uc.mem_read(screen, SHOT))
    mode = e.r8(e.rt(0x95F6C))
    dead += pr[0x2A] == 0
    away += mode == 1
    gear += view
    if not PORT:
        continue
    b = ctypes.create_string_buffer(SHOT)
    profs = bytes(e.uc.mem_read(e.rt(PROFILES), 60 * PROFILE))
    m = PORT.ja1f_aim_page(ISO, EXE, profs, bytes(so), pi, view, b)
    if m != mode:
        print(f"case {i} p {pi}: mode port {m} original {mode}")
        bad += 1
    if b.raw != o:
        k = next(q for q in range(SHOT) if b.raw[q] != o[q])
        n = sum(1 for q in range(SHOT) if b.raw[q] != o[q])
        print(f"case {i} p {pi} view {view} health {pr[0x2A]} status {pr[0x4B]} team {i % 3 == 0}/{so[0x19D]}: {n} pixels, first "
              f"({k % 320},{k // 320}): port {b.raw[k]} original {o[k]}")
        bad += 1
    if bad > 10:
        break
print(f"aimpage_diff: {ncases} cases, {dead} dead, {away} away, {gear} gear, {tips} tooltips, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
