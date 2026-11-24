"""Inventory screen mirror: UseInventoryItem? 0x53A9C (the UI loop of the key 'I' screen) and its item helpers.

Analysis mirror for validate/diff/inv_diff.py (not the clean-room port). `w` gives memory at image addresses
(g8/g16/g32/s8/s16/s32, tile8) and ext(name, *args) for the callees the test stubs on both sides (UI drawing,
sounds, prompts, Spend AP, PutItem ...). ext("poll") is the mouse poll 0x678E8: it writes the mouse x / y
(0x229D00 / 0x229D04), the buttons (0x229CF4) and the keyboard flags, and ends the run when the script is over.
A soldier is passed around as a Rec (record view at its image address); ext calls get the Rec.
"""
import ai
from ai import s8

CURSOR, SWAP = 0x222B50, 0x222B68         # 12-byte item on the mouse cursor; swap buffer
PICKED = 0x28EF30                         # u8[8]: this merc already paid the ground pick-up AP
GIVE_A, GIVE_B = 0x28EF38, 0x28EF39       # "gave an item" pair (0x40 = none)
VEST_OWNER = 0x92E40                      # slot whose vest pockets the cursor vest holds (0x40 = none)
MOUSE_X, MOUSE_Y, BUTTONS = 0x229D00, 0x229D04, 0x229CF4
KBD, SHIFT, CTRL = 0x22A46C, 0x284F21, 0x284F1F
TEXT_ON = 0x1163BD                        # message flag ('T' key toggles it through 0x3F994)
ITEM_POS = 0x949F2                        # 14 x (i16 x, i16 y): pockets 0..11, 12 = hand attachment, 13 = ground
SLOT_POS = 0x93686                        # 8 x (i16 x, i16 y): merc slots
ITEM_TBL = 0x94DF8                        # 12 B per item type: +5 clip, +6 max stack
AMMO_OF = 0x94C32                         # 6 B per gun type: ammo item type
G_SOLDIERS, G_SOLDIER_PTR = 0xFA958, 0x100958
G_GROUND = 0xEB160
G_SECTORS, G_CUR_SECTOR = 0x222B74, 0x22A039
SLOT_GFX = 0x95678                        # pointer passed to DrawItemSlot


class Rec:
    """soldier record at an image address (reads and writes go through w)"""
    def __init__(self, w, addr):
        self.w, self.addr = w, addr

    def __getitem__(self, i):
        return self.w.g8(self.addr + i)

    def __setitem__(self, i, v):
        self.w.s8(self.addr + i, v & 0xFF)

    def __len__(self):
        return 0x200


class Leave(Exception):
    pass


def rec(w, slot):
    return Rec(w, G_SOLDIERS + 0x200 * slot)


def mouse_in(w, x0, y0, x1, y1):
    """MouseInRect? 0x67C0C (signed compares, edges inside)"""
    x, y = ai._i32(w.g32(MOUSE_X)), ai._i32(w.g32(MOUSE_Y))
    return x0 <= x <= x1 and y0 <= y <= y1


def wait_left_up(w):
    """0x67C54: poll until the left button is up"""
    while True:
        w.ext("poll")
        if not w.g8(BUTTONS) & 1:
            return


def wait_right_up(w):
    """0x67C68: poll until the right button is up"""
    while True:
        w.ext("poll")
        if not w.g8(BUTTONS) & 2:
            return


def copy_item(w, dst, src):
    """0x47090 (EAX dst, EDX src): copy a 12-byte item entry"""
    for k in range(12):
        w.s8(dst + k, w.g8(src + k))


def clear(w, a, n):
    for k in range(n):
        w.s8(a + k, 0)


def max_stack(w, t):
    return w.g8(ITEM_TBL + 12 * t + 6)


def vest_pocket(s, k):
    """0x4A464 (EAX soldier, DL entry) -> 1 if entry 7..11 is a pocket the vest (entry 6) does not have: a merc's vest
    type below 0x32 has none, 0x32 one (7..8 need 0x32+, 9 needs 0x33+, 10 0x34+, 11 0x35+); non-mercs 0"""
    if s[0x17A] >= 8:
        return 0
    k = s8(k & 0xFF)
    need = {7: 0x32, 8: 0x32, 9: 0x33, 10: 0x34, 11: 0x35}.get(k)
    if need is None:
        return 0
    return 0 if s[0x48] >= need else 1


