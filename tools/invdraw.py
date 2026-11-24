"""Inventory screen drawing (analysis mirror for validate/diff/invdraw_diff.py).

DrawMercBody? 0x555DC (EAX soldier) draws the whole inventory screen of the tactical view (key I) into the panel
buffer [0x93E60], marks the 8 merc records (+0x1AC eye, +0x1BB "out of reach"), redraws the 8 panel slots, copies
the panel buffer to the screen [0x95678] and returns the armour rating. Callees mirrored here: DrawItemSlot? 0x4AAD4
(one of the 14 item places), DrawItemCondition? 0x55BE4 via 0x55FD8 (the 7 boxes of the bottom strip), the item
picture 0x46750 (INV.DAT bank [0x1163CC]), the condition bar 0x4B064, the attachment pull-up 0x466D0, BlinkUi? 0x33768,
the time meter 0x3DB3C, the button labels 0x35AB8 and the button shade 0x254F0.

As a module: draw_body(m, s, load, sight) with m an emulator (r8/r16/r32/w8/read/write, rt), s the soldier address,
load(name, dst, pitch) -> loader result (the test stubs the PCC loader 0x532BC on both sides) and
sight(s, x, y, tile, tx, ty, flag, rng, last) -> SightLine 0x58760 result (stubbed on both sides too).
"""
import panel as P
import ai

ITEM_RECTS = 0x949F2         # 14 x (i16 x, i16 y): the item places (DrawItemSlot)
ITEM_TABLE = 0x94DF8         # 12 bytes per item type: +4 picture, +6 max stack, +7, +9, +0xA (no condition when 0)
PICS = 0x1163CC              # INV.DAT bank: picture pointers
BUTTONS = 0x9345A            # 10 bytes per button: i16 x, i16 y, u8 element, u8 pressed element, u8 kind, ?, dx, dy
LABELS = 0x966C9             # 6 bytes per button: label text
STRIP_X = 0x93B14            # 7 x i32: bottom strip box x
STRIP_PIC_X = 0x93B30        # 7 x i32: bottom strip picture x
STRIP_ENTRY = 0x93D59        # 7 x u8: soldier entry shown in bottom strip box k
DARK = 0xE8CBC               # pointer to the darken table
REDRAW_OFF = 0x92DDF
LOAD_RESULT = 0x229D18
P_SCREEN = 0x95678
BLINK = 0x96068
TIME_W = 0x22A0FB            # i32 width of the time meter
CUR_SECTOR = 0x22A039
SECTOR_ENEMIES = 0x22979D    # + 9 x sector
SMALL_PANEL_FLAG = 0x92E30


def text(p, s, x, y, centre, fg, shadow, font):
    p.text(s, p.panel, P.PITCH, x, y, centre, fg, shadow, font, 300)


def picture(m, p, pic, dst, pitch, x, y, colour):                 # 0x46750
    w, dx = (42, 0) if pic <= 20 else (30, 6) if pic <= 90 else (16, 13)
    t = p.remap()
    old = m.r8(t + 0x17)
    m.w8(t + 0x17, colour)
    p.blit_remap(dst + x + dx + y * pitch, m.r32(m.rt(PICS + 4 * pic)), w, 16, pitch, w, t, 0)
    m.w8(t + 0x17, old)


def cond_bar(p, dst, pitch, cond, x, y):                         # 0x4B064
    h = 10 - P.cdiv(cond, 10)
    if h:
        p.copy(dst + y * pitch + x, p.sheet + 0x524A, 2, h, pitch, P.PITCH)


def missing(m, s, k):                                             # 0x4A464: vest pocket k missing
    if m.r8(s + 0x17A) >= 8:
        return 0
    need = {7: 0x32, 8: 0x32, 9: 0x33, 10: 0x34, 11: 0x35}.get(k)
    return 1 if need is not None and m.r8(s + 0x48) < need else 0


def pull_up(m, s, k):                                             # 0x466D0: a worn-out item gives way to its attachment
    e = s + 12 * k
    if P.s8(m.r8(e + 2)) <= 0:
        m.write(e, m.read(e + 3, 3))
        m.write(e + 3, bytes(3))


def itype(m, t, o):
    return m.r8(m.rt(ITEM_TABLE + 12 * t + o))


