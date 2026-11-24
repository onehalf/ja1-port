"""Combat rules ported from JA.EXE (reference implementation, diff-tested by validate/diff/combat_diff.py (AP, to-hit),
damage_diff.py (bullet damage, hit location + armour), move_diff.py (move codes, step AP), flight_diff.py (projectile
tile test)).

usage: ./t combat build/mem_NAME.bin [TARGET_SLOT]
  per merc: AP per turn (MercMaxAP 0x56210), main-hand weapon; with TARGET_SLOT: distance (tiles), current AP,
  shot AP cost (ShotApCost 0x564F8), aim clicks affordable, and ChanceToHit 0x56630 for 0..4 aim clicks.

Byte fields (soldier = 0x200 B at g_soldiers 0xFA958): +0x183 hp, +0x184 max hp, +0x185 wound total, +0x18A energy?
(100 = fresh), +0x18B agi, +0x18C dex, +0x191 mrk, +0x192 level, +0xD0 u16 tile of the last target.
"""
import struct
import sys

G_SOLDIERS, SLOT = 0xFA958, 0x200
WEAPONS = 0x94C31          # 6 B per item type 1..16: class, ammo item, ?, speed, b4, b5


def s8(b):
    return b - 256 if b & 0x80 else b


def cdiv(a, b):
    """C (truncating) signed division, as idiv."""
    q = abs(a) // abs(b)
    return q if (a >= 0) == (b > 0) else -q


