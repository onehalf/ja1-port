"""Differential test (in ./t check): the clicks of the tile cursor (docs/spec/cursor.md "Clicks") of the clean-room
front-end (port/ja1front ja1f_click, on the library world) vs the original UseItemAt? 0x1D9B8 with the left or the
right mouse button down, in Unicorn on mem_fight59.

usage: .venv/bin/python validate/diff/click_diff.py [ncases=3000] [DUMP] [--lib SO]
Stubbed in the original: the drawing stubs of hover_diff, the mouse poll 0x678E8 (all buttons up; WaitLeftUp 0x67C54 pops the scripted polls until the left button is up), KeyPressed 0x5616C
(no key), 0x6782C 0x682AC, TacticalRedraw 0x176C8, 0x5C880, the item info 0x1D950, the banner 0x23B4C, the merc
sound 0x17B70 (recorded at UseItemAt's 3 sites), ShowMercMessage? 0x299B8; recorded: StatusMessage? 0x27D24, MsgMercText 0x15910, and the library calls
TacticalClick 0x59C14, MsgReloaded? 0x40094 (Reload), SelectMerc? 0x1148C (scripted results). MoveMercTo 0x1C608
runs for real up to its walk (0x1C8AA: recorded with the walk AP of a confirm, then it returns 0); its texts before
0x1C797 (the gates) do not count (the port's ja1_move_gate shows none). The confirm-move option 0x1163C6 is random.
AimLoop's polls 0x6782C / KeyPressed 0x5616C / GetKey 0x560F4 are scripted in half of the aim cases (right clicks, idle
polls, other keys, then a left click or Escape; each right click logs AP, colours and the text 0x229DE0 at WaitRightUp).
WaitConfirmClick? 0x1C514 is scripted (a left click on T = 1; Escape: tile 0xC41; the right button or a click off T:
tile 0xC41, item_timer 2000; else 0); the port gets the same answer. AimLoop 0x1D744 runs for real: the left button is
still down, so it fires at once (the port answers the aim with a left click). In AimLoop one ClickSound runs per right
click and the limit 0x682AC holds the mouse, then frees the screen: the test ends red if these rules change.
The right button is a hold (cursor.md "Clicks", right step 3): the mouse poll gives a scripted sequence of 1 to 3
polls (buttons, mouse y; the last one with the right button up), the same sequence goes to the port; recorded:
TurnTo 0x1AA18 (scripted result), SetAnim 0x17F90, the look panel routine 0x248E4.
Compared: the cursor and hover state, the library calls with their arguments, the status line text, and the whole
library world. A left click with the mine detector 0x4D: TacticalClick's stub with result 2 runs MoveMercTo with confirm 1
(the port: ja1_click_detector, cursor.rs detector_move). Exits 1 on any mismatch.
"""
import ctypes, os, random, struct, sys
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.join(HERE, "..", "..")
sys.path[:0] = [HERE, os.path.join(ROOT, "tools")]
import ai                            # noqa: E402
import port_lib                      # noqa: E402
import livecast                      # noqa: E402
from emu import Emu                  # noqa: E402
from unicorn import UC_HOOK_CODE                # noqa: E402
from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_ESP, UC_X86_REG_EBP   # noqa: E402

argv = sys.argv[1:]
LIB = argv[argv.index("--lib") + 1] if "--lib" in argv else os.path.join(ROOT, "port", "ja1front", "target", "release",
                                                                         "libja1front.so")
args = [a for i, a in enumerate(argv) if not a.startswith("-") and (i == 0 or argv[i - 1] not in ("--lib", "--only"))]
ncases = int(args[0]) if args and args[0].isdigit() else 3000
ONLY = int(argv[argv.index("--only") + 1]) if "--only" in argv else None     # debug: run case N only, verbose
dump = next((a for a in args if not a.isdigit()), os.path.join(ROOT, "build", "mem_fight59.bin"))

u8, i8, i16, i32 = ctypes.c_uint8, ctypes.c_int8, ctypes.c_int16, ctypes.c_int32


class CursorState(ctypes.Structure):
    _fields_ = [("over_map", u8), ("tactical_on", u8), ("overlay_up", u8), ("value", u8), ("mode", u8),
                ("out_of_range", u8), ("combat", u8), ("ap_fg", u8), ("ap_shadow", u8), ("blink_phase", u8),
                ("no_blink", u8), ("under_soldier", u8), ("wobble_dir", i8), ("pad", u8 * 3), ("tile", i32),
                ("ap", i32), ("blink_timer", i32), ("wobble", i32), ("wobble_dx", i16), ("wobble_dy", i16),
                ("label_x", i16), ("label_y", i16), ("label", u8 * 16),
                ("marker_kind", i32), ("marker_tile", i32), ("marker_life", i32), ("marker_blink", i32),
                ("marker_phase", u8), ("pad2", u8 * 3),
                ("shown_slot", i32), ("spotted", i32), ("view_tile", i32), ("mark_timer", i32),
                ("mark_phase", i8), ("pad3", u8 * 3)]


