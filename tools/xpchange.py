"""Morning stat changes mirror (ExperienceChangeMsg? 0x45BBC, called by the morning chain after PayDayDialog?): the
practice progress of each merc becomes stat points (health, agility, ..., level), a new level changes the salary
(0x62DA8) and the effective pay, and the stats that changed are flagged for the next screen (0x10DD10).

World w as in duties.py: g8/g16/s8/s16 on linear addresses and ext(name, *args) for the stubbed calls:
ext("stat_begin") = the key check 0x67300 + TimerStart 0x47020 of one stat, ext("stat_end") = the wait
(TimerElapsedMs 0x47040), ext("count", slot, stat, ptr, delta) = the counting display 0x45624 (it then stores
value + delta; done here), ext("hpbar", k, x) = 0x47144, ext("msg", text) = StatusMessage 0x27D24,
ext("w_67478", 0, 0x13F, 199) and ext("w_682ac", 0, 0) at the end.
"""
import struct
import ai
from ai import s8, s16, cdiv

SOL = ai.G_SOLDIERS
PROG = ai.G_STAT_PROGRESS          # i16[9] per team slot
CHANGED = 0x10DD10                 # u8[64 x 10]: profile x 10 + stat (0 = pay) changed today
SALARY, SALARY2, NICK, IN_SECTOR, HP, HP_MAX, DUTY, SLOT, PID, HEALED = \
    0x110, 0x112, 0x16E, 0x182, 0x183, 0x184, 0x17E, 0x17A, 0x179, 0x19A
# stat 1..9 -> (the field 0x47350 reads and 0x45624 writes, the lowest value)
FIELDS = {1: (HP, 15), 2: (0x18B, 1), 3: (0x18C, 1), 4: (0x18D, 1), 5: (0x18E, 0), 6: (0x18F, 0), 7: (0x190, 0),
          8: (0x191, 0), 9: (0x192, 1)}
S_RISEN, S_FALLEN, S_AIM_UP, S_AIM_DOWN, S_TAIL = 0x98BBF, 0x98C37, 0x98CAF, 0x98D27, 0x98D9F
DOWN = struct.unpack("<d", struct.pack("<Q", 0x3FE5555555555555))[0]   # 0x911ED (2/3 rounded down)


def chop(n, f):
    """n x f on the x87 in double precision (53-bit mantissa, round to nearest even: 72 x 2/3 = 48), then
    truncated (__CHP); n >= 0"""
    return int(n * f)


def salary_step(sal, up):
    """0x62DA8: salary x 1.5 (up) or x 2/3, rounded to 5 / 10 / 25 / 50 / 100 / 500 / 1000 (half up)"""
    v = chop(sal & 0xFFFF, 1.5 if up else DOWN) & 0xFFFFFFFF
    u = v & 0xFFFF
    g = 5 if u <= 500 else 10 if u <= 1000 else 25 if u <= 1500 else 50 if u <= 2500 else 100 if u <= 5000 \
        else 500 if u <= 10000 else 1000
    r = u % g
    return (((v - r) & 0xFFFF) + (r * 2 // g) * g) & 0xFFFF


def cstr(w, a):
    out = bytearray()
    while w.g8(a + len(out)):
        out.append(w.g8(a + len(out)))
    return bytes(out)


def value(w, s, stat):
    """0x47350: salary2 for stat 0, else the stat's field as a signed byte"""
    return w.g16(s + SALARY2) if stat == 0 else s8(w.g8(s + FIELDS[stat][0]))


def count(w, k, stat, d):
    """0x45624 on soldier k: the display (stubbed), then the field = value + d (a word for stat 0, else a byte)"""
    s = SOL + 0x200 * k
    ptr = s + (SALARY2 if stat == 0 else FIELDS[stat][0])
    w.ext("count", w.g8(s + SLOT), stat, ptr, s16(d & 0xFFFF))
    v = value(w, SOL + 0x200 * w.g8(s + SLOT), stat) + s16(d & 0xFFFF)
    if stat == 0:
        w.s16(ptr, v)
    else:
        w.s8(ptr, v)


def level_delta(p, v):
    """the level (stat 9): whole steps of (level + d) x 1000 progress -> (d, progress), clamped to 1..10"""
    d = 0
    if p < 0:
        while not (v + d) * -1000 < p:
            step = s16((v + d) & 0xFFFF)
            d = s16((d - 1) & 0xFFFF)
            p = s16((p + step * 1000) & 0xFFFF)
    else:
        while not p < (d + v) * 1000:
            step = s16((v + d) & 0xFFFF)
            d = s16((d + 1) & 0xFFFF)
            p = s16((p - step * 1000) & 0xFFFF)
    if d + v > 10:
        d, p = s16((10 - v) & 0xFFFF), 0
    if d + v < 1:
        d, p = s16((1 - v) & 0xFFFF), 0
    return d, p


def stat_changes(w):
    """0x45BBC"""
    k = 0
    while k <= 7 and w.g8(SOL + 0x200 * k + IN_SECTOR):
        s = SOL + 0x200 * k
        if not w.g8(s + HP):
            k += 1
            continue
        for stat in range(1, 10):
            w.ext("stat_begin")
            field, low = FIELDS[stat]
            pa = PROG + 18 * k + 2 * (stat - 1)
            p = s16(w.g16(pa))
            v = s8(w.g8(s + field))
            if stat == 9:
                d, p = level_delta(p, v)
            else:
                d = s16(int(p / 100) & 0xFFFF)
                p = s16((p - 100 * int(p / 100)) & 0xFFFF)
                if d + v > 100:
                    d, p = s16((100 - v) & 0xFFFF), 0
                if d + v < low:
                    d, p = s16((low - v) & 0xFFFF), 0
            w.s16(pa, p)
            if stat == 1:
                w.s8(s + HP_MAX, w.g8(s + HP_MAX) + d)
                d = s16((d + w.g8(s + HEALED)) & 0xFFFF)
                w.s8(s + HEALED, 0)
            if d:
                w.s8(CHANGED + 10 * w.g8(s + PID) + stat, 1)
            count(w, k, stat, d)
            if stat == 1:
                w.ext("hpbar", k, w.g8(s + SLOT) * 0x25 + 0x2C)
            if stat == 9 and d:
                for _ in range(abs(d)):
                    w.s16(s + SALARY, salary_step(w.g16(s + SALARY), d > 0))
                sal = w.g16(s + SALARY)
                text = cstr(w, s + NICK) + cstr(w, S_RISEN if d > 0 else S_FALLEN) + b" %d!  " % s8(w.g8(s + 0x192)) \
                    + cstr(w, S_AIM_UP if d > 0 else S_AIM_DOWN) + b" $%d " % sal + cstr(w, S_TAIL)
                w.ext("msg", text)
                if w.g8(s + DUTY) in (1, 4):
                    d2 = cdiv(sal + 1, 2) - w.g16(s + SALARY2)
                else:
                    d2 = sal - w.g16(s + SALARY2)
                d2 = s16(d2 & 0xFFFF)
                if d2:
                    w.s8(CHANGED + 10 * w.g8(s + PID), 1)
                count(w, k, 0, d2)
            w.ext("stat_end")
        k += 1
    w.ext("w_67478", 0, 0x13F, 199)
    w.ext("w_682ac", 0, 0)
