"""Enemy / guard AI ported from JA.EXE (reference implementation, diff-tested by validate/diff/ai_diff.py).

Per soldier and tick, AiThink 0x6D1A8 (think) updates the alert state (UpdateAlertState 0x6D574), lets the state's decider
pick an action (ChooseAiAction 0x6D7C0 -> CalmAction 0x6DB10, AlertAction 0x6DEE8, CombatAction 0x6E3F4,
State3Action 0x6EB54), checks the AP (AiActionAffordable 0x6D864) and starts it (AiDoAction 0x6D904).

Soldier bytes used (s = bytearray of the 0x200-byte record):
  +0xBA aiType   0..2 / 3..4 (2 = never acts, 4 = Santino; 1 and 3 have extra rules)
  +0xBB alert    0 calm, 1 alerted (noise / sighting), 2 combat, 3 (+0x1D5 > 0)
  +0x131 b131    bonus to the calm move rolls (AlertAction sets 15, CombatAction 30 before falling back to calm)
  +0x132 target  tile or direction (1..8) of the chosen action, 0xC41 = none
  +0x1A1 facing  current direction 1..8;  +0x1CF postFacing (direction to face when idle, 1..8)
  +0x1A8, +0x1AB, +0x1C1 (re-think), +0x1C4, +0x1D3 attitude / +0x1D4 orders (0..5 / 0..4), +0x1D6, +0x1D7
  +0x18A energy, +0x183/+0x184 hp/max, +0x1A5 apLeft.
Callees not ported here go through w.ext(name, s, ...) (the diff test scripts their results).
"""
NO_TILE = 0xC41
G_ALARM = 0x22A0CE           # u8: an enemy raised the alarm (action 7)
G_COMBAT = 0x92DCB           # u8: enemies near (turn-based combat)
G_CUR_SECTOR = 0x22A039
G_SECTOR_TBL9 = 0x22979C     # 9 B per sector, b1 = enemies alive

A_NONE, A_TURN, A_MOVE_DRY, A_MOVE3, A_MOVE4, A_MOVE5, A_SHOUT, A_ALARM = 0, 1, 2, 3, 4, 5, 6, 7


def s8(b):
    return b - 256 if b & 0x80 else b


def s16(v):
    v &= 0xFFFF
    return v - 0x10000 if v & 0x8000 else v


def cdiv(a, b):
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def u16(s, o):
    return s[o] | s[o + 1] << 8


def w16(s, o, v):
    s[o], s[o + 1] = v & 0xFF, (v >> 8) & 0xFF


def in_water(s):
    """InWater 0x23A34: ground under the soldier (+0x1B2) >= 0x6C and the object there (+0x1B4) not 0x1C/0x1F."""
    return s[0x1B2] >= 0x6C and s[0x1B4] not in (0x1C, 0x1F)


def update_alert(s, w):
    """UpdateAlertState 0x6D574: new alert state (+0xBB) from the AI type, then re-think / orders updates."""
    old = s8(s[0xBB])
    typ = s[0xBA]
    if typ <= 2:
        if s8(s[0x1D5]) > 0:
            s[0xBB] = 3
        elif w.g8(G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR) + 1):
            s[0xBB] = 2
        else:
            s[0xBB] = 0
            if old >= 2 and s8(s[0xBA]) == 1:
                s[0x1D4] = 3 if u16(s, 0x12E) == NO_TILE else 1
                s[0x131] = 0
                s[0x1D7] = 0
                w.ext("idle", s)
    elif typ <= 4:
        if s8(s[0x1D5]) > 0:
            s[0xBB] = 3
        elif s[0xBB] == 3:
            s[0xBB] = 2
        elif s[0xBB] == 0:
            if w.g8(G_ALARM) or s[0x1C4]:
                s[0xBB] = 2
            elif s16(w.ext("noise_tile", s)[0]) != NO_TILE:
                s[0xBB] = 1
        elif s[0xBB] == 1:
            if w.g8(G_ALARM) or s[0x1C4]:
                s[0xBB] = 2
            elif s16(w.ext("noise_tile", s)[0]) == NO_TILE and not s[0x1D6]:
                s[0xBB] = 0
    new = s[0xBB]
    if old == s8(new):
        if (new == 0 and s8(s[0x18A]) < 75) or (s8(new) == 1 and s8(s[0x18A]) < 50):
            if not in_water(s):
                s[0x1C1] = 1
    else:
        if old < 2 or s8(new) < 2:
            s[0x1C1] = 1
        if old < 2 and s8(new) >= 2 and s[0x1D4] < 2:
            if not (s8(s[0xBA]) == 3 and s[0x1D4] == 0 and s8(s[0x18F]) == 99):
                s[0x1D4] = (s[0x1D4] + 1) & 0xFF


G_DAY = 0x229FA4              # i32 read (u16 day number + the next word)
G_HOME_DAY = 0x95C36          # s16 per difficulty: home (0x3B) is not attacked up to this day: 6 / 4 / 2
G_HOT_SECTORS = 0x95BE8       # u8[5]: sectors 0x1C 0x1D 0x26 0x30 0x31 get +15
G_PLANTS = 0x22A12A           # 4 x 12 B {i32 capacity, i32 sector, ...}
G_CRATE_DROPS = 0x95CC0       # 4 x {u8 sector, u8 day, ...}
HOME = 0x3B


def _story_cond(w, sector):
    """0x3ED3C (inlined the same way in SectorValue): bytes 0x22A049 == 2 and 0x22A046 == 2 and 0x22A04B < 1,
    for sectors 0x11 and 0x1B"""
    return (s8(w.g8(0x22A049)) == 2 and s8(w.g8(0x22A046)) == 2 and s8(w.g8(0x22A04B)) < 1
            and sector & 0xFF in (0x11, 0x1B))


def sector_value(w, sector, flag):
    """SectorValue 0x3E03C: how much the enemy wants sector `sector` (20..100, or -100 never / 250 / 1000).
    -100: sector byte 8 != 0; home before day G_HOME_DAY[difficulty] + 1; 0x1B owned unless the story condition
    (0x3ED3C); 0x0B when 0x633D8(0x1C9) and mercs are there. 1000: 0x35 while byte 0x22A04D == 1; the story
    condition gives 1000 for 0x1B, 250 for 0x11. Else 30 + trees x 5 + workable x 2 + tappers / 3 x 3 + 5 if
    connected to home (0x41178) + 1 per open side (flag bits 2/4/8/0x10 clear) + 15 for G_HOT_SECTORS + 20 for 5 and
    0x15 + 25 for 0x0B (as above) + 25 per plant in it + 25 if a crate lands there tomorrow + 50 for byte 0x22A057
    while 0x22A04B == 1 and 0x22A055 == 0x22A056 == 0 + day for home + 100 for sector 0 with mercs; -25 without
    `flag`; clamped to 20..100."""
    t = G_SECTOR_TBL9 + 9 * sector
    day = _i32(w.g32(G_DAY))
    if w.g8(t + 8):
        return -100
    if sector == HOME and s16(w.g16(G_HOME_DAY + 2 * s8(w.g8(G_DIFFICULTY)))) >= day:
        return -100
    if sector == 0x1B and w.g8(t) & 1 and not _story_cond(w, sector):
        return -100
    if sector == 0x35 and s8(w.g8(0x22A04D)) == 1:
        return 1000
    if _story_cond(w, sector):
        return 1000 if sector == 0x1B else 250
    v = 30 + w.g8(t + 6) * 5 + w.g8(t + 7) * 2 + w.g8(t + 3) // 3 * 3
    v += (_i32(w.ext("connected_home", G_SECTOR_TBL9, sector)) & 0xFF) * 5   # the table address is not logged
    v += sum(1 for bit in (2, 4, 8, 0x10) if not w.g8(t) & bit)
    if any(s8(w.g8(G_HOT_SECTORS + k)) == sector for k in range(5)):
        v += 15
    if sector == 5:
        v += 20
    if sector == 0xB and _i32(w.ext("quest_check", 0x1C9)) & 0xFF:
        if w.g8(t + 2):
            return -100
        v += 25
    v += 25 * sum(1 for k in range(4) if _i32(w.g32(G_PLANTS + 4 + 12 * k)) == sector)
    if any(s8(w.g8(G_CRATE_DROPS + 4 * k)) == sector and day + 1 == s8(w.g8(G_CRATE_DROPS + 4 * k + 1))
           for k in range(4)):
        v += 25
    if s8(w.g8(0x22A04B)) == 1 and sector == s8(w.g8(0x22A057)) and not w.g8(0x22A056) and not w.g8(0x22A055):
        v += 50
    if sector == HOME:
        v = _i32(v + day)
    if sector == 0x15:
        v += 20
    if sector == 0 and w.g8(t + 2):
        v += 100
    if not flag & 0xFF:
        v = _i32(v - 25)
    return 100 if v > 100 else 20 if v < 20 else v


ANIM_TURNS = 0x93168         # u8 per animation x 8 (+2 next to ANIM_CAN_ACT): != 0 = the animation steps on (0x1F2C4)


def clear_target(s, w, flag):
    """0x56ED0: forget the attack: attackSlot +0x1D8 and +0x1C2 = 0x40 (none), +0x1BE = 0, lastTargetTile +0xD0 and
    +0xDC = 0xC41; with `flag`, if the current animation has ANIM_TURNS set, step the animation (0x1F2C4)."""
    s[0x1D8] = s[0x1C2] = 0x40
    s[0x1BE] = 0
    w16(s, 0xD0, NO_TILE)
    w16(s, 0xDC, NO_TILE)
    if flag & 0xFF and w.g8(ANIM_TURNS + 8 * u16(s, 0x1E8 + 2 * s[0x1F3])):
        w.ext("anim_step", s)


def idle(s, w):
    """AiIdle? 0x5AFA4 (CalmAction's idle ticks, UpdateAlertState calming down): clear_target(s, 1) if +0x1FE != 0."""
    if s[0x1FE]:
        clear_target(s, w, 1)


def _hurt(s):
    """(max hp - hp) + (100 - energy), signed bytes"""
    return (s8(s[0x184]) - s8(s[0x183])) + (100 - s8(s[0x18A]))


def calm_action(s, w):
    """CalmAction 0x6DB10 (alert 0): returns the action, target in +0x132.
    Out of combat 9 in 10 ticks do nothing (idle). In water or on a 0x80-flagged tile (gas?) without item 0x28
    (gas mask?) in slot 3: walk to the nearest dry / clear tile (2). Then with energy >= 75: roll a patrol walk (2),
    a walk to a second kind of spot (3), or turning to a new direction (1), weighted by orders and attitude."""
    water = in_water(s)
    gas = bool(w.tile8(u16(s, 0x114), 5) & 0x80) and s[0x24] != 0x28
    if not (w.g8(G_COMBAT) or s[0x1AB] or s[0x1D7] or water or gas) and w.random(10) != 0:
        w.ext("idle", s)
        w16(s, 0x132, NO_TILE)
        return A_NONE
    if water or gas:
        w16(s, 0x132, w.ext("find_dry_tile", s))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE_DRY
    if not (s8(s[0x18A]) >= 75 or s[0x1D7] or water):
        w16(s, 0x132, NO_TILE)
        return A_NONE
    v = s[0x131] + 25 + {0: -20, 1: 15, 2: 25, 4: -10}.get(s[0x1D4], 0)
    v += {0: -10, 1: 5, 3: 5, 5: 10}.get(s[0x1D3], 0)
    v = s16(v - _hurt(s))
    if s[0x1AB] or s[0x1D7] or water or w.random(100) < v:
        w16(s, 0x132, w.ext("patrol_tile", s))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            s[0x1D7] = 0
            return A_MOVE_DRY
    v = s[0x131] + 25 + {0: -20, 1: 10, 2: 20, 4: -10}.get(s[0x1D4], 0)
    if s[0x1D3] in (1, 3):
        v = cdiv(s16(v), 2)
    elif s[0x1D3] in (2, 4):
        v += 10
    v = s16(v - _hurt(s))
    if w.random(100) < v and w.ext("buddy_tile", s) & 0xFF:
        s[0x1A8] = 0
        return A_MOVE3
    v = s[0x131] + 25 + (25 if s[0x1D4] == 0 else 0) + (25 if s[0x1D3] == 0 else 0)
    if w.random(100) < s16(v):
        while True:
            post = s8(s[0x1CF])
            if 1 <= post <= 8 and s[0x1A1] != s[0x1CF] and w.random(2) != 0:
                d = post
            else:
                d = w.random(8) + 1
            w16(s, 0x132, d)
            if s8(s[0x1A1]) != s16(d):
                return A_TURN
    w16(s, 0x132, NO_TILE)
    return A_NONE


def tile_x(t):
    """TileToX 0x15E50"""
    return (t - cdiv(t, 56) * 56) * 16 + 5


def tile_y(t):
    """TileToY 0x15E6C"""
    return cdiv(t, 56) * 14 + 5


def _toward(s, w, tile):
    """Direction 0x52560 from the soldier to tile (1..8)."""
    me = s16(u16(s, 0x114))
    return w.ext("direction", tile_x(me), tile_y(me), tile_x(tile), tile_y(tile)) & 0xFF


def alert_action(s, w):
    """AlertAction 0x6DEE8 (alert 1): only AI type 3 reacts; the others do nothing (and keep +0x132).
    noise_tile 0x7038C gives the heard tile and its loudness. In order, each by a roll weighted by orders and attitude:
    turn toward the noise (1, within 14 tiles), shout for help (6: enemies with >= 5 AP and allies near, by the
    sector's strength 0x3E03C), with energy >= 50 (or in water) walk toward the noise (2, chance = loudness + 75 +
    orders/attitude shifts - tiredness) or toward the last sighting 0x702A8 (3), search 0x70850 (5), else half the time behave
    calm with +15 (CalmAction)."""
    if s8(s[0xBA]) != 3:
        return A_NONE
    noise, loud = w.ext("noise_tile", s)
    noise = s16(noise)
    if noise == NO_TILE:
        return A_NONE
    d = _toward(s, w, noise)
    if s8(s[0x1A1]) != d and w.ext("tile_dist", s16(u16(s, 0x114)), noise) <= 14:
        c = (60 if s[0x1D4] == 0 else 35) + (15 if s[0x1D3] == 0 else 0)
        if w.random(100) < c:
            w16(s, 0x132, d)
            return A_TURN
    if s[0x17A] >= 0x28 and s8(s[0x1A5]) >= 5:
        c = (w.ext("news_value", s) & 0xFF) * 5
        if c and s[0x1B2] < 0x9A:
            st = s16(w.ext("sector_value", w.g8(G_CUR_SECTOR), 1))
            st = 0 if st == -100 else min(st, 100)
            c += cdiv(st, 2)
            c += {0: 20, 2: -10, 3: 10, 4: -20}.get(s[0x1D4], 0)
            c += {0: 20, 1: -10, 3: -5, 5: -20}.get(s[0x1D3], 0)
            if w.random(100) < s16(c):
                return A_SHOUT
    if not (s8(s[0x18A]) >= 50 or in_water(s)):
        w16(s, 0x132, NO_TILE)
        return A_NONE
    tired = 100 - s8(s[0x18A])
    c = loud + 75 + {0: -20, 1: -10, 2: 10, 4: 25}.get(s[0x1D4], 0)
    c += {0: -10, 1: 10, 2: 5, 3: 5, 5: 20}.get(s[0x1D3], 0)
    if w.random(100) < s16(c - tired):
        w16(s, 0x132, w.ext("approach_tile", s, noise))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE_DRY
    seen = s16(w.ext("ally_call_tile", s))
    if seen != NO_TILE:
        c = 50 - w.ext("tile_dist", s16(u16(s, 0x114)), seen)
        c += {0: -20, 1: -10, 3: 20, 4: 10}.get(s[0x1D4], 0)
        c += {0: -10, 2: 20, 4: 20, 5: -20}.get(s[0x1D3], 0)
        if w.random(100) < s16(c - tired):
            w16(s, 0x132, w.ext("approach_tile", s, seen))
            if s16(u16(s, 0x132)) != NO_TILE:
                s[0x1A8] = 0
                return A_MOVE3
    c = {0: 45, 1: 35, 2: 20, 4: 5}.get(s[0x1D4], 25)
    c += {0: 10, 1: -15, 2: -20, 3: 20, 4: 15, 5: -10}.get(s[0x1D3], 0)
    if w.random(100) < s16(c - tired):
        w16(s, 0x132, w.ext("search_tile", s, 0))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE5
    if w.random(100) < 50:
        s[0x131] = 15
        return calm_action(s, w)
    w16(s, 0x132, NO_TILE)
    return A_NONE


A_MOVE_OBJ, A_RADIO, A_MOVE_CALL, A_OPERATE, A_USE_OBJ = 8, 10, 13, 9, 16
G_ITEM_KIND = 0x94DFA        # g_itemTable + 2: kind per item type (12 B each), 0x92 gun, 0x96 knife
G_DIFFICULTY = 0x22A0CC
G_ALARM_CHANCE = 0x95C5A     # i16[3] per difficulty: 50 / 80 / 95 % to raise the alarm while none is raised
G_MISSION = 0x22A0D0         # i8 scripted mission step (> 0 active; 1 = use the item on the target)
G_MISSION_DONE = 0x22A1FB    # u8 per sector
G_MISSION_ORDER = 0x92DA8    # u8 slot ordered to the target, +2 u16 target tile
G_MISSION_STEP = 0x22A0D0     # u8 g_missionStep
G_MISSION_SPOT = 0x92E50     # u16 tile to stand on for it
ITEM_RADIO = 0x40


def quest_check(w, quest):
    """QuestCheck? 0x633D8 (AX = quest id) -> 1 for 0x1C9 in sector 0x0B while story byte 0x22A04E = 1 (s8; the asm reads dword 0x22A04B >> 24), the safe
    0x22A109 is closed, story bytes 0x22A05C and 0x22A05E are 0, and no merc carries item 0x6E or 0x7C
    (FindSquadItem 0x61528, all mercs, no drop, no probe); else 0."""
    if (w.g8(G_CUR_SECTOR) != 0xB or s16(quest & 0xFFFF) != 0x1C9 or s8(w.g8(0x22A04E)) != 1
            or w.g8(G_SAFE_OPENED) or w.g8(0x22A05C) or w.g8(0x22A05E)):
        return 0
    if find_squad_item(w, 0x6E, 0, 0x40, 0) != -1 or find_squad_item(w, 0x7C, 0, 0x40, 0) != -1:
        return 0
    return 1


def radio_result(w):
    """RadioResult? 0x63254 -> the new mission step (byte). Mission target (0x92DAA) = the first tile 0..0xC3F whose
    ground item (byte 6 < 0xFF) is type 0x72, else NO_TILE. Then: story byte 0x22A04B = 1 (s8), cur_sector = s8
    0x22A057 and bytes 0x22A056, 0x22A055 = 0: the first story slot k with S[0x9F + k] = 0xAE puts the mission
    target on its tile u16 S[0x82 + 2k] -> 1. Sector 0x0B: quest_check(0x1C9) -> 3, else 0. Other sectors: plants
    1..3 (NOT plant 0) whose i32 sector = cur: s8 byte +8 = 3 -> 2, else 0. Mission done for cur -> 0. A mission
    target -> 4; an enemy 0x28..0x2F (the soldier pointer 0x1009F8 is slot 0x28's) in the sector, with health,
    +0x188 = 0 and a radio (0x40) in item 1 or item 0 -> 4."""
    w.s16(G_MISSION_ORDER + 2, NO_TILE)
    for t in range(0xC40):
        g = w.tile8(t, 6)
        if g < 0xFF and w.g8(G_GROUND_ITEMS + 12 * g) == 0x72:
            w.s16(G_MISSION_ORDER + 2, t)
            break
    cur = w.g8(G_CUR_SECTOR)
    if (s8(w.g8(0x22A04B)) == 1 and cur == s8(w.g8(0x22A057)) and not w.g8(0x22A056) and not w.g8(0x22A055)):
        sec = G_SECTORS + 0xB1 * cur
        for k in range(5):
            if w.g8(sec + 0x9F + k) == 0xAE:
                w.s16(G_MISSION_ORDER + 2, w.g16(sec + 0x82 + 2 * k))
                return 1
    if cur == 0xB:
        return 3 if _i32(w.ext("quest_check", 0x1C9)) & 0xFF else 0
    for k in range(1, 4):
        if w.g32(G_PLANTS + 12 * k + 4) == cur:
            return 2 if s8(w.g8(G_PLANTS + 12 * k + 8)) == 3 else 0
    if w.g8(G_MISSION_DONE + cur):
        return 0
    if s16(w.g16(G_MISSION_ORDER + 2)) != NO_TILE:
        return 4
    for k in range(0x28, 0x30):
        r = G_SOLDIERS + 0x200 * k
        if w.g8(r + 0x182) and w.g8(r + 0x183) and not w.g8(r + 0x188) and ITEM_RADIO in (w.g8(r + 0xC), w.g8(r)):
            return 4
    return 0


def _wrap8(v):
    return s8(v & 0xFF)


def combat_action(s, w):
    """CombatAction 0x6E3F4 (alert 2). afford = AP left >= move_ap 0x1B118. Scripted mission first (enemy slots,
    [0x22A0D0] > 0): a radio carrier calls in (10); the soldier ordered in 0x92DA8 walks to its spot (13), then
    operates the target (9) or, at step 1, uses the gun / knife on it (16). Then: tired (energy < 25, not in water,
    no +0x1C4) -> nothing; strength = opp_strength 0x6F314 (0 = we are much stronger?): at 0 walk to an objective
    (8); raise the alarm (7) by the difficulty table, or once raised by allies near x 10 + orders/attitude shifts, scaled by the
    strength; then three weights (advance 4, approach the last sighting 3, attack/search 5) set by strength and
    orders/attitude shifts are tried highest first; else turn toward the target (1), else calm behaviour with +30."""
    if s[0x1A5] == 0:
        w16(s, 0x132, NO_TILE)
        return A_NONE
    ap = s8(s[0x1A5])
    afford = ap >= _i32(w.ext("move_ap", s))
    if s[0x17A] >= 0x28 and s8(w.g8(G_MISSION)) > 0:
        if w.g8(G_MISSION_DONE + w.g8(G_CUR_SECTOR)) == 0 and ITEM_RADIO in (s[0x0C], s[0x00]):
            if ap >= 2:
                return A_RADIO
            w16(s, 0x132, NO_TILE)
            return A_NONE
        target = s16(w.g16(G_MISSION_ORDER + 2))
        if w.g8(G_MISSION_ORDER) == s[0x17A] and target != NO_TILE:
            saved, s[0x9C] = s[0x9C], 0xFF
            r = w.ext("reach_ap", s, target, 0)
            s[0x9C] = saved
            ok = False
            if r:
                spot = w.g16(G_MISSION_SPOT)
                if spot == u16(s, 0x114):
                    if s8(w.g8(G_MISSION)) != 1:
                        if ap >= 2:
                            w16(s, 0x132, target)
                            return A_OPERATE
                    else:
                        c = s8(w.ext("attack_ready", s, target) & 0xFF)
                        if c in (-6, -5):
                            w.ext("swap_hand", s, 0, 6)
                            c = s8(w.ext("attack_ready", s, target) & 0xFF)
                        if c == 1:
                            k0, k1 = w.g8(G_ITEM_KIND + 12 * s[0]), w.g8(G_ITEM_KIND + 12 * s[0x0C])
                            if k1 == 0x96 or k0 not in (0x92, 0x96):
                                w.ext("swap_hand", s, 0, 6)
                            if w.g8(G_ITEM_KIND + 12 * s[0]) in (0x92, 0x96) and \
                                    _i32(w.ext("action_ap", s, target, 1)) <= ap:
                                w16(s, 0x132, target)
                                return A_USE_OBJ
                    w16(s, 0x132, NO_TILE)
                    return A_NONE
                if not afford:
                    w16(s, 0x132, NO_TILE)
                    return A_NONE
                if w.ext("can_stand_at", s, s16(spot)) & 0xFF:
                    w16(s, 0x132, spot)
                    return A_MOVE_CALL
            w.s8(G_MISSION_ORDER, 0x40)
            w.ext("mission_cancel")
    if s8(s[0x18A]) < 25 and not in_water(s) and not s[0x1C4]:
        w16(s, 0x132, NO_TILE)
        return A_NONE
    st = w.ext("opp_strength", s) & 0xFF
    if st == 0 and afford:
        w16(s, 0x132, w.ext("retreat_tile", s))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = st
            return A_MOVE_OBJ
    if s[0x17A] >= 0x28 and ap >= 5:
        c = s16((w.ext("news_value", s) & 0xFF) * 10)
        if c and s[0x1B2] < 0x9A:
            if not w.g8(G_ALARM):
                c = s16(w.g16(G_ALARM_CHANCE + 2 * s8(w.g8(G_DIFFICULTY))))
            else:
                c += {0: 20, 2: -10, 3: 10, 4: -20}.get(s[0x1D4], 0)
                c += {0: 20, 1: -10, 3: -5, 5: -20}.get(s[0x1D3], 0)
                c = s16(c)
                if st == 0:
                    c = s16(c * 3)
                elif st == 1:
                    c = s16(c * 2)
                elif st == 3:
                    c = cdiv(c, 2)
                elif st == 4:
                    c = cdiv(c, 3)
            if w.random(100) < c:
                return A_ALARM
    if afford:
        adv, app, att = {0: (-99, -99, 1), 1: (-1, 0, 1), 3: (1, 0, -1), 4: (1, 0, -1)}.get(st, (0, 0, 0))
        adv, app, att = [_wrap8(v + d) for v, d in zip((adv, app, att), {
            0: (-1, -1, 1), 3: (0, 1, -1), 4: (1, 0, -1)}.get(s[0x1D4], (0, 0, 0)))]
        adv, app, att = [_wrap8(v + d) for v, d in zip((adv, app, att), {
            0: (-1, 0, 1), 1: (1, -1, -1), 2: (1, 1, -1), 3: (0, -1, 1), 4: (0, 1, 1), 5: (1, 0, -1)}.get(
            s[0x1D3], (0, 0, 0)))]
        while not (adv <= -90 and app <= -90 and att <= -90):
            if adv > -90 and adv >= app and adv >= att:
                t = s16(w.ext("advance_tile", s, 1))
                if t != NO_TILE:
                    w16(s, 0x132, w.ext("approach_tile", s, t))
                    if s16(u16(s, 0x132)) != NO_TILE:
                        s[0x1A8] = 0
                        return A_MOVE4
                adv = -99
            if app > -90 and adv <= app and att <= app:
                t = s16(w.ext("ally_call_tile", s))
                if t != NO_TILE:
                    w16(s, 0x132, w.ext("approach_tile", s, t))
                    if s16(u16(s, 0x132)) != NO_TILE:
                        s[0x1A8] = 0
                        return A_MOVE3
                app = -99
            if att > -90 and att >= adv and att >= app:
                t = s16(w.ext("target_tile", s))
                if t != NO_TILE:
                    w16(s, 0x132, w.ext("search_tile", s, 1 if st < 2 else 2 if st in (3, 4) else 0))
                    if s16(u16(s, 0x132)) != NO_TILE:
                        s[0x1A8] = 0
                        return A_MOVE5
                att = -99
    t = s16(w.ext("target_tile", s))
    if t != NO_TILE:
        d = _toward(s, w, t)
        if s8(s[0x1A1]) != d and w.ext("tile_dist", s16(u16(s, 0x114)), t) <= 14:
            c = (50 if s[0x1D4] == 0 else 25) + (25 if s[0x1D3] == 0 else 0)
            if w.random(100) < c:
                w16(s, 0x132, d)
                return A_TURN
    if afford:
        s[0x131] = 30
        return calm_action(s, w)
    w16(s, 0x132, NO_TILE)
    return A_NONE


def _i32(v):
    v &= 0xFFFFFFFF
    return v - (1 << 32) if v & 0x80000000 else v


A_SHOOT, A_THROW, A_STAB = 11, 12, 14
G_SOLDIERS = 0xFA958
G_TILEMAP = 0x100A18


class Undefined(LookupError):
    """the original reads undefined data here (stack garbage): the diff tests skip such a case. A plain IndexError or
    KeyError is a mirror bug, never a skip."""
ATTACK_MIN_CHANCE = {0: 75, 1: 10, 2: 25, 3: 25, 4: 50}   # by opp_strength (> 4: uninitialised in the original)


def _record(b):
    """attack option record (gun 0x71590 / throw 0x717F8 / knife 0x71DD4): valid, target slot, aim clicks,
    hit chance %, i32 score, u16 target tile"""
    return {"valid": b[0], "slot": b[1], "aim": b[2], "chance": b[3],
            "score": _i32(b[4] | b[5] << 8 | b[6] << 16 | b[7] << 24), "tile": b[8] | b[9] << 8}


def _hand_of(s, w, kind):
    if w.g8(G_ITEM_KIND + 12 * s[0]) == kind:
        return 0
    return 1 if w.g8(G_ITEM_KIND + 12 * s[0x0C]) == kind else -1


def engaged_action(s, w):
    """EngagedAction 0x6EB54 (alert 3: +0x1D5 > 0, enemies in view): pick an attack. The soldier ordered by the
    mission (0x92DA8) uses CombatAction. In gas or exhausted (energy < 5): head for the objective (8). If the
    hands can attack (attack_ready 0x584A0 == 1, not in gas) each weapon kind is scored: gun (0x92, in hand; a target
    below 15 hp makes a soldier with attitude (+0x1D3) not 5 press on with CombatAction if it can advance), thrown (0x95) and
    knife (0x96, swapped into the hand for the evaluation); gun vs throw, else gun vs knife, by score. The pick is
    taken if its hit chance reaches 75/10/25/25/50 % (by opp_strength); otherwise move first (objective 8 /
    search 5) if affordable, else attack anyway; else leave water/gas (2, 8), else turn toward the target (1)."""
    if s[0x1A5] == 0:
        w16(s, 0x132, NO_TILE)
        return A_NONE
    if w.g8(G_MISSION_ORDER) == s[0x17A]:
        return combat_action(s, w)
    ap = s8(s[0x1A5])
    water = in_water(s)
    gas = bool(w.tile8(u16(s, 0x114), 5) & 0x80) and s[0x24] != 0x28
    st = w.ext("opp_strength", s) & 0xFF
    afford = ap >= _i32(w.ext("move_ap", s))
    if gas or s8(s[0x18A]) < 5:
        if afford:
            w16(s, 0x132, w.ext("retreat_tile", s))
            if s16(u16(s, 0x132)) != NO_TILE:
                s[0x1A8] = 0
                return A_MOVE_OBJ
        st = 0
    c = s8(w.ext("attack_ready", s, NO_TILE) & 0xFF)
    if c in (-6, -5):
        w.ext("swap_hand", s, 0, 6)
        c = s8(w.ext("attack_ready", s, NO_TILE) & 0xFF)
    if gas:
        c = 0
    last = s16(u16(s, 0xD0))
    gun = thrown = knife = {"valid": 0}
    if c == 1:
        if w.g8(G_ITEM_KIND + 12 * s[0]) != 0x92:
            w.ext("swap_hand", s, 0, 6)
        if w.g8(G_ITEM_KIND + 12 * s[0]) != 0x92:
            w16(s, 0x132, NO_TILE)
            return A_NONE
        if ap >= w.ext("action_ap", s, last, 0) & 0xFF:
            gun = _record(w.ext("gun_option", s))
            if s8(w.g8(G_SOLDIERS + 0x200 * gun["slot"] + 0x183)) < 15 and s[0x1D3] != 5:
                if s16(w.ext("advance_tile", s, 0)) != NO_TILE:
                    return combat_action(s, w)
    hands = {}
    for kind, name in ((0x95, "throw_option"), (0x96, "knife_option")):
        if c != 1:
            continue
        hands[kind] = h = _hand_of(s, w, kind)
        if h > -1:
            if h == 1:
                w.ext("swap_hand", s, 0, 6)
            if ap >= w.ext("action_ap", s, last, 0) & 0xFF:
                rec = _record(w.ext(name, s))
                if kind == 0x95:
                    thrown = rec
                else:
                    knife = rec
            if h == 1:
                w.ext("swap_hand", s, 0, 6)
    act, sel = A_NONE, None
    if c == 1:
        if gun["valid"] and thrown["valid"]:
            act = A_THROW if gun["score"] < thrown["score"] else A_SHOOT
        elif gun["valid"] and knife["valid"]:
            act = A_STAB if gun["score"] < knife["score"] else A_SHOOT
        elif gun["valid"]:
            act = A_SHOOT
        elif thrown["valid"]:
            act = A_THROW
        elif knife["valid"]:
            act = A_STAB
        if act == A_SHOOT:
            sel = gun
        elif act == A_THROW:
            sel = thrown
            if hands[0x95] == 1:
                w.ext("swap_hand", s, 0, 6)
        elif act == A_STAB:
            sel = knife
            if hands[0x96] == 1:
                w.ext("swap_hand", s, 0, 6)

    def attack():
        w16(s, 0x132, sel["tile"])
        s[0x1FD] = sel["aim"]
        s[0x1D8] = sel["slot"]
        return act

    if c and act and sel["chance"] >= ATTACK_MIN_CHANCE[st]:
        return attack()
    if afford:
        if st == 0:
            w16(s, 0x132, w.ext("retreat_tile", s))
            if s16(u16(s, 0x132)) != NO_TILE:
                s[0x1A8] = 0
                return A_MOVE_OBJ
        arg = 0 if water else 1 if st < 2 else 2 if st in (3, 4) else 0
        w16(s, 0x132, w.ext("search_tile", s, arg))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE5
    if c and act:
        return attack()
    if (water or gas) and afford:
        w16(s, 0x132, w.ext("find_dry_tile", s))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE_DRY
        w16(s, 0x132, w.ext("retreat_tile", s))
        if s16(u16(s, 0x132)) != NO_TILE:
            s[0x1A8] = 0
            return A_MOVE_OBJ
        st = 0
    if st > 0 or afford:
        if not _i32(w.ext("busy", s)):
            t = s16(w.ext("target_tile", s))
            if t != NO_TILE:
                d = _toward(s, w, t)
                if s8(s[0x1A1]) != d:
                    w16(s, 0x132, d)
                    return A_TURN
    w16(s, 0x132, NO_TILE)
    return A_NONE


DECIDERS = None     # set below: alert state -> decider


def choose_action(s, w, edx=0):
    """ChooseAiAction 0x6D7C0 -> action code (+0x1D1). A pending calm bonus (+0x131) below alert 3 means calm
    behaviour; alert states above 3 return the caller's EDX unchanged (no decider)."""
    if s[0x131] and s8(s[0xBB]) < 3:
        return calm_action(s, w)
    fn = DECIDERS.get(s[0xBB])
    return fn(s, w) if fn else edx


def action_affordable(s, w):
    """AiActionAffordable 0x6D864: AP cost of the chosen action (+0x1D1) <= AP left (signed bytes)."""
    a = s[0x1D1]
    if a == 1:
        cost = 1
    elif a in (2, 3, 4, 5, 8, 13):
        cost = w.ext("move_ap", s) & 0xFF
    elif a in (6, 7):
        cost = 5
    elif a in (9, 10):
        cost = 2
    elif a in (11, 12, 14, 16):
        cost = w.ext("action_ap", s, s16(u16(s, 0x132)), 1) & 0xFF
    else:
        cost = 0
    return int(s8(cost) <= s8(s[0x1A5]))


def do_action(s, w):
    """AiDoAction 0x6D904: start the chosen action (+0x1D1) at +0x132. Returns 1 while it goes on (turn, walk
    that reached its tile, attacks), 0 when it is finished or failed (the action and +0x132 are then cleared)."""
    a = s[0x1D1]
    if a == 0:
        return 1
    if a == A_TURN:
        if w.ext("turn", s, s[0x132], 30):
            return 1
        s[0x1A5] = 0
        s[0x1C1] = 1
        s[0x1D1] = 0
        w16(s, 0x132, NO_TILE)
        return 0
    if a in (2, 3, 4, 5, 8, 13):
        w.ext("walk_to", s, s16(u16(s, 0x132)))
        if u16(s, 0x124) != u16(s, 0x132):
            s[0x1C1] = 1
            dest = u16(s, 0x132)
            w16(s, 0x132, NO_TILE)
            s[0x1D1] = 0
            w16(s, 0x10E, dest)
            return 0
        w16(s, 0x10E, NO_TILE)
        s[0x1AB] = 0
        return 1
    if a in (11, 12, 14, 16):
        w.ext("click", s, s16(u16(s, 0x132)), s[0])
        return 1
    if a == A_OPERATE:
        w.ext("click", s, s16(u16(s, 0x132)), 0)
    elif a == A_RADIO:
        w.ext("spend_ap", s, 2, 0)
        w.ext("wait", 1000)
        m = w.ext("radio_result") & 0xFF
        w.s8(G_MISSION, m)
        if s8(m) != 1:
            w.s8(G_MISSION_ORDER, 0x40)
    elif a in (A_SHOUT, A_ALARM):
        if a == A_ALARM and s[0x17A] >= 0x28 and not w.g8(G_ALARM):
            w.s8(G_ALARM, 1)
            if s8(w.g8(G_MISSION)) > 0 and s16(w.g16(G_MISSION_ORDER + 2)) != NO_TILE:
                w.ext("mission_cancel")
        w.ext("spend_ap", s, 5, 0)
        w.ext("share_sightings", s, 0)
    else:
        return 0
    w16(s, 0x132, NO_TILE)
    s[0x1D1] = 0
    return 0


G_TURN_BASED = 0x92DAC      # i32 != 0: turn-based (AI acts every tick, timeouts expire at once)
G_BUSY = 0x92E38            # i32 > 0: something is animating, AI waits (real time)
G_ACTIVE = 0x284F28         # u8 slot whose action runs (< 0x40: AI waits, real time)
G_SANTINO_SPOKE = 0x22A0C2
G_SANTINO_TIMER = 0x22A484  # i16; Santino speaks once it reaches 20
G_SHOW_AI = 0x92DCD         # u8: redraw before each AI action (0x12CBC)
ANIM_CAN_ACT = 0x93166      # u8 per animation x 8: 0 = soldier can't act in this animation


def think(s, w):
    """AiThink 0x6D1A8: one AI tick for an enemy / guard (slot >= 8). +0x1B0 = 1 ends its turn.
    +0x1D6 = action still running, +0xF0 = ticks it has run (timeout 350: an attack is fired 0x57B34, anything
    else ends the turn), +0x120 = tile it walks to (+0x124 = reached)."""
    if s[0x17A] < 8:
        return
    if not w.g32(G_TURN_BASED):
        if _i32(w.g32(G_BUSY)) > 0 or w.g8(G_ACTIVE) < 0x40:
            return
    typ = s8(s[0xBA])
    if typ == 2:
        s[0x1B0] = 1
        return
    if typ == 4:
        if s8(s[0x9C]) > 0 and not w.g8(G_SANTINO_SPOKE):
            if s16(w.g16(G_SANTINO_TIMER)) < 20:
                return
            enemy_panel(w)
            w.ext("delay", 1500)
            w.ext("santino_speech")
            w.ext("delay", 300)
            w.s8(G_SANTINO_SPOKE, 1)
            w.s32(0x22A008, 0)
        s[0x1B0] = 1
        return
    anim = u16(s, 0x1E8 + 2 * s[0x1F3])
    if not s[0x182] or not s[0x138] or s8(s[0x183]) < 15 or (anim < 4 and s8(s[0x18A]) < 10):
        s[0x1B0] = 1
        return
    if s[0x197] or not w.g8(ANIM_CAN_ACT + 8 * anim):
        return
    if anim < 4 and s8(s[0x183]) >= 15 and s8(s[0x18A]) >= 10:
        if s8(s[0x1A5]) < 7:
            s[0x1B0] = 1
        return
    w.ext("perceive", s, 0)
    update_alert(s, w)
    if s[0x1C1]:
        s[0x1C1] = 0
        s[0x131] = 0
        if s[0x1D1]:
            w16(s, 0x120, NO_TILE)
            s[0x1B8] = 1
            s[0x1D1] = 0
            w16(s, 0x132, NO_TILE)
            s[0x1D6] = 0
            s[0x1A2] = s[0x1A1]
            w16(s, 0x124, u16(s, 0x114))
            w16(s, 0x126, u16(s, 0x116))
            w16(s, 0x128, u16(s, 0x118))
    if not s[0x1B8]:
        return
    if s[0x1D1] and s[0x1D6]:
        t = s16(u16(s, 0x120))
        if t != NO_TILE and u16(s, 0x124) != u16(s, 0x120):
            w.ext("walk_to", s, t)
            if u16(s, 0x124) != u16(s, 0x120):
                s[0x1C1] = 1
            return
        if s[0x1D1] == 1 and s8(s[0x1A2]) != s16(u16(s, 0x132)):
            s[0x1A2] = s[0x132]
            return
        if s[0x1D1] == A_STAB and _occupant(w, s16(u16(s, 0x132))) >= 0x40:
            w16(s, 0xF0, 350)
        if w.g32(G_TURN_BASED):
            w16(s, 0xF0, 350)
        w16(s, 0xF0, u16(s, 0xF0) + 1)
        if u16(s, 0xF0) < 350:
            return
        if s[0x1D1] in (11, 12, 14, 16):
            w.ext("fire", s[0x17A])
        else:
            s[0x1A5] = 0
            s[0x1B0] = 1
        w16(s, 0xF0, 0)
        s[0x1C1] = 1
        return
    w16(s, 0xF0, 0)
    if not s[0x1D1]:
        s[0x1D6] = 0
        s[0x1D1] = choose_action(s, w, 0) & 0xFF
        if not s[0x1D1]:
            s[0x1B0] = 1
            return
    if not action_affordable(s, w):
        s[0x1B0] = 1
        return
    enemy_panel(w)
    s[0x1D6] = do_action(s, w) & 0xFF


G_KNOWN = 0x10D860           # i8 per slot: what the other side knows of this soldier (knowledge level, see G_FRESHER)
G_REMEMBER_E = 0x10D890      # u8[8][0x28] per enemy: != 0 = remembers merc/guard j (0..15)
G_REMEMBER_M = 0x10D9D0      # u8[16][8] per merc/guard: != 0 = remembers enemy j (slot 0x28 + j)
G_FRESHER = 0x95DE7          # u8 [old * 8 + new] (levels -3..4, signed index): > 0 = new is fresher than old
#                              freshness 1 > 2 > -1 > 3 > -2 > 4 > -3 > 0 (1 seen now .. 4 seen long ago, -1..-3 heard)
KNOW_WEIGHT = (60, 70, 80, 50, 100, 90, 80, 70)   # 0x6D1A0 (code bytes): % of a soldier's power by knowledge -3..4
ARMOR_VALUE = ((0x3C, {0x2D: 20, 0x2E: 30, 0x2F: 40, 0x30: 60}),    # body armour entry
               (0x18, {0x23: 5, 0x24: 7, 0x25: 10, 0x26: 15}),      # helmet entry
               (0x48, {0x31: 20}))                                  # Shielded Vest in the vest-pocket entry


def armor_rating(s):
    """ArmorRating 0x55500: sum of armour value x condition / 100 over body armour, helmet and Shielded Vest."""
    r = 0
    for off, values in ARMOR_VALUE:
        v = values.get(s[off], 0)
        if v:
            r += cdiv(s8(s[off + 2]) * v, 100)
    return r


def _alive(p):
    return p[0x182] and p[0x138] and s8(p[0x183]) >= 15


def soldier_power(p, w, tile=NO_TILE, flag=0):
    """SoldierPower 0x6F8DC: fighting value of soldier record p (-999 = not in play): max AP + AP left / 2 + armour / 4
    + marksmanship / 5 - wounds / 10 - fatigue / 10, plus the main-hand weapon (item 1..14: 3 x type; 0x0F/0x5D +2,
    0x10 +5, other kind 0x95 +15); against `tile`: +1/10 if it is the last target, else +1/20 if facing it; times the
    level; below 15 hp divided by 19 - hp, else with `flag` scaled by the line-of-fire chance 0x580CC (%). Min 1."""
    if not p[0x182] or not p[0x138] or not p[0x183]:
        return -999
    from combat import max_ap
    v = (max_ap(p) + cdiv(s8(p[0x1A5]), 2) + cdiv(armor_rating(p), 4) + cdiv(s8(p[0x191]), 5)
         - cdiv(100 - s8(p[0x183]), 10) - cdiv(100 - s8(p[0x18A]), 10))
    t = p[0]
    if 1 <= t <= 14:
        v += 3 * t
    elif t in (0x5D, 0x0F):
        v += 2
    elif t == 0x10:
        v += 5
    elif w.g8(G_ITEM_KIND + 12 * t) == 0x95:
        v += 15
    tile = s16(tile)
    if tile != NO_TILE:
        d = 0
        if tile & 0xFFFF == u16(p, 0xD0):
            d = 10
        else:
            me = s16(u16(p, 0x114))
            if s8(p[0x1A1]) == _i32(w.ext("direction", tile_x(me), tile_y(me), tile_x(tile), tile_y(tile))):
                d = 20
        if d:
            v += cdiv(v, d)
    v *= s8(p[0x192])
    if s8(p[0x183]) < 15:
        if v > 0:
            v = cdiv(v, 19 - s8(p[0x183]))
    elif flag & 0xFF and tile != NO_TILE:
        v = cdiv(_i32(v * w.ext("fire_line", p[0x17A], tile, 1)), 100)
    return max(v, 1)


def opp_strength(s, w):
    """OppStrength 0x6F314: 0..4 how the soldier's side stands. Sums the power of every opponent in play (vs this
    soldier's tile) and of every ally (self at 100 %), each weighted by KNOW_WEIGHT of what is known about it: for
    an opponent the fresher of this soldier's sight flag and the side's G_KNOWN (both 0: remembered = level 0, weight
    50, else skipped); for an ally its G_KNOWN, replaced by each counted opponent's sight flag on it that is staler or
    equal (the original's argument order: a fresher one never replaces it). Ratio
    allies*100/opponents (500 if none) -> 0 (<= 25), 1 (<= 50), 2 (<= 150), 3 (<= 300), 4; then +1/-1 for the
    sector's strength (> 50 / < 35), attitude +0x1D3 (1, 2: +1; 0: -1), energy and hp (> 75 +1, < 40 -1, < 10/20 -1),
    +0x1C4 == 0 +1, enemies -2 once Santino has spoken. Clamped to 0..4."""
    slot = s[0x17A]
    if slot >= 0x28:
        opps, allies, row = range(0, 0x10), range(0x28, 0x30), G_REMEMBER_E + (slot - 0x28) * 0x28
    else:
        opps, allies, row = range(0x28, 0x30), range(0, 0x10), G_REMEMBER_M + slot * 8
    known = [s8(w.g8(G_KNOWN + k)) for k in allies]
    fresher = lambda old, new: w.g8(G_FRESHER + 8 * old + new)
    me = s16(u16(s, 0x114))
    opp_sum = ally_sum = 0
    for j, i in enumerate(opps):
        o = w.rec(i)
        if not _alive(o):
            continue
        a, b = s8(s[0x13A + j]), s8(w.g8(G_KNOWN + i))
        if a == 0 and b == 0:
            if not w.g8(row + j):
                continue
            lvl = 0
        else:
            lvl = a if fresher(b, a) or a == b else b
        opp_sum = s16(opp_sum + cdiv(KNOW_WEIGHT[lvl + 3] * s16(soldier_power(o, w, me)), 100))
        for k, ally in enumerate(allies):
            c = s8(o[0x13A + k])
            if c and _alive(w.rec(ally)) and (fresher(c, known[k]) or c == known[k]):
                known[k] = c
    for k, ally in enumerate(allies):
        p = w.rec(ally)
        if _alive(p):
            weight = 100 if ally == slot else KNOW_WEIGHT[known[k] + 3]
            ally_sum = s16(ally_sum + cdiv(weight * s16(soldier_power(p, w)), 100))
    r = 500 if opp_sum == 0 else s16(cdiv(ally_sum * 100, opp_sum))
    v = 0 if r <= 25 else 1 if r <= 50 else 2 if r <= 150 else 3 if r <= 300 else 4
    st = s16(w.ext("sector_value", w.g8(G_CUR_SECTOR), 1))
    v += 1 if st > 50 else -1 if st < 35 else 0
    v += 1 if s[0x1D3] in (1, 2) else -1 if s[0x1D3] == 0 else 0
    for val, low in ((s8(s[0x18A]), 10), (s8(s[0x183]), 20)):
        v += 1 if val > 75 else -(val < 40) - (val < low)
    v += not s[0x1C4]
    if w.g8(G_SANTINO_SPOKE) and slot >= 0x28:
        v -= 2
    return min(max(v, 0), 4)


G_LAST_SEEN_E = 0x10DA90     # u16[8][0x28] per enemy: tile where it last knew opponent j (slot j) to be
G_LAST_SEEN_M = 0x10E190     # u16[16][8] per merc/guard: tile where it last knew enemy 0x28 + j to be


def _opponents(slot):
    """opponent slots (enemies scan 0..0x27, mercs/guards 0x28..0x2F); sight flag j = +0x13A + j"""
    return range(0, 0x28) if slot >= 0x28 else range(0x28, 0x30)


def target_tile(s, w):
    """TargetTile 0x70114: tile of the nearest opponent (Distance 0x1FD28 under 1500) by what is known: each opponent
    in play (any hp) that this soldier or its side knows of (sight flag or G_KNOWN != 0) is placed at this
    soldier's own last-seen tile when its sight flag is the fresher (or equal), else at the opponent's +0x108 (where
    the side last knew it). Tiles equal to the soldier's own are skipped. 0xC41 = none."""
    from combat import distance
    slot = s[0x17A]
    row = G_LAST_SEEN_M + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
    me = s16(u16(s, 0x114))
    best, tile = 1500, NO_TILE
    for j, i in enumerate(_opponents(slot)):
        o = w.rec(i)
        a, b = s8(s[0x13A + j]), s8(w.g8(G_KNOWN + i))
        if not (o[0x182] and o[0x138] and o[0x183]) or not (a or b):
            continue
        t = w.g16(row + 2 * j) if w.g8(G_FRESHER + 8 * b + a) or a == b else u16(o, 0x108)
        if t == u16(s, 0x114):
            continue
        d = s16(distance(tile_x(me), tile_y(me), tile_x(s16(t)), tile_y(s16(t))))
        if d < best:
            best, tile = d, t
    return tile


def update_sightings(s, w, age):
    """UpdateSightings 0x740FC (AiThink, before the alert state): for each opponent in play (any hp) with a sight flag,
    re-check the sight (0x72F50, may rewrite the flag; returns new sightings), then with `age` the flag grows older:
    2..4 -> +1 (past 4: 0), -1..-3 -> -1 (past -3: 0), 1 stays. A merc/guard that made new sightings reacts
    (ShareSightings 0x736BC). AI type 2 skips all of it."""
    if s8(s[0xBA]) == 2:
        return
    new = 0
    for j, i in enumerate(_opponents(s[0x17A])):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and o[0x183]) or not s[0x13A + j]:
            continue
        new = (new + w.ext("sight_check", s, i)) & 0xFF
        if age & 0xFF:
            v = s8(s[0x13A + j])
            if v >= 2:
                v += 1
                s[0x13A + j] = 0 if v > 4 else v & 0xFF
            elif v <= -1:
                v -= 1
                s[0x13A + j] = 0 if v < -3 else v & 0xFF
    if new and s[0x17A] < 0x28:
        w.ext("share_sightings", s, 0)


G_SIGHT_RANGE = 0x92FC3       # u8 [facing * 8 + direction]: tiles a soldier sees toward a direction (1..8 each):
                             # 14 ahead, 12 at 45 degrees, 8 at 90, 3 at 135, 0 behind


def enemy_turned(s, w):
    """0x73578 (op 63, an enemy finished a turn): SightCheck 0x72F50 on each soldier 0..0x27 in play"""
    for i in range(0x28):
        o = w.rec(i)
        if o[0x182] and o[0x138] and o[0x183]:
            w.ext("sight_check", s, i)


def guard_turned(s, w):
    """0x7351C (op 64, a merc or guard finished a turn): SightCheck 0x72F50 on each enemy 0x28..0x2F in play, then
    the "sees N enemies" message 0x11448 when 0x92DC6 (sees_slot) is this soldier's slot byte"""
    for i in range(0x28, 0x30):
        o = w.rec(i)
        if o[0x182] and o[0x138] and o[0x183]:
            w.ext("sight_check", s, i)
    if w.g8(0x92DC6) == s[0x17A]:
        w.ext("w_11448", s)


def _bump_orders(s):
    """BumpOrders 0x6F29C: orders +0x1D4 + 1 while below 2, except a type-3 soldier with orders 0 and +0x18F == 99"""
    if s[0x1D4] < 2 and not (s8(s[0xBA]) == 3 and s[0x1D4] == 0 and s8(s[0x18F]) == 99):
        s[0x1D4] = (s[0x1D4] + 1) & 0xFF


def _sight_ok(s, o):
    """the shared guard of SightCheck? / MarkSeen: both on the map, in play and alive, on opposite sides"""
    a, b = s[0x17A], o[0x17A]
    return (a < 0x30 and b < 0x30 and s[0x182] and s[0x138] and s[0x183] and o[0x182] and o[0x138] and o[0x183]
            and (b >= 0x28 or a >= 0x28) and (b < 0x28 or a < 0x28) and s8(s[0xBA]) != 2)


def _knowledge(s, i):
    """(sight flag index, remember-table address, last-seen-table address) for soldier s about opponent slot i"""
    slot = s[0x17A]
    if i < 0x28:
        return i, G_REMEMBER_E + (slot - 0x28) * 0x28 + i, G_LAST_SEEN_E + (slot - 0x28) * 0x50 + 2 * i
    return i - 0x28, G_REMEMBER_M + slot * 8 + i - 0x28, G_LAST_SEEN_M + slot * 16 + 2 * (i - 0x28)


def mark_seen(s, w, i):
    """MarkSeen 0x732FC: soldier s sees opponent slot i now. Sight flag -> 1, last-seen tile -> the opponent's tile;
    newly seen (flag was not 1): +0x1D5 + 1, orders bump (0x6F29C) while the alert state is below 2, alert 3, and
    re-think unless the flag was 2 with the opponent still on the last-seen tile, or the action is 3/4. The remember
    byte becomes 0xFF if it was 0, else 1; the opponent's +0x9C becomes at least 1."""
    o = w.rec(i)
    if not _sight_ok(s, o):
        return
    j, rem, seen = _knowledge(s, i)
    old = s[0x13A + j]
    s[0x13A + j] = 1
    last = w.g16(seen)
    w.s16(seen, u16(o, 0x114))
    if old != 1:
        s[0x1D5] = (s[0x1D5] + 1) & 0xFF
        if s8(s[0xBB]) < 2:
            _bump_orders(s)
        s[0xBB] = 3
        if s8(old) != 2 or last != u16(o, 0x114):
            s[0x1C1] = 1
        if s[0x1D1] in (3, 4):
            s[0x1C1] = 1
    w.s8(rem, 0xFF if w.g8(rem) == 0 else 1)
    if s8(o[0x9C]) <= 0:
        w.s8(G_SOLDIERS + 0x200 * i + 0x9C, 1)


def no_ally_sees(s, w, i):
    """0x73268: 1 unless another soldier of s's side (aiType < 3: slots 0..15, else 0x28..0x2F) in play with
    hp >= 15 has a sight flag of 1 on opponent slot i"""
    i = w.rec(i)[0x17A]
    first, end, j = (0, 0x10, i - 0x28) if s8(s[0xBA]) < 3 else (0x28, 0x30, i)
    for k in range(first, end):
        p = w.rec(k)
        if p[0x182] and p[0x138] and s8(p[0x183]) >= 15 and k != s[0x17A] and w.g8(G_SOLDIERS + 0x200 * k + 0x13A
                                                                                     + (j & 0xFF)) == 1:
            return 0
    return 1


def sight_check(s, w, i):
    """SightCheck? 0x72F50 (UpdateSightings, per opponent with a sight flag) -> 1 for a new sighting. Needs both
    soldiers in play on opposite sides, s not type 2, its current animation code (+0x1E8 + 2 * +0x1F3) >= 4, and a
    type-4 soldier (Santino) with +0x9C > 0. Seen: type 4 sees whoever has a sight flag 1 on slot 0x28 (and clears
    the tile-byte-3 rooms, 0x61744); others need the opponent within the sight range (14 tiles while flagged, else the
    G_SIGHT_RANGE cone by facing and DirectionTo) and SightLine 0x58760 from (x+2, y+4) to (x+2, y+4). Seen ->
    MarkSeen 0x732FC; a flag of 1 or 2 (or s an enemy) gives no new sighting, a merc-side flag of 1/2 sets
    g_known 1. Lost (flag was 1): +0x1D5 - 1 (at 0: alert 2), flag 2, remember byte 0xFF -> 1, and g_known 1 -> 2
    once no ally sees the opponent (0x73268)."""
    o = w.rec(i)
    if s[0x17A] >= 0x30 or o[0x17A] >= 0x30:
        return 0
    if not _sight_ok(s, o) or u16(s, 0x1E8 + 2 * s[0x1F3]) < 4 or (s8(s[0xBA]) == 4 and s8(s[0x9C]) <= 0):
        return 0
    j, rem, _ = _knowledge(s, i)
    old = s8(s[0x13A + j])
    seen = False
    if s8(s[0xBA]) == 4:
        if s8(o[0x13A]) == 1:
            w.ext("clear_rooms")
            seen = True
    else:
        if old:
            rng = 0xE
        else:
            d = _i32(w.ext("direction", s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(u16(o, 0x116)),
                           s16(u16(o, 0x118)))) & 0xFF
            rng = w.g8(G_SIGHT_RANGE + 8 * s8(s[0x1A1]) + d)
        if (_i32(w.ext("tile_dist", s16(u16(s, 0x114)), s16(u16(o, 0x114)))) & 0xFF) <= rng and _i32(
                w.ext("sight_line", s, s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, s16(u16(o, 0x114)),
                      s16(u16(o, 0x116)) + 2, s16(u16(o, 0x118)) + 4, old, rng, i)):
            seen = True
    if seen:
        mark_seen(s, w, i)
    elif old == 1:
        s[0x1D5] = (s[0x1D5] - 1) & 0xFF
        if s[0x1D5] == 0:
            s[0xBB] = 2
        s[0x13A + j] = 2
        if w.g8(rem) == 0xFF:
            w.s8(rem, 1)
        if s8(w.g8(G_KNOWN + i)) == 1 and no_ally_sees(s, w, i):
            w.s8(G_KNOWN + i, 2)
    if seen:
        if old in (1, 2):
            if s[0x17A] < 0x28:
                w.s8(G_KNOWN + i, 1)
        else:
            return 1
    return 0


DIR_STEP_1 = (-56, -55, 1, 57, 56, 55, -1, -57)   # i16 table 0x92FBA: tile step for direction 1..8 (N .. NW)
G_MOVECODE = 0x108258        # u16 per tile: terrain move code (combat.py)


def off_map_step(frm, to):
    """OffMapStep 0x46F70: 1 if the step from tile frm to tile to leaves the map. Only checked for frm on the
    left/right column or frm <= 56 or frm >= 0xC08: to < 0; left column -> column 55; right column -> column 0;
    frm >= 0xC08 and to >= 0xC40."""
    c = frm - cdiv(frm, 56) * 56
    if c not in (0, 55) and 56 < frm < 0xC08:
        return 0
    if to < 0:
        return 1
    d = to - cdiv(to, 56) * 56
    return int((c == 0 and d == 55) or (c == 55 and d == 0) or (frm >= 0xC08 and to >= 0xC40))


def step_tile(tile, step):
    """StepTile 0x46FF4: tile + step, or tile itself if that leaves the map."""
    return tile if off_map_step(tile, tile + step) else tile + step


def soldier_busy(s, w):
    """SoldierBusy? 0x5AD1C -> 1 while the animation word u16 +0x1E8 + 2 x +0x1F3 is 0x1B, 0x1F or 0x23, else 0."""
    return int(u16(s, 0x1E8 + 2 * s[0x1F3]) in (0x1B, 0x1F, 0x23))


def swap_hand(s, w, a, b):
    """SwapHand? 0x55B0C (EAX soldier, DL a, BL b; signed bytes into the place table 0x93D59 = 0 7 8 9 10 11 1):
    items table[a] and table[b] change places (12 bytes each); the selected soldier's item panel redraws
    (0x55FD8 (soldier, 0, 1))."""
    place = (0, 7, 8, 9, 10, 11, 1)
    a, b = s8(a & 0xFF), s8(b & 0xFF)
    if not (0 <= a < 7 and 0 <= b < 7):
        return 0
    ia, ib = 12 * place[a], 12 * place[b]
    kept = bytes(s[ia:ia + 12])
    s[ia:ia + 12] = s[ib:ib + 12]
    s[ib:ib + 12] = kept
    if s[0x17A] == w.g8(G_SELECTED):
        w.ext("item_panel", s, 0, 1)
    return 0


def move_ap_cost(s, w):
    """MoveApCost 0x1B118: cheapest StepApCost (no sneak / Alt / +0x177) of the 8 neighbours on the map, 99 if none
    (the AI's "can walk" = AP left >= this)."""
    from combat import step_ap_cost
    me = s16(u16(s, 0x114))
    best = 99
    for step in DIR_STEP_1:
        nb = step_tile(me, step)
        if nb != me:
            best = min(best, step_ap_cost(w.g16(G_MOVECODE + 2 * nb), 0, 0, 0))
    return best


def tile_distance(a, b):
    """TileDistance 0x15A24: max(|row difference|, |column difference|) of two tiles (C division)."""
    dy = abs(cdiv(a, 56) - cdiv(b, 56))
    dx = abs((a - cdiv(a, 56) * 56) - (b - cdiv(b, 56) * 56))
    return dx if dy < dx else dy


G_LAST_SPOTTER = 0x22A108    # u8 enemy slot that last called out a sighting (ShareSightings)
G_CALL_M = 0x92DBA           # u16 tile + u8 level (+2) of the mercs' side's latest call (ShareSightings)
G_CALL_E = 0x92DBE           # same for the enemies' side


G_SHOW_UI = 0x92DCC           # u8: tactical screen drawn (DrawCursorPass? runs only when set)
G_SELECTED = 0x284F11         # u8 slot of the selected merc


def share_sightings(s, w, flag):
    """ShareSightings 0x736BC: soldier s shares what it knows with its side. Enemies set G_LAST_SPOTTER. For each
    opponent in play with a sight flag on it that is fresher than (or equal to) the side's G_KNOWN level: G_KNOWN
    takes the flag and the opponent's +0x108 knownTile takes s's last-seen tile for it. A merc/guard (slot < 16)
    that sees it (flag 1, hp >= 15) counts it as a new sighting when the side did not know it (G_KNOWN was 0) and as
    a new noise when G_KNOWN was not 1/2 (only while no new sighting); the nearest by Distance (first of each kind
    wins ties, a new sighting resets the search) is remembered. A merc (slot < 8) practises stat 9 (x2) for each new
    sighting the opponent does not see back. Then, with any new sighting or noise: a merc reacts (0x56D28:
    slot, 0xFF none seen / 0 unseen / 2 seen back with `flag` / 0xFF, the nearest), and as the selected merc on the
    tactical screen 0x104C8 + 0x196F8 and +0x197 = 1; a guard with a new sighting focuses (0x3F724) on itself and,
    if the nearest is visible (+0x9C > 0) or 0x93458 is set, on it too (0x229C94 / 0x92E52 / 0x92E53 = it), then
    shows "Guard sees a new enemy!". Last, the side's call (G_CALL_M / G_CALL_E) takes s's call tile and level if
    s's level is higher."""
    slot = s[0x17A]
    if slot >= 0x28:
        w.s8(G_LAST_SPOTTER, slot)
    best, near, seen, heard, unseen = 1500, 0x40, 0, 0, True
    for k, j in enumerate(_opponents(slot)):
        o = w.rec(j)
        f, known = s8(s[0x13A + k]), s8(w.g8(G_KNOWN + j))
        if not (o[0x182] and o[0x138] and o[0x183]) or not f:
            continue
        if not w.g8(G_FRESHER + 8 * known + f) and f != known:
            continue
        if slot < 0x10 and s8(o[0x183]) >= 15 and f == 1:
            d = None
            if known == 0:
                seen = (seen + 1) & 0xFF
                d = _distance_to(s, o)
                if seen == 1 or s16(d) < s16(best):
                    best, near = d, j
                if slot < 8:
                    if s8(w.g8(G_SOLDIERS + 0x200 * j + 0x13A + slot)) == 1:
                        unseen = False
                    else:
                        w.ext("stat_practice", s, 9, 2, 0)
            elif known not in (1, 2):
                heard = (heard + 1) & 0xFF
                if seen == 0:
                    d = _distance_to(s, o)
                    if heard == 1 or s16(d) < s16(best):
                        best, near = d, j
        w.s8(G_KNOWN + j, s[0x13A + k])
        row = G_LAST_SEEN_M - 0x50 + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
        w.s16(G_SOLDIERS + 0x200 * j + 0x108, w.g16(row + 2 * o[0x17A]))
    if heard or seen:
        if slot < 8:
            how = 0xFF if not seen else 0 if unseen else 2 if flag & 0xFF else 0xFF
            w.ext("merc_sees", slot, how, near)
            if w.g8(G_SHOW_UI) and slot == w.g8(G_SELECTED):
                w.ext("stop_walk", s, 0)
                w.ext("stop_anim", s, 0)
                s[0x197] = 1
        elif seen:
            w.ext("focus", slot, 0)
            if s8(w.rec(near)[0x9C]) > 0 or w.g8(0x93458):
                w.s32(0x229C94, near)
                w.s8(0x92E53, near)
                w.s8(0x92E52, near)
                w.ext("focus", near, 0)
            w.ext("message", "Guard sees a new enemy!")
    call = G_CALL_M if slot < 0x28 else G_CALL_E
    if w.g8(call + 2) < s[0x1C0]:
        w.s16(call, u16(s, 0x122))
        w.s8(call + 2, s[0x1C0])


def _distance_to(s, o):
    """Distance 0x1FD28 from s's world pixel (+0x116, +0x118) to the screen point of o's tile (TileToX/Y)"""
    from combat import distance
    t = s16(u16(o, 0x114))
    return distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(t), tile_y(t))


def ally_call_tile(s, w):
    """AllyCallTile 0x702A8: tile of the ally (own side, not itself, in play) that is in combat (+0x1C4) or is the
    last spotter, with the lowest non-zero PathApCost under 1000; 0xC41 none (EAX high word = stack garbage)."""
    slot = s[0x17A]
    allies = range(0, 0x28) if slot < 0x28 else range(0x28, 0x30)
    best, tile = 1000, NO_TILE
    for i in allies:
        o = w.rec(i)
        if i == slot or not (o[0x182] and o[0x138] and o[0x183]):
            continue
        if not o[0x1C4] and w.g8(G_LAST_SPOTTER) != o[0x17A]:
            continue
        ap = s16(w.ext("path_ap", s, s16(u16(o, 0x114))))
        if ap and ap < best:
            best, tile = ap, u16(o, 0x114)
    return tile


def news_value(s, w):
    """NewsValue? 0x705F4 (shout / alarm chances x5 / x10): the soldier's call level +0x1C0 above its side's latest
    call level (0 if not above), plus for every opponent how much fresher the soldier's sight flag is than its
    side's knowledge (g_fresher). u8."""
    slot = s[0x17A]
    side = w.g8((G_CALL_M if slot < 0x28 else G_CALL_E) + 2)
    v = s[0x1C0] - side if s[0x1C0] > side else 0
    for j, i in enumerate(_opponents(slot)):
        v += w.g8(G_FRESHER + 8 * s8(w.g8(G_KNOWN + i)) + s8(s[0x13A + j]))
    return v & 0xFF


def noise_tile(s, w):
    """NoiseTile 0x7038C -> (tile, score): the most pressing thing heard, by a 16-bit score (start -10000, the
    highest wins, ties keep the earlier): for each opponent in play (any hp) that the soldier only heard (sight flag
    < 0): distance to its own last-seen tile x the flag; that its side only heard (G_KNOWN < 0): distance to the
    opponent's +0x108 x the level; then the soldier's own call (tile +0x122, level +0x1C0; at its own tile the
    call is cleared: +0x122 = none, +0x1C0 = 0; else (level / 2 - 6) x distance);
    then its side's latest call (level / 2 - 6) x distance, unless at its own tile. Distances (TileDistance) use
    the low byte. EDX receives the score (u16), 0xC41 = nothing (EAX high word = stack garbage)."""
    slot = s[0x17A]
    me = s16(u16(s, 0x114))
    row = G_LAST_SEEN_M + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
    call = G_CALL_M if slot < 0x28 else G_CALL_E
    best, tile = -10000, NO_TILE

    def offer(target, factor, result):
        nonlocal best, tile
        v = s16((w.ext("tile_dist", me, target) & 0xFF) * factor)
        if v > best:
            best, tile = v, result

    for j, i in enumerate(_opponents(slot)):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and o[0x183]):
            continue
        if s8(s[0x13A + j]) < 0:
            offer(s16(w.g16(row + 2 * j)), s8(s[0x13A + j]), w.g16(row + 2 * j))
        if s8(w.g8(G_KNOWN + i)) < 0:
            offer(s16(u16(o, 0x108)), s8(w.g8(G_KNOWN + i)), u16(o, 0x108))
    if s16(u16(s, 0x122)) != NO_TILE:
        if u16(s, 0x122) == u16(s, 0x114):
            w16(s, 0x122, NO_TILE)
            s[0x1C0] = 0
        else:
            offer(s16(u16(s, 0x122)), (s[0x1C0] >> 1) - 6, u16(s, 0x122))
    t = s16(w.g16(call))
    if t != NO_TILE and t & 0xFFFF != u16(s, 0x114):
        offer(t, (w.g8(call + 2) >> 1) - 6, t & 0xFFFF)
    return tile, best & 0xFFFF


LEASH = {0: 1, 1: 5, 2: 15, 3: 99, 4: 99}   # orders +0x1D4 -> tiles a soldier strays from its post +0x12E
G_DIR_STEP = 0x92FB8         # i16 per direction (DirStep 0x47010 reads [dir]; 1..8 = DIR_STEP_1)
G_PATHS = 0x109BD8           # u8 directions 0..7, 0x50 per slot; soldier +0xD4 = count


def dir_step(w, d):
    """DirStep 0x47010"""
    return s16(w.g16(G_DIR_STEP + 2 * (d & 0xFF)))


def patrol_tile(s, w):
    """PatrolTile 0x6FAEC: up to 10 random tiles until CanStandAt?: inside the leash box around the post (+0x12E,
    clipped to the map), or anywhere (Random(0xC40)) for leash 99. 0xC41 none. (Returns the full i32.)"""
    r = LEASH.get(s[0x1D4], 0)
    post = s16(u16(s, 0x12E))
    boxed = r < 55
    left = top = wide = high = 0
    if boxed:
        col, row = post - cdiv(post, 56) * 56, cdiv(post, 56)
        left, right = min(r, col), min(r, 55 - col)
        top, bottom = min(r, row), min(r, 55 - row)
        wide, high = s8((left + right + 1) & 0xFF), s8((top + bottom + 1) & 0xFF)
    for _ in range(10):
        if boxed:
            dx = w.random(wide)
            dy = w.random(high)
            tile = s8((dx - left) & 0xFF) + post + s8((dy - top) & 0xFF) * 56
        else:
            tile = w.random(0xC40)
        if w.ext("can_stand_at", s, s16(tile)) & 0xFF:
            return tile & 0xFFFFFFFF
    return NO_TILE


def approach_tile(s, w, tile):
    """ApproachTile 0x7068C: a tile to walk to toward `tile`: the tile itself if CanStandAt?, else one random
    neighbour of it (8 tries of Random(8), CanStandAt?), else none. If that lies beyond the leash from the post
    (leash < 99, TileDistance), follow the soldier's planned path (+0xD4 steps in G_PATHS) instead, stepping from the
    last standable tile, keeping the last standable one, until a step leaves the leash. None (0xC41) if the result
    is the soldier's own tile."""
    r = LEASH.get(s[0x1D4], 0)
    t = tile & 0xFFFF
    if u16(s, 0x114) == t:
        return NO_TILE
    if not w.ext("can_stand_at", s, s16(t)) & 0xFF:
        for _ in range(8):
            d = w.random(8)
            nb = step_tile(s16(t), dir_step(w, d + 1))
            if w.ext("can_stand_at", s, s16(nb)) & 0xFF:
                t = nb & 0xFFFF
                break
        else:
            return NO_TILE
    post = s16(u16(s, 0x12E))
    if r < 99 and _i32(w.ext("tile_dist", post, s16(t))) > r:
        t = u16(s, 0x114)
        k = 0
        while k < s16(u16(s, 0xD4)):     # re-read each pass: CanStandAt's FindPath rewrites the path (0x70812)
            nb = step_tile(s16(t), dir_step(w, w.g8(G_PATHS + 0x50 * s[0x17A] + k) + 1))
            if _i32(w.ext("tile_dist", post, s16(nb))) > r:
                break
            if w.ext("can_stand_at", s, s16(nb)) & 0xFF:
                t = nb & 0xFFFF
            k += 1
    return NO_TILE if t == u16(s, 0x114) else t


def _known_tile(s, w, j, i, row):
    """where soldier s places opponent i (index j): None if neither it nor its side knows of it, else its own
    last-seen tile when its sight flag is the fresher (or equal), else the opponent's +0x108 (TargetTile & co.)"""
    a, b = s8(s[0x13A + j]), s8(w.g8(G_KNOWN + i))
    if not (a or b):
        return None
    return w.g16(row + 2 * j) if w.g8(G_FRESHER + 8 * b + a) or a == b else u16(w.rec(i), 0x108)


def advance_tile(s, w, weak):
    """AdvanceTile 0x6FE7C: where to advance, by walking AP (PathApCost, 0 = unreachable, 16-bit compares): the
    cheapest known opponent position (as TargetTile places them, own tile skipped) of an opponent with >= 15 hp
    (start 1000); if none, with `weak` the cheapest of those below 15 hp. Then the soldier's own call tile +0x122
    (cleared if it is its own tile) and its side's latest call tile replace it when cheaper. 0xC41 none."""
    slot = s[0x17A]
    row = G_LAST_SEEN_M + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
    call = G_CALL_M if slot < 0x28 else G_CALL_E
    strong, weak_best = [1000, NO_TILE], [1000, NO_TILE]
    for j, i in enumerate(_opponents(slot)):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and o[0x183]):
            continue
        t = _known_tile(s, w, j, i, row)
        if t is None or t == u16(s, 0x114):
            continue
        ap = s16(w.ext("path_ap", s, s16(t)))
        if ap:
            best = strong if s8(o[0x183]) >= 15 else weak_best
            if ap < best[0]:
                best[:] = ap, t
    best, tile = 1000, NO_TILE
    if strong[1] != NO_TILE:
        best, tile = strong
    elif weak & 0xFF and weak_best[1] != NO_TILE:
        best, tile = weak_best
    if s16(u16(s, 0x122)) != NO_TILE:
        t = u16(s, 0x122)
        if t == u16(s, 0x114):
            w16(s, 0x122, NO_TILE)
            s[0x1C0] = 0
        else:
            ap = s16(w.ext("path_ap", s, s16(t)))
            if ap and ap < best:
                best, tile = ap, t
    t = w.g16(call)
    if s16(t) != NO_TILE and t != u16(s, 0x114):
        ap = s16(w.ext("path_ap", s, s16(t)))
        if ap and ap < best:
            tile = t
    return tile


def buddy_tile(s, w):
    """BuddyTile 0x6FC90 (calm walk 3): 1 and +0x132 = a standable tile next to a random living soldier (AI types
    0..2: slots 0..0x27, 3..4: the enemy slots 0x28..0x2F; other types: undefined range in the original), not
    itself, whose tile is within leash + 1 of the post +0x12E (post none: skipped). n - 1 picks (dead ones are not
    picked again), each with 8 tries of a random neighbour (Random(8)) inside the leash that passes CanStandAt?."""
    if s[0xBA] <= 2:
        n, base = 0x28, 0
    elif s[0xBA] <= 4:
        n, base = 8, 0x28
    else:
        raise ValueError("AI type > 4: slot range undefined")
    r = LEASH.get(s[0x1D4], 0)
    tried = {k: k == s[0x17A] for k in range(base, base + n)}
    post = s16(u16(s, 0x12E))
    for _ in range(n - 1):
        for _ in range(100000):
            k = (w.random(n) + base) & 0xFF
            if not tried[k]:
                break
        else:
            raise RuntimeError("BuddyTile: no untried soldier left (the original loops forever)")
        o = w.rec(k)
        if not (o[0x182] and o[0x138] and o[0x183]):
            tried[k] = True
            continue
        if r < 99 and (post == NO_TILE or _i32(w.ext("tile_dist", post, s16(u16(o, 0x114)))) - 1 > r):
            continue
        for _ in range(8):
            nb = step_tile(s16(u16(o, 0x114)), dir_step(w, w.random(8) + 1))
            if r < 99 and _i32(w.ext("tile_dist", post, s16(nb))) > r:
                continue
            if w.ext("can_stand_at", s, s16(nb)) & 0xFF:
                w16(s, 0x132, nb)
                return 1
    return 0


G_T0_CLASS = 0x284E58        # u8 per t0 (ground) value: 3 = water a calm type-1 soldier avoids
G_T1_CLASS = 0x108158        # u8 per t1 (object) value: <= 5 ground-level, can be stood on
T1_STAND = {0x3B: 1, 0x41: 1, 0x5A: 1, 0x60: 1, 0x3D: -2, 0x3E: -2, 0x5C: -2, 0x5D: -2,
            0x43: -1, 0x45: -1, 0x62: -1, 0x64: -1}   # tall objects that are 1 / -1 / -2; others: t3 == 0x73


def tile_standable(w, tile):
    """TileStandable 0x23538: > 0 if a soldier may stand on the tile: 0 if its occupant (byte 5 & 0x7F, >= 0x30 =
    none) has +0x9C > 0; 1 for objects of class <= 5; else by object (T1_STAND), otherwise 1 only if byte 3 is 0x73."""
    occ = w.tile8(tile, 5) & 0x7F
    if occ < 0x30 and s8(w.g8(G_SOLDIERS + 0x200 * occ + 0x9C)) > 0:
        return 0
    t1 = w.tile8(tile, 1)
    if w.g8(G_T1_CLASS + t1) <= 5:
        return 1
    return T1_STAND.get(t1, 1 if w.tile8(tile, 3) == 0x73 else 0)


def is_water_tile(t0, t1):
    """IsWaterTile 0x23A10: ground t0 >= 0x6C and object t1 not 0x1C / 0x1F (as InWater)"""
    return t0 >= 0x6C and t1 not in (0x1C, 0x1F)


def _side_ok(nb, tile, flag, obj, standable):
    """0x3DC04: may the soldier use `tile` from neighbour nb? Yes unless all of: no flag (0, or >= 0xFE), the target
    is standable (>= 0), its object is 0x37..0x82, and nb comes before the target (nb <= tile: north or west)."""
    return int(0 < flag < 0xFE or standable < 0 or not 0x37 <= obj < 0x83 or nb > tile)


def reach_tile(s, w, tile, flag):
    """ReachTile 0x3DC4C -> 1 if the soldier can get to `tile` or next to it; the spot goes to G_MISSION_SPOT.
    The tile itself if it is standable and FindPath(s, tile, 0) finds a path (else pathLen +0xD4 = 0). Otherwise the
    N / E / S / W neighbour (StepTile) that is standable, has a path (FindPath(s, nb, 1), which sets pathLen), is
    not water, passes 0x3DC04 and has the shortest pathLen below 0x100 (first wins ties)."""
    standable = tile_standable(w, tile)
    if standable > 0:
        if w.ext("find_path", s, s16(tile), 0):
            w.s16(G_MISSION_SPOT, tile)
            return 1
        w16(s, 0xD4, 0)
    obj = w.tile8(tile, 1)
    best, pick, cand = 0x100, 0, []
    for i, d in enumerate((1, 3, 5, 7)):
        nb = step_tile(tile, dir_step(w, d))
        cand.append(nb)
        if tile_standable(w, nb) > 0 and w.ext("find_path", s, s16(nb), 1) and not is_water_tile(
                w.tile8(nb, 0), w.tile8(nb, 1)) and s16(u16(s, 0xD4)) < best and _side_ok(
                nb, tile, flag & 0xFF, obj, standable):
            pick, best = i, s16(u16(s, 0xD4))
    if best >= 0x100:
        return 0
    w.s16(G_MISSION_SPOT, cand[pick])
    return 1


ANIM_CAN_AIM = 0x93167       # u8 per animation x 8 (column 1 of the ANIM_CAN_ACT table): 0 = can't attack from it


def hand_gun_state(s):
    """0x56F20: main-hand item 1..0x10 (a gun): 1 if +0x02 (signed) >= 10, else -1; any other item 0"""
    if not 0 < s[0] <= 0x10:
        return 0
    return 1 if s8(s[2]) >= 10 else -1


def weapon_status_text(s):
    """WeaponStatusText 0x56F4C: the main hand's status text. HandGunState 0x56F20 1: the main hand count +0x01
    (signed) < 0 = JAMMED, else NO AMMO; -1 = ITEM BROKEN; 0 = NO WEAPON."""
    g = hand_gun_state(s)
    if g == 1:
        return b"JAMMED" if s8(s[1]) < 0 else b"NO AMMO"
    if g == -1:
        return b"ITEM BROKEN"
    return b"NO WEAPON"


def sees_enemies_text(s):
    """MsgSeesEnemies 0x11448: the nickname (+0x16E up to the NUL) + " sees N enemies!" ("enemy" for exactly 1),
    N = +0x1D5 signed; the slot +0x17A goes to 0x92DC6."""
    name = bytes(s[0x16E:]).split(b"\0", 1)[0]
    n = s8(s[0x1D5])
    if n == 1:
        return name + b" sees 1 enemy!"
    return name + b" sees " + str(n).encode() + b" enemies!"


def sight_reaction(s, how, near, effect_showing):
    """FUN_56D3C 0x56D3C: the queued sighting reaction of soldier record `s`. None = the effect animation still
    runs (no call, no reset); else (text, how, near) of ShowMercMessage: the text is empty while the main hand can
    fire (HandGunState != 0 and count +0x01 signed >= 1, or how 5), else WeaponStatusText 0x56F4C. The reset
    FUN_2F7A4 0x2F7A4 sets (0x229E00..3) = (0x40, 0x7F, 0x40, 0xFF) after a call."""
    if effect_showing:
        return None
    g = hand_gun_state(s)
    text = b"" if g != 0 and (s8(s[1]) >= 1 or how == 5) else weapon_status_text(s)
    return (text, how, near)


def attack_ready(s, w, tile):
    """AttackReady 0x584A0 -> i32: 1 = can attack now. -3 target is the soldier's own tile, -2 in water, -4 the current
    animation can't attack (ANIM_CAN_AIM), -6 the main hand holds neither a gun (items 1..0x10, 0x56F20) nor a throwable
    (kind 0x95), -5 main-hand count +0x01 is 0, -1 the animation can't act (ANIM_CAN_ACT)."""
    if tile == s16(u16(s, 0x114)):
        return -3
    if is_water_tile(s[0x1B2], s[0x1B4]):
        return -2
    anim = u16(s, 0x1E8 + 2 * s[0x1F3])
    if not w.g8(ANIM_CAN_AIM + 8 * anim):
        return -4
    if not hand_gun_state(s) and w.g8(G_ITEM_KIND + 12 * s[0]) != 0x95:
        return -6
    if not s[1]:
        return -5
    if not w.g8(ANIM_CAN_ACT + 8 * anim):
        return -1
    return 1


def can_stand_at(s, w, tile):
    """CanStandAt 0x72578: 1 if the soldier may go and stand on the tile: not ground class 3 for a calm (no combat)
    AI type 1; TileStandable > 0; not water (t0 >= 0x6C with t1 not 0x1C/0x1F); no 0x80 flag in byte 5 unless
    wearing item 0x28 in entry 3; nobody there; not its own tile; FindPath finds a way; not the tile its last
    walk failed to reach (+0x10E)."""
    t = s16(tile)
    if s8(s[0xBA]) == 1 and not w.g8(G_COMBAT) and w.g8(G_T0_CLASS + w.tile8(t, 0)) == 3:
        return 0
    if tile_standable(w, t) <= 0:
        return 0
    if w.tile8(t, 0) >= 0x6C and w.tile8(t, 1) not in (0x1C, 0x1F):
        return 0
    if w.tile8(t, 5) & 0x80 and s[0x24] != 0x28:
        return 0
    if _occupant(w, t) != 0x40 or t & 0xFFFF == u16(s, 0x114):
        return 0
    if not w.ext("find_path", s, t, 0) or t & 0xFFFF == u16(s, 0x10E):
        return 0
    return 1


def find_dry_tile(s, w):
    """FindDryTile 0x713D0: in squares of radius 5, 10 .. 25 around the soldier (clipped to the map, rows then
    columns), the CanStandAt? tile with the lowest non-zero PathApCost (16-bit, start 1000); the first radius with
    one wins. 0xC41 none."""
    me = s16(u16(s, 0x114))
    col, row = me - cdiv(me, 56) * 56, cdiv(me, 56)
    tile, best = NO_TILE, 1000
    for r in (5, 10, 15, 20, 25):
        left, right, up, down = min(r, col), min(r, 55 - col), min(r, row), min(r, 55 - row)
        for dy in range(-s8(up & 0xFF), s8(down & 0xFF) + 1):
            for dx in range(-s8(left & 0xFF), s8(right & 0xFF) + 1):
                t = s16(dx + me + dy * 56)
                if w.ext("can_stand_at", s, t) & 0xFF:
                    ap = s16(w.ext("path_ap", s, t))
                    if ap and ap < best:
                        best, tile = ap, t & 0xFFFF
        if tile != NO_TILE:
            break
    return tile


G_RETREAT_RADIUS = 0x95C60   # u8 per difficulty (stride 2): 3 / 5 / 7 tiles


def retreat_tile(s, w):
    """RetreatTile 0x70FD0 (was ObjectiveTile?): the tile in a box around the soldier (radius 3/5/7 by difficulty,
    in alert state 3 at most AP left / 2) that is CanStandAt?, reachable (PathApCost != 0; in state 3 within the AP
    left) and farthest from the nearest known opponent (>= 15 hp, power != -999, placed as TargetTile does), only if
    farther than the soldier is now (soldier px +0x116 / +0x118). Distances: Distance 0x1FD28, 16-bit compares,
    start 1500. Enemies only consider slots 0..15. 0xC41 none."""
    from combat import distance
    slot = s[0x17A]
    opps = range(0x28, 0x30) if slot < 0x28 else range(0, 0x10)
    row = G_LAST_SEEN_M + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
    me = s16(u16(s, 0x114))
    near, known = 1500, []
    for j, i in enumerate(opps):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and s8(o[0x183]) >= 15) or not (s[0x13A + j] or w.g8(G_KNOWN + i)):
            continue
        if s16(soldier_power(o, w, me)) == -999:
            continue
        t = _known_tile(s, w, j, i, row)
        d = s16(distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(s16(t)), tile_y(s16(t))))
        near = min(near, d)
        known.append(t)
    if not known:
        return NO_TILE
    r = w.g8(G_RETREAT_RADIUS + 2 * s8(w.g8(G_DIFFICULTY)))
    if s8(s[0xBB]) == 3:
        half = cdiv(s8(s[0x1A5]), 2) & 0xFF
        r = min(r, half)
    col, rw = me - cdiv(me, 56) * 56, cdiv(me, 56)
    left, right, up, down = min(r, col), min(r, 55 - col), min(r, rw), min(r, 55 - rw)
    best, tile = near, NO_TILE
    for dy in range(-s8(up & 0xFF), s8(down & 0xFF) + 1):
        for dx in range(-s8(left & 0xFF), s8(right & 0xFF) + 1):
            c = s16(me + dx + dy * 56)
            if not w.ext("can_stand_at", s, c) & 0xFF:
                continue
            ap = s16(w.ext("path_ap", s, c))
            if not ap or (s8(s[0xBB]) == 3 and ap > s8(s[0x1A5])):
                continue
            far = 1500
            for t in known:
                far = min(far, s16(distance(tile_x(c), tile_y(c), tile_x(s16(t)), tile_y(s16(t)))))
            if far > best:
                best, tile = far, c & 0xFFFF
    return tile


def position_score(w, me_slot, my_tile, opp_slot, opp_tile, power, dist):
    """PositionScore 0x6F750 (stack: power, dist): with both soldiers moved there for the moment (tile + px
    +0x116/+0x118, restored after), (300 - dist) x power x ((100 - opponent's LineOfFire? at my tile) - (100 - mine at
    its tile, or 100 if mine is below 25 %)), byte differences, i32 wrap-around."""
    saved = []
    for k, t in ((me_slot, my_tile), (opp_slot, opp_tile)):   # move each soldier onto its tile (unless there)
        a = G_SOLDIERS + 0x200 * k
        old = w.g16(a + 0x114)
        if t & 0xFFFF == old:
            saved.append(None)
            continue
        saved.append((a, old, w.g16(a + 0x116), w.g16(a + 0x118)))
        w.s16(a + 0x114, t)
        w.s16(a + 0x116, tile_x(s16(t)))
        w.s16(a + 0x118, tile_y(s16(t)))
    theirs = (100 - w.ext("fire_line", opp_slot, s16(my_tile), 1)) & 0xFF
    mine = w.ext("fire_line", me_slot, s16(opp_tile), 1) & 0xFF
    mine = 100 if s8(mine) < 25 else (100 - mine) & 0xFF
    for v in saved:                    # put them back; a soldier whose old tile was 0xC41 stays moved (as the original)
        if v and v[1] != NO_TILE:
            a, old, x, y = v
            w.s16(a + 0x114, old)
            w.s16(a + 0x116, x)
            w.s16(a + 0x118, y)
    return _i32((300 - s16(dist)) * _i32(s16(power) * s8((theirs - mine) & 0xFF)))


def search_tile(s, w, mode):
    """SearchTile 0x70850: attack position. Opponents (>= 15 hp, power != -999, enemies: slots 0..15) known and within
    300 (Distance from the soldier's tile to where they are placed, as TargetTile does) are scored from here
    (PositionScore with their power and that distance); then every tile of a box (radius as RetreatTile; outside
    alert 3 with a leash < 99 each side is pulled in by how far its edge lies beyond the leash from the post)
    that is CanStandAt?, reachable (alert 3: within the AP left) and, for mode 1 not nearer / mode 2 not farther to
    the nearest opponent than now, is scored the same way (distances from the tile). The best tile beating the
    current score wins if it improves on it by >= 20 % of max(|best|, |current|). 0xC41 none."""
    from combat import distance
    slot = s[0x17A]
    opps = range(0x28, 0x30) if slot < 0x28 else range(0, 0x10)
    row = G_LAST_SEEN_M + slot * 16 if slot < 0x28 else G_LAST_SEEN_E + (slot - 0x28) * 0x50
    me = s16(u16(s, 0x114))
    dist = lambda a, b: s16(distance(tile_x(s16(a)), tile_y(s16(a)), tile_x(s16(b)), tile_y(s16(b))))
    near, near_tile, cur, targets = 1500, NO_TILE, 0, []
    for j, i in enumerate(opps):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and s8(o[0x183]) >= 15) or not (s[0x13A + j] or w.g8(G_KNOWN + i)):
            continue
        t = _known_tile(s, w, j, i, row)
        d = dist(me, t)
        if d > 300:
            continue
        power = s16(soldier_power(o, w, me))
        if power == -999:
            continue
        targets.append((t, o[0x17A], power))
        cur = _i32(cur + position_score(w, slot, me, o[0x17A], t, power, d))
        if d < near:
            near, near_tile = d, t
    if not targets:
        return NO_TILE
    r = w.g8(G_RETREAT_RADIUS + 2 * s8(w.g8(G_DIFFICULTY)))
    if s8(s[0xBB]) == 3:
        r = min(r, cdiv(s8(s[0x1A5]), 2) & 0xFF)
    col, rw = me - cdiv(me, 56) * 56, cdiv(me, 56)
    left, right, up, down = min(r, col), min(r, 55 - col), min(r, rw), min(r, 55 - rw)
    leash = LEASH.get(s[0x1D4], 0)
    if s8(s[0xBB]) != 3 and leash < 99:
        post = s16(u16(s, 0x12C + 2))
        over = lambda t: (w.ext("tile_dist", t, post) & 0xFF) - leash
        v = over(me - s8(left))
        left = (left - v) & 0xFF if v > 0 else left
        v = over(me + s8(right))
        right = (right - v) & 0xFF if v > 0 else right
        v = over(me - s8(up) * 56)
        up = (up - v) & 0xFF if v > 0 else up
        v = over(me + s8(down) * 56)
        down = (down - v) & 0xFF if v > 0 else down
    best, tile = cur, NO_TILE
    for dy in range(s8(-up & 0xFF), s8(down) + 1):
        for dx in range(s8(-left & 0xFF), s8(right) + 1):
            c = s16(me + dx + dy * 56)
            if not w.ext("can_stand_at", s, c) & 0xFF:
                continue
            if mode & 0xFF:
                dm = dist(c, near_tile)
                if (mode & 0xFF == 1 and dm < near) or (mode & 0xFF == 2 and dm > near):
                    continue
            ap = s16(w.ext("path_ap", s, c))
            if not ap or (s8(s[0xBB]) == 3 and ap > s8(s[0x1A5])):
                continue
            total = 0
            for t, k, power in targets:
                total = _i32(total + position_score(w, slot, c, k, t, power, dist(c, t)))
            if total > best:
                best, tile = total, c & 0xFFFF
    if tile == NO_TILE:
        return NO_TILE
    m = _i32(max(abs(best), abs(cur)))         # abs(INT_MIN) stays negative, as in the original
    if m <= 0 or s16(cdiv(_i32((best - cur) * 100), m)) < 20:
        return NO_TILE
    return tile


G_WEAPON_B5 = 0x94C36        # g_weaponStats + 5: u8 per item type (6 B each): damage base
G_ITEM_RANGE = 0x94E03       # g_itemTable + 0xB: u8 weapon range in tiles (12 B each)
G_HELMET_ABSORB = 0x92F3C    # s16 per item type
G_ARMOR_ABSORB = 0x92F1E     # s16 per item type (body armour, Shielded Vest 0x31)


def expected_damage(s, o, w, chance):
    """ExpectedDamage 0x7208C: u8 >= 1 damage s's main-hand gun would do to o: b5 x (100 - distance x 50 /
    (range x 15) + chance / 4) / 100 (distance between the px positions +0x116/+0x118), minus (helmet x 15 + body
    x 80) / 100 where helmet = condition x helmet absorb / 100 and body = the same for a Shielded Vest in entry 6
    plus the body armour in entry 5 (bytes)."""
    from combat import distance
    d = s16(distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(u16(o, 0x116)), s16(u16(o, 0x118))))
    t = s[0]
    q = cdiv(d * 50, s16(w.g8(G_ITEM_RANGE + 12 * t) * 15))
    base = cdiv(w.g8(G_WEAPON_B5 + 6 * t) * (100 - s16(q) + (chance & 0xFF) // 4), 100) & 0xFF
    helm = body = 0
    if o[0x18]:
        helm = cdiv(s8(o[0x1A]) * s16(w.g16(G_HELMET_ABSORB + 2 * o[0x18])), 100) & 0xFF
    if o[0x48] == 0x31:
        body = (body + cdiv(s8(o[0x4A]) * s16(w.g16(G_ARMOR_ABSORB + 2 * 0x31)), 100)) & 0xFF
    if o[0x3C]:
        body = (body + cdiv(s8(o[0x3E]) * s16(w.g16(G_ARMOR_ABSORB + 2 * o[0x3C])), 100)) & 0xFF
    v = s8((base - (helm * 15 + body * 80) // 100) & 0xFF)
    return max(v, 1)


def gun_option(s, w):
    """GunOption 0x71590 -> 10-byte record (valid, slot, aim, chance, i32 score, u16 tile): over the opponents the
    soldier sees now (flag 1; slots 0..0x27 for AI type >= 3, else 0x28..0x2F) that are in play: if ActionApCost? at
    them fits the AP left and LineOfFire? (%) is not 0, the aim (0 .. min(4, AP left - cost)) with the best
    chance x AP left / (aim + ShotApCost - 1) (16-bit); chance' = line x chance / 100 (byte); score = that best x
    ExpectedDamage x chance' x SoldierPower(opponent, own tile, line of fire). A target replaces the kept one only
    with a higher chance' and either +50 % chance' or a score at least as high."""
    rec = bytearray(10)
    rec[1] = 0x40
    w16(rec, 8, NO_TILE)
    opps = range(0, 0x28) if s8(s[0xBA]) >= 3 else range(0x28, 0x30)
    me = s16(u16(s, 0x114))
    for j, i in enumerate(opps):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and o[0x183]) or s8(s[0x13A + j]) != 1:
            continue
        ot = s16(u16(o, 0x114))
        cost = w.ext("action_ap", s, ot, 1) & 0xFF
        if cost > s8(s[0x1A5]):
            continue
        line = w.ext("fire_line", s, ot, 0) & 0xFF
        if not line:
            continue
        sap = (w.ext("shot_ap", s, NO_TILE) - 1) & 0xFF
        top = min(s8(s[0x1A5]) - cost, 4)
        best = aim = chance = 0
        for a in range(top + 1):
            c = w.ext("chance_to_hit", s, ot, a) & 0xFF
            v = s16(cdiv(c * s8(s[0x1A5]), a + sap))
            if v > best:
                best, aim, chance = v, a, c
        if not best:
            continue
        eff = (line * chance // 100) & 0xFF
        if not eff:
            continue
        power = s16(soldier_power(o, w, me, 1))
        score = _i32(_i32(_i32(best * expected_damage(s, o, w, chance)) * eff) * power)
        prev = rec[3]
        if eff <= prev:
            continue
        if prev and s16(cdiv(eff * 100, prev) - 100) < 50 and score < _i32(u16(rec, 4) | u16(rec, 6) << 16):
            continue
        rec[0], rec[1], rec[2], rec[3] = 1, j + opps[0], aim, eff
        rec[4:8] = (score & 0xFFFFFFFF).to_bytes(4, "little")
        w16(rec, 8, u16(o, 0x114))
    return bytes(rec)


G_ENEMY_HIT_MOD = 0x95C4E    # g_enemyDiffHitMod: s16 per difficulty


def _melee_skill(p, first, second, aim=0):
    """one side of MeleeChance: (first x 3 + second + max hp + level x 10) / 6 (16-bit), + aim x 5, -20 when +0x1B9
    == 4 (min 1), minus wounds (as MercMaxAP) and fatigue"""
    v = s16(cdiv(s16(s8(p[first]) * 3 + s8(p[second]) + s8(p[0x184]) + s8(p[0x192]) * 10), 6))
    if aim & 0xFF:
        v = s16(v + (aim & 0xFF) * 10 // 2)
    if p[0x1B9] == 4:
        v = s16(v - 20)
        if v < 1:
            v = 1
    if s8(p[0x183]) < s8(p[0x184]):
        lost = ((p[0x184] - p[0x183] - p[0x185]) & 0xFF) // 2 + s8(p[0x184]) - s8(p[0x183])
        v = s16(v - cdiv(v * 2 * lost, s8(p[0x184]) * 3))
    if s8(p[0x18A]) < 100:
        v = s16(v - cdiv((100 - s8(p[0x18A])) * v, 200))
    return v


def melee_chance(s, o, w, aim):
    """MeleeChance 0x56A78: % (1..99) a knife (item 0x0F / 0x10 in the main hand, else 0) hits: attacker skill (dex x 3
    + agi ..) x 100 / (attacker + defender skill (agi x 3 + dex ..)), + the enemy difficulty modifier for enemies."""
    if s[0] not in (0x0F, 0x10):
        return 0
    a = _melee_skill(s, 0x18C, 0x18B, aim)
    d = _melee_skill(o, 0x18B, 0x18C)
    v = cdiv(a * 100, a + d)
    if s[0x17A] >= 0x28:
        v += s16(w.g16(G_ENEMY_HIT_MOD + 2 * s8(w.g8(G_DIFFICULTY))))
    v = s16(v)
    return 1 if v < 1 else min(v, 99)


def knife_damage(s, o, w, chance, sneak):
    """KnifeDamage 0x723D0: u8 >= 1: (b5 + (level - 1) x 2 + (wounds / 2 + hp - 50) / 5 + chance / 10) x the knife's
    condition / 100 (bytes); without `sneak` minus the Shielded Vest and body armour absorb x condition / 100."""
    b = (w.g8(G_WEAPON_B5 + 6 * s[0]) + (s[0x192] - 1) * 2) & 0xFF
    lost = s8((s[0x184] - s[0x183] - s[0x185]) & 0xFF)
    b = (s8(b) + cdiv(cdiv(lost, 2) + s8(s[0x183]) - 50, 5)) & 0xFF
    b = (s8(b) + (chance & 0xFF) // 10) & 0xFF
    b = cdiv(s8(s[2]) * s8(b), 100) & 0xFF
    if not sneak & 0xFF:
        if o[0x48] == 0x31:
            b = (s8(b) - cdiv(s8(o[0x4A]) * s16(w.g16(G_ARMOR_ABSORB + 2 * 0x31)), 100)) & 0xFF
        if o[0x3C]:
            b = (s8(b) - cdiv(s16(w.g16(G_ARMOR_ABSORB + 2 * o[0x3C])) * s8(o[0x3E]), 100)) & 0xFF
    return max(s8(b), 1)


def knife_option(s, w):
    """KnifeOption 0x71DD4 -> record as GunOption: seen opponents (flag 1) in play whose ActionApCost? fits; sneak
    attack (chance 99, armour ignored) if the opponent does not see the soldier and stands orthogonally next to it,
    else MeleeChance per aim 0 .. min(4, AP left - cost); best chance x AP left / (aim + ShotApCost - 1); score =
    best x KnifeDamage x chance x SoldierPower(opponent, own tile). Same replacement rule as GunOption."""
    rec = bytearray(10)
    rec[1] = 0x40
    w16(rec, 8, NO_TILE)
    if s8(s[0xBA]) >= 3:
        opps, own = range(0, 0x28), 0x28
    else:
        opps, own = range(0x28, 0x30), 0
    me = s16(u16(s, 0x114))
    for j, i in enumerate(opps):
        o = w.rec(i)
        if not (o[0x182] and o[0x138] and o[0x183]) or s8(s[0x13A + j]) != 1:
            continue
        ot = s16(u16(o, 0x114))
        cost = w.ext("action_ap", s, ot, 1) & 0xFF
        if cost > s8(s[0x1A5]):
            continue
        sap = (w.ext("shot_ap", s, NO_TILE) - 1) & 0xFF
        sneak = 0
        if s8(o[0x13A + s[0x17A] - own]) != 1 and _i32(w.ext("tile_dist", me, ot)) == 1:
            sneak = int(abs(me - ot) & 0xFF in (1, 56))
        top = min(s8(s[0x1A5]) - cost, 4)
        best = aim = chance = 0
        for a in range(top + 1):
            c = 99 if sneak else melee_chance(s, o, w, a)
            v = s16(cdiv(s8(s[0x1A5]) * c, sap + a))
            if v > best:
                best, aim, chance = v, a, c
        if not best:
            continue
        power = s16(soldier_power(o, w, me))
        score = _i32(_i32(_i32(knife_damage(s, o, w, chance, sneak) * best) * chance) * power)
        prev = rec[3]
        if chance <= prev:
            continue
        if prev and s16(cdiv(chance * 100, prev) - 100) < 50 and score < _i32(u16(rec, 4) | u16(rec, 6) << 16):
            continue
        rec[0], rec[1], rec[2], rec[3] = 1, i, aim, chance
        rec[4:8] = (score & 0xFFFFFFFF).to_bytes(4, "little")
        w16(rec, 8, u16(o, 0x114))
    return bytes(rec)


THROWN = {0x16: (0, None), 0x18: (10, 60), 0x1A: (22, 50), 0x1C: (37, 0)}   # item -> (blast, gas); 0x16 gas 75/37


def throw_damage(s, o, w, tile):
    """ThrowDamage 0x72250: damage s's main-hand thrown item does to o at `tile`: blast (armour: vest + body absorb x
    condition / 100, min 1) + gas / 3 (gas x (100 - mask condition) / 100 with item 0x28 in entry 3; item 0x16:
    75 on ground class 3, else 37), x the unit's condition (+2 if one left, else unit byte 6 + qty) / 100 (bytes);
    then halved on ground class >= 8, else x 1.5."""
    t = s16(tile)
    blast, gas = THROWN.get(s[0], (0, 0))
    if s[0] == 0x16:
        gas = 0x4B if w.g8(G_T0_CLASS + w.tile8(t, 0)) == 3 else 0x25
    if blast:
        if o[0x48] == 0x31:
            blast = (s8(blast) - cdiv(s16(w.g16(G_ARMOR_ABSORB + 2 * 0x31)) * s8(o[0x4A]), 100)) & 0xFF
        if o[0x3C]:
            blast = (s8(blast) - cdiv(s8(o[0x3E]) * s16(w.g16(G_ARMOR_ABSORB + 2 * o[0x3C])), 100)) & 0xFF
        if s8(blast) < 1:
            blast = 1
    if gas and o[0x24] == 0x28:
        gas = cdiv((100 - s8(o[0x26])) * s8(gas), 100) & 0xFF
    v = cdiv(s8(gas), 3) + s8(blast)
    cond = s8(s[2]) if s8(s[1]) == 1 else s8(s[(6 + s8(s[1])) & 0x1FF])
    r = s8(cdiv(s8(v & 0xFF) * cond, 100) & 0xFF)
    if w.g8(G_T0_CLASS + w.tile8(t, 0)) >= 8:
        return cdiv(r, 2)
    return cdiv(r, 2) + r


def throw_ap_cost(s, w):
    """ThrowApCost 0x58B14: 0 unless the main-hand item is of kind 0x95; else (max AP x 200 / (dex / 2 x 2 + 100) + 1)
    / 2, at most max AP, at least 1."""
    from combat import max_ap
    if w.g8(G_ITEM_KIND + 12 * s[0]) != 0x95:
        return 0
    m = max_ap(s) & 0xFF
    v = cdiv(cdiv(m * 200, cdiv(s8(s[0x18C]), 2) * 2 + 100) + 1, 2) & 0xFF
    v = min(v, m)
    return v or 1


def throw_chance(s, w, tile, aim):
    """ThrowChance 0x58BB0: 1..99 % a throw lands at `tile`: (mrk + dex) / 2, +10 at the last target, + aim x 10, +
    the enemy difficulty modifier; reach = range x 15 + hp - 50, less fatigue, >= 0; + reach / 2 - distance
    (px; 10 % less with item 0x27 in entry 3), halved when beyond reach; minus wounds (scaled down by level) and
    fatigue (scaled by dexterity)."""
    from combat import distance
    t = s16(tile)
    v = cdiv(s8(s[0x191]) + s8(s[0x18C]), 2)
    if t == s16(u16(s, 0xD0)):
        v += 10
    if aim & 0xFF:
        v += (aim & 0xFF) * 10
    if s[0x17A] >= 0x28:
        v += s16(w.g16(G_ENEMY_HIT_MOD + 2 * s8(w.g8(G_DIFFICULTY))))
    d = s16(distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(t), tile_y(t)))
    if s[0x24] == 0x27:
        d -= cdiv(d, 10)
    reach = w.g8(G_ITEM_RANGE + 12 * s[0]) * 15 + s8(s[0x183]) - 50
    reach = max(reach - cdiv((100 - s8(s[0x18A])) * reach, 200), 0)
    v += cdiv(reach, 2) - d
    if d > reach:
        v = cdiv(v, 2)
    if s8(s[0x183]) < s8(s[0x184]):
        lost = ((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + s8(s[0x184]) - s8(s[0x183])
        q = cdiv(v * 2 * lost, s8(s[0x184]) * 3) & 0xFF
        v -= cdiv((100 - (s8(s[0x192]) - 1) * 10) * q, 100)
    if s8(s[0x18A]) < 100:
        q = cdiv((100 - s8(s[0x18A])) * v, 200) & 0xFF
        v -= cdiv((100 - (s8(s[0x18C]) - 10)) * q, 100)
    return 1 if v < 1 else min(v, 99)


def throw_option(s, w):
    """ThrowOption 0x717F8 -> record as GunOption (slot = the targeted opponent, tile = where to throw): for each seen
    opponent (flag 1, in play) every tile within 2 (clipped to the map) whose ActionApCost? fits and LineOfFire? is
    not 0, and no ally in play (the soldier included) within 4 tiles; damage = sum over seen opponents within 3 of
    ThrowDamage x (100 - 20 x distance) / 100 (byte) x their power (16-bit); best aim 0..min(4, AP - cost) by
    min(100, chance x 1.5) x AP left / (ThrowApCost + aim); score = (line x that chance / 100) x best x damage; a
    strictly higher score replaces the kept one."""
    rec = bytearray(10)
    rec[1] = 0x40
    w16(rec, 8, NO_TILE)
    if s[0x17A] >= 0x28:
        allies, opps = range(0x28, 0x30), range(0, 0x28)
    else:
        allies, opps = range(0, 0x28), range(0x28, 0x30)
    ally_tiles = [u16(p, 0x114) for p in map(w.rec, allies) if p[0x182] and p[0x138] and p[0x183]]
    me = s16(u16(s, 0x114))
    targets = []
    for j, i in enumerate(opps):
        o = w.rec(i)
        if o[0x182] and o[0x138] and o[0x183] and s8(s[0x13A + j]) == 1:
            targets.append((u16(o, 0x114), o[0x17A], s16(soldier_power(o, w, me))))
    for t, k, _ in targets:
        tt = s16(t)
        col, rw = tt - cdiv(tt, 56) * 56, cdiv(tt, 56)
        left, right, up, down = min(col, 2), min(2, 55 - col), min(rw, 2), min(2, 55 - rw)
        for dy in range(s8(-up & 0xFF), s8(down & 0xFF) + 1):
            for dx in range(s8(-left & 0xFF), s8(right & 0xFF) + 1):
                c = s16(tt + dx + dy * 56)
                cost = w.ext("action_ap", s, c, 1) & 0xFF
                if cost > s8(s[0x1A5]):
                    continue
                line = w.ext("fire_line", s, c, 0) & 0xFF
                if not line:
                    continue
                if any(_i32(w.ext("tile_dist", s16(a), c)) <= 4 for a in ally_tiles):
                    continue
                dmg = 0
                for ot, ok, power in targets:
                    dist = w.ext("tile_dist", s16(ot), c) & 0xFF
                    if dist <= 3:
                        hit = throw_damage(s, w.rec(ok), w, c) & 0xFF
                        dmg = (dmg + (cdiv((100 - dist * 20) * hit, 100) & 0xFF) * power) & 0xFFFF
                tac = throw_ap_cost(s, w)
                best = aim = chance = 0
                for a in range(min(s8(s[0x1A5]) - cost, 4) + 1):
                    ch = throw_chance(s, w, c, a) & 0xFF
                    ch = (ch + ch // 2) & 0xFF
                    ch = min(ch, 100)
                    v = s16(cdiv(s8(s[0x1A5]) * ch, tac + a))
                    if v > best:
                        best, aim, chance = v, a, ch
                if not best:
                    continue
                eff = (chance * line // 100) & 0xFF
                if not eff:
                    continue
                score = _i32(_i32(eff * best) * s16(dmg))
                if score <= _i32(u16(rec, 4) | u16(rec, 6) << 16):
                    continue
                rec[0], rec[1], rec[2], rec[3] = 1, k, aim, eff
                rec[4:8] = (score & 0xFFFFFFFF).to_bytes(4, "little")
                w16(rec, 8, c)
    return bytes(rec)


def cheb_px(ax, ay, bx, by):
    """0x1FD8C: max(|dx|, |dy|) in pixels"""
    return max(abs(ax - bx), abs(ay - by))


def line_of_fire(s, w, tile, thrown):
    """LineOfFire 0x580CC: % chance a shot (or, with `thrown` / a kind-0x95 main-hand item, a throw) from the soldier
    reaches `tile`; 100 for its own tile. Bresenham walk in pixels from (x+2, y+4) (+0x116/+0x118) to the tile's
    (x+2, y+4); height: bullets start at 4500 + Random(1001) sloping toward 4000 over the distance (max(|dx|, |dy|)),
    throws start at 8000 rising to a cap (9000 if +0x1B3 == 3, else 22000) and then falling. Each new tile other
    than the soldier's own: off the map (OffMapStep) -> 0; ProjectileHitTest (stubbed: 'hit_test', with the skip
    byte = distance if 0x58760 sees a line, chance in/out) stops -> the % so far if it is the target, else 0;
    otherwise % = % x chance / 100."""
    from combat import view_tile
    me = s16(u16(s, 0x114))
    if me == tile:
        return 100
    tx, ty = tile_x(tile) + 2, tile_y(tile) + 4
    x, y = s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4
    d = _i32(w.ext("direction", x, y, tx, ty)) & 0xFF
    dist = cheb_px(x, y, tx, ty)
    skip = 0
    if _i32(w.ext("sight_line", s, x, y, tile, tx, ty, 0, 0xE, 0x40)):
        skip = cheb_px(x, y, tx, ty) & 0xFF
    item = 1 if thrown & 0xFF else s[0]
    cap = down = 0
    is_throw = w.g8(G_ITEM_KIND + 12 * item) == 0x95
    if is_throw:
        cap = 9000 if s[0x1B3] == 3 else 22000
        h = 8000
        slope = cdiv(cdiv((cap - 8000) * 2, cdiv(dist, 2)) + 1, 2)
        down = cdiv(cdiv(-cap * 2, cdiv(dist, 2)) - 1, 2)
    else:
        h = w.random(0x3E9) + 4500
        e = 4000 - h
        slope = cdiv(cdiv(e * 2, dist) + (1 if e >= 0 else -1), 2)
    ax, ay = tx - x, ty - y
    sx, sy = (-1 if ax < 0 else 1), (-1 if ay < 0 else 1)
    ax, ay = abs(ax), abs(ay)
    if ax >= ay:
        major, minor, mx, my = ax, ay, sx, 0
    else:
        major, minor, mx, my = ay, ax, 0, sy
    err = 2 * minor - major
    prev = view_tile(x, y)
    flown, pct = 0, 100
    while True:
        if err < 0:
            x, y, err = x + mx, y + my, err + 2 * minor
        else:
            x, y, err = x + sx, y + sy, err + 2 * minor - 2 * major
        t = view_tile(x, y)
        flown += 1
        h = max(h + slope, 0)
        if cap and h > cap:
            h = cap
            slope = down if is_throw else 0
        if t != prev and t != me:
            if off_map_step(prev, t):
                return 0
            prev = t
        if t != me:
            res, skip, chance = w.ext("hit_test", h, x, y, flown, d, skip, cap, s16(tile), 1, 100)   # chance output starts at 100
            if _i32(res):
                return pct if t == tile else 0
            pct = cdiv(pct * _i32(chance), 100)
        if t == tile:
            return pct


G_WINDOW_TBL = 0x95788        # u8 [object * 8 + direction] (ProjectileHitTest; SightLine opacity of objects 0x37..0x82)
G_SIGHT_BLOCK = 0x9567C       # i32 56: opacity a sight line may gather inside one tile


def tile_opacity(w, tile, d, steps, x, y):
    """0x23818 -> (u8 opacity, brush): by the tile's object t1: 0x37..0x82 G_WINDOW_TBL[t1 * 8 + direction];
    0x27..0x36 (brush / trees, brush = 1): steps / 4 clamped to 2..8 is a radius around the tile's (x+2, y+4), and
    the line point (x, y) inside it (ChebPx) gives steps / 15 x 2, outside 0; 0xE2 / 0xE3: steps / 15 / 2; else 0."""
    t1 = w.tile8(tile, 1)
    if 0x37 <= t1 < 0x83:
        return w.g8(G_WINDOW_TBL + 8 * t1 + d), 0
    if 0x27 <= t1 <= 0x36:
        r = min(max(cdiv(steps, 4), 2), 8)
        if cheb_px(x, y, tile_x(tile) + 2, tile_y(tile) + 4) < r:
            return cdiv(steps, 15) * 2 & 0xFF, 1
        return 0, 1
    if t1 in (0xE2, 0xE3):
        return cdiv(cdiv(steps, 15), 2) & 0xFF, 0
    return 0, 0


def sight_line(s, w, x, y, tile, tx, ty, flag, rng, slot):
    """SightLine 0x58760 -> 1 if the soldier at pixel (x, y) sees pixel (tx, ty) (`tile`'s), within `rng` tiles.
    Seeing a soldier `slot` (< 0x40) whose +0x1B1 is set and whose +0x1B3 is 1, 2, 4, 5, 6 or 7 cuts the range by
    30 % unless `flag` (the old sight flag) is set. Bresenham in pixels, at most range x 15 steps (16-bit). On each new
    tile the opacity (tile_opacity) is taken and the tile's sum restarts; 0x62 and up are windows: they set the step
    limit (steps + 30 for 0x63 within 15 steps; past 15 steps steps + 30 for 0x62, else the current step) and count
    3. Each step on a tile with opacity or brush adds to the tile's sum (brush: the opacity again for the new point;
    the third step in a brush tile also takes 15 off the limit, not below 0); a sum above G_SIGHT_BLOCK blocks the
    line, unless `flag` and LineOfFire(s, tile, thrown) >= 25 (then seen; 0x95680 is cleared first)."""
    from combat import view_tile
    d = _i32(w.ext("direction", x, y, tx, ty)) & 0xFF
    if (slot & 0xFF) < 0x40 and w.g8(G_SOLDIERS + 0x200 * (slot & 0xFF) + 0x1B1) and not flag & 0xFF:
        if w.g8(G_SOLDIERS + 0x200 * (slot & 0xFF) + 0x1B3) in (1, 2, 4, 5, 6, 7):
            rng = _i32(rng - cdiv(_i32(rng * 30), 100))
    limit = _i32(rng * 15)
    ax, ay = _i32(tx - x), _i32(ty - y)
    sx, sy = (-1 if s16(ax) < 0 else 1), (-1 if s16(ay) < 0 else 1)
    ax, ay = (_i32(-ax) if sx < 0 else ax), (_i32(-ay) if sy < 0 else ay)
    if s16(ax) < s16(ay):
        major, minor, mx, my = ay, ax, 0, sy
    else:
        major, minor, mx, my = ax, ay, sx, 0
    err = _i32(2 * minor - major)
    diag = _i32(err - major)
    prev = view_tile(x, y) & 0xFFFF
    steps = total = count = opa = brush = 0
    while True:
        if s16(err) < 0:
            x, y, err = x + mx, y + my, _i32(err + 2 * minor)
        else:
            x, y, err = x + sx, y + sy, _i32(err + diag)
        cur = view_tile(x, y) & 0xFFFF
        steps = (steps + 1) & 0xFFFF
        if s16(steps) > s16(limit):
            return 0
        if cur != prev:
            brush = count = total = 0
            opa, brush = tile_opacity(w, s16(cur), d, s16(steps), x, y)
            if opa >= 0x62:
                if s16(steps) > 15:
                    limit = steps + 30 if opa == 0x62 else steps
                elif opa == 0x63:
                    limit = steps + 30
                opa = 3
            if brush:
                count += 1
        if opa or brush:
            if brush and s16(count) >= 0:
                count = (count + 1) & 0xFFFF
                if s16(count) > 4:
                    limit, count = _i32(limit - 15), 0xFFFF
                    if s16(limit) < 0:
                        limit = 0
            if prev == cur and brush:
                total = (total + tile_opacity(w, s16(cur), d, s16(steps), x, y)[0]) & 0xFFFF
            else:
                total = (total + opa) & 0xFFFF
            if s16(total) > _i32(w.g32(G_SIGHT_BLOCK)):
                if not flag & 0xFF:
                    return 0
                w.s8(0x95680, 0)
                return int(_i32(w.ext("fire_line", s, tile, 1)) >= 25)
        if x == tx and y == ty:
            return 1
        prev = cur


def _occupant(w, tile):
    """TileOccupant 0x23944"""
    v = w.tile8(tile, 5) & 0x7F
    return 0x40 if v >= 0x30 else v


ANIM_DELAY = 0x93164         # u8 per animation x 8 (column -2): fixed ticks per frame, 0 = computed (AnimDelay)
ANIM_AP = 0x93162            # u8 per animation x 8 (column -4): AP the animation costs (99 = free, no check)
ANIM_FREE = 0x93095          # u8 per animation: != 0 = no AP check
ANIM_SOUND = 0x930DA         # i16 per animation: sound argument
ANIM_FRAMES = 0x106C18       # 0x50 B per animation: frame table (soldier +0x90 points at the current one)
TERRAIN_DELAY = {1: 1, 2: 1, 4: 2, 5: 2, 6: 4, 7: 4, 8: 6, 9: 6}   # +0x1B3 terrain class -> extra ticks (x2)


def msg_not_enough_ap(s, w, ap, flag):
    """MsgNotEnoughAP 0x15960 -> 0 if the selected soldier (or any, with flag 99) in combat mode has fewer AP
    (+0x1A5 + +0x1E2) than `ap`: a merc gets the "doesn't have enough Action Points!" text, others end their turn
    (+0x1B0 = 1). Else 1."""
    if (w.g8(G_SELECTED) == s[0x17A] or s8(flag & 0xFF) == 99) and w.g8(G_COMBAT) and \
            s8(s[0x1E2]) + s8(s[0x1A5]) < (ap & 0xFF):
        if s[0x17A] < 8:
            w.ext("merc_text", s, "doesn't have enough Action Points!", 0x4B0)
        else:
            s[0x1B0] = 1
        return 0
    return 1


def anim_delay(s, w):
    """AnimDelay 0x15BB8: ticks per frame of the current animation into +0xD2. The table value if not 0. Standing
    (4) and wading (0x33, 0x36): (100 - hp) x 10 + energy x 10, at least 50. Wading deep (0x39): 4 x max(25, 100 -
    energy) + max(25, 100 - hp) (bytes). Others: a base (50 for 2, 0xD, 0x1A, 0x1D..0x1E, 0x21..0x22, 0x25, 0x3A;
    100 for 0x17; 40 for 0x34; 80 for 0x37; the soldier's +0xA4 for 0x16; else 0) + 40 while +0x1B8 is set, else
    2 x the terrain extra (+0x1B3) + (100 - hp clamped 15..50) + for mercs (100 - energy, at most 50) + (100 -
    +0x18B clamped 20..70), for others 20. Also +0x19F = 0 for animations 0x1A..0x27, else 2."""
    a = u16(s, 0x1E8 + 2 * s[0x1F3])
    d = w.g8(ANIM_DELAY + 8 * s16(a))
    w16(s, 0xD2, d)
    if d:
        return
    if a in (4, 0x33, 0x36):
        w16(s, 0xD2, (100 - s8(s[0x183])) * 10 + s8(s[0x18A]) * 10)
        if s16(u16(s, 0xD2)) < 0x32:
            w16(s, 0xD2, 0x32)
        return
    if a == 0x39:
        hi = s8((100 - s[0x183]) & 0xFF)
        lo = s8((100 - s[0x18A]) & 0xFF)
        w16(s, 0xD2, 4 * max(lo, 25) + max(hi, 25))
        return
    if a in (2, 0xD, 0x1A, 0x1D, 0x1E, 0x21, 0x22, 0x25, 0x3A):
        base = 0x32
    elif a == 0x16:
        base = _i32(u16(s, 0xA4) | u16(s, 0xA6) << 16)
    else:
        base = {0x17: 100, 0x34: 0x28, 0x37: 0x50}.get(a, 0)
    s[0x19F] = 0 if 0x1A <= s16(a) <= 0x27 else 2
    extra = 0x28 if s[0x1B8] else 2 * TERRAIN_DELAY.get(s[0x1B3], 0)
    if s[0x17A] >= 0x28:
        tired, wis = 0, 0x14
    else:
        tired = min(s8((100 - s[0x18A]) & 0xFF), 0x32)
        wis = max(min(s8((100 - s[0x18B]) & 0xFF), 0x46), 0x14)
    hurt = min(max(s8((100 - s[0x183]) & 0xFF), 0xF), 0x32)
    w16(s, 0xD2, hurt + tired + wis + base + extra)


def set_anim(s, w, code, keep, reset):
    """SetAnim? 0x17F90: start animation `code`. Nothing when the current animation cannot act, its frame count (first
    word of the frame table at +0x90) is <= 0x15E and not `reset`. The same animation: recompute the delay
    (AnimDelay) and, with `keep`, point +0x90 at its frames. Else (the selected merc, codes other than 9 / 10: a UI
    refresh 0x1D8B8 first): standing up (codes 4 / 0x15) from a lower posture queues a transition animation (and a
    second one from animation 0) that must pass the AP check (unless free) or nothing happens; code 0x43 from
    animation 1 flips the facing (0x724E0). Otherwise with `reset` the queue restarts; codes below 2 or 0x3C+ clear
    +0x195; the animation needs its AP (MsgNotEnoughAP, unless free or cost 99) and starts with its sound (code 0x24
    seen by the player also plays effect 11). Then walking codes 0x15 / 0x34 / 0x37 / 0x3A start the step sound
    (0x335F8), the delay is recomputed and +0x90 points at the frames. Last +0x1AD = 0 and the frame routine 0x18498
    runs."""
    code &= 0xFFFF
    cur = lambda: u16(s, 0x1E8 + 2 * s[0x1F3])
    if not w.g8(ANIM_CAN_ACT + 8 * cur()) and w.raw16(_u32(s, 0x90)) <= 0x15E and not reset & 0xFF:
        return
    if code == cur():
        anim_delay(s, w)
        if keep & 0xFF:
            _w32(s, 0x90, w.ptr(ANIM_FRAMES + cur() * 0x50))
        s[0x1AD] = 0
        w.ext("anim_frame", s)
        return
    if w.g8(G_SELECTED) == s[0x17A] and code not in (9, 10):
        path_marks_off(w)
    chain, first, second, posture = False, 1000, 1000, False
    if code in (4, 0x15):
        c = cur()
        if c == 0:
            chain, first, second = True, 0xD, 2
        elif c == 1:
            chain, first = True, 0xD
        elif c == 5:
            chain, first = True, 7
        elif c == 0xF:
            chain, first = True, 0x10
        elif c in (0x17, 0x18):
            chain, first = True, 0x19
            s[0x177] = 0
        elif 0x1A <= c <= 0x1C:
            chain, first, posture = True, 0x1D, True
        elif 0x1E <= c <= 0x20:
            chain, first, posture = True, 0x21, True
        elif 0x22 <= c <= 0x24:
            chain, first = True, 0x25
    elif code == 0x43 and cur() == 1:
        d = (s[0x1A1] + 4) & 0xFF
        if d > 8:
            d = (d - 8) & 0xFF
        s[0x1A2] = s[0x1A1] = d
        w.ext("anim_turn", s, 0)
    ok = lambda a: w.g8(ANIM_FREE + a) or msg_not_enough_ap(s, w, w.g8(ANIM_AP + 8 * a), 0)
    snd = lambda a: w.ext("spend_ap", s, s8(w.g8(ANIM_AP + 8 * a)), s16(w.g16(ANIM_SOUND + 2 * a)) & 0xFFFFFFFF)
    if chain:
        pick = 1000
        if second != 1000:
            if ok(second):
                pick = second
        elif ok(first):
            pick = first
        if pick == 1000:
            return
        w16(s, 0x1E8 + 2 * s[0x1F3], code)
        s[0x1F3] = (s[0x1F3] + 1) & 0xFF
        w16(s, 0x1E8 + 2 * s[0x1F3], first)
        s[0x1FE] = 0
        if second != 1000:
            w16(s, 0x1E8 + 2 * s[0x1F3], first)
            s[0x1F3] = (s[0x1F3] + 1) & 0xFF
            w16(s, 0x1E8 + 2 * s[0x1F3], second)
            s[0x1FE] = 0
        snd(pick)
        if posture:
            w.ext("reserve_ap", s)
    else:
        if reset & 0xFF:
            s[0x1F3] = 0
        if code < 2 or code >= 0x3C:
            s[0x195] = 0
        if s8(w.g8(ANIM_AP + 8 * code)) == 99:
            w16(s, 0x1E8 + 2 * s[0x1F3], code)
            s[0x1FE] = 0
            snd(code)
        else:
            if not ok(code):
                return
            w16(s, 0x1E8 + 2 * s[0x1F3], code)
            s[0x1FE] = 0
            snd(code)
            if code == 0x24 and s8(s[0x9C]) > 0:
                w.ext("sfx", 0xB, 0x2B11, 0x58, 1)
    if code in (0x15, 0x34, 0x37, 0x3A):
        w.ext("walk_sound", s[0x17A], 1)
    anim_delay(s, w)
    _w32(s, 0x90, w.ptr(ANIM_FRAMES + cur() * 0x50))
    s[0x1AD] = 0
    w.ext("anim_frame", s)


def anim_end(s, w):
    """AnimEnd 0x1F0F0 (script word 0x1F3, the end of every animation script): with a queued animation before the
    current one (+0x1F3 > 0) the soldier goes back to it: AP check of that animation (unless free; cost 99 counts as
    0), the current entry becomes 1000, the queue index drops, +0x90 points at its frames (the code is the entry's
    low byte), walking codes start the step sound, standing (4) after 0xD places the soldier again, delay +
    frame routine, its start sound; an animation with the still flag (row column 3) that is not at its step point
    goes on with AnimStep. An empty queue: AnimStep. (A second exit through ReplaceSoldier / StopAnim / StopWalk is
    dead code: its flag is never set.)"""
    idx = s[0x1F3]
    if idx == 0:
        w.ext("anim_step", s)
        return
    prev = u16(s, 0x1E6 + 2 * idx)
    ap = w.g8(ANIM_AP + 8 * prev)
    if ap == 0x63:
        ap = 0
    if not w.g8(ANIM_FREE + prev) and not msg_not_enough_ap(s, w, ap, 0):
        return
    from_d = u16(s, 0x1E8 + 2 * idx) == 0xD
    w16(s, 0x1E8 + 2 * idx, 0x3E8)
    s[0x1F3] = idx = (idx - 1) & 0xFF
    code = s[0x1E8 + 2 * idx]
    _w32(s, 0x90, w.ptr(ANIM_FRAMES + code * 0x50))
    if code in (0x15, 0x34, 0x37, 0x3A):
        w.ext("walk_sound", s[0x17A], 1)
    if code == 4 and from_d:
        w.ext("replace", s)
    s[0x1FE] = 0
    anim_delay(s, w)
    s[0x1AD] = 0
    w.ext("anim_frame", s)
    w.ext("spend_ap", s, s8(w.g8(ANIM_AP + 8 * code)), s16(w.g16(ANIM_SOUND + 2 * code)) & 0xFFFFFFFF)
    if w.g8(ANIM_AP + 8 * code + 3) and not (u16(s, 0x126) == u16(s, 0x116) and u16(s, 0x128) == u16(s, 0x118)):
        w.ext("anim_step", s)


ANIM_STEPS_TILE = 0x93051    # u8 per animation: != 0 = a step into a new tile snaps the step target to its point
ANIM_TURN_WHILE = 0x93382    # u8 per animation: != 0 = AnimNext turns one step toward the turn target
G_VIEW_X, G_VIEW_Y = 0x229CF8, 0x229CFC
G_VIEW_W, G_VIEW_H = 0x229CBC, 0x229CC0
SET_FACING = {0x1CD: 1, 0x1CE: 2, 0x1CF: 3, 0x1D0: 4, 0x1D1: 5, 0x1D2: 6, 0x1D3: 7, 0x1D4: 8}
STEP_PX = {0x1D7: 1, 0x1D8: 2, 0x1D9: 3, 0x1DA: 4, 0x1DB: -1, 0x1DC: -2, 0x1DD: -3, 0x1DE: -4}
FRAME_SFX = {5: (0x1B, 0xF, 1, "flag"), 0xA: (0x1B, 0xF, 1, "flag"), 0x2D: (0x12, 0x32, 1, "seen"),
             0x57: (0xD, 0x32, 1, ""), 0x85: (0xD, 0x32, 1, ""), 0x8F: (0x1C, 0x1E, 1, "seen"),
             0x98: (0x1C, 0x1E, 1, "seen"), 0x93: (0x1A, 0x1E, 1, "seen"), 0x96: (0x1B, 0x32, 1, "seen"),
             0x1B8: (0x1F, 0x32, 1, "screen"), 0x1B9: (0x21, 0x32, 1, "screen"), 0x1BA: (0x1E, 0x32, 1, "screen"),
             0x1BB: (0x1F, 0x32, 2, "screen"), 0x1BC: (0x19, 0x32, 1, "screen")}


def wrap_dir(d):
    """0x1FD10: direction byte wrapped into 1..8 (signed)"""
    d = s8(d & 0xFF)
    return (d - 8) & 0xFF if d > 8 else (d + 8) & 0xFF if d < 1 else d & 0xFF


VIEW_DIRS = {1: (8, 2, 1, 7, 3), 2: (1, 3, 2, 8, 4), 3: (2, 4, 3, 1, 5), 4: (3, 5, 4, 2, 6),
             5: (4, 6, 5, 3, 7), 6: (5, 7, 6, 4, 8), 7: (6, 8, 7, 5, 1), 8: (7, 1, 8, 6, 2)}   # 0x147E4 by facing
G_VIEW_RAYS_ODD, G_VIEW_RAYS_EVEN = 0x93554, 0x935ED   # 17 rays x 9 bytes: VIEW_DIRS index per step
G_SPOT_STOP, G_STORY_FLAG = 0x92E64, 0x22A008
ROOF_OPEN = {0x12: (0x53, 0x52, 0x54, 0x53, 9), 0x13: (0x5E, 0x5D, 0x5F, 0x5E, 0x14),
             0x14: (0x69, 0x68, 0x6A, 0x69, 0x1F)}   # t2 class of t3: north keep above, south needs, roof above, 0x4C, + t2


FAN_FIXES = set()   # #70: fan fix ids the diff test turns on (ai_diff --fan); the mirror is 1.13b otherwise


def reveal_view(facing, rng_, s, w, think_):
    """0x1491C (AL = facing, DL = range, EBX = soldier, CL = think after): a merc's view sweep. 17 rays (9 steps each
    in the table for odd or even facings, 0x93554 / 0x935ED) step from the soldier's tile through the five
    directions around the facing (0x147E4); a ray stops at the map edge, after range steps, when its budget (99, or
    2 when a 0xB object sits on the first step) runs out, or after the object blocking (ObjBlock 0x23780, against the
    side direction while a second direction exists for the step; table value 5 = past the five directions: as the
    step it ends the ray, as the side it passes the tile number) reaches 2, or one step after a 1. With the mine
    detector (item 0x4D in hand 0, condition >= 10, +0x177 set) the first two steps find buried mines by Random(100)
    < condition on object-free tiles (0x19D74; a trap 0x11 practises stat 6); without it +0x177 clears. Each tile
    with blocking below 3 is revealed (byte 4 bit 7), spends budget, and in turn-based combat (0x92DCC, not 0x92DD0,
    +0x1D5 0) a hidden ground item (+6 = 0) becomes seen and the first one stops the merc (quote 3, 0x197D8,
    ClearTask; walking: 0x92E64, +0x16D, 0x1148C) and practises stat 4. Roofs open (t3 by its t2 class 0x12..0x14,
    0x73 = 0 with the move code), object 0x57 clears the north roof, story sector tiles 0xAC..0xAF note the merc
    (0x92DE3, hp >= 15, unless 0x1F6E4), sector 0 tile 0x279 clears 0x22A008. Then +0x9D = 0, think if CL."""
    if s[0x17A] < 8:
        from combat import move_code
        dirs = VIEW_DIRS.get(facing & 0xFF, (0, 0, 0, 0, 0))
        steps = [dir_step(w, d) for d in dirs] + [0]   # index 5 = past the five: step 0 (ends the ray)
        rays = G_VIEW_RAYS_EVEN if (facing & 0xFF) % 2 == 0 else G_VIEW_RAYS_ODD
        found = 0
        for r in range(0, 0x99, 9):
            budget, after1, tile = 99, 0, s16(u16(s, 0x114))
            for k in range(rng_ & 0xFF):
                a = w.g8(rays + r + k)
                b = w.g8(rays + r + k + 1) if k < 8 else 99
                nt = step_tile(tile, steps[a])
                if nt == tile:
                    break
                tile = nt
                m = G_TILEMAP + 8 * tile
                gi = w.g8(m + 6)
                if s[0x177] and k <= 1 and gi < 200 and w.g8(G_GROUND_ITEMS + 12 * gi + 6) == 0xFF \
                        and w.g8(ITEM_TBL + 12 * w.g8(G_GROUND_ITEMS + 12 * gi) + 12):
                    if s[0] == 0x4D and s8(s[2]) >= 10:
                        if w.random(100) < s8(s[2]) and w.g8(m + 1) == 0:
                            w.ext("mine_flag", s, tile)
                            if w.g8(G_GROUND_ITEMS + 12 * gi) == 0x11 and s8(w.g8(G_GROUND_ITEMS + 12 * gi + 2)) < 0:
                                w.ext("stat_practice", s, 6, 2, 0)
                    else:
                        s[0x177] = 0
                side = NO_TILE if b == 99 else tile if b == 5 else dirs[b]   # index 5 reads the ray's tile
                blk = s8(obj_block(w, tile, dirs[a], side))
                if blk == 0xB and k < 2:
                    if k < 1:
                        blk, budget = 0, 2
                    else:
                        blk = 1
                if blk < 3:
                    w.s8(m + 4, w.g8(m + 4) | 0x80)
                    if not s[0x1D5] and not w.g8(0x92DD0) and w.g8(0x92DCC) and gi < 200 \
                            and not w.g8(G_GROUND_ITEMS + 12 * gi + 6):
                        w.s8(G_GROUND_ITEMS + 12 * gi + 6, 1)
                        if not found:
                            w.ext("quote_event", s[0x17A], 3, 0, tile, 1)
                            w.ext("w_197d8", s, 0)
                            clear_task(s, 0)
                            if not s[0x16D] and s16(u16(s, 0x120)) != NO_TILE and u16(s, 0x120) != u16(s, 0x114):
                                w.s8(G_SPOT_STOP, 1)
                                s[0x16D] = 1
                                if 61 in FAN_FIXES:   # #70 F61 (1.15m MercView 0x14C75 -> cave 0x8AF04)
                                    w.ext("w_53660")
                                w.ext("w_1148c", s[0x17A], 1, 0)
                            w.ext("stat_practice", s, 4, 1, 0)
                            found = 1
                    t3 = w.g8(m + 3)
                    c = w.g8(G_T2_CLASS + t3)
                    if c >= 0x12:
                        if t3 == 0x73:
                            w.s8(m + 3, 0)
                            if w.g16(G_MOVECODE + 2 * tile) == 0x4B1:
                                w.s16(G_MOVECODE + 2 * tile, move_code(
                                    w.g8(m), w.g8(m + 1), 0, 0x4B1, [w.g8(G_T0_CLASS + i) for i in range(256)],
                                    [w.g8(G_T1_CLASS_TBL + i) for i in range(256)]))
                        if c in ROOF_OPEN:
                            keep, south, above, roof4c, add = ROOF_OPEN[c]
                            t2 = w.g8(m + 2)
                            if w.g8(m - 8 * 56 + 3) <= keep:
                                w.s8(m - 8 * 56 + 3, 0)
                            if t2 in (0x49, 0x4A, 0x4C) and w.g8(m + 8 * 56 + 3) >= south:
                                if w.g8(m + 3) > above:
                                    w.s8(m + 3, roof4c if t2 == 0x4C else t2 + add & 0xFF)
                            else:
                                w.s8(m + 3, 0)
                    if w.g8(m + 1) == 0x57 and w.g8(m - 8 * 56 + 3) in (0x53, 0x5E, 0x69):
                        w.s8(m - 8 * 56 + 3, 0)
                    if not w.g8(0x22A056) and w.g8(G_CUR_SECTOR) == s8(w.g8(0x22A057)):
                        if 0xAC <= w.g8(m + 1) <= 0xAF and not w.ext("w_1f6e4") & 0xFF and s8(s[0x183]) >= 15:
                            w.s8(0x92DE3, s[0x17A])
                    elif not w.g8(G_CUR_SECTOR) and w.g32(G_STORY_FLAG) and tile == 0x279:
                        w.s32(G_STORY_FLAG, 0)
                    budget -= 1
                if budget <= 0 or after1 == 1:
                    break
                if blk > 0:
                    if blk > 1:
                        break
                    after1 = 1
    s[0x9D] = 0
    if think_ & 0xFF:
        think(s, w)


def anim_turn(s, w, flag):
    """0x724E0: a running AI turn (action 1) is done: action and running flag cleared, target none; with `flag` the
    facing is reported (0x1491C), an enemy reacts (0x73578), a merc/guard reacts (0x7351C) and shares sightings."""
    if s[0x1D1] == 1 and s[0x1D6]:
        s[0x1D6] = s[0x1D1] = 0
        w16(s, 0x132, NO_TILE)
        if flag & 0xFF:
            reveal_view(s[0x1A1], s[0x19E], s, w, 0)
            if s[0x17A] >= 0x28:
                w.ext("enemy_turned", s)
            elif s[0x17A] < 0x10:
                w.ext("guard_turned", s)
                w.ext("share_sightings", s, 0)


def turn_one(s, w):
    """0x14F7C: facing one step (TurnStep) toward the turn target, wrapped to 1..8; reaching it reports (0x1491C)"""
    s[0x1A1] = (s[0x1A1] + turn_step(s8(s[0x1A1]), s8(s[0x1A2]))) & 0xFF
    if s8(s[0x1A1]) > 8:
        s[0x1A1] = 1
    elif s8(s[0x1A1]) < 1:
        s[0x1A1] = 8
    if s[0x1A1] == s[0x1A2]:
        reveal_view(s[0x1A1], s[0x19E], s, w, 0)


def rotate(s, w, cw, now):
    """0x14FEC: turn target = facing +1 (cw) / -1, wrapped; with `now` the facing follows at once (anim_turn 1)"""
    s[0x1A2] = wrap_dir(s[0x1A1] + (1 if cw & 0xFF else -1))
    if now & 0xFF:
        s[0x1A1] = s[0x1A2]
        w.ext("anim_turn", s, 1)


def on_screen(s, w):
    """0x1FC34: 1 unless option 0x92DDF is set and the soldier's sprite point lies outside the view (20 px margin)"""
    if not w.g8(0x92DDF):
        return 1
    y = s16(u16(s, 0x118)) if s16(u16(s, 0x114)) < 0x38 else s16(u16(s, 0x118)) - s16(u16(s, 0x11C))
    x = s16(u16(s, 0x116)) - s16(u16(s, 0x11A))
    vx, vy = _i32(w.g32(G_VIEW_X)), _i32(w.g32(G_VIEW_Y))
    if x + 20 < vx or y + 20 < vy or x > vx + _i32(w.g32(G_VIEW_W)) - 1 or y > vy + _i32(w.g32(G_VIEW_H)) - 1:
        return 0
    return 1


def occupy(w, tile, slot):
    """0x23974: 1 if the tile (<= 0xC40) is free or the soldier's: takes it (byte 5 low 7 bits = slot, flag 0x80 kept)"""
    if tile > 0xC40:
        return 0
    occ = _occupant(w, tile)
    if occ != slot:
        if occ < 0x30:
            return 0
        w.s8(G_TILEMAP + 8 * tile + 5, slot | (w.tile8(tile, 5) & 0x80))
    return 1


def vacate(w, tile):
    """0x2395C: tile occupant = none (0x40), flag 0x80 kept"""
    w.s8(G_TILEMAP + 8 * tile + 5, (w.tile8(tile, 5) & 0x80) | 0x40)


def next_path_dir(s, w):
    """0x195F8: direction to walk next: path step +0x1AF + 1 while the path has steps; else, not at the step point:
    another tile -> a new leg to the step target (path step + 1), same tile -> DirectionTo the step point; at it:
    the facing"""
    row = G_PATHS + 0x50 * s[0x17A]
    if s16(u16(s, 0xD4)) > 0:
        return (w.g8(row + s[0x1AF]) + 1) & 0xFF
    if not at_waypoint(s):
        if u16(s, 0x114) != u16(s, 0x124):
            w16(s, 0x12A, u16(s, 0x124))
            start_leg(s, w)
            return (w.g8(row + s[0x1AF]) + 1) & 0xFF
        return _i32(w.ext("direction", s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(u16(s, 0x126)),
                          s16(u16(s, 0x128)))) & 0xFF
    return s[0x1A1]


def _blocked(w, tile):
    return tile_standable(w, tile) < 1 or is_water_tile(w.tile8(tile, 0), w.tile8(tile, 1))


def anim_frame(s, w):
    """AnimFrame? 0x18498: run the animation script at +0x90 (16-bit words). Frame numbers below 0x1B6 are shown
    (some play a sound first and go on); words 0x1B6..0x1F3 are commands (facing, turning, pixel steps 0x1D7..0x1DE,
    branches on the turn way, door / mine / item / projectile / death effects, ...). A command that goes on calls
    AnimNext; a pixel step moves the soldier along its facing (diagonals 65 %), and entering a new tile takes it
    when standable and free (else the step is undone)."""
    p = _u32(s, 0x90)
    op = w.raw16(p)
    go, step = True, 0
    seen = s8(s[0x9C]) > 0
    if op in FRAME_SFX:
        n, vol, loops, cond = FRAME_SFX[op]
        if (cond == "" or (cond == "flag" and w.g8(0x1163C3) and seen) or (cond == "seen" and seen)
                or (cond == "screen" and seen and on_screen(s, w))):
            w.ext("sfx", n, 0x2B11, vol, loops)
    elif op == 0x37:
        if s8(s[0x1A3]) == -1:
            turn_one(s, w)
        go = False
    elif op == 0x54:
        w.ext("w_17b70", s[0x17A], 3)
    elif op == 0x1B6:
        w.s32(0x229E04, 0xFFFFFFFF)
    elif op == 0x1B7:
        if w.g8(G_COMBAT):
            w.ext("w_3f490", s16(w.g16(0x92E4C)), 0)
    elif op == 0x1BD:
        s[0x1BE] = 0
    elif op == 0x1BE:
        s[0x1FE] = 1
    elif op == 0x1BF:
        _w32(s, 0x90, p + 2)
        go = False
        w.ext("quote", s, s16(u16(s, 0x134)))
    elif op == 0x1C0 or op == 0x1E4 or op == 0x1ED:
        pass
    elif op == 0x1C1:
        _w32(s, 0x90, p + 2)
        go = False
        w.ext("w_58574", s)
    elif op == 0x1C2:
        go = False
        _w32(s, 0x90, p + 2)
        if s[0x1A6] == 0x4C and _i32(w.ext("quest_check", s16(u16(s, 0x134)) & 0xFFFFFFFF)) & 0xFF:
            w.ext("use_special", s)
        else:
            w.ext("door_fx", s, s16(u16(s, 0x134)), s[0x1A6])
        clear_task(s, 1)
    elif op == 0x1C3:
        if s8(s[0x1A2]) not in (3, 5):
            _w32(s, 0x90, p + 10)
    elif op == 0x1C4:
        s[0x9D] = 1
    elif op == 0x1C5:
        if s8(s[0x1A3]) == -1:
            _w32(s, 0x90, p + 2)
    elif op == 0x1C6:
        a = u16(s, 0x1E8 + 2 * s[0x1F3])
        _w32(s, 0x90, w.ptr(ANIM_FRAMES) + a * 0x50 + 2 * w.raw16(p + 2))
        go = False
    elif op == 0x1C7:
        if s[0x1A1] == s[0x1A2]:
            w.ext("anim_step", s)
            w.ext("anim_turn", s, 1)
        else:
            w.ext("set_anim", s, 8, 1, 0)
        go = False
    elif op == 0x1C8:
        if s8(s[0x1A3]) == -1:
            _w32(s, 0x90, p + 4)
    elif op == 0x1C9:
        if s8(s[0x1A3]) == 1:
            _w32(s, 0x90, p + 2)
    elif op == 0x1CA:
        if s8(s[0x1A3]) == 1:
            turn_one(s, w)
    elif op == 0x1CB:
        if w.random(0xC) == 0 and s8(s[0x18A]) > 0x46 and not s[0x1B9] and not s[0x185]:
            w.ext("set_anim", s, 9 if s8(s[0x1A1]) % 2 == 0 else 10, 0, 0)
            go = False
    elif op == 0x1CC:
        go = False
    elif op in SET_FACING:
        s[0x1A1] = s[0x1A2] = SET_FACING[op]
    elif op in (0x1D5, 0x1D6):
        w.ext("popups", s, 1 if op == 0x1D5 else 0)
        w.ext("stop_walk", s, 0)
    elif op in STEP_PX:
        step = STEP_PX[op]
    elif op == 0x1DF:
        s[0x1A2] = next_path_dir(s, w)
    elif op in (0x1E0, 0x1E2):
        rotate(s, w, 1, 1)
    elif op in (0x1E1, 0x1E3):
        rotate(s, w, 0, 1)
    elif op == 0x1E5:
        me = s16(u16(s, 0x114))
        back = (s[0x1A1] + 4) & 0xFF
        back = (back - 8) & 0xFF if back > 8 else back
        if _blocked(w, step_tile(me, dir_step(w, back))) or _blocked(w, step_tile(me, dir_step(w, s[0x1A1]))):
            w.ext("set_anim", s, 0x2B, 0, 1)
            go = False
    elif op == 0x1E6:
        if s[0x17D] == s[0x1A1]:
            s[0x17D] = 0
            w16(s, 0x136, 0)
            w.ext("set_anim", s, 1, 1, 1)
            go = False
        else:
            if not s[0x17D]:
                s[0x17D] = s[0x1A1]
            me = s16(u16(s, 0x114))
            if _blocked(w, step_tile(me, dir_step(w, s[0x1A1]))) or (
                    s8(s[0x1A1]) == 4 and w.tile8(me + 1, 1) >= 0x83):
                rotate(s, w, 1, 1)
                w16(s, 0x136, 0x28)
                go = False
            else:
                w16(s, 0x136, 0)
    elif op == 0x1E7:
        d = (s[0x1A1] + 4) & 0xFF
        s[0x1A2] = s[0x1A1] = (d - 8) & 0xFF if d > 8 else d
        w.ext("anim_turn", s, 0)
    elif op == 0x1E8:
        go = False
        _w32(s, 0x90, p + 2)
        w.ext("w_3cdd4", s)
    elif op == 0x1E9:
        go = False
        _w32(s, 0x90, p + 2)
        if u16(s, 0x1E8 + 2 * s[0x1F3]) == 0xC:
            w.ext("w_3cfd0", s, s16(u16(s, 0x134)), s[0x1A6])
        else:
            w.ext("bury_mine", s, s16(u16(s, 0x134)))
    elif op == 0x1EA:
        w.ext("w_401b4", s, 0)
        go = False
    elif op == 0x1EB:
        w.ext("w_3521c", s)
    elif op == 0x1EC:
        w16(s, 0x124, u16(s, 0x114))
        w16(s, 0x120, NO_TILE)
        w.ext("stop_walk", s, 0)
        if s8(s[0x183]) <= 0:
            w.ext("w_3a8c0", s, 0)
            go = False
    elif op == 0x1EE:
        anim_next(s, w)
        t = s16(u16(s, 0xDC))
        if t != NO_TILE and s[1]:
            w.ext("w_5abe0", s, t)
            w.ext("w_5ad50", s, t)
        go = False
    elif op == 0x1EF:
        w.ext("launch", s)
    elif op == 0x1F0:
        s[0x1AD] ^= 1
    elif op == 0x1F1:
        if s8(s[0x183]) < 15:
            w16(s, 0x120, NO_TILE)
            for j in range(0x28):
                s[0x13A + j] = 0
            forget_tiles(w, s)
            s[0x1D5] = 0
            clear_target(s, w, 0)
            w16(s, 0x126, u16(s, 0x116))
            w16(s, 0x128, u16(s, 0x118))
            w16(s, 0x124, u16(s, 0x114))
            if not in_water(s):
                if u16(s, 0x1E8 + 2 * s[0x1F3]):
                    w.ext("set_anim", s, 0x2A, 0, 1)
                    w.ext("end_task", s, 0)
            elif u16(s, 0x1E8 + 2 * s[0x1F3]) < 0x40:
                w.ext("w_3a8c0", s, 0)
            go = False
    elif op == 0x1F2:
        w.ext("merc_dies", s, 0)
        go = False
    elif op == 0x1F3:
        w.ext("anim_end", s, 0)
        go = False
    else:
        go = False
    if not step:
        if go:
            anim_next(s, w)
        return
    ox, oy = u16(s, 0x116), u16(s, 0x118)
    diag = cdiv(step * 65, 100)
    f = s[0x1A1]
    dx = {1: 0, 2: step, 3: step, 4: step, 5: 0, 6: -step, 7: -step, 8: -step}.get(f)
    dy = {1: -step, 2: -diag, 3: 0, 4: diag, 5: step, 6: diag, 7: 0, 8: -diag}.get(f)
    if dx is not None:
        w16(s, 0x116, ox + dx)
        w16(s, 0x118, oy + dy)
    from combat import view_tile
    t = view_tile(s16(u16(s, 0x116)), s16(u16(s, 0x118)))
    if t != s16(u16(s, 0x114)):
        if tile_standable(w, t) < 1 or not occupy(w, t, s[0x17A]):
            w16(s, 0x116, ox)
            w16(s, 0x118, oy)
            anim_next(s, w)
            return
        vacate(w, s16(u16(s, 0x114)))
        w16(s, 0x114, t)
        if w.g8(ANIM_STEPS_TILE + u16(s, 0x1E8 + 2 * s[0x1F3])):
            w16(s, 0x126, tile_x(t))
            w16(s, 0x128, tile_y(t))
            w.ext("stop_anim", s, 1)
        else:
            w.ext("stop_anim", s, 1)
            w16(s, 0x126, u16(s, 0x116))
            w16(s, 0x128, u16(s, 0x118))
            anim_next(s, w)
            return
    anim_next(s, w)


def anim_next(s, w):
    """AnimNext 0x19470: a repeating command (+0x136 != 0) runs again; else frame numbers below 0xB6 (and 0x1CC) are
    remembered in +0x1F4, the script moves on one word and runs it (words 500+ start animation word - 500 instead).
    Then a soldier facing its turn target finishes an AI turn (anim_turn 1); else animations with ANIM_TURN_WHILE
    turn one step."""
    if u16(s, 0x136):
        anim_frame(s, w)
        return
    p = _u32(s, 0x90)
    v = w.raw16(p)
    if v < 0xB6 or v == 0x1CC:
        s[0x1F4] = v & 0xFF
    p += 2
    _w32(s, 0x90, p)
    v = w.raw16(p)
    if v > 0x1F3:
        w.ext("set_anim", s, (v - 0x1F4) & 0xFFFF, 1, 0)
    else:
        anim_frame(s, w)
    if s[0x1A1] == s[0x1A2]:
        w.ext("anim_turn", s, 1)
    elif w.g8(ANIM_TURN_WHILE + s16(u16(s, 0x1E8 + 2 * s[0x1F3]))):
        turn_one(s, w)


def _u32(s, o):
    return u16(s, o) | u16(s, o + 2) << 16


def _w32(s, o, v):
    w16(s, o, v)
    w16(s, o + 2, v >> 16)


G_T0_CLASS, G_T1_CLASS_TBL, G_T2_CLASS = 0x284E58, 0x108158, 0x109AD8   # class per ground / object / byte-2 code


def replace_soldier(s, w):
    """ReplaceSoldier? 0x2FB98: put the soldier back on its tile as standing: +0xF4 = tile, ground +0x1B2 / object
    +0x1B4 re-read, queue reset with animation 4 (water: 0x33 / 0x36 / 0x39 by the ground), frames and delay set,
    in play (+0x138 = 1), ground / object / byte-2 codes and their classes (+0x1B3, +0x1B5, +0x1B6, +0x1B7)."""
    t = s16(u16(s, 0x114))
    w16(s, 0xF4, t)
    s[0x1B2], s[0x1B4] = w.tile8(t, 0), w.tile8(t, 1)
    s[0x1F3] = 0
    w16(s, 0x1E8, 4 if not is_water_tile(s[0x1B2], s[0x1B4]) else _water_anim(s, 0x33))
    _w32(s, 0x90, w.ptr(ANIM_FRAMES + u16(s, 0x1E8) * 0x50))
    anim_delay(s, w)
    s[0x138] = 1
    s[0x1B2] = w.tile8(t, 0)
    s[0x1B3] = w.g8(G_T0_CLASS + s[0x1B2])
    s[0x1B4] = w.tile8(t, 1)
    s[0x1B5] = w.g8(G_T1_CLASS_TBL + s[0x1B4])
    s[0x1B6] = w.tile8(t, 2)
    s[0x1B7] = w.g8(G_T2_CLASS + s[0x1B6])


def init_xra_enemy(s, w):
    """InitXraEnemy 0x2B6D0 (per enemy slot after the XRA load): +0x1E0 = Random(4), in the sector (+0x182) and in
    play (+0x138); Random(3) picks the start tile among the three XRA candidates u16 +0x114 / +0x116 / +0x118, which
    becomes tile, step target +0x124 and +0x12E; item 5 type 0x31 -> 0x30; items 7..9: empty -> bytes 1, 2 = 0, else
    byte 3 = 0 -> bytes 4, 5 = 0; wounds 0, +0x1BD = 0x6D, u16 +0x10C = 0, energy 100, hpMax = hp, +0x17B += 4,
    +0x17C = Random(8) + 2; then OccupyTile and ReplaceSoldier."""
    s[0x1E0] = w.random(4) & 0xFF
    s[0x182] = 1
    s[0x138] = 1
    t = s16(u16(s, (0x114, 0x116, 0x118)[w.random(3)]))
    if s[0x3C] == 0x31:
        s[0x3C] = 0x30
    for o in (0x54, 0x60, 0x6C):
        if s[o] == 0:
            s[o + 2] = s[o + 1] = 0
        elif s[o + 3] == 0:
            s[o + 5] = s[o + 4] = 0
    for o in (0x124, 0x114, 0x12E):
        w16(s, o, t)
    s[0x185] = 0
    s[0x1BD] = 0x6D
    w16(s, 0x10C, 0)
    s[0x18A] = 100
    s[0x184] = s[0x183]
    s[0x17B] = (s[0x17B] + 4) & 0xFF
    s[0x17C] = (w.random(8) + 2) & 0xFF
    occupy(w, t, s[0x17A])
    w.ext("replace", s)


def slot_reset(s, w, k):
    """SlotReset? 0x2B250 (EAX soldier, EDX slot k; sector start, every slot 0..47): clears the turn, view, path and
    message state, +0x9C = 0xFF, +0x188 = 1, +0x1C8 = 0x78, animation 4; +0x12A = tile = step target = 0xC41 (off
    the map), +0xDE / +0xE0 = 0xC41 copied to +0xE4 / +0xE6, slot +0x17A = k, +0x180 = (k > 3). Slots 8 and up:
    +0x182 = 0 and +0x17E = 1 on the pointer table's soldier k, +0x1B1 = 0 on s."""
    for o in (0x1AB, 0xB9, 0x1BD, 0x1F3, 0x17F, 0x1A4, 0x1C7, 0x1AD, 0x1B9, 0x189, 0x1D2, 0x181, 0x1A6, 0x1A7):
        s[o] = 0
    s[0x9C] = 0xFF
    s[0x188] = 1
    s[0x1C8] = 0x78
    for o in (0x12A, 0xDE, 0xE0, 0x134):
        w16(s, o, NO_TILE)
    w16(s, 0x1E8, 4)
    w16(s, 0x10A, 0)
    s[0x17A] = k & 0xFF
    w16(s, 0x124, u16(s, 0x12A))
    w16(s, 0x114, u16(s, 0x12A))
    s[0x180] = 1 if _i32(k) > 3 else 0
    w16(s, 0xE4, u16(s, 0xDE))
    w16(s, 0xE6, u16(s, 0xE0))
    if _i32(k) >= 8:
        r = w.rec(k)
        r[0x182] = 0
        r[0x17E] = 1
        s[0x1B1] = 0


def place_merc(s, w, tile, facing):
    """PlaceMerc? 0x2B380 (EAX soldier, EDX tile, BL facing): tile + step target = tile, +0x1E0 = slot, the tile's
    occupant byte 5 = slot (overwritten) and byte 4 |= 0x80, +0x1CF = 0, +0x9C = 1, facing + turn target, ReplaceSoldier,
    +0x1BD = 0x6A, in play. Orders +0x1D4 / attitude +0x1D3: profile > 0x3B -> 2 / 2; else medical +0x18E > marksmanship +0x191
    (signed) -> 3 / 0; else 4 / by +0x19C: 3 -> 5, 4 or 8 -> 0, 2 or 6 -> Random(5), any other -> Random(6)."""
    w16(s, 0x124, tile)
    w16(s, 0x114, tile)
    t = s16(u16(s, 0x114))
    s[0x1E0] = s[0x17A]
    w.s8(G_TILEMAP + 8 * t + 5, s[0x17A])
    w.s8(G_TILEMAP + 8 * t + 4, w.tile8(t, 4) | 0x80)
    s[0x1CF] = 0
    s[0x9C] = 1
    s[0x1A2] = s[0x1A1] = facing & 0xFF
    w.ext("replace", s)
    s[0x1BD] = 0x6A
    s[0x138] = 1
    if s[0x179] > 0x3B:
        s[0x1D4] = s[0x1D3] = 2
    elif s8(s[0x18E]) > s8(s[0x191]):
        s[0x1D4], s[0x1D3] = 3, 0
    else:
        s[0x1D4] = 4
        c = s[0x19C]
        s[0x1D3] = 5 if c == 3 else 0 if c in (4, 8) else w.random(5 if c in (2, 6) else 6) & 0xFF


def merc_entry(w, tile, facing):
    """MercEntry? 0x2B470 (EAX entry tile, DL facing): the selected merc takes the entry tile if it is standable;
    ReserveAp for it. Each merc 0..7 in the sector, in play and alive (else +0x138 = 0) and not placed yet tries the
    8 directions around a base tile (the entry tile; with ring 2, two steps out) for a standable tile without an
    occupant; none: the base moves to the tile of the next placed merc after the last one used (slot order, the index
    is kept across mercs), then ring 2 once (kept too); then the merc stays unplaced. Last, the highest placed merc
    other than the selected one swaps tiles with it (the selected merc's tile as remembered at its placement)."""
    sel = w.g8(G_SELECTED)
    placed = [0] * 8
    last, ring2, lead = -1, 0, None
    if tile_standable(w, tile) > 0:
        place_merc(w.rec(sel), w, tile, facing)
        over = lambda v, at: ((v & ~(0xFF << (at * 8))) | (1 << (at * 8)))
        if sel < 8:
            placed[sel] = 1
        elif sel < 12:                   # #2 row 48: the byte goes over the locals after placed[8]
            tile = over(tile, sel - 8)
        elif sel < 16:
            last = over(last, sel - 12)      # stays signed: -1 with one byte changed
        elif sel in (24, 25):
            ring2 = 1
        elif sel == 32:
            facing = 1
        lead = s16(tile & 0xFFFF)
    w.ext("reserve_ap", w.rec(sel))
    for k in range(8):
        s = w.rec(k)
        if not s[0x182] or not s[0x138] or not s[0x183]:
            s[0x138] = 0
            continue
        base = tile
        while not placed[s[0x17A]]:
            d = 1
            while d <= 8:
                c = step_tile(base, dir_step(w, d))
                if ring2:
                    c = step_tile(c, dir_step(w, d))
                if tile_standable(w, c) > 0 and _occupant(w, c) >= 0x40:
                    place_merc(s, w, c, facing)
                    placed[s[0x17A]] = 1
                    if sel == s[0x17A]:
                        lead = s16(c & 0xFFFF)
                    break
                d += 1
            if d < 9:
                continue
            j = last + 1
            if j < 0:
                raise Undefined("selected 12..14: the search reads stack bytes under the frame")
            while j < 8 and not placed[j]:
                j += 1
            if j < 8:
                base = s16(u16(w.rec(j), 0x114))
                last = j
                continue
            if ring2:
                break
            ring2 = 1
    for i in range(7, -1, -1):
        if placed[i] and i != sel:
            if lead is None:
                raise Undefined("the selected merc was never placed: its tile is stack garbage")
            ti = s16(u16(w.rec(i), 0x114))
            place_merc(w.rec(sel), w, ti, facing)
            place_merc(w.rec(i), w, lead, facing)
            return


G_GROUND_ITEMS = 0xEB160      # 12 B per ground item, byte 0 = item type


G_PURIFIER = 0x22A0CB         # u8 g_purifierPending: the next unowned sector gets the Micro-Purifier (0x79)
G_SCRATCH_PTR = 0x93E64        # pointer to a 0x200-byte scratch record: XRA templates of absent enemies land here
G_STORY_STATE = 0x22A048       # byte +3 == 2: the 0xAC / 0xAD story objects are gone
GUARD_DIRS = (2, 6, 4, 8, 1, 5, 3, 7)


def _sector_resets(w, speech=True):
    """SetupSectorFromXra 0x2E918 part 1: the per-sector globals (speech=False: without the 0x284F02 draw, for a
    harness whose port draws it itself)"""
    for a in (0x92DAC, 0x92DB0, 0x229DD4, 0x229DDC, 0x229DC8, 0x93D54, 0x92E3C):
        w.s32(a, 0)
    for a in (0x92DCE, 0x284F25, 0x284F26, 0x284F24, 0x92DBC, 0x92DC0, 0x92DCD, 0x92DC4, 0x92DDC, 0x92DDD,
              0x92DD1, 0x92DCF, 0x92DD0, 0x22A0D1):
        w.s8(a, 0)
    for a in (0x284F10, 0x92DC2, 0x95E0C, 0x92E52, 0x92DEC, 0x22A1F1, 0x92DA8, 0x284F28, 0x22A108, 0x92DD2):
        w.s8(a, 0x40)
    w.s32(0x229D1C, 20000)
    w.s8(0x10E380, w.g8(0x90684))
    w.s8(0x22A444, w.g8(0x90684))
    w.s8(0x284F19, 0xF8)
    w.s32(0x229C94, 0x40)
    w.s32(0x229C8C, NO_TILE)
    w.s32(0x229CB4, 0x40)
    for a, v, n in ((0x22A412, 0x40, 0x32), (0x10D890, 0, 0x140), (0x10D9D0, 0, 0x80), (0x10D860, 0, 0x30),
                    (0x22A0EC, 0, 8), (0xFA928, 0, 0x30), (0x10E290, 0, 0xF0)):
        for i in range(n):
            w.s8(a + i, v)
    w.s32(0x92DE8, 0xFFFFFFFF)
    w.s32(0x229E04, 0xFFFFFFFF)
    w.s8(0x22A0CD, 1)
    w.s16(0x92DBA, NO_TILE)
    w.s16(0x92DBE, NO_TILE)
    w.s16(0x22A484, 0)
    w.s16(0x22A486, 0)
    for k in range(5):
        w.s32(0x22A488 + 0xD * k, 0xFFFFFFFF)
        w.s16(0x22A44E + 6 * k, NO_TILE)
    if speech:
        w.s8(0x284F02, (w.random(3) + 1) & 0xFF)
    w.s8(G_ALARM, 1 if w.g8(G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR)) & 1 else 0)
    for i in range(0x3C):
        w.s8(0x229D50 + i, 0x41 if i % 2 == 0 else 0x0C)


def _sector_tail(w):
    """SetupSectorFromXra 0x2E918, after the soldier starts: the per-sector globals outside the port world (the port's
    host op 95 sector_tail; the 0x22A478 write comes before the selected merc's +0x1BD in the original, nothing reads
    either in between)"""
    w.s16(0x22A478, 0)
    for k in range(15):
        w.s8(0xEB002 + 0x12 * k, 0)
    for k in range(9):
        w.s8(0x229E2F + 0xB * k, 0)
    w.s32(0x92EF8, 0xFA)
    for i, v in enumerate((0x40, 0x7F, 0x40, 0xFF)):
        w.s8(0x229E00 + i, v)


def _ground_items_load(w):
    """XRA record tail (no sec%02d.tmp): ground item byte per tile (tiles 0xC3F down to 0), 50 ground items with
    per-unit conditions for stacks, then notes already placed elsewhere are taken off the map"""
    for t in range(0xC3F, -1, -1):
        w.s8(G_TILEMAP + 8 * t + 6, w.read(1)[0])
    for i in range(50):
        o = G_GROUND_ITEMS + 12 * i
        for j, b in enumerate(w.read(12)):
            w.s8(o + j, b)
        if w.g8(o + 3) == 0:
            w.s8(o + 5, 0)
            w.s8(o + 4, 0)
        if not w.g8(o) or w.g8(ITEM_TBL + 12 * w.g8(o) + 6) <= 1 or s8(w.g8(o + 1)) <= 1:
            continue
        j = 0
        while j < s8(w.g8(o + 1)) - 1:
            u = o + 8 + j
            base = s8(w.g8(o + 2))
            if base < 0:
                w.s8(u, (w.random(10) - 5 + w.g8(o + 2)) & 0xFF)
                if s8(w.g8(u)) > -1:
                    w.s8(u, 0xFF)
                if s8(w.g8(u)) <= -100:
                    w.s8(u, w.g8(o + 2))
            else:
                if base < 100:
                    w.s8(u, (w.random(10) - 5 + w.g8(o + 2)) & 0xFF)
                if s8(w.g8(u)) > 100:
                    w.s8(u, 100)
                if s8(w.g8(u)) <= 0:
                    w.s8(u, w.g8(o + 2))
            j += 1
    for t in range(0xC40):
        gi = w.tile8(t, 6)
        if gi != 0xFF and note_placed(w, w.g8(G_GROUND_ITEMS + 12 * gi)) == 1:
            w.ext("w_3c05c", t, 0)


def _xra_load(w):
    """XRA.DAT record g_curSector: 8 templates (pass 1, kept where the sector's enemy flag is 2; items 7 and 8 dropped
    on every slot) then 8 more (pass 2, flag 1); absent ones go to the scratch record. Every slot 0x28..0x2F: not in
    the sector, slot number set. Without a sec%02d.tmp the ground items follow."""
    cur = w.g8(G_CUR_SECTOR)
    w.ext("open_cd", "DAT\\XRA.DAT", 0)
    w.ext("fseek", 0, cur * 0x2E98, 0)
    sec = G_SECTORS + 0xB1 * cur
    for want in (2, 1):
        for k in range(0x28, 0x30):
            s = w.rec(k)
            data = w.read(0x200)
            if s8(w.g8(sec + 0x6E + k)) == want:
                s[0:0x200] = data
            else:
                w.raw_write(w.g32(G_SCRATCH_PTR), data)
            if want == 2:
                w.ext("drop_item", w.ptr(G_SOLDIERS + 0x200 * k + 0x54))
                w.ext("drop_item", w.ptr(G_SOLDIERS + 0x200 * k + 0x60))
            s[0x182] = 0
            s[0x17A] = k
    return sec


G_GUARD_SKILL = 0x22A030       # u8 added to (level - 1) x 5: guards' equipment and +0x1B1 chance


def init_tapper(s, w, tile, facing):
    """InitTapper? 0x2BB90 (EAX soldier, EDX tile, BL facing; sector start, beside a tree): in the sector and in play,
    +0x9C = 1, on duty, +0x1BD = 0x6C, colour 0x19, tile = step target = +0x12E = tile, facing = turn target = +0x1A4,
    +0x17C = Random(8) + 2, +0x1E0 = Random(4), no orders/attitude, +0x19C = 7, level 1; agility 30, dexterity 40, hp 40,
    wisdom 20 + two rolls each (26, 26, 21, 26); wounds 0, u16 +0x10C = 0, energy 100, +0x1F3 = 0, hpMax = hp,
    animation 0x11, then OccupyTile."""
    s[0x182] = s[0x138] = s[0x9C] = 1
    s[0x17E] = 0
    s[0x1BD] = 0x6C
    s[0x17B] = 0x19
    for o in (0x124, 0x114, 0x12E):
        w16(s, o, tile)
    s[0x1A2] = s[0x1A1] = s[0x1A4] = facing & 0xFF
    s[0x17C] = (w.random(8) + 2) & 0xFF
    r = w.random(4)
    s[0x1D4] = s[0x1D3] = 0
    s[0x19C] = 7
    s[0x1E0] = r & 0xFF
    s[0x192] = 1
    for o, base, k in ((0x18B, 0x1E, 0x1A), (0x18C, 0x28, 0x1A), (0x183, 0x28, 0x15), (0x18D, 0x14, 0x1A)):
        v = w.random(k) + base
        s[o] = (v + w.random(k)) & 0xFF
    s[0x185] = 0
    w16(s, 0x10C, 0)
    s[0x18A] = 100
    s[0x1F3] = 0
    s[0x184] = s[0x183]
    w16(s, 0x1E8, 0x11)
    occupy(w, tile, s[0x17A])


def init_guard(s, w, tile, facing, orders, anchor):
    """InitGuard? 0x2B7DC (EAX soldier, EDX tile, BL facing, CL orders, stack anchor tile; ret 4): in the sector and in
    play, +0x9C = 1, on duty, +0x1BD = 0x6B, colour 0x17, tile = step target = tile, facing = turn target, +0x17C =
    Random(8) + 2, +0x1E0 = Random(4), +0x12E = anchor, orders = CL, attitude = Random(6), +0x19C = 7, level =
    Random(day + 5) / 10 + 1 (at most 9); agility, dexterity, hp 40, wisdom 20, marksmanship 30 + two Random(26);
    medical = two Random(16). A level above 1 adds (level - 1) x 5 to the six (at most 100, signed). c = [0x22A030] +
    (level - 1) x 5 (byte): +0x1B1 = Random(100) < c; item 0 = 1 + (Random(2) != 0) + c / 5 (at most 0xE), FUN_53708;
    Random(100) < c + 50: item 1 = 0xF + (Random(100) < c + 25), FUN_53708; c != 0: Random(c) / 10 != 0 -> item 2 =
    that + 0x22 (at most 0x26), and again item 5 = that + 0x2C (at most 0x30), each FUN_53708(.., 1, 0). Wounds 0,
    u16 +0x10C = 0, hpMax = hp, energy 100, OccupyTile, ReplaceSoldier. (Item pointers are logged as the record of
    slot +0x17A.)"""
    s[0x182] = s[0x9C] = s[0x138] = 1
    s[0x17E] = 0
    s[0x1BD] = 0x6B
    s[0x17B] = 0x17
    w16(s, 0x124, tile)
    w16(s, 0x114, tile)
    s[0x1A2] = s[0x1A1] = facing & 0xFF
    s[0x17C] = (w.random(8) + 2) & 0xFF
    s[0x1E0] = w.random(4) & 0xFF
    w16(s, 0x12E, anchor)
    s[0x1D4] = orders & 0xFF
    s[0x1D3] = w.random(6) & 0xFF
    s[0x19C] = 7
    s[0x192] = (cdiv(w.random(s16((w.g16(G_DAY) + 5) & 0xFFFF)), 10) + 1) & 0xFF
    if s8(s[0x192]) > 9:
        s[0x192] = 9
    for o, base in ((0x18B, 0x28), (0x18C, 0x28), (0x183, 0x28), (0x18D, 0x14), (0x191, 0x1E)):
        v = w.random(0x1A) + base
        s[o] = (v + w.random(0x1A)) & 0xFF
    v = w.random(0x10)
    s[0x18E] = (v + w.random(0x10)) & 0xFF
    a = ((s[0x192] - 1) * 5) & 0xFF
    if a:
        for o in (0x18B, 0x18C, 0x183, 0x18D, 0x191, 0x18E):
            s[o] = (s[o] + a) & 0xFF
            if s8(s[o]) > 100:
                s[o] = 100
    c = (w.g8(G_GUARD_SKILL) + (((s[0x192] - 1) * 5) & 0xFF)) & 0xFF
    s[0x1B1] = 1 if w.random(100) < c else 0
    rec = G_SOLDIERS + 0x200 * s[0x17A]
    s[0] = (1 + (1 if w.random(2) else 0) + c // 5) & 0xFF
    if s[0] > 0xE:
        s[0] = 0xE
    w.ext("w_53708", w.ptr(rec), 1, 2)
    if w.random(100) < c + 0x32:
        s[0xC] = (0xF + (1 if w.random(100) < c + 0x19 else 0)) & 0xFF
        w.ext("w_53708", w.ptr(rec + 0xC), 1, 2)
    if c:
        for o, base, cap in ((0x18, 0x22, 0x26), (0x3C, 0x2C, 0x30)):
            q = (w.random(c) // 10) & 0xFF
            if q:
                v = (q + base) & 0xFF
                s[o] = cap if v > cap else v
                w.ext("w_53708", w.ptr(rec + o), 1, 0)
    s[0x185] = 0
    w16(s, 0x10C, 0)
    s[0x184] = s[0x183]
    s[0x18A] = 100
    occupy(w, tile, s[0x17A])
    w.ext("replace", s)


EQ_QTY = (1, 1, 1, 2, 2, 3)                          # 0x29F20: item 1 count by Random(6)
EQ_GUNS = (1, 2, 3, 4, 8, 9, 10, 11)                 # 0x29F26: gun by the weapon roll (index 0..7)
EQ_THROWN = (0x1C, 0x1C, 0x1C, 0x16, 0x16, 0x18, 0x1A)   # 0x29F2E: item 1 by Random(7)
G_EQ_SKILL = 0x95C48            # u8 per difficulty (2 B each): base of the equipment chance
G_EQ_ITEMS, G_EQ_GUNS, G_EQ_THROWN = 0x22A0BE, 0x22A0BF, 0x22A0C1   # u8 chance bonuses (sector / weapon / item 1)
ATTACH_TBL = 0x94AAA            # 82 x 4 B: kind ('B' = 0x42, '[' = 0x5B), item type, ..


def clip_count(w, t):
    """0x30A88 (AL item type, keeps EDX): units in a new stack. Item class (item table +6) 5: Random(100) < 1 -> 5,
    < 3 -> 4, < 10 -> 3, < 25 -> 2, else 1; any other class 1 (no Random)."""
    if w.g8(ITEM_TBL + 12 * t + 6) != 5:
        return 1
    r = w.random(100) & 0xFF
    for lim, q in ((1, 5), (3, 4), (10, 3), (25, 2)):
        if r < lim:
            return q
    return 1


def item_init(w, p, dl, bl):
    """FUN_53708 (EAX item pointer, DL condition mode, BL count mode): attachment bytes 3..5 = 0. Empty type: count,
    condition and unit conditions 8..11 = 0, done. DL 1: condition = Random(31) + 70; DL 2: condition = 100; DL != 0
    clears unit conditions 8..11. Item class (item table +6) 1: a max count (+5) of 99 -> count 99; else BL 0 caps the
    count at the max (signed), BL 1 count = Random(max - 1) + 1, BL 2 count = max, BL > 2 leaves it. Other classes:
    count capped at the class (signed), 0 -> 1; above 1, unit conditions 8.. for units 2..count: DL 1 Random(31) + 70,
    DL 2 100, else an empty one takes the condition."""
    dl, bl = dl & 0xFF, bl & 0xFF
    for o in (3, 4, 5):
        w.raw_s8(p + o, 0)
    t = w.raw8(p)
    if not t:
        for o in (2, 1, 8, 9, 10, 11):
            w.raw_s8(p + o, 0)
        return
    if dl == 1:
        w.raw_s8(p + 2, (w.random(0x1F) + 0x46) & 0xFF)
    elif dl == 2:
        w.raw_s8(p + 2, 100)
    if dl:
        for o in (8, 9, 10, 11):
            w.raw_s8(p + o, 0)
    cls, top = w.g8(ITEM_TBL + 12 * t + 6), w.g8(ITEM_TBL + 12 * t + 5)
    if cls == 1:
        if top == 99 or bl == 2:
            w.raw_s8(p + 1, top)
        elif bl == 0:
            if s8(w.raw8(p + 1)) > s8(top):
                w.raw_s8(p + 1, top)
        elif bl == 1:
            w.raw_s8(p + 1, (w.random(s8(top) - 1) + 1) & 0xFF)
        return
    if s8(w.raw8(p + 1)) > cls:
        w.raw_s8(p + 1, cls)
    if not w.raw8(p + 1):
        w.raw_s8(p + 1, 1)
    i = 0
    while i < s8(w.raw8(p + 1)) - 1:
        if dl == 1:
            w.raw_s8(p + 8 + i, (w.random(0x1F) + 0x46) & 0xFF)
        elif dl == 2:
            w.raw_s8(p + 8 + i, 100)
        elif not w.raw8(p + 8 + i):
            w.raw_s8(p + 8 + i, w.raw8(p + 2))
        i += 1


def _eq_swap(s, old, new):
    """items 7..11 of type old become type new"""
    for o in range(0x54, 0x90, 0xC):
        if s[o] == old:
            s[o] = new


def equip(s, w, slot, flag):
    """Equipment roll 0x2FCE4 (EAX soldier, DL flag: 2 in the sector's enemy list). Byte +0xBA == 4: items 2 and 5
    cleared, done. flag 0: FUN_53708(item, c, c) per item 0..11 with c = (condition == 0); else items 0..11 zeroed.
    c = [0x95C48 + 2 x difficulty] + (day < 5 ? day x 5 - 25 : 0) + (level - 4) x 5 (signed byte); A, B, C = the u8
    bonuses at 0x22A0BE / BF / C1. +0x1B1 = Random(100) < A + c + 15. With flag or A: gun roll (u = 7 down to 1, stop
    when Random(100) < (7 - u) x 5 + c + B; u 0 when none stops), gun = table[u]; a better gun than item 0 replaces it
    (and its type in items 7..11, and the old gun's ammo by the new ammo when the old type is 1..14), FUN_53708(.., 1,
    1). Item 1 empty: Random(100) < C + c + 15 -> thrown table[Random(7)], count table[Random(6)]; else Random(100) <
    A + 25 + 25 (attitude 5) -> 0xF + (Random(100) < A + 10 + 25 (attitude 5)); else no item 1. Armour 0x26 down to 0x23
    and helmet 0x30 down to 0x2D: a step rolls Random(100) < A + c + 50 only while step - base < +0x1B4. Item 6 =
    0x31 (+0x1B4 == 4 and Random(100) < A + c) or 0x32 + Random(4). Items 4 and 3 if empty: 0x2B / 0x28 at A + c,
    else 0x2A / 0x27 (count 1) at A + c + 25. Item 0 without attachment: a 'B' table match rolls Random(100) < c - 5 +
    A (attachment 0x42), else a '[' match at c - 10 + A (0x5B); condition Random(31) + 70. flag 0: items 7..9 of type
    0x36..0x3E get clip_count, and item 1 of type 0x1C at condition 100 gets 50 + Random(50); flag: each of items
    0..5 copies (bytes 0..2, 8..11) to the next of items 7..9 by Random(100) < 8. Last: gun 1..14 and Random(100) <
    (flag ? 45 : 25) -> item 11 = its ammo, count clip_count."""
    rec = G_SOLDIERS + 0x200 * slot

    def item(k):
        return w.ptr(rec + 12 * k)
    flag &= 0xFF
    if s[0xBA] == 4:
        s[0x18] = s[0x3C] = 0
        return
    if not flag:
        for k in range(12):
            c = 0 if s[12 * k + 2] else 1
            w.ext("w_53708", item(k), c, c)
    else:
        for o in range(0x90):
            s[o] = 0
    lv = (s[0x192] - 4) * 5
    day = w.g32(G_DAY)
    d = w.g8(G_DAY) * 5 - 0x19 if day < 5 or day >= 0x80000000 else 0
    c = s8((w.g8(G_EQ_SKILL + 2 * s8(w.g8(G_DIFFICULTY))) + d + lv) & 0xFF)
    a = w.g8(G_EQ_ITEMS)
    s[0x1B1] = 1 if w.random(100) < a + c + 15 else 0
    if flag or a:
        old = s[0]
        u = 7
        while w.random(100) >= (7 - u) * 5 + c + w.g8(G_EQ_GUNS):
            u -= 1
            if not u:
                break
        new = EQ_GUNS[u]
        if new > old:
            if old:
                _eq_swap(s, old, new)
                if 1 <= old <= 14:
                    _eq_swap(s, w.g8(WEAPON_TBL + 1 + 6 * old), w.g8(WEAPON_TBL + 1 + 6 * new))
            s[0] = new
            w.ext("w_53708", item(0), 1, 1)
        if not s[0xC]:
            if w.random(100) < w.g8(G_EQ_THROWN) + c + 15:
                s[0xC] = EQ_THROWN[w.random(7)]
                s[0xD] = EQ_QTY[w.random(6)]
                w.ext("w_53708", item(1), 1, 0)
            else:
                b = 25 if s[0x1D3] == 5 else 0
                if w.random(100) < a + b + 25:
                    s[0xC] = 0x10 if w.random(100) < a + b + 10 else 0xF
                    w.ext("w_53708", item(1), 1, 0)
    for k, top, o in ((2, 0x26, 0x18), (5, 0x30, 0x3C)):
        base = top - 3
        t = top
        while t >= base:
            if t - base < s[0x1B4] and w.random(100) < a + c + 50:
                break
            t -= 1
        if t >= base:
            if s[o]:
                _eq_swap(s, s[o], t)
            s[o] = t
            w.ext("w_53708", item(k), 1, 0)
        else:
            s[o] = 0
    if s[0x1B4] == 4 and w.random(100) < a + c:
        s[0x48] = 0x31
    else:
        s[0x48] = (w.random(4) + 0x32) & 0xFF
    w.ext("w_53708", item(6), 1, 0)
    for k, o, hi, lo in ((4, 0x30, 0x2B, 0x2A), (3, 0x24, 0x28, 0x27)):
        if not s[o]:
            if w.random(100) < a + c:
                s[o] = hi
            elif w.random(100) < c + 25 + a:
                s[o] = lo
                if k == 3:
                    s[0x25] = 1
            else:
                continue
            w.ext("w_53708", item(k), 1, 0)
    if not s[3]:
        fb = fl = False
        for k in range(0x52):
            if w.g8(ATTACH_TBL + 4 * k + 1) == s[0]:
                kind = w.g8(ATTACH_TBL + 4 * k)
                fb = fb or kind == 0x42
                fl = fl or kind == 0x5B
        for on, cut, att in ((fb, 5, 0x42), (fl, 10, 0x5B)):
            if on and (att == 0x42 or s[3] != 0x42) and w.random(100) < c - cut + a:
                s[3] = att
                s[4] = 1
                s[5] = (w.random(0x1F) + 0x46) & 0xFF
    if not flag:
        for k in (7, 8, 9):
            t = s[12 * k]
            if 0x36 <= t <= 0x3E:
                s[12 * k + 1] = clip_count(w, t)
                w.ext("w_53708", item(k), 1, 0)
        if s[0xC] == 0x1C and s[0xE] == 100:
            s[0xE] = (w.random(0x32) + 0x32) & 0xFF
    else:
        dst = 7
        for k in range(6):
            if s[12 * k] and w.random(100) < 8:
                for o in (0, 1, 2, 8, 9, 10, 11):
                    s[12 * dst + o] = s[12 * k + o]
                w.ext("w_53708", item(dst), 0, 0)
                dst += 1
                if dst > 9:
                    break
    if 1 <= s[0] <= 14 and w.random(100) < (0x2D if flag else 0x19):
        s[0x84] = w.g8(WEAPON_TBL + 1 + 6 * s[0])
        s[0x85] = clip_count(w, s[0x84])
        w.ext("w_53708", item(11), 1, 0)


G_SLOT_T34 = 0x10D650           # u8 per slot: an enemy (slot >= 0x28) with orders 3 or 4 (set at sector start)
SOLDIER_NAMES = ((0x9073C, "GUARD %d"), (0x90730, "TAPPER%2d"), (0x90724, "ENEMY %d"))


def armor_bonus(s):
    """0x5383C (EAX soldier): +0x1D0 = 1 when level > 3 (signed) else 0; item 4 of type 0x2B overrides it with
    +0x1D0 + 5 - (100 - item 4 condition) / 20 (signed, truncated)."""
    s[0x1D0] = 1 if s8(s[0x192]) > 3 else 0
    if s[0x30] == 0x2B:
        s[0x1D0] = (s8(s[0x1D0]) + 5 - cdiv(100 - s8(s[0x32]), 20)) & 0xFF


def wakes_alert(s, w, dx):
    """0x2C6A0 (EAX soldier, DX left over from the caller): sector 0 -> 1. Base by attitude +0x1D4: 0 -> 5, 1 -> 10,
    2 -> 50, 3 / 4 -> 25, else DX; 1 when Random(100) < (base + (level - 1) x 5) as a signed 16-bit value."""
    if not w.g8(G_CUR_SECTOR):
        return 1
    base = {0: 5, 1: 10, 2: 0x32, 3: 0x19, 4: 0x19}.get(s[0x1D4], dx)
    return 1 if w.random(100) < s16((base + (s8(s[0x192]) - 1) * 5) & 0xFFFF) else 0


def init_soldier(s, w):
    """InitSoldierNames 0x2C034 (EAX soldier): the per-sector reset of one slot. Four words +0x1EA.. = 1000; type +0xBA
    and nickname +0x16E by slot (< 8: 0; < 0x10: 1 "GUARD %d"; < 0x28: 2 "TAPPER%2d"; sector 0 slot 0x28: 4 "SANTINO";
    else 3 "ENEMY %d"); a long list of fields to 0 / 0xC41 / -1; +0xA4 = (10 - level) x 10; ShotEnd(1, 0),
    ClearTarget(0). In the sector (+0x182): AP, armour bonus 0x5383C, screen x / y, object / ground / byte-2 under the
    tile with their classes, +0x186 / +0x187 = hp < 15, +0xA8 = Random(300), enemies +0xB9 = 0x2C6A0 (DX 0) and
    G_SLOT_T34, the first animation frame, AnimDelay, +0x162..0x169 = 0, +0x19E = 7 / 8 by level (+1 with item 3 type
    0x27), mercs +0x1B0 = 0, others +0x179 = 0x40, +0xEF = 0, +0x1B0 = 1; out of it +0x194 = 1. Then +0x13A..0x161 =
    0; unless type 2: ForgetTiles and its remember row cleared (slot < 0x10: 8 B of G_REMEMBER_M, else 0x28 B of
    G_REMEMBER_E at slot - 0x28); +0x1CF = facing (enemies) or 0; empty items 0..11 zeroed."""
    from combat import max_ap
    for o in range(0x1EA, 0x1F2, 2):
        w16(s, o, 1000)
    slot = s[0x17A]
    if slot >= 0x28:
        if not w.g8(G_CUR_SECTOR) and slot == 0x28:
            s[0xBA] = 4
            name = b"SANTINO\0"
        else:
            s[0xBA] = 3
            name = b"ENEMY %d\0" % slot
    elif slot >= 0x10:
        s[0xBA] = 2
        name = b"TAPPER%2d\0" % slot
    elif slot >= 8:
        s[0xBA] = 1
        name = b"GUARD %d\0" % slot
    else:
        s[0xBA] = 0
        name = b""
    for i, c in enumerate(name):
        s[0x16E + i] = c
    for o in (0x1CC, 0x1FA, 0x1D7, 0x130, 0x1DB, 0x1FC, 0x1E1, 0x9D, 0x16D, 0x1D1, 0x1D6, 0x1C1, 0x1C0, 0x197, 0x196,
              0x193, 0x1D2):
        s[o] = 0
    for o in (0x10E, 0x108, 0x132, 0x122, 0x120):
        w16(s, o, NO_TILE)
    s[0x1BC] = 1
    s[0x1E3] = s[0x1E2] = s[0x1E1]
    _w32(s, 0xC0, 0xFFFFFFFF)
    _w32(s, 0xC4, 0xFFFFFFFF)
    s[0x1F7] = 0x40
    w16(s, 0xEC, NO_TILE)
    s[0x1FF] = s[0x1BA] = s[0x1FE] = 0
    s[0x198] = 0xFF
    w16(s, 0x106, NO_TILE)
    if s[0x183]:
        s[0x188] = 0
    s[0x1A8] = 0
    s[0x1AC] = 2
    s[0x1A3] = 0
    _w32(s, 0xA4, (10 - s8(s[0x192])) * 10)
    s[0x177] = 0
    shot_end(s, w, 1, 0)
    clear_target(s, w, 0)
    s[0x1D5] = 0
    w16(s, 0xDC, NO_TILE)
    w16(s, 0x1E6, NO_TILE)
    s[0x1F6] = 0
    w16(s, 0x1F8, NO_TILE)
    s[0x195] = 0
    s[0x1DA] = 0x40
    w16(s, 0xD0, NO_TILE)
    s[0x1F2] = s[0x1FD] = 0
    if s[0x182]:
        s[0x131] = 0
        w16(s, 0xF0, 0)
        s[0x1C4] = s[0x194] = 0
        s[0x1A5] = max_ap(s) & 0xFF
        armor_bonus(s)
        w16(s, 0x11A, 0xA)
        w16(s, 0x11C, 0xD)
        t = s16(u16(s, 0x114))
        x, y = tile_x(t), tile_y(t)
        w16(s, 0x116, x)
        w16(s, 0x126, x)
        w16(s, 0x118, y)
        w16(s, 0x128, y)
        s[0x1B4] = w.tile8(t, 1)
        s[0x1B5] = w.g8(G_T1_CLASS_TBL + s[0x1B4])
        s[0x1B2] = w.tile8(t, 0)
        s[0x1B3] = w.g8(G_T0_CLASS + s[0x1B2])
        s[0x1B6] = w.tile8(t, 2)
        s[0x1B7] = w.g8(G_T2_CLASS + s[0x1B6])
        s[0x186] = s[0x187] = 1 if s8(s[0x183]) < 0xF else 0
        s[0x1B9] = 0
        w16(s, 0xD6, NO_TILE)
        s[0x1CA] = s[0x1A9] = s[0x1AE] = 0
        w16(s, 0xD4, 0)
        _w32(s, 0xB0, 100)
        _w32(s, 0xBC, 0x32)
        _w32(s, 0xA8, w.random(300))
        _w32(s, 0xAC, 100)
        _w32(s, 0x1DC, 0)
        _w32(s, 0xB4, 0xFFFFFFFF)
        if slot < 0x28:
            s[0xB9] = 0
            if slot < 8 and s[0x17E] == 8:
                s[0x17E] = 0
        else:
            s[0xB9] = wakes_alert(s, w, 0)
            w.s8(G_SLOT_T34 + slot, 1 if s[0x1D4] in (3, 4) else 0)
        _w32(s, 0xCC, 100)
        s[0x19F] = 2
        s[0x1B8] = 1
        w16(s, 0x136, 0)
        s[0x1A6] = s[0x1AD] = s[0x1F3] = 0
        p = ANIM_FRAMES + 0x50 * u16(s, 0x1E8)
        _w32(s, 0x90, w.ptr(p))
        w16(s, 0xC8, 0)
        s[0x1C9] = 0
        s[0x1F4] = w.g8(p)
        s[0x1AF] = 0
        anim_delay(s, w)
        s[0x1CD] = 0
        for o in range(0x162, 0x16A):
            s[o] = 0
        _w32(s, 0xA0, 500)
        s[0x19E] = 7 if s[0x192] <= 2 else 8
        if s[0x24] == 0x27:
            s[0x19E] = (s[0x19E] + 1) & 0xFF
        if slot >= 8:
            s[0x179] = 0x40
            s[0xEF] = 0
            s[0x1B0] = 1
        else:
            s[0x1B0] = 0
    else:
        s[0x194] = 1
    for o in range(0x13A, 0x162):
        s[o] = 0
    if s8(s[0xBA]) != 2:
        forget_tiles(w, s)
        if slot < 0x10:
            for j in range(8):
                w.s8(G_REMEMBER_M + 8 * slot + j, 0)
        else:
            for j in range(0x28):
                w.s8(G_REMEMBER_E + 0x28 * (slot - 0x28) + j, 0)
    s[0x1CF] = s[0x1A1] if slot >= 0x28 else 0
    for k in range(12):
        if not s[12 * k]:
            for o in range(12 * k, 12 * k + 12):
                s[o] = 0


G_CRATE_DROPS = 0x95CC0        # 4 x {i8 sector, i8 day, i8 patch record count, u8 the enemy keeps it}
G_CRATE_STATE = 0x22A06B       # u8 per drop: 1 dropped, 2 picked up, 3 lost to the enemy, 4 kept, 5 sector owned
G_CRATE_SHOWN = 0x22A06F       # u8 per drop: crate tiles on the map (0 / 1; 0xFF = not yet)
G_EQ_MEDS = 0x22A0C0           # u8 the fourth enemy supply bonus (raised with the three of the equipment roll)
FIXED_ITEMS = 0x95F98          # 11 x 8 B: u16 tile, type, count, condition (sector 0x3B, first visit)


def tile_patch(w, path, n, bl, cl):
    """ApplyTileObjPatch 0x317EC (EAX path, EDX record count, BL, CL): OpenCDFile (-1: returns -1). Signed EDX records
    of u16 tile + u16 object (a short read keeps the old bytes: they are stack words): with CL an object 0x9E..0xAA (signed)
    + 1, kept; tile != 0 and signed < 0xC40: t1 = the object's low byte, MoveCode, THEN with BL t3 = 0x73. fclose;
    the number of records applied."""
    if _i32(w.ext("open_cd", path, n & 0xFFFFFFFF)) == -1:
        return -1
    rec = bytearray(4)
    done = 0
    for _ in range(_i32(n)):
        for o in (0, 2):
            d = w.read(2)
            rec[o:o + len(d)] = d
        obj = int.from_bytes(rec[2:4], "little")
        if cl & 0xFF and 0x9E <= s16(obj) <= 0xAA:
            rec[2:4] = ((obj + 1) & 0xFFFF).to_bytes(2, "little")
        t = s16(int.from_bytes(rec[0:2], "little"))
        if t and t < 0xC40:
            w.s8(G_TILEMAP + 8 * t + 1, rec[2])
            move_code_at(w, t)
            if bl & 0xFF:
                w.s8(G_TILEMAP + 8 * t + 3, 0x73)
            done += 1
    w.ext("fclose", 0)
    return done


G_ITEM_BUF = 0x222B68          # 12-byte item scratch entry (item patch records, story items)


def item_patch(w, path, n, bl, cl):
    """ApplyItemPatch 0x318C4 (EAX path, EDX record count, BL, CL): OpenCDFile (-1: returns -1). Signed EDX records of
    u16 tile (a stack word: a short read keeps it) + a 12-byte item read into G_ITEM_BUF; tile != 0, signed < 0xC40 and an item
    type: BL -> TakeGroundItem(tile, 0); else (CL and a ground item on the tile: PutItem of that entry (signed +6, 0, 0,
    0), then the type of the tile's entry, read again, = 0) PutItem(the record, signed +6, 1, 1, 0). fclose; the number
    of records applied."""
    if _i32(w.ext("open_cd", path, n & 0xFFFFFFFF)) == -1:
        return -1
    rec = bytearray(2)
    done = 0
    for _ in range(_i32(n)):
        d = w.read(2)
        rec[0:len(d)] = d
        for j, b in enumerate(w.read(12)):
            w.s8(G_ITEM_BUF + j, b)
        t = s16(int.from_bytes(rec, "little"))
        if not t or t >= 0xC40 or not w.g8(G_ITEM_BUF):
            continue
        if bl & 0xFF:
            w.ext("w_3c05c", t & 0xFFFFFFFF, 0)
        else:
            if cl & 0xFF and w.g8(G_TILEMAP + 8 * t + 6) != 0xFF:
                o = G_GROUND_ITEMS + 12 * w.g8(G_TILEMAP + 8 * t + 6)
                put_item(w, t, w.g32(o), w.g32(o + 4), w.g32(o + 8), s8(w.g8(o + 6)) & 0xFFFFFFFF, 0, 0, 0)
                w.s8(G_GROUND_ITEMS + 12 * w.g8(G_TILEMAP + 8 * t + 6), 0)
            put_item(w, t, w.g32(G_ITEM_BUF), w.g32(G_ITEM_BUF + 4), w.g32(G_ITEM_BUF + 8),
                     s8(w.g8(G_ITEM_BUF + 6)) & 0xFFFFFFFF, 1, 1, 0)
        done += 1
    w.ext("fclose", 0)
    return done


def _crate_patch(w, kind, sec, n, bl, cl):
    """sprintf MISC\\CRATE%02d.BIN or MISC\\ITEM%02d.BIN into the text buffer, then the patch routine (EAX path,
    EDX record count, BL, CL)"""
    path = (b"MISC\\CRATE%02d.BIN" if kind == "tile_patch" else b"MISC\\ITEM%02d.BIN") % sec
    for k, ch in enumerate(path + b"\0"):
        w.s8(G_TEXT_BUF + k, ch)
    (tile_patch if kind == "tile_patch" else item_patch)(w, path.decode("latin-1"), n, bl, cl)


def crate_lost(w, k):
    """0x62780 (EAX drop index): unless already 3, the drop's state becomes 3 and the enemy gains supplies: drop 0
    0x22A0BE + 10, BF + 15, C0 + 10, C1 + 5; drop 1 BE + 10, BF + 25, C0 + 25; drop 2 BE + 10, BF + 5, C0 + 5,
    C1 + 20; drop 3 nothing more."""
    if s8(w.g8(G_CRATE_STATE + k)) == 3:
        return
    w.s8(G_CRATE_STATE + k, 3)
    add = {0: (10, 15, 10, 5), 1: (10, 25, 25, 0), 2: (10, 5, 5, 20)}.get(k)
    if add:
        for a, v in zip((G_EQ_ITEMS, G_EQ_GUNS, G_EQ_MEDS, G_EQ_THROWN), add):
            w.s8(a, (w.g8(a) + v) & 0xFF)


def crate_drops(w):
    """LoadCrateDrops 0x62360: each of the 4 supply drops in the current sector (signed sector byte). On its day: state
    0 -> 5 when the sector is owned, else crate tiles (BL 1) + items (CL 1), state 1, shown 0; state 1 -> 2 when owned,
    4 when the enemy keeps it, else crate tiles (CL 1) + items removed (BL 1), crate_lost, shown 1. After its day:
    state 4 and shown 0xFF -> tiles (BL 1) + items (CL 1), shown 0; state 3 and shown 0xFF -> tiles (BL 1, CL 1), shown
    1; state 3 and shown 0 -> tiles (CL 1) + items removed (BL 1), shown 1."""
    day = _i32(w.g32(G_DAY))
    for k in range(4):
        e = G_CRATE_DROPS + 4 * k
        sec = s8(w.g8(e))
        if sec != w.g8(G_CUR_SECTOR):
            continue
        dday, n = s8(w.g8(e + 1)), s8(w.g8(e + 2))
        st, sh = G_CRATE_STATE + k, G_CRATE_SHOWN + k
        owned = w.g8(G_SECTOR_TBL9 + 9 * sec) & 1
        if dday == day:
            if not w.g8(st):
                if owned:
                    w.s8(st, 5)
                    continue
                _crate_patch(w, "tile_patch", sec, n, 1, 0)
                _crate_patch(w, "item_patch", sec, n, 0, 1)
                w.s8(st, 1)
                w.s8(sh, 0)
                continue
            if s8(w.g8(st)) == 1:
                if owned:
                    w.s8(st, 2)
                elif w.g8(e + 3):
                    w.s8(st, 4)
                else:
                    _crate_patch(w, "tile_patch", sec, n, 0, 1)
                    _crate_patch(w, "item_patch", sec, n, 1, 0)
                    crate_lost(w, k)
                    w.s8(sh, 1)
                continue
        if dday >= day:
            continue
        s, f = s8(w.g8(st)), s8(w.g8(sh))
        if s == 4 and f == -1:
            _crate_patch(w, "tile_patch", sec, n, 1, 0)
            _crate_patch(w, "item_patch", sec, n, 0, 1)
            w.s8(sh, 0)
        elif s == 3 and f == -1:
            _crate_patch(w, "tile_patch", sec, n, 1, 1)
            w.s8(sh, 1)
        elif s == 3 and not w.g8(sh):
            _crate_patch(w, "tile_patch", sec, n, 0, 1)
            _crate_patch(w, "item_patch", sec, n, 1, 0)
            w.s8(sh, 1)


def _plant_patch(w, fmt, kind, sec, n, bl, cl):
    """sprintf a MISC plant / item patch path (%02d = the record's sector dword) into the text buffer, then the patch
    routine (EAX path, EDX record count, BL, CL)"""
    path = fmt % _i32(sec)
    for k, ch in enumerate(path + b"\0"):
        w.s8(G_TEXT_BUF + k, ch)
    (tile_patch if kind == "tile_patch" else item_patch)(w, path.decode("latin-1"), n, bl, cl)


def plant_sector(w):
    """LoadPlantSector 0x62848: the first g_plants record k = 1..3 whose sector dword (+4) equals the current sector
    (u8). Only state (+8, signed) 2 acts: while [0x22A045] (signed) < 2 the dead plant: DEADPL tiles (+9 records, BL 0)
    + items (+10 records, BL 1), state 0, replay event 'Q' (signed sector); else the live plant: LIVEPL tiles (+9) +
    items (+10; owned sector BL 1, else BL 0 and state 3). Every CL is 0."""
    cur = w.g8(G_CUR_SECTOR)
    for k in range(1, 4):
        e = G_PLANTS + 12 * k
        sec = w.g32(e + 4)
        if sec != cur:
            continue
        if s8(w.g8(e + 8)) != 2:
            return
        nt, ni = s8(w.g8(e + 9)), s8(w.g8(e + 10))
        if s8(w.g8(0x22A045)) < 2:
            _plant_patch(w, b"MISC\\DEADPL%02d.BIN", "tile_patch", sec, nt, 0, 0)
            _plant_patch(w, b"MISC\\ITEM%02d.BIN", "item_patch", sec, ni, 1, 0)
            w.s8(e + 8, 0)
            w.ext("replay_event", s8(cur) & 0xFFFFFFFF, 0, 0x51)
            return
        _plant_patch(w, b"MISC\\LIVEPL%02d.BIN", "tile_patch", sec, nt, 0, 0)
        owned = w.g8(G_SECTOR_TBL9 + 9 * cur) & 1
        _plant_patch(w, b"MISC\\ITEM%02d.BIN", "item_patch", sec, ni, owned, 0)
        if not owned:
            w.s8(e + 8, 3)
        return


def plant_done(w):
    """0x629AC: the first g_plants record k = 1..3 whose sector dword equals the current sector (u8): state 3 -> 0 and
    replay event 'Q' (signed sector); any other state, nothing."""
    cur = w.g8(G_CUR_SECTOR)
    for k in range(1, 4):
        e = G_PLANTS + 12 * k
        if w.g32(e + 4) != cur:
            continue
        if s8(w.g8(e + 8)) == 3:
            w.s8(e + 8, 0)
            w.ext("replay_event", s8(cur) & 0xFFFFFFFF, 0, 0x51)
        return


G_BOMB_COUNT = 0x92E3C         # i32: ground items of type 0x3F set off by 0x355D0 (3 or more: the chain effect flag)


def ground_blast(w, typ, flag, power, thrower, last):
    """0x355D0 (AL item type, DL flag, EBX power, CL thrower, stack byte; ret 4): type 0x11 with power < 100 rolls
    power 500 + Random(1000). Tiles 0..3135 in order, ground item byte < 0xFF holding an item of the type:
    - 0x11: taken off the map (0x3C05C(tile, 0)), sector +0xB0 - 1; the scan goes on while that count stays nonzero.
    - else Explode(thrower, tile, signed item byte 1 + 1, 1), after: 0x6E byte 1 = 1 then 0x629AC; 0x71 0x629AC. A 0x3F
      item counts in G_BOMB_COUNT. 0x29 / 0x3F / 0x71 then start effect 0x157BC(0xC41, type, flag, power, thrower,
      signed last): flag 1 from 3 counted 0x3F items on (flag < 1), flag 0 for 0x29.
      Only the first such tile: the scan stops."""
    typ, flag, thrower, last = typ & 0xFF, flag & 0xFF, thrower & 0xFF, last & 0xFF
    power = _i32(power)
    more = False
    if power < 100 and typ == 0x11:
        power = w.random(1000) + 500
    for t in range(0xC40):
        gi = w.tile8(t, 6)
        if gi >= 0xFF:
            continue
        o = G_GROUND_ITEMS + 12 * gi
        it = w.g8(o)
        if it != typ:
            continue
        if it == 0x11:
            w.ext("w_3c05c", t, 0)
            a = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR) + 0xB0
            w.s8(a, w.g8(a) - 1)
            more = w.g8(a) != 0
        else:
            chain = False
            if it in (0x29, 0x3F, 0x6E, 0x71):
                if it == 0x6E:
                    w.s8(o + 1, 1)
                if it in (0x6E, 0x71):
                    plant_done(w)
                chain = typ != 0x6E
            w.ext("explode", thrower, t, (s8(w.g8(o + 1)) + 1) & 0xFFFFFFFF, 1)
            if it == 0x3F:
                w.s32(G_BOMB_COUNT, w.g32(G_BOMB_COUNT) + 1)
            if chain:
                if _i32(w.g32(G_BOMB_COUNT)) >= 3 and s8(flag) < 1:
                    flag = 1
                if typ == 0x29:
                    flag = 0
                w.ext("fx_157bc", 0xC41, typ, flag, power & 0xFFFFFFFF, thrower, s8(last) & 0xFFFFFFFF)
        if not more:
            return


class _ItemBuf:
    """an item outside game memory (a stack local) for item_init: offsets 0.. of a scratch buffer"""
    def __init__(self, w, data):
        self.w, self.b = w, bytearray(data) + bytearray(0x100)

    def raw8(self, p):
        return self.b[p]

    def raw_s8(self, p, v):
        self.b[p] = v & 0xFF

    def random(self, k):
        return self.w.random(k)

    def g8(self, a):
        return self.w.g8(a)


def fixed_items(w):
    """0x2ACAC (sector 0x3B, first visit): the 11 items of FIXED_ITEMS, each a fresh item (type, count, condition)
    through FUN_53708(.., 0, 0), put on its tile (PutItem, no flags)."""
    for i in range(11):
        b = FIXED_ITEMS + 8 * i
        it = _ItemBuf(w, bytes([w.g8(b + 2), w.g8(b + 4), w.g8(b + 6)]) + bytes(9))
        item_init(it, 0, 0, 0)
        d = [int.from_bytes(it.b[o:o + 4], "little") for o in (0, 4, 8)]
        put_item(w, s16(w.g16(b)), d[0], d[1], d[2], 0, 0, 0, 0)


def guard_tile(w, tile, d, dist):
    """GuardTile? 0x2C70C (EAX tile, DL direction, EBX distance): `dist` steps from the tile in direction d; the first
    such tile that is standable, free (occupant >= 0x40) and of ground class neither 3 nor >= 8 wins. Else the next
    direction (1..8, wraps), one step farther after a full turn; the start tile when the distance passes 27 or after
    150 misses on ground class >= 8."""
    d0 = d = d & 0xFF
    misses = 0
    if dist > 0x1B:
        return tile
    while True:
        t = tile
        for _ in range(dist):
            t = step_tile(t, dir_step(w, d))
        c = w.g8(G_T0_CLASS + w.tile8(t, 0))
        if tile_standable(w, t) > 0 and _occupant(w, t) >= 0x40 and c != 3 and c < 8:
            return t
        d = (d + 1) & 0xFF
        if d > 8:
            d = 1
        if d == d0:
            dist += 1
        if c > 7:
            misses += 1
            if misses >= 0x96:
                return tile
        if dist > 0x1B:
            return tile


G_DAT_FILE = 0x92F54           # FILE* g_datFile: the data file open at the moment


EDGE_TILES = (0x240, 0x241, 0x242, 0x278, 0x2B0, 0x2B1, 0x2B2)     # 0x29F36: the 7 tiles of edge 9 (sector 0)


def edge_tile(w, d):
    """0x30490 (AL edge d, signed byte): a random free tile on the map edge d (1 bottom row 0xC08 + Random(56), 3 left
    column Random(56) x 56, 5 top row Random(56), 7 right column Random(56) x 56 + 55, 9 one of EDGE_TILES; any other
    d keeps the tile of the try before, -1 at first). A try wins when the tile is standable (> 0) and its occupant is
    0x40 or more, with ground t0 below 0x6C (after 10 tries that failed on ground >= 0x6C, any ground). -1 after
    1000 tries."""
    t, tries, deep = -1, 0, 0
    d = (d - 1) & 0xFF
    while tries < 1000:
        if d == 0:
            t = w.random(0x38) + 0xC08
        elif d == 2:
            t = w.random(0x38) * 0x38
        elif d == 4:
            t = w.random(0x38)
        elif d == 6:
            t = w.random(0x38) * 0x38 + 0x37
        elif d == 8:
            t = EDGE_TILES[w.random(7)]
        if tile_standable(w, t) > 0:
            if w.tile8(t, 0) < 0x6C or deep >= 10:
                if _occupant(w, t) >= 0x40:
                    return t
            else:
                deep += 1
        tries += 1
    return -1


def guard_enter(w, d):
    """0x30600 (AL edge d): a hired guard walks in at edge d. The first record 8..15 not in the sector (+0x182 = 0)
    takes it: edge_tile(d) (-1: result 0), the record = 0, SlotReset(k), InitGuard(tile, facing Random(8) + 1,
    orders 4, anchor 0xC41), InitSoldierNames, +0x1AB = 1; result 1. No free record: result 0."""
    k = next((k for k in range(8, 16) if w.rec(k)[0x182] == 0), 16)
    if k >= 16:
        return 0
    t = edge_tile(w, s8(d & 0xFF))
    if t == -1:
        return 0
    s = w.rec(k)
    s[0:0x200] = bytes(0x200)
    slot_reset(s, w, k)
    f = (w.random(8) + 1) & 0xFF
    init_guard(s, w, t, s8(f), 4, NO_TILE)
    init_soldier(s, w)
    s[0x1AB] = 1
    return 1


def enemy_walk_in(w, slot, d):
    """0x306BC (AL slot, DL edge d): an enemy walks into the current sector at edge d (strategic.md Enemy walk-in)."""
    slot &= 0xFF
    d &= 0xFF
    t = edge_tile(w, s8(d))
    if t == -1:
        return 0
    s = w.rec(slot)
    s[0:0x200] = bytes(0x200)
    slot_reset(s, w, slot)
    s[0x1E0] = w.random(4) & 0xFF
    s[0x1A1] = (w.random(8) + 1) & 0xFF if d == 9 else d
    s[0xBB] = 2
    for o in (0x124, 0x114, 0x12E):
        w16(s, o, t)
    s[0x182] = 1
    s[0x17B] = (w.random(6) + 0x11) & 0xFF
    s[0x17C] = (w.random(8) + 2) & 0xFF
    r = w.random(6)
    s[0x1D4] = 4
    s[0x17E] = 0
    s[0x1F3] = 0
    w16(s, 0x1E8, 4)
    w16(s, 0x134, NO_TILE)
    s[0x186] = s[0x188] = s[0x189] = s[0x19C] = 0
    w16(s, 0x12A, NO_TILE)
    s[0x185] = 0
    s[0x1BD] = 0x6D
    s[0x9C] = 0xFF
    s[0x1D3] = r & 0xFF
    s[0x1A2] = s[0x1A1]
    day = _i32(w.g32(G_DAY))
    n = 500 if day > 50 else (1000 - 10 * (day - 1)) & 0xFFFFFFFF
    r = w.random(s16(n & 0xFFFF))
    i = 8
    while i >= 0 and i * i * i > r:
        i -= 1
    s[0x192] = (9 - i) & 0xFF
    if w.g8(G_CUR_SECTOR) == 0:
        if s8(s[0x192]) < 6:
            s[0x192] = 6
    else:
        q = cdiv(day + 2, 3)
        if s8(s[0x192]) > q & 0xFF:
            s[0x192] = q & 0xFF
    for o in (0x183, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191):
        v = (s[0x192] * 5 + 0x23) & 0xFF
        v = (v + w.random(0xB)) & 0xFF
        s[o] = (v + w.random(0xA)) & 0xFF
    w16(s, 0x10C, 0)
    s[0x18A] = 100
    s[0x184] = s[0x183]
    s[0x1B4] = (cdiv(s8(s[0x192]), 2) + w.random(2)) & 0xFF
    if s[0x1B4] > 4:
        s[0x1B4] = 4
    equip(s, w, slot, 1)
    occupy(w, t, s[0x17A])
    w.ext("replace", s)
    init_soldier(s, w)
    return 1


def tmp_file(w):
    """SectorTmpFile 0x2BCF0: fopen("sec%02d.tmp", "rb") (u8 current sector) into g_datFile; without the file 0. Else
    one byte each of t1 t2 t3 t4 and the ground item byte (+1 +2 +3 +4 +6) per tile 0..3135, then the 0x960 bytes of
    the ground item table; fclose; 1."""
    fp = w.ext("fopen", "sec%02d.tmp" % w.g8(G_CUR_SECTOR), "rb") & 0xFFFFFFFF
    w.s32(G_DAT_FILE, fp)
    if not fp:
        return 0
    for t in range(0xC40):
        for o in (1, 2, 3, 4, 6):
            w.s8(G_TILEMAP + 8 * t + o, w.read(1)[0])
    for j, b in enumerate(w.read(0x960)):
        w.s8(G_GROUND_ITEMS + j, b)
    w.ext("fclose", fp)
    return 1


def setup_sector(w, tile, facing):
    """SetupSectorFromXra 0x2E918 (EAX entry tile, DL facing): sector globals reset, every slot reset off the map,
    sec%02d.tmp + move codes; an unowned sector loads its XRA record (enemies, ground items) and starts each present
    enemy (InitXraEnemy, equipment 0x2FCE4 with flag 2, the sector's special item to the first one); crate drops,
    plants, radio step, story objects and story items, tree states, mercs at the entry (team on duty), tappers
    around the trees, guards near the trees and around the map centre, names, then the trailing resets."""
    _sector_resets(w)
    for k in range(0x30):
        slot_reset(w.rec(k), w, k)
    tmp = tmp_file(w)
    w.ext("rebuild")
    cur = w.g8(G_CUR_SECTOR)
    sec = G_SECTORS + 0xB1 * cur
    if not w.g8(G_SECTOR_TBL9 + 9 * cur) & 1:
        _xra_load(w)
        if not tmp:
            _ground_items_load(w)
        w.ext("fclose", 0)
        if w.g8(G_PURIFIER) and cur != 0x3B:
            w.s8(sec + 0x9E, 0x79)
            w.s8(G_PURIFIER, 0)
        given = False
        for i in range(8):
            if s8(w.g8(sec + 0x96 + i)) > 0:
                s = w.rec(0x28 + i)
                init_xra_enemy(s, w)
                equip(s, w, 0x28 + i, 1 if w.g8(sec + 0x96 + i) == 2 else 0)
                it = w.g8(sec + 0x9E)
                if it and not given:
                    s[0x6C] = it
                    q = w.g8(ITEM_TBL + 12 * it + 5)
                    s[0x6D] = q if s8(q) < 99 else 99
                    s[0x6E] = 100
                    s[0x6F] = s[0x70] = s[0x71] = s[0x72] = 0
                    given = True
    crate_drops(w)
    plant_sector(w)
    w.s8(G_MISSION_STEP, w.ext("radio_result"))
    for k in range(5):
        t = s16(w.g16(sec + 0x82 + 2 * k))
        b = w.g8(sec + 0x9F + k)
        if t != NO_TILE and b:
            if b in (0xAC, 0xAD) and w.g8(G_STORY_STATE + 3) == 2:
                w.s8(G_TILEMAP + 8 * t + 1, 0)
                w.s8(sec + 0x9F + k, 0)
                w.s16(sec + 0x82 + 2 * k, NO_TILE)
                w.s8(0x22A057, 0xFF)
            else:
                w.s8(G_TILEMAP + 8 * t + 1, b)
            move_code_at(w, t)
        t = s16(w.g16(sec + 0x8C + 2 * k))
        if t != NO_TILE:
            w.s8(0x222B68, w.g8(sec + 0xA4 + k))
            w.ext("w_53708", w.ptr(0x222B68), 1, 2)
            if w.g8(0x222B68) == 0x86:
                w.s8(0x222B6B, 0x69)
                w.s16(0x222B6C, 0x6400)
                w.s8(0x222B6C, 99)
            put_item(w, t, w.g32(0x222B68), w.g32(0x222B6C), w.g32(0x222B70),
                     0xFFFFFFFF if w.g8(0x222B68) == 0x5E else 0, 1, 1, 0)
            w.s8(sec + 0xA4 + k, 0)
            w.s16(sec + 0x8C + 2 * k, NO_TILE)
    while w.g8(sec + 0xAE) and w.g8(sec + 0xAD):
        w.s8(sec + 0xAE, w.g8(sec + 0xAE) - 1)
        w.s8(sec + 0xAD, w.g8(sec + 0xAD) - 1)
    if w.g8(sec + 0xB0):
        ground_blast(w, 0x11, 0, 0, 0x40, 0)
    for t in range(0xC40):
        b = w.tile8(t, 1)
        if b == 0x35 or b == 0x36:
            if w.g8(sec + 0xAC):
                w.s8(G_TILEMAP + 8 * t + 1, 0x34)
                w.s8(G_TILEMAP + 8 * (t + 0x38) + 4, w.tile8(t + 0x38, 4) & 0xB7)
                w.s8(sec + 0xAC, w.g8(sec + 0xAC) - 1)
                if b == 0x36:
                    w.s8(sec + 0xAD, w.g8(sec + 0xAD) - 1)
            elif b == 0x35:
                if w.g8(sec + 0xAE):
                    w.s8(G_TILEMAP + 8 * t + 1, 0x36)
                    w.s8(sec + 0xAE, w.g8(sec + 0xAE) - 1)
            elif w.g8(sec + 0xAD):
                w.s8(G_TILEMAP + 8 * t + 1, 0x35)
                w.s8(sec + 0xAD, w.g8(sec + 0xAD) - 1)
    if w.g32(G_TEAM_ON_DUTY):
        merc_entry(w, tile, facing)
    w.s8(0x92DC7, w.g8(G_SELECTED))
    t9 = G_SECTOR_TBL9 + 9 * cur
    groups = (w.g8(t9 + 3) + 2) // 3
    done, done3, slot = 0, 0, 0x10
    for t in range(0xC3F, -1, -1):
        if w.tile8(t, 1) == 0x36 and done < groups:
            r = w.random(4) & 0xFF
            d = 1
            for j in range(min(w.g8(t9 + 3) - done3, 3)):
                if j == r:
                    d = (d + 2) & 0xFF
                init_tapper(w.rec(slot), w, step_tile(t, dir_step(w, d)), ((d + 3) % 8 + 1) & 0xFF)
                slot += 1
                d = (d + 2) & 0xFF
            done3 += 3
            done += 1
    gd = w.g8(t9 + 4)
    per = 0 if gd == 0 or groups == 0 else 1 if gd < groups else gd // groups
    per = min(per, 3)
    slot, placed = 8, 0
    if per:
        for t in range(0xC3F, -1, -1):
            if w.tile8(t, 1) == 0x36:
                for m in range(per):
                    c = guard_tile(w, t, GUARD_DIRS[m], w.random(3) + 2)
                    init_guard(w.rec(slot), w, c, (w.random(8) + 1) & 0xFF, 1, t)
                    slot += 1
                    placed += 1
                if placed >= groups * per:
                    break
    for m in range(gd - placed):
        if m >= 8:
            raise Undefined("leftover guard direction past the 8-byte table: stack garbage")
        c = guard_tile(w, 0x63C, GUARD_DIRS[m], w.random(11) + 5)
        init_guard(w.rec(slot), w, c, (w.random(8) + 1) & 0xFF, 3, NO_TILE)
        slot += 1
    for k in range(0x30):
        init_soldier(w.rec(k), w)
    if w.g32(G_TEAM_ON_DUTY):
        w.rec(w.g8(G_SELECTED))[0x1BD] = 0x6E
    _sector_tail(w)
    w.s32(0x92E38, 0)
    if cur == 0x3B and w.g8(G_SECTORS + 0xB1 * 0x3B + 0xA9) == 0:
        fixed_items(w)
    if w.g8(t9 + 2):
        w.s8(sec + 0xA9, 1)

G_CTRL_HELD = 0x284F1F
G_ITEM_TYPES_RANGE = 0x94E03  # g_itemTable + 0xB (= G_ITEM_RANGE)


def item_index(w, tile):
    """0x23768: ground item index on the tile (byte 6), 0xFF if 200+"""
    v = w.tile8(tile, 6)
    return 0xFF if v >= 0xC8 else v


def _face_point(s, w, tile):
    """DirectionTo from the soldier's pixel point to the tile's (x + 8, y + 7) (TacticalClick's facing)"""
    return _i32(w.ext("direction", s16(u16(s, 0x116)), s16(u16(s, 0x118)),
                      (tile - cdiv(tile, 56) * 56) * 16 + 8, cdiv(tile, 56) * 14 + 7)) & 0xFF


G_ITEM_JAMS = 0x94DFF         # g_itemTable + 7: != 0 = the item can jam


def jam_check(s, w):
    """0x59A54 -> 0 if the main-hand item jams: jamming items (not 0x11) in condition +2 above 15 jam when
    Random(80) + 20 > condition + 50; condition becomes Random(10) + 1, the selected merc's panel refreshes (0x55FD8)
    and message 15 shows. Else 1."""
    item = s[0]
    cond = s8(s[2])
    if w.g8(G_ITEM_JAMS + 12 * item) and cond > 15 and item != 0x11:
        if w.random(0x50) + 20 > cond + 50:
            s[2] = (w.random(10) + 1) & 0xFF
            if w.g8(G_SELECTED) == s[0x17A]:
                w.ext("item_panel", s, 0, 1)
            w.ext("merc_message", s, 0xF, 0x40, None, 1)
            return 0
    return 1


G_OBJ_BLOCK = 0x95694          # 9 bytes per tile object from 0x37, by direction 0..8 (ObjBlock 0x23780, BuryMine)


def bury_mine(s, w, tile):
    """BuryMine 0x3CB24 (EAX = soldier, EDX = tile; animation op 0x1E9 when the current animation is not 0xC): a
    target tile not under the soldier moves one more step in the soldier's direction to it when byte
    [object - 0x37][direction] of 0x95694 is 0xB. Hand item 0x11 (mine): condition 10 or more arms it (byte 1 = 1),
    PutItem with byte 6 = -1, the sector's mine count (+0xAF) rises, "Mine buried"; a weaker one is only put down.
    Any other item (an explosive set off in place): condition 0xFF, +0x1C9 = its blast kind, effect 0x157BC(tile,
    kind, combat x (Random(2) + 1) as a byte, 0x2134 - 6000 x combat, 0x42, 1), PutItem. Both stat practice 6 (5)
    except the weak mine. Then 0x17B70(slot, 0), DropItem, and the item panel for the selected merc."""
    me = s16(u16(s, 0x114))
    if tile != me:
        d = w.ext("direction", tile_x(me), tile_y(me), tile_x(tile), tile_y(tile)) & 0xFF
        o = w.tile8(tile, 1)
        if s8(w.g8((G_OBJ_BLOCK + (o - 0x37) * 9 + d) & 0xFFFFFFFF)) == 0xB:
            tile = step_tile(tile, dir_step(w, d))

    def put(b6):
        w.ext("put_item", tile, 0, 0, 0, _u32(s, 0), _u32(s, 4), _u32(s, 8), b6, 1, 0, 1)

    if s[0] == 0x11:
        if s8(s[2]) < 10:
            put(1)
        else:
            s[1] = 1
            put(0xFFFFFFFF)
            a = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR) + 0xAF
            w.s8(a, (w.g8(a) + 1) & 0xFF)
            w.ext("message", "Mine buried")
            w.ext("stat_practice", s, 6, 5, 0)
    else:
        s[2] = 0xFF
        s[0x1C9] = explosive_kind(s[0])
        c = s8(w.g8(G_COMBAT))
        k = (w.g8(G_COMBAT) * (w.random(2) + 1)) & 0xFF
        w.ext("fx_157bc", s16(tile), s[0x1C9], k, (0x2134 - 6000 * c) & 0xFFFFFFFF, 0x42, 1)
        put(1)
        w.ext("stat_practice", s, 6, 5, 0)
    w.ext("w_17b70", s[0x17A], 0)
    w.ext("drop_item", s)
    if w.g8(G_SELECTED) == s[0x17A]:
        w.ext("item_panel", s, 0, 1)


def gun_jam(s, w):
    """0x58574 (EAX = soldier; animation op 0x1C1): mercs only (slot below 8), not while +0x1C9 is set. A loaded
    hand item (byte 1 above 0) jams unless Random(100) < condition (byte 2, signed) + Random(50): byte 1 is negated
    (negative = jammed), focus, voice and click, the panel refreshes, the attack is forgotten (0x56ED0) and message
    0x10 shows. A jammed item (byte 1 below 0) clears when Random(100) < 20 + half of +0x190 (when above 0): byte 1
    is negated back, panel, voice and click, stat practice 7 (2); else focus, voice and click, attack forgotten."""
    if s[0x17A] >= 8 or s[0x1C9]:
        return
    c = s8(s[1])
    if c == 0:
        return
    if c > 0:
        v = s8(s[2]) + w.random(0x32)
        if w.random(0x64) < v:
            return
    else:
        skill = s8(s[0x190])
        if w.random(0x64) < (cdiv(skill, 2) + 0x14 if skill > 0 else 0x14):
            s[1] = -s8(s[1]) & 0xFF
            w.ext("item_panel", s, 0, 1)
            w.ext("cry", s[0x179], 0xFFFFFFFF, "GOOD", 1, 1)
            w.ext("voc", 0x2B11, 0x32, 1)
            w.ext("stat_practice", s, 7, 2, 0)
            return
    w.ext("focus", s[0x17A], 1)
    w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
    w.ext("voc", 0x2B11, 0x32, 1)
    if c > 0:
        s[1] = -s8(s[1]) & 0xFF
        w.ext("item_panel", s, 0, 1)
        clear_target(s, w, 1)
        w.ext("merc_message", s, 0x10, 0x40, None, 1)
    else:
        clear_target(s, w, 1)


def start_attack(s, w, tile):
    """0x5ABE0 (EAX = soldier, EDX = tile): only at attack stage 1 (+0x1BE). Faces the tile (+0x1A2 from 0x5260C, its
    word output at +0x10A) and raises the weapon (0x5AA08). A loaded hand item (byte 1) moves the stage to 2 with the
    attack tile +0xDC. An empty item 0..0xE runs MsgReloaded? and, if that answers, ends the shot (0x56DD4 with 1, 0)
    and forgets the attack. Else +0x1C9 = the hand item's blast kind; a merc aiming at another merc (occupant below
    8) gets message 8 once per pair (+0x162 + occupant, unless +0x19C is 3) and both pair flags are set; +0x1D8 =
    0x40."""
    if s[0x1BE] != 1:
        return
    me = s16(u16(s, 0x114))
    d, word = aim_dir(tile_x(me), tile_y(me), tile_x(tile), tile_y(tile))
    w16(s, 0x10A, word)
    s[0x1A2] = d & 0xFF
    w.ext("w_5aa08", s, tile)
    occ = w.tile8(tile, 5) & 0x7F
    occ = occ if occ < 0x30 else 0x40
    if s[1]:
        s[0x1BE] = 2
        w16(s, 0xDC, tile)
    if s8(s[0]) <= 0xE and not s[1]:
        if _i32(w.ext("reloaded", s)):
            w.ext("w_56dd4", s, 1, 0)
            clear_target(s, w, 1)
            return
    s[0x1C9] = explosive_kind(s[0])
    if occ >= 0x40 or s[0x17A] >= 8 or occ >= 8:
        return
    if not s[0x162 + occ] and s[0x19C] != 3:
        w.ext("merc_message", s, 8, occ, None, 1)
    s[0x162 + occ] = 1
    w.s8(G_SOLDIERS + 0x200 * occ + 0x162 + s[0x17A], 1)
    s[0x1D8] = 0x40


def fire_gun(s, w, tile):
    """FireGun 0x5959C (EAX = soldier, EDX = tile; AttackGo): the shot's cost, end point and to-hit roll. In combat
    (0x92DCB) an enemy (slot >= 8) the player sees (+0x9C > 0) gets the view focus (FocusOn(slot, 0)), and the word
    0x92E4C = tile. Knives (s8 hand item 0xF / 0x10) stop here. SpendAP(s, s8(ShotApCost(s, tile) + s8 aim +0x1FD),
    attack sound of the weapon class). A soldier on the tile: end point = its pixel + (2, 4), height by s8
    Random(20): < 16 -> 2500 + Random(2500), < 19 -> 5000 + Random(1001), else Random(2500); / 6 (s16) when its
    animation is <= 3 or >= 0x39. No soldier: the tile's screen point, y + 5 for facing +0x1A2 1, 2, 8, y - 5 for
    4..6; height by the object's class: 6..8 5000, 9 5000 for objects 0xE2 / 0xE3 else 2000, other 0. Then
    chance = ChanceToHit(s, tile, +0x1FD), r = Random(100) + 1 (bytes, signed compare): r <= chance hits (+0x1BC =
    1, +0xDC = tile, +0x1FC = chance - r); else +0x1BC = 0, +0x1FC = 0, d = Distance(own px, end point),
    ThrowScatter(s, r - chance, d), height += -/+ (s16 +0xE0 x s8(r - chance) / 100) (minus on Random(2) != 0) - d,
    not below 0 (all 16 bits). Last: +0xD0 = tile."""
    from combat import distance
    if w.g8(G_COMBAT):
        if s[0x17A] >= 8 and s8(s[0x9C]) > 0:
            w.ext("focus", s[0x17A], 0)
        w.s16(0x92E4C, tile)
    item = s8(s[0])
    if item in (0xF, 0x10):
        return
    cost = w.ext("shot_ap", s, tile) & 0xFF
    snd = s16(w.g16(G_ATTACK_SND + 2 * w.g8(WEAPON_TBL + 6 * item)))
    w.ext("spend_ap", s, s8((cost + s8(s[0x1FD])) & 0xFF) & 0xFFFFFFFF, snd & 0xFFFFFFFF)
    occ = _occupant(w, tile)
    if occ < 0x40:
        o = s if occ == s[0x17A] else w.rec(occ)
        w16(s, 0xDE, u16(o, 0x116) + 2)
        w16(s, 0xE0, u16(o, 0x118) + 4)
        r = s8(w.random(20) & 0xFF)
        if r < 16:
            h = w.random(2500) + 2500
        elif r < 19:
            h = w.random(1001) + 5000
        else:
            h = w.random(2500)
        w16(s, 0xE2, h)
        anim = u16(o, 0x1E8 + 2 * o[0x1F3])
        if anim <= 3 or anim >= 0x39:
            w16(s, 0xE2, cdiv(s16(u16(s, 0xE2)), 6))
    else:
        w16(s, 0xDE, tile_x(tile))
        w16(s, 0xE0, tile_y(tile))
        f = s[0x1A2]
        if f in (1, 2, 8):
            w16(s, 0xE0, u16(s, 0xE0) + 5)
        elif f in (4, 5, 6):
            w16(s, 0xE0, u16(s, 0xE0) - 5)
        obj = w.tile8(tile, 1)
        c = w.g8(G_T1_CLASS_TBL + obj)
        if 6 <= c <= 8 or (c == 9 and obj in (0xE2, 0xE3)):
            w16(s, 0xE2, 5000)
        elif c == 9:
            w16(s, 0xE2, 2000)
        else:
            w16(s, 0xE2, 0)
    chance = s8(w.ext("chance_to_hit", s, tile, s[0x1FD]) & 0xFF)
    r = s8((w.random(100) + 1) & 0xFF)
    if r <= chance:
        s[0x1BC] = 1
        w16(s, 0xDC, tile)
        s[0x1FC] = (chance - r) & 0xFF
    else:
        s[0x1BC] = 0
        m = (r - chance) & 0xFF
        s[0x1FC] = 0
        d = distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(u16(s, 0xDE)), s16(u16(s, 0xE0)))
        throw_scatter(s, w, m, s16(d & 0xFFFF))
        dh = cdiv(s16(u16(s, 0xE2)) * s8(m), 100)
        if w.random(2):
            dh = -dh
        h = (u16(s, 0xE2) + dh - d) & 0xFFFF
        w16(s, 0xE2, h)
        if s16(h) < 0:
            w16(s, 0xE2, 0)
    w16(s, 0xD0, tile)


def chance_to_hit(s, w, tile, aim):
    """ChanceToHit 0x56630 (EAX = shooter, DX = tile, BL = aim clicks) -> 1..99: combat.chance_to_hit on the record
    copy s, everything else (target record, tables) read through w."""
    import combat
    return combat.chance_to_hit(bytes(s), s16(tile), aim & 0xFF, lambda a, n: bytes(w.g8(a + i) for i in range(n)))


SIGHT_FLAG = 0x58     # AttackGo's sight-line flag: the caller's BL, the low byte of the soldier record pointer


def attack_go(s, w):
    """AttackGo? 0x5AD50 (EAX = soldier; AnimFrame op 0x1EE after StartAttack): only at attack stage 2 (+0x1BE) in
    an aiming animation (0x1B, 0x1F, 0x23) with an attack tile +0xDC (not 0xC41): FireGun(s, tile), then occ = the
    occupant of the (new) +0xDC. The skip distance +0x1FF = ChebPx from the soldier's pixel: to the occupant's pixel
    when the shooter sees it (s8 +0x13A + occ (- 0x28 for enemies) = 1; an enemy target, or any target of an enemy
    shooter), else to the end point +0xDE / +0xE0 when SightLine(s, x + 2, y + 4, +0xDC, +0xDE, +0xE0, flag, 14,
    occ (a merc on merc) / 0x40 (no soldier)) answers non-zero. The fire animation by the hand item (u8): <= 7
    0x2D when the attack tile is within 1 tile (TileDistance < 2) and holds no soldier or one in animation < 4, else
    0x1C; <= 0xE 0x20; <= 0x10 0x24 (in combat an enemy gets the focus); else none. SetAnim(s, code, 0, 1)."""
    if s[0x1BE] != 2:
        return
    if u16(s, 0x1E8 + 2 * s[0x1F3]) not in (0x1B, 0x1F, 0x23):
        return
    tile = s16(u16(s, 0xDC))
    if tile == NO_TILE:
        return
    fire_gun(s, w, tile)
    t = s16(u16(s, 0xDC))
    occ = _occupant(w, t)
    x, y = s16(u16(s, 0x116)), s16(u16(s, 0x118))
    ex, ey = s16(u16(s, 0xDE)), s16(u16(s, 0xE0))
    near = None
    if occ < 0x30 and not (occ < 0x28 and s[0x17A] < 0x28):
        if s8(s[0x13A + (occ - 0x28 if occ >= 0x28 else occ)]) == 1:
            o = s if occ == s[0x17A] else w.rec(occ)
            near = (s16(u16(o, 0x116)), s16(u16(o, 0x118)))
    elif _i32(w.ext("sight_line", s, (x + 2) & 0xFFFFFFFF, (y + 4) & 0xFFFFFFFF, t & 0xFFFFFFFF, ex & 0xFFFFFFFF,
                    ey & 0xFFFFFFFF, SIGHT_FLAG, 14, occ if occ < 0x30 else 0x40)):
        near = (ex, ey)
    if near is not None:
        s[0x1FF] = cheb_px(x, y, near[0], near[1]) & 0xFF
    item = s[0]
    if item <= 7:
        code = 0x1C
        if _i32(w.ext("tile_dist", t & 0xFFFFFFFF, s16(u16(s, 0x114)) & 0xFFFFFFFF)) < 2:
            if occ >= 0x30:
                code = 0x2D
            else:
                o = s if occ == s[0x17A] else w.rec(occ)
                if u16(o, 0x1E8 + 2 * o[0x1F3]) < 4:
                    code = 0x2D
    elif item <= 0xE:
        code = 0x20
    elif item <= 0x10:
        code = 0x24
        if w.g8(G_COMBAT) and s[0x17A] >= 8:
            w.ext("focus", s[0x17A], 0)
    else:
        return
    w.ext("set_anim", s, code, 0, 1)


def aim_posture(s, w, tile):
    """0x5AA08: raise the weapon toward `tile` (when the animation can aim and the soldier is not in water): the
    direction comes from 0x5260C (floating point; it also writes the word at +0x10A); a loaded gun / knife in the
    main hand switches to its aiming animation unless already in it (pistols 1..7 and odd bytes: 0x1A..0x1C, rifles
    8..14: 0x1E..0x20, knives 0xF..0x10: 0x22..0x24; SetAnim(code, 0, 1)); a new direction becomes the turn target
    (with sound 1 / 10 for items below 0xF when no animation started)."""
    anim = s16(u16(s, 0x1E8 + 2 * s[0x1F3]))
    if not w.g8(ANIM_CAN_AIM + 8 * anim) or is_water_tile(s[0x1B2], s[0x1B4]):
        return
    it = s[0] if s[1] else 0
    me = s16(u16(s, 0x114))
    d, word = aim_dir(tile_x(me), tile_y(me), tile_x(tile), tile_y(tile))
    w16(s, 0x10A, word)
    d &= 0xFF
    code = 0
    if it and s8(it) <= 0x10:
        if s8(it) >= 0xF:
            code = 0 if 0x22 <= anim <= 0x24 else 0x22
        elif s8(it) >= 8:
            code = 0 if 0x1E <= anim <= 0x20 else 0x1E
        else:
            code = 0 if 0x1A <= anim <= 0x1C else 0x1A
    if code:
        w.ext("set_anim", s, code, 0, 1)
    if d != s[0x1A2]:
        if s[0] < 0xF and not code:
            w.ext("spend_ap", s, 1, 0xA)
        s[0x1A2] = d


def use_cost(s, w, spot, tile, gidx, kind):
    """0x59AE8 -> AP (byte) of walking to `spot` (PathApCost) plus the use: hand / kind 0 or 0xFE+: +4 standable
    tile, +3 unstandable, else by the ground item (+5 none or index 200+, +4 when its byte 6 is set, +2 for an
    enemy at item 0x72, else +5); kind 0x76 / 0x9B +10, 0x96 + ShotApCost, 0x93 / 0x9E +5, 0x9D +6, others +0."""
    p = _i32(w.ext("path_ap", s, spot)) & 0xFF
    kind &= 0xFF
    if kind == 0 or kind >= 0xFE:
        ts = s8(tile_standable(w, tile) & 0xFF)
        if ts > 0:
            return (p + 4) & 0xFF
        if ts:
            return (p + 3) & 0xFF
        gidx &= 0xFF
        if gidx >= 200:
            return (p + 5) & 0xFF
        if w.g8(G_GROUND_ITEMS + 12 * gidx + 6):
            return (p + 4) & 0xFF
        if s[0x17A] >= 0x28 and w.g8(G_GROUND_ITEMS + 12 * gidx) == 0x72:
            return (p + 2) & 0xFF
        return (p + 5) & 0xFF
    if kind in (0x76, 0x9B):
        return (p + 10) & 0xFF
    if kind == 0x96:
        return (p + _i32(w.ext("shot_ap", s, tile))) & 0xFF
    if kind in (0x93, 0x9E):
        return (p + 5) & 0xFF
    if kind == 0x9D:
        return (p + 6) & 0xFF
    return p


def tactical_click(s, w, tile, item):
    """TacticalClick? 0x59C14 -> 1 when a walk to a use spot starts: the soldier uses `item` (0 = hand) at `tile`.
    Nothing (0) while the current animation cannot act. Reach: hand 15 px (range 1), throwables (kind 0x95) range x
    15 + hp - 50, less (100 - energy) / 200 of it, range = reach / 15; others range x 15 (24 for range < 2). Out of
    reach (for the hand, items above 0x23 except 0x5D, or range 1), or range 1 to a diagonal / standable other tile:
    a walk is needed (the item kept as the task +0x1A6, +0x1A7 = range): range > 1 walks to the tile; range 1
    checks the task (knives / first aid need someone there, 0x45 needs a door-class object), reaches a spot
    (ReachTile), and walks there if its AP cost (0x59AE8) is affordable and the spot is dry. In reach, guns (1..14)
    and knives aim at the tile (attacker fields), throwables / 0x5D aim, others start their use animations behind an
    AP check (hand: pick up / search 0xB, 0x26, 0xC; 0x4C / 0x57..0x58: 0x26; 0x11 and 0x17 / 0x19 / 0x1B / 0x1E..
    0x20: 0x12 / 0x13; 0x76, 0x87: 0xC; 0x45: 0x2E; 0x46 drinks (sound, one charge less); 0x4D moves; 0x49 / 0x4A
    first aid), unknown items clear the task. A done click without Ctrl held updates the merc panel (0x1C438)."""
    s[0xB0:0xB4] = bytes(4)
    occ = _occupant(w, tile)
    gidx = item_index(w, tile)
    if not w.g8(ANIM_CAN_ACT + 8 * u16(s, 0x1E8 + 2 * s[0x1F3])):
        return 0
    item &= 0xFF
    kind = w.g8(G_ITEM_KIND + 12 * item)
    if item == 0:
        rng, reach, kind = 1, 0xF, 0
    elif kind == 0x95:
        r = w.g8(G_ITEM_TYPES_RANGE + 12 * item) * 15 + s8(s[0x183]) - 50
        r = max(r - cdiv((100 - s8(s[0x18A])) * r, 200), 0)
        reach = r & 0xFFFF
        rng = cdiv(s16(reach), 15) & 0xFF
    else:
        rng = w.g8(G_ITEM_TYPES_RANGE + 12 * item)
        reach = rng * 15 if rng > 1 else 0x18
    me = s16(u16(s, 0x114))
    walk = False
    from combat import distance
    if distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(tile), tile_y(tile)) > s16(reach):
        if item == 0 or (item > 0x23 and item != 0x5D) or rng == 1:
            walk = True
            if item:
                s[0x1A6] = item
    else:
        diag = rng == 1 and (me - cdiv(me, 56) * 56) != (tile - cdiv(tile, 56) * 56) and cdiv(me, 56) != cdiv(tile, 56)
        if diag or (rng == 1 and tile_standable(w, tile) > 0 and me != tile):
            walk = True
            if item:
                s[0x1A6] = item
    if not w.g8(G_COMBAT):
        w16(s, 0x134, tile)
    if walk:
        s[0x1A7] = rng
        if rng != 1:
            w.ext("walk_to", s, tile)
            s[0x1A6] = s[0]
            s[0x1A7] = rng
            return 0
        task, abort = s[0x1A6], False
        if task in (0xF, 0x10, 0x49, 0x4A):
            if occ == 0x40:
                w.ext("message", "No one there")
                abort = True
                s[0x1A6] = 0
        elif task == 0x45:
            c = w.g8(G_T1_CLASS_TBL + w.tile8(tile, 1))
            if c in (7, 8):
                s[0x1A6] = 0x45
            else:
                abort = True
                s[0x1A6] = 0
        if not _i32(w.ext("reach_ap", s, tile, s[0x1A6])):
            w.ext("message", "Can't get there!")
            s[0x1A6] = 0
            return 0
        if abort:
            return 0
        spot = s16(w.g16(G_MISSION_SPOT))
        ap = _i32(w.ext("w_59ae8", s, spot, s16(tile), gidx, kind)) & 0xFF
        if msg_not_enough_ap(s, w, ap, 0) and not is_water_tile(w.tile8(spot, 0), w.tile8(spot, 1)):
            s[0x1A6] = 0xFF if not kind else item
            w16(s, 0x134, tile)
            w.ext("walk_to", s, spot)
            if not w.g8(G_CTRL_HELD):
                w.ext("w_1c438", s, 1)
            return 1
        w16(s, 0x134, NO_TILE)
        s[0x1A6] = 0
        return 0
    done = False

    def finish():
        if done and not w.g8(G_CTRL_HELD):
            w.ext("w_1c438", s, 1)
        return 0
    if 0 < item <= 0xE:
        w.ext("w_5aa08", s, tile)
        if not _i32(w.ext("w_59a54", s)):
            w.ext("w_1c438", s, 1)
            return 0
        if _i32(w.ext("attack_ready", s, tile)) == 1:
            s[0x1D8] = occ
            if occ < 0x40:
                if s8(w.g8(G_SOLDIERS + 0x200 * occ + 0x9C)) > 0:
                    s[0x1C2] = occ
                    if s[0x17A] < 0x28 and occ < 0x28:
                        s[0x1C3] = 1
            else:
                s[0x1C2] = 0xFE
            s[0x1BE] = 1
            w16(s, 0xDC, tile)
            w.s32(0x229DD4, 40000)
            done = True
        elif not s[1] and _i32(w.ext("reloaded", s)) and s[0x17A] < 8:
            w.ext("merc_message", s, 5, 0x40, None, 1)
            s[0x1CD] = 1
        return finish()
    ap, snd = 0, 0
    if item and not _i32(w.ext("w_59a54", s)):
        w.ext("w_1c438", s, 1)
        return 0
    if in_water(s):
        w.ext("w_1c438", s, 1)
        return 0

    def use(a, cost, sound, task, anim, reset, face):
        nonlocal done
        if msg_not_enough_ap(s, w, cost, 0):
            s[0x1A6] = task
            w16(s, 0x134, tile)
            if face is not None:
                pass
            w.ext("set_anim", s, anim, 0, reset)
            if sound is not None:
                w.ext("spend_ap", s, s8(cost & 0xFF), sound & 0xFFFFFFFF)
            if face is not None:
                s[0x1A2] = face
        done = True
    if item == 0:
        if tile == me:
            use(0, 4, 0, 0xFF, 0xB, 0, None)
        else:
            d = _face_point(s, w, tile)
            if tile_standable(w, tile) < 0:
                use(0, 3, 0x1E, 0xFF, 0x26, 0, d)
            else:
                c = 2 if s[0x17A] >= 0x28 and w.g8(G_GROUND_ITEMS + 12 * gidx) == 0x72 else 5
                use(0, c, 0, 0xFF, 0xC, 0, d)
        return finish()
    if item in (0x4C, 0x57, 0x58):
        cost, sound = (3, 0x1E) if item != 0x4C else (10, -250)
        d = _face_point(s, w, tile)
        if tile_standable(w, tile) < 0 or (item == 0x4C and _i32(w.ext("quest_check", s16(tile))) & 0xFF):
            use(0, cost, sound, item, 0x26, 0, d)
        done = True
        return finish()
    if item == 0x4D:
        move_merc(s, w, tile, 1, 0, 1, 0, 1, 0)
        return finish()
    if item in (0xF, 0x10):
        if tile == me:
            return finish()
        if msg_not_enough_ap(s, w, _i32(w.ext("action_ap", s, tile, 1)) & 0xFF, 0):
            w.ext("w_5aa08", s, tile)
            if _i32(w.ext("attack_ready", s, tile)) == 1:
                if occ < 0x40:
                    s[0x1D8] = s[0x1C2] = occ
                    if s[0x17A] < 0x28 and occ < 0x28:
                        s[0x1C3] = 1
                s[0x1BE] = 1
                w16(s, 0xDC, tile)
                done = True
        return finish()
    if item in (0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D, 0x5D):
        if msg_not_enough_ap(s, w, _i32(w.ext("action_ap", s, tile, 1)) & 0xFF, 0):
            d = _face_point(s, w, tile)
            s[0x1C2] = 0xFE
            s[0x1A2] = d
            w16(s, 0xDC, tile)
            done = True
            s[0x1BE] = 1
            w.ext("w_1c438", s, 1)
        return finish()
    if item in (0x49, 0x4A):
        if msg_not_enough_ap(s, w, 5, 0):
            if tile == me:
                d = 5
                r = _i32(w.ext("first_aid", s, me, 6, 5)) & 0xFF
            else:
                d = _face_point(s, w, tile)
                r = _i32(w.ext("first_aid", s, tile, 5, d)) & 0xFF
            done = bool(r)
            clear_task(s, 1)
            w.ext("w_1c438", s, 1)
            if not r:
                s[0x1A2] = d
            else:
                w16(s, 0x120, NO_TILE)
        return finish()
    if item == 0x11 or item in (0x17, 0x19, 0x1B, 0x1E, 0x1F, 0x20):
        if item == 0x11:
            if tile != me and tile_standable(w, tile) <= 0:
                w.ext("message", "Can't bury mine there!")
                clear_task(s, 1)
                done = True
                return finish()
            cost, sound = 10, 0xFA
        else:
            cost, sound = 6, 0x14
        d = _face_point(s, w, tile) if tile != me else None
        if msg_not_enough_ap(s, w, cost, 0):
            s[0x1A6] = item
            w16(s, 0x134, tile)
            if tile == me:
                w.ext("set_anim", s, 0x13, 0, 1)
            else:
                s[0x1A2] = d
                w.ext("set_anim", s, 0x12, 0, 1)
            w.ext("spend_ap", s, s8(cost), sound & 0xFFFFFFFF)
        done = True
        return finish()
    if item == 0x76:
        if tile == me:
            return finish()
        d = _face_point(s, w, tile)
        use(0, 10, 0, 0x76, 0xC, 0, d)
        return finish()
    if item == 0x45:
        if tile == me:
            return finish()
        c = w.g8(G_T1_CLASS_TBL + w.tile8(tile, 1))
        if c not in (7, 8):
            s[0x1A6] = 0
            return finish()
        s[0x1A6] = 0x45
        w16(s, 0x134, tile)
        d = _face_point(s, w, tile)
        if msg_not_enough_ap(s, w, 5, 0):
            s[0x1A2] = d
            w.ext("set_anim", s, 0x2E, 0, 1)
        done = True
        return finish()
    if item == 0x87:
        if tile == me:
            return finish()
        d = _face_point(s, w, tile)
        if msg_not_enough_ap(s, w, 8, 0):
            s[0x1A6] = 0x87
            s[0x1A2] = d
            w16(s, 0x134, tile)
            w.ext("set_anim", s, 0xC, 0, 1)
            w.ext("spend_ap", s, 8, (-50) & 0xFFFFFFFF)
        else:
            clear_task(s, 1)
        done = True
        return finish()
    if item == 0x46:
        if s8(s[1]) > 0 and w.random(100) < s8(s[2]):
            w.ext("sfx", 0x2C, 0x2B11, 0x58, 1)
            if w.g8(0x1163BD):
                w.ext("message", "drink")
            s[1] = (s[1] - 1) & 0xFF
            if w.g8(G_SELECTED) == s[0x17A]:
                w.ext("item_cond", 0, s, 0, 1)
    done = True
    clear_task(s, 1)
    return finish()


G_WPN_TRACKING = 0x94C35      # g_weaponStats + 5 (6 B per item): bullet tracking -> +0x1CB
G_SHOT_SOUND = 0x94DDC        # 2 B per item: sound id (< 0x63 plays), volume
G_ITEM_WEAR = 0x94DFD         # g_itemTable + 3: wear factor (condition - 1 with chance 1 / (2 x it))
G_SILENCED = 0x92F00          # u8 per item: silencer quality (sound volume by attachment condition)


def launch_projectile(s, w):
    """LaunchProjectile 0x58DE4: fire the main-hand item from (x + 2, y + 4) toward the shot end point (+0xDE / +0xE0,
    height +0xE2). Nothing with a negative count or when the end lies within 1 px. Projectile fields: +0xC8 flown 0,
    +0x1BF = 3, +0x1F5 item, +0x1E6 target tile, +0x1C6 facing, attack fields reset (second target 0xFE / merc-side
    flag -> none). Throwables: height 8500 (+0xF2), cap +0xCA 9000 (terrain class 3) or 22000, slopes up +0xE8 and down
    +0xEA over half the distance, +0x1CB = 2, mercs practise stats 3 and 6. Bullets: cap 0, +0xCC = 15, height 4500 +
    Random(1001), slope +0xE8 toward the end height, tracking +0x1CB from the weapon, mercs practise stat 8.
    Bresenham steps (+0xF8 error, +0xFA/+0xFC signs, +0xFE/+0x100 major steps, +0x102/+0x104 increments), the busy
    counter + 1, 0x92DB4 = 5000, the shot sound (silenced guns by attachment condition). Mercs and throwables: gun
    wear (items 1..14: condition - 1 on Random(2 x wear) = 0), +0x1E4 = condition, ammo - 1 (throwables 0x535E8), the
    last round empties the gun (+0x1CD = 0, +0xB4 = 2000) or drops the item (0x470B0); panel. Within 20 px the
    projectile lands at once (0x56DD4, ProjectileImpact)."""
    if s8(s[1]) < 0:
        return
    w16(s, 0xE4, u16(s, 0x116) + 2)
    w16(s, 0xE6, u16(s, 0x118) + 4)
    dist = cheb_px(s16(u16(s, 0xE4)), s16(u16(s, 0xE6)), s16(u16(s, 0xDE)), s16(u16(s, 0xE0)))
    w16(s, 0xC8, 0)
    if dist < 2:
        return
    s[0x1BF] = 3
    s[0x1F5] = s[0]
    w16(s, 0x1E6, u16(s, 0xDC))
    s[0x1C6] = s[0x1A1]
    s[0x1BE] = 0
    if s[0x1C2] == 0xFE:
        s[0x1C2] = 0x40
    if s[0x1C3]:
        s[0x1C3] = 0
        s[0x1C2] = 0x40
    slope = lambda d, n: cdiv((-1 if d < 0 else 1) + cdiv(2 * d, n), 2)
    if w.g8(G_ITEM_KIND + 12 * s[0x1F5]) == 0x95:
        w16(s, 0xF2, 0x2134)
        w16(s, 0xCA, 9000 if s[0x1B3] == 3 else 22000)
        w16(s, 0xE8, slope(s16(u16(s, 0xCA)) - s16(u16(s, 0xF2)), cdiv(dist, 2)))
        w16(s, 0xEA, slope(s16(u16(s, 0xE2)) - s16(u16(s, 0xCA)), cdiv(dist, 2)))
        s[0x1CB] = 2
        if s[0x17A] < 8:
            w.ext("stat_practice", s, 3, 2, 0)
            w.ext("stat_practice", s, 6, 2, 0)
    else:
        w16(s, 0xCA, 0)
        s[0xCC:0xD0] = bytes([0xF, 0, 0, 0])
        w16(s, 0xF2, w.random(0x3E9) + 0x1194)
        w16(s, 0xE8, slope(s16(u16(s, 0xE2)) - s16(u16(s, 0xF2)), dist))
        s[0x1CB] = w.g8(G_WPN_TRACKING + 6 * s[0])
        if s[0x17A] < 8:
            w.ext("stat_practice", s, 8, 1, 0)
    dx = (u16(s, 0xDE) - u16(s, 0xE4)) & 0xFFFF
    dy = (u16(s, 0xE0) - u16(s, 0xE6)) & 0xFFFF
    s[0x1FB] = 1
    w16(s, 0xFA, 0xFFFF if s16(dx) < 0 else 1)
    dx = (-dx) & 0xFFFF if s16(dx) < 0 else dx
    w16(s, 0xFC, 0xFFFF if s16(dy) < 0 else 1)
    dy = (-dy) & 0xFFFF if s16(dy) < 0 else dy
    if s16(dx) < s16(dy):
        w16(s, 0xFE, 0)
        major, minor = dy, dx
        w16(s, 0x100, u16(s, 0xFC))
    else:
        w16(s, 0x100, 0)
        w16(s, 0xFE, u16(s, 0xFA))
        major, minor = dx, dy
    w16(s, 0x104, 2 * minor)
    w16(s, 0xF8, 2 * minor - major)
    w16(s, 0x102, 2 * minor - 2 * major)
    w.s32(G_BUSY, w.g32(G_BUSY) + 1)
    w.s32(0x92DB4, 5000)
    it = s[0x1F5]
    if it < 0x11:
        if s[3] == 0x5B:
            q = cdiv(100, w.g8(G_SILENCED + it) - 1)
            vol = (((cdiv(100 - s8(s[5]), q) + 1) & 0xFF) * 10 + 0x14) & 0xFF
            w.ext("sfx", 0x17, 0x2B11, min(vol, 0x78), 1)
        elif w.g8(G_SHOT_SOUND + 2 * it) < 0x63:
            w.ext("sfx", w.g8(G_SHOT_SOUND + 2 * it), 0x2B11, s8(w.g8(G_SHOT_SOUND + 2 * it + 1)) & 0xFFFFFFFF, 1)
    if s[0x17A] < 8 or w.g8(G_ITEM_KIND + 12 * s[0]) == 0x95:
        n = s8(w.g8(G_ITEM_WEAR + 12 * s[0])) * 2
        if s[0] <= 0xE and w.random(s16(n)) == 0:
            s[2] = (s[2] - 1) & 0xFF
        s[0x1E4] = s[2]
        if w.g8(G_ITEM_KIND + 12 * s[0]) == 0x95:
            w.ext("use_throw", s)
        else:
            s[1] = (s[1] - 1) & 0xFF
        if not s[1]:
            if s[0x1F5] <= 0xE:
                s[0x1CD] = 0
                s[0xB4:0xB8] = (2000).to_bytes(4, "little")
            else:
                w.ext("drop_item", s)
        if w.g8(G_SELECTED) == s[0x17A]:
            w.ext("item_cond", 0, s, 0, 1)
    if dist <= 20:
        w16(s, 0xC8, dist)
        w16(s, 0xE4, u16(s, 0xDE))
        w16(s, 0xE6, u16(s, 0xE0))
        tile, x8, y4 = s16(u16(s, 0xDC)), (s16(u16(s, 0xE4)) - 8) & 0xFFFFFFFF, (s16(u16(s, 0xE6)) - 4) & 0xFFFFFFFF
        w.ext("w_56dd4", s, 0, 0)               # read before: ShotEnd clears the attack tile +0xDC
        w.ext("impact", s, tile, x8, y4, 1)     # ECX kept


G_SHOT_HEIGHT_RANGE = 0x9634B  # i8 per facing: flown pixels before a missing bullet may fall (by facing)


def shot_end(s, w, silent, notify):
    """ShotEnd 0x56DD4: the projectile is over. Unless `silent` (DL): the target reacts (0x746D4, with BL) when the
    attack slot is a soldier, and guns (items 1..14) make a noise at the shooter's tile (0x727D0: loudness = the
    silencer value, with a 0x5B attachment (100 - its condition) / (100 / (value - 1)) + 1). Then all shot points
    none, +0x1BF / +0x1C7 / +0x1FF = 0, +0x1BC = 1, the busy counter - 1 (not below 0)."""
    if not silent & 0xFF:
        if s[0x1D8] < 0x30:
            w.ext("w_746d4", s, notify & 0xFF)
        if s[0x1F5] <= 0xE:
            t = w.g8(G_SILENCED + s[0x1F5])
            if s[3] == 0x5B:
                t = (cdiv(100 - s8(s[5]), cdiv(100, t - 1)) + 1) & 0xFF
            w.ext("make_noise", s[0x17A], s16(u16(s, 0x114)), s[0x1B3], t, 5)
    for o in (0xDC, 0xDE, 0xE0, 0xE4, 0xE6, 0x1E6):
        w16(s, o, NO_TILE)
    s[0x1BF] = s[0x1C7] = s[0x1FF] = 0
    s[0x1BC] = 1
    if _i32(w.g32(G_BUSY)) > 0:
        w.s32(G_BUSY, w.g32(G_BUSY) - 1)


def projectile_step(s, w):
    """ProjectileStep 0x57BB8 (per frame; only when the frame timer +0xCC is down to 0, then 15 again): the
    projectile moves pixel by pixel (Bresenham fields), +0xC8 flown + 1 and the height +0xF2 changes by the slope
    (not below 0; a cap +0xCA stops the climb: throwables then take the down slope +0xEA, bullets fly level). Each
    new tile (not the shooter's): +0x1FB + 1; leaving the map ends the shot (ShotEnd(0, 1), FireNow, +0xE2 = 0); the
    skip distance +0x1FF clears once flown past; ProjectileHitTest decides a stop -> ShotEnd(0, 0) and
    ProjectileImpact at (x - 8, y - 6 - height / 600). Up to tracking +0x1CB tiles per frame (always on through the
    shooter's tile). A bullet still flying past the facing's range low enough over open ground (object < 0x27,
    byte 2 < 0x10) and any throwable get a shadow: +0x1C7 = 1, +0xF6 = x, +0x9E = y - height / 400 (thrown: / 1000,
    above 18000 y - 18), and the sprite +0x1C5 by height / 1000: 0..2 none, 3..4 0xD2 .. 21..22 0xDB."""
    if _i32(_u32(s, 0xCC)) > 0:
        return
    s[0xCC:0xD0] = bytes([0xF, 0, 0, 0])
    from combat import view_tile
    prev = view_tile(s16(u16(s, 0xE4)), s16(u16(s, 0xE6)))
    hit = steps = 0
    me = s16(u16(s, 0x114))
    while True:
        if s16(u16(s, 0xF8)) < 0:
            w16(s, 0xE4, u16(s, 0xE4) + u16(s, 0xFE))
            w16(s, 0xE6, u16(s, 0xE6) + u16(s, 0x100))
            w16(s, 0xF8, u16(s, 0xF8) + u16(s, 0x104))
        else:
            w16(s, 0xE4, u16(s, 0xE4) + u16(s, 0xFA))
            w16(s, 0xE6, u16(s, 0xE6) + u16(s, 0xFC))
            w16(s, 0xF8, u16(s, 0xF8) + u16(s, 0x102))
        t = view_tile(s16(u16(s, 0xE4)), s16(u16(s, 0xE6)))
        w16(s, 0xC8, u16(s, 0xC8) + 1)
        w16(s, 0xF2, u16(s, 0xF2) + u16(s, 0xE8))
        if s16(u16(s, 0xF2)) < 0:
            w16(s, 0xF2, 0)
        if s16(u16(s, 0xCA)) and s16(u16(s, 0xF2)) > s16(u16(s, 0xCA)):
            w16(s, 0xF2, u16(s, 0xCA))
            w16(s, 0xE8, u16(s, 0xEA) if s[0x1C9] else 0)
        if t != prev and t != me:
            s[0x1FB] = (s[0x1FB] + 1) & 0xFF
            if off_map_step(prev, t):
                w.ext("w_56dd4", s, 0, 1)
                w.ext("fire", s[0x17A])
                w16(s, 0xE2, 0)
                return
            prev = t
        if t != me:
            if s[0x1FF] and s[0x1FF] <= u16(s, 0xC8):
                s[0x1FF] = 0
            res, skip, _ = w.ext("hit_test", s16(u16(s, 0xF2)), s16(u16(s, 0xE4)), s16(u16(s, 0xE6)),
                                 s16(u16(s, 0xC8)), s[0x1C6], s[0x1FF], s16(u16(s, 0xCA)), s16(u16(s, 0xDC)),
                                 s8(s[0x1BC]) & 0xFFFFFFFF, 0)   # chance output starts at 0 here
            s[0x1FF] = skip & 0xFF
            if _i32(res):
                hx = s16(u16(s, 0xE4)) - 8
                hy = s16(u16(s, 0xE6)) - 6 - cdiv(s16(u16(s, 0xF2)), 600)
                w.ext("w_56dd4", s, 0, 0)
                w.ext("impact", s, s16(t), hx & 0xFFFFFFFF, hy & 0xFFFFFFFF, res & 0xFFFFFFFF)
                return
        steps = (steps + 1) & 0xFF
        if steps < s8(s[0x1CB]) or t == me:
            continue
        break
    if not s[0x1C9]:
        if (s16(u16(s, 0xE4)) == NO_TILE or s8(w.g8(G_SHOT_HEIGHT_RANGE + s8(s[0x1A1]))) >= s16(u16(s, 0xC8))
                or w.tile8(t, 1) >= 0x27 or w.tile8(t, 2) >= 0x10):
            s[0x1C7] = 0
            return
    s[0x1C7] = 1
    w16(s, 0xF6, u16(s, 0xE4))
    h = s16(u16(s, 0xF2))
    if s[0x1C9] and h > 0x4650:
        w16(s, 0x9E, u16(s, 0xE6) - 0x12)
    else:
        w16(s, 0x9E, s16(u16(s, 0xE6)) - cdiv(h, 1000 if s[0x1C9] else 400))
    q = cdiv(h, 1000) & 0xFFFFFFFF
    if q <= 2:
        s[0x1C5] = 0
    elif q <= 22:
        s[0x1C5] = 0xD2 + (q - 3) // 2


G_IMPACT_NOISE = 0x92F21      # u8 per item (< 0x21): impact loudness; others Random(3)


def projectile_impact(s, w, tile, x, y, hit):
    """ProjectileImpact 0x5713C (shooter s, landing tile, point x / y, stack: the hit value): the shooter acts
    (0x284F28), the impact makes noise (0x727D0: loudness from the item table or Random(3), the ground class). A
    soldier on the tile: in combat mode the view looks there (0x3F490); hit counter 0x92DC4 + 1; enemy involvement
    records replay event 'M' once (0x22A0D1) and an enemy target may advance story byte 0x22A1F5 1 -> 2. Bullets:
    TakeDamage(target, height, BulletDamage, 1, shooter) and the merc practises stat 8. Thrown items: explosives
    (+0x1C9 > 1) explode (0x3B79C) on hit value -1 or a condition roll (condition +0x1E4 >= 10 and Random(100) -
    Random(50) < it), else land (0x56FE4) and may set off an effect (0x157BC, same roll); other throwables hurt for
    Random(4) + 1 (TakeDamage kind 1 at height 2500) and land as item 0x5D. No soldier: thrown items as above (then
    the view looks there in combat mode); bullets: an enemy shooting at a merc makes that merc practise stat 2, a
    story object 0xAE may react (0x61D28), puffs / splashes and sounds by the tile (0x156B8), and an unstandable tile
    may break (SectorDoorLookup value vs BulletDamage, 1 in 3: 0x3ACB8, +0x9D = 1, stat 8)."""
    w.s8(G_ACTIVE_SLOT, s[0x17A])
    vol = 0x58
    it = s[0x1F5]
    loud = w.g8(G_IMPACT_NOISE + it) if it < 0x21 else w.random(3)
    t = s16(tile)
    w.ext("make_noise", s[0x17A], t, w.g8(G_T0_CLASS + w.tile8(t, 0)), loud & 0xFF, 4)
    occ = _occupant(w, t)
    hit &= 0xFFFFFFFF
    cond = s8(s[0x1E4])

    def roll():
        return cond >= 10 and w.random(100) - w.random(50) < cond

    def thrown_effect():
        c = s8(w.g8(G_COMBAT))
        r = w.random(0x1964)
        w.ext("fx_157bc", t, s[0x1C9], w.random(2), (r - 2000 * c) & 0xFFFFFFFF, s[0x17A], 0)

    def explosive():
        if hit == 0xFFFFFFFF or roll():
            w.ext("explode", s[0x17A], t, s8(s[0x1C9]) & 0xFFFFFFFF, hit)
            return
        w.ext("land_item", s, t, it, cond & 0xFFFFFFFF, x & 0xFFFFFFFF, y & 0xFFFFFFFF)
        if roll():
            thrown_effect()
    if occ < 0x40:
        if w.g8(G_COMBAT):
            w.s16(0x92E4C, t)
            w.ext("w_3f490", t & 0xFFFFFFFF, 0)
        target = w.rec(occ)
        w.s8(0x92DC4, w.g8(0x92DC4) + 1)
        if target[0x17A] >= 0x28 or s[0x17A] >= 0x28:
            if not w.g8(0x22A0D1):
                w.s8(0x22A0D1, 1)
                sec = s8(w.g8(G_CUR_SECTOR)) & 0xFFFFFFFF
                w.ext("replay_event", sec, sec, 0x4D)
            if target[0x17A] >= 0x28 and s8(w.g8(0x22A1F5)) == 1:
                w.s8(0x22A1F5, 2)
        if s[0x1C9]:
            if s8(s[0x1C9]) > 1:
                explosive()
            else:
                w.ext("take_damage", occ, 0x9C4, w.random(4) + 1, 1, s[0x17A], NO_TILE)
                w.ext("land_item", s, t, 0x5D, 100, x & 0xFFFFFFFF, y & 0xFFFFFFFF)
            return
        dmg = s16(w.ext("bullet_damage", s))
        w.ext("take_damage", occ, s16(u16(s, 0xF2)) & 0xFFFFFFFF, dmg & 0xFFFFFFFF, 1, s[0x17A], NO_TILE)
        if s[0x17A] < 8:
            w.ext("stat_practice", s, 8, 1, 0)
        return
    ts = tile_standable(w, t)
    if s8(s[0x1C9]) > 0:
        if s8(s[0x1C9]) > 1:
            explosive()
            if w.g8(G_COMBAT):
                w.s16(0x92E4C, t)
                w.ext("w_3f490", t & 0xFFFFFFFF, 0)
        else:
            w.ext("land_item", s, t, 0x5D, 100, x & 0xFFFFFFFF, y & 0xFFFFFFFF)
        return
    if s[0x17A] >= 0x28 and s[0x1D8] < 8:
        w.ext("stat_practice", s[0x1D8], 2, 1, 0)
    if (w.g8(G_CUR_SECTOR) == s8(w.g8(0x22A057)) and not w.g8(0x22A055) and not w.g8(0x22A056)
            and w.tile8(t, 1) == 0xAE):
        w.ext("story_hit")
    if w.tile8(t, 2) >= 0x10 and s16(u16(s, 0x116)) <= 0:
        w.s8(0x229E03, 0)
    else:
        obj = w.tile8(t, 1)
        if obj == 0 and w.g8(G_T0_CLASS + w.tile8(t, 0)) >= 8:
            r = _i32(w.ext("puff", t, x & 0xFFFFFFFF, y & 0xFFFFFFFF, w.ptr(0x95670), 5, s8(s[0x1C9]) & 0xFFFFFFFF, 1)) & 0xFF
            sid = 0x11
        else:
            if ts < 0:
                x, y = tile_x(t) - 8, tile_y(t) - 7
            r = _i32(w.ext("puff", t, x & 0xFFFFFFFF, y & 0xFFFFFFFF, w.ptr(0x9566B), 5, 0, 1)) & 0xFF
            if obj:
                sid = (w.random(3) + 0x14) & 0xFF
                vol = 0x28
            else:
                sid = 0x13
        w.s8(0x229E03, r)
        w.s32(0x229E04, s8(r) & 0xFFFFFFFF)
        w.ext("sfx", sid, 0x2B11, s8(vol) & 0xFFFFFFFF, 1)
    if ts < 0:
        d = _i32(w.ext("door_lookup", t))
        if d > 100:
            d -= 0x46
        if d < s16(w.ext("bullet_damage", s)) and w.random(3) == 0:
            w.ext("break_obj", t, w.tile8(t, 1), 1, 0)
            s[0x9D] = 1
            w.ext("stat_practice", s, 8, 2, 0)


G_DMG_TEXT = 0x10E290         # 5 B per slot: damage number shown over the soldier (itoa of -damage)
G_KNOCKDOWN = 0x92E72         # u8: the next bullet hit knocks the soldier down (0x17F5C)


def take_damage(s, w, height, dmg, kind, attacker, tile):
    """TakeDamage 0x392FC (soldier, height, damage, kind 1 bullet / 2 / 3 / 4 / 6, stack: attacker slot, tile):
    Santino takes none. Queued aim animations are noted (+0x1E1 with its AP back in +0x1E3); bullets and kind 4 cost
    energy (Rest 0x32754 by -(50 x damage x (10000 - +0x10C) / 10000)) and go through HitLocationDamage (not at height
    0x1356). With damage: the number shows (0x10E290), +0x1D2 = 40, +0xC4 = 10, +0xD8 / +0xDA = -4 / 2, hp drops (an
    enemy below 15 dies outright), a merc practises stat 9, wounds +0x185 grow, +0x1F0 = hp / (+0x1F2 + 10) + 1,
    portrait; below 15 hp the soldier drops (0x1A460, attack forgotten, +0x186, +0x1AC below 10, the selected
    merc's cursor resets) and 0x323D4. Then re-think, attack tile none (last target too unless +0x19C is 3) and a hit
    animation by posture and kind (falls 0x2F / 0x29 / 0x30 / 3, water 0x35 / 0x38 / 0x3B, standing 0x28 facing the
    attacker or a knockdown, 0x2C / 0x2A for heavy hits, 0x2B for an unconscious soldier); a cry (kind except 3),
    noise sqrt(damage) - explosives / 2, sound 0x20 for kind 2 anims, frame delay, +0xB0, AP lost = damage / 4
    (+4 for some) up to the AP left (sound), merc posture, killer slot +0x1DA and the attacker's reaction (0x74304)."""
    dmg = _i32(dmg)
    kind &= 0xFF
    attacker &= 0xFF
    di = u16(s, 0x1E8 + 2 * s[0x1F3])
    extra = 0
    path_marks_off(w)
    if s8(s[0xBA]) == 4:
        return
    prev = s8(s[0x184]) - s8(s[0x183]) - s8(s[0x185])
    s[0x1BE] = 0
    if s[0x17A] == w.g8(0x95E0C):
        w.s8(0x95E0C, 0x40)
    if not s[0x1E1]:
        if di == 0x1B:
            s[0x1E1] = 0x1A
        elif di == 0x1F:
            s[0x1E1] = 0x1E
        elif di == 0x23:
            s[0x1E1] = 0x22
        elif di == 5:
            s[0x1E1] = 6
        elif di in (0xE, 0xF):
            t = s16(u16(s, 0x114))
            w16(s, 0x126, tile_x(t))
            w16(s, 0x116, tile_x(t))
            w16(s, 0x128, tile_y(t))
            w16(s, 0x118, tile_y(t))
            s[0x1E1] = 0xE
        if s[0x1E1]:
            q = s8(s[0x1E1])
            if s8(w.g8(ANIM_AP + 8 * q)) != 0x63:
                s[0x1E3] = (s[0x1E3] + w.g8(ANIM_AP + 8 * q)) & 0xFF
    if kind in (1, 4):
        v = 10000 - s16(u16(s, 0x10C))
        w.ext("rest", s, (-s16(cdiv(_i32(50 * dmg * v), 10000))) & 0xFFFFFFFF, 1)
        if height != 0x1356:
            dmg = _i32(w.ext("hit_location", s, height & 0xFFFFFFFF, dmg & 0xFFFFFFFF))
    s[0x195] = 1
    buf = G_DMG_TEXT + 5 * s[0x17A]
    if dmg:
        for k, ch in enumerate(str(-dmg).encode() + b"\0"):
            w.s8(buf + k, ch)
        s[0x1D2] = 0x28
        s[0xC4:0xC8] = bytes([10, 0, 0, 0])
        w16(s, 0xD8, 0xFFFC)
        w16(s, 0xDA, 2)
        if s8(s[0x183]) < dmg:
            dmg = s8(s[0x183])
        s[0x183] = (s[0x183] - dmg) & 0xFF
        if s[0x17A] >= 0x28 and s8(s[0x183]) < 15 and s[0x183]:
            s[0x183] = 0
        if s[0x17A] < 8:
            w.ext("stat_practice", s, 9, s16(dmg) & 0xFFFFFFFF, 0)
        s[0x185] = (s[0x184] - (s[0x183] + prev)) & 0xFF
        w16(s, 0x1F0, cdiv(s8(s[0x183]), s8(s[0x1F2]) + 10) + 1)
        s[0x189] = 7
        w.ext("portrait", s[0x17A], 1)
        if s8(s[0x183]) < 15:
            clear_path(s)
            clear_target(s, w, 0)
            if s8(s[0x186]) >= 0:
                s[0x186] = 1
            s[0x181] = 0
            s[0x1AC] = 1 if s8(s[0x183]) < 10 else 0
            if w.g8(G_SELECTED) == s[0x17A]:
                w.s8(0x284F1D, 0xA1)
                w.s8(0x284F1B, 0)
        w.ext("w_323d4", s, 1)
    else:
        w.s8(buf, w.g8(0x90914))
    s[0x1C1] = 1
    w16(s, 0xDC, NO_TILE)
    if s[0x19C] != 3:
        w16(s, 0xD0, NO_TILE)
    hk = 2
    if s16(di) > 0x31:
        if kind == 2:
            s[0x1E1] = s[0x195] = 0
            w.ext("set_anim", s, 0x3C, 0, 1)
            hk = 3
        elif s16(di) <= 0x35:
            w.ext("set_anim", s, 0x35, 1, 1)
        elif s16(di) <= 0x38:
            w.ext("set_anim", s, 0x38, 1, 1)
        elif s16(di) < 0x40:
            w.ext("set_anim", s, 0x3B, 1, 1)
    elif di in (0, 2, 0x29, 0x2A, 0x2F):
        s[0x1E1] = 0
        if kind == 1:
            w.ext("set_anim", s, 0x2F, 0, 1)
        else:
            w.ext("set_anim", s, 0x29, 0, 1)
            hk = 3
    elif di == 0x2B:
        hk = 3
        s[0x1E1] = 0
    elif di in (1, 3, 0xD):
        s[0x1E1] = 0
        if kind == 1:
            w.ext("set_anim", s, 0x30, 0, 1)
        else:
            w.ext("set_anim", s, 3, 0, 1)
            hk = 3
    else:
        if kind == 1:
            if w.g8(G_KNOCKDOWN) and s8(s[0x183]) >= 15:
                w.ext("anim2", s, 0x28, 0x2B, 4)
                extra = 4
                w.s8(G_KNOCKDOWN, 0)
            else:
                if s8(s[0x183]) < 15 and attacker < 0x40:
                    a = G_SOLDIERS + 0x200 * attacker       # its own record when it hit itself (x / y just set)
                    ax, ay = ((u16(s, 0x116), u16(s, 0x118)) if attacker == s[0x17A]
                              else (w.g16(a + 0x116), w.g16(a + 0x118)))
                    s[0x1A2] = _i32(w.ext("direction", s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(ax), s16(ay))) & 0xFF
                w.ext("set_anim", s, 0x28, 1, 1)
        elif kind == 3:
            s[0x195] = 0
        else:
            if dmg > 0x28 or (tile & 0xFFFF) == u16(s, 0x114):
                w.ext("set_anim", s, 0x2C, 0, 1)
            elif dmg > 5:
                w.ext("set_anim", s, 0x2A, 0, 1)
            else:
                s[0x195] = 0
            extra = 4
            if dmg > 5 and s[0x1B9]:
                w.ext("end_task", s, 0)
        if kind in (3, 6):
            hk = 3
            if not s[0x183] and u16(s, 0x1E8 + 2 * s[0x1F3]) >= 4:
                w.ext("set_anim", s, 0x2B, 1, 1)
                if s[0x1B9]:
                    w.ext("end_task", s, 0)
            elif kind == 6 and dmg == 1:
                w.ext("set_anim", s, 0x2B, 1, 1)
                if s[0x1B9]:
                    w.ext("end_task", s, 0)
    if kind != 3:
        w.ext("w_17b70", s[0x17A], hk)
    import math
    sq = int(math.sqrt(dmg)) if dmg >= 0 else -0x80000000
    v = (sq - cdiv(s8(s[0x192]), 2)) & 0xFF
    if s8(v) > 0:
        w.ext("make_noise", s[0x17A], s16(u16(s, 0x114)), s[0x1B3], s8(v) & 0xFFFFFFFF, 7)
    if hk == 2:
        w.ext("sfx", 0x20, 0x2B11, 0x32, 1)
    anim_delay(s, w)
    s[0xB0:0xB4] = (s16(u16(s, 0xD2)) & 0xFFFFFFFF).to_bytes(4, "little")
    ap = (cdiv(dmg, 4) + s8(extra)) & 0xFF
    if s8(ap) > s8(s[0x1A5]):
        ap = s[0x1A5]
    w.ext("spend_ap", s, s8(ap) & 0xFFFFFFFF, 0)
    if s[0x17A] < 8:
        w.ext("reserve_ap", s)
    if attacker < 0x40:
        s[0x1DA] = attacker
        w.ext("w_74304", s, attacker, 1, dmg & 0xFF)



G_HIT_SCALE = 0x92DC5          # u8 damage / 10 while a hit reaction runs (0 otherwise)


def hit_reaction(s, w, attacker, b, dmg):
    """0x74304 (EAX = the hit soldier, DL = attacker slot, BL, CL = damage): only across sides (one slot below 0x28,
    the other 0x28+), with hp 15+, state +0xBA not 2, and the attacker not already known as 1 (+0x112 + slot for a
    merc, +0x13A + slot for an enemy): runs the reaction 0x743E8(soldier, attacker from the pointer table 0x100958,
    BL) with 0x92DC5 = damage / 10, then clears 0x92DC5."""
    attacker &= 0xFF
    mine, theirs = s[0x17A] < 0x28, w.g8(G_SOLDIERS + 0x200 * attacker + 0x17A) < 0x28
    if mine == theirs or s8(s[0x183]) < 15 or s[0xBA] == 2:
        return
    if s8(s[(0x112 if mine else 0x13A) + attacker]) == 1:
        return
    w.s8(G_HIT_SCALE, (dmg & 0xFF) // 10)
    w.ext("react", s, attacker, b & 0xFF)
    w.s8(G_HIT_SCALE, 0)



def hit_alert(s, w, i, flag):
    """0x743E8 (EAX = the hit soldier, EDX = the attacker's record, BL): a merc with BL set looks at itself (0x3F724),
    cries out (0x24E44) and plays sample 0x2B11; every merc practises stat 9. Across sides only: the old sight flag
    on the attacker is kept; within 14 tiles with a clear sight line (range 14) the soldier sees it (MarkSeen); a
    merc then focuses on a visible attacker (or with 0x93458 set; 0x229C94 / 0x92E52 / 0x92E53 = it) and its side
    knows it (G_KNOWN = 1). Unseen: re-think, alert 2 with a orders bump when below 2, the sight flag becomes 0xFF
    unless it is 2, the last-seen tile is the attacker's tile, and a merc's side marks it heard (G_KNOWN = 0xFF)
    unless the side already knew it (1 / 2 / 0xFF). Last 0x73AB4(soldier, attacker, the old flag)."""
    o = w.rec(i)
    slot, oslot = s[0x17A], o[0x17A]
    if slot < 8:
        if flag & 0xFF:
            w.ext("focus", slot, 0)
            w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
            w.ext("voc", 0x2B11, 0x58, 1)
        w.ext("stat_practice", s, 9, 1, 0)
    mine = slot < 0x28
    if mine == (oslot < 0x28):
        return
    flag_off = (0x112 if mine else 0x13A) + oslot
    old = s[flag_off]
    tell = not (mine and s8(w.g8(G_KNOWN + oslot)) in (1, 2, -1))
    seen = False
    if _i32(w.ext("tile_dist", s16(u16(s, 0x114)), s16(u16(o, 0x114)))) <= 14:
        seen = bool(_i32(w.ext("sight_line", s, s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, s16(u16(o, 0x114)),
                               s16(u16(o, 0x116)) + 2, s16(u16(o, 0x118)) + 4, 1, 0xE, oslot)))
    if seen:
        mark_seen(s, w, i)
        if mine:
            if s8(w.rec(i)[0x9C]) > 0 or w.g8(0x93458):    # MarkSeen just raised it to 1+
                w.s32(0x229C94, oslot)
                w.s8(0x92E53, oslot)
                w.s8(0x92E52, oslot)
                w.ext("focus", oslot, 0)
            w.s8(G_KNOWN + oslot, 1)
    else:
        s[0x1C1] = 1
        if s8(s[0xBB]) < 2:
            s[0xBB] = 2
            _bump_orders(s)
        if s8(s[flag_off]) != 2:
            s[flag_off] = 0xFF
        if not mine:
            w.s16(G_LAST_SEEN_E + (slot - 0x28) * 0x50 + 2 * oslot, u16(o, 0x114))
        else:
            w.s16(G_LAST_SEEN_M - 0x50 + slot * 16 + 2 * oslot, u16(o, 0x114))
            if tell and slot < 8:
                w.s8(G_KNOWN + oslot, 0xFF)
    w.ext("w_73ab4", s, i, old)



def shot_alert(s, w, flag):
    """0x746D4 (EAX = shooter, DL = flag; from ShotEnd for an attack slot below 0x30): the target (+0x1D8) must be in
    play with hp 15+. A target in state 2 only gets +0x1C4 = 1. Across sides only (an enemy shooting a merc / guard /
    ally or enemy 0x28, anyone else shooting an enemy): the target's old sight flag on the shooter is kept; an enemy
    shooting knows whether the merc side knew it (G_KNOWN 1 / 2 / 0xFF: no news). Without the flag the occupant of
    the shot's end point (ViewTile of +0xE4 / +0xE6) gets +0x1C4 = 1. The shot must have flown at least the pixel
    distance to the target minus 45 (+0xC8), and without the flag the end point must be empty. Then the target gets
    +0x1C4 = 1; within 14 tiles with a clear sight line it sees the shooter (MarkSeen, a merc side shares it).
    Unseen: re-think, alert 2 with a orders bump when below 2, the sight flag becomes 0xFF unless it is 2, the
    last-seen tile is the shooter's; a merc target with news practises stat 9, reacts (0x56D28(slot, 1, 0x40)) and
    its side marks the shooter heard (G_KNOWN = 0xFF). Last 0x73AB4(target, shooter, the old flag)."""
    from combat import view_tile
    k = s[0x1D8]
    if k >= 0x40:
        return
    o = w.rec(k)
    if not o[0x182] or not o[0x138] or s8(o[0x183]) < 15:
        return
    if s8(o[0xBA]) == 2:
        o[0x1C4] = 1
        return
    slot, oslot = s[0x17A], o[0x17A]
    if slot < 0x28 and oslot < 0x28 or slot >= 0x28 and oslot > 0x28:
        return
    news = True
    if oslot < 0x28:
        if s8(w.g8(G_KNOWN + slot)) in (1, 2, -1):
            news = False
        flag_off = 0x112 + slot
    else:
        flag_off = 0x13A + slot
    old = o[flag_off]
    occ = 0x40
    if not flag & 0xFF:
        occ = _occupant(w, s16(view_tile(s16(u16(s, 0xE4)), s16(u16(s, 0xE6))) & 0xFFFF))
        if occ < 0x40:
            w.rec(occ)[0x1C4] = 1
    t = s16(u16(o, 0x114))
    near = s16(cheb_px(s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, tile_x(t) + 2, tile_y(t) + 4) & 0xFFFF)
    if s16(u16(s, 0xC8)) < near - 0x2D or (not flag & 0xFF and occ < 0x40):
        return
    o[0x1C4] = 1
    seen = False
    if _i32(w.ext("tile_dist", t, s16(u16(s, 0x114)))) <= 14:
        seen = bool(_i32(w.ext("sight_line", k, s16(u16(o, 0x116)) + 2, s16(u16(o, 0x118)) + 4, s16(u16(s, 0x114)),
                               s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, 1, 0xE, slot)))
    if seen:
        mark_seen(o, w, slot)
        if oslot < 0x28:
            w.ext("share_sightings", k, 1)
    else:
        o[0x1C1] = 1
        if s8(o[0xBB]) < 2:
            o[0xBB] = 2
            _bump_orders(o)
        if s8(o[flag_off]) != 2:
            o[flag_off] = 0xFF
        if oslot >= 0x28:
            w.s16(G_LAST_SEEN_E + (oslot - 0x28) * 0x50 + 2 * slot, u16(s, 0x114))
        else:
            w.s16(G_LAST_SEEN_M - 0x50 + oslot * 16 + 2 * slot, u16(s, 0x114))
            if news and oslot < 8:
                w.ext("stat_practice", k, 9, 1, 0)
                w.ext("merc_sees", oslot, 1, 0x40)
                w.s8(G_KNOWN + slot, 0xFF)
    w.ext("w_73ab4", k, slot, s8(old) & 0xFFFFFFFF)


G_INTERRUPTED = 0x92DD3        # u8 set when a soldier qualified for an interrupt


def interrupt_check(s, w, i, old):
    """0x73AB4 (EAX = soldier, EDX = the opponent's record, BL = its old sight flag on the opponent): a soldier on
    the map (+0x18A), not in state 2, standing (animation 4+) may interrupt. Seeing the opponent now (flag 1) when
    the old flag was not 1 / 2: it needs MoveApCost AP. Else when the opponent sees it (flag 1), it has seen nothing
    new (+0x1D5 <= 0) and the old flag was not 0xFF: it needs 1 AP. With AP left (+0x1A5) >= the need: 0x92DD3 = 1;
    a merc asks 0x73D28 and, granted, 0x1304C when 0x92DCF is set, else the turn moves on (0x12D58(1)) when 0x92DD0
    is set; an enemy asks 0x73F3C and, granted with 0x92DD0 set, the selected merc (0x284F11 below 8) is kept in
    0x284F0F and the turn moves on."""
    o = w.rec(i)
    old &= 0xFF
    if s8(s[0xBA]) == 2 or not s[0x18A] or u16(s, 0x1E8 + 2 * s[0x1F3]) < 4:
        return
    slot, oslot = s[0x17A], o[0x17A]
    if oslot >= 0x28:
        mine, theirs = s[0x112 + oslot], o[0x13A + slot]
    else:
        mine, theirs = s[0x13A + oslot], o[0x112 + slot]
    if s8(mine) == 1:
        if s8(old) in (1, 2):
            return
        need = w.ext("move_ap", s) & 0xFF
    else:
        if s8(theirs) != 1 or s8(s[0x1D5]) > 0 or s8(old) == -1:
            return
        need = 1
    if s8(s[0x1A5]) < need:
        return
    w.s8(G_INTERRUPTED, 1)
    if slot < 0x28:
        if not w.ext("w_73d28", s, i) & 0xFF:
            return
        if w.g8(0x92DCF):
            w.ext("w_1304c")
            return
        if not w.g8(0x92DD0):
            return
    else:
        if not w.ext("w_73f3c", s, i) & 0xFF or not w.g8(0x92DD0):
            return
        if w.g8(G_SELECTED) < 8:
            w.s8(0x284F0F, w.g8(G_SELECTED))
    w.ext("next_turn", 1)



G_ITQ, G_ITQ_N = 0x22A412, 0x92DD1     # u8[] interrupt queue (slots) and its length
G_DIFF_BONUS = 0x95C54                 # s16 per difficulty: the enemy side's bonus in an interrupt contest


def noise_interrupt(s, w):
    """0x73C00 (EAX = soldier hearing a kind-8 noise, from MsgHearsNoise): a soldier not in state 2, on the map
    (+0x18A), standing (animation 4+) with AP left takes the turn. A merc / guard / ally (slot < 0x28) only when the
    tactical screen is not drawn (0x92DCC). +0x1B0 = 0; the selected soldier is kept: a merc (below 8) in 0x284F0F,
    else it is queued. A merc then acts at once (0x92DD2 = slot, InterruptTurn 0x1304C); others are queued and the
    turn moves on (NextTurn(1))."""
    if s8(s[0xBA]) == 2 or not s[0x18A] or u16(s, 0x1E8 + 2 * s[0x1F3]) < 4 or s8(s[0x1A5]) < 1:
        return
    slot = s[0x17A]
    if slot < 0x28 and w.g8(G_SHOW_UI):
        return
    s[0x1B0] = 0
    sel = w.g8(G_SELECTED)
    if sel >= 8:
        _itq_push(w, sel)
    else:
        w.s8(0x284F0F, sel)
    if slot < 8:
        w.s8(0x92DD2, slot)
        w.ext("w_1304c")
        return
    _itq_push(w, slot)
    w.ext("next_turn", 1)


def _itq_push(w, slot):
    n = w.g8(G_ITQ_N)
    w.s8(G_ITQ + n, slot)
    w.s8(G_ITQ_N, n + 1)


def _interrupt_grant(s, w):
    """the shared tail of a won interrupt: +0x1B0 = 0; with an enemy turn running (selected 8+) and 0x92DD3 set,
    0x92DD3 = 0 and the selected slot is queued"""
    s[0x1B0] = 0
    if w.g8(G_SELECTED) >= 8 and s8(w.g8(G_INTERRUPTED)) == 1:
        w.s8(G_INTERRUPTED, 0)
        _itq_push(w, w.g8(G_SELECTED))


def merc_interrupt(s, w, i):
    """0x73D28 (EAX = merc / guard, EDX = enemy record) -> 1 when it wins the interrupt: re-think; it needs MoveApCost
    AP when it sees the enemy (else 1 AP), none while the tactical screen is drawn (0x92DCC). An enemy that does not
    see it loses outright; else level contest: the merc's level (+1 when it remembers the enemy, - (+0x1D5 - 1) above
    1, -1 when it only heard it, - the hit scale) must beat the enemy's level + the difficulty bonus (+1 when it
    remembers the merc, - (+0x1D5 - 1) above 1, +1 when the merc side is known to it). Won: the grant tail; a
    visible enemy (or 0x93458) sets 0x93D54 = 5000 and 0x229C94 = the enemy; a merc becomes the one acting (0x92DD2,
    0x92DCF = 1; with 0x22A0CD set the kept merc 0x284F0F below 8 instead), a guard is queued (0x92DD0 = 1)."""
    o = w.rec(i)
    slot, c = s[0x17A], (o[0x17A] - 0x28) & 0xFF
    s[0x1C1] = 1
    need = w.ext("move_ap", s) & 0xFF if s8(s[0x13A + c]) == 1 else 1
    if s8(need) > s8(s[0x1A5]) or w.g8(G_SHOW_UI):
        return 0
    won = s8(o[0x13A + slot]) != 1
    if not won:
        a = s[0x192] + (w.g8(G_REMEMBER_M + 8 * slot + c) == 1)
        if s8(s[0x1D5]) > 1:
            a -= s[0x1D5] - 1
        if s8(s[0x13A + c]) == -1:
            a -= 1
        a -= w.g8(G_HIT_SCALE)
        b = o[0x192] + w.g8(G_DIFF_BONUS + 2 * s8(w.g8(G_DIFFICULTY))) + (w.g8(G_REMEMBER_E + 0x28 * c + slot) == 1)
        if s8(o[0x1D5]) > 1:
            b -= o[0x1D5] - 1
        b += w.g8(G_KNOWN + slot) != 0
        won = s8(a & 0xFF) > s8(b & 0xFF)
    if not won:
        return 0
    _interrupt_grant(s, w)
    if s8(o[0x9C]) > 0 or w.g8(0x93458):
        w.s32(0x93D54, 5000)
        w.s32(0x229C94, o[0x17A])
    if slot < 8:
        w.s8(0x92DD2, slot)
        w.s8(0x92DCF, 1)
        if w.g8(0x22A0CD) and w.g8(0x284F0F) < 8:
            w.s8(0x92DD2, w.g8(0x284F0F))
    else:
        _itq_push(w, slot)
        w.s8(0x92DD0, 1)
    return 1


def enemy_interrupt(s, w, i):
    """0x73F3C (EAX = enemy, EDX = merc / guard record) -> 1 when it wins the interrupt: re-think; it needs
    MoveApCost AP when it sees the merc (else 1 AP). A merc that does not see it, or an enemy in state 4, wins
    outright; else level contest: the enemy's level + the difficulty bonus (+1 when it remembers the merc,
    - (+0x1D5 - 1) above 1, +1 when the merc side is known, -1 when it only heard it, - the hit scale) must beat the
    merc's level (+1 when it remembers the enemy, - (+0x1D5 - 1) above 1). Won: the grant tail, then the enemy is
    queued (0x92DD0 = 1)."""
    o = w.rec(i)
    e, oslot = (s[0x17A] - 0x28) & 0xFF, o[0x17A]
    s[0x1C1] = 1
    need = w.ext("move_ap", s) & 0xFF if s8(s[0x13A + oslot]) == 1 else 1
    if s8(need) > s8(s[0x1A5]):
        return 0
    won = s8(o[0x13A + e]) != 1 or s8(s[0xBA]) == 4
    if not won:
        a = s[0x192] + w.g8(G_DIFF_BONUS + 2 * s8(w.g8(G_DIFFICULTY))) + (w.g8(G_REMEMBER_E + 0x28 * e + oslot) == 1)
        if s8(s[0x1D5]) > 1:
            a -= s[0x1D5] - 1
        a += w.g8(G_KNOWN + oslot) != 0
        if s8(s[0x13A + oslot]) == -1:
            a -= 1
        a -= w.g8(G_HIT_SCALE)
        b = o[0x192] + (w.g8(G_REMEMBER_M + 8 * oslot + e) == 1)
        if s8(o[0x1D5]) > 1:
            b -= o[0x1D5] - 1
        won = s8(a & 0xFF) > s8(b & 0xFF)
    if not won:
        return 0
    _interrupt_grant(s, w)
    _itq_push(w, s[0x17A])
    w.s8(0x92DD0, 1)
    return 1



def make_noise(w, src, tile, cls, loud, kind):
    """MakeNoise 0x727D0 (AL = source slot or 0x40+, EDX = tile, BL = noise class, ECX = loudness, stack = kind; ret 4):
    only in combat (0x92DCB) with loudness, and from a living source. Listeners: the enemies (0x28..0x2F) for a merc
    / guard source or 0x42, the mercs and guards (0..0x0F) for an enemy or 0x41, everyone (0..0x2F) otherwise. The
    class is BL when 1..9, else the source's +0x1B3 when 1..9, else the class of the tile's ground. Each listener in
    play, not unconscious (+0x188), hp 15+ and not in state 2 that does not see the source already (side knowledge
    G_KNOWN 1, or its own sight flag 1) hears it at loudness + its +0x1D0 hearing, -5 when exactly one of it and the
    noise is class 3 (water), - (Distance / 15 - 1), - its +0x1D5; above 0 it reacts (0x72A60: listener, tile, low
    byte of that, kind; kind 4 from a soldier gives the source's own tile)."""
    src &= 0xFF
    kind &= 0xFF
    if not w.g8(G_COMBAT) or not loud & 0xFFFFFFFF:
        return
    if src < 0x40 and not w.g8(G_SOLDIERS + 0x200 * src + 0x183):
        return
    if src < 0x28 or src == 0x42:
        first, end = 0x28, 0x30
    elif src < 0x30 or src == 0x41:
        first, end = 0, 0x10
    else:
        first, end = 0, 0x30
    c = 0
    if src < 0x30:
        c = cls & 0xFF
        if not 1 <= c <= 9:
            c = w.g8(G_SOLDIERS + 0x200 * src + 0x1B3)
    if not 1 <= c <= 9:
        c = w.g8(G_T0_CLASS + w.tile8(s16(tile), 0))
    for k in range(first, end):
        p = w.rec(k)
        if not p[0x182] or not p[0x138] or p[0x188] or s8(p[0x183]) < 15 or s8(p[0xBA]) == 2:
            continue
        if src < 0x40:
            if s8(w.g8(G_KNOWN + src)) == 1 or s8(p[(0x13A if src < 0x28 else 0x112) + src]) == 1:
                continue
        v = (loud + s8(p[0x1D0])) & 0xFFFF
        if (p[0x1B3] == 3) != (c == 3):
            v = (v - 5) & 0xFFFF
        from combat import distance
        t = s16(tile)
        d = s16(distance(s16(u16(p, 0x116)), s16(u16(p, 0x118)), tile_x(t), tile_y(t)))
        v = s16((v - (cdiv(d, 15) - 1) - s8(p[0x1D5])) & 0xFFFF)
        if v <= 0:
            continue
        where = s16(w.g16(G_SOLDIERS + 0x200 * src + 0x114)) if kind == 4 and src < 0x30 else t
        w.ext("hears", k, where, s8(v & 0xFF) & 0xFFFFFFFF, kind)



G_TEXT_BUF = 0xEBAC0           # char[] status-message buffer (sprintf target)
NOISE_LEVEL_TXT, NOISE_KIND_TXT, DIR_TXT = 0x96532, 0x9655A, 0x9AEBB   # char[10] rows: FAINT.., UNKNOWN.., north..


def _cstr(w, a):
    out = bytearray()
    while w.g8(a + len(out)):
        out.append(w.g8(a + len(out)))
    return bytes(out)


def hears_text(w, slot, level, kind, d):
    """the "<name> hears a <LEVEL> <KIND> noise coming from the <direction>" text into G_TEXT_BUF (port host op 213)"""
    text = b" ".join([_cstr(w, G_SOLDIERS + 0x200 * slot + 0x16E), _cstr(w, 0x9AAC5),
                      _cstr(w, NOISE_LEVEL_TXT + 10 * level), _cstr(w, NOISE_KIND_TXT + 10 * kind),
                      _cstr(w, 0x9AADF), _cstr(w, DIR_TXT + 10 * (d - 1))])
    for k, ch in enumerate(text + b"\0"):
        w.s8(G_TEXT_BUF + k, ch)


def hears(s, w, tile, strength, kind):
    """MsgHearsNoise? 0x72A60 (EAX = listener, EDX = tile, BL = strength, CL = kind) from MakeNoise. The noise's soldier
    is the tile's occupant (none for kinds 6, 8, 9); one on the listener's side is ignored. Level by strength: 0 below
    3, 1 below 6, 2 below 10, else 3. A merc (slot < 8) prints "<name> hears a  <LEVEL> <KIND> noise coming from the
    <direction>" unless the kind is 4..6 / 8..9 or its side already knows the soldier (1 / 2 / 0xFF). Within 14 tiles
    it looks: a sight line (range 14) to the soldier's point (or the tile's) = seen; it is turned away when its facing
    is not the direction to that point. A soldier seen: MarkSeen (a merc shares sightings). Unseen: re-think, alert 2
    + orders bump for kinds 4/5 above level 0, sight flag 0xFF unless 2, last-seen tile = its tile, a merc's side marks
    it heard (unless known). No soldier: kind 6 above level 0 bumps the alert; unseen (or kind 8 turned away) a noise
    at least as strong as +0x1C0 becomes the one to check (+0x122 = tile, +0x1C0 = strength up to 12), else re-think
    and alert 2. Last: kind 8 -> 0x73C00, else with a soldier InterruptCheck? (old sight flag)."""
    tile = s16(tile)
    strength &= 0xFF
    kind &= 0xFF
    slot = s[0x17A]
    known = True
    occ = 0x40 if kind in (6, 8, 9) else _occupant(w, tile)
    old = 0
    if occ < 0x30:
        if (slot < 0x28) == (occ < 0x28):
            return
        if slot < 0x28:
            if s8(w.g8(G_KNOWN + occ)) in (1, 2, -1):
                known = False
            old = s[0x112 + occ]
        else:
            old = s[0x13A + occ]
    msg = known and kind not in (4, 5, 6, 8, 9)
    st = s8(strength)
    level = 0 if st < 3 else 1 if st < 6 else 2 if st < 10 else 3
    me = s16(u16(s, 0x114))
    if slot < 8 and msg:
        d = _i32(w.ext("direction", tile_x(me), tile_y(me), tile_x(tile), tile_y(tile))) & 0xFF
        hears_text(w, slot, level, kind, d)
        w.ext("message", G_TEXT_BUF)
    seen = turned = False
    if _i32(w.ext("tile_dist", me, tile)) <= 14:
        if occ < 0x30:
            o = w.rec(occ)
            px, py = s16(u16(o, 0x116)) + 2, s16(u16(o, 0x118)) + 4
        else:
            px, py = tile_x(tile), tile_y(tile)
        px, py = s16(px), s16(py)
        sx, sy = s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4
        turned = s8(s[0x1A1]) != _i32(w.ext("direction", sx, sy, px, py))
        seen = bool(_i32(w.ext("sight_line", s, sx, sy, tile, px, py, 1, 0xE, occ)))
    if occ < 0x30:
        if seen:
            mark_seen(s, w, occ)
            if slot < 0x28:
                w.ext("share_sightings", s, 0)
        else:
            s[0x1C1] = 1
            if kind in (4, 5) and level and s8(s[0xBB]) < 2:
                s[0xBB] = 2
                _bump_orders(s)
            if slot >= 0x28:
                if s8(s[0x13A + occ]) != 2:
                    s[0x13A + occ] = 0xFF
                w.s16(G_LAST_SEEN_E + (slot - 0x28) * 0x50 + 2 * occ, w.g16(G_SOLDIERS + 0x200 * occ + 0x114))
            else:
                if s8(s[0x112 + occ]) != 2:
                    s[0x112 + occ] = 0xFF
                w.s16(G_LAST_SEEN_M - 0x50 + slot * 16 + 2 * occ, w.g16(G_SOLDIERS + 0x200 * occ + 0x114))
                if known and slot < 8:
                    w.s8(G_KNOWN + occ, 0xFF)
    else:
        if kind == 6 and level and s8(s[0xBB]) < 2:
            s[0xBB] = 2
            _bump_orders(s)
        if not seen or (kind == 8 and turned):
            if st >= s[0x1C0]:
                w16(s, 0x122, tile)
                s[0x1C0] = strength if st < 12 else 12
                s[0x1C1] = 1
        else:
            s[0x1C1] = 1
            if s8(s[0xBB]) < 2:
                s[0xBB] = 2
                _bump_orders(s)
    if kind == 8:
        w.ext("w_73c00", s)
    elif occ < 0x30:
        w.ext("w_73ab4", s, occ, old)



def clear_path(s):
    """0x1A460: path length +0xD4 = 0"""
    w16(s, 0xD4, 0)


G_REDRAW_OFF = 0x92DDF         # u8: 2+ = no panel redraws


def rest(s, w, amount, redraw):
    """Rest? 0x32754 (EAX = soldier, EDX = amount, BL = redraw): fatigue +0x10C (word) -= amount (negative tires),
    kept within 0..10000; energy +0x18A = 100 - fatigue / 100. A changed energy: 50 or more clears +0x1BA, and a
    merc's energy bar is redrawn (0x32610, BL) unless 0x92DDF is 2+; unchanged with BL set: the merc's panel
    (0x33204) instead."""
    f = s16((u16(s, 0x10C) - amount) & 0xFFFF)
    w16(s, 0x10C, max(f, 0))
    if s16(u16(s, 0x10C)) > 10000:
        w16(s, 0x10C, 10000)
    e = (100 - cdiv(s16(u16(s, 0x10C)), 100)) & 0xFF
    if s8(e) > 100:
        e = 100
    if s8(e) < 0:
        e = 0
    slot = s[0x17A]
    if e != s[0x18A]:
        s[0x18A] = e
        if s8(s[0x18A]) >= 50:
            s[0x1BA] = 0
        if w.g8(G_REDRAW_OFF) < 2 and slot <= 7:
            w.ext("energy_bar", s, s8(redraw & 0xFF) & 0xFFFFFFFF)
    elif redraw & 0xFF and w.g8(G_REDRAW_OFF) < 2 and slot < 8:
        w.ext("w_33204", slot)



G_MARKS_ON, G_MARKS_SHOWN, G_MARKS_N, G_MARKS = 0x229DD8, 0x229DF4, 0x229E08, 0x10DF90   # path preview marks


def path_marks_off(w):
    """0x1D8A4 (its EAX argument is overwritten): HidePathMarks? 0x1D8B8 with AL = 1 when the preview flag 0x229DD8
    is set, else 0"""
    w.ext("ui_refresh", int(w.g32(G_MARKS_ON) != 0))


def escape_stop(w):
    """TacticalKeyHandler 0x13FF3 (Escape): the selected merc (< 8; combat 0x92DCB off or the screen 0x92DCC drawn)
    stops: shown slot 0x229CB4 = 0x40, ClearTarget(1), +0x1F7 = 0x40, ClearTask(1), +0x120 = +0x114, EndTask(1),
    +0x197 = 0, SnapToTile(1), AnimStep when AtWaypoint, StopWalk(0) when the busy display 0x92DCE is on."""
    slot = w.g8(G_SELECTED)
    if slot >= 8 or (w.g8(G_COMBAT) and not w.g8(G_SHOW_UI)):
        return
    s = w.rec(slot)
    w.s32(0x229CB4, 0x40)
    clear_target(s, w, 1)
    s[0x1F7] = 0x40
    clear_task(s, 1)
    w16(s, 0x120, u16(s, 0x114))
    w.ext("end_task", s, 1)
    s[0x197] = 0
    snap_to_tile(s, w, 1)
    if at_waypoint(s):
        w.ext("anim_step", s)
    if w.g8(G_BUSY_UI):
        w.ext("stop_walk", s, 0)


def hide_path_marks(w, force):
    """HidePathMarks? 0x1D8B8 (AL = force): 0x229DEC = 0, 0x229D2C = none; when marks are shown (0x229DF4) and the
    preview flag 0x229DD8 is not positive or AL is set: both flags clear and, with 0x1163BF set, each marked tile
    (u16 list 0x10DF90, count 0x229E08) loses its mark (tile byte 7 = 0), the count clears and the list is zeroed
    (0x100 bytes)."""
    w.s32(0x229DEC, 0)
    w.s32(0x229D2C, NO_TILE)
    if not w.g32(G_MARKS_SHOWN):
        return
    if _i32(w.g32(G_MARKS_ON)) > 0 and not force & 0xFF:
        return
    w.s32(G_MARKS_ON, 0)
    w.s32(G_MARKS_SHOWN, 0)
    if not w.g8(0x1163BF):
        return
    for i in range(_i32(w.g32(G_MARKS_N))):
        w.s8(G_TILEMAP + 8 * s16(w.g16(G_MARKS + 2 * i)) + 7, 0)
    w.s32(G_MARKS_N, 0)
    for i in range(0x100):
        w.s8(G_MARKS + i, 0)



def rect_visible(w, x0, y0, x1, y1):
    """0x1FBDC: 1 unless the rectangle lies wholly outside the view"""
    vx, vy = _i32(w.g32(G_VIEW_X)), _i32(w.g32(G_VIEW_Y))
    if x1 < vx or y1 < vy or x0 > vx + _i32(w.g32(G_VIEW_W)) - 1 or y0 > vy + _i32(w.g32(G_VIEW_H)) - 1:
        return 0
    return 1


G_BLAST_LIST = 0x229D50        # u16[30] tiles; an explosion on one of them clears its entry (0xC41)
BLAST_OFS, BLAST_SFX = 0x93D90, 0x94D23   # i8[3] per blast size: sprite x / y offset; u8 per size: sound sample
G_BLAST_FX, G_BLAST_FX_ID, G_BLASTER = 0x229E03, 0x229E04, 0x92DC2


def explode(w, slot, tile, kind, hit):
    """Explode 0x3B79C (AL = thrower slot, EDX = tile, EBX = kind, ECX = the impact test's result): kind 10 counts as 3.
    Size / loudness / volume by kind: 2 -> 2 / 25 / 30, 3 -> 3 / 50 / 50, 4..9 -> 4 / 10 / 30. The blast sprite point
    is the tile's corner plus
    the size's offsets; out of view the volume drops by 20. Sound (sample per size). A size-2 blast on an unstandable
    tile breaks its object (0x3ACB8) unless the sector forbids it (0x92E41 per 0x22A038) and marks a merc thrower
    (+0x9D = 1). The effect starts (0x156B8: tile, sprite x, y, list 0x22A4D0 below kind 4 else 0x229E18, 0x15 /
    0x0A, kind, ECX low byte) and its id goes to 0x229E03 / 0x229E04. Noise kind 6 from the thrower, class 0, at the
    loudness; 0x92DC2 = the thrower (0x40 for none). Last the tile leaves the list 0x229D50."""
    slot &= 0xFF
    kind = _i32(kind)
    if kind == 10:
        kind = 3
    if kind == 2:
        size, loud, vol = 2, 0x19, 0x1E
    elif kind == 3:
        size, loud, vol = 3, 0x32, 0x32
    elif 4 <= kind <= 9:
        size, loud, vol = 4, 0xA, 0x1E
    else:
        raise ValueError(f"Explode: kind {kind} leaves its locals unset")
    t = _i32(tile)
    x = s16((t % 56) * 16 + s8(w.g8(BLAST_OFS + 3 * size)))
    y = s16((t // 56) * 14 + s8(w.g8(BLAST_OFS + 3 * size + 1)))
    if not rect_visible(w, x, y, x + 0x28, y + 0x1E):
        vol = (vol - 0x14) & 0xFF
    w.ext("sfx", s8(w.g8(BLAST_SFX + size)) & 0xFFFFFFFF, 0x2B11, s8(vol) & 0xFFFFFFFF, 1)
    if size == 2 and tile_standable(w, t) < 0 and not w.g8(0x92E41 + s8(w.g8(0x22A038))):
        w.ext("break_obj", t, w.tile8(t, 1), 1, 1)        # TileObject 0x23760 = tile byte 1
        if slot < 8:
            w.s8(G_SOLDIERS + 0x200 * slot + 0x9D, 1)
    fx = w.ext("puff", t, x & 0xFFFFFFFF, y & 0xFFFFFFFF, w.ptr(0x22A4D0 if kind < 4 else 0x229E18),
               0x15 if kind < 4 else 0xA, s8(kind & 0xFF) & 0xFFFFFFFF, s8(hit & 0xFF) & 0xFFFFFFFF) & 0xFF
    w.s8(G_BLAST_FX, fx)
    w.s32(G_BLAST_FX_ID, s8(fx) & 0xFFFFFFFF)
    w.ext("make_noise", slot, t, 0, loud, 6)
    w.s8(G_BLASTER, slot if slot < 0x30 else 0x40)
    blast_list_remove(w, t)



def land_item(s, w, tile, item, cond, x, y):
    """LandItem? 0x56FE4 (EAX = thrower, EDX = tile, BL = item, CL = condition, stack: x, y; ret 8): an item already on
    the tile goes through 0x1AC68 (thrower, tile, its index, 0); the item (count 1, the condition) is put on the tile
    (0x3FC78: tile, 0, the 12-byte item by value, 1, 0, 0, 0). On water a splash effect starts (0x156B8: list 0x95670,
    5, 0, 1; id to 0x229E03 / 0x229E04), sample 0x1F, noise kind 8; on land sample 0x10, noise kind 8 for item 0x5D
    else 9. r = Random(4) + 4: sound volume r x 10 + 20, noise from the thrower at loudness r, class of the ground."""
    t = _i32(tile)
    item &= 0xFF
    d0 = item | 1 << 8 | (cond & 0xFF) << 16
    if item_index(w, t) < 0xFF:
        w.ext("w_1ac68", s, t, item_index(w, t), 0)
    w.ext("put_item", t, 0, 0, d0, d0, 0, 0, 1, 0, 0, 0)
    if is_water_tile(w.tile8(t, 0), w.tile8(t, 1)):
        fx = w.ext("puff", t, x & 0xFFFFFFFF, y & 0xFFFFFFFF, w.ptr(0x95670), 5, 0, 1) & 0xFF
        w.s8(G_BLAST_FX, fx)
        w.s32(G_BLAST_FX_ID, s8(fx) & 0xFFFFFFFF)
        r = (w.random(4) + 4) & 0xFF
        sample, kind = 0x1F, 8
    else:
        r = (w.random(4) + 4) & 0xFF
        sample, kind = 0x10, 8 if item == 0x5D else 9
    vol = (r * 10 + 0x14) & 0xFF
    w.ext("sfx", sample, 0x2B11, s8(vol) & 0xFFFFFFFF, 1)
    w.ext("make_noise", s[0x17A], t, w.g8(G_T0_CLASS + w.tile8(t, 0)), r, kind)



G_BLED = 0xFA928               # u8 per slot: bled this turn


def bleed(s, w):
    """Bleed? 0x15034 (start of turn): only with more than 12 wound points or +0x186 = 1, and not while doing first
    aid (+0x1B9 4 / 6). The bleed timer +0x1F0 counts down; at 0: with hp short of hpMax - wounds and +0x186 = 0 the
    wounds grow by 1 (0x323D4, portrait flash); else (+0x186 = 1: a merc with hp 10+ says message 9 once, +0x187 = 1;
    hpMax above 15 loses 1 with the wounds) the soldier takes 1 damage (kind 3, no attacker). Then 0xFA928[slot] = 1
    and the timer restarts at hp / (+0x1F2 + 10) + 1."""
    if not (s8(s[0x185]) > 12 or s8(s[0x186]) == 1) or s[0x1B9] in (4, 6):
        return
    w16(s, 0x1F0, u16(s, 0x1F0) - 1)
    if s16(u16(s, 0x1F0)) > 0:
        return
    if (s[0x184] - s[0x185] - s[0x183]) & 0xFF and not s[0x186]:
        s[0x185] = (s[0x185] + 1) & 0xFF
        w.ext("w_323d4", s, 1)
        s[0x189] = 7
        w.ext("portrait", s[0x17A], 1)
    else:
        if s8(s[0x186]) == 1:
            if s8(s[0x183]) >= 10 and not s[0x187]:
                keep = w.g8(G_SHOW_UI)
                if w.g8(G_COMBAT):
                    w.s8(G_SHOW_UI, 0)
                w.ext("merc_message", s, 9, 0x40, w.ptr(0x90384), 1)
                w.s8(G_SHOW_UI, keep)
                s[0x187] = 1
            if s8(s[0x184]) > 15:
                s[0x184] = (s[0x184] - 1) & 0xFF
                s[0x185] = (s[0x185] - 1) & 0xFF
        w.ext("take_damage", s, 0x1356, 1, 3, 0x40, NO_TILE)
    w.s8(G_BLED + s[0x17A], 1)
    w16(s, 0x1F0, cdiv(s8(s[0x183]), s8(s[0x1F2]) + 10) + 1)


def drown(s, w):
    """Drown? 0x3843C (start of turn, energy 0): only in a water animation (0x39+). The view comes to a visible soldier
    (or with 0x93458). First time (+0x1BA = 0): message 10, +0x1BA = 1, stat 4 practised by 5. Later: damage kind 2
    of Random(10) + 2 + Random(10), doubled when +0x19C is 5, at height 0x1356, no attacker; dead -> 0x3A8C0(s, 0)."""
    if u16(s, 0x1E8 + 2 * s[0x1F3]) < 0x39:
        return
    if s8(s[0x9C]) > 0 or w.g8(0x93458):
        w.ext("focus", s[0x17A], 0)
    if not s[0x1BA]:
        w.ext("merc_message", s, 0xA, 0x40, w.ptr(0x90914), 1)
        s[0x1BA] = 1
        w.ext("stat_practice", s, 4, 5, 0)
        return
    d = (w.random(10) + 2) & 0xFF
    d = (d + w.random(10)) & 0xFF
    if s[0x19C] == 5:
        d = (d * 2) & 0xFF
    w.ext("take_damage", s, 0x1356, d, 2, 0x40, NO_TILE)
    if not s[0x183]:
        w.ext("w_3a8c0", s, 0)


def start_turn(s, w):
    """StartTurn? 0x11FB4 (per soldier at its side's turn start): +0x1C4 1 -> 0xFF, 0xFF -> 0; queued aim and its
    AP cleared (+0x1E1..+0x1E3), +0x16D = 0; first aid runs (0x15260) while +0x1B9; 0x32D04; +0x1DB = 0; bleeding
    (wounds or +0x186 = 1); drowning (alive with energy 0). A living soldier: +0x1F2 = 0; in combat: sightings aged
    (UpdateSightings, 1), the noise to check (+0x122) ages out with +0x1C0, AP = MercMaxAP + up to 5 saved (0 stays
    0), at most 25; a merc whose +0x17E is 8 gets +0x17E = 0, +0x1B0 = 0, +0x16D = 1 when +0x120 is another tile,
    posture, 0x331A4(slot, 1); an enemy +0x1B0 = 0. Out of combat AP = MercMaxAP. A merc with hp 15+ in the current
    sector (0x22A057, not 0x22A056) faces again (0x1491C) unless 0x1F6E4. Then AnimDelay."""
    from combat import max_ap
    v = s8(s[0x1C4])
    if v == 1:
        s[0x1C4] = 0xFF
    elif v == -1:
        s[0x1C4] = 0
    s[0x1E1] = s[0x1E2] = s[0x1E3] = 0
    s[0x16D] = 0
    if s[0x1B9]:
        w.ext("aid_tick", s)
    w.ext("w_32d04", s)
    s[0x1DB] = 0
    if s[0x185] or s8(s[0x186]) == 1:
        bleed(s, w)
    if s[0x183] and not s[0x18A]:
        drown(s, w)
    if not s[0x183]:
        return
    slot = s[0x17A]
    s[0x1F2] = 0
    if w.g8(G_COMBAT):
        w.ext("perceive", s, 1)
        if s16(u16(s, 0x122)) != NO_TILE and s[0x1C0]:
            s[0x1C0] = (s[0x1C0] - 1) & 0xFF
            if not s[0x1C0]:
                w16(s, 0x122, NO_TILE)
        saved = 5 if s8(s[0x1A5]) > 5 else s[0x1A5]
        m = max_ap(s) & 0xFF
        s[0x1A5] = (m + saved) & 0xFF if m else 0
        if s8(s[0x1A5]) > 0x19:
            s[0x1A5] = 0x19
        if slot < 8:
            if s[0x17E] == 8:
                s[0x17E] = 0
                s[0x1B0] = 0
                if s16(u16(s, 0x120)) != NO_TILE and u16(s, 0x120) != u16(s, 0x114):
                    s[0x16D] = 1
                w.ext("reserve_ap", s)
                w.ext("w_331a4", slot, 1)
        else:
            s[0x1B0] = 0
    else:
        s[0x1A5] = max_ap(s) & 0xFF
    if (slot < 8 and s8(s[0x183]) >= 15 and w.g8(G_CUR_SECTOR) == s8(w.g8(0x22A057)) and not w.g8(0x22A056)
            and not w.ext("w_1f6e4") & 0xFF):
        reveal_view(s[0x1A1], s[0x19E], s, w, 0)
    anim_delay(s, w)



POCKET_SLOTS = 0x93D59         # u8 per pocket 1..6 (+1): the item entry index it maps to


def revive_roll(s, w):
    """ReviveRoll? 0x401B4 (animation op 0x1EA; a collapsed soldier, hp > 0, in play, not unconscious): noise kind 3
    at loudness 7 from its tile. A medical item (0x0F / 0x10) in hand counts; else each pocket (entry from 0x93D59)
    holding one is swapped into the hand when two Random(100) rolls fall below dexterity, or always for a profile
    0x3C+ (not a hired merc) or an aiType-1 soldier (counted too). Random(5) = 0: animation 0x3D, nothing else. Chance = ((hpMax -
    hp - wounds) / 2 + hp) x energy / 100 - (100 - dexterity) / 4, +80 % with a medical item, +40 % for a profile
    0x3C+ or aiType 1, at most 95. Random(100) at or above it: the death routine 0x3A8C0(s, 0). Else animation 0x3F:
    with a medical item stats 4/3/2 practise by 25 (animation 0x3F again); without one by 10, then Random(30) damage
    (kind 2, no attacker), AnimStep and StopWalk."""
    if s8(s[0x183]) <= 0 or not s[0x182] or not s[0x138] or s[0x188]:
        return
    # 0x11D40(0, 7) returns at once for EAX = 0
    w.ext("make_noise", s[0x17A], s16(u16(s, 0x114)), s[0x1B3], 7, 3)
    kit = False
    if 0xF <= s[0] <= 0x10:
        kit = True
    else:
        for i in range(1, 7):
            if 0xF <= s[12 * s8(w.g8(POCKET_SLOTS + i))] <= 0x10:
                dex = s8(s[0x18C])
                if (w.random(100) < dex and w.random(100) < dex) or s[0x179] >= 0x3C or s8(s[0xBA]) == 1:
                    kit = True
                    w.ext("swap_hand", s, 0, i)
    if not w.random(5):
        w.ext("set_anim", s, 0x3D, 1, 1)
        return
    c = cdiv(((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + s8(s[0x183]), 1) * s8(s[0x18A])
    c = cdiv(c, 100) - cdiv(100 - s8(s[0x18C]), 4)
    if kit:
        c += cdiv(c * 80, 100)
    if s[0x179] >= 0x3C or s8(s[0xBA]) == 1:
        c += cdiv(c * 40, 100)
    if c > 0x5F:
        c = 0x5F
    if w.random(100) >= c:
        w.ext("w_3a8c0", s, 0)
        return
    w.ext("set_anim", s, 0x3F, 0, 1)
    for stat in (4, 3, 2):
        w.ext("stat_practice", s, stat, 0x19 if kit else 0xA, 0)
    if kit:
        w.ext("set_anim", s, 0x3F, 0, 1)
        return
    w.ext("take_damage", s, 0x1356, w.random(30), 2, 0x40, NO_TILE)
    w.ext("anim_step", s)
    w.ext("stop_walk", s, 0)



WEAPON_TBL = 0x94C31           # 6 B per item type: class, ammo item, ?, speed, b4, damage (b5)
G_ATTACK_SND = 0x92E66         # i16 per weapon class: attack sound


def melee_hit(s, w):
    """MeleeHit? 0x3521C (AnimFrame op 0x1EB; the attacker): it becomes the acting soldier, 0x229E04 = -2. No soldier
    on the attack tile (+0xDC): in the story sector (0x22A057, not 0x22A055 / 0x22A056) hitting object 0xAE runs
    0x61D28. A soldier: its +0x1C4 = 1, a merc attacker practises stat 3. The blow cannot miss (chance 99, height
    0x1356) when the target is on the same side, does not see the attacker (sight flag not 1) or has hp below 15;
    else chance = MeleeChance(attacker, target, +0x1FD aim) and height = Random(2500) + 2500. Random(100) below the
    chance hits: damage = (weapon damage + Random(31) - 15 + (level - 1) x 2 + ((hpMax - hp - wounds) / 2 + hp - 50)
    / 5 + (chance - roll) / 5) x condition / 100, at least 1, as kind 1 from the attacker (TakeDamage); a merc
    practises stat 3; a weapon above condition 10 wears by Random(n), n = damage / 10 (halved for item 0x10), at most
    condition / 10, only when n > 1. A miss on a target that could have missed: a target with hp 15+ not in state 2
    reacts (HitAlert, flag 1), a merc target practises stat 2. Always: SoldierSound(attacker, ShotApCost + aim, the
    weapon class's sound), +0x1BE = 0."""
    slot = s[0x17A]
    w.s8(G_ACTIVE_SLOT, slot)
    w.s32(0x229E04, 0xFFFFFFFE)
    tile = s16(u16(s, 0xDC))
    occ = _occupant(w, tile)
    if occ >= 0x40:
        if (w.g8(G_CUR_SECTOR) == s8(w.g8(0x22A057)) and not w.g8(0x22A055) and not w.g8(0x22A056)
                and w.tile8(tile, 1) == 0xAE):
            w.ext("story_hit")
    else:
        if occ == slot:                       # its own record when it hits itself
            s[0x1C4] = 1
        else:
            w.s8(G_SOLDIERS + 0x200 * occ + 0x1C4, 1)
        o = s if occ == slot else w.rec(occ)
        if slot < 8:
            w.ext("stat_practice", s, 3, 1, 0)
        if occ < 0x28 and slot >= 0x28:
            sure = s8(o[0x112 + slot]) != 1
        elif occ >= 0x28 and slot < 0x28:
            sure = s8(o[0x13A + slot]) != 1
        else:
            sure = False
        if s8(o[0x183]) < 15:
            sure = True
        if sure:
            height, chance = 0x1356, 0x63
        else:
            chance = melee_chance(s, o, w, s[0x1FD]) & 0xFF
            height = (w.random(0x9C4) + 0x9C4) & 0xFFFF
        roll = w.random(100) & 0xFF
        if roll < chance:
            d = (w.g8(WEAPON_TBL + 6 * s[0] + 5) + w.random(0x1F) - 15) & 0xFFFF
            d = (d + (s8(s[0x192]) - 1) * 2) & 0xFFFF
            d = s16(d) + cdiv(((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + s8(s[0x183]) - 0x32, 5)
            d = s16(d & 0xFFFF) + cdiv(chance - roll, 5)
            d = cdiv(s8(s[2]) * s16(d & 0xFFFF), 100)
            if s16(d & 0xFFFF) < 1:
                d = 1
            d = s16(d & 0xFFFF)
            w.ext("take_damage", occ, height, d & 0xFFFFFFFF, 1, slot, NO_TILE)
            if slot < 8:
                w.ext("stat_practice", s, 3, 1, 0)
            if s8(s[2]) > 10:
                q = cdiv(d, 10) & 0xFF
                if s[0] == 0x10:
                    q >>= 1
                cap = cdiv(s8(s[2]), 10)
                if q > cap:
                    q = cap & 0xFF
                if q > 1:
                    s[2] = (s[2] - w.random(q)) & 0xFF
        else:
            o = s if occ == slot else w.rec(occ)
            if sure and s8(o[0x183]) >= 15 and s8(o[0xBA]) != 2:
                w.ext("react", occ, slot, 1)
            if occ < 8:
                w.ext("stat_practice", occ, 2, 1, 0)
    cost = w.ext("shot_ap", s, tile) & 0xFF
    snd = s16(w.g16(G_ATTACK_SND + 2 * w.g8(WEAPON_TBL + 6 * s[0])))
    w.ext("spend_ap", s, s8((cost + s8(s[0x1FD])) & 0xFF) & 0xFFFFFFFF, snd & 0xFFFFFFFF)
    s[0x1BE] = 0



G_TEAM_ON_DUTY = 0x229FAC      # i32 mercs on duty
G_DEAD_NPC, G_DEAD_MERCS, G_DEAD_ALLIES = 0x229AE8, 0x229AE0, 0x229AE4   # i32 death counters
G_SECTOR_REC = 0x222C12        # 0xB1 B per sector (byte compared with an enemy's entry 9 item)


def forget_tiles(w, s):
    """0x74BD4 (EAX = soldier): unless +0xBA is 2, the soldier's own row of last-known tiles is cleared to 0xC41: a
    merc or guard (slot byte below 0x28) its 8 words of G_LAST_SEEN_M, an enemy its 0x28 words of G_LAST_SEEN_E."""
    if s8(s[0xBA]) == 2:
        return
    slot = s[0x17A]
    if slot < 0x28:
        for j in range(8):
            w.s16(G_LAST_SEEN_M + 16 * slot + 2 * j, NO_TILE)
    else:
        for j in range(0x28):
            w.s16(G_LAST_SEEN_E + 0x50 * (slot - 0x28) + 2 * j, NO_TILE)


def forget_soldier(s, w):
    """0x3A148 (EAX = soldier) when it leaves the board: +0x108 = 0xC41. Every soldier 0..0x2F whose +0x1D8, +0x1F7
    or +0x1C2 names its slot gets 0x40 there. The other side forgets it: a merc / ally (slot below 0x28) for enemies
    0x28..0x2F (sees flag +0x13A + slot: 1 -> +0x1D5 - 1; flag 0, last-seen tile 0xC41, remembers 0), an enemy for
    soldiers 0..15 (flag +0x112 + slot, G_REMEMBER_M, G_LAST_SEEN_M). Its own last-known row (0x74BD4) goes for a merc
    or guard (slot below 0x10) and an enemy, which also clears G_LAST_SPOTTER when it was the spotter. Last its
    knowledge level is 0. (The slot byte is signed in the original; it is always 0..0x2F. The original walks the
    pointer table 0x100958, which always points at g_soldiers.)"""
    w16(s, 0x108, NO_TILE)
    slot = s[0x17A]
    for k in range(0x30):
        p = G_SOLDIERS + 0x200 * k
        for f in (0x1D8, 0x1F7, 0x1C2):
            if w.g8(p + f) == slot:
                w.s8(p + f, 0x40)
    if slot < 0x28:
        for k in range(0x28, 0x30):
            p = G_SOLDIERS + 0x200 * k
            if s8(w.g8(p + 0x13A + slot)) == 1:
                w.s8(p + 0x1D5, w.g8(p + 0x1D5) - 1)
            w.s8(p + 0x13A + slot, 0)
            w.s16(G_LAST_SEEN_E + 0x50 * (k - 0x28) + 2 * slot, NO_TILE)
            w.s8(G_REMEMBER_E + 0x28 * (k - 0x28) + slot, 0)
        if slot < 0x10:
            forget_tiles(w, w.rec(slot))
    else:
        for k in range(0x10):
            p = G_SOLDIERS + 0x200 * k
            if s8(w.g8(p + 0x112 + slot)) == 1:
                w.s8(p + 0x1D5, w.g8(p + 0x1D5) - 1)
            w.s8(p + 0x112 + slot, 0)
            w.s8(G_REMEMBER_M + 8 * k + slot - 0x28, 0)
            w.s16(G_LAST_SEEN_M + 16 * k + 2 * (slot - 0x28), NO_TILE)
        forget_tiles(w, w.rec(slot))
        if w.g8(G_LAST_SPOTTER) == slot:
            w.s8(G_LAST_SPOTTER, 0x40)
    w.s8(G_KNOWN + slot, 0)


def kill_stragglers(w):
    """0x74C44 (home-sector event stage 5): enemies 0x29..0x2F (not 0x28; the original starts at the pointer of slot
    0x29 and steps 0x200) that are present (+0x182, +0x138), with +0x183 of 15 or more, +0x9C of 0 or less (signed)
    and +0x1C4 clear, leave the board (0x3A148) and die (MercDies, not quiet)."""
    for k in range(0x29, 0x30):
        s = w.rec(k)
        if s[0x182] and s[0x138] and s8(s[0x183]) >= 15 and s8(s[0x9C]) <= 0 and not s[0x1C4]:
            forget_soldier(s, w)
            merc_dies(s, w, 0)


def kill(s, w, quiet):
    """SoldierDies? 0x3A8C0 (EAX = soldier, EDX = quiet): the selected soldier first runs 0x53660. Already dead (+0x186
    = 0xFF): nothing. Screen point +0x126/+0x128 = x/y; the selected one clears 0x92DCE; hp, hpMax, wounds, AP = 0,
    fatigue 10000. Not quiet: StopAnim(1), Rest(0, 0), EndTask(1); quiet: +0x1B9 = 0. +0x17F, +0x181, +0x189, +0x1D5 = 0;
    the selected one (not quiet) 0x1C438(0); 0x3A148.
    - merc (slot < 8): +0x1DC = 0, +0x189 = 0, not quiet 0x32EB8(slot, 1); the sector's merc count (tbl9 b2) - 1;
      replay event (sector, slot, profile); every item entry: not quiet put on the tile (0x3FC78) and dropped
      (0x470B0); quiet dropped unless 0x3FC04(item). The panel (0x55FD8) refreshes when it was selected (or during an
      enemy turn it was 0x92DC7), not quiet. Mercs on duty - 1; a non-merc profile (0x3C+) sets 0x92DD5 = 0x40 and
      counts in 0x229AE8, a merc in 0x229AE0.
    - guard (8..15): tbl9 b4 - 1, 0x229AE8 + 1, replay event 'R'; other ally (16..0x27): tbl9 b3 - 1, 0x229AE4 + 1,
      replay 'S'.
    - enemy: a mission runner (0x92DA8) is cleared (and the mission cancelled while it runs with a target); item
      entries 7..11 go on the tile and are dropped (entry 9 clears the sector's matching byte); 0x3A2D4.
    Then +0x186 = 0xFF. Quiet: MercDies?(1). Else +0x195 = 0, death animation 0x40 in water / 0x43, and a merc killer
    (+0x1DA below 8) gets a kill (+0x1D9 + 1), practises stat 9 by level x 5, and is remembered in 0x22A0D2."""
    slot = s[0x17A]
    base = G_SOLDIERS + 0x200 * slot
    if slot == w.g8(G_SELECTED):
        w.ext("w_53660")
    if s8(s[0x186]) == -1:
        return
    w16(s, 0x126, u16(s, 0x116))
    w16(s, 0x128, u16(s, 0x118))
    if w.g8(G_SELECTED) == slot:
        w.s8(0x92DCE, 0)
    s[0x185] = s[0x184] = s[0x183] = 0
    s[0x1A5] = 0
    w16(s, 0x10C, 10000)
    if not quiet:
        w.ext("stop_anim", s, 1)
        w.ext("rest", s, 0, 0)
        w.ext("end_task", s, 1)
    else:
        s[0x1B9] = 0
    s[0x17F] = s[0x181] = s[0x189] = s[0x1D5] = 0
    if slot == w.g8(G_SELECTED) and not quiet:
        w.ext("w_1c438", s, 0)
    forget_soldier(s, w)
    sec = w.g8(G_CUR_SECTOR)
    here = s8(w.g8(G_CUR_SECTOR))
    if slot < 8:
        s[0x1DC:0x1E0] = bytes(4)
        s[0x189] = 0
        if not quiet:
            w.ext("w_32eb8", slot, 1)
        w.s8(G_SECTOR_TBL9 + 9 * sec + 2, w.g8(G_SECTOR_TBL9 + 9 * sec + 2) - 1)
        w.ext("replay_event", here & 0xFFFFFFFF, s8(s[0x17A]) & 0xFFFFFFFF, s8(s[0x179]) & 0xFFFFFFFF)
        tile = s16(u16(s, 0x114))
        for i in range(12):
            e0, e1, e2 = (int.from_bytes(s[12 * i + k:12 * i + k + 4], "little") for k in (0, 4, 8))
            ent = slot if i == 0 else w.ptr(base + 12 * i)
            if not quiet:
                w.ext("put_item", tile, w.ptr(base + 12 * i), w.ptr(base + 0x90), e1, e0, e1, e2, 1, 0, 0, 0)
                w.ext("drop_item", ent)
            elif not w.ext("w_3fc04", s[12 * i]):
                w.ext("drop_item", ent)
        sel = w.g8(G_SELECTED)
        if (sel == slot or (sel >= 8 and slot == w.g8(0x92DC7))) and not quiet:
            w.ext("item_panel", s, 1, 1)
        w.s32(G_TEAM_ON_DUTY, w.g32(G_TEAM_ON_DUTY) - 1)
        if s[0x179] >= 0x3C:
            w.s8(0x92DD5, 0x40)
            w.s32(G_DEAD_NPC, w.g32(G_DEAD_NPC) + 1)
        else:
            w.s32(G_DEAD_MERCS, w.g32(G_DEAD_MERCS) + 1)
    elif slot <= 0xF:
        w.s8(G_SECTOR_TBL9 + 9 * sec + 4, w.g8(G_SECTOR_TBL9 + 9 * sec + 4) - 1)
        w.s32(G_DEAD_NPC, w.g32(G_DEAD_NPC) + 1)
        w.ext("replay_event", here & 0xFFFFFFFF, here & 0xFFFFFFFF, 0x52)
    elif slot <= 0x27:
        w.s8(G_SECTOR_TBL9 + 9 * sec + 3, w.g8(G_SECTOR_TBL9 + 9 * sec + 3) - 1)
        w.s32(G_DEAD_ALLIES, w.g32(G_DEAD_ALLIES) + 1)
        w.ext("replay_event", here & 0xFFFFFFFF, here & 0xFFFFFFFF, 0x53)
    else:
        if slot == w.g8(G_MISSION_ORDER):
            w.s8(G_MISSION_ORDER, 0x40)
            if s8(w.g8(0x22A0D0)) > 0 and s16(w.g16(G_MISSION_ORDER + 2)) != NO_TILE:
                w.ext("mission_cancel")
        tile = s16(u16(s, 0x114))
        for i in range(7, 12):
            if i == 9 and s[0x6C] == w.g8(G_SECTOR_REC + 0xB1 * sec):
                w.s8(G_SECTOR_REC + 0xB1 * sec, 0)
            e0, e1, e2 = (int.from_bytes(s[12 * i + k:12 * i + k + 4], "little") for k in (0, 4, 8))
            w.ext("put_item", tile, w.ptr(base + 12 * i), i, e0, e0, e1, e2, 1, 0, 1, 0)
            w.ext("drop_item", w.ptr(base + 12 * i))
        w.ext("w_3a2d4")
    s[0x186] = 0xFF
    if quiet:
        w.ext("merc_dies", s, 1)
        return
    s[0x195] = 0
    w.ext("set_anim", s, 0x40 if in_water(s) else 0x43, 0, 1)
    k = s[0x1DA]
    if k < 8:
        if k == slot:                         # its own record when it killed itself
            s[0x1D9] = (s[0x1D9] + 1) & 0xFF
        else:
            w.s8(G_SOLDIERS + 0x200 * k + 0x1D9, w.g8(G_SOLDIERS + 0x200 * k + 0x1D9) + 1)
        w.ext("stat_practice", k, 9, s16((s8(s[0x192]) * 5) & 0xFFFF) & 0xFFFFFFFF, 0)
        w.s8(0x22A0D2, k)



def blast_tile(w, centre, tile, ring, kind):
    """BlastTile? 0x38638 (EAX = blast centre, EDX = tile, EBX = ring distance, ECX = kind): kind 2: damage
    Random(26) + 25, 3: Random(51) + 50 (effect 4, explosion); 4..5: the tile is gassed (byte 5 | 0x80) and stun
    Random(1001) + 2000; 6..7: stun Random(2001) + 5000 and damage 1; 8..9: stun Random(2001) + 4000, damage
    Random(16) + 15 (effect 6, gas); 99 clears the gas mark; others nothing. A soldier in play and alive on the
    tile: rings below 5 take 20 % less per ring, farther ones a tenth (not below 0). Any damage or stun: a merc
    blaster (0x92DC2) practises stat 6. A standing soldier off the centre turns to it (+0x1A2 = DirectionTo its point
    to the centre tile's corner + (8, 7)). Damage (not aiType 4): TakeDamage at height 2500 with the effect as kind,
    from the blaster, the centre as tile; explosions then run 0x389BC(damage). Stun on a living soldier (not aiType
    4; a gas mask, item 0x28 in entry 3, lets (100 - its condition) % through): fatigue rises by it, without damage
    the soldier coughs (0x17B70, 3) and the reaction flag clears; gas on water or kind 4 sets +0x1DB; re-think.
    +0x1C4 = 1. Without damage, from a soldier blaster: stun names it the killer (+0x1DA) and a soldier with hp 15+
    reacts (HitReaction: blaster, flag, stun / 300)."""
    kind &= 0xFFFFFFFF
    dmg = stun = 0
    fx = None
    flag = 1
    t = _i32(tile)
    if kind == 2:
        dmg, fx = w.random(0x1A) + 0x19, 4
    elif kind == 3:
        dmg, fx = w.random(0x33) + 0x32, 4
    elif kind in (4, 5):
        w.s8(G_TILEMAP + 8 * t + 5, w.tile8(t, 5) | 0x80)
        stun, fx = w.random(0x3E9) + 0x7D0, 6
    elif kind in (6, 7):
        stun, dmg, fx = w.random(0x7D1) + 0x1388, 1, 6
    elif kind in (8, 9):
        stun, fx = w.random(0x7D1) + 0xFA0, 6
        dmg = w.random(0x10) + 0xF
    elif kind == 0x63:
        w.s8(G_TILEMAP + 8 * t + 5, w.tile8(t, 5) & 0x7F)
    occ = _occupant(w, t)
    if occ >= 0x40:
        return
    p = w.rec(occ)
    if not p[0x182] or not p[0x138] or not p[0x183]:
        return
    ring = _i32(ring)
    if ring < 5:
        dmg -= cdiv(dmg * ring * 20, 100)
        stun -= cdiv(stun * ring * 20, 100)
    else:
        dmg, stun = cdiv(dmg, 10), cdiv(stun, 10)
    dmg, stun = max(dmg, 0), max(stun, 0)
    blaster = w.g8(G_BLASTER)
    if (dmg or stun) and blaster < 8:
        w.ext("stat_practice", blaster, 6, 1, 0)
    c = _i32(centre)
    if u16(p, 0x1E8 + 2 * p[0x1F3]) >= 4 and s16(u16(p, 0x114)) != c:
        p[0x1A2] = _i32(w.ext("direction", s16(u16(p, 0x116)), s16(u16(p, 0x118)), (c % 56) * 16 + 8,
                              (c // 56) * 14 + 7)) & 0xFF
    if dmg and s8(p[0xBA]) != 4:
        w.ext("take_damage", occ, 0x9C4, dmg, fx, blaster, s16(c & 0xFFFF))
        if fx == 4:
            blast_wear(w.rec(occ), w, dmg)
        p = w.rec(occ)
    if p[0x183] and stun and s8(p[0xBA]) != 4:
        if p[0x24] == 0x28:
            stun = cdiv((100 - s8(p[0x26])) * stun, 100)
        if stun:
            w.ext("rest", occ, (-stun) & 0xFFFFFFFF, 1)
            p = w.rec(occ)
            if not dmg:
                w.ext("w_17b70", p[0x17A], 3)
                flag = 0
            if fx == 6 and (w.g8(G_T0_CLASS + w.tile8(s16(u16(p, 0x114)), 0)) == 3 or kind == 4):
                p[0x1DB] = 1
            p[0x1C1] = 1
    p[0x1C4] = 1
    if not dmg and blaster < 0x40:
        if stun:
            p[0x1DA] = blaster
        if s8(p[0x183]) >= 15:
            w.ext("w_74304", occ, blaster, flag, cdiv(stun, 300) & 0xFF)



ITEM_TBL = 0x94DF8             # 12 B per item type (+2 kind, +6 stack class, +7 wears in blasts)


def _blast_wear_amount(w, item, dmg):
    """0x38AC8 (AL = item, EDX = damage) -> condition lost in a blast: 0 unless the item type wears (+7); helmets
    0x23..0x26 and armour 0x2D..0x31 first subtract their absorb value (not below 0); a set byte at +12 of the entry
    (the next type's +0) halves it; Random(n) when n > 1, else 0."""
    item &= 0xFF
    if not w.g8(ITEM_TBL + 12 * item + 7):
        return 0
    d = _i32(dmg)
    if 0x23 <= item <= 0x26:
        d = max(d - s16(w.g16(0x92F3C + 2 * item)), 0)
    if 0x2D <= item <= 0x31:
        d = max(d - s16(w.g16(0x92F1E + 2 * item)), 0)
    if w.g8(ITEM_TBL + 12 * item + 12):
        d = cdiv(d, 2)
    return w.random(s16(d)) & 0xFF if d > 1 else 0


def blast_wear(s, w, dmg):
    """BlastWear? 0x389BC (EAX = soldier, EDX = damage), after blast damage: every item entry with an item and a
    non-negative condition loses 0x38AC8(item, damage) condition (at least 1 left); for a stacking type (+6 > 1) with
    count above 1 the four stacked conditions (+8..+11) above 0 wear too. An attachment (+3) wears its condition +5
    (entries with a negative condition skip this). The selected merc's panel refreshes (0x55FD8(s, 0, 1))."""
    for k in range(12):
        e = 12 * k
        if s[e]:
            if s8(s[e + 2]) < 0:
                continue
            s[e + 2] = (s[e + 2] - _blast_wear_amount(w, s[e], dmg)) & 0xFF
            if s8(s[e + 2]) < 1:
                s[e + 2] = 1
            if w.g8(ITEM_TBL + 12 * s[e] + 6) > 1 and s8(s[e + 1]) > 1:
                for c in range(4):
                    if s8(s[e + 8 + c]) > 0:
                        s[e + 8 + c] = (s[e + 8 + c] - _blast_wear_amount(w, s[e], dmg)) & 0xFF
                        if s8(s[e + 8 + c]) < 1:
                            s[e + 8 + c] = 1
        if s[e + 3]:
            s[e + 5] = (s[e + 5] - _blast_wear_amount(w, s[e + 3], dmg)) & 0xFF
            if s8(s[e + 5]) < 1:
                s[e + 5] = 1
    if s[0x17A] < 8 and s[0x17A] == w.g8(G_SELECTED):
        w.ext("item_panel", s, 0, 1)


def obj_block(w, tile, d, side):
    """0x23780 (EAX = tile, DL = direction, EBX = side or 0xC41): how the tile's object blocks passing in that
    direction: none below 0x27 and from 0x83 except 0xE2 / 0xE3 (2); 0x27..0x36 = 2; walls 0x37..0x82 from the
    9-byte rows at 0x95694 (column = direction); with a side given, a value up to 2 is cleared when the row's
    orientation byte (+9) is 1 and the side 3 / 7, or 2 and the side 1 / 5."""
    o = w.tile8(tile, 1)
    if o < 0x27:
        return 0
    if o >= 0x83:
        return 2 if 0xE2 <= o <= 0xE3 else 0
    if o <= 0x36:
        return 2
    row = (o - 0x37) * 9
    v = w.g8(G_OBJ_BLOCK + (d & 0xFF) + row)
    if side == NO_TILE or v > 2:
        return v
    m = s8(w.g8(0x9569D + row))
    if (m == 1 and side in (3, 7)) or (m == 2 and side in (1, 5)):
        return 0
    return v


def blast(w, centre, radius, kind):
    """Blast? 0x38B60 (EAX = centre tile, EDX = radius, EBX = kind): BlastTile on the centre (ring 0), then along each
    of the 8 directions ring by ring up to the radius: a step that leaves the map, or into an object of class 6
    (that shortens the reach by one each time), or one the object blocks (0x23780) stops that direction (a kind-3
    blast at ring 1 breaks an unstandable blocker, unless the sector forbids it); else that tile takes BlastTile at
    the ring, and a side fan of radius - ring tiles in the next direction (clockwise) follows from it, each at the
    same ring, skipping blocked steps."""
    c = _i32(centre)
    blast_tile(w, c, c, 0, kind)
    for d in range(1, 9):
        cur, i, lim = c, 1, _i32(radius)
        if s16(lim & 0xFFFF) < 1:
            continue
        side = (lim - 1) & 0xFF
        dn = d + 1
        while True:
            nxt = s16(step_tile(s16(cur), dir_step(w, d)) & 0xFFFF)
            if nxt == s16(cur):
                ok = False
            elif w.g8(G_T1_CLASS_TBL + w.tile8(nxt, 1)) == 6:
                lim -= 1
                ok = not s16(i) > s16(lim & 0xFFFF)
            else:
                ok = obj_block(w, nxt, d, NO_TILE) == 0
            if not ok:
                if (kind == 3 and s16(i) < 2 and tile_standable(w, nxt) < 0
                        and not w.g8(0x92E41 + s8(w.g8(0x22A038)))):
                    w.ext("break_obj", nxt, w.tile8(nxt, 1), 1, nxt & 0xFFFFFFFF)
                break
            blast_tile(w, c, nxt, s16(i), kind)
            cur = nxt
            n = side
            if n:
                d2 = 1 if dn > 8 else dn
                cur2 = nxt
                j = 0
                while True:
                    t2 = s16(step_tile(s16(cur2), dir_step(w, d2)) & 0xFFFF)
                    if t2 != s16(cur2) and obj_block(w, t2, d2, NO_TILE) == 0:
                        blast_tile(w, c, t2, s16(i), kind)
                        cur2 = t2
                    j += 1
                    if n <= s16(j):
                        break
            side = (side - 1) & 0xFF
            i = (i + 1) & 0xFFFF
            if not s16(i) <= s16(lim & 0xFFFF):
                break



G_GAS_CLOUDS = 0x22A44E        # 5 x (u16 tile, i16 turns left, i16 radius); tile 0xC41 = free


def gas_tick(w):
    """GasTick? 0x38520 (once per turn): each cloud in use gasses its area again (Blast kind 4); its turns count down
    and at 0 the gas is cleared (Blast kind 99) and the cloud freed."""
    for k in range(5):
        e = G_GAS_CLOUDS + 6 * k
        if s16(w.g16(e)) == NO_TILE:
            continue
        blast(w, s16(w.g16(e)), s16(w.g16(e + 4)), 4)
        w.s16(e + 2, w.g16(e + 2) - 1)
        if s16(w.g16(e + 2)) <= 0:
            blast(w, s16(w.g16(e)), s16(w.g16(e + 4)), 0x63)
            w.s16(e, NO_TILE)



def sector_workable(w, tbl, sec):
    """SectorWorkable 0x41E1C (EAX = 9-byte sector table, EDX = sector) -> b7, which it also stores: 0 unless the
    sector is owned (b0 bit 0) and connected to home (0x41178). Security = 25 per safe side: north (sector below 10,
    or the sector 10 lower owned, or barrier bit 2), east (column 9, sector + 1 owned, bit 4), south (sector 50 or
    above, sector + 10 owned, bit 8), west (column 0, sector - 1 owned, bit 0x10). b7 = the lower of b6 - b8 and
    b6 x security / 100 (bytes)."""
    b = tbl + 9 * sec
    own = lambda n: w.g8(tbl + 9 * n) & 1
    if not w.g8(b) & 1 or not w.ext("connected_home", tbl, sec) & 0xFF:
        w.s8(b + 7, 0)
        return 0
    f = w.g8(b)
    safe = 25 if sec <= 9 else 25 if own(sec - 10) or f & 2 else 0
    safe += 25 if sec % 10 == 9 or own(sec + 1) or f & 4 else 0
    safe += 25 if sec >= 50 or own(sec + 10) or f & 8 else 0
    safe += 25 if sec % 10 == 0 or own(sec - 1) or f & 0x10 else 0
    v = cdiv(w.g8(b + 6) * safe, 100) & 0xFF
    room = (w.g8(b + 6) - w.g8(b + 8)) & 0xFF
    w.s8(b + 7, room if v > room else v)
    return w.g8(b + 7)


G_HELD = 0x222B50              # the 12-byte item entry on the cursor (picked up)
G_HELD_PROBE = 0x222B68        # 12-byte scratch entry for 0x51A24
G_LISTED_ITEMS = 0x92EB2       # 32 item types (0x3FC04 tests membership)
G_ITEM_KEEPS = 0x94E02         # g_itemTable + 0xA: 0 = a picked-up item gets byte 2 = 100
TILE_STEPS4 = (-56, 1, 56, -1)  # the bytes c8 01 38 ff at 0x1590A: north, east, south, west
PICKUP_QUOTES = {0x79: (0x22A05A, 0x12), 0x86: (0x22A05B, 0x15), 0x5F: (0x22A05D, 0x1B), 0x6E: (0x22A05C, 0x1F)}


G_STAGE_FX = 0x95E0E           # 12 x 7 B: i16 tile, b2, b3 stage, i16 +4, b6 lowest difficulty
G_STAGE = 0x22A1F2             # stage counter of the home-sector event (TriggerItem / LandItem raise it)
G_LOSS_SLOT, G_LOSS_TIMER, G_LOSS_AMOUNT = 0x22A1F1, 0x92DE8, 0x22A02F   # as in combat.py


def age_knowledge(w, side):
    """0x741F8 (AL = side) at a turn change. Side not 0: the mercs' latest call level (0x92DBC) counts down and at
    0 its tile becomes 0xC41, then enemies 0x28..0x2F age; side 0: the enemies' call (0x92DC0 / 0x92DBE) likewise,
    then slots 0..0x27. For each present soldier (+0x182, +0x138 and +0x183 set) the other side's knowledge of it
    (G_KNOWN by its slot byte) moves away from 0 as a byte: 2 or more rises (above 4 -> 0), -1 or less falls
    (below -3 -> 0)."""
    lo, hi, call = (0x28, 0x30, G_CALL_M) if side & 0xFF else (0, 0x28, G_CALL_E)
    t = w.g8(call + 2)
    if t:
        w.s8(call + 2, t - 1)
        if t == 1:
            w.s16(call, NO_TILE)
    for k in range(lo, hi):
        p = G_SOLDIERS + 0x200 * k
        if not (w.g8(p + 0x182) and w.g8(p + 0x138) and w.g8(p + 0x183)):
            continue
        a = G_KNOWN + w.g8(p + 0x17A)
        v = s8(w.g8(a))
        if v >= 2:
            n = s8((v + 1) & 0xFF)
            w.s8(a, 0 if n > 4 else n)
        elif v <= -1:
            n = s8((v - 1) & 0xFF)
            w.s8(a, 0 if n < -3 else n)


def stage_effects(w):
    """0x635DC: every record of the 12 at 0x95E0E whose stage byte equals 0x22A1F2 and whose difficulty byte is at
    most g_difficulty (signed) starts its effect: 0x157BC(tile, b2, 0, i16 +4, 0x41, 0). At stage 0 the stat-loss
    message timer is set to 0x1D4C with merc 0 and amount 0xFE."""
    for k in range(12):
        r = G_STAGE_FX + 7 * k
        if w.g8(r + 3) == w.g8(G_STAGE) and w.g8(r + 6) <= s8(w.g8(G_DIFFICULTY)):
            w.ext("fx_157bc", s16(w.g16(r)), w.g8(r + 2), 0, s16(w.g16(r + 4)) & 0xFFFFFFFF, 0x41, 0)
    if not w.g8(G_STAGE):
        w.s32(G_LOSS_TIMER, 0x1D4C)
        w.s8(G_LOSS_SLOT, 0)
        w.s8(G_LOSS_AMOUNT, 0xFE)


def call_enemies(w, tile):
    """0x72568 (AX = tile): the enemies' latest call (0x92DBE) goes to the tile with level 0xC."""
    w.s16(G_CALL_E, tile & 0xFFFF)
    w.s8(G_CALL_E + 2, 0xC)


def listed_item(w, typ):
    """0x3FC04 (AL = item type) -> 1 if the type is not 0 and is one of the 32 bytes at 0x92EB2, else 0."""
    typ &= 0xFF
    return int(typ != 0 and any(w.g8(G_LISTED_ITEMS + i) == typ for i in range(32)))


def blast_list_remove(w, tile):
    """BlastListRemove? 0x3B718 (EAX = tile): the first of the 30 words at 0x229D50 equal to the tile -> 0xC41."""
    for i in range(30):
        if s16(w.g16(G_BLAST_LIST + 2 * i)) == _i32(tile):
            w.s16(G_BLAST_LIST + 2 * i, NO_TILE)
            break


def blast_list_add(w, tile):
    """0x3B744 (EAX = tile): nothing when the tile is already in the 30 words at 0x229D50; else it goes into the
    first empty (0xC41) entry, scanned in order and stopped at the first empty one (none free: nothing)."""
    k = 0
    for i in range(30):
        v = s16(w.g16(G_BLAST_LIST + 2 * i))
        if v == _i32(tile):
            return
        if v == NO_TILE:
            break
        k += 1
    if k < 30:
        w.s16(G_BLAST_LIST + 2 * k, tile & 0xFFFF)


def _blast_listed(w, tile):
    return any(s16(w.g16(G_BLAST_LIST + 2 * i)) == _i32(tile) for i in range(30))


G_FX = 0x22A488                # 5 x 13 B timed effects (0x157BC): i32 +0 (-1 = free), u16 +6 tile, +9 timer, +0xC
G_FX_FLAG = 0x284F29


def fx_cancel(w, tile):
    """0x15760 (EAX = tile): the first of the 5 effect records whose tile (+6, 16 bits) equals it is freed (+0 = -1,
    +6 = 0xC41, +0xC = 0), 0x284F29 = 0, returns 1; none: 0."""
    for i in range(0, 0x41, 13):
        if w.g16(G_FX + i + 6) == tile & 0xFFFF:
            w.s32(G_FX + i, 0xFFFFFFFF)
            w.s16(G_FX + i + 6, NO_TILE)
            w.s8(G_FX + i + 0xC, 0)
            w.s8(G_FX_FLAG, 0)
            return 1
    return 0


def _neg_cond(w, k):
    a = G_GROUND_ITEMS + 12 * k + 2
    w.s8(a, -w.g8(a) & 0xFF)


def mine_spotted(s, w, tile, k):
    """0x3C31C (EAX = soldier, EDX = tile, BL = ground item): a merc (slot below 8) at ground item k = an armed mine
    (type 0x11, byte 6 = -1, condition above 0, signed): the tile joins the blast list (0x3B744) and the condition
    is negated; returns 1. Otherwise 0. No bound check on k (0xFF reads past the 200 entries)."""
    g = G_GROUND_ITEMS + 12 * (k & 0xFF)
    if w.g8(g) != 0x11 or s8(w.g8(g + 6)) != -1 or s[0x17A] >= 8 or s8(w.g8(g + 2)) <= 0:
        return 0
    if not _blast_listed(w, tile):
        blast_list_add(w, tile)
    _neg_cond(w, k & 0xFF)
    return 1


def fx_here(s, w):
    """0x3CCEC (EAX = soldier): 0 unless one of the 5 effect records is active (+0 >= 0) on the soldier's tile. Then
    Random(8) < level (+0x192) + explosives (+0x18F) / 3 (signed) puts the tile on the blast list (unless there),
    and the condition of the tile's ground item (byte 6, unchecked) is negated either way; returns 1."""
    me = u16(s, 0x114)
    for i in range(0, 0x41, 13):
        if w.g16(G_FX + i + 6) == me and _i32(w.g32(G_FX + i)) >= 0:
            break
    else:
        return 0
    if w.random(8) < s8(s[0x192]) + cdiv(s8(s[0x18F]), 3):
        t = s16(me)
        if not _blast_listed(w, t):
            blast_list_add(w, t)
    _neg_cond(w, w.tile8(s16(me), 6))
    return 1


def pickup_here(s, w):
    """0x3CDD4 (EAX = soldier; animation op 0x1E8): the ground item of the soldier's own tile (0x23768), unless its
    type is 0x44 / 0x52 / 0x70 or 0x29 / 0x3F / 0x71 (then none: quote event 0xFC). 0x3C31C notes an armed mine
    there, 0x3CCEC an active effect (c), ClearTarget(0). With an item: DisarmExplosivePrompt(soldier, tile, item,
    0, 1) above 0 picks it up (0x3C16C(.., 1)), frees the tile's effect and, with c, lowers the sector's mine count
    (+0xAF); otherwise with c the item's condition is negated back, or with no item there the mine count drops
    and the effect is freed. Last +0x1A6 = 0, +0x134 = 0xC41, +0x1A7 = 0, +0x95 = 0, +0x96 = 0xC41."""
    me = s16(u16(s, 0x114))
    k = item_index(w, me)
    if w.g8(G_GROUND_ITEMS + 12 * k) in (0x44, 0x52, 0x70):
        k = 0xFF
    if k < 0xFF and w.g8(G_GROUND_ITEMS + 12 * k) in (0x29, 0x3F, 0x71):
        k = 0xFF
    if k >= 0xFF:
        w.ext("quote_event", s[0x17A], 0xFC, 1, NO_TILE, 0)
    mine_spotted(s, w, me, k)
    c = fx_here(s, w) & 0xFF
    clear_target(s, w, 0)
    if k < 0xFF:
        if _i32(disarm_explosive(s, w, me, k, 0, 1)) > 0:
            w.ext("w_3c16c", s, me, 1)
            fx_cancel(w, me)
            if c:
                _mines_down(w)
        elif c:
            j = item_index(w, me)
            if j < 0xFF:
                _neg_cond(w, j)
            else:
                _mines_down(w)
                fx_cancel(w, me)
    s[0x1A6] = 0
    w16(s, 0x134, NO_TILE)
    s[0x1A7] = 0
    s[0x95] = 0
    w16(s, 0x96, NO_TILE)


def _mines_down(w):
    a = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR) + 0xAF
    w.s8(a, (w.g8(a) - 1) & 0xFF)


G_TILESET = 0x22A038
G_LOCK_A, G_LOCK_B = 0x95681, 0x9568B   # u8 per tileset: objects 0x9A..0xA4 / 0xA6..0xAA are locked containers there
G_SAFE_OPENED = 0x22A109                 # u8: the sector 0xB object 0x8E was opened with item 0x87


def _no_pickup(w, k):
    return w.g8(G_GROUND_ITEMS + 12 * k) in (0x44, 0x52, 0x70)


def use_on_tile(s, w, tile, flag):
    """0x3CFD0 (EAX = soldier, EDX = tile, BL = flag +0x1A6; animation op 0x1E9 with current animation 0xC): use the
    hand item on a tile. Tile above 0xC40 (signed): nothing at all. The ground item counts only when UseSideOk(own
    tile, tile, 0xFF, object, standable) passes; its byte 6 above 0 = already handled. A merc (slot below 8, not
    the user) on the tile: message 0xE for him, done. With no occupant and the item not handled, the tile object
    decides: 0x8E = a container that needs no crowbar (silent unless tileset 7; in sector 0xB it needs item 0x87:
    without it message 0x17 and stop unless opened before; with it the one-time opening: focus, voice, sound, delay
    1000, practice 4 (0x32), mission step 0 and order 0x40); 0x9A/9C/9E/A0/A2/A4 by 0x95681[tileset], A6/A8/AA by
    0x9568B[tileset] = locked; any other object on an unstandable tile with flag 0x76 = break it open. Locked: with
    an item the disarm prompt (.., 1, 1) must pass, then UseCrowbar (not for 0x8E) must return 1 (else message 0x17)
    and the object byte (t1) rises, with sound 0x22 + noise 3 unless silent; without an item the same without the
    prompt and quote event 0xFB. Not locked: an item of type 0x44 / 0x52 / 0x70 or a non-merc user triggers it
    (0x1AC68(.., 1)); the non-merc then stops. A merc checks the mine (0x3C31C) and practises; break-open: UseCrowbar
    1 -> BreakObject(tile, object, 1, 0xC41), quote event 0xFA, +0x9D = 1; else quote event 0xF9. Then with an item:
    practising -> the disarm prompt (.., 0, 1): yes picks up, frees the effect, a spotted mine lowers the sector
    count; no: -1 marks the item (byte 6 = -1) and ends practice, a spotted mine is negated back (or, with no item
    left, the count drops and the effect is freed). Not practising: unless 0x44 / 0x52 / 0x70 or a non-merc,
    practice 4 (1), byte 6 = 1 and quote event 0xFD. No item, not locked, flag 0 or 0xFE+: quote event 0xFC. Last
    practice 4 (1) when still practising, and the pending action fields are cleared (as 0x3CDD4)."""
    obj = w.tile8(tile, 1)
    k = item_index(w, tile)
    occ = _occupant(w, tile)
    noisy, handled, practise, locked, breaks, nocrow, spotted = True, False, False, False, False, False, 0
    if _i32(tile) > 0xC40:
        return
    st = tile_standable(w, tile)
    if not _side_ok(s16(u16(s, 0x114)), tile, 0xFF, w.tile8(tile, 1), st):
        k = 0xFF
    if k < 0xFF:
        handled = s8(w.g8(G_GROUND_ITEMS + 12 * k + 6)) > 0

    def finish():
        if practise:
            w.ext("stat_practice", s, 4, 1, 0)
        s[0x1A6] = 0
        w16(s, 0x134, NO_TILE)
        s[0x1A7] = 0
        s[0x95] = 0
        w16(s, 0x96, NO_TILE)

    if occ < 0x40:
        if occ < 8 and occ != s[0x17A]:
            w.ext("merc_message", occ, 0xE, 0x40, 0x90914, 1)
        return finish()
    ts = s8(w.g8(G_TILESET))
    if not handled:
        if obj == 0x8E:
            nocrow = locked = True
            noisy = ts == 7
            if w.g8(G_CUR_SECTOR) == 0xB and s[0] != 0x87:
                if not w.g8(G_SAFE_OPENED):
                    w.ext("merc_message", s, 0x17, 0x40, 0x90914, 1)
                    return finish()
            elif w.g8(G_CUR_SECTOR) == 0xB:
                w.ext("focus", s[0x17A], 0)
                w.ext("cry", s[0x179], 0xFFFFFFFF, "GOOD", 1, 1)
                w.ext("voc", 0x2B11, 0x32, 1)
                w.ext("delay", 0x3E8)
                w.s8(G_SAFE_OPENED, 1)
                w.ext("stat_practice", s, 4, 0x32, 0)
                w.s8(G_MISSION_STEP, 0)
                w.s8(G_MISSION_ORDER, 0x40)
        elif obj in (0x9A, 0x9C, 0x9E, 0xA0, 0xA2, 0xA4):
            locked = bool(w.g8((G_LOCK_A + ts) & 0xFFFFFFFF))
        elif obj in (0xA6, 0xA8, 0xAA):
            locked = bool(w.g8((G_LOCK_B + ts) & 0xFFFFFFFF))
        elif tile_standable(w, tile) < 0 and flag & 0xFF == 0x76:
            breaks = True

    def opened():
        if noisy:
            w.ext("sfx", 0x22, 0x2B11, 0x32, 1)
            w.ext("make_noise", s[0x17A], tile, s[0x1B3], 3, 2)

    if locked:
        if k < 0xFF:
            if _i32(disarm_explosive(s, w, tile, k, 1, 1)) <= 0:
                k = 0xFF
            elif not nocrow and _i32(use_crowbar(s, w, tile, flag)) != 1:
                w.ext("merc_message", s, 0x17, 0x40, 0x90914, 1)
                k = 0xFF
            else:
                w.s8(G_TILEMAP + 8 * tile + 1, w.tile8(tile, 1) + 1 & 0xFF)
                opened()
        elif not nocrow and _i32(use_crowbar(s, w, tile, flag)) != 1:
            w.ext("merc_message", s, 0x17, 0x40, 0x90914, 1)
        else:
            opened()
            w.s8(G_TILEMAP + 8 * tile + 1, w.tile8(tile, 1) + 1 & 0xFF)
            w.ext("quote_event", s[0x17A], 0xFB, 6, NO_TILE, 0)
    elif _no_pickup(w, k):
        w.ext("w_1ac68", s, tile, k, 1)
    elif s[0x17A] >= 8:
        w.ext("w_1ac68", s, tile, k, 1)
        return finish()
    else:
        spotted = mine_spotted(s, w, tile, k) & 0xFF
        practise = True
        if breaks:
            if _i32(use_crowbar(s, w, tile, flag)) == 1:
                w.ext("break_obj", tile, obj, 1, NO_TILE)
                w.ext("quote_event", s[0x17A], 0xFA, 2, NO_TILE, 1)
                s[0x9D] = 1
            else:
                w.ext("quote_event", s[0x17A], 0xF9, 2, NO_TILE, 1)
    if k < 0xFF:
        if practise:
            r = _i32(disarm_explosive(s, w, tile, k, 0, 1))
            if r > 0:
                w.ext("w_3c16c", s, tile, 1)
                fx_cancel(w, s16(tile))
                if spotted:
                    _mines_down(w)
            else:
                if r == -1:
                    w.s8(G_GROUND_ITEMS + 12 * k + 6, 0xFF)
                    practise = False
                if spotted:
                    j = item_index(w, tile)
                    if j < 0xFF:
                        _neg_cond(w, j)
                    else:
                        _mines_down(w)
                        fx_cancel(w, s16(tile))
        elif not _no_pickup(w, k) and s[0x17A] < 8:
            w.ext("stat_practice", s, 4, 1, 0)
            w.s8(G_GROUND_ITEMS + 12 * k + 6, 1)
            w.ext("quote_event", s[0x17A], 0xFD, 4, NO_TILE, 1)
    elif not locked and not (flag & 0xFF and flag & 0xFF < 0xFE):
        w.ext("quote_event", s[0x17A], 0xFC, 4, NO_TILE, 0)
    finish()


def take_ground_item(w, tile, keep):
    """0x3C05C (EAX = tile, DL = to the cursor): nothing off the map (0..0xC40) or without a ground item (tile byte
    6 = 0xFF). Else a 0x86 tile marker is cleared with the move codes rebuilt (0x19D50 -> 0x2E730) and the tile leaves
    the blast list. To the cursor: 0x53660 puts back what the cursor holds, the 12-byte ground entry is copied to
    0x222B50, and its byte 2 becomes 100 unless the item type keeps it (g_itemTable + 0xA) or is 0x6F / 0x57 / 0x58.
    Not to the cursor, a listed type (0x3FC04) stays where it is. Then tile byte 6 = 0xFF and, for entries below
    200, the first 7 bytes of the entry are cleared (bytes 7..11 stay)."""
    tile = _i32(tile)
    if not 0 <= tile < 0xC41 or w.g8(G_TILEMAP + 8 * tile + 6) == 0xFF:
        return
    gi = w.g8(G_TILEMAP + 8 * tile + 6)
    if w.g8(G_TILEMAP + 8 * tile + 1) == 0x86:
        w.s8(G_TILEMAP + 8 * tile + 1, 0)
        rebuild_move_codes(w)
    blast_list_remove(w, tile)
    ent = G_GROUND_ITEMS + 12 * gi
    if keep & 0xFF:
        w.ext("w_53660")
        for k in range(12):
            w.s8(G_HELD + k, w.g8(ent + k))
        typ = w.g8(G_HELD)
        if not w.g8(G_ITEM_KEEPS + 12 * typ) and typ not in (0x6F, 0x57, 0x58):
            w.s8(G_HELD + 2, 100)
    elif w.ext("w_3fc04", w.g8(ent)):
        return
    w.s8(G_TILEMAP + 8 * tile + 6, 0xFF)
    if gi < 200:
        for k in range(7):
            w.s8(ent + k, 0)


def take_ground_items_flood(w, tile, typ):
    """0x19EB0 (EAX = tile, DL = item type): takes the tile's ground item away (0x3C05C, not to the cursor), then
    repeats on each 4-neighbour (-56, +1, +56, -1 as 16-bit tiles, no map or row checks) whose ground item has the
    same type: a connected patch of one item type is removed."""
    take_ground_item(w, tile, 0)
    for d in TILE_STEPS4:
        t = s16((d + tile) & 0xFFFF)
        gi = w.g8(G_TILEMAP + 8 * t + 6)
        if gi < 0xFF and w.g8(G_GROUND_ITEMS + 12 * gi) == typ & 0xFF:
            take_ground_items_flood(w, t, typ & 0xFF)


def find_squad_item(w, typ, drop, who, probe):
    """0x61528 (EAX = item type, DL = drop it, BL = merc slot, 8 or more = mercs 0..7, CL = probe) -> the slot of
    the first present merc (+0x182) that carries the type (byte 0 of one of its 12 entries, compared as a dword),
    after DropItem (0x470B0) on that entry with DL. (The original follows the first slot's pointer from 0x100958 at
    0x200 steps; that table always points at g_soldiers.) None and CL: 0x222B68 gets the type and 0x51A24(0x222B68,
    0, 0, DL as signed) not 0 -> 0x63. Else -1."""
    who = who & 0xFF
    lo, hi = (0, 7) if who > 7 else (who, who)
    p, r = G_SOLDIERS + 0x200 * lo, -1
    for k in range(lo, hi + 1):
        if w.g8(p + 0x182):
            for j in range(12):
                if w.g8(p + 12 * j) == typ & 0xFFFFFFFF:
                    if drop & 0xFF:
                        w.ext("drop_item", k if j == 0 else w.ptr(p + 12 * j))   # entry 0 = the record
                    r = k
                    break
            if r > -1:
                break
        p += 0x200
    if r == -1 and probe & 0xFF:
        w.s8(G_HELD_PROBE, typ & 0xFF)
        if w.ext("w_51a24", w.ptr(G_HELD_PROBE), 0, 0, s8(drop & 0xFF) & 0xFFFFFFFF):
            r = 0x63
    return r


def pick_up(s, w, tile, cursor):
    """0x3C16C (EAX = soldier, EDX = tile, BL = cursor): the ground item goes to the cursor (0x3C05C); with BL the
    cursor item goes to the soldier (0x54EC8). For the four story items (type held before 0x54EC8) 0x79 / 0x86 /
    0x5F / 0x6E: if the cursor now holds another type, a squad search (0x61528, all mercs) must find the type; then,
    once per game (flag 0x22A05A / B / D / C), the soldier's quote 0x12 / 0x15 / 0x1B / 0x1F (0x1B2A4)."""
    take_ground_item(w, tile, 1)
    typ = w.g8(G_HELD)
    if cursor & 0xFF:
        w.ext("w_54ec8", s)
    if typ not in PICKUP_QUOTES:
        return
    flag, quote = PICKUP_QUOTES[typ]
    if w.g8(G_HELD) != typ and find_squad_item(w, typ, 0, 0x40, 0) == -1:
        return
    if w.g8(flag):
        return
    w.ext("quote_event", s[0x17A], quote, 0, NO_TILE, 1)
    w.s8(flag, 1)


G_SECTORS = 0x222B74           # 0xB1 B per sector; starts with 12 (tile, value) word pairs (doors, story tiles)


def door_lookup(w, tile):
    """SectorDoorLookup? 0x3AFE0 (EAX = tile) -> the value of the first of the current sector's 12 (tile, value)
    word pairs whose tile equals it (signed words), else 0."""
    b = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR)
    for i in range(12):
        if s16(w.g16(b + 4 * i)) == _i32(tile):
            return s16(w.g16(b + 4 * i + 2))
    return 0


def lock_lookup(w, tile):
    """SectorLockLookup? 0x3B430 (EAX = tile): as door_lookup over the 12 word pairs at +0x30 of the current sector's
    record (the block holds 10; pairs 10 and 11 are the first two tile triggers at +0x58)."""
    b = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR) + 0x30
    for i in range(12):
        if s16(w.g16(b + 4 * i)) == _i32(tile):
            return s16(w.g16(b + 4 * i + 2))
    return 0


G_T0CLASS, G_T1CLASS = 0x284E58, 0x108158   # u8 per ground / object byte (MoveCode)
# 0x2E730 door pass: object -> (move code of the tile itself or None, directions whose neighbour becomes a door code)
DOOR_SIDES = {0x1B: (None, (7, 8, 6)), 0x1D: (None, (3, 2, 4)), 0x1E: (None, (1, 8, 2)), 0x20: (None, (5, 6, 4)),
              0x3B: (0x6338, (1, 8, 2, 5, 6)), 0x5A: (0x6338, (1, 8, 2, 5, 6)),
              0x3D: (None, (1, 8, 2, 5, 6)), 0x3E: (None, (1, 8, 2, 5, 6)),
              0x5C: (None, (1, 8, 2, 5, 6)), 0x5D: (None, (1, 8, 2, 5, 6)),
              0x41: (0x6338, (7, 8, 6, 3, 2)), 0x60: (0x6338, (7, 8, 6, 3, 2)),
              0x43: (None, (7, 8, 6, 3, 2)), 0x45: (None, (7, 8, 6, 3, 2)),
              0x62: (None, (7, 8, 6, 3, 2)), 0x64: (None, (7, 8, 6, 3, 2))}


def move_code_at(w, t):
    """MoveCode 0x2BE54 (EAX tile): the tile's move code rebuilt from its layers, its old code as the base."""
    from combat import move_code
    w.s16(G_MOVECODE + 2 * t, move_code(w.tile8(t, 0), w.tile8(t, 1), w.tile8(t, 3), w.g16(G_MOVECODE + 2 * t),
                                        [w.g8(G_T0CLASS + i) for i in range(256)], [w.g8(G_T1CLASS + i) for i in range(256)]))


def rebuild_move_codes(w):
    """0x2E730: every move code from scratch (cleared, then MoveCode 0x2BE54 per tile with old code 0), then a door
    pass in tile order: open doors 0x3B / 0x5A / 0x41 / 0x60 set their own tile to 0x6338, and each door object marks
    its listed neighbours (DirStep, StepTile) with the door code 0x62D4 unless that neighbour holds 0x639C (wall),
    0x4B1 or 0x5DC."""
    from combat import move_code
    t0c = [w.g8(G_T0CLASS + i) for i in range(256)]
    t1c = [w.g8(G_T1CLASS + i) for i in range(256)]
    for t in range(3136):
        w.s16(G_MOVECODE + 2 * t, move_code(w.tile8(t, 0), w.tile8(t, 1), w.tile8(t, 3), 0, t0c, t1c))
    for t in range(3136):
        side = DOOR_SIDES.get(w.tile8(t, 1))
        if side is None:
            continue
        if side[0] is not None:
            w.s16(G_MOVECODE + 2 * t, side[0])
        for d in side[1]:
            nb = step_tile(t, dir_step(w, d))
            if w.g16(G_MOVECODE + 2 * nb) not in (0x639C, 0x4B1, 0x5DC):
                w.s16(G_MOVECODE + 2 * nb, 0x62D4)


def clear_marker86(w, tile):
    """ClearTileMarker86? 0x19D50 (EAX = tile): a 0x86 object (tile byte 1, the blue flag) becomes 0 and the move
    codes are rebuilt (0x2E730)."""
    if w.g8(G_TILEMAP + 8 * tile + 1) == 0x86:
        w.s8(G_TILEMAP + 8 * tile + 1, 0)
        rebuild_move_codes(w)


def disarm_explosive(s, w, tile, k, armed, menu):
    """DisarmExplosivePrompt 0x3B9D8 (EAX = soldier, EDX = tile, BL = ground item entry, CL = armed, stack byte = from
    the menu) -> 1 handled, 0 cancelled, -1 set off (TriggerItem(tile, entry, 1)). Only an armed explosive counts
    (condition below -100 with CL, below 0 without); anything else returns 1. A tile in the blast list (or any non-merc
    user) is a known explosive: a merc is asked "Attempt to disarm explosive?"; no clears its pending action (+0x134,
    +0x96 none, +0x1A6, +0x1A7, +0x95 = 0) and on a blue flag (object 0x86) asks "Remove blue flag?" (yes: 0x19D50),
    0. An unknown one is spotted when explosives / 30 + level reaches its strength (|condition|, above 100 less 100):
    it joins the blast list, the merc practises stats 4 and 6 by the strength, says message 0x13 and is asked the
    same (no: pending action cleared, 0); a blue flag counts as spotted. From the menu 0x1CB78(0x92DD4) refreshes it.
    A spotted explosive under a merc is then disarmed when Random(100) is below explosives x 70 / 100 + dexterity x
    20 / 100 + level - (100 - wisdom) / 5 (practice 6 by 10), else it goes off (practice 6 by 5). Disarmed by type:
    0x11 (mine) condition Random(9) + 1, +6 = 1, 0x53708; 0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D condition negated;
    0x13, 0x2C, 0x72 become item 0x40 and 0x29, 0x3F, 0x71 item 0x21 (condition Random(10) + 1, count 1, +6 = 1,
    0x53708, 0x19D50); others condition Random(31) + 70. A count above 1 of an item whose table byte +6 is above 1
    then gives each further unit a condition of the item's + Random(10) - 5 in 1..100 (bytes +8 on)."""
    k &= 0xFF
    g = G_GROUND_ITEMS + 12 * k
    cond = s8(w.g8(g + 2))
    if not (cond < -100 and armed & 0xFF or cond < 0 and not armed & 0xFF):
        return 1
    merc = s[0x17A] < 8
    menu &= 0xFF

    def ask(text):
        return w.ext("yes_no", text, s[0x179], 0, 1, 0) & 0xFF

    def cancel():
        w16(s, 0x134, NO_TILE)
        w16(s, 0x96, NO_TILE)
        s[0x1A6] = s[0x1A7] = s[0x95] = 0

    def refresh():
        if menu:
            w.ext("w_1cb78", w.g8(0x92DD4))

    known = any(s16(w.g16(G_BLAST_LIST + 2 * i)) == _i32(tile) for i in range(30)) if merc else True
    spotted = known
    if known:
        if merc:
            if not ask("Attempt to disarm explosive?"):
                cancel()
                if w.tile8(tile, 1) == 0x86 and ask("Remove blue flag?"):
                    clear_marker86(w, tile)
                return 0
            refresh()
    else:
        strength = abs(cond)
        if strength > 100:
            strength -= 100
        if w.tile8(tile, 1) == 0x86:
            spotted = True
        if cdiv(s8(s[0x18F]), 30) + s8(s[0x192]) >= strength:
            blast_list_add(w, tile)
            spotted = True
            w.ext("stat_practice", s, 4, strength, 0)
            w.ext("stat_practice", s, 6, strength, 0)
            w.ext("merc_message", s, 0x13, 0x40, 0x90914, s8(menu) & 0xFFFFFFFF)
            refresh()
            if not ask("Attempt to disarm explosive?"):
                cancel()
                return 0
            refresh()
    if not spotted or not merc:
        w.ext("w_1ac68", s, tile, k, 1)
        return -1
    chance = (cdiv(s8(s[0x18F]) * 70, 100) + cdiv(s8(s[0x18C]) * 20, 100) + s8(s[0x192])
              - cdiv(100 - s8(s[0x18D]), 5))
    if _i32(w.random(100)) >= chance:
        w.ext("stat_practice", s, 6, 5, 0)
        w.ext("w_1ac68", s, tile, k, 1)
        return -1
    w.ext("stat_practice", s, 6, 10, 0)
    typ = w.g8(g)
    if typ == 0x11:
        r = w.random(9)
        w.s8(g + 6, 1)
        w.s8(g + 2, (r + 1) & 0xFF)
        w.ext("w_53708", w.ptr(g), 0, 0)
    elif typ in (0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D):
        w.s8(g + 2, -w.g8(g + 2) & 0xFF)
    elif typ in (0x13, 0x2C, 0x72, 0x29, 0x3F, 0x71):
        w.s8(g, 0x40 if typ in (0x13, 0x2C, 0x72) else 0x21)
        w.s8(g + 2, (w.random(10) + 1) & 0xFF)
        w.s8(g + 1, 1)
        w.s8(g + 6, 1)
        w.ext("w_53708", w.ptr(g), 0, 0)
        clear_marker86(w, tile)
    else:
        w.s8(g + 2, (w.random(0x1F) + 0x46) & 0xFF)
    if s8(w.g8(g + 1)) > 1 and w.g8(ITEM_TBL + 6 + 12 * w.g8(g)) > 1:
        j = 0
        while j < s8(w.g8(g + 1)) - 1:
            c = (w.g8(g + 2) + w.random(10) - 5) & 0xFF
            w.s8(g + 8 + j, c if s8(c) > 0 else 1)
            if s8(w.g8(g + 8 + j)) > 100:
                w.s8(g + 8 + j, 100)
            j += 1
    return 1


def use_crowbar(s, w, tile, flag):
    """UseCrowbar? 0x3B474 (EAX = soldier, EDX = tile, BL = flag) -> 1 opened, 0 not. The lock strength v is the door
    value x 2 on a tile nobody may stand on, else the lock value (SectorLockLookup?). v 0: SpendAP(0, fatigue 50), 1.
    Force f = strength (+0x18A) x ((hp + (missing hp - wounds when hp < hpMax) / 2) / 2) / 100 (signed, truncated).
    With flag 0x76 and a crowbar (item 0x76) of condition 10 or more in hand: f + f x 50 / 100; the chance the bar
    breaks = min(f, v) as a byte minus its condition (not below 0): Random(100) below it breaks the bar (condition
    Random(9) + 1, focus, "BAD" cry, sample 0x2B11, "Crowbar breaks!", f back to the unaided force / 2, no bonus);
    otherwise a min(f, v) byte of 20 or more wears it by Random(that / 10). The selected merc's item panel refreshes.
    SpendAP(0, fatigue 50 x (f when below v, else v x v (32-bit, wraps) / f with the bar, else v) as a byte). f >= v: StatPractice
    (stat 1, v / 10), 1."""
    flag &= 0xFF
    v = _i32(w.ext("door_lookup", tile)) * 2 if _i32(tile_standable(w, tile)) < 0 else lock_lookup(w, tile)
    if v == 0:
        w.ext("spend_ap", s, 0, 0x32)
        return 1
    hp = s8(s[0x183])
    missing = s8(s[0x184]) - hp - s8(s[0x185]) if hp < s8(s[0x184]) else 0
    f = cdiv(s8(s[0x18A]) * cdiv(hp + cdiv(missing, 2), 2), 100)
    bar = flag == 0x76 and s[0] == 0x76 and s8(s[2]) >= 10
    if bar:
        bonus = cdiv(f * 50, 100)
        f += bonus
        m = (f if f < v else v) & 0xFF
        risk = max(s8((m - s[2]) & 0xFF), 0)
        if risk and _i32(w.random(100)) < risk:
            bar = False
            f = cdiv(f - bonus, 2)
            s[2] = (w.random(9) + 1) & 0xFF
            w.ext("focus", s[0x17A], 1)
            w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
            w.ext("voc", 0x2B11, 0x32, 1)
            w.ext("message", "Crowbar breaks!")
        elif m >= 0x14:
            s[2] = (s[2] - w.random(m // 10)) & 0xFF
        if s[0x17A] == w.g8(0x284F11):
            w.ext("item_panel", s, 0, 1)
    a = f if f < v else cdiv(_i32(v * v), f) if bar else v       # v x v wraps at 32 bits
    w.ext("spend_ap", s, 0, s16((a & 0xFF) * 0x32 & 0xFFFF))
    if v <= f:
        w.ext("stat_practice", s, 1, s16(cdiv(v, 10) & 0xFFFF), 0)
        return 1
    return 0


def use_key(s, w, big, v):
    """0x3AE78 (EAX = soldier, DL = big, BX = door value) -> 1 when a key fits. Key item 0x57 (0x58 for a big door,
    DL != 0); entries in pocket order (POCKET_SLOTS: hand, then 7..11, 1). An entry holds byte 1 = number of key ids,
    id 0 at byte 2, ids 1..4 at bytes 8..11. The first id equal to v (signed byte vs the 16-bit value) is used up:
    id 0: the last id moves into byte 2 (count above 1), else the entry is dropped (DropItem); id k >= 1: below 4 the
    ids after it move down and byte 0x0B OF THE SOLDIER (the hand entry, not the key's) becomes 0 (original bug),
    id 4: byte 11 = 0; the count drops. The selected merc's item panel shows the entry (item_cond).
    Diff-tested (ai_diff door_fx, DropItem real) 2026-10-22; the hand-entry bug is issue #2 row 9."""
    want = 0x57 + (big & 0xFF != 0)
    v = s16(v & 0xFFFF)
    for i in range(7):
        j = s8(w.g8(POCKET_SLOTS + i))
        b = 12 * j
        if s[b] != want:
            continue
        k = 0
        while k < s8(s[b + 1]):
            c = s8(s[b + 1])
            if k == 0:
                if s8(s[b + 2]) == v:
                    if c > 1:
                        s[b + 2] = s[b + c + 6]
                        s[b + s8(s[b + 1]) + 6] = 0
                        s[b + 1] = (s[b + 1] - 1) & 0xFF
                    else:
                        s[b:b + 12] = bytes(12)
                    if w.g8(G_SELECTED) == s[0x17A]:
                        w.ext("item_cond", i, s, 0, s8(w.g8(0x92E36)))
                    return 1
            elif s8(s[b + k + 7]) == v:
                if k < 4:
                    for m in range(k - 1, 3):
                        s[b + m + 8] = s[b + m + 9]
                    s[0x0B] = 0
                else:
                    s[b + k + 7] = 0
                s[b + 1] = (s[b + 1] - 1) & 0xFF
                if w.g8(G_SELECTED) == s[0x17A]:
                    w.ext("item_cond", i, s, 0, s8(w.g8(0x92E36)))
                return 1
            k += 1
    return 0


def reload_gun(s, w):
    """MsgReloaded? 0x40094 (EAX = soldier; host op 190 "reloaded") -> 0 = reloaded, 1 = no clip. The gun in hand
    (type t = byte 0) takes ammo item WEAPON_TBL[t] +1 (0x94C32 + 6t); the first entry in pocket order (POCKET_SLOTS,
    the hand first) of that type is the clip (0x40068). Byte 1 (rounds) += item table byte 0x94DFD + 12t (the gun's
    clip size); +0x1D5 > 0: StatPractice (s, 3, 1, 0); sfx (0xF for t < 8 else 0xE, 0x2B11, 0x32, 1); one clip
    leaves the pocket entry (0x56090: count above 1 zeroes byte count + 6; the count drops; 0 drops the entry); the
    selected merc's panel shows entry 0 and the pocket (item_cond); option 0x1163BD: merc_text "has reloaded." 800."""
    t = s[0]
    ammo = w.g8(WEAPON_TBL + 1 + 6 * t)
    pk = -1
    for i in range(7):
        if s[12 * w.g8(POCKET_SLOTS + i)] == ammo:
            pk = i
            break
    if pk == -1:
        return 1
    s[1] = (s[1] + w.g8(0x94DFD + 12 * t)) & 0xFF
    if s8(s[0x1D5]) > 0:
        w.ext("stat_practice", s, 3, 1, 0)
    w.ext("sfx", 0xF if t < 8 else 0xE, 0x2B11, 0x32, 1)
    b = 12 * s8(w.g8(POCKET_SLOTS + pk))
    c = s8(s[b + 1])
    if c > 1:
        s[b + c + 6] = 0
    s[b + 1] = (s[b + 1] - 1) & 0xFF
    if s[b + 1] == 0:
        s[b:b + 12] = bytes(12)
    if w.g8(G_SELECTED) == s[0x17A]:
        w.ext("item_cond", 0, s, 0, s8(w.g8(0x92E36)))
        w.ext("item_cond", pk, s, 0, s8(w.g8(0x92E36)))
    if w.g8(0x1163BD):
        w.ext("merc_text", s, "has reloaded.", 800)
    return 0


def door_fx(s, w, tile, task):
    """PlayDoorFx? 0x3B03C (EAX = soldier, EDX = tile, BL = task; host op 70 door_fx): a soldier works the door or
    window on the tile. v = the door value (door_lookup); above 100: a big one (v - 100). Result r: v 0 -> 11 (open);
    else with the hand item it (none for task 0xFF): lock pick 0x4C: skill = (wisdom +0x18D x mechanical +0x190 /
    100) x dexterity +0x18C / 100 + level +0x192 x 5 (signed bytes); condition (s8 +2) x skill / 100 >= v -> 10 (picked; StatPractice 7/2, 4/2,
    3/1), else Random(100) < v wears the pick by 1 (condition not 0), 7; other items: a key that fits (use_key) -> 9,
    else 6 with a key in hand (0x57 / 0x58), 8 without. UI: door_snd 0 (stop sounds, load the door sounds) first, door_snd 1 (stop,
    overhead sounds) last; FX animations (op door_anim: big, r, phase 0 the try,
    1 the outcome; no outcome for 8 on a big one). r >= 9: BreakObject (tile, its object, sound 1), MakeNoise (slot,
    tile, +0x1B3, Random(3), kind 2), +0x9D = 1. The selected merc's panel shows the hand item (item_cond), not
    after 8 on a big one. Diff-tested 2026-10-22 (ai_diff door_fx: 4000 cases, 163/167 mutants; the survivors are
    equivalent: task masking, `it` for task 0xFF, the /101 boundary at value 100)."""
    task &= 0xFF
    obj = w.tile8(s16(tile), 1)
    w.ext("door_snd", 0)
    v = s16(door_lookup(w, tile) & 0xFFFF)
    big = v > 100
    if big:
        v = s16((v - 100) & 0xFFFF)
    if v == 0:
        r = 11
    else:
        it = 0 if task == 0xFF else s[0]
        if it == 0x4C:
            skill = cdiv(cdiv(s8(s[0x18D]) * s8(s[0x190]), 100) * s8(s[0x18C]), 100) + s8(s[0x192]) * 5
            if cdiv(s8(s[2]) * skill, 100) >= v:
                r = 10
                w.ext("stat_practice", s, 7, 2, 0)
                w.ext("stat_practice", s, 4, 2, 0)
                w.ext("stat_practice", s, 3, 1, 0)
            else:
                if _i32(w.random(100)) < v and s[2]:
                    s[2] = (s[2] - 1) & 0xFF
                r = 7
        elif use_key(s, w, big, v):
            r = 9
        else:
            r = 6 if it in (0x57, 0x58) else 8
    w.ext("door_anim", int(big), r, 0)
    if r == 8 and big:
        w.ext("door_snd", 1)
        return
    if r >= 9:
        w.ext("break_obj", s16(tile), obj, 1)
        loud = w.random(3)
        w.ext("make_noise", s[0x17A], s16(tile), s[0x1B3], loud, 2)
        s[0x9D] = 1
    w.ext("door_anim", int(big), r, 1)
    if w.g8(G_SELECTED) == s[0x17A]:
        w.ext("item_cond", 0, s, 0, s8(w.g8(0x92E36)))
    w.ext("door_snd", 1)


def story_hit(w):
    """0x61D28 (a hit on a story object): 0x22A055 = 1. For the current sector's 5 story slots (record bytes +0x9F..
    +0xA3 with tiles at the words +0x82..+0x8A): 0xAE becomes 0xAC and 0xAF becomes 0xAD, and a slot holding 0xAC /
    0xAD puts that object on its tile (byte 1). Then the mission step becomes RadioResult? (0x63254) and the mission
    order 0x40."""
    w.s8(0x22A055, 1)
    rec = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR)
    for i in range(5):
        tile = s16(w.g16(rec + 0x82 + 2 * i))
        for hit, broken in ((0xAE, 0xAC), (0xAF, 0xAD)):
            if w.g8(rec + 0x9F + i) == hit:
                w.s8(rec + 0x9F + i, broken)
            if w.g8(rec + 0x9F + i) in (hit, broken):
                w.s8(G_TILEMAP + 8 * tile + 1, broken)
    w.s8(G_MISSION_STEP, w.ext("radio_result"))
    w.s8(G_MISSION_ORDER, 0x40)


def break_object(w, tile, obj, sound):
    """0x3ACB8 (EAX = tile, DL = its object, BL = sound): doors and windows change to their open / broken object.
    Objects 0x3E / 0x5D lose 3 and 0x3D / 0x5C lose 2, and the object of the tile to the west (tile - 1) loses 2;
    0x45 / 0x64 lose 4 and 0x43 / 0x62 lose 2, and the tile to the north (tile - 56) loses 2 with byte 2 = 0x45 if
    it is now 0x42, else 0x4E. Both set the move code to 0x6338. Other objects: nothing. With BL the sound 0x18
    plays (0x5CB34)."""
    obj &= 0xFF
    m = G_TILEMAP + 8 * tile
    if obj in (0x3E, 0x5D, 0x3D, 0x5C):
        w.s8(m + 1, w.g8(m + 1) - (3 if obj in (0x3E, 0x5D) else 2))
        w.s16(G_MOVECODE + 2 * tile, 0x6338)
        w.s8(m - 7, w.g8(m - 7) - 2)
    elif obj in (0x45, 0x64, 0x43, 0x62):
        w.s8(m + 1, w.g8(m + 1) - (4 if obj in (0x45, 0x64) else 2))
        n = m - 8 * 56
        v = (w.g8(n + 1) - 2) & 0xFF
        w.s16(G_MOVECODE + 2 * tile, 0x6338)
        w.s8(n + 1, v)
        w.s8(n + 2, 0x45 if v == 0x42 else 0x4E)
    else:
        return
    if sound & 0xFF:
        w.ext("sfx", 0x18, 0x2B11, 0x32, 1)


def close_door(w, tile, obj, sound):
    """0x3ADB4 (EAX = tile, DL = its object, BL = sound): nothing while a soldier stands on the tile (TileOccupant
    below 0x30). Doors 0x3B / 0x5A gain 3 and the object to the west (tile - 1) gains 2; doors 0x41 / 0x60 gain 2
    and the object to the north (tile - 56) gains 2 with byte 2 = 0. Both set the move code to 0x5DC. Other
    objects: nothing. With BL the sound 0x18 plays."""
    if w.tile8(tile, 5) & 0x7F < 0x30:
        return
    obj &= 0xFF
    m = G_TILEMAP + 8 * tile
    if obj in (0x3B, 0x5A):
        a, b = w.g8(m + 1) + 3, w.g8(m - 7) + 2
        w.s8(m + 1, a)
        w.s16(G_MOVECODE + 2 * tile, 0x5DC)
        w.s8(m - 7, b)
    elif obj in (0x41, 0x60):
        n = m - 8 * 56
        v, a = w.g8(n + 1) + 2, w.g8(m + 1) + 2
        w.s8(n + 1, v)
        w.s8(n + 2, 0)
        w.s8(m + 1, a)
        w.s16(G_MOVECODE + 2 * tile, 0x5DC)
    else:
        return
    if sound & 0xFF:
        w.ext("sfx", 0x18, 0x2B11, 0x32, 1)


def toggle_doors(w, mode, door):
    """0x35774 (AL = mode, DL = door id): over all 0xC40 map tiles whose object is 0x3B..0x64 and whose door lookup
    (0x3AFE0) equals the id: mode 1 opens those that cannot be stood on (TileStandable < 0) without sound (0x3ACB8);
    any other mode closes those with no occupant (byte 5 = 0x40) with sound (0x3ADB4)."""
    for t in range(0xC40):
        obj = w.tile8(t, 1)
        if not 0x3B <= obj <= 0x64:
            continue
        if mode & 0xFF == 1:
            if _i32(tile_standable(w, t)) < 0 and _i32(w.ext("door_lookup", t)) == door & 0xFF:
                w.ext("break_obj", t, obj, 0, t)
        elif _i32(w.ext("door_lookup", t)) == door & 0xFF and w.tile8(t, 5) == 0x40:
            w.ext("close_door", t, obj, 1)


HOME_SECTOR = 0x3B
HOME_STEPS = (1, 10, -1, -10)          # the dword 0xF6FF0A01 at 0x410C0: east, south, west, north


def connected_home(w, tbl, sec, frame=None):
    """SectorConnectedToHome 0x41178 (EAX = 9-byte sector table, DL = sector) -> 1 if a walk over owned sectors
    reaches home 0x3B, else 0. Home itself is 1, an unowned start 0. A local frame holds a junction stack (0x00),
    a done flag per sector (0x3C) and a copy of every b0 (0x78) whose bits 4/8/0x10/2 mark the sides already tried.
    Each round tries the side d of the current sector (0 E, 1 S, 2 W, 3 N): an untried side is marked and taken; a
    sector whose copied b0 is 0x1E or more (all sides tried, or high bits set) is flagged done on the first visit
    and on a later visit pops the stack (top 0 with the start done -> 0; top 0 -> back to the start). Without a
    move d advances (wrapping after 3). A move skips the map edge column, sectors 60 and up (byte wrap), unowned
    sectors and done sectors (the position still moves onto a done sector); on a new sector it marks the side it
    came from and pushes the sector (d = 0) when more than 2 of its 4 neighbours are owned in the real table,
    read without edge or range checks. A test may pass a zeroed 0xB4-byte frame to read the walk state back.
    A sector 60 or above (the office screen keeps 0xFF and the like) makes the original read its caller's stack: the
    result is undefined there, and the port answers 0 (not on the map, so not connected)."""
    s = sec & 0xFF
    if s == HOME_SECTOR:
        return 1
    if s >= 60:
        return 0
    f = bytearray(0xB4) if frame is None else frame    # the frame up to the step table (a deep stack hits the flags)
    for i in range(60):
        f[0x78 + i] = w.g8(tbl + 9 * i)
    start, d, top = s, 0, 1
    f[0] = s
    if not f[0x78 + s] & 1:
        return 0
    while True:
        moved = 0
        while not moved:
            bit = (4, 8, 0x10, 2)[d]
            if not f[0x78 + s] & bit:
                f[0x78 + s] |= bit
                moved = 1
            if f[0x78 + s] >= 0x1E:
                if f[0x3C + s]:
                    top = (top - 1) & 0xFF
                    if top == 0 and f[0x3C + start]:
                        return 0
                    if top == 0:
                        s, top = start, 1
                    else:
                        s = f[top]
                else:
                    f[0x3C + s] = 1
            if not moved:
                d = (d + 1) & 0xFF
            if d > 3:
                d = 0
        n = (HOME_STEPS[d] + s) & 0xFF
        if not (s % 10 == 0 and d == 2 or s % 10 == 9 and d == 0 or n >= 60 or not f[0x78 + n] & 1):
            s = n
            if not f[0x3C + n]:
                f[0x78 + n] |= (0x10, 2, 4, 8)[d]
                if sum(w.g8(tbl + 9 * (s + k)) & 1 for k in HOME_STEPS) > 2:
                    if top >= 0xB4:
                        raise RuntimeError("connected_home: junction stack left the frame")
                    f[top] = s
                    top, d = top + 1, 0
        if s == HOME_SECTOR:
            return 1


G_TEAM_COUNT, G_TEAM_ON_DUTY, G_TREE_PRICE = 0x229FA8, 0x229FAC, 0x229FB0
G_TREES_WORKABLE, G_TREES_OWNED, G_TREES_TAPPED = 0x229FB4, 0x229FB8, 0x229FBC
G_DAY_INCOME, G_BALANCE, G_BALANCE_AFTER_DAY, G_DAY_NET = 0x229FC0, 0x229FC4, 0x229FC8, 0x229FCC
G_TEAM_HALF_SALARIES, G_TEAM_SALARIES, G_TEAM_PAY, G_WORKER_POOL = 0x229FD0, 0x229FD4, 0x229FD8, 0x229FDC
G_TAPPERS, G_GUARDS, G_WORKERS, G_IDLE_POOL = 0x229FE0, 0x229FE4, 0x229FE8, 0x229FEC
G_NATIVE_WAGE, G_TAPPERS_COST, G_GUARDS_COST, G_NATIVES_COST = 0x229FF4, 0x229FF8, 0x229FFC, 0x22A000
G_DAY_EXPENSES, G_SECTORS_OWNED, G_PROCESS_CAP, G_PLANTS_OWNED = 0x22A004, 0x22A014, 0x22A024, 0x22A028
G_PREV_BALANCE = 0x229AC0


def i32(v):
    v &= 0xFFFFFFFF
    return v - 0x100000000 if v & 0x80000000 else v


def calc_process_cap(w):
    """CalcProcessCap 0x41574: over the 4 plants {i32 capacity, i32 sector, u8 state}: plant sector byte owned ->
    g_plantsOwned + 1; and connected to home (0x41178) and state byte (+8) = 2 -> g_processCap += capacity."""
    w.s32(G_PLANTS_OWNED, 0)
    w.s32(G_PROCESS_CAP, 0)
    for k in range(4):
        p = G_PLANTS + 12 * k
        sec = w.g8(p + 4)
        if w.g8(G_SECTOR_TBL9 + 9 * sec) & 1:
            w.s32(G_PLANTS_OWNED, w.g32(G_PLANTS_OWNED) + 1)
            if w.ext("connected_home", G_SECTOR_TBL9, sec) & 0xFF and w.g8(p + 8) == 2:
                w.s32(G_PROCESS_CAP, w.g32(G_PROCESS_CAP) + w.g32(p))


def recalc_territory(w):
    """RecalcTerritory 0x50EA8: totals over the 60 sectors. Unowned: b3 tappers = b4 guards = b7 workable = 0.
    Owned: count, tappers, guards, b6 trees summed; connected to home -> g_treesWorkable += SectorWorkable (0x41E1C)
    and g_treesTapped += min(b6 - b8 (byte), b3 / 3); else b7 = 0. Then g_treesTapped capped by g_processCap
    (signed), g_workers = tappers + guards, g_idlePool = g_workerPool - workers, at least 0."""
    for a in (G_SECTORS_OWNED, G_TAPPERS, G_GUARDS, G_TREES_OWNED, G_TREES_WORKABLE, G_TREES_TAPPED):
        w.s32(a, 0)
    for sec in range(60):
        b = G_SECTOR_TBL9 + 9 * sec
        if not w.g8(b) & 1:
            w.s8(b + 7, 0)
            w.s8(b + 4, 0)
            w.s8(b + 3, 0)
            continue
        w.s32(G_SECTORS_OWNED, w.g32(G_SECTORS_OWNED) + 1)
        w.s32(G_TAPPERS, w.g32(G_TAPPERS) + w.g8(b + 3))
        w.s32(G_GUARDS, w.g32(G_GUARDS) + w.g8(b + 4))
        w.s32(G_TREES_OWNED, w.g32(G_TREES_OWNED) + w.g8(b + 6))
        if not w.ext("connected_home", G_SECTOR_TBL9, sec) & 0xFF:
            w.s8(b + 7, 0)
            continue
        w.s32(G_TREES_WORKABLE, w.g32(G_TREES_WORKABLE) + (w.ext("workable", G_SECTOR_TBL9, sec) & 0xFF))
        third, room = w.g8(b + 3) // 3, (w.g8(b + 6) - w.g8(b + 8)) & 0xFF
        w.s32(G_TREES_TAPPED, w.g32(G_TREES_TAPPED) + min(third, room))
    if i32(w.g32(G_PROCESS_CAP)) < i32(w.g32(G_TREES_TAPPED)):
        w.s32(G_TREES_TAPPED, w.g32(G_PROCESS_CAP))
    work = w.g32(G_TAPPERS) + w.g32(G_GUARDS)
    w.s32(G_WORKERS, work)
    idle = i32(w.g32(G_WORKER_POOL) - work)
    w.s32(G_IDLE_POOL, max(idle, 0))


def calc_natives_cost(w):
    """CalcNativesCost 0x5102C: tappers / guards cost = g_nativeWage x count, g_nativesCost = their sum."""
    t, g = i32(w.g32(G_NATIVE_WAGE) * w.g32(G_TAPPERS)), i32(w.g32(G_NATIVE_WAGE) * w.g32(G_GUARDS))
    w.s32(G_TAPPERS_COST, t)
    w.s32(G_GUARDS_COST, g)
    w.s32(G_NATIVES_COST, t + g)


def calc_team_pay(w):
    """CalcTeamPay 0x51068: soldiers 0..7 until one has inCurSector 0: count, sum of salaries (+0x110), sum of
    halves (s + 1) / 2, on duty (+0x17E = 0); +0x112 = the salary charged today (the half when +0x17E is 1 or 4),
    g_teamPay = the sum charged."""
    pay = full = half = n = duty = 0
    while n < 8:
        s = G_SOLDIERS + 0x200 * n
        if not w.g8(s + 0x182):
            break
        sal = w.g16(s + 0x110)
        h = (sal + 1) // 2
        full, half = full + sal, half + h
        c = h if w.g8(s + 0x17E) in (1, 4) else sal
        w.s16(s + 0x112, c)
        pay += c
        duty += w.g8(s + 0x17E) == 0
        n += 1
    w.s32(G_TEAM_PAY, pay)
    w.s32(G_TEAM_COUNT, n)
    w.s32(G_TEAM_ON_DUTY, duty)
    w.s32(G_TEAM_HALF_SALARIES, half)
    w.s32(G_TEAM_SALARIES, full)


def calc_ledger(w):
    """CalcLedger 0x5113C: income = tree price x trees tapped, expenses = natives cost + team pay, net."""
    inc = i32(w.g32(G_TREE_PRICE) * w.g32(G_TREES_TAPPED))
    exp = i32(w.g32(G_NATIVES_COST) + w.g32(G_TEAM_PAY))
    w.s32(G_DAY_INCOME, inc)
    w.s32(G_DAY_EXPENSES, exp)
    w.s32(G_DAY_NET, inc - exp)


def calc_ledger_all(w):
    """CalcLedgerAll 0x51178: plants, territory, natives cost, team pay, ledger; g_balanceAfterDay = balance + net."""
    calc_process_cap(w)
    recalc_territory(w)
    calc_natives_cost(w)
    calc_team_pay(w)
    calc_ledger(w)
    w.s32(G_BALANCE_AFTER_DAY, w.g32(G_BALANCE) + w.g32(G_DAY_NET))


def apply_day_ledger(w):
    """ApplyDayLedger 0x51210 (day end): team pay, ledger, g_balance = g_prevBalance + net."""
    calc_team_pay(w)
    calc_ledger(w)
    w.s32(G_BALANCE, w.g32(G_PREV_BALANCE) + w.g32(G_DAY_NET))


G_SECTORS_LOST, G_SECTORS_GAINED, G_NATIVE_DEATHS_TOTAL = 0x22A01C, 0x22A018, 0x229FF0
G_SECTOR_ORDER = 0x92DF0       # 61 sector indices (shuffled at new game)
POOL_BASE, POOL_LOSS = 0x95C6C, 0x95C72   # s16 per difficulty (20, 15, 10); byte per difficulty at +2k (15, 20, 25)


def gained_bonus(w):
    """0x50CF4: workers for the sectors gained, 15 each for the first 10, 14 each for the next 10, ... 1 each for
    gains 141..150 (later gains add nothing)."""
    g, v = i32(w.g32(G_SECTORS_GAINED)), 0
    for i in range(0, 150, 10):
        if i < g:
            v += min(g - i, 10) * (15 - i // 10)
    return i32(v)


def natives_quit(w, n):
    """MercsQuitMsg? 0x50D2C (AL = n): n natives quit, guards first. Over the 61 entries of g_sectorOrder, sectors
    connected to home lose guards (b4) one by one while n lasts; message "%d guard(s) quit this morning!". Then
    the same order for tappers (b3); afterwards a sector's tappers that are not a multiple of 3 lose the remainder
    too (n goes down by it, not below 0). The tapper message prints the guard count register (CL), not the tapper
    count: original bug (in Unicorn with the Python sprintf CL keeps the guard count)."""
    n &= 0xFF
    guards = tappers = 0
    if n:
        for k in range(61):
            sec = w.g8(G_SECTOR_ORDER + k)
            if not w.ext("connected_home", G_SECTOR_TBL9, sec) & 0xFF:
                continue
            b = G_SECTOR_TBL9 + 9 * sec + 4
            while w.g8(b) and n:
                w.s8(b, w.g8(b) - 1)
                n -= 1
                guards += 1
    if guards:
        quit_text(w, b"%d %s quit this morning!" % (guards, b"guard" if guards < 2 else b"guards"))
    if n:
        for k in range(61):
            sec = w.g8(G_SECTOR_ORDER + k)
            if not w.ext("connected_home", G_SECTOR_TBL9, sec) & 0xFF:
                continue
            b = G_SECTOR_TBL9 + 9 * sec + 3
            while w.g8(b) and n:
                w.s8(b, w.g8(b) - 1)
                n -= 1
                tappers = (tappers + 1) & 0xFF
            r = w.g8(b) % 3
            if r:
                w.s8(b, w.g8(b) - r)
                n = n - r if n > r else 0
                tappers = (tappers + r) & 0xFF
    if tappers:
        quit_text(w, b"%d %s quit this morning!" % (guards, b"tapper" if guards < 2 else b"tappers"))


def quit_text(w, t):
    for i, c in enumerate(t + b"\0"):     # sprintf_ into g_textBuf, then StatusMessage? 0x27D24
        w.s8(G_TEXT_BUF + i, c)
    w.ext("message", G_TEXT_BUF)


def worker_pool(w):
    """0x50BC4 (new game, each morning, strategic screen): g_workerPool from difficulty, wage, sectors gained and
    lost, two story counters and native deaths, all in 16 bits, clamped to 0..500; RecalcTerritory; when the
    hired natives exceed the pool, the excess quits (0x50D2C), g_idlePool = 0, the accounts are recalculated and
    the map redrawn (0x41C3C, 0x36010)."""
    k = s8(w.g8(G_DIFFICULTY))
    loss = w.g8(POOL_LOSS + 2 * k)
    wage = s16(w.g32(G_NATIVE_WAGE) - 35)
    lost = s16(w.g8(0x22A031) + w.g16(G_SECTORS_LOST))
    q = cdiv(i32(wage * i32(lost + i32(w.g32(G_SECTORS_GAINED)))), 5)
    v = s16(w.g16(POOL_BASE + 2 * k) + wage + gained_bonus(w) - loss * lost + q + w.g16(0x22A034) - w.g16(0x22A032)
            - w.g16(G_NATIVE_DEATHS_TOTAL))
    v = min(max(v, 0), 500)
    w.s32(G_WORKER_POOL, v)
    recalc_territory(w)
    idle = i32(w.g32(G_WORKER_POOL) - w.g32(G_WORKERS))
    w.s32(G_IDLE_POOL, idle)
    if idle >= 0:
        return
    natives_quit(w, -idle)
    w.s32(G_IDLE_POOL, 0)
    calc_ledger_all(w)
    w.ext("draw_sectors", G_SECTOR_TBL9)
    w.ext("w_36010", 0x90FAC, w.g32(0x95678), 0x140, 0, 0xE)


G_PROFILES, PROFILE_SIZE = 0xEBB40, 0x5A
G_STASH_COUNTS = 0x93DD4          # u8 per item type: records in the stash
G_SOLDIER_PTRS = 0x100958         # soldier* per team slot 0..7
G_STAT_PROGRESS = 0x22A15A        # 0x12 bytes per team slot
G_AIM_RESIGNATIONS = 0x22A040
CASH_SLOTS = 0x93D59              # 7 signed item-place bytes searched for cash (dword 0x93D56 + k, >> 24)
GREED = 0x955ED                   # byte per profile: days of pay a merc runs off with (99 = never in practice)
CASH = 0x6F                       # item type: money


def cash_value(w, rec):
    """0x25B84 (EAX = 12-byte item) -> money in it: 0 unless type 0x6F; 10 x the signed byte +2, and for a stack
    of n (signed +1) > 1 also 10 x the signed bytes +8 .. +6+n."""
    if w.g8(rec) != CASH:
        return 0
    n = s8(w.g8(rec + 1))
    v = s8(w.g8(rec + 2)) * 10
    for j in range(1, n):
        v += s8(w.g8(rec + 7 + j)) * 10
    return v


def cash_count(w, s, drop):
    """0x4458C (EAX = soldier, DL = drop): 0 when +0x182 (inCurSector) is 0; else the money over the 7 places named
    by the signed bytes at 0x93D59; with drop, each place with money goes to DropItem 0x470B0 too."""
    total = 0
    if not w.g8(s + 0x182):
        return 0
    for j in range(7):
        rec = s + 12 * s8(w.g8(CASH_SLOTS + j))
        v = cash_value(w, rec)
        if v:
            total = i32(total + v)
            if drop & 0xFF:
                w.ext("cash_drop", w.ptr(rec))
    return total


def collect_cash(w):
    """0x445DC (morning): the money of the 8 team soldiers (pointer table 0x100958, which points at g_soldiers), dropped; when the sum is above
    0 it goes into g_dayIncome, g_balance and g_dayNet."""
    total = 0
    for k in range(8):
        total = i32(total + cash_count(w, G_SOLDIERS + 0x200 * k, 1))
    if total > 0:
        for a in (G_DAY_INCOME, G_BALANCE, G_DAY_NET):
            w.s32(a, w.g32(a) + total)


def stash_item(w, rec, clear=True):
    """AddStashItem? 0x51D50 with DL = 0 (EAX = 12-byte item). Nothing for type 0. k = 1 for a type of stack class
    (item table +6) < 2, else the signed count +1; only when k + g_stashCounts[type] < 100: class < 2 -> one record
    (type, +1, +2) and, when +3 (attachment) is not 0, a second (+3, +4, +5); else one record (type, 1, cond) per
    stacked entry, cond = +2 for the first and +7 + j for entry j. Each record goes to 0x51CB0 (stash_put), which
    on success (1) counts it in g_stashCounts[record type]. On success the item's first 6 bytes are cleared; callers
    that pass a stack copy (0x47090) give clear=False: their own item stays as it was."""
    t = w.g8(rec)
    if not t:
        return
    cls = w.g8(ITEM_TBL + 12 * t + 6)
    k = 1 if cls < 2 else s8(w.g8(rec + 1))
    if k + w.g8(G_STASH_COUNTS + t) >= 100:
        return
    if cls < 2:
        puts = [(t, w.g8(rec + 1), w.g8(rec + 2))]
        if w.g8(rec + 3):
            puts.append((w.g8(rec + 3), w.g8(rec + 4), w.g8(rec + 5)))
    else:
        puts = [(t, 1, w.g8(rec + 2) if j == 0 else w.g8(rec + 7 + j)) for j in range(s8(w.g8(rec + 1)))]
    ok = 0
    for p in puts:
        ok = w.ext("stash_put", *p) & 0xFF
        if ok:
            w.s8(G_STASH_COUNTS + p[0], w.g8(G_STASH_COUNTS + p[0]) + 1)
    if ok and clear:              # the last record stored: the item's first 6 bytes are cleared
        for j in range(6):
            w.s8(rec + j, 0)


def merc_leaves(w, s, status):
    """0x50618 (EAX = soldier, DL = status): the soldier's profile (+0x179) keeps status (+0x4B), salary, health,
    agility, marksmanship, wisdom, medical, mechanical, dexterity, explosives and level; of the 12 item places,
    empty / money places give kit byte 0 (places 0..9, profile +0x50 + i); a place >= 10 or a listed type
    (0x3FC04, type or attachment +3) goes to the stash (AddStashItem, DL 0) and gives kit 0; others keep the type
    as kit. Profile +0x3A = 99 when +0x3C is not 99 and equals it. Face buffer cleared (0x46AC4, team slot +0x17A);
    nickname byte 0 = byte 0x90FAC, inCurSector 0, profile 99, +0x19D = 0."""
    p = G_PROFILES + PROFILE_SIZE * w.g8(s + 0x179)
    w.s8(p + 0x4B, status)
    w.s16(p + 0x28, w.g16(s + 0x110))
    for po, so in ((0x2A, 0x184), (0x2C, 0x18B), (0x2D, 0x191), (0x2E, 0x18D), (0x40, 0x18E), (0x41, 0x190),
                   (0x42, 0x18C), (0x43, 0x18F), (0x44, 0x192)):
        w.s8(p + po, w.g8(s + so))
    for i in range(12):
        rec = s + 12 * i
        t = w.g8(rec)
        if t and t != CASH:
            if i >= 10 or listed_item(w, t) or listed_item(w, w.g8(rec + 3)):
                stash_item(w, rec, clear=False)   # AddStashItem gets a 12-byte stack copy (0x47090)
            else:
                w.s8(p + 0x50 + i, t)
                continue
        if i < 10:
            w.s8(p + 0x50 + i, 0)
    if w.g8(p + 0x3C) != 0x63 and w.g8(p + 0x3C) == w.g8(p + 0x3A):
        w.s8(p + 0x3A, 0x63)
    w.ext("face_clear", w.g8(s + 0x17A))
    w.s8(s + 0x16E, w.g8(0x90FAC))
    w.s8(s + 0x182, 0)
    w.s8(s + 0x179, 0x63)
    w.s8(s + 0x19D, 0)



def store_all(w, s):
    """0x52150 with DL = 0 (key E store, no redraw): places 11..0 with a type go to the stash (a copy), then their
    bytes 0..5 are cleared (also when nothing was stored)."""
    for i in range(11, -1, -1):
        rec = s + 12 * i
        if w.g8(rec):
            stash_item(w, rec, clear=False)
            for j in range(6):
                w.s8(rec + j, 0)


def fire_grudge(w, p, n):
    """0x50B50 (AL = p, DL = n): n tries u = Random(60); profile u +0x4C = p when u is not on the team
    (TeamHasProfile? 0x50AC0), u != p, u.+0x3C != p, u.+0x4C = 99, u.+0x2F != 3 and u != 7."""
    for _ in range(n):
        u = w.random(0x3C) & 0xFF
        on_team = any(w.g8(G_SOLDIERS + 0x200 * j + 0x182) and w.g8(G_SOLDIERS + 0x200 * j + 0x179) == u
                      for j in range(8))
        q = G_PROFILES + PROFILE_SIZE * u
        if (not on_team and u != p and w.g8(q + 0x3C) != p and w.g8(q + 0x4C) == 0x63 and w.g8(q + 0x2F) != 3
                and u != 7):
            w.s8(q + 0x4C, p)


def fire_merc(w, k, x, y):
    """0x5097C (AL = slot k, EDX = x, EBX = y): Fire the merc (strategic.md "Firing a merc")."""
    import morning
    s = G_SOLDIERS + 0x200 * (k & 0xFF)
    p = w.g8(s + 0x179)
    if p >= 0x3C:
        store_all(w, s)
        morning.native_leaves(w, s)
        return
    if w.g8(s + 0x19D):
        merc_leaves(w, s, 0)
        return
    w.s8(G_PROFILES + PROFILE_SIZE * p + 0x3D, 1)
    w.s8(0x92F8B, 1)
    w.ext("quote", p, 14 * p, 9, 200, 0x2299BC, 0, p, 1)
    w.ext("balloon", k & 0xFF, x, y, 5)
    if w.g8(0x1163BD):
        w.ext("w_28698", 0x25)
    d = i32(w.g32(G_DAY) - s8(w.g8(s + 0x1E5)))
    if d > 0:
        if w.random(s16(d)) == 0:
            fire_grudge(w, p, (w.random(3) + 1) & 0xFF)
    else:
        fire_grudge(w, p, 4)
    w.s8(0x22A03C, w.g8(0x22A03C) + 1)
    store_all(w, s)
    merc_leaves(w, s, 1)


def fire_yes(w, s, x):
    """AssignScreen? 0x4904A..0x49095 (EBP = soldier, [esp + 0x98] = x): the hire-day refund, then 0x5097C(slot
    +0x17A, x, 0x14)."""
    if i32(w.g32(G_DAY) - s8(w.g8(s + 0x1E5))) == 0:
        w.s16(G_PROFILES + PROFILE_SIZE * w.g8(s + 0x179) + 0x3E, w.g16(s + 0x112))
        w.s8(0x22A041, w.g8(0x22A041) + 1)
    fire_merc(w, w.g8(s + 0x17A), x, 0x14)

def merc_join(w, p):
    """0x21FC0 (AL = profile p): k = TeamSize 0x513B8, 0x21B74 fills soldier record k from profile p, then
    g_facesLoaded = 0 and LoadMercFace (k, p) (the harness logs "face"). Returns k (strategic.md "A merc joins the
    team"; k = 8: -1, the record is not written)."""
    p &= 0xFF
    k = 0
    while k < 8 and w.g8(G_SOLDIERS + 0x200 * k + 0x182):
        k += 1
    if k >= 8:
        return -1
    s, pr = G_SOLDIERS + 0x200 * k, G_PROFILES + PROFILE_SIZE * p
    j = 0
    while True:
        c = w.g8(pr + j)
        w.s8(s + 0x16E + j, c)
        if c == 0:
            break
        j += 1
    w.s8(s + 0x17A, k)
    w.s8(s + 0x179, p)
    for o in (0x16A, 0x16B, 0x16C, 0x194):
        w.s8(s + o, 0)
    w.s8(s + 0x182, 1)
    w.s16(s + 0x112, w.g16(pr + 0x28))
    w.s16(s + 0x110, w.g16(pr + 0x28))
    w.s8(s + 0x183, w.g8(pr + 0x2A))
    w.s8(s + 0x184, w.g8(s + 0x183))
    for so, po in ((0x18A, 0x2B), (0x18B, 0x2C), (0x18C, 0x42), (0x18D, 0x2E), (0x18E, 0x40), (0x18F, 0x43),
                   (0x190, 0x41), (0x191, 0x2D), (0x192, 0x44), (0x19B, 0x4A), (0x19C, 0x2F), (0x17C, 0x49),
                   (0x17B, 0x48)):
        w.s8(s + so, w.g8(pr + po))
    w.s8(s + 0x1DA, 0x40)
    for o in (0x1D9, 0x1B1, 0x17E, 0x185, 0x188, 0x19A):
        w.s8(s + o, 0)
    w.s16(s + 0x10C, 0)
    w.s8(s + 0x18A, 100)
    row = G_STAT_PROGRESS + 0x12 * k                      # 0x21DB8: practice start
    w.s16(row + 0x10, 0)
    for i, f in enumerate((0x184, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191)):
        st = w.g8(s + 0x19B)
        if st == 0:
            w.s16(row + 2 * i, 0)
        elif st == 1:
            if w.g8(s + f) >= 100:
                w.s16(row + 2 * i, 0)
            else:
                r = w.random(100)
                w.s16(row + 2 * i, r & 0xFFFF)
                if s8(w.g8(s + 0x192)) < 10:
                    w.s16(row + 0x10, (w.g16(row + 0x10) + r) & 0xFFFF)
        elif st == 2:
            if w.g8(s + f) == 0:
                w.s16(row + 2 * i, 0)
            else:
                r = w.random(100)
                w.s16(row + 2 * i, -r & 0xFFFF)
                if s8(w.g8(s + 0x192)) > 1:
                    w.s16(row + 0x10, (w.g16(row + 0x10) - r) & 0xFFFF)
        else:
            break
    for i in range(12):
        e = s + 12 * i
        t = w.g8(pr + 0x50 + i) if i < 10 else 0
        w.s8(e, t)
        if t == 0:
            w.s8(e + 1, 0)
            w.s8(e + 2, 0)
        else:
            w.s8(e + 1, w.g8(ITEM_TBL + 12 * t + 5) if w.g8(ITEM_TBL + 12 * t + 6) == 1 else 1)
            w.s8(e + 2, 20 if t == 0x6F else 100)
        item_init(w, w.ptr(e), 0, 0)
    if w.g8(s) == 0 and w.g8(s + 12):
        for o in range(6):
            w.s8(s + o, w.g8(s + 12 + o))
            w.s8(s + 12 + o, 0)
    return k


def team_shift(w, to, frm):
    """0x50790 (AL = to, DL = from): stat progress 0x12 bytes and the 0x200-byte soldier record move from slot
    `from` to `to` (the source progress is zeroed), +0x17A of the moved record = to, face buffer copied (BlitCopy
    32 x 32); the source record: salary, salary2, duty 0, profile 99, nickname byte 0 = byte 0x90FAC, inCurSector,
    +0x18A, +0x184, +0x185, +0x18B, +0x191, +0x183, +0x18D = 0, the first 6 bytes of each of the 12 item places 0,
    +0x17A = 99, +0x19D = 0, face buffer cleared (0x46AC4, from)."""
    to, frm = to & 0xFF, frm & 0xFF
    for j in range(0x12):
        w.s8(G_STAT_PROGRESS + 0x12 * to + j, w.g8(G_STAT_PROGRESS + 0x12 * frm + j))
    for j in range(0x12):
        w.s8(G_STAT_PROGRESS + 0x12 * frm + j, 0)
    src, dst = G_SOLDIERS + 0x200 * frm, G_SOLDIERS + 0x200 * to
    for j in range(0x200):
        w.s8(dst + j, w.g8(src + j))
    w.s8(dst + 0x17A, to)
    w.ext("blit", w.g32(0x93428 + 4 * to), w.g32(0x93428 + 4 * frm), 0x20, 0x20, 0x20, 0x20)
    w.s16(src + 0x110, 0)
    w.s16(src + 0x112, 0)
    w.s8(src + 0x17E, 0)
    w.s8(src + 0x179, 0x63)
    w.s8(src + 0x16E, w.g8(0x90FAC))
    for o in (0x182, 0x18A, 0x184, 0x185, 0x18B, 0x191, 0x183, 0x18D):
        w.s8(src + o, 0)
    for i in range(12):
        for j in range(6):
            w.s8(src + 12 * i + j, 0)
    w.s8(src + 0x17A, 0x63)
    w.s8(src + 0x19D, 0)
    w.ext("face_clear", frm)


def team_compact(w, redraw):
    """0x50750 (AL = redraw): for team slots j = 0..6 in order, when the soldier of pointer j (0x100958) has
    inCurSector 0 (the table points at g_soldiers), slot j + 1 moves into j (0x50790); then 0x21B3C when redraw is not 0."""
    for j in range(7):
        if not w.g8(G_SOLDIERS + 0x200 * j + 0x182):
            team_shift(w, j, j + 1)
    if redraw & 0xFF:
        w.ext("w_21b3c")


def pay_mercs(w):
    """PayMercs? 0x44740 (morning): team slots k = 0.. while inCurSector (+0x182) and k < 8: a soldier with hp
    (+0x183) not 0 and +0x19C not 1 or 7 whose money (0x4458C, no drop) is at least GREED[profile] x salary
    (u16 +0x110) runs off: quote, speech balloon, "Perhaps you should've given the money to a merc you can trust?",
    merc_leaves(status 0xE), profile 99, inCurSector 0, g_aimResignations + 1, team_compact(0), island map,
    ApplyDayLedger, panel redraw; then slot k is tested again (the next soldier moved into it)."""
    k = 0
    while k < 8:
        s = G_SOLDIERS + 0x200 * k
        if not w.g8(s + 0x182):
            break
        if (w.g8(s + 0x183) and w.g8(s + 0x19C) not in (1, 7)
                and cash_count(w, s, 0) >= i32(w.g8(GREED + w.g8(s + 0x179)) * w.g16(s + 0x110))):
            prof = w.g8(s + 0x179)
            w.s8(0x92F8B, 1)
            w.ext("quote", prof, prof * 15, 0xC, 200, 0x2299BC, 2, prof, 1)
            w.ext("w_67bf0")
            w.ext("w_467e4", 0, 0x34, 0x25)
            w.ext("balloon", k, w.g8(s + 0x17A) * 0x25 + 0xB, 0x14, 5)
            w.ext("w_28698", 0x25)
            w.ext("message", 0x99597, 0xE)
            merc_leaves(w, s, 0xE)
            w.s8(s + 0x179, 0x63)
            w.s8(s + 0x182, 0)
            w.s8(G_AIM_RESIGNATIONS, w.g8(G_AIM_RESIGNATIONS) + 1)
            team_compact(w, 0)
            w.ext("bass_map")
            apply_day_ledger(w)
            w.ext("w_36010", 0x90B18, w.g32(0x93E60), 0x140, 0x2B, 0x33)
            w.ext("w_67bf0")
            w.ext("w_46bf4")
            w.ext("w_673ac")
            continue
        k += 1


def explosive_kind(typ):
    """0x5AB78 (AL = item type) -> blast kind: 0x16..0x1B -> 4..9, 0x1C / 0x1D -> 2, 0x5D -> 1, up to 0x15 -> 0,
    others 3"""
    typ &= 0xFF
    if typ <= 0x15:
        return 0
    if typ <= 0x1B:
        return typ - 0x12
    if typ <= 0x1D:
        return 2
    return 1 if typ == 0x5D else 3


def shot_ap(s, w, tile):
    """ShotApCost 0x564F8 (EAX = soldier, EDX = tile) -> AP (byte) of one shot with the main-hand gun (types 1..0x10,
    else 0): 1 extra when the tile is not the last target s16 +0xD0 (or that is 0xC41); skill = +0x191, or for weapon
    class 4 (weapon byte 0) the average of agility and dexterity; d = ((skill / 2) + 50) x weapon byte 3 / 2; cost =
    extra + (200 x MercMaxAP / d + 1) / 2, at most MercMaxAP, at least 1 (bytes, unsigned compares)."""
    from combat import max_ap
    y = s[0]
    if not 1 <= y <= 0x10:
        return 0
    last = s16(u16(s, 0xD0))
    c = 1 if _i32(tile) != last or last == NO_TILE else 0
    m = max_ap(s) & 0xFF
    wb = G_WEAPON_B5 - 5 + 6 * y
    k = cdiv(s8(s[0x18B]) + s8(s[0x18C]), 2) & 0xFF if w.g8(wb) == 4 else s[0x191]
    d = cdiv(((k >> 1) + 50) * w.g8(wb + 3), 2)
    b = (c + cdiv(cdiv(200 * m, d) + 1, 2)) & 0xFF
    if b > m:
        b = m
    return b or 1


ANIM_AIMING = (0x1B, 0x1F, 0x23)      # ActionApCost: aiming animations (a turn to the target costs 1 AP)


def action_ap(s, w, tile, flag):
    """ActionApCost 0x56310 (EAX = soldier, EDX = tile, BL = flag) -> AP (byte) of using the main-hand item at the tile,
    by its kind: 0x95 ThrowApCost; 0x96 (knife): for a tile below 0xC41 (signed) that is not a neighbour in the same
    column or row (TileDistance > 1, or (tile % 56) and (tile / 56) both differ), ReachTile(soldier, tile, 0xF) ->
    PathApCost(soldier, s16 0x92E50) and tile = 0xC41, or 0x63 when it fails; + ShotApCost(soldier, tile). 0x92
    (gun): ShotApCost(soldier, tile), + the weapon byte 2 unless the current animation aims (0x1B / 0x1F / 0x23); an
    aiming soldier with BL pays 1 more when the tile is 0xC41 or Direction(soldier, tile) is not its facing +0x1A1.
    Others: 0x63."""
    tile = _i32(tile)
    kind = w.g8(G_ITEM_KIND + 12 * s[0])
    if kind == 0x95:
        return throw_ap_cost(s, w) & 0xFF
    me = s16(u16(s, 0x114))
    if kind == 0x96:
        c = 0
        if tile < 0xC41:
            col = lambda t: t - cdiv(t, 56) * 56
            if (_i32(w.ext("tile_dist", me, tile)) > 1
                    or (col(me) != col(tile) and cdiv(me, 56) != cdiv(tile, 56))):
                if _i32(w.ext("reach_ap", s, tile, 0xF)):
                    c = w.ext("path_ap", s, s16(w.g16(G_MISSION_SPOT))) & 0xFF
                    tile = NO_TILE
                else:
                    c = 0x63
        return (c + w.ext("shot_ap", s, tile)) & 0xFF
    if kind != 0x92:
        return 0x63
    sap = w.ext("shot_ap", s, tile) & 0xFF
    if u16(s, 0x1E8 + 2 * s[0x1F3]) not in ANIM_AIMING:
        return (sap + w.g8(G_WEAPON_B5 - 3 + 6 * s[0])) & 0xFF
    if not flag & 0xFF:
        return sap
    if tile != NO_TILE and _i32(w.ext("direction", tile_x(me), tile_y(me), tile_x(tile), tile_y(tile))) == s8(s[0x1A1]):
        return sap
    return (sap + 1) & 0xFF


def reserve_ap(s, w):
    """0x17C88 (EAX = soldier): +0xEF = the AP held back for the item in hand (entry 0), by its blast kind
    (0x5AB78): kinds 3 / 5 / 7 / 9 -> 6; kind 0 with type 0 or above 0xE -> 0; otherwise ActionApCost(soldier,
    0xC41, 1), and when the soldier's +0x1A5 is above it (signed) and +0x16A is set, 4 more, at most +0x1A5."""
    k = explosive_kind(s[0])
    if k in (3, 5, 7, 9):
        s[0xEF] = 6
        return
    if k == 0 and not 0 < s[0] <= 0xE:
        s[0xEF] = 0
        return
    ap = w.ext("action_ap", s, NO_TILE, 1) & 0xFF
    s[0xEF] = ap
    if s8(s[0x1A5]) > s8(ap) and s[0x16A]:
        v = (ap + 4) & 0xFF
        s[0xEF] = v
        if s8(v) > s8(s[0x1A5]):
            s[0xEF] = s[0x1A5]


TRAP_SILENT =(0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D)     # 0x1AC68: armed explosives that go off without the click


def play_empty_click(s, w):
    """PlayEmptyClick 0x1ABAC (EAX = soldier): the click of a sprung ground item that has no effect. With +0x9C
    above 0 (signed) the click sound plays, then a 50 ms wait - or the "click" subtitle for 500 ms when Subtitles
    (0x1163BD) is on. A merc (profile below 0x40) also takes the view, cries "BAD", plays the voice sample and
    waits 100 ms (the busy wait 0x67250, op 37 delay)."""
    if s8(s[0x9C]) > 0:
        w.ext("sfx", 0x23, 0x2B11, 0x58, 1)
        if w.g8(0x1163BD):
            w.ext("show_msg", "click", 0, 0, 1, 1, 0, 0x1F4, 1)
        else:
            w.ext("delay", 0x32)
    if s[0x179] < 0x40:
        w.ext("focus", s[0x17A], 1)
        w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
        w.ext("voc", 0x2B11, 0x58, 1)
        w.ext("delay", 0x64)


def trigger_item(s, w, tile, gi, armed):
    """0x1AC68 (EAX = soldier, EDX = tile, BL = ground item entry, CL = armed / sprung) -> 1 when something happened.
    By the entry's type: 0x11 (a set mine, condition below 0): click + Explode(kind 2). 0x13: click, a flare (0x157BC
    with the count, or 0x355D0(0x29) without); sprung by a merc the entry becomes a used item 0x40 (count 1,
    condition Random(100) + 1, +6 = 1) and a burning object (0x86) goes out (0x2E730). 0x2C: click, 0x635DC for a
    soldier in state 4 else 0x355D0(0x3F, count, 800, 0x41, 0); then as 0x13. 0x72: click, wait 1000, the mission
    step and order are cleared; sprung by a merc as 0x13 without the fire. 0x44: 0x72568(tile, 0x44) and
    0x19EB0(tile, 0x44). 0x52 / 0x70: 0x35774(0 / 1, condition) and 0x19EB0(tile, type), 0x52 with click. 0x29 / 0x3F /
    0x71: armed: click + Explode(kind 3), else 0. Other types: a set explosive (condition below 0) that is armed goes
    off with its explosive_kind (with click, except TRAP_SILENT), else 0. When something happened: the reset entry
    goes through 0x3C16C(soldier, tile, 1), else 0x3C05C(tile, 0)."""
    g = G_GROUND_ITEMS + 12 * (gi & 0xFF)
    armed &= 0xFF
    typ = w.g8(g)
    slot = s[0x17A]
    r, reset = 1, False

    def spent():
        w.s8(g, 0x40)
        v = w.random(100)
        w.s8(g + 1, 1)
        w.s8(g + 2, v + 1)
        w.s8(g + 6, 1)

    def put_out():
        if w.tile8(tile, 1) == 0x86:
            w.s8(G_TILEMAP + 8 * tile + 1, 0)
            rebuild_move_codes(w)

    if typ == 0x11:
        if s8(w.g8(g + 2)) < 0:
            play_empty_click(s, w)
            w.ext("explode", slot, tile, 2, 1)
        else:
            r = 0
    elif typ in (0x13, 0x2C):
        play_empty_click(s, w)
        if typ == 0x13:
            if s8(w.g8(g + 1)) > 0:
                w.ext("fx_157bc", NO_TILE, 0x29, w.g8(g + 1), 0x1F4, 0x41, 0)
            else:
                w.ext("w_355d0", 0x29, 0, 0x1F4, 0x41, 0)
        elif s8(s[0xBA]) == 4:
            w.ext("w_635dc")
        else:
            w.ext("w_355d0", 0x3F, s8(w.g8(g + 1)) & 0xFFFFFFFF, 0x320, 0x41, 0)
        if armed and slot < 8:
            spent()
            reset = True
            put_out()
    elif typ == 0x72:
        play_empty_click(s, w)
        w.ext("wait", 1000)
        w.s8(G_MISSION_STEP, 0)
        w.s8(G_MISSION_ORDER, 0x40)
        w.s16(G_MISSION_ORDER + 2, NO_TILE)
        if armed and slot < 8:
            spent()
            reset = True
    elif typ == 0x44:
        w.ext("w_72568", tile, 0x44)
        w.ext("w_19eb0", tile, 0x44)
    elif typ in (0x52, 0x70):
        if typ == 0x52:
            play_empty_click(s, w)
        w.ext("w_35774", 0 if typ == 0x52 else 1, w.g8(g + 2))
        w.ext("w_19eb0", tile, typ)
    elif typ in (0x29, 0x3F, 0x71):
        if armed:
            play_empty_click(s, w)
            w.ext("explode", slot, tile, 3, 1)
        else:
            r = 0
    else:
        k = explosive_kind(typ)
        if s8(w.g8(g + 2)) < 0 and armed:
            if typ not in TRAP_SILENT:
                play_empty_click(s, w)
            w.ext("explode", slot, tile, k, 1)
        else:
            r = 0
    if r:
        if reset:
            w.ext("w_3c16c", s, tile, 1)
        else:
            w.ext("w_3c05c", tile, 0)
    return r


G_NOTES_FOUND = 0x92ED2        # 15 x {item type, placed flag}


def note_placed(w, typ):
    """NoteAlreadyPlaced 0x3FC34 (AL = item type) -> 1 if it is a one-off note already placed; a note placed for the
    first time is flagged now (0)"""
    for k in range(15):
        if w.g8(G_NOTES_FOUND + 2 * k) == typ & 0xFF:
            if w.g8(G_NOTES_FOUND + 2 * k + 1) == 1:
                return 1
            w.s8(G_NOTES_FOUND + 2 * k + 1, 1)
            return 0
    return 0


def ground_slot(w):
    """0x2390C -> the first free ground item entry (type 0) of 200, or 200"""
    for k in range(200):
        if not w.g8(G_GROUND_ITEMS + 12 * k):
            return k
    return 200


# 0x23610 for objects of class 6..8: 1 = items may lie on it, -1 / -2 = no (callers only test 1), else 0
OBJECT_SURFACE = {0x3B: 1, 0x3D: -2, 0x3E: -2, 0x41: 1, 0x43: -1, 0x45: -1, 0x5A: 1, 0x5C: -2, 0x5D: -2, 0x60: 1,
                  0x62: -1, 0x64: -1}


def object_surface(w, tile):
    """0x23610 (EAX = tile): 1 for an object of class 5 or below / 9 or above, else by object (OBJECT_SURFACE)"""
    obj = w.tile8(tile, 1)
    c = w.g8(G_T1_CLASS + obj)
    if c <= 5 or c >= 9:
        return 1
    return OBJECT_SURFACE.get(obj, 0)


def item_tile_ok(w, tile, flag):
    """0x3FAF0 (EAX = tile, DL = flag) -> 1 when an item may be put there: no ground item with a positive condition
    or a +6 other than 0xFF, not water, and (unless the flag) object_surface 1"""
    gi = w.tile8(tile, 6)
    if gi != 0xFF:
        g = G_GROUND_ITEMS + 12 * gi
        if s8(w.g8(g + 2)) > 0 or s8(w.g8(g + 6)) != -1:
            return 0
    if is_water_tile(w.tile8(tile, 0), w.tile8(tile, 1)):
        return 0
    if flag & 0xFF:
        return 1
    return 1 if object_surface(w, tile) == 1 else 0


def find_item_tile(w, tile, flag):
    """0x3FB6C (EAX = tile, BL = flag) -> the tile itself if item_tile_ok, else the first ok tile r = 1..11 steps out
    in directions 1..8 (StepTile stops at the map edge), else none"""
    if item_tile_ok(w, tile, flag) > 0:
        return tile
    for r in range(1, 12):
        for d in range(1, 9):
            t = tile
            for _ in range(r):
                t = step_tile(t, dir_step(w, d))
            if item_tile_ok(w, t, flag):
                return t
    return NO_TILE


def put_item(w, tile, d0, d1, d2, b6, exact, note, near):
    """PutItem 0x3FC78 (EAX = tile; stack: the 12-byte item entry as 3 dwords, +6 byte, exact flag, note flag, search
    flag; ret 0x1C): no item type: nothing; a note (flag) already placed: nothing; on water a type with item table
    +13 set sinks (nothing). Takes the first free ground entry; the tile is the given one (exact) or the nearest ok
    one (find_item_tile with the search flag). No tile: 0x3FC04(type), and if that is 0, 0x3FC04(byte 3). With an
    entry below 200 and a tile: the tile's +6 = entry, the entry = the item with +6 replaced; a stacking type (item
    table +6 above 1) with count above 1 fills empty unit conditions (+8..) from +2. When the tile differs from the
    asked one and the asked tile holds a ground item whose +6 is not 0xFF, the two tiles' +6 swap (the asked one
    gets the new entry)."""
    it = bytearray((d0 & 0xFFFFFFFF).to_bytes(4, "little") + (d1 & 0xFFFFFFFF).to_bytes(4, "little")
                   + (d2 & 0xFFFFFFFF).to_bytes(4, "little"))
    typ = it[0]
    if not typ:
        return
    if note & 0xFF and note_placed(w, typ) == 1:
        return
    if is_water_tile(w.tile8(tile, 0), w.tile8(tile, 1)) and w.g8(ITEM_TBL + 12 * typ + 13):
        return
    slot = ground_slot(w)
    dest = tile if exact & 0xFF else find_item_tile(w, tile, near)
    if dest == NO_TILE and not w.ext("w_3fc04", typ):
        w.ext("w_3fc04", it[3])
    if slot < 200 and dest != NO_TILE:
        w.s8(G_TILEMAP + 8 * dest + 6, slot)
        g = G_GROUND_ITEMS + 12 * slot
        for k in range(12):
            w.s8(g + k, it[k])
        w.s8(g + 6, b6)
        if w.g8(ITEM_TBL + 12 * w.g8(g) + 6) > 1 and s8(w.g8(g + 1)) > 1:
            j = 0
            while j < s8(w.g8(g + 1)) - 1:
                if not w.g8(g + 8 + j):
                    w.s8(g + 8 + j, w.g8(g + 2))
                j += 1
    if tile != dest:
        gi = w.tile8(tile, 6)
        if gi < 0xFF and s8(w.g8(G_GROUND_ITEMS + 12 * gi + 6)) != -1:
            w.s8(G_TILEMAP + 8 * tile + 6, slot)
            w.s8(G_TILEMAP + 8 * dest + 6, gi)


def gas_start(w, tile, kind):
    """0x38594 (EAX = tile, DL = kind 4 / 5) -> cloud index or -1 when all 5 are in use: lasts 3 turns (kind 4) or 5,
    one or two fewer (Random(2) + 1) unless the ground is class 3; radius 3 (kind 4) or 5"""
    kind &= 0xFF
    turns = 3 if kind == 4 else 5
    if w.g8(G_T0_CLASS + w.tile8(tile, 0)) != 3:
        turns -= w.random(2) + 1
    for k in range(5):
        e = G_GAS_CLOUDS + 6 * k
        if s16(w.g16(e)) == NO_TILE:
            w.s16(e, tile)
            w.s16(e + 2, turns)
            w.s16(e + 4, 3 if kind == 4 else 5)
            return k
    return -1


def sector_tick(w):
    """SectorTick? 0x1583C (op 160, once per new turn): n = 1000 / s8 splash[cur_sector] tries (none when n <= 0):
    t = Random(0xC40) (kept in 0x229AFC); on water ground (byte 0 >= 0x97) with no object and an occupant below 0x40
    with hp > 0 and an animation word below 0x3C: SetAnim(occupant, 0x3D, 0, 1) and stop."""
    n = 1000 // s8(w.g8(G_SPLASH + w.g8(G_CUR_SECTOR)))       # (positive divisor: C and Python division agree)
    for _ in range(n):
        t = w.random(0xC40)
        w.s32(0x229AFC, t & 0xFFFFFFFF)
        if w.tile8(t, 0) < 0x97 or w.tile8(t, 1) != 0:
            continue
        o = w.tile8(t, 5) & 0x7F
        if o >= 0x40:
            continue
        r = G_SOLDIERS + 0x200 * o
        if s8(w.g8(r + 0x183)) > 0 and w.g16(r + 0x1E8 + 2 * w.g8(r + 0x1F3)) < 0x3C:
            w.ext("set_anim", o, 0x3D, 0, 1)
            return


EFFECT_RADIUS = {2: 2, 3: 4, 4: 2, 5: 4, 7: 4, 9: 4}     # 0x173B4's switch on the kind; others 3


def effect_hit(w, tile, kind, power):
    """0x173B4 (EAX = tile, EDX = kind, BL = power; once per explosion effect record from DrawProjectilePass): an
    object 0x86 on the tile is cleared (0x2E730). Kind 3 on a tree (object 0x35 / 0x36) costs the sector a tree (b6)
    and, for 0x35, b8; replay event V (0x56) / U (0x55); SectorWorkable. Kinds 4 / 5 start a gas cloud and stop when
    none is free. Kinds 2 / 3 with power above 0 load the explosion art (0x33BD4, flag 1 for kind 3) and, in the
    story sector (0x22A057), hit the story tiles (5 words at sector record +0x12) that equal the tile (0x61D28);
    otherwise 0x3C05C(tile, 0). With power above 0 the blast follows: Blast(tile, radius by kind, kind)."""
    power = s8(power & 0xFF)
    if w.tile8(tile, 1) == 0x86:
        w.s8(G_TILEMAP + 8 * tile + 1, 0)
        rebuild_move_codes(w)
    obj = w.tile8(tile, 1)
    sec = w.g8(G_CUR_SECTOR)
    if kind == 3 and obj in (0x35, 0x36):
        w.s8(G_SECTOR_TBL9 + 9 * sec + 6, w.g8(G_SECTOR_TBL9 + 9 * sec + 6) - 1)
        if obj == 0x35:
            w.s8(G_SECTOR_TBL9 + 9 * sec + 8, w.g8(G_SECTOR_TBL9 + 9 * sec + 8) - 1)
        r = s8(sec) & 0xFFFFFFFF
        w.ext("replay_event", r, r, 0x56 if obj == 0x35 else 0x55)
        w.ext("workable", G_SECTOR_TBL9, w.g8(G_CUR_SECTOR))
    if kind in (4, 5) and gas_start(w, tile, kind) == -1:
        return
    if kind in (2, 3) and power > 0:
        w.ext("explode_art", tile, 1 if kind == 3 else 0)
        if w.g8(G_CUR_SECTOR) == s8(w.g8(0x22A057)):
            for j in range(5):
                if s16(w.g16(0x222BF6 + 0xB1 * w.g8(G_CUR_SECTOR) + 2 * j)) == _i32(tile):
                    w.ext("story_hit")
    else:
        w.ext("w_3c05c", tile, 0)
    if power > 0:
        blast(w, tile, EFFECT_RADIUS.get(kind, 3), kind)


EXPLODE_OFS = (-57, -56, -55, -1, 0, 1, 55, 56, 57)    # LoadExplodeDat 0x31BF0: the 3 x 3 cells; byte 2 sits a row higher
EXPLODE_FILL = (0, 0, 0, 0, 0, 7, 0, 8, 9)             # 0x31C02: object per cell for the small-object fill (cells 5, 7, 8)
G_EXPLODE_CACHE = 0x95E0E     # 12 records of 7 B (u16 tile, ..., u8 difficulty at +6): sector 0's tiles that stay whole
G_EXPLODE_PIECE = 0x95FF0     # 10 records of 12 B (i32 tile offset, u32 byte 1, u32 byte 2): EXPLODE.DAT words 1000..1009


def explode_class(o):
    """0x33B60 (AL = object byte) -> 0 none, 6 0x27..0x36, 7 1..0x55 / 0x7F / 0x80, 8 0x56..0x7E / 0x81 / 0x82, 9 above"""
    o &= 0xFF
    if o == 0:
        return 0
    if 0x27 <= o <= 0x36:
        return 6
    if o <= 0x55 or 0x7F <= o <= 0x80:
        return 7
    if o <= 0x7E or 0x81 <= o <= 0x82:
        return 8
    return 9


def _mark(w, t, v):
    """tile byte 4 = its 0x80 bit | v"""
    a = G_TILEMAP + 8 * t + 4
    w.s8(a, (w.g8(a) & 0x80) | v)


def explode_marks(w, t, cls):
    """0x33820 (EAX = tile, DL = class): byte-4 marks round a broken object. 0x23938 gives 0 / 1 (byte 4 != 0), so its
    tests against 0x50, 0x51, 0x49, ... never pass (kept as in the original)."""
    def q(x):
        return 1 if w.tile8(x, 4) else 0
    o = w.tile8(t, 1)
    d, at = 0, None
    if o == 6:
        at = t + 56
        if q(t + 56) == 0x50:
            d = 7
        elif q(t + 57) == 0x50:
            _mark(w, t + 57, 0x4D)
        elif w.tile8(t - 1, 1) != 0x52:
            d = 0x1E
        if q(t + 1) == 0x51:
            _mark(w, t + 1, 0x4E)
        if q(t + 57) in (0x49, 0x4E, 0x46, 0x4C):
            _mark(w, t + 57, 0x63)
        if q(t + 1) in (0x4F, 0x4B):
            _mark(w, t + 1, 0x63)
        if q(t + 57) == 0x51:
            _mark(w, t + 57, 0x4F)
    elif o == 0xB:
        at = t + 1
        d = 0xC if q(t + 1) == 0x4E else 0xA
    elif o == 0xC:
        at = t + 56
        d = 0xB if q(t + 56) == 0x4C else 8
    elif o == 0xF:
        if cls & 0xFF == 6:
            _mark(w, t + 56, 0x63)
        else:
            for x in (t, t - 1, t - 56, t - 57):
                _mark(w, x, 0x63)
    elif o in (0x22, 0x25):
        at, d = t, 0x1E
    elif o == 0x52:
        at = t + 57
        if q(t + 57) != 0x50:
            d = 7
    elif o == 0x53:
        at, d = t + 57, 9
    if d:
        _mark(w, at, d + 0x45)


def explode_dat(w, tile, big):
    """LoadExplodeDat 0x33BD4 (EAX = tile, EDX = big; host op 336 from the effect hit, big = 1 for kind 3): the crater.
    Sector 0 (Santino not yet spoken to twice): tiles 0x122..0x124 and the cache tiles of 0x95E0E up to the difficulty
    stay whole. Opens DAT\\EXPLODE.DAT (256 rows of 9 cells x 2 words, row = the cell's old object). Water ground ->
    0x1D + Random(3). The ground item: 0x5F -> 0x7B, 0x6E -> 0x7C (+ 0x22A05E = 1), both count 99, byte 6 = 0; item 0x3F
    on tile 0x15B in sector 0 after Santino: ForgetSoldier + enemies + 1 + MercDies + KillStragglers; others are taken
    (0x3C05C). Big: class 9 objects count as small. Object below 0x1B: none -> 1 + Random(2) (3 + on class-3 ground).
    Else per cell the file's words rewrite byte 1 / byte 2 (classes 6..8 and 0x10 / 0x11 only when big), marks
    (0x33820), the big-blast object fixes and the 1000+ pieces, the small-object fill. fclose; move code of the tile."""
    t = s16(tile)
    big = _i32(big)
    if w.g8(G_CUR_SECTOR) == 0 and s8(w.g8(G_SANTINO_SPOKE)) < 2:
        if 0x122 <= t <= 0x124:
            return
        for i in range(12):
            r = G_EXPLODE_CACHE + 7 * i
            if s16(w.g16(r)) == t and w.g8(r + 6) <= s8(w.g8(G_DIFFICULTY)):
                return
    w.ext("open_cd", "dat\\explode.dat")
    obj0 = w.tile8(t, 1)
    cls0 = w.g8(G_T0_CLASS + w.tile8(t, 0))

    def t1(x):
        return w.tile8(x, 1)

    def t2(x):
        return w.tile8(x, 2)

    def set1(x, v):
        w.s8(G_TILEMAP + 8 * x + 1, v)

    def set2(x, v):
        w.s8(G_TILEMAP + 8 * x + 2, v)

    if cls0 == 3:
        w.s8(G_TILEMAP + 8 * t, (w.random(3) + 0x1D) & 0xFF)
    k = item_index(w, t)
    if k < 0xFF:
        g = G_GROUND_ITEMS + 12 * k
        typ = w.g8(g)
        if typ in (0x5F, 0x6E):
            w.s8(g, 0x7B if typ == 0x5F else 0x7C)
            w.s8(g + 1, 0x63)
            if typ == 0x6E:
                w.s8(0x22A05E, 1)
            w.s8(g + 6, 0)
        else:
            if (typ == 0x3F and w.g8(G_CUR_SECTOR) == 0 and t == 0x15B and w.g8(G_SANTINO_SPOKE)):
                w.ext("forget_soldier", 0x28)      # g_soldiersPtr 0x1009F8 = slot 0x28's record
                a = G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR) + 1
                w.s8(a, w.g8(a) + 1)
                w.ext("merc_dies", 0x28, 0)
                w.ext("stage_five")
            w.ext("w_3c05c", t, 0)
    cls = None                       # the word at [esp+0x8C]: set before every use the original makes of it
    if big:
        cls = explode_class(obj0)
        if cls == 9:
            big = 0
    if obj0 < 0x1B:
        if obj0 == 0 and 1 <= cls0 <= 7:
            set1(t, (w.random(2) + (3 if cls0 == 3 else 1)) & 0xFF)
    else:
        O = obj0
        for j in range(0, 18, 2):
            n1, n2 = t + EXPLODE_OFS[j // 2], t + EXPLODE_OFS[j // 2] - 56
            o = t1(n1)
            w.ext("fseek", 0, 2 * j + o * 0x24, 0)
            v = s16(int.from_bytes(w.read(2), "little"))
            if v < 0xFF:
                cls = explode_class(o)
                if not 6 <= cls <= 8 or big:
                    set1(n1, v & 0xFF)
                explode_marks(w, n1, cls)
                v = s16(int.from_bytes(w.read(2), "little"))
                if v != 0xFF:
                    cls = w.g8(G_T2_CLASS + t2(n2))
                    if cls not in (0x10, 0x11) or big:
                        set2(n2, v & 0xFF)
            if j in (0, 6) and o < 7 and t2(n2 - 1) == 0x4A:
                set2(n2, 0x50)
            if 1 <= o < 0x83 and big:
                if O in (0x56, 0x66):
                    if j == 4:
                        if o == 0x67:
                            set1(n1, 0x72); set2(n2, 0)
                    elif j == 10:
                        if o == 0x57 or 0x75 <= o <= 0x79:
                            set1(n1, 0xE); set2(n2, 0x51)
                        elif o == 0x71:
                            set1(n1, 6); set2(n2, 0)
                        elif o == 0x5B:
                            set1(n1, 6); set2(n2, 0); set1(n1 + 1, 0xE); set2(n2 + 1, 0x51)
                    elif j == 14:
                        if o == 0x6D:
                            set1(n1 - 56, 0xD); set2(n2 - 56, 0)
                        elif o == 0x61:
                            set1(n1, 6); set2(n2, 0); set1(n1 + 56, 0xD); set2(n2 + 56, 0)
                    if j == 14 and (o == 0x58 or 0x7A <= o <= 0x7E):
                        set1(n1, 0xD); set2(n2, 0)
                if O == 0x37:
                    if j == 14 and o == 0x39:
                        set1(n1, 0xB); set2(n2, 0)
                    elif j == 10:
                        if o == 0x38:
                            set1(n1, 0xC); set2(n2, 0)
                        elif o == 0x52:
                            set1(n1, 6); set2(n2, 0)
                        elif o == 0x3C:
                            set1(n1, 6); set2(n2, 0); set1(n1 + 1, 0xC); set2(n2 + 1, 0x48)
                if O == 0x57 and o == 0x67 and j == 12:
                    set1(n1, 0x71); set2(n2, 0x50)
                if O == 0x67:
                    if o == 0x65 and j == 14 and t1(n1 - 1) == 6:
                        set1(n1, 6); set2(n2, 0)
                    elif o == 0x57 and j == 12 and t1(n1 + 1) != 0x65:
                        set1(n1, 0x71); set2(n2, 0x50)
                if O in (0x38, 0x52) and j == 6 and o == 0x37 and t1(n1 - 1) == 0:
                    set1(n1, 6); set2(n2, 0)
                    if t1(n1 - 1) == 0 and t2(n2 - 1) == 0x4A:
                        set2(n2 - 1, 0x50)
                if O == 0x4B and j == 6 and o == 0x37:
                    set1(n1, 0xB); set2(n2, 0)
                if o == 0x56 and j == 2:
                    x = t1(n1 - 56)
                    if x == 0x4E:
                        set1(n1 - 56, 0x53); set2(n2 - 56, 0)
                    elif x == 0x50:
                        set1(n1 - 56, 0x52); set2(n2 - 56, 0x47)
                if ((o == 0x56 and j == 6 and t1(t - 2) == 0) or (O == 0x56 and j == 10 and t1(t + 1) == 0x71)
                        or (o == 0x72 and j == 14 and O == 0x56) or (O == 0x39 and j == 6 and o == 0x71)):
                    set1(n1, 6); set2(n2, 0)
                if O == 0x39 and j in (6, 12) and o == 0x57:
                    set1(n1, 0x71); set2(n2, 0x50)
                    set1(t, 6); set2(t - 56, 0)
                    explode_marks(w, t, cls)
                    set1(t + 56, 6 if t1(t + 56) == 0x53 else 0xB)
                    set2(t, 0)
                    explode_marks(w, t + 56, cls)
                if O == 0x4A and j == 10:
                    if o == 0x38:
                        set1(n1, 0xC); set2(n2, 0x48)
                    elif o == 0x52:
                        set1(n1, 6); set2(n2, 0)
                    explode_marks(w, n1, cls)
                if (o == 0x5D and j == 6 and t1(t - 2) == 0x5B) or (o == 0x5A and j == 6 and t1(t - 2) == 0x59):
                    set1(t - 2, 0x71); set2(t - 58, 0x50)
                if (o in (0x3D, 0x3E) and j == 6 and t1(t - 2) == 0x3C) or (o == 0x3B and j == 6 and t1(t - 2) == 0x3A):
                    set1(t - 2, 0x52); set2(t - 58, 0x47)
                    explode_marks(w, t - 2, cls)
                if o in (0x64, 0x62) and j == 2 and t1(t - 112) == 0x63:
                    set1(t - 112, 0x72)
                if (o == 0x41 and j == 2 and t1(t - 112) == 0x42) or (o in (0x43, 0x45) and j == 2 and t1(t - 112) == 0x44):
                    set1(t - 112, 0x53)
                    explode_marks(w, t - 112, cls)
                if o == 0x57 and j == 0 and t1(t - 112) == 0x39:
                    set1(t - 112, 0x53)
                    explode_marks(w, t - 112, cls)
                    set1(t - 56, 6); set2(t - 112, 0)
                    explode_marks(w, t - 56, cls)
                if O in (0x58, 0x72, 0x39) and t1(t - 112) in (0x58, 0x6D) and t1(t - 56) == 6:
                    set1(t - 112, 0x72)
                if O == 0x58 and t1(t - 112) == 0xD and t1(t - 56) == 6:
                    set1(t - 112, 6)
                if O in (0x58, 0x72) and t1(t - 112) == 0x4E:
                    set1(t - 112, 0x53)
                if O == 0x49 and j == 14:
                    if o == 0x39:
                        set1(n1, 0xB)
                    elif o == 0x53:
                        set1(n1, 6)
                    explode_marks(w, n1, cls)
                if j == 2 and o in (0x66, 0x47) and t1(n1 - 56) == 0:
                    set1(n1, 6)
            if 0x1B <= o <= 0x1D:
                set2(t, 0)
            if 0x1E <= o <= 0x20:
                set2(t - 1, 0)
            if v >= 1000 and big:
                v = s16(v - 1000)
                if v < 10:
                    set1(n1, 6); set2(n2, 0)
                    r = G_EXPLODE_PIECE + 12 * v
                    d = _i32(w.g32(r))
                    set1(n1 + d, w.g8(r + 4))
                    set2(n2 + d, w.g8(r + 8))
                elif v == 10:
                    x = t1(n1 - 1)
                    if x in (0x57, 0xE, 0x5C, 0x5D) or 0x75 <= x <= 0x79:
                        set1(n1, 0x71); set2(n2, 0x50)
                    elif x == 0xC:
                        set1(n1 - 1, 6); set2(n2 - 1, 0); set1(n1, 6); set2(n2, 0)
                    else:
                        set1(n1, 6); set2(n2, 0)
                elif v == 11:
                    set1(n1, 0x72 if t1(n1 - 56) == 0x6D else 6); set2(n2, 0)
                elif v == 12:
                    if t1(n1 - 56) == 6:
                        set1(n1, 6); set2(n2, 0); set1(n1 + 56, 0xD); set2(n2 + 56, 0)
                elif v == 13:
                    x = t1(n1 - 1)
                    if 0x56 <= x <= 0x58 or 0x75 <= x <= 0x79:
                        set1(n1 - 1, 0x71); set2(n2 - 1, 0x50)
                    else:
                        set1(n1 - 2, 0x71); set2(n2 - 2, 0x50); set1(n1 - 1, 6); set2(n2 - 1, 0)
                    set1(n1, 0x53); set2(n2, 0)
                elif v == 14:
                    x = t1(n1 - 1)
                    if 0x37 <= x <= 0x39 or x == 0x7F:
                        set1(n1, 0x52); set2(n2, 0x47)
                    else:
                        set1(n1, 0x71); set2(n2, 0x50)
                elif v == 15:
                    set1(n1, 6 if t1(n1 - 1) == 0 else 0x53); set2(n2, 0)
                elif v == 16:
                    if t1(n1 - 56) == 0x67:
                        set1(n1 - 56, 0x72); set2(n2 - 56, 0)
                    set1(n1, 0xE); set2(n2, 0x51)
                elif v == 17:
                    if t1(n1 - 56) == 0x72:
                        set1(n1, 6); set2(n2, 0)
                    else:
                        set1(n1, 0xE); set2(n2, 0x51)
                elif v == 18:
                    if t1(n1 + 56) != 0x72:
                        set1(n1, 6); set2(n2, 0)
                    else:
                        set1(n1, 0xD); set2(n2, 0)
            cls = w.g8(G_T1_CLASS + O)
            if cls in (7, 8) and t1(n1) < 10:
                if j in (10, 14):
                    set1(n1, EXPLODE_FILL[j // 2])
                elif j == 16 and t1(n1 - 56) < 15 and t1(n1 - 1) < 15:
                    set1(n1, EXPLODE_FILL[8])
    w.ext("fclose", 0)
    if w.g8(G_T1_CLASS + obj0) in (2, 3):
        w.s16(G_MOVECODE + 2 * t, 0x62D4)
    elif w.g16(G_MOVECODE + 2 * t) not in (0x62D4, 0x6338):
        w.ext("w_2be54", t)


G_STORY_TIMERS = 0x22A491      # u8 at +0 of 5 records of 13 B: counters that run down each turn


def _fill(w, a, v, n):
    for i in range(n):
        w.s8(a + i, v)


def new_turn(w, focus):
    """NewTurn? 0x126C8 (EAX = focus the view) -> AL 1 when guards arrive instead (0 otherwise). Turn state reset
    (0x92DCE, a pending redraw flag 0x92DA4 kept for the end, bleed marks, merc quote 0x1B324 for 0x92DEC).
    Unless 0x92DDD was set: guards arriving (0x1F724(5)) end it with AL = 1; a queued interrupt turn (0x92DD0) hands
    over (0x12D58(1), keeping the selected merc) and sets 0x92DDC / 0x92DDD. Else the new turn: timers and flags
    (0x229D1C = 20000, 0x229E04 = -1, 0x92E64 = 0x92DCC = 1, ...); in combat the AP warnings and the interrupt queue
    reset (0x10398(1)); out of combat the guards 8..15 think (AiThink, +0x197 = 0). 0x1583C; the gas clouds tick;
    combat ends when the sector has no mercs left. Every living soldier below slot 0x28 starts its turn; enemies keep
    +0x9C in +0x198. In combat: 0x741F8, the merc to show (0x92DC7, from 0x2AC60 when 0x30+) and the view / panel
    / mouse handling; out of combat a selected enemy falls back to slot 8. Then 0x22A0CD = 1, 0x92E3C = 0, the five
    story timers and 0x284F02 count down, a pending speaker 0x92DD5 may talk (0x297D4), the Santino countdown in
    sector 0 (0x22A1F2, 0x635DC, at 5 the sector's enemy count - 1 and 0x74C44), the bleeders cough, and the kept
    redraw (0x31550) / flag 0x22A1F3 are handled."""
    keep = False
    w.s8(0x92DCE, 0)
    if w.g8(G_COMBAT):
        w.ext("w_67e30")
    if w.g32(0x92DA4):
        keep = True
        w.s32(0x92DA4, 0)
    w.ext("w_6a960")
    if w.g8(G_COMBAT):
        w.s8(0x92DE0, 1)
        w.s8(0x92DE1, 1)
    _fill(w, G_BLED, 0, 0x30)
    q = w.g8(0x92DEC)
    if q < 0x40:
        w.s8(0x284F26, 0)
        w.ext("merc_quote", q)
    if not w.g8(0x92DDD):
        if w.ext("guards_arrive", 5) & 0xFFFFFFFF:
            w.s8(0x92DE1, 0)
            w.s8(0x92DE0, 0)
            return 1
        if w.g8(0x92DD0):
            if w.g8(G_SELECTED) < 8:
                w.s8(0x284F0F, w.g8(G_SELECTED))
            w.ext("next_turn", 1)
            w.s8(0x92DDC, 1)
            w.s8(0x284F0F, 0x40)
            w.s8(0x92DDD, 1)
            w.s8(0x92DE1, 0)
            w.s8(0x92DE0, 0)
            return 0
    else:
        w.s8(0x92DDD, 0)
    w.s8(0x92DDC, 0)
    w.s32(0x229D1C, 20000)
    w.s8(0x92DC9, 0)
    w.s8(0x92DCD, 0)
    w.s32(0x229E04, 0xFFFFFFFF)
    w.s8(0x92E64, 1)
    w.s8(G_SHOW_UI, 1)
    if w.g8(G_COMBAT):
        _fill(w, 0x22A0EC, 0, 8)
        w.s8(G_ITQ_N, 0)
        w.s8(0x92DD0, 0)
        w.s8(0x92DCF, 0)
        w.s8(0x92DD2, 0x40)
        w.s8(0x284F0F, 0x40)
        _fill(w, G_ITQ, 0x40, 0x32)
        w.ext("w_10398", 1)
    else:
        for k in range(8, 16):
            w.s8(G_SOLDIERS + 0x200 * k + 0x197, 0)
            think(w.rec(k), w)
    w.ext("w_1583c")
    gas_tick(w)
    sec = w.g8(G_CUR_SECTOR)
    if not w.g8(G_SECTOR_TBL9 + 9 * sec + 2):
        w.s8(G_COMBAT, 0)
    for k in range(0x28):
        p = w.rec(k)
        if p[0x182] and p[0x138] and p[0x183]:
            start_turn(p, w)
    for k in range(0x28, 0x30):
        w.s8(G_SOLDIERS + 0x200 * k + 0x198, w.g8(G_SOLDIERS + 0x200 * k + 0x9C))
    if w.g8(G_COMBAT):
        w.ext("w_741f8", w.g8(G_SHOW_UI))
        if w.g8(0x92DC7) >= 0x30:
            w.s8(0x92DC7, w.ext("w_2ac60") & 0xFF)
        v = w.g8(0x92DC7)
        w.s32(0x229CB4, v)
        w.s32(0x93D54, 5000)
        if s8(w.g8(G_SOLDIERS + 0x200 * v + 0x183)) <= 0:
            r = w.ext("w_2ac60") & 0xFFFFFFFF
            w.s32(0x229CB4, r)
            if _i32(r) >= 8:
                w.s8(G_SELECTED, w.g8(0x92DC7))
                w.s8(0x92DE1, 0)
                w.s8(0x92DE0, 0)
                w.ext("w_67e30")
                if w.ext("mouse_in_rect", w.g32(0x229CB8), w.g32(0x229CC4), w.g32(0x229CD8), w.g32(0x229CDC)):
                    w.s8(0x284F13, 1)
                else:
                    w.ext("w_673ac")
                return 0
        if w.g8(G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR) + 2):
            if focus:
                w.ext("w_124fc")
                w.ext("focus", w.g8(0x229CB4), 0)
                w.s32(0x229C94, 0x40)
            w.ext("w_1148c", w.g32(0x229CB4), 0, 0)
            if w.g8(G_SELECTED) < 8:
                w.ext("item_panel", w.g8(G_SELECTED), 0, 1)
            if focus:
                w.ext("w_1259c")
            w.ext("w_67e30")
            if w.ext("mouse_in_rect", w.g32(0x229CB8), w.g32(0x229CC4), w.g32(0x229CD8), w.g32(0x229CDC)):
                w.s8(0x284F13, 1)
            else:
                w.ext("w_673ac")
    elif w.g8(G_SELECTED) > 8:
        w.s8(G_SELECTED, 8)
        w.s8(0x284F1B, 0)
        w.s8(0x284F1D, 0xA1)
    w.s8(0x22A0CD, 1)
    w.s32(0x92E3C, 0)
    for i in range(5):
        b = G_STORY_TIMERS + 13 * i
        if w.g8(b):
            w.s8(b, w.g8(b) - 1)
    if w.g8(0x284F02):
        w.s8(0x284F02, w.g8(0x284F02) - 1)
    x = w.g8(0x92DD5)
    if (x < 0x40 and not w.g8(0x22A075 + w.g8(G_CUR_SECTOR)) and not w.g8(0x284F02) and w.ext("w_410c4") & 0xFFFFFFFF
            and not w.ext("w_1f6e4") & 0xFF and s8(w.g8(G_SOLDIERS + 0x200 * x + 0x183)) >= 15
            and w.g8(G_SOLDIERS + 0x200 * x + 0x138)):
        w.ext("w_297d4", x, s8(w.g8(G_CUR_SECTOR)) & 0xFFFFFFFF)
    if not w.g8(G_CUR_SECTOR) and w.g8(0x22A0C2) and w.g8(0x22A1F2) < 5:
        v = (w.g8(0x22A1F2) + 1) & 0xFF
        w.s8(0x22A1F2, v)
        if v < 4:
            w.ext("w_635dc")
        if w.g8(0x22A1F2) == 5:
            sec = w.g8(G_CUR_SECTOR)
            w.s8(G_SECTOR_TBL9 + 9 * sec + 1, w.g8(G_SECTOR_TBL9 + 9 * sec + 1) - 1)
            kill_stragglers(w)
    w.s8(0x92DE1, 0)
    w.s8(0x92DE0, 0)
    for k in range(0x28):
        slot = w.g8(G_SOLDIERS + 0x200 * k + 0x17A)
        if w.g8(G_BLED + slot):
            w.ext("w_17b70", slot, 3)
    if keep:
        w.ext("w_31550")
    if w.g8(0x22A1F3):
        w.s8(0x22A1F3, 0)
        w.s32(0x92DA4, 1)
    return 0



def itq_pop(w):
    """0x12C14 -> AL: takes the last usable entry of the interrupt queue (in play, hp 15+, turn not done), else 0x40.
    Free entries (0x40) are passed over without shortening the queue; an unusable soldier is dropped (its entry and
    the queue length shrink) and, when it was the first entry, returned anyway (sic)."""
    n = w.g8(G_ITQ_N)
    i = n
    r = 0x40
    while i > 0:
        r = w.g8(G_ITQ + i - 1)
        if r == 0x40:
            i -= 1
            if i <= 0:
                r = 0x40
            continue
        p = w.rec(r)
        n = (n - 1) & 0xFF
        w.s8(G_ITQ + n, 0x40)
        if p[0x182] and s8(p[0x183]) >= 15 and not p[0x1B0]:
            break
        i -= 1
        if i == 0:
            break
        if i < 0:
            r = 0x40
    w.s8(G_ITQ_N, n)
    return r


def next_turn(w, mode):
    """NextTurn? 0x12D58 (AL = mode): nothing while 0x92E38 > 0. The queued-interrupt flag 0x92DD0 clears; with no
    merc to show (0x92DC7 = 0x40) it becomes the selected merc (or 0x32828 for an enemy turn). A drawn tactical screen
    (0x92DCC) redraws (0x12CBC) and turns 0x92DCC off, 0x92DCD on. Mode 1: the next interrupter is selected and the
    turn banner shows (0x1054C); else it goes to 0x284F10."""
    if _i32(w.g32(0x92E38)) > 0:
        return
    w.s8(0x92DD0, 0)
    if w.g8(0x92DC7) == 0x40:
        sel = w.g8(G_SELECTED)
        w.s8(0x92DC7, sel if sel < 8 else w.ext("w_32828") & 0xFF)
    if w.g8(G_SHOW_UI):
        enemy_panel(w)
        w.s8(G_SHOW_UI, 0)
        w.s8(0x92DCD, 1)
    if s8(mode & 0xFF) == 1:
        w.s8(G_SELECTED, itq_pop(w))
        w.ext("turn_banner")
    else:
        w.s8(0x284F10, itq_pop(w))


BANNERS = ((0x9AB2D, 0x72), (0x9AB47, 0xA2), (0x9AB61, 0x72), (0x9AB7B, 0xA2), (0x9AC7F, 0x72))   # kind -> text, colour
BANNER_TEXTS = (b"OPPONENTS' TURN...", b"GUARDS' TURN...", b"OPPONENT INTERRUPTS!", b"GUARD INTERRUPTS!",
                b"SANTINO INTERRUPTS!")
G_BANNER_TEXT = 0x10E380     # the turn banner's text (shown by TacticalRedraw, kept in quick saves)
G_BANNER_COLOUR = 0x92DDE    # u8 its colour
G_FAST_TIMER = 0x92DB0       # i32 ms until turn-based (0x92DAC) comes back on (the 10 ms timer counts it down)


def banner_show(w, kind):
    """the banner part of 0x1054C: text `kind` (BANNER_TEXTS, with its 0) to 0x10E380, its colour to 0x92DDE"""
    t = BANNER_TEXTS[kind] + b"\0"
    for i, c in enumerate(t):
        w.s8(G_BANNER_TEXT + i, c)
    w.s8(G_BANNER_COLOUR, BANNERS[kind][1])


def turn_start(w):
    """ShowTurnBanner? 0x1054C: the selected soldier's turn starts. Not in the sector / not in play / hp below 15:
    turn done (+0x1B0 = 1), nothing else. Turn done already or AI type 2: nothing. Else the banner (kind: an
    interrupt when the kept merc 0x284F0F is below 8, or when the interrupt queue holds someone (0x92DD1) and its
    first slot is on the other side; AI type below 3: guards (GUARD INTERRUPTS! / GUARDS' TURN...), type 3 OPPONENT
    INTERRUPTS!, else SANTINO INTERRUPTS!, no interrupt OPPONENTS' TURN...), +0x197 = 0; an enemy reacts (0x73578),
    a merc / guard below 16 reacts (0x7351C) and shares sightings (flag 0); UpdateAlertState; alert 2 or 3:
    +0x1C1 = 1, else AllyAlert 0x74A60. Then 15 s of real time (0x92DB0 = 15000, turn-based 0x92DAC = 0) and
    MercView (facing, range +0x19E, think 1)."""
    s = w.rec(w.g8(G_SELECTED))
    if not s[0x182] or not s[0x138] or s8(s[0x183]) < 15:
        s[0x1B0] = 1
        return
    if s[0x1B0] or s8(s[0xBA]) == 2:
        return
    intr = w.g8(0x284F0F) < 8
    if w.g8(0x92DD1):
        q = w.g8(G_ITQ)
        if (q >= 0x28 and s[0x17A] < 0x28) or (q < 0x28 and s[0x17A] >= 0x28):
            intr = True
    t = s8(s[0xBA])
    kind = (3 if intr else 1) if t < 3 else (2 if t == 3 else 4) if intr else 0
    banner_show(w, kind)
    s[0x197] = 0
    if s[0x17A] >= 0x28:
        w.ext("enemy_turned", s)
    elif s[0x17A] < 0x10:
        w.ext("guard_turned", s)
        w.ext("share_sightings", s, 0)
    update_alert(s, w)
    if s8(s[0xBB]) in (2, 3):
        s[0x1C1] = 1
    else:
        ally_alert(s, w)
    w.s32(G_FAST_TIMER, 15000)
    w.s32(G_TURN_BASED, 0)
    reveal_view(s[0x1A1], s[0x19E], s, w, 1)


def ally_alert(s, w):
    """AllyAlert? 0x74A60: the soldier looks at its own side (mercs and guards 0..15, enemies 0x28..0x2F; the records
    in slot order from the pointer table 0x100958): another one in the sector, in play and alive, at most 14 tiles away
    (TileDistance) and in sight (SightLine from x + 2, y + 4 to its tile, x + 2, y + 4, flag 1, range 14, its slot):
    alerted (alert 2+, +0x1C4 set or hp below 15): alert 2, BumpOrders, +0x1C1 = 1, done. Alert 1 while this one is
    below 1: alert 1, +0x1C1 = 1, call level +0x1C0 = 3, call tile +0x122 = its tile; the search goes on."""
    lo, hi = (0x28, 0x30) if s[0x17A] >= 0x28 else (0, 0x10)
    for k in range(lo, hi):
        if k == s[0x17A]:
            continue
        o = w.rec(k)
        if not (o[0x182] and o[0x138] and o[0x183]):
            continue
        if _i32(w.ext("tile_dist", s16(u16(s, 0x114)), s16(u16(o, 0x114)))) > 14:
            continue
        if not _i32(w.ext("sight_line", s, s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, s16(u16(o, 0x114)),
                          s16(u16(o, 0x116)) + 2, s16(u16(o, 0x118)) + 4, 1, 0xE, k)):
            continue
        a = s8(o[0xBB])
        if a >= 2 or o[0x1C4] or s8(o[0x183]) < 15:
            s[0xBB] = 2
            _bump_orders(s)
            s[0x1C1] = 1
            return
        if a == 1 and s8(s[0xBB]) < 1:
            s[0xBB] = 1
            s[0x1C1] = 1
            s[0x1C0] = 3
            w16(s, 0x122, u16(o, 0x114))


def enemy_panel(w):
    """RedrawTacticalPanel? 0x12CBC: with show_ai 0x92DCD set the pause turns into a shown enemies' turn: show_ai and the
    turn timer 0x22A486 clear, the kept merc 0x284F0F gets +0x1BD = 0x6A and frame picture 5, all buttons shade, and each
    merc in the sector and in play with +0x1B0 clear goes to duty 8 with a portrait redraw."""
    if not w.g8(0x92DCD):
        return
    w.s8(0x92DCD, 0)
    w.s16(0x22A486, 0)
    km = w.g8(0x284F0F)
    if km < 8:
        w.s8(G_SOLDIERS + km * 512 + 0x1BD, 0x6A)
        w.ext("w_46a4c", km, 5)
    w.ext("w_10454", 0x18)
    for k in range(8):
        r = G_SOLDIERS + k * 512
        if w.g8(r + 0x182) and w.g8(r + 0x138) and not w.g8(r + 0x1B0):
            w.s8(r + 0x17E, 8)
            w.ext("w_331a4", k, 1)


def deselect(w, show):
    """DeselectSoldier 0x17F10 (AL show): the selected merc (slot < 8) gets +0x1BD = 0x6A and frame picture 5, then selected =
    0x40, then his portrait redraw."""
    old = w.g8(G_SELECTED)
    if old < 8:
        w.s8(G_SOLDIERS + old * 512 + 0x1BD, 0x6A)
        w.ext("w_46a4c", old, 5)
        w.s8(G_SELECTED, 0x40)
        w.ext("w_331a4", old, show)


def select_soldier(w, slot, show, quote):
    """SelectSoldier? 0x17D80 (AL = slot, DL = show, BL = quote) -> 1 if the merc (slot 0..7, in the sector, state
    +0x17E 0) is selected, else 0. A new one: the old selected merc (below 8) gets +0x1BD = 0x6A, a redraw (0x46A4C,
    5), selected = 0x40 and its portrait refresh (0x331A4(old, show)); then selected = slot, +0x1BD = 0x6E, redraw
    (0x46A4C, 0xC); hp 15+: cursor state 0x284F1B = 1, cursor 0x284F1D = 0x8E, with quote 0x17B70(slot, 0), ReserveAp;
    else cursor 0xA1, state 0; portrait refresh (slot, show). With show: in view mode 3 (0x92DDF) the panel moves
    (0x24880(0, 0x14, 0xD), 0x92E30 = 3000, 0x92E36 = 1, the view scrolls 14 px down when 0x229D04 >= 0xB6 and the
    view stays inside 0x310 - 0x229CC0: g_viewY + 14, 0x229D04 - 14, 0x681D0(0x229D00, 0x229D04)); item panel
    (0x55FD8(s, 0, 1))."""
    slot &= 0xFF
    s = w.rec(slot)
    if slot > 7 or not s[0x182] or s[0x17E]:
        return 0
    old = w.g8(G_SELECTED)
    show_ = s8(show & 0xFF) & 0xFFFFFFFF          # (the byte, sign-extended)
    if slot != old:
        if old < 8:
            w.rec(old)[0x1BD] = 0x6A
            w.ext("w_46a4c", old, 5)
            w.s8(G_SELECTED, 0x40)
            w.ext("w_331a4", old, show_)
        w.s8(G_SELECTED, slot)
        s[0x1BD] = 0x6E
        w.ext("w_46a4c", slot, 0xC)
        if s8(s[0x183]) >= 15:
            w.s8(0x284F1B, 1)
            w.s8(0x284F1D, 0x8E)
            if quote & 0xFF:
                w.ext("w_17b70", slot, 0)
            w.ext("reserve_ap", s)
        else:
            w.s8(0x284F1D, 0xA1)
            w.s8(0x284F1B, 0)
        w.ext("w_331a4", slot, show_)
    if show & 0xFF:
        if w.g8(0x92DDF) == 3:
            w.ext("w_24880", 0, 0x14, 0xD)
            w.s32(0x92E30, 3000)
            w.s8(0x92E36, 1)
            b = _i32(w.g32(0x229D04))
            if b >= 0xB6:
                y = _i32(w.g32(G_VIEW_Y)) + 0xE
                if y < 0x310 - _i32(w.g32(0x229CC0)):
                    w.s32(G_VIEW_Y, y)
                    w.s32(0x229D04, b - 0xE)
                    w.ext("w_681d0", w.g32(0x229D00), (b - 0xE) & 0xFFFFFFFF)
        w.ext("item_panel", s, 0, 1)
    return 1


def select_merc(w, slot, force, quote):
    """SelectMerc? 0x1148C (EAX = slot, DL = force, BL = quote; op 120) -> 1 when the merc is selected. Nothing (0)
    unless the tactical screen is drawn (0x92DCC); a dead one (hp <= 0) gives 0. When it is the shown slot (0x229CB4)
    or the on-screen option 0x92DDF is off, on screen, and the view is not held (0x92DCE), or with force and the view
    not held: SelectSoldier(slot, 1, quote), cursor (0x1C464); selected: the queued merc quote (0x92DEC < 0x40: delay
    0x284F26 = 0, MercQuoteEvent), the "sees enemies" message (0x92DC6 < 0x40, a merc), marks off when shown
    (0x229DD8: 0x1D8B8(1)); a walk target +0x120 (not none, not the own tile) with +0x16D: the path preview (PathApCost
    mark = 1 when 0x92E58 is the slot or 0x92E64 is set, which clears) and, AP below the walk cost 0x229DF0: marks off
    (0x1D8B8(byte 0x229DD8)), +0x16D = 0. Else (not held): portrait refresh (slot, 1), focus (slot, 0); with 0x92DCB,
    +0x16D clear and such a walk target: +0x16D = 1, preview (mark 1), and AP below the walk cost: marks off, merc
    shown 0x92DC8 = 0x40, +0x16D = 0; then the mouse wait 0x67C54; result 0."""
    if not w.g8(G_SHOW_UI):
        return 0
    s = w.rec(slot & 0xFF if slot < 0x30 else slot)
    if s8(s[0x183]) <= 0:
        return 0
    held = w.g8(0x92DCE)
    if (((slot == w.g32(0x229CB4) or not w.g8(0x92DDF)) and on_screen(s, w) and not held)
            or (force & 0xFF and not held)):
        r = select_soldier(w, slot, 1, quote)
        w.ext("w_1c464", s)
        if not r:
            return r
        if w.g8(0x92DEC) < 0x40:
            w.s8(0x284F26, 0)
            merc_quote(s, w)
        if w.g8(0x92DC6) < 0x40 and s[0x17A] < 8:
            w.ext("w_11448", s)
        if w.g32(0x229DD8):
            w.ext("ui_refresh", 1)
        t = s16(u16(s, 0x120))
        if t != NO_TILE and u16(s, 0x120) != u16(s, 0x114) and s[0x16D]:
            mark = 0
            if w.g8(0x92E58) == slot or w.g8(G_SPOT_STOP):
                mark = 1
                w.s8(G_SPOT_STOP, 0)
            w.ext("path_ap", s, t, mark, mark, s8(s[0x1A8]), s8(s[0x177]), s8(s[0x196]))
            if s8(s[0x1A5]) < _i32(w.g32(G_WALK_AP)):
                w.ext("ui_refresh", s8(w.g8(0x229DD8)) & 0xFFFFFFFF)
                s[0x16D] = 0
        return r
    if held:
        return 0
    w.ext("w_331a4", slot & 0xFF, 1)
    w.ext("focus", slot & 0xFF, 0)
    t = s16(u16(s, 0x120))
    if w.g8(0x92DCB) and not s[0x16D] and t != NO_TILE and u16(s, 0x120) != u16(s, 0x114):
        s[0x16D] = 1
        w.ext("path_ap", s, t, 1, 1, s8(s[0x1A8]), s8(s[0x177]), s8(s[0x196]))
        if s8(s[0x1A5]) < _i32(w.g32(G_WALK_AP)):
            w.ext("ui_refresh", s8(w.g8(0x229DD8)) & 0xFFFFFFFF)
            w.s8(0x92DC8, 0x40)
            s[0x16D] = 0
    w.ext("w_67c54")
    return 0


def interrupt_turn(w):
    """InterruptTurn? 0x1304C: nothing while 0x92E38 > 0. The tactical screen is drawn again (0x92DCC = 1), 0x22A486 =
    20, 0x22A484 = 0, 0x92DCF / 0x92DCD = 0, 0x10398(1). Each merc / guard in play and alive set to state 8 (+0x17E):
    with its turn not done it leaves state 8, else +0x194 = 1; all but the interrupter (0x92DD2) get 0x331A4(slot, 1).
    The interrupter (0 when above 7) is shown: 0x229CB4, 0x93D54 = 5000, FocusOn, 0x92DCE = 0, 0x1148C(it, 1, 1),
    0x67E30."""
    if _i32(w.g32(0x92E38)) > 0:
        return
    w.s8(G_SHOW_UI, 1)
    w.s16(0x22A486, 0x14)
    w.s16(0x22A484, 0)
    w.s8(0x92DCF, 0)
    w.s8(0x92DCD, 0)
    w.ext("w_10398", 1)
    for k in range(16):
        p = w.rec(k)
        if p[0x182] and p[0x183] and p[0x17E] == 8:
            if not p[0x1B0]:
                p[0x17E] = 0
            else:
                p[0x194] = 1
            if w.g8(0x92DD2) != p[0x17A]:
                w.ext("w_331a4", p[0x17A], 1)
    v = w.g8(0x92DD2)
    if v > 7:
        w.s32(0x229CB4, 0)
        w.s8(0x92DD2, 0)
    else:
        w.s32(0x229CB4, v)
    w.s32(0x93D54, 5000)
    w.ext("focus", w.g8(0x229CB4), 0)
    w.s8(0x92DCE, 0)
    w.ext("w_1148c", w.g32(0x229CB4), 1, 1)
    w.ext("w_67e30")


def first_merc(w):
    """FirstMerc? 0x2AC60 -> the first merc k = 0..7 (pointer table 0x100958) in the sector (+0x182) with state 0 or 8
    (+0x17E) and hp != 0; else 0x40."""
    for k in range(8):
        p = G_SOLDIERS + 0x200 * k
        if w.g8(p + 0x182) and w.g8(p + 0x17E) in (0, 8) and w.g8(p + 0x183):
            return k
    return 0x40


def turn_sightings(w, both):
    """TurnSightings? 0x11E2C (AL = both sides): with AL, enemies 0x28..0x2F in play (+0x182, +0x138, hp 15+) react
    (0x73578). Mercs and guards 0..15 in play react (0x7351C) and share their sightings (flag 0); guards
    8..15 in play contest an interrupt (MercInterrupt 0x73D28) against each enemy 0x28..0x2F they remember as -1
    (0x10D9D0 row). With AL, enemies 0x28..0x2F in play contest (EnemyInterrupt 0x73F3C) against each soldier
    0..0x27 they remember as -1 (0x10D890 row). (Every loop reads the records in slot order.)"""
    def ok(k):
        p = G_SOLDIERS + 0x200 * k
        return w.g8(p + 0x182) and w.g8(p + 0x138) and s8(w.g8(p + 0x183)) >= 15
    if both & 0xFF:
        for k in range(0x28, 0x30):
            if ok(k):
                w.ext("enemy_turned", w.rec(k))
    for k in range(16):
        if ok(k):
            w.ext("guard_turned", w.rec(k))
            w.ext("share_sightings", w.rec(k), 0)
    for k in range(8, 16):
        if ok(k):
            for j in range(8):
                if w.g8(G_REMEMBER_M + 8 * k + j) == 0xFF:
                    w.ext("w_73d28", w.rec(k), w.rec(0x28 + j))
    if both & 0xFF:
        for k in range(8):
            if ok(0x28 + k):
                for j in range(0x28):
                    if w.g8(G_REMEMBER_E + 0x28 * k + j) == 0xFF:
                        w.ext("w_73f3c", w.rec(0x28 + k), w.rec(j))


def enemies_turn(w):
    """EnemiesTurn? 0x12404: the enemies' turn starts. Bleed marks cleared, 0x92DDC = 1; every enemy 0x28..0x2F in the
    sector (+0x182) with hp != 0 starts its turn (StartTurn 0x11FB4) and gets +0x9C = 0xFF. A purifier mission
    (g_missionStep > 0, alarm 0x22A0CE = 1) whose order 0x92DA8 has a tile but slot 0x40 is cancelled (0x72670).
    TurnSightings(0), AgeKnowledge(0x92DCC); selected = 8, turn banner. Enemies that bled this turn, are seen
    (+0x9C > 0, so never right after the 0xFF above) and on screen cough (0x17B70(slot, 3))."""
    _fill(w, G_BLED, 0, 0x30)
    w.s8(0x92DDC, 1)
    for k in range(0x28, 0x30):
        p = w.rec(k)
        if p[0x182] and p[0x183]:
            start_turn(p, w)
            p[0x9C] = 0xFF
    if (s8(w.g8(0x22A0D0)) > 0 and s8(w.g8(0x22A0CE)) == 1 and s16(w.g16(0x92DAA)) != NO_TILE
            and w.g8(0x92DA8) == 0x40):
        w.ext("mission_cancel")
    turn_sightings(w, 0)
    age_knowledge(w, w.g8(G_SHOW_UI))
    w.s8(G_SELECTED, 8)
    w.ext("turn_banner")
    for k in range(0x28, 0x30):
        p = w.rec(k)
        if w.g8(G_BLED + p[0x17A]) and s8(p[0x9C]) > 0 and on_screen(p, w):
            w.ext("w_17b70", p[0x17A], 3)


def end_turn(w):
    """EndTurn? 0x121D8 (the end-turn key): the five timed effects (G_FX, 13 B) that are active with their timer
    (+9) at 0 or below go off and free: on a map tile (+6 below 0xC41) the ground item there explodes (Explode(+0xB,
    tile, kind +8, 1)) if its blast kind is +8; otherwise 0x355D0(+8, +0xA, +4 word, +0xB, +0xC). The view targets
    clear (0x229CB4 = 0x229C94 = 0x40, 0x229C8C = 0xC41), a queued merc quote 0x92DEC < 0x40 restarts its delay
    (0x284F26 = 0). With the tactical screen drawn (0x92DCC): 0x22A0CD = 0, mouse off (0x67368), 0x92DCD = 0,
    byte 7 of every tile = 0, the shown merc 0x92DC7 = selected, 0x92DCC = 0, 0x22A486 = 0, kept merc 0x284F0F =
    0x40, 0x17F10(0), 0x10454(0x18); every merc 0..7 in play (+0x182, +0x138) and not dead (+0x188) goes to state 8
    (+0x17E), +0x194 = 0, turn done (+0x1B0 = 1), 0x331A4(slot, 1); 0x67BF0, 0x681D0(0x50, 8) when 0x92DDF < 3,
    0x67E18. Then the interrupt queue: a soldier below 0x40 is selected with the turn banner; none: EnemiesTurn when
    0x92DDC = 0, else selected = 8 and the banner."""
    for i in range(5):
        f = G_FX + 13 * i
        if _i32(w.g32(f)) < 0 or s8(w.g8(f + 9)) > 0:
            continue
        tile = s16(w.g16(f + 6))
        w.s32(f, 0xFFFFFFFF)
        if tile < NO_TILE:
            if item_index(w, tile) == 0xFF:
                continue
            k = explosive_kind(w.g8(G_GROUND_ITEMS + 12 * item_index(w, tile)))
            if k == w.g8(f + 8):
                w.ext("explode", w.g8(f + 0xB), tile & 0xFFFFFFFF, k, 1)
        else:
            w.ext("w_355d0", w.g8(f + 8), s8(w.g8(f + 0xA)) & 0xFFFFFFFF, s16(w.g16(f + 4)) & 0xFFFFFFFF,
                  w.g8(f + 0xB), s8(w.g8(f + 0xC)) & 0xFFFFFFFF)
    w.s32(0x229CB4, 0x40)
    w.s32(0x229C94, 0x40)
    w.s32(0x229C8C, NO_TILE)
    if w.g8(0x92DEC) < 0x40:
        w.s8(0x284F26, 0)
    if not w.g8(G_SHOW_UI):
        return
    w.s8(0x22A0CD, 0)
    w.ext("mouse_hide")
    w.s8(0x92DCD, 0)
    for t in range(3136):
        w.s8(G_TILEMAP + 8 * t + 7, 0)
    w.s8(0x92DC7, w.g8(G_SELECTED))
    w.s8(G_SHOW_UI, 0)
    w.s16(0x22A486, 0)
    w.s8(0x284F0F, 0x40)
    deselect(w, 0)
    w.ext("w_10454", 0x18)
    for k in range(8):
        p = w.rec(k)
        if p[0x182] and p[0x138] and not p[0x188]:
            p[0x17E] = 8
            p[0x194] = 0
            p[0x1B0] = 1
            w.ext("w_331a4", p[0x17A], 1)
    w.ext("w_67bf0")
    if w.g8(0x92DDF) < 3:
        w.ext("w_681d0", 0x50, 8)
    w.ext("w_67e18")
    r = itq_pop(w)
    if r < 0x40:
        w.s8(G_SELECTED, r)
    elif not w.g8(0x92DDC):
        enemies_turn(w)
        return
    else:
        w.s8(G_SELECTED, 8)
    w.ext("turn_banner")


def mercs_turn(w):
    """MercsTurn? 0x12DD4 (the turn hand-over gives the turn to a merc): 0x22A486 = 20 and 0x22A484 = 0 unless
    0x22A0CD; the tactical screen is drawn (0x92DCC = 1). After a merc's interrupt (0x92DCD set): 0x92DCD = 0, a kept
    merc 0x284F0F below 8 becomes selected; the selected soldier's +0x1A6 = +0x95, +0x134 = +0x96; with a walk
    target +0x120 (not 0xC41, not its own tile) the walk is priced (PathApCost(tile, 0, 0; +0x1A8, +0x177, +0x196))
    and goes on (MoveMercTo(tile, 0, Shift 0x284F21; +0x177, Alt 0x284F20, 0, 0)) when the AP left reach G_WALK_AP.
    Else (a new merc turn): 0x10398(1); mercs 0..7 in the sector in state 8 leave it if their turn is not done (else
    +0x194 = 1), all but the kept merc get 0x331A4(k, 1). The merc to show: the kept merc when below 8 and the
    selected soldier is in the sector with hp 10+ (that soldier: +0x1B0 = 0, +0x16D = 1 when its walk target is set and
    not its tile; the kept merc then clears), else the first merc (0x2AC60); none at all: EndTurn. 0x92E52 = it,
    FocusOn(it, 0), an enemy number (8+) becomes the first merc, 0x92DCA = 1, 0x92DCE = 0, 0x1148C(it, 1, 1) = 0:
    selected = the first merc."""
    if not w.g8(0x22A0CD):
        w.s16(0x22A486, 0x14)
        w.s16(0x22A484, 0)
    w.s8(G_SHOW_UI, 1)
    if w.g8(0x92DCD):
        w.s8(0x92DCD, 0)
        if w.g8(0x284F0F) < 8:
            w.s8(G_SELECTED, w.g8(0x284F0F))
        p = w.rec(w.g8(G_SELECTED))
        p[0x1A6] = p[0x95]
        w16(p, 0x134, u16(p, 0x96))
        tile = s16(u16(p, 0x120))
        if tile == NO_TILE or u16(p, 0x114) == u16(p, 0x120):
            return
        w.ext("path_ap", p, tile & 0xFFFFFFFF, 0, 0, s8(p[0x1A8]), s8(p[0x177]), s8(p[0x196]))
        if s8(p[0x1A5]) < _i32(w.g32(0x229DF0)):
            return
        w.ext("move_merc", p, tile & 0xFFFFFFFF, 0, s8(w.g8(0x284F21)), s8(p[0x177]), s8(w.g8(0x284F20)), 0, 0)
        return
    w.ext("w_10398", 1)
    for k in range(8):
        p = w.rec(k)
        if p[0x182] and p[0x17E] == 8:
            if not p[0x1B0]:
                p[0x17E] = 0
            else:
                p[0x194] = 1
            if w.g8(0x284F0F) != p[0x17A]:
                w.ext("w_331a4", k, 1)
    w.s32(0x229CB4, 0x40)
    kept = w.g8(0x284F0F)
    if kept < 8:
        p = w.rec(w.g8(G_SELECTED))
        if p[0x182] and s8(p[0x183]) >= 10:
            w.s32(0x229CB4, kept)
            p[0x1B0] = 0
            if s16(u16(p, 0x120)) != NO_TILE and u16(p, 0x120) != u16(p, 0x114):
                p[0x16D] = 1
        w.s8(0x284F0F, 0x40)
    if w.g32(0x229CB4) == 0x40:
        v = first_merc(w)
        w.s32(0x229CB4, v)
        if v >= 0x40:
            end_turn(w)
            return
    v = w.g32(0x229CB4)
    w.s8(0x92E52, v & 0xFF)
    w.ext("focus", v & 0xFF, 0)
    if _i32(w.g32(0x229CB4)) >= 8:
        w.s32(0x229CB4, first_merc(w))
    w.s8(0x92DCA, 1)
    w.s8(0x92DCE, 0)
    if not w.ext("w_1148c", w.g32(0x229CB4), 1, 1) & 0xFFFFFFFF:
        w.s8(G_SELECTED, first_merc(w))



G_WEATHER_TIRE = 0x22A100      # i32 fatigue modifier from the conditions (a fifth of it for some soldiers)
G_MERC_TIRE = 0x22A059         # i8 fatigue modifier for mercs and guards


def fatigue_scale(s, w, amount):
    """0x32AD8 (EAX = soldier, DX = amount) -> fatigue actually added: factor = (200 - energy) + (100 - ((hpMax - hp
    - wounds) / 2 + hp)) + the conditions modifier 0x22A100 (a fifth of it for a class-3 soldier, on ground 0x97+ or
    with +0x19C = 7) + 0x22A059 for slots below 0x28, x5 when swimming (+0x19C 5 in animation 0x39 / 0x3A / 0x3C);
    16-bit. Positive amounts scale by factor / 100, others by 100 / factor."""
    c = (200 - s8(s[0x18A])) & 0xFFFF
    t = ((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + s8(s[0x183])
    c = (s16(c) + 100 - t) & 0xFFFFFFFF
    if s[0x1B3] == 3 or s[0x1B2] >= 0x97 or s[0x19C] == 7:
        c = s16(c & 0xFFFF) + cdiv(_i32(w.g32(G_WEATHER_TIRE)), 5)
    else:
        c = (c + w.g16(G_WEATHER_TIRE)) & 0xFFFF
    if s[0x17A] < 0x28:
        c = (c + (s8(w.g8(G_MERC_TIRE)) & 0xFFFF)) & 0xFFFFFFFF
    if s[0x19C] == 5 and s16(u16(s, 0x1E8 + 2 * s[0x1F3])) in (0x3A, 0x39, 0x3C):
        c = c * 5
    c = s16(c & 0xFFFF)
    a = s16(amount & 0xFFFF)
    return cdiv(a * c, 100) if a > 0 else cdiv(a * 100, c)


def spend_ap(s, w, ap, tire):
    """SpendAP? 0x329BC (EAX = soldier, DL = AP, BX = fatigue; was named SoldierSound?): nothing for a dead or
    unconscious soldier. AP (not 0 / 99): at most the AP left, except for the selected soldier in an animation
    without the AP check (0x93095); in combat the queued-aim reserve +0x1E2 pays first, the rest comes off +0x1A5;
    the selected soldier never goes below 0. A non-zero fatigue is scaled (0x32AD8). Then the panel: 0x32EB8(slot, 0)
    when +0x181 is 0, else (in play and alive) 0x328A4; and Rest(-fatigue, 1)."""
    if s8(s[0x183]) <= 0 or s[0x188]:
        return
    d = ap & 0xFF
    slot = s[0x17A]
    if s8(d) != 0x63 and d:
        if slot != w.g8(G_SELECTED) or w.g8(ANIM_FREE + u16(s, 0x1E8 + 2 * s[0x1F3])):
            if s8(d) > s8(s[0x1A5]):
                d = s[0x1A5]
        if w.g8(G_COMBAT):
            r = s[0x1E2]
            if s8(d) <= s8(r):
                s[0x1E2] = (r - d) & 0xFF
            else:
                s[0x1E2] = 0
                s[0x1A5] = (s[0x1A5] - ((d - r) & 0xFF)) & 0xFF
        if s8(s[0x1A5]) < 0 and slot == w.g8(G_SELECTED):
            s[0x1A5] = 0
    t = tire & 0xFFFF
    if t:
        t = fatigue_scale(s, w, t) & 0xFFFFFFFF
    if not s[0x181]:
        w.ext("w_32eb8", slot, 0)
    else:
        if not s[0x182] or not s[0x183] or not s[0x138]:
            return
        w.ext("w_328a4", s)
    w.ext("rest", s, (-s16(t & 0xFFFF)) & 0xFFFFFFFF, 1)



G_STAT_PROGRESS = 0x22A15A     # i16[9] per merc slot 0..7: progress per stat 1..9 (hpMax, agi, dex, wis, ..., level)
STAT_FIELDS = (0x184, 0x18B, 0x18C, 0x18D, 0x18E, 0x18F, 0x190, 0x191, 0x192)


def stat_practice(s, w, stat, n, flag):
    """StatPractice 0x62A68 (EAX = soldier, DL = stat 1..9, BX = rolls, CL = flag): mercs only (slot < 8) with a
    practice mode (+0x19B; mode 2 ignores flagged practice). Without the flag every stat but 1 rolls three times as
    often. Mode 1 (gain), per roll: chance = 100 - (progress / 100 + stat), for the level (9) 100 - (progress /
    (level x 1000) + level) x 10; when positive scaled by (wisdom progress / 100 + wisdom - 50) %, marksmanship (8)
    a fifth in state 6, at most 99; Random(100) below it: +1. Other modes (loss): chance = progress / 100 + stat
    (- 1 for stats 1..4; level (progress / (level x 1000) + level - 1) x 10), reduced by the same wisdom %, at
    least 1; Random(100) below it: -1. The total goes to the stat's progress and, unflagged and not for the level,
    to the level's progress too."""
    slot = s[0x17A]
    stat &= 0xFF
    flag &= 0xFF
    if slot > 7 or not s[0x19B]:
        return
    if flag and s[0x19B] == 2:
        return
    n &= 0xFFFF
    if not flag and stat != 1:
        n = (n * 3) & 0xFFFF
    prog = G_STAT_PROGRESS + 18 * slot
    if not 1 <= stat <= 9:
        return
    field, pw = STAT_FIELDS[stat - 1], prog + 2 * (stat - 1)
    wp = s16(w.g16(prog + 6))
    delta = 0
    for _ in range(max(s16(n), 0)):
        p = s16(w.g16(pw))
        v = s[field]
        bonus = cdiv(wp, 100) + s8(s[0x18D]) - 50
        if s[0x19B] == 1:
            if stat != 9:
                c = 100 - (cdiv(p, 100) + v)
            else:
                c = 100 - (cdiv(p, v * 1000) + v) * 10
            c = s16(c & 0xFFFF)
            if c > 0:
                c = s16((c + cdiv(bonus * c, 100)) & 0xFFFF)
            if stat == 8 and s[0x19C] == 6:
                c = cdiv(c, 5)
            if c > 99:
                c = 99
            if w.random(100) < c:
                delta += 1
        else:
            if stat == 9:
                c = (cdiv(p, v * 1000) + v - 1) * 10
            elif stat <= 4:
                c = cdiv(p, 100) + v - 1
            else:
                c = cdiv(p, 100) + v
            c = s16(c & 0xFFFF)
            if c > 0:
                c = s16((c - cdiv(c * bonus, 100)) & 0xFFFF)
                if c < 1:
                    c = 1
            if w.random(100) < c:
                delta -= 1
    w.s16(pw, w.g16(pw) + delta)
    if stat != 9 and not flag:
        w.s16(prog + 0x10, w.g16(prog + 0x10) + delta)



# fatigue per spare AP at the turn start, by animation 0..0x3F (0x32D04's switch; 0x40+ = -100): negative rests
TURN_REST = [-200] * 11 + [-100, -100, -200, -100, -50, -200, -100, -100, -100, -100, -50, -100, -100, -100, -100,
             -50, -50, -100, -100, -50, -50, -100, -100, -50, -50, -100, -100, -100, -100, -200, -100, -100, -200,
             -100, -50, -100, -200, -100, -50, -50] + [-200] * 6 + [25, 50, -100, 50, 100, 100, 100]


def turn_rest(s, w):
    """TurnRest? 0x32D04 (start of turn): with more than 5 AP left, n = AP - 5 spare points change fatigue by the
    animation's rate (TURN_REST): a positive rate tires by (25 - n) x rate; a negative one rests by n x rate unless
    +0x1DB is set or the tile is gassed without a gas mask (item 0x28 in entry 3). Through SpendAP (0 AP)."""
    ap = s8(s[0x1A5])
    if ap <= 5:
        return
    n = ap - 5
    anim = u16(s, 0x1E8 + 2 * s[0x1F3])
    v = TURN_REST[anim] if anim <= 0x3F else -100
    if v >= 0:
        f = (25 - n) * v
    else:
        if s[0x1DB]:
            return
        if w.tile8(s16(u16(s, 0x114)), 5) & 0x80 and s[0x24] != 0x28:
            return
        f = n * v
    w.ext("spend_ap", s, 0, s16(f & 0xFFFF) & 0xFFFFFFFF)


def merc_sees(w, how, seen, near):
    """MercSees? 0x56D28 (AL, DL, BL): stores them in 0x229E00 / 0x229E01 / 0x229E02 for the merc's reaction"""
    w.s8(0x229E00, how)
    w.s8(0x229E01, seen)
    w.s8(0x229E02, near)


def drop_item(w, addr):
    """DropItem 0x470B0 (EAX = item entry): clears the 12-byte entry"""
    for k in range(12):
        w.s8(addr + k, 0)


def use_throw(w, addr):
    """UseThrow? 0x535E8 (EAX = item entry): one thrown item leaves the stack: a non-stacking type (+6 = 1) or the last
    one clears the entry; else the next stacked condition (+8) becomes the entry's, the rest move down one and the
    count drops by one."""
    cnt = s8(w.g8(addr + 1))
    if cnt <= 0:
        return
    if w.g8(ITEM_TBL + 12 * w.g8(addr) + 6) == 1 or cnt == 1:
        drop_item(w, addr)
        return
    w.s8(addr + 2, w.g8(addr + 8))
    for k in range(1, cnt - 1):
        w.s8(addr + 7 + k, w.g8(addr + 8 + k))
    w.s8(addr + cnt + 6, 0)
    w.s8(addr + 1, w.g8(addr + 1) - 1)


def mercs_alert(w):
    """0x3A2D4 (an enemy died): every merc in play with hp 15+, standing (animation 4+) and on the map gets +0x9D = 1"""
    for k in range(8):
        a = G_SOLDIERS + 0x200 * k
        if (w.g8(a + 0x182) and s8(w.g8(a + 0x183)) >= 15 and w.g16(a + 0x1E8 + 2 * w.g8(a + 0x1F3)) >= 4
                and w.g8(a + 0x138)):
            w.s8(a + 0x9D, 1)


G_ACTIVE_SLOT = 0x284F28      # u8 slot whose action runs (= G_ACTIVE)


def fire_now(s, w):
    """FireNow? 0x57B34 (AL = slot; AiThink's timeout): ends the attack: a running attack / use action (11, 12, 14,
    16) is cleared (action, target, running flag), the attack tile +0xDC = none, nobody acts (0x284F28 = 0x40), then
    the turn moves on (0x12D58(1) when 0x92DD0 is set, else 0x1304C when 0x92DCF is set)."""
    if s[0x1D6] and s[0x1D1] in (0xB, 0xC, 0xE, 0x10):
        s[0x1D1] = 0
        w16(s, 0x132, NO_TILE)
        s[0x1D6] = 0
    w16(s, 0xDC, NO_TILE)
    w.s8(G_ACTIVE_SLOT, 0x40)
    if w.g8(0x92DD0):
        w.ext("next_turn", 1)
    elif w.g8(0x92DCF):
        w.ext("w_1304c")


def stop_anim(s, w, flag):
    """StopAnim 0x196F8: the walk ends here: walk target none, +0x1F6 = 0, step target = own tile. With flag (DL)
    also the stored target +0x120 none, +0x1A8 / +0x196 = 0, the step sound stops (0x336B4), and a running AI walk
    (actions 2, 3, 4, 5, 8, 13; action 14 only without a task +0x1A6) is ended. Without flag a task +0x1A6 is kept
    for later in +0x95 / +0x96 (its target +0x134)."""
    w16(s, 0x12A, NO_TILE)
    s[0x1F6] = 0
    w16(s, 0x124, u16(s, 0x114))
    if not flag & 0xFF:
        if s[0x1A6]:
            s[0x95] = s[0x1A6]
            w16(s, 0x96, u16(s, 0x134))
        return
    w16(s, 0x120, NO_TILE)
    s[0x1A8] = s[0x196] = 0
    w.ext("step_sound", s[0x17A], 1)
    if s[0x1D1] and s[0x1D6] and not (s[0x1D1] == 0xE and s[0x1A6]):
        if s[0x1D1] in (2, 3, 4, 5, 8, 0xD):
            s[0x1D1] = 0
            w16(s, 0x132, NO_TILE)
            s[0x1D6] = 0


def snap_to_tile(s, w, flag):
    """0x197D8: step target point = the own tile's point (x + 5, y + 5), then StopAnim(flag)"""
    t = s16(u16(s, 0x114))
    w16(s, 0x126, tile_x(t))
    w16(s, 0x128, tile_y(t))
    w.ext("stop_anim", s, s8(flag & 0xFF) & 0xFFFFFFFF)   # DL sign-extended


G_BUSY_UI = 0x92DCE           # u8: the selected merc's "busy" display is on (turn-based)


def stop_walk(s, w, flag):
    """StopWalk 0x104C8 (mercs only, the selected one, in combat mode): flag (DL) shows the busy display once (text
    0x13 via 0x254F0, 0x92DCA = 0); without flag it is taken down again (0x10310)."""
    if s[0x17A] >= 8:
        return
    on = w.g8(G_BUSY_UI)
    if flag & 0xFF:
        if not on and w.g8(G_COMBAT) and s[0x17A] == w.g8(G_SELECTED):
            w.s8(G_BUSY_UI, 1)
            w.ext("ui_text", 0x13, None, 0x140, on)
            w.s8(0x92DCA, on)
    elif on and w.g8(G_COMBAT) and s[0x17A] == w.g8(G_SELECTED):
        w.ext("ui_10310", None)
        w.s8(G_BUSY_UI, 0)


ANIM_STILL = 0x93165          # u8 per animation x 8 (column -1 of ANIM_CAN_ACT): != 0 = the soldier does not walk in it
G_SPLASH = 0x92E76            # s8 per sector: n for the Random(n) = 0 chance of a splash per step on water


def walk_move(s, w):
    """0x1A048: the point (+0x116, +0x118) moves toward the step target point (+0x126, +0x128) by the speed +0x19F on
    each axis, never past it. Result = ViewTile of the new point, x + 5 at facing 2, x + 1 at facing 4, y + 3 at
    facing 6 (facing +0x1A2)."""
    from combat import view_tile
    face = s[0x1A2]
    x, y = s16(u16(s, 0x116)), s16(u16(s, 0x118))
    tx, ty = s16(u16(s, 0x126)), s16(u16(s, 0x128))
    v = s[0x19F]
    if x < tx:
        x = min(x + v, tx)
    elif x > tx:
        x = max(x - v, tx)
    if y < ty:
        y = min(y + v, ty)
    elif y > ty:
        y = max(y - v, ty)
    w16(s, 0x116, x & 0xFFFF)
    w16(s, 0x118, y & 0xFFFF)
    if face == 2:
        x += 5
    elif face == 4:
        x += 1
    elif face == 6:
        y += 3
    return _i32(view_tile(s16(x & 0xFFFF), s16(y & 0xFFFF)))


def walk_step(s, w):
    """0x1A108: one frame of a walk. +0x193 = 1. A walking animation (ANIM_STILL 0) moves the point (walk_move) and
    gives the tile under it; in a still one the tile stays, and a code < 4 with s8 +0x1A5 < 5 ends the walk (+0x1B8 =
    1, StopWalk(0); s8 +0xBA >= 1: +0x1B0 = 1). Same tile: AnimNext. An occupied new tile: point and speed back,
    0x19818(tile, occupant). A free one: merc tired-walk message once, VacateTile, tile + its bytes and classes,
    AnimDelay, a +0x1F6 target farther than 8 drops +0x1F8, +0x1AE = 0, +0x1F2 + 1, AnimNext, focus on the selected
    soldier, then on open water a Random splash (animation 0x3D; a merc cries "BAD")."""
    s[0x193] = 1
    old = s16(u16(s, 0x114))
    ox, oy, speed = u16(s, 0x116), u16(s, 0x118), s[0x19F]
    code = s[0x1E8 + 2 * s[0x1F3]]
    if w.g8(ANIM_STILL + 8 * code) == 0:
        new = walk_move(s, w)
    else:
        new = old
        if code < 4 and s8(s[0x1A5]) < 5:
            s[0x1B8] = 1
            w.ext("stop_walk", s, 0)
            if s8(s[0xBA]) >= 1:
                s[0x1B0] = 1
    if new == old:
        w.ext("anim_next", s)
        return
    occ = _occupant(w, new)
    if occ != 0x40:
        w16(s, 0x116, ox)
        w16(s, 0x118, oy)
        s[0x19F] = speed
        w.ext("walk_blocked", s, new, occ)
        return
    a = u16(s, 0x1E8 + 2 * s[0x1F3])
    if s[0x17A] < 8 and s8(s[0x18A]) < 0x14 and not s[0x1BA] and 0x15 <= a <= 0x17 and not s[0x1D5]:
        s[0x1BA] = 1
        w.ext("merc_message", s, 0x1E, 0x40, None, 1)
    vacate(w, s16(u16(s, 0x114)))
    w16(s, 0x114, new & 0xFFFF)
    s[0x1B2] = w.tile8(new, 0)
    s[0x1B3] = w.g8(G_T0_CLASS + s[0x1B2])
    s[0x1B4] = w.tile8(new, 1)
    s[0x1B5] = w.g8(G_T1_CLASS_TBL + s[0x1B4])
    s[0x1B6] = w.tile8(new, 2)
    s[0x1B7] = w.g8(G_T2_CLASS + s[0x1B6])
    anim_delay(s, w)
    far = s16(u16(s, 0x1F8))
    if far != NO_TILE and _i32(w.ext("tile_dist", s16(u16(s, 0x114)), far)) > 8:
        w16(s, 0x1F8, NO_TILE)
    s[0x1AE] = 0
    s[0x1F2] = (s[0x1F2] + 1) & 0xFF
    w.ext("anim_next", s)
    sel = w.g8(G_SELECTED)
    if sel == s[0x17A] and (s8(s[0x9C]) > 0 or w.g8(0x93458)) and w.g8(G_COMBAT) and (s[0x17A] >= 8 or w.g8(0x1163C2)):
        w.ext("focus", sel, 1)
    if s[0x1B2] < 0x97 or s[0x1B4] or s8(s[0x183]) <= 0:
        return
    if w.random(s8(w.g8(G_SPLASH + w.g8(G_CUR_SECTOR)))):
        return
    if (u16(s, 0x1E8 + 2 * s[0x1F3]) >= 0x3C or s[0x179] >= 0x3C) and w.random(5) != 1:
        return
    w.ext("set_anim", s, 0x3D, 0, 1)
    if s[0x179] < 0x40:
        w.ext("focus", s[0x17A], 1)
        w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
        w.ext("voc", 0x2B11, 0x32, 1)


def walk_blocked(s, w, tile, occ):
    """0x19818 (from walk_step): the next tile `tile` holds occupant `occ`. "Stop" = step target point = the own tile's
    point, then StopAnim(flag). In combat: the step target (+0x124) blocked -> stop(1) and AnimStep, any other
    -> stop(0). Out of combat, no occupant (0x40): a target the soldier may not stand on -> stop(1) if `tile` is the
    target or FindPath(target, 1) fails, else StartLeg; a standable one: +0x12A = it, StartLeg. An occupant: a
    target it may not stand on within 3 tiles -> stop(1); else +0x1AE + 1, past the patience (mercs 50, others 0)
    -> stop(1); an occupant (< 0x40) still short of its own step target waits; at it: `tile` = the target -> stop(1),
    else +0x12A = it, StartLeg."""
    def stop(flag):
        t = s16(u16(s, 0x114))
        w16(s, 0x126, tile_x(t))
        w16(s, 0x128, tile_y(t))
        w.ext("stop_anim", s, flag)
    target = s16(u16(s, 0x124))
    if w.g8(G_COMBAT):
        if tile == target:
            stop(1)
            w.ext("anim_step", s)
        else:
            stop(0)
        return
    if occ == 0x40:
        if tile_standable(w, target) != 1:
            if tile == target or not w.ext("find_path", s, target, 1):
                stop(1)
            else:
                start_leg(s, w)
            return
    else:
        if tile_standable(w, target) < 1 and _i32(w.ext("tile_dist", s16(u16(s, 0x114)), target)) < 4:
            stop(1)
            return
        s[0x1AE] = (s[0x1AE] + 1) & 0xFF
        if (0x32 if s[0x17A] < 8 else 0) < s[0x1AE]:
            stop(1)
            return
        if occ >= 0x40 or not at_waypoint(w.rec(occ)):
            return
        if tile == target:
            stop(1)
            return
    w16(s, 0x12A, u16(s, 0x124))
    start_leg(s, w)


def stand_up(s, w):
    """0x1B188 (per-frame update of a still soldier): a lying soldier stands up. Who: with 0x92DCC set, slots below
    0x28 (only mercs in combat); with it clear, slots 8 and up. Then, in animation < 3 with no task (+0x1B9 = 0): +0xD0 = none; with energy
    (s8 +0x18A) >= 10, animation <= 1 and AP (s8 +0x1A5) >= 5: +0x1CC = 1, SetAnim(4, 0, 0), step target point = the
    own tile's point, StopAnim(1), +0x1B8 = 0, re-think (+0x1C1 = 1)."""
    if w.g8(0x92DCC):
        if (w.g8(G_COMBAT) and s[0x17A] >= 8) or s[0x17A] >= 0x28:
            return
    elif s[0x17A] < 8:
        return
    a = u16(s, 0x1E8 + 2 * s[0x1F3])
    if a >= 3 or s[0x1B9]:
        return
    w16(s, 0xD0, NO_TILE)
    if s8(s[0x18A]) < 10 or a > 1 or s8(s[0x1A5]) < 5:
        return
    s[0x1CC] = 1
    w.ext("set_anim", s, 4, 0, 0)
    t = s16(u16(s, 0x114))
    w16(s, 0x126, tile_x(t))
    w16(s, 0x128, tile_y(t))
    w.ext("stop_anim", s, 1)
    s[0x1B8] = 0
    s[0x1C1] = 1


def walk_end(s, w, code):
    """0x1B8AC (DL = the animation byte): the walk ends. Out of energy (s8 +0x18A <= 0) in an animation 4..0x31: path
    marks off (0x1D8B8 with 0x229DD8 != 0), SetAnim(0x2B). Else at the stored target +0x120 (or none): code < 0x32 ->
    SetAnim(4) when +0xDC is none; code >= 0x32 -> the water stand animation by the ground +0x1B2 (0x33 below 0x97,
    0x36 below 0x9A, else 0x39). Then StopAnim(1 at the stored target or none, else 0), StopWalk(0), +0x1B8 = 1, an
    enemy (slot >= 0x28) marks G_SLOT_T34, a non-merc with +0x197 ends its turn (+0x1B0 = 1)."""
    code &= 0xFF
    if s8(s[0x18A]) <= 0 and 4 <= code < 0x32:
        w.ext("ui_refresh", int(w.g32(0x229DD8) != 0))
        w.ext("set_anim", s, 0x2B, 0, 0)
    elif u16(s, 0x114) == u16(s, 0x120) or s16(u16(s, 0x120)) == NO_TILE:
        if code < 0x32:
            if s16(u16(s, 0xDC)) == NO_TILE:
                w.ext("set_anim", s, 4, 0, 0)
        else:
            g = s[0x1B2]
            w.ext("set_anim", s, 0x33 if g < 0x97 else 0x36 if g < 0x9A else 0x39, 0, 0)
    flag = 1 if u16(s, 0x120) == u16(s, 0x114) or s16(u16(s, 0x120)) >= NO_TILE else 0
    w.ext("stop_anim", s, flag)
    w.ext("stop_walk", s, 0)
    s[0x1B8] = 1
    if s[0x17A] >= 0x28:
        w.s8(G_SLOT_T34 + s[0x17A], 1)
    if s[0x17A] >= 8 and s[0x197]:
        s[0x1B0] = 1


def frame_checks(s, w):
    """0x1B504 (per-frame update of a still soldier): out of energy (s8 +0x18A <= 0) in an animation 4..0x27 and
    +0x186 = 0 -> path marks off (0x1D8B8, 0x229DD8 != 0), EndTask(1), SetAnim(0x2B). Timer +0xB4 at 0 with option
    0x22A02E on, an empty gun in hand (item 0 type > 0, count +1 = 0), +0x1BF = 0, MsgReloaded? != 0 and +0x1CD = 0
    -> quote event (slot, 5, 1, none, 1), +0x1CD = 1, +0xB4 = -1. Then MercQuoteEvent? 0x1B324."""
    code = s[0x1E8 + 2 * s[0x1F3]]
    if s8(s[0x18A]) <= 0 and 4 <= code < 0x28 and not s[0x186]:
        w.ext("ui_refresh", int(w.g32(0x229DD8) != 0))
        w.ext("end_task", s, 1)
        w.ext("set_anim", s, 0x2B, 0, 0)
    if not int.from_bytes(s[0xB4:0xB8], "little") and w.g8(0x22A02E) and not s[1] and s[0] and not s[0x1BF] \
            and _i32(w.ext("reloaded", s)) and not s[0x1CD]:
        w.ext("quote_event", s[0x17A], 5, 1, NO_TILE, 1)
        s[0x1CD] = 1
        _w32(s, 0xB4, 0xFFFFFFFF)
    merc_quote(s, w)



def soldier_frame(s, w):
    """0x1B694 (EAX = soldier): the per-frame update of a soldier that is not the active one (TacticalLoop, timer
    +0xB0 run out). The animation code (low byte) is read first. A soldier not yet walking (+0x1B8 = 0) with hp 15+:
    at its step point (+0x116 / +0x118 = +0x126 / +0x128) StopAnim(at the stored target +0x120 or +0x120 >= 0xC41
    signed), +0x1B8 = 1, AnimStep; else WalkStep 0x1A108. +0x9D set -> MercView(facing +0x1A1, range +0x19E, 0).
    Then only animations with ANIM_STILL (0x93165) or +0xBA = 2 go on: AnimNext, +0xB0 = s16 +0xD2, FrameChecks
    0x1B504. hp 15+: StandUp 0x1B188, and with +0xBA = 2, code 4+ and table 0x93166 (step point allowed): a soldier
    away from its patrol tile +0x12E, which is not the step target, walks there again (+0x1B8 = 0, MoveMercTo(tile, 0,
    0; 0, 0, 0, 0)); else at the waypoint (0x1EE84) an animation other than 0x11 starts 0x11 (SetAnim keep 1) and the
    turn target +0x1A2 = +0x1A4. hp below 15: stored target +0x120 = none. Last, a soldier in the sector, in play
    (+0x138) and not out (+0x188) takes its tile (OccupyTile)."""
    code = s[0x1E8 + 2 * s[0x1F3]]
    if not s[0x1B8] and s8(s[0x183]) >= 15:
        if u16(s, 0x126) == u16(s, 0x116) and u16(s, 0x128) == u16(s, 0x118):
            w.ext("stop_anim", s, int(u16(s, 0x120) == u16(s, 0x114) or s16(u16(s, 0x120)) >= NO_TILE))
            s[0x1B8] = 1
            w.ext("anim_step", s)
        else:
            walk_step(s, w)
    if s[0x9D]:
        reveal_view(s[0x1A1], s[0x19E], s, w, 0)
    if not w.g8(ANIM_STILL + 8 * code) and s[0xBA] != 2:
        return
    w.ext("anim_next", s)
    _w32(s, 0xB0, s16(u16(s, 0xD2)) & 0xFFFFFFFF)
    frame_checks(s, w)
    if s8(s[0x183]) >= 15:
        stand_up(s, w)
        if s[0xBA] == 2 and code >= 4 and w.g8(ANIM_CAN_ACT + 8 * code):
            patrol = u16(s, 0x12E)
            if u16(s, 0x114) != patrol and patrol != u16(s, 0x124):
                s[0x1B8] = 0
                move_merc(s, w, s16(patrol), 0, 0, 0, 0, 0, 0)
            elif at_waypoint(s) and code != 0x11:
                w.ext("set_anim", s, 0x11, 1, 0)
                s[0x1A2] = s[0x1A4]
    else:
        w16(s, 0x120, NO_TILE)
    if s[0x182] and s[0x138] and not s[0x188]:
        occupy(w, s16(u16(s, 0x114)), s[0x17A])


STEP_ENERGY = {1000: 5, 1200: 10, 1500: 5, 1600: 20, 2000: 35, 3000: 50, 3600: 75, 5000: 100, 25300: 5, 25400: 5}
"""0x39C10 (EAX = tile): energy (breath) a step onto the tile costs, by its move code (g_moveCode); other codes 0"""


def exhausted_fall(s, w):
    """0x1B5E4 (EAX = soldier): a soldier with hp below 15 in animation 4..0x27 (the whole word) stops on its own
    tile: step point = the tile's point, StopAnim(1), path marks off (0x1D8B8 with 0x229DD8 != 0), SetAnim 0x40 in
    water, else 0x2B (keep 0, reset 0)."""
    if s8(s[0x183]) >= 15 or not 4 <= u16(s, 0x1E8 + 2 * s[0x1F3]) < 0x28:
        return
    me = s16(u16(s, 0x114))
    w16(s, 0x126, tile_x(me))
    w16(s, 0x128, tile_y(me))
    w.ext("stop_anim", s, 1)
    w.ext("ui_refresh", int(w.g32(0x229DD8) != 0))
    w.ext("set_anim", s, 0x40 if in_water(s) else 0x2B, 0, 0)


def soak_items(s, w):
    """0x407B8 (EAX = soldier record = its 12 items of 12 B): deep water. Every item (type != 0) whose item table
    byte +8 is set loses Random(100) condition points (byte +2) when that roll is below 10; a condition below 0
    (signed) becomes Random(8) + 2."""
    for k in range(12):
        o = 12 * k
        if s[o] and w.g8(ITEM_TBL + 12 * s[o] + 8):
            r = w.random(100)
            if r < 10:
                s[o + 2] = (s[o + 2] - r) & 0xFF
                if s8(s[o + 2]) < 0:
                    s[o + 2] = (w.random(8) + 2) & 0xFF


TRIGGER_STEPS = (-56, 1, 56, -1)     # 0x15906: the 4 neighbour steps (bytes C8 01 38 FF)


def consume_trigger(w, idx, depth=0):
    """ConsumeSectorTrigger 0x19E20 (EAX = trigger index): the trigger's tile (sector +0x58 + 4 idx) becomes 0xC41
    (used); then every trigger whose tile is a 4-neighbour of the old tile (16-bit sums, no map check) is used up
    too, recursively (N, E, S, W; triggers 0..9 in order for each). A neighbour sum of 0xC41 matches the used ones
    again: the original recurses until the stack runs out (old tile 0xC09 / 0xC40 / 0xC42 / 0xC79)."""
    if depth > 40:
        raise Undefined("ConsumeSectorTrigger recursion")
    sec = 0x222BCC + 0xB1 * w.g8(G_CUR_SECTOR)
    t = w.g16(sec + 4 * idx)
    w.s16(sec + 4 * idx, NO_TILE)
    for d in TRIGGER_STEPS:
        v = (t + d) & 0xFFFF
        for i in range(10):
            if v == w.g16(sec + 4 * i):
                consume_trigger(w, i, depth + 1)


def fire_trigger(s, w, idx, keep):
    """FireSectorTrigger 0x19F30 (EAX = merc, EDX = trigger index, BL = keep): message 4, the merc stops on its own
    tile (step point = its point, StopAnim 1); without keep the trigger is used up (ConsumeSectorTrigger 0x19E20)."""
    w.ext("merc_message", s, 4, 0x40, 0x903AC, 1)
    me = s16(u16(s, 0x114))
    w16(s, 0x126, tile_x(me))
    w16(s, 0x128, tile_y(me))
    w.ext("stop_anim", s, 1)
    if not keep:
        consume_trigger(w, idx)


def tile_triggers(s, w):
    """CheckSectorTileTriggers 0x19FA0 (EAX = merc): the 10 (tile, value) word pairs at sector +0x58 (0x222BCC +
    0xB1 x sector). On the merc's tile: value < 0 fires when s8 +0x192 <= |value| (stat 4 practised by 5); value >= 0
    fires when s8 +0x192 >= value (practised by 10). Firing = FireSectorTrigger(index, keep 0)."""
    sec = 0x222BCC + 0xB1 * w.g8(G_CUR_SECTOR)
    for i in range(10):
        if u16(s, 0x114) != w.g16(sec + 4 * i):
            continue
        v = s16(w.g16(sec + 4 * i + 2))
        if v < 0:
            if s8(s[0x192]) <= abs(v):
                fire_trigger(s, w, i, 0)
                w.ext("stat_practice", s, 4, 5, 0)
        elif s8(s[0x192]) >= v:
            fire_trigger(s, w, i, 0)
            w.ext("stat_practice", s, 4, 10, 0)


def step_start(s, w):
    """StepStart 0x1A46C (EAX = soldier) -> 1 when the soldier starts the step to the next tile. The tile lies in
    the turn-target direction +0x1A2 (+4 wrapped when backing up, +0x196; backing up stops on water). In real time
    a step that costs more AP than the soldier has (minus the reserve +0xEF for a merc with +0x16B = 1 not yet
    warned) stops the walk (StopWalk 0, +0x197 = 1): above the AP themselves StopAnim(1 only for an impassable step
    onto the step target, else 0); else the low-AP stop (0x95DC8 = 2000 with +0x16B, warned, step point = own tile,
    StopAnim 0, ClearTask 0, +0x16D = 1, 0x1148C(slot, 1, 0)); 0. Else a standable tile (or +0xBA = 2) goes; an
    object to break (TileStandable < 0) with +0xB9 breaks (0x3ACB8; mercs in play re-view, +0x106 = tile) and goes;
    else WalkBlocked. A merc stepping onto a mine (ground item 0x13 / 0x2C) it detects (s8 +0x192 >= |byte +2|)
    does not go: FireSectorTrigger(0xFF, keep), MineFlagReroute?. A forgetful merc (+0x19C = 2) after 6+ steps in
    combat (16+ otherwise) by Random(15) = 0 queues quote 0xD (delay 2, "Where was I going again?"), stops
    (StopAnim 1) and in combat loses Random(AP) + 1 AP. Going: step point = the tile's point; the step animation by ground and object
    (0x32 walk, 0x31 wade, 0x34 / 0x37 / 0x3A water depths)."""
    slot = s[0x17A]
    code = u16(s, 0x1E8 + 2 * s[0x1F3])
    d = s[0x1A2]
    if s[0x196]:
        d = (d + 4) & 0xFF
        if d > 8:
            d = (d - 8) & 0xFF
    me = s16(u16(s, 0x114))
    nt = step_tile(me, dir_step(w, d))
    t0, t1 = w.tile8(nt, 0), w.tile8(nt, 1)
    cls = w.g8(G_T0_CLASS + t0)
    if s[0x196] and is_water_tile(t0, t1):
        s[0x196] = 0
    if nt != me:
        from combat import step_ap_cost
        ap = step_ap_cost(w.g16(G_MOVECODE + 2 * nt), s[0x1A8], s[0x177], s[0x196]) & 0xFF
    else:
        ap = 0
    have = s[0x1A5]
    if slot < 8 and not w.g8(G_AP_WARNED + slot) and s[0x16B] == 1:
        have = (have - s[0xEF]) & 0xFF
    if w.g8(0x92DCB) and ap > have:
        w.ext("stop_walk", s, 0)
        s[0x197] = 1
        if ap > s8(s[0x1A5]):
            w.ext("stop_anim", s, int(ap >= 100 and nt == s16(u16(s, 0x124))))
            return 0
        if s[0x16B]:
            w.s32(0x95DC8, 2000)
        w.s8(G_AP_WARNED + slot, 1)
        w16(s, 0x126, tile_x(me))
        w16(s, 0x128, tile_y(me))
        w.ext("stop_anim", s, 0)
        clear_task(s, 0)
        s[0x16D] = 1
        w.ext("w_1148c", slot, 1, 0)
        return 0
    go = 0
    st = tile_standable(w, nt)
    if st >= 1 or s[0xBA] == 2:
        go = 1
    elif st < 0 and s[0xB9]:
        w.ext("break_obj", nt, t1, int(s8(s[0x9C]) > 0), 1)
        for k in range(8):
            if k == slot:
                if s[0x138]:
                    s[0x9D] = 1
            elif w.g8(G_SOLDIERS + 0x200 * k + 0x138):
                w.s8(G_SOLDIERS + 0x200 * k + 0x9D, 1)
        w16(s, 0x106, nt)
        go = 1
    else:
        w.ext("walk_blocked", s, nt, w.tile8(nt, 5) & 0x7F)
    gi = w.tile8(nt, 6)
    if slot < 8 and gi < 0xFF and w.g8(G_GROUND_ITEMS + 12 * gi) in (0x13, 0x2C) \
            and s8(s[0x192]) >= abs(s8(w.g8(G_GROUND_ITEMS + 12 * gi + 2))):
        go = 0
        fire_trigger(s, w, 0xFF, 1)
        w.ext("mine_flag", s, nt)
    if s[0x19C] == 2 and slot < 8 and s[0x1AF] > (5 if w.g8(0x92DCB) else 15) and w.random(15) == 0:
        w.s8(0x92DEC, slot)
        w.s8(0x284F27, 0xD)
        w.s8(0x284F26, 2)
        go = 0
        w.ext("stop_anim", s, 1)
        if w.g8(0x92DCB) and s[0x1A5]:
            s[0x1A5] = (s[0x1A5] - (w.random(s8(s[0x1A5])) + 1)) & 0xFF
    if not go:
        return 0
    w16(s, 0x126, tile_x(nt))
    w16(s, 0x128, tile_y(nt))
    wading = 0 < t1 < 0x21
    if not in_water(s) and u16(s, 0x1E8 + 2 * s[0x1F3]) < 0x32:
        if cls >= 8 and not wading:
            w.ext("set_anim", s, 0x32, 1, 1)
        return 1
    code = s16(code)
    if cls < 8 or wading:
        new = 0x31
    elif t0 >= 0x9A:
        new = 0x3A if code < 0x39 else None
    elif t0 >= 0x97:
        new = 0x37 if code < 0x36 or code >= 0x39 else None
    else:
        new = 0x34 if code > 0x34 else 0x32 if code < 0x32 else None
    if new is not None:
        w.ext("set_anim", s, new, 1, 1)
    return 1


def _turn_ends(s, w, check_target):
    """the walk of the soldier in turn ends with the end of combat movement (0x92DD0): 0x284F0F = the turn slot when
    a merc, StopAnim (only off the step target when check_target), +0x197 = 1, NextTurn?(0)"""
    if w.g8(0x284F11) < 8:
        w.s8(0x284F0F, w.g8(0x284F11))
    if not check_target or u16(s, 0x114) != u16(s, 0x124):
        w.ext("stop_anim", s, 0)
    s[0x197] = 1
    w.ext("next_turn", 0)


def active_frame(s, w):
    """0x1B9D4 (EAX = soldier): the per-frame update of the soldier whose turn it is (or any merc out of combat), with
    timer +0xB0 run out. See REVERSE.md "Soldier update per frame". Walking (+0x1B8 = 0, hp 15+): off the step point
    -> WalkStep 0x1A108. At the step point: door to close (+0x106), sneak practice, a step taken (+0x193: AP, sector
    triggers, ground item, drowning), sightings and turn hand-over, mine flag, AiThink for the turn slot, then the
    step target reached (WalkEnd, the queued action +0x1A6 on +0x134) or the next step (NextPathDir, 0x1A46C,
    WalkStep), out of energy -> 0x2B. Still (+0x1B8 or hp below 15): AnimNext, AiThink, attack start, AnimStep.
    Last: view, FrameChecks, StandUp, the timer +0xB0 from +0xD2, OccupyTile, the turn banner."""
    slot = s[0x17A]
    code0 = u16(s, 0x1E8 + 2 * s[0x1F3])
    ended = throw = 0
    if not s[0x1B8] and s8(s[0x183]) >= 15:
        if u16(s, 0x126) != u16(s, 0x116) or u16(s, 0x128) != u16(s, 0x118):
            walk_step(s, w)
        else:
            door = s16(u16(s, 0x106))
            if door != NO_TILE and u16(s, 0x114) != u16(s, 0x106):
                w.ext("close_door", door, w.tile8(door, 1), int(s8(s[0x9C]) > 0))
                w16(s, 0x106, NO_TILE)
            if slot < 8 and s[0x1A8] and s8(s[0x1D5]) > 0:
                for j in range(8):
                    if s[0x13A + j] == 1 and w.g8(G_SOLDIERS + 0x200 * (0x28 + j) + 0x13A + slot) != 1:
                        w.ext("stat_practice", s, 2, 1, 0)
            me = s16(u16(s, 0x114))
            if s[0x193]:
                s[0x193] = 0
                if u16(s, 0xF4) != u16(s, 0x114):
                    from combat import step_ap_cost
                    code = w.g16(G_MOVECODE + 2 * me)
                    w.ext("spend_ap", s, step_ap_cost(code, s[0x1A8], s[0x177], s[0x196]) & 0xFF,
                          STEP_ENERGY.get(code, 0))
                w16(s, 0xF4, u16(s, 0x114))
                s[0x17D] = 0
                tile_triggers(s, w)
                throw = 1
                idx = item_index(w, me)
                if idx < 0xFF:
                    kind = w.g8(G_GROUND_ITEMS + 12 * idx)
                    if slot < 0x28 or kind == 0x70:
                        w.ext("w_1ac68", s, me, idx, 0)
                    elif kind == 0x11 and s8(w.g8(G_GROUND_ITEMS + 12 * idx + 2)) >= 10:
                        w.ext("explode", slot, me, 2, 1)
                        w.ext("w_3c05c", me, 0)
                if not s[0x18A] and in_water(s) and s[0x183]:
                    drown(s, w)
                    w16(s, 0x124, u16(s, 0x114))
                    walk_end(s, w, s[0x1E8 + 2 * s[0x1F3]])
            if slot >= 0x28:
                w.ext("enemy_turned", s)
                mercs_see(s, w)
                if s8(s[0x9C]) > 0:
                    w.ext("focus", slot, 1)
                    if w.g8(0x92DDF):
                        w.ext("tac_redraw", 1)
                if w.g8(0x92DCF):
                    w.ext("w_1304c")
                    ended = 1
                elif w.g8(0x92DD0):
                    w.ext("next_turn", 0)
                    ended = 1
            else:
                if slot < 0x10:
                    w.ext("guard_turned", s)
                    enemies_see(s, w)
                    w.ext("share_sightings", s, 0)
                    reveal_view(s[0x1A1], s[0x19E], s, w, 0)
                if w.g8(0x92DD0):
                    ended = 1
                    _turn_ends(s, w, True)
            if throw:
                step_noise(s, w)
            if w.g8(0x92DCB) and w.g8(0x284F11) != slot and u16(s, 0x114) != u16(s, 0x124):
                w.ext("stop_anim", s, 0)
            if s16(u16(s, 0xEC)) != NO_TILE:
                w.ext("mine_flag", s, s16(u16(s, 0xEC)))
                w16(s, 0xEC, NO_TILE)
            if slot == w.g8(0x284F11):
                w.ext("ai_think", s)
            if slot < 8 and in_water(s) and s[0x1B2] >= 0x9A:
                soak_items(s, w)
            if u16(s, 0x114) == u16(s, 0x124):
                walk_end(s, w, code0 & 0xFF)
                if (not w.g8(0x92DCB) or w.g8(0x284F11) == slot) and s[0x1A6] and not s[0x1B0]:
                    act = 0 if s[0x1A6] >= 0xFE else s[0x1A6] if s[0] == s[0x1A6] else -1
                    if act < 0 or in_water(s):
                        clear_task(s, 1)
                    else:
                        target = s16(u16(s, 0x134))
                        warn = False
                        if slot < 8:
                            c = w.ext("w_59ae8", s, me, target, w.g8(G_TILEMAP + 6 + 8 * target), 0) & 0xFF
                            have = s[0x1A5]
                            if not w.g8(G_AP_WARNED + slot) and s[0x16B]:
                                have = (have - s[0xEF]) & 0xFF
                            warn = w.g8(0x92DCB) and c > have and s[0x16B]
                        if warn:
                            w.s8(G_AP_WARNED + slot, 1)
                            w.s32(0x95DC8, 2000)
                            w16(s, 0x126, tile_x(me))
                            w16(s, 0x128, tile_y(me))
                            w.ext("stop_anim", s, 0)
                            clear_task(s, 0)
                            s[0x16D] = 1
                            w.ext("w_1148c", slot, 1, 0)
                        else:
                            w.ext("click", s, target, act & 0xFF)
            elif s8(s[0x18A]) > 0 or in_water(s):
                if not ended:
                    d = next_path_dir(s, w)
                    s[0x1A2] = d
                    if s[0x196]:
                        d = (d + 4) & 0xFF
                        s[0x1A2] = d - 8 if d > 8 else d
                    if step_start(s, w):
                        w16(s, 0xD4, u16(s, 0xD4) - 1)
                        s[0x1AF] = (s[0x1AF] + 1) & 0xFF
                        walk_step(s, w)
                    elif u16(s, 0x124) == u16(s, 0x114):
                        walk_end(s, w, s[0x1E8 + 2 * s[0x1F3]])
            elif s16(code0) >= 4:
                w16(s, 0x126, tile_x(me))
                w16(s, 0x128, tile_y(me))
                w.ext("stop_anim", s, 1)
                w.ext("ui_refresh", int(w.g32(0x229DD8) != 0))
                w.ext("set_anim", s, 0x2B, 0, 0)
            exhausted_fall(s, w)
    else:
        if w.g8(ANIM_STILL + 8 * u16(s, 0x1E8 + 2 * s[0x1F3])):
            w.ext("anim_next", s)
        if w.g8(0x284F11) == slot:
            w.ext("ai_think", s)
        if s8(s[0x183]) >= 15 or in_water(s):
            if not in_water(s) and s[0] > 0x10 and s[0x1BE] == 1:
                w.ext("w_5abe0", s, s16(u16(s, 0xDC)))
                throw_follow(s, w)
            if s16(u16(s, 0x12A)) != NO_TILE:
                w.ext("anim_step", s)
        else:
            exhausted_fall(s, w)
    if slot < 8 and s[0x9D]:
        w.ext("guard_turned", s)
        enemies_see(s, w)
        w.ext("share_sightings", s, 0)
        if w.g8(0x92DD0):
            _turn_ends(s, w, False)
        reveal_view(s[0x1A1], s[0x19E], s, w, 0)
    frame_checks(s, w)
    if s8(s[0x183]) >= 15:
        stand_up(s, w)
        if s[0x19C] == 4 and not w.g8(0x22A0FF) and _i32(w.ext("w_1aaf0", s)) > 5:
            w.ext("merc_message", s, 4, 0x40, 0x903AC, 1)
            w.s8(0x22A0FF, 1)
    else:
        w16(s, 0x120, NO_TILE)
    if slot >= 0x28 and s8(s[0x9C]) == -1:
        _w32(s, 0xB0, 0)
    else:
        v = s16(u16(s, 0xD2))
        if not (s[0xBA] == 1 and w.g8(0x92DCB)) and (not w.g8(0x1163C1)
                                                     or w.g8(ANIM_STILL + 8 * u16(s, 0x1E8 + 2 * s[0x1F3]))):
            _w32(s, 0xB0, v & 0xFFFFFFFF)
        else:
            _w32(s, 0xB0, cdiv(v, 2) & 0xFFFFFFFF)
    if s[0x182] and s[0x138] and not s[0x188]:
        occupy(w, s16(u16(s, 0x114)), slot)
    if w.g8(0x284F10) != 0x40:
        w.s8(0x284F11, w.g8(0x284F10))
        w.ext("turn_banner")
        w.s8(0x284F10, 0x40)

QUOTE_CRIES = {0xF9: "BAD", 0xFA: "GOOD", 0xFB: "EMPTY", 0xFC: "NOTHING", 0xFD: "HMM"}


def merc_quote(s, w):
    """MercQuoteEvent? 0x1B324 (EAX = soldier): the queued merc quote (slot 0x92DEC, event 0x284F27, tile word
    0x92DEE, delay 0x284F26, focus flag 0x284F25) plays when the soldier is that slot and the slot is a merc (< 8).
    A delay above 0 (signed) counts down by 1 and nothing else happens. Events below 0x21 are text: ShowMercMessage?
    0x299B8(soldier, event, tile, "", 1); event 5 also forgets the attack (0x56ED0, 1). Events 0xF9..0xFD are cries
    BAD / GOOD / EMPTY / NOTHING / HMM (0x24E44(+0x179, -1, name, 1, 1)); then with timer +0xC0 <= 0 and +0x16C = 0:
    focus (only with the focus flag), sample 0x2B11 at volume 0x58, +0xC0 = 400. Other events play nothing. After
    any event the queue clears: slot 0x40, tile 0xC41, delay and focus flag 0."""
    slot = s[0x17A]
    if slot != w.g8(0x92DEC) or slot >= 8:
        return
    if s8(w.g8(0x284F26)) > 0:
        w.s8(0x284F26, w.g8(0x284F26) - 1)
        return
    ev = w.g8(0x284F27)
    if ev < 0x21:
        w.ext("merc_message", w.rec(slot), ev, s16(w.g16(0x92DEE)), 0x903AC, 1)
        if ev == 5:
            clear_target(s, w, 1)
    elif ev in QUOTE_CRIES:
        w.ext("cry", s[0x179], 0xFFFFFFFF, QUOTE_CRIES[ev], 1, 1)
        if _i32(int.from_bytes(s[0xC0:0xC4], "little")) <= 0 and not s[0x16C]:
            if w.g8(0x284F25):
                w.ext("focus", slot, 1)
            w.ext("voc", 0x2B11, 0x58, 1)
            _w32(s, 0xC0, 400)
    w.s8(0x284F25, 0)
    w.s8(0x92DEC, 0x40)
    w.s8(0x284F26, 0)
    w.s16(0x92DEE, NO_TILE)


STEP_NOISE_TEXT = ("splash", "SPLASH!", "clunk", "CLUNK!", "snap", "SNAP!")   # 0x964BC, 8 bytes each


def step_noise(s, w):
    """StepNoise 0x19A6C (was ThrowItem?; EAX = soldier, after each step in ActiveFrame): how loud the step is.
    q = s8 +0x192 x 5 + s8 +0x18C / 2; scaled down by (50 - h)% for h = ((max hp - hp - wounds) as a byte) / 2 + hp
    (byte) below 50, and by (50 - energy)% for s8 energy below 50; -10 in water, -10 more for a merc in posture 3 or
    an other with attitude 5 (only the low byte counts from there). Walking (+0x1A8 = 0): loudness = Random(6 - q / 20,
    +1 in water) + 1. Sneaking: a roll Random(100) (rolled again when not below q) below q is silent; else loudness =
    (roll - q + 1) / 20 + 1, and a merc hears itself: sound 0x1E/0x1F (water, Random(2)), 0x2A (ground class 3) or
    0x2B/0x1C (Random(2)), volume loudness x 15 + 0x23, and with option 0x1163BD the text splash / clunk / snap
    (capitals for loudness 3+) in g_textBuf, shown for 500. Loudness above 0: MakeNoise(slot, tile, ground class
    +0x1B3, loudness, 3 in water else 1)."""
    q = s8(s[0x192]) * 5 + cdiv(s8(s[0x18C]), 2)
    h = (((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + s8(s[0x183])) & 0xFF
    if h < 0x32:
        q = (q & 0xFF) - cdiv((0x32 - h) * (q & 0xFF), 100)
    e = s8(s[0x18A])
    if e < 0x32:
        q = (q & 0xFF) - cdiv((0x32 - e) * (q & 0xFF), 100)
    q &= 0xFF
    water = is_water_tile(s[0x1B2], s[0x1B4])
    if water:
        q = (q - 10) & 0xFF
    if (s[0x17A] < 8 and s[0x19C] == 3) or (s[0x17A] >= 8 and s[0x1D3] == 5):
        q = (q - 10) & 0xFF
    if not s[0x1A8]:
        n = 6 - q // 20
        if water:
            n += 1
        loud = (w.random(n & 0xFF) + 1) & 0xFF
    else:
        r = w.random(100) & 0xFF
        if r >= q:
            r = w.random(100) & 0xFF
        if r < q:
            loud = 0
        else:
            loud = ((r - q + 1) // 20 + 1) & 0xFF
            if s[0x17A] < 8:
                if water:
                    fx = int(w.random(2) == 0) + 0x1E
                    k = int(loud >= 3)
                elif s[0x1B3] == 3:
                    fx, k = 0x2A, int(loud >= 3) + 2
                else:
                    fx = 0x1C if w.random(2) else 0x2B
                    k = int(loud >= 3) + 4
                text = STEP_NOISE_TEXT[k]
                for i, ch in enumerate(text.encode() + b"\0"):
                    w.s8(G_TEXT_BUF + i, ch)
                w.ext("sfx", fx, 0x2B11, s8((loud * 15 + 0x23) & 0xFF) & 0xFFFFFFFF, 1)
                if w.g8(0x1163BD):
                    w.ext("show_msg", text, 0, 0, 1, 1, 0, 0x1F4, 1)
    if loud:
        w.ext("make_noise", s[0x17A], s16(u16(s, 0x114)), s[0x1B3], loud, 3 if water else 1)


def nearest_merc(s, w, edx):
    """NearestMerc 0x1AAF0 (EAX = soldier, EDX = whatever the caller left there) -> the distance (i32) to the
    nearest other merc, at most 99. Register quirk: the tests (k not the own slot, the CALLER's hp 15+, in sector,
    in play) gate only the TileDistance call; the compare runs for every k = 0..7 with the last EDX, so the own slot
    (and every k when the caller's own tests fail) compares the previous distance, or the entry EDX for slot 0 /
    a failing caller. The other merc's own state is never tested."""
    best = 0x63
    for k in range(8):
        if k != s[0x17A] and s8(s[0x183]) >= 15 and s[0x182] and s[0x138]:
            edx = _i32(w.ext("tile_dist", s16(w.g16(G_SOLDIERS + 0x200 * k + 0x114)), s16(u16(s, 0x114))))
        if _i32(edx) < best:
            best = _i32(edx)
    return best


def find_path_op(s, w, tile, store):
    """FindPath 0x40810 (EAX = soldier, EDX = destination tile, BX = store) -> 1 found / 0: the A* of combat.py
    find_path from the soldier's tile over G_MOVECODE (ground byte 0 and occupant byte 5 of the tile map, +0x177,
    +0xB9, combat flag). Found and BX != 0: the first 80 directions go to G_PATHS[slot], +0xD4 = their count,
    +0x1AF = 0. Not found, or BX = 0: nothing is written."""
    from combat import find_path
    mc = [w.g16(G_MOVECODE + 2 * t) for t in range(3136)]
    t0 = [w.g8(G_TILEMAP + 8 * t) for t in range(3136)]

    def occ(t):
        o = w.g8(G_TILEMAP + 8 * t + 5) & 0x7F
        return o if o < 0x30 else 0x40
    dirs = find_path(s16(u16(s, 0x114)), s16(tile & 0xFFFF), mc, t0, occ, s[0x177], s[0xB9], s[0x17A], w.g8(G_COMBAT))
    if dirs is None:
        return 0
    if store & 0xFFFF:
        dirs = dirs[:80]
        for k, d in enumerate(dirs):
            w.s8(G_PATHS + 0x50 * s[0x17A] + k, d)
        w16(s, 0xD4, len(dirs))
        s[0x1AF] = 0
    return 1


def path_ap_cost(s, w, tile, mark, extra, shift, b177, alt):
    """PathApCost 0x1D32C (EAX = soldier, DX = tile, BL = mark the path on the map, CL = one step more; stack: Shift,
    +0x177 byte, Alt) -> the AP of the walk: G_WALK_AP (0x229DF0) = 0, FindPath(tile, store 1); found: the sum of
    StepApCost over the stored steps (+1 step with CL: reads one direction past the count), G_WALK_AP = the first
    step's cost; then getting up (current animation word 0 / 3: 7, 1 / 2: 5, 0xE / 0xF: 3) is added to both. With
    BL set and the preview flag 0x229DF4 already set: 0 at once.
    Marks (BL set, option 0x1163BF on), in tile byte 7 (high nibble = direction in, low = direction out, as
    direction + 1): after step 0, when the first step's tile byte 0 < 0x9A: the start tile's low nibble = step 0's
    direction, list 0x10DF90 = [start], count 0x229E08 = 1 (else the list and count stay). Each step k < n - 1
    appends its tile; a tile with byte 0 >= 0x9A and byte 1 = 0 gets high nibble 0xA (low kept), else high = step
    k's direction and low = step k + 1's (with CL on the last two steps: step k's again). At the end (also with no
    path) BL sets 0x229DF4 = 1, and BL + CL set 0x229DD8 = slot + 1."""
    from combat import step_ap_cost, posture_ap_cost
    mark, extra = mark & 0xFF, extra & 0xFF
    if mark and w.g32(G_MARKS_SHOWN):
        return 0
    w.s32(0x229DF0, 0)
    c = post = 0
    if find_path_op(s, w, tile, 1):
        n = s16(u16(s, 0xD4)) + (1 if extra else 0)
        t = s16(u16(s, 0x114))
        path = G_PATHS + 0x50 * s[0x17A]
        marks = mark and w.g8(0x1163BF)
        for k in range(n):
            d = (w.g8(path + k) + 1) & 0xFF
            t = step_tile(t, dir_step(w, d))
            c += step_ap_cost(w.g16(G_MOVECODE + 2 * t), shift & 0xFF, b177 & 0xFF, alt & 0xFF)
            if k == 0:
                w.s32(0x229DF0, c)
                if marks and w.g8(G_TILEMAP + 8 * t) < 0x9A:
                    st = s16(u16(s, 0x114))
                    w.s16(G_MARKS, u16(s, 0x114))
                    a = G_TILEMAP + 8 * st + 7
                    w.s8(a, (w.g8(a) & 0xF0) | (d & 0xF))
                    w.s32(G_MARKS_N, 1)
            if marks and k < n - 1:
                cnt = (w.g32(G_MARKS_N) + 1) & 0xFFFFFFFF
                w.s16(G_MARKS + 2 * (cnt - 1), t & 0xFFFF)
                w.s32(G_MARKS_N, cnt)
                a = G_TILEMAP + 8 * t + 7
                if w.g8(G_TILEMAP + 8 * t) >= 0x9A and not w.g8(G_TILEMAP + 8 * t + 1):
                    w.s8(a, (w.g8(a) & 0xF) | 0xA0)
                else:
                    w.s8(a, ((d & 0xF) << 4) | (w.g8(a) & 0xF))
                    out = d if k >= n - 2 and extra else w.g8(path + k + 1) + 1
                    w.s8(a, (w.g8(a) & 0xF0) | (out & 0xF))
        post = posture_ap_cost(u16(s, 0x1E8 + 2 * s[0x1F3]))
    w.s32(0x229DF0, (w.g32(0x229DF0) + post) & 0xFFFFFFFF)
    if mark:
        w.s32(G_MARKS_SHOWN, 1)
        if extra:
            w.s32(G_MARKS_ON, s[0x17A] + 1)
    return (c + post) & 0xFFFFFFFF


THROW_SCATTER_DIRS = ((1, 5), (2, 6), (3, 7), (4, 8))     # 0x59888 jump table (facing - 1): x / x+y+C / y / x+y


def throw_scatter(s, w, margin, dist):
    """ThrowScatter? 0x598A8 (EAX = soldier, DL = miss margin, BX = distance): a missed throw lands off the aimed
    point (+0xDE / +0xE0, then +0xDC = ViewTile). Facing +0x1A1: 1/5 x only, 3/7 y only, 2/6 and 4/8 both (2/6: both
    signs alike, 4/8: opposite). Size = max(1, dist / 30) x max(1, margin / 15); both axes: half the size each; each
    used axis at least 4; random sign(s) by Random(2) (x first)."""
    from combat import view_tile
    d = s[0x1A1]
    ax = d in (1, 5, 2, 6, 4, 8)
    ay = d in (2, 6, 3, 7, 4, 8)
    same = d in (2, 6)
    q1 = cdiv(s16(dist & 0xFFFF), 30)
    if s16(q1 & 0xFFFF) < 1:
        q1 = 1
    q2 = (margin & 0xFF) // 15
    if q2 < 1:
        q2 = 1
    size = q1 * q2
    if ax and ay:
        x = y = cdiv(s16(size & 0xFFFF), 2)
    else:
        x = size if ax else 0
        y = size if ay else 0
    if ax and s16(x & 0xFFFF) < 4:
        x = 4
    if ay and s16(y & 0xFFFF) < 4:
        y = 4
    if ax and ay:
        r = w.random(2)
        if same:
            if r:
                x, y = -x, -y
        elif r:
            x = -x
        else:
            y = -y
    else:
        if ax and w.random(2):
            x = -x
        if ay and w.random(2):
            y = -y
    w16(s, 0xDE, (u16(s, 0xDE) + x) & 0xFFFF)
    w16(s, 0xE0, (u16(s, 0xE0) + y) & 0xFFFF)
    w16(s, 0xDC, view_tile(s16(u16(s, 0xDE)), s16(u16(s, 0xE0))) & 0xFFFF)


def throw_aim(s, w, tile):
    """ThrowAim? 0x59364 (EAX = soldier, EDX = target tile): in combat the effect tile 0x92E4C = tile (an enemy in
    view first gets the focus). With a thrown item in the hand (kind 0x95, signed type): pays ActionApCost(tile, 1)
    + aim +0x1FD AP; aimed point = the tile's point, height 0; roll Random(100) + 1 against ThrowChance (signed
    bytes): above = miss (+0x1BC = 0, ThrowScatter by the margin), else hit (+0x1BC = 1, +0xDC = tile). The throw
    reaches range x 15 + hp - 50, less (100 - energy)/200 of it, at least 0: a farther end point is pulled back along
    the line to that distance (+0xDC = ViewTile of it). +0xD0 = tile."""
    from combat import distance, view_tile
    t16 = tile & 0xFFFF
    if w.g8(G_COMBAT):
        if s[0x17A] >= 8 and s8(s[0x9C]) > 0:
            w.ext("focus", s[0x17A], 0)
        w.s16(0x92E4C, t16)
    typ = s[0]
    if w.g8(G_ITEM_KIND + 12 * s8(typ)) != 0x95:
        return
    ap = (w.ext("action_ap", s, tile, 1) + s[0x1FD]) & 0xFF
    w.ext("spend_ap", s, s8(ap), 0)
    w16(s, 0xDE, tile_x(tile) & 0xFFFF)
    w16(s, 0xE0, tile_y(tile) & 0xFFFF)
    w16(s, 0xE2, 0)
    dist = distance(s16(u16(s, 0x116)), s16(u16(s, 0x118)), s16(u16(s, 0xDE)), s16(u16(s, 0xE0)))
    d16 = s16(dist & 0xFFFF)
    ch = throw_chance(s, w, tile, s[0x1FD]) & 0xFF
    r = (w.random(100) + 1) & 0xFF
    if s8(r) > s8(ch):
        s[0x1BC] = 0
        throw_scatter(s, w, (r - ch) & 0xFF, d16)
    else:
        s[0x1BC] = 1
        w16(s, 0xDC, t16)
    reach = w.g8(G_ITEM_RANGE + 12 * typ) * 15 + s8(s[0x183]) - 0x32
    reach -= cdiv((100 - s8(s[0x18A])) * reach, 200)
    if reach < 0:
        reach = 0
    if s16(reach & 0xFFFF) < d16:
        b = s16(reach & 0xFFFF)
        dx = s16((u16(s, 0xDE) - u16(s, 0x116)) & 0xFFFF)
        dy = s16((u16(s, 0xE0) - u16(s, 0x118)) & 0xFFFF)
        w16(s, 0xDE, (cdiv(dx * b, d16) + s16(u16(s, 0x116))) & 0xFFFF)
        w16(s, 0xE0, (cdiv(dy * b, d16) + s16(u16(s, 0x118))) & 0xFFFF)
        w16(s, 0xDC, view_tile(s16(u16(s, 0xDE)), s16(u16(s, 0xE0))) & 0xFFFF)
    w16(s, 0xD0, t16)


def throw_follow(s, w):
    """ThrowFollow? 0x5AFBC (EAX = soldier; ActiveFrame after the attack start): with a target tile (+0xDC not none)
    and a thrown item in the hand (kind 0x95, unsigned type): ThrowAim at it, turn to the end point (DirectionTo
    from the point + (2, 4)) and start the throw animation 0x27 (SetAnim 0, 1)."""
    t = s16(u16(s, 0xDC))
    if t == NO_TILE or w.g8(G_ITEM_KIND + 12 * s[0]) != 0x95:
        return
    throw_aim(s, w, t)
    s[0x1A2] = w.ext("direction", s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, s16(u16(s, 0xDE)),
                     s16(u16(s, 0xE0))) & 0xFF
    w.ext("set_anim", s, 0x27, 0, 1)


def mercs_see(s, w):
    """0x735BC (EAX = an enemy, after its step): an enemy in view drops out of it (s8 +0x9C > 0 -> 0); G_INTERRUPTED
    = 1; every merc or guard (slots 0..15) in the sector, in play, with hp 15+ that sees it (SightCheck 0x72F50)
    shares its sightings (0x736BC, 0) and asks for an interrupt (MercInterrupt? 0x73D28). Then an enemy still out of
    view (s8 +0x9C < 1) that is the spotted one (i32 0x229C94 = its slot) is that no more (0x229C94 = 0x40)."""
    if s8(s[0x9C]) > 0:
        s[0x9C] = 0
    w.s8(G_INTERRUPTED, 1)
    for k in range(0x10):
        a = G_SOLDIERS + 0x200 * k
        if w.g8(a + 0x182) and w.g8(a + 0x138) and s8(w.g8(a + 0x183)) >= 15:
            if w.ext("sight_check", k, s) & 0xFF:
                w.ext("share_sightings", k, 0)
                w.ext("w_73d28", k, s)
    if s8(s[0x9C]) < 1 and s[0x17A] == _i32(w.g32(0x229C94)):
        w.s32(0x229C94, 0x40)


def enemies_see(s, w):
    """0x7365C (EAX = a merc or guard): G_INTERRUPTED = 1; every enemy (slots 0x28..0x2F) in the sector, in play, with
    hp 15+ that sees the soldier (SightCheck 0x72F50) asks for an interrupt (0x73F3C)."""
    w.s8(G_INTERRUPTED, 1)
    for k in range(0x28, 0x30):
        a = G_SOLDIERS + 0x200 * k
        if w.g8(a + 0x182) and w.g8(a + 0x138) and s8(w.g8(a + 0x183)) >= 15:
            if w.ext("sight_check", k, s) & 0xFF:
                w.ext("w_73f3c", k, s)


def end_task(s, w, flag):
    """EndTask? 0x3D98C (mercs' shared tasks, state +0x1B9 4..6 with partner tile +0xD6 and counter +0x1CA): any
    task sets re-think. States 5 / 6 end: the partner on +0xD6 counts down and drops its state 4 at 0 (portrait
    redraw); the soldier keeps state 4 while its own counter is above 0, else 0; partner tile none, counter and
    +0x181 cleared; with flag 0x197D8 and animation 0x10; portrait redraw. State 4 ends the tasks of mercs in state 5
    working on its tile, then, still 4, clears itself (portrait, with flag 0x197D8); a merc also re-posts its posture
    (0x17C88)."""
    if s[0x1B9]:
        s[0x1C1] = 1
    if s[0x1B9] in (5, 6):
        occ = _occupant(w, s16(u16(s, 0xD6)))
        if occ < 0x40:
            p = w.rec(occ)
            p[0x1CA] = (p[0x1CA] - 1) & 0xFF
            if s8(p[0x1CA]) <= 0:
                p[0x1CA] = p[0x1B9] = p[0x181] = 0
                w.ext("portrait", p[0x17A], 1)
        s[0x1B9] = 0 if s8(s[0x1CA]) <= 0 else 4
        w16(s, 0xD6, NO_TILE)
        s[0x1CA] = s[0x181] = 0
        if flag:   # the whole EDX
            w.ext("w_197d8", s, 0)
            w.ext("set_anim", s, 0x10, 0, 0)
        w.ext("portrait", s[0x17A], 1)
    if s[0x1B9] == 4:
        for k in range(8):
            p = w.rec(k)
            if p[0x182] and p[0x138] and p[0x183] and p[0x1B9] == 5 and u16(p, 0xD6) == u16(s, 0x114):
                end_task(p, w, 1)
        if s[0x1B9] == 4:
            s[0x181] = s[0x1B9] = 0
            s[0x1CA] = 0
            w.ext("portrait", s[0x17A], 1)
            if flag:   # the whole EDX
                w.ext("w_197d8", s, 0)
        if s[0x17A] < 8:
            w.ext("reserve_ap", s)


def item_charges(s, j):
    """0x151BC (EAX = item entries, EDX = entry): sum of the entry's unit conditions (count +1; the first unit's at
    +2, unit k >= 1 at +7 + k), signed bytes"""
    b = 12 * j
    return sum(s8(s[b + 2] if k == 0 else s[b + 7 + k]) for k in range(s8(s[b + 1])))


def use_charges(s, w, n):
    """0x151FC (EAX = item entry, EDX = charges): takes n charges from the last unit back: a unit with more keeps the
    rest; an emptied extra unit gets condition 0 and the count drops by one; an emptied first unit drops the whole
    entry (0x470B0)."""
    for k in range(s8(s[1]) - 1, -1, -1):
        if n <= 0:
            return
        o = 2 if k == 0 else 7 + k
        c = s8(s[o])
        if c > n:
            s[o] = (s[o] - n) & 0xFF
            n = 0
        elif k == 0:
            n -= c
            w.ext("drop_item", s)
        else:
            s[o] = 0
            n -= c
            s[1] = (s[1] - 1) & 0xFF


def apply_first_aid(p, s, w, supplies, skill_cap):
    """ApplyFirstAid? 0x39CA4 (EAX = patient, EDX = medic, EBX = kit charges, ECX = charges capped at 100) -> charges
    used. Nothing for a patient without wounds and hp 15+. Points = medic AP x ((dexterity + 2 x cap + 3 x medical +
    10 x level) / 7) / 50 (0: returns 0); a medical kit in hand (item 0x4A) gives half again. Points are capped by the
    need: 2 per hp below 15, plus 1 per wound beyond those. Charges: with the kit half the points (at least 1), else
    the points, at most the supplies (then points = 2 x / 1 x supplies). Hp below 15 heals first, 2 points per hp
    (wounds drop with it, at 0 at least); at 15 +0x186 / +0x187 = 0; 0x331A4(slot, 1). Points left close wounds one
    each when +0x186 is 0 (0x323D4(patient, 1)). The medic spends used x AP / uncapped points AP (kit: two thirds), 50 fatigue
    per AP, and practises medical (stat 5) used / 3 and dexterity (stat 3) used / 2 / 3 times."""
    if s8(p[0x185]) < 1 and s8(p[0x183]) >= 15:
        return 0
    ap = s8(s[0x1A5])
    full = cdiv(_i32(ap * cdiv(_i32(s8(s[0x18C]) + 2 * skill_cap + 3 * s8(s[0x18E]) + 10 * s8(s[0x192])), 7)), 50)
    if not full:
        return 0
    kit = s[0] == 0x4A
    pts = base = _i32(full + cdiv(full, 2)) if kit else full
    short = 15 - p[0x183] & 0xFF if s8(p[0x183]) < 15 else 0
    need = 2 * short
    if s8(p[0x185]) - short > 0:
        need += s8(p[0x185]) - short
    if need < pts:
        pts = need
    sup = _i32(supplies)
    if kit:
        used = cdiv(pts, 2)
        if used == 0 and pts > 0:
            used = 1
        if used > sup:
            used, pts = sup, _i32(2 * sup)
    else:
        used = pts
        if used > sup:
            used = pts = sup
    left = pts & 0xFF
    if left and s8(p[0x183]) < 15:
        if left >= 2 * short:
            p[0x183] = 15
            p[0x185] = (p[0x185] - short) & 0xFF
            left = (left - 2 * short) & 0xFF
        else:
            half = left // 2
            p[0x183] = (s8(p[0x183]) + half) & 0xFF
            p[0x185] = (s8(p[0x185]) - half) & 0xFF
            left %= 2
        if s8(p[0x185]) < 0:
            p[0x185] = 0
        if s8(p[0x183]) >= 15:
            p[0x187] = p[0x186] = 0
        w.ext("w_331a4", p[0x17A], 1)
    if left and p[0x185] and not p[0x186]:
        if s8(p[0x185]) <= left:
            left = (left - p[0x185]) & 0xFF
            p[0x185] = 0
        else:
            p[0x185] = (p[0x185] - left) & 0xFF
            left = 0
        w.ext("w_323d4", p, 1)
    pts = _i32(pts - left)
    spent = cdiv(_i32(pts * ap), base)
    if kit:
        spent = cdiv(_i32(spent * 2), 3)
    w.ext("spend_ap", s, s8(spent & 0xFF) & 0xFFFFFFFF, s16(spent * -50 & 0xFFFF) & 0xFFFFFFFF)
    if pts:
        w.ext("stat_practice", s, 5, s16(cdiv(pts, 3) & 0xFFFF) & 0xFFFFFFFF, 0)
    if cdiv(pts, 2):
        w.ext("stat_practice", s, 3, s16(cdiv(cdiv(pts, 2), 3) & 0xFFFF) & 0xFFFFFFFF, 0)
    return used


def first_aid_tick(s, w):
    """FirstAidTick? 0x15260 (turn start, task +0x1B9 5 or more, on the bandaging animation 0xF): the patient is the
    occupant of the partner tile +0xD6. No one there, a patient that is dying (+0x188), dead or not in task 4 / 6:
    "abandoned first aid" (EndTask). A patient without wounds and hp 15+: EndTask only. Else ApplyFirstAid with the
    hand entry's charges (cap 100); a patient now healed says "is all bandaged!" and ends its task; the used charges
    come off the hand entry; the selected merc's panel refreshes (0x55FD8(s, 0, 1)) unless 0x92DCB; no charges left:
    "ran out of bandages!" (EndTask). Messages last 1000 ticks."""
    if s[0x1B9] < 5 or u16(s, 0x1E8 + 2 * s[0x1F3]) != 0xF:
        return
    occ = _occupant(w, s16(u16(s, 0xD6)))
    msg = "abandoned first aid"
    if occ != 0x40:
        p = w.rec(occ)
        if not p[0x188] and p[0x183] and p[0x1B9] in (4, 6):
            if not p[0x185] and s8(p[0x183]) >= 15:
                w.ext("end_task", s, 1)
                return
            sup = item_charges(s, 0)
            used = apply_first_aid(p, s, w, sup, min(sup, 100))
            if not p[0x185] and s8(p[0x183]) >= 15:
                w.ext("merc_text", p, "is all bandaged!", 1000)
                w.ext("end_task", p, 1)
            use_charges(s, w, used)
            left = item_charges(s, 0)
            if not w.g8(0x92DCB) and s[0x17A] == w.g8(G_SELECTED):
                w.ext("item_panel", s, 0, 1)
            if left > 0:
                return
            msg = "ran out of bandages!"
    w.ext("merc_text", s, msg, 1000)
    w.ext("end_task", s, 1)


G_MARKER = 0x229CA8            # i32 kind, i32 tile (+4); 0x229D20 i32 = 1000, 0x229D40 i32 = 200: tile marker (0x1F634)


def tile_marker(w, kind, tile):
    """0x1F634 (AL = kind, EDX = tile): stores the marker kind and tile, 1000 and 200 in its timers"""
    w.s32(G_MARKER, kind & 0xFF)
    w.s32(G_MARKER + 4, tile)
    w.s32(0x229D40, 200)
    w.s32(0x229D20, 1000)


def first_aid_start(s, w, tile, task):
    """FirstAidStart? 0x3D750 (EAX = medic, EDX = tile, BL = task: 5 bandage the soldier there, 6 bandage itself)
    -> 1 when started. A medic already in a task (other than 4 asking for 6) gets "is already performing first aid!
    Press ESC to abort that first." (2000). Tasks other than 5 / 6 do nothing. No soldier on the tile: "No one there".
    A patient with no wounds and +0x186 not 1 says "doesn't need first aid" (1000). Else the medic drops its target
    (ClearTarget), pays 5 AP, takes the task with the tile (+0xD6), animation 0xE; the patient's counter +0x1CA rises.
    Task 5: +0x181 = 0xB8, the medic turns toward the patient; a patient not bandaging itself (task 6) gets task 4,
    +0x181 = 0xB9 and, with hp 15+ on an animation 4+ without ANIM_TURNS, drops its target, animation 4 and turns
    toward the medic; patient portrait. Task 6: +0x181 = 0xB7, facing 5. Medic portrait; tile marker 0x93."""
    task &= 0xFF
    if s[0x1B9] and not (s[0x1B9] == 4 and task == 6):
        w.ext("merc_text", s, "is already performing first aid! Press ESC to abort that first.", 2000)
        return 0
    if not 5 <= task <= 6:
        return 0
    occ = _occupant(w, tile)
    if occ >= 0x40:
        w.ext("message", "No one there")
        return 0
    p = w.rec(occ)
    if not p[0x185] and p[0x186] != 1:
        w.ext("merc_text", p, "doesn't need first aid", 1000)
        return 0
    clear_target(s, w, 0)
    w.ext("spend_ap", s, 5, 0)
    s[0x1B9] = task
    w16(s, 0xD6, tile)
    w.ext("set_anim", s, 0xE, 0, 0)
    p[0x1CA] = (p[0x1CA] + 1) & 0xFF
    if task == 5:
        s[0x181] = 0xB8
        s[0x1A2] = w.ext("direction", s16(u16(s, 0x116)), s16(u16(s, 0x118)),
                         s16(u16(p, 0x116)), s16(u16(p, 0x118))) & 0xFF
        if p[0x1B9] != 6:
            p[0x1B9] = 4
            p[0x181] = 0xB9
            anim = u16(p, 0x1E8 + 2 * p[0x1F3])
            if s8(p[0x183]) >= 15 and anim >= 4 and not w.g8(ANIM_TURNS + 8 * anim):
                clear_target(p, w, 0)
                w.ext("set_anim", p, 4, 0, 0)
                p[0x1A2] = w.ext("direction", s16(u16(p, 0x116)), s16(u16(p, 0x118)),
                                 s16(u16(s, 0x116)), s16(u16(s, 0x118))) & 0xFF
            w.ext("portrait", p[0x17A], 1)
    else:
        s[0x181] = 0xB7
        s[0x1A2] = 5
    w.ext("portrait", s[0x17A], 1)
    tile_marker(w, 0x93, tile)
    return 1


G_ENEMY_DEATHS = 0x229AF0     # i32: enemies killed (MercDies)
G_SQUAD_LOST = 0x22A10B       # u8: set by MercDies when no other merc 0..7 is in play in the sector
DEATH_FRAMES = 0x22A4EF       # g_backBuffer: 10 portrait frames of 0x1C bytes per row step (MercDies)


def merc_death_show(s, w, quiet, redraw=None):
    """MercDies? 0x3A4A4, its merc part before the squad check (the port's host op 186 death_show): 0x229D2C = none;
    not quiet: 0x67368 and, with 0x92DDF set, TacticalRedraw(1); the death portrait, sound, voice, 10 frames,
    "is dead!", FormatSectorDatName, 0x67398. redraw (--port): the port asked its effect pass (op 358) = the
    TacticalRedraw call; None = the original's condition."""
    slot = s[0x17A]
    w.s32(0x229D2C, NO_TILE)
    if not quiet:
        w.ext("mouse_hide")
    if (not quiet and w.g8(G_REDRAW_OFF)) if redraw is None else redraw:   # (redraw: the port asked op 358)
        w.ext("tac_redraw", 1)
    w.ext("open_dat", "dat\\mf1.dat" if s[0x179] < 0x1E else "dat\\mf2.dat")
    w.s32(0x229D18, w.ext("load_pcc", w.g32(0x92F48), None, f"morf{s[0x179]}.pcc", None, 0x180))
    w.ext("fx_load", "FX\\OVERHEAD", 0xFFFFFFFF, "DEATH")
    w.ext("merc_voice", s[0x179], 0xFFFFFFFF, "DEATH", 0, 0)
    w.ext("fx_play")
    w.ext("voc", 0x2B11, 0x6E, 1)
    for i in range(10):
        w.ext("blit", w.g32(0x93428 + 4 * slot) + 0x42, DEATH_FRAMES + 0x1C * i, 0x1C, 0x19, 0x20, 0x180)
        if not quiet:
            w.ext("w_320a0" if w.g8(G_REDRAW_OFF) >= 2 else "w_331a4", slot, 1)
            w.ext("delay", 10)
    if not quiet:
        w.ext("merc_text", s, "is dead!", 800)
        w.ext("sector_dat")
        w.ext("mouse_show")


def merc_dies(s, w, quiet):
    """MercDies? 0x3A4A4 (EAX = soldier, EDX = quiet): nothing when already dead (+0x188 = 1). Zeroes +0x195, wounds,
    hpMax, hp, AP; +0x1AC = 4, fatigue +0x10C = 10000.
    A merc (slot < 8): 0x229D2C = none; not quiet: 0x67368 and, with 0x92DDF set, TacticalRedraw(1). Its death
    portrait (morf<profile>.pcc from dat\\mf1.dat, profiles 30+ dat\\mf2.dat) loads into 0x229D18, the overhead death
    sound and the merc's DEATH voice play, then 10 portrait frames are drawn (each, not quiet, refreshes the panel:
    0x320A0 with 0x92DDF 2+, else 0x331A4 — and waits 10). Not quiet: "is dead!" (800), FormatSectorDatName,
    0x67398. With no mercs left in the sector's table (byte 2), 0x22A10B = 1 unless another merc 0..7 is in play.
    Others: +0x182 = 0; an enemy (slot 0x28+) while the sector has enemies (byte 1): enemy deaths + 1, sector enemies
    - 1; the last one runs 0x3A0B0; else a merc killer (+0x1DA < 8) that sees the body (SightLine from its point +2/+4
    to the body's, flag 1, 0xE, slot 0x40) queues quote 0xFA (0x1B2A4) and soldiers 0..15 in play with +0x1D5 0 and
    +0x1FE set idle (AiIdle). The enemy's sector byte (0x222BE2 + sector x 0xB1 + slot) = 0xFE; replay event T.
    Then: dead (+0x188 = 1), off the board (+0x138 = 0, +0x17E = 0), tile vacated, tile / +0x124 none, positions
    0, +0x1A8 = 0; a merc that leaves the squad lost (0x22A10B), not quiet: 0x17F10(1), selected = it, 0x3A32C."""
    if s[0x188] == 1:
        return
    s[0x195] = 0
    s[0x1AC] = 4
    s[0x185] = s[0x184] = s[0x183] = 0
    s[0x1A5] = 0
    w16(s, 0x10C, 10000)
    slot = s[0x17A]
    if slot < 8:
        merc_death_show(s, w, quiet)
        if not w.g8(G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR) + 2):
            w.s8(G_SQUAD_LOST, 1)
            for k in range(8):
                p = w.rec(k)
                if p[0x182] and p[0x138] and k != slot:
                    w.s8(G_SQUAD_LOST, 0)
                    break
    else:
        s[0x182] = 0
        sec = w.g8(G_CUR_SECTOR)
        if slot >= 0x28 and w.g8(G_SECTOR_TBL9 + 9 * sec + 1):
            w.s32(G_ENEMY_DEATHS, w.g32(G_ENEMY_DEATHS) + 1)
            left = (w.g8(G_SECTOR_TBL9 + 9 * sec + 1) - 1) & 0xFF
            w.s8(G_SECTOR_TBL9 + 9 * sec + 1, left)
            if not left:
                w.ext("sector_won")
            elif s[0x1DA] < 8:
                k = w.rec(s[0x1DA])
                if w.ext("sight_line", s[0x1DA], s16(u16(k, 0x116)) + 2, s16(u16(k, 0x118)) + 4, s16(u16(s, 0x114)),
                         s16(u16(s, 0x116)) + 2, s16(u16(s, 0x118)) + 4, 1, 0xE, 0x40):
                    w.ext("quote_event", s[0x1DA], 0xFA, 0, NO_TILE, 0)
                    for j in range(16):
                        p = w.rec(j)
                        if p[0x182] and p[0x138] and not p[0x1D5] and p[0x1FE]:
                            w.ext("idle", j)
            sec = w.g8(G_CUR_SECTOR)
            w.s8(0x222BE2 + 0xB1 * sec + slot, 0xFE)
            w.ext("replay_event", s8(sec) & 0xFFFFFFFF, s8(sec) & 0xFFFFFFFF, 0x54)
    s[0x188] = 1
    s[0x138] = 0
    s[0x17E] = 0
    vacate(w, s16(u16(s, 0x114)))
    w16(s, 0x114, NO_TILE)
    w16(s, 0x118, 0)
    w16(s, 0x124, NO_TILE)
    w16(s, 0x116, 0)
    w16(s, 0x128, 0)
    w16(s, 0x126, 0)
    s[0x1A8] = 0
    if slot < 8 and w.g8(G_SQUAD_LOST) and not quiet:
        if w.g8(G_SELECTED) == slot:
            s[0x1BD] = 0x6A         # (deselect writes the world, s is this merc's working copy)
        deselect(w, 1)
        w.s8(G_SELECTED, slot)
        w.ext("squad_lost")


def _ratio(dx, dy):
    """0x383E0: 1 if |dx| > 2 x |dy| steps (|dx| / |dy| > 2), 2 if |dy| / |dx| > 2, else 0 (C division)"""
    ax, ay = abs(_i32(dx)), abs(_i32(dy))
    if ax > ay:
        return 1 if cdiv(ax, ay) > 2 else 0
    return 2 if cdiv(ay, ax) > 2 else 0


def direction_px(x1, y1, x2, y2):
    """0x15A90: direction 1..8 (N, NE, E, SE, S, SW, W, NW) from point 1 toward point 2; 0 for the same point.
    Diagonal unless one axis is more than twice the other."""
    dx, dy = _i32(x2 - x1), _i32(y2 - y1)
    if dx == 0:
        return 1 if dy < 0 else 5 if dy > 0 else 0
    if dy == 0:
        return 7 if dx < 0 else 3
    r = _ratio(dx, dy)
    if dx < 0:
        return (8, 7, 1)[r] if dy < 0 else (6, 7, 5)[r]
    return (2, 3, 1)[r] if dy < 0 else (4, 3, 5)[r]


def at_waypoint(s):
    """0x1EE84: 1 unless the soldier (task +0x1B9 below 5, animation >= 4) is still short of its step target: tile
    +0x114 vs +0x124, pixel +0x116 / +0x118 vs +0x126 / +0x128"""
    if s[0x1B9] < 5 and u16(s, 0x1E8 + 2 * s[0x1F3]) >= 4:
        if u16(s, 0x114) != u16(s, 0x124) or u16(s, 0x116) != u16(s, 0x126) or u16(s, 0x118) != u16(s, 0x128):
            return 0
    return 1


def anim_can_walk(s, w, anim):
    """0x1EEEC: ANIM_CAN_ACT of the animation (signed), 1 for 0x31 / 0x32 anyway; 0 for animations below 2 when
    hp < 15 or energy 0"""
    anim = s16(anim)
    v = s8(w.g8(ANIM_CAN_ACT + 8 * anim))
    if not v and anim in (0x31, 0x32):
        v = 1
    if anim <= 1 and (s8(s[0x183]) < 15 or not s[0x18A]):
        v = 0
    return v


def clear_task(s, flag):
    """0x3DDB8: +0x1A6 = 0, +0x134 = none, +0x1A7 = 0; with flag also +0x95 = 0, +0x96 = none"""
    s[0x1A6] = 0
    w16(s, 0x134, NO_TILE)
    s[0x1A7] = 0
    if flag & 0xFF:
        s[0x95] = 0
        w16(s, 0x96, NO_TILE)


def start_leg(s, w):
    """0x1EF34: begin walking to the walk target +0x12A (none: +0x1FA = 0). Someone standing there and +0x1A7 clear
    with the target within 2 tiles: stop on the own tile (pixel target = its point, 0x196F8). Else the step target
    +0x124 takes it (+0x12A = none) and, unless +0x1FA, FindPath(s, it, 1); no path: a merc gets message 13 and
    clear_task, the soldier stops on its tile and faces the target (turn target +0x1A2). With a path the turn target
    is the first path direction + 1; +0x1FA = +0x1E1 = 0."""
    t = s16(u16(s, 0x12A))
    if t == NO_TILE:
        s[0x1FA] = 0
        return

    def stop():
        me = s16(u16(s, 0x114))
        w16(s, 0x126, tile_x(me))
        w16(s, 0x128, tile_y(me))
        w.ext("stop_anim", s, 1)
    if _occupant(w, t) < 0x40 and s[0x1A7] < 1 and _i32(w.ext("tile_dist", s16(u16(s, 0x114)), t)) < 3:
        stop()
        return
    w16(s, 0x124, u16(s, 0x12A))
    w16(s, 0x12A, NO_TILE)
    if not s[0x1FA] and not _i32(w.ext("find_path", s, s16(u16(s, 0x124)), 1)):
        if s[0x17A] < 8:
            w.ext("merc_message", s, 0xD, 0x40, None, 1)   # ECX = the message text (not compared)
            clear_task(s, 1)
        stop()
        t = s16(u16(s, 0x124))
        s[0x1A2] = direction_px(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(t), tile_y(t)) & 0xFF
        return
    s[0x1A2] = (w.g8(G_PATHS + 0x50 * s[0x17A]) + 1) & 0xFF
    s[0x1FA] = 0
    s[0x1E1] = 0


def _water_anim(s, base):
    """standing / wading animation by the ground under the soldier: base, base + 3 (0x97..0x99), base + 6 (0x9A+)"""
    g = s[0x1B2]
    return base if g < 0x97 else base + 3 if g < 0x9A else base + 6


def anim_step(s, w):
    """AnimStep? 0x1F2C4: pick the soldier's next animation and start it (SetAnim 0x17F90 with 1, 0). Facing the turn
    target clears the turn way. Walking on when not at the step point (start the next leg if a walk target is set),
    or at it with a walk target whose animation allows walking (stepping animations first face the target): +0x1B8
    = 0 and code 2 (anim 0), 0xD (1..2), 0x15 / 0x16 (+0x1A8) / 0x18 (+0x177) on land, 0x34 / 0x37 / 0x3A in water;
    end of a task step (0x3D98C, 0). Standing: type 2 code 0x11 and turn target +0x1A4; else the task ends when
    +0x1B9 >= 5 without +0x195 (0x3D98C, 1), code 4 unless animation 5, and +0x1CC re-places the soldier (0x2FB98);
    in water 0x33 / 0x36 / 0x39. A soldier hit (+0x195, cleared while alive) outside combat mode, slot < 16, killed
    by a merc (+0x1DA < 8), not selected, hp >= 15, on land, animation >= 4, with a gun (items 1..0x10) and code 4
    fires back: SetAnim(4, 1, 0), +0x1E1 = 0, TacticalClick at the attacker's tile with the gun. A queued
    animation +0x1E1 (signed) replaces the code; +0x1E2 += +0x1E3, +0x1E3 = 0."""
    anim = u16(s, 0x1E8 + 2 * s[0x1F3])
    code = 1000
    if s[0x1A2] == s[0x1A1]:
        s[0x1A3] = 0
    if at_waypoint(s):
        t = s16(u16(s, 0x12A))
        moving = False
        if t != NO_TILE and anim_can_walk(s, w, anim):
            if w.g8(ANIM_TURNS + 8 * s16(anim)):
                s[0x1A2] = direction_px(s16(u16(s, 0x116)), s16(u16(s, 0x118)), tile_x(t), tile_y(t)) & 0xFF
            start_leg(s, w)
            moving = True
    else:
        moving = True
        if s16(u16(s, 0x12A)) != NO_TILE:
            start_leg(s, w)
    if moving:
        s[0x1B8] = 0
        if s16(anim) < 0x32:
            if anim == 0:
                code = 2
            elif anim <= 2:
                code = 0xD
            else:
                code = 0x18 if s[0x177] else 0x15 + (s[0x1A8] != 0)
        else:
            code = _water_anim(s, 0x34)
        w.ext("end_task", s, 0)
    elif s16(anim) < 0x32:
        if s8(s[0xBA]) == 2:
            code = 0x11
            s[0x1A2] = s[0x1A4]
        else:
            if s[0x1B9] >= 5 and not s[0x195]:
                w.ext("end_task", s, 1)
            if s16(anim) != 5:
                code = 4
            if s[0x1CC]:
                s[0x1CC] = 0
                w.ext("replace", s)
    else:
        code = _water_anim(s, 0x33)
    if s[0x195]:
        if s[0x183]:
            s[0x195] = 0
            item = s[0]
            if (not w.g8(G_COMBAT) and s[0x17A] < 0x10 and s[0x1DA] < 8 and w.g8(G_SELECTED) != s[0x17A]
                    and s8(s[0x183]) >= 15 and s[0x1B2] < 0x6C and u16(s, 0x1E8 + 2 * s[0x1F3]) >= 4
                    and 0 < item <= 0x10 and code == 4):
                w.ext("set_anim", s, code, 1, 0)
                s[0x1E1] = 0
                code = 1000
                w.ext("click", s, s16(w.g16(G_SOLDIERS + 0x200 * s[0x1DA] + 0x114)), item)
        if s[0x1E1]:
            code = s8(s[0x1E1])
            s[0x1E1] = 0
            s[0x1E2] = (s[0x1E2] + s[0x1E3]) & 0xFF
            s[0x1E3] = 0
    if code < 1000:
        w.ext("set_anim", s, code & 0xFFFF, 1, 0)


G_PATH_SAVE, G_CONFIRM_MOVE, G_CURSOR = 0xEB110, 0x1163C6, 0x284F1D   # 0x50 B path copy; confirm option; cursor id
G_ALT, G_SHIFT, G_WALK_AP = 0x284F20, 0x284F21, 0x229DF0


def move_merc(s, w, tile, marker, sneak, b177, alt, confirm, squad):
    """MoveMercTo? 0x1C608 (EAX = soldier, EDX = tile, BL = target marker, CL = sneak; stack bytes: +0x177, Alt,
    confirm, squad) -> 1 = walking. Nothing (0) for slot 0x28+, a soldier out of the sector, hp below 15, +0x17E set
    or an animation code above 0x3C. A soldier with posture 3, +0x1D5 > 0 and a living soldier on +0xD0 that holds a
    loaded gun (HandGunState, +1) below item 0xF gets message 8 and 0 (an item 0xF/0x10 only when +0xD0 is one tile
    away on its own pixel point, which never holds). An unstandable tile (TileStandable <= 0) without +0xBA 2 gives 0
    and "Can't move there!" unless it is the own tile or 0xC41. A merc with the confirm option (0x1163C6) and confirm
    (not squad) keeps a copy of its path, prices it with the path marked (PathApCost 1, 0, sneak, b177, Alt; <= 0:
    the message, 0), sets the cursor 0x8F and waits for the confirm click (0x1C514, cursor 0x1C464); none restores the
    path, 0. Squad: every other merc 0..7 (pointer table 0x100958) moves the same way (marker 1, Shift, Alt). Without
    the confirm the path is priced again unmarked (+0xD4 = 0); with it 0x229DF0 is kept. Enough AP (MsgNotEnoughAP
    0x229DF0, 1): +0x197 = 0, StopWalk(1), the marker (0x17B70 with 0x1163C7; 0x229CA8 / AC / D20 / D40), step point
    = position if the animation allows (0x93166), +0x177 / +0x1A8 / +0x196 / +0x1FA = 1, WalkTo, 0x32EB8, +0xB0 = 0,
    1. Else +0x120 = tile, the same bytes, 0x32EB8, 0."""
    if s[0x17A] >= 0x28 or not s[0x182] or s8(s[0x183]) < 15 or s[0x17E] or u16(s, 0x1E8 + 2 * s[0x1F3]) > 0x3C:
        return 0
    t = s16(u16(s, 0xD0))
    if s[0x19C] == 3 and s8(s[0x1D5]) > 0 and t != NO_TILE:
        o = w.tile8(t, 5) & 0x7F              # TileOccupant: 0x30 and up = none
        if o < 0x30 and s8(w.g8(G_SOLDIERS + 0x200 * o + 0x183)) > 0 and hand_gun_state(s) and s[1]:
            me = s16(u16(s, 0x114))
            if s[0] < 0xF or (_i32(w.ext("tile_dist", me, t)) == 1 and me - cdiv(me, 56) * 56 == t - cdiv(t, 56) * 56
                              and cdiv(me, 56) == cdiv(t, 56)):
                w.ext("merc_message", s, 8, 0x40, 0x903AC, 1)
                return 0
    if tile_standable(w, tile) <= 0 and s8(s[0xBA]) != 2:
        if s16(u16(s, 0x114)) != tile and tile != NO_TILE:
            w.ext("message", "Can't move there!")
        return 0
    held = None
    if s[0x17A] < 8 and w.g8(G_CONFIRM_MOVE) and confirm & 0xFF and not squad & 0xFF:
        path = [w.g8(G_PATHS + 0x50 * s[0x17A] + i) for i in range(0x50)]
        for i, b in enumerate(path):
            w.s8(G_PATH_SAVE + i, b)
        if _i32(w.ext("path_ap", s, tile, 1, 0, s8(sneak & 0xFF), s8(b177 & 0xFF), s8(alt & 0xFF))) <= 0:
            w.ext("message", "Can't move there!")
            return 0
        w.s8(G_CURSOR, 0x8F + bool(squad & 0xFF))
        ok = _i32(w.ext("confirm_click", tile))
        w.ext("w_1c464", s)
        if not ok:
            for i in range(0x50):
                w.s8(G_PATHS + 0x50 * s[0x17A] + i, w.g8(G_PATH_SAVE + i))
            return 0
        held = w.g32(G_WALK_AP)
    if squad & 0xFF:
        for i in range(8):
            if i != s[0x17A]:
                move_merc(w.rec(i), w, tile, 1, s8(w.g8(G_SHIFT)), 0, s8(w.g8(G_ALT)), 0, 0)
    if held is not None:
        w.s32(G_WALK_AP, held)
    else:
        w16(s, 0xD4, 0)
        w.ext("path_ap", s, tile, 0, 0, s8(sneak & 0xFF), s8(b177 & 0xFF), s8(alt & 0xFF))
    if not msg_not_enough_ap(s, w, w.g8(G_WALK_AP), 1):
        w16(s, 0x120, tile)
        s[0x177], s[0x196], s[0x1A8] = b177 & 0xFF, alt & 0xFF, sneak & 0xFF
        w.ext("w_32eb8", s[0x17A], 1)
        return 0
    s[0x197] = 0
    w.ext("stop_walk", s, 1)
    if w.g8(0x1163C7) and marker & 0xFF:
        w.ext("w_17b70", s[0x17A], 1)
    if marker & 0xFF:
        w.s32(0x229CA8, 0x8A)
        w.s32(0x229CAC, tile)
        w.s32(0x229D20, 1000)
        w.s32(0x229D40, 200)
    if w.g8(ANIM_CAN_ACT + 8 * u16(s, 0x1E8 + 2 * s[0x1F3])):
        w16(s, 0x126, u16(s, 0x116))
        w16(s, 0x128, u16(s, 0x118))
    s[0x177], s[0x1A8], s[0x196], s[0x1FA] = b177 & 0xFF, sneak & 0xFF, alt & 0xFF, 1
    w.ext("walk_to", s, tile)
    w.ext("w_32eb8", s[0x17A], 1)
    _w32(s, 0xB0, 0)
    return 1


def walk_to(s, w, tile):
    """WalkTo? 0x195A0: walk target +0x12A and +0x120 = tile, lastTargetTile +0xD0 = none; then AiIdle if the
    current animation has ANIM_TURNS set or +0x1D5 > 0, and +0x1FE is set; otherwise one animation step (0x1F2C4)
    does the walking."""
    w16(s, 0x12A, tile)
    w16(s, 0x120, tile)
    w16(s, 0xD0, NO_TILE)
    if (w.g8(ANIM_TURNS + 8 * u16(s, 0x1E8 + 2 * s[0x1F3])) or s8(s[0x1D5]) > 0) and s[0x1FE]:
        w.ext("idle", s)
    else:
        w.ext("anim_step", s)


def turn_step(a, b):
    """0x19540: which way to turn from direction a to b (1..8): 0 same, 1 clockwise, -1 counter-clockwise; exactly
    opposite (|a - b| == 4) turns clockwise"""
    if a == b:
        return 0
    d = abs(a - b)
    if d == 4:
        return 1
    if a > b:
        return -1 if d <= b + 8 - a else 1
    return 1 if d <= 8 - b + a else -1


G_AP_WARNED = 0x22A0EC        # u8 per merc slot: the low-AP stop already happened (AP reserve +0xEF no longer kept)
G_STOP_TIMER = 0x95DC8        # i32, set to 2000 by the low-AP stop


def ap_short(s, w, cost):
    """0x1A9A8 -> 1 if a merc (slot < 8, +0x16B set) in combat mode has fewer AP left than `cost` (less its AP
    reserve +0xEF until the first stop): marks the stop (G_AP_WARNED, G_STOP_TIMER = 2000). Others: 0."""
    slot = s[0x17A]
    ap = s8(s[0x1A5])
    if slot >= 8 or not s[0x16B]:
        return 0
    if not w.g8(G_AP_WARNED + slot):
        ap -= s8(s[0xEF])
    if not w.g8(G_COMBAT) or cost <= ap:
        return 0
    w.s32(G_STOP_TIMER, 2000)
    w.s8(G_AP_WARNED + slot, 1)
    return 1


def turn_to(s, w, d, ticks):
    """TurnTo? 0x1AA18 -> 1 if a turn toward direction d starts. Nothing (0) if already facing d or the current
    animation code (low byte) is below 4. +0x1A3 = turn_step(facing, d). Animations 4..10 turn (and start
    animation 8 first); others need ANIM_CAN_ACT, and without ANIM_TURNS only codes below 0x3D turn (below 0x32 also
    start animation 8). A turning soldier stops there when short of 1 AP (0x1A9A8, still returns 1); else +0x1A2 =
    d (turn target), sound 1/10, +0xD2 = ticks (signed byte)."""
    anim = s[0x1E8 + 2 * s[0x1F3]]
    d &= 0xFF
    if d == s8(s[0x1A1]) or anim < 4:
        return 0
    s[0x1A3] = turn_step(s8(s[0x1A1]), d) & 0xFF
    start = ok = False
    if anim <= 10:
        start = ok = True
    elif w.g8(ANIM_CAN_ACT + 8 * anim):
        if w.g8(ANIM_TURNS + 8 * anim):
            ok = True
        elif anim < 0x32:
            start = ok = True
        elif anim < 0x3D:
            ok = True
    if not ok:
        return 0
    if not ap_short(s, w, 1):
        s[0x1A2] = d
        if start:
            w.ext("set_anim", s, 8, 0, 0)
        w.ext("spend_ap", s, 1, 10)
        w16(s, 0xD2, s8(ticks & 0xFF))
    return 1


G_SEEN_ENEMIES = 0x22A478     # u16: enemies in play that are seen (counted by the soldier loop each frame)
G_HITS = 0x92DC4              # u8: sum of the soldiers' +0x195 (in flight / hit count); FireNow waits for 0
G_FX_ANIM = 0xEAFF0           # 16 x 0x12 B: +0 count (s8), +1 timer (i32, 140 ms per count; the 10 ms tick)
G_TICK_STEP = 0x92F60         # i32: ms the 10 ms tick takes off the soldier timers (10; 1000 = fast, 0x11D40)


def soldier_loop(w):
    """TacticalLoop 0x107BC, its soldier part 0x10C5B..0x10F4E (one frame; runs when the frame flag 0x284F14 is 0) ->
    the sum of the NewTurn results it adds to the loop's counter. For every slot 0..0x2F: in the sector, in play and
    +0x188 = 0: seen enemies (0x28+, s8 +0x9C > 0) counted in 0x22A478, +0x195 summed into 0x92DC4; timer +0xB0 <= 0:
    ActiveFrame for the soldier in turn (+0x1B0 = 0 and +0x17A = 0x284F11) or any slot < 0x10 out of combat, else
    SoldierFrame; the +0xC4 / +0x1D2 countdown (+0xD8 / +0xDA down every 4th), merc portrait / idle timers +0xBC /
    +0xA8; ProjectileStep for +0x1BF = 3. Then for every slot (any state) whose turn is over (+0x1B0, out of the
    sector or play, +0x188) while it holds the turn (+0x17A = 0x284F11): the hand-over (ItqPop, 0x12DD4, the next
    enemy + banner, 0x12404 or NewTurn(1)). After the loop: the fx anim 0x229E04 ends when its count is out; FireNow
    when an attack waits (0x284F28 < 0x40), nothing is in flight (0x92DC4 = 0) and no fx anim runs."""
    w.s8(G_HITS, 0)
    w.s16(G_SEEN_ENEMIES, 0)
    total = 0
    for i in range(0x30):
        s = w.rec(i)
        if s[0x182] and s[0x138] and not s[0x188]:
            if i >= 0x28 and s8(s[0x9C]) > 0:
                w.s16(G_SEEN_ENEMIES, w.g16(G_SEEN_ENEMIES) + 1)
            w.s8(G_HITS, w.g8(G_HITS) + s[0x195])
            if _i32(_u32(s, 0xB0)) <= 0:
                if (not s[0x1B0] and w.g8(G_SELECTED) == s[0x17A]) or (i < 0x10 and not w.g8(G_COMBAT)):
                    w.ext("frame_active", s)
                elif _i32(_u32(s, 0xB0)) <= 0:
                    w.ext("frame_other", s)
            if _u32(s, 0xC4) == 0:
                s[0x1D2] = (s[0x1D2] - 1) & 0xFF
                if s8(s[0x1D2]) - cdiv(s8(s[0x1D2]), 4) * 4 == 0:
                    w16(s, 0xD8, u16(s, 0xD8) - 1)
                    w16(s, 0xDA, u16(s, 0xDA) - 1)
                _w32(s, 0xC4, 10 if s8(s[0x1D2]) > 0 else 0xFFFFFFFF)
            if i < 8:
                if _i32(_u32(s, 0xBC)) <= 0 and s8(s[0x189]) > 0:
                    s[0x189] = (s[0x189] - 1) & 0xFF
                    w.ext("portrait", s[0x17A], 1)
                    _w32(s, 0xBC, 0x96)
                if _i32(_u32(s, 0xA8)) <= 0:
                    s[0x1AC] = (s[0x1AC] + 1) & 0xFF
                    r = 0
                    if s8(s[0x1AC]) > 0x14:
                        s[0x1AC] = 0
                    if s8(s[0x1AC]) < 3 and w.g8(0x92DC8) != s[0x17A] and not s[0x194] and s[0x1A5]:
                        w.ext("portrait", i, 1)
                    else:
                        r = w.random(0x28)
                    _w32(s, 0xA8, r + 100)
            if s[0x1BF] == 3:
                w.ext("projectile_step", s)
        if (s[0x1B0] or not s[0x182] or not s[0x138] or s[0x188]) and s[0x17A] == w.g8(G_SELECTED):
            r = w.ext("itq_pop") & 0xFF
            if r < 0x40:
                if r < 8:
                    w.ext("w_12dd4")
                else:
                    w.s8(G_SELECTED, r)
                    w.ext("turn_banner")
            elif w.g8(0x284F0F) < 8:
                w.ext("w_12dd4")
            elif w.g8(G_SELECTED) < 0x2F:
                if w.g8(G_SELECTED) >= 8:
                    w.s8(G_SELECTED, w.g8(G_SELECTED) + 1)
                    w.ext("turn_banner")
            elif not w.g8(0x92DDC):
                w.ext("w_12404")
            elif w.g8(G_COMBAT):
                total += w.ext("new_turn", 1) & 0xFF
    a = _i32(w.g32(0x229E04))
    if a >= 0 and s8(w.g8(G_FX_ANIM + 0x12 * a)) <= 0:
        w.s32(0x229E04, 0xFFFFFFFF)
    if w.g8(G_ACTIVE) < 0x40 and not w.g8(G_HITS) and _i32(w.g32(0x229E04)) == -1:
        w.ext("fire", w.g8(G_ACTIVE))
    return total & 0xFF


def soldier_ticks(w):
    """The 10 ms timer callback 0x5EE98 (AIL timer, period 10000 us; registered by 0x5F2B0 at the tactical start), its
    soldier part: nothing while 0x92DE1 or the re-entry flag 0x92DB8 is set, or (after the global timers) while 0x92DE0
    is set. The stop timer 0x95DC8 counts down by 10 to 0 (written back even when the callback does nothing).
    For every slot in the sector and in play: +0xB0 and +0xCC lose the tick step 0x92F60; +0xA8 loses 10 when hp is
    15+ and the screen is drawn (0x92DCC); +0xC0 loses 10 while > 0; +0xC4 loses 10 while > 0 (below 0: -1); +0xB4
    loses 10 while > 0 (not below 0); mercs (0..7): +0xBC loses 10 while s8 +0x189 > 0, +0x1DC loses 10 while > 0.
    Two turn timers too: the show timer 0x93D54 (before the 0x92DE0 gate, 0x5EF9E) loses the tick step while > 0; at
    0 or below it is 0 and 0x229CB4 (shown slot) = 0x229C94 = 0x40, 0x229C8C = 0xC41. The turn clock 0x229D1C (after
    the gate, 0x5F065) loses the tick step while > 0 (below 0: 0). Before it the busy timer 0x92DB4 loses 10 while
    > 0; at 0 or below it is 0 and the busy counter 0x92E38 = 0. Between the two the fast timer 0x92DB0 (set by the
    turn start 0x1054C) loses 10 while > 0; at 0 or below it is 0 and turn-based 0x92DAC = 1.
    The tile and effect animations of the same loop are front_ticks (below). The other timers of the callback are not
    mirrored (Undefined if they matter)."""
    stop = _i32(w.g32(0x95DC8))
    if w.g8(0x92DB8):
        raise Undefined("timer callback re-entered")
    if w.g8(0x92DE1):
        return
    for a in (G_AMBIENT_T1, G_AMBIENT_T2, G_BLINK_TIMER):     # (0x5EECC: with other UI timers, not mirrored)
        w.s32(a, (w.g32(a) - 10) & 0xFFFFFFFF)
    step = w.g32(G_TICK_STEP)
    show = _i32(w.g32(0x93D54))
    if show > 0:
        show = _i32((show - step) & 0xFFFFFFFF)
        w.s32(0x93D54, show)
        if show <= 0:
            w.s32(0x93D54, 0)
            w.s32(0x229CB4, 0x40)
            w.s32(0x229C94, 0x40)
            w.s32(0x229C8C, 0xC41)
    if w.g8(0x92DE0):
        return
    busy_t = _i32(w.g32(0x92DB4))          # the busy timer (0x5F013): at 0 nobody is busy any more
    if busy_t > 0:
        busy_t -= 10
        w.s32(0x92DB4, busy_t)
        if busy_t <= 0:
            w.s32(0x92DB4, 0)
            w.s32(G_BUSY, 0)
    fast = _i32(w.g32(G_FAST_TIMER))       # 15 s of real time after a turn start (0x5F03A): then turn-based again
    if fast > 0:
        fast -= 10
        w.s32(G_FAST_TIMER, fast)
        if fast <= 0:
            w.s32(G_TURN_BASED, 1)
            w.s32(G_FAST_TIMER, 0)
    clock = _i32(w.g32(0x229D1C))
    if clock > 0:
        clock = _i32((clock - step) & 0xFFFFFFFF)
        w.s32(0x229D1C, max(clock, 0))
    w.s32(G_TURN_TICK, (w.g32(G_TURN_TICK) - 10) & 0xFFFFFFFF)
    w.s32(G_RIPPLE_TIMER, (w.g32(G_RIPPLE_TIMER) - 10) & 0xFFFFFFFF)
    if stop > 0:
        stop -= 10
        if stop < 0:
            stop = 0
    for i in range(0x30):
        s = w.rec(i)
        if not (s[0x182] and s[0x138]):
            continue
        _w32(s, 0xB0, (_u32(s, 0xB0) - step) & 0xFFFFFFFF)
        _w32(s, 0xCC, (_u32(s, 0xCC) - step) & 0xFFFFFFFF)
        if s8(s[0x183]) >= 15 and w.g8(0x92DCC):
            _w32(s, 0xA8, (_u32(s, 0xA8) - 10) & 0xFFFFFFFF)
        if _i32(_u32(s, 0xC0)) > 0:
            _w32(s, 0xC0, (_u32(s, 0xC0) - 10) & 0xFFFFFFFF)
        if _i32(_u32(s, 0xC4)) > 0:
            _w32(s, 0xC4, (_u32(s, 0xC4) - 10) & 0xFFFFFFFF)
            if _i32(_u32(s, 0xC4)) < 0:
                _w32(s, 0xC4, 0xFFFFFFFF)
        if _i32(_u32(s, 0xB4)) > 0:
            _w32(s, 0xB4, (_u32(s, 0xB4) - 10) & 0xFFFFFFFF)
            if _i32(_u32(s, 0xB4)) <= 0:
                _w32(s, 0xB4, 0)
        if i < 8:
            if s8(s[0x189]) > 0:
                _w32(s, 0xBC, (_u32(s, 0xBC) - 10) & 0xFFFFFFFF)
            if _i32(_u32(s, 0x1DC)) > 0:
                _w32(s, 0x1DC, (_u32(s, 0x1DC) - 10) & 0xFFFFFFFF)
    for i in range(5):                     # the timed effects' fuses (0x5F214): +0 counts down while +9 <= 0
        f = G_FX + 13 * i
        if _i32(w.g32(f)) <= 0:
            continue
        if s8(w.g8(f + 9)) <= 0:
            w.s32(f, max(_i32(w.g32(f)) - 10, 0))
        if w.g8(f + 0xC) and _i32(w.g32(0x229E0C)) > 0:     # a ticking fuse: 0x284F29 = 1 every 700 ms
            t = _i32(w.g32(0x229E0C)) - 10
            w.s32(0x229E0C, t & 0xFFFFFFFF)
            if t <= 0:
                w.s8(0x284F29, 1)
                w.s32(0x229E0C, 700)
    w.s32(0x95DC8, stop)
    front_ticks(w, gated=False)



def start_tile_anim(w, tile, frames, count):
    """StartTileAnim 0x15720 (EAX tile, EDX frame table, BL count): the first tile animation k = 1..9 (record 0 is
    never used) with count byte 0 gets count - 1, +7 = the frame table, +5 = the tile (the timer +1 is left as it
    is); if the new count is not 0 the tile's byte 3 = k. No free record: nothing."""
    for k in range(1, 10):
        a = G_TILE_ANIMS + 0xB * k
        if w.g8(a):
            continue
        c = (count - 1) & 0xFF
        w.s32(a + 7, w.ptr(frames))
        w.s8(a, c)
        w.s16(a + 5, tile & 0xFFFF)
        if c:
            w.s8(G_TILEMAP + 8 * tile + 3, k)
        return


G_RIPPLE_TIMER = 0x92EF8       # i32: ms to the next water-ripple try (sector start 250; the 10 ms timer takes 10 off)
G_AMBIENT_T1, G_AMBIENT_T2 = 0x229D30, 0x229D34    # i32: palette-cycle timers 200 / 300 ms (10 ms timer: -10 each)
G_BLINK_TIMER = 0x229D48       # i32: the UI blink timer 800 / 400 ms (10 ms timer: -10)


def update_rest(w):
    """TacticalLoop 0x107BC, its update pass after the soldier loop (0x10F4E..0x111EE, run while 0x284F14 = 0);
    returns what it adds to the loop's leave counter [ebp - 4] (0 or 1).
    (1) Sector won: sector_state row (9 B) of the current sector: bit 0 of byte 0 clear and byte 1 (enemies) = 0:
    0x3FA54(s8 sector) (strategic: marks the sector won), StatPractice(merc, 9, 10, 0) for mercs 0..7 in play with
    hp 15+, then in sector 0 only: 0x61688(0x1D, 0x63, 1), 0x93449 = 1 and +1. Bit 0 set, byte 1 > 0 and 0x22A0CF
    = 0: MsgEnemiesNearby (sets 0x22A0CF = 1), music 0x5C5EC(2, 0).
    (2) UI blink: timer 0x229D48 <= 0: 0x33768(0); 0x284F06 ^= 1, timer 800 if it is now 1 else 400; 0x284F04 + 1
    (0 again when s8 > 2); u16 0x93AAA = 190 + 2 x s8 0x284F04.
    (3) Water ripples: timer 0x92EF8 <= 0: t = Random(3136) (kept in 0x229AFC); ground byte >= 0x97, object byte 0,
    no occupant (TileOccupant = 0x40) and Random(s8 splash[sector]) = 0: start_tile_anim(t, 0x95659, 0x12); timer =
    s8 splash[sector] x 12.
    (4) Ambient: timer 0x229D30 <= 0 and option 0x1163C0: palette cycle 0x528AC(0xEDA08, 0xAC, 0xB2), timer 200;
    timer 0x229D34 <= 0: palette cycle (.., 0xA5, 0xAB), timer 300, 0x284F19 + 1 (below 0xFE, else 0xF8), Random(20)
    = 0 and time compression 0x9300C = 0: PlaySfxSlot(Random(6) + 0x24, 0x2B11, 0x23, 1) (Random(6) only then);
    then (still under the first timer) palette 0x52870(0xEDA08, 0xA5, 0xB2).
    (5) 0x284F14 = 1."""
    add = 0
    sec = w.g8(G_CUR_SECTOR)
    row = G_SECTOR_TBL9 + 9 * sec
    b0, b1 = w.g8(row), w.g8(row + 1)
    if not b0 & 1:
        if b1 == 0:
            w.ext("sector_gained", s8(sec) & 0xFFFFFFFF)
            for k in range(8):
                p = w.rec(k)
                if p[0x182] and s8(p[0x183]) >= 15 and p[0x138]:
                    w.ext("stat_practice", k, 9, 10, 0)
            if sec == 0:
                w.ext("w_61688", 0x1D, 0x63, 1)
                w.s8(0x93449, 1)
                add = 1
    elif b1 and not w.g8(0x22A0CF):
        w.ext("enemies_msg")
        w.s8(0x22A0CF, 1)
        w.ext("music", 2, 0)
    if _i32(w.g32(G_BLINK_TIMER)) <= 0:
        w.ext("w_33768", 0)
        v = w.g8(0x284F06) ^ 1
        w.s8(0x284F06, v)
        w.s32(G_BLINK_TIMER, 800 if v else 400)
        c = (w.g8(0x284F04) + 1) & 0xFF
        if s8(c) > 2:
            c = 0
        w.s8(0x284F04, c)
        w.s16(0x93AAA, (190 + 2 * s8(c)) & 0xFFFF)
    if _i32(w.g32(G_RIPPLE_TIMER)) <= 0:
        t = w.random(3136)
        w.s32(0x229AFC, t & 0xFFFFFFFF)
        splash = s8(w.g8(G_SPLASH + sec))
        tb = G_TILEMAP + 8 * t
        if w.g8(tb) >= 0x97 and w.g8(tb + 1) == 0 and (w.g8(tb + 5) & 0x7F) >= 0x30 and w.random(splash) == 0:
            start_tile_anim(w, t, 0x95659, 0x12)
        w.s32(G_RIPPLE_TIMER, (splash * 12) & 0xFFFFFFFF)
    if _i32(w.g32(G_AMBIENT_T1)) <= 0 and w.g8(0x1163C0):
        w.ext("pal_cycle", 0xEDA08, 0xAC, 0xB2)
        w.s32(G_AMBIENT_T1, 200)
        if _i32(w.g32(G_AMBIENT_T2)) <= 0:
            w.ext("pal_cycle", 0xEDA08, 0xA5, 0xAB)
            w.s32(G_AMBIENT_T2, 300)
            c = w.g8(0x284F19)
            w.s8(0x284F19, c + 1 if c < 0xFE else 0xF8)
            if w.random(20) == 0 and w.g8(0x9300C) == 0:
                w.ext("sfx", w.random(6) + 0x24, 0x2B11, 0x23, 1)
        w.ext("pal_set", 0xEDA08, 0xA5, 0xB2)
    w.s8(0x284F14, 1)
    return add


def fx_fuses(w):
    """TacticalLoop 0x11228..0x1130D, every pass (update and draw): a timed effect (G_FX, 13 B) whose i32 +0 is
    exactly 0 (its fuse ran out in the 10 ms timer) and s8 +9 <= 0 goes off and frees (+0 = -1): on a map tile (s16
    +6 below 0xC41) Explode(+0xB, tile, kind +8, 1) when the ground item there has blast kind +8 or when +8 = 0xA (any
    tile, item or not); off the map 0x355D0(+8, s8 +0xA, s16 +4, +0xB, s8 +0xC). (End turn's own pass is end_turn.)"""
    for i in range(5):
        f = G_FX + 13 * i
        if w.g32(f) != 0 or s8(w.g8(f + 9)) > 0:
            continue
        w.s32(f, 0xFFFFFFFF)
        tile = s16(w.g16(f + 6))
        k = w.g8(f + 8)
        if tile < NO_TILE:
            idx = item_index(w, tile)
            if (idx != 0xFF and explosive_kind(w.g8(G_GROUND_ITEMS + 12 * idx)) == k) or k == 0xA:
                w.ext("explode", w.g8(f + 0xB), tile & 0xFFFFFFFF, k, 1)
        else:
            w.ext("w_355d0", k, s8(w.g8(f + 0xA)) & 0xFFFFFFFF, s16(w.g16(f + 4)) & 0xFFFFFFFF,
                  w.g8(f + 0xB), s8(w.g8(f + 0xC)) & 0xFFFFFFFF)


G_TURN_TICK = 0x229D24         # i32: 100 ms step timer of the turn timers (10 ms timer: -10 after the 0x92DE0 gate)


def combat_end(w):
    """0x106F4 (enemies gone in combat): every soldier 0..0x27 in play (+0x182, +0x138) with hp byte != 0: AP +0x1A5 =
    1, +0x1B0 = +0x194 = +0x17E = +0x197 = 0, AiIdle (op 1); screen_drawn 0x92DCC = 1, new_round 0x22A0CD = 1;
    selected >= 8: selected = FirstMerc; selected < 8: select light 0x46A4C(selected, 0xC); 0x92DDC = 0x92DCE = 0;
    LoadStripPccs(1, 1)."""
    for k in range(0x28):
        p = w.rec(k)
        if p[0x182] and p[0x138] and p[0x183]:
            p[0x1A5] = 1
            for o in (0x1B0, 0x194, 0x17E, 0x197):
                p[o] = 0
            w.ext("idle", p)
    w.s8(0x92DCC, 1)
    w.s8(0x22A0CD, 1)
    if w.g8(G_SELECTED) >= 8:
        w.s8(G_SELECTED, first_merc(w))
    if w.g8(G_SELECTED) < 8:
        w.ext("w_46a4c", w.g8(G_SELECTED), 0xC)
    w.s8(0x92DDC, 0)
    w.s8(0x92DCE, 0)
    w.ext("strip", 1, 1)


def loop_head(w):
    """TacticalLoop 0x108D8..0x10AC1, every pass before the key handler; returns what it adds to the leave counter
    (the NewTurn results, 8-bit).
    (1) Turn timers, every 100 ms (0x229D24 <= 0, then 10 again): screen not drawn (0x92DCC = 0): s16 0x22A484 + 1
    while below 20; drawn: 0x22A484 > 0 -> - 1, else s16 0x22A486 > 0 and new_round 0x22A0CD = 1 -> 0x22A486 - 1.
    (2) u8 0x229E00 < 0x30: 0x56D3C (weapon status text, UI).
    (3) Enemies in the sector (sector_state byte 1 != 0): not in combat and byte 2 (mercs here) != 0 -> combat
    starts: 0x11D40(0) (time compression 0x9300C = 0, tick step 10; button-up waits = platform) combat 0x92DCB = 1, 0x67E18, merc_to_show 0x92DC7 >= 8 -> FirstMerc, NewTurn(0),
    LoadStripPccs(0x92DDF < 3, 1), turn_shown 0x92DCA = 1; else redraw_pending 0x92DA4 != 0 -> NewTurn(0). Then the
    turn clock 0x229D1C = -1. No enemies: in combat -> combat = 0, turn clock 20000, turn_shown 0, combat_end,
    redraw_pending -> NewTurn(0); not in combat: turn clock = 0 -> (busy 0x92E38 < 1 -> busy = 0, NewTurn(0));
    clock != 0 and redraw_pending -> NewTurn(0).
    (4) busy != 0 and busy timer 0x92DB4 = 0: busy timer 5000.
    (5) selected >= 8, turn_based 0x92DAC = 0 and fast timer 0x92DB0 <= 0: fast timer 15000."""
    add = 0
    if _i32(w.g32(G_TURN_TICK)) < 1:
        t2 = s16(w.g16(0x22A484))
        if not w.g8(0x92DCC):
            if t2 < 0x14:
                w.s16(0x22A484, (t2 + 1) & 0xFFFF)
        elif t2 < 1:
            t = s16(w.g16(0x22A486))
            if t > 0 and w.g8(0x22A0CD) == 1:
                w.s16(0x22A486, (t - 1) & 0xFFFF)
        else:
            w.s16(0x22A484, (t2 - 1) & 0xFFFF)
        w.s32(G_TURN_TICK, 10)
    if w.g8(0x229E00) < 0x30:
        w.ext("weapon_text")

    def new_turn():
        nonlocal add
        add = (add + w.ext("new_turn", 0)) & 0xFF
    row = G_SECTOR_TBL9 + 9 * w.g8(G_CUR_SECTOR)
    if w.g8(row + 1):
        if not w.g8(G_COMBAT) and w.g8(row + 2):
            w.s8(0x9300C, 0)               # TimeCompress(0) 0x10986
            w.s32(G_TICK_STEP, 10)
            w.s8(G_COMBAT, 1)
            w.ext("w_67e18")
            if w.g8(0x92DC7) >= 8:
                w.s8(0x92DC7, first_merc(w))
            new_turn()
            w.ext("strip", 1 if w.g8(0x92DDF) < 3 else 0, 1)
            w.s8(0x92DCA, 1)
        elif w.g32(0x92DA4):
            new_turn()
        w.s32(0x229D1C, 0xFFFFFFFF)
    elif w.g8(G_COMBAT):
        w.s8(G_COMBAT, 0)
        w.s32(0x229D1C, 20000)
        w.s8(0x92DCA, 0)
        combat_end(w)
        if w.g32(0x92DA4):
            new_turn()
    elif w.g32(0x229D1C) == 0:
        if _i32(w.g32(G_BUSY)) < 1:
            w.s32(G_BUSY, 0)
            new_turn()
    elif w.g32(0x92DA4):
        new_turn()
    if w.g32(G_BUSY) and w.g32(0x92DB4) == 0:
        w.s32(0x92DB4, 5000)
    if w.g8(G_SELECTED) >= 8 and w.g32(G_TURN_BASED) == 0 and _i32(w.g32(G_FAST_TIMER)) <= 0:
        w.s32(G_FAST_TIMER, 15000)
    return add

def leave_tests(w):
    """TacticalLoop 0x1130D..0x11352, after the fuses of every pass: returns what it adds to the leave counter
    (8-bit). Home sector 59 not ours (sector_state row 59 byte 0 bit 0 clear, 0x2299AF): StatusMessage("Home sector
    LOST!" 0x9AC65), leave_reason 0x93449 = 4, counter + 1. Flag 0x22A05C != 0 and s8 0x22A04E < 2: counter + 1.
    The loop then leaves when the counter != 0 (0x11358), else the next pass (0x10800)."""
    add = 0
    if not w.g8(G_SECTOR_TBL9 + 9 * 59) & 1:
        w.ext("message", 0x9AC65)
        add += 1
        w.s8(0x93449, 4)
    if w.g8(0x22A05C) and s8(w.g8(0x22A04E)) < 2:
        add += 1
    return add


def leave_sector(w, flag):
    """LeaveSectorCleanup? 0x30D6C (AL flag), from the tactical loop's leave path: mercs 0..7: flag != 0 and +0x17E = 8
    -> +0x17E = 0; in the sector (+0x182), in play (+0x138), +0x188 = 0 and hp (+0x183) 0 -> MercDies(quiet 1).
    enemy_up = an enemy 0x28..0x2F in the sector with s8 hp >= 15; guard_up = the same for 8..0x27 (mercs not counted).
    Enemies 0x28..0x2F in the sector: 0 < s8 hp < 15 and (not enemy_up or guard_up) -> SoldierDies(quiet 0); hp 0 ->
    MercDies(0). Then 8..0x27 alike with (not guard_up or enemy_up). Sector table row cur: enemies (byte 1) != 0:
    tappers (byte 4) or guards (byte 3) != 0 -> ResolveSectorBattle(cur, 1); else owned (byte 0 bit 0) ->
    ResolveSectorBattle(cur, 1). No enemies and not owned -> SectorGained(s8 cur). Last SaveSectorTmp."""
    for k in range(8):
        r = w.rec(k)
        if flag & 0xFF and r[0x17E] == 8:
            r[0x17E] = 0
        if r[0x182] and r[0x138] and not r[0x188] and r[0x183] == 0:
            w.ext("merc_dies", r, 1)
    enemy_up = any(w.rec(k)[0x182] and s8(w.rec(k)[0x183]) >= 15 for k in range(0x28, 0x30))
    guard_up = any(w.rec(k)[0x182] and s8(w.rec(k)[0x183]) >= 15 for k in range(8, 0x28))
    for lo, hi, mine, other in ((0x28, 0x30, enemy_up, guard_up), (8, 0x28, guard_up, enemy_up)):
        for k in range(lo, hi):
            r = w.rec(k)
            if not r[0x182]:
                continue
            if 0 < s8(r[0x183]) < 15 and (not mine or other):
                w.ext("w_3a8c0", r, 0)
            if r[0x183] == 0:
                w.ext("merc_dies", r, 0)
    cur = w.g8(G_CUR_SECTOR)
    row = G_SECTOR_TBL9 + 9 * cur
    if w.g8(row + 1):
        if w.g8(row + 4) or w.g8(row + 3) or w.g8(row) & 1:
            w.ext("resolve_battle", cur, 1)
    elif not w.g8(row) & 1:
        w.ext("sector_gained", s8(cur) & 0xFFFFFFFF)
    w.ext("save_tmp")


def leave_path(w):
    """TacticalLoop 0x11358..0x1140F, when the leave counter is != 0 after the leave tests: FadePalette 0x52908
    (0xEDA08, 0, 0x38, 0x100, 0), CursorItemBack? 0x53660, MusicStop? 0x5C82C, 0x284F13 = 0, u16 0x96360 = 0xFFFF.
    0x92E74 = 0 and leave_reason 0x93449 = 0: EndOfDayScreen 0x29F44 (EDX 0); sector byte 2 (mercs) of cur > 0 ->
    RecordReplayEvent(s8 cur, s8 cur, 0x59) and byte 2 = 0; LeaveSectorCleanup 0x30D6C(1); Santino spoke 0x22A0C2 and
    0x93449 still 0 -> 0x93449 = 1. Always: TimerOff? 0x5F298, cur sector = 0xFF."""
    w.ext("fade", 0xEDA08, 0, 0x38, 0x100, 0)
    w.ext("w_53660")
    w.ext("music_stop")
    w.s8(0x284F13, 0)
    w.s16(0x96360, 0xFFFF)
    if not w.g8(0x92E74) and not w.g8(0x93449):
        w.ext("end_of_day")
        cur = w.g8(G_CUR_SECTOR)
        if w.g8(G_SECTOR_TBL9 + 9 * cur + 2):
            w.ext("replay_event", s8(cur) & 0xFFFFFFFF, s8(cur) & 0xFFFFFFFF, 0x59)
            w.s8(G_SECTOR_TBL9 + 9 * cur + 2, 0)
        w.ext("leave_sector", 1)
        if w.g8(G_SANTINO_SPOKE) and not w.g8(0x93449):
            w.s8(0x93449, 1)
    w.ext("timer_off")
    w.s8(G_CUR_SECTOR, 0xFF)


G_LOSS_PART, G_LOSS_STAT = 0xFA908, 0xFA918      # the stat-loss message texts (HitLocationDamage 0x38D80 copies them)


def _loss_name(w, slot):
    """the soldier name at g_soldiers + slot x 0x200 + 0x16E (sprintf %s: up to the zero byte)"""
    b = bytes(w.g8(G_SOLDIERS + 0x200 * slot + 0x16E + k) for k in range(0x92))
    return b.split(b"\0", 1)[0].decode("latin-1")


def _loss_text(w, a, n):
    b = bytes(w.g8(a + k) for k in range(n))
    return b.split(b"\0", 1)[0].decode("latin-1")


def loss_message(w):
    """TacticalLoop 0x107BC..0x108D8, once per tactical day: the setup (combat flag 0x92DCB = 0, LoadTacticalSector?
    0x2AD20, the timers 0x47020, 0x5F2B0, 0x92EFC = 0) and then every pass's message: when the loss timer
    (g_lossTimer 0x92DE8) is 0 and the slot (g_lossSlot 0x22A1F1) is below 0x40: timer = -1 and the message:
    amount (s8 g_lossAmount 0x22A02F) -1 -> MsgMercText 0x15910 "NAME hit in head, causing extra damage!" (800 ms,
    mercs only: +0x17A above 7 shows nothing); below -1 -> TeamQuote 0x61688 (0x1C, 0xE, 0), the stage-0 home
    warning; 0 and above -> ShowMessage 0x27B98 (2000 ms) "NAME receives a critical PART injury, losing N point(s)
    of STAT" with the texts g_lossPart 0xFA908 / g_lossStat 0xFA918."""
    w.s8(0x92DCB, 0)
    w.s32(0x92EFC, 0)
    if w.g32(G_LOSS_TIMER) != 0 or w.g8(G_LOSS_SLOT) >= 0x40:
        return
    slot = w.g8(G_LOSS_SLOT)
    amount = s8(w.g8(G_LOSS_AMOUNT))
    w.s32(G_LOSS_TIMER, -1)
    if amount < 0:
        if amount == -1:
            if w.g8(G_SOLDIERS + 0x200 * slot + 0x17A) > 7:      # MsgMercText 0x15910: mercs only
                return
            w.ext("show_msg", "%s %s" % (_loss_name(w, slot), "hit in head, causing extra damage!"),
                  0, 0, 1, 1, 0, 0x320, 1)
        else:
            w.ext("w_61688", 0x1C, 0xE, 0)
    else:
        text = "%s receives a critical %s injury, losing %d %s %s" % (
            _loss_name(w, slot), _loss_text(w, G_LOSS_PART, 16), amount,
            "point of" if amount == 1 else "points of", _loss_text(w, G_LOSS_STAT, 16))
        w.ext("show_msg", text, 0, 0, 1, 1, 0, 0x7D0, 1)


def end_of_day(w):
    """EndOfDayScreen 0x29F44 (no arguments; from the leave path): LoadMainSound 0x31B8C, 0x92DCC = 1, i16 0x22A486 =
    i16 0x22A484 = 0, 0x92E4F = 1; sunset.pcx picture (0x532BC) and palette (0x53300) into 0x229D18, palette copy 0x52820
    (0xFA608, 0xEDA08, 10); text "End of Day " + itoa(i32 g_day); 0x67BF0, 0x46BF4 (back buffer to screen); h =
    PlaySfxSlot(0x12, 0x2B11, 0x32, 0). 119 frames i = 0..118: not skipped -> wait (TimerElapsedMs(0x22A3FC) >= 2, then
    TimerStart); PaletteRotate(0xEDA08, 3, 0xFF); not skipped -> PaletteSet(0xEDA08, 0, i = 0 ? 0xFF : 0xFE); i = 30 ->
    DrawText of the text on the screen; 0x6782C; 0x67E44 != 0 (a key or click): i < 30 -> the text and PaletteSet(0, 0xFF),
    then skipped = 1. i % 4 = 0 and Random(5) = 0: a = Random(2) + 1, b = s8(Random(10) + 5), c = Random(4) + 0x13,
    PlaySfxSlot(c, 0x2B11, b, a). Then 0x5CE60(h). Sector cur not connected to home (0x41178), leave_reason 0x93449 = 0,
    Santino not spoke and (story 0x22A05C = 0 or byte 0x22A04E = 2): skipped -> PaletteSet(0, 0xFE); 0x5CE08,
    LoadOverheadSounds(0x2D), retreat 0x30B04, 0x46BF4, fade(0xEDA08, 0, 0x38, 0x100, 0). Else fade(0, 0x18, 0x100, 0),
    0x5CE08, LoadOverheadSounds(0x2D). Last: 8 times, merc record 0's +0x17E >= 8 -> 0 (the loop never steps to the
    next record)."""
    w.ext("load_main_sound")
    w.s8(0x92DCC, 1)
    w.s16(0x22A486, 0)
    w.s16(0x22A484, 0)
    w.s8(0x92E4F, 1)
    w.s32(0x229D18, w.ext("load_pcc", w.g32(0x92F44), 0x10AD58, "sunset.pcx", w.g32(0x93E60), 0x140) & 0xFFFFFFFF)
    w.s32(0x229D18, w.ext("load_pal", w.g32(0x92F44), 0x10AD58, "sunset.pcx", 0xFA608) & 0xFFFFFFFF)
    w.ext("pal_copy", 0xFA608, 0xEDA08, 10)
    text = "End of Day " + str(_i32(w.g32(G_DAY)))
    w.ext("w_67bf0")
    w.ext("w_46bf4")
    h = w.ext("sfx", 0x12, 0x2B11, 0x32, 0)
    skipped = False
    for i in range(119):
        if not skipped:
            w.ext("eod_wait")
        w.ext("pal_cycle", 0xEDA08, 3, 0xFF)
        if not skipped:
            w.ext("pal_set", 0xEDA08, 0, 0xFE if i else 0xFF)
        if i == 30:
            w.ext("eod_text", text, 0, 0x140, 0x9F, 0x40, 1, 0xFF, 1, 2, 0x12C)
        w.ext("w_6782c")
        if w.ext("key_any"):
            if i < 30:
                w.ext("eod_text", text, 0, 0x140, 0x9F, 0x40, 1, 0xFF, 1, 2, 0x12C)
                w.ext("pal_set", 0xEDA08, 0, 0xFF)
            skipped = True
        if i % 4 == 0 and w.random(5) == 0:
            a = w.random(2) + 1
            b = s8((w.random(10) + 5) & 0xFF)
            c = w.random(4) + 0x13
            w.ext("sfx", c, 0x2B11, b & 0xFFFFFFFF, a)
    w.ext("w_5ce60", h & 0xFFFFFFFF)
    cur = w.g8(G_CUR_SECTOR)
    if (not w.ext("connected_home", G_SECTOR_TBL9, cur) & 0xFF and not w.g8(0x93449) and not w.g8(G_SANTINO_SPOKE)
            and (not w.g8(0x22A05C) or w.g8(0x22A04E) == 2)):
        if skipped:
            w.ext("pal_set", 0xEDA08, 0, 0xFE)
        w.ext("w_5ce08")
        w.ext("overhead_snd", 0x2D)
        retreat(w)
        w.ext("w_46bf4")
        w.ext("fade", 0xEDA08, 0, 0x38, 0x100, 0)
    else:
        w.ext("fade", 0xEDA08, 0, 0x18, 0x100, 0)
        w.ext("w_5ce08")
        w.ext("overhead_snd", 0x2D)
    r = w.rec(0)
    if r[0x17E] >= 8:
        r[0x17E] = 0


def retreat(w):
    """MsgRetreatCasualty? 0x30B04 (end of the day away from home): risk = 30 x (not owned (cur b0 bit 0 clear) + a way
    out), a way out = a neighbour (north cur > 9: cur - 10 with side bit 2; east cur % 10 != 9: cur + 1, bit 4; south
    cur < 50: cur + 10, bit 8; west cur % 10 != 0: cur - 1, bit 0x10) connected to home (0x41178) while the side bit of
    the current sector's b0 is clear (each neighbour is asked only when its edge test passes; all four are asked).
    Mercs 0..7 in the sector (+0x182), in play (+0x138), +0x188 = 0: Random(100) <= risk -> 0x46BF4, wait 200,
    RetreatHit(s, Random(s16 risk) + 1) (Random(0) divides by zero in the original), DrawMercText(s, "is shot
    retreating!"), wait 1500; then s8 hp < 15 -> SoldierDies(s, 1), 0x46BF4, wait 200, DrawMercText(s, "dies
    retreating!"), wait 1500. A wait = the 10 ms timer 0x92EF8 = ms, run down to 0 or below."""
    cur = w.g8(G_CUR_SECTOR)
    b0 = w.g8(G_SECTOR_TBL9 + 9 * cur)
    out = 0
    for ok, nb, bit in ((cur > 9, cur - 10, 2), (cur % 10 != 9, cur + 1, 4), (cur < 50, cur + 10, 8),
                        (cur % 10 != 0, cur - 1, 0x10)):
        if ok and w.ext("connected_home", G_SECTOR_TBL9, nb & 0xFF) & 0xFF and not b0 & bit:
            out = 1
    risk = 30 * ((0 if b0 & 1 else 1) + out)
    for k in range(8):
        r = w.rec(k)
        if not (r[0x182] and r[0x138] and not r[0x188]):
            continue
        if w.random(100) <= risk:
            w.ext("w_46bf4")
            w.ext("wait", 200)
            w.s32(0x92EF8, 0)
            retreat_hit(r, w, w.random(risk) + 1)
            w.ext("merc_text2", r, "is shot retreating!")
            w.ext("wait", 1500)
            w.s32(0x92EF8, 0)
        if s8(r[0x183]) < 15:
            w.ext("w_3a8c0", r, 1)
            w.ext("w_46bf4")
            w.ext("wait", 200)
            w.s32(0x92EF8, 0)
            w.ext("merc_text2", r, "dies retreating!")
            w.ext("wait", 1500)
            w.s32(0x92EF8, 0)


def retreat_hit(s, w, dmg):
    """0x3928C (EAX soldier, EDX damage): d = HitLocationDamage(s, 0x9C4, damage); d != 0: d > s8 hp -> d = s8 hp;
    hp -= d (8-bit); StatPractice(s, 9, s16 d, 0); 0x17B70(slot, s8 hp < 15 ? 3 : 2)."""
    d = _i32(w.ext("hit_location", s, 0x9C4, dmg & 0xFFFFFFFF) & 0xFFFFFFFF)
    if d == 0:
        return
    if d > s8(s[0x183]):
        d = s8(s[0x183])
    s[0x183] = (s[0x183] - d) & 0xFF
    w.ext("stat_practice", s, 9, s16(d & 0xFFFF) & 0xFF, 0)
    w.ext("w_17b70", s[0x17A], 3 if s8(s[0x183]) < 15 else 2)


G_TILE_ANIMS = 0x229E24       # 10 x 11 B tile animations: s8 count, i32 timer, s16 tile (+5); its tile's byte 3 = frame
G_FX_ANIMS = 0xEAFF0          # 16 x 0x12 B effect animations: count, i32 timer


def front_ticks(w, gated=True):
    """The 10 ms timer callback 0x5EE98, the animation part of its soldier loop (0x5F0CC..0x5F136, the same gates
    0x92DE1 / 0x92DE0 as soldier_ticks): tile animation k (0..9) with s8 count > 0 loses 10 off its timer; at 0 or
    below the count goes down by 1; then if the count is > 0 the timer is 0x8C, else (if s16 tile < 0xC41: also for
    a negative tile) the tile's byte 3 = 0 and the tile = 0xC41. Effect animation k (0..15) with count != 0 loses 10
    off its timer; at 0 or below the timer is 0x8C and the count goes down by 1 (8-bit).
    The front-end owns both tables (it starts the animations); the port's harness runs this as the host's timer."""
    if gated and (w.g8(0x92DE1) or w.g8(0x92DE0)):
        return
    for k in range(10):
        a = G_TILE_ANIMS + 0xB * k
        c = s8(w.g8(a))
        if c <= 0:
            continue
        t = _i32((w.g32(a + 1) - 10) & 0xFFFFFFFF)
        w.s32(a + 1, t & 0xFFFFFFFF)
        if t > 0:
            continue
        c = s8((c - 1) & 0xFF)
        w.s8(a, c & 0xFF)
        if c > 0:
            w.s32(a + 1, 0x8C)
            continue
        tile = s16(w.g16(a + 5))
        if tile < 0xC41:
            w.s8(G_TILEMAP + 8 * tile + 3, 0)
            w.s16(a + 5, 0xC41)
    for k in range(16):
        a = G_FX_ANIMS + 0x12 * k
        if not w.g8(a):
            continue
        t = _i32((w.g32(a + 1) - 10) & 0xFFFFFFFF)
        w.s32(a + 1, t & 0xFFFFFFFF)
        if t <= 0:
            w.s32(a + 1, 0x8C)
            w.s8(a, (w.g8(a) - 1) & 0xFF)


DIR_TABLE = (3, 4, 5, 6, 7, 8, 1, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8)    # 0x96268 (dwords)


def direction_to(x1, y1, x2, y2):
    """DirectionTo? 0x52560 (EAX x1, EDX y1, EBX x2, ECX y2: pixels) -> direction 1..8: x87 atan2(1.2 dy, dx) (dx = 0
    counts as 2^-8), + pi/8, x 4/pi, truncated; the table 0x96268 at i (> 0), i + 7 (< 0) or 0 (exactly 0). The
    same point gives 3 (east). Matches the original in double precision (validate/diff/pure_diff.py)."""
    import math
    dx, dy = _i32(x2 - x1), _i32(y2 - y1)
    a = math.atan2(1.2 * dy, float(dx) if dx else 2.0 ** -8)
    v = (a + 0.39269908175) * 1.2732395445689122
    i = int(v)
    return DIR_TABLE[i] if v > 0 else DIR_TABLE[i + 7] if v < 0 else DIR_TABLE[0]



AIM_DIRS = (3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 1, 1, 1, 2, 2, 2, 3)     # 0x96288 (dwords)


def aim_dir(x1, y1, x2, y2):
    """DirectionAtan? 0x5260C (EAX x1, EDX y1, EBX x2, ECX y2, stack: u16* out) -> (direction 1..8, the word for
    soldier +0x10A): as direction_to, in 24 parts: (a + pi/24) x 12/pi, the index i (> 0), i + 23 (< 0) or 0;
    directions 0x96288, words 0x962E8 = 0, 1, -1 by index mod 3 (validate/diff/pure_diff.py)."""
    import math
    dx, dy = _i32(x2 - x1), _i32(y2 - y1)
    a = math.atan2(1.2 * dy, float(dx) if dx else 2.0 ** -8)
    v = (a + 0.1308996939166667) * 3.819718633706735
    i = int(v)
    k = i if v > 0 else i + 23 if v < 0 else 0
    return AIM_DIRS[k], (0, 1, 0xFFFF)[k % 3]

DECIDERS = {0: calm_action, 1: alert_action, 2: combat_action, 3: engaged_action}


def mine_flag(s, w, tile):
    """MineFlagReroute? 0x19D74 (EAX = soldier, EDX = tile): a detected mine gets a blue flag. Tile object = 0x86,
    MoveCode (tile); FindPath (s, s16 +0x124, 1); no path and slot < 8: ClearTask (s, 1), goal point = the point of
    the own tile, StopAnim (s, 0), AnimStep, StopWalk (s, 0). Always +0x1A2 = the first path byte + 1."""
    w.s8(G_TILEMAP + 8 * tile + 1, 0x86)
    move_code_at(w, tile)
    if not w.ext("find_path", s, s16(u16(s, 0x124)), 1) and s[0x17A] < 8:
        clear_task(s, 1)
        t = s16(u16(s, 0x114))
        w16(s, 0x126, tile_x(t) & 0xFFFF)
        w16(s, 0x128, tile_y(t) & 0xFFFF)
        w.ext("stop_anim", s, 0)
        w.ext("anim_step", s)
        w.ext("stop_walk", s, 0)
    s[0x1A2] = (w.g8(G_PATHS + 0x50 * s[0x17A]) + 1) & 0xFF


def use_special(s, w):
    """UseSpecialItem? 0x63458 (EAX = soldier): the lock kit (hand item 0x4C) on the mission safe. skill = byte of
    s8 dex x s8 mech x s8 wis / 10000 + byte of level x 5 (a byte), then byte of skill x s8 kit condition / 100; need
    = word table 0x95CB4 (80 / 90 / 100) by s8 difficulty. skill >= need: GOOD, sound, delay 1000, safe opened, stat
    practice 7 / 4 / 3 by 100, Use on tile (s, 0x1C9, 0x4C), mission step 0, mission order 0x40. Else BAD, sound and
    the kit wears by Random(5) + 1 when its condition is above 5."""
    if s[0] != 0x4C:
        return
    w.ext("focus", s[0x17A], 0)
    k = _i32(s8(s[0x18C]) * s8(s[0x190]) * s8(s[0x18D]))
    k = (int(k / 10000) + s[0x192] * 5) & 0xFF
    k = int(k * s8(s[2]) / 100) & 0xFF
    need = s16(w.g16((0x95CB4 + 2 * s8(w.g8(G_DIFFICULTY))) & 0xFFFFFFFF))
    if k >= need:
        w.ext("cry", s[0x179], 0xFFFFFFFF, "GOOD", 1, 1)
        w.ext("voc", 0x2B11, 0x32, 1)
        w.ext("delay", 0x3E8)
        w.s8(G_SAFE_OPENED, 1)
        for st in (7, 4, 3):
            w.ext("stat_practice", s, st, 100, 0)
        w.ext("w_3cfd0", s, 0x1C9, s[0])
        w.s8(G_MISSION_STEP, 0)
        w.s8(G_MISSION_ORDER, 0x40)
    else:
        w.ext("cry", s[0x179], 0xFFFFFFFF, "BAD", 1, 1)
        w.ext("voc", 0x2B11, 0x32, 1)
        if s8(s[2]) > 5:
            s[2] = (s[2] - (w.random(5) + 1)) & 0xFF


G_WALTER_SAID = 0x22A0B1      # u8[12]: the sector where each overheard story line was said (99 = not yet)
G_WALTER_NEXT = 0x22A0C3      # u8[8]: the next caption numbers of the plain sounds
G_WALTER_SECTOR = 0x92E2D     # u8: the sector of the last story line (99 at start, not saved)


def walter(s, w, tile):
    """PlayWalterQuote? 0x3C574 (EAX = soldier, EDX = tile): the listening device (hand item 0x45) at a door.
    Worn out (s8 +2 <= 10): caption 0. Else Jam check, then the tile's t1 class: < 7 nothing; 9 (a ground item
    spot): 1 ticking (an armed trap, s8 byte 2 < 0) or 2 silence; a door: enemies 0x28..0x2F behind it (0x3C3D4:
    near < 3 + 1, < 6 + 100), then the story lines (Story 0x3C454, crate talk 0x3C4E0) or the counters at 0x22A0C3.
    The clip + caption (0x3C985..0x3CACB, stubbed as walter_say (q)); q = 1: BlastListAdd, message 0x13, stat
    practice 4 by 10; q >= 0x6E: stat practice 4 by 5."""
    if s[0] != 0x45:
        return
    W = lambda k: s8(w.g8(G_WALTER_SAID + k))
    cur = w.g8(G_CUR_SECTOR)
    here = lambda: w.g8(G_WALTER_SECTOR) == cur
    st = lambda k: s8(w.g8(0x22A040 + k))

    def said(b):
        w.s8(G_WALTER_SAID + b, cur)
        w.s8(G_WALTER_SECTOR, cur)

    def story(q, a, b, p):
        if q != -1 or here() or st(5 + a) != 1 or W(b) != 99:
            return q
        if p is not None and (W(p) == 99 or cur == W(p)):
            return -1
        said(b)
        return b + 0x6E

    def count(k, top, back):
        q = s8(w.g8(G_WALTER_NEXT + k))
        w.s8(G_WALTER_NEXT + k, (q + 1) & 0xFF)
        if s8(w.g8(G_WALTER_NEXT + k)) > top:
            w.s8(G_WALTER_NEXT + k, back)
        return q

    q = -1
    if s8(s[2]) <= 10:
        q = 0
    else:
        if jam_check(s, w) == 0:
            return
        c = w.g8(G_T1_CLASS + w.g8(G_TILEMAP + 8 * tile + 1))
        if c < 7:
            return
        if c == 9:
            g = item_index(w, tile)
            q = 1 if g != 0xFF and s8(w.g8(G_GROUND_ITEMS + 12 * g + 2)) < 0 else 2
        else:
            n = 0
            for k in range(0x28, 0x30):
                b = G_SOLDIERS + 0x200 * k
                if (w.g8(b + 0x182) and s8(w.g8(b + 0x183)) >= 15 and w.g8(G_T0_CLASS + w.g8(b + 0x1B2)) == 3
                        and s8(w.g8(b + 0x9C)) <= 0):
                    d = _i32(w.ext("tile_dist", s16(w.g16(b + 0x114)), tile))
                    if d < 6:
                        n += 1 if d < 3 else 100
            near, far = (n % 100) & 0xFF, (n // 100) & 0xFF
            alarm = w.g8(G_ALARM)
            if near >= 2:
                q = story(q, 0, 7, None)
                q = story(q, 7, 8, None)
                q = story(q, 1, 9, None)
                q = story(q, 2, 9, None)
                if q == -1 and not here() and st(0xB) == 1:
                    if W(0) == 99:
                        q = 0x6E
                        w.s8(G_WALTER_SAID, cur)
                    elif W(1) == 99:
                        if cur != W(0):
                            q = 0x6F
                            w.s8(G_WALTER_SAID + 1, cur)
                    elif W(2) == 99 and cur != W(1):
                        q = 0x70
                        w.s8(G_WALTER_SAID + 2, cur)
                    if q != -1:
                        w.s8(G_WALTER_SECTOR, cur)
                if q == -1 and st(0xA) == 1:
                    q = story(q, 5, 3, None)
                    q = story(q, 5, 4, 3)
                if q == -1 and not here():
                    day = _i32(w.g32(G_DAY))
                    for k in range(3):
                        d = s8(w.g8(G_CRATE_DROPS + 4 * k + 1))
                        if d > day and d - 5 <= day:
                            if W(0xA) == 99:
                                q = 0x78
                                said(0xA)
                            elif W(0xA) != cur and W(0xB) == 99:
                                q = 0x79
                                said(0xB)
                            break
                if q == -1 and st(0xE) == 1 and _i32(w.g32(G_DAY)) > 10:
                    q = story(q, 9, 5, None)
                    q = story(q, 9, 6, 5)
                if q == -1:
                    q = count(2, 0x3B, 0x28) if alarm else count(3, 0x27, 0x14)
            elif near + far >= 2:
                q = count(0, 0x6D, 0x5A) if alarm else count(1, 0x59, 0x46)
            elif near == 1:
                q = count(6, 0x13, 0xA) if alarm else count(7, 0x40, 0x3C)
            elif far == 1:
                q = count(0, 0x45, 0x41) if alarm else count(1, 0x40, 0x3C)
            else:
                q = 2
    w.ext("walter_say", q)
    if q == 1:
        blast_list_add(w, tile)
        w.ext("merc_message", s, 0x13, 0x40, None, 1)
        w.ext("stat_practice", s, 4, 10, 0)
    if q >= 0x6E:
        w.ext("stat_practice", s, 4, 5, 0)
