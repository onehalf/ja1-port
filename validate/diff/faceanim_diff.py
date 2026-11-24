#!/usr/bin/env python3
"""faceanim_diff.py [N] [--seed S] — the face animation (panel.md "Face animation (eyes and mouth)"), original vs port.

The original in Unicorn on the plain image, Random 0x53538 stubbed with a list of fuzzed numbers (R(n) = V mod n):
- eye step 0x27E38 (EAX &e, DL d, BL b, e 0..12): the new e and the count of numbers used;
- mouth step 0x27FB8 (EAX &m, silence byte 0x337C9F, m 0..3): the same;
- the face: a fuzzed 148 x 69 sheet at *0x93E70; the 56 x 50 face copied to the panel buffer *0x93E6C + 0x644
  (pitch 228, as SpeechWait? 0x28034 does), then eye frame 0x27D64 (e, EDX 0) and mouth frame 0x27F40 (m): the
  56 x 50 panel block; and 0x27D64 (e, EDX 1) to the screen *0x95678 + 0x208E (pitch 320) vs the eye block;
- the lip sync: reset 0x66D50, the samples fed to the scan 0x66DB8 in fuzzed chunks (timer byte 0x337C9E = 1, so
  the AIL timer is not registered), then K calls of the timer 0x66CC4: the silence table (count 0x337970, starts
  0x337978, ends 0x337B08) and the ticks at which byte 0x337C9F changes;
- the parts of a quote voice (PlayQuote modes 1..3: A + name, name + B, A + name + B): each stream part (A, B) gets
  its own reset 0x66D50 + scan 0x66DB8 as SpeechStream / SpeechRefill? do, the name (a cry sample) no scan; the
  silence tables of A and of B (moved by the start of B; A + name is a multiple of 200 samples, so the ms add up
  exactly) vs the silences of the port's one lip view (sound.rs lip_view). A's quiet end run is never a silence;
- the voice stream (box 1 review #45 c13004): SpeechRefill? 0x5CCF8 itself, with only the AIL calls stubbed (buffer
  ready = fuzzed 0 / 1 / -1, status 4 while loading, then 2; fread_ from the voice; LipSyncTimerStart? 0x66EB0 sets
  0x337C9E = 1 as it does; AIL_release_timer_handle): reset 0x66D50, remaining 0x337764 = the voice length, busy
  word 0x96372 = 1, lip flag 0x284F1E = 1, refills until nothing remains, the timer K times, then one refill with
  status 2. Compared vs the port's scan of the whole voice (the port plays and scans the whole voice): the silence
  table and the timer ticks; and the stream itself: reads of at most 0x4000 bytes into buffer 0x28F2B8 + i * 0x4000
  that add up to the voice in order, each read scanned and loaded with its own length, no read on a -1 buffer, and
  after the status-2 refill busy word 0, lip flag 0, silence byte 0 (ReleaseTacticalTimer? 0x66D94; the port's
  voice ends and `at` answers false).
The port: `ja1front --face-anim CASES` (faceanim.rs). Exits 1 on any mismatch.
"""
import os, random, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

FRONT = os.path.join(ROOT, "port/ja1front/target/release/ja1front")
args = [a for a in sys.argv[1:] if not a.startswith("--")]
N = int(args[0]) if args else 2000
seed = int(sys.argv[sys.argv.index("--seed") + 1]) if "--seed" in sys.argv else 1


def vals(r):
    return [r.choice([0, 1, 2, 3, r.randrange(1 << 15)]) for _ in range(40)]


def pcm(r):
    out = bytearray()
    for _ in range(r.randrange(1, 12)):
        k = r.choice([0, 1, 2])
        n = r.choice([r.randrange(1, 3000), r.choice([1099, 1100, 1101]), r.randrange(1000, 8000)])
        if k == 0:
            out += bytes(r.randrange(0x7B, 0x86) for _ in range(n))
        elif k == 1:
            out += bytes(r.choice([r.randrange(0, 0x7B), r.randrange(0x86, 256)]) for _ in range(min(n, 400)))
        else:
            out += bytes(r.randrange(256) for _ in range(min(n, 400)))
    return bytes(out)