def rect(m, k):
    a = m.rt(ITEM_RECTS + 4 * k)
    return P.s16(m.r16(a)), P.s16(m.r16(a + 2))


def item_slot(m, p, s, k, dst, pitch):                            # DrawItemSlot? 0x4AAD4 (EAX s, DL k, EBX dst, ECX pitch)
    x, y = rect(m, k)
    bar_x = x + 0x2E
    if k < 7:
        shadow, back, bar_y, dy = 10, 0x30, y + 4, 1
    else:
        shadow, back, bar_y, dy = 0xE2, 0x31, y + 5, 2
    b6 = 0
    if k == 13:
        tile = P.s16(m.r16(s + 0x114))
        g = m.r8(m.rt(0x100A1E + 8 * tile))
        ga = m.rt(0xEB160 + 12 * g)
        if m.r8(ga + 6) == 0xFF and P.s8(m.r8(ga + 2)) < 0:
            g = 0xFF
        if g == 0xFF:
            t = 0
            n = c = 0
        else:
            t, n, c, b6 = m.r8(ga), m.r8(ga + 1), m.r8(ga + 2), m.r8(ga + 6)
    elif k == 12:
        if P.s8(m.r8(s + 5)) <= 0:
            m.w8(s + 4, 0)
            m.w8(s + 5, 0)
            m.w8(s + 3, 0)
        t, c, n = m.r8(s + 3), m.r8(s + 5), m.r8(s + 4)
    else:
        if P.s8(m.r8(s + 2)) <= 0:                                # entry 0's condition, whatever k is (original)
            m.w8(s + 1, 0)
            m.w8(s + 2, 0)
            m.w8(s, 0)
        e = s + 12 * k
        t, c, n = m.r8(e), m.r8(e + 2), m.r8(e + 1)
        if t and itype(m, t, 0xA) == 0:
            c = 100
        ax, ay = rect(m, 12)
        if k == 0:
            if t == 0:
                p.blit_key(dst + ay * pitch + ax, *_elem_src(p, 0x43), pitch, P.PITCH, 0)
            else:
                p.blit_key(dst + ay * pitch + ax, *_elem_src(p, 0x31), pitch, P.PITCH, 0)
                cond_bar(p, dst, pitch, P.s8(m.r8(s + 5)), ax + 0x2E, ay + 5)
        p.blit_key(dst + y * pitch + x, *_elem_src(p, back), pitch, P.PITCH, 0)
    if t == 0:
        if k <= 11:
            cond_bar(p, dst, pitch, P.s8(c), bar_x, bar_y)
        return
    if k < 13:
        cond_bar(p, dst, pitch, P.s8(c), bar_x, bar_y)
        px, py, col = x + 1, y + dy, shadow
    else:
        px, py, col = x + 5, y + dy, 0
    picture(m, p, itype(m, t, 4), dst, pitch, px, py, col)
    n = P.s8(n)
    if n < 99 and (k < 12 or k == 13):
        if itype(m, t, 6) < 2 or t == 0x6F:
            if k <= 12 and t != 0x6F:
                p.text(b"%02d" % abs(n), dst, pitch, x + 2, y + dy + 10, 0, 0x29, shadow, 0, 300)
        elif k != 3:
            p.text(b"%2d" % abs(n), dst, pitch, x + 0x23, y + dy + 10, 0, 0xED, shadow, 0, 300)
    if n < 0:
        p.text(b"jammed", dst, pitch, x + 0x16, y + dy + 6, 1, 0xED, 1, 0, 300)
    if b6 == 0xFF:
        p.text(b"buried", dst, pitch, x + 0x16, y + dy + 6, 1, 0xED, 1, 0, 300)
    elif c < 10 and (itype(m, t, 9) or itype(m, t, 7)):                # unsigned byte compare
        p.text(b"useless", dst, pitch, x + 0x16, y + dy + 6, 1, 0xED, 1, 0, 300)


def _elem_src(p, e):
    ex, ey, w, h = p.elem(e)
    return p.sheet + ey * P.PITCH + ex, w, h


