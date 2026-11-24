"""Merc panel of the tactical screen (analysis mirror for validate/diff/panel_diff.py).

The tactical screen has 8 merc slots (4 left, 4 right, 48 x 42 pixels each) in the panel buffer [0x93E60] (320 x 200,
pitch 320). DrawMercSlot? 0x32EB8 (AL slot, DL copy to screen) draws one whole slot: background, bar frame, name,
health / energy / AP bars, then the portrait (face, eyes, status tint, icon, AP number, ON DUTY, frames).

usage: ./t panel DUMP OUT.png        (draws all 8 slots of a memory dump with this mirror)
As a module: draw_slot(m, slot) where m has r8/r16/r32/w8/read/write on runtime addresses and rt(image address).
"""
import struct, sys

SLOT_POS = 0x93686          # 8 x (i16 x, i16 y) slot origin in the panel buffer
SLOT_OFF = 0x936A6          # 2 x 10 signed bytes per panel side (+0x180): face x,y; ?,?; health x,y; energy x,y; AP x,y
ELEMS = 0x93732             # panel sheet elements: (i16 x, i16 y, i16 w - 1, i16 h - 1)
P_SHEET, P_PANEL, P_FACES = 0x93454, 0x93E60, 0x93428
SOLDIERS = 0x100958
REMAP = 0xE8C48             # pointer to the remap table (256 bytes, page aligned use: table & ~0xFF | pixel)
FONT_CHARS, FONT_SHEETS, FONT_DIMS = 0x9453A, 0x229790, 0x949D6
NAME_BUF = 0xEBAC0
PITCH = 0x140


def s8(v):
    v &= 0xFF
    return v - 0x100 if v & 0x80 else v


def s16(v):
    v &= 0xFFFF
    return v - 0x10000 if v & 0x8000 else v


def cdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def cmod(a, b):
    return a - cdiv(a, b) * b


