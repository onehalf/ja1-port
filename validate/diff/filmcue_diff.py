#!/usr/bin/env python3
"""filmcue_diff.py [N] [--seed S] — the film cue step (ending.md "Film cues") of the original vs the port.

The original: the film callback 0x5BE18 (called by FlcPlay 0x5BC58 after each frame) in Unicorn on the plain image,
with a fuzzed cue table 0xA5448 (200 bytes) + cue voice names 0xA5510 (120 bytes), cue rate word 0x96370, cue voice
volume word 0x337948, counters 0x1163B4 (i) / 0x1163B0 (j) = 0, and the frame counter word +0x9A of a film struct.
Per case a list of frame counters c (the real order 0, 1, .., N - 2, the ring frame N - 1 and the cue index reset
that FlcPlay does at a pass end, or random c). Stubs (logged): KeyPressed 0x5616C = no key, PlaySfxSlot 0x5CB34,
FillRect 0x52AB4 (under the clear helper 0x5EBC0), the subtitle text 0x5EBE8 (op 29's routine, tested by sab_diff),
ArcSeek 0x534A4 (found = the name is a member of INTRO\\INTROSP), SpeechStream 0x5CC6C, SetMusic 0x5C5EC, the speech
pump 0x5CCF8. After each callback: c, i, j.
The port: `ja1front ISO EXE --film-cue CASES` (sab.rs Sab::film_cue with the LogIo log). Exits 1 on any mismatch.
"""
import os, random, struct, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1front")
ISO, EXE = os.path.join(ROOT, "rom/JA1.gog"), os.path.join(ROOT, "build/game113b/JA.EXE")
CB, FILM, TAB, NAMES, I, J, RATE, VOL, SCREEN = (0x5BE18, 0x28EF44, 0xA5448, 0xA5510, 0x1163B4, 0x1163B0, 0x96370,
                                                0x337948, 0x95678)

args = [a for a in sys.argv[1:] if not a.startswith("--")]
N = int(args[0]) if args else 300
seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
snd = open(os.path.join(ROOT, "rom/INTRO/INTROSP"), "rb").read()
MEMBERS = [snd[0x20 + 32 * i:0x30 + 32 * i].split(b"\0")[0] for i in range(struct.unpack_from("<I", snd, 0x14)[0])]
MEMBERS = [m for m in MEMBERS if m]
s16 = lambda v: (v & 0xFFFF) - 0x10000 if v & 0x8000 else v & 0xFFFF
s32 = lambda v: v - (1 << 32) if v & 0x80000000 else v


def make_case(r):
    """(rate, vol, 320 table bytes, frame counters)"""
    n = r.randrange(3, 60)
    ents, voices = [], 0
    for _ in range(r.randrange(0, 20)):
        kind = r.randrange(6)
        a = [r.randrange(-3, 99), 99, r.randrange(100, 200), r.randrange(200, 330), r.randrange(231, 238),
             r.randrange(-40, 400)][kind]
        if 100 <= a < 200:
            if voices >= 10:
                a = 99
            voices += 1
        f = 9998 if r.random() < 0.08 else r.randrange(-2, n + 3)
        ents.append((f, a))
    if r.random() < 0.7:
        ents.sort(key=lambda e: e[0] if e[0] != 9998 else r.randrange(n))
    ents += [(9999, r.choice([0, 0, 5]))] * (50 - len(ents))
    tab = b"".join(struct.pack("<hh", f, a) for f, a in ents)
    names = b"".join((r.choice(MEMBERS) if r.random() < 0.6 else (b"" if r.random() < 0.4 else b"NOPE%d.8" % r.randrange(9)))
                     .ljust(12, b"\0") for _ in range(10))
    if r.random() < 0.7:
        cs = []
        loops = r.randrange(1, 3)
        for p in range(loops):
            cs += list(range(n - 1))
            if p < loops - 1:
                cs += [n - 1, -1]
    else:
        cs = [r.randrange(-1, n + 3) for _ in range(r.randrange(1, 40))]
    cs = cs[:70]
    return r.choice([11025, 22050, 8000]), r.choice([0, 90, 127, 40]), tab + names, cs


