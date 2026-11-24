#!/usr/bin/env python3
"""Differential test: the timed input wait WaitMs? 0x67F48 and the key test EscPressed? 0x67300, original vs port.

usage: .venv/bin/python validate/diff/waitms_diff.py [ncases=3000] [DUMP] [--lib SO]
WaitMs? part: the original's body runs in Unicorn with the real KeyOrClick? 0x67E44. Stubs: TimerStart 0x47020,
TimerElapsedMs 0x47040 (the tape's time), MousePoll? 0x678E8 (the next tape row: buttons and new keys), KeyPressed 0x5616C
and GetKey 0x560F4 (the key queue; these two are the DOS keyboard). A case: ms, the start flags 0x284F1A / 0x284F1C, the old
key byte 0x95DC3 (0, a key, or Escape), and a tape of rows with held buttons, plain keys, Escape and extended keys (scan
codes include 0x1B). Compared: the answer byte, the rows used, the key byte, both flags, the bytes left in the queue.
EscPressed? part: the real body on a key queue, with several calls: each answer and the bytes left.
WaitKeyOrClick 0x67F34 part: the real body and the same stubs, no timer; the tape ends with a key and released buttons.
Compared: rows used, key byte, both flags, bytes left. The port: invextra::WaitKey through ja1f_waitkey.
The port: invextra::WaitMs and invextra::esc_pressed through ja1f_waitms / ja1f_esc_pressed, the code the window runs for the
battle show, Sleep and the day replay. The window's event loop is not compared (smoke_play battle_click runs it).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 3000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"waitms_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
e = Emu(dump=dump, fast=True)
rt = e.rt
MB, KEYBYTE, LEFT, RIGHT = 0x229CF4, 0x95DC3, 0x284F1A, 0x284F1C
st = {}


def poll(em, *_):
    pos = st["pos"]
    if pos >= len(st["tape"]):
        raise EmuStop("tape end")
    row = st["tape"][pos]
    st["pos"] = pos + 1
    em.w32(rt(MB), row[1])
    st["queue"].extend(row[2])
    return 0


def elapsed(em, *_):
    pos = st["pos"]
    if pos >= len(st["tape"]):
        raise EmuStop("tape end")
    return st["tape"][pos][0]


def key_pressed(em, *_):
    return 1 if st["queue"] else 0


def get_key(em, *_):
    return st["queue"].pop(0)


e.stub(rt(0x47020), lambda em, *_: 0)
e.stub(rt(0x47040), elapsed)
e.stub(rt(0x678E8), poll, pop=0)
e.stub(rt(0x5616C), key_pressed)
e.stub(rt(0x560F4), get_key)


def key_bytes(r):
    k = r.random()
    if k < 0.25:
        return [0x1B]
    if k < 0.55:
        return [r.choice([0x0D, 0x20, 0x41, 0x61, 0x7F, 0x80, 0xFF])]
    return [0, r.choice([0x1B, 0x3B, 0x48, 0x4D, 0x85])]


def case(r):
    ms = r.choice([1, 500, 700, 800, 1000, 2799, 2800, 30000])
    t, tape = 0, []
    pkey, pbtn = r.choice([0, 0.05, 0.3]), r.choice([0, 0.05, 0.4])
    level = r.choice([0, 0, 0, 1, 2, 3])
    for _ in range(r.randint(1, 30)):
        t += r.choice([0, 1, 10, 55, 110])
        # A row near the end time shows the edge: elapsed = ms ends, ms - 1 does not.
        if r.random() < 0.1:
            t = max(t, ms - r.randint(0, 1))
        if r.random() < pbtn:
            level = r.randint(0, 3)
        keys = []
        while r.random() < pkey and len(keys) < 6:
            keys += key_bytes(r)
        tape.append((min(t, 65535), level, keys))
    # The tape always ends the wait, so both sides stop by the wait's own rule.
    tape.append((65535, level, []))
    tape += [(65535, 0, [])] * 4
    return ms, r.choice([0, 0, 1, 2, 3]), r.choice([0, 0, 0x41, 0x1B]), tape


def orig_wait(ms, flags, old, tape):
    st.update(tape=tape, pos=0, queue=[])
    e.w8(rt(LEFT), flags & 1)
    e.w8(rt(RIGHT), flags >> 1 & 1)
    e.w8(rt(KEYBYTE), old)
    a = e.call(rt(0x67F48), ms) & 0xFF
    return [a, st["pos"], e.r8(rt(KEYBYTE)), e.r8(rt(LEFT)) | e.r8(rt(RIGHT)) << 1, len(st["queue"])]


def port_wait(ms, flags, old, tape):
    raw, keys = bytearray(), bytearray()
    for t, b, k in tape:
        # A row's arrival count is one byte.
        raw += bytes((t & 0xFF, t >> 8, b, len(k)))
        keys += bytes(k)
    iv = (ctypes.c_int * 8)(ms, flags, old)
    PORT.ja1f_waitms(bytes(raw), len(tape), bytes(keys) or b"\0", len(keys), iv)
    return list(iv)[3:]


def key_case(r):
    pkey, pbtn = r.choice([0, 0.05, 0.3]), r.choice([0.05, 0.4])
    level, tape = r.choice([0, 0, 0, 1, 2, 3]), []
    for _ in range(r.randint(0, 25)):
        if r.random() < pbtn:
            level = r.randint(0, 3)
        keys = []
        while r.random() < pkey and len(keys) < 6:
            keys += key_bytes(r)
        tape.append((0, level, keys))
    tape.append((0, 0, [0x41]))              # ends a wait that is not in a release; level 0 ends one that is
    tape += [(0, 0, [])] * 3
    return r.choice([0, 0, 1, 2, 3]), r.choice([0, 0, 0x41, 0x1B]), tape


def orig_key(flags, old, tape):
    st.update(tape=tape, pos=0, queue=[])
    e.w8(rt(LEFT), flags & 1)
    e.w8(rt(RIGHT), flags >> 1 & 1)
    e.w8(rt(KEYBYTE), old)
    e.call(rt(0x67F34))
    return [st["pos"], e.r8(rt(KEYBYTE)), e.r8(rt(LEFT)) | e.r8(rt(RIGHT)) << 1, len(st["queue"])]


def port_key(flags, old, tape):
    raw, keys = bytearray(), bytearray()
    for t, b, k in tape:
        raw += bytes((0, 0, b, len(k)))
        keys += bytes(k)
    iv = (ctypes.c_int * 8)(0, flags, old)
    PORT.ja1f_waitkey(bytes(raw), len(tape), bytes(keys) or b"\0", len(keys), iv)
    return list(iv)[4:]


def main():
    r = random.Random(0x67F48)
    bad = esc_cases = ended = timed = escs = clicks = 0
    for n in range(ncases):
        c = case(r)
        o, p = orig_wait(*c), port_wait(*c)
        ended += 1
        timed += o[1] < len(c[3]) and o[0] == 0
        escs += o[0] == 1
        clicks += c[3][max(o[1] - 1, 0)][1] != 0
        if o != p:
            bad += 1
            if bad <= 5:
                print(f"wait case {n}: ms {c[0]} flags {c[1]} old {c[2]:#x}\n orig {o}\n port {p}\n tape {c[3][:12]}")
    for n in range(ncases // 3):
        q = []
        for _ in range(r.randint(0, 6)):
            q += key_bytes(r)
        calls = r.randint(1, 8)
        st.update(queue=list(q), tape=[], pos=0)
        oa = [e.call(rt(0x67300)) & 0xFF for _ in range(calls)]
        ol = len(st["queue"])
        out = (ctypes.c_ubyte * calls)()
        pl = PORT.ja1f_esc_pressed(bytes(q) or b"\0", len(q), calls, out)
        esc_cases += 1
        if oa != list(out) or ol != pl:
            bad += 1
            if bad <= 5:
                print(f"esc case {n}: keys {q} orig {oa} left {ol} port {list(out)} left {pl}")
    key_ends = {"key": 0, "left": 0, "right": 0}
    for n in range(ncases):
        c = key_case(r)
        o, p = orig_key(*c), port_key(*c)
        used = c[2][o[0] - 1]
        prev = c[2][o[0] - 2][1] if o[0] > 1 else 0
        key_ends["left" if prev & 1 and not used[1] & 1 else "right" if prev & 2 and not used[1] & 2 else "key"] += 1
        if o != p:
            bad += 1
            if bad <= 5:
                print(f"key wait case {n}: flags {c[0]} old {c[1]:#x}\n orig {o}\n port {p}\n tape {c[2][:12]}")
    print(f"waitms_diff: WaitKeyOrClick {ncases} waits {key_ends}")
    if min(key_ends.values()) < ncases // 30:
        print("waitms_diff: the key wait cases do not reach each end")
        sys.exit(1)
    print(f"waitms_diff: {ended} waits ({escs} Escape, {timed} no-Escape ends, {clicks} with a button), {esc_cases} EscPressed "
          f"queues, {bad} mismatches")
    if escs < ncases // 20 or clicks < ncases // 20:
        print("waitms_diff: the cases do not reach Escape and clicks")
        sys.exit(1)
    sys.exit(1 if bad else 0)


main()
