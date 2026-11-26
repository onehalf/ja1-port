#!/usr/bin/env python3
"""Differential test: the voice and effect loaders on the real files. PlayMercVoice 0x24D14, PlaySndsVoc 0x24E44, LoadVocSample
0x5C960, LoadSfxSlot 0x5CA98, LoadMercVoices 0x31A70, SpeechStream 0x5CC6C, LoadFxSound 0x24CB4,
FormatFxPath 0x24C90, LoadMainSound 0x31B8C, LoadDoorSounds 0x31BBC, LoadOverheadSounds 0x31B44, LoadEnemySounds 0x31A10, StopQuoteSample? 0x5CDD8,
ClickSound 0x5C880, OpenArchiveForSpeech? 0x5C0EC, OpenFileOrCD? 0x53338, ArcSeek 0x534A4 against
Sound::name_voice, op 100, op 186, load_tactical and speech_raw (sound.rs, the code the window runs).

usage: .venv/bin/python validate/diff/voice_diff.py [N] [--lib SO]
The original's bodies run in Unicorn on the loaded image. Stubs: the clib file calls (fopen_, fread_, fseek_, fclose_, open_,
filelength_, close) and FileExists?, which serve build/game113b first and then the CD, as OpenFileOrCD? asks; and the AIL calls of
PlayVocSample 0x5C9C0, which report the sample address and length. Compare: the bytes that AIL would play.
Cases: N random speaker and name pairs, every pair over 32768 bytes, the digit boundary 9/10, missing members (the original reads
the old size at the file position after the directory; speaker 29 has no 40.VOC, a player can reach this), speakers 0x40 and up (nothing loads), each of the 5 cries of the MercQuoteEvent? sites
for each speaker, the death cry, the 4 tactical effects per soldier slot of LoadMercVoices, and the speech stream with Sound FX on and off
(rate, volume, and that the refill runs), the banks slot by slot, the click sound, the speech stop. LipSyncReset is stubbed: faceanim_diff tests the mouth.
--pfix 18 (#71, #2 row 51): the original with the cave at LoadVocSample 0x5C98B (a missed member plays nothing), the port with
port fix P18 on.
"""
import ctypes
import os
import random
import struct
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from common import cd_read  # noqa: E402
from emu import Emu  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
import pfix  # noqa: E402
PFIX = pfix.parse(argv)
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
ISO = os.path.join(ROOT, "rom", "JA1.gog").encode()
GAME = os.path.join(ROOT, "build", "game113b")
EXE = os.path.join(GAME, "JA.EXE").encode()
PORT = ctypes.CDLL(LIB)
PORT.ja1f_voice.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
OUT = ctypes.create_string_buffer(1 << 20)

MERC_VOICE, SNDS_VOICE, PLAY_VOC, MERC_BANK, OPEN_SPEECH, CLOSE_SPEECH = 0x24D14, 0x24E44, 0x5C9C0, 0x31A70, 0x5C0EC, 0x5C144
FOPEN, FREAD, FSEEK, FCLOSE, OPEN, FLEN, CLOSE, EXISTS = 0x75EA0, 0x758AD, 0x757B1, 0x75A77, 0x76693, 0x768E1, 0x7691E, 0x4707C
A_INIT, A_TYPE, A_ADDR, A_RATE, A_VOL, A_LOOP, A_START, A_END, A_STATUS = (0x78338, 0x7852D, 0x784AB, 0x78763, 0x787D9, 0x788C5, 0x785AF,
                                                                           0x786F6, 0x7893B)
DIGITAL, OPTION0, SIZE, VOC_LEN, SLOTS, SOLDIERS = 0x96399, 0x1163B8, 0x337940, 0x337768, 0x28F010, 0xFA958
CRY_CALLS = (0x1B3E7, 0x1B40A, 0x1B42A, 0x1B44A, 0x1B46A)    # the calls of MercQuoteEvent?, cases 0xF9..0xFD (port: cry 0..4)
SPEECH, REFILL, LIP, RATE = 0x5CC6C, 0x5CCF8, 0x66D50, 22050
OLD = 0x7000                        # the stale ArcSeek size at the start of each case


def serve(path):
    p = path.replace("\\", "/").upper()        # DOS file names
    if p[1:3] == ":/":
        p = p[3:]
        return cd_read(p)
    f = os.path.join(GAME, p)
    return open(f, "rb").read() if os.path.exists(f) else None