class HoverState(ctypes.Structure):
    _fields_ = [("ap_timer", i32), ("item_timer", i32), ("light_timer", i32), ("target", i32), ("lit", u8),
                ("named", u8), ("light_wait", u8), ("redraw_off", u8)]


PORT = ctypes.CDLL(LIB)
PORT.ja1f_click.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host), ctypes.c_char_p,
                            ctypes.POINTER(CursorState), ctypes.POINTER(HoverState)] + [i32] * 4 + [
                            ctypes.POINTER(i32)] * 3 + [ctypes.c_char_p, ctypes.POINTER(i32), ctypes.POINTER(i32), ctypes.POINTER(i32)]
PORT.ja1f_click.restype = i32
PORT.ja1f_press_walks.argtypes = [ctypes.POINTER(port_lib.World), ctypes.POINTER(port_lib.Host), ctypes.c_char_p,
                                  ctypes.POINTER(CursorState), ctypes.POINTER(HoverState), i32, i32]
PORT.ja1f_press_walks.restype = i32
EXE = os.path.join(ROOT, "build", "game113b", "JA.EXE").encode()

C = dict(over_map=(0x284F13, "B"), tactical_on=(0x92DCC, "B"), overlay_up=(0x92DCE, "B"), value=(0x284F1D, "B"),
         mode=(0x284F1B, "B"), out_of_range=(0x955C8, "B"), ap_fg=(0x92E5C, "i"), ap_shadow=(0x92E60, "i"),
         wobble_dir=(0x284F05, "b"), tile=(0x229D2C, "i"), ap=(0x229DEC, "i"), wobble=(0x229D3C, "i"),
         no_blink=(0x95E0D, "B"))
H = dict(ap_timer=(0x229DD4, "i"), item_timer=(0x229DDC, "i"), light_timer=(0x93D50, "i"), target=(0x229C4C, "i"),
         lit=(0x284F0E, "B"), named=(0x95E0C, "B"), light_wait=(0x93D4C, "B"), redraw_off=(0x92DDF, "B"))
SEL, SHIFT, ALT, PREVIEW, BUTTONS, LABEL, STRIP = 0x284F11, 0x284F21, 0x284F20, 0x229DD8, 0x229CF4, 0x22A444, 0x93448
MOUSE_X, MOUSE_Y, PRESS_Y, COMBAT = 0x229D00, 0x229D04, 0x229C44, 0x92DCB
SOLDIERS = 0xFA958
VALUES = [0x8E, 0x8F, 0x91, 0xB5, 0xA1, 0xA4, 0x94, 0x77, 0x8C, 0x92, 0x95, 0x96, 0x97, 0x98, 0x74, 0x75, 0x76,
          0x93, 0x8B, 0x8D, 0xA2]

e = Emu(dump=dump, fast=True)
for a in (0x67BF0, 0x331A4, 0x31C98, 0x32EB8, 0x335F8, 0x682AC, 0x176C8, 0x5C880,
          0x1D950, 0x23B4C):
    e.stub(e.rt(a), lambda em, *r: 0)


def effect_slot(em, eax, edx, ebx, ecx):
    """EffectSlot 0x17B70: recorded as kind 10 at the three sites of UseItemAt (the OK sound after TacticalClick). The site
    in MoveMercTo 0x1C983 is the library's own op 65."""
    if em.r32(em.uc.reg_read(UC_X86_REG_ESP)) in (e.rt(0x1DC8D), e.rt(0x1E08E), e.rt(0x1E103)):
        calls.append((10, eax & 0xFF, edx & 0xFF))
    return 0


e.stub(e.rt(0x17B70), effect_slot)


def merc_msg(em, eax, edx, ebx, ecx):
    """ShowMercMessage? 0x299B8 (EAX merc, DL quote, BL near; one stack argument): recorded as kind 8 outside the gates"""
    if not gating[0]:
        calls.append((8, slot_of(eax), edx & 0xFF, ebx & 0xFF))
    return 0


e.stub(e.rt(0x299B8), merc_msg, pop=4)
calls, rets = [], [0, 0, 0, 0, 0, 0, 0]
polls = []                    # the scripted mouse polls of a right button hold: (buttons, mouse y)
CONFIRM_MOVE = 0x1163C6
FAST_FIRE = 0x1163C5          # Control Panel switch 13 Fast Firing, random (rets[6])
gating = [False]


def slot_of(p):
    return (p - e.rt(SOLDIERS)) // 512


def poll(em, *r):
    b, y = polls.pop(0) if polls else (0, rd(MOUSE_Y, "i"))
    em.uc.mem_write(em.rt(BUTTONS), bytes([b]))
    wr(MOUSE_Y, "i", y)