def gave_item(w, slot):
    """0x53958 (AL slot): the "gave an item" pair. B (the giver, 0x40 = none) set and slot neither A nor B: both mercs
    with hp 15+ pay Spend AP 6 (fatigue 0x14), get PICKED = 1 and a slot redraw; A = B, B = none. Else slot == A
    clears A; B = none."""
    b = w.g8(GIVE_B)
    if b == 0x40:
        return
    if slot != b and slot != w.g8(GIVE_A):
        if s8(rec(w, b)[0x183]) > 14:
            w.ext("spend_ap", rec(w, b), 6, 0x14)
            b = w.g8(GIVE_B)
            w.s8(PICKED + b, 1)
            w.ext("slot_redraw", b, 1)
        if s8(rec(w, slot)[0x183]) > 14:
            w.ext("spend_ap", rec(w, slot), 6, 0x14)
            w.s8(PICKED + slot, 1)
            w.ext("slot_redraw", slot, 1)
        w.s8(GIVE_A, w.g8(GIVE_B))
        w.s8(GIVE_B, 0x40)
        return
    if slot == w.g8(GIVE_A):
        w.s8(GIVE_A, 0x40)
    w.s8(GIVE_B, 0x40)


def unstack_one(w, a):
    """0x53580 (EAX entry; ctrl-click): a stack of 2+ of a stacking type (+6 max != 1, count != 99) moves its top
    condition to the end of the list (UseThrow, then count + 1 with the old condition last) -> 1; else 0"""
    cnt = w.g8(a + 1)
    if max_stack(w, w.g8(a)) == 1 or cnt == 99 or s8(cnt) < 1 or cnt == 1:
        return 0
    c = w.g8(a + 2)
    ai.use_throw(w, a)
    n = (w.g8(a + 1) + 1) & 0xFF
    w.s8(a + 1, n)
    w.s8(a + s8(n) + 6, c)
    return 1


def merge_stack(w, dst, src):
    """0x538A0 (EAX dst, EDX src): append src's unit conditions to dst (the first one is src +2, the rest src +8..);
    an empty dst takes the first into +2"""
    n = s8(w.g8(src + 1))
    if n < 1:
        return
    for i in range(n):
        if i == 0:
            v = w.g8(src + 2)
            if w.g8(dst + 1) == 0:
                w.s8(dst + 2, v)
            else:
                w.s8(dst + s8(w.g8(dst + 1)) + 7, v)
        else:
            w.s8(dst + s8(w.g8(dst + 1)) + 7, w.g8(src + 7 + i))
        w.s8(dst + 1, w.g8(dst + 1) + 1)


def drop_lost_pockets(w, s):
    """0x538FC (EAX soldier): every pocket 7..11 the vest no longer has and that holds an item goes to the ground
    (PutItem on the soldier's tile, b6 2) and is cleared"""
    for k in range(7, 12):
        a = s.addr + 12 * k
        if vest_pocket(s, k) and w.g8(a):
            put_item(w, tile(s), a)
            ai.drop_item(w, a)


def swap_pockets(w, s, owner):
    """0x470E8 (EAX soldier, DL owner): swap entries 7..11 with the owner's"""
    o = G_SOLDIERS + 0x200 * owner
    for k in range(7, 12):
        a, b = s.addr + 12 * k, o + 12 * k
        tmp = [w.g8(a + i) for i in range(12)]
        copy_item(w, a, b)
        for i in range(12):
            w.s8(b + i, tmp[i])


def put_item(w, t, a):
    """PutItem 0x3FC78 (EAX tile; stack: the 12-byte entry as 3 dwords, b6 2, exact 0, note 0, search 0)"""
    w.ext("put_item", t, 0, 0, 0, w.g32(a), w.g32(a + 4), w.g32(a + 8), 2, 0, 0, 0)


def tile(s):
    return ai.s16(s[0x114] | s[0x115] << 8)


def rec_by_ptr(w, slot):
    """the record the pointer table 0x100958 gives for a slot"""
    return Rec(w, w.ptr_image(w.g32(G_SOLDIER_PTR + 4 * slot)))


def set_skill_bonus(w, s):
    """+0x1D0 = 1 when level (+0x192) > 3; with item 4 type 0x2B: +0x1D0 += 5 - (100 - its condition) / 20"""
    s[0x1D0] = 0
    if s8(s[0x192]) > 3:
        s[0x1D0] = s[0x1D0] + 1
    if s[0x30] == 0x2B:
        s[0x1D0] = s8(s[0x1D0]) + 5 - ai.cdiv(100 - s8(s[0x32]), 20)