class Orig:
    def __init__(self):
        self.e = e = Emu(fast=True)
        self.files, self.cache = {}, {}
        self.play = None
        e.stub_sprintf()

        def arg(em, k):
            return em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 4 + 4 * k)

        def read_file(path):
            if path not in self.cache:
                try:
                    self.cache[path] = serve(path)
                except Exception:
                    self.cache[path] = None
            return self.cache[path]

        def exists(em, a, *_):
            return 1 if os.path.exists(os.path.join(GAME, em.cstr(a).decode().replace("\\", "/").upper())) else 0

        def fopen(em, a, *_):
            d = read_file(em.cstr(a).decode())
            if d is None:
                return 0
            h = 0x500 + len(self.files)
            self.files[h] = [d, 0]
            return h

        def fread(em, a, d, b, c, *_):
            f = self.files[c]
            out = f[0][f[1]:f[1] + d * b]
            if out:
                em.uc.mem_write(a, out)
            f[1] += len(out)
            return len(out) // max(d, 1)

        def fseek(em, a, d, b, *_):
            f = self.files[a]
            f[1] = {0: d, 1: f[1] + d, 2: len(f[0]) + d}[b]
            return 0

        def op(em, a, *_):
            self.last_len = len(read_file(em.cstr(a).decode()) or b"")
            return 7

        def addr(em, *_):
            self.play = (arg(em, 1), arg(em, 2))

        e.stub(EXISTS, exists)
        e.stub(FOPEN, fopen)
        e.stub(FREAD, fread)
        e.stub(FSEEK, fseek)
        e.stub(FCLOSE, lambda *_: 0)
        e.stub(OPEN, op)
        e.stub(FLEN, lambda *_: self.last_len)
        e.stub(CLOSE, lambda *_: 0)
        e.stub(A_ADDR, addr)
        e.stub(A_END, lambda *_: setattr(self, "ended", self.ended + 1))
        e.stub(A_STATUS, lambda *_: 2)
        e.stub(A_LOOP, lambda em, *_: self.speech.__setitem__("loops", arg(em, 1)))
        e.stub(A_RATE, lambda em, *_: self.speech.__setitem__("rate", int.from_bytes(em.uc.mem_read(em.uc.reg_read(UC_X86_REG_ESP) + 8, 4), "little", signed=True)))
        e.stub(A_VOL, lambda em, *_: self.speech.__setitem__("vol", int.from_bytes(em.uc.mem_read(em.uc.reg_read(UC_X86_REG_ESP) + 8, 4), "little", signed=True)))
        for a in (A_INIT, A_TYPE, A_START):
            e.stub(a, lambda *_: None)
        self.speech = {}
        e.stub(REFILL, lambda *_: self.speech.__setitem__("on", True))
        e.stub(LIP, lambda *_: None)

    def reset(self):
        u = self.e.uc
        self.files.clear()
        u.mem_write(0x92F8F, b"\0")
        u.mem_write(DIGITAL, b"\1")
        u.mem_write(OPTION0, b"\1")
        self.e.w32(SIZE, OLD)
        self.e.w32(VOC_LEN, 3)
        u.mem_write(0x326ED9, bytes([1, 2, 3]))
        self.play = None

    def cry(self):
        self.e.call(PLAY_VOC, 0x2B11, 100, 1)
        a, n = self.play
        return bytes(self.e.uc.mem_read(a, n))

    def name(self, p, m):
        self.reset()
        self.e.call(MERC_VOICE, p, m, 0, 1, 1)
        return self.cry()

    def snds(self, p, member):
        self.reset()
        self.e.call(SNDS_VOICE, p, 0xFFFFFFFF, member, 1, 1)
        return self.cry()

    def death(self, p):
        self.reset()
        self.e.call(OPEN_SPEECH, 0x9093C)
        self.e.call(MERC_VOICE, p, 0xFFFFFFFF, 0x90948, 0, 0)
        self.e.call(CLOSE_SPEECH)
        return self.cry()

    def stream(self, rate, vol, off):
        self.reset()
        self.speech = {}
        self.e.uc.mem_write(OPTION0, bytes([0 if off else 1]))
        self.e.call(SPEECH, rate, vol, 0)
        if not self.speech.get("on"):
            return None
        step = (max(self.speech["rate"], 0) << 16) // RATE
        return struct.pack("<3i", step, max(min(self.speech["vol"], 127), 0), 1)

    def slot(self, k):
        a, n = self.e.r32(SLOTS + 8 * k), self.e.r32(SLOTS + 8 * k + 4)
        return bytes(self.e.uc.mem_read(a, n))

    def load_bank(self, addr, *args):
        self.reset()
        self.e.call(addr, *args)

    def stop(self):
        """StopQuoteSample?: the sample ends and the stream flag falls."""
        self.reset()
        self.e.uc.mem_write(0x96372, b"\1\0")
        self.ended = 0
        self.e.call(0x5CDD8)
        return bytes(self.e.uc.mem_read(0x96372, 2)), self.ended

    def click(self):
        self.reset()
        self.effect = None
        self.e.w32(0x33775C, 8)
        self.e.w32(0x28F010, 0x5000)
        self.e.w32(0x28F014, 100)
        self.e.call(0x5C880)
        return self.speech

    def cry_member(self, c):
        """The member name that the asm puts in EBX before call c of MercQuoteEvent?."""
        code = bytes(self.e.uc.mem_read(CRY_CALLS[c] - 24, 24))
        return struct.unpack("<I", code[code.rindex(b"\xBB") + 1:][:4])[0]

    def bank(self, p, slot, k):
        """LoadMercVoices of soldier `slot`: the bytes of effect 0x2D + 4 * slot + k."""
        self.reset()
        self.e.uc.mem_write(SOLDIERS + 512 * slot + 0x179, bytes([p]))
        self.e.call(MERC_BANK, slot)
        a = self.e.r32(SLOTS + 8 * (0x2D + 4 * slot + k))
        n = self.e.r32(SLOTS + 8 * (0x2D + 4 * slot + k) + 4)
        return bytes(self.e.uc.mem_read(a, n))


