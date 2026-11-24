"""PayDayDialog? 0x45100 mirror (strategic.md "Not enough funds"): in the morning, while the balance is below 0, the
player must dismiss mercs (no pay: the merc leaves the team) until the balance is 0 or more, the team is empty, or
only a native guide (who works for free) is left.

World w as in invscreen.py: g8/g16/g32/s8/s16/s32 on linear addresses and ext(name, *args) for the stubbed UI
(drawing, messages, prompts, sounds, timer). ext("poll") is the mouse poll 0x678E8: it writes the mouse x / y
(0x229D00 / 0x229D04) and the buttons (0x229CF4), and ends the run when the script is over.
"""
import ai

MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4
SOL = ai.G_SOLDIERS
TIP = 0x93EA8                       # the item tooltip number (0x23420 clears it)
BALANCE = 0x229FC4                  # g_balance
TERMINATIONS, NON_PAYMENTS = 0x22A03C, 0x22A041
SPEECH_WAIT = 0x1163BD              # != 0: the balloon is drawn before the quote's wait (0x28698), else after it
MSG_FUNDS, MSG_CHOOSE, MSG_GUIDE = 0x9807F, 0x980F7, 0x9816F    # "NOT ENOUGH FUNDS ...", "Choose someone ...", guide
ASK_FAMILY, ASK_MEMBER, MEMBER = 0x981E7, 0x9825F, 0x90B2C      # "Don't pay MEMBER's family?", "Don't pay MEMBER?"
QUOTE_BUF = 0x229B1C
FUNDS_RECT = 0x934F8                # i16 x at +2, i16 y at +4: the funds box (a click on it repeats MSG_FUNDS)


def sol(k):
    return SOL + 0x200 * k


def mouse_in(w, x0, y0, x1, y1):
    """MouseInRect? 0x67C0C"""
    x, y = ai._i32(w.g32(MOUSE_X)), ai._i32(w.g32(MOUSE_Y))
    return x0 <= x <= x1 and y0 <= y <= y1


def team_size(w):
    """TeamSize 0x513B8: team slots in use, counted from slot 0 up to the first empty one (at most 8)"""
    n = 0
    while n < 8 and w.g8(sol(n) + 0x182):
        n += 1
    return n


def first_guide(w):
    """0x62A00: the first team slot whose soldier is in the sector, has hp and is a native guide (profile 60..64);
    0x40 = none"""
    for k in range(8):
        s = sol(k)
        if w.g8(s + 0x182) and w.g8(s + 0x183) and 0x3C <= w.g8(s + 0x179) <= 0x40:
            return k
    return 0x40


def faces_redraw(w):
    """0x4508C: the face frame (panel element 0x67) of each team merc who is not a native guide, then the screen"""
    for k in range(8):
        s = sol(k)
        if not w.g8(s + 0x182):
            break
        if w.g8(s + 0x179) < 0x3C:
            w.ext("panel_elem", 0x67, w.g32(0x93E60), 0x140, 0xD + 0x25 * k, 0x14)
    w.ext("hide")
    w.ext("w_46bf4")
    w.ext("show")


def dismiss(w, k, x0):
    """the yes branch for team slot k (face rectangle x x0): the merc leaves without pay"""
    s = sol(k)
    prof = w.g8(s + 0x179)
    if ai.s8(w.g8(s + 0x183)) > 0:
        w.s8(0x92F8B, 1)
        w.ext("quote", prof, prof * 15, 0, 200, 0x2299BC, 2, prof, 1)
        if not w.g8(SPEECH_WAIT):
            w.ext("w_28698", 0x25)
            w.ext("show")
        w.ext("balloon", s, x0, 0x14, 5)
    else:                                   # dead: his family is not paid
        p = ai.G_PROFILES + ai.PROFILE_SIZE * prof
        w.s16(p + 0x3E, w.g16(s + 0x112))
        w.s8(TERMINATIONS, w.g8(TERMINATIONS) + 1)
    ai.merc_leaves(w, s, 1)
    w.s8(s + 0x182, 0)
    w.s8(NON_PAYMENTS, w.g8(NON_PAYMENTS) + 1)
    w.s8(s + 0x179, 0x63)
    if w.g8(SPEECH_WAIT):
        w.ext("w_28698", 0x25)
    w.ext("show")
    w.ext("tip_item", w.g8(TIP))
    ai.team_compact(w, 0)
    ai.apply_day_ledger(w)
    w.ext("bass_map")
    w.ext("w_36010", 0x90B18, w.g32(0x93E60), 0x140, 0x2B, 0x33)
    w.ext("hide")
    w.ext("w_46bf4")
    w.ext("show")


