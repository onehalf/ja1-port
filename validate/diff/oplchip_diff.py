#!/usr/bin/env python3
"""Differential test: the port's FM chip imitation (port/ja1front/src/fm.rs Opl, export ja1f_opl_render) vs DOSBox's DBOPL, the
chip that the original game plays on here (GOG dosboxja.conf: sbtype sb16, oplmode auto, oplemu default; music.md).

JA.EXE holds no chip code, so the expected values come from DBOPL: `--make-ref BIN` runs each register script of CASES in a
program built on dosbox-x src/hardware/dbopl.cpp (input: `W reg val` / `S samples` lines, output: raw int32 samples at 49716 Hz)
and writes the measured curves to validate/diff/oplchip_ref.json. The JSON holds numbers only; the normal run needs no DBOPL.

Each case compares one measure, always relative to the 0 dB sine of case `level_tl0`:
- env:   the level in dB per 10 ms window, for the windows where one of the two is above FLOOR.
- harm:  the level of harmonics 1..12 of the base frequency in a steady part, for those above FLOOR.
- range: the top minus the bottom of the level in a steady part (the tremolo).
usage: .venv/bin/python validate/diff/oplchip_diff.py [--make-ref BIN] [-v]. Exits 1 on any mismatch.
"""
import array, ctypes, json, math, os, subprocess, sys, tempfile
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
REF = os.path.join(HERE, "oplchip_ref.json")
LIB = os.path.join(ROOT, "port/ja1front/target/release/libja1front.so")
RATE_REF, RATE_PORT, FLOOR = 49716, 22050, -55.0
OPS = [0, 1, 2, 8, 9, 10, 16, 17, 18]
F440, F340 = 0x244, 0x1C0      # fnum 0x1C0 has bit 9 off: the key code has no note-select part


def voice(v=0, mod=(0x21, 0x3F, 0xF0, 0x00, 0), car=(0x21, 0, 0xF0, 0x00, 0), c0=1, fnum=F340, block=4, setup=()):
    """the writes of one note on voice v: registers 20, 40, 60, 80, E0 of each operator, C0, A0, B0 with key on"""
    w = [(1, 0x20)] + list(setup)
    for o, regs in ((OPS[v], mod), (OPS[v] + 3, car)):
        w += [(base + o, x) for base, x in zip((0x20, 0x40, 0x60, 0x80, 0xE0), regs)]
    return w + [(0xC0 + v, c0), (0xA0 + v, fnum & 255), (0xB0 + v, 0x20 | block << 2 | fnum >> 8)]


def off(v=0, fnum=F340, block=4):
    return [(0xB0 + v, block << 2 | fnum >> 8)]


def case(kind, script, tol=3.0, start=0, freq=None, floor=FLOOR):
    return dict(kind=kind, script=script, tol=tol, start=start, freq=freq, floor=floor)


def harm(writes, ms=320, freq=F340 * 49716 / 65536, tol=3.0, floor=FLOOR):
    return case("harm", writes + [ms], tol, 100, freq, floor)


def env(writes, ms=1000, tol=3.0, start=0):
    return case("env", writes + [ms], tol, start)


