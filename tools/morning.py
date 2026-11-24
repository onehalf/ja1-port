"""The tail of the morning chain (DayEndScreen? 0x65BD4, after ExperienceChangeMsg?): mirrors of the small steps.

World w as in duties.py (g8/g16/s8/s16 on linear addresses, ext(name, *args) for stubbed calls).
- duty_wake 0x42378: the guard on duty 7 (0x92DDA) and healed patients / doctors go back on duty.
- profile_sync 0x65B40: the team's salary and stats are copied back into their profiles.
"""
import ai
from ai import s8, s16, i32

SOL = ai.G_SOLDIERS
PROFILES, PROFILE_SIZE = ai.G_PROFILES, ai.PROFILE_SIZE
TEAM_ON_DUTY = 0x229FAC          # i32
DUTY7_MERC = 0x92DDA             # u8 soldier index (< 0x40) whose duty 7 ends in the morning, else 0x40
SALARY, SALARY2, PID, DUTY, IN_SECTOR, HP, HP_MAX, WOUNDS = 0x110, 0x112, 0x179, 0x17E, 0x182, 0x183, 0x184, 0x185
# soldier field -> profile field (0x65B40; salary is a u16)
PROFILE_COPY = ((0x192, 0x44), (0x184, 0x2A), (0x18B, 0x2C), (0x18C, 0x42), (0x18D, 0x2E), (0x18E, 0x40),
                (0x190, 0x41), (0x191, 0x2D), (0x18F, 0x43))


def healed_up(w, s):
    """hpMax - hp = the wounds byte +0x185 (all signed)"""
    return s8(w.g8(s + HP_MAX)) - s8(w.g8(s + HP)) == s8(w.g8(s + WOUNDS))


def duty_wake(w):
    """0x42378"""
    g = w.g8(DUTY7_MERC)
    if g < 0x40 and w.g8(SOL + 0x200 * g + DUTY) == 7:
        w.s8(SOL + 0x200 * g + DUTY, 0)
        w.s8(DUTY7_MERC, 0x40)
    for k in range(8):
        s = SOL + 0x200 * k
        if w.g8(s + DUTY) == 4 and healed_up(w, s):
            w.s16(s + SALARY2, w.g16(s + SALARY))
            w.s32(TEAM_ON_DUTY, w.g32(TEAM_ON_DUTY) + 1)
            w.s8(s + DUTY, 0)
    if not any(w.g8(SOL + 0x200 * k + DUTY) == 4 for k in range(8)):
        for k in range(8):
            s = SOL + 0x200 * k
            if w.g8(s + DUTY) == 3 and healed_up(w, s):
                w.s32(TEAM_ON_DUTY, w.g32(TEAM_ON_DUTY) + 1)
                w.s8(s + DUTY, 0)


def profile_sync(w):
    """0x65B40"""
    for k in range(8):
        s = SOL + 0x200 * k
        if not w.g8(s + IN_SECTOR):
            continue
        p = PROFILES + PROFILE_SIZE * w.g8(s + PID)
        w.s16(p + 0x28, w.g16(s + SALARY))
        for so, po in PROFILE_COPY:
            w.s8(p + po, w.g8(s + so))


# AIM rate complaints (0x45010 -> 0x44E7C; AimRate 0x1FDEC). The accounts block 0x229FA0 (savegame.md):
DEATHS, DAY, BAD_DAYS, TERMINATIONS, HIRINGS = 0x229FA0, 0x229FA4, 0x22A03B, 0x22A03C, 0x22A03D
RESIGNATIONS, NON_PAYMENTS = 0x22A040, 0x22A041
B19C, SLOT = 0x19C, 0x17A
TEAM_PTRS = 0x100958


