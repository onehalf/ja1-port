#!/usr/bin/env python3
"""Differential test: the speech wait (SpeechWait? 0x28034, panel.md "Speech balloon" step 7, "The wait"), original vs port.

usage: .venv/bin/python validate/diff/speechwait_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
The original runs in Unicorn on tac60. A case fuzzes the face point, mode b, the text time, the hold time, Subtitles
(byte 0x1163BD), the voice bytes (0x92F8B, 0x1163B8), the text and a fuzzed 148 x 69 face sheet at *0x93E70. MousePoll
0x678E8 reads a tape: one entry per pass of the time step, mouse (0x229D00, 0x229D04), buttons, key, voice playing
(SpeechPlaying? 0x5CA74) and silence byte 0x337C9F. Timers 0x47020 / 0x47040 read the tape's clock; Random 0x53538
is the port's AnimRng so both draw the same numbers. FaceEyeStep, FaceMouthStep, the frames, DrawText, BlitCopy and
MouseInRect are real. The face block on the screen is filled with a mark before each pass: a changed block = a redraw.
The port: ja1f_speech_wait = faceanim::SpeechWait over the same tape, painted with FaceSpot::paint + balloon::wait_text.
Compared: each pass's redraw and its 56 x 50 block, the pass the wait ends on, bytes 0x92F64 / 0x92F65, the plain face
at the end. Half the cases run the real PlayQuote 0x24F08 first, on a fake archive that holds any of the whole member,
A and B: its mode byte 0x284F23 picks the voice parts. The stubs of PlayVocSample 0x5C9C0, SpeechStream 0x5CC6C and
SpeechSeek? 0x5C16C log each part with its volume and member; each part plays for a fuzzed number of passes. The log must
match ja1f_quote_plan (sound::quote_plan, the parts cat_voice joins into one sample). A wait that ends first must give a
start of the plan. The port's tape says "playing" until the pass where the original runs out of parts. A line with no
member: mode 1 with no seek (#2 rows 53 and 62): the stream from where the last miss left it (logged under the A name: SpeechSeek? misses),
then the cry sample; the port's plan part 4 (kept bytes). Exits 1 on mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"speechwait_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB) if "--port" in argv else None
ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MX, MY, MB, SIL, F64, F65, SUB, VOICE, OPT, MODE, SPEC = (0x229D00, 0x229D04, 0x229CF4, 0x337C9F, 0x92F64, 0x92F65, 0x1163BD,
                                                          0x92F8B, 0x1163B8, 0x284F23, 0x96372)
e = Emu(dump=dump, fast=True)
rt = e.rt
sheet_p = e.alloc(148 * 69 + 16)
text_p = e.alloc(64)
st = {}
MARK = b"\xEE" * 56


def rect_get():
    scr, (x, y) = e.r32(rt(0x95678)), st["at"]
    return b"".join(bytes(e.uc.mem_read(scr + (y + j) * 320 + x, 56)) for j in range(50))


def rect_mark():
    scr, (x, y) = e.r32(rt(0x95678)), st["at"]
    for j in range(50):
        e.uc.mem_write(scr + (y + j) * 320 + x, MARK)


def note_pass():
    if st["pos"] > 0:
        b = rect_get()
        st["blocks"].append(None if b == MARK * 50 else b)
    rect_mark()


def poll(em, *_):
    note_pass()
    t = st["tape"][min(st["pos"], len(st["tape"]) - 1)]
    st["pos"] += 1
    st["cur"] = t
    st["clock"] = t[0]
    e.w32(rt(MX), t[1] & 0xFFFFFFFF)
    e.w32(rt(MY), t[2] & 0xFFFFFFFF)
    e.w32(rt(MB), t[3])
    e.uc.mem_write(rt(SIL), bytes([t[6]]))
    st["keys"] = [] if t[4] < 0 else ([0, t[4] & 0xFF] if t[4] >= 256 else [t[4]])
    return 0


def part_start(kind, vol):
    """a voice part starts: log it; it plays for the next fuzzed number of passes"""
    if st["durs"] is None:
        return 0
    st["parts"].append((kind, vol & 0xFF, st["seek"] if kind == "stream" else "voc"))
    st["part_end"] = st["pos"] + (st["durs"].pop(0) if st["durs"] else 3)
    st["over"] = None
    return 0


def speech_playing(em, *_):
    if st["durs"] is None:
        return 1 if st["cur"][5] else 0
    if st["pos"] < st["part_end"]:
        return 1
    if st["over"] is None:
        st["over"] = st["pos"]
    return 0


def arc_seek(em, eax, edx, ebx, ecx):
    """ArcSeek 0x534A4 on the fake archive: 0 = found"""
    name = bytes(e.uc.mem_read(ebx, 32)).split(b"\0")[0].decode()
    st["seek"] = name
    st["seeks"].append(name)
    return 0 if name in st["members"] else 0xFFFFFFFE


def speech_seek(em, eax, *_):
    st["seek"] = bytes(e.uc.mem_read(eax, 32)).split(b"\0")[0].decode()
    return 0


def timer_start(em, eax, *_):
    st["timers"][eax] = st["clock"]
    return 0


def timer_ms(em, eax, *_):
    return (st["clock"] - st["timers"][eax]) & 0xFFFFFFFF


def rand(em, eax, *_):
    # faceanim::AnimRng
    st["seed"] = (st["seed"] * 1103515245 + 12345) & 0xFFFFFFFF
    return ((st["seed"] >> 16) % eax) if 0 < eax < 0x80000000 else 0


def key_wait(em, *_):
    return 1 if st["keys"] else 0


def get_key(em, *_):
    return st["keys"].pop(0) if st["keys"] else 0


for a in (0x67BF0, 0x673AC, 0x5CCF8, 0x5C144, 0x5CE08, 0x67C54, 0x67C68,
          0x38304, 0x757B1, 0x758AD, 0x75A77, 0x5C0EC):    # the last 5: the text file and the archive open of PlayQuote
    e.stub(rt(a), lambda em, *_: 0)
e.stub(rt(0x5C9C0), lambda em, eax, edx, *_: part_start("voc", edx))
e.stub(rt(0x5CC6C), lambda em, eax, edx, *_: part_start("stream", edx))
e.stub(rt(0x5C16C), speech_seek)
e.stub(rt(0x534A4), arc_seek, 4)
e.stub_sprintf()
e.stub(rt(0x678E8), poll)
e.stub(rt(0x47020), timer_start)
e.stub(rt(0x47040), timer_ms)
e.stub(rt(0x53538), rand)
e.stub(rt(0x5616C), key_wait)
e.stub(rt(0x560F4), get_key)
e.stub(rt(0x5CA74), speech_playing)
e.w32(rt(0x93E70), sheet_p)

r = random.Random(11)
r2 = random.Random(12)
bad = ends = redraws = 0
kinds = {}
chains = {}
quote_buf = e.alloc(256)
PART = {0: "whole", 1: "A", 2: "voc", 3: "B"}
TEXTS = [b"", b"", b"JAMMED", b"NO AMMO", b"ITEM BROKEN"]
for case in range(ncases):
    x, y = r.choice([(59, 43), (8, 20), (60, 44), r.choice([(100, 70), (131, 25), (200, 100)])])
    b, sub = r.choice([0, 0, 1]), r.choice([0, 0, 1])
    voice, opt = r.choice([(0, 0), (0, 1), (1, 0), (1, 1), (1, 1)])
    text_ms = r.choice([0, 80, 400, 1600, 4000])
    hold = r.choice([0, 0, 500, 1000])
    text = r.choice(TEXTS)
    seed = r.getrandbits(32) | 1
    # the tape: times, the mouse near the rectangle's corners, sometimes a button or a key, the voice stops once
    tape, now = [], 0
    n = r.randrange(20, 120)
    speak_end = r.choice([0, 300, 1500, 1 << 30])
    stop_at = r.choice([-1, n // 3, n // 2, r.randrange(n)])
    press = r.choice([0, 0.02, 0.1])
    for k in range(n):
        step = r.choice([0, 1, 5, 20, 20, 60, 100, 300, 700])
        # exact times: the left button's 1000 ms, the text time, the hold time
        snap = [t for t in (1000, 1001, text_ms, text_ms + 1) if now < t <= now + step]
        at_edge = snap and r.random() < 0.7
        now = r.choice(snap) if at_edge else now + step
        # the rectangle is (x, y) .. (x + 56, y + 50), the ends included
        mx, my = r.choice([(x + 10, y + 10), (x, y), (x - 1, y), (x, y - 1), (x + 56, y + 50), (x + 57, y + 10),
                           (x + 10, y + 51), (5, 5), (x + 20, y + 20), (310, 190)])
        btn = r.choice([1, 1, 2]) if r.random() < press else 0
        if at_edge and now in (1000, 1001) and r.random() < 0.8:
            btn, mx, my = 1, 5, 5
        key = -1
        if k == stop_at:
            key = r.choice([13, 27, 32])
        elif r.random() < 0.05:
            key = r.choice([65, 12, 14, 26, 28, 0x100 + 13, 0x100 + 32])
        tape.append((now, mx, my, btn, key, 1 if now < speak_end else 0, r.choice([0, 0, 1])))
    now += 1
    tape.append((now, 5, 5, 0, 13, 0, 0))
    sheet = bytes(r.randrange(1, 255) for _ in range(148 * 69))
    e.uc.mem_write(sheet_p, sheet)
    e.uc.mem_write(text_p, text + b"\0")
    e.uc.mem_write(rt(SUB), bytes([sub]))
    e.uc.mem_write(rt(VOICE), bytes([voice]))
    e.uc.mem_write(rt(OPT), bytes([opt]))
    e.uc.mem_write(rt(MODE), b"\0")
    e.uc.mem_write(rt(SPEC), b"\0\0")
    e.uc.mem_write(rt(F64), b"\0\0")
    st.update(tape=tape, pos=0, clock=0, timers={}, keys=[], cur=tape[0], seed=seed | 1, blocks=[], at=(x + 1, y + 1),
              durs=None, parts=[], part_end=0, over=None, seek="", seeks=[], members=set())
    flags = None
    if r2.random() < 0.5:
        # Mostly voice on and few keys: the chain must get far.
        flags = r2.choice([(1, 0, 0), (0, 1, 0), (0, 0, 1), (0, 1, 1), (0, 1, 1), (1, 1, 1), (1, 1, 0), (0, 0, 0)])
        speaker, q = r2.randrange(60), r2.randrange(14)
        base = f"{speaker}AIM{q}"
        st["members"] = {base + t + ".8" for t, on in zip(("", "A", "B"), flags) if on}
        st["durs"] = [r2.randrange(1, 8) for _ in range(4)]
        e.call(rt(0x24F08), speaker, speaker * 14, q, 200, quote_buf, 0, speaker, 1, max_steps=3_000_000)
        st["seek_base"] = base
    rect_mark()
    e.call(rt(0x28034), x, y, text_ms, b, 1, text_p, hold, max_steps=30_000_000)
    plain = rect_get()
    f64, f65 = e.uc.mem_read(rt(F64), 2)
    if flags is not None:
        # what the original played, against the plan; the wait may end before the last part
        got = [(k, v, n) for k, v, n in st["parts"]]
        if voice == 0:
            plan = []
        else:
            pl = (ctypes.c_int32 * 8)()
            n = PORT.ja1f_quote_plan(*flags, pl) if PORT else 0
            plan = [("voc" if pl[2 * i] == 2 else "stream", pl[2 * i + 1],
                     "voc" if pl[2 * i] == 2 else st["seek_base"] + {0: "", 1: "A", 3: "B", 4: "A"}[pl[2 * i]] + ".8") for i in range(n)]
        if PORT and plan is not None and opt and voice:
            if got != plan[:len(got)] or (st["over"] is not None and got != plan) or (plan and not got):
                print(f"MISMATCH case {case} flags {flags}: the voice parts: original {got}, port {plan}")
                bad += 1
                if bad > 10:
                    break
            elif st["over"] is not None:
                chains[flags] = chains.get(flags, 0) + 1
    if not PORT:
        continue
    # The original's last pass drew no redraw of its own unless it did before it ended: at the end it paints the plain
    # face over it, so the last pass's picture is not visible.
    passes = st["pos"]
    m = len(tape)
    tp = (ctypes.c_int32 * (7 * m))()
    for k, t in enumerate(tape):
        key = t[4] if 0 <= t[4] < 256 else 0
        # a voice chain plays on the port's side as one sample, until the pass where the original ran out of parts
        speaking = t[5] if flags is None else int(st["over"] is None or k + 1 < st["over"])
        tp[7 * k:7 * k + 7] = [t[0], t[1], t[2], t[3], key, speaking, t[6]]
    ru = (ctypes.c_int32 * 4)(text_ms, int(voice != 0 and opt != 0), hold, sub)
    sp = (ctypes.c_int32 * 3)(x + 1, y + 1, b)
    out = (ctypes.c_int32 * (4 * (m + 1)))()
    blk = ctypes.create_string_buffer(2800 * (m + 1))
    ct = (ctypes.c_int32 * 1)()
    sh = (ctypes.c_ubyte * len(sheet)).from_buffer_copy(sheet)
    rc = PORT.ja1f_speech_wait(ISO, EXE, sh, ru, sp, text, ctypes.c_uint32(seed), tp, m, out, blk, ct)
    why = None
    p_passes = ct[0]
    p_end = out[4 * (p_passes - 1) + 2] if p_passes else 0
    if rc != 0:
        why = f"rc {rc}"
    elif p_passes != passes:
        why = f"the wait ended on pass: original {passes}, port {p_passes} (end {p_end})"
    elif (f65 != 0) != (p_end == 2) or (f64 != 0) != (p_end == 3):
        why = f"flags: original {f64}/{f65}, port end {p_end}"
    elif blk.raw[m * 2800:(m + 1) * 2800] != plain:
        why = "the plain face at the end"
    else:
        for k in range(passes - 1):
            o, pr = st["blocks"][k], out[4 * k]
            if (o is not None) != (pr != 0):
                why = f"pass {k}: redraw: original {o is not None}, port {pr}"
                break
            if o is not None:
                redraws += 1
                pb = blk.raw[k * 2800:(k + 1) * 2800]
                if o != pb:
                    j = next(j for j in range(2800) if o[j] != pb[j])
                    why = f"pass {k}: block ({j % 56},{j // 56}): original {o[j]}, port {pb[j]} (text {out[4 * k + 1]})"
                    break
    kinds[p_end] = kinds.get(p_end, 0) + 1
    if why:
        print(f"MISMATCH case {case} (at {x},{y} b {b} sub {sub} voice {voice}/{opt} time {text_ms} hold {hold} text {text}): {why}")
        bad += 1
        if bad > 10:
            break
print(f"speechwait_diff: {ncases} cases, ends {sorted(kinds.items())}, {redraws} redraws, full voice chains {sorted(chains.items())}, "
      f"{bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