def make_case(r):
    k = r.choice(["eye", "eye", "mouth", "face", "scan", "parts", "refill"])
    if k == "parts":
        mode = r.choice([1, 2, 3])
        a = pcm(r) if mode != 2 else b""
        if a and r.random() < 0.6:
            a += bytes(r.randrange(0x7B, 0x86) for _ in range(r.choice([r.randrange(1, 1100), 1100, r.randrange(1100, 6000)])))
        name = bytes(r.randrange(256) for _ in range(r.randrange(200, 3000)))
        name += bytes(r.randrange(256) for _ in range(-(len(a) + len(name)) % 200))
        b = pcm(r) if mode != 1 else b""
        return ("parts", mode, a, name, b, sorted(r.randrange(len(a) + 1) for _ in range(r.randrange(0, 3))))
    if k == "eye":
        return ("eye", r.randrange(13), r.choice([0, 1, r.randrange(256)]), r.choice([0, 1, 2, r.randrange(256)]),
                vals(r))
    if k == "mouth":
        return ("mouth", r.randrange(4), r.choice([0, 0, 1]), vals(r))
    if k == "face":
        return ("face", r.randrange(13), r.randrange(4), bytes(r.randrange(256) for _ in range(148 * 69)))
    data = pcm(r)
    if k == "refill":
        while r.random() < 0.5 and len(data) < 50000:
            data += pcm(r)
        if r.random() < 0.2:
            data = data[:r.choice([0x4000, 0x8000, 0x3FFF, 0x4001])] if len(data) > 0x8000 else data
        ready = [r.choice([0, 1, 0, 1, -1]) for _ in range(200)]
        return ("refill", len(data) * 45 // 2500 + r.randrange(0, 200), data, ready)
    return ("scan", len(data) * 45 // 2500 + r.randrange(0, 200), data,
            sorted(r.randrange(len(data) + 1) for _ in range(r.randrange(0, 4))))


class Orig:
    def __init__(self):
        self.e = Emu(fast=True)
        self.seq, self.k = [], 0
        self.e.stub(0x53538, self.rand)
        self.cell = self.e.alloc(4)
        self.sheet = self.e.alloc(148 * 69)
        self.panel = self.e.alloc(228 * 100)
        self.screen = self.e.alloc(64000)
        self.e.w32(0x93E70, self.sheet)
        self.e.w32(0x93E6C, self.panel)
        self.e.w32(0x95678, self.screen)
        self.buf = self.e.alloc(70000)
        self.st = {}
        for a, f in ((0x79101, self.ail_ready), (0x791E6, self.ail_load), (0x7893B, self.ail_status),
                     (0x77B88, lambda *_: 0), (0x758AD, self.fread), (0x66EB0, self.timer_start)):
            self.e.stub(a, f)

    def arg(self, i):
        from unicorn.x86_const import UC_X86_REG_ESP
        return self.e.r32(self.e.uc.reg_read(UC_X86_REG_ESP) + 4 + 4 * i)

    def ail_ready(self, em, *_):
        st = self.st
        v = st["ready"][st["k"] % len(st["ready"])] if st["status"] == 4 else -1
        st["k"] += 1
        st["last"] = v
        return v & 0xFFFFFFFF

    def ail_load(self, em, *_):
        self.st["loads"].append((self.arg(1), self.arg(2), self.arg(3)))

    def ail_status(self, em, *_):
        return self.st["status"]

    def fread(self, em, buf, size, n, fp):
        st = self.st
        k = size * n
        d = st["data"][st["pos"]:st["pos"] + k]
        em.uc.mem_write(buf, d)
        st["reads"].append((buf, k, st["last"]))
        st["pos"] += len(d)
        return len(d) // size if size else 0

    def timer_start(self, em, *_):
        em.w8(0x337C9E, 1)

    def rand(self, em, a, *_):
        v = self.seq[self.k] if self.k < len(self.seq) else 0
        self.k += 1
        return v % a if a > 0 else 0

    def run(self, c):
        e = self.e
        if c[0] in ("eye", "mouth"):
            self.seq, self.k = c[-1], 0
            e.w32(self.cell, c[1])
            if c[0] == "eye":
                e.call(0x27E38, self.cell, c[2], c[3], 0x55)
            else:
                e.w8(0x337C9F, c[2])
                e.call(0x27FB8, self.cell)
            return [f"{c[0]} {e.r32(self.cell)} {self.k}"]
        if c[0] == "face":
            _, ee, m, sheet = c
            e.uc.mem_write(self.sheet, sheet)
            for j in range(50):
                e.uc.mem_write(self.panel + 0x644 + 228 * j, sheet[148 * j:148 * j + 56])
            e.call(0x27D64, ee, 0)
            e.call(0x27F40, m)
            face = b"".join(bytes(e.uc.mem_read(self.panel + 0x644 + 228 * j, 56)) for j in range(50))
            e.uc.mem_write(self.screen, bytes(64000))
            e.call(0x27D64, ee, 1)
            scr = b"".join(bytes(e.uc.mem_read(self.screen + 0x208E + 320 * j, 46)) for j in range(23))
            f = min(ee, 3)
            sx, sy = (5, 5) if f <= 0 else (56, 23 * (f - 1))
            want = b"".join(sheet[148 * (sy + j) + sx:148 * (sy + j) + sx + 46] for j in range(23))
            return [f"face {face.hex()}", f"screen {'ok' if scr == want else 'bad'}"]
        if c[0] == "parts":
            _, mode, a, name, b, cuts = c
            out = []
            for part, cs, off in ((a, cuts, 0), (b, [], (len(a) + len(name)) * 45 // 1000)):
                if not part:
                    continue
                e.call(0x66D50)
                e.w8(0x337C9E, 1)
                e.uc.mem_write(self.buf, part)
                pos = 0
                for cut in cs + [len(part)]:
                    if cut > pos:
                        e.call(0x66DB8, self.buf + pos, cut - pos)
                        pos = cut
                n = e.r32(0x337970) & 0xFFFF
                out += [f"{e.r32(0x337978 + 4 * i) + off},{e.r32(0x337B08 + 4 * i) + off}" for i in range(n)]
            return ["parts " + " ".join(out)]
        if c[0] == "refill":
            _, k, data, ready = c
            st = self.st = {"data": data, "pos": 0, "ready": ready, "k": 0, "status": 4, "last": -1, "reads": [], "loads": []}
            e.call(0x66D50)
            e.w8(0x337C9E, 1)
            e.w8(0x284F1E, 1)
            e.w16(0x96372, 1)
            e.w32(0x337764, len(data))
            e.w32(0x33776C, 0x5A5A0)
            e.w32(0x92F4C, 0x1234)
            calls, bad = 0, []
            while (e.r32(0x337764) or not st["loads"]) and calls < 400:
                e.call(0x5CCF8)
                calls += 1
            n = e.r32(0x337970) & 0xFFFF
            sil = " ".join(f"{e.r32(0x337978 + 4 * i)},{e.r32(0x337B08 + 4 * i)}" for i in range(n))
            ch, was = [], e.r8(0x337C9F)
            for t in range(k):
                e.call(0x66CC4)
                v = e.r8(0x337C9F)
                if v != was:
                    ch.append(str(t))
                    was = v
            st["status"] = 2
            e.call(0x5CCF8)
            if st["pos"] != len(data) or e.r32(0x337764):
                bad.append(f"read {st['pos']} of {len(data)}")
            for (buf, ln, idx), ld in zip(st["reads"], st["loads"]):
                if idx < 0 or ln > 0x4000 or buf != 0x28F2B8 + idx * 0x4000 or ld != (idx, buf, ln):
                    bad.append(f"read {buf:#x} {ln} buffer {idx} load {ld}")
            if any(i < 0 for i, _, _ in st["loads"]) or len(st["loads"]) < len(st["reads"]):
                bad.append("load on -1")
            if e.r16(0x96372) or e.r8(0x284F1E) or e.r8(0x337C9F):
                bad.append(f"end busy {e.r16(0x96372)} lip {e.r8(0x284F1E)} silence {e.r8(0x337C9F)}")
            return [f"scan {sil} | {' '.join(ch)}" + (" BAD " + "; ".join(bad[:2]) if bad else "")]
        _, k, data, cuts = c
        e.call(0x66D50)
        e.w8(0x337C9E, 1)
        e.uc.mem_write(self.buf, data)
        pos = 0
        for cut in cuts + [len(data)]:
            if cut > pos:
                e.call(0x66DB8, self.buf + pos, cut - pos)
                pos = cut
        n = e.r32(0x337970) & 0xFFFF
        sil = " ".join(f"{e.r32(0x337978 + 4 * i)},{e.r32(0x337B08 + 4 * i)}" for i in range(n))
        ch, was = [], e.r8(0x337C9F)
        for t in range(k):
            e.call(0x66CC4)
            v = e.r8(0x337C9F)
            if v != was:
                ch.append(str(t))
                was = v
        return [f"scan {sil} | {' '.join(ch)}"]


def main():
    r = random.Random(seed)
    cases = [make_case(r) for _ in range(N)]
    o = Orig()
    orig = [o.run(c) for c in cases]
    path = os.path.join(ROOT, "build", "faceanim_cases.txt")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as fh:
        for c in cases:
            if c[0] == "eye":
                fh.write(f"eye {c[1]} {c[2]} {c[3]} {' '.join(map(str, c[4]))}\n")
            elif c[0] == "mouth":
                fh.write(f"mouth {c[1]} {c[2]} {' '.join(map(str, c[3]))}\n")
            elif c[0] == "face":
                fh.write(f"face {c[1]} {c[2]} {c[3].hex()}\n")
            elif c[0] == "parts":
                fh.write(f"parts {len(c[2])} {len(c[2]) + len(c[3])} {(c[2] + c[3] + c[4]).hex()}\n")
            else:                    # scan and refill: the port scans the whole voice
                fh.write(f"scan {c[1]} {c[2].hex()}\n")
    p = subprocess.run([FRONT, "--face-anim", path], capture_output=True, text=True, timeout=300)
    port = p.stdout.splitlines()
    bad, kinds, sils = 0, {}, 0
    for n, (c, ol) in enumerate(zip(cases, orig)):
        kinds[c[0]] = kinds.get(c[0], 0) + 1
        q = port[n] if n < len(port) else "?"
        if c[0] in ("scan", "refill"):
            sils += ol[0].split("|")[0].count(",")
        if c[0] == "parts":
            sils += ol[0].count(",")
        ok = ol[0] == q and (c[0] != "face" or ol[1] == "screen ok")
        if not ok:
            bad += 1
            if bad <= 3:
                print(f"case {n} {c[0]} {c[1:2] if c[0] == 'parts' else c[1:3]}: original {ol[0][:160]} {ol[1:]} port {q[:160]}")
    if len(port) != len(orig):
        print(f"port answered {len(port)} of {len(orig)} cases ({p.stderr.strip()[-200:]})")
        bad += 1
    print(f"faceanim_diff: {len(cases)} cases {kinds}, {sils} silences, {bad} mismatches")
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