def inventory_screen(w, s, redraw):
    """UseInventoryItem? 0x53A9C (EAX soldier, DL redraw): the inventory screen loop until Esc / face click / right
    click under the panel with an empty cursor, or a ground pick-up that sets off a mine."""
    w.s8(GIVE_A, 0x40)
    w.s8(GIVE_B, 0x40)
    clear(w, PICKED, 8)
    w.ext("show")
    armour = ai.armor_rating(s)
    clear(w, SWAP, 6)
    w.ext("w_4b730", 0)
    leave = False
    redraw &= 0xFF
    while True:
        if redraw:
            if w.g8(CURSOR) == 0x6F:
                v = w.ext("item_value", CURSOR)
            else:
                v = s8(w.g8(CURSOR + 2))
            if w.ext("item_info", s[0x179], w.g8(CURSOR), s8(w.g8(CURSOR + 1)), ai.s16(v), 1, 1, 0):
                armour = w.ext("body", s)
            redraw = 0
        w.ext("poll")
        reloaded = False
        vest_done = False
        kb = w.g16(KBD)
        w.s8(SHIFT, 1 if kb & 3 else 0)
        w.s8(CTRL, 1 if kb & 0x104 else 0)
        if w.ext("key_pressed"):
            k = w.ext("get_key") & 0xFF
            if k == 0:
                w.ext("get_key")
            else:
                c = s8(k)
                if ord("a") <= c <= ord("z"):
                    c -= 0x20
                c &= 0xFFFFFFFF
                if c == 0x1B:
                    if w.g8(CURSOR) == 0:
                        leave = True
                    else:
                        w.ext("message", 0x97887)
                elif c == 0x54:
                    w.ext("text_toggle", 0x9ADD1, TEXT_ON)
            if leave:
                break
        b = w.g8(BUTTONS)
        if not b & 1:
            if b & 2:
                hit = -1
                for k in range(13):
                    x, y = ai.s16(w.g16(ITEM_POS + 4 * k)), ai.s16(w.g16(ITEM_POS + 4 * k + 2))
                    if mouse_in(w, x, y, x + 0x29, y + 0xF):
                        hit = k
                        break
                if hit >= 0:
                    w.ext("click")
                    if hit == 12:
                        a = s.addr
                        v = s8(w.g8(a + 5)) * 10 if w.g8(a + 3) == 0x6F else s8(w.g8(a + 5))
                        r = w.ext("item_info", s[0x179], w.g8(a + 3), s8(w.g8(a + 4)), ai.s16(v), 1, 1, 1)
                    else:
                        a = s.addr + 12 * hit
                        v = w.ext("item_value", a) & 0xFFFF
                        if v == 0:
                            v = s8(w.g8(a + 2))
                        r = w.ext("item_info", s[0x179], w.g8(a), s8(w.g8(a + 1)), ai.s16(v), 1, 1, 1)
                    if r:
                        armour = w.ext("body", s)
                    wait_right_up(w)
                wait_right_up(w)
                if ai._i32(w.g32(MOUSE_Y)) + ai._i32(w.g32(0x229C48)) <= 0xB5:
                    continue
                if w.g8(CURSOR) != 0:
                    w.s8(0x955C9, 1)
                    w.s32(0x229D18, w.ext("w_682ac", 1, 0xBE, 0x13E, 0xC5))
                break
            if mouse_in(w, 0x48, 0x1F, 0x64, 0x37):
                w.ext("tip_merc", s8(w.g8(ai.G_SELECTED) + 1))
            else:
                w.ext("tip_item", w.g8(0x93EA8))
            continue
        # left button held
        if mouse_in(w, 0x6C, 0x3E, 0x9C, 0x56) and ai.msg_not_enough_ap(s, w, 5, 0x63):
            drink(w, s)
            wait_left_up(w)
            continue
        if mouse_in(w, 0x70, 0x52, 0x96, 0x76) and ai.msg_not_enough_ap(s, w, 0xA, 0x63):
            r = camo(w, s)
            if r is not None:
                armour = r
            wait_left_up(w)
            continue
        if mouse_in(w, 0x46, 0x1C, 0x66, 0x38):
            w.ext("click")
            c = w.g8(CURSOR)
            if c == 0:
                wait_left_up(w)
                w.s8(0x93EA8, c)
                w.ext("hide")
                w.s8(0x284F13, 1)
                break
            w.ext("message", 0x97887)
            wait_left_up(w)
            continue
        x = ai._i32(w.g32(MOUSE_X))
        if x < 0x30 or x > 0x10F:
            r = slot_click(w, s)
            if r == "leave":
                leave = True
                break
            if isinstance(r, tuple):
                s, armour = r
            continue
        r = item_click(w, s, armour)
        if r == "leave":
            break
        armour = r
    set_skill_bonus(w, s)
    w.ext("w_2b6a0", s)
    w.ext("w_4b764")


