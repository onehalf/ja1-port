"""Differential test: the tactical Control Panel's frame around its input loop (ControlPanel 0x4B788 without
ControlPanelInput 0x4BAA4; panel.md "The Control Panel" and "Other users of Lock"), original vs port.

usage: .venv/bin/python validate/diff/ctlshell_diff.py [ncases=200] [DUMP] [--lib SO]
Per case: the lock state before (round_lock 0x92DE0, lock_owned 0x92DE2, clock switch 0x92DE5, blink byte 0x284F06,
blink_timer 0x229D48), the close result that ControlPanelInput returns (-1, 0, 16, 99, 100, 101, 102, 0x10064, 0x1FFFF: the
original compares the low 16 bits), the in-combat byte and the Quick Restore presence. The original ControlPanel runs
whole in Unicorn on tac60 with ControlPanelInput stubbed to return that result, RedrawChoice 0x1CB78, the pointer
calls 0x67BF0 / 0x673AC / 0x67C54 and SetMouseLimit? 0x682AC stubbed and logged, QuickSave 0x2D880 and QuickRestore
0x2C898 stubbed and logged; DrawText runs for real on a scratch back buffer (the picture is ctlpanel_diff's work).
The port (ja1f_ctl_shell: pause::Pause lock / unlock, the calls of PlayState::locked_ticks, and
ctlpanel::after_close, which the window and the script use) gets the same state and result.
Compared: the 5 lock bytes after, and which of QuickSave / QuickRestore runs. Asserted on the original side only
(the port has no such state): QuickRestore gets EAX = 1, RedrawChoice runs once more before either, the last mouse
limit is the whole screen (0, 0, 319, 199) (the port's window clamps only inside the panel loop), and the pointer
byte 0x92E4F is 0 after (the mouse driver's box flag; the port has no mouse driver).
Exits 1 on any mismatch.
"""
import ctypes, os, random, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
from emu import Emu                 # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("--") and (i == 0 or argv[i - 1] != "--lib")]
ncases = int(args[0]) if args else 200
dump = args[1] if len(args) > 1 else os.path.join(ROOT, "build", "mem_tac60.bin")
PORT = ctypes.CDLL(LIB)
PORT.ja1f_ctl_shell.argtypes = [ctypes.c_void_p, ctypes.c_int32]
PORT.ja1f_ctl_shell.restype = ctypes.c_int32

SCREEN_P, PANEL_ON, COMBAT, SAVEANY = 0x95678, 0x92DCC, 0x92DCB, 0x92DC3
ROUND_LOCK, LOCK_OWNED, CLOCK, BLINK, BLINK_T, BOXFLAG = 0x92DE0, 0x92DE2, 0x92DE5, 0x284F06, 0x229D48, 0x92E4F
RESULTS = (-1, 0, 16, 99, 100, 101, 102, 0x10064, 0x1FFFF)

e = Emu(dump=dump, fast=True)
screen = e.alloc(bytes(64000))
e.w32(e.rt(SCREEN_P), screen)
log = []
for a in (0x67BF0, 0x673AC, 0x67C54):
    e.stub(e.rt(a), lambda em, *_: 0)
e.stub(e.rt(0x1CB78), lambda em, *_: log.append(("redraw",)) or 0)
e.stub(e.rt(0x682AC), lambda em, a, d, b, c: log.append(("limit", a, d, b, c)) or 0)
e.stub(e.rt(0x2D880), lambda em, *_: log.append(("save",)) or 0)
e.stub(e.rt(0x2C898), lambda em, a, *_: log.append(("restore", a)) or 0)
restore_flag = [0]
e.stub(e.rt(0x4707C), lambda em, *_: restore_flag[0])
result = [0]
e.stub(e.rt(0x4BAA4), lambda em, *_: log.append(("input",)) or result[0] & 0xFFFFFFFF)
e.w8(e.rt(PANEL_ON), 1)
e.w8(e.rt(SAVEANY), 0)

r = random.Random(0x4B788)
bad = 0
seen = {"save": 0, "restore": 0, "none": 0, "owned_kept": 0, "taken": 0}
for i in range(ncases):
    st = [r.choice([0, 1]), r.choice([0, 1]), r.choice([0, 1]), r.choice([0, 1]),
          r.choice([0, 400, 800, -10, r.randrange(-2000, 2000)])]
    result[0] = RESULTS[i % len(RESULTS)]
    restore_flag[0] = r.choice([0, 1])
    e.w8(e.rt(COMBAT), r.choice([0, 1]))
    e.w8(e.rt(ROUND_LOCK), st[0])
    e.w8(e.rt(LOCK_OWNED), st[1])
    e.w8(e.rt(CLOCK), st[2])
    e.w8(e.rt(BLINK), st[3])
    e.w32(e.rt(BLINK_T), st[4] & 0xFFFFFFFF)
    e.w8(e.rt(BOXFLAG), r.choice([0, 1]))
    log.clear()
    e.call(e.rt(0x4B788))
    t = e.r32(e.rt(BLINK_T))
    o = [e.r8(e.rt(ROUND_LOCK)), e.r8(e.rt(LOCK_OWNED)), e.r8(e.rt(CLOCK)), e.r8(e.rt(BLINK)),
         t - (1 << 32) if t >> 31 else t]
    calls = [c for c in log if c[0] in ("save", "restore")]
    o_disp = -1 if not calls else (1 if calls[0][0] == "save" else 0)
    why = []
    if len(calls) > 1 or (calls and calls[0][0] == "restore" and calls[0][1] != 1):
        why.append(f"original dispatch {calls}")
    if calls and log[log.index(calls[0]) - 1] != ("redraw",):
        why.append(f"original: no redraw before {calls[0]}: {log}")
    limits = [c for c in log if c[0] == "limit"]
    if not limits or limits[-1][1:] != (0, 0, 319, 199):
        why.append(f"original last mouse limit {limits[-1:]}")
    if e.r8(e.rt(BOXFLAG)) != 0:
        why.append("original pointer byte 0x92E4F not 0")
    pst = (ctypes.c_int32 * 5)(*st)
    p_disp = PORT.ja1f_ctl_shell(pst, result[0])
    if list(pst) != o:
        why.append(f"lock state original {o} port {list(pst)}")
    if p_disp != o_disp:
        why.append(f"dispatch original {o_disp} port {p_disp}")
    seen[("none", "restore", "save")[o_disp + 1]] += 1
    seen["taken"] += st[0] == 0
    seen["owned_kept"] += st[0] == 1 and st[1] == 1
    if why:
        bad += 1
        print(f"MISMATCH case {i} (state {st} result {result[0]}): " + "; ".join(why))
        if bad > 10:
            break
if 0 in seen.values():
    print(f"MISMATCH: a branch never ran {seen}")
    bad += 1
print(f"CASES ctlshell={ncases}")
print(f"ctlshell_diff: {ncases} cases, {bad} mismatches ({seen})")
sys.exit(1 if bad else 0)
