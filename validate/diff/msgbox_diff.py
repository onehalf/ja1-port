"""Differential test: the message box (panel.md "Message box"), original vs port.

usage: .venv/bin/python validate/diff/msgbox_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: a text (the island map's messages first, then fuzzed words: short / long words, words wider than the wrap
width, double and leading / trailing spaces, the 0xB1 line-break word), a darkening table (32 = the strategic
screen, 31, or another one except 28, which depends on the tileset) and a noise screen. The original runs
StatusMessage? 0x27D24 in Unicorn on tac60 (byte 0x284F08 = the table); the screen is taken at the wait 0x67E44
(which then answers "key pressed") and must be back to the noise after when every changed pixel lies inside the
saved rectangle. The port
(port/ja1front ja1f_message_box) draws the box on the same noise; the 320 x 200 screens are compared.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_message_box.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32, ctypes.c_void_p]
    PORT.ja1f_message_box.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MESSAGES = [
    "This sector is in enemy hands!", "This sector is not linked!", "No one on duty!", "No Mercenaries there!",
    "Starting sector already specified!", "No Tappers there!", "Not enough workers available!",
    "There are no trees there!", "Tappers are too scared to work there!", "No more tappers will work there!",
    "No Guards there!", "No workers available!", "Guard capacity reached!", "Starting sector not specified!",
    "Costs are higher than you can afford!",
    "You may want to visit the ASSIGNMENT screen where you can outfit your mercs.",
    "To do this, click on the RAISED team cost button in the lower left area of the screen.",
    "Natives' salary is at its maximum!", "Fidel seems to have disappeared!",
]
TABLE_P, SCREEN_P = 0x284F08, 0x95678

e = Emu(dump=dump, fast=True)
for a in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040):   # mouse, frame, timer: nothing
    e.stub(e.rt(a), lambda em, *_: None)
held = []
screen = e.r32(e.rt(SCREEN_P))
e.stub(e.rt(0x67E44), lambda em, *_: held.append(bytes(em.uc.mem_read(screen, 64000))) or 1)
TEXT = e.alloc(1024)


def fuzz(r):
    words = []
    for _ in range(r.choice([1, 2, 3, 5, 8, 12, 20, 30])):
        k = r.random()
        if k < 0.04:
            words.append("\xb1")
        elif k < 0.08:
            words.append("".join(r.choice("MW@#") for _ in range(r.randint(20, 40))))    # wider than 189
        else:
            words.append("".join(chr(r.randint(0x21, 0x7E)) for _ in range(r.choice([1, 2, 3, 4, 6, 9]))))
    sep = lambda: " " * r.choice([1, 1, 1, 1, 2, 3])
    s = sep().join(words)
    if r.random() < 0.2:
        s = " " * r.randint(1, 3) + s
    if r.random() < 0.2:
        s += " " * r.randint(1, 3)
    return s.encode("latin-1")


r = random.Random(1)
bad = 0
cases = [(m.encode(), 32) for m in MESSAGES] + [(fuzz(r), r.choice([32, 32, 31] + [t for t in range(34) if t != 28]))
                                                 for _ in range(max(0, ncases - len(MESSAGES)))]
restored = 0
for i, (s, table) in enumerate(cases):
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    e.uc.mem_write(TEXT, s + b"\0")
    e.w8(e.rt(TABLE_P), table)
    held.clear()
    e.call(e.rt(0x27D24), TEXT)
    after = bytes(e.uc.mem_read(screen, 64000))
    if len(held) != 1:
        print(f"case {i}: the wait ran {len(held)} times")
        bad += 1
        continue
    o = held[0]
    ch = [n for n in range(64000) if o[n] != base[n]]
    if ch and all(37 <= n // 320 <= 116 and 52 <= n % 320 <= 267 for n in ch):
        restored += 1
        if after != base:
            print(f"case {i}: the screen after the box is not the screen before")
            bad += 1
    if PORT:
        buf = ctypes.create_string_buffer(base, 64000)
        rc = PORT.ja1f_message_box(ISO, EXE, s, table, buf)
        p = buf.raw
        if rc != 0 or p != o:
            n = next((k for k in range(64000) if p[k] != o[k]), -1)
            print(f"MISMATCH case {i} table {table} {s[:60]!r}: rc {rc}, screen @({n % 320},{n // 320}) "
                  f"{o[n]:#x} vs {p[n]:#x}" if n >= 0 else f"MISMATCH case {i}: rc {rc}")
            bad += 1
            if bad > 10:
                break
print(f"msgbox_diff: {len(cases)} cases ({restored} restored checked), {bad} mismatches"
      + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