def lib_stub(kind):
    def f(em, eax, edx, ebx, ecx):
        if kind == 1:
            calls.append((1, slot_of(eax), edx, ebx & 0xFF))
            if ebx & 0xFF == 0x4D and rets[0] == 2:   # the rest of the click: MoveMercTo (S, T, 1, 0, 1, 0, 1, 0)
                sp = em.uc.reg_read(UC_X86_REG_ESP)
                ra = em.r32(sp)
                em.uc.reg_write(UC_X86_REG_ESP, sp - 0x14)
                for k, v in enumerate((em.rt(0x1C608), ra, 1, 0, 1, 0)):
                    em.w32(sp - 0x14 + 4 * k, v)
                em.uc.reg_write(UC_X86_REG_EBX, 1)
                em.uc.reg_write(UC_X86_REG_ECX, 0)
                return eax
            return rets[0]
        if kind == 2:
            calls.append((2, slot_of(eax)))
            return rets[1]
        if kind == 3:
            calls.append((3, eax & 0xFF, edx & 0xFF, ebx & 0xFF))
            if rets[2]:
                em.uc.mem_write(em.rt(SEL), bytes([eax & 0xFF]))   # as SelectMerc does
            return rets[2]
    return f


def walk_hook(em, *_):
    """MoveMercTo 0x1C8AA (the walk starts): record it, go on at 0x1C891 (result 0)"""
    sp = em.uc.reg_read(UC_X86_REG_ESP)
    walk = em.r32(sp) if em.r8(sp + 0x14) else 0xFFFFFFFF
    calls.append((4, slot_of(em.uc.reg_read(UC_X86_REG_EBP)), em.r32(sp + 8) & 0xFFFF, em.r8(sp + 0x10),
                  em.r8(sp + 0xC), em.r8(sp + 0x2C), struct.unpack("<i", struct.pack("<I", walk))[0], em.r8(sp + 0x28)))
    em.uc.reg_write(UC_X86_REG_ESP, sp - 4)
    em.w32(sp - 4, em.rt(0x1C891))


def wait_confirm(em, eax, *_):
    """WaitConfirmClick? 0x1C514 (EAX tile), scripted by rets[3]"""
    if rets[3] == 0:
        em.w32(em.rt(0x229D2C), eax)
        return 1
    em.w32(em.rt(0x229D2C), 0xC41)
    if rets[3] != 1:
        em.w32(em.rt(0x229DDC), 2000)
    return 0


def gate_flag(v):
    def f(uc, *_):
        gating[0] = v
    return f


def text_stub(kind):
    def f(em, eax, edx, ebx, ecx):
        if not gating[0]:
            calls.append((kind, em.cstr(eax if kind == "status" else edx).decode("latin-1")))
        return 0
    return f


e.stub(e.rt(0x678E8), poll)


def wait_left_up(em, *r):
    """WaitLeftUp 0x67C54: polls (the scripted ones) until a poll has the left button up (cursor.md right 3.1).
    Left-button cases have no scripted polls: an instant return keeps the button state for the waits that follow
    (aim, confirm), which the port answers as open waits."""
    while polls:
        poll(em)
        if not em.uc.mem_read(em.rt(BUTTONS), 1)[0] & 1:
            return 0


e.stub(e.rt(0x67C54), wait_left_up)
aim_script, aim_key = [], [0]
AIM_LOOP = (e.rt(0x1D744), e.rt(0x1D8A0))


def in_aim(em):
    return AIM_LOOP[0] <= em.r32(em.uc.reg_read(UC_X86_REG_ESP)) < AIM_LOOP[1]


def aim_poll(em, *r):
    """MouseQueueNext 0x6782C inside AimLoop: the next scripted poll (no script: the buttons stay, left is down)."""
    if in_aim(em) and aim_script:
        b, aim_key[0] = aim_script.pop(0)
        em.uc.mem_write(em.rt(BUTTONS), bytes([b]))
    return 0


def aim_wait_right(em, *r):
    """WaitRightUp 0x67C68: AimLoop's right click is complete here."""
    if in_aim(em):
        ap = rd(0x229DEC, "i")
        row = (9, ap, rd(0x92E5C, "i") & 0xFF, rd(0x92E60, "i") & 0xFF)
        text = em.cstr(em.rt(0x229DE0)).decode("latin-1")
        calls.append(row if text == str(ap) else row + (text,))
    return 0


def aim_key_read(em, *r):
    k, aim_key[0] = aim_key[0], 0
    return k


aim_clicks, aim_limits = [0], [0, 0, 0]


def aim_click_sound(em, *r):
    """ClickSound 0x5C880 inside AimLoop: one per right click (play.rs answers Right with one; window_aim_hold counts it)."""
    aim_clicks[0] += in_aim(em)
    return 0


def aim_limit(em, eax, edx, ebx, ecx):
    """SetMouseLimit? 0x682AC inside AimLoop: first the mouse's own place (main.rs holds it there), at the end the screen."""
    if in_aim(em):
        mx, my = em.r32(em.rt(0x229D00)), em.r32(em.rt(0x229D04))
        pin = (eax, edx, ebx, ecx) == (mx, my, mx, my)
        free = (eax, edx, ebx, ecx) == (0, 0, 0x13F, 0xC7)
        aim_limits[0 if pin else 1 if free else 2] += 1
    return 0


