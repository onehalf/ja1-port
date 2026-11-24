#!/usr/bin/env python3
"""Differential test: the hold of a pressed button, PressPanelButton? 0x35D44 and PressedFieldBox? 0x35ED4, original vs port.

usage: .venv/bin/python validate/diff/pressbtn_diff.py [ncases=2000] [DUMP] [--lib SO]
The bodies of 0x35D44 and 0x35ED4 run in Unicorn with the real MouseSavePos? 0x6742C, MouseRestorePos? 0x67444 and
MouseQueueNext? 0x6782C. Stubs: MouseQueuePop? 0x68470 (the tape: mouse moves and button levels), MouseSetPos? 0x681D0,
HidePointer 0x67BF0, ShowMouse? 0x673AC and the four face calls 0x35DA4, 0x35C0C, 0x35F0C, 0x35E84. A case: the press place,
the off byte 0x284F0B, the button or field number, and a tape.
Compared with pointer::ButtonHold (the code of each button in the window): queue rows used, and the place MouseSetPos? gets.
Original side only: the call order (pressed face, set place, hide, up face, show), the mouse bytes after the call, and the
extra show of MouseRestorePos? when the pointer is off. Exits 1 on mismatch.
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
ncases = int(args[0]) if args else 2000
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
if not os.path.exists(dump):
    print(f"pressbtn_diff: {dump} missing")
    sys.exit(1)
PORT = ctypes.CDLL(LIB)
MX, MY, BUTTONS, OFF = 0x229D00, 0x229D04, 0x229CF4, 0x284F0B
e = Emu(dump=dump, fast=True)
rt = e.rt
st = {"tape": [], "pos": 0, "log": []}


def pop(em, eax, edx, ebx, *_):
    if st["pos"] >= len(st["tape"]):
        raise EmuStop("tape end")
    x, y, b = st["tape"][st["pos"]]
    st["pos"] += 1
    em.w32(eax, x)
    em.w32(edx, y)
    em.w32(ebx, b)
    return 0


def logged(name):
    def f(em, eax=0, edx=0, *_):
        st["log"].append((name, eax & 0xFF) if name.startswith("face") else ("set", eax, edx) if name == "set" else (name,))
        return 0
    return f


e.stub(rt(0x68470), pop)
e.stub(rt(0x681D0), logged("set"))
e.stub(rt(0x67BF0), logged("hide"))
e.stub(rt(0x673AC), logged("show"))
for a, n in ((0x35DA4, "press"), (0x35C0C, "up"), (0x35F0C, "field press"), (0x35E84, "field up")):
    e.stub(rt(a), logged("face " + n))


def case(r):
    x, y = r.randrange(320), r.randrange(200)
    tape, cx, cy = [], x, y
    for _ in range(r.randint(0, 20)):
        if r.random() < 0.7:
            cx, cy = r.randrange(320), r.randrange(200)
        # The right button alone does not end the hold; only bit 0 keeps it.
        tape.append((cx, cy, r.choice((1, 1, 1, 3))))
    tape.append((r.randrange(320), r.randrange(200), r.choice((0, 0, 2))))
    return x, y, tape


def main():
    r = random.Random(0x35D44)
    bad, cover = 0, {"panel": 0, "field": 0, "moved": 0, "right ends not": 0, "pointer off": 0}
    for n in range(ncases):
        x, y, tape = case(r)
        field = n & 1 == 1
        b = r.randrange(0, 20) if field else r.choice((0x13, 0x14, 5, 6, 7, 8, 14))
        off = r.randrange(2)
        st.update(tape=tape, pos=0, log=[])
        e.w32(rt(MX), x)
        e.w32(rt(MY), y)
        e.w8(rt(BUTTONS), 1)
        e.w8(rt(OFF), off)
        e.call(rt(0x35ED4 if field else 0x35D44), b, max_steps=50_000)
        out = (ctypes.c_int * 3)()
        rc = PORT.ja1f_button_hold(x, y, bytes(t[2] for t in tape), len(tape), out)
        kind = "field" if field else "panel"
        press, up = (("face field press", "face field up") if field else ("face press", "face up"))
        show_extra = [("show",)] if off else []
        want = [(press, b), ("set", x, y)] + show_extra + [("hide",), (up, b), ("show",)]
        why = []
        if rc != 0 or [st["pos"], x, y] != list(out):
            why.append(f"rows or place: orig {st['pos']} ({x},{y}), port rc {rc} {list(out)}")
        if st["log"] != want:
            why.append(f"calls {st['log']} want {want}")
        if (e.r32(rt(MX)), e.r32(rt(MY))) != (x, y):
            why.append(f"mouse bytes {e.r32(rt(MX))},{e.r32(rt(MY))}")
        cover[kind] += 1
        cover["moved"] += any((t[0], t[1]) != (x, y) for t in tape)
        cover["right ends not"] += any(t[2] == 3 for t in tape)
        cover["pointer off"] += off
        if why:
            bad += 1
            if bad <= 5:
                print(f"case {n} {kind} {b}: {'; '.join(why)}")
    print(f"pressbtn_diff: {ncases} holds, {bad} mismatches; {cover}")
    if min(cover.values()) < ncases // 10:
        print("pressbtn_diff: a branch stays below 10 %")
        sys.exit(1)
    sys.exit(1 if bad else 0)


main()
