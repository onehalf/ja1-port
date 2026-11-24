"""Merc stat popup of the tactical panel (analysis mirror for validate/diff/popup_diff.py).

The left mouse button held on a slot's bar area shows DrawPopupInventory? 0x3FF3C (AL slot): POPINV.PCC (MAIN.DAT,
100 x 42) is loaded into the panel buffer [0x93E60] at (173, 18) (loader 0x532BC, result to [0x229D18]), the 10
numbers of 0x3FE50 (value 0x47350) are drawn on it, then the 100 x 42 block is copied to the screen [0x95678] next to
the slot (slot x + 48 on the left side, slot x - 100 on the right side; slot y).

As a module: draw_popup(m, slot, load) where m is an emulator (r8/r16/r32/w8, rt) and load(dst, pitch) writes the
picture (the test stubs the loader on both sides).
"""
import panel as P

POP_POS = 0x93FB0           # 10 x (i16 x, i16 y): number offsets from (173, 18)
POP_COL = 0x93F10           # 10 x 10 bytes: +4 i16 colour, +6 i16 shadow colour
P_SCREEN = 0x95678
LOAD_RESULT = 0x229D18


def stat(m, s, k):                                                # 0x47350 (EAX k, EDX soldier)
    """k 0: u16 +0x112; 1: signed hp +0x183; 2..9: signed byte +0x189 + k (agility .. level)"""
    if k == 0:
        return m.r16(s + 0x112)
    if k == 1:
        return P.s8(m.r8(s + 0x183))
    return P.s8(m.r8(s + 0x189 + k))


def number(m, p, k, s, x0, y0):                                   # 0x3FE50 (EAX k, EDX soldier, EBX x, ECX y)
    a = m.rt(POP_POS + 4 * k)
    x, y = x0 + P.s16(m.r16(a)), y0 + P.s16(m.r16(a + 2))
    v = stat(m, s, k)
    p.number(v, 4, p.panel, P.PITCH, x + 1, y + 1, P.s16(m.r16(m.rt(POP_COL + 10 * k + 6))), 0, 0, 300)
    fg = POP_COL + 4 if k == 1 else POP_COL + 10 * k + 4       # k = 1 (hp) takes the colour of k = 0
    p.number(v, 4, p.panel, P.PITCH, x, y, P.s16(m.r16(m.rt(fg))), 0, 0, 300)


def draw_popup(m, slot, load):                                    # 0x3FF3C (AL slot)
    p = P.Panel(m)
    sx, sy = p.pos(slot)
    x = sx + 0x30 if slot < 4 else sx - 0x64
    m.w32(m.rt(LOAD_RESULT), load(p.panel + 0x172D, P.PITCH))
    s = p.soldier(slot)
    for k in range(10):
        number(m, p, k, s, 0xAD, 0x12)
    scr = m.r32(m.rt(P_SCREEN))
    p.copy(scr + x + sy * P.PITCH, p.panel + 0x172D, 100, 42, P.PITCH, P.PITCH)
