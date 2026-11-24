"""Differential test: the debug pages (Ctrl+P / Shift+left click) and the opponent lists (Ctrl+O) of cursor.md
"Debug keys (level 4)", original vs port.

usage: .venv/bin/python validate/diff/dbgpage_diff.py [ncases=200] [DUMP] [--lib SO]
Per case on the tac60 dump: a random slot (0..15 or 40..47) whose record gets random bytes (+0x17A kept = the slot,
+0x90 pointed at its own bytes +0x1FE so the script word is fuzzed too), random g_callM / g_callE, the current
sector's "enemies" byte, g_known and the remember tables. The original runs DebugShowSoldierFlags 0x6BC28 (it calls
DebugShowSoldier 0x6A964 and DebugShowSoldierAI? 0x6B2BC) in Unicorn with DrawText 0x358D0 / DrawNumber 0x36090
recorded (DrawNumber's text: ltoa, '^' on the left to the digit count), the redraws 0x176C8 / 0x1CB78 and the wait
0x67F34 stubbed; then DebugShowOppList? 0x6CF88 with the message calls 0x27D24 / 0x27CE0 recorded. The port:
ja1f_debug_pages and ja1f_opp_lists. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
P = ctypes.CDLL(LIB)
P.ja1f_debug_pages.argtypes = [ctypes.c_char_p] + [ctypes.c_int32] * 6 + [ctypes.c_void_p, ctypes.c_int32]
P.ja1f_opp_lists.argtypes = [ctypes.c_char_p] * 4 + [ctypes.c_void_p, ctypes.c_int32]

SOL, CALL_M, KNOWN, REM_E, REM_M, SECT, CUR = 0xFA958, 0x92DBA, 0x10D860, 0x10D890, 0x10D9D0, 0x22979C, 0x22A039
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
out = []


def sargs(em, n):
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    return [em.r32(sp + 4 + 4 * i) for i in range(n)]


def text(em, eax, edx, ebx, ecx):
    y, c, fg, sh, font, mode = sargs(em, 6)
    out.append(f"{page[0]} {ecx} {y} {em.cstr(eax).decode('latin-1')}" + ("" if (c, fg, sh, font, mode) == (0, 255, 8, 1, 300)
                                                                         else f" STYLE {c} {fg} {sh} {font} {mode}"))


def number(em, eax, edx, ebx, ecx):
    x, y, fg, sh, font, mode = sargs(em, 6)
    s = str(eax - (1 << 32) if eax >> 31 else eax).rjust(edx & 0xFF, "^")
    out.append(f"{page[0]} {x} {y} {s}" + ("" if (fg, sh, font, mode) == (255, 8, 1, 300) else " STYLE"))


def redraw(kind):
    def f(em, *_):
        page[0] += 1
        out.append(f"{page[0]} redraw {kind}")
    return f


page = [0]
e.stub(e.rt(0x358D0), text, 24)
e.stub(e.rt(0x36090), number, 24)
e.stub(e.rt(0x176C8), redraw("map"))
e.stub(e.rt(0x1CB78), redraw("screen"))
e.stub(e.rt(0x67F34), lambda em, *_: 0)
e.stub(e.rt(0x27D24), lambda em, eax, *_: out.append(em.cstr(eax).decode("latin-1")))
e.stub(e.rt(0x27CE0), lambda em, eax, edx, *_: out.append(em.cstr(eax).decode("latin-1") + str(edx - (1 << 32) if edx >> 31 else edx)))

rng = random.Random(5)
bad = 0
for i in range(ncases):
    slot = rng.choice(list(range(16)) + list(range(40, 48)))
    ra = e.rt(SOL) + 512 * slot
    rec = bytearray(rng.randbytes(512))
    rec[0x17A] = slot
    rec[0x1F3] = rng.randrange(11)          # the animation word stays inside the record (+0x1E8 .. +0x1FD)
    if rng.random() < 0.5:
        rec[0x1B9] = rng.randrange(8)      # AtWaypoint's paths
        if rng.random() < 0.5:
            rec[0x114:0x11A] = rec[0x124:0x12A]
    e.uc.mem_write(ra, bytes(rec))
    e.w32(ra + 0x90, ra + 0x1FE)
    calls = rng.randbytes(8)
    e.uc.mem_write(e.rt(CALL_M), calls)
    cur = e.r8(e.rt(CUR))
    enemies = rng.randrange(256)
    e.w8(e.rt(SECT) + 9 * cur + 1, enemies)
    known, rm, re_ = rng.randbytes(48), rng.randbytes(128), rng.randbytes(320)
    e.uc.mem_write(e.rt(KNOWN), known)
    e.uc.mem_write(e.rt(REM_M), rm)
    e.uc.mem_write(e.rt(REM_E), re_)
    out.clear(); page[0] = 0
    e.call(e.rt(0x6BC28), ra)
    want_p = list(out)
    out.clear()
    e.call(e.rt(0x6CF88), ra)
    want_o = list(out)
    rec = bytes(e.uc.mem_read(ra, 512))
    buf = ctypes.create_string_buffer(65536)
    w16 = lambda b, o: int.from_bytes(b[o:o + 2], "little", signed=True)
    n = P.ja1f_debug_pages(rec, w16(calls, 0), calls[2], w16(calls, 4), calls[6], enemies,
                           int.from_bytes(rec[0x1FE:0x200], "little"), buf, len(buf))
    got_p = buf.value.decode("latin-1").splitlines()
    P.ja1f_opp_lists(rec, known, rm, re_, buf, len(buf))
    got_o = buf.value.decode("latin-1").splitlines()
    for kind, w, g in (("pages", want_p, got_p), ("opp lists", want_o, got_o)):
        if w != g:
            bad += 1
            if bad <= 5:
                d = next(k for k in range(max(len(w), len(g))) if k >= len(w) or k >= len(g) or w[k] != g[k])
                print(f"MISMATCH case {i} slot {slot} {kind} line {d}: original {w[d] if d < len(w) else None!r}"
                      f" port {g[d] if d < len(g) else None!r}")
print(f"dbgpage_diff: {ncases} cases, {bad} mismatches")
sys.exit(1 if bad else 0)