def aim_rate(w, kind, one_more):
    """AimRate 0x1FDEC: AL = rate in % (byte arithmetic as the original)"""
    day = (w.g8(DAY) + (1 if one_more else 0)) & 0xFF
    other = day if kind == 0 else w.g8(HIRINGS)
    den = (day // 2 + other) & 0xFF
    if den == 0:
        return 0
    den = max(den, 10)
    if kind == 0:
        n = w.g8(DEATHS)
    elif kind == 1:
        n = (w.g8(RESIGNATIONS) + w.g8(TERMINATIONS)) & 0xFF
    elif kind == 2:
        n = w.g8(NON_PAYMENTS)
    else:
        return 0
    if n == 0:
        return 0
    if one_more:
        n = (n + 1) & 0xFF
    r = (n * 100 // den) & 0xFF
    return 100 if r > 100 else r


def complain(w, slot):
    """0x44E7C: merc `slot` (team slot byte) warns about each other merc whose AIM limits are reached"""
    w.ext("w_67bf0", slot, 0x34, 0x25)
    w.ext("w_467e4", 0)
    w.ext("w_673ac")
    said = 0
    for k in range(8):
        s = SOL + 0x200 * k
        if not w.g8(s + IN_SECTOR) or not w.g8(s + HP):
            break
        if w.g8(s + SLOT) == slot:
            continue
        pid = w.g8(s + PID)
        if pid >= 0x3C:
            continue
        p = PROFILES + PROFILE_SIZE * pid
        for kind, quote, lim in ((0, 8, 0x36), (1, 9, 0x37), (2, 0xB, 0x39)):
            if aim_rate(w, kind, True) >= w.g8(p + lim):
                w.ext("complain", slot, quote, pid, said)
                said = 1
        bad = w.g8(BAD_DAYS)
        if bad and bad + 1 >= w.g8(p + 0x38):
            w.ext("complain", slot, 10, pid, said)
            said = 1
    if said:
        w.ext("w_28698", 0x25)


def complain_pick(w):
    """0x45010: up to 8 random team slots (each once); the first informer (in sector, alive, personality +0x19C = 1,
    profile < 60) speaks (complain)"""
    picked = [0] * 8
    for _ in range(8):
        while True:
            r = w.ext("random", 8)
            if not picked[r]:
                break
        picked[r] = 1
        s = SOL + 0x200 * r              # the team pointer table: the test checks it = records 0..7
        if w.g8(s + IN_SECTOR) and w.g8(s + HP) and w.g8(s + B19C) == 1 and w.g8(s + PID) <= 0x3B:
            complain(w, w.g8(s + SLOT))
            return


QUOTE_SHOWN, QUOTE_PROFILE = 0x92F8B, 0x22A1F0     # u8 globals the quote text uses (set before a quote)


def hated_check(w):
    """0x44C10: each team merc whose disliked merc (profile +0x3A) is in the team says quote 6 about it;
    profile +0x3C = that merc"""
    for k in range(8):
        s = SOL + 0x200 * k
        if not w.g8(s + IN_SECTOR):
            break
        if not w.g8(s + HP):
            continue
        pid = w.g8(s + PID)
        if pid >= 0x3C:
            continue
        p = PROFILES + PROFILE_SIZE * pid
        other = w.g8(p + 0x3A)
        n = sum(1 for j in range(8) if w.g8(SOL + 0x200 * j + IN_SECTOR) and w.g8(SOL + 0x200 * j + PID) == other)
        if n & 0xFF == 0:
            continue
        w.s8(p + 0x3C, other)
        w.ext("name", other)
        w.s8(QUOTE_SHOWN, 1)
        w.ext("quote", pid, pid * 15, 6)
        w.ext("replace")
        w.s8(QUOTE_PROFILE, w.g8(p + 0x3C))
        w.ext("w_67bf0", w.g8(p + 0x3C), 0x34, 0x25)
        w.ext("w_467e4", 0)
        w.ext("text_box", k, w.g8(s + SLOT) * 0x25 + 0xB, 0x14, 5)
        w.ext("w_28698", 0x25)


# Corpse export (CorpseExportPrompt? 0x46328 with CorpseRefused? 0x462E0, CorpseGrudge? 0x46548, NativeLeaves? 0x505A0)
BALANCE, DAY_NET, DIFFICULTY = 0x229FC4, 0x229FCC, 0x22A0CC
CORPSE_COST = 0x95C2A            # s16[3] by difficulty: 600, 800, 1000
TEXT_OUT = 0x229B1C              # the text buffer of the question
NICK0 = 0x90FAC                  # the byte a cleared nickname gets


def dead(w, s):
    return s8(w.g8(s + HP)) < 1


def native_leaves(w, s):
    """NativeLeaves? 0x505A0: listed items (type or attachment) go to the stash (a copy), then the soldier is gone"""
    for i in range(12):
        rec = s + 12 * i
        t = w.g8(rec)
        if t and (ai.listed_item(w, t) or ai.listed_item(w, w.g8(rec + 3))):
            ai.stash_item(w, rec, clear=False)
    w.ext("face_clear", w.g8(s + SLOT))
    w.s8(s + 0x16E, w.g8(NICK0))
    w.s8(s + IN_SECTOR, 0)
    w.s8(s + PID, 0x63)


def corpse_grudge(w, k):
    """CorpseGrudge? 0x46548 (team slot k of a dead AIM merc)"""
    r = s8(w.ext("random", 2) & 0xFF)
    if r == -1:
        return
    dpid = w.g8(SOL + 0x200 * k + PID)          # through the pointer table (= records 0..7)
    n = 0
    while True:
        while True:
            u = w.ext("random", 0x3C) & 0xFF
            if u != dpid:
                break
        p = PROFILES + PROFILE_SIZE * u
        if (w.g8(p + 0x3C) != dpid and w.g8(p + 0x3B) == 99 and w.g8(p + 0x4D) == 99 and w.g8(p + 0x2F) != 3):
            on_team = sum(1 for j in range(8) if w.g8(SOL + 0x200 * j + IN_SECTOR) and w.g8(SOL + 0x200 * j + PID) == u)
            if on_team & 0xFF == 0:
                w.s8(p + 0x3B, w.g8(SOL + 0x200 * k + PID))
            else:
                w.s8(p + 0x4D, w.g8(SOL + 0x200 * k + PID))
        n = (n + 1) & 0xFF
        if n >= ((r + 1) & 0xFF):
            break


def corpse_export(w):
    """CorpseExportPrompt? 0x46328"""
    found = False
    for k in range(8):
        s = SOL + 0x200 * k
        if not w.g8(s + IN_SECTOR):
            break
        if dead(w, s) and w.g8(s + PID) < 0x3C:
            found = True
            break
    if found:
        cost = s16(w.g16(CORPSE_COST + 2 * s8(w.g8(DIFFICULTY))))
        w.ext("w_5e8e0", 0x90AA0)
        w.ext("str_replace", b"%d" % cost)
        txt = b"Dish out $%d for corpse export?" % cost
        for j, b in enumerate(txt + b"\0"):
            w.s8(TEXT_OUT + j, b)
        paid = False
        if cost > i32(w.g32(BALANCE)):
            w.ext("confirm", 0x944BE)
        elif w.ext("confirm", 0x944A6):
            w.s32(BALANCE, w.g32(BALANCE) - cost)
            w.s32(DAY_NET, w.g32(DAY_NET) - cost)
            w.ext("w_28698", 0x25)
            w.ext("w_36010", 0x2B, 0x33)
            w.ext("w_67bf0")
            w.ext("blit", i32(w.g32(0x95678) + 0xB680), i32(w.g32(0x93E60) + 0xB680), 0x140, 0x36, 0x140, 0x140)
            w.ext("w_673ac")
            paid = True
        if not paid:
            for k in range(8):                  # CorpseRefused? 0x462E0
                s = SOL + 0x200 * k
                if dead(w, s) and w.g8(s + IN_SECTOR) and w.g8(s + PID) < 0x3C:
                    corpse_grudge(w, k)
    k = 0
    while k < 8:
        s = SOL + 0x200 * k
        if dead(w, s) and w.g8(s + IN_SECTOR):
            if w.g8(s + PID) < 0x3C:
                ai.merc_leaves(w, s, 0)
            else:
                native_leaves(w, s)
            k -= 1
            ai.team_compact(w, 0)
        k += 1
    w.ext("bass_map")
    w.ext("w_36010", 0x2B, 0x33)
    w.ext("w_4508c")


# Salary raise (RaiseSalaryPrompt? 0x448A8 with RaiseReason? 0x44634 and the answer rule of SalaryDialog? 0x29074)
OFFER = 0x22A470                 # i16: the salary the player offers in the dialog
QUOTE_WAIT = 0x92F8B             # byte set to 1 before each quote
GRUDGE_SHOWN = 0x22A1F0          # the profile of the grudge merc the raise asks about
TEXT_BUF, QUOTE_TEXT = 0xEBAC0, 0x2299BC
NO_WAIT = 0x1163BD               # not 0: no 0x28698 after a dialog answer


def cstr(w, a):
    out = bytearray()
    while w.g8(a + len(out)):
        out.append(w.g8(a + len(out)))
    return bytes(out)


def raise_reason(w, s):
    """0x44634: 0 = no reason, 1 fatality, 2 turnover, 3 bad days (or +0x19C = 8 with +0x1D9), 4 non-payment,
    5 = a grudge merc (profile +0x4D < 60: the merc's voice, GRUDGE_SHOWN); returns (reason, name bytes)"""
    if w.g8(s + B19C) == 8 and w.g8(s + 0x1D9):
        return 3, b""
    p = PROFILES + PROFILE_SIZE * w.g8(s + PID)
    if aim_rate(w, 0, 0) >= w.g8(p + 0x36):
        return 1, b""
    if aim_rate(w, 1, 0) >= w.g8(p + 0x37):
        return 2, b""
    if aim_rate(w, 2, 0) >= w.g8(p + 0x39):
        return 4, b""
    if w.g8(BAD_DAYS) >= w.g8(p + 0x38):
        return 3, b""
    g = w.g8(p + 0x4D)
    if g >= 60:
        return 0, b""
    q = PROFILES + PROFILE_SIZE * g
    name = bytearray()
    while w.g8(q + len(name)):
        name.append(w.g8(q + len(name)))
    w.ext("voice", w.g8(s + PID), s8(g))
    w.s8(GRUDGE_SHOWN, g)
    return 5, bytes(name)


def salary_answer(w, pid, slot, offer):
    """the answer rule of 0x29074: -1 = no offer (< 1), 0 = refused, 1 = ask again, 2 = accepted.
    pct = (offer - salary of soldier `slot`) * 100 / salary (int, toward zero, then 16-bit); profile +0x30 = the
    raise in % the merc accepts (1: any offer counts as 1 %, 0: never), +0x31 = below it the merc refuses"""
    offer = s16(offer)
    if offer < 1:
        return -1
    sal = w.g16(SOL + 0x200 * slot + SALARY)
    d = (offer - sal) * 100
    pct = s16((abs(d) // sal * (1 if d >= 0 else -1)) & 0xFFFF)
    p = PROFILES + PROFILE_SIZE * pid
    if w.g8(p + 0x30) == 1 and pct < 1:
        pct = 1
    if pct >= w.g8(p + 0x30) and w.g8(p + 0x30):
        return 2
    return 0 if pct < w.g8(p + 0x31) else 1


def raise_salary(w):
    """RaiseSalaryPrompt? 0x448A8: every team merc with a reason asks for a raise; no deal = the merc leaves"""
    k = -1
    while True:
        k += 1
        s = SOL + 0x200 * k
        if not w.g8(s + IN_SECTOR) or k >= 8:
            return
        if not w.g8(s + HP) or w.g8(s + PID) >= 60:
            continue
        reason, name = raise_reason(w, s)
        if s8(reason) < 1:
            continue
        pid = w.g8(s + PID)
        w.s8(QUOTE_WAIT, 1)
        w.ext("quote", pid, s8(reason))
        for j, b in enumerate(name):
            w.s8(TEXT_BUF + j, b + 1)
        w.s8(TEXT_BUF + len(name), 0)
        src, pat = cstr(w, QUOTE_TEXT), cstr(w, 0x90B24)     # StrReplace 0x252B0: quote text with the name
        rep = bytes(x + 1 & 0xFF for x in name)
        w.ext("str_replace", rep)
        for j, x in enumerate(src.replace(pat, rep, 1) + b"\0"):
            w.s8(TEXT_OUT + j, x)
        j = 0
        while True:
            b = w.g8(TEXT_OUT + j)
            w.s8(QUOTE_TEXT + j, b)
            if not b:
                break
            j += 1
        w.ext("w_67bf0")
        w.ext("w_467e4", 0, 0x34, 0x25)
        w.ext("w_673ac")
        w.ext("text_box", k, w.g8(s + SLOT) * 0x25 + 0xB, 0x14, 5)
        p = PROFILES + PROFILE_SIZE * pid
        w.s16(OFFER, w.g16(s + SALARY))
        leaves = True
        n = 0
        while w.g8(p + 0x32) > n:
            offer = w.ext("offer", s16(w.g16(OFFER))) & 0xFFFF
            w.s16(OFFER, offer)
            r = salary_answer(w, pid, w.g8(s + SLOT), offer)
            if 0 <= r < 2:
                w.s8(QUOTE_WAIT, 1)
                w.ext("quote", pid, 0xE)
            elif r == 2:
                new = s16(w.g16(OFFER))
                if w.g16(s + SALARY2) < w.g16(s + SALARY):
                    w.s16(s + SALARY2, int(new / 2))
                else:
                    w.s16(s + SALARY2, new)
                w.s16(s + SALARY, new)
                w.s16(p + 0x28, new)
                leaves = False
                w.s8(QUOTE_WAIT, 1)
                w.ext("quote", pid, 0xD)
                o = {1: 0x36, 2: 0x37, 3: 0x38, 4: 0x39, 5: 0x4D}.get(reason)
                if o:
                    w.s8(p + o, {0x38: 10, 0x4D: 99}.get(o, 0x65))
            if r >= 0:
                w.ext("w_67bf0")
                if not w.g8(NO_WAIT):
                    w.ext("w_28698", 0x25)
                w.ext("text_box", k, w.g8(s + SLOT) * 0x25 + 0xB, 0x14, 5)
            if r != 1:
                break
            n += 1
        w.ext("w_28698", 0x25)
        if leaves:
            if w.g8(s + B19C) == 8 and w.g8(s + 0x1D9):
                w.s8(s + 0x18D, 0)
            ai.merc_leaves(w, s, 1)
            w.s8(s + PID, 0x63)
            w.s8(s + IN_SECTOR, 0)
            w.s8(RESIGNATIONS, w.g8(RESIGNATIONS) + 1)
            k -= 1
            ai.team_compact(w, 0)
        w.ext("bass_map")
        w.ext("w_36010", 0x2B, 0x33)
        w.ext("w_4508c")
