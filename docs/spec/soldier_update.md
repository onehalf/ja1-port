# Soldier update per frame

What the tactical loop does for one soldier in one frame: the walk (one pixel step after the other, tile by
tile), the end of a walk, a soldier that stands up, the queued merc quote, and, for the soldier in turn, the
things that happen on each new tile (AP, sector triggers, mines, sightings, the hand-over of the turn).

The tactical loop runs, per frame and for each soldier in the sector and in play (+0x188 = 0) whose timer
u32 +0xB0 ran out: **Active frame** for the soldier whose turn it is (and for any merc out of combat), else
**Soldier frame**. (The loop itself, its timers and the turn hand-over are not part of this page yet.)

Conventions as in ai.md "Search procedures": s = the soldier record of `slot`, NONE = 0xC41, px(t) / py(t) =
the pixel point of tile t, occupant(t), step(t, k), dirStep(d), tile_standable(t), Occupy, Vacate, In water,
Clear the task, Next path direction, Start a leg, Stop on the own tile (ai.md "Next animation" and "Frame
routine" helpers). cur = the current animation word u16 +0x1E8 + 2 × +0x1F3 (read again at each use unless a
step says otherwise); code = its low byte. **still**(a) = anim_rows[8 × a + 3] (column 3: ≠ 0 = the animation
does not move the soldier); **can act**(a) = anim_table[8 × a + 0]; **turns**(a) = anim_table[8 × a + 2].
"Seen" = s8 +0x9C > 0. "Merc" = slot < 8, "enemy" = slot ≥ 0x28. All tests of a record byte are on the
unsigned byte unless "s8" is written. "Host op N name (args)" = a host call (port_api.md); its result is a raw
32-bit value, and the step says which part counts.

Library ops used here (the library runs them itself unless their host_ops bit is set): 52 set_anim, 84
anim_step, 97 anim_next (= ai.md "Next word"), 98 walk_blocked (this page), 106 consume_trigger (this page,
Consume trigger), 109 take_ground_item (sector_start.md batch 4, keep = the second argument), 111 enemy_step (this
page, Enemy step seen), 117 step_noise (this page, Step noise), 119 ai_think (= ai.md "One tick"), 122
throw_follow (this page, Throw follow-up). Ops 63 and up share host_ops bit 63.

## Entry points

```c
void ja1_soldier_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_active_frame(Ja1World *w, const Ja1Host *h, int32_t slot);
/* the parts, also entry points (tests): */
void ja1_walk_step(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_walk_blocked(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t occupant);
void ja1_walk_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t code);   /* code: low byte */
void ja1_stand_up(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_frame_checks(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_merc_quote(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_enemies_see(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_enemy_step_seen(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_step_noise(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_consume_trigger(Ja1World *w, const Ja1Host *h, int32_t index);  /* index 0..9 */
void ja1_throw_follow(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_throw_aim(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);       /* tile: signed 16-bit */
void ja1_throw_scatter(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t margin, int32_t distance);
        /* margin: low byte; distance: signed 16-bit */
```

`ja1_walk_blocked`: tile = a signed 16-bit tile, occupant = its low byte.

New world fields (port_api.md, "soldier update"): `screen_drawn`, `interrupt_go`, `turn_moves`, `kept_merc`,
`next_selected`, `quote_slot`, `quote_tile`, `quote_focus`, `quote_delay`, `quote_event`, `opt_half_delay`,
`opt_follow`, `opt_reload_msg`, `alone_warned`, `follow_all`, `stop_timer`, `interrupted`, `walk_ap`,
`ap_warned` (8 bytes), `splash` (60 bytes), `spotted`, `opt_step_text`. Existing fields read here: `combat`, `selected`, `cur_sector`,
`path_preview`, `opt_onscreen`, `enemy_watch`, `sectors`, `ground_items`, `move_codes`, `paths`,
`item_types`, `anim_table`, `anim_rows`, `t0_class`, `t1_class`, `t2_class`, `dir_steps`, `text_buf`.

## Helpers

- **Tile point stop** (flag): u16 +0x126 = px(s16 +0x114), u16 +0x128 = py(s16 +0x114); host op 53 stop_anim
  (slot, flag). ("Stop on the own tile" in ai.md is this with flag 1.)
- **At the step point**: u16 +0x126 = u16 +0x116 and u16 +0x128 = u16 +0x118.
- **Stored-target flag**: 1 if u16 +0x120 = u16 +0x114 or s16 +0x120 ≥ NONE (signed), else 0.
- **At the waypoint** (of a record r): 0 if r +0x1B9 < 5 and r's current animation word ≥ 4 and (u16 r +0x114 ≠
  u16 r +0x124 or u16 r +0x116 ≠ u16 r +0x126 or u16 r +0x118 ≠ u16 r +0x128); else 1.
- **Water tile** (t0, t1): t0 ≥ 0x6C and t1 is neither 0x1C nor 0x1F.
- **Ground index** of tile t: g = byte 6 of t; 0xFF if g ≥ 200, else g.
- **Step energy** of a move code m (the breath a step costs): 1000 → 5, 1200 → 10, 1500 → 5, 1600 → 20, 2000 →
  35, 3000 → 50, 3600 → 75, 5000 → 100, 25300 → 5, 25400 → 5, any other code → 0.
- **Step AP** of tile t = low byte of "AP to enter a tile" (combat.md) for move code move_codes[t], sneak
  +0x1A8, byte177 +0x177, alt +0x196.
- **Low-AP stop**: ap_warned[slot] = 1; stop_timer = 2000 (only where the step says "with +0x16B": then only if
  +0x16B ≠ 0); tile point stop (0); Clear the task with +0x95 / +0x96 kept (+0x1A6 = 0, u16 +0x134 = NONE,
  +0x1A7 = 0); +0x16D = 1; op 120 low_ap_stop (slot, 1, 0) (library op: Select merc, select.md).
- **Forget the attack** (flag): +0x1D8 = +0x1C2 = 0x40; +0x1BE = 0; u16 +0xD0 = u16 +0xDC = NONE; if flag and
  turns(cur) ≠ 0: op 84 anim_step (slot).
- **Exhausted fall**: if s8 +0x183 < 15 and 4 ≤ cur < 0x28 (the whole word): tile point stop (1); host op 55
  ui_refresh (1 if path_preview ≠ 0, else 0); op 52 set_anim (slot, 0x40 if In water else 0x2B, 0, 0).
- **Soak items** (deep water): for k = 0..11, item entry k at +12 k: if its type (byte 0) ≠ 0 and
  item_types[12 × type + 8] ≠ 0: r = random(100); if r < 10: byte 2 = byte 2 − r (8-bit); if s8 byte 2 < 0:
  byte 2 = random(8) + 2.
- **Fire trigger** (index, keep): host op 54 merc_message (slot, 4, 0x40, 0, 1); tile point stop (1); if keep =
  0: op 106 consume_trigger (index).
- **Tile triggers**: row = sectors row cur_sector; for i = 0..9: tile u16 row[0x58 + 4 i], value s16
  row[0x5A + 4 i]. If u16 +0x114 ≠ tile: next. If value < 0: if s8 +0x192 ≤ |value|: Fire trigger (i, 0), then
  host op 105 stat_practice (slot, 4, 5, 0). If value ≥ 0: if s8 +0x192 ≥ value: Fire trigger (i, 0), then host
  op 105 stat_practice (slot, 4, 10, 0).

## Walk move

Moves the pixel point one frame toward the step point and gives the tile under the new point.

1. x = s16 +0x116, y = s16 +0x118, tx = s16 +0x126, ty = s16 +0x128, v = +0x19F (unsigned).
2. If x < tx: x = min(x + v, tx); else if x > tx: x = max(x − v, tx). The same for y with ty.
3. u16 +0x116 = x, u16 +0x118 = y (low 16 bits).
4. With f = +0x1A2: f = 2 → x = x + 5; f = 4 → x = x + 1; f = 6 → y = y + 3.
5. Result = vt(s16 x, s16 y) = cdiv(x, 16) + cdiv(y, 14) × 56 (signed 32-bit, cdiv truncates toward zero).

## Walk step (`ja1_walk_step`)

One frame of a walk.

1. +0x193 = 1. old = s16 +0x114; keep ox = u16 +0x116, oy = u16 +0x118, speed = +0x19F; a = code (low byte).
2. If still(a) = 0: new = Walk move. Else new = old, and if a < 4 and s8 +0x1A5 < 5: +0x1B8 = 1; host op 72
   stop_walk (slot, 0); if s8 +0xBA ≥ 1: +0x1B0 = 1.
3. If new = old: op 97 anim_next (slot). Stop.
4. o = occupant(new). If o ≠ 0x40: u16 +0x116 = ox, u16 +0x118 = oy, +0x19F = speed; op 98 walk_blocked (slot,
   new, o). Stop.
5. Tired walk: if slot < 8 and s8 +0x18A < 0x14 and +0x1BA = 0 and 0x15 ≤ cur ≤ 0x17 and +0x1D5 = 0: +0x1BA = 1;
   host op 54 merc_message (slot, 0x1E, 0x40, 0, 1).
6. Vacate(s16 +0x114). u16 +0x114 = new. +0x1B2 = byte 0 of new, +0x1B3 = t0_class[+0x1B2]; +0x1B4 = byte 1,
   +0x1B5 = t1_class[+0x1B4]; +0x1B6 = byte 2, +0x1B7 = t2_class[+0x1B6].
7. Frame delay (ja1_anim_delay, ai.md).
8. If s16 +0x1F8 ≠ NONE and host op 7 tile_dist (s16 +0x114, s16 +0x1F8) (i32) > 8: u16 +0x1F8 = NONE.
9. +0x1AE = 0; +0x1F2 = +0x1F2 + 1 (8-bit). Op 97 anim_next (slot).
10. Follow: if selected = slot and (seen or follow_all ≠ 0) and combat ≠ 0 and (slot ≥ 8 or opt_follow ≠ 0):
    host op 99 focus (selected, 1).
11. Splash: stop here if +0x1B2 < 0x97, or +0x1B4 ≠ 0, or s8 +0x183 ≤ 0. Stop if random(s8 splash[cur_sector])
    ≠ 0 (random(n) with n ≤ 0 gives 0). If cur ≥ 0x3C or +0x179 ≥ 0x3C: stop if random(5) ≠ 1.
12. Op 52 set_anim (slot, 0x3D, 0, 1). If +0x179 < 0x40: host op 99 focus (slot, 1); host op 100 cry (+0x179,
    −1, 0 = "BAD", 1, 1); host op 101 voice (0x2B11, 0x32, 1).

## Walk blocked (`ja1_walk_blocked`; op 98)

The next tile `tile` holds occupant `occupant` (from Walk step: the occupant; from Step start: byte 5 & 0x7F).
target = s16 +0x124. "Stop (f)" = tile point stop (f).

1. If combat ≠ 0: if tile = target: stop (1), op 84 anim_step (slot); else stop (0). Done.
2. If occupant = 0x40: if tile_standable(target) ≠ 1: if tile = target, or host op 41 find_path (slot, target, 1)
   returns 0 (all 32 bits): stop (1); else Start a leg. Done. If tile_standable(target) = 1: go to step 4.
3. Occupant ≠ 0x40: if tile_standable(target) < 1 and host op 7 tile_dist (s16 +0x114, target) (i32) < 4: stop
   (1); done. +0x1AE = +0x1AE + 1 (8-bit). Patience = 0x32 for slot < 8, else 0; if +0x1AE > patience: stop (1);
   done. If occupant ≥ 0x40, or At the waypoint (record occupant) = 0: done (it waits). If tile = target: stop
   (1); done.
4. u16 +0x12A = u16 +0x124; Start a leg.

## Walk end (`ja1_walk_end`)

code = the argument's low byte (the animation byte the caller read).

1. If s8 +0x18A ≤ 0 and 4 ≤ code < 0x32: host op 55 ui_refresh (path_preview ≠ 0); op 52 set_anim (slot, 0x2B,
   0, 0).
   Else if u16 +0x114 = u16 +0x120 or s16 +0x120 = NONE: if code < 0x32: if s16 +0xDC = NONE, op 52 set_anim
   (slot, 4, 0, 0); if code ≥ 0x32: op 52 set_anim (slot, 0x33 if +0x1B2 < 0x97, 0x36 if +0x1B2 < 0x9A, else
   0x39; 0, 0).
2. Host op 53 stop_anim (slot, stored-target flag). Host op 72 stop_walk (slot, 0). +0x1B8 = 1.
3. If slot ≥ 0x28: enemy_watch[slot] = 1. If slot ≥ 8 and +0x197 ≠ 0: +0x1B0 = 1.

## Stand up (`ja1_stand_up`)

1. Who: if screen_drawn ≠ 0: done for slot ≥ 0x28, and for slot ≥ 8 when combat ≠ 0. If screen_drawn = 0: done
   for slot < 8.
