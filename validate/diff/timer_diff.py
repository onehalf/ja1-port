#!/usr/bin/env python3
"""Differential test of the whole 10 ms timer callback TacticalTimer? 0x5EE98, original vs the port's real path:
play::timer_ticks (ja1_tick, AnimTimers::step, Pause::tick, marker_tick, clocks_tick, the scroll timers, loss_tick) and
play::panel_countdown, through ja1f_timer_step. ai_diff "ticks" covers ja1_tick only and takes the animations from a
Python mirror; this test has no mirror.

The original runs in Unicorn on a dump with every input fuzzed, 1 to 3 calls per case. Compared: the 32 timer values,
the 10 tile animations, the 16 effect timers, 48 soldier records, the tile map and the 5 fuses. A whole-data compare
also proves that the callback writes nothing outside that list except the two bytes the port has no place for:
  0x92EFC (+10 per tick): only TacticalLoop 0x107FB clears it, nothing reads it.
  0x92EF4 (-10 per tick): only this callback touches it.
  0x92DB8: the re-entry guard, set and cleared inside one call.

usage: .venv/bin/python validate/diff/timer_diff.py [ncases=3000] [DUMP...]
Exits 1 on a mismatch or when a gate or an end event is not reached.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu  # noqa: E402

argv = sys.argv[1:]
ncases = int(argv[0]) if argv else 3000
dumps = argv[1:] or [os.path.join(ROOT, "build", "mem_tac60.bin")]
P = ctypes.CDLL(os.path.join(ROOT, "port", "ja1front", "target", "release", "libja1front.so"))
ISO, EXE = (os.path.join(ROOT, *q).encode() for q in (("rom", "JA1.gog"), ("build", "game113b", "JA.EXE")))
TIMER, SOLD, TILES, ANIMS, FXA, FUSE = 0x5EE98, 0xFA958, 0x100A18, 0x229E24, 0xEAFF0, 0x22A488
N_SOLD, N_TILES, N_FXA, N_FUSE = 48 * 512, 3136 * 8, 16 * 0x12, 5 * 13
# name, address, bytes. Inputs of the first group are not written by the callback.
VALS = [("lock2", 0x92DE1, 1), ("lock", 0x92DE0, 1), ("lit", 0x284F0E, 1), ("hover", 0x229D28, 4), ("delay", 0x229D44, 4),
        ("ambient_t1", 0x229D30, 4), ("ambient_t2", 0x229D34, 4), ("mark_timer", 0x229D38, 4), ("pause_blink", 0x229D48, 4),
        ("cursor_blink", 0x229D4C, 4), ("small_panel", 0x92E30, 4), ("panel_ends", 0x92E35, 1), ("marker_tile", 0x229CAC, 4),
        ("marker_life", 0x229D20, 4), ("marker_blink", 0x229D40, 4), ("show_timer", 0x93D54, 4), ("tick_step", 0x92F60, 4),
        ("shown_slot", 0x229CB4, 4), ("spotted", 0x229C94, 4), ("view_tile", 0x229C8C, 4), ("loss_timer", 0x92DE8, 4),
        ("busy_timer", 0x92DB4, 4), ("busy", 0x92E38, 4), ("fast_timer", 0x92DB0, 4), ("turn_based", 0x92DAC, 4),
        ("turn_clock", 0x229D1C, 4), ("turn_tick", 0x229D24, 4), ("ripple_timer", 0x92EF8, 4), ("stop_timer", 0x95DC8, 4),
        ("screen_drawn", 0x92DCC, 1), ("fuse_beep_timer", 0x229E0C, 4), ("fuse_beep", 0x284F29, 1)]


class Io(ctypes.Structure):
    _fields_ = [(v[0], ctypes.c_int) for v in VALS] + [("anim_count", ctypes.c_int * 10), ("anim_timer", ctypes.c_int * 10),
                                                       ("anim_tile", ctypes.c_int * 10), ("fx_timer", ctypes.c_int * 16)]


IGNORE = [(0x92EFC, 4), (0x92EF4, 4), (0x92DB8, 1)]
TIMES = [0, 1, 5, 9, 10, 11, 20, 140, 700, 3000, 20000, 0xFFFFFFFF, 0xFFFFFFF6, 0x80000005]
r = random.Random(0x5EE98)
bad = 0
seen = {}


def fail(case, text):
    global bad
    bad += 1
    if bad <= 12:
        print(f"MISMATCH case {case}: {text}")


def sgn(v, k):
    return struct.unpack("<b" if k == 1 else "<i", int(v).to_bytes(k, "little"))[0]


def run_dump(dump, cases):
    global bad
    e = Emu(dump=dump, fast=True)
    rt = e.rt
    for case in range(cases):
        for name, a, k in VALS:
            if k == 1:
                v = {"lit": r.choice([0x40, 0x40, 0, 3]), "lock2": r.choice([0, 0, 0, 0, 1]), "lock": r.choice([0, 0, 0, 0, 1]),
                     "fuse_beep": r.choice([0, 0, 1]), "panel_ends": r.choice([0, 0, 1]), "screen_drawn": r.choice([0, 1])}[name]
                e.w8(rt(a), v)
            elif name == "marker_tile":
                e.w32(rt(a), r.choice([0xC41, r.randrange(3136), 100]))
            elif name == "tick_step":
                e.w32(rt(a), r.choice([10, 10, 1000, 0]))
            elif name in ("shown_slot", "spotted", "view_tile", "busy", "turn_based"):
                e.w32(rt(a), r.choice([0, 1, 2, 3, 0x40, 0xC41, 0xFFFFFFFF, 1234]))
            else:
                e.w32(rt(a), r.choice(TIMES + [r.randrange(1 << 32)]))
        e.w8(rt(0x92DB8), 0)
        for k_ in range(48):
            p = rt(SOLD) + k_ * 512
            e.w8(p + 0x182, r.choice([0, 1, 1, 1, 1]))
            e.w8(p + 0x138, r.choice([0, 1, 1, 1, 1]))
            e.w8(p + 0x183, r.choice([0, 14, 15, 50, 0x80]))
            e.w8(p + 0x189, r.choice([0, 1, 0x80]))
            for o in (0xB0, 0xCC, 0xA8, 0xC0, 0xC4, 0xB4, 0xBC, 0x1DC):
                e.w32(p + o, r.choice(TIMES + [r.randrange(1 << 32)]))
        for k_ in range(10):
            a = rt(ANIMS) + 0xB * k_
            e.w8(a, r.choice([0, 0, 1, 1, 2, 5, 0x7F, 0x80, 0xFF]))
            e.w32(a + 1, r.choice(TIMES))
            t = r.choice([r.randrange(3136), 0xC41, 0xD00])
            e.w16(a + 5, t)
            if t < 3136:
                e.w8(rt(TILES) + 8 * t + 3, r.choice([0, 1, 5, 9]))
        for k_ in range(16):
            a = rt(FXA) + 0x12 * k_
            e.w8(a, r.choice([0, 0, 1, 2, 5, 0x80, 0xFF]))
            e.w32(a + 1, r.choice(TIMES))
        for k_ in range(5):
            f = rt(FUSE) + 13 * k_
            e.w32(f, r.choice(TIMES))
            e.w8(f + 9, r.choice([0, 0, 1, 0xFF, 0x80]))
            e.w8(f + 0xC, r.choice([0, 0, 1, 0x80]))
        n = r.choice([1, 1, 1, 2, 3])
        # The port's input, read from the same memory.
        io = Io()
        for name, a, k in VALS:
            setattr(io, name, sgn(e.r8(rt(a)) if k == 1 else e.r32(rt(a)), k))
        for k_ in range(10):
            a = rt(ANIMS) + 0xB * k_
            io.anim_count[k_] = sgn(e.r8(a), 1)
            io.anim_timer[k_] = sgn(e.r32(a + 1), 4)
            io.anim_tile[k_] = struct.unpack("<h", bytes(e.uc.mem_read(a + 5, 2)))[0]
        for k_ in range(16):
            io.fx_timer[k_] = sgn(e.r32(rt(FXA) + 0x12 * k_ + 1), 4)
        rd = lambda a, k: bytes(e.uc.mem_read(rt(a), k))  # noqa: E741,E731
        sold, tiles, fxa, fuse = (ctypes.create_string_buffer(rd(a, k), k) for a, k in ((SOLD, N_SOLD), (TILES, N_TILES), (FXA, N_FXA), (FUSE, N_FUSE)))
        ranges = [(SOLD, N_SOLD), (TILES, N_TILES), (ANIMS, 0xB * 10), (FXA, N_FXA), (FUSE, N_FUSE)] + [(a, k) for _, a, k in VALS] + IGNORE
        ranges.sort()
        check_all = case < 40
        if check_all:
            before = rd(0x90000, 0x33A730 - 0x90000)
        for _ in range(n):
            e.call(rt(TIMER))
        if check_all:
            after = rd(0x90000, 0x33A730 - 0x90000)
            pos = 0x90000
            for a, k in ranges + [(0x33A730, 0)]:
                if before[pos - 0x90000:a - 0x90000] != after[pos - 0x90000:a - 0x90000]:
                    fail(case, f"the callback wrote outside the list, between {pos:#x} and {a:#x}")
                pos = max(pos, a + k)
        rc = P.ja1f_timer_step(ISO, EXE, ctypes.byref(io), sold, tiles, fxa, fuse, n)
        if rc:
            fail(case, f"rc {rc}")
            return
        ld2 = e.r8(rt(0x92DE1)) != 0
        lk = e.r8(rt(0x92DE0)) != 0
        seen["locked2" if ld2 else "locked" if lk else "open"] = seen.get("locked2" if ld2 else "locked" if lk else "open", 0) + 1
        for name, a, k in VALS:
            o = sgn(e.r8(rt(a)) if k == 1 else e.r32(rt(a)), k)
            if name == "lit" or name == "tick_step":
                continue
            if o != getattr(io, name):
                fail(case, f"{name}: original {o} port {getattr(io, name)} (n {n}, lock2 {int(ld2)}, lock {int(lk)})")
            if name == "panel_ends" and o == 1:
                seen["ends"] = seen.get("ends", 0) + 1
        for k_ in range(10):
            a = rt(ANIMS) + 0xB * k_
            o = (sgn(e.r8(a), 1), sgn(e.r32(a + 1), 4), struct.unpack("<h", bytes(e.uc.mem_read(a + 5, 2)))[0])
            p = (io.anim_count[k_], io.anim_timer[k_], io.anim_tile[k_])
            if o != p:
                fail(case, f"tile animation {k_}: original {o} port {p}")
        for k_ in range(16):
            o = sgn(e.r32(rt(FXA) + 0x12 * k_ + 1), 4)
            if o != io.fx_timer[k_]:
                fail(case, f"effect timer {k_}: original {o} port {io.fx_timer[k_]}")
        # (the effect entries' timers live in the port's fx_timers, so the entries compare outside bytes 1..4)
        fo, fp = bytearray(rd(FXA, N_FXA)), bytearray(fxa.raw)
        for k_ in range(16):
            fo[k_ * 0x12 + 1:k_ * 0x12 + 5] = fp[k_ * 0x12 + 1:k_ * 0x12 + 5] = b"\0" * 4
        for name, a, k, buf, want in (("soldiers", SOLD, N_SOLD, sold, None), ("tiles", TILES, N_TILES, tiles, None),
                                      ("effect animations", FXA, N_FXA, None, (fo, fp)), ("fuses", FUSE, N_FUSE, fuse, None)):
            x, y = want if want else (rd(a, k), buf.raw)
            if x != y:
                d = next(j for j in range(k) if x[j] != y[j])
                fail(case, f"{name} +{d:#x}: original {x[d]:#x} port {y[d]:#x}")


per = ncases // len(dumps)
for d in dumps:
    if not os.path.exists(d):
        print(f"timer_diff: {d} missing")
        sys.exit(1)
    run_dump(d, per)
for need in ("open", "locked", "locked2", "ends"):
    if seen.get(need, 0) < per // 100:
        print(f"timer_diff: {need} reached {seen.get(need, 0)} times only")
        bad += 1
print(f"timer_diff: {per * len(dumps)} cases {seen}, {bad} mismatches")
sys.exit(1 if bad else 0)
