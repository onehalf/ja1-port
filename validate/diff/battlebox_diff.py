"""Differential test: the battle box (strategic.md "The battle box"), original vs port.

usage: .venv/bin/python validate/diff/battlebox_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: a sector, three counts, a kind (99 or 0..2, sometimes others), a darkening table (byte 0x284F08; not 28,
which depends on the tileset), view offsets 0x229CB8 (0..15) / 0x229CC4 (0..13), a noise back buffer and a noise
screen. The original runs EnemyAttackSectorDlg? 0x3E2D4 in Unicorn on tac60. The port (port/ja1front
ja1f_battle_box) gets the screen as the tactical view shows the back buffer (screen (X, Y) = back buffer
(X - ox + 32, Y - oy + 28) in the box area 52..267 x 37..116, the noise elsewhere); the 320 x 200 screens are
compared (outside the shown part 63..257 x 42..111 the port must keep its input). Without --port only the original runs. Exits 1 on any mismatch.
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
    PORT.ja1f_battle_box.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32] * 6 + [ctypes.c_void_p]
    PORT.ja1f_battle_box.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

TABLE_P, SCREEN_P, BACK, BW, BH, OX, OY = 0x284F08, 0x95678, 0x22A4EF, 384, 200, 0x229CB8, 0x229CC4
e = Emu(dump=dump, fast=True)
e.stub_sprintf()
screen = e.r32(e.rt(SCREEN_P))
r = random.Random(1)
bad = 0
for i in range(ncases):
    s = r.choice([r.randrange(60), r.randrange(256)])
    cnt = [r.choice([0, r.randrange(10), r.randrange(100), r.randrange(256)]) for _ in range(3)]
    kind = r.choice([99, 0, 1, 2, 0, 1, 2, r.randrange(256)])
    table = r.choice([32, 31] + [t for t in range(34) if t != 28])
    ox, oy = r.randrange(16), r.randrange(14)
    back = bytes(r.randrange(256) for _ in range(BW * BH))
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(e.rt(BACK), back)
    e.uc.mem_write(screen, base)
    e.w8(e.rt(TABLE_P), table)
    e.w32(e.rt(OX), ox)
    e.w32(e.rt(OY), oy)
    e.call(e.rt(0x3E2D4), s, cnt[0], cnt[1], cnt[2], kind)
    o = bytes(e.uc.mem_read(screen, 64000))
    out = [n for n in range(64000) if o[n] != base[n] and not (63 <= n % 320 <= 257 and 42 <= n // 320 <= 111)]
    if out:
        print(f"case {i}: the original changed {len(out)} pixels outside the shown part, first ({out[0] % 320},{out[0] // 320})")
        bad += 1
    if PORT:
        inp = bytearray(base)
        for y in range(37, 117):
            for x in range(52, 268):
                inp[y * 320 + x] = back[(y - oy + 28) * BW + x - ox + 32]
        buf = ctypes.create_string_buffer(bytes(inp), 64000)
        rc = PORT.ja1f_battle_box(ISO, EXE, table, s, cnt[0], cnt[1], cnt[2], kind, buf)
        p = buf.raw
        # Outside the shown part 63..257 x 42..111 the port keeps its input (the original's screen keeps the noise).
        exp = bytearray(o)
        for y in range(37, 117):
            for x in range(52, 268):
                if not (63 <= x <= 257 and 42 <= y <= 111):
                    exp[y * 320 + x] = inp[y * 320 + x]
        o = bytes(exp)
        if rc != 0 or p != o:
            n = next((k for k in range(64000) if p[k] != o[k]), -1)
            print(f"MISMATCH case {i} s {s} counts {cnt} kind {kind} table {table} off ({ox},{oy}): rc {rc}, "
                  + (f"screen @({n % 320},{n // 320}) {o[n]:#x} vs {p[n]:#x}" if n >= 0 else "rc"))
            bad += 1
            if bad > 10:
                break
print(f"battlebox_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
