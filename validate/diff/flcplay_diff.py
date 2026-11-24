#!/usr/bin/env python3
"""flcplay_diff.py [--seed S] [--only NAME] — the film player of the original vs the port.

The original: PlayFilm 0x5BFF8 whole in Unicorn on the plain image, with FlcOpen 0x5BB30, FlcPlay 0x5BC58, FlcFirstFrame
0x5B920, the frame reader 0x5B990 and FilmCallback 0x5BE18 unstubbed. Stubs: the open and seek callbacks 0x5BA24 /
0x5BA74 and fread_ 0x758AD read the film bytes; the AIL timer calls 0x775CC, 0x777AA, 0x779EA, 0x77B88 (the period is
logged); a code hook at the spin 0x5BD11 is the timer tick; KeyPressed 0x5616C / GetKey 0x560F4 give the case's key per
callback; PlaySfxSlot 0x5CB34, SetMusic 0x5C5EC, the quote read 0x24F08, the text draw 0x3625C, SpeechStream 0x5CC6C,
fseek 0x757B1 and the speech pump 0x5CCF8 are stubs; the subtitle gate 0x5EBE8 and ArcSeek 0x534A4 (on a made speech
directory at 0x10C6D8) run real. The
screen pointer +0x9C (0xA0000, the VGA window, which is image data in this emulator) is moved at FlcPlay's entry.
Cases: each film with its cue table (effects, clears, voices that exist, voices that do not, empty voices, subtitles
with the option on and off, music), 1, 2 or 3 passes or "until a key" (loops 0), keys that stop and keys that do not,
extended keys (GetKey 0, then a scan code 0x1B that is not Escape), cut film bytes (a frame that does not read), and
films with a zero second-frame place +0x54 (no game file has one: the FlcFirstFrame fallback runs on spliced headers,
and main fails if a game film ever shows the fallback).
Log per step: tick + time, frame j + screen and DAC crc at the callback, sounds, voice name + rate + volume + member
crc, quote + colours, show + crc + cue index + abort flag.
The port: `ja1f_film_trace` (sab.rs Sab::play_film + flc.rs FilmSteps). Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, subprocess, sys, zlib
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import (UC_X86_REG_GDTR, UC_X86_REG_CS, UC_X86_REG_SS, UC_X86_REG_DS,  # noqa: E402
                               UC_X86_REG_ES, UC_X86_REG_GS, UC_X86_REG_ESP, UC_X86_REG_EAX)
from common import cd_read           # noqa: E402
import datarc                        # noqa: E402

argv = sys.argv[1:]
seed = int(argv[argv.index("--seed") + 1]) if "--seed" in argv else 1
only = argv[argv.index("--only") + 1].upper() if "--only" in argv else None
PORT = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
PORT.ja1f_film_trace.argtypes = [ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32,
                                 ctypes.c_int32, ctypes.c_int32, ctypes.c_int32, ctypes.c_char_p, ctypes.c_char_p,
                                 ctypes.c_int32, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_char_p, ctypes.c_int32,
                                 ctypes.c_char_p, ctypes.c_int32, ctypes.c_int32, ctypes.c_int32]
PORT.ja1f_film_trace.restype = ctypes.c_int32
PLAY_FILM, FLC_PLAY, CB_OPEN, CB_SEEK, FREAD = 0x5BFF8, 0x5BC58, 0x5BA24, 0x5BA74, 0x758AD
ARC_DIR, SUB_OPT, VOICE_VOL = 0x10C6D8, 0x1163BD, 0x337948
REC, BUF_PTR, TICK, CUE_I, CUE_J, TAB, ABORT, ANYKEY, RATE, SCREEN_PTR = (0x28EF44, 0x93E64, 0x28EF40, 0x1163B4, 0x1163B0,
                                                                         0xA5448, 0x95DC4, 0x93D58, 0x96370, 0x95678)
BIG = {"SIRTECH.FLC": (0, 0), "TITLE1.FLC": (0, 0), "TITLE2.FLC": (0, 0), "INTRO05.FLC": (0, 15),
       "HELILIFT.FLC": (0, 0), "HELILEAV.FLC": (0, 0)}
EXT = 0xFF     # a key byte of a case: an extended key
s32 = lambda v: v - (1 << 32) if v & 0x80000000 else v


class DacEmu(Emu):
    """OUT 0x3C8 = DAC write index, OUT 0x3C9 = the next 6-bit component (VGA), as flc_diff.py."""
    def _io(self, uc, port, size, value, *rest):
        if not hasattr(self, "dac"):
            self.dac, self.dac_i = bytearray(1024), 0
        if port == 0x3C8:
            self.dac_i = 4 * (value & 0xFF)
        elif port == 0x3C9:
            self.dac[self.dac_i] = value & 0x3F
            self.dac_i += 1
            if self.dac_i % 4 == 3:
                self.dac_i += 1
            self.dac_i %= 1024
        else:
            return super()._io(uc, port, size, value, *rest)


def films():
    names = subprocess.check_output(["bsdtar", "-tf", os.path.join(ROOT, "rom", "JA1.gog")]).decode().split("\n")
    seen = set()
    for p in sorted(n for n in names if n.upper().endswith(".DAT")):
        d = cd_read(p)
        try:
            ms = datarc.members(d)
        except Exception:
            continue
        for n, o, s in ms:
            b = d[o:o + s]
            if len(b) > 128 and struct.unpack_from("<H", b, 4)[0] in (0xAF11, 0xAF12) and b not in seen:
                seen.add(b)
                if only is None or n.upper() == only:
                    yield n.upper(), b


e = DacEmu(fast=True)
SCREEN, FRAMEBUF, NAME = e.alloc(64000), e.alloc(0x40000), e.alloc(16)
e.w32(BUF_PTR, FRAMEBUF)
e.w32(SCREEN_PTR, SCREEN)
GDT = e.alloc(24)
e.uc.mem_write(GDT, struct.pack("<QQQ", 0, 0x00CF9A000000FFFF, 0x00CF92000000FFFF))
e.uc.reg_write(UC_X86_REG_GDTR, (0, GDT, 23, 0))
for reg, sel in ((UC_X86_REG_CS, 0x08), (UC_X86_REG_SS, 0x10), (UC_X86_REG_DS, 0x10), (UC_X86_REG_ES, 0x10),
                 (UC_X86_REG_GS, 0x10)):
    e.uc.reg_write(reg, sel)
st = {"data": b"", "pos": 0, "log": [], "keys": b"", "at": 0, "ticks": 0, "ms": 0, "second": False, "seek": 0, "arc": b""}


def fread(em, buf, size, n, fp):
    want = size * n
    d, p = st["data"], st["pos"]
    if p + want > len(d):
        return 0
    em.uc.mem_write(buf, d[p:p + want])
    st["pos"] = p + want
    return n


def seek(em, rec, off, *_):
    st["pos"] = off
    return 0


def opened(em, rec, *_):
    st["pos"] = 0
    return 0


def key_waits(*_):
    return 1 if st["at"] < len(st["keys"]) and st["keys"][st["at"]] else 0


def get_key(*_):
    k = st["keys"][st["at"]]
    if k == EXT:        # GetKey 0x560F4: an extended key gives 0, then its scan code on the next call
        st["second"] = not st["second"]
        return 0 if st["second"] else 0x1B
    return k


def period(em, *_):
    st["ms"] = em.r32(em.uc.reg_read(UC_X86_REG_ESP) + 8) // 1000


def crc():
    return "%x %x" % (zlib.crc32(bytes(e.uc.mem_read(SCREEN, 64000))), zlib.crc32(bytes(e.dac)))


def fseek(em, fp, off, *_):
    st["seek"] = off
    return 0


def speech(em, rate, vol, b, c):
    off = st["seek"]
    size = next(struct.unpack_from("<I", st["arc"], k + 20)[0] for k in range(0x20, len(st["arc"]), 0x20)
                if struct.unpack_from("<I", st["arc"], k + 16)[0] == off)
    st["log"].append(f"speech {s32(rate)} {s32(vol)} {s32(b)} {zlib.crc32(st['arc'][off:off + size]):x}")


def quote(em, a, d, b, c):
    st["log"].append(f"quote {s32(b)}")


def text(em, a, d, b, c):
    st["log"].append(f"text {d & 0xFF} {b & 0xFF}")


def show(*_):
    st["log"].append(f"show {crc()} {e.r32(CUE_I)} {e.r8(ABORT)}")


def h_first(*_):
    if not e.uc.reg_read(UC_X86_REG_EAX):
        show()


def h_spin(*_):
    st["ticks"] += 1
    st["log"].append(f"tick {st['ms'] * st['ticks']}")
    e.w32(TICK, 1)


def h_callback(*_):
    st["log"].append(f"frame {e.r16(REC + 0x9A)} {crc()}")


def h_callback_end(*_):
    show()
    st["at"] += 1


def h_play(*_):
    e.w32(REC + 0x9C, SCREEN)
    e.dac_i = 0


for addr, fn in ((FREAD, fread), (CB_SEEK, seek), (CB_OPEN, opened), (0x5616C, key_waits), (0x560F4, get_key),
                 (0x775CC, lambda *_: 7), (0x777AA, period), (0x779EA, lambda *_: 0), (0x77B88, lambda *_: 0),
                 (0x5CCF8, lambda *_: 0), (0x757B1, fseek), (0x5CC6C, speech),
                 (0x5CB34, lambda em, a, d, b, c: st["log"].append(f"sfx {s32(a)} {s32(d)} {b & 0xFF} {s32(c)}")),
                 (0x5C5EC, lambda em, a, d, b, c: st["log"].append(f"music {s32(a)} {s32(d)}"))):
    e.stub(addr, fn)
e.stub(0x24F08, quote, pop=16)
e.stub(0x3625C, text, pop=32)
for addr, fn in ((FLC_PLAY, h_play), (0x5BD11, h_spin), (0x5BE18, h_callback), (0x5BCD5, h_first), (0x5BD3C, h_callback_end),
                 (0x5BD43, h_callback_end)):
    e.uc.hook_add(UC_HOOK_CODE, fn, None, addr, addr)
e.uc.ctl_flush_tb()


def run_original(c):
    data, x, y, flag, loops, film_flag, rate, tab, keys, screen, dac, sub_on, vol = c
    e.uc.mem_write(ARC_DIR, VOICES)
    e.w8(SUB_OPT, sub_on)
    e.w16(VOICE_VOL, vol)
    e.uc.mem_write(SCREEN, screen)
    e.uc.mem_write(FRAMEBUF, bytes(0x40000))
    e.uc.mem_write(TAB, tab)
    e.w32(RATE - 2, rate << 16)
    e.w32(CUE_I, 0)
    e.w32(CUE_J, 0)
    e.w8(ABORT, 0)
    e.w8(ANYKEY, film_flag)
    e.dac = bytearray(dac)
    st.update(data=data, pos=0, log=[], keys=keys, at=0, ticks=0, ms=0, second=False, seek=0, arc=VOICES)
    e.call(PLAY_FILM, NAME, x, y, flag, loops, max_steps=1_000_000_000)
    return st["log"]


def run_port(c):
    data, x, y, flag, loops, film_flag, rate, tab, keys, screen, dac, sub_on, vol = c
    out = ctypes.create_string_buffer(1 << 22)
    n = PORT.ja1f_film_trace(data, len(data), x, y, flag, loops, film_flag, rate, tab, keys, len(keys), screen, bytes(dac),
                             out, 1 << 22, VOICES, len(VOICES), sub_on, vol)
    return out.raw[:n].decode().splitlines()


def make_voices():
    """A speech archive (ArcSeek 0x534A4: records 1..count-1), each member other bytes."""
    r = random.Random(0x5BE18)
    arc = bytearray(0x20 * (len(VOICE_NAMES) + 2))
    struct.pack_into("<I", arc, 0x14, len(VOICE_NAMES) + 1)
    for i, nm in enumerate(VOICE_NAMES, 1):
        d = r.randbytes(r.randrange(50, 400))
        arc[0x20 * i:0x20 * i + 16] = nm.ljust(16, b"\0")
        struct.pack_into("<II", arc, 0x20 * i + 16, len(arc), len(d))
        arc += d
    return bytes(arc)


VOICE_NAMES = [b"V%d.8" % k for k in range(6)]
VOICES = make_voices()


def make_table(r, n):
    ents = []
    for _ in range(r.randrange(0, 8)):
        a = r.choice([r.randrange(0, 99), 99, r.randrange(100, 200), r.randrange(200, 330), r.randrange(231, 238)])
        ents.append((r.randrange(1, n + 2), a))
    ents.sort()
    if r.random() < 0.3:
        ents.insert(r.randrange(len(ents) + 1), (9998, r.randrange(0, 20)))
    ents += [(9999, 0)] * (50 - len(ents))
    names = bytearray(120)
    for k in range(r.randrange(0, 11)):
        nm = r.choice(VOICE_NAMES + [b"NONE.8", b"v2.8"])
        names[12 * k:12 * k + len(nm)] = nm
    return b"".join(struct.pack("<hh", f, a) for f, a in ents) + bytes(names)


def make_cases(r, name, data):
    w, h = struct.unpack_from("<HH", data, 8)
    n = struct.unpack_from("<H", data, 6)[0]
    big = name in BIG
    (x, y) = BIG[name] if big else (r.randrange(0, 320 - w // 2), r.randrange(0, 200 - h // 2))
    plans = [(1, False, None)]
    if big:
        # a long film: a stop key early on, as the player's Escape
        plans = [(1, r.random() < 0.5, r.randrange(1, min(n, 25))), (0, True, r.randrange(1, min(n, 25)))]
    else:
        plans += [(2, False, None), (3, False, r.randrange(0, 2 * n + 2)), (0, False, r.randrange(0, 3 * n + 1)),
                  (0, True, n - 1), (1, True, r.randrange(0, n))]
    for loops, film_flag, stop in plans:
        keys = bytearray(stop + 1 if stop is not None else 0)
        for k in range(len(keys)):
            if not film_flag and r.random() < 0.15:
                keys[k] = r.choice((0x61, 0x61, EXT))
        if stop is not None:
            keys[stop] = r.choice((0x61, EXT)) if film_flag else 27
        d = data
        if not big and loops == 2 and r.random() < 0.3 and len(data) > 400:
            d = data[:r.randrange(len(data) // 2, len(data))]
        elif not big and loops == 2:
            d = data[:84] + bytes(4) + data[88:]   # no second-frame place: FlcFirstFrame 0x5B920 finds it
        dac = bytearray(r.randrange(64) for _ in range(1024))
        dac[3::4] = bytes(256)
        yield (d, x, y, 1 if big else r.choice((0, 1)), loops, int(film_flag), r.choice((11025, 22050)),
               make_table(r, n), bytes(keys), r.randbytes(64000), bytes(dac), r.randrange(2), r.randrange(1, 128))


def main():
    r = random.Random(seed)
    nfilm = ncase = bad = fallback = spliced = 0
    steps = {}
    for name, data in films():
        nfilm += 1
        fallback += struct.unpack_from("<I", data, 84)[0] == 0
        for c in make_cases(r, name, data):
            ncase += 1
            spliced += struct.unpack_from("<I", c[0], 84)[0] == 0
            o, p = run_original(c), run_port(c)
            o.append(f"end {e.r32(CUE_I)} {e.r8(ABORT)}")
            for l in o:
                steps[l.split()[0]] = steps.get(l.split()[0], 0) + 1
            p.append(o[-1])     # the show lines hold the port's end state
            if o != p:
                bad += 1
                if bad <= 3:
                    k = next((i for i, (a, b) in enumerate(zip(o, p)) if a != b), min(len(o), len(p)))
                    print(f"{name} loops {c[4]} flag {c[3]} any {c[5]} len {len(c[0])}/{len(data)} keys {c[8].hex()}: "
                          f"line {k} of {len(o)}/{len(p)}: orig {o[k:k + 2]} port {p[k:k + 2]}")
    print(f"flcplay_diff: {nfilm} films ({fallback} with no second frame place), {ncase} cases ({spliced} made with none), steps {sorted(steps.items())}, {bad} mismatches")
    if fallback:
        print("a game film has no second-frame place: the flc.rs fallback is reachable in play")
    if not spliced:
        print("no case ran the FlcFirstFrame fallback")
        bad += 1
    for kind in ("tick", "frame", "show", "sfx", "music", "speech", "quote", "text"):
        if not steps.get(kind):
            print(f"no {kind} step ran")
            bad += 1
    sys.exit(1 if bad else 0)


main()