def max_ap(s):
    """MercMaxAP 0x56210: action points per turn."""
    if s[0x183] == 0:
        return 0
    hp, maxhp = s8(s[0x183]), s8(s[0x184])
    ap = cdiv(s8(s[0x192]) * 10 + s8(s[0x18B]) * 2 + maxhp + s8(s[0x18C]) + 10, 20) & 0xFF
    if hp < maxhp:
        lost = ((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + (maxhp - hp)
        ap = (ap - cdiv(ap * 2 * lost, maxhp * 3)) & 0xFF
    energy = s8(s[0x18A])
    if energy < 100:
        ap = (ap - cdiv((100 - energy) * ap, 200)) & 0xFF
    return max(ap, 8)


def weapon(img_or_dump, t):
    return img_or_dump(WEAPONS + 6 * t, 6)


def shot_ap_cost(s, target_tile, wpn):
    """ShotApCost 0x564F8: AP for one shot at target_tile with the main-hand item (s[0]); wpn = weapon record of s[0].
    0 = not a gun (type 0 or > 16). +1 when the target tile differs from the last one (turning/re-targeting)."""
    t = s[0]
    if t == 0 or t > 16:
        return 0
    cost = 1 if (target_tile != struct.unpack_from("<H", s, 0xD0)[0] or target_tile == 0xC41) else 0
    ap = max_ap(s)
    skill = cdiv(s8(s[0x18B]) + s8(s[0x18C]), 2) & 0xFF if wpn[0] == 4 else s[0x191]   # class 4: avg(agi, dex)
    denom = cdiv((skill // 2 + 50) * wpn[3], 2)
    cost = (cost + cdiv(cdiv(ap * 2 * 100, denom) + 1, 2)) & 0xFF
    return max(min(cost, ap), 1)


ITEMS = 0x94DF8            # 12 B per item type: +2 kind (0x92 gun, 0x96 knife ...), +0xB range in tiles
G_TILEMAP, G_DIFFICULTY, DIFF_MOD = 0x100A18, 0x22A0CC, 0x95C4E
SUN_GOGGLES, SNIPER_SCOPE = 0x27, 0x42


def tile_xy(t):
    """TileToX/TileToY 0x15E50/0x15E6C."""
    return (t % 56) * 16 + 5, (t // 56) * 14 + 5


def distance(ax, ay, bx, by):
    """0x1FD28: 15 units per tile in both axes (x px * 15/16, y px * 15/14)."""
    dx, dy = abs(ax - bx) * 15 // 16, abs(ay - by) * 15 // 14
    return int((dx * dx + dy * dy) ** 0.5)


def occupant(read, tile):
    """0x23944: soldier slot standing on tile (g_tileMap byte 5, low 7 bits), 0x40 = none."""
    o = read(G_TILEMAP + tile * 8 + 5, 1)[0] & 0x7F
    return o if o < 0x30 else 0x40


def chance_to_hit(s, tile, aim, read):
    """ChanceToHit 0x56630 (EAX=shooter, EDX=target tile, BL=aim clicks) -> 1..99 (%).
    read(addr, n) = game memory (difficulty, tile map, target soldier, item tables)."""
    mrk = s8(s[0x191])
    c = mrk if s[2] >= mrk else cdiv(mrk + s[2], 2)          # worn gun (condition < mrk) drags skill down
    if tile == struct.unpack_from("<h", s, 0xD0)[0]:
        c += 10                                              # same target as last shot
    c += aim * 10                                            # each aim click +10
    if s[0x17A] >= 0x28:                                     # enemy shooter: difficulty modifier
        c += struct.unpack("<h", read(DIFF_MOD + 2 * s8(read(G_DIFFICULTY, 1)[0]), 2))[0]
    if s[0x1B9] == 4:
        c = max(c - 20, 1)
    tx, ty = tile_xy(tile)
    d = distance(*struct.unpack_from("<hh", s, 0x116), tx, ty)
    if s[0x24] == SUN_GOGGLES:                               # face slot (item entry 3)
        d -= cdiv(d, 10)
    if s[3] == SNIPER_SCOPE and d > 150 and aim:             # main-hand attachment
        d -= cdiv(struct.unpack("<h", struct.pack("<i", cdiv((d - 150) * aim * 15, 100))[:2])[0] * s8(s[5]), 100)
    c += cdiv(75 - d, 3)                                     # 0 at 5 tiles, -5 per tile beyond
    w = read(WEAPONS + 6 * s[0], 6)
    o = occupant(read, tile)
    if o < 0x40:
        t = read(G_SOLDIERS + o * SLOT, SLOT)
        anim = t[0x1E8 + 2 * t[0x1F3]]
        if (anim <= 3 or anim >= 0x39) and d > 20:
            c -= cdiv(d - 20, 3)
        if 5 <= anim <= 7 or 14 <= anim <= 16:
            c -= 20
        if d > 75 and t[0x1B1] and t[0x1B3] in (1, 2, 4, 5, 6, 7):
            c -= 5
        if s8(t[0x183]) >= 15:                               # conscious target: motion / dodge
            tap = max_ap(t)
            used = (tap - t[0x1A5]) & 0xFF if tap > s8(t[0x1A5]) else 0
            m = tap * t[0x1F2] // used & 0xFF if used else t[0x1F2]
            if s8(t[0x1D5]) > 0:
                if t[0x17A] >= 0x28:                         # target's line-of-sight flags toward the shooter
                    sees = t[0x13A + s[0x17A]]
                else:
                    sees = t[0x112 + s[0x17A]] if s[0x17A] >= 0x28 else 0
                if sees == 1:
                    m = (m + cdiv(s8(t[0x18B]), 20) + cdiv(s8(t[0x192]), 2)) & 0xFF
            if m > w[4]:
                c -= (m - w[4]) * 5
    hp, maxhp = s8(s[0x183]), s8(s[0x184])
    if hp < maxhp:
        lost = ((s[0x184] - s[0x183] - s[0x185]) & 0xFF) // 2 + (maxhp - hp)
        c -= cdiv((cdiv(c * 2 * lost, maxhp * 3) & 0xFF) * (100 - (s8(s[0x192]) - 1) * 10), 100)
    energy = s8(s[0x18A])
    if energy < 100:
        c -= cdiv((100 - (s8(s[0x18C]) - 10)) * (cdiv((100 - energy) * c, 200) & 0xFF), 100)
    if d > read(ITEMS + 12 * s[0] + 0xB, 1)[0] * 15:          # beyond the weapon's range: halved
        c = cdiv(c, 2)
    return min(max(c, 1), 99)


ARMOR_ABSORB, ARMOR_WEAR = 0x92F1E, 0x92F14     # s16 per item type: damage an armour piece can stop / wear factor
HELMET_ABSORB, HELMET_WEAR = 0x92F3C, 0x92F20   # same, read with the helmet's type (tables overlap: base + 2*type)
SHIELDED_VEST = 0x31
G_KNOCKDOWN = 0x92E72                            # set by a leg hit that cost agility on a standing target


def s16(v):
    v &= 0xFFFF
    return v - 0x10000 if v & 0x8000 else v


def bullet_damage(sh, rng, read):
    """BulletDamage 0x39FD8 (EAX = shooter): damage of its projectile on impact, >= 1 (0 = not a gun).
    sh = shooter record (its projectile: +0x1F5 weapon type, +0xC8 i16 pixels flown, +0x1FC hit margin);
    rng(n) = Random(n) 0x53538. damage = wpn.b5 * (100 - flown*50/(range*15) + Random(51)-25 + margin/2) / 100."""
    w = sh[0x1F5]
    if not 1 <= w <= 14:
        return 0
    rng15 = s16(read(ITEMS + 12 * w + 0xB, 1)[0] * 15)
    if rng15 == 0:
        return 0
    fall = s16(cdiv(struct.unpack_from("<h", sh, 0xC8)[0] * 50, rng15))
    roll = s8((rng(0x33) - 0x19) & 0xFF)
    half_margin = s8((sh[0x1FC] // 2) & 0xFF)
    dmg = s16(cdiv(read(WEAPONS + 6 * w + 5, 1)[0] * (100 - fall + roll + half_margin), 100))
    return max(dmg, 1)


def _armor(s, slot, absorb_tbl, wear_tbl, r, dmg, read):
    """one armour piece (item entry `slot`): returns (absorbed, dmg left); wears its condition byte."""
    e = 12 * slot
    t = s[e]
    a = struct.unpack("<h", read(absorb_tbl + 2 * t, 2))[0]
    over = r - s8(s[e + 2])
    if over > 0:
        a = max(a - over, 0)
    a = min(a, dmg)
    if a:
        wear = struct.unpack("<h", read(wear_tbl + 2 * t, 2))[0]
        s[e + 2] = (s[e + 2] - cdiv(wear * cdiv(a * 50, 100), 100)) & 0xFF
    return a, dmg - a


def _stat_loss(s, off, dmg, rng):
    """Random(dmg) + 1 > 20 -> stat at `off` loses Random(dmg/2) + 1, leaving at least 1. Returns the loss or None."""
    if rng(s16(dmg)) + 1 <= 20:
        return None
    loss = rng(s16(cdiv(dmg, 2))) + 1
    if s8(s[off]) - loss < 1:
        loss = s8(s[off]) - 1
    s[off] = (s[off] - loss) & 0xFF
    return loss


G_LOSS_SLOT, G_LOSS_TIMER, G_LOSS_AMOUNT = 0x22A1F1, 0x92DE8, 0x22A02F   # stat-loss message: merc, i32 timer, amount
G_LOSS_PART, G_LOSS_STAT = 0xFA908, 0xFA918                              # its texts: body part, stat name


def _loss_msg(s, mem, part, stat, loss):
    """a merc (slot < 8) that lost a stat gets the message: slot, timer 500, amount, part and stat texts; a head
    hit without a loss (part None) gives any soldier the message with amount 0xFF and no texts"""
    if mem is None or (part and s[0x17A] >= 8):
        return
    mem[G_LOSS_SLOT] = s[0x17A]
    for k, b in enumerate(struct.pack("<i", 500)):
        mem[G_LOSS_TIMER + k] = b
    mem[G_LOSS_AMOUNT] = loss & 0xFF
    for base, text in ((G_LOSS_PART, part), (G_LOSS_STAT, stat)) if part else ():
        for k, b in enumerate(text + b"\0"):
            mem[base + k] = b


def hit_location_damage(s, height, dmg, rng, read, mem=None):
    """HitLocationDamage 0x38D80 (EAX = target, EDX = projectile height, EBX = damage) -> damage after hit location
    and armour; mutates s (stat losses, armour wear). mem (dict) receives G_KNOCKDOWN = 1 like the original.
    Height (prone target with height <= 1000: x6): < 0 or > 6000 nothing; >= 5000 head: helmet (entry 2) absorbs,
    else x1.5 and maybe wisdom loss; >= 2500 torso: Shielded Vest (entry 6, type 0x31) then body armour (entry 5) absorb, nothing
    absorbed -> maybe dexterity loss; below: legs, damage halved, maybe agility loss (+ knockdown when standing).
    A merc's stat loss also fills the message (mem): "head" / "wisdom", "arm" / "dexterity", "leg" / "agility"; an
    unarmoured head hit without a loss gives any soldier the message with amount 0xFF (no texts)."""
    r = rng(100) + 1
    if dmg == 0:
        return 0
    anim = s16(struct.unpack_from("<H", s, 0x1E8 + 2 * s[0x1F3])[0])
    if (anim <= 3 or anim >= 0x39) and height <= 1000:
        height *= 6
    if height > 6000 or height < 0:
        return 0
    if height >= 5000:
        a = 0
        if s[0x18]:
            a, dmg = _armor(s, 2, HELMET_ABSORB, HELMET_WEAR, r, dmg, read)
        if not a:
            dmg += cdiv(dmg * 50, 100)
            loss = _stat_loss(s, 0x18D, dmg, rng)
            if loss is not None:
                _loss_msg(s, mem, b"head", b"wisdom", loss)
            else:
                _loss_msg(s, mem, None, None, 0xFF)
    elif height >= 2500:
        total = 0
        if s[0x48] == SHIELDED_VEST:
            a, dmg = _armor(s, 6, ARMOR_ABSORB, ARMOR_WEAR, r, dmg, read)
            total += a
        if s[0x3C]:
            a, dmg = _armor(s, 5, ARMOR_ABSORB, ARMOR_WEAR, r, dmg, read)
            total += a
        if not total:
            loss = _stat_loss(s, 0x18C, dmg, rng)
            if loss is not None:
                _loss_msg(s, mem, b"arm", b"dexterity", loss)
    else:
        dmg -= cdiv(dmg * 50, 100)
        loss = _stat_loss(s, 0x18B, dmg, rng)
        if loss is not None:
            _loss_msg(s, mem, b"leg", b"agility", loss)
            if 4 <= anim < 0x29 and anim != 0xD and mem is not None:
                mem[G_KNOCKDOWN] = 1
    return max(dmg, 0)


# --- movement (PathApCost 0x1D32C = sum of StepApCost over the path + PostureApCost)
G_MOVECODE = 0x108258          # u16 per tile: terrain move code (built by MoveCode 0x2BE54, saved in quick saves)
G_T0CLASS, G_T1CLASS = 0x284E58, 0x108158
MOVE_DOOR, MOVE_WALL = 25300, 25500
STEP_EXTRA = {1000: 2, 1200: 3, 1201: 4, 1500: 2, 1600: 4, 2000: 5, 3000: 6, 3600: 8, 5000: 7, 25300: 2, 25400: 2}


def move_code(t0, t1, t3, old, t0class, t1class):
    """MoveCode 0x2BE54: terrain move code of a tile from its layers (t0 ground, t1 object, t3 roof); old = the
    tile's previous code (kept when the ground class is 0; a door code 25300 is never lowered)."""
    code = old
    if t1:
        c = t1class[t1]
        if c == 1:
            code = 2000
        elif c in (2, 3):
            code = 25400
        elif c in (4, 5):
            code = MOVE_DOOR if t1 in (0x21, 0x23, 0x24, 0x26) else MOVE_WALL
        elif t1 in (0x3D, 0x3E, 0x43, 0x45, 0x5C, 0x5D, 0x62, 0x64):
            code = 1500
        else:
            code = 1201 if t3 == 0x73 else MOVE_WALL
    else:
        c = t0class[t0]
        if c in (1, 4, 5):
            code = 1200
        elif c in (2, 3):
            code = 1000
        elif c in (6, 7):
            code = 1600
        elif c == 8:
            code = 3000 if t0 < 0x80 else 3600
        elif c == 9:
            code = 5000 if t0 > 0x96 else 3600
    if old == MOVE_DOOR and code < MOVE_DOOR:
        code = MOVE_DOOR
    return code


def step_ap_cost(code, shift, b177, alt):
    """StepApCost 0x39B30 (EAX tile -> its move code; EDX = Shift held (sneak), ECX = soldier +0x177, EBX = Alt
    held): AP to enter one tile. 100 = impassable (wall code 25500). Returned as a signed byte."""
    if code == MOVE_WALL:
        return 100
    base = 2 if shift else (3 if b177 else 0)
    if alt:
        base += 1
    return s8((base + STEP_EXTRA.get(code, 0)) & 0xFF)


# FindPath 0x40810 (A* over the 56x56 tile grid)
DIR_STEP = (-56, -55, 1, 57, 56, 55, -1, -57)   # 0x96070: N NE E SE S SW W NW (odd = diagonal, +1 cost)
PATH_BUF = 0x109BD8                             # u8 direction per step, 0x50 per soldier slot (+0x17A)
POOL, OPEN, CLOSED, TRAILS = 0x200, 0, 0x1FF, 0x1000


def _heuristic(tile, dest):
    """Octile estimate in move-code units: longer axis + 1.4 x shorter axis (truncated)."""
    dr, dc = abs(cdiv(dest, 56) - cdiv(tile, 56)), abs(dest % 56 - tile % 56)
    return dr + cdiv(dc * 14, 10) if dc < dr else dc + cdiv(dr * 14, 10)


def _manhattan(tile, dest):
    return abs(dest % 56 - tile % 56) + abs(cdiv(dest, 56) - cdiv(tile, 56))


def find_path(start, dest, mc, t0, occ, b177=0, bB9=0, slot=0, combat=0, stats=None):
    """FindPath 0x40810 (EAX soldier, DX dest tile, BX store the path): A* from the soldier's tile (+0x114).
    mc = 3136 move codes (g_moveCode), t0 = ground byte per tile, occ(tile) = TileOccupant (64 = none);
    b177 / bB9 = soldier bytes +0x177 / +0xB9, slot = +0x17A, combat = [0x92DCB] (enemies near).
    Returns the step directions (DIR_STEP indices, start -> dest) or None (no path, or a pool ran out).
    stats (dict, optional) counts 'recycled' nodes and the 'end' reason (path / none / nodes / trails).

    Same data structures as the original, so ties and overflows come out identical: 512 16-byte nodes, node 0 =
    head of the open list (sorted by g + h; equal f: nodes nearer the goal by Manhattan distance first), node 0x1FF =
    head of the closed list; when nodes 2..0x1FE are used up, the oldest closed node is recycled. Each node points
    at a trail record (parent trail, direction); 4096 of those, running out = no path. Stale open nodes (a cheaper
    way to the tile was found later) stay in the list and are skipped when popped.
    Edge cost = move code of the entered tile: occupied tiles (other than dest) cost 20000, or block in combat;
    +0x177 makes everything below 3000 cost 1000; 1500 (fence/hedge?) only as dest or from 0x62D4 with +0xB9 set;
    0x639C blocks; 1201 blocks slots >= 8 (not player mercs); 0x6338 only to/from 0x6338/0x62D4 and costs 1000,
    0x62D4 costs 1000 (t0 < 0x6C) or 6000; dest costs 0; if start and dest are both 3600/5000 (water?), 3600/5000
    cost 500. Diagonals +1."""
    nxt, prv, tl, g, h, tr = [0] * POOL, [0] * POOL, [0] * POOL, [0] * POOL, [0] * POOL, [0] * POOL
    tpar, tdir = [0] * TRAILS, [0] * TRAILS
    best = [0xFFFFFFFF] * 3136
    count = 2
    nxt[OPEN] = prv[OPEN] = 1
    g[OPEN], tl[OPEN], tl[1], h[1] = 99900, start, start, _heuristic(start, dest)
    nxt[CLOSED] = prv[CLOSED] = CLOSED
    best[start] = 0
    ntrail = 1
    both_water = mc[start] in (3600, 5000) and mc[dest] in (3600, 5000)
    while True:
        k = nxt[OPEN]
        t, gk = tl[k], g[k]
        nxt[prv[k]], prv[nxt[k]] = nxt[k], prv[k]            # unlink from open, append to closed
        nxt[k], prv[k] = CLOSED, prv[CLOSED]
        nxt[prv[CLOSED]] = k
        prv[CLOSED] = k
        tl[k] = -1
        if best[t] >= gk:
            for d, step in enumerate(DIR_STEP):
                n = t + step
                if not 0 <= n < 3136 or (t % 56 == 0 and n % 56 == 55) or (t % 56 == 55 and n % 56 == 0):
                    continue
                code = mc[n]
                if n != dest and occ(n) < 0x40:
                    if combat:
                        continue
                    code = 20000
                elif b177 and code < 3000:
                    code = 1000
                cur = mc[t]
                if code == 1500 and n != dest and not (bB9 and cur == 0x62D4):
                    continue
                if code == 0x639C or (code == 1201 and slot >= 8):
                    continue
                if cur == 0x6338 and code not in (0x6338, 0x62D4):
                    continue
                if code == 0x6338 and cur not in (0x6338, 0x62D4):
                    continue
                if code == 0x6338:
                    code = 1000
                elif code == 0x62D4:
                    code = 1000 if t0[n] < 0x6C else 6000
                if n == dest:
                    code = 0
                if both_water and code in (3600, 5000):
                    code = 500
                new = gk + code + (d & 1)
                if new >= best[n]:
                    continue
                if count < CLOSED:
                    idx = count
                    count += 1
                else:                                       # recycle the oldest closed node
                    idx = nxt[CLOSED]
                    nxt[prv[idx]], prv[nxt[idx]] = nxt[idx], prv[idx]
                    if stats is not None:
                        stats["recycled"] = stats.get("recycled", 0) + 1
                if idx in (0, CLOSED):
                    if stats is not None:
                        stats["end"] = "nodes"
                    return None
                tpar[ntrail], tdir[ntrail] = tr[k], d
                tr[idx] = ntrail
                ntrail += 1
                if ntrail >= TRAILS:
                    if stats is not None:
                        stats["end"] = "trails"
                    return None
                tl[idx], g[idx], h[idx] = n, new, _heuristic(n, dest)
                best[n] = new
                f, md = new + h[idx], _manhattan(n, dest)
                c = nxt[OPEN]
                while c and g[c] + h[c] < f:
                    c = nxt[c]
                while c and g[c] + h[c] == f and md > _manhattan(tl[c], dest):
                    c = nxt[c]
                nxt[idx], prv[idx] = c, prv[c]
                nxt[prv[c]] = idx
                prv[c] = idx
        if not nxt[OPEN] or tl[nxt[OPEN]] == dest:
            break
    k = nxt[OPEN]
    if stats is not None:
        stats["end"] = "path" if tl[k] == dest else "none"
    if tl[k] != dest:
        return None
    dirs, i = [], tr[k]
    while i:
        dirs.append(tdir[i])
        i = tpar[i]
    return dirs[::-1]


def posture_ap_cost(anim):
    """PathApCost tail: getting up first. anim = the soldier's current animation (+0x1E8 + 2 * +0x1F3)."""
    if anim in (0, 3):
        return 7
    if anim in (1, 2):
        return 5
    if anim in (0xE, 0xF):
        return 3
    return 0


# --- projectile flight (ProjectileStep 0x57BB8 calls ProjectileHitTest once per new tile)
POSTURE_TBL = 0x9300D          # u8 per animation; > 5 lets a non-target soldier block a short shot (1/30)
WINDOW_TBL = 0x95788           # u8 per (object * 8 + direction): >= 0x62 = window-like, bullets may pass


def view_tile(x, y):
    """ViewTile 0x46F3C: map tile under pixel (x, y)."""
    return cdiv(x, 16) + cdiv(y, 14) * 56


def cheb(ax, ay, bx, by):
    """0x1FD8C: max(|ax - bx|, |ay - by|)."""
    return max(abs(ax - bx), abs(ay - by))


def projectile_hit_test(height, x, y, flown, direction, skip, cap, target_tile, roll_ok, rng, read, chance=0):
    """ProjectileHitTest 0x577B4 -> (result, skip, chance): result 1 = stops here (soldier hit or obstacle),
    0 = flies on, -1 = stops on a roof-high wall; skip (+0x1FF) is cleared when the target soldier's tile is
    reached; chance = pass-through % the original writes back (informational).
    height = projectile height (> 0), x/y = pixel position, flown = pixels so far, direction = +0x1C6,
    cap = maximum height (+0xCA; 0 for bullets), target_tile = +0xDC, roll_ok = the to-hit roll succeeded (+0x1BC);
    chance = the chance output's starting value (a tree stops the projectile only when it is 0)."""
    if height < 1:
        return 1, skip, chance
    tile = view_tile(x, y)
    tm = read(G_TILEMAP + 8 * tile, 8)
    obj = tm[1]
    cls = read(G_T1CLASS + obj, 1)[0]
    h = {6: 9000, 7: 10000, 8: 10000}.get(cls, 0)
    if cls == 9:
        h = 7000 if obj in (0xE2, 0xE3) else 3000
    if cap > 9000 and read(G_T0CLASS + tm[0], 1)[0] == 3 and tm[3] >= 0x52:
        h = 10001                                            # roof
    occ = occupant(read, tile)
    if occ < 0x40:
        t = read(G_SOLDIERS + occ * SLOT, SLOT)
        ttile, tx, ty = struct.unpack_from("<Hhh", t, 0x114)
        anim = struct.unpack_from("<H", t, 0x1E8 + 2 * t[0x1F3])[0]
        if not (target_tile == ttile or flown >= 0x4B or (rng(30) == 0 and s8(read(POSTURE_TBL + anim, 1)[0]) >= 6)):
            return 0, skip, chance
        h += 1000 if (anim < 4 or anim > 0x37) else 6000
        if h >= height and cheb(x, y, tx + 2, ty + 4) <= 1:
            return 1, skip, chance
        if target_tile == ttile:
            skip = 0
        return 0, skip, chance
    if h < height:
        return 0, skip, chance
    cx, cy = tile_xy(tile)
    if h == 9000:                                            # tree
        if flown >= 30 or tile == target_tile:
            n = max(25 - cdiv(flown, 15), 1)
            if rng(s16(n)) == 0 and chance == 0:
                return 1, skip, chance
            return 0, skip, 100 - cdiv(100, n)
        if roll_ok:
            return 0, skip, 100
        if rng(100) == 0 and chance == 0:
            return 1, skip, chance
        return 0, skip, 99
    if h < 10000:                                            # low object: only its middle stops the projectile
        return (1 if cheb(x, y, cx, cy) <= 2 else 0), skip, chance
    if skip:
        return 0, skip, chance
    win = read(WINDOW_TBL + obj * 8 + direction, 1)[0]
    if s8(win) >= 0x62 and height < 9000:
        p = 100 - cdiv(flown - 15, 5)
        if rng(100) >= p and chance == 0:                    # (0x57AB7..0x57AC0: entry chance 0 only)
            return 1, skip, chance
        return 0, skip, p
    if s8(win) < 4 and cap <= 9000:
        return 0, skip, chance
    if cap <= 9000 or h <= 10000:
        return 1, skip, chance
    return -1, skip, chance


def main():
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    from memdump import Dump
    d = Dump(a[0])
    tgt = int(a[1]) if len(a) > 1 else None
    ttile = None
    if tgt is not None:
        ts = d.read(G_SOLDIERS + tgt * SLOT, SLOT)
        ttile = struct.unpack_from("<H", ts, 0x114)[0]
    for k in range(8):
        s = d.read(G_SOLDIERS + k * SLOT, SLOT)
        if s[0x183] == 0:
            continue
        nick = s[0x16E:0x178].split(b"\0")[0].decode("latin1")
        tile = struct.unpack_from("<H", s, 0x114)[0]
        w = d.read(WEAPONS + 6 * s[0], 6) if 0 < s[0] <= 16 else bytes(6)
        line = (f"{k} {nick:8s} ({tile % 56:2d},{tile // 56:2d}) hp {s[0x183]:3d}/{s[0x184]:3d} mrk {s[0x191]:3d} "
                f"lvl {s[0x192]} maxAP {max_ap(s):2d} item0 {s[0]:#04x}/{s[1]} wpn {w.hex(' ')}")
        if ttile is not None:
            dx, dy = ttile % 56 - tile % 56, ttile // 56 - tile // 56
            cost, cur = shot_ap_cost(s, ttile, w), s[0x1A5]
            tx, ty = tile_xy(ttile)
            dist = distance(*struct.unpack_from("<hh", s, 0x116), tx, ty)
            hits = " ".join(f"{chance_to_hit(s, ttile, a, d.read):2d}" for a in range(5))
            line += (f" | d {dist / 15:4.1f}t ({dx:+d},{dy:+d}) AP {cur:2d} shot {cost} -> max aim "
                     f"{max(0, min(4, cur - cost))}  hit% aim0..4: {hits}")
        print(line)


if __name__ == "__main__":
    main()
