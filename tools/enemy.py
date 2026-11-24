"""Analysis mirror of the enemy commander's day-clock moves (validate/diff/enemy_diff.py tests it against the original).

enemy_pick 0x3F45C (op 310), the order shuffle 0x3ED94, enemy_move 0x3EDE8 (op 311) with its helpers: take an enemy
0x3DFDC, put an enemy 0x3DF4C (walk-in 0x306BC = w.ext("walk_in", slot, edge)), the defence weights 0x3EC9C /
0x3ED7C and the attack weight 0x3ECB4. Host calls through w.ext: sector_value (SectorValue 0x3E03C: ai.sector_value
runs on w), walk_in, replay (RecordReplayEvent 0x3FA04), resolve (ResolveSectorBattle 0x3E508), time_compress
(0x11D40 with 0). Random through w.random.

The x87 arithmetic: every operation rounds to a 64-bit mantissa (extended precision, round to nearest), the score
is stored as a float (24-bit mantissa), __CHP + fistp truncates toward zero; a division by zero, an infinity or a
NaN gives the integer 0x80000000.
"""
from fractions import Fraction

from ai import G_CUR_SECTOR, G_SECTOR_TBL9, G_SECTORS, G_SECTOR_ORDER, _story_cond, s16, sector_value  # noqa: F401

G_ENEMY_COUNTER = 0x22A0BD     # u8: index into g_sectorOrder of the last pick (0..60)
G_SANTINO = 0x22A008           # i32: Santino's reserve (enemies he can still send into sector 0)
G_GUARD_SKILL = 0x22A030       # u8
G_SUPPLIES = 0x22A0BE          # u8 supplies[0]
G_SPLASH = 0x92E76             # s8 per sector
G_TIME_COMPRESS = 0x9300C
OFFSET = (1, 10, -10, -1)      # east, south, north, west
BONUS = (15, 10, -10, -15)
WALL = (4, 8, 2, 0x10)         # the source's barrier bit toward the neighbour
EDGE = (3, 5, 1, 7)            # the edge code the walk-in gets


def s8(v):
    v &= 0xFF
    return v - 256 if v & 0x80 else v


BAD = "bad"                    # an infinity / NaN / division by zero
INF = "inf"                    # the score +infinity


def _round(x, bits):
    """x (Fraction) rounded to `bits` mantissa bits, to nearest, ties to even"""
    if x == 0 or x is BAD:
        return x
    neg, a = x < 0, abs(x)
    e = a.numerator.bit_length() - a.denominator.bit_length()
    if Fraction(2) ** e > a:
        e -= 1
    q = a / Fraction(2) ** (e - bits + 1)
    n = q.numerator // q.denominator
    r = q - n
    if r > Fraction(1, 2) or (r == Fraction(1, 2) and n & 1):
        n += 1
    v = n * Fraction(2) ** (e - bits + 1)
    return -v if neg else v


def _ext(x):
    return _round(x, 64)


def _div(a, b):
    if a is BAD or b is BAD or b == 0:
        return BAD
    return _ext(Fraction(a) / Fraction(b))


def _mul(a, b):
    if a is BAD or b is BAD:
        return BAD
    return _ext(Fraction(a) * Fraction(b))


def _chop(x):
    """__CHP + fistp dword"""
    if x is BAD:
        return 0x80000000
    v = int(x)          # toward zero
    return v & 0xFFFFFFFF if -(1 << 31) <= v < (1 << 31) else 0x80000000


def _share(w, tot, val, v0):
    """chop(tot x (val / (val + v0)) + 0.5): the low byte"""
    x = _mul(tot, _div(val, val + v0))
    if x is not BAD:
        x = _ext(x + Fraction(1, 2))
    return _chop(x) & 0xFF


def _t(w, s, k):
    return w.g8(G_SECTOR_TBL9 + 9 * s + k)


def defend_merc(w):
    """0x3EC9C"""
    return w.g8(G_GUARD_SKILL) // 2 + 50


