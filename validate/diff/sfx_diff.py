#!/usr/bin/env python3
"""Differential test: the effect channels. PlaySfxSlot 0x5CB34, ButtonPressSound 0x5C8A0, ButtonReleaseSound 0x5C900,
StopAllSounds 0x5CE08, PlayVocSample 0x5C9C0 against Sound::sfx_chan / button / stop_all / the cry ops (sound.rs, the code the window runs).

usage: .venv/bin/python validate/diff/sfx_diff.py [N] [--lib SO]
The original's bodies run in Unicorn on the loaded image, with only the AIL calls and ReleaseTacticalTimer stubbed; the AIL
stubs keep a status per handle (2 = done, 4 = playing). Random steps: effect (rate, volume incl. above 127, loops), button
press or release, a channel that ends by itself, the two gate bytes, every sound stops, the cry sample. The compare after each step: the
returned channel, and per channel the effect, the rate step, the volume and the loops. All effects hold data: the original
has no test for an empty effect (it starts a zero-length sample), and the port leaves the channel free.
AIL_set_sample_volume's negative values are compared as 0 (the port's Chan clamps; the mixer has no volume below 0).
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
ncases = int(args[0]) if args else 300
PORT = ctypes.CDLL(LIB)
PORT.ja1f_sfx_ops.argtypes = [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p]

PLAY, PRESS, RELEASE, STOP_ALL, PLAY_VOC = 0x5CB34, 0x5C8A0, 0x5C900, 0x5CE08, 0x5C9C0
A_STATUS, A_END, A_INIT, A_TYPE, A_ADDR, A_RATE, A_VOL, A_LOOP, A_START, TIMER = (0x7893B, 0x786F6, 0x78338, 0x7852D, 0x784AB,
                                                                                  0x78763, 0x787D9, 0x788C5, 0x785AF, 0x66D94)
DIGITAL, OPTION0, COUNT, HANDLES, BUTTON, TABLE = 0x96399, 0x1163B8, 0x33775C, 0x32EEB8, 0x337758, 0x28F010
SPEECH, CRY, STREAM_ON, TIMED = 0x33776C, 0x337760, 0x96372, 0x284F1E
RATE = 22050                             # sound.rs RATE, the mixer rate
BASE, H_SPEECH, H_CRY = 0x100, 0x200, 0x201
STEPS = 16


class Orig:
    def __init__(self):
        self.e = e = Emu(fast=True)
        self.rng = random.Random(7)
        self.state = {}
        self.ended = []
        self.timer = 0

        def arg(em, k):
            sp = em.uc.reg_read(UC_X86_REG_ESP)
            return int.from_bytes(em.uc.mem_read(sp + 4 + 4 * k, 4), "little", signed=True)

        def status(em, eax, edx, ebx, ecx):
            return 4 if self.state.get(arg(em, 0), {}).get("on") else 2

        def end(em, eax, edx, ebx, ecx):
            h = arg(em, 0)
            self.ended.append(h)
            self.state[h] = {}

        def init(em, eax, edx, ebx, ecx):
            self.state[arg(em, 0)] = {}

        def setter(name, k=1):
            def f(em, eax, edx, ebx, ecx):
                self.state.setdefault(arg(em, 0), {})[name] = arg(em, k)
            return f

        def start(em, eax, edx, ebx, ecx):
            self.state[arg(em, 0)]["on"] = True

        def timer(em, eax, edx, ebx, ecx):
            self.timer += 1

        e.stub(A_STATUS, status)
        e.stub(A_END, end)
        e.stub(A_INIT, init)
        e.stub(A_TYPE, lambda *a: None)
        e.stub(A_ADDR, setter("addr"))
        e.stub(A_RATE, setter("rate"))
        e.stub(A_VOL, setter("vol"))
        e.stub(A_LOOP, setter("loops"))
        e.stub(A_START, start)
        e.stub(TIMER, timer)

    def reset(self):
        u = self.e.uc
        self.state = {}
        for a, n in ((DIGITAL, 1), (OPTION0, 1)):
            u.mem_write(a, bytes([n]))
        u.mem_write(COUNT, (8).to_bytes(4, "little"))
        u.mem_write(BUTTON, b"\0\0\0\0")           # the image's start value
        for k in range(8):
            u.mem_write(HANDLES + 4 * k, (BASE + k).to_bytes(4, "little"))
        for k in range(6):
            u.mem_write(TABLE + 8 * k, (0x5000 + k).to_bytes(4, "little") + (100).to_bytes(4, "little"))
        u.mem_write(SPEECH, H_SPEECH.to_bytes(4, "little"))
        u.mem_write(CRY, H_CRY.to_bytes(4, "little"))

    def step(self, op):
        kind, a, b, c, d = op
        e, u = self.e, self.e.uc
        ret = -1
        if kind == 0:
            ret = e.call(PLAY, a, b, c, d)
            ret = int.from_bytes(ret.to_bytes(4, "little"), "little", signed=True)
        elif kind == 1:
            e.call(RELEASE if a else PRESS)
        elif kind == 2:
            self.state[BASE + a] = {}
        elif kind == 3:
            u.mem_write(DIGITAL if self.rng.random() < .5 else OPTION0, bytes([0 if a else 1]))
            if not a:
                u.mem_write(DIGITAL, b"\1")
                u.mem_write(OPTION0, b"\1")
        elif kind == 5:
            self.state.setdefault(H_CRY, {}).setdefault("addr", 0x5000 + 9)
            e.call(PLAY_VOC, a, b & 0xFF, c)
        else:
            u.mem_write(STREAM_ON, b"\1\0")
            timed = self.rng.choice((0, 1))
            u.mem_write(TIMED, bytes([timed]))
            self.ended, self.timer = [], 0
            self.state[H_SPEECH] = {"on": True}
            e.call(STOP_ALL)
            if bytes(u.mem_read(STREAM_ON, 2)) != b"\0\0" or self.timer != timed or \
                    sorted(self.ended) != [BASE + k for k in range(8)] + [H_SPEECH, H_CRY]:
                raise AssertionError(f"StopAllSounds: ended {self.ended}, timer {self.timer}, timed {timed}")
        r = [ret]
        for h in [BASE + k for k in range(8)] + [H_CRY]:
            s = self.state.get(h, {})
            if s.get("on"):
                vol = s["vol"]
                r += [9 if h == H_CRY else s["addr"] - 0x5000, (max(s["rate"], 0) << 16) // RATE, max(min(vol, 127), 0), s["loops"]]
            else:
                r += [-1, 0, 0, 0]
        return r


def main():
    rng = random.Random(4242)
    o = Orig()
    bad = steps = starts = full = 0
    for c in range(ncases):
        ops = []
        for _ in range(STEPS):
            k = rng.choice((0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 2, 2, 3, 4, 5, 5))
            if k == 0:
                ops.append((0, rng.randrange(6), rng.choice((8000, 11025, 22050, 0x2B11, 44100)),
                            rng.choice((0, 5, 20, 50, 0x6E, 127, 128, 200, 255)), rng.choice((0, 1, 1, 2, 3))))
            elif k == 1:
                ops.append((1, rng.randrange(2), 0, 0, 0))
            elif k == 2:
                ops.append((2, rng.randrange(8), 0, 0, 0))
            elif k == 3:
                ops.append((3, rng.choice((0, 0, 1)), 0, 0, 0))
            elif k == 5:
                ops.append((5, rng.choice((8000, 0x2B11, 22050)), rng.choice((0, 0x6E, 127, 128, 255)), rng.choice((0, 1, 2)), 0))
            else:
                ops.append((4, 0, 0, 0, 0))
        flat = (ctypes.c_int * (5 * STEPS))(*[x for op in ops for x in op])
        out = (ctypes.c_int * (37 * STEPS))()
        PORT.ja1f_sfx_ops(flat, STEPS, out)
        o.reset()
        for i, op in enumerate(ops):
            want = o.step(op)
            got = list(out[37 * i:37 * i + 37])
            steps += 1
            starts += want[0] >= 0
            full += op[0] == 0 and want[0] < 0
            if want != got:
                bad += 1
                if bad <= 10:
                    d = [(k, want[1 + 4 * k:5 + 4 * k], got[1 + 4 * k:5 + 4 * k]) for k in range(9) if want[1 + 4 * k:5 + 4 * k] != got[1 + 4 * k:5 + 4 * k]]
                    print(f"FAIL case {c} step {i} {op}: ret {want[0]} / {got[0]}, channels (orig, port) {d[:3]}; before {ops[:i][-3:]}")
                break
    print(f"sfx_diff: {steps} steps, {ncases} cases, {starts} starts, {full} refused, {bad} mismatches")
    sys.exit(1 if bad or not starts or not full else 0)


if __name__ == "__main__":
    main()
