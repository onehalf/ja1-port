#!/usr/bin/env python3
"""effslot_diff.py [N] [--seed S] — host op 65 effect_slot (sound.md "Requests") of the original vs the port.

The original: 0x17B70 (AL slot k, DL kind c) in Unicorn on the plain image, with fuzzed fields of record k
(u32 +0xC0 voice timer, +0x16C, s8 +0x183 health, s8 +0x18A energy, +0x1E0); PlaySfxSlot 0x5CB34 is stubbed and
logged (effect, rate, volume, loops). After the call: u32 +0xC0 of record k.
The port: `ja1front --effect-slot CASES` (sound.rs effect_slot). Exits 1 on any mismatch.
"""
import os, random, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1-port")
FN, SOL = 0x17B70, 0xFA958

args = [a for a in sys.argv[1:] if not a.startswith("--")]
N = int(args[0]) if args else 3000
seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1
s32 = lambda v: v - (1 << 32) if v & 0x80000000 else v


def make_case(r):
    k = r.choice([r.randrange(8), r.randrange(8), r.randrange(8, 48)])
    c = r.choice([0, 1, 1, 2, 3, r.randrange(256)])
    timer = r.choice([0, 0, 400, r.randrange(-20, 20), -1, r.randrange(1 << 32) - (1 << 31)])
    b16c = r.choice([0, 0, 0, 1, r.randrange(256)])
    hp = r.choice([r.randrange(0, 101), r.randrange(256), 49, 50])
    en = r.choice([r.randrange(0, 101), r.randrange(256), 59, 60, 100])
    b1e0 = r.choice([r.randrange(4), r.randrange(256)])
    return k, c, timer, b16c, hp, en, b1e0


def run_original(e, case):
    k, c, timer, b16c, hp, en, b1e0 = case
    rec = SOL + 512 * k
    log = []
    e.w32(rec + 0xC0, timer)
    e.w8(rec + 0x16C, b16c)
    e.w8(rec + 0x183, hp)
    e.w8(rec + 0x18A, en)
    e.w8(rec + 0x1E0, b1e0)
    e.stub(0x5CB34, lambda em, a, d, b, cc: log.append(f"sfx {a} {s32(d)} {s32(b)}") or 0)
    e.call(FN, k, c)
    log.append(f"timer {s32(e.r32(rec + 0xC0))}")
    return log


def main():
    r = random.Random(seed)
    cases = [make_case(r) for _ in range(N)]
    e = Emu(fast=True)
    orig = [run_original(e, c) for c in cases]
    path = os.path.join(ROOT, "build", "effslot_cases.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as fh:
        for c in cases:
            fh.write(" ".join(map(str, c)) + "\n")
    p = subprocess.run([FRONT, "--effect-slot", path], capture_output=True, text=True, timeout=120)
    port, k = [], []
    for l in p.stdout.splitlines():
        if l.startswith("none"):
            continue
        k.append(l)
        if l.startswith("timer"):
            port.append(k)
            k = []
    bad, plays = 0, 0
    for n, (o, q) in enumerate(zip(orig, port)):
        plays += any(l.startswith("sfx") for l in o)
        if o != q:
            bad += 1
            if bad <= 3:
                print(f"case {n} {cases[n]}: original {o} port {q}")
    if len(port) != len(orig):
        print(f"port answered {len(port)} of {len(orig)} cases ({p.stderr.strip()[-200:]})")
        bad += 1
    print(f"effslot_diff: {len(cases)} cases, {plays} sounds, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