def strip_box(m, p, k, s, dark, copy):                            # DrawItemCondition? 0x55BE4 (AL k, EDX s, BL, CL)
    if m.r8(s + 0x17A) > 7:
        return
    x = _s32(m.r32(m.rt(STRIP_X + 4 * k)))
    e = m.r8(m.rt(STRIP_ENTRY + k))
    pull_up(m, s, e)
    y = 0xB7
    a = s + 12 * e
    if k == 0 and m.r8(a) == 0:
        p.draw_elem(0x2E, x, y)
    else:
        p.draw_elem(0x2F if 0 < k < 6 else 0x69, x, y)
        if missing(m, s, e):
            p.remap_rect(p.panel + y * P.PITCH + x, 0x2A, 0x10, P.PITCH, m.r32(m.rt(DARK)))
    t = m.r8(a)
    if t and not missing(m, s, e):
        px = _s32(m.r32(m.rt(STRIP_PIC_X + 4 * k)))
        picture(m, p, itype(m, t, 4), p.panel, P.PITCH, px, y, 8)
        n = P.s8(m.r8(a + 1))
        if n < 99 and t != 0x6F:
            if itype(m, t, 6) > 1:
                text(p, b"%2d" % abs(n), px + 0x22, y + 0xB, 0, 0xED, 0xA, 0)
            else:
                text(p, b"%02d" % abs(n), px + 1, y + 0xB, 0, 0x29, 0xA, 0)
            if P.s8(m.r8(a + 1)) < 0:
                text(p, b"jammed", px + 0x15, y + 6, 1, 0xED, 1, 0)
        if P.s8(m.r8(a + 2)) < 10 and (itype(m, t, 9) or itype(m, t, 7)):
            text(p, b"useless", px + 0x15, y + 6, 1, 0xED, 1, 0)
    if dark and not missing(m, s, e):
        p.remap_rect(p.panel + y * P.PITCH + x, 0x2E if k in (0, 6) else 0x2A, 0x10, P.PITCH, m.r32(m.rt(DARK)))
    if (p.gb(REDRAW_OFF) <= 2 or _s32(m.r32(m.rt(SMALL_PANEL_FLAG))) > 0) and copy:
        scr = m.r32(m.rt(P_SCREEN))
        p.copy(scr + y * P.PITCH + x, p.panel + y * P.PITCH + x, 0x2A, 0x10, P.PITCH, P.PITCH)


def _s32(v):
    return v - (1 << 32) if v & 0x80000000 else v


def strip(m, p, s, dark, copy):                                   # 0x55FD8 (EAX s, DL dark, BL copy to screen)
    if m.r8(s + 0x17A) < 8 and (p.gb(REDRAW_OFF) < 3 or _s32(m.r32(m.rt(SMALL_PANEL_FLAG))) != 0):
        for k in range(7):
            strip_box(m, p, k, s, dark, 0)
        if copy:
            scr = m.r32(m.rt(P_SCREEN))
            m.write(scr + 0xE380, m.read(p.panel + 0xE380, 0x1680))


def blink(m, p, on):                                              # BlinkUi? 0x33768 (AL)
    sec = p.gb(CUR_SECTOR)
    en = m.r8(m.rt(SECTOR_ENEMIES + 9 * sec))
    if (en or on) and p.gb(REDRAW_OFF) <= 2:
        e = 100
        if en:
            v = m.r8(m.rt(BLINK)) ^ 1
            m.w8(m.rt(BLINK), v)
            e = 100 if v else 99
        p.draw_elem(e, 0xF, 3)
        p.number(sec + 1, 2, p.panel, P.PITCH, 0x15, 4, 0xFF, 1, 0, 300)


def time_meter(m, p):                                             # 0x3DB3C (AL 0)
    w = _s32(m.r32(m.rt(TIME_W)))
    if w > 0 and p.gb(REDRAW_OFF) < 3:
        ex, ey, _, h = p.elem(95)
        p.copy(p.panel + 0x4DE, p.sheet + ex + ey * P.PITCH, w, h, P.PITCH, P.PITCH)


def button(m, p, i):                                              # 0x35AB8 (label, DL 0) + 0x254F0 (shade)
    button_label(m, p, i)
    button_shade(m, p, i)


def button_label(m, p, i):                                        # 0x35AB8 (AL i, DL 0)
    a = m.rt(BUTTONS + 10 * i)
    x, y = P.s16(m.r16(a)), P.s16(m.r16(a + 2))
    kind = P.s8(m.r8(a + 6))
    if kind > 2:
        lab = P.cstr(m, m.rt(LABELS + 6 * i))
        text(p, lab, x + P.s8(m.r8(a + 8)) - 1, y + P.s8(m.r8(a + 9)) - 3, 1, 0x2A, 3, 1 if kind != 4 else 0)