class Panel:
    def __init__(self, m):
        self.m = m
        g = m.rt
        self.g = g
        self.sheet = m.r32(g(P_SHEET))
        self.panel = m.r32(g(P_PANEL))

    # --- globals ---
    def gb(self, a):
        return self.m.r8(self.g(a))

    def soldier(self, slot):
        return self.m.r32(self.g(SOLDIERS + 4 * slot))

    def pos(self, slot):
        a = self.g(SLOT_POS + 4 * slot)
        return s16(self.m.r16(a)), s16(self.m.r16(a + 2))

    def off(self, s, k):
        return s8(self.gb(SLOT_OFF + 10 * self.m.r8(s + 0x180) + k))

    def remap(self):
        return self.m.r32(self.g(REMAP))

    # --- blit primitives ---
    def blit_key(self, dst, src, w, h, dp, sp, key):            # 0x522A0
        m = self.m
        for r in range(h):
            row = m.read(src + r * sp, w)
            for i, v in enumerate(row):
                if v != key:
                    m.w8(dst + r * dp + i, v)

    def blit_remap(self, dst, src, w, h, dp, sp, table, key):   # 0x522E4: v = table[src]; written unless v == key
        m, base = self.m, table & ~0xFF
        for r in range(h):
            row = m.read(src + r * sp, w)
            for i, v in enumerate(row):
                v = m.r8(base | v)
                if v != key:
                    m.w8(dst + r * dp + i, v)

    def blit_remap2(self, dst, src, w, h, dp, sp, table, key):  # 0x5232D: src == key skipped, else table[src]
        m, base = self.m, table & ~0xFF
        for r in range(h):
            row = m.read(src + r * sp, w)
            for i, v in enumerate(row):
                if v != key:
                    m.w8(dst + r * dp + i, m.r8(base | v))

    def remap_rect(self, dst, w, h, pitch, table):                # 0x52481: in place
        m, base = self.m, table & ~0xFF
        for r in range(h):
            for i in range(w):
                a = dst + r * pitch + i
                m.w8(a, m.r8(base | m.r8(a)))

    def copy(self, dst, src, w, h, dp, sp):                       # 0x52AE0
        m = self.m
        for r in range(h):
            m.write(dst + r * dp, m.read(src + r * sp, w))

    def fill(self, dst, v, w, h, pitch):                          # 0x52AB4
        for r in range(h):
            self.m.write(dst + r * pitch, bytes([v & 0xFF]) * w)

    def elem(self, e):
        a = self.g(ELEMS + 8 * e)
        m = self.m
        return s16(m.r16(a)), s16(m.r16(a + 2)), s16(m.r16(a + 4)) + 1, s16(m.r16(a + 6)) + 1

    def draw_elem(self, e, x, y, h=None):                         # 0x469F0 (h = None) / 0x46990 (h rows)
        ex, ey, w, eh = self.elem(e)
        self.blit_key(self.panel + y * PITCH + x, self.sheet + ey * PITCH + ex, w, eh if h is None else h,
                      PITCH, PITCH, 0)

    def bar_rows(self, e, x, y, n):                               # 0x1F660: n rows of the element's top-left 3 px
        ex, ey, _, _ = self.elem(e)
        for i in range(n):
            self.copy(self.panel + (y + i) * PITCH + x, self.sheet + ey * PITCH + ex, 3, 1, PITCH, PITCH)

    def frame(self, dst, pitch, x0, y0, x1, y1, colour):          # 0x46C44
        m = self.m
        t = self.remap()
        m.w8(t + 1, colour)
        ew = eh = 0
        if x1 - x0 > 0xC2:
            ew = s8(x1 - x0 - 0xC2)
            x1 = x0 + 0xC2
        if y1 - y0 > 0x53:
            eh = s8(y1 - y0 - 0x53)
            y1 = y0 + 0x53
        hsrc, vsrc = self.sheet + 0xC70E, self.sheet + 0x13F
        R = lambda d, s, w, h: self.blit_remap(d, s, w, h, pitch, PITCH, t, 0)
        R(dst + y0 * pitch + x0, hsrc, x1 - x0, 1)
        R(dst + (y1 + eh) * pitch + x0, hsrc, x1 - x0 + 1, 1)
        if ew > 0:
            R(dst + y0 * pitch + x1, hsrc, ew, 1)
            R(dst + (y1 + eh) * pitch + x1, hsrc, ew + 1, 1)
        R(dst + y0 * pitch + x0, vsrc, 1, y1 - y0)
        R(dst + y0 * pitch + x1 + ew, vsrc, 1, y1 - y0)
        if eh > 0:
            R(dst + y1 * pitch + x0, vsrc, 1, eh)
            R(dst + y1 * pitch + x1 + ew, vsrc, 1, eh)
        m.w8(t + 1, 1)

    # --- text ---
    def char(self, font, c):
        a = self.g(FONT_CHARS + font * 0x189 + c * 3)
        return s16(self.m.r16(a)), self.m.r8(a + 2)

    def text(self, s, dst, pitch, x, y, centre, fg, shadow, font, mode):   # 0x358D0
        m = self.m
        if centre:
            tot = sum(self.char(font, c - 0x20)[1] for c in s)
            x -= cdiv(tot, 2)
        if font == 0:
            y -= 1
        t = self.remap()
        old = m.r8(t + 0xFF), m.r8(t + 0xFE)
        m.w8(t + 0xFF, fg)
        m.w8(t + 0xFE, shadow)
        fsheet = m.r32(self.g(FONT_SHEETS + 4 * font))
        da = self.g(FONT_DIMS + 4 * font)
        sp, fh = s16(m.r16(da)), s16(m.r16(da + 2))
        for c in s:
            c -= 0x20
            if c < 0 or c >= 0x83:
                return                                            # remap 0xFF / 0xFE stay changed (original)
            xo, w = self.char(font, c)
            if (x + w) & 0xFFFFFFFF > pitch:
                w = pitch - x
            d, src = dst + y * pitch + x, fsheet + xo
            if mode == 300:
                if shadow & 0xFF:
                    self.blit_remap2(d, src, w, fh, pitch, sp, t, 0)
                else:
                    self.blit_remap(d, src, w, fh, pitch, sp, t, 0)
            else:
                m.w8(t, mode)
                self.blit_remap(d, src, w, fh, pitch, sp, t, 0)
                m.w8(t, 0)
            x += w
            if x > pitch - 1:
                break
        m.w8(t + 0xFF, old[0])
        m.w8(t + 0xFE, old[1])

    def number(self, v, digits, dst, pitch, x, y, fg, shadow, font, mode):   # 0x36090: '^' pads to digits
        s = str(v).encode()
        s = b"^" * max(0, digits - len(s)) + s
        self.text(s, dst, pitch, x, y, 0, fg, shadow, font, mode)

    # --- slot parts ---
    def sel_k(self, slot, k):
        return k if slot == self.gb(0x284F11) else 0

    def name(self, s):                                            # 0x32DB8
        m = self.m
        slot = m.r8(s + 0x17A)
        fg, sh = (0x16, 0x15) if slot == self.gb(0x284F11) else (0x0E, 0xFE)
        raw = m.read(s + 0x16E, 64)
        raw = raw[:raw.index(0)] if 0 in raw else raw
        nb = self.g(NAME_BUF)
        m.write(nb, raw + b"\0")
        if raw:
            m.w8(nb, raw[0] + 0x20)
        x, y = self.pos(slot)
        self.text(m.read(nb, len(raw)) if raw else b"", self.panel, PITCH, x + 3, y + 0x20, 0, fg, sh, 1, 300)

    def health(self, s):                                          # 0x323D4
        m = self.m
        slot = m.r8(s + 0x17A)
        if not (self.gb(0x92DDF) < 2 and slot < 8 and m.r8(s + 0x182)):
            return
        k = self.sel_k(slot, 3)
        X, Y = self.pos(slot)
        y = self.off(s, 5) + Y + 1
        x = self.off(s, 4) + X + 1
        hpmax, hp, wd = s8(m.r8(s + 0x184)), s8(m.r8(s + 0x183)), s8(m.r8(s + 0x185))
        gap = cdiv(100 - hpmax, 4)
        nh = 0 if not hp else cdiv(hp - 1, 4) + 1
        nw = 0 if not wd else cdiv(wd - 1, 4) + 1
        if cmod(100 - hpmax, 4) < cmod(100 - (hpmax - hp), 4) and nh > 1:
            nh -= 1
        rest = 25 - gap - nh - nw
        if gap > 0:
            self.draw_elem(k + 0x58, x, y, gap)
        y += gap
        if nw > 0:
            self.bar_rows(0x57, x, y, nw)
        y += nw
        if rest > 0:
            self.bar_rows(0x56, x, y, rest)
        y += rest
        if nh > 0:
            self.draw_elem(0x51, x, y)
            if nh > 1:
                self.bar_rows(0x50, x, y + 1, nh - 1)

    def bar(self, s, k_elem, top, rows, ox, oy, v):              # energy 0x32610 / AP 0x328A4 shape
        slot = self.m.r8(s + 0x17A)
        k = self.sel_k(slot, 3)
        X, Y = self.pos(slot)
        gap = 25 - v
        x, y = self.off(s, ox) + X, Y + self.off(s, oy)
        if gap > 0:
            self.draw_elem(k + k_elem, x, y, gap)
        if v > 0:
            y += gap
            self.draw_elem(top, x, y)
            if v > 1:
                self.bar_rows(rows, x, y + 1, v - 1)

    def energy(self, s):                                          # 0x32610
        m = self.m
        if not (m.r8(s + 0x17A) < 8 and m.r8(s + 0x138)):
            return
        e = s8(m.r8(s + 0x18A))
        v = 0 if not e else cdiv(e, 4)
        if v == 0 and e > 0:
            v = 1
        self.bar(s, 0x59, 0x53, 0x52, 6, 7, v)

    def ap_bar(self, s):                                          # 0x328A4
        m = self.m
        v = min(s8(m.r8(s + 0x1A5)), 0x19)
        if not (m.r8(s + 0x17A) < 8 and self.gb(0x92DDF) < 2 and m.r8(s + 0x138)):
            return
        self.bar(s, 0x5A, 0x55, 0x54, 8, 9, v)

    def tint(self, dst, pitch, x, y, colour):                     # 0x31C0C: element 17, remap[0x1B] = colour
        m, t = self.m, self.remap()
        old = m.r8(t + 0x1B)
        m.w8(t + 0x1B, colour)
        ex, ey, w, h = self.elem(17)
        self.blit_remap(dst + (y + 2) * pitch + x + 2, self.sheet + ey * PITCH + ex, w, h, pitch, PITCH, t, 0)
        m.w8(t + 0x1B, old)

    def face_frame(self, slot, colour):                           # 0x3341C
        s = self.soldier(slot)
        X, Y = self.pos(slot)
        x, y = self.off(s, 0) + X, Y + self.off(s, 1)
        self.frame(self.panel, PITCH, x + 1, y + 1, x + 0x1E, y + 0x1B, colour)

    def slot_frame(self, slot):                                   # 0x335F8 (DL 0)
        if not (self.gb(0x92DDF) < 2 and slot < 8):
            return
        s = self.soldier(slot)
        colour = 0xA2 if not self.m.r8(s + 0x1A8) else 0x16
        if not self.m.r8(s + 0x17F):
            self.face_frame(slot, colour)
        else:
            X, Y = self.pos(slot)
            self.frame(self.panel, PITCH, X + 1, Y + 1, X + 0x2E, Y + 0x28, colour)

    def frames(self, s):                                          # 0x333B8
        m = self.m
        if not (m.r8(s + 0x182) and m.r8(s + 0x138)):
            return
        if self.gb(0x284F0E) == m.r8(s + 0x17A):
            self.face_frame(self.gb(0x284F0E), 0xFF)
            return
        if not at_waypoint(m, s) or s16(m.r16(s + 0x120)) < 0xC41:
            self.slot_frame(m.r8(s + 0x17A))

    def icon(self, dst, pitch, x, y, sprite):                     # DrawCompiledSprite 0x52DC8 on the panel
        for (dx, dy), v in sprite_pixels(self.m, sprite).items():
            self.m.w8(dst + (y + dy) * pitch + x + dx, v)

    def portrait(self, slot):                                     # DrawMercPortraitPanel? 0x31C98 (DL 0)
        m = self.m
        if self.gb(0x92DDF) > 1 or slot > 7:
            return
        s = self.soldier(slot)
        X, Y = self.pos(slot)
        x, y = X + self.off(s, 0), self.off(s, 1) + Y
        face = m.r32(self.g(P_FACES + 4 * slot))
        P = self.panel
        self.copy(P + y * PITCH + x, face, 0x1F, 0x1C, PITCH, 0x20)
        hp = s8(m.r8(s + 0x183))
        if m.r8(s + 0x189):
            m.w8(s + 0x1AC, 0)
        elif not m.r8(s + 0x1A5) and hp >= 15:
            m.w8(s + 0x1AC, 3)
        eye = s8(m.r8(s + 0x1AC))
        if eye < 2:
            self.copy(P + (m.r8(s + 0x1AA) + y + 2) * PITCH + x + 2, face + (eye * 2 + 0x1C) * 0x20, 0x1C, 2,
                      PITCH, 0x20)
        if m.r8(s + 0x189) or hp < 15:
            self.tint(P, PITCH, x, y, s8(self.gb(0x94BF8 + s8(m.r8(s + 0x189)))))
        elif m.r8(s + 0x182) and not m.r8(s + 0x138):
            self.tint(P, PITCH, x, y, 2)
        if m.r8(s + 0x181):
            self.icon(P, PITCH, x + 2, y + 0xD, m.r32(self.g(0xA92BC + 4 * m.r8(s + 0x181))))
        if hp >= 15 and self.gb(0x92DCB) and m.r8(s + 0x138):
            ap = s8(m.r8(s + 0x1A5))
            if ap < s8(m.r8(s + 0xEF)):
                fg, sh = 0x72, 1
            else:
                fg = 0x16 if self.gb(0x22A0EC + m.r8(s + 0x17A)) else 0xFF
                sh = 0xFE
            if fg != 0xFF:
                o = (y + 0x14) * PITCH + x
                if ap < 10:
                    self.fill(P + o + 0x19, 0, 5, 7, PITCH)
                else:
                    self.fill(P + o + 0x16, 0, 8, 7, PITCH)
            self.number(ap, 2, P, PITCH, x + 0x16, y + 0x15, fg, sh, 0, 300)
        if m.r8(s + 0x17E) or not m.r8(s + 0x1A5):
            self.remap_rect(P + (y + 2) * PITCH + x + 2, 0x1C, 0x19, PITCH, m.r32(self.g(0xE8CBC)))
        duty = m.r8(s + 0x17E)
        if duty and duty < 8:
            self.text(cstr(m, self.g(0x9662A + 8 * duty)), P, PITCH, x + 0x10, y + 0xC, 1, 0x27, 1, 0, 300)
        self.frames(s)
        if slot == self.gb(0x93448) and hp > 0:
            yy = Y + self.off(s, 5)
            xx = self.off(s, 4) + X
            self.frame(P, PITCH, xx, yy, xx + 10, yy + 0x1A, 0x27)

    def shade(self, slot):                                        # 0x552F8: element 104 x 4, remap[0x1B] = 0x70
        m, t = self.m, self.remap()
        old = m.r8(t + 0x1B)
        m.w8(t + 0x1B, 0x70)
        X, Y = self.pos(slot)
        ex, ey, w, h = self.elem(104)
        for dx, dy in ((0, 0), (0x18, 0), (0, 0x14), (0x18, 0x14)):
            self.blit_remap(self.panel + (Y + dy) * PITCH + X + dx, self.sheet + ey * PITCH + ex, w, h,
                            PITCH, PITCH, t, 0)
        m.w8(t + 0x1B, old)

    def slot(self, slot):                                         # DrawMercSlot? 0x32EB8 (DL 0)
        m = self.m
        if not (self.gb(0x92DDF) < 2 and slot < 8):
            return
        s = self.soldier(slot)
        X, Y = self.pos(slot)
        self.draw_elem(self.sel_k(slot, 7), X, Y)
        self.draw_elem(self.sel_k(slot, 7) + 4, X + self.off(s, 4), self.off(s, 5) + Y)
        self.name(s)
        if m.r8(s + 0x182):
            self.health(s)
            self.energy(s)
            self.ap_bar(s)
        b194 = m.r8(s + 0x194)
        if not b194 or slot != self.gb(0x284F0E):
            self.portrait(slot)
        if b194 or m.r8(s + 0x1BB):
            self.shade(slot)
            if slot == self.gb(0x284F0E):
                self.portrait(slot)


