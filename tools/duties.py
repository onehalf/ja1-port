"""Morning duties mirror (0x4B48C, called by the morning chain after PayMercs? and before PayDayDialog?): healing of
resting and patient mercs, then per duty (+0x17E) rest, training (+0x199 = what), doctor and repair practice.

World w as in payday.py: g8/g16/g32/s8/s16/s32 on linear addresses and ext(name, *args) for the stubbed calls:
ext("random", n) = Random 0x53538 (0..n-1), ext("practice", soldier, stat, amount, flag) = StatPractice 0x62A68
(amount a signed 16-bit value). DropItem 0x470B0 is a 12-byte clear (done here).
"""
import ai
from ai import s8, s16, cdiv

SOL = ai.G_SOLDIERS
ITEM_TBL = ai.ITEM_TBL
HP, HP_MAX, DUTY, TRAIN, HEALED, SLOT = 0x183, 0x184, 0x17E, 0x199, 0x19A, 0x17A
AGI, DEX, WIS, MED, EXPL, MECH, MARKS, LEVEL = 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191, 0x192
FIRST_AID, MEDKIT, TOOLKIT = 0x49, 0x4A, 0x4B
REPAIR_PLACES = (1, 7, 8, 9, 10, 11)        # table 0x470D0


def sol(k):
    return SOL + 0x200 * k


def sb(w, a):
    return s8(w.g8(a))


def drop(w, it):
    """DropItem 0x470B0"""
    for j in range(12):
        w.s8(it + j, 0)


def need(w, s):
    """hpMax - (hp + healed), signed"""
    return sb(w, s + HP_MAX) - (sb(w, s + HP) + w.g8(s + HEALED))


def med_weight(t):
    return 1 if t == FIRST_AID else 2 if t == MEDKIT else 0


def unit(w, it, j):
    """the condition of stacked unit j: byte +2 for j = 0, else the byte at +7 + j (signed)"""
    return sb(w, it + 2) if j == 0 else sb(w, it + 7 + j)


def med_points(w, s):
    """0x48318: first aid kit units count once, medical kit units twice"""
    total = 0
    for p in range(12):
        it = s + 12 * p
        m = med_weight(w.g8(it))
        if m:
            j = 0
            while j < sb(w, it + 1):
                total += unit(w, it, j) * m
                j += 1
    return total


def doctor_cap(w, s):
    """0x483AC -> (min(med points, cap), cap)"""
    t = cdiv((100 - sb(w, s + DEX)) * 20, 100)
    cap = cdiv((sb(w, s + WIS) - t) * sb(w, s + MED), 100) * 2
    return min(med_points(w, s), cap), cap


def doctor(w, s):
    """0x48418: the doctor heals himself first, then the patients (duty 3 or 4) evenly, then one point each"""
    avail = doctor_cap(w, s)[0]
    if not avail:
        return 0
    own = 0
    while need(w, s) != 0 and avail:
        avail -= 1
        own += 1
        w.s8(s + HEALED, w.g8(s + HEALED) + 1)
    if own:
        w.ext("practice", s, 5, s16(cdiv(own, 3) & 0xFFFF), 0)
    if not avail:
        return own
    me = w.g8(s + SLOT)
    pats = [k for k in range(8) if k != me and w.g8(sol(k) + DUTY) in (3, 4)]
    if not pats:
        return own
    per = cdiv(avail, len(pats))
    done = 0
    for k in range(8):
        if k == me or w.g8(sol(k) + DUTY) not in (3, 4):
            continue
        p, n = sol(k), 0
        while avail and need(w, p) and n < per:
            avail -= 1
            done += 1
            n += 1
            w.s8(p + HEALED, w.g8(p + HEALED) + 1)
    if avail:
        for k in range(8):
            p = sol(k)
            if k != me and w.g8(p + DUTY) in (3, 4) and avail and need(w, p):
                avail -= 1
                done += 1
                w.s8(p + HEALED, w.g8(p + HEALED) + 1)
    if done:
        w.ext("practice", s, 5, s16(cdiv(done, 3) & 0xFFFF), 0)
    return own + done