def button_shade(m, p, i):                                        # 0x254F0 (AL i): the button greyed out
    a = m.rt(BUTTONS + 10 * i)
    x, y = P.s16(m.r16(a)), P.s16(m.r16(a + 2))
    _, _, w, h = p.elem(m.r8(a + 4))
    t = p.remap()
    old = m.r8(t + 0x1B)
    m.w8(t + 0x1B, 0x70)
    src = p.sheet + 0x372
    d = p.panel + y * P.PITCH + x
    if w < 0x13:
        p.blit_remap(d, src, w, h, P.PITCH, P.PITCH, t, 0)
    else:
        p.blit_remap(d, src, 0x13, h, P.PITCH, P.PITCH, t, 0)
        p.blit_remap(d + 0x13, src, w - 0x13, h, P.PITCH, P.PITCH, t, 0)
    m.w8(t + 0x1B, old)


def marks(m, s, sight):
    """the 8 merc records: +0x1AC = 3 below 15 hp; +0x1BB = 1 = out of reach for an item hand-over"""
    me = m.r8(s + 0x17A)
    for k in range(8):
        o = m.r32(m.rt(P.SOLDIERS + 4 * k))
        if P.s8(m.r8(o + 0x183)) < 15:
            m.w8(o + 0x1AC, 3)
        m.w8(o + 0x1BB, 0)
        out = (not m.r8(o + 0x182) or m.r8(o + 0x17E) or P.s8(m.r8(o + 0x183)) < 1
               or ai.tile_distance(P.s16(m.r16(s + 0x114)), P.s16(m.r16(o + 0x114))) > 2)
        if not out and me != k:
            out = not sight(s, P.s16(m.r16(s + 0x116)) + 2, P.s16(m.r16(s + 0x118)) + 4, P.s16(m.r16(o + 0x114)),
                            P.s16(m.r16(o + 0x116)) + 2, P.s16(m.r16(o + 0x118)) + 4, 1, 0xE, 0x40)
        if out:
            m.w8(o + 0x1BB, 1)
        slot_redraw(m, k)


def slot_redraw(m, k):                                            # 0x331A4 (AL k, DL 0)
    p = P.Panel(m)
    o = m.r32(m.rt(P.SOLDIERS + 4 * k))
    if p.gb(REDRAW_OFF) < 2:
        v = m.r8(o + 0x17F)
        if v == 0:
            p.slot(m.r8(o + 0x17A))
        elif v == 1:
            raise NotImplementedError("0x330C0 switch view")


MOUSE_TILE = 0x229CF0         # i32 (set to 0xC41 = no tile by the strip drawing)
SELECTED = 0x284F11
TURN_SLOT = 0x284F0F
STRIP_ON = 0x92E36            # u8: the bottom strip shows a merc's items
COMBAT = 0x92DCB
COMBAT_SEEN = 0x92DCA
SECTOR_MERCS = 0x22979E       # + 9 x sector


def slot_show(m, k):                                              # 0x33204 (AL k): slot k of the panel to the screen
    p = P.Panel(m)
    a = m.rt(P.SLOT_POS + 4 * k)
    x, y = P.s16(m.r16(a)), P.s16(m.r16(a + 2))
    o = y * P.PITCH + x
    p.copy(m.r32(m.rt(P_SCREEN)) + o, p.panel + o, 0x30, 0x2A, P.PITCH, P.PITCH)