def payday(w):
    """PayDayDialog? 0x45100"""
    first, blink = True, 0
    while ai._i32(w.g32(BALANCE)) < 0 and team_size(w):
        if team_size(w) == 1 and first_guide(w) == 0:
            break
        if first:
            w.ext("message", MSG_FUNDS)
            w.ext("message", MSG_CHOOSE)
            first = False
            w.ext("timer_start", 0x22A404)
        w.ext("w_682ac", 0, 0, 0x13F, 0xC7)
        w.ext("cursor2", 0)
        done = False
        while not done:
            w.ext("poll")
            hover = False
            for k in range(8):
                x0 = 0xC + 0x25 * k
                if mouse_in(w, x0, 0x14, x0 + 0x1E, 0x2E) and w.g8(sol(k) + 0x182):
                    w.ext("tip_merc", k + 1, x0 + 1, 0x14, x0 + 0x1D, 0x2D, 0x99E61, 0x90A60, 0x90A60, 0)
                    hover = True
            if not hover:
                w.ext("tip_item", w.g8(TIP))
            if ai._i32(w.ext("elapsed", 0x22A404)) > 200:
                if ai._i32(w.g32(MOUSE_Y)) > 0x91:
                    w.ext("hide")
                blink ^= 1
                if blink:
                    w.ext("w_36010", 0x90A60, w.g32(0x95678), 0x140, 0x34, 0x35)
                else:
                    w.ext("w_36010", 0x90A60, w.g32(0x95678), 0x140, 0x2F, 0x33)
                w.ext("timer_start", 0x22A404)
                if ai._i32(w.g32(MOUSE_Y)) > 0x91:
                    w.ext("show")
            if not w.g8(BUTTONS) & 1:
                continue
            for k in range(8):
                s = sol(k)
                if not w.g8(s + 0x182):
                    break
                x0 = 0xD + 0x25 * k
                if not mouse_in(w, x0, 0x14, x0 + 0x1B, 0x2C):
                    continue
                if w.g8(s + 0x179) >= 0x3C:
                    w.ext("hide")
                    w.ext("message", MSG_GUIDE)
                    w.ext("show")
                    continue
                w.ext("str_replace", ASK_FAMILY if ai.s8(w.g8(s + 0x183)) <= 0 else ASK_MEMBER, QUOTE_BUF,
                      s + 0x16E, MEMBER)
                w.ext("hide")
                w.ext("w_467e4", 0, 0x34, 0x25, 1)
                if not w.ext("yes_no", QUOTE_BUF, w.g8(s + 0x179), s + 0x16E, 1, 0) & 0xFF:
                    w.ext("w_28698", 0x25)
                    w.ext("show")
                    continue
                dismiss(w, k, x0)
                done = True
                break
            fx, fy = ai.s16(w.g16(FUNDS_RECT + 2)), ai.s16(w.g16(FUNDS_RECT + 4))
            if mouse_in(w, fx, fy, fx + 0x19, fy + 0xF):
                w.ext("message", MSG_FUNDS)
    w.ext("hide")
    w.ext("w_681d0", ai.s16(w.g16(FUNDS_RECT + 2)), ai.s16(w.g16(FUNDS_RECT + 4)), 0x13F, 0xC7)
    w.ext("w_682ac", 0, 0x92, 0x13F, 0xC7)
    w.ext("w_674b0", 0xC9, 0x92, 0x140, 0x2B)
    w.ext("show")
    w.ext("w_36010", 0x90B18, w.g32(0x93E60), 0x140, 0x2B, 0x33)
    if ai._i32(w.g32(BALANCE)) >= 0:
        faces_redraw(w)