2. a = cur. If a ≥ 3 or +0x1B9 ≠ 0: done. u16 +0xD0 = NONE.
3. If s8 +0x18A < 10 or a > 1 or s8 +0x1A5 < 5: done.
4. +0x1CC = 1; op 52 set_anim (slot, 4, 0, 0); tile point stop (1); +0x1B8 = 0; +0x1C1 = 1.

## Queue a merc quote (host op 103 quote_event)

Arguments: slot (byte), event e (byte), delay d (byte), tile t (s16), focus f (byte). The front-end does it as
QueueQuote? 0x1B2A4 does:
1. If quote_slot < 0x40 and quote_event = 0xFE: quote_delay = 0, then Merc quote (`ja1_merc_quote`) for the
   record quote_slot (the pending quote ends now).
2. quote_slot = slot, quote_event = e, quote_focus = f, quote_delay = d (signed); if t ≠ 0xC41 (NONE):
   quote_tile = t.
The soldier update then plays it (Merc quote below, after d passes of that merc). Evidence (2026-10-28): asm
0x1B2A4..0x1B30C (ret 4: f is the one stack argument); smoke step quote_queue (merc 1 picks up at its own tile
where only item type 68 lies: event 0xFC, delay 1, then the "NOTHING" cry).

## Team quote (host op 202 won_quote)

Arguments: quote q (byte, signed), range r (int), a flag (not read). The front-end does it as TeamQuote? 0x61688
does: mercs 0..7 each get one try, in a random order: tried[0..7] = 0; 8 rounds: k = random(8), drawn again until
tried[k] = 0; tried[k] = 1. Record R = soldier k. Skip R unless +0x182 ≠ 0, +0x138 ≠ 0, s8 +0x183 ≥ 15, +0x188 = 0
and tile_dist (s16 +0x114, 0x15B) ≤ r (tile 0x15B = Santino's tile in sector 0). Else: Merc message (R, q, near
0x40, empty text), then the view is drawn again. Each merc's message ends before the next round (one speech balloon
at a time, its voice with it), so the last merc to speak holds the last quote (world_load.md "The last quote").
In player terms: when Santino's sector is won, every healthy merc near his house says the "we won" line, one after
another. Evidence (2026-10-28): asm 0x61688..0x61742 (called at 0x10FE1 with q 0x1D, r 0x63); BL is not read.

## Merc quote (`ja1_merc_quote`)

The one queued merc quote (quote_slot, quote_event, quote_tile, quote_delay, quote_focus).

1. If slot ≠ quote_slot or slot ≥ 8: done.
2. If s8 quote_delay > 0: quote_delay = quote_delay − 1; done.
3. e = quote_event. If e < 0x21: host op 54 merc_message (slot, e, s16 quote_tile, 0, 1); if e = 5: Forget the
   attack (1).
   If e is 0xF9..0xFD: host op 100 cry (+0x179, −1, e − 0xF9, 1, 1) (0..4 = "BAD", "GOOD", "EMPTY", "NOTHING",
   "HMM"); then if i32 +0xC0 ≤ 0 and +0x16C = 0: if quote_focus ≠ 0, host op 99 focus (slot, 1); host op 101
   voice (0x2B11, 0x58, 1); u32 +0xC0 = 400.
   Any other e: nothing.
4. quote_focus = 0; quote_slot = 0x40; quote_delay = 0; quote_tile = NONE.

## Frame checks (`ja1_frame_checks`)

1. If s8 +0x18A ≤ 0 and 4 ≤ code < 0x28 and +0x186 = 0: host op 55 ui_refresh (path_preview ≠ 0); host op 50
   end_task (slot, 1); op 52 set_anim (slot, 0x2B, 0, 0).
2. If u32 +0xB4 = 0 and opt_reload_msg ≠ 0 and byte +0x01 = 0 and byte +0x00 ≠ 0 and +0x1BF = 0: then if host
   op 102 reload_check (slot) (i32; the Reload procedure of combat.md, which loads a pocket clip) ≠ 0 and +0x1CD = 0: host op 103 quote_event (slot, 5, 1, NONE, 1); +0x1CD =
   1; u32 +0xB4 = 0xFFFFFFFF. (Op 102 is asked only when the tests before it hold.)
3. Merc quote.

## Enemies see (`ja1_enemies_see`)

1. interrupted = 1.
2. For k = 0x28..0x2F: if record k has +0x182 ≠ 0, +0x138 ≠ 0 and s8 +0x183 ≥ 15: if the low byte of host op 115
   sight_check (k, slot) ≠ 0: host op 116 ask_interrupt (k, slot).

## Enemy step seen (`ja1_enemy_step_seen`, op 111 enemy_step)

After an enemy's step: the mercs and guards that see the enemy now.

1. If s8 +0x9C > 0: +0x9C = 0.
2. interrupted = 1.
3. For k = 0..15: if record k has +0x182 ≠ 0, +0x138 ≠ 0 and s8 +0x183 ≥ 15: if the low byte of host op 115
   sight_check (k, slot) ≠ 0: host op 34 share_sightings (k, 0), then host op 128 ask_merc_interrupt (k, slot).
4. If s8 +0x9C < 1 and spotted = slot (the whole int32): spotted = 0x40.

## Step noise (`ja1_step_noise`, op 117 step_noise)

After each step of the soldier in turn: how loud the step is. All values are ints; "(8-bit)" wraps to 0..255.

1. q = s8 +0x192 × 5 + cdiv(s8 +0x18C, 2).
2. h = ((+0x184 − +0x183 − +0x185) (8-bit) / 2 + s8 +0x183) (8-bit). If h < 50: q = (q (8-bit)) − ((50 − h) ×
   (q (8-bit))) / 100.
3. e = s8 +0x18A. If e < 50: q = (q (8-bit)) − ((50 − e) × (q (8-bit))) / 100.
4. q = q (8-bit). If In water: q = (q − 10) (8-bit). If (slot < 8 and +0x19C = 3) or (slot ≥ 8 and +0x1D3 = 5):
   q = (q − 10) (8-bit).
5. Walking (+0x1A8 = 0): n = 6 − q / 20, plus 1 if In water; loud = (random(n (8-bit)) + 1) (8-bit).
6. Sneaking (+0x1A8 ≠ 0): r = random(100) (8-bit); if r ≥ q: r = random(100) (8-bit) (a second roll). If r < q:
   loud = 0. Else loud = ((r − q + 1) / 20 + 1) (8-bit), and if slot < 8 the merc hears its own step:
   - In water: fx = 0x1F if random(2) = 0, else 0x1E; text 0 (1 if loud ≥ 3).
   - Else if +0x1B3 = 3: fx = 0x2A; text 2 (3 if loud ≥ 3).
   - Else: fx = 0x1C if random(2) ≠ 0, else 0x2B; text 4 (5 if loud ≥ 3).
   - text_buf = the text, 0-terminated. Texts 0..5: "splash", "SPLASH!", "clunk", "CLUNK!", "snap", "SNAP!".
   - Host op 60 sfx (fx, 0x2B11, v, 1), v = (loud × 15 + 0x23) (8-bit) as a signed byte (−128..127).
   - If opt_step_text ≠ 0: host op 129 step_text (text number).
7. If loud ≠ 0: host op 127 make_noise (slot, s16 +0x114, +0x1B3, loud, 3 if In water else 1).

## Consume trigger (`ja1_consume_trigger`, op 106 consume_trigger)

A fired sector trigger is used up, and so is every trigger next to it, and every trigger next to those. row =
sectors row cur_sector (nothing happens if cur_sector ≥ 60); trigger i has its tile at u16 row[0x58 + 4 i].

1. t = the tile of trigger index; the tile of trigger index = NONE (used).
2. For each d of −56, +1, +56, −1 (in this order): v = (t + d) mod 65536; for i = 0..9 (reading the tile again
   each time): if the tile of trigger i = v: Consume trigger (i), then go on with the next i.

The sums have no map check. A used trigger has tile NONE and an empty one 0xC40, and these are neighbours: if
the chain ever reaches tile 0xC09, 0xC40, 0xC42 or 0xC79, the original never ends (it crashes when its stack is
full). No sector of the game has a trigger that leads there (no trigger tile is 0xC09 = 3081). The library stops
the chain after 40 nested levels; what happens then is not part of the observable behaviour.

## Throw follow-up (`ja1_throw_follow`, op 122 throw_follow)

After the attack start of the soldier in turn (Active frame B.3): a thrown item flies. kind(ty) = item_types[12
× ty + 2], range(ty) = item_types[12 × ty + 0xB] (item types below 136 only; the game has no others in a hand).
D = World distance and vt = the tile under a pixel (ai.md); Throw chance as in ai.md "Attack options".

1. t = s16 +0xDC. If t = NONE or kind(byte +0x00) ≠ 0x95: nothing.
2. Throw aim (t).
3. +0x1A2 = low byte of host op 6 direction (s16 +0x116 + 2, s16 +0x118 + 4, s16 +0xDE, s16 +0xE0).
4. Op 52 set_anim (slot, 0x27, 0, 1).

### Throw aim (`ja1_throw_aim`)

Tile t (signed 16-bit).

1. Only if combat ≠ 0: (a) if slot ≥ 8 and s8 +0x9C > 0: host op 99 focus (slot, 0); (b) effect_tile = t. (Out
   of combat effect_tile stays.)
2. ty = byte +0x00. If kind(ty) ≠ 0x95: stop here.
3. a = (low byte of op 17 action_ap (slot, t, 1) + +0x1FD) (8-bit). Host op 31 spend_ap (slot, a, 0).
4. u16 +0xDE = px(t), u16 +0xE0 = py(t), u16 +0xE2 = 0.
5. d = s16(D(s16 +0x116, s16 +0x118, s16 +0xDE, s16 +0xE0)).
6. ch = low byte of Throw chance (t, aim clicks +0x1FD). r = (random(100) + 1) (8-bit). If s8 r > s8 ch (a
   miss): +0x1BC = 0; Throw scatter (margin (r − ch) (8-bit), distance d). Else (a hit): +0x1BC = 1; u16 +0xDC = t.
7. reach = range(ty) × 15 + s8 +0x183 − 50; reach = reach − cdiv((100 − s8 +0x18A) × reach, 200); if reach < 0:
   reach = 0.
8. If reach < d (the throw falls short): dx = s16(u16 +0xDE − u16 +0x116), dy = s16(u16 +0xE0 − u16 +0x118);
   u16 +0xDE = cdiv(dx × reach, d) + s16 +0x116; u16 +0xE0 = cdiv(dy × reach, d) + s16 +0x118; u16 +0xDC =
   vt(s16 +0xDE, s16 +0xE0).
9. u16 +0xD0 = t.

### Throw scatter (`ja1_throw_scatter`)

A missed throw lands off the aimed point. margin = a byte (0..255), dist = a signed 16-bit value. f = +0x1A1 (the
facing). The x axis moves for f = 1, 2, 4, 5, 6, 8; the y axis for f = 2, 3, 4, 6, 7, 8 (any other f: neither).

1. q1 = cdiv(dist, 30); if s16(q1) < 1: q1 = 1. q2 = margin / 15; if q2 < 1: q2 = 1. size = q1 × q2.
2. Both axes: x = y = cdiv(s16(size), 2). Else: x = size if the x axis moves, else 0; y = size if the y axis
   moves, else 0.
3. If the x axis moves and s16(x) < 4: x = 4. If the y axis moves and s16(y) < 4: y = 4.
4. Signs. Both axes: r = random(2); for f = 2 or 6: if r ≠ 0: x = −x and y = −y; for f = 4 or 8: if r ≠ 0: x =
   −x, else y = −y. One axis or none: if the x axis moves and random(2) ≠ 0: x = −x; then if the y axis moves and
   random(2) ≠ 0: y = −y.
5. u16 +0xDE = u16 +0xDE + x, u16 +0xE0 = u16 +0xE0 + y (16-bit sums); u16 +0xDC = vt(s16 +0xDE, s16 +0xE0).

## Move to the patrol tile

Part of Soldier frame (MoveMercTo with no marker, no sneak, no confirm, no squad). t = the tile (signed). Result 1
= walking, 0 = not.

1. 0 if slot ≥ 0x28, or +0x182 = 0, or s8 +0x183 < 15, or +0x17E ≠ 0, or cur > 0x3C.
2. Not done with the target (+0x19C = 3 relentless; DAY quote 8 "I'm not done yet!" / "I ain't finished with him!"): if +0x19C = 3 and s8 +0x1D5 > 0 and d = s16 +0xD0 ≠ NONE: o = byte 5 of d & 0x7F; if o < 0x30 and s8
   (record o +0x183) > 0 and the hand gun state ≠ 0 (type +0x00 in 1..0x10) and byte +0x01 ≠ 0: if +0x00 < 0xF, or
   (host op 7 tile_dist (s16 +0x114, d) (i32) = 1 and column(+0x114) = column(d) and row(+0x114) = row(d)): host op
   54 merc_message (slot, 8, 0x40, 0, 1); result 0.
