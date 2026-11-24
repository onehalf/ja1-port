#!/usr/bin/env python3
"""Differential test: an inventory remark PlayInvExtra? 0x25AD8 + its box wait ShowMessage? 0x27B98 + KeyOrClick 0x67E44, original vs port.

usage: .venv/bin/python validate/diff/invextra_diff.py [ncases=4000] [DUMP] [--lib SO]
The original runs in Unicorn; the archive calls, SpeechStream, MousePoll, SpeechRefill, the timer, the keyboard, WrapText, the
screen save and restore, the pointer and Lock are stubbed and logged. A tape of polls gives the time, the button level, a key and
the busy word 0x96372. The port: invextra::run (ja1f_inv_extra).
Compared: the sound member, rate and volume; box or no box and its text; the order and the mode of every poll; every refill; the
button rules (a button down at the start, a press, the release), the time limit; the keys left in the queue.
The 14 callers are read from the image: each (member, text) must be a pair of invscreen::extra_names.
On the original side only: the archive name and handle, the box's arguments, the close at the end, Lock + Unlock and the pointer.
Not compared: the box picture (msgbox_diff), which remark the library picks (inv_diff), SpeechStream with Sound FX off (the tape's
"not busy" stands for it).
Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu, EmuStop  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 4000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"invextra_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
PORT.ja1f_inv_extra.argtypes = [ctypes.c_int] * 5 + [ctypes.c_void_p, ctypes.c_int, ctypes.c_void_p, ctypes.c_int]
e = Emu(dump=dump, fast=True)
rt = e.rt
SUBS, BUSY, BUTTONS, RELOCK = 0x1163BD, 0x96372, 0x229CF4, 0x284F0B
SOUNDS = {1: "EMPTY.8", 2: "HMM.8", 3: "HUH.8", 4: "NOROOM.8"}
TEXTS = {0: "", 1: "Empty", 2: "Drank...", 3: "OK", 4: "No Room"}
bad = 0


def report(c, msg):
    global bad
    bad += 1
    if bad <= 12:
        print(f"case {c}: {msg}")


def text_at(a):
    return e.cstr(a).decode("latin-1")


def names_of(snd, txt):
    """The port's names for a pair: its log has them when there is a box and a start."""
    tape = (ctypes.c_int * 4)(0, 0, 0, 0)
    out = ctypes.create_string_buffer(400)
    n = PORT.ja1f_inv_extra(snd, txt, 1, 0, 0, tape, 1, out, 400)
    lines = out.raw[:n].decode("latin-1").split("\n")
    return (next(l.split()[1] for l in lines if l.startswith("start ")),
            next((l[4:] for l in lines if l.startswith("box ")), ""))


PAIRS = {names_of(s, t): (s, t) for s in SOUNDS for t in TEXTS}
assert len(PAIRS) == 20, len(PAIRS)
# The 14 callers: EAX and EDX as the bytes before the call load them; an EDX loaded far before is set by name.
SITES = {0x4928F: None, 0x492C2: None, 0x49A82: None, 0x49B26: None, 0x49C7C: None, 0x53A60: None, 0x53D38: 0x9AC17,
         0x53D4F: None, 0x53D8E: None, 0x53E59: 0x99BBD, 0x53E89: None, 0x5464A: None, 0x54704: None, 0x5484F: None}
seen = {}
for site, edx in SITES.items():
    code = bytes(e.uc.mem_read(rt(site - 10), 10))
    if code[5] != 0xB8 or (edx is None and code[0] != 0xBA):
        report(site, f"a caller loads its arguments in another way: {code.hex()}")
        continue
    # A dump holds the code with its fixups: the numbers in the code are addresses already.
    pa = int.from_bytes(code[6:10], "little")
    pd = rt(edx) if edx is not None else int.from_bytes(code[1:5], "little")
    member, text = text_at(pa), text_at(pd)
    pair = (member.upper(), text)
    if pair not in PAIRS:
        report(site, f"the port has no pair {pair}")
    else:
        seen[pair] = (pa, pd)
for want in [("HMM.8", "Drank..."), ("HMM.8", "OK"), ("HMM.8", ""), ("HUH.8", ""), ("EMPTY.8", "Empty"), ("NOROOM.8", "No Room")]:
    if want not in seen:
        report(0, f"no caller has {want}")
log, keys, st = [], [], {"tape": [], "i": 0, "ms": 0}


def sp(k):
    return e.r32(e.uc.reg_read(UC_X86_REG_ESP) + k)


def poll(em, a, *_):
    if st["i"] >= len(st["tape"]):
        raise EmuStop("tape end")
    ms, buttons, key, _busy = st["tape"][st["i"]]
    st["i"] += 1
    st["ms"] = ms
    log.append(f"poll {a}")
    e.w32(rt(BUTTONS), (e.r32(rt(BUTTONS)) & ~3) | buttons)
    if key >= 0:
        keys.append(key)
    return a


def refill(em, *_):
    log.append("refill")
    e.w16(rt(BUSY), st["tape"][st["i"] - 1][3])
    return 0


def stream(em, a, d, b, *_):
    log.append(("stream", a, d, b))
    e.w16(rt(BUSY), st["playing"])
    return 0


def wrap(em, a, *_):
    log.append(f"box {e.cstr(a).decode('latin-1')}")
    log.append(("wrapargs", sp(4), sp(8), sp(12)))
    return 0