def drink(w, s):
    """canteen (0x41, one at a time) or 0x4F on the cursor over the mouth rect: up to 20 sips (0x41 keeps one),
    Spend AP 5, Rest (100 - energy) x sips, Reserve AP; empty: cleared (0x4F waits 1 s)"""
    w.ext("click")
    c = w.g8(CURSOR)
    if c not in (0x41, 0x4F):
        return
    if s8(w.g8(CURSOR + 1)) == 1 or c == 0x4F:
        w.ext("click")
        cond = s8(w.g8(CURSOR + 2))
        if cond > 0x14:
            n = 0x14
        elif w.g8(CURSOR) == 0x4F:
            n = cond
        else:
            n = s8((cond - 1) & 0xFF)
        if n <= 1:
            w.ext("inv_extra", 0x91020, 0x99EC9)
        else:
            w.ext("spend_ap", s, 5, 0)
            w.ext("slot_redraw", s[0x17A], 1)
            w.ext("rest", s, (100 - s8(s[0x18A])) * n, 1)
            w.ext("reserve_ap", s)
            w.ext("inv_extra", 0x91018, 0x9AC17)
            w.s8(CURSOR + 2, w.g8(CURSOR + 2) - n)
        if s8(w.g8(CURSOR + 2)) <= 0:
            if w.g8(CURSOR) == 0x4F:
                w.ext("delay", 1000)
            clear(w, CURSOR, 12)
            w.ext("inv_extra", 0x91020, 0x99EC9)
    else:
        w.ext("message", 0x98F7F)
    w.ext("cursor2", w.g8(CURSOR))


def camo(w, s):
    """camouflage kit (0x47, one at a time) over the body rect: +0x1B1 += 3, Spend AP 10, kit condition -20"""
    w.ext("click")
    if w.g8(CURSOR) != 0x47:
        return None
    armour = None
    if s8(w.g8(CURSOR + 1)) == 1:
        w.ext("click")
        s[0x1B1] = s[0x1B1] + 3
        w.ext("spend_ap", s, 0xA, 0)
        w.ext("slot_redraw", s[0x17A], 1)
        armour = w.ext("body", s)
        w.ext("inv_extra", 0x91018, 0x99BBD)
        v = (w.g8(CURSOR + 2) - 0x14) & 0xFF
        w.s8(CURSOR + 2, v)
        if s8(v) <= 0:
            clear(w, CURSOR, 12)
            w.ext("inv_extra", 0x91020, 0x99EC9)
    else:
        w.ext("message", 0x98FF7)
    w.ext("cursor2", w.g8(CURSOR))
    return armour


def slot_click(w, s):
    """left click outside the body area: the 8 merc slots (select another merc; with an item on the cursor both mercs
    need 6 AP when hp 15+). Returns (new soldier, armour), "leave", or None"""
    for k in range(8):
        x, y = ai.s16(w.g16(SLOT_POS + 4 * k)), ai.s16(w.g16(SLOT_POS + 4 * k + 2))
        if not mouse_in(w, x, y, x + 0x2F, y + 0x29):
            continue
        o = rec(w, k)
        if o[0x1BB] or s[0x17A] == k or (s8(o[0x183]) < 15 and s8(s[0x183]) < 15):
            if s[0x17A] != k:
                wait_left_up(w)
                return None
            if w.g8(CURSOR):
                w.ext("message", 0x97887)
                wait_left_up(w)
                return None
            wait_left_up(w)
            return "leave"
        if w.g8(CURSOR) and w.g8(GIVE_A) != k and s8(s[0x183]) >= 15 and not ai.msg_not_enough_ap(s, w, 6, 0x63):
            wait_left_up(w)
            return None
        if w.g8(CURSOR) and w.g8(GIVE_A) != k and s8(o[0x183]) >= 15 and \
                not ai.msg_not_enough_ap(rec_by_ptr(w, k), w, 6, 0x63):
            wait_left_up(w)
            return None
        if not w.ext("select", k, 0, 0):
            wait_left_up(w)
            return None
        if w.g8(CURSOR) and w.g8(GIVE_B) == 0x40:
            w.s8(GIVE_B, s[0x17A])
        set_skill_bonus(w, s)
        w.ext("w_2b6a0", s)
        s = rec_by_ptr(w, k)
        armour = w.ext("body", s)
        wait_left_up(w)
        return s, armour
    wait_left_up(w)
    return None