3. If tile_standable(t) ≤ 0 and s8 +0xBA ≠ 2: if s16 +0x114 ≠ t and t ≠ NONE, host op 125 message (1 = "Can't move
   there!"); result 0.
4. u16 +0xD4 = 0. Host op 40 path_ap (slot, t) (the host prices the walk and stores the path and walk_ap).
5. Enough AP: no if (selected = slot) and combat ≠ 0 and s8 +0x1E2 + s8 +0x1A5 < low byte of walk_ap; then if slot
   < 8 host op 57 merc_text (slot, 1, 0x4B0), else +0x1B0 = 1; then u16 +0x120 = t; +0x177 = +0x196 = +0x1A8 = 0;
   host op 126 move_sound (slot, 1); result 0.
6. +0x197 = 0; host op 72 stop_walk (slot, 1).
7. If can act(cur) ≠ 0: u16 +0x126 = u16 +0x116, u16 +0x128 = u16 +0x118.
8. +0x177 = 0, +0x1A8 = 0, +0x196 = 0, +0x1FA = 1. Host op 29 walk_to (slot, t). Host op 126 move_sound (slot, 1).
   u32 +0xB0 = 0. Result 1.

## Soldier frame (`ja1_soldier_frame`)

The update of a soldier that is not the one in turn.

1. a = code (low byte, read once here).
2. If +0x1B8 = 0 and s8 +0x183 ≥ 15: at the step point: host op 53 stop_anim (slot, stored-target flag); +0x1B8 =
   1; op 84 anim_step (slot). Not at the step point: Walk step.
3. If +0x9D ≠ 0: host op 62 reveal_view (slot, +0x1A1, +0x19E, 0).
4. If still(a) = 0 and +0xBA ≠ 2: done (step 7 does not run either).
5. Op 97 anim_next (slot). u32 +0xB0 = s16 +0xD2 (sign-extended). Frame checks.
6. If s8 +0x183 ≥ 15: Stand up; then if +0xBA = 2 and a ≥ 4 and can act(a) ≠ 0: p = u16 +0x12E; if u16 +0x114 ≠
   p and p ≠ u16 +0x124: +0x1B8 = 0, Move to the patrol tile (s16 p). Else if At the waypoint (s) and a ≠ 0x11:
   op 52 set_anim (slot, 0x11, 1, 0); +0x1A2 = +0x1A4.
   If s8 +0x183 < 15: u16 +0x120 = NONE.
7. If +0x182 ≠ 0 and +0x138 ≠ 0 and +0x188 = 0: Occupy(s16 +0x114, slot).

## Step start

Part of Active frame. Result 1 = the soldier starts the step to the next tile.

1. d = +0x1A2; if +0x196 ≠ 0: d = d + 4 (8-bit), and d − 8 if d > 8. me = s16 +0x114. nt = step(me, dirStep(d)).
   t0 = byte 0 of nt, t1 = byte 1 of nt, cls = t0_class[t0]. (cur is read at the start, as c0.)
2. If +0x196 ≠ 0 and water tile (t0, t1): +0x196 = 0.
3. ap = Step AP of nt if nt ≠ me, else 0 (an unsigned byte). have = +0x1A5; if slot < 8 and ap_warned[slot] = 0
   and +0x16B = 1: have = have − +0xEF (8-bit).
4. If combat ≠ 0 and ap > have (unsigned): host op 72 stop_walk (slot, 0); +0x197 = 1. If ap > s8 +0x1A5: host op
   53 stop_anim (slot, 1 if ap ≥ 100 and nt = s16 +0x124, else 0); result 0. Else: low-AP stop with +0x16B;
   result 0.
5. go = 0. st = tile_standable(nt). If st ≥ 1 or +0xBA = 2: go = 1. Else if st < 0 and +0xB9 ≠ 0: host op 107
   break_object (nt, t1, 1 if seen else 0); for k = 0..7: k = slot: if +0x138 ≠ 0, +0x9D = 1; k ≠ slot: if record
   k +0x138 ≠ 0, record k +0x9D = 1. u16 +0x106 = nt; go = 1. Else: op 98 walk_blocked (slot, nt, byte 5 of nt &
   0x7F).
6. Mine: g = byte 6 of nt. If slot < 8 and g < 0xFF and byte 0 of ground entry g is 0x13 or 0x2C and s8 +0x192 ≥
   |s8 byte 2 of ground entry g|: go = 0; Fire trigger (0xFF, keep 1); host op 118 mine_flag (slot, nt).
7. Forgetful merc: if +0x19C = 2 (personality: forgets where it was going) and slot < 8 and +0x1AF > (5 if combat ≠ 0, else 15) and random(15) = 0: quote_slot =
   slot, quote_event = 0xD, quote_delay = 2; go = 0; host op 53 stop_anim (slot, 1); if combat ≠ 0 and +0x1A5 ≠ 0:
   +0x1A5 = +0x1A5 − (random(s8 +0x1A5) + 1) (8-bit). (random(15) is drawn only when the tests before it hold.)
8. If go = 0: result 0.
9. u16 +0x126 = px(nt), u16 +0x128 = py(nt). wading = 0 < t1 < 0x21.
10. If the soldier is not In water and the current word (read now) < 0x32: if cls ≥ 8 and not wading: op 52
    set_anim (slot, 0x32, 1, 1). Result 1.
11. Else, with c = s16 c0: cls < 8 or wading → n = 0x31; t0 ≥ 0x9A → n = 0x3A if c < 0x39; 0x97 ≤ t0 < 0x9A → n =
    0x37 if c < 0x36 or c ≥ 0x39; else n = 0x34 if c > 0x34, 0x32 if c < 0x32. If n is set: op 52 set_anim (slot,
    n, 1, 1). Result 1.

## Drown

Part of Active frame (a soldier with energy 0 in water).

1. If cur < 0x39: done.
2. If seen or follow_all ≠ 0: host op 99 focus (slot, 0).
3. If +0x1BA = 0: host op 54 merc_message (slot, 0xA, 0x40, 0, 1); +0x1BA = 1; host op 105 stat_practice (slot,
   4, 5, 0). Done.
4. d = random(10) + 2 (8-bit); d = d + random(10) (8-bit); if +0x19C = 5: d = d × 2 (8-bit). Host op 110
   take_damage (slot, 0x1356, d, 2, 0x40, NONE). If +0x183 = 0 now: host op 78 kill (slot, 0).

## Active frame (`ja1_active_frame`)

The update of the soldier in turn (or a merc out of combat). c0 = cur at the start (whole word). ended = 0,
step = 0.

**A. Walking** (+0x1B8 = 0 and s8 +0x183 ≥ 15). Not at the step point: Walk step; go to C. At the step point:

1. Door: if s16 +0x106 ≠ NONE and u16 +0x114 ≠ u16 +0x106: host op 104 close_door (s16 +0x106, byte 1 of that
   tile, 1 if seen else 0); u16 +0x106 = NONE.
2. Sneak practice: if slot < 8 and +0x1A8 ≠ 0 and s8 +0x1D5 > 0: for j = 0..7: if +0x13A + j = 1 and record
   0x28 + j byte +0x13A + slot ≠ 1: host op 105 stat_practice (slot, 2, 1, 0).
3. me = s16 +0x114. If +0x193 ≠ 0: +0x193 = 0.
   a. If u16 +0xF4 ≠ u16 +0x114: m = move_codes[me]; host op 31 spend_ap (slot, Step AP of me, step energy of m).
   b. u16 +0xF4 = u16 +0x114; +0x17D = 0; Tile triggers; step = 1.
   c. g = ground index of me. If g < 0xFF: k = byte 0 of ground entry g. If slot < 0x28 or k = 0x70: host op 108
      trigger_item (slot, me, g, 0). Else if k = 0x11 and s8 byte 2 of the entry ≥ 10: host op 90 explode (slot,
      me, 2, 1); op 109 take_ground_item (me, 0).
   d. If +0x18A = 0 and In water and +0x183 ≠ 0: Drown; u16 +0x124 = u16 +0x114; Walk end (code now).
4. If slot ≥ 0x28: host op 63 enemy_turned (slot); op 111 enemy_step (slot); if seen: host op 99 focus (slot,
   1), and if opt_onscreen ≠ 0, host op 112 tac_redraw (1). Then if interrupt_go ≠ 0: host op 113 merc_interrupt;
   ended = 1. Else if turn_moves ≠ 0: host op 114 next_turn (0); ended = 1.
   Else (slot < 0x28): if slot < 0x10: host op 64 guard_turned (slot); Enemies see; host op 34 share_sightings
   (slot, 0); host op 62 reveal_view (slot, +0x1A1, +0x19E, 0). Then if turn_moves ≠ 0: ended = 1; Turn ends
   (with the target test).
5. If step = 1: op 117 step_noise (slot).
6. If combat ≠ 0 and selected ≠ slot and u16 +0x114 ≠ u16 +0x124: host op 53 stop_anim (slot, 0).
7. If s16 +0xEC ≠ NONE: host op 118 mine_flag (slot, s16 +0xEC); u16 +0xEC = NONE.
8. If slot = selected: op 119 ai_think (slot).
9. If slot < 8 and In water and +0x1B2 ≥ 0x9A: Soak items.
10. If u16 +0x114 = u16 +0x124: Walk end (low byte of c0). Then if (combat = 0 or selected = slot) and +0x1A6 ≠ 0
    and +0x1B0 = 0: act = 0 if +0x1A6 ≥ 0xFE; else +0x1A6 if byte +0x00 = +0x1A6; else none.
    - act none, or In water: Clear the task (+0x95 / +0x96 too).
    - Else: target = s16 +0x134. warn = no. If slot < 8: c = low byte of host op 121 use_cost (slot, me, target,
      byte 6 of tile target, 0); have = +0x1A5, minus +0xEF (8-bit) if ap_warned[slot] = 0 and +0x16B ≠ 0; warn =
      combat ≠ 0 and c > have (unsigned) and +0x16B ≠ 0. If warn: low-AP stop (stop_timer always). Else: host op
      30 click (slot, target, act).
    Else if s8 +0x18A > 0 or In water: if ended = 0: d = Next path direction; +0x1A2 = d; if +0x196 ≠ 0: d = d + 4
    (8-bit), +0x1A2 = d − 8 if d > 8, else d. If Step start = 1: u16 +0xD4 = u16 +0xD4 − 1; +0x1AF = +0x1AF + 1
    (8-bit); Walk step. Else if u16 +0x124 = u16 +0x114: Walk end (code now).
    Else if s16 c0 ≥ 4: tile point stop (1); host op 55 ui_refresh (path_preview ≠ 0); op 52 set_anim (slot,
    0x2B, 0, 0).
11. Exhausted fall.

**B. Still** (+0x1B8 ≠ 0 or s8 +0x183 < 15):

1. If still(cur) ≠ 0: op 97 anim_next (slot).
2. If selected = slot: op 119 ai_think (slot).
3. If s8 +0x183 ≥ 15 or In water: if not In water and byte +0x00 > 0x10 and +0x1BE = 1: host op 79 start_attack
   (slot, s16 +0xDC); op 122 throw_follow (slot). Then if s16 +0x12A ≠ NONE: op 84 anim_step (slot).
   Else: Exhausted fall.

**C. Every case:**

1. If slot < 8 and +0x9D ≠ 0: host op 64 guard_turned (slot); Enemies see; host op 34 share_sightings (slot, 0);
   if turn_moves ≠ 0: Turn ends (without the target test); host op 62 reveal_view (slot, +0x1A1, +0x19E, 0).
2. Frame checks.
3. If s8 +0x183 ≥ 15: Stand up; then if +0x19C = 4 and alone_warned = 0 and host op 124 nearest_merc (slot) (i32)
   > 5: host op 54 merc_message (slot, 4, 0x40, 0, 1); alone_warned = 1. (Op 124 is asked only when the two tests
   before it hold.) If s8 +0x183 < 15: u16 +0x120 = NONE.
4. Timer: if slot ≥ 0x28 and s8 +0x9C = −1: u32 +0xB0 = 0. Else v = s16 +0xD2; u32 +0xB0 = v (sign-extended),
   except cdiv(v, 2) when (+0xBA = 1 and combat ≠ 0) or (opt_half_delay ≠ 0 and still(cur) = 0).
5. If +0x182 ≠ 0 and +0x138 ≠ 0 and +0x188 = 0: Occupy(s16 +0x114, slot).
6. If next_selected ≠ 0x40: selected = next_selected; host op 123 turn_banner; next_selected = 0x40.

**Turn ends** (with or without the target test): if selected < 8: kept_merc = selected. Host op 53 stop_anim
(slot, 0): without the target test always; with it only if u16 +0x114 ≠ u16 +0x124. +0x197 = 1. Host op 114
next_turn (0).

## Path ops (ops 1, 29, 40, 41)

The four ops the AI and the soldier update use to plan and start a walk. The library runs them itself unless
their host_ops bit is set.

```c
int32_t ja1_find_path_op(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);   /* op 41 */
int32_t ja1_path_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile,
                    int32_t sneak, int32_t byte177, int32_t alt);       /* op 40 = sneak, byte177, alt all 0 */
void    ja1_walk_to(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);                      /* op 29 */
void    ja1_idle(Ja1World *w, const Ja1Host *h, int32_t slot);                                       /* op 1 */
```

Tile arguments are signed 16-bit values (the low 16 bits of the int).

### Find path (`ja1_find_path_op`, op 41 find_path)

1. The path search of combat.md "Finding the path" from the start s16 +0x114 to the destination `tile`, with
   the move code of a tile t = move_codes[t], its ground value = byte 0 of tile t, "occupied" = (byte 5 of tile t
   & 0x7F) < 0x30, the record bytes +0x177 and +0xB9 of s, the slot, and "combat" = `combat` ≠ 0.
   A destination off the map (outside 0..3135) gives "no path".
2. No path: result 0; nothing is written.
3. Found: if the low 16 bits of `flag` are ≠ 0: n = min(80, number of steps); paths[80 × slot + k] = direction k
   (0..7) for k = 0..n − 1 (the bytes after them stay); u16 +0xD4 = n; +0x1AF = 0. Result 1 (also with flag 0,
   which writes nothing).

### Path AP (`ja1_path_ap`, op 40 path_ap)

The AP price of a walk to `tile`. Pricing a walk always stores its path (Find path with flag 1).

1. walk_ap = 0; c = 0; up = 0.
2. If Find path (slot, tile, 1) ≠ 0: n = s16 +0xD4 (read after the search); t = s16 +0x114; for k = 0..n − 1:
   t = step(t, dirStep(paths[80 × slot + k] + 1)) (ai.md "Step from a tile"); c = c + "AP to enter a tile"
   (combat.md) for move code move_codes[t] with sneak = low byte of `sneak`, crouch flag = low byte of `byte177`,
   Alt = low byte of `alt` (a byte ≠ 0 = set; the signed 8-bit result of that rule is added); after the first
   step (k = 0): walk_ap = c. Then up = the getting-up cost of the current animation word cur (the whole u16):
   0 or 3 → 7; 1 or 2 → 5; 0xE or 0xF → 3; else 0.
3. walk_ap = walk_ap + up. Result = c + up (32-bit).

(Path AP = select.md "Path AP with marks" with mark = extra = 0; the selection's path preview uses the two.)

So walk_ap is the AP of the first step plus getting up (0 when there is no path); the AP check of a walk reads
its low byte.

### Walk to (`ja1_walk_to`, op 29 walk_to)

1. u16 +0x12A = tile; u16 +0x120 = tile; u16 +0xD0 = NONE.
2. If (turns(cur) ≠ 0 or s8 +0x1D5 > 0) and +0x1FE ≠ 0: op 1 idle (slot). Else: op 84 anim_step (slot) (ai.md
   "Next animation": it starts the walk).

### Idle (`ja1_idle`, op 1 idle)

If +0x1FE ≠ 0: Forget the attack (1).

## Move order (`ja1_move_order`)

The order "walk to this tile" for one soldier (the player's click on a tile, also the squad order). The general
form of "Move to the patrol tile" above (which is this order with marker, sneak, byte177, alt and squad all 0).

```c
int32_t ja1_move_order(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t marker,
                       int32_t sneak, int32_t byte177, int32_t alt, int32_t squad);
        /* tile: signed 16-bit; marker, sneak, byte177, alt, squad: low byte (≠ 0 = set). Result 1 = walking */
```

The original game has an option that makes a merc's order wait for a second click on the same tile (a
confirm). That wait is the front-end's: it asks before it calls the move order. The library has no confirm.

New world fields (port_api.md): `key_shift` (u8, Shift is held: sneak), `key_alt` (u8, Alt is held),
`opt_move_fx` (u8, an option: a merc's order with a marker also makes host op 65). New host op 130
tile_marker (byte kind, tile): the front-end shows marker `kind` on the tile (the order uses kind 0x8A).

1. Steps 1, 2 and 3 of "Move to the patrol tile" (refused, not done with the target, not standable), with t = tile.
2. Squad (squad ≠ 0): for i = 0..7 with i ≠ slot: Move order (i, tile, marker 1, sneak = key_shift, byte177 0,
   alt = key_alt, squad 0) (each result is not used).
3. u16 +0xD4 = 0. Path AP (slot, tile, sneak, byte177, alt) (the library procedure, never a host call here;
   its result is not used: the next step reads walk_ap).
4. Not enough AP: if (selected = slot) and combat ≠ 0 and s8 +0x1E2 + s8 +0x1A5 < the low byte of walk_ap
   (unsigned): if slot < 8 host op 57 merc_text (slot, 1, 0x4B0), else +0x1B0 = 1; then u16 +0x120 = tile;
   +0x177 = byte177, +0x196 = alt, +0x1A8 = sneak (low bytes); host op 126 move_sound (slot, 1); result 0.
5. +0x197 = 0; host op 72 stop_walk (slot, 1).
6. If marker ≠ 0: if opt_move_fx ≠ 0, host op 65 effect_slot (slot, 1); then host op 130 tile_marker (0x8A,
   tile).
7. If can act(cur) ≠ 0: u16 +0x126 = u16 +0x116, u16 +0x128 = u16 +0x118.
8. +0x177 = byte177, +0x1A8 = sneak, +0x196 = alt (low bytes), +0x1FA = 1. Op 29 walk_to (slot, tile). Host op
   126 move_sound (slot, 1). u32 +0xB0 = 0. Result 1.

## Small ops (6 direction, 7 tile_dist, 53 stop_anim)

Three more ops the walk needs. The library runs them itself unless their host_ops bit is set.

```c
int32_t ja1_direction(int32_t x1, int32_t y1, int32_t x2, int32_t y2);   /* op 6; no world */
int32_t ja1_tile_dist(int32_t a, int32_t b);                             /* op 7; no world */
void    ja1_stop_anim(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);   /* op 53; flag: low byte */
```

New host op 139 step_sound_stop (byte slot, int 1): the soldier's step sound stops.

### Direction (op 6 direction)

The direction 1..8 (N, NE, E, SE, S, SW, W, NW; screen y grows to the south) from pixel point (x1, y1) toward
(x2, y2).

1. dx = x2 − x1, dy = y2 − y1 (32-bit). In double precision: fx = dx, or 2⁻⁸ (0.00390625) when dx = 0.
2. a = atan2(1.2 × dy, fx) (the C function: the angle of the point (fx, 1.2 dy), −π..π).
3. v = (a + 0.39269908175) × 1.2732395445689122; i = v truncated toward zero.
4. T = 3, 4, 5, 6, 7, 8, 1, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8 (T[0] first). Result = T[i] if
   v > 0, T[i + 7] if v < 0, T[0] if v = 0. (The same point gives 3.)

### Aim direction (`ja1_aim_dir`; was host op 189 aim_dir)

The same angle as Direction, cut into 24 parts instead of 8: the direction 1..8 and a turn word for soldier +0x10A.

1. dx, dy, fx and a as in Direction steps 1 and 2.
2. v = (a + 0.1308996939166667) × 3.819718633706735 (that is (a + π/24) × 12/π); i = v truncated toward zero.
3. k = i if v > 0, i + 23 if v < 0, 0 if v = 0.
4. D = 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 1, 1, 1, 2, 2, 2, 3 (D[0] first). Result: direction
   D[k] and word W[k], with W[k] = 0, 1, 0xFFFF for k mod 3 = 0, 1, 2. (The same point gives (3, 0).)

### Tile distance (op 7 tile_dist)

max(|row(a) − row(b)|, |column(a) − column(b)|) with row and column as in ai.md "Tile parts" (C division, so
negative tiles give negative parts). 32-bit result.

### Stop animation (`ja1_stop_anim`, op 53 stop_anim)

The walk of the soldier ends where it is.

1. u16 +0x12A = NONE; +0x1F6 = 0; u16 +0x124 = u16 +0x114.
2. flag = 0: if +0x1A6 ≠ 0: +0x95 = +0x1A6 and u16 +0x96 = u16 +0x134. Done.
3. flag ≠ 0: u16 +0x120 = NONE; +0x1A8 = 0; +0x196 = 0; host op 139 step_sound_stop (slot, 1). If +0x1D1 ≠ 0 and
   +0x1D6 ≠ 0 and not (+0x1D1 = 0xE and +0x1A6 ≠ 0): if +0x1D1 is 2, 3, 4, 5, 8 or 0xD: +0x1D1 = 0; u16 +0x132
   = NONE; +0x1D6 = 0.

## Merc view (`ja1_merc_view`, op 62 reveal_view)

What a merc sees when it faces a direction (combat.md "A merc's view"; called after a turn and at the start of a
turn): the map tiles it reveals, mines its detector finds, ground items it spots, roofs that open. Arguments: slot,
facing (low byte), range (low byte), think (low byte). Library op 62.

```c
void ja1_merc_view(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t facing, int32_t range, int32_t think);
```

New world fields (port_api.md): `view_rays` (306 bytes, read-only), `obj_block` (685 bytes, read-only),
`story_finder`, `spot_stop` (u8 each). **story_off** = story[0x16] (≠ 0: no story objects to find) and
**story_sector** = story[0x17] (the sector with the story objects, compared as a signed byte) are bytes of the
`story` block, not separate fields: Setup sector writes story[0x17] (sector_start.md step 7). Read too: `tiles`, `ground_items`, `item_types`,
`move_codes`, `t0_class`, `t1_class`, `t2_class`, `dir_steps`, `cur_sector`, `santino_word`, `turn_moves`,
`screen_drawn`. Tile bytes: byte k of tile t = tiles[8 × t + k] (t0 = byte 0, t1 = byte 1 the object, t2 = byte 2,
t3 = byte 3 the roof, byte 4 bit 7 = revealed, byte 6 = ground index).

**Five directions** D(f) for the facing f (by f; any other value: 0, 0, 0, 0, 0): 1 → 8, 2, 1, 7, 3; 2 → 1, 3, 2,
8, 4; 3 → 2, 4, 3, 1, 5; 4 → 3, 5, 4, 2, 6; 5 → 4, 6, 5, 3, 7; 6 → 5, 7, 6, 4, 8; 7 → 6, 8, 7, 5, 1; 8 → 7, 1,
8, 6, 2. step[i] = dirStep(D(f)[i]) for i = 0..4 (dirStep of 0 is dir_steps[0]); step[5] = 0.

**Object block** (t, d, side) (d = a direction byte, side = a value or NONE): o = t1 of t. o < 0x27 → 0. o ≥ 0x83
→ 2 if o is 0xE2 or 0xE3, else 0. o ≤ 0x36 → 2. Else row = (o − 0x37) × 9; v = obj_block[row + d]. If side = NONE
or v > 2 → v. m = s8 obj_block[row + 9]; if (m = 1 and side is 3 or 7) or (m = 2 and side is 1 or 5) → 0; else v.

**Merc busy**: 1 if a slot k in 0..7 has +0x182 ≠ 0, +0x138 ≠ 0 and (s8 +0x1D5 ≥ 1 or +0x1C4 ≠ 0); else 0.

**Procedure:**
1. If slot ≥ 8: go to step 4.
2. base = 0x99 bytes into `view_rays` if f is even (f = facing as a byte), else 0. found = 0.
3. For each ray r = 0, 9, 18, ..., 144 (17 rays): budget = 99, after1 = 0, t = s16 +0x114. For k = 0, 1, ..., up
   to range − 1 (range 0: no steps):
   a. a = view_rays[base + r + k]; b = view_rays[base + r + k + 1] if k < 8, else 99. (Mercs have range 7; with
      k ≥ 9 a ray reads on into the next ray's bytes. An index of 306 or more reads as 5: the ray ends. The
      original reads other EXE bytes there, not valid ray steps.)
   b. n = step(t, step[a]). If n = t: end the ray. t = n.
   c. g = byte 6 of t. **Mine search**: if +0x177 ≠ 0 and k ≤ 1 and g < 200 and ground_items[12 g + 6] = 0xFF
      and item_types[12 × ground_items[12 g] + 12] ≠ 0 (that is byte 0 of the next item type's entry; 0 for type
      135 and above): if +0x000 = 0x4D and s8 +0x002 ≥ 10: if random(100) < s8 +0x002 and t1 of t = 0: host op
      118 mine_flag (slot, t); then if ground_items[12 g] = 0x11 and s8 ground_items[12 g + 2] < 0: host op 105
      stat_practice (slot, 6, 2, 0). (Not 0x4D or condition below 10: +0x177 = 0.)
   d. side = NONE if b = 99; t if b = 5; else D(f)[b]. blk = s8 of Object block (t, D(f)[a], side).
   e. If blk = 0xB and k < 2: k = 0 → blk = 0, budget = 2; k = 1 → blk = 1.
   f. If blk < 3:
      - byte 4 of t = byte 4 | 0x80.
      - **Spotted item**: if +0x1D5 = 0 and turn_moves = 0 and screen_drawn ≠ 0 and g < 200 and ground_items[12 g +
        6] = 0: ground_items[12 g + 6] = 1. If found = 0: host op 103 quote_event (slot, 3, 0, t, 1); tile point
        stop (0) (Helpers); +0x1A6 = 0, u16 +0x134 = NONE, +0x1A7 = 0; if +0x16D = 0 and s16 +0x120 ≠ NONE and
        u16 +0x120 ≠ u16 +0x114: spot_stop = 1, +0x16D = 1, host op 120 low_ap_stop (slot, 1, 0); host op 105
        stat_practice (slot, 4, 1, 0); found = 1.
      - **Roofs**: c = t2_class[t3 of t]. If c ≥ 0x12: if t3 = 0x73: t3 of t = 0, and if move_codes[t] = 0x4B1:
        move_codes[t] = the move code of t (combat.md "Movement") with roof layer 0 and previous code 0x4B1.
        Then for c = 0x12 / 0x13 / 0x14 (keep, south, above, own, add) = (0x53, 0x52, 0x54, 0x53, 9) / (0x5E, 0x5D,
        0x5F, 0x5E, 0x14) / (0x69, 0x68, 0x6A, 0x69, 0x1F): if t3 of t − 56 ≤ keep: t3 of t − 56 = 0. If t2 of t
        is 0x49, 0x4A or 0x4C and t3 of t + 56 ≥ south: if t3 of t > above: t3 of t = own if t2 = 0x4C, else t2 +
        add (8-bit). Else (t2 not one of those, or the south roof lower): t3 of t = 0. (Other c: nothing more.)
      - If t1 of t = 0x57 and t3 of t − 56 is 0x53, 0x5E or 0x69: t3 of t − 56 = 0.
      - **Story tiles**: if story_off = 0 and cur_sector = s8 story_sector (the u8 against the signed value): if t1
        of t is 0xAC..0xAF and Merc busy = 0 and s8 +0x183 ≥ 15: story_finder = slot. Else (that first test
        fails): if cur_sector = 0 and santino_word ≠ 0 and t = 0x279: santino_word = 0.
      - budget = budget − 1.
   g. If budget ≤ 0 or after1 = 1: end the ray. If blk > 1: end the ray. If blk = 1: after1 = 1.
4. +0x09D = 0. If think ≠ 0: op 119 ai_think (slot).

Roof tiles outside the map: for t in row 0 the original reads and writes "t − 56" before the tile map (bytes of
soldier record 47 and of the game's soldier pointer list); for t in row 55 it reads "t + 56" after the map. The
port reads such a byte as 0 and skips such a write.

*Evidence (2026-10-21): mirror tools/ai.py reveal_view (ai_diff view, and real inside active_frame / soldier_frame
tests); Merc busy read from the code: 8 records, 4 byte tests).*

## One frame (`ja1_frame`)

The soldier part of one frame of the tactical loop. The game alternates: one frame of updates, then one screen
draw, and so on. A soldier acts in a frame only when its own timer u32 +0xB0 ran out (see Timer tick); the
animation sets that timer again (the frame delay).

```c
int32_t ja1_frame(Ja1World *w, const Ja1Host *h);    /* result: the 8-bit sum of the op 134 new_turn results */
void    ja1_tick(Ja1World *w, const Ja1Host *h);     /* Timer tick: call it every 10 ms of game time */
```

New world fields (port_api.md): `seen_enemies` (u16), `in_flight` (u8), `merc_shown` (u8), `interrupt_turn` (u8),
`fx_anims` (16 × 0x12 bytes, read only here: byte 0 of entry k is its count, signed), `tick_step` (i32). Existing
fields: `selected` (the soldier whose turn it is), `combat`, `kept_merc`, `active_slot`, `anim_signal`,
`stop_timer`, `screen_drawn`.
New host ops: 131 itq_pop () → slot (low byte; 0x40 = none), 132 turn_hand_over (), 133 round_end (), 134
new_turn (int focus) → low byte, 135 portrait (byte slot, int 1), 136 projectile_step (slot). Library ops: 137
active_frame (slot) = Active frame, 138 soldier_frame (slot) = Soldier frame (host_ops bit 63 covers them).

1. in_flight = 0; seen_enemies = 0; sum = 0.
2. For i = 0..0x2F, s = record i:
   a. If +0x182 ≠ 0 and +0x138 ≠ 0 and +0x188 = 0:
      1. If i ≥ 0x28 and s8 +0x9C > 0: seen_enemies = seen_enemies + 1 (16-bit).
      2. in_flight = in_flight + +0x195 (8-bit).
      3. If s32 +0xB0 ≤ 0: if (+0x1B0 = 0 and selected = +0x17A) or (i < 0x10 and combat = 0): op 137
         active_frame (i); else, if s32 +0xB0 ≤ 0 (read again): op 138 soldier_frame (i).
      4. If u32 +0xC4 = 0: +0x1D2 = +0x1D2 − 1 (8-bit); if s8 +0x1D2 is a multiple of 4 (C remainder 0, also for
         negative values): u16 +0xD8 and u16 +0xDA each − 1 (16-bit); then u32 +0xC4 = 10 if s8 +0x1D2 > 0, else
         −1.
      5. If i < 8:
         - If s32 +0xBC ≤ 0 and s8 +0x189 > 0: +0x189 = +0x189 − 1; host op 135 portrait (+0x17A, 1); u32 +0xBC =
           0x96.
         - If s32 +0xA8 ≤ 0: +0x1AC = +0x1AC + 1 (8-bit); r = 0; if s8 +0x1AC > 0x14: +0x1AC = 0. If s8 +0x1AC < 3
           and merc_shown ≠ +0x17A and +0x194 = 0 and +0x1A5 ≠ 0: host op 135 portrait (i, 1); else r =
           random(0x28). u32 +0xA8 = r + 100.
      6. If +0x1BF = 3: host op 136 projectile_step (i).
   b. Hand-over (for every i, also a soldier that step a passes over): if (+0x1B0 ≠ 0 or +0x182 = 0 or +0x138 = 0 or
      +0x188 ≠ 0) and +0x17A = selected:
      - r = low byte of host op 131 itq_pop ().
      - r < 8: host op 132 turn_hand_over (). 8 ≤ r < 0x40: selected = r; host op 123 turn_banner ().
      - r ≥ 0x40: if kept_merc < 8: host op 132 turn_hand_over (); else if selected < 0x2F: if selected ≥ 8:
        selected = selected + 1 and host op 123 turn_banner (); else (selected ≥ 0x2F): if interrupt_turn = 0:
        host op 133 round_end (); else if combat ≠ 0: sum = sum + low byte of host op 134 new_turn (1) (8-bit).
3. If anim_signal ≥ 0 and s8 fx_anims[0x12 × anim_signal] ≤ 0: anim_signal = −1.
4. If active_slot < 0x40 and in_flight = 0 and anim_signal = −1: host op 39 fire (active_slot).
5. Result = sum.

## Update end (`ja1_update_end`)

The rest of the update pass of the tactical loop, after One frame. The front-end's loop alternates an update pass
(One frame, then this, then its own UI blink timer) and a draw pass; Fuses runs after every pass. The result is
added to the loop's leave counter (as One frame's result): a counter ≠ 0 ends the tactical loop.

```c
int32_t ja1_update_end(Ja1World *w, const Ja1Host *h);   /* result 0 or 1 */
void    ja1_fuses(Ja1World *w, const Ja1Host *h);
```

World fields: new `ripple_timer`, `ripple_tile`, `ambient_t1`, `ambient_t2`, `ambient_count`, `opt_ambient`,
`time_compress`, `enemies_warned`, `leave_reason`; existing `cur_sector`, `sector_state`, `splash`, `tiles`,
`soldiers`, `timed_fx`. New host ops 201-207 (port_api.md); others used: 105 stat_practice, 60 sfx.

1. Sector won: row = the 9 bytes of `sector_state` for cur_sector. If bit 0 of row byte 0 is clear:
   - If row byte 1 (enemies left) = 0: host op 201 sector_gained (cur_sector as a signed byte). For each merc
     i = 0..7 with +0x182 ≠ 0, s8 +0x183 ≥ 15 and +0x138 ≠ 0: host op 105 stat_practice (i, 9, 10, 0). Then, if
     cur_sector = 0: host op 202 won_quote (0x1D, 0x63, 1), leave_reason = 1, result = 1.
   Else (bit 0 set): if row byte 1 > 0 and enemies_warned = 0: host op 203 enemies_msg, enemies_warned = 1, host
   op 204 music (2, 0).
2. Water ripples: if s32 ripple_timer ≤ 0: t = random(3136); ripple_tile = t; n = s8 splash[cur_sector]. If tile
   byte 0 of t ≥ 0x97 and tile byte 1 = 0 and (tile byte 5 & 0x7F) ≥ 0x30 (no occupant) and random(n) = 0 (the
   second random number is drawn only when the three tile tests pass): host op 205 tile_anim (t, 0). Then
   ripple_timer = n × 12 (32-bit).
3. Ambient: if s32 ambient_t1 ≤ 0 and opt_ambient ≠ 0:
   - host op 206 pal_cycle (0xAC, 0xB2); ambient_t1 = 200.
   - If s32 ambient_t2 ≤ 0: host op 206 pal_cycle (0xA5, 0xAB); ambient_t2 = 300; ambient_count = ambient_count
     + 1 if it is below 0xFE, else 0xF8. r = random(20); if r = 0 and time_compress = 0: host op 60 sfx (random(6)
     + 0x24, 0x2B11, 0x23, 1) (random(6) only then).
   - host op 207 pal_set (0xA5, 0xB2).

**The front-end's part of the update pass** (not the library): after Update end, its UI blink timer (when ≤ 0:
it toggles a blink flag, restarts at 800 ms when the flag is now on, else 400 ms, and steps a 0..2 colour
counter) and its pass flag (the next pass is a draw pass). Tile animation kind 0 (host op 205) = the frame table
of the water ripple with count 0x12 (frames[0..17] = 0, then 0xC0, 0xC1, … 0xD0): the front-end takes the first free tile animation of 1..9 (animation 0 is
never used; free = count 0), sets its count to 0x11, its frames and tile (not its timer), and the tile's byte 3 to
the animation's number; no free animation: nothing.

## Fuses (`ja1_fuses`)

After every pass (update and draw). For each of the 5 `timed_fx` records f (13 bytes):

1. If i32 f+0 = 0 exactly (its fuse ran out, see Timer tick) and s8 f+9 ≤ 0: f+0 = −1 (free), tile = s16 f+6,
   k = f+8.
   - If tile < 0xC41: g = the tile's ground item index (as in turns.md End turn step 1); if (g is an item and its
     blast kind is k) or k = 0xA: host op 90 explode (f+0xB, tile, k, 1).
   - Else: library op 150 ground_blast (sector_start.md Ground blast) (k, s8 f+0xA, s16 f+4, f+0xB, s8 f+0xC).
   (Unlike End turn: f+0 must be exactly 0, and kind 0xA goes off on any map tile.)

