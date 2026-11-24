#!/usr/bin/env python3
"""Differential test of the FM driver (docs/spec/music.md "The FM driver (SBPRO1.MDI)").

The original rom/SBPRO1.MDI runs in Unicorn (mdi_run.py: start, the AIL install reset, a timbre install, then
random MIDI channel messages one by one); the port (port/ja1front fm.rs, ja1f_mdi) gets the same input.
Compared: every register write (time, chip, register, value) and the install result.
Usage: mdi_diff.py [cases] --port (./t check: 80 cases) [--lib PATH] [--msgs N] [--seed S]"""
import ctypes, os, random, struct, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, HERE)
from mdi_run import Driver, AIL_RESET  # noqa: E402

argv = sys.argv[1:]
opt = lambda k, d: argv[argv.index(k) + 1] if k in argv else d  # noqa: E731
LIB = opt("--lib", os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
NMSG = int(opt("--msgs", "300"))
SEED = int(opt("--seed", "1"))
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] not in ("--lib", "--msgs", "--seed"))]
ncases = int(args[0]) if args else 50
if "--port" not in argv:
    sys.exit("mdi_diff.py tests the port only: give --port")
BANK = os.path.join(ROOT, "rom", "MADLAB.AD")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_mdi.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32, ctypes.c_char_p, ctypes.c_int32,
                          ctypes.POINTER(ctypes.c_int32), ctypes.c_void_p, ctypes.c_int32]
PORT.ja1f_mdi.restype = ctypes.c_int32
CAP = 400000
OUT = (ctypes.c_int32 * (4 * CAP))()

ad = open(BANK, "rb").read()
have = []
i = 0
while True:
    p, b, o = struct.unpack_from("<BBI", ad, i)
    i += 6
    if p == 0xFF and b == 0xFF:
        break
    have.append((b, p))
drums = [p for b, p in have if b == 127]
mel = [(b, p) for b, p in have if b != 127]


def gen(rng, n):
    """Random messages: notes (drums on channel 10), note-offs, bank + program changes (some missing), pitch
    bends and the controllers the driver acts on (and some it ignores)."""
    out, held = [], []
    prot = rng.random() < 0.3
    chans = rng.sample(range(16), rng.randint(1, 6)) + [9]
    for _ in range(n):
        c = rng.choice(chans)
        r = rng.random()
        if r < (0.5 if prot else 0.35):
            key = rng.choice(drums) if c == 9 and rng.random() < 0.9 else rng.randint(0, 127)
            out.append((0x90 | c, key, rng.randint(0, 127)))
            held.append((c, key))
        elif r < 0.6 and held:
            c2, key = held.pop(rng.randrange(len(held)))
            out.append((0x80 | c2, key, rng.randint(0, 127)) if rng.random() < .5 else (0x90 | c2, key, 0))
        elif r < 0.68:
            b, p = rng.choice(mel)
            if rng.random() < 0.3:
                p = rng.randint(0, 127)
            out.append((0xB0 | c, 114, b))
            out.append((0xC0 | c, p, 0))
        elif r < 0.76:
            out.append((0xE0 | c, rng.randint(0, 127), rng.randint(0, 127)))
        elif r < 0.78:
            out.append((rng.choice([0xA0, 0xD0]) | c, rng.randint(0, 127), rng.randint(0, 127)))
        else:
            cc = rng.choice([1, 6, 7, 10, 11, 64, 112, 121, 123, 7, 10, 64, 32, 58, 113, 114] + [112] * 6 * prot)
            v = rng.randint(0, 127)
            if cc == 6:
                v = rng.choice([0, 1, 2, 12, 24, 127])
            out.append((0xB0 | c, cc, v))
    return out


def orig(tims, msgs):
    d = Driver()
    d.t = -2
    d.init(ail_reset=False)
    log = [e for e in d.log if e[1] != "B"]
    d.log.clear()
    d.t = -1
    for ch in range(16):
        d.xmit([(s | ch, a, b) for s, a, b in AIL_RESET])
    d.t = 0
    r = d.install(tims)
    for k, x in enumerate(msgs, 1):
        d.t = k
        d.xmit([x])
    return r, log + d.log


def port(tims, msgs):
    tb = bytes(v for b, p in tims for v in (b, p))
    mb = bytes(v for m in msgs for v in (list(m) + [0, 0])[:3])
    inst = ctypes.c_int32()
    n = PORT.ja1f_mdi(BANK.encode(), tb, len(tims), mb, len(msgs), ctypes.byref(inst), OUT, CAP)
    if n < 0:
        sys.exit("ja1f_mdi failed: %d" % n)
    return inst.value, [(OUT[4 * i], "LR"[OUT[4 * i + 1]], OUT[4 * i + 2], OUT[4 * i + 3]) for i in range(n)]


def edge(c):
    """Directed case: a pitch bend near the top of the fold (a = 0x5FF, music.md "a folded into 0..0x5FF"), then
    every high key, on a melodic program (ledger row ja1front:9500)."""
    b, p = mel[c % len(mel)]
    msgs = [(0xB0, 114, b), (0xC0, p, 0), (0xE0, 0, 92 + c % 4)]
    for key in range(84, 128):
        msgs += [(0x90, key, 100), (0x80, key, 0)]
    return msgs, [(b, p)] + [t for t in have if t[0] == 127][:20]


EDGE = 8


def main():
    bad = writes = 0
    for c in range(ncases + EDGE):
        rng = random.Random(SEED * 100003 + c)
        if c >= ncases:
            msgs, tims = edge(c - ncases)
        else:
            msgs = gen(rng, NMSG)
            tims = rng.sample(have, rng.randint(20, 60))
            if rng.random() < 0.2:
                tims.insert(rng.randrange(len(tims)), (1, 126))       # a missing timbre: the install stops there
        r1, a = orig(tims, msgs)
        r2, b = port(tims, msgs)
        writes += len(a)
        if r1 != r2 or a != b:
            bad += 1
            j = next((j for j, (x, y) in enumerate(zip(a, b)) if x != y), min(len(a), len(b)))
            t = (a[j] if j < len(a) else b[j])[0]
            print("case %d install %d/%d: first diff at write %d (message %d %s): orig %s port %s" % (
                c, r1, r2, j, t, " ".join("%02X" % v for v in msgs[t - 1]) if t > 0 else "-", a[j:j + 3], b[j:j + 3]))
            if bad >= 5:
                break
    print("mdi_diff: %d cases x %d messages + %d pitch-edge cases, %d register writes, %d mismatches" % (ncases, NMSG, EDGE, writes, bad))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