def at_waypoint(m, s):                                            # 0x1EE84 (ai.at_waypoint on memory)
    if m.r8(s + 0x1B9) < 5 and m.r16(s + 0x1E8 + 2 * m.r8(s + 0x1F3)) >= 4:
        for o in (0x114, 0x116, 0x118):
            if m.r16(s + o) != m.r16(s + o + 0x10):
                return 0
    return 1


def cstr(m, a):
    out = bytearray()
    while m.r8(a + len(out)):
        out.append(m.r8(a + len(out)))
    return bytes(out)


_runner = None


def sprite_pixels(m, a):
    """compiled sprite at runtime address a -> {(dx, dy): colour} (run in csprite's scratch Unicorn)"""
    global _runner
    if _runner is None:
        from csprite import Runner
        _runner = Runner()
    return _runner.run(bytes(m.read(a, 0x2000)))


def draw_slot(m, slot):
    Panel(m).slot(slot)


def main(argv):
    from emu import Emu
    from pcx import png
    from csprite import palette
    if len(argv) < 2:
        print(__doc__); return
    e = Emu(dump=argv[0], fast=True)
    e.read = lambda a, n: bytes(e.uc.mem_read(a, n))
    e.write = lambda a, b: e.uc.mem_write(a, bytes(b))
    p = Panel(e)
    for k in range(8):
        p.slot(k)
    png(argv[1], 320, 200, e.read(p.panel, 320 * 200), palette(0))


if __name__ == "__main__":
    main(sys.argv[1:])