e.stub(rt(0x5C0EC), lambda em, a, d, b, c, *_: log.append(("open", e.cstr(a))) or 7)
e.stub(rt(0x534A4), lambda em, a, d, b, c, *_: log.append(("seek", a, e.cstr(b).decode("latin-1"))) or 0, 4)
e.stub(rt(0x5CC6C), stream)
e.stub(rt(0x678E8), poll)
e.stub(rt(0x5CCF8), refill)
e.stub(rt(0x5C144), lambda em, *_: log.append("close") or 0)
e.stub(rt(0x47020), lambda em, a, *_: log.append("timer") or 0)
e.stub(rt(0x47040), lambda em, a, *_: st["ms"])
e.stub(rt(0x5616C), lambda em, *_: 1 if keys else 0)
e.stub(rt(0x560F4), lambda em, *_: keys.pop(0) if keys else 0)
e.stub(rt(0x362AC), wrap, 48)
e.stub(rt(0x467E4), lambda em, *_: log.append("save") or 0)
e.stub(rt(0x468B0), lambda em, *_: log.append("boxend") or 0)
e.stub(rt(0x67BF0), lambda em, *_: e.w8(rt(RELOCK), 1) or 0)
e.stub(rt(0x673AC), lambda em, *_: e.w8(rt(RELOCK), 0) or 0)
e.stub(rt(0x4B730), lambda em, *_: log.append("lock") or 0)
e.stub(rt(0x4B764), lambda em, *_: log.append("unlock") or 0)
rng = random.Random(25)
pairs = sorted(seen)
seen_kinds = {}
for c in range(ncases):
    pair = rng.choice(pairs)
    a, d = seen[pair]
    snd, txt = PAIRS[pair]
    subs = rng.random() < 0.7
    playing = int(rng.random() < 0.8)
    start = rng.choice((0, 0, 1, 1, 2, 3))
    tape, ms = [], 0
    level = start
    for i in range(rng.randrange(0, 24)):
        ms = 1000 if ms < 1000 and rng.random() < 0.08 else ms + rng.choice((0, 10, 55, 300))
        r = rng.random()
        if r < 0.3:
            level = rng.randrange(4)
        key = rng.choice((0x41, 0x1B, 0x20)) if rng.random() < 0.06 else -1
        tape.append((ms, level, key, int(rng.random() < 0.8)))
    tape += [(5000, 0, -1, 0)] * 6
    log.clear(); keys.clear()
    st.update(tape=tape, i=0, ms=0, playing=playing)
    e.w8(rt(SUBS), subs)
    e.w8(rt(RELOCK), 0)
    e.w16(rt(BUSY), 0)
    e.w32(rt(BUTTONS), start)
    try:
        e.call(rt(0x25AD8), a, d)
    except EmuStop as x:
        report(c, f"the original stopped: {x}")
        continue
    orig = []
    for l in log:
        if isinstance(l, str) and l not in ("close", "timer", "save", "lock", "unlock"):
            orig.append(l)
        elif isinstance(l, tuple) and l[0] == "seek":
            stream_call = next(x for x in log if isinstance(x, tuple) and x[0] == "stream")
            orig.append(f"start {l[2].upper()} {stream_call[1]} {stream_call[2]}")
    boxed = bool(pair[1]) and subs
    fixed = [l for l in log if isinstance(l, tuple)]
    if not any(x[0] == "stream" and x[3] == 0 for x in fixed) or ("open", b"INV\\extra") not in fixed or log[-1] != "close":
        report(c, f"the original's calls: {fixed} {log[-1]}")
    if boxed and not ({"save", "timer", "lock", "unlock"} <= {l for l in log if isinstance(l, str)}):
        report(c, f"the box calls: {log}")
    # the keys left in the queue: the port queues the ones the original did not read
    arr = (ctypes.c_int * (4 * len(tape)))(*[x for r in tape for x in r])
    out = ctypes.create_string_buffer(20000)
    n = PORT.ja1f_inv_extra(snd, txt, int(subs), playing, start, arr, len(tape), out, 20000)
    port = out.raw[:n].decode("latin-1").split("\n")[:-1]
    pq = [int(l[6:]) for l in port if l.startswith("queue ")]
    port = [l for l in port if not l.startswith("queue ")]
    # a port key queued at its poll; the original's key list also holds the keys of polls after a box that ended on this one
    if orig != port:
        report(c, f"{pair} subs {subs} playing {playing} start {start} tape {tape[:st['i'] + 1]}\n  orig {orig}\n  port {port}")
    if pq != keys:
        report(c, f"{pair} queue orig {keys} port {pq} tape {tape[:st['i'] + 1]}")
    k = ("box" if boxed else "nobox", "wait" if "poll 1" in orig else "nowait", "held" if start else "free",
         "press" if boxed and st["i"] > 1 else "-", "qkey" if pq else "-")
    seen_kinds[k] = seen_kinds.get(k, 0) + 1
if len(seen_kinds) < 8:
    report("kinds", f"too few kinds of case: {sorted(seen_kinds)}")
print(f"invextra_diff: {ncases} cases, {len(seen)} caller pairs, {len(seen_kinds)} kinds: {bad} mismatches")
sys.exit(1 if bad else 0)