MERGE_TBL = 0x94AAA                       # 82 x (cursor type, hand type, result type, kind)
NOTE_MERGE, PCC_MERGE = 0x9843F, 0x90C28  # "Attempt to PERMANENTLY merge ...", "merge.pcc"
QUOTE_BUF = 0x2299BC


def vest_not_empty(w, owner):
    """MsgVestNotEmpty? 0x4A7BC: 0 + message when merc `owner` (< 0x40) has an item in a vest pocket (entries 7..11)"""
    if owner < 0x40:
        p = w.ptr_image(w.g32(G_SOLDIER_PTR + 4 * owner))
        for k in range(7, 12):
            if w.g8(p + 12 * k):
                w.ext("message", 0x9AE05)
                return 0
    return 1


def merge_items(w, s, hit, a, owner):
    """MergeItemsPrompt 0x4A838: may item `a` go to entry `hit` of s? 1 go on, 0 "huh", -1 cancelled.
    hit 12 = on the hand item (entry 0): attach or merge by the table MERGE_TBL (merge_apply)."""
    t = w.g8(a)
    if t == 0:
        return 1
    if 0x31 <= t <= 0x35:                                   # a vest
        if hit == 6:
            return 1
        if hit > 1:
            return 0
        return 1 if vest_not_empty(w, owner) else 0
    k = (hit - 2) & 0xFF
    if k > 10:
        return 1
    hit = k + 2
    if hit == 2:
        return 1 if 0x23 <= t <= 0x26 else 0
    if hit == 3:
        if not 0x27 <= t <= 0x28:
            return 0
        return 0 if t == 0x27 and s8(w.g8(a + 1)) > 1 else 1
    if hit == 4:
        return 1 if 0x2A <= t <= 0x2B else 0
    if hit == 5:
        return 1 if 0x2D <= t <= 0x30 else 0
    if hit == 6:
        return 1 if 0x31 <= t <= 0x35 else 0
    if hit < 12:
        return 0 if 0x31 <= t <= 0x35 else 1
    if w.g8(a + 3):
        return 0
    if max_stack(w, t) > 1 and s8(w.g8(a + 1)) > 1:
        return 0
    if max_stack(w, s[0]) > 1 and s8(s[1]) > 1:
        return 0
    for i in range(82):
        e = MERGE_TBL + 4 * i
        if t == w.g8(e) and s[0] == w.g8(e + 1):
            res = w.g8(e + 2)
            if res == 0:
                return 1
            if not w.ext("merge_prompt", NOTE_MERGE, PCC_MERGE, t, 10, 0x14, 0x10, s[0], 0x14, 0x32, 1, 1, 0):
                return -1
            r = merge_apply(w, a, s, i)
            if r < 2:
                return r
            s[0] = res
            if s8(w.g8(ITEM_TBL + 12 * res + 5)) == 0x63:
                s[1] = w.g8(ITEM_TBL + 12 * res + 5)
            return r
    return 0


