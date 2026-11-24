"""Differential test: the sunrise screen's loop (SunriseScreen 0x2A268, strategic.md "The sunrise screen"), original vs port.

usage: .venv/bin/python validate/diff/sunrise_screen_diff.py [ncases=40] [DUMP] [--port] [--lib SO]
Per case: the day, the funds, the new arrivals, the Random seed 0x9BBBC and the poll that skips (or none) are fuzzed.
The original runs in Unicorn on tac60. Real: PaletteRotate 0x528AC, PaletteCopy 0x52820, DrawText 0x358D0, BlitCopy
0x52AE0, CopyPanelToScreen 0x46BF4, FadePalette 0x52908, Random 0x53538, itoa_ / ltoa_ and memset_. Stubbed: LoadArchivePcx 0x532BC and
LoadArchivePalette 0x53300 (MAIN.DAT members; sab_diff tests both), LoadMainSound, StopMusic, HidePointer, LoadPanels
0x46EC4, OvernightHeal 0x4B3F0 (sunrise_diff tests it) and the mouse queue 0x6782C. A frame of the original =
PaletteSet 0x52870 (the colour cycle) or the helicopter's TimerElapsedMs 0x47040 at 0x2A897 (right after its screen
copy): the 320 x 200 screen and the palette 0xEDA08. KeyOrClick 0x67E44 answers yes on the case's poll. FadePalette
0x52908 runs for real: each level is one frame, read at the DAC ports. PlaySfxSlot 0x5CB34
logs (effect, rate, volume, loops, frames shown so far); EndSampleSlot 0x5CE60 logs its effect.
The port: ja1f_sunrise_screen runs Sunrise::run with its frame log on (no window, script polls).
Compared: every frame (screen and 6-bit palette), the fade calls' steps and levels, the sounds with the frame each
starts on, the stopped sounds and the final seed. Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE     # noqa: E402
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 40
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"sunrise_screen_diff: {dump} missing")
    sys.exit(1)
PORT = None
FRAME = 64000 + 768
CAP = 57 + 90 + 1 + 32 + 25 + 2
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
DAY, BALANCE, ARRIVALS, SEED, SCREEN_P, PAL = 0x229FA4, 0x229FC4, 0x22A02C, 0x9BBBC, 0x95678, 0xEDA08
HELI_WAIT = 0x2A89C            # the return address of the helicopter's TimerElapsedMs call
e = Emu(dump=dump, fast=True)
st = {}
pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in pics:
        pics[n] = pcx.decode(datarc.member(MAIN, n))
    return pics[n]


def load(em, eax, edx, ebx, ecx):
    w, h, px, _ = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


def load_pal(em, eax, edx, ebx, ecx):
    # ReadPalPalette 0x53204: the file's last 0x300 bytes, each >> 2
    e.uc.mem_write(ecx, bytes(v >> 2 for v in pic(e.cstr(ebx))[3]))
    return 0


def snap():
    scr = bytes(e.uc.mem_read(e.r32(e.rt(SCREEN_P)), 64000))
    p = bytes(e.uc.mem_read(e.rt(PAL), 1024))
    return scr + bytes(p[i * 4 + c] for i in range(256) for c in range(3))


def pal_set(em, *_):
    st["frames"].append(snap())
    return 0


def fade(uc, *_):
    # The real FadePalette runs after this code hook: each level reaches the DAC (port 0x3C8, then 768 bytes to 0x3C9).
    reg = [uc.reg_read(r) for r in (UC_X86_REG_EDX, UC_X86_REG_EBX, UC_X86_REG_ECX)]
    to = e.r32(uc.reg_read(UC_X86_REG_ESP) + 4)
    st["fades"].append(((*reg, to), snap()[:64000], []))
    if len(st["fades"]) == 1:
        st["shown"] = 57           # the port's frames of the fade in


def dac(port, size, v):
    if port == 0x3C8:
        st["fades"][-1][2].append(bytearray())
    elif port == 0x3C9:
        st["fades"][-1][2][-1].append(v & 0xFF)
    else:
        raise AssertionError(f"port {port:#x}")


def timer(em, *_):
    if e.r32(e.uc.reg_read(UC_X86_REG_ESP)) != e.rt(HELI_WAIT):
        return 0x7F
    # The helicopter: one wait per frame, one poll in it (the first answer stays below the wait), then the wait ends.
    st["heli"] = not st["heli"]
    if st["heli"]:
        st["frames"].append(snap())
        return 0
    return 0x7F


def key(em, *_):
    k = st["polls"]
    st["polls"] = k + 1
    return 1 if k == st["poll_at"] else 0


def sfx(em, eax, edx, ebx, ecx):
    shown = st["shown"] + len(st["frames"])
    st["sfx"].append((eax & 0xFF, edx & 0xFFFF, ebx & 0xFF, ecx & 0xFF, shown))
    st["slots"].append(eax & 0xFF)
    return len(st["slots"]) - 1


def end(em, eax, *_):
    k = eax & 0xFFFFFFFF
    if k != 0xFFFFFFFF:
        st["ends"].append(st["slots"][k])
    return 0


for a in (0x47020, 0x31B8C, 0x5C82C, 0x67BF0, 0x46EC4, 0x4B3F0, 0x6782C, 0x673AC):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
e.stub(e.rt(0x53300), load_pal)
e.stub(e.rt(0x52870), pal_set)
e.uc.hook_add(UC_HOOK_CODE, fade, begin=e.rt(0x52908), end=e.rt(0x52908))
e.port_in = lambda p: 0x08     # the retrace poll waits for bit 0 = 0, then bit 8 = 1
e.port_out = dac
e.stub(e.rt(0x47040), timer)
e.stub(e.rt(0x67E44), key)
e.stub(e.rt(0x5CB34), sfx)
e.stub(e.rt(0x5CE60), end)

r = random.Random(7)
bad = 0
fades_in = heli_cases = skips = 0
for i in range(ncases):
    day = r.choice([1, 2, 9, 10, 99, 100, r.randrange(1, 1000)])
    bal = r.choice([0, 1, 25000, -1, -2865, r.randrange(1 << 31) - (1 << 30), 0x7FFFFFFF])
    arr = r.choice([0, 0, 1, 1, 2, 5, -1])
    seed = r.getrandbits(32)
    poll_at = r.choice([-1, -1, 0, 9, 10, 11, 35, 75, 89, 90, 95, 121, r.randrange(122)])
    st.update(frames=[], fades=[], sfx=[], slots=[], ends=[], polls=0, poll_at=poll_at, heli=False, shown=0)
    e.w32(e.rt(DAY), day)
    e.w32(e.rt(BALANCE), bal & 0xFFFFFFFF)
    e.uc.mem_write(e.rt(ARRIVALS), (arr & 0xFFFF).to_bytes(2, "little"))
    e.w32(e.rt(SEED), seed)
    e.call(e.rt(0x2A268), max_steps=60_000_000)
    seed_o = e.r32(e.rt(SEED))
    # FadePalette 0x52908 (EBX steps, ECX from, stack to): in 56 steps 0 -> 256, out 24 steps 256 -> 0.
    assert all(len(l) == 768 for f in st["fades"] for l in f[2]) and len(st["fades"]) == 2 and st["fades"][0][0] == (0, 0x38, 0, 0x100) and st["fades"][1][0] == (0, 0x18, 0x100, 0), \
        [f[0] for f in st["fades"]]
    heli_cases += arr > 0
    skips += st["polls"] > poll_at >= 0
    if not PORT:
        continue
    buf = ctypes.create_string_buffer(CAP * FRAME)
    ev = (ctypes.c_int32 * (5 * 64))()
    ct = (ctypes.c_int32 * 4)()
    rc = PORT.ja1f_sunrise_screen(ISO, EXE, day, ctypes.c_int32(bal), ctypes.c_int16(arr), poll_at,
                                  ctypes.c_uint32(seed), buf, CAP, ev, 64, ct)
    n, ne, seed_p = ct[0], ct[1], ct[2] & 0xFFFFFFFF
    fr = [buf.raw[k * FRAME:(k + 1) * FRAME] for k in range(n)]
    why = None
    want = len(st["fades"][0][2]) + len(st["frames"]) + len(st["fades"][1][2])
    if rc != 0:
        why = f"rc {rc}"
    elif n != want:
        why = f"frames: original {want}, port {n}"
    else:
        pairs = [(f"fade in {k}", st["fades"][0][1] + bytes(o), fr[k]) for k, o in enumerate(st["fades"][0][2])]
        pairs += [(f"frame {k}", o, fr[57 + k]) for k, o in enumerate(st["frames"])]
        pairs += [(f"fade out {k}", st["fades"][1][1] + bytes(o), fr[57 + len(st["frames"]) + k])
                  for k, o in enumerate(st["fades"][1][2])]
        for name, o, p in pairs:
            if o != p:
                j = next(j for j in range(FRAME) if o[j] != p[j])
                where = f"screen ({j % 320},{j // 320})" if j < 64000 else f"palette byte {j - 64000}"
                why = f"{name}: {where}: original {o[j]}, port {p[j]}"
                break
    psfx = [tuple(ev[5 * k:5 * k + 5]) for k in range(ne)]
    pends = [x[1] for x in psfx if x[0] == -1]
    psfx = [x for x in psfx if x[0] != -1]
    if why is None and psfx != st["sfx"]:
        k = next((k for k in range(max(len(psfx), len(st["sfx"])))
                  if k >= len(psfx) or k >= len(st["sfx"]) or psfx[k] != st["sfx"][k]))
        why = f"sound {k}: original {st['sfx'][k:k + 1]}, port {psfx[k:k + 1]}"
    if why is None and pends != st["ends"]:
        why = f"stopped sounds: original {st['ends']}, port {pends}"
    if why is None and seed_p != seed_o:
        why = f"seed: original {seed_o:#x}, port {seed_p:#x}"
    if why:
        print(f"MISMATCH case {i} (day {day}, funds {bal}, arrivals {arr}, poll {poll_at}): {why}")
        bad += 1
        if bad > 10:
            break
print(f"sunrise_screen_diff: {ncases} cases, {heli_cases} with arrivals, {skips} skipped, {bad} mismatches"
      + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