CASES = {
    # the level: register 40 of the carrier, 0.75 dB per step
    **{f"level_tl{t}": env(voice(car=(0x21, t, 0xF0, 0, 0)), 200, 1.0, 50) for t in (0, 10, 40)},
    # the attack: rate = 4 * AR + key scale part
    **{f"attack_ar{a}": env(voice(car=(0x21, 0, a << 4, 0, 0)), 1500) for a in (3, 5, 7, 10)},
    # the decay to the sustain level, held there (EG type 1); SL 15 = 93 dB
    **{f"decay_dr{d}_sl15": env(voice(car=(0x21, 0, 0xF0 | d, 0xF0, 0)), 1500) for d in (3, 5, 7)},
    "decay_dr7_sl5": env(voice(car=(0x21, 0, 0xF7, 0x50, 0)), 1000),
    "decay_dr6_sl14": env(voice(car=(0x21, 0, 0xF6, 0xE0, 0)), 2000),
    # no hold (EG type 0): the release rate goes on from the sustain level
    "no_hold_rr6": env(voice(car=(0x01, 0, 0xF7, 0x36, 0)), 1500),
    # key off: the release
    **{f"release_rr{r}": env(voice(car=(0x21, 0, 0xF0, r, 0)) + [300] + off(), 1500) for r in (5, 8)},
    # the key scale switch of register 20 at block 6: rate + block * 2 (on) or + block / 2 (off)
    "ksr_on_dr5": env(voice(car=(0x31, 0, 0xF5, 0xF0, 0), block=6), 1000),
    "ksr_off_dr5": env(voice(car=(0x21, 0, 0xF5, 0xF0, 0), block=6), 1000),
    "ksr_on_ar5": env(voice(car=(0x31, 0, 0x50, 0, 0), block=6), 500),
    # the wave form: only with register 1 bit 0x20; voice 1 finds its registers by operator offset
    **{f"wave_{w}": harm(voice(car=(0x21, 0, 0xF0, 0, w))) for w in (1, 2, 3)},
    "wave_off_without_switch": harm(voice(car=(0x21, 0, 0xF0, 0, 2)) + [(1, 0)]),
    "wave_voice1": harm(voice(1, car=(0x21, 0, 0xF0, 0, 1))),
    "mult_voice1": harm(voice(1, car=(0x23, 0, 0xF0, 0, 0))),
    # a level write while the note sounds reaches each operator (the driver's volume)
    "tl_write_carrier": env(voice(1, car=(0x21, 0, 0xF0, 0, 0)) + [100, (0x44, 40)], 300, 1.5),
    "tl_write_modulator": env(voice(1, mod=(0x21, 0, 0xF0, 0, 0), car=(0x21, 63, 0xF0, 0, 0)) + [100, (0x41, 40)], 300, 1.5),
    # a write of A0 alone while the note sounds: the key scale level follows the new number
    "a0_ksl": env(voice(car=(0x21, 0xC0, 0xF0, 0, 0), fnum=0x1FF, block=6) + [100, (0xA0, 0)], 300, 3.0),
    # the tremolo depth: register BD bit 0x80 = 4.8 dB, else 1 dB
    "trem_deep": case("range", voice(car=(0xA1, 0, 0xF0, 0, 0), setup=[(0xBD, 0x80)]) + [1000], 1.0, 100),
    "trem_small": case("range", voice(car=(0xA1, 0, 0xF0, 0, 0)) + [1000], 1.0, 100),
    # a voice stays on while only one operator sounds
    "modulator_gone_first": env(voice(mod=(0x21, 0x3F, 0xF0, 0x0F, 0), car=(0x21, 0, 0xF0, 0x05, 0)) + [200] + off(), 1200),
    # feedback of the modulator (additive voice, carrier silent); feedback 6 stays out: the port mixes at 22050 Hz, DBOPL at
    # 49716 Hz, and its feedback loop gives other upper harmonics (up to 8 dB)
    **{f"feedback_{f}": harm(voice(mod=(0x21, 0, 0xF0, 0, 0), car=(0x21, 63, 0xF0, 0, 0), c0=1 | f << 1), floor=-30.0) for f in (3, 5)},
    # frequency modulation
    "fm_tl20": harm(voice(mod=(0x21, 20, 0xF0, 0, 0), c0=0), floor=-40.0),
    # wave form 3 (the quarter sine) on the modulator of an FM voice: the pulse sits on the first quarter of each half
    # wave; on the second quarter the carrier gets other harmonics (a carrier alone has the same levels either way)
    "wave3_modulator_fm": harm(voice(mod=(0x21, 20, 0xF0, 0, 3), c0=0), floor=-40.0),
    # a B0 write with the key bit still on (a new frequency while the note holds): no new attack, DBOPL's
    # Channel::WriteB0 keys on only when the key bit changes
    "b0_write_key_held": env(voice(car=(0x21, 0, 0x50, 0, 0)) + [800, (0xB0, 0x20 | 4 << 2 | F340 >> 8)], 700),
    # vibrato (register 20 bit 0x40) on the modulator alone of an additive voice: the two sines stay in step
    "vibrato_modulator": env(voice(mod=(0x61, 0, 0xF0, 0, 0), car=(0x21, 0, 0xF0, 0, 0), c0=1), 1500),
}