## Loop head (`ja1_loop_head`)

The start of every pass of the tactical loop, before the front-end reads the keys and the mouse (and before One
frame or the draw). It runs the two turn timers, starts combat when the sector has enemies and mercs, and ends it
when the enemies are gone. The result is added to the loop's leave counter, as One frame's result.

```c
int32_t ja1_loop_head(Ja1World *w, const Ja1Host *h);   /* result: the 8-bit sum of the op 134 new_turn results */
```

World fields: new `turn_tick` (i32, see Timer tick); existing `turn_timer` (s16), `turn_timer2` (s16),
`screen_drawn`, `new_round`, `cur_sector`, `sector_state`, `combat`, `merc_to_show`, `turn_shown`,
`redraw_pending` (i32), `turn_clock` (i32), `busy` (i32), `busy_timer` (i32), `selected`, `turn_based` (i32),
`fast_timer` (i32), `opt_onscreen`, `interrupt_turn`, `view_hold`, `soldiers`. New host op 208 strip (int a, int
b) (port_api.md); others used: 1 idle, 134 new_turn, 147 mouse_save, 195 select_light. First merc is turns.md
First merc (a library procedure, no host call).

sum = 0. "New turn" below = sum = sum + low byte of host op 134 new_turn (0) (8-bit).

1. Turn timers (every 100 ms): if s32 turn_tick < 1:
   - t2 = s16 turn_timer2. If screen_drawn = 0: if t2 < 20: turn_timer2 = t2 + 1.
   - Else (screen_drawn ≠ 0): if t2 ≥ 1: turn_timer2 = t2 − 1. Else (t2 < 1): if s16 turn_timer > 0 and
     new_round = 1: turn_timer = turn_timer − 1.
   - turn_tick = 10.