def run_original(e, case):
    rate, vol, tab, cs = case
    log = []
    e.uc.mem_write(TAB, tab)
    e.w16(RATE, rate)
    e.w16(VOL, vol)
    e.w32(I, 0)
    e.w32(J, 0)
    screen = e.r32(SCREEN)
    found = [True]

    def arcseek(em, eax, edx, ebx, ecx):
        name = e.cstr(ebx)
        log.append("voice " + name.decode("latin-1"))
        found[0] = name.upper() in [m.upper() for m in MEMBERS]
        return 0 if found[0] else 1

    e.stub(0x5616C, lambda *_: 0)
    e.stub(0x5CB34, lambda em, a, d, b, c: log.append(f"sfx_play {s32(a)} {s32(d)} {b & 0xFF} {s32(c)}"))
    e.stub(0x52AB4, lambda em, a, d, b, c: log.append(f"clear {a - screen} {d} {b} {c} {e.r32(e.sp() + 4)}"), 4)
    e.stub(0x5EBE8, lambda em, a, d, b, c: log.append(f"subtitle {s16(a)} {s16(d)} {s16(b)}"))
    e.stub(0x534A4, arcseek, 4)
    e.stub(0x5CC6C, lambda em, a, d, b, c: log.append(f"speech {s32(a)} {s32(d)}"))
    e.stub(0x5C5EC, lambda em, a, d, b, c: log.append(f"music {s32(a)} {s32(d)}"))
    e.stub(0x5CCF8, lambda *_: 0)
    for c in cs:
        if c == -1:
            e.w32(I, 0)
            continue
        e.w16(FILM + 0x9A, c)
        e.call(CB, FILM)
        log.append(f"cue {c} {e.r32(I)} {e.r32(J)}")
    log.append("case")
    return log


def main():
    r = random.Random(seed)
    cases = [make_case(r) for _ in range(N)]
    # the three real cue files of the intro, played as one pass of 160 frames
    for f in ("HELI.BIN", "SANTALK.BIN", "SANTINON.BIN"):
        cases.append((11025, 0, open(os.path.join(ROOT, "rom/INTRO", f), "rb").read(), list(range(160))))
    e = Emu(fast=True)
    if not hasattr(e, "sp"):
        from unicorn.x86_const import UC_X86_REG_ESP
        e.sp = lambda: e.uc.reg_read(UC_X86_REG_ESP)
    orig = []
    for c in cases:
        orig.append(run_original(e, c))
    path = os.path.join(ROOT, "build", "filmcue_cases.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as fh:
        for rate, vol, tab, cs in cases:
            fh.write(f"{rate} {vol} {tab.hex()} {','.join(map(str, cs))}\n")
    p = subprocess.run([FRONT, ISO, EXE, "--film-cue", path], capture_output=True, text=True, timeout=120)
    port = p.stdout.splitlines()
    pc, k = [], []
    for l in port:
        k.append(l)
        if l == "case":
            pc.append(k)
            k = []
    bad = 0
    acts, kinds = 0, {}
    for n, (o, q) in enumerate(zip(orig, pc)):
        for l in o:
            if not l.startswith(("cue", "case")):
                acts += 1
                kinds[l.split()[0]] = kinds.get(l.split()[0], 0) + 1
        if o != q:
            bad += 1
            if bad <= 3:
                i = next(i for i in range(min(len(o), len(q)) + 1) if i >= len(o) or i >= len(q) or o[i] != q[i])
                print(f"case {n}: line {i}: original {o[max(0, i - 2):i + 3]} port {q[max(0, i - 2):i + 3]}")
    if len(pc) != len(orig):
        print(f"filmcue_diff: port printed {len(pc)} cases, original {len(orig)}")
        bad += 1
    print(f"filmcue_diff: {len(cases)} cases ({N} fuzzed + 3 intro cue files), {acts} cue actions "
          f"({', '.join(f'{k} {v}' for k, v in sorted(kinds.items()))}), {bad} mismatches")
    sys.exit(1 if bad else 0)


main()