def windows(pcm, rate, scale):
    out = []
    for k in range(len(pcm) * 100 // rate):
        a, b = k * rate // 100, (k + 1) * rate // 100     # 220.5 samples at the port's rate: no drift
        out.append(10 * math.log10(max(sum(x * x for x in pcm[a:b]) / (b - a), 1e-30)) - scale)
    return out


def harmonics(pcm, rate, freq, start_ms, scale):
    a = int(start_ms * rate / 1000)
    # a whole number of base cycles, about 200 ms
    n = int(round(round(0.2 * freq) * rate / freq))
    part, out = pcm[a:a + n], []
    for h in range(1, 13):
        w = 2 * math.pi * h * freq / rate
        re = sum(x * math.cos(w * i) for i, x in enumerate(part))
        im = sum(x * math.sin(w * i) for i, x in enumerate(part))
        # the level of a sine with this amplitude: same scale as `windows`
        amp2 = (re * re + im * im) * 4 / (n * n)
        out.append(10 * math.log10(max(amp2 / 2, 1e-30)) - scale)
    return out


def measure(c, pcm, rate, scale):
    if c["kind"] == "env":
        return windows(pcm, rate, scale)
    if c["kind"] == "harm":
        return harmonics(pcm, rate, c["freq"], c["start"], scale)
    w = windows(pcm, rate, scale)[c["start"] // 10:]
    return [max(w) - min(w)]


def scale_of(pcm, rate):
    """the mean power of the steady 0 dB sine, level_tl0, in dB"""
    w = windows(pcm, rate, 0)[5:]
    return sum(w) / len(w)


def make_ref(binary):
    out = {}
    scale = None
    for name in ["level_tl0"] + [k for k in CASES if k != "level_tl0"]:
        c = CASES[name]
        with tempfile.TemporaryDirectory() as d:
            s, o = os.path.join(d, "s.txt"), os.path.join(d, "o.raw")
            with open(s, "w") as f:
                for x in c["script"]:
                    f.write(f"S {x * RATE_REF // 1000}\n" if isinstance(x, int) else f"W {x[0]:x} {x[1]:x}\n")
            subprocess.run([binary, s, o], check=True)
            pcm = array.array("i", open(o, "rb").read()).tolist()
        if scale is None:
            scale = scale_of(pcm, RATE_REF)
        out[name] = [round(v, 1) for v in measure(c, pcm, RATE_REF, scale)]
    with open(REF, "w") as f:
        json.dump(out, f, separators=(",", ":"))
        f.write("\n")
    print(f"oplchip_diff: wrote {len(out)} curves, {os.path.getsize(REF)} bytes")


def port_pcm(lib, script):
    flat = []
    for x in script:
        flat += [1, x, 0] if isinstance(x, int) else [0, x[0], x[1]]
    arr = (ctypes.c_int32 * len(flat))(*flat)
    cap = sum(x for x in script if isinstance(x, int)) * RATE_PORT // 1000 + 1
    buf = (ctypes.c_float * cap)()
    n = lib.ja1f_opl_render(arr, len(flat) // 3, buf, cap)
    assert n >= 0, n
    return list(buf[:n])


def main():
    if "--make-ref" in sys.argv:
        return make_ref(sys.argv[sys.argv.index("--make-ref") + 1])
    verbose = "-v" in sys.argv
    ref = json.load(open(REF))
    lib = ctypes.CDLL(LIB)
    lib.ja1f_opl_render.argtypes = [ctypes.c_void_p, ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32]
    lib.ja1f_opl_render.restype = ctypes.c_int32
    scale = scale_of(port_pcm(lib, CASES["level_tl0"]["script"]), RATE_PORT)
    bad = 0
    for name, c in CASES.items():
        got = measure(c, port_pcm(lib, c["script"]), RATE_PORT, scale)
        want = ref[name]
        if c["kind"] == "env":
            pairs = [(i * 10, a, b) for i, (a, b) in enumerate(zip(want, got)) if i * 10 >= c["start"] and max(a, b) > c["floor"]]
        elif c["kind"] == "harm":
            pairs = [(i + 1, a, b) for i, (a, b) in enumerate(zip(want, got)) if max(a, b) > c["floor"]]
        else:
            pairs = [(0, want[0], got[0])]
        worst = max(pairs, key=lambda t: abs(t[1] - t[2]))
        d = abs(worst[1] - worst[2])
        if d > c["tol"]:
            bad += 1
            print(f"MISMATCH {name}: at {worst[0]} DBOPL {worst[1]:.1f} dB, port {worst[2]:.1f} dB (limit {c['tol']})")
        elif verbose:
            print(f"ok {name}: worst {d:.1f} dB at {worst[0]}")
    print(f"oplchip_diff: {len(CASES)} cases, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
