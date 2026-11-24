"""Differential test: InitProgramWorld? 0x377B0 with SrandTime? 0x53564, original vs port (#45 weak row).

usage: .venv/bin/python validate/diff/startworld_diff.py [ncases=8] [DUMP]

Per case: noise in the bytes that the routine writes, and a fuzzed clock. The original runs in Unicorn on tac60 with the
real SrandTime; stubs: time_ 0x7697C gives the clock, LoadJaggedDat 0x371BC and TimerStart 0x47020 do nothing. Then it
compares with ja1f_program_start: the tile map (25088 B), the 8 view values, the soldier pointer table (48 entries, as
offsets from g_soldiers), the seed of rand_ 0x9BBBC.
Then the program: without JA1_SEED, `--play` and `--menu` print a seed within 3 s of the clock, and JA1_SEED=N prints
nothing, so the tests stay repeatable. Exits 1 on any mismatch.
"""
import ctypes
import os
import random
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

args = [a for a in sys.argv[1:] if not a.startswith("--")]
ncases = int(args[0]) if args else 8
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
LIB = os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so")
FRONT = os.path.join(ROOT, "port", "ja1front", "target", "release", "ja1front")
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog"), os.path.join(ROOT, "build", "game113b", "JA.EXE")
START, TIME, JAGGED, TIMER = 0x377B0, 0x7697C, 0x371BC, 0x47020
TABLE, SOLDIERS, TILES, SEED, TILE_BYTES = 0x100958, 0xFA958, 0x100A18, 0x9BBBC, 25088
WINDOW = (0x229CD4, 0x229CCC, 0x229CC8, 0x229CD0, 0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC)

if not os.path.exists(dump):
    print(f"startworld_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_program_start.argtypes = [ctypes.c_char_p, ctypes.c_char_p, ctypes.c_uint64, ctypes.c_char_p,
                                    ctypes.POINTER(ctypes.c_int32), ctypes.POINTER(ctypes.c_uint32),
                                    ctypes.POINTER(ctypes.c_uint32)]
e = Emu(dump=dump, fast=True)
rt = e.rt
clock = [0]


def time_stub(emu, eax, *_):
    emu.w32(eax, clock[0])
    return clock[0]


e.stub(rt(TIME), time_stub)
e.stub(rt(JAGGED), lambda em, *_: 0)
e.stub(rt(TIMER), lambda em, *_: 0)

rng = random.Random(int(os.environ.get("START_SEED", "1")))
fails = 0


def bad(what, a, b):
    global fails
    fails += 1
    if fails <= 10:
        print(f"MISMATCH {what}: original {a} port {b}")


for i in range(ncases):
    # byte 7 of each tile is not written by the routine: the port's 0 needs a clean byte there too.
    e.uc.mem_write(rt(TILES), bytes(0 if k % 8 == 7 else rng.randrange(256) for k in range(TILE_BYTES)))
    e.uc.mem_write(rt(TABLE), bytes(rng.randrange(256) for _ in range(0xC0)))
    for a in WINDOW:
        e.w32(rt(a), rng.getrandbits(32))
    e.w32(rt(SEED), rng.getrandbits(32))
    clock[0] = rng.choice([1, 0xFFFFFFFF, rng.getrandbits(32), 0x80000000 + i])
    e.call(rt(START))
    tiles = bytes(e.uc.mem_read(rt(TILES), TILE_BYTES))
    window = [e.r32(rt(a)) for a in WINDOW]
    table = [(e.r32(rt(TABLE) + 4 * k) - rt(SOLDIERS)) & 0xFFFFFFFF for k in range(48)]
    seed = e.r32(rt(SEED))

    out = ctypes.create_string_buffer(TILE_BYTES)
    pw, pt, ps = (ctypes.c_int32 * 8)(), (ctypes.c_uint32 * 48)(), ctypes.c_uint32()
    if PORT.ja1f_program_start(ISO.encode(), EXE.encode(), clock[0], out, pw, pt, ctypes.byref(ps)):
        bad("world", "ok", "failed")
        continue
    if tiles != out.raw:
        k = next(k for k in range(TILE_BYTES) if tiles[k] != out.raw[k])
        bad(f"case {i} tile byte {k}", tiles[k], out.raw[k])
    if window != [v & 0xFFFFFFFF for v in pw]:
        bad(f"case {i} view values", window, list(pw))
    if table != list(pt):
        bad(f"case {i} pointer table", table[:4], list(pt)[:4])
    if seed != ps.value:
        bad(f"case {i} clock {clock[0]:#x} seed", hex(seed), hex(ps.value))


def printed_seed(cmd, env):
    """The seed line of a program start; the line comes before the window opens, so no display is needed."""
    p = subprocess.Popen(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True,
                         stdin=subprocess.DEVNULL)
    line = None
    try:
        t0 = time.time()
        os.set_blocking(p.stderr.fileno(), False)
        while time.time() - t0 < 15 and line is None and p.poll() is None:
            for s in p.stderr.readlines():
                if s.startswith("start seed "):
                    line = s
            time.sleep(.02)
        if line is None:
            for s in p.stderr.readlines():
                if s.startswith("start seed "):
                    line = s
    finally:
        p.kill()
        p.wait()
    return int(line.split()[2]) if line else None


env = {k: v for k, v in os.environ.items() if k != "JA1_SEED"}
env["DISPLAY"] = ":0.bad"                    # never a real display: the program stops after the line
env["SDL_VIDEODRIVER"] = "offscreen"
env["SDL_AUDIODRIVER"] = "dummy"
save = os.path.join(ROOT, "rom", "__support", "save", "QICKSTRT.SAV")
runs = {"--play": [FRONT, ISO, EXE, "--play", save], "--menu": [FRONT, ISO, EXE, "--menu", os.path.join(ROOT, "build")]}
for name, cmd in runs.items():
    got = printed_seed(cmd, env)
    if got is None or abs(got - time.time()) > 3:
        bad(f"{name} seed at the clock {int(time.time())}", "clock", got)
    if printed_seed(cmd, {**env, "JA1_SEED": "7"}) is not None:
        bad(f"{name} JA1_SEED=7 prints a clock seed", None, "line")
print(f"startworld_diff: {ncases} cases + 2 program starts; {fails} mismatches")
sys.exit(1 if fails else 0)