def defend_guard(w):
    """0x3ED7C"""
    return w.g8(G_GUARD_SKILL) // 2 + 40


def attack_weight(w, sector, f):
    """0x3ECB4: supplies[0] / 2 + 60; x 4 for the story sectors (the story condition) and for 0x35 while story byte
    0x22A04D = 1; else halved when f = 0"""
    v = w.g8(G_SUPPLIES) // 2 + 60
    if _story_cond(w, sector) or (sector & 0xFF == 0x35 and s8(w.g8(0x22A04D)) == 1):
        return v * 4
    return v if f & 0xFF else v // 2


def take_enemy(w, s):
    """0x3DFDC: the highest enemy slot k (7..0) of sector s with s8 > 0 is negated, enemies - 1; 1. None: 0."""
    base = G_SECTORS + 0xB1 * s + 0x96
    for k in range(7, -1, -1):
        if s8(w.g8(base + k)) > 0:
            w.s8(base + k, -s8(w.g8(base + k)) & 0xFF)
            w.s8(G_SECTOR_TBL9 + 9 * s + 1, (_t(w, s, 1) - 1) & 0xFF)
            return 1
    return 0


def put_enemy(w, s, edge):
    """0x3DF4C: the first enemy slot k (0..7) of sector s with s8 < 0; with mercs in s the walk-in (slot 0x28 + k,
    edge) must answer != 0 (low byte). Then the slot is negated, enemies + 1; 1. No slot or no walk-in: 0."""
    base = G_SECTORS + 0xB1 * s + 0x96
    for k in range(8):
        if s8(w.g8(base + k)) < 0:
            break
    else:
        return 0
    if _t(w, s, 2) and not w.ext("walk_in", k + 0x28, edge) & 0xFF:
        return 0
    w.s8(base + k, -s8(w.g8(base + k)) & 0xFF)
    w.s8(G_SECTOR_TBL9 + 9 * s + 1, (_t(w, s, 1) + 1) & 0xFF)
    return 1


def shuffle(w):
    """0x3ED94: for i = 60 down to 1: j = Random(i) (0 for i = 1), swap order[i] and order[j]"""
    for i in range(60, 0, -1):
        j = (w.random(i) if i > 1 else 0) & 0xFF
        a, b = w.g8(G_SECTOR_ORDER + i), w.g8(G_SECTOR_ORDER + j)
        w.s8(G_SECTOR_ORDER + j, a)
        w.s8(G_SECTOR_ORDER + i, b)


def enemy_pick(w):
    """0x3F45C: counter + 1 (8-bit); above 60: shuffle and counter 0. Result order[counter]."""
    c = (w.g8(G_ENEMY_COUNTER) + 1) & 0xFF
    w.s8(G_ENEMY_COUNTER, c)
    if c > 60:
        shuffle(w)
        w.s8(G_ENEMY_COUNTER, 0)
    return w.g8(G_SECTOR_ORDER + w.g8(G_ENEMY_COUNTER))


def _time_off(w):
    if w.g8(G_TIME_COMPRESS):
        w.ext("time_compress", 0)