def merge_apply(w, a, s, i):
    """0x4A4DC: merge cursor item `a` into the hand item (entry 0) by kind MERGE_TBL[i].3 -> 0 none, 1 failed
    (items damaged), 2 done, 3 done (kind 0), 4 condition added (kind 4)"""
    kind = w.g8(MERGE_TBL + 4 * i + 3)
    r = 0
    if kind == 0:
        r = 3
        w.ext("stat_practice", s, 4, 2, 0)
    elif kind in (1, 2):
        eff = s8(s[0x190]) - int((100 - s8(s[0x18C])) / 2)
        roll = w.ext("random", 100)
        if kind == 1 and roll >= eff:
            u = w.ext("random", 3)
            d = roll - eff
            hand = cur = 0
            if u == 0:
                hand = cur = int(d / 2)
            elif u == 1:
                hand = d
            elif u == 2:
                cur = d
            s[2] = s[2] - hand
            if s8(s[2]) <= 0:
                s[0] = s[1] = s[2] = 0
            w.s8(a + 2, (w.g8(a + 2) - cur) & 0xFF)
            if s8(w.g8(a + 2)) <= 0:
                clear(w, a, 3)
            w.ext("hide")
            w.ext("cursor", 0, 0x34, 0x25)
            r = 1
            w.ext("w_467e4", 0, 0x34, 0x25)
            w.ext("show")
            w.s8(0x92F8B, 1)
            p = s[0x179]
            w.ext("quote", p, 33 * p, 6, 200, QUOTE_BUF, 1, p, 1)
            w.ext("yes_no", QUOTE_BUF, p, s.addr + 0x16E, 0, 0)
            w.ext("w_28698", 0x25, 7, 5)
            w.ext("stat_practice", s, 7, 5, 0)
        else:
            r = 2
            w.ext("stat_practice", s, 7, 10, 0)
    elif kind == 3:
        eff = s8(s[0x18F])
        roll = w.ext("random", 100)
        r = 2
        w.ext("stat_practice", s, 6, 10, 0)
        hc, cc = s8(s[2]), s8(w.g8(a + 2))
        v = int((int((hc + cc) / 2) + int(hc * cc / 100)) / 2) + eff - roll
        s[2] = min(max(v, 10), 100)
    elif kind == 4:
        r = 4
        cc = w.g8(a + 2)
        if s8(s[2]) + s8(cc) <= 100:
            s[2] = s[2] + cc
            w.s8(a + 2, 0)
        else:
            w.s8(a + 2, (s[2] + cc - 100) & 0xFF)
            s[2] = 100
            s[3] = s[0]
            s[5] = w.g8(a + 2)
            s[4] = 0x63 if w.g8(ITEM_TBL + 12 * s[3] + 6) == 1 else 1
    return r


