#!/usr/bin/env python3
"""Differential test: the sequence volume and its fade, and the sequencer's channel messages.
SetSequenceVolume? 0x5C858, AIL_API_set_sequence_volume 0x84500, the service 0x82C50, 0x82C10 and the cc7 scale in 0x822E0
against Music::set_volume and Music::tick (music.rs).

usage: .venv/bin/python validate/diff/musicfade_diff.py [N=100] [--lib SO]
The original's bodies run in Unicorn on the tac60 dump with the sequence and driver structures of that dump, on the real
XMI file. AIL_API_init_sequence and start run; only the driver side is stubbed: XMI message 0x81C00 (the log), timbre
calls. Each case: a file and theme, random volume calls (volume, ms, also 0 ms and short fades), Music option changes
StopMusic and a new start of the theme, master volume calls AIL_API_set_XMIDI_master_volume 0x845D0 (Music::set_master:
each playing sequence sends its channels' last cc7 again, 0x82C10), `TICKS` service ticks.
The compare: every channel message (tick, status, data 1, data 2) and its order.
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
ncases = int(args[0]) if args else 100
PORT = ctypes.CDLL(LIB)
PORT.ja1f_music_fade.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_int, ctypes.c_int,
                                 ctypes.c_void_p, ctypes.c_int]
GAME = os.path.join(ROOT, "build", "game113b")
EXE = os.path.join(GAME, "JA.EXE").encode()
CAP = 200000
TICKS = 1500
SET_VOL, INIT, START, SERVICE, SEND, MEMMOVE = 0x5C858, 0x83EF0, 0x842B0, 0x82C50, 0x81C00, 0x85C43
SEQ, DRIVER_ON, OPTION, LOCK_FLAG = 0x337774, 0x9639A, 0x1163B9, 0x9CA24
CALL_DRIVER, STOP_MUSIC, THEMES = 0x7F2BB, 0x5C82C, 0x9635C
STOP, OFF, ON, AGAIN, MASTER = -3, -2, -1, -4, -5   # op volumes: StopMusic, Music option off, on, theme start again,
# AIL_API_set_XMIDI_master_volume 0x845D0 (value in the ms field; the volume keys + - reach it, volume_diff)
SET_MASTER, MASTER_OFF = 0x845D0, 0x1B0


class Orig:
    def __init__(self):
        self.e = e = Emu(dump=os.path.join(ROOT, "build", "mem_tac60.bin"), fast=True)
        self.seq = e.r32(e.rt(SEQ))
        self.drv = e.r32(self.seq)
        self.log = []
        self.tick = 0
        self.files = {}

        def arg(em, k):
            sp = em.uc.reg_read(UC_X86_REG_ESP)
            return int.from_bytes(em.uc.mem_read(sp + 4 + 4 * k, 4), "little")

        def send(em, eax, edx, ebx, ecx):
            st = arg(em, 1) & 0xFF
            if st & 0xF0 in (0xA0, 0xD0):
                return      # the port's parser drops them: the driver does nothing with them (mdi_diff sends both)
            # a program change has one data byte: the sequencer pushes whatever follows it in the file
            self.log.append((self.tick, st, arg(em, 2) & 0xFF, 0 if st & 0xF0 == 0xC0 else arg(em, 3) & 0xFF))

        def memmove(em, eax, edx, ebx, ecx):
            # Unicorn loses the stack across push es / pop es in memmove_ (dump mode): same bytes by hand
            em.uc.mem_write(eax, bytes(em.uc.mem_read(edx, ebx)))
            return eax

        e.stub(e.rt(SEND), send)
        e.stub(e.rt(MEMMOVE), memmove)

        def call_driver(em, eax, edx, ebx, ecx):
            # the timbre calls: the answer "installed" (fn 0x503) ends AIL_API_init_sequence without its error text;
            # the port's side is Driver::install (mdi_diff)
            out = arg(em, 3)
            if out:
                em.w16(out, 1)

        e.stub(e.rt(CALL_DRIVER), call_driver)
        e.stub(e.rt(0x7F395), lambda *a: None)     # AIL_API_delay: a real-time wait

    def file(self, name):
        if name not in self.files:
            self.files[name] = self.e.alloc(open(os.path.join(GAME, "MISC", name + ".XMI"), "rb").read())
        return self.files[name]

    def run(self, name, theme, ops):
        e = self.e
        e.w32(e.rt(LOCK_FLAG), 0)
        e.w8(e.rt(DRIVER_ON), 1)
        e.w8(e.rt(OPTION), 1)
        e.w32(self.drv + MASTER_OFF, 127)   # the port's Music starts at master 127
        self.log = []
        self.tick = -1
        r = e.call(e.rt(INIT), self.seq, self.file(name), theme, cdecl=True)
        if r != 1:
            raise SystemExit(f"musicfade_diff: FAIL init {name} {theme}: {r} {e.stop_reason}")
        e.call(e.rt(START), self.seq, cdecl=True)
        if e.stop_reason:
            raise SystemExit(f"musicfade_diff: FAIL start: {e.stop_reason}")
        e.call(e.rt(SET_VOL), 127, 0)
        e.w16(e.rt(THEMES + 2), theme)
        self.log = []
        for k in range(TICKS):
            self.tick = k
            for o in ops:
                if o[0] == k:
                    if o[1] == AGAIN:
                        # SetMusic 0x5C5EC with the option on: stop, init, start, volume 127 at once
                        if e.r8(e.rt(OPTION)):
                            e.call(e.rt(STOP_MUSIC))
                            e.call(e.rt(INIT), self.seq, self.file(name), theme, cdecl=True)
                            e.call(e.rt(START), self.seq, cdecl=True)
                            e.call(e.rt(SET_VOL), 127, 0)
                    elif o[1] == STOP:
                        e.call(e.rt(STOP_MUSIC))
                    elif o[1] == MASTER:
                        e.call(e.rt(SET_MASTER), self.drv, o[2], cdecl=True)
                    elif o[1] in (OFF, ON):
                        e.w8(e.rt(OPTION), int(o[1] == ON))
                    else:
                        e.call(e.rt(SET_VOL), o[1], o[2])
            e.call(e.rt(SERVICE), self.drv, cdecl=True)
            if e.stop_reason:
                raise SystemExit(f"musicfade_diff: FAIL service tick {k}: {e.stop_reason}")
        return self.log


def main():
    rng = random.Random(4242)
    o = Orig()
    out = (ctypes.c_int * (4 * CAP))()
    bad = msgs = cc7 = fades = masters = 0
    for c in range(ncases):
        name = rng.choice(("DAY", "MENUS", "INTRO"))
        theme = rng.choice((1, 1, 2, 3)) if name != "INTRO" else 1
        ops = []
        if c % 8:
            for _ in range(rng.randint(1, 4)):
                v = rng.choice((0, 0, 0, 30, 64, 100, 127, 127, ON, OFF, STOP, AGAIN, MASTER, MASTER))
                ms = rng.choice((0, 20, 64, 100, 127, 127)) if v == MASTER else rng.choice((0, 1, 100, 500, 1200, 1200, 2000, 3000))
                ops.append((rng.randrange(TICKS - 200), v, ms))
        ops.sort()
        ops = sorted(set(ops), key=lambda x: x[0])
        flat = (ctypes.c_int * (3 * max(1, len(ops))))(*[x for op in ops for x in op])
        n = PORT.ja1f_music_fade(EXE, name.encode(), theme, flat, len(ops), TICKS, out, CAP)
        got = [tuple(out[4 * i:4 * i + 4]) for i in range(n)]
        want = o.run(name, theme, ops)
        msgs += len(want)
        fades += sum(1 for x in ops if x[1] >= 0 and x[2])
        masters += sum(1 for x in ops if x[1] == MASTER)
        cc7 += sum(1 for m in want if m[1] & 0xF0 == 0xB0 and m[2] == 7)
        if want != got:
            bad += 1
            k = next((i for i, (a, b) in enumerate(zip(want, got)) if a != b), min(len(want), len(got)))
            if bad <= 8:
                print(f"FAIL case {c} {name} {theme} ops {ops}: {len(want)} / {len(got)} msgs, first diff {k}: "
                      f"orig {want[k:k + 3]} port {got[k:k + 3]}")
    print(f"musicfade_diff: {ncases} cases, {msgs} messages, {cc7} volume messages, {fades} fades, {masters} master volumes, {bad} mismatches")
    sys.exit(1 if bad or not cc7 or not fades or not masters else 0)


if __name__ == "__main__":
    main()