2. row = the 9 bytes of `sector_state` for cur_sector.
   - If row byte 1 (enemies) ≠ 0:
     - If combat = 0 and row byte 2 (mercs here) ≠ 0 (**combat starts**): combat = 1; host op 147 mouse_save ();
       if merc_to_show ≥ 8 (unsigned): merc_to_show = low byte of First merc (); New turn; host op 208 strip (1
       if opt_onscreen < 3 (unsigned) else 0, 1); turn_shown = 1.
     - Else, if redraw_pending ≠ 0: New turn.
     - Then (both cases) turn_clock = −1.
   - Else, if combat ≠ 0 (**combat ends**): combat = 0; turn_clock = 20000; turn_shown = 0; Combat end (below);
     then if redraw_pending ≠ 0: New turn.
   - Else (no enemies, no combat), if turn_clock = 0: if s32 busy < 1: busy = 0 and New turn (busy ≥ 1:
     nothing).
   - Else (turn_clock ≠ 0), if redraw_pending ≠ 0: New turn.
3. If busy ≠ 0 and busy_timer = 0: busy_timer = 5000.
4. If selected ≥ 8 (unsigned) and turn_based = 0 and s32 fast_timer ≤ 0: fast_timer = 15000.
5. Result = sum.

**Combat end:** for each soldier i = 0..0x27 with +0x182 ≠ 0, +0x138 ≠ 0 and +0x183 ≠ 0 (all bytes here): +0x1A5 = 1,
+0x1B0 = 0, +0x194 = 0, +0x17E = 0, +0x197 = 0, host op 1 idle (i). Then screen_drawn = 1; new_round = 1. If selected ≥ 8
(unsigned): selected = low byte of First merc (). Then (selected read again) if selected < 8: host op 195
select_light (selected, 0xC). interrupt_turn = 0; view_hold = 0; host op 208 strip (1, 1).