e.stub(e.rt(0x5C880), aim_click_sound)
e.stub(e.rt(0x682AC), aim_limit)
e.stub(e.rt(0x6782C), aim_poll)
e.stub(e.rt(0x67C68), aim_wait_right)
e.stub(e.rt(0x5616C), lambda em, *r: 1 if aim_key[0] and in_aim(em) else 0)
e.stub(e.rt(0x560F4), aim_key_read)
e.stub(e.rt(0x1C514), wait_confirm)
e.stub(e.rt(0x59C14), lib_stub(1))
e.stub(e.rt(0x40094), lib_stub(2))
e.stub(e.rt(0x1148C), lib_stub(3))
e.stub(e.rt(0x1C8AA), walk_hook)
e.stub(e.rt(0x1AA18), lambda em, eax, edx, ebx, ecx: (calls.append((5, slot_of(eax), edx & 0xFF, ebx & 0xFF)), rets[5])[1])
e.stub(e.rt(0x17F90), lambda em, eax, edx, ebx, ecx: calls.append((6, slot_of(eax), edx & 0xFFFF, ebx & 0xFF, ecx & 0xFF)))
e.stub(e.rt(0x248E4), lambda em, *r: calls.append((7,)))
press_orig, press_port = [0], [0]


def press_hook(uc, *_):
    press_orig[0] = 1


e.uc.hook_add(UC_HOOK_CODE, press_hook, None, e.rt(0x1CFEC), e.rt(0x1CFEC))
for a, v in ((0x1C608, True), (0x1C797, False)):
    e.uc.hook_add(UC_HOOK_CODE, gate_flag(v), None, e.rt(a), e.rt(a))
e.uc.ctl_flush_tb()
e.stub(e.rt(0x27D24), text_stub("status"))
e.stub(e.rt(0x15910), text_stub("merc"))
OBJ2 = (e.rt(0x90000), 0x33A730 - 0x90000)
obj2_base = bytes(e.uc.mem_read(*OBJ2))
base_bufs, base_sc = port_lib.world_load(e, ai)
base_bufs = {k: bytes(v) for k, v in base_bufs.items()}
tiles = base_bufs["tiles"]
vx, vy = (struct.unpack("<i", e.uc.mem_read(e.rt(a), 4))[0] for a in (0x229CF8, 0x229CFC))
c0, r0 = vx // 16, vy // 14
occupied = [t for t in range(3136) if (tiles[8 * t + 5] & 0x7F) < 0x30]
own = {tiles[8 * t + 5] & 0x7F: t for t in occupied if (tiles[8 * t + 5] & 0x7F) < 8}
mercs = [t for t in occupied if (tiles[8 * t + 5] & 0x7F) < 8]
enemies = [t for t in occupied if (tiles[8 * t + 5] & 0x7F) >= 0x28]
doors = [t for t in range(3136) if base_bufs["t1_class"][tiles[8 * t + 1]] > 5 and tiles[8 * t + 1] in
         (0x3D, 0x3E, 0x5C, 0x5D, 0x43, 0x45, 0x62, 0x64)]
