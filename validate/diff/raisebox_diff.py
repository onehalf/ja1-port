"""Differential test: the salary box of the morning raise (strategic.md "Salary raise", host op 297), original vs port.

usage: .venv/bin/python validate/diff/raisebox_diff.py [ncases=200] [DUMP] [--port] [--lib SO]
Per case: a profile with a face in MAIN.DAT, a nickname, a start offer (fuzzed around the step limits 500 / 1000 /
2500 / 5000 and the top 25000) and a press list (down / up / OK, then OK and NO so the box ends) over a noise screen.
The original runs SalaryDialog? 0x29074 (text 0x99E47 "Raise salary?") with its number entry 0x2573C, the field
0x25414, the OK shade 0x2560C, 0x256B8 and the buttons for real in Unicorn on tac60: the panel buffer *0x93E68 holds
PANEL_P.PCC and the face sheet *0x93E70 the profile's face (as the merc message 0x50908 before it leaves them), the
PCC loader 0x532BC loads MAIN.DAT members by name (MYBRUSH.PCC into the panel sheet), the mouse calls and LoadPanels
0x46EC4 are stubbed, and the button test 0x67CD8 answers the case's presses (a press of a button that the round does
not test is dropped at the next round). The screen is taken at the start of every round. Then the pressed face of a
random button (DrawPanelButtonDown? 0x35DA4) on the last screen.
--port: ja1f_salary draws the box, takes the same presses and gives the screen before each press, the offer and the
pressed face; screens and offer are compared. Exits 1 on any mismatch.
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
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = None
if "--port" in argv:
    PORT = ctypes.CDLL(LIB)
    PORT.ja1f_salary.argtypes = [ctypes.c_char_p] * 2 + [ctypes.c_int32, ctypes.c_char_p, ctypes.c_int32,
                                                         ctypes.c_void_p, ctypes.c_int32, ctypes.c_int32,
                                                         ctypes.c_void_p, ctypes.c_void_p]
    PORT.ja1f_salary.restype = ctypes.c_int32
    ISO, EXE = os.path.join(ROOT, "rom", "JA1.gog").encode(), os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

MAIN = open(os.path.join(ROOT, "rom", "DAT", "MAIN.DAT"), "rb").read()
SCREEN_P = 0x95678
BUTTON = [5, 6, 0x11, 0x12]          # press k (0 NO, 1 OK, 2 down, 3 up) = button number
_pics = {}


def pic(name):
    n = name.upper()
    if n not in _pics:
        w, h, px, _ = pcx.decode(datarc.member(MAIN, n))
        _pics[n] = (w, h, px)
    return _pics[n]


def put_pic(addr, name, pitch):
    w, h, px = pic(name)
    for y in range(h):
        e.uc.mem_write(addr + y * pitch, bytes(px[y * w:(y + 1) * w]))


e = Emu(dump=dump, fast=True)


def load(em, eax, edx, ebx, ecx):
    put_pic(ecx, e.cstr(ebx).decode("latin-1"), e.r32(e.uc.reg_read(UC_X86_REG_ESP) + 4))
    return 0


for a in (0x678E8, 0x67BF0, 0x673AC, 0x67E18, 0x67E30, 0x681D0, 0x67C54, 0x46EC4, 0x5C8A0, 0x5C900):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x532BC), load, 4)
screen = e.r32(e.rt(SCREEN_P))
st = {}


def button_test(em, eax, edx, ebx, ecx):
    b = eax & 0xFF
    if b == 5:                       # a new round of 0x2573C
        st["snaps"].append(bytes(em.uc.mem_read(screen, 64000)))
        if st["round"] and not st["taken"]:
            st["i"] += 1             # the last round did not test the pressed button: the press is dropped
        st["round"] += 1
        st["taken"] = False
        if st["round"] > len(st["keys"]) + 2:
            raise RuntimeError("the box does not end")
    i = st["i"]
    if i < len(st["keys"]) and BUTTON[st["keys"][i]] == b:
        st["i"] += 1
        st["taken"] = True
        return 1
    return 0


e.stub(e.rt(0x67CD8), button_test)
faces = [n for n in (m[0] if isinstance(m, tuple) else m for m in datarc.members(MAIN)) if n.isdigit() and int(n) < 256]
NICK = e.alloc(64)
# the answer rule after the box divides by soldier 0's salary: keep it above 0
e.uc.mem_write(e.rt(0xFA958) + 0x110, (500).to_bytes(2, "little"))
r = random.Random(7)
bad = 0
for i in range(ncases):
    prof = int(r.choice(faces))
    nick = r.choice([b"Ivan", b"Grunty", b"MD", b"Wolf", b"", bytes(r.randint(0x21, 0x7E) for _ in range(r.randint(1, 12)))])
    start = r.choice([r.randint(1, 6000), r.choice([495, 500, 995, 1000, 2495, 2500, 4995, 5000]) + r.randint(-6, 6),
                      r.randint(24800, 25100), r.randint(-30, 30), r.randint(25100, 32767)])
    n = r.choice([0, 1, 2, 4, 8, 16, 30])
    keys = [r.choice([1, 2, 2, 3, 3, 3]) for _ in range(n)] + [1, 0]
    if i % 4 == 0:                       # the top: the offer lands on 25000 exactly, and up stays there (0x2573C)
        start, k_ = r.choice([(25000, 2), (24900, 3), (24800, 4), (24700, 6)])
        keys = [3] * k_ + [1, 0]
    e.uc.mem_write(screen, bytes(r.randrange(256) for _ in range(64000)))
    base = bytes(e.uc.mem_read(screen, 64000))
    put_pic(e.r32(e.rt(0x93E68)), "PANEL_P.PCC", 216)
    put_pic(e.r32(e.rt(0x93E70)), str(prof), 0x94)
    e.w8(e.rt(0x284F0C), 0)
    e.uc.mem_write(NICK, nick + b"\0")
    st.update(snaps=[], keys=keys, i=0, round=0, taken=False)
    rc = e.call(e.rt(0x29074), e.rt(0x99E47), prof, NICK, 0, 0, start & 0xFFFF if start >= 0 else start & 0xFFFFFFFF)
    rounds = len(st["snaps"])
    offer = e.r16(e.rt(0x22A470))
    offer = offer - 0x10000 if offer >= 0x8000 else offer
    face = r.randrange(4)
    e.call(e.rt(0x35DA4), BUTTON[face])
    last = bytes(e.uc.mem_read(screen, 64000))
    if not PORT:
        continue
    pk = (ctypes.c_int32 * len(keys))(*keys)
    frames = ctypes.create_string_buffer(64000 * len(keys))
    buf = ctypes.create_string_buffer(base, 64000)
    po = PORT.ja1f_salary(ISO, EXE, prof, nick, start if start < 0x8000 else start - 0x10000, pk, len(keys), face,
                          frames, buf)
    msg = []
    if po != offer:
        msg.append(f"offer {offer} vs {po}")
    for k in range(rounds):
        o, q = st["snaps"][k], frames.raw[64000 * k:64000 * (k + 1)]
        if o != q:
            n_ = next(j for j in range(64000) if o[j] != q[j])
            msg.append(f"round {k} (keys {keys[:k]}) @({n_ % 320},{n_ // 320}) {o[n_]:#x} vs {q[n_]:#x}")
            break
    if not msg and buf.raw != last:
        n_ = next(j for j in range(64000) if buf.raw[j] != last[j])
        msg.append(f"pressed face {face} @({n_ % 320},{n_ // 320}) {last[n_]:#x} vs {buf.raw[n_]:#x}")
    if msg:
        print(f"MISMATCH case {i} profile {prof} {nick!r} start {start} keys {keys}: {'; '.join(msg)}")
        bad += 1
        if bad > 10:
            break
print(f"raisebox_diff: {ncases} cases, {bad} mismatches" + ("" if PORT else " (original only)"))
sys.exit(1 if bad else 0)