**The front-end's part of the loop head** (not the library): before step 2, the front-end runs Merc sighting
(below). Host op 208 strip (a, b) loads the panel strip pictures (which picture a = 0 or 1 selects is
not traced; the loop head passes a = 1 when the on-screen option is below 3). The loop's order of a pass: Loop head, the keys and the mouse, then the update pass (One frame,
Update end) or the draw pass, then Fuses.

## Merc sighting (front-end; host op 184 merc_sees)

A merc that sees or hears an enemy reacts at the start of the next loop pass: the time compression stops, the
merc reports it and the view goes to the enemy. This is front-end work (the library only calls host op 184).

State (front-end, three bytes): `sight_slot`, `sight_how` (signed), `sight_near`. **Sighting reset**: sight_slot
= 0x40, sight_how = 0x7F, sight_near = 0x40, and the world's fx_id = 0xFF. The sector start does a Sighting reset.
Host op 184 merc_sees (slot, how, near): sight_slot = slot, sight_how = how, sight_near = near (low bytes). A
later op 184 before the next pass overwrites the earlier one.

At the loop head (before the library's Loop head), if sight_slot < 0x30 (unsigned):
1. Wait for an impact effect: if s8 fx_id ≥ 0 and s8 byte 0 of fx_anims entry fx_id (entry = 0x12 bytes) > 0:
   stop here (the reaction stays for a later pass). (SPEC GAP (not seen: the game sets fx_id only to 0xFF (sector start, reset), 0 (a hit on a high tile) or an op 178 puff result (1..15, or -1 when all are in use)): fx_id ≥ 16 reads past the 16 entries; the
   front-end treats it as count 0.)
2. R = soldier record sight_slot. g = Hand gun state of R (shooting.md: main-hand item type 1..0x10: 1 if s8 +2
   ≥ 10 else −1; other items 0). If g ≠ 0 and (s8 +1 > 0 or sight_how = 5): Merc message (R, sight_how,
   sight_near, ""). Else the text t is: g = 1 and s8 +1 < 0: "JAMMED"; g = 1 (+1 = 0): "NO AMMO"; g = −1: "ITEM
   BROKEN"; g = 0: "NO WEAPON"; then Merc message (R, sight_how, sight_near, t).
3. Sighting reset.

**Merc message** (R, how, near, text) (also host op 54 merc_message (slot, how, near, 0, 1) with an empty text):
   near is a whole signed 32-bit value, not a byte: for how = 3 it is a tile (Merc quote passes s16 quote_tile;
   asm 0x1B384 `sar ebx, 0x10`, 0x299C4 keeps the dword; the slot test `near < 0x40` is a signed dword compare). A
   port that cuts near to 8 bits jumps the view to rows 0..4 when a merc spots an item (the maintainer play test #20 F; smoke
   found_view).
1. If R +0x17A > 7 (unsigned): nothing.
2. how < 0 (signed): Time compression off (turns.md `ja1_time_compress` (0)). Voice: the merc's "BAD" cry: member
   `BAD.VOC` of `SNDS\<profile>` (profile = R +0x179) becomes the cry sample (sound.md op 100 with cry 0), then the
   cry channel plays it at rate 0x2B11, volume 50, once (as op 101 (0x2B11, 50, 1)). If opt_step_text ≠ 0: a message box "Enemy spotted!" for 700 ms
   (panel.md "Tactical messages"). Then the Focus step.
3. how ≥ 0: if the ear item (R +0x30, item slot 4: the radio) is 0 or its s8 condition (R +0x32) < 10: nothing
   (no focus step either). Else Time compression off; if s8 hp (+0x183) ≥ 10 and (s8 hp ≥ 15 or how = 9): the
   merc's speech balloon with quote `how` (panel.md "Speech balloon"; a right click during its wait sets a "skip"
   byte that cancels the Focus step; the weapon text (JAMMED ...) goes to the balloon's wait, which draws it over
   the face while the mouse is on it: panel.md "Speech balloon" step 7). Then show_timer =
   5000 and shown_slot = R +0x17A; then the Focus step.
4. **Focus step**: if how = 3: show_timer = 5000; view_tile = near; the view jumps to tile near (as host op 99's
   jump, world_load.md "Focus" step 2, but with near as the tile and no soldier tests: only when opt_onscreen ≠ 0
   and the tile's pixel point is outside the view). Else if near < 0x40 and (s8 soldier near +0x9C > 0 or
   follow_all ≠ 0): show_timer = 5000; spotted = near; focus_slot = near; focus_last = near; host op 99 focus
   (near, 0). Else: host op 99 focus (R +0x17A, 0). Then Hide path marks (force = 1 if path_preview ≠ 0, else
   0) (select.md).

The values of how from the library (ai.md, shooting.md): −1 (0xFF) = an enemy seen or heard, 0, 1 = a shooter
it does not see, 2. So op 184 with how ≥ 0 needs the radio; how = −1 does not.

*Evidence (2026-10-25):* MercSees? 0x56D28 (stores 0x229E00..02), loop head 0x1094B (slot < 0x30 → 0x56D3C),
0x56D3C (fx wait 0x229E03 / 0xEAFF0, HandGunState 0x56F20, WeaponStatusText? 0x56F4C, strings 0x99B55..0x99BA3,
reset 0x2F7A4; sector start 0x2F730), ShowMercMessage? 0x299B8 (radio test +0x30 / +0x32, TimeCompress 0x11D40,
PlayQuote 0x24F08 + SpeechBalloon? 0x29544, how < 0: PlaySndsVoc 0x24E44 (profile, −1, "BAD" 0x9063C, 1) + PlayVocSample 0x5C9C0 (0x2B11, 0x32, 1), show_timer 0x93D54, shown_slot 0x229CB4, "Enemy spotted!" 0x9AB13
700 ms under 0x1163BD, skip byte 0x92F65 (SpeechWait? 0x283FB), tile jump 0x3F490, FocusOn? 0x3F724,
PathMarksOff? 0x1D8A4).

## Leave tests (`ja1_leave_tests`)

After Fuses, at the end of every pass of the tactical loop. The result is added to the loop's leave counter (as the
results of Loop head, One frame and Update end); the loop ends when the counter is ≠ 0 after this step, else the
next pass starts.

```c
int32_t ja1_leave_tests(Ja1World *w, const Ja1Host *h);   /* result 0, 1 or 2 */
```

