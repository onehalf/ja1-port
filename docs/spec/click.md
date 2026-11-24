# The tactical click: a soldier uses an item on a tile

What happens when a soldier is told to use an item (or the bare hand) on a tile: the player's click, and the AI's
attacks and uses (ai.md "Starting the chosen action": host op 30 click). **Tactical click** (op 30) decides whether
the soldier must walk first, aims guns and knives (the attack then starts with the attack animation, shooting.md
batch 5), and starts the use animations of the other items. **Attack ready** (op 15), **Reach tile** (op 14) and
**Gun jam** (op 68, an animation word, ai.md) belong to it. These four were host ops until now; they become
library ops (the library runs them unless their host_ops bit is set: 14, 15 and 30 have their own bits, 68 uses
bit 63).

Conventions as in shooting.md: s = the soldier record of `slot`, NONE = 0xC41, px / py, occupant(t),
tile_standable(t), t0 / t1 of a tile, column(t) / row(t) (ai.md "Tile parts"), World distance (ai.md), Water tile
(t0, t1) and In water (soldier_update.md), the current animation (shooting.md batch 5), cdiv. Record values are
bytes unless written u16 / s16; "s8" = the signed byte.

## Entry points

```c
int32_t ja1_click(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t item);   /* = library op 30 */
int32_t ja1_attack_ready(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);          /* = library op 15 */
int32_t ja1_reach_tile(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);  /* op 14 */
void    ja1_gun_jam(Ja1World *w, const Ja1Host *h, int32_t slot);                             /* = library op 68 */
/* also an entry point (tests): */
int32_t ja1_jam_check(Ja1World *w, const Ja1Host *h, int32_t slot);
```

(tile: low 16 bits, signed; item, flag: low bytes.) Results are i32.

New world field (port_api.md, "click"): `key_ctrl` (u8: Ctrl is held; the panel is not updated after a click).
Existing fields used: `selected`, `combat`, `mission_spot`, `opt_step_text`, `ground_items`, `item_types` (byte 2 =
kind, byte 7 ≠ 0 = the item can jam, byte 11 = range in tiles), `anim_table` (column 0 = can act, 1 = can attack),
`t1_class`, `tiles`, `soldiers`.