def item_click(w, s, armour):
    """left click in the body area: the 14 item rects. Returns the armour value or "leave" """
    hit = -1
    for k in range(14):
        x, y = ai.s16(w.g16(ITEM_POS + 4 * k)), ai.s16(w.g16(ITEM_POS + 4 * k + 2))
        if mouse_in(w, x - 4, y - 4, x + 0x2D, y + 0x13):
            hit = k
            if w.g8(CTRL):
                if unstack_one(w, s.addr + 12 * k):
                    w.ext("hide")
                    w.ext("draw_slot", s, s8(k), 0, 0x140)
                    w.ext("show")
                hit = -1
            break
    if hit < 0 or hit >= 13:
        if hit != 13:
            wait_left_up(w)
            return armour
        if w.g8(CURSOR) == 0:
            return pick_up(w, s, armour)
        return put_down(w, s)
    a = s.addr + 12 * hit
    if hit < 12 and w.g8(a + 1) == 0 and 0 < w.g8(a) <= 0xE and s8(w.g8(CURSOR + 1)) == 1 and \
            w.g8(CURSOR) == w.g8(AMMO_OF + 6 * w.g8(a)):
        t = w.g8(a)
        w.s8(a + 1, w.g8(a + 1) + w.g8(ITEM_TBL + 12 * t + 5))
        if s8(s[0x1D5]) > 0:
            w.ext("stat_practice", s, 3, 1, 0)
        w.ext("sfx", 0xF if w.g8(a) < 8 else 0xE, 0x2B11, 0x32, 1)
        ai.drop_item(w, CURSOR)
        w.ext("hide")
        w.ext("draw_slot", s, s8(hit), 0, 0x140)
        if hit == 0:
            w.ext("draw_slot", s, 0xC, 0, 0x140)
        w.ext("cursor", w.g8(CURSOR))
        w.ext("show")
        if w.g8(TEXT_ON):
            w.ext("merc_text", s, 0x9A083, 0x320)
        wait_left_up(w)
        return armour
    copy_item(w, SWAP, CURSOR)
    c = s8(merge_items(w, s, s8(hit), SWAP, w.g8(VEST_OWNER)) & 0xFF)
    if c < 0:
        wait_left_up(w)
        return armour
    if c == 0:
        w.ext("inv_extra", 0x9102C, 0x91028)
        wait_left_up(w)
        return armour
    w.s32(s.addr + 0xB4, 2000)
    w.s16(s.addr + 0xD0, 0xC41)
    s[0x130] = 1
    vest_done = False
    cur = w.g8(CURSOR)
    if hit == 6 and 0x32 <= cur <= 0x35:
        own = w.g8(VEST_OWNER)
        if own < 0x40 and own != s[0x17A]:
            swap_pockets(w, s, own)
        else:
            if own < 0x40:
                w.s8(VEST_OWNER, 0x40)
            if s[0x48] > w.g8(CURSOR):
                w.ext("w_467e4", 0, 0x34, 0x25)
                copy_item(w, CURSOR, a)
                copy_item(w, a, SWAP)
                drop_lost_pockets(w, s)
                w.ext("w_28698", 0x25)
                vest_done = True
                gave_item(w, s[0x17A])
    elif hit == 6:
        w.s8(VEST_OWNER, s[0x17A])
    if hit < 12:
        if not vest_done and not vest_pocket(s, hit):
            pocket_move(w, s, hit, a)
    elif s[0]:
        gave_item(w, s[0x17A])
        p = s.addr
        w.s8(CURSOR + 5, 0)
        w.s8(CURSOR + 4, 0)
        w.s8(CURSOR, w.g8(p + 3))
        w.s8(CURSOR + 3, 0)
        w.s8(CURSOR + 1, w.g8(p + 4))
        w.s8(CURSOR + 2, w.g8(p + 5))
        clear(w, CURSOR + 8, 4)
        if c < 2:
            for i in range(3):
                w.s8(p + 3 + i, w.g8(SWAP + i))
        else:
            w.ext("inv_extra", 0x91018, 0x91028)
            if c == 4:
                for i in range(3):
                    w.s8(p + 3 + i, 0)
            else:
                w.s8(CURSOR + 2, 0)
                w.s8(CURSOR + 1, 0)
                w.s8(CURSOR, 0)
    ai.drop_item(w, SWAP)
    if hit == 6:
        w.ext("hide")
        w.ext("draw_slot", s, 6, 0, 0x140)
        w.ext("cursor", w.g8(CURSOR))          # (EDX 1, EBX 0x10F, ECX 0xB5)
        w.ext("show")
        w.ext("vestmap", s)
        w.ext("click")
    else:
        w.ext("hide")
        if not vest_pocket(s, hit):
            w.ext("draw_slot", s, s8(hit), 0, 0x140)
            w.ext("click")
        if hit in (0, 0xC):
            w.ext("draw_slot", s, 0, 0, 0x140)
            w.ext("draw_slot", s, 0xC, 0, 0x140)
        w.ext("cursor", w.g8(CURSOR))
        w.ext("show")
    w.ext("w_55fd8", s, 1, 1)
    if ai.armor_rating(s) == armour:
        wait_left_up(w)
        return armour
    armour = w.ext("body", s)
    wait_left_up(w)
    return armour


def no_room(w, dst, src):
    """PlayNoRoom? 0x53A34 (EAX entry, EDX cursor): the entry full -> "No Room"; else units move from the end of the
    cursor's stack to the end of the entry's stack until the entry is full"""
    n = max_stack(w, w.g8(dst)) - s8(w.g8(dst + 1))
    if n < 1:
        w.ext("inv_extra", 0x9100C, 0x99EE3)
        return
    for _ in range(n):
        sc = s8(w.g8(src + 1))
        b = w.g8(src + 6 + sc)
        w.s8(src + 6 + sc, 0)
        w.s8(src + 1, (sc - 1) & 0xFF)
        dc = (w.g8(dst + 1) + 1) & 0xFF
        w.s8(dst + 1, dc)
        w.s8(dst + 6 + s8(dc), b)


