"""Differential test: the question box (panel.md "Question box"), original vs port.

usage: .venv/bin/python validate/diff/question_diff.py [ncases=300] [DUMP] [--port] [--lib SO] [--face]
--face: the merc-face question box 0x28C84 instead (a face of MAIN.DAT, a nickname, PANEL_P / PANEL_B by byte
0x284F0C; the screen at the input, after an optional press, is compared).
Per case: a text (the island map's question first, then fuzzed words as in msgbox_diff with the wrap width 129), a
picture (Q_MARK.PCC, MERGE.PCC or another MAIN.DAT picture), every other case an item picture (type, colour,
place; the discard question's is (26, 18) with colour 0) and a noise screen. The original runs 0x278A0 (save 1,
restore 1) in Unicorn on tac60 with the PCC loader 0x532BC stubbed (MAIN.DAT member by name); the screen is taken
at the input 0x2538C (which answers the case's yes / no) and must be back to the noise after. Every third case also
runs the press 0x35DA4 (button 5 or 6) on that screen. The port (port/ja1front ja1f_question) draws the box (and the
pressed face) on the same noise; the 320 x 200 screens are compared.
Without --port only the original runs (a smoke test of the harness). Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                  # noqa: E402
from unicorn.x86_const import UC_X86_REG_ESP   # noqa: E402
import datarc, pcx                   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 300
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_question.argtypes = [ctypes.c_char_p] * 4 + [ctypes.c_int32, ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32]
    PORT.ja1f_question.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
PICS = ["Q_MARK.PCC", "MERGE.PCC", "Q_MARK.PCC", "POPINV.PCC"]
QUESTIONS = ["Start day without anyone assigned to duty?", "Get to work?",
             "Raise natives' salary? \xb1 (don't expect to lower it later)"]
SCREEN_P = 0x95678
_pics = {}


def pic(name):
    n = name.decode("latin-1").upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    return _pics[n]


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    w, h, px = pic(e.cstr(ebx))
    pitch = e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4)
    for y in range(h):
        e.uc.mem_write(ecx + y * pitch, bytes(px[y * w:(y + 1) * w]))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x4B730, 0x4B764, 0x47020, 0x47040, 0x681D0):
    e.stub(e.rt(a), lambda em, *_: 0)
if "--mouse" not in argv:
    for a in (0x67E18, 0x67E30):
        e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
held, answer = [], [0]
screen = e.r32(e.rt(SCREEN_P))
e.stub(e.rt(0x2538C), lambda em, *_: held.append(bytes(em.uc.mem_read(screen, 64000))) or answer[0])
# the text-only box (ECX 0) waits in SpeechWait? 0x28034 (EBX = ms); `waited` keeps that time
waited = []
e.stub(e.rt(0x28034), lambda em, a, d, b, c: held.append(bytes(em.uc.mem_read(screen, 64000))) or waited.append(b) or 0, 12)
TEXT, PIC = e.alloc(1024), e.alloc(64)


def fuzz(r):
    words = []
    for _ in range(r.choice([1, 2, 3, 5, 8, 12])):
        k = r.random()
        if k < 0.04:
            words.append("\xb1")
        elif k < 0.08:
            words.append("".join(r.choice("MW@#") for _ in range(r.randint(14, 22))))    # wider than 129
        else:
            words.append("".join(chr(r.randint(0x21, 0x7E)) for _ in range(r.choice([1, 2, 3, 4, 6, 9]))))
    sep = lambda: " " * r.choice([1, 1, 1, 2])
    s = sep().join(words)
    if r.random() < 0.2:
        s = " " + s
    return s.encode("latin-1")


r = random.Random(1)
bad = restored = 0
if "--mouse" in argv:    # YesNoPrompt? 0x28D88 and 0x28F52: the mouse moves into the box (Subtitles on), and with a
    # stack argument (the A.I.M. quote box) it comes back. The real 0x67E18 / 0x67E30 run; 0x681D0 logs its place.
    if PORT:
        PORT.ja1f_prompt_mouse.argtypes = [ctypes.c_int32] * 6 + [ctypes.c_void_p]
        PORT.ja1f_prompt_mouse.restype = ctypes.c_int32
    MX, MY, SUB = e.rt(0x229D00), e.rt(0x229D04), e.rt(0x1163BD)
    calls, move = [], [(0, 0)]
    NICK = e.alloc(64)
    e.uc.mem_write(NICK, b"Ivan\0")
    e.uc.mem_write(TEXT, b"Fire Ivan?\0")

    def setpos(em, a, d, *_):
        calls.append((a, d))
        em.w32(MX, a)
        em.w32(MY, d)
        return 0

    def wait(em, *_):
        em.w32(MX, em.r32(MX) + move[0][0])
        em.w32(MY, em.r32(MY) + move[0][1])
        return 0
    e.stub(e.rt(0x681D0), setpos)
    e.stub(e.rt(0x2538C), wait)
    e.stub(e.rt(0x28034), wait, 12)
    moved = back = 0
    edge_x, edge_y = (0, 51, 52, 53, 200, 266, 267, 268, 319), (0, 36, 37, 38, 80, 115, 116, 117, 199)
    for i in range(ncases):
        sub, ecx, stack = i & 1, (i >> 1) & 1, (i >> 2) & 1
        x, y = r.choice(edge_x), r.choice(edge_y)
        dx, dy = r.choice((0, 9, -9, 10, -10, 3)), r.choice((0, 9, -9, 10, -10, 3))
        e.w8(SUB, sub)
        e.w32(MX, x)
        e.w32(MY, y)
        calls.clear()
        move[0] = (dx, dy)
        e.call(e.rt(0x28C84), TEXT, 1, NICK, ecx, stack)
        moved += any(c == (237, 102) for c in calls)
        back += stack and len(calls) > (1 if sub and calls and calls[0] == (237, 102) else 0)
        if PORT:
            out = (ctypes.c_int32 * 4)()
            n = PORT.ja1f_prompt_mouse(sub, x, y, stack, dx, dy, out)
            q = [(out[2 * k], out[2 * k + 1]) for k in range(n)]
            if q != calls:
                print(f"MISMATCH case {i} sub {sub} ecx {ecx} stack {stack} mouse ({x},{y}) move ({dx},{dy}): {calls} vs port {q}")
                bad += 1
                if bad > 10:
                    break
    print(f"question_diff --mouse: {ncases} cases, {moved} moved, {back} back, {bad} mismatches" + ("" if PORT else " (original only)"))
    sys.exit(1 if bad or not moved or not back else 0)
if "--face" in argv:     # panel.md "Merc-face question box": 0x28C84(text, DL profile, EBX nickname, ECX 1, stack 0)
    if PORT:
        PORT.ja1f_face_question.argtypes = [ctypes.c_char_p] * 3 + [ctypes.c_int32, ctypes.c_char_p, ctypes.c_int32,
                                                                    ctypes.c_int32, ctypes.c_void_p]
        PORT.ja1f_face_question.restype = ctypes.c_int32
    import datarc as _d
    faces = [n for n in (m[0] if isinstance(m, tuple) else m for m in _d.members(MAIN)) if n.isdigit() and int(n) < 256]
    NICK = e.alloc(64)
    for i in range(ncases):
        s = b"Fire Ivan?" if i == 0 else b"Accept this native guide?" if i == 1 else fuzz(r)
        prof = int(r.choice(faces))
        nick = r.choice([b"Ivan", b"Grunty", b"MD", b"Wolf", b"", bytes(r.randint(0x21, 0x7E) for _ in range(r.randint(1, 12)))])
        office = r.random() < 0.3
        base = bytes(r.randrange(256) for _ in range(64000))
        e.uc.mem_write(screen, base)
        e.uc.mem_write(TEXT, s + b"\0")
        e.uc.mem_write(NICK, nick + b"\0")
        e.w8(e.rt(0x284F0C), int(office))
        held.clear()
        answer[0] = i & 1
        text_only = i % 3 == 1           # ECX 0: a merc speaks with Subtitles on (text only, the speech wait)
        if text_only:                    # a stored quote holds no control byte and no "!" (it would decode to a
            s = s.replace(b"\xb1", b"Q").replace(b"!", b"Q")    # space, but spaces are stored as they are)
            e.uc.mem_write(TEXT, s + b"\0")
        waited.clear()
        rc = e.call(e.rt(0x28C84), TEXT, prof, NICK, 0 if text_only else 1, 0)
        if text_only and waited != [80 * len(s)]:
            print(f"case {i}: the text box waits {waited}, not 80 ms x {len(s)} bytes")
            bad += 1
        if text_only:
            rc = answer[0]
        if len(held) != 1 or (rc & 0xFF) != answer[0]:
            print(f"case {i}: input ran {len(held)} times, answer {rc & 0xFF}")
            bad += 1
            continue
        o = held[0]
        pressed = -2 if text_only else -1
        if i % 3 == 2:
            pressed = r.randrange(2)
            e.call(e.rt(0x35DA4), 5 + pressed)
            o = bytes(e.uc.mem_read(screen, 64000))
        if PORT:
            buf = ctypes.create_string_buffer(base, 64000)
            # the text-only box takes the quote as stored (+1 encoding) and decodes it (every byte but the space
            # taken - 1); the port's callers pass the decoded text
            ps = bytes(c if c == 0x20 else (c - 1) & 0xFF for c in s) if text_only else s
            rc = PORT.ja1f_face_question(ISO, EXE, ps, prof, nick, int(office), pressed, buf)
            q = buf.raw
            if rc != 0 or q != o:
                n_ = next((k for k in range(64000) if q[k] != o[k]), -1)
                print(f"MISMATCH face case {i} profile {prof} {nick!r} office {office} pressed {pressed} {s[:40]!r}: rc {rc}, "
                      f"screen @({n_ % 320},{n_ // 320}) {o[n_]:#x} vs {q[n_]:#x}" if n_ >= 0 else f"MISMATCH case {i}: rc {rc}")
                bad += 1
                if bad > 10:
                    break
    fires = 0
    if PORT:
        # The day-end Fire question (DayEndScreen? 0x65BD4: sprintf "%s%s?" with "Fire " and the nickname at +0x16E, then the
        # face question): the pictures of MapUi::payroll_fire_imgs = the face question just compared with the original.
        save = open(os.path.join(ROOT, "build", "DAY_day3.SAV"), "rb").read()
        under = bytes(r.randrange(256) for _ in range(64000))
        for k in range(8):
            out, rec = ctypes.create_string_buffer(3 * 64000), ctypes.create_string_buffer(512)
            rc = PORT.ja1f_payroll_fire(ISO, EXE, save, len(save), k, under, out, rec)
            if rec.raw[0x182] == 0:
                continue
            nick = rec.raw[0x16E:0x16E + 9].split(b"\0")[0]
            fires += 1
            for f, pressed in enumerate((-1, 0, 1)):
                buf = ctypes.create_string_buffer(under, 64000)
                PORT.ja1f_face_question(ISO, EXE, b"Fire " + nick + b"?", rec.raw[0x179], nick, 0, pressed, buf)
                if rc != 3 or out.raw[f * 64000:(f + 1) * 64000] != buf.raw:
                    print(f"MISMATCH day-end Fire slot {k} {nick!r} picture {f}: rc {rc}")
                    bad += 1
        if not fires:
            sys.exit("no merc in the day save for the Fire question")
    print(f"question_diff --face: {ncases} cases + {fires} day-end Fire questions, {bad} mismatches" + ("" if PORT else " (original only)"))
    sys.exit(1 if bad else 0)
cases = [(q.encode("latin-1"), "Q_MARK.PCC") for q in QUESTIONS]
# The tactical ABORT / RESTART questions (AskAbortDay 0x11DC0, panel.md "Abort and restart questions").
cases += [(b"\xb1 ABORT this day?", "!MARK.PCC"), (b"\xb1 RESTART this day?", "Q_MARK.PCC")]
cases += [(fuzz(r), r.choice(PICS)) for _ in range(max(0, ncases - len(cases)))]
for i, (s, pn) in enumerate(cases):
    base = bytes(r.randrange(256) for _ in range(64000))
    e.uc.mem_write(screen, base)
    e.uc.mem_write(TEXT, s + b"\0")
    e.uc.mem_write(PIC, pn.lower().encode() + b"\0")
    held.clear()
    answer[0] = i & 1
    t = ix = iy = ic = 0                 # every other fuzzed case: an item picture (the discard question: (26, 18), 0)
    if i >= len(QUESTIONS) and i % 2:
        t, ic = r.randrange(1, 0x71), r.choice([0, 0, 8, 0x0A])
        ix, iy = (26, 18) if r.random() < .5 else (r.randrange(0, 170), r.randrange(0, 60))
    t2 = ix2 = iy2 = 0                   # every fourth case with an item: a second one (MergeItemsPrompt: colour 10)
    if t and i % 4 == 1:
        t2 = r.randrange(1, 0x71)
        ix2, iy2 = (20, 50) if r.random() < .5 else (r.randrange(0, 170), r.randrange(0, 60))
    rc = e.call(e.rt(0x278A0), TEXT, PIC, t, ic, ix, iy, t2, ix2, iy2, 1, 1, 0)
    after = bytes(e.uc.mem_read(screen, 64000))
    if len(held) != 1 or (rc & 0xFF) != answer[0]:
        print(f"case {i}: input ran {len(held)} times, answer {rc & 0xFF}")
        bad += 1
        continue
    o = held[0]
    if after == base:
        restored += 1
    else:
        print(f"case {i}: the screen after the box is not the screen before")
        bad += 1
    pressed = -1
    if i % 3 == 2:
        pressed = r.randrange(2)
        e.uc.mem_write(screen, o)
        e.call(e.rt(0x35DA4), 5 + pressed)
        o = bytes(e.uc.mem_read(screen, 64000))
    if PORT:
        buf = ctypes.create_string_buffer(base, 64000)
        item = e.r8(e.rt(0x94DFC) + 12 * t) | ic << 8 | ix << 16 | iy << 24 if t else -1
        item2 = e.r8(e.rt(0x94DFC) + 12 * t2) | 10 << 8 | ix2 << 16 | iy2 << 24 if t2 else -1
        rc = PORT.ja1f_question(ISO, EXE, s, pn.encode(), pressed, buf, item, item2)
        p = buf.raw
        if rc != 0 or p != o:
            n = next((k for k in range(64000) if p[k] != o[k]), -1)
            print(f"MISMATCH case {i} {pn} pressed {pressed} {s[:50]!r}: rc {rc}, screen @({n % 320},{n // 320}) "
                  f"{o[n]:#x} vs {p[n]:#x}" if n >= 0 else f"MISMATCH case {i}: rc {rc}")
            bad += 1
            if bad > 10:
                break
print(f"question_diff: {len(cases)} cases ({restored} restored), {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