New host ops: 191 first_aid (slot, tile, byte a, byte b): result low byte ≠ 0 = first aid starts (a, b: 6, 5 on the
own tile, else 5 and the facing). 192 attack_cursor (): none (the front-end's attack cursor timer, 40000). 193 voc
(int 0x2B11, int 0x32, int 1): none (the gun click sound). Host op 125 message gets the texts 2 "No one there", 3
"Can't get there!", 4 "Can't bury mine there!", 5 "drink". Other ops used: 6 direction, 17 action_ap, 29 walk_to,
31 spend_ap, 41 find_path, 44 quest_check, 52 set_anim, 54 merc_message, 57 merc_text, 60 sfx, 99 focus, 100
cry, 105 stat_practice, 121 use_cost, 140 move_order, 166 item_panel, 174 item_cond, 185 click_panel, 190 reloaded;
the procedures Aim posture and Forget the attack.

**AP check** (ap, flag) → 1 = affordable: if (selected = slot, or s8 flag = 99) and combat ≠ 0 and s8 +0x1E2 + s8
+0x1A5 < u8 ap: if slot < 8, host op 57 merc_text (slot, 1, 0x4B0), else +0x1B0 = 1; result 0. Else 1.

**Face point** (tile) = low byte of op 6 direction (s16 +0x116, s16 +0x118, column(tile) × 16 + 8, row(tile) × 14
+ 7).

**Clear the task** (flag): +0x1A6 = 0; u16 +0x134 = NONE; +0x1A7 = 0; if flag ≠ 0: +0x95 = 0, u16 +0x96 = NONE.

**Ground index** g of a tile: its byte 6; 0xFF if that is 200 or more.

## Tactical click (`ja1_click`, op 30 click)

Result 1 = a walk to a use spot started, else 0.

1. u32 +0xB0 = 0. occ = occupant(tile). g = Ground index of tile.
2. If anim_table[8 × current animation] = 0: result 0.
3. it = item (low byte). k = item_types[12 × it + 2].
   - it = 0: range = 1, reach = 15, k = 0.
   - k = 0x95: r = item_types[12 × it + 11] × 15 + s8 +0x183 − 50; r = max(r − cdiv((100 − s8 +0x18A) × r, 200),
     0); reach = r (u16); range = low byte of cdiv(s16 reach, 15).
   - else: range = item_types[12 × it + 11]; reach = range × 15 if range > 1, else 0x18.
4. me = s16 +0x114. walk = no.
   - If World distance (s16 +0x116, s16 +0x118, px(tile), py(tile)) > s16 reach: if it = 0, or (it > 0x23 and it ≠
     0x5D), or range = 1: walk = yes, and if it ≠ 0: +0x1A6 = it.
   - Else: if range = 1 and (column(me) ≠ column(tile) and row(me) ≠ row(tile), or tile_standable(tile) > 0 and me
     ≠ tile): walk = yes, and if it ≠ 0: +0x1A6 = it.
5. If combat = 0: u16 +0x134 = tile.
6. If walk:
   a. +0x1A7 = range.
   b. If range ≠ 1: op 29 walk_to (slot, tile); +0x1A6 = +0x00; +0x1A7 = range; result 0.
   c. task = +0x1A6; stop = no. If task is 0xF, 0x10, 0x49 or 0x4A: if occ = 0x40: host op 125 message (2); stop
      = yes; +0x1A6 = 0. Else if task = 0x45: if t1_class[t1 of tile] is 7 or 8: +0x1A6 = 0x45; else stop = yes,
      +0x1A6 = 0.
   d. If op 14 reach_ap (slot, tile, +0x1A6) = 0 (32 bits): host op 125 message (3); +0x1A6 = 0; result 0.
   e. If stop: result 0.
   f. spot = s16 mission_spot. c = low byte of host op 121 use_cost (slot, spot, tile, g, k).
   g. If AP check (c, 0) = 1 and not Water tile (t0 of spot, t1 of spot): +0x1A6 = 0xFF if k = 0, else it; u16
      +0x134 = tile; op 29 walk_to (slot, spot); if key_ctrl = 0: host op 185 click_panel (slot, 1); result 1.
   h. u16 +0x134 = NONE; +0x1A6 = 0; result 0.
7. done = no. **Finish** below = if done and key_ctrl = 0: host op 185 click_panel (slot, 1); then result 0.
8. Guns, 1 ≤ it ≤ 0xE:
   a. Aim posture (slot, tile).
   b. If Jam check = 0: host op 185 click_panel (slot, 1); result 0.
   c. If op 15 attack_ready (slot, tile) = 1 (32 bits): +0x1D8 = occ; if occ < 0x40: if s8 record occ +0x9C > 0:
      +0x1C2 = occ, and if slot < 0x28 and occ < 0x28: +0x1C3 = 1. If occ = 0x40: +0x1C2 = 0xFE. +0x1BE = 1; u16
      +0xDC = tile; host op 192 attack_cursor (); done = yes.
   d. Else if +0x01 = 0 and host op 190 reloaded (slot) ≠ 0 (32 bits) and slot < 8: op 54 merc_message (slot, 5,
      0x40, 0, 1); +0x1CD = 1.
   e. Finish.
9. If it ≠ 0 and Jam check = 0: host op 185 click_panel (slot, 1); result 0. If In water: host op 185 click_panel
   (slot, 1); result 0.
10. **Use** (cost, sound, task, anim, reset, face): if AP check (cost, 0) = 1: +0x1A6 = task; u16 +0x134 = tile; op
    52 set_anim (slot, anim, 0, reset); if a sound is given: op 31 spend_ap (slot, s8 cost, sound); if a face is
    given: +0x1A2 = face. Always done = yes.
11. By it:
    - 0 (the hand): if tile = me: Use (4, sound 0, task 0xFF, anim 0xB, reset 0, no face). Else: d = Face point
      (tile); if tile_standable(tile) < 0: Use (3, 0x1E, 0xFF, 0x26, 0, d); else c = 2 if slot ≥ 0x28 and g < 200
      and ground_items[12 × g] = 0x72, else 5; Use (c, 0, 0xFF, 0xC, 0, d). Finish.
    - 0x4C, 0x57, 0x58: (cost, sound) = (10, −250) for 0x4C, else (3, 0x1E). d = Face point (tile). If
      tile_standable(tile) < 0, or it = 0x4C and the low byte of op 44 quest_check (tile) ≠ 0: Use (cost, sound,
      it, 0x26, 0, d). done = yes. Finish.
    - 0x4D: op 140 move_order (slot, tile, 1, 0, 1, 0, 1, 0). Finish. (Confirm 1: a merc then waits for a second click on
      the tile, but only with the game's confirm-move option on; +0x177 stays 0 during the wait. The library cannot
      wait, so its Move order ignores confirm and the window uses `ja1_click_detector`: cursor.rs `detector_move`.
      Live check: #45.)
    - 0xF, 0x10 (knives): if tile = me: Finish. If AP check (low byte of op 17 action_ap (slot, tile, 1), 0) = 1:
      Aim posture (slot, tile); if op 15 attack_ready (slot, tile) = 1: if occ < 0x40: +0x1D8 = +0x1C2 = occ, and
      if slot < 0x28 and occ < 0x28: +0x1C3 = 1; then +0x1BE = 1; u16 +0xDC = tile; done = yes. Finish.
    - 0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D, 0x5D (thrown): if AP check (low byte of op 17 action_ap (slot, tile, 1),
      0) = 1: d = Face point (tile); +0x1C2 = 0xFE; +0x1A2 = d; u16 +0xDC = tile; done = yes; +0x1BE = 1; host op
      185 click_panel (slot, 1). Finish.
    - 0x49, 0x4A (first aid): if AP check (5, 0) = 1: if tile = me: d = 5, r = low byte of host op 191 first_aid
      (slot, me, 6, 5); else d = Face point (tile), r = low byte of first_aid (slot, tile, 5, d). done = (r ≠ 0).
      Clear the task (1). Host op 185 click_panel (slot, 1). If r = 0: +0x1A2 = d; else u16 +0x120 = NONE. Finish.
    - 0x11 (mine), 0x17, 0x19, 0x1B, 0x1E, 0x1F, 0x20: for 0x11: if tile ≠ me and tile_standable(tile) ≤ 0: host
      op 125 message (4); Clear the task (1); done = yes; Finish. (cost, sound) = (10, 0xFA) for 0x11, else (6,
      0x14). If tile ≠ me: d = Face point (tile). If AP check (cost, 0) = 1: +0x1A6 = it; u16 +0x134 = tile; if
      tile = me: op 52 set_anim (slot, 0x13, 0, 1); else +0x1A2 = d and op 52 set_anim (slot, 0x12, 0, 1); then op
      31 spend_ap (slot, cost, sound). done = yes. Finish.
    - 0x76: if tile = me: Finish. d = Face point (tile); Use (10, 0, 0x76, 0xC, 0, d). Finish.
    - 0x45: if tile = me: Finish. If t1_class[t1 of tile] is not 7 or 8: +0x1A6 = 0; Finish. +0x1A6 = 0x45; u16
      +0x134 = tile; d = Face point (tile); if AP check (5, 0) = 1: +0x1A2 = d and op 52 set_anim (slot, 0x2E, 0,
      1). done = yes. Finish.
    - 0x87: if tile = me: Finish. d = Face point (tile). If AP check (8, 0) = 1: +0x1A6 = 0x87; +0x1A2 = d; u16
      +0x134 = tile; op 52 set_anim (slot, 0xC, 0, 1); op 31 spend_ap (slot, 8, −50). Else Clear the task (1).
      done = yes. Finish.
    - 0x46 (a drink): if s8 +0x01 > 0 and random(100) < s8 +0x02: op 60 sfx (0x2C, 0x2B11, 0x58, 1); if
      opt_step_text ≠ 0: host op 125 message (5); +0x01 = +0x01 − 1; if selected = slot: op 174 item_cond (0, slot,
      0, 1). Then as below.
    - any other item (and 0x46 after its step): done = yes; Clear the task (1); Finish.

(The original reads entry 255 of its ground item list for g = 0xFF in the hand step; that entry is never 0x72 in
the game, so the port tests g < 200.)

## Jam check (`ja1_jam_check`)

Result 0 = the hand item jams now, else 1.
1. it = +0x00, c = s8 +0x02. If item_types[12 × it + 7] ≠ 0 and c > 15 and it ≠ 0x11: if random(0x50) + 20 > c +
   50: +0x02 = random(10) + 1; if selected = slot: op 166 item_panel (slot, 0, 1); op 54 merc_message (slot, 0xF,
   0x40, 0, 1); result 0.
2. Result 1.

## Attack ready (`ja1_attack_ready`, op 15 attack_ready)

Result (i32): 1 = the soldier can attack now. a = the current animation.
1. If tile = s16 +0x114: −3. If Water tile (+0x1B2, +0x1B4): −2. If anim_table[8 × a + 1] = 0: −4.
2. If +0x00 is not 1..0x10 and item_types[12 × +0x00 + 2] ≠ 0x95: −6. If +0x01 = 0: −5.
3. If anim_table[8 × a] = 0: −1. Else 1.

## Reach tile (`ja1_reach_tile`, op 14 reach_ap)

Result 1 = the soldier can get to the tile or next to it; mission_spot = the spot.
1. st = tile_standable(tile). If st > 0: if op 41 find_path (slot, tile, 0) ≠ 0: mission_spot = tile; result 1.
   Else u16 +0xD4 = 0.
2. b = t1 of tile. best = 0x100. For i = 0..3 with d = 1, 3, 5, 7: n_i = step(tile, dirStep(d)); n_i is a candidate
   if, in this order (stop at the first that fails): tile_standable(n_i) > 0; op 41 find_path (slot, n_i, 1) ≠ 0;
   not Water tile (t0 of n_i, t1 of n_i); s16 +0xD4 < best; and (0 < flag < 0xFE, or st < 0, or b < 0x37 or b ≥
   0x83, or n_i > tile). A candidate: pick = i, best = s16 +0xD4.
3. If best ≥ 0x100: result 0. mission_spot = n_pick; result 1.

## Gun jam (`ja1_gun_jam`, op 68 gun_jam)

1. If slot ≥ 8 or +0x1C9 ≠ 0: done. c = s8 +0x01. If c = 0: done.
2. If c > 0: v = s8 +0x02 + random(0x32); if random(0x64) < v: done.
3. If c < 0: m = s8 +0x190; if random(0x64) < (cdiv(m, 2) + 0x14 if m > 0, else 0x14): +0x01 = −c; op 166
   item_panel (slot, 0, 1); op 100 cry (+0x179, −1, 1, 1, 1) ("GOOD"); host op 193 voc (0x2B11, 0x32, 1); op 105
   stat_practice (slot, 7, 2, 0); done.
4. Op 99 focus (slot, 1); op 100 cry (+0x179, −1, 0, 1, 1) ("BAD"); host op 193 voc (0x2B11, 0x32, 1).
5. If c > 0: +0x01 = −c; op 166 item_panel (slot, 0, 1); Forget the attack (1); op 54 merc_message (slot, 0x10,
   0x40, 0, 1). Else: Forget the attack (1).

## Tests

- `ai_diff.py --only click --port`, `--only attack_ready --port`, `--only reach_tile --port`, `--only gun_jam
  --port`, `--only w_59a54 --port`: 0 mismatches.
- `walk_diff.py --shoot` with the click in the port (the whole shot from the click on): 0 mismatches.

Evidence: tools/ai.py tactical_click (TacticalClick? 0x59C14), attack_ready (0x584A0), reach_tile (0x3DC4C),
gun_jam (0x58574), jam_check (0x59A54); ai_diff deciders of the same names, diff-tested against the original.
