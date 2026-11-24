"""Differential test: the experience count (strategic.md "The experience count"), original vs port.

usage: .venv/bin/python validate/diff/xpcount_diff.py [ncases=300] [DUMP] [--port] [--lib SO]
Per case: a merc slot k (slot byte k), a stat 0..9, a random field value, a change d (small, big, or a multiple that
takes a big step), a noise panel buffer P and a noise BASSMAP B (the 640-pitch buffer *0x93E5C), a random fast
flag and random Escape presses. The original runs StatCountUp? 0x45624 in Unicorn on tac60 with its drawing for
real (BlitCopy, DrawFrame, DrawNumber); 0x46C1C (P rows 0..145 to the screen) records the picture, the waits
0x67250 / 0x67EEC and ClickSound 0x5C880 are recorded, EscPressed? 0x67300 answers from the case's list. The port
(port/ja1front ja1f_xp_count) runs on the same input. Compared: the event list (shows, waits, clicks), each shown
picture, P at the end and the fast flag. Without --port only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
import xpchange as xp                # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
SHOW = 320 * 146
CAP = 4096
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_xp_count.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_void_p] * 2 + [ctypes.c_int32] * 4 + \
        [ctypes.c_void_p] * 2 + [ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32]
    PORT.ja1f_xp_count.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()
    SHOTS = ctypes.create_string_buffer(SHOW * 1200)
    EVS = (ctypes.c_int32 * (2 * CAP))()

SOL, PANEL_P, BG_P = 0xFA958, 0x93E60, 0x93E5C
e = Emu(dump=dump, fast=True)
rt = e.rt
panel = e.r32(rt(PANEL_P))
bgbuf = e.r32(rt(BG_P))
log, esc = [], []


def show(em, *_):
    log.append(("show", bytes(e.uc.mem_read(panel, SHOW))))
    return 0


def esc_pressed(em, *_):
    return esc.pop(0) if esc else 0


e.stub(rt(0x46C1C), show)
e.stub(rt(0x67250), lambda em, eax, *_: log.append(("wait", eax)) or 0)
e.stub(rt(0x67EEC), lambda em, eax, *_: log.append(("wait", eax)) or 0)
e.stub(rt(0x5C880), lambda em, *_: log.append(("click", 0)) or 0)
e.stub(rt(0x67300), esc_pressed)
FLAG = rt(0x22A404 + 0x100)


def delta(r):
    kind = r.randrange(6)
    if kind == 0:
        return 0
    if kind < 3:
        return r.randint(-25, 25)
    if kind == 3:
        return r.choice([20, 22, 50, 100, 250, 500, 1000, 5000, 10000]) * r.choice([1, -1])
    if kind == 4:
        g = r.choice([2, 5, 10, 25, 50])
        return g * r.randint(-8, 8)
    return r.randint(-120, 120)


def main():
    r = random.Random(7)
    bad = 0
    for case in range(ncases):
        k, stat = r.randrange(8), r.randrange(10)
        s = rt(SOL + 0x200 * k)
        e.w8(s + xp.SLOT, k)
        d = delta(r)
        if stat == 0:
            v = r.choice([r.randrange(65536), r.randrange(3000), r.randrange(100)])
            e.w16(s + xp.SALARY2, v)
            field = s + xp.SALARY2
        else:
            raw = r.choice([r.randrange(256), r.randrange(100), r.randrange(10)])
            field = s + xp.FIELDS[stat][0]
            e.w8(field, raw)
            v = raw - 256 if raw >= 128 else raw
        nvals = abs(d) // abs(xp_step(d)) + 1
        if nvals > 60:
            d = r.randint(-25, 25)
            nvals = abs(d) + 1
        pnl = r.randbytes(64000)
        bg = r.randbytes(320 * 146)
        e.uc.mem_write(panel, pnl)
        for y in range(146):
            e.uc.mem_write(bgbuf + 640 * y, bg[320 * y:320 * y + 320])
        fast = 1 if r.random() < 0.15 else 0
        presses = [1 if r.random() < 0.05 else 0 for _ in range(nvals + 2)]
        e.w8(FLAG, fast)
        log.clear()
        esc[:] = presses
        e.call(rt(0x45624), k, stat, field, d & 0xFFFF, FLAG, max_steps=50_000_000)
        o_panel = bytes(e.uc.mem_read(panel, 64000))
        o_fast = e.r8(FLAG)
        if PORT is None:
            continue
        pb = ctypes.create_string_buffer(pnl, 64000)
        fl = ctypes.c_int32(fast)
        eb = bytes(presses)
        n = PORT.ja1f_xp_count(ISO, EXE, pb, bg, k, stat, v, d, ctypes.byref(fl), eb, len(eb), EVS, CAP, SHOTS, 1200)
        if n < 0:
            print(f"case {case}: port error {n}")
            bad += 1
            continue
        p_log = []
        for i in range(n):
            a, b = EVS[2 * i], EVS[2 * i + 1]
            if a == 1:
                p_log.append(("show", SHOTS.raw[SHOW * b:SHOW * (b + 1)]))
            else:
                p_log.append(("wait" if a == 2 else "click", b))
        why = None
        if [x[0] for x in log] != [x[0] for x in p_log]:
            why = f"events {[x[0][0] for x in log][:40]} vs {[x[0][0] for x in p_log][:40]}"
        else:
            for i, (a, b) in enumerate(zip(log, p_log)):
                if a != b:
                    if a[0] == "show":
                        diff = [j for j in range(SHOW) if a[1][j] != b[1][j]]
                        why = f"show {i}: {len(diff)} pixels, first (x {diff[0] % 320}, y {diff[0] // 320})"
                    else:
                        why = f"event {i}: {a} vs {b}"
                    break
        if why is None and o_panel != pb.raw:
            diff = [j for j in range(64000) if o_panel[j] != pb.raw[j]]
            why = f"panel at the end: {len(diff)} pixels, first (x {diff[0] % 320}, y {diff[0] // 320})"
        if why is None and o_fast != fl.value:
            why = f"fast {o_fast} vs {fl.value}"
        if why:
            bad += 1
            if bad <= 5:
                print(f"MISMATCH case {case} (k {k}, stat {stat}, v {v}, d {d}, fast {fast}): {why}")
    print(f"xpcount_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
    return 1 if bad else 0


def xp_step(d):
    a = abs(d)
    for lo, g in ((10000, 1000), (5000, 500), (1000, 100), (500, 50), (250, 25), (100, 10), (50, 5), (20, 2)):
        if a >= lo and a % g == 0:
            return g
    return 1


if __name__ == "__main__":
    sys.exit(main())
