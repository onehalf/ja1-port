# Turns: the end of the mercs' turn, the enemies' turn, the mercs' turn again

How a combat round moves from side to side. The player ends the mercs' turn (**End turn**); the enemies' side
starts (**Enemies' turn**); the soldiers 8..0x2F then act one after the other (soldier_update.md "One frame",
step 2 b, the hand-over); when the last one is done a new round starts (op 134 new_turn, combat.md "A side's new
turn", see **New turn**) or, after a merc's interrupt, the mercs' turn goes on (**Mercs' turn**).

Conventions as in soldier_update.md: record k = the 0x200-byte soldier record k (the original reaches the
records through a table of 48 pointers that always points at the records in order), NONE = 0xC41, "s8" = the
signed byte, every other byte test is on the unsigned byte. "In play"(k) = +0x182 ≠ 0 and +0x138 ≠ 0 and s8
+0x183 ≥ 15. "Host op N name (args)" = a host call (port_api.md).

## Entry points

```c
void    ja1_end_turn(Ja1World *w, const Ja1Host *h);          /* the player's End turn key / button */
/* also entry points (tests): */
int32_t ja1_first_merc(Ja1World *w, const Ja1Host *h);        /* result 0..7, or 0x40 = none */
void    ja1_turn_sightings(Ja1World *w, const Ja1Host *h, int32_t both);   /* both: low byte */
int32_t ja1_itq_pop(Ja1World *w, const Ja1Host *h);           /* = library op 131; result: low byte */
void    ja1_enemies_turn(Ja1World *w, const Ja1Host *h);      /* = library op 133 round_end */
void    ja1_mercs_turn(Ja1World *w, const Ja1Host *h);        /* = library op 132 turn_hand_over */
```

Library ops (the library runs them itself unless their host_ops bit is set): 120 low_ap_stop = Select merc
(select.md), 123 turn_banner = Turn start, 131 itq_pop = Interrupt queue,
132 turn_hand_over = Mercs' turn, 133 round_end = Enemies' turn, 141 start_turn = Start of a soldier's turn (this page); 140 move_order = soldier_update.md
"Move order"; 40 path_ap = soldier_update.md "Path ops". Inside this page's procedures, First merc, Turn
sightings, Interrupt queue, Enemies' turn, End turn and Age knowledge are the library's own procedures, never
host calls.

New world fields (port_api.md, "turns"): `itq` (50 bytes, the interrupt queue: soldier slots, 0x40 = free),
`itq_len` (u8), `bled` (48 bytes, one per slot: the soldier bled at the start of this turn), `timed_fx` (5 × 13
bytes, below), `new_round` (u8), `turn_timer` (i16), `turn_timer2` (i16), `shown_slot`
(i32), `view_tile` (i32), `focus_slot` (u8), `turn_shown` (u8), `merc_to_show` (u8), `view_hold` (u8).
Existing fields used: `show_ai` (≠ 0: the mercs' turn is paused for the other side, whose actions redraw the screen), `selected` (the soldier whose turn it is), `kept_merc`, `screen_drawn`, `interrupt_turn`
(≠ 0: the enemies' side has started its turn in this round), `combat`, `quote_slot`, `quote_delay`, `spotted`,
`opt_onscreen`, `walk_ap`, `key_shift`, `key_alt`, `mission_step`, `mission_slot`, `mission_target`, `alarm`,
`knowledge`, `calls`, `remember_m`, `remember_e`, `tiles`, `ground_items`, `item_types`.

**Timed effects** (`timed_fx`, 5 entries of 13 bytes, entry f at byte 13 × f): i32 +0 (negative = free), i16 +4,
i16 +6 tile, u8 +8 kind, i8 +9 timer, i8 +0xA, u8 +0xB owner slot, i8 +0xC.

New host ops: 140 move_order is a library op (above). 141 start_turn (slot): the soldier's turn starts (library
op: Start of a soldier's turn, below). 142 mouse_hide (). 143 deselect (int 0). 144
panel_redraw (int 0x18). 145 mouse_off (). 146 mouse_move (int 0x50, int 8). 147 mouse_save (). 148
panel_slots (int 1). 149 portrait_refresh (byte slot, int 1). 150 timed_effect (byte kind, int a, int b, byte
owner, int c). None of them returns anything that is read. Existing host ops used: 19 mission_cancel, 34
share_sightings, 63 enemy_turned, 64 guard_turned, 65 effect_slot, 90 explode, 99 focus, 116 ask_interrupt, 120
low_ap_stop (here: select the soldier; result 0 = not selected), 128 ask_merc_interrupt; library op 123 turn_banner
(Turn start, below).

## First merc (`ja1_first_merc`)

The first merc k = 0..7 (in order) with +0x182 ≠ 0, +0x17E = 0 or 8, and +0x183 ≠ 0. Result k; none: 0x40.

## Turn sightings (`ja1_turn_sightings`)

1. If both ≠ 0: for k = 0x28..0x2F with in play(k): host op 63 enemy_turned (k).
2. For k = 0..15 with in play(k): host op 64 guard_turned (k); host op 34 share_sightings (k, 0).
3. For k = 8..15 with in play(k): for j = 0..7: if remember_m[8 × k + j] = 0xFF: host op 128 ask_merc_interrupt
   (k, 0x28 + j).
4. If both ≠ 0: for k = 0..7 with in play(0x28 + k): for j = 0..0x27: if remember_e[0x28 × k + j] = 0xFF: host op
   116 ask_interrupt (0x28 + k, j).

"In play" is read again at each k (a host call may change the records).

## Interrupt queue (`ja1_itq_pop`, op 131)

The last usable soldier in the queue. All values are bytes.

1. n = itq_len; i = n; r = 0x40.
2. While i > 0:
   - r = itq[i − 1].
   - If r = 0x40 (a free entry): i = i − 1; if i ≤ 0: r = 0x40. Go on with the next round of step 2.
   - n = n − 1 (8-bit); itq[n] = 0x40. (The queue loses its last entry, which need not be entry i − 1.)
   - If record r has +0x182 ≠ 0, s8 +0x183 ≥ 15 and +0x1B0 = 0: stop (step 3).
   - i = i − 1. If i = 0: stop (step 3; r stays the unusable soldier — a quirk of the original).
3. itq_len = n. Result r.

## Age knowledge (procedure)

Age knowledge(side), side a byte:
1. If side ≠ 0: lo = 0x28, hi = 0x30, c = 0 (the mercs' call, `calls` bytes 0..2); else lo = 0, hi = 0x28, c = 4
   (the enemies' call, bytes 4..6).
2. t = calls[c + 2]. If t ≠ 0: calls[c + 2] = t − 1; if t = 1: u16 at calls[c] = NONE.
3. For k = lo .. hi − 1 with +0x182, +0x138 and +0x183 all ≠ 0: a = knowledge[+0x17A of record k]; v = s8 a.
   If v ≥ 2: v = v + 1 (8-bit, read as s8 again); a = 0 if v > 4, else v. Else, if v ≤ −1: v = v − 1 (8-bit,
   s8 again); a = 0 if v < −3, else v. (So 0x7F becomes 0x80 = −128 and stays there this turn.)

## Enemies' turn (`ja1_enemies_turn`, op 133)

1. bled[0..47] = 0. interrupt_turn = 1.
2. For k = 0x28..0x2F: if record k has +0x182 ≠ 0 and +0x183 ≠ 0: op 141 start_turn (k) (Start of a soldier's turn); then +0x9C = 0xFF.
3. If s8 mission_step > 0, s8 alarm = 1, s16 mission_target ≠ NONE and mission_slot = 0x40: host op 19
   mission_cancel ().
4. Turn sightings (0). Age knowledge (screen_drawn).
5. selected = 8. Host op 123 turn_banner ().
6. For k = 0x28..0x2F, s = record k: if bled[+0x17A] ≠ 0 and s8 +0x9C > 0 and On screen(s) (ai.md helpers, "On screen"): host op 65 effect_slot (+0x17A, 3).

## End turn (`ja1_end_turn`)

The player ends the mercs' turn (the original's End turn key and button).

1. For f = 0..4, e = entry f of `timed_fx`: if s32 e+0 ≥ 0 and s8 e+9 ≤ 0:
   - t = s16 e+6; e+0 = −1 (i32).
   - If t < NONE (signed compare): g = byte 6 of tile t; if g < 200: k = the blast kind (sector_start.md batch 9)
     of the type ground_items[12 × g]; if k = e+8: host op 90 explode (e+0xB, t, k, 1).
   - Else: library op 150 ground_blast (sector_start.md Ground blast) (e+8, s8 e+0xA, s16 e+4, e+0xB, s8 e+0xC).
2. shown_slot = 0x40; spotted = 0x40; view_tile = NONE (both i32).
3. If quote_slot < 0x40: quote_delay = 0.
4. If screen_drawn = 0: done.
5. new_round = 0. Host op 142 mouse_hide (). show_ai = 0. Byte 7 of every tile (0..3135) = 0.
6. merc_to_show = selected. screen_drawn = 0. turn_timer = 0. kept_merc = 0x40. Host op 143 deselect (0). Host
   op 144 panel_redraw (0x18).
7. For k = 0..7, s = record k: if +0x182 ≠ 0 and +0x138 ≠ 0 and +0x188 = 0: +0x17E = 8; +0x194 = 0; +0x1B0 = 1;
   host op 149 portrait_refresh (+0x17A, 1).
8. Host op 145 mouse_off (). If opt_onscreen < 3: host op 146 mouse_move (0x50, 8). Host op 147 mouse_save (). The window moves the mouse to (80, 8) and keeps that place; the next op 157 mouse_refresh puts the mouse back there (live in the original, #45). A move during the enemies' turn does not count.
9. r = Interrupt queue (). If r < 0x40: selected = r; host op 123 turn_banner (). Else, if interrupt_turn = 0:
   Enemies' turn; done. Else: selected = 8; host op 123 turn_banner ().

## Mercs' turn (`ja1_mercs_turn`, op 132)

The turn goes (back) to the mercs: after a merc's interrupt in the enemies' turn, or when the hand-over finds a
merc.

1. If new_round = 0: turn_timer = 20; turn_timer2 = 0. screen_drawn = 1.
2. **After a pause** (show_ai ≠ 0):
   1. show_ai = 0. If kept_merc < 8: selected = kept_merc.
   2. s = record selected. +0x1A6 = +0x95. u16 +0x134 = u16 +0x96.
   3. t = s16 +0x120. If t = NONE or u16 +0x114 = u16 +0x120: done.
   4. Path AP (selected, t, sneak = s8 +0x1A8, byte177 = s8 +0x177, alt = s8 +0x196) = op 40 path_ap; as a host
      call its arguments are (selected, t). Its result is not used: the next step reads walk_ap.
   5. If s8 +0x1A5 < walk_ap (signed 32-bit compare): done.
   6. Move order (selected, t, marker 0, sneak = key_shift, byte177 = +0x177, alt = key_alt, squad 0) = op 140
      move_order; its host call arguments are (selected, t, 0, key_shift, +0x177, key_alt, confirm 0, squad 0).
      The result is not used. Done.
3. **A new mercs' turn** (show_ai = 0):
   1. Host op 148 panel_slots (1).
   2. For k = 0..7, s = record k: if +0x182 ≠ 0 and +0x17E = 8: if +0x1B0 = 0: +0x17E = 0, else +0x194 = 1. Then
      if kept_merc ≠ +0x17A: host op 149 portrait_refresh (k, 1).
   3. shown_slot = 0x40. If kept_merc < 8: s = record selected; if +0x182 ≠ 0 and s8 +0x183 ≥ 10: shown_slot =
      kept_merc, +0x1B0 = 0, and if s16 +0x120 ≠ NONE and u16 +0x120 ≠ u16 +0x114: +0x16D = 1. Then (in both
      cases) kept_merc = 0x40.
   4. If shown_slot = 0x40: shown_slot = First merc (); if it is 0x40 or more: End turn; done.
   5. focus_slot = low byte of shown_slot. Host op 99 focus (low byte of shown_slot, 0).
   6. If shown_slot ≥ 8 (signed): shown_slot = First merc ().
   7. turn_shown = 1. view_hold = 0.
   8. r = op 120 low_ap_stop (shown_slot, 1, 1) (Select merc, select.md). If r = 0 (32-bit): selected = First merc () (low byte).

## New turn (`ja1_new_turn`, op 134 new_turn)

A new round (combat.md "A side's new turn"): the soldier loop (soldier_update.md "One frame") makes op 134 when
every soldier of the round is done. Library op 134: the library runs this procedure itself unless host_ops bit 63
is set. Argument: focus (≠ 0, all 32 bits: the view goes to the merc to show). Result: a byte (1 = guards arrived
instead; else 0).

```c
int32_t ja1_new_turn(Ja1World *w, const Ja1Host *h, int32_t focus);
```

New world fields (port_api.md, "turns"): `round_lock`, `round_lock2`, `handed_over`, `turn_flag`, `interrupter`,
`speaker`, `speech_wait`, `stage`, `redraw_next` (u8 each), `redraw_pending`, `turn_clock`, `show_timer` (i32
each), `sector_spoken` (60 bytes, one per sector). Existing fields used: `view_hold`, `combat`, `bled`, `quote_slot`,
`quote_delay`, `selected`, `kept_merc`, `interrupt_turn`, `show_ai`, `anim_signal`, `spot_stop`, `screen_drawn`,
`ap_warned`, `itq`, `itq_len`, `turn_moves`, `interrupt_go`, `cur_sector`, `sector_state`, `merc_to_show`,
`shown_slot`, `spotted`, `new_round`, `bomb_count`, `timed_fx`, `santino_spoke`, `soldiers`.

New host ops (none returns anything that is read unless a result is named): 157 mouse_refresh (). 158 turn_ui ().
159 guards_arrive (int 5): result (32 bits) ≠ 0 = guards arrive in the sector. 160 sector_tick (). 161 gas_tick ()
(the gas clouds act once: combat.md "Gas clouds"). 162 mouse_check (): the front-end tests the mouse against
its panel and restores the mouse pointer when it is outside. 163 cursor_reset (): the front-end resets its
cursor. 164 view_save (). 165 view_restore (). 166 item_panel
(slot, int 0, int 1). 167 speech_ready (): result (32 bits) ≠ 0 = a sector speech may start. 168 sector_speech
(slot, int sector). 169 stage_effects (). 170 stage_five () (the home-sector event at stage 5: the enemies that
nobody sees leave). 171 redraw_kept (). Existing ops used: 65 effect_slot, 99 focus, 114 next_turn (library op,
below), 119 ai_think, 120 low_ap_stop, 141 start_turn, 148 panel_slots.

"Record v" for v = 0x40 (no merc): the original reads byte 0x200 × 0x40 + 0x183 − 0x60C0 = 0x20C3 of `tiles`
(tile 1048, byte 3) as its +0x183. Values 0x30..0x3F do not occur here.

**Procedure:**
1. keep = 0. view_hold = 0. If combat ≠ 0: host op 157 mouse_refresh ().
2. If redraw_pending ≠ 0: keep = 1; redraw_pending = 0.
3. Host op 158 turn_ui ().
4. If combat ≠ 0: round_lock = 1; round_lock2 = 1.
5. bled[0..47] = 0.
6. q = quote_slot. If q < 0x40: quote_delay = 0; Merc quote (soldier_update.md) for slot q.
7. If handed_over = 0:
   a. If host op 159 guards_arrive (5) ≠ 0: round_lock2 = 0; round_lock = 0. Result 1. Done.
   b. If turn_moves ≠ 0: if selected < 8: kept_merc = selected. Op 114 next_turn (1). interrupt_turn = 1;
      kept_merc = 0x40; handed_over = 1; round_lock2 = 0; round_lock = 0. Result 0. Done.
   Else (handed_over ≠ 0): handed_over = 0.
8. interrupt_turn = 0. turn_clock = 20000. turn_flag = 0. show_ai = 0. anim_signal = −1. spot_stop = 1.
   screen_drawn = 1.
9. If combat ≠ 0: ap_warned[0..7] = 0; itq_len = 0; turn_moves = 0; interrupt_go = 0; interrupter = 0x40;
   kept_merc = 0x40; itq[0..49] = 0x40; host op 148 panel_slots (1).
   Else: for k = 8..15: +0x197 of record k = 0; op 119 ai_think (k).
10. Op 160 sector_tick () (a library op: Sector tick below). Host op 161 gas_tick ().
11. If sector_state[9 × cur_sector + 2] = 0: combat = 0.
12. For k = 0..0x27: if record k has +0x182 ≠ 0, +0x138 ≠ 0 and +0x183 ≠ 0: op 141 start_turn (k).
13. For k = 0x28..0x2F: +0x198 of record k = its +0x9C.
14. If combat ≠ 0:
    a. Age knowledge (screen_drawn).
    b. If merc_to_show ≥ 0x30: merc_to_show = low byte of First merc ().
    c. v = merc_to_show. shown_slot = v. show_timer = 5000.
    d. If s8 +0x183 of record v ≤ 0: r = First merc (); shown_slot = r. If r ≥ 8: selected = merc_to_show;
       round_lock2 = 0; round_lock = 0; Mouse check (below). Result 0. Done.
    e. If sector_state[9 × cur_sector + 2] ≠ 0:
       - If focus ≠ 0: host op 164 view_save (); host op 99 focus (low byte of shown_slot, 0); spotted = 0x40.
       - Op 120 low_ap_stop (shown_slot, 0, 0) (Select merc).
       - If selected < 8: host op 166 item_panel (selected, 0, 1).
       - If focus ≠ 0: host op 165 view_restore ().
       - (Front-end, wipe.rs.) Op 164 = 0x124FC: for i < view_w / 2 the view columns left + i and right − i go
         black, 1 ms wait per step. Op 165 = 0x1259C: the new view is drawn into the back buffer; for d < view_h / 2
         view rows half − d and half + d go to the screen, 1 ms wait per step (view row 0 never does: quirk).
         Test: wipe_diff --port.
       - Mouse check.
15. Else (combat = 0): if selected > 8: selected = 8; host op 163 cursor_reset ().
16. new_round = 1. bomb_count = 0.
17. For f = 0..4: if timed_fx[13 f + 9] ≠ 0: timed_fx[13 f + 9] = timed_fx[13 f + 9] − 1 (8-bit).
18. If speech_wait ≠ 0: speech_wait = speech_wait − 1.
19. x = speaker. If x < 0x40 and sector_spoken[cur_sector] = 0 and speech_wait = 0 and host op 167 speech_ready ()
    ≠ 0 and Merc busy (soldier_update.md, Merc view) = 0 and s8 +0x183 of record x ≥ 15 and +0x138 of record x ≠ 0:
    host op 168 sector_speech (x, s8 cur_sector). (The tests stop at the first one that fails: the host op 167 call
    happens only when the three tests before it pass.)
    **Sector line** (front-end): op 167 = CountLiveMercs? 0x410C4 (the count of the 8 merc records with profile
    +0x179 < 0x3C, +0x182 != 0, s8 +0x183 > 0). Op 168 = 0x297D4 (record x, sector): nothing when +0x30 = 0; else
    time compression off, Lock, the face of profile +0x179, PlayQuote (profile, row 0, quote = sector, 200, the
    shared buffer, category 8 NAT, voice member NAT<sector>.8 of NAT\NAT), the speech balloon of the merc message
    (panel.md "Speech balloon", panel slot +0x17A, no hurt face, empty weapon text), shown_slot = +0x17A, FocusOn
    (shown_slot, 0), show_timer = 5000, sector_spoken[cur_sector] = 1, Unlock. The last quote = (profile, 8, row 0,
    sector). Tests: balloon_diff --port (native cases, live-merc counts), smoke step sector_line.
20. If cur_sector = 0 and santino_spoke ≠ 0 and stage < 5: stage = stage + 1; if stage < 4: host op 169
    stage_effects (). If stage = 5 (read again): sector_state[9 × cur_sector + 1] = that byte − 1 (8-bit); host op
    170 stage_five ().
21. round_lock2 = 0; round_lock = 0.
22. For k = 0..0x27: sl = +0x17A of record k; if bled[sl] ≠ 0: host op 65 effect_slot (sl, 3).
23. If keep ≠ 0: host op 171 redraw_kept ().
24. If redraw_next ≠ 0: redraw_next = 0; redraw_pending = 1.
25. Result 0.

**Mouse check**: host op 157 mouse_refresh (); host op 162 mouse_check ().

### Sector tick (`ja1_sector_tick`, op 160 sector_tick)

Once per new turn a soldier standing in water may start a splash animation. Library op 160: wherever the library's
code makes it, it runs these steps, unless bit 63 of host_ops is set.
1. d = s8 splash[cur_sector]. n = cdiv(1000, d) (if d = 0: host op 47 div_zero, n = 0). Nothing more if n ≤ 0.
2. Repeat n times: t = Random(0xC40); ripple_tile = t. Skip to the next try if byte 0 of t is below 0x97, or byte 1
   of t is not 0, or o = byte 5 of t & 0x7F is 0x30 or more (SPEC GAP (not seen: tile byte 5 is only ever 0x40 (map load, Vacate) or a soldier slot 0..0x2F (Occupy, sector start), each with or without the 0x80 flag): the original reads past the soldier records
   for 0x30..0x3F; the library treats them as no soldier). If s8 +0x183 of record o > 0 and the animation word u16
   +0x1E8 + 2 × +0x1F3 of record o is below 0x3C: op 52 set_anim (o, 0x3D, 0, 1), and stop.

## Next turn (`ja1_next_turn`, op 114 next_turn)

The turn moves on after an interrupt (combat.md "Handing the turn over"). Library op 114 (host_ops bit 63).
Argument: mode (low byte, signed). Uses `busy` (i32), `turn_moves`, `merc_to_show`, `selected`, `screen_drawn`,
`show_ai`, `next_selected`. New host op: 172 enemy_focus (): result (low byte) = the merc to show during an enemy
turn. Existing ops used: 36 redraw; library op 123 turn_banner (Turn start).

```c
void ja1_next_turn(Ja1World *w, const Ja1Host *h, int32_t mode);
```

1. If busy > 0: done.
2. turn_moves = 0.
3. If merc_to_show = 0x40: merc_to_show = selected if selected < 8, else the low byte of host op 172 enemy_focus ().
4. If screen_drawn ≠ 0: host op 36 redraw (); screen_drawn = 0; show_ai = 1.
5. If s8 mode = 1: selected = Interrupt queue (); host op 123 turn_banner (). Else: next_selected = Interrupt
   queue ().

## Turn start (`ja1_turn_start`, op 123 turn_banner)

The selected soldier's turn (or interrupt) starts: its side's banner, its alert, and 15 seconds of real time.
Library op 123 (host_ops bit 63): Enemies' turn, End turn, Next turn and One frame (the hand-over) run it after
they set `selected`. No arguments. s = record selected. Uses `selected`, `kept_merc`, `itq`, `itq_len`,
`turn_based` and the new world field `fast_timer` (i32, below). New host op 194 banner (kind): the front-end shows
the banner `kind` (nothing is read back). An original-game bug (issue #2 row 25): with selected = 0x40 (an empty
interrupt queue) "record 0x40" lies in `tiles`: field f is byte f + 0x1F40 of `tiles` (+0x182 = byte 2 of tile 1048,
+0x183 = byte 3 of tile 1048, +0x138 = byte 0 of tile 1039, +0x1B0 = byte 0 of tile 1054, +0xBA = byte 2 of tile
1023). The port runs steps 1 and 2 below on those bytes: step 1 can write byte 0 of tile 1054 (the ground of that
tile) = 1. Step 2 then always ends the procedure (SPEC GAP (not seen: byte 0 of tile 1054 is not 0 in all 60 maps
of TERRAIN.DAT, and the only writes to a byte 0 at run time are this step 1 (1) and the craters (0x1D..0x1F)):
steps 3..9 with record 0x40). The banner kinds:

| kind | text | colour (palette index) |
|---|---|---|
| 0 | `OPPONENTS' TURN...` | 0x72 |
| 1 | `GUARDS' TURN...` | 0xA2 |
| 2 | `OPPONENT INTERRUPTS!` | 0x72 |
| 3 | `GUARD INTERRUPTS!` | 0xA2 |
| 4 | `SANTINO INTERRUPTS!` | 0x72 |

(The original keeps the text and the colour in two variables that its screen redraw and its quick save use: they
belong to the front-end.) Existing host ops used: 7 tile_dist, 34 share_sightings, 49 sight_line, 63
enemy_turned, 64 guard_turned. Library procedures used: Alert state (ai.md), Orders bump (shooting.md), Merc view
(op 62 reveal_view, soldier_update.md).

```c
void ja1_turn_start(Ja1World *w, const Ja1Host *h);       /* = library op 123 turn_banner */
void ja1_ally_alert(Ja1World *w, const Ja1Host *h, int32_t slot);   /* also an entry point (tests) */
```

1. If +0x182 = 0 or +0x138 = 0 or s8 +0x183 < 15: +0x1B0 = 1. Done.
2. If +0x1B0 ≠ 0 or s8 +0xBA = 2: done.
3. interrupt = (kept_merc < 8). If itq_len ≠ 0 and q = itq[0] is on the other side (q ≥ 0x28 and slot < 0x28, or
   q < 0x28 and slot ≥ 0x28): interrupt = 1.
4. kind: if s8 +0xBA < 3: 3 if interrupt, else 1. Else: if interrupt: 2 if s8 +0xBA = 3, else 4; else 0. Host op
   194 banner (kind).
5. +0x197 = 0. If slot ≥ 0x28: host op 63 enemy_turned (slot). Else if slot < 0x10: host op 64 guard_turned
   (slot); host op 34 share_sightings (slot, 0).
6. Alert state (s) (ai.md).
7. If s8 +0xBB is 2 or 3: +0x1C1 = 1. Else: Ally alert (slot).
8. fast_timer = 15000; turn_based = 0.
9. Merc view (+0x1A1, +0x19E, slot, think 1) (op 62 reveal_view (slot, +0x1A1, +0x19E, 1)).

**Fast timer.** While fast_timer > 0 the turn runs in real time (turn_based = 0); the Timer tick (soldier_update.md)
counts it down by 10 each 10 ms and then sets turn_based = 1. (The original's tactical loop, outside the library,
also starts it again at 15000 whenever selected ≥ 8, turn_based = 0 and fast_timer ≤ 0, before it reads the
keys: a front-end that runs the turn-based loop does that check once per pass.)
The sector start (SetupSectorFromXra 0x2E941) sets fast_timer = 0 together with turn_based = 0.

### Ally alert (`ja1_ally_alert`)

The soldier looks at its own side: k = 0..15 for a slot below 0x28, else k = 0x28..0x2F, in order. O = record k.

1. Skip k if k = slot, or O +0x182 = 0, or O +0x138 = 0, or O +0x183 = 0.
2. Skip k if host op 7 tile_dist (s16 +0x114, s16 O +0x114) (i32) > 14.
3. Skip k if host op 49 sight_line (slot, s16 +0x116 + 2, s16 +0x118 + 4, s16 O +0x114, s16 O +0x116 + 2, s16 O
   +0x118 + 4, 1, 0x0E, k) = 0.
4. If s8 O +0xBB ≥ 2, or O +0x1C4 ≠ 0, or s8 O +0x183 < 15: +0xBB = 2; Orders bump (s); +0x1C1 = 1. Done (no
   more k).
5. Else if s8 O +0xBB = 1 and s8 +0xBB < 1: +0xBB = 1; +0x1C1 = 1; +0x1C0 = 3; u16 +0x122 = u16 O +0x114.
   (The search goes on.)

## Merc interrupt turn (`ja1_interrupt_turn`, op 113 merc_interrupt)

A merc's interrupt is granted (combat.md "Handing the turn over", "A merc interrupt turn"). Library op 113 (host_ops
bit 63). Uses `busy`, `screen_drawn`, `turn_timer`, `turn_timer2`, `interrupt_go`, `show_ai`, `interrupter`,
`shown_slot`, `show_timer`, `view_hold`. Existing ops used: 99 focus, 120 low_ap_stop, 148 panel_slots, 149
portrait_refresh, 157 mouse_refresh.

```c
void ja1_interrupt_turn(Ja1World *w, const Ja1Host *h);
```

1. If busy > 0: done.
2. screen_drawn = 1. turn_timer = 20. turn_timer2 = 0. interrupt_go = 0. show_ai = 0. Host op 148 panel_slots (1).
3. For k = 0..15, s = record k: if +0x182 ≠ 0 and +0x183 ≠ 0 and +0x17E = 8: if +0x1B0 = 0: +0x17E = 0, else
   +0x194 = 1. Then if interrupter ≠ +0x17A: host op 149 portrait_refresh (+0x17A, 1).
4. v = interrupter. If v > 7: shown_slot = 0; interrupter = 0. Else: shown_slot = v.
5. show_timer = 5000. Host op 99 focus (low byte of shown_slot, 0). view_hold = 0. Op 120 low_ap_stop (Select merc)
   (shown_slot, 1, 1). Host op 157 mouse_refresh ().

*Evidence (2026-10-22): mirrors tools/ai.py new_turn (compose_diff --newturn), next_turn, interrupt_turn (ai_diff
turn_flow); the original NewTurn? 0x126C8 read again for the record-v read and the 32-bit tests.*

## Start of a soldier's turn (`ja1_start_turn`, op 141 start_turn)

What each soldier does when its side's turn starts (combat.md "Start of a soldier's turn"). Library op 141: the
Enemies' turn (step 2) and the new round (op 134, **New turn** step 12) run it for a soldier. s = record
slot. Uses the world fields `combat`, `cur_sector`, `story_off` = story[0x16], `story_sector` = story[0x17] (soldier_update.md, Merc view),
`bled` and `screen_drawn`.

```c
void ja1_start_turn(Ja1World *w, const Ja1Host *h, int32_t slot);
/* the parts, also entry points (tests): */
void ja1_bleed(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_turn_drown(Ja1World *w, const Ja1Host *h, int32_t slot);
```

New host ops: 151 aid_tick (slot): the soldier's first aid goes on for one turn. 152 turn_rest (slot): the
soldier rests on its spare AP (fatigue). 153 wound_bar (slot, int 1): the wound display of the soldier changes.
None of them returns anything that is read. Existing host ops used: 35 perceive (here with the argument 1), 54
merc_message, 58 reserve_ap, 62 reveal_view (= Merc view, library op), 78 kill, 99 focus, 105 stat_practice, 110
take_damage, 135 portrait, 149 portrait_refresh.

**Procedure:**
1. v = s8 +0x1C4. If v = 1: +0x1C4 = 0xFF. Else if v = −1: +0x1C4 = 0.
2. +0x1E1 = +0x1E2 = +0x1E3 = 0. +0x16D = 0.
3. If +0x1B9 ≠ 0: host op 151 aid_tick (slot).
4. Host op 152 turn_rest (slot). +0x1DB = 0.
5. If +0x185 ≠ 0 or s8 +0x186 = 1: Bleed (below).
6. If +0x183 ≠ 0 and +0x18A = 0: Drown at the turn start (below).
7. If +0x183 = 0: done (no Frame delay either).
8. +0x1F2 = 0.
9. If combat ≠ 0:
   a. Host op 35 perceive (slot, 1).
   b. If s16 +0x122 ≠ NONE and +0x1C0 ≠ 0: +0x1C0 = +0x1C0 − 1; if it is 0 now: u16
      +0x122 = NONE.
   c. saved = 5 if s8 +0x1A5 > 5, else +0x1A5 (the byte). m = the low byte of Max AP (ai.md "Max AP") of s.
      +0x1A5 = m + saved (8-bit) if m ≠ 0, else 0. If s8 +0x1A5 > 25: +0x1A5 = 25.
   d. If slot < 8 and +0x17E = 8: +0x17E = 0; +0x1B0 = 0; if s16 +0x120 ≠ NONE and u16 +0x120 ≠ u16 +0x114:
      +0x16D = 1; then host op 58 reserve_ap (slot) and host op 149 portrait_refresh (slot, 1). (A merc with
      another +0x17E: nothing in this step.) If slot ≥ 8: +0x1B0 = 0.
10. Else (combat = 0): +0x1A5 = the low byte of Max AP of s.
11. If slot < 8 and s8 +0x183 ≥ 15 and cur_sector = s8 story_sector (the u8 against the signed value) and
    story_off = 0 and Merc busy (soldier_update.md, Merc view) = 0: Merc view (slot, facing +0x1A1, range +0x19E,
    think 0) = op 62 reveal_view.
12. Frame delay (ja1_anim_delay, ai.md) of s.

Step 9 b in plain words: a remembered noise to check (+0x122) is forgotten when its counter +0x1C0 runs out.

### Bleed (`ja1_bleed`)

1. If not (s8 +0x185 > 12 or s8 +0x186 = 1): done. If +0x1B9 is 4 or 6 (the soldier gives first aid): done.
2. u16 +0x1F0 = u16 +0x1F0 − 1. If s16 +0x1F0 > 0: done.
3. If (+0x184 − +0x185 − +0x183) mod 256 ≠ 0 and +0x186 = 0: +0x185 = +0x185 + 1 (8-bit); host op 153 wound_bar
   (slot, 1); +0x189 = 7; host op 135 portrait (slot, 1).
4. Else:
   a. If s8 +0x186 = 1:
      - If s8 +0x183 ≥ 10 and +0x187 = 0: keep = screen_drawn; if combat ≠ 0: screen_drawn = 0. Host op 54
        merc_message (slot, 9, 0x40, 0, 1). screen_drawn = keep. +0x187 = 1.
      - If s8 +0x184 > 15: +0x184 = +0x184 − 1; +0x185 = +0x185 − 1 (8-bit each).
   b. Host op 110 take_damage (slot, 0x1356, 1, 3, 0x40, NONE).
5. bled[+0x17A] = 1.
6. u16 +0x1F0 = cdiv(s8 +0x183, s8 +0x1F2 + 10) + 1 (read after step 4; a divisor of 0: port_api.md, Division
   by zero).

### Drown at the turn start (`ja1_turn_drown`)

Not the same as soldier_update.md "Drown" (that one runs per frame). cur = the current animation word u16 +0x1E8 +
2 × +0x1F3.

1. If cur < 0x39: done.
2. If s8 +0x9C > 0 or follow_all ≠ 0: host op 99 focus (slot, 0).
3. If +0x1BA = 0: host op 54 merc_message (slot, 0xA, 0x40, 0, 1); +0x1BA = 1; host op 105 stat_practice (slot,
   4, 5, 0). Done.
4. d = random(10) + 2 (8-bit); d = d + random(10) (8-bit); if +0x19C = 5: d = d × 2 (8-bit). Host op 110
   take_damage (slot, 0x1356, d, 2, 0x40, NONE). If +0x183 = 0 now: host op 78 kill (slot, 0).

### Rest on spare AP (`ja1_turn_rest`, op 152 turn_rest)

Library op 152: wherever the library's code makes op 152 turn_rest (slot), it runs this procedure, unless bit 63 of
host_ops is set. s = record slot. cur = the current animation word u16 +0x1E8 + 2 × +0x1F3. combat.md "Resting on
spare AP" says it in plain words.

1. a = s8 +0x1A5 (the AP left from the last turn). If a ≤ 5: done. n = a − 5.
2. rate = the rate of cur from the table below; −100 for cur ≥ 0x40.
3. If rate ≥ 0 (tiring): f = (25 − n) × rate.
4. Else (resting): if +0x1DB ≠ 0: done. If byte 5 of tile s16 +0x114 has bit 0x80 (gassed; a tile outside
   0..3135 reads 0) and byte +0x24 ≠ 0x28 (no gas mask in item entry 3): done. f = n × rate.
5. Spend AP (s, amount 0, tire = the low 16 bits of f as a signed number) = op 31 spend_ap (slot, 0, f).

Rates by animation word (rest < 0, tire > 0):

| cur | rate | cur | rate | cur | rate |
|---|---|---|---|---|---|
| 0x00..0x0A | −200 | 0x1C..0x1D | −100 | 0x2E | −100 |
| 0x0B..0x0C | −100 | 0x1E..0x1F | −50 | 0x2F | −200 |
| 0x0D | −200 | 0x20..0x21 | −100 | 0x30 | −100 |
| 0x0E | −100 | 0x22..0x23 | −50 | 0x31..0x32 | −50 |
| 0x0F | −50 | 0x24..0x27 | −100 | 0x33..0x38 | −200 |
| 0x10 | −200 | 0x28 | −200 | 0x39 | 25 |
| 0x11..0x14 | −100 | 0x29..0x2A | −100 | 0x3A | 50 |
| 0x15 | −50 | 0x2B | −200 | 0x3B | −100 |
| 0x16..0x19 | −100 | 0x2C | −100 | 0x3C | 50 |
| 0x1A..0x1B | −50 | 0x2D | −50 | 0x3D..0x3F | 100 |

*Evidence (2026-10-25): TurnRest? 0x32D04 (jump table 0x32C04) read again for this section; mirror tools/ai.py
turn_rest (ai_diff turn_rest).*

*Evidence (2026-10-22): mirrors tools/ai.py start_turn, bleed, drown (ai_diff start_turn, bleed, drown; the
original StartTurn? 0x11FB4 read again for this page).*

## Spend AP (`ja1_spend_ap`, op 31 spend_ap)

Every action spends AP and fatigue through this procedure (combat.md "Spending AP and fatigue"). Library op 31:
wherever the library's code makes op 31 spend_ap (slot, amount, tire), it runs this procedure, unless bit 31 of
host_ops is set. Arguments: slot, amount (low byte), tire (low 16 bits). s = record slot. cur = the current
animation word u16 +0x1E8 + 2 × +0x1F3; anim_free[cur] reads 0 for cur ≥ 256.

```c
void ja1_spend_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t tire);
/* the parts, also entry points (tests): */
void ja1_rest(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t amount, int32_t redraw);   /* redraw: low byte */
```

New world fields (port_api.md, "turns"): `weather_tire` (i32, the fatigue modifier of the conditions), `merc_tire`
(i8, the fatigue modifier for mercs and guards). Read too: `selected`, `combat`, `anim_free`, `opt_onscreen`.
New host ops: 154 energy_bar (slot, int redraw): the merc's energy bar changes. 155 merc_panel (byte slot): the
merc's panel is drawn. 156 panel_update (slot): the soldier's panel values change. Existing host op used: 126
move_sound, here with the arguments (byte slot, int 0). None of them returns anything that is read.

**Procedure:**
1. If s8 +0x183 ≤ 0 or +0x188 ≠ 0: done.
2. d = amount (byte). If d ≠ 0x63 and d ≠ 0:
   a. If slot ≠ selected or anim_free[cur] ≠ 0: if s8 d > s8 +0x1A5: d = +0x1A5.
   b. If combat ≠ 0: r = +0x1E2. If s8 d ≤ s8 r: +0x1E2 = r − d (8-bit). Else: +0x1E2 = 0; +0x1A5 = +0x1A5 −
      (d − r) (8-bit each).
   c. If s8 +0x1A5 < 0 and slot = selected: +0x1A5 = 0.
3. t = tire (u16). If t ≠ 0: t = Fatigue scale (s, t) (32-bit).
4. If +0x181 = 0: host op 126 move_sound (slot, 0). Else: if +0x182 = 0 or +0x183 = 0 or +0x138 = 0: done (no Rest);
   else host op 156 panel_update (slot).
5. Rest (s, amount = −(s16 of the low 16 bits of t), as a 32-bit value; redraw 1).

### Fatigue scale (s, amount)

All values are integers; "s16(x)" = the low 16 bits of x read as signed.
1. c = (200 − s8 +0x18A) mod 65536.
2. h = ((+0x184 − +0x183 − +0x185) mod 256) / 2 (the byte, halved down) + s8 +0x183. c = s16(c) + 100 − h.
3. If +0x1B3 = 3 or +0x1B2 ≥ 0x97 or +0x19C = 7: c = s16(c) + cdiv(weather_tire, 5). Else: c = (c + (weather_tire
   mod 65536)) mod 65536.
4. If slot < 0x28: c = c + (s8 merc_tire mod 65536).
5. If +0x19C = 5 and s16 cur is 0x39, 0x3A or 0x3C: c = c × 5.
6. c = s16(c). a = s16(amount). Result: cdiv(a × c, 100) if a > 0, else cdiv(a × 100, c) (c = 0: port_api.md,
   Division by zero).

### Rest (`ja1_rest`)

Fatigue +0x10C (i16) goes down by amount (a negative amount tires); energy +0x18A follows it.
1. f = s16(u16 +0x10C − amount). u16 +0x10C = f if f > 0, else 0. If s16 +0x10C > 10000: u16 +0x10C = 10000.
2. e = (100 − cdiv(s16 +0x10C, 100)) mod 256. If s8 e > 100: e = 100. If s8 e < 0: e = 0.
3. If e ≠ +0x18A: +0x18A = e; if s8 +0x18A ≥ 50: +0x1BA = 0. If opt_onscreen < 2 and the strip slot byte
   +0x17A ≤ 7: host op 154 energy_bar (slot, s8 redraw as a 32-bit value).
4. Else (e = +0x18A): if redraw (byte) ≠ 0 and opt_onscreen < 2 and +0x17A < 8: host op 155 merc_panel (+0x17A).
   Both tests read +0x17A, not the record number (0x327EA, 0x32814; heal_diff fuzzes +0x17A).

*Evidence (2026-10-22): mirrors tools/ai.py spend_ap, fatigue_scale, rest (ai_diff spend_ap, rest).*

## The day clock (`ja1_day_clock`, op 159 guards_arrive)

Every new turn (New turn, step 7a) moves the day clock on by 5 minutes. The tactical day starts at 7:00 and
ends after 12 hours (at 19:00): the result is then 1, New turn passes it on, and the tactical loop ends the day
(soldier_update.md, Leaving the sector: the leave counter). Out of combat a new turn comes every 20 seconds of
real time (turn_clock 20000 ms); with Time compression on, the timer runs 100 times faster. Library op 159: the
library runs this procedure itself unless host_ops bit 63 is set (the op keeps its old name guards_arrive).

```c
int32_t ja1_day_clock(Ja1World *w, const Ja1Host *h, int32_t minutes);
```

New world fields (port_api.md, "day clock"): `clock_minute` (u8, the minutes on the clock), `clock_hour` (u8,
the hour on the clock, 1..12), `day_hours` (s8, full hours since the day began), `day_minutes` (i32, minutes
since the day began), `day_bar` (i32, the length of the day bar), `hard_day` (u8, ≠ 0 on a day of hard weather:
the guide's message of the day sets it), `night_warned` (u8, the merc's evening warning came today), `guard_eta`
(960 bytes: 60 sectors × 8 s16 = the minute of the day at which a hired guard reaches the sector; 9999 = none),
`guard_rec` (480 bytes: 60 × 8, for each entry of `guard_eta` the side the guard comes in from: 1, 3, 5 or 7,
strategic.md The guard screen, Side; the guard screen fills both).
Existing fields used: `day`, `difficulty`, `cur_sector`, `sector_state`, `time_compress`, `opt_onscreen`,
`weather_tire`, `alone_warned`, `redraw_next`, `quote_slot`, `quote_delay`, `quote_event`, `tick_step`,
`soldiers`.

New host ops: 309 day_bar (int time_compress): the front-end draws the day bar (`day_bar` = its length in
steps of 5 %, 0..20). 310 enemy_pick (): result (byte) = the sector where the enemy commander sends troops next
(the strategic layer's own order). 311 enemy_move (byte sector): the strategic layer moves enemy troops to the
sector; result (byte) = the number of soldiers who come into the current sector because of it (0 = none).
312 guard_enter (byte side): a hired guard (side = the `guard_rec` entry) walks into the current sector at its
edge; result (byte) ≠ 0 = the guard came in, 0 = no free slot or no free edge tile (he tries again next time).
313 mode_banner (): the front-end draws the tactical screen again with its current panel. 314 guards_msg (int
count, int number): the status line message "1 Guard arrives in sector N" (count 1; number = sector + 1) or
"COUNT Guards arrive in sector" (count > 1: the original passes an unrelated number here; the port passes
sector + 1 and the front-end does not show it). Existing ops used: 112 tac_redraw (int 1), 89 replay_event,
218 resolve_battle, 43 connected_home (library op).

Tables (constants of the original, per difficulty d = 0, 1, 2): wave_day = 3, 2, 1; wave_period = 4, 3, 2.

**Redraw** (used twice below): if opt_onscreen ≠ 0: host op 112 tac_redraw (1); else host op 313
mode_banner (). (The original also sets a drawing flag of its own when time_compress ≠ 0; that is the
front-end's matter.)

**Procedure** (n = minutes, all 32 bits; arrivals = 0, an 8-bit counter):
1. clock_minute = clock_minute + n (8-bit). day_minutes = day_minutes + n (32-bit wrap).
2. v = (day_minutes × 100 (32-bit wrap)) / 720, then v = v / 5 (C division, truncated toward zero). If v ≠
   day_bar: day_bar = v; host op 309 day_bar (time_compress).
3. If s8 clock_minute > 59:
   a. clock_minute = s8 clock_minute mod 60 (C remainder). clock_hour = clock_hour + 1 (8-bit); if clock_hour =
      13: clock_hour = 1. day_hours = day_hours + 1 (8-bit).
   b. If s8 day_hours ≥ 12: **result 1** (the day is over). Done: none of the steps below runs.
   c. alone_warned = 0. x = |6 − s8 day_hours|. weather_tire = (6 − x) × 25 if hard_day ≠ 0, else (6 − x) × 15.
4. **Enemy waves.** p = 5 × wave_period[difficulty].
   a. If wave_day[difficulty] < day (signed 32-bit) and day_minutes mod p = 0 (C remainder): s = host op 310
      enemy_pick (); arrivals = arrivals + host op 311 enemy_move (s).
   b. If cur_sector = 0 and day_minutes mod p = 0: arrivals = arrivals + host op 311 enemy_move (0).
5. **Hired guards.** If day_minutes mod 5 = 0, for s = 0..59 in order:
   a. count = 0. For j = 0..7: e = 8 s + j. If s16 guard_eta[e] ≤ day_minutes (signed) and (s ≠ cur_sector, or
      host op 312 guard_enter (s8 guard_rec[e] as a 32-bit value) gives a byte ≠ 0): sector_state[9 s + 4] =
      that byte + 1, sector_state[9 s + 5] = that byte − 1 (8-bit each), guard_eta[e] = 9999, count = count + 1.
   b. If count ≠ 0: Redraw; host op 314 guards_msg (count, s + 1); Redraw; host op 89 replay_event (s, s8
      count, 0x5D). Then if s = cur_sector: arrivals = arrivals + count (8-bit); else if bit 0 of
      sector_state[9 s] is clear: host op 218 resolve_battle (s, 1).
6. If arrivals ≠ 0: Time compression (0); redraw_next = 1. Result 0.
7. Else, if s8 day_hours ≥ 11 and night_warned = 0 and op 43 connected_home (cur_sector) = 0 (byte) and
   r = Pick night merc (below) < 8: night_warned = 1; quote_event = 0x20; quote_delay = 2; quote_slot = r
   (the merc remarks that night falls far from home; Merc quote plays it at the next new turn). Result 0.
8. Else result 0.

**Pick night merc** (8-bit result): tried[0..7] = 0. Up to 8 times: draw k = Random(8) again until tried[k] = 0;
if record k has +0x182 ≠ 0, +0x138 ≠ 0, s8 +0x183 ≥ 15 and s8 +0x18A > 10: the result is k (done); else
tried[k] = 1. After 8 failed tries the result is 0x40 (no Random draw for a ninth try).

**Start of the day.** `ja1_enter_sector` step 0 (world_load.md section 4) sets: day_minutes = 0, day_bar = 0,
weather_tire = 0, redraw_next = 0, night_warned = 0, day_hours = 0, clock_minute = 0, clock_hour = 7.

*Evidence (2026-10-23): DayClock 0x1F724 (labels: GuardsArriveMsg), PickNightMerc 0x32828, LoadTacticalSector
0x2AD20 (the start values), tables 0x95C30 / 0x95C3C, guard table 0x10D2B8 / 0x10D0D8 (QuickSave writes both);
validate/diff/clock_diff.py.*

## Time compression (`ja1_time_compress`)

```c
void ja1_time_compress(Ja1World *w, const Ja1Host *h, int32_t on);
```

New host ops: 315 mouse_release () (the front-end waits until the right mouse button is up), 316 refresh () (the
front-end draws the tactical screen and its panel again). Existing ops: 198 mouse_wait, 53 stop_anim (library op).

- on ≠ 0 (all 32 bits): time_compress = 1. For k = 0..7: if record k has +0x182 ≠ 0, +0x138 ≠ 0 and s8 +0x183
  ≥ 15: Stop on the own tile (ai.md) with flag 1. Then host op 316 refresh (); tick_step = 1000.
- on = 0: time_compress = 0; tick_step = 10; host op 198 mouse_wait (); host op 315 mouse_release ().

**The key (front-end).** Key C: if combat = 0: ja1_time_compress (1 if time_compress = 0, else 0); else the
status line shows "Can't compress time, there are enemies nearby!" (nothing else changes). While
time_compress ≠ 0, any other key and any mouse button press only calls ja1_time_compress (0) (the key or click
does nothing else).

*Evidence (2026-10-23): TimeCompress 0x11D40, TacticalKeyHandler 0x13490 (key C, the any-key and mouse tests at
its start); validate/diff/clock_diff.py.*

## Start a timed effect (`ja1_effect_start`, op 91 effect)

Arguments: tile (low 16 bits used), kind (byte), timer (byte), power (int), owner (byte), flag (byte). Result 1
when an entry took the effect, else 0.

1. For f = 0..4, e = entry f of `timed_fx`: if s32 e+0 = −1 exactly (other negative values are not free):
   - e+0 = power (i32); e+4 = power (low 16 bits); e+6 = tile (16 bits); e+8 = kind; e+9 = timer; e+0xA = timer;
     e+0xB = owner; e+0xC = flag.
   - If flag ≠ 0: `fuse_beep_timer` = 700.
   - Result 1.
2. No free entry: result 0 (the effect is lost).

The effect goes off at End turn (step 1) once its timer byte e+9 is 0 or less; New turn step 17 lowers the timer
by 1 each turn.

## Home-sector stages (`ja1_stage_effects`, op 169; `ja1_stage_five`, op 170)

Sector 0 (Santino's base) blows up in stages after Santino speaks (End turn step 20 raises `stage`). The stage
records are game data (tile, kind, stage, delay, lowest difficulty):

| # | tile | kind | stage | delay | lowest difficulty |
|---|---|---|---|---|---|
| 0 | 0x15B | 3 | 0 | 0 | 0 |
| 1 | 0x192 | 3 | 0 | 800 | 0 |
| 2 | 0x1C8 | 3 | 0 | 1600 | 0 |
| 3 | 0x1C6 | 3 | 0 | 2400 | 0 |
| 4 | 0x1FC | 3 | 0 | 3200 | 0 |
| 5 | 0x194 | 3 | 1 | 30000 | 1 |
| 6 | 0x202 | 3 | 1 | 10000 | 2 |
| 7 | 0x274 | 3 | 1 | 2500 | 0 |
| 8 | 0x203 | 3 | 2 | 1000 | 2 |
| 9 | 0x2AA | 3 | 2 | 20000 | 1 |
| 10 | 0x15D | 3 | 2 | 8000 | 0 |
| 11 | 0x159 | 0xA | 3 | 500 | 0 |

**Stage effects** (op 169): for each record in order whose stage = `stage` and whose lowest difficulty (unsigned)
≤ s8 `difficulty`: Start a timed effect (tile, kind, timer 0, power = delay, owner 0x41, flag 0). Then, if
`stage` = 0: host op 183 loss_msg (0, 0xFE, 4) (the original arms its message timer with 7500 ms for merc 0 and
amount 0xFE; part 4 stands for that message).

A record's blast goes off at the next End turn only when its tile holds a ground item of the same blast kind (End
turn step 1); the delay only has to be 0 or more.

**Stage five** (op 170): for k = 0x29..0x2F (not 0x28, Santino): if +0x182 ≠ 0 and +0x138 ≠ 0 and s8 +0x183 ≥ 15
and s8 +0x9C ≤ 0 (no merc sees it) and +0x1C4 = 0: Forget soldier k (combat.md), then host op 82 merc_dies (k, 0).

## Tests

`validate/diff/ai_diff.py --only turn_end --port` runs the five entry points (first merc, turn sightings,
enemies' turn, end turn, mercs' turn) against the original on fuzzed worlds, every op a host call (ops 40, 140,
141 included), and compares the world and the call list. `--only turn_flow --port` does the same for the
interrupt queue.
`--only start_turn --port`, `--only bleed --port` and `--only drown --port` run Start of a soldier's turn and
its parts (Merc view answered by the mirror, Merc busy real on the original side).
`--only spend_ap --port` and `--only rest --port` run Spend AP and Rest.
`--only turn_start --port` runs Turn start and Ally alert (Alert state and Merc view real on both sides; the test
host answers op 194 banner by writing the original's text and colour).
`validate/diff/clock_diff.py --port` runs The day clock and Time compression against the original (3000 fuzzed
cases; the strategic callees are host ops answered from a script).
`validate/diff/stage_diff.py --port` runs Start a timed effect, Stage effects and Stage five against the original
(3000 fuzzed cases: the effect table, the stage, the difficulty and the enemy records).
Live check (2026-10-27, state fight59, build/probe/turnview2.py): a right click ends the waiting item use, then
key D ends the turn. With Hector (slot 1, the selected merc) on the screen the view does not move all turn: the
enemies' turn selects slot 42, then 47, and the mercs' turn shows slot 1 (shown_slot 1, selected 1). With the view
first moved right (Hector off the screen) the view jumps at once to the first enemy to act (slot 42: view 96, 14),
and the mercs' turn leaves it there because Hector is on the screen again. Both match New turn + Focus above.