def enemy_move(w, src):
    """0x3EDE8 (byte src): result (byte) = enemies who came into a sector with mercs"""
    src &= 0xFF
    res = 0
    if _t(w, src, 0) & 1:
        return 0
    if not _t(w, src, 2):
        if src == 0:
            g = w.g32(G_SANTINO)
            g = g - (1 << 32) if g & 0x80000000 else g
            keep = (8 - g) & 0xFF if g < 8 else 0
        else:
            keep = 3 if not w.g8(G_SECTORS + 0xB1 * src + 0xAA) else 1
        keep = s8(keep)
        surplus = s8(_t(w, src, 1) - keep)
        if surplus > 0:
            v0 = s16(sector_value(w, src, 1))
            while True:
                best, last, pick = -100, -100, None
                for d in range(4):
                    if (d == 0 and src % 10 == 9 or d == 1 and src > 49 or d == 2 and src < 10
                            or d == 3 and src % 10 == 0):
                        continue
                    nb = s8((OFFSET[d] + src) & 0xFF)
                    room = s8(8 - _t(w, nb, 1))
                    if room < 1:
                        continue
                    f = 1 if not _t(w, src, 0) & WALL[d] or _story_cond(w, nb) else 0
                    sv = s16(sector_value(w, nb, f))
                    if sv == -100:
                        continue
                    val = s16(sv + BONUS[d])
                    cnt = room if room <= surplus else surplus
                    if _t(w, nb, 0) & 1 or _t(w, nb, 2):
                        if surplus < 2:
                            continue
                        cnt = min(cnt, 7)
                        de = s16(_t(w, nb, 2) * defend_merc(w) + _t(w, nb, 4) * defend_guard(w) + 2 * _t(w, nb, 3))
                        if de == 0:
                            n = s8(_share(w, _t(w, src, 1), val, v0))
                            if n < 2:
                                continue
                            cnt = min(cnt, n)
                        att = s16(attack_weight(w, src, f) * cnt + attack_weight(w, nb, 1) * _t(w, nb, 1))
                        if de + att == 0:       # x / 0: NaN for att = 0 (never taken), else an infinity
                            if att == 0 or (att > 0) != (v0 > 0):
                                continue
                            score = INF         # +infinity: taken, best = 0x80000000 (low word 0)
                        else:
                            score = _round(_mul(_div(att * 100, de + att), _div(val, v0)), 24)
                    else:
                        n = s8(_share(w, _t(w, nb, 1) + _t(w, src, 1), val, v0))
                        n = s8(min(n, 8) - _t(w, nb, 1))
                        if n < 1:
                            continue
                        cnt = min(cnt, n)
                        score = Fraction((cnt - 1) * 10 + 50)
                    if score is not INF and (score < 50 or score <= best):
                        continue
                    last, best = best, 0 if score is INF else s16(_chop(score))
                    pick = (nb, f, cnt, d)
                if best > -100:
                    nb, f, cnt, d = pick
                    moved = 0
                    for _ in range(cnt):
                        if not f:
                            if not (w.random(6) < w.random(s8(w.g8(G_SPLASH + src)))
                                    and w.random(6) < w.random(s8(w.g8(G_SPLASH + (nb & 0xFF))))):
                                if w.random(100) < 75:
                                    w.ext("replay", src, src, 0x54)
                                    continue
                        if take_enemy(w, src) and put_enemy(w, nb, EDGE[d]):
                            moved += 1
                    if s8(moved) > 0:
                        w.ext("replay", src, nb, s8(moved + 100))
                        if _t(w, nb, 2):
                            _time_off(w)
                        if not _t(w, nb, 0) & 1 and not _t(w, nb, 2):
                            w.ext("replay", src, nb, 0x4B)
                        else:
                            w.ext("replay", src, nb, 0x4C)
                            if not _t(w, nb, 2):
                                w.ext("resolve", nb & 0xFF, f)
                            else:
                                res = (res + moved) & 0xFF
                    surplus = s8(surplus - cnt)
                if not last > -100 or not surplus > keep:
                    break
    if src == 0:
        n = s8(8 - _t(w, 0, 1))
        if _t(w, 0, 2) and n > 2:
            n = 2
        g = w.g32(G_SANTINO)
        g = g - (1 << 32) if g & 0x80000000 else g
        if n > g:
            n = s8(g & 0xFF)
        if n:
            for _ in range(n if n > 0 else 0):
                if put_enemy(w, 0, 9):
                    w.s32(G_SANTINO, (w.g32(G_SANTINO) - 1) & 0xFFFFFFFF)
            if _t(w, 0, 2):
                _time_off(w)
                res = (res + n) & 0xFF
            w.ext("replay", 0, 0, s8(n + 100))
            w.ext("replay", 0, 0, 0x57)
    return res