def use_supplies(w, s, used, total):
    """0x48224: the medical units used up, from the last unit of each kit to the first"""
    used = min(used, total)
    for p in range(12):
        it = s + 12 * p
        m = med_weight(w.g8(it))
        if not m:
            continue
        c = sb(w, it + 1) - 1
        while c >= 0:
            if c == 0:
                v = sb(w, it + 2)
                if v * m <= used:
                    drop(w, it)
                    used -= v * m
                else:
                    w.s8(it + 2, (v - cdiv(used, m)) & 0xFF)
                    used = 0
            else:
                v = sb(w, it + 7 + c)
                if v * m <= used:
                    w.s8(it + 1, w.g8(it + 1) - 1)
                    used -= v * m
                    w.s8(it + 7 + c, 0)
                else:
                    w.s8(it + 7 + c, (v - cdiv(used, m)) & 0xFF)
                    used = 0
            c -= 1


def repair_cap(w, s):
    """0x4882C -> (points from the toolkit's condition, points with a perfect toolkit)"""
    v = cdiv((sb(w, s + MECH) * sb(w, s + DEX) + (sb(w, s + LEVEL) - 1) * 5) * 3, 200)
    t = sb(w, s + 2) if w.g8(s) == TOOLKIT else 0
    return cdiv(t * v, 100), v


def repair(w, s):
    """0x48684: repair the items in places 1, 7..11 (types with item table byte +9), then wear the toolkit"""
    pts = repair_cap(w, s)[0]
    if not pts:
        return
    done = 0
    for p in REPAIR_PLACES:
        it = s + 12 * p
        if not w.g8(ITEM_TBL + 12 * w.g8(it) + 9):
            continue
        q = sb(w, it + 1)
        if q < 0 and pts >= 2:
            w.s8(it + 1, -q & 0xFF)
            done += 2
            pts -= 2
        n = 100 - sb(w, it + 2)
        while n and pts:
            n -= 1
            done += 1
            pts -= 1
            w.s8(it + 2, w.g8(it + 2) + 1)
        if w.g8(ITEM_TBL + 12 * w.g8(it) + 6) > 1 and sb(w, it + 1) > 1:
            c = 0
            while c < sb(w, it + 1) - 1:
                n = 100 - sb(w, it + 8 + c)
                while n and pts:
                    w.s8(it + 8 + c, w.g8(it + 8 + c) + 1)
                    n -= 1
                    done += 1
                    pts -= 1
                c += 1
    if not done:
        return
    w.ext("practice", s, 7, s16(done & 0xFFFF), 0)
    if ai._i32(w.ext("random", 100)) < done and sb(w, s + 2) > 10:
        x = w.ext("random", s16((cdiv(done, 15) + 1) & 0xFFFF))
        c = (w.g8(s + 2) - ((x + 1) & 0xFF)) & 0xFF
        w.s8(s + 2, c)
        if s8(c) < 0:
            w.s8(s + 2, 0)
            drop(w, s)


TRAINING = {0: [(1, HP_MAX, 4), (2, AGI, 4), (3, DEX, 4)], 1: [(5, MED, 12)],
         2: [(6, LEVEL, 12)],           # explosives (stat 6) practice measured by the experience level (0x4B60B)
         3: [(7, MECH, 12)], 4: [(8, MARKS, 12)]}


def morning_duties(w):
    """0x4B48C"""
    for k in range(8):
        s = sol(k)
        d = sb(w, s + HP_MAX) - sb(w, s + HP)
        if d == 0 or w.g8(s + DUTY) not in (1, 4):
            continue
        n = cdiv(sb(w, s + HP_MAX), 10) & 0xFF
        if not n:
            continue
        r = ((w.ext("random", n) + 1) + (w.ext("random", n) + 1)) & 0xFF
        if r > d:
            r = d & 0xFF
        w.s8(s + HEALED, w.g8(s + HEALED) + r)
    for k in range(8):
        s = sol(k)
        duty = w.g8(s + DUTY)
        if duty in (0, 7):
            w.ext("practice", s, 1, s16((100 - sb(w, s + HP_MAX)) & 0xFFFF), 0)
        elif duty == 2:
            for stat, f, m in TRAINING.get(w.g8(s + TRAIN), []):
                w.ext("practice", s, stat, s16(((100 - sb(w, s + f)) * m) & 0xFFFF), 1)
        elif duty == 3:
            total = med_points(w, s)
            used = doctor(w, s)
            use_supplies(w, s, used, total)
        elif duty == 5:
            repair(w, s)
        if w.g8(s + 0x182) and not w.g8(s + 0x188) and sb(w, s + HP) >= 15:
            w.ext("practice", s, 9, 0x19, 0)