def tac_strips(m, top, keep, load):                               # LoadStripPccs? 0x2F928 (AL top, DL keep)
    """the tactical top strip (TSTRIP.PCC, buttons, sector box, time meter) and bottom strip (BSTRIP.PCC, items)"""
    p = P.Panel(m)
    lr = m.rt(LOAD_RESULT)
    if top and p.gb(REDRAW_OFF) < 3:
        m.w32(m.rt(MOUSE_TILE), 0xC41)
        if not keep:
            m.w32(lr, load(b"tstrip.pcc", p.panel, P.PITCH) & 0xFFFFFFFF)
        for i in range(0x13, 0x19):
            if keep:
                a = m.rt(BUTTONS + 10 * i)
                p.draw_elem(0x78, P.s16(m.r16(a)), P.s16(m.r16(a + 2)))
            button_label(m, p, i)
            if (not p.gb(COMBAT) and i == 0x13) or (not m.r8(m.rt(SECTOR_MERCS + 9 * p.gb(CUR_SECTOR))) and i <= 0x15):
                button_shade(m, p, i)
        scr = m.r32(m.rt(P_SCREEN))
        m.write(scr, m.read(p.panel, 0x1180))
        time_meter(m, p)
        blink(m, p, 1)
    m.w32(lr, load(b"bstrip.pcc", p.panel + 0xE380, P.PITCH) & 0xFFFFFFFF)
    if p.gb(STRIP_ON):
        sel, turn = p.gb(SELECTED), p.gb(TURN_SLOT)
        in_play = lambda k: m.r8(m.rt(0xFAA90 + 0x200 * k))
        if sel < 8 and in_play(sel):
            strip(m, p, m.r32(m.rt(P.SOLDIERS + 4 * sel)), 0, 1)
        elif turn < 8 and in_play(sel):               # original: tests the selected slot's record, not the turn slot's
            strip(m, p, m.r32(m.rt(P.SOLDIERS + 4 * turn)), 1, 1)
        else:
            scr = m.r32(m.rt(P_SCREEN))
            m.write(scr + 0xE380, m.read(p.panel + 0xE380, 0x1680))
    if not p.gb(COMBAT):
        m.w8(m.rt(COMBAT_SEEN), 0)
    if p.gb(REDRAW_OFF) < 2:
        for k in range(8):
            o = m.r32(m.rt(P.SOLDIERS + 4 * k))
            j = m.r8(o + 0x17A)
            if j < 8:
                slot_redraw(m, j)
                slot_show(m, j)


def draw_body(m, s, load, sight):                                 # DrawMercBody? 0x555DC (EAX s) -> armour rating
    p = P.Panel(m)
    lr = m.rt(LOAD_RESULT)
    m.w32(lr, load(b"tstrip.pcc", p.panel, P.PITCH) & 0xFFFFFFFF)
    time_meter(m, p)
    blink(m, p, 1)
    for i in range(0x13, 0x19):
        button(m, p, i)
    m.write(m.rt(ITEM_RECTS), m.read(m.rt(0x94A3A), 56))           # 0x4AAB0: the inventory place table
    m.w8(m.rt(0x955CA), 1)
    m.w32(lr, load(b"invamb.pcc", p.panel + 0x11B0, P.PITCH) & 0xFFFFFFFF)
    num = str(m.r8(s + 0x17C)).encode()
    m.w32(lr, load(b"body" + num + b".pcc", p.panel + 0x4DEE, P.PITCH) & 0xFFFFFFFF)
    if m.r8(s + 0x1B1):
        m.w32(lr, load(b"camo" + num + b".pcc", p.panel + 0x60AE, P.PITCH) & 0xFFFFFFFF)
    m.w32(lr, load(b"bstrip.pcc", p.panel + 0xE380, P.PITCH) & 0xFFFFFFFF)
    text(p, P.cstr(m, s + 0x16E), 0x70, 0x1B, 0, 0xE, 5, 1)
    slot = m.r8(s + 0x17A)
    if slot < 8:
        p.copy(p.panel + 0x2708, m.r32(m.rt(P.P_FACES + 4 * slot)) + 0x42, 0x1C, 0x19, P.PITCH, 0x20)
        text(p, b"SALARY:", 0x70, 0x24, 0, 0xB5, 5, 0)
        p.number(m.r16(s + 0x112), 5, p.panel, P.PITCH, 0x8C, 0x24, 0x28, 1, 0, 300)
    armour = ai.armor_rating(m.read(s, 0x200))
    p.number(armour, 3, p.panel, P.PITCH, 0x7D, 0x58, 0xFF, 1, 0, 300)
    text(p, b"%", 0x89, 0x58, 0, 0xFF, 1, 1)
    for k in range(14):
        if not missing(m, s, k):
            item_slot(m, p, s, k, p.panel, P.PITCH)
    import popup
    for k in range(2, 10):
        popup.number(m, p, k, s, 0xAD, 0x12)
    marks(m, s, sight)
    strip(m, p, s, 1, 0)
    scr = m.r32(m.rt(P_SCREEN))
    m.write(scr, m.read(p.panel, 64000))
    return armour