World fields (existing): `sector_state`, `story`, `leave_reason`. Host op used: 125 message (6) (text 6 = "Home
sector LOST!").

1. r = 0. If bit 0 of byte 0 of sector_state entry 59 (the home sector) is clear (the home sector is not the
   player's any more): host op 125 message (6); r = r + 1; leave_reason = 4.
2. If story byte 0x1C ≠ 0 and story byte 0x0E (signed) < 2: r = r + 1.
3. Result = r.

When the counter is ≠ 0 the loop ends: see "Leaving the sector" below.

## Leaving the sector (`ja1_leave_path`, `ja1_leave_sector`)

When the tactical loop's leave counter is ≠ 0 after Leave tests, the loop runs Leave path once and the tactical
screen ends (the strategic screen comes next; that is the front-end's). Leave sector is also library op 214
leave_sector (int flag): Leave path runs it through the op.

```c
void ja1_leave_path(Ja1World *w, const Ja1Host *h);
void ja1_leave_sector(Ja1World *w, const Ja1Host *h, int32_t flag);
```

World fields: `cur_sector`, `sector_state`, `santino_spoke`, `leave_reason`, the soldier records, and the new field
`quit_tactical` (uint8: ≠ 0 when the player quits the tactical screen with no end of day, for example to load a
game; the front-end sets it, the sector setup clears it). New host ops (none returns anything that is read): 214
leave_sector (int flag; library op), 215 fade (int 0, int 0x38, int 0x100, int 0: the screen fades out; the
numbers are the original's fade arguments, the palette is the game palette), 216 music_stop () (the music stops;
the front-end also clears its mouse hover flag and its "next music" number), 217 end_of_day () (the end-of-day
screen), 218 resolve_battle (int sector, int 1) (the strategic layer settles the battle in the sector), 219
save_tmp () (the sector's temporary save), 220 timer_off () (the game timer stops). Existing ops used: 85
cursor_back, 89 replay_event, 201 sector_gained, and the library ops 78 kill and 82 merc_dies.

**Leave path:**

1. Host op 215 fade (0, 0x38, 0x100, 0). Host op 85 cursor_back (). Host op 216 music_stop ().
2. If quit_tactical = 0 and leave_reason = 0:
   a. Op 217 end_of_day () (library op: End of day, below).
   b. c = cur_sector. If byte 2 of sector_state entry c (mercs in the sector) ≠ 0: host op 89 replay_event (s8 c,
      s8 c, 0x59) (both as signed 8-bit values, widened); byte 2 of entry c = 0.
   c. Op 214 leave_sector (1).
   d. If santino_spoke ≠ 0 and leave_reason = 0: leave_reason = 1.
3. Host op 220 timer_off (). cur_sector = 0xFF.

**Leave sector (flag):** hp = byte +0x183 of a record; "in the sector" = +0x182 ≠ 0.

1. For each merc k = 0..7, record r: if the low byte of flag ≠ 0 and +0x17E = 8: +0x17E = 0. Then if +0x182 ≠ 0,
   +0x138 ≠ 0, +0x188 = 0 and hp = 0: op 82 merc_dies (k, 1).
2. enemy_up = some record 0x28..0x2F is in the sector with s8 hp ≥ 15. guard_up = the same for the records
   8..0x27 (the mercs do not count). Both are found once, here.
3. For each record k = 0x28..0x2F in the sector: if 0 < s8 hp < 15 and (enemy_up = no or guard_up = yes): op 78
   kill (k, 0). Then (hp read again) if hp = 0: op 82 merc_dies (k, 0).
4. For each record k = 8..0x27 in the sector: if 0 < s8 hp < 15 and (guard_up = no or enemy_up = yes): op 78 kill
   (k, 0). Then if hp = 0: op 82 merc_dies (k, 0).
5. c = cur_sector, e = sector_state entry c. If byte 1 of e (enemies) ≠ 0: if byte 4 of e ≠ 0, or byte 3 of e ≠ 0,
   or bit 0 of byte 0 of e is set: host op 218 resolve_battle (c, 1). Else (byte 1 = 0), if bit 0 of byte 0 of e
   is clear: host op 201 sector_gained (s8 c).
6. Host op 219 save_tmp ().

So soldiers that are dying when the player leaves die, unless their side still has someone standing and the other
side does not.

*Evidence (2026-10-23): mirrors tools/ai.leave_path (TacticalLoop 0x11358..0x1140F) and ai.leave_sector
(LeaveSectorCleanup? 0x30D6C); ai_diff leave_path 500 cases and leave_sector 2000 cases, 0 mismatches; mirror
mutants 61/64 and 111/113 (the two leave_sector survivors read record 0x30, past the table).*

## Traverse to a neighbour sector (`ja1_traverse_edge`, `ja1_traverse_check`, `ja1_traverse`, `ja1_traverse_out`, `ja1_traverse_in`)

The squad goes from the tactical map into the next sector north, east, south or west, inside the same day (no
end of day, no strategic screen, the day clock goes on). The player presses T or the TRAV button of the tactical
strip and picks a direction in a small popup.

```c
int32_t ja1_traverse_edge(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir);
int32_t ja1_traverse_check(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir, int16_t *out);
int32_t ja1_traverse(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t dir, int16_t *out);
void ja1_traverse_out(Ja1World *w, const Ja1Host *h, int32_t dest);
int32_t ja1_traverse_in(Ja1World *w, const Ja1Host *h, int32_t theme, int32_t playing);
```

dir = the direction code: 10 north, 11 east, 12 south, 13 west. `out` = 3 int16 values: the entry tile in the new
sector, the new sector, the facing (1, 3, 5 or 7; also the entry side's kind below). World fields used:
`cur_sector`, `sector_state`, `selected`, `combat`, `turn_timer`, `opt_onscreen` and the view, `seen_enemies`,
`merc_to_show`, `redraw_pending`, `first_hit`, the soldier records. No new fields. Host ops: 54 merc_message, 57
merc_text (new text 10 "doesn't have enough Action Points to traverse!"), 125 message (new texts 11 "Can't traverse here!" and 12 "Can't traverse during an interrupt!"), 214
leave_sector (library op), 89 replay_event, 215 fade, 7 tile_dist (library op). t(r) = s16 +0x114 of record r (its
tile), hp(r) = s8 +0x183.

**The popup (front-end).** Only when selected < 8, byte 2 (mercs) of sector_state entry cur_sector ≥ 1 and hp
(selected) > 15; else T and the button do nothing. T key: popup corner (x0, y0) = (126, 60). The button: y0 = the
button's y + 10, x0 = the button's x − 10. The popup is element 0x28 of the strip art at (x0, y0) and element 0x2A
at (x0, y0 + 20), with the words NORTH, WEST, EAST, SOUTH at (x0 + dx, y0 + dy + 7), (dx, dy) = (28, 1), (16, 11),
(42, 11), (28, 21), centred, the small font, colour 8 with shadow 0x0E (the word under the mouse: 0xFF with shadow
8). The T key puts the mouse at (x0 + 45, y0 + 35). Keys (upper case): N north, W west, E east, S south, Escape =
none. The mouse over word i, the rectangle (x0 + dx − 10, y0 + 4 + dy) .. (x0 + dx + 10, y0 + 10 + dy), lights it
and the left button picks it (a key pick can be changed by the mouse over another word); the right button =
none. Each poll the pick starts as none: a key sets it and ends the popup, the mouse over a word then sets it (also
after a key, also after Escape), and the left button down ends the popup with the pick of that poll (none when the
mouse is over no word). The popup closes (the screen under it comes back) and, with a pick, Traverse (selected, code) runs: word 0..3
= codes 10, 13, 11, 12.

**Traverse edge (slot, dir):** r = record slot, t = t(r), c = cur_sector (unsigned byte).

1. dir 10: 0 when t > 0x118 or c < 10. dir 11: 0 when t mod 56 < 51 or c mod 10 = 9. dir 12: 0 when t < 0xB28
   or c > 49. dir 13: 0 when t mod 56 > 5 or c mod 10 = 0. Other dir values: no test here.
2. Result = On screen (r) (ai.md: 1 when opt_onscreen = 0 or the soldier is in the view).

So the merc must stand within 5 tiles of the map edge, the island must have a sector on that side, and the merc
must be on the screen.

**Team nearby (slot):** for k = 0..7, record k, with +0x182 ≠ 0, hp(k) > 0, +0x138 ≠ 0 and +0x17A ≠ slot:

1. hp(k) < 15: if hp(k) ≥ 10 and k is not In water (ai.md): host op 54 merc_message (k, 9, 0x40, 0, 1). Then
   host op 54 merc_message (slot, 0x11, 0x40, 0, 1). Result 0.
2. Tile distance (t(k), t(slot)) > 7: host op 54 merc_message (k, 7, 0x40, 0, 1). Result 0.
3. +0x17E of k = 8, or turn_timer > 0: host op 125 message (12). Result 0.

Result 1 when no merc stopped it.

**Traverse check (slot, dir, out):** r = record slot, t = t(r).

1. Team nearby (byte +0x17A of r, the record's own slot number) = 0: result 0.
2. The edge tile, by dir. 10: while t > 55: t − 56; kind 1, steps (a, b) = (−1, +1). 11: while t mod 56 < 55: t + 1;
   kind 3, steps (−56, +56). 12: while t < 0xC08: t + 56; kind 5, steps (−1, +1). 13: while t mod 56 > 0: t − 1;
   kind 7, steps (+56, −56). Other dir values (the popup never passes them): the original uses unset values
   (SPEC GAP (not seen: the popup passes only the listed dir values): the library keeps t, runs no search, adds 0 and gives kind 0).
3. The search along the edge from t: if Tile standable (t) ≠ 0 (ai.md) or the occupant of t (tile byte 5 and 0x7F;
   0x40 when 0x30 or more) < 0x28, t stays. Else ta = tb = t, both sides on; repeat: side a on: ta + a; side a goes
   off when (kind 1 or 3) ta < 0, (kind 5) ta < 0xC08, (kind 7) ta > 0xC08; still on and Tile standable (ta) = 1:
   t = ta, the search ends. Then side b on: tb + b; side b goes off when (kind 1) tb > 0x38, (kind 3 or 5) tb >
   0xC3F, (kind 7) tb < 0; still on and Tile standable (tb) = 1: t = tb, the search ends. Both sides off: result 0.
4. Tile distance (t, t(r)) > 5: result 0.
5. If combat ≠ 0: the first k = 0..7 with +0x182 ≠ 0, +0x138 ≠ 0 and s8 +0x1A5 (AP) < 5: host op 57 merc_text (k,
   10, 2000) ("doesn't have enough Action Points to traverse!"); result −1.
6. out[0] = t + (kind 1: 0xC08, 3: −55, 5: −0xC08, 7: +55) (the same place on the opposite edge of the new map);
   out[1] = c + (kind 1: −10, 3: +1, 5: +10, 7: −1) (16-bit, c = cur_sector); out[2] = kind. Result 1.

**Traverse (slot, dir, out):** Traverse edge (slot, dir) ≠ 0: v = Traverse check (slot, dir, out); v < 0: result
−1; v = 1: result 1 (the front-end then moves the squad, below). Else (the edge test or the check gave 0): host op
125 message (11) "Can't traverse here!"; result 0.

**The move (front-end, with the library steps).** r = the traversing merc; (tile, dest, facing) = out.

1. Traverse out (dest): byte 2 (mercs) of sector_state entry dest (s16) = byte 2 of entry cur_sector; byte 2 of
   entry cur_sector = 0. Op 214 leave_sector (0) (Leaving the sector: the old sector's result and its tmp file).
   Host op 89 replay_event (s8 cur_sector, s8 dest, 0x4A). Host op 215 fade (0, 0x38, 0x100, 0). cur_sector = dest
   (low byte).
2. The front-end: the new sector's map (world_load.md section 4 step 1, also the tileset), then the sector setup
   `ja1_setup_sector (tile, facing)` with the section 7 host (sector_start.md batch 8: the mercs come in at tile,
   facing `facing`). No other part of the sector entry runs: no clock reset, no day-start copies.
3. The front-end: the view jumps to t(r) (world_load.md section 8 "Focus", the jump with flag 0); the mouse goes
   to (159, 99); the target mark goes away; the tactical strips load again, the screen is drawn, and the screen
   fades in.
4. m = Traverse in (theme, playing): theme = the number of the music theme that plays now (0xFFFF when none, as
   after a stop; music.md), playing = 1 when that music still plays. If selected < 8: merc_to_show = selected.
   Music (u16 theme): 1 or 2: if byte 1 (enemies) of sector_state entry cur_sector ≠ 0: m = 2 when seen_enemies =
   0, else 3; enemies = 0: m = 4. 3: seen_enemies = 0: m = 4, else m = 0. 4 or 5: playing = 0: m = 1, else 0. 0 or
   above 5: m = 0. redraw_pending = 1. If s8 first_hit < 2: first_hit = 1 when the enemies byte ≠ 0, else 0.
   Result m.
5. The front-end: m ≠ 0: the music changes to theme m (music.md; the original first waits for the bar check,
   which never waits).

*Evidence (2026-10-26): asm of TraversePrompt 0x1CBA8, CanTraverseEdge 0x30FA0, TeamNearby? 0x311D8, the edge
search 0x310D0, TraverseCheck 0x31344 and the move 0x315DC; validate/diff/trav_diff.py runs the original
functions in Unicorn.*

## Squad lost (front-end, host op 188)

The last merc of the squad in the sector dies (shooting.md "Death", step 7: host op 188 squad_lost). The game
leaves that sector and loads sector 59 again in the same day (the island's fixed corner sector, also when the squad
was lost there). Steps, with s = selected at the call:

1. combat = 0.
2. The music changes to theme 6 (music.md; the original first waits for the bar check, which never waits).
3. Op 214 leave_sector (0) (Leaving the sector: the old sector's result and its tmp file). The screen fades out
   (as host op 215 fade (0, 0x38, 0x100, 0)).
4. cur_sector = 59. The new map (world_load.md section 4 step 1, also the tileset), then `ja1_setup_sector (tile,
   1)` with tile = the entry tile of sector 59 (the entry tile table of world_load.md section 4) and the section 7 host. No
   other part of the sector entry runs: no clock reset, no day-start copies.
5. The view jumps to that entry tile (world_load.md section 8 "Focus", the jump with flag 0). The tactical strips
   and the screen are drawn.
6. If s ≥ 8: s = First merc (turns.md; 0x40 when no merc is on duty). selected = s. The cursor mode = 0 (cursor.md,
   walk), the strips' combat seen byte = 0 (panel.md). The screen fades in.
   The top strip buttons go up, then DONE, TRAV and INV get the shade (0x10398 (1, 1), 0x10454 (0x15, 1)), with any
   mercs byte of sector 59. The shade stays until the next top strip reload (view mode 3 to 2, a sector load, the guard
   screen, the inventory exit when 0x92DDF < 2, a fight). A live check on #45 shows that view modes 1 to 2, 2 to 3, 1 to 0, 0 to 1 and 0 to 3 keep it.

The steps run inside the death: host op 188 returns after step 6, and the code that called the death goes on in
sector 59. So the rest of that pass (the later soldiers' ticks, the effect hits, Update end, Fuses and the Leave
tests) runs on the new sector, and the old sector's tmp file (step 3) holds the old sector as it was at the death.

*Evidence (2026-10-26): asm of SquadLost? 0x3A32C (called by MercDies? 0x3A4A4): 0x92DCB = 0, MusicBarCheck +
SetMusic (6, 1, 0x100), 0x30D6C (0), 0x52908 fade, cur_sector = 0x3B, LoadTerrainTileset, SetupSectorFromXra
(*(0x95CD0 + 4 × 59), 1), 0x3F490 (same tile, 0), 0x1CB78, BlinkUi? (1), panel BlitCopy, FirstMerc? 0x2AC60 when
s ≥ 8, 0x284F11 = s, 0x46A4C (s, 0xC), 0x331A4 (s, 1), 0x92DCC = 1, 0x284F13 = 1, 0x92DCE = 0, 0x10398 (1, 1),
0x10454 (0x15), 0x92DCA = 0, 0x284F1B = 0, MoveCursorState? 0x1C464, 0x1CB78, fade in 0x52908.*

## End of day (`ja1_end_of_day`, library op 217)

The end-of-day screen: the sunset picture with "End of Day N", then (when the squad ends the day away from home
land) the retreat, in which mercs can be hit or die. Leave path step 2a runs it as library op 217 end_of_day ().

```c
void ja1_end_of_day(Ja1World *w, const Ja1Host *h);
```

World fields (existing): `day`, `cur_sector`, `sector_state`, `santino_spoke`, `leave_reason`, `story`, the soldier
records. Op 43 connected_home is a library op (Joined to home, below). New host ops (none returns a value that is read, except 257): 254 eod_begin (int day) (the front-end loads
its main sounds, shows the sunset picture with its palette and starts the sunset sound), 255 eod_wait () (wait for
the next frame: 2 ms of the game timer), 256 eod_text (int day) (the text "End of Day " followed by `day` in decimal,
signed, drawn centred on the screen), 257 eod_poll () (the screen is shown; result ≠ 0: a key or a mouse button came),
258 eod_sound_stop () (the sunset sound stops), 259 overhead_sounds () (all sound stops; the tactical sounds are
loaded again), 260 show_screen () (the picture without the texts is shown again). Existing ops used: 37 delay, 47 div_zero, 57 merc_text (new texts 3 "is shot retreating!" and 4 "dies retreating!"), 60 sfx, 65
effect_slot, 105 stat_practice, 206 pal_cycle, 207 pal_set, 215 fade, and the library ops 43 connected_home, 78 kill and
180 hit_location. Random(n) is the game's one random sequence; Random(0) is a division by zero (port_api.md): op 47
div_zero, then the result 0.

**Sunset:**

1. Host op 254 eod_begin (day).
2. skipped = no. For frame i = 0..118 (always 119 frames, also after a key):
   a. If skipped = no: host op 255 eod_wait ().
   b. Host op 206 pal_cycle (3, 0xFF). If skipped = no: host op 207 pal_set (0, 0xFF when i = 0, else 0xFE).
   c. If i = 30: host op 256 eod_text (day).
   d. If host op 257 eod_poll () ≠ 0: if i < 30: host op 256 eod_text (day) and host op 207 pal_set (0, 0xFF).
      Then skipped = yes.
   e. If i is a multiple of 4 and Random(5) = 0: a = Random(2) + 1; b = Random(10) + 5; c = Random(4) + 0x13 (in
      this order); host op 60 sfx (c, 0x2B11, b, a).
3. Host op 258 eod_sound_stop ().
4. c = cur_sector. If host op 43 connected_home (c) is 0 in its low byte, leave_reason = 0, santino_spoke = 0, and
   (story byte 0x1C = 0 or story byte 0x0E = 2): the squad is stranded:
   a. If skipped = yes: host op 207 pal_set (0, 0xFE).
   b. Host op 259 overhead_sounds (). Retreat (below). Host op 260 show_screen (). Host op 215 fade (0, 0x38, 0x100,
      0).
   Else: host op 215 fade (0, 0x18, 0x100, 0). Host op 259 overhead_sounds ().
5. Eight times: if byte +0x17E of soldier record 0 is ≥ 8: it becomes 0. (Only record 0: the original never steps to
   the next merc. Leave sector already cleared the value 8 of every merc.)

**Retreat:** c = cur_sector, b = byte 0 of sector_state entry c.

1. way_out = no. The four neighbours in this order, each with its edge test, its sector and its side bit of b:
   north (c > 9; c − 10; bit 0x02), east (c mod 10 ≠ 9; c + 1; 0x04), south (c < 50; c + 10; 0x08), west (c mod 10 ≠
   0; c − 1; 0x10). For each neighbour whose edge test passes: host op 43 connected_home (neighbour); if its low byte
   ≠ 0 and the side bit of b is clear: way_out = yes. (All four are asked; nothing stops early.)
2. risk = 30 × ((1 if bit 0 of b is clear (not owned), else 0) + (1 if way_out, else 0)): 0, 30 or 60.
3. For each merc k = 0..7 with +0x182 ≠ 0 (in the sector), +0x138 ≠ 0 (in play) and +0x188 = 0:
   a. If Random(100) ≤ risk (the merc is hit): host op 260 show_screen (); host op 37 delay (200); d = Random(risk) +
      1 (risk 0: op 47 div_zero, then d = 1); Retreat hit (k, d); host op 57 merc_text (k, 3, 1500).
   b. Then (hp read again) if s8 hp (+0x183) < 15: op 78 kill (k, 1); host op 260 show_screen (); host op 37 delay
      (200); host op 57 merc_text (k, 4, 1500).

**Retreat hit (k, d):** r = op 180 hit_location (k, 0x9C4, d) (signed 32-bit). If r = 0: nothing more. If r > s8 hp:
r = s8 hp. hp = hp − r (8-bit). Host op 105 stat_practice (k, 9, low byte of r, 0). Host op 65 effect_slot (k, 3 if s8
hp < 15, else 2).

So a squad that ends the day in a sector that is not joined to home land has a 30 % chance per merc to be shot
(60 % if the sector is not owned and a joined neighbour is open), for 1..risk damage before armour; a merc left
below 15 hp dies. In an owned sector with no open joined neighbour the risk is 0, but a roll of 0 still counts as a
hit and the original then crashes (Random(0)).

*Evidence (2026-10-23): mirrors tools/ai.end_of_day (EndOfDayScreen 0x29F44), ai.retreat (0x30B04) and
ai.retreat_hit (0x3928C); ai_diff end_of_day 100 cases (24 stranded), 0 mismatches; 3 of 3 hand mutants killed.*

## Joined to home (`ja1_connected_home`, library op 43)

The strategic test "a chain of player-owned sectors joins sector c to the home sector" (End of day, Retreat and
ai.md Sector value ask it as op 43 connected_home (byte c)). No host calls, no random numbers.

```c
int32_t ja1_connected_home(Ja1World *w, const Ja1Host *h, int32_t sector);   /* result 0 or 1 */
```

World field: `sector_state` (80 entries of 9 bytes; b0(n) = byte 0 of entry n, "owned" = bit 0 of b0(n); an
entry n outside 0..79 reads as 0). The map is 6 rows of 10 sectors (n = 10 × row + column); home = 59 (0x3B).
Directions d = 0 east, 1 south, 2 west, 3 north; step(d) = +1, +10, −1, −10; try bit(d) = 0x04, 0x08, 0x10, 0x02;
back bit(d) = 0x10, 0x02, 0x04, 0x08 (the try bit of the opposite direction). All byte values below are 8-bit.

The walk works in one 180-byte array A (all 0 at the start): A[0..59] is the junction stack, A[60 + n] the "done"
flag of sector n, A[120 + n] a copy of b0(n). (The stack can grow past index 59 into the done flags; that is part
of the behaviour.)

1. c = the low byte of `sector`. If c = 59: result 1. If c ≥ 60: result 0.
2. A[120 + n] = b0(n) for n = 0..59. start = c, s = c, d = 0, top = 1, A[0] = c. If bit 0 of A[120 + s] is
   clear: result 0.
3. Repeat (one **round**):
   a. moved = no. While moved = no:
      - If A[120 + s] & try bit(d) = 0: A[120 + s] = A[120 + s] | try bit(d); moved = yes.
      - If A[120 + s] ≥ 0x1E (unsigned): if A[60 + s] ≠ 0: top = top − 1; if top = 0 and A[60 + start] ≠ 0:
        result 0; else if top = 0: s = start and top = 1; else s = A[top]. Else (A[60 + s] = 0): A[60 + s] = 1.
      - If moved = no: d = d + 1. Then (always) if d > 3: d = 0.
   b. n = (s + step(d)) as a byte (so a step below 0 gives 246..255). The move is **blocked** when (s mod 10 = 0
      and d = 2) or (s mod 10 = 9 and d = 0) or n ≥ 60 or bit 0 of A[120 + n] is clear. If not blocked: s = n,
      and only if A[60 + n] = 0: A[120 + n] = A[120 + n] | back bit(d), and then if more than 2 of b0(s + 1), b0(s + 10),
      b0(s − 1), b0(s − 10) have bit 0 set (read from sector_state, not from A, with no edge test; s − 10 and
      s − 1 can be below 0 and read 0, s + 10 can be 60..69): A[top] = s, top = top + 1, d = 0.
   c. If s = 59: result 1.

SPEC GAP (not seen: top never reaches 120, see "Why the walk ends" below): when top reaches 180 before a push,
the original would write past its array (into its other locals); the port then gives result 0. SPEC GAP
(not seen: the walk ends within 166 rounds, see below): on some tables the walk would never end (the original would hang); the
port gives result 0 after 1,000,000 rounds.

**Why the walk ends.** Bits 1..4 of b0 (the barrier sides) have no writer in the game: they come with the world
and are the same in every save (136 dumps). On that fixed table 166 side bits are clear over all 60 sectors.
- Every round sets exactly one clear try bit (step a ends only after a set), and bits are only ever set while
  top < 120 (only a push at top ≥ 120 writes into A[120..179]). So until then there are at most 166 rounds.
- Sector 0 is never pushed (at most 2 owned neighbours), so a push at top 60..119 only writes a non-zero value
  into a done flag. A done flag written as 1 can turn a stack entry into 1, so s is always an owned sector or 1.
- While top has never passed 60 no done flag is wrong, and each push costs 2 clear bits: the round's try bit at
  the old s and the back bit at the new s (an edge carries at most one move each way, and a move sets the other
  end's bit, so that back bit is still clear).
- Reaching top 120 needs 59 pushes to 60 (118 bits) plus 60 more pushes (at least 60 bits): 178 > 166.
So top stays below 120, the table bits only grow, and the walk ends (result 0 or 1) within 166 rounds; the port's
two guards never act. (C check build/probe/home/walk2.c: hill-climb over 600,000 owned sets × 60 starts found top
≤ 55 and ≤ 166 rounds; the stale-back-bit assert never fired.)

*Evidence (2026-10-23): mirrors tools/ai.connected_home (SectorConnectedToHome 0x41178); ai_diff connected 6000
cases, 0 mismatches (result, table and the 180-byte array compared).*

## Timer tick (`ja1_tick`)

The game's 10 ms timer, its soldier and turn part (the other timers of the original's timer belong to the
front-end). No host calls, no random numbers. Uses the turns.md world fields `round_lock`, `round_lock2`,
`show_timer`, `turn_clock`, `shown_slot`, `spotted`, `view_tile`, `fast_timer`, `turn_based`.

0. If round_lock2 ≠ 0: nothing happens (also no step below).
   - ambient_t1 = ambient_t1 − 10 and ambient_t2 = ambient_t2 − 10 (32-bit, always).
   - Show timer: if s32 show_timer > 0: show_timer = show_timer − tick_step (32-bit); if it is then ≤ 0:
     show_timer = 0, shown_slot = 0x40, spotted = 0x40, view_tile = 0xC41.
   - If round_lock ≠ 0: no more steps.
   - Busy timer (shooting.md): if s32 busy_timer > 0: busy_timer = busy_timer − 10; if it is then ≤ 0:
     busy_timer = 0 and busy = 0.
   - Fast timer (turns.md Turn start): if s32 fast_timer > 0: fast_timer = fast_timer − 10; if it is then ≤ 0:
     fast_timer = 0 and turn_based = 1.
   - Turn clock: if s32 turn_clock > 0: turn_clock = turn_clock − tick_step (32-bit); if it is then below 0:
     turn_clock = 0.
   - turn_tick = turn_tick − 10 (32-bit, always; Loop head).
   - ripple_timer = ripple_timer − 10 (32-bit, always).
1. If stop_timer > 0: stop_timer = stop_timer − 10; if it is then below 0: stop_timer = 0.
2. For i = 0..0x2F with +0x182 ≠ 0 and +0x138 ≠ 0 (all values 32-bit, wrapping):
   - u32 +0xB0 = +0xB0 − tick_step; u32 +0xCC = +0xCC − tick_step.
   - If s8 +0x183 ≥ 15 and screen_drawn ≠ 0: +0xA8 = +0xA8 − 10.
   - If s32 +0xC0 > 0: +0xC0 = +0xC0 − 10.
   - If s32 +0xC4 > 0: +0xC4 = +0xC4 − 10, then if s32 +0xC4 < 0: +0xC4 = −1.
   - If s32 +0xB4 > 0: +0xB4 = +0xB4 − 10, then if s32 +0xB4 ≤ 0: +0xB4 = 0.
   - If i < 8: if s8 +0x189 > 0: +0xBC = +0xBC − 10; if s32 +0x1DC > 0: +0x1DC = +0x1DC − 10.
3. Fuses: for each `timed_fx` record f with s32 f+0 > 0:
   - If s8 f+9 ≤ 0: f+0 = f+0 − 10; if it is then ≤ 0: f+0 = 0 (Fuses sets it off in the next pass).
   - If f+0xC ≠ 0 and s32 fuse_beep_timer > 0: fuse_beep_timer = fuse_beep_timer − 10; if it is then ≤ 0:
     fuse_beep = 1 and fuse_beep_timer = 700.
     **Fuse tick** (front-end): every pass of the play loop, after the timer ticks and before One frame, when
     fuse_beep ≠ 0: fuse_beep = 0 and effect 0x2C (tactical bank) plays as op 60 sfx (0x2C, 11025, 0x68, 1).
     Evidence: TacticalLoop 0x10BD1..0x10BFB (PlaySfxSlot 0x5CB34, then byte 0x284F29 = 0).

tick_step is 10 in normal play (the original sets 1000 for a fast mode; the front-end keeps 10).

**The front-end's animation timers** (not the library; under the same gates as steps 1-2, every 10 ms): the
front-end starts tile animations (up to 10: a count, a timer and a tile; it sets the tile's byte 3 to the frame) and
effect animations (the 16 `fx_anims` entries). Each 10 ms: a tile animation with count > 0 (signed) loses 10 off its
timer; when the timer is then ≤ 0, the count goes down by 1, and if the count is still > 0 the timer restarts at
140, else (when its tile is below 0xC41 as a signed 16-bit value) **the tile's byte 3 becomes 0** and the animation
has no tile. An effect animation with count ≠ 0 loses 10 off its timer; when the timer is then ≤ 0 it restarts at
140 and the count goes down by 1 (8-bit). The library only reads these (`fx_anims` count in One frame step 3, tile
byte 3 when it draws nothing): a front-end that does not run this timer leaves tile byte 3 set.

## Notes for the port

- Off-map reads: a tile outside 0..3135 reads its bytes as 0 here (the tests keep the reads that matter on the
  map). A ground index 200..254 reads its ground entry as 0.
- Every step that says "(8-bit)" wraps the result to a byte. Step AP and the use cost compare as unsigned bytes;
  "ap > s8 +0x1A5" compares the unsigned ap with the signed AP.

*Port evidence (2026-10-21): port/ja1logic src/update.rs (clean-room, from this page) gives 0 mismatches against
the original in ai_diff.py --port for all 9 entry points (walk_step, walk_blocked, walk_end, stand_up,
frame_checks, merc_quote, enemies_see 2,000 cases each; soldier_frame, active_frame 6,000).*

*Evidence: tools/ai.py walk_move / walk_step / walk_blocked / stand_up / walk_end / frame_checks / merc_quote /
enemies_see / soldier_frame / step_start / active_frame and their leaves, diff-tested against the original in
ai_diff.py (mutation scores in REVERSE.md "Soldier update per frame").*
