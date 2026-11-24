#!/usr/bin/env python3
"""Differential test: the theme change. SetMusic 0x5C5EC, MusicStopAtBar 0x5C78C, MusicBarCheck 0x5C724, StopMusic 0x5C82C
against Music::play / stop_theme (music.rs, the code the window runs); docs/spec/music.md "Who plays what".

usage: .venv/bin/python validate/diff/setmusic_diff.py [N] [--lib SO]
The original's bodies run in Unicorn on the loaded image, with only the AIL calls and MousePoll stubbed. Each case is a list
of random steps: play theme t (-2, -1, 0, in the file, past the end of the file; stop-first word 0 or 1), stop, the Music
option on or off. Fuzzed bars for 0x7AE9F (the bar check) and fuzzed AIL_sequence_status values (4 = playing). The
compare after each step: playing (high word of 0x9635C), wanted (0x96360), the theme that started. Driver word 0x95DC0 is
the FM value 0, as in the GOG install (live 2026-11-02).
"""
import ctypes
import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 400
PORT = ctypes.CDLL(LIB)
PORT.ja1f_music_ops.argtypes = [ctypes.c_char_p, ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

SET_MUSIC, STOP_MUSIC, BAR_CHECK = 0x5C5EC, 0x5C82C, 0x5C724
AIL_INIT, AIL_START, AIL_END, AIL_STATUS, AIL_VOLUME, AIL_POS, MOUSE = 0x79D69, 0x79E62, 0x79FA9, 0x7A190, 0x7A098, 0x7AE9F, 0x678E8
DRIVER, OPTION, GM_WORD, THEMES, WANTED, LAST_BAR = 0x9639A, 0x1163B9, 0x95DC0, 0x9635C, 0x96360, 0x92E54
STEPS = 12


def i16(b):
    return int.from_bytes(b, "little", signed=True)


class Orig:
    def __init__(self):
        self.e = e = Emu(fast=True)
        self.count = 0
        self.rng = random.Random(1)
        self.started = -1
        self.pending = 0

        def arg(em, k):
            sp = em.uc.reg_read(UC_X86_REG_ESP)
            return int.from_bytes(em.uc.mem_read(sp + 4 + 4 * k, 4), "little", signed=True)

        def init(em, eax, edx, ebx, ecx):
            t = arg(em, 2)
            self.pending = t
            return 1 if 0 <= t < self.count else self.rng.choice((0, -1))

        def start(em, eax, edx, ebx, ecx):
            self.started = self.pending

        def status(em, eax, edx, ebx, ecx):
            return self.rng.choice((4, 4, 2, 0))

        def pos(em, eax, edx, ebx, ecx):
            # bars: 0, 1, 7, huge; beats go to the first pointer, the bar to the second (MusicBarCheck compares [esp])
            self.bar = self.rng.choice((0, 1, 7, 0x7FFFFFFF, 0xFFFFFFFF))
            em.uc.mem_write(arg(em, 1) & 0xFFFFFFFF, self.rng.choice((0, 3)).to_bytes(4, "little"))
            em.uc.mem_write(arg(em, 2) & 0xFFFFFFFF, self.bar.to_bytes(4, "little"))

        e.stub(AIL_INIT, init)
        e.stub(AIL_START, start)
        e.stub(AIL_END, lambda *a: None)
        e.stub(AIL_STATUS, status)
        e.stub(AIL_VOLUME, lambda *a: None)
        e.stub(AIL_POS, pos)
        e.stub(MOUSE, lambda *a: None)

    def reset(self):
        u = self.e.uc
        u.mem_write(DRIVER, b"\1")
        u.mem_write(OPTION, b"\1")
        u.mem_write(GM_WORD, b"\0\0")
        u.mem_write(THEMES, (0xFFFF009B).to_bytes(4, "little"))     # the image's start values
        u.mem_write(WANTED, b"\xFF\xFF")
        u.mem_write(LAST_BAR, b"\xFF\xFF\xFF\xFF")

    def step(self, op):
        self.started = -1
        k, a, b = op
        if k == 0:
            self.e.call(SET_MUSIC, a & 0xFFFFFFFF, b)
        elif k == 1:
            self.e.call(STOP_MUSIC)
        else:
            self.e.uc.mem_write(OPTION, bytes([a]))
        u = self.e.uc
        return i16(u.mem_read(THEMES + 2, 2)), i16(u.mem_read(WANTED, 2)), self.started


def main():
    rng = random.Random(12345)
    o = Orig()
    probe = (ctypes.c_int * 3)(0, 0, 0)
    o.count = PORT.ja1f_music_ops(EXE, (ctypes.c_int * 3)(1, 0, 0), 1, probe)
    if o.count < 2:
        print(f"setmusic_diff: FAIL DAY has {o.count} sequences")
        sys.exit(1)
    bad = steps = starts = 0
    for c in range(ncases):
        ops = []
        for _ in range(STEPS):
            k = rng.choice((0, 0, 0, 0, 0, 1, 2))
            if k == 0:
                ops.append((0, rng.choice((-2, -1, 0, 1, 1, 2, 2, o.count - 1, o.count, o.count + 3)), rng.choice((0, 0, 1))))
            elif k == 1:
                ops.append((1, 0, 0))
            else:
                ops.append((2, rng.choice((0, 1, 1)), 0))
        flat = (ctypes.c_int * (3 * STEPS))(*[x for op in ops for x in op])
        out = (ctypes.c_int * (3 * STEPS))()
        PORT.ja1f_music_ops(EXE, flat, STEPS, out)
        o.reset()
        for i, op in enumerate(ops):
            want = o.step(op)
            got = tuple(out[3 * i:3 * i + 3])
            steps += 1
            if bytes(o.e.uc.mem_read(LAST_BAR, 4)) != b"\xFF\xFF\xFF\xFF":
                bad += 1
                print(f"FAIL case {c} step {i} {op}: the stored bar left -1, the port has no bar wait")
                break
            if op[0] == 0 and o.e.call(BAR_CHECK) != (0 if o.bar == 0xFFFFFFFF else 1):
                bad += 1
                print(f"FAIL case {c} step {i}: MusicBarCheck, bar {o.bar:#x}")
                break
            starts += want[2] >= 0
            if want != got:
                bad += 1
                if bad <= 12:
                    print(f"FAIL case {c} step {i} {op}: original playing, wanted, started {want}, port {got}; before {ops[:i][-3:]}")
                break
    print(f"setmusic_diff: {steps} steps, {ncases} cases, {starts} starts, {bad} mismatches")
    sys.exit(1 if bad or not starts else 0)


if __name__ == "__main__":
    main()
