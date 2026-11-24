"""Differential test: the volume keys + - . , of TacticalKeyHandler 0x13490, original vs port.

usage: .venv/bin/python validate/diff/volume_diff.py [ncases=4000] [DUMP] [--port] [--lib SO]
Per case: fuzzed music / sound numbers (0x963A0 / 0x963A8; mostly 0..100, some out of range) and one of the four key
branches (jump table 0x13328, index = key - 1): + 0x13D0A, - 0x13CE8, . 0x13D2E, , 0x13D54; the run ends at the
handler's common tail 0x1409B. The original runs in Unicorn on tac60 with 0x5CEB4 / 0x5CE94 unstubbed and the AIL
wrappers AIL_set_XMIDI_master_volume 0x7A524 / AIL_set_digital_master_volume 0x78DB4 stubbed (cdecl: handle, value
recorded). The port (port/ja1front ja1f_volume_key = sound::volume_key) runs on the same numbers; both numbers, the
AIL call kind and its value must match. Without --port only the original runs. Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
from unicorn.x86_const import UC_X86_REG_ESP
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 4000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_volume_key.argtypes = [ctypes.c_uint8] + [ctypes.POINTER(ctypes.c_int32)] * 3
    PORT.ja1f_volume_key.restype = ctypes.c_int32

MUSIC, SOUND = 0x963A0, 0x963A8
KEYS = {b"+": 0x13D0A, b"-": 0x13CE8, b".": 0x13D2E, b",": 0x13D54}
DIG, MDI = 0x337780, 0x337790                 # the driver handles the wrappers get

e = Emu(dump=dump, fast=True)
rt = e.rt
calls = []


def ail(kind, handle_at):
    def f(em, *r):
        sp = em.uc.reg_read(UC_X86_REG_ESP)
        h, v = em.r32(sp + 4), em.r32(sp + 8)
        calls.append((kind, v - (1 << 32) if v >> 31 else v, h == em.r32(rt(handle_at))))
    return f


e.stub(rt(0x7A524), ail(1, MDI))
e.stub(rt(0x78DB4), ail(2, DIG))
e.stub(rt(0x1409B), lambda em, *a: 0)         # the key handler's common tail: end of a key run


def s32(v):
    return v - (1 << 32) if v >> 31 else v


rng = random.Random(int(os.environ.get("VOLUME_SEED", "1")))
fails, by_key = 0, {}
save = {a: e.uc.mem_read(rt(a), 4) for a in (MUSIC, SOUND)}
for i in range(ncases):
    def num():
        return rng.choice([rng.randrange(101), rng.randrange(101), 0, 100, 70, rng.randrange(-300, 400),
                           rng.choice([-1, 101, -(1 << 31), (1 << 31) - 1])])
    music, sound = num(), num()
    key = rng.choice(list(KEYS))
    e.w32(rt(MUSIC), music & 0xFFFFFFFF)
    e.w32(rt(SOUND), sound & 0xFFFFFFFF)
    calls.clear()
    e.call(rt(KEYS[key]))
    got = (s32(e.r32(rt(MUSIC))), s32(e.r32(rt(SOUND))), calls[0][0] if calls else 0, calls[0][1] if calls else 0)
    bad = [] if len(calls) == 1 and calls[0][2] else [f"AIL calls {calls}"]
    by_key[key] = by_key.get(key, 0) + 1
    if PORT:
        m, s, sent = ctypes.c_int32(music), ctypes.c_int32(sound), ctypes.c_int32(0)
        kind = PORT.ja1f_volume_key(key[0], ctypes.byref(m), ctypes.byref(s), ctypes.byref(sent))
        want = (m.value, s.value, kind, sent.value)
        if got != want:
            bad.append(f"(music, sound, AIL kind, value) original {got} port {want}")
    if bad:
        fails += 1
        if fails <= 10:
            print(f"MISMATCH case {i}: key {key.decode()} music {music} sound {sound}: {bad}")
for a, b in save.items():
    e.uc.mem_write(rt(a), bytes(b))
print(f"volume_diff: {ncases} cases ({', '.join(f'{k.decode()} {n}' for k, n in sorted(by_key.items()))})"
      f"{'' if PORT else ' (original only)'}; {fails} mismatches")
sys.exit(1 if fails else 0)