def pocket_move(w, s, hit, a):
    """cursor <-> pocket `hit` (0..11) after the merge prompt: same stacking type = add to the stack (shift: swap),
    no room -> "No Room"; a stack with an empty cursor: take one (shift: the whole stack); else swap"""
    cur = w.g8(CURSOR)
    if cur == w.g8(a) and max_stack(w, cur) > 1 and hit != 3:
        w.ext("click")
        if w.g8(SHIFT):
            copy_item(w, CURSOR, a)
            copy_item(w, a, SWAP)
            gave_item(w, s[0x17A])
            return
        if max_stack(w, w.g8(a)) < s8(w.g8(a + 1)) + s8(w.g8(CURSOR + 1)) or hit == 3:
            if 1 < hit < 7:
                w.ext("inv_extra", 0x9100C, 0x99EE3)
            else:
                no_room(w, a, CURSOR)
            return
        merge_stack(w, a, CURSOR)
        ai.drop_item(w, CURSOR)
        gave_item(w, s[0x17A])
        return
    if s8(w.g8(a + 1)) <= 1 or max_stack(w, w.g8(a)) <= 1:
        if hit == 3 and w.g8(CURSOR) == 0x27 and s8(w.g8(CURSOR + 1)) > 1:
            w.ext("inv_extra", 0x9100C, 0x99EE3)
            return
        copy_item(w, CURSOR, a)
        copy_item(w, a, SWAP)
        gave_item(w, s[0x17A])
        return
    if w.g8(CURSOR) == 0:
        if w.g8(SHIFT):
            copy_item(w, CURSOR, a)
            copy_item(w, a, SWAP)
            return
        for i in range(12):                       # 0x47090 copies 12 bytes (EBX = 4 is memset's count)
            w.s8(CURSOR + i, w.g8(a + i))
        w.s8(CURSOR + 1, 1)
        clear(w, CURSOR + 8, 4)
        ai.use_throw(w, a)
        if w.g8(a + 1) == 0:
            ai.drop_item(w, a)
        return
    copy_item(w, CURSOR, a)
    copy_item(w, a, SWAP)
    gave_item(w, s[0x17A])


def put_down(w, s):
    """cursor item to the ground rect: PutItem on the merc's tile (a vest takes its owner's pockets along)"""
    gave_item(w, s[0x17A])
    w.s8(PICKED + s[0x17A], 1)
    w.ext("click")
    if 0x32 <= w.g8(CURSOR) <= 0x35:
        own = w.g8(VEST_OWNER)
        if own < 0x40:
            for off in range(0x54, 0x90, 0xC):
                p = rec_by_ptr(w, w.g8(VEST_OWNER)).addr + off
                put_item(w, tile(s), p)
                ai.drop_item(w, rec_by_ptr(w, w.g8(VEST_OWNER)).addr + off)
        w.s8(VEST_OWNER, 0x40)
    put_item(w, tile(s), CURSOR)
    ai.drop_item(w, CURSOR)
    armour = w.ext("body", s)
    w.ext("cursor2", w.g8(CURSOR))
    wait_left_up(w)
    return armour


def pick_up(w, s, armour):
    """empty cursor on the ground rect: the tile's ground item (none when buried: b6 = 0xFF and condition < 0);
    first pick-up per merc costs 4 AP; mine check, disarm prompt, PickUpItem. Returns armour or "leave" """
    t = tile(s)
    g = w.tile8(t, 6)
    if s8(w.g8(G_GROUND + 12 * g + 6)) == -1 and s8(w.g8(G_GROUND + 12 * g + 2)) < 0:
        g = 0xFF
    slot = s[0x17A]
    if not w.g8(PICKED + slot) and (g == 0xFF or not ai.msg_not_enough_ap(s, w, 4, 0x63)):
        wait_left_up(w)
        return armour
    if not w.g8(PICKED + slot):
        w.ext("spend_ap", s, 4, 0)
        w.s8(PICKED + s[0x17A], 1)
    mine = w.ext("mine_spotted", s, t, g) & 0xFF
    r = ai._i32(w.ext("disarm", s, t, g, 0, 0))
    sec = G_SECTORS + 0xB1 * w.g8(G_CUR_SECTOR) + 0xAF
    if r > 0:
        w.ext("body", s)
        w.ext("pick_up", s, tile(s), 0)
        if mine:
            w.s8(sec, w.g8(sec) - 1)
        w.ext("cursor", w.g8(CURSOR))
        armour = w.ext("body", s)
        wait_left_up(w)
        return armour
    g2 = ai.item_index(w, tile(s))
    if g2 == 0xFF:
        w.s8(sec, w.g8(sec) - 1)
        w.ext("focus", s[0x17A], 1)
        return "leave"
    armour = w.ext("body", s)
    if not mine:
        wait_left_up(w)
        return armour
    a = G_GROUND + 12 * g2 + 2
    w.s8(a, -s8(w.g8(a)))
    wait_left_up(w)
    return armour