items = [t for t in range(3136) if tiles[8 * t + 6] < 200]
itypes = base_bufs["item_types"]
hand = [k for k in range(1, len(itypes) // 12) if itypes[12 * k + 2] in (0x92, 0x95, 0x96)] or [0]
other = [k for k in range(1, len(itypes) // 12) if itypes[12 * k + 2] not in (0x92, 0x95, 0x96)] or [0]
print(f"click_diff: view ({c0},{r0}), {len(mercs)} merc / {len(enemies)} enemy tiles, {len(doors)} doors, "
      f"{len(items)} item tiles, {len(hand)} weapon types")


def wr(a, f, v):
    e.uc.mem_write(e.rt(a), struct.pack("<" + f, v))


def rd(a, f):
    return struct.unpack("<" + f, e.uc.mem_read(e.rt(a), struct.calcsize(f)))[0]


def case(r):
    k = r.random()
    if k < .25 and enemies:
        t = r.choice(enemies)
    elif k < .4 and mercs:
        t = r.choice(mercs)
    elif k < .55 and doors:
        t = r.choice(doors)
    elif k < .65 and items:
        t = r.choice(items)
    else:
        t = min(max(r0 + r.randrange(-1, 13), 0), 55) * 56 + min(max(c0 + r.randrange(-1, 15), 0), 55)
    c = dict(over_map=0, tactical_on=r.choice([1] * 12 + [0]), overlay_up=r.choice([0] * 12 + [1]),
             value=r.choice(VALUES), mode=r.choice([1, 1, 2, 2, 2, 3, 0]), out_of_range=r.randrange(2),
             ap_fg=0xFF, ap_shadow=0xFE, wobble_dir=r.choice([0, -2, 1]),
             tile=r.choice([t, t, 0xC41, r.randrange(3136)]), ap=r.choice([0, 0, r.randrange(1, 30)]),
             wobble=r.choice([-1000, -1000, 550, 10, 0, -5]), no_blink=0)
    h = dict(ap_timer=r.choice([800, 800, 10, 20, 0, r.randrange(-20, 900)]), item_timer=r.choice([2000, 10, 0, 20]),
             light_timer=r.choice([300, 5, 0, -10]), target=r.choice([0x40, r.randrange(48)]),
             lit=r.choice([0x40, 0x40, r.randrange(8)]), named=r.choice([0x40, 0x40, r.randrange(48)]),
             light_wait=r.randrange(2), redraw_off=r.choice([0, 0, 0, 1, 2]))
    sel = r.choice(list(range(8)) * 8 + [8])
    m = dict(hp=r.choice([None, None, None, 10, 0, 14, 15]), b1B8=r.choice([None, None, 0, 1]),
             b1BE=r.choice([0, 0, 0, 1]), b1BF=r.choice([0, 0, 0, 1]), b16A=r.randrange(2),
             apLeft=r.choice([None, None, r.randrange(0, 30), 3]),
             hand=r.choice([None, r.choice(hand), r.choice(hand), r.choice(other), 0]),
             cond=r.choice([None, None, 5, 50]), count=r.choice([None, None, 0, 1]), inCur=r.choice([None] * 6 + [0]),
             b19C=r.choice([None] * 4 + [3]), uD0=r.choice([None, None, r.choice(occupied or [0]), 0xC41]),
             u114=r.choice([None] * 8 + [t]), b1E2=r.choice([None, None, 0, 5]))
    g = dict(shift=r.choice([0, 0, 1]), alt=r.choice([0, 0, 1]), preview=r.choice([0, 0, 1]),
             combat=r.choice([None, None, 0, 1]), button=r.choice([1, 1, 1, 2]))
    ret = [r.choice([0, 1]), r.choice([0, 0, 1]), r.choice([0, 1, 1]), r.choice([0, 0, 1, 2, 3]),
           r.choice([0, 1, 1]), r.choice([0, 1, 1]), r.choice([0, 0, 0, 1])]
    # the hold: 1 to 3 polls, the last one with the right button up (a drag down: mouse y + 11 or more)
    pl = [(r.choice([2, 2, 3, 3, 1, 0]), 100 + r.choice([0, 0, 5, 11, 20])) for _ in range(r.choice([0, 0, 1, 2]))]
    pl.append((r.choice([0, 0, 0, 1]), 100 + r.choice([0, 0, 0, 5, 11])))
    m["facing"] = r.choice([None, None, None, r.randrange(1, 9)])
    if g["button"] == 2 and sel in own and r.random() < .3:      # (the posture: a press on the merc's own tile)
        t = own[sel]
    if g["button"] == 1 and g["alt"] and sel in own and r.random() < .3:  # (Alt left on the own tile: the posture)
        t = own[sel]
    m["anim"] = r.choice([None, None, 4, 5, 2])
    g["aim"] = []
    if g["button"] == 1 and r.random() < .6:
        rights = r.choice([0, 1, 1, 2, 3, 5, 6])
        g["aim"] = [r.choice([(2, 0), (2, 0), (2, 0), (0, 0), (0, 0x61)]) for _ in range(rights)]
        g["aim"].append(r.choice([(1, 0), (1, 0), (0, 27), (1, 27), (3, 0)]))
    if enemies and r.random() < .3:      # bias: the aim (a loaded gun on a tile in a fight)
        t = r.choice(enemies + [r.randrange(3136)])
        g.update(button=1, shift=0, alt=0, combat=1, preview=0)
        ret[6] = 0
        c.update(tactical_on=1, overlay_up=0, mode=r.choice([1, 2, 3]))
        h["redraw_off"] = r.choice([0, 0, 1])
        m.update(hp=None, hand=r.choice(hand), cond=50, count=1, inCur=None, b19C=None, u114=None, b1E2=None, uD0=None,
                 apLeft=r.randrange(1, 40), b16A=r.randrange(2), b1B8=None, b1BE=0, b1BF=0)
        g["aim"] = [r.choice([(2, 0), (2, 0), (2, 0), (0, 0), (0, 0x61)]) for _ in range(r.choice([0, 1, 2, 3, 5, 6]))]
        g["aim"].append(r.choice([(1, 0), (1, 0), (0, 27), (1, 27), (3, 0)]))
    return t, c, h, sel, m, g, ret, pl


def edge_case(r, base):
    """Own stream (the main cases stay): a shot whose TargetMemory? 0x1CAD0 terms stand at their edges: the hand
    type around 1..16 (a weapon by the item table) and the occupant around 0x28 with its +0x9C."""
    t, c, h, sel, m, g, ret, pl = base
    t = r.choice(enemies)
    g.update(button=1, shift=0, alt=0, combat=1, preview=0)
    ret[:] = [1, 0, 1, 0, 1, 1, 0]
    c.update(tactical_on=1, overlay_up=0, mode=2, value=0x95, tile=t)
    h.update(redraw_off=0)
    m.update(hp=None, hand=r.choice([1, 15, 16, 16, 17, 17, 18]), cond=50, count=1, inCur=None, b19C=None, u114=None,
             b1E2=None, uD0=None, apLeft=r.randrange(20, 40), b16A=r.randrange(2), b1B8=None, b1BE=0, b1BF=0)
    g["aim"] = [(1, 0)]
    m["edge"] = dict(occ=r.choice([8, 0x26, 0x27, 0x27, 0x28, 0x28, 0x29, 0x2F, None]), hit=r.choice([1, 1, 1, 0, 0xFF]))
    return t, c, h, sel, m, g, ret, pl


def detector_case(r, base):
    """Own stream: a left click with the mine detector 0x4D in hand. Result 2 of the click: MoveMercTo runs for real with
    confirm 1, so the option, the answer and the AP decide the wait (live check #45)."""
    t, c, h, sel, m, g, ret, pl = base
    t = r.randrange(3136)
    if sel in own and r.random() < .8:   # a near tile: a path exists, so the confirm wait opens
        t = (own[sel] + r.choice([1, 2, 3, -1, -2, -3, 56, 112, 168, -56, -112, -168, 57, -55])) % 3136
    g.update(button=1, shift=r.choice([0, 1]), alt=r.choice([0, 1]), preview=0, aim=[])
    ret[0], ret[3], ret[4] = r.choice([2, 2, 2, 2, 0, 1]), r.choice([0, 0, 1, 2]), r.choice([1, 1, 1, 0])
    c.update(tactical_on=1, overlay_up=0, mode=2, value=0x95, tile=0xC41)
    h.update(redraw_off=0)
    m.update(hp=None, hand=0x4D, cond=50, count=1, inCur=None, b19C=None, u114=None, b1E2=None, uD0=None, facing=None,
             anim=None, apLeft=r.choice([0, 3, 20, 40, 40, 40]), b16A=r.randrange(2), b1B8=None, b1BE=0, b1BF=0)
    return t, c, h, sel, m, g, ret, pl


def setup(t, c, h, sel, m, g):
    e.uc.mem_write(*OBJ2[:1], obj2_base)
    sc = dict(base_sc)
    if g["combat"] is not None:
        sc["combat"] = g["combat"]
    bufs = {k: bytearray(v) for k, v in base_bufs.items()}
    ed = m.get("edge")
    if ed:
        if sel < 8:
            bufs["item_types"][12 * m["hand"] + 2] = 0x92
        if ed["occ"] is not None:
            bufs["tiles"][8 * t + 5] = (bufs["tiles"][8 * t + 5] & 0x80) | ed["occ"]
    port_lib.world_store(e, ai, bufs, sc)
    for k, (a, f) in C.items():
        wr(a, f, c[k])
    for k, (a, f) in H.items():
        wr(a, f, h[k])
    wr(SEL, "B", sel)
    wr(CONFIRM_MOVE, "B", rets[4])
    wr(FAST_FIRE, "B", rets[6])
    wr(SHIFT, "B", g["shift"])
    wr(ALT, "B", g["alt"])
    wr(PREVIEW, "i", g["preview"])
    wr(BUTTONS, "B", g["button"])
    wr(MOUSE_Y, "i", 100)
    wr(PRESS_Y, "i", 100)
    wr(STRIP, "B", 0x40)
    e.uc.mem_write(e.rt(LABEL), b"xx\0" + bytes(13))
    if sel < 8:
        rec = SOLDIERS + 512 * sel
        for k, off in (("hp", 0x183), ("b1B8", 0x1B8), ("b1BE", 0x1BE), ("b1BF", 0x1BF), ("b16A", 0x16A),
                       ("apLeft", 0x1A5), ("hand", 0), ("cond", 2), ("count", 1), ("inCur", 0x182), ("b19C", 0x19C),
                       ("b1E2", 0x1E2)):
            if m[k] is not None:
                wr(rec + off, "B", m[k])
        if m.get("facing") is not None:
            wr(rec + 0x1A1, "B", m["facing"])
        if ed and ed["occ"] is not None:
            wr(SOLDIERS + 512 * ed["occ"] + 0x9C, "B", ed["hit"])
        if m.get("anim") is not None and rd(rec + 0x1F3, "B") < 11:
            wr(rec + 0x1E8 + 2 * rd(rec + 0x1F3, "B"), "H", m["anim"])
        for k, off in (("uD0", 0xD0), ("u114", 0x114)):
            if m[k] is not None:
                wr(rec + off, "H", m[k])


def original(t, ret):
    calls.clear()
    gating[0] = False
    press_orig[0] = 0
    e.call(e.rt(0x1D9B8), t)
    c = {k: rd(a, f) for k, (a, f) in C.items()}
    h = {k: rd(a, f) for k, (a, f) in H.items()}
    lab = bytes(e.uc.mem_read(e.rt(LABEL), 16)).split(b"\0")[0]
    bufs, sc = port_lib.world_load(e, ai)
    return c, h, lab, bufs, sc, list(calls)


def port(t, g, ret, pl):
    bufs, sc = port_lib.world_load(e, ai)
    arrays = {k: (ctypes.c_uint8 * len(v)).from_buffer(v) for k, v in bufs.items()}
    w = port_lib.World(**{k: ctypes.cast(a, port_lib.u8p) for k, a in arrays.items()}, **sc)
    ops = []

    def rnd(_ctx, k):
        ops.append(("random", k))
        return 0

    def cb(_ctx, op, a, nargs, out, size):
        ops.append(op)
        return 0
    host = port_lib.Host(None, port_lib.RANDOM_FN(rnd), port_lib.CALL_FN(cb), 0)
    st, hs = CursorState(), HoverState()
    for k, (a, f) in C.items():
        setattr(st, k, rd(a, f) if k not in ("ap_fg", "ap_shadow") else rd(a, f) & 0xFF)
    st.label[:] = list(e.uc.mem_read(e.rt(LABEL), 16))
    for k, (a, f) in H.items():
        setattr(hs, k, rd(a, f))
    # The press gate of the edge drag reads the state before the click: 1 when UseItemAt reaches AllMoveTraverse?.
    press_port[0] = PORT.ja1f_press_walks(ctypes.byref(w), ctypes.byref(host), EXE, ctypes.byref(st), ctypes.byref(hs), t, g["shift"])
    rets_c, log, n, wait = (i32 * 7)(*ret), (i32 * 64)(), i32(), i32()
    pc = (i32 * (1 + 2 * len(pl)))(len(pl), *[v for p in pl for v in p])
    status = ctypes.create_string_buffer(80)
    ac = (i32 * (1 + 2 * len(g["aim"])))(len(g["aim"]), *[v for p in g["aim"] for v in p])
    rc = PORT.ja1f_click(ctypes.byref(w), ctypes.byref(host), EXE, ctypes.byref(st), ctypes.byref(hs), t,
                         g["button"], g["shift"], g["alt"], rets_c, log, ctypes.byref(n), status, ctypes.byref(wait), pc, ac)
    for k in sc:
        sc[k] = getattr(w, k)
    c = {k: getattr(st, k) for k in C}
    h = {k: getattr(hs, k) for k in H}
    lab = bytes(st.label).split(b"\0")[0]
    pcalls = []
    for j in range(min(n.value, 8)):
        row = list(log[8 * j:8 * j + 8])
        pcalls.append(tuple(row[:{1: 4, 2: 2, 3: 4, 4: 8, 5: 4, 6: 5, 7: 1, 8: 4, 9: 4, 10: 3}[row[0]]]))
    return rc, c, h, lab, bufs, sc, ops, pcalls, status.value.decode("latin-1"), wait.value


def norm_calls(oc):
    """the original's calls -> (library calls, status text)"""
    lib = [x for x in oc if isinstance(x[0], int)]
    texts = [x for x in oc if not isinstance(x[0], int)]
    return lib, texts


r = random.Random(1)
edge = random.Random(0x1CAD0)
detector = random.Random(0x1C7A8)
bad = skipped = 0
seen = {}                     # coverage: library call kinds, waits, status texts
for i in range(ncases):
    t, c, h, sel, m, g, ret, pl = case(r)
    if i % 5 == 4 and enemies:
        t, c, h, sel, m, g, ret, pl = edge_case(edge, (t, c, h, sel, m, g, ret, pl))
    elif i % 5 == 2:
        t, c, h, sel, m, g, ret, pl = detector_case(detector, (t, c, h, sel, m, g, ret, pl))
    if ONLY is not None and i != ONLY:
        continue
    rets[:] = ret
    setup(t, c, h, sel, m, g)
    rc, pc, ph, plab, pbufs, psc, ops, pcalls, pstat, pwait = port(t, g, ret, pl if g["button"] == 2 else [])
    setup(t, c, h, sel, m, g)
    polls[:] = pl if g["button"] == 2 else []
    aim_script[:] = g["aim"]
    aim_key[0] = 0
    try:
        oc, oh, olab, obufs, osc, ocalls = original(t, ret)
    except Exception as x:
        print("EMU", i, t, sel, c, m, g, x)
        bad += 1
        continue
    oc["ap_fg"] &= 0xFF
    oc["ap_shadow"] &= 0xFF
    olib, otexts = norm_calls(ocalls)
    diffs = [k for k in oc if oc[k] != pc[k]] + [k for k in oh if oh[k] != ph[k]]
    diffs += ["label"] if olab != plab else []
    diffs += [k for k in obufs if k not in port_lib.READ_ONLY and obufs[k] != pbufs[k]]
    diffs += [k for k in osc if osc[k] != psc[k] and k != "marked_slot"]   # = named (0x95E0C), compared above
    diffs += ["calls"] if olib != pcalls else []
    # the port's texts in order, one per line; a merc text is "NAME text" (the original's text has no name)
    plines = pstat.split("\n") if pstat else []
    if len(plines) != len(otexts) or any(not (pl_.endswith(" " + ot) if ok == "merc" else pl_ == ot)
                                         for pl_, (ok, ot) in zip(plines, otexts)):
        diffs.append("status")
    diffs += ["host ops"] if ops else []
    if g["button"] == 1:
        diffs += ["press gate"] if press_port[0] != press_orig[0] else []
        kn = f"press gate {press_orig[0]}"
        seen[kn] = seen.get(kn, 0) + 1
    diffs += ["rc"] if rc else []
    if ONLY is not None:
        print("occupant", tiles[8 * t + 5] & 0x7F, "calls", olib, pcalls, "texts", otexts, repr(pstat), "wait", pwait)
    for x in pcalls:
        kn = ("click", "reload", "select", "move", "turn", "anim", "look", "message", "aim right", "ok sound")[x[0] - 1]
        seen[kn] = seen.get(kn, 0) + 1
    if pwait:
        kn = ("aim", "confirm", "walk confirm", "posture")[pwait - 1]
        seen[kn] = seen.get(kn, 0) + 1
    if pstat:
        seen["status"] = seen.get("status", 0) + 1
    livecast.status("click_diff", i + 1, ncases, bad, f"tile {t} button {g['button']} mode {c['mode']}")
    if diffs:
        bad += 1
        if bad <= 8:
            print(f"MISMATCH case {i}: tile {t} sel {sel} {c} {h} {m} {g} rets {ret}: {diffs}")
            for k in diffs[:4]:
                if k in oc:
                    print(f"  {k}: {oc[k]} vs {pc[k]}")
                elif k in oh:
                    print(f"  {k}: {oh[k]} vs {ph[k]}")
                elif k == "label":
                    print(f"  label: {olab} vs {plab}")
                elif k in osc:
                    print(f"  {k}: {osc[k]} vs {psc[k]}")
                elif k == "calls":
                    print(f"  calls: {olib} vs {pcalls}")
                elif k == "status":
                    print(f"  status: {otexts} vs {pstat!r}")
                elif k == "host ops":
                    print(f"  host ops: {ops[:10]}")
                elif k in obufs:
                    j = next(j for j in range(len(obufs[k])) if obufs[k][j] != pbufs[k][j])
                    print(f"  {k}[{j}]: {obufs[k][j]} vs {pbufs[k][j]}")
print("click_diff: covered", dict(sorted(seen.items())))
if ncases >= 3000 and min(seen.get("press gate 0", 0), seen.get("press gate 1", 0)) < 50:   # both answers must stay reached
    print("click_diff: too few press gate cases", seen.get("press gate 0", 0), seen.get("press gate 1", 0))
    bad += 1
if aim_clicks[0] != seen.get("aim right", 0) or aim_limits[2] or aim_limits[0] != aim_limits[1] or not aim_limits[0]:
    print("click_diff: AimLoop sound or mouse limit:", aim_clicks[0], seen.get("aim right", 0), aim_limits)
    bad += 1
if ncases >= 3000 and seen.get("aim right", 0) < 100:      # the scripted AimLoop runs must stay reached
    print("click_diff: too few right clicks of the aim", seen.get("aim right", 0))
    bad += 1
# ja1_click_detector (LIB_HOOK above answers it in the cases): the same steps as ja1_click (ai_diff click compares that one
# with the original), then result 2 in place of its op 140 move_merc.
same = 0
for sl, tl in own.items():
    if sl < 8:
        for dt in (0, 1, -1, 56, -56, 168):
            runs = []
            for name, extra in (("click", (tl + dt, 0x4D)), ("click_detector", tl + dt)):
                bufs_ = {k: bytearray(v) for k, v in base_bufs.items()}
                bufs_["soldiers"][0x200 * sl] = 0x4D
                ops_ = []
                res = port_lib.ai_run(name, bufs_, dict(base_sc), sl, lambda n_: 0, lambda o_, a_: (ops_.append((o_, a_)), (0, None))[1],
                                      fallback=extra, host_ops=1 << 63)
                runs.append((res, ops_, bufs_))
            (r0, o0, b0), (r1, o1, b1) = runs
            if any(o_ == "move_merc" for o_, _ in o0):
                same += 1
                if r1 != 2 or [x for x in o0 if x[0] != "move_merc"] != o1 or b0 != b1:
                    print("MISMATCH detector click", sl, tl + dt, r0, r1, o0, o1)
                    bad += 1
if same < 5:
    print("click_diff: too few detector clicks", same)
    bad += 1
print(f"click_diff: {same} detector clicks equal to ja1_click")
print(f"click_diff: {ncases} cases ({skipped} skipped), {bad} mismatches")
sys.exit(1 if bad else 0)