def port(kind, a, b):
    n = PORT.ja1f_voice(ISO, EXE, kind, a, b, OUT, len(OUT))
    return None if n < 0 else OUT.raw[:n]


def members(p):
    d = cd_read(f"NAMES/{p}")
    n = struct.unpack_from("<I", d, 0x14)[0]
    return {d[0x20 + 0x20 * i:0x30 + 0x20 * i].split(b"\0")[0].decode(): struct.unpack_from("<I", d, 0x34 + 0x20 * i)[0]
            for i in range(n - 1)}


def main():
    o = Orig()
    if PFIX:
        class _FrontFixes:          # the port side is libja1front: its own copy of the fix masks
            @staticmethod
            def port_fixes(m):
                PORT.ja1_port_fixes_set.argtypes = [ctypes.c_uint64]
                PORT.ja1_port_fixes_set(m)
        pfix.apply(o.e, PFIX, _FrontFixes)
    rng = random.Random(11)
    bad = total = 0

    def same(what, a, b):
        nonlocal bad, total
        total += 1
        if a != b:
            bad += 1
            if bad <= 12:
                print(f"MISMATCH {what}: orig {len(a)} B, port {'none' if b is None else str(len(b)) + ' B'}")

    names = {p: members(p) for p in range(64)}
    pairs = [(p, int(m[:2])) for p in names for m, n in names[p].items() if n > 0x8000]
    pairs += [(0, 9), (0, 10), (1, 9), (1, 10), (3, 0), (57, 57)]
    pairs += [(rng.randrange(64), rng.randrange(60)) for _ in range(ncases)]
    pairs += [(29, 40), (29, 29), (0, 0), (15, 3), (60, 59)]      # members that do not exist; 29/40 is reachable
    for p, m in pairs:
        same(f"name {p} {m}", o.name(p, m), port(0, p, m))
    for p in (0x40, 0x41, 0x7F, 0xFF):
        same(f"name speaker {p}", o.name(p, 3), port(0, p, 3))
    for p in range(64):
        for c in range(5):
            same(f"cry {p} {c}", o.snds(p, o.cry_member(c)), port(1, p, c))
    same("death", o.death(3), port(2, 3, 0))
    for p in rng.sample(range(64), 12):
        slot = rng.randrange(8)
        for k in range(4):
            same(f"bank {p} slot {slot} {k}", o.bank(p, slot, k), port(3, p, k))
    for rate, vol in [(0x5622, 0x46), (0x5622, 0x68), (11025, 127), (0x5622, 128), (0x5622, 0), (44100, 1), (22050, -1)]:
        for off in (0, 1):
            same(f"speech {rate} {vol} {off}", o.stream(rate, vol, off) or b"none", port(4 + off, rate, vol) or b"none")
    banks = ((6, 0x31B8C, (), range(27)), (7, 0x31BBC, (), range(8)), (8, 0x31B44, (17,), range(17)), (9, 0x31B44, (45,), range(45)),
             (9, 0x31A10, (), range(0x4D, 0x55)))
    for kind, addr, a, slots in banks:
        o.load_bank(addr, *a)
        for k in slots:
            same(f"bank {kind} effect {k}", o.slot(k), port(kind, 0, k))
    st = o.click()
    same("click", struct.pack("<4i", 1, (st["rate"] << 16) // RATE, st["vol"], st["loops"]), port(10, 0, 0))
    flag, ended = o.stop()
    same("speech stop", (flag, ended), (b"\0\0", 1))
    same("speech stop port", b"none", port(11, 0, 0) or b"none")
    print(f"voice_diff{' --pfix ' + ','.join(map(str, PFIX)) if PFIX else ''}: {total} cases, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
