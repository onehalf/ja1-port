# Shooting: a projectile from the launch to the impact

How a shot or a throw flies (combat.md "Launching a projectile", "Projectile flight (per frame)", "Projectile
impact"). An attack animation launches the main-hand item (**Launch**, op 81); each frame the projectile moves
(**Projectile step**, op 136, from soldier_update.md "One frame"); it stops on the map edge or where the
projectile test stops it (**Shot end**, then **Projectile impact**). **Fire now** (op 39) ends an attack that
timed out.

Conventions as in soldier_update.md: s = the soldier record of `slot` (its slot is s +0x17A), NONE = 0xC41,
px(t) = (t % 56) × 16 + 5, py(t) = (t / 56) × 14 + 5 (truncated toward zero), vt(x, y) = cdiv(x, 16) + cdiv(y,
14) × 56 (the tile under a pixel), occupant(t), tile_standable(t), t0 / t1 / t2 of a tile = its bytes 0 / 1 / 2.
cdiv = C division (truncation toward zero); "(div_zero when 0)" = the original divides by zero there (the port
may do anything). "Step from a tile" = ai.md "Search procedures" (the leave-map test). "Host op N name (args)" =
a host call (port_api.md). All record values are bytes unless written u16 / s16 / u32 / s32; "s8" = the signed
byte, other byte tests are unsigned. 16-bit fields wrap at 16 bits.

Projectile fields of the record (all u16 unless written): +0xC8 flown pixels, +0xCA height cap (0 = none), +0xCC
(u32) frame timer, +0xDC target tile, +0xDE / +0xE0 end point x / y, +0xE2 end height, +0xE4 / +0xE6 point x / y,
+0xE8 slope, +0xEA down slope, +0xF2 height, +0xF6 shadow x, +0x9E shadow y, +0xF8 error, +0xFA / +0xFC x / y
sign, +0xFE / +0x100 major step x / y, +0x102 / +0x104 error steps; bytes +0x1BC roll flag, +0x1BE attack stage,
+0x1BF projectile state (3 = flying), +0x1C2 second target, +0x1C3 merc-side flag, +0x1C5 shadow sprite, +0x1C6
facing of the shot, +0x1C7 shadow shown, +0x1C9 thrown kind (0 = bullet; > 1 = explosive), +0x1CB tiles per
frame, +0x1CD gun loaded, +0x1D8 attack target slot, +0x1E4 condition of the item, +0x1E6 (u16) the shot's target
tile, +0x1F5 the item, +0x1FB tiles flown, +0x1FF skip distance. The main-hand item entry is record bytes +0..+11:
+0 type, +1 count, +2 condition, +3 attachment, +5 attachment condition, +8.. the conditions of stacked items.

## Entry points

```c
void    ja1_launch(Ja1World *w, const Ja1Host *h, int32_t slot);            /* = library op 81 launch */
void    ja1_projectile_step(Ja1World *w, const Ja1Host *h, int32_t slot);   /* = library op 136 projectile_step */
void    ja1_fire_now(Ja1World *w, const Ja1Host *h, int32_t slot);          /* = library op 39 fire */
/* also entry points (tests): */
void    ja1_shot_end(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t silent, int32_t notify);   /* low bytes */
void    ja1_projectile_impact(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t x, int32_t y,
                              int32_t hit);   /* tile: low 16 bits, signed */
```

Library ops (run by the library unless their host_ops bit is set; bit 63 for op ≥ 63): 81 launch = Launch, 136
projectile_step = Projectile step, 39 fire = Fire now. Shot end, Projectile impact and Throw one are the library's
own procedures, never host calls.

New world fields (port_api.md, "shooting"): `busy_timer` (i32: when it runs out nobody is busy any more; Timer
tick), `replay_m` (u8: the 'M' replay event of this sector is recorded), `first_hit` (u8: a story stage, 1 → 2 when
an enemy is first hit), `fx_id` (u8: the id of the last impact effect). Read-only tables: `silencer` (17 bytes,
one per item 0..16: the loudness of the gun), `shot_sound` (34 bytes, two per item 0..16: sound effect, i8
volume), `impact_noise` (33 bytes, one per item 0..32: the loudness of its impact), `shot_range` (9 bytes, one per
facing 0..8: the pixels a bullet flies before a missing shot may show its shadow). Existing fields used: `busy`,
`selected`, `combat`, `cur_sector`, `story`, `active_slot`, `in_flight` (here: a hit counter), `effect_tile`,
`anim_signal`, `turn_moves`, `interrupt_go`, `item_types` (byte 2 = kind, 5 = wear, 6 = 1: the item does not
stack), `weapons` (byte 4 of item t's row = tiles per frame), `t0_class`, `tiles`.

New host ops: 173 shot_alert (slot, byte flag): the target of the shot reacts. 174 item_cond (int 0, slot, int 0,
int 1): the selected merc's item panel shows the new condition. 175 bullet_damage (slot): result (low 16 bits,
signed) = the damage of the soldier's bullet. 176 land_item (slot, tile, byte item, int condition, int x, int y):
a thrown item lands on the tile. 177 story_hit (): a shot hit the story object. 178 puff (tile, int x, int y, int
look, int 5, int kind, int 1): an impact effect at the point (look 0 = a splash on water ground, 1 = a puff);
result (low byte) = the effect id. 179 door_lookup (tile): result (i32) = the strength of the object on the tile.
Existing host ops used: 48 hit_test, 60 sfx, 66 tile_effect, 89 replay_event, 90 explode, 91 effect, 105
stat_practice, 107 break_object, 110 take_damage, 127 make_noise; library ops 113 merc_interrupt and 114 next_turn
(turns.md).

## Launch (`ja1_launch`, op 81 launch)

1. If s8 +1 < 0: done.
2. u16 +0xE4 = s16 +0x116 + 2; u16 +0xE6 = s16 +0x118 + 4. dist = max(|s16 +0xE4 − s16 +0xDE|, |s16 +0xE6 −
   s16 +0xE0|). u16 +0xC8 = 0. If dist < 2: done.
3. +0x1BF = 3. +0x1F5 = +0. u16 +0x1E6 = u16 +0xDC. +0x1C6 = +0x1A1. +0x1BE = 0. If +0x1C2 = 0xFE: +0x1C2 = 0x40.
   If +0x1C3 ≠ 0: +0x1C3 = 0 and +0x1C2 = 0x40.
4. slope(d, n) = cdiv((−1 if d < 0 else 1) + cdiv(2 × d, n), 2). it = +0x1F5.
   - Thrown (item_types byte 2 of it = 0x95): u16 +0xF2 = 8500. u16 +0xCA = 9000 if +0x1B3 = 3, else 22000.
     u16 +0xE8 = slope(s16 +0xCA − s16 +0xF2, cdiv(dist, 2)). u16 +0xEA = slope(s16 +0xE2 − s16 +0xCA, cdiv(dist,
     2)). +0x1CB = 2. If slot < 8: host op 105 stat_practice (slot, 3, 2, 0); host op 105 stat_practice (slot, 6,
     2, 0).
   - Else: u16 +0xCA = 0. u32 +0xCC = 15. u16 +0xF2 = random(1001) + 4500. u16 +0xE8 = slope(s16 +0xE2 − s16 +0xF2,
     dist). +0x1CB = weapons byte 4 of item +0. If slot < 8: host op 105 stat_practice (slot, 8, 1, 0).
5. dx = u16 +0xDE − u16 +0xE4, dy = u16 +0xE0 − u16 +0xE6 (16-bit). +0x1FB = 1. u16 +0xFA = −1 if s16 dx < 0, else
   1; then dx = −dx if s16 dx < 0 (16-bit). The same for dy with u16 +0xFC. If s16 dx < s16 dy: u16 +0xFE = 0,
   u16 +0x100 = u16 +0xFC, major = dy, minor = dx. Else: u16 +0x100 = 0, u16 +0xFE = u16 +0xFA, major = dx, minor =
   dy. u16 +0x104 = 2 × minor; u16 +0xF8 = 2 × minor − major; u16 +0x102 = 2 × minor − 2 × major.
6. busy = busy + 1. busy_timer = 5000.
7. Shot sound, only if it < 17: if +3 = 0x5B (a silencer): q = cdiv(100, silencer[it] − 1) (div_zero when 0); v =
   u8(u8(cdiv(100 − s8 +5, q) + 1) × 10 + 20) (div_zero when q = 0); host op 60 sfx (0x17, 0x2B11, min(v, 120), 1).
   Else if shot_sound[2 × it] < 0x63: host op 60 sfx (shot_sound[2 × it], 0x2B11, s8 shot_sound[2 × it + 1], 1).
8. If slot < 8 or item_types byte 2 of item +0 = 0x95:
   a. If +0 ≤ 0x0E: n = s8 item_types byte 5 of item +0 × 2; if random(s16 n) = 0: +2 = +2 − 1.
   b. +0x1E4 = +2.
   c. If item_types byte 2 of item +0 = 0x95: Throw one (the entry at +0). Else: +1 = +1 − 1.
   d. If +1 = 0: if +0x1F5 ≤ 0x0E: +0x1CD = 0 and u32 +0xB4 = 2000; else bytes +0..+11 = 0.
   e. If selected = slot: host op 174 item_cond (0, slot, 0, 1).
9. If dist ≤ 20: u16 +0xC8 = dist; u16 +0xE4 = u16 +0xDE; u16 +0xE6 = u16 +0xE0; t = s16 +0xDC; x = s16 +0xE4 − 8;
   y = s16 +0xE6 − 4; Shot end (slot, 0, 0); Projectile impact (slot, t, x, y, 1). (t, x and y are read before Shot
   end sets +0xDC, +0xE4 and +0xE6 to NONE.)

**Throw one** (item entry E, 12 bytes): c = s8 E+1. If c ≤ 0: nothing. If item_types byte 6 of item E+0 = 1, or
c = 1: E+0..E+11 = 0. Else: E+2 = E+8; for k = 1 .. c − 2: E+(7 + k) = E+(8 + k); E+(c + 6) = 0; E+1 = E+1 − 1.

## Shot end (`ja1_shot_end`)

The projectile is over. Arguments silent and notify (bytes).
1. If silent = 0:
   a. If +0x1D8 < 0x30: host op 173 shot_alert (slot, notify).
   b. If +0x1F5 ≤ 0x0E: v = silencer[+0x1F5]; if +3 = 0x5B: v = u8(cdiv(100 − s8 +5, cdiv(100, v − 1)) + 1)
      (div_zero when a divisor is 0). Host op 127 make_noise (slot, s16 +0x114, +0x1B3, v, 5).
2. u16 +0xDC, +0xDE, +0xE0, +0xE4, +0xE6 and +0x1E6 = NONE. +0x1BF = 0, +0x1C7 = 0, +0x1FF = 0. +0x1BC = 1.
3. If s32 busy > 0: busy = busy − 1.

## Projectile step (`ja1_projectile_step`, op 136 projectile_step)

1. If s32 +0xCC > 0: done. u32 +0xCC = 15.
2. prev = vt(s16 +0xE4, s16 +0xE6). n = 0 (a byte). me = s16 +0x114.
3. Repeat:
   a. If s16 +0xF8 < 0: +0xE4 += +0xFE, +0xE6 += +0x100, +0xF8 += +0x104. Else: +0xE4 += +0xFA, +0xE6 += +0xFC,
      +0xF8 += +0x102 (all u16).
   b. t = vt(s16 +0xE4, s16 +0xE6). +0xC8 += 1. +0xF2 += +0xE8; if s16 +0xF2 < 0: +0xF2 = 0. If s16 +0xCA ≠ 0 and
      s16 +0xF2 > s16 +0xCA: +0xF2 = +0xCA, and +0xE8 = +0xEA if +0x1C9 ≠ 0, else 0.
   c. If t ≠ prev and t ≠ me: +0x1FB = +0x1FB + 1. If the step from prev to t leaves the map ("Step from a tile",
      with n = t): Shot end (slot, 0, 1); Fire now (slot); u16 +0xE2 = 0. Done. prev = t.
   d. If t ≠ me:
      - If +0x1FF ≠ 0 and +0x1FF ≤ u16 +0xC8: +0x1FF = 0.
      - r = host op 48 hit_test (s16 +0xF2, s16 +0xE4, s16 +0xE6, s16 +0xC8, +0x1C6, +0x1FF, s16 +0xCA, s16 +0xDC,
        s8 +0x1BC) (32 bits); +0x1FF = the new skip (out byte +0).
      - If r ≠ 0: x = s16 +0xE4 − 8; y = s16 +0xE6 − 6 − cdiv(s16 +0xF2, 600); Shot end (slot, 0, 0); Projectile
        impact (slot, t, x, y, r). Done.
   e. n = n + 1. If n < s8 +0x1CB, or t = me: go on with a. Else leave the loop.
4. Shadow. If +0x1C9 = 0, and s16 +0xE4 = NONE, or s8 shot_range[s8 +0x1A1] ≥ s16 +0xC8, or t1 of t ≥ 0x27, or
   t2 of t ≥ 0x10: +0x1C7 = 0. Done.
5. +0x1C7 = 1. u16 +0xF6 = u16 +0xE4. h = s16 +0xF2. If +0x1C9 ≠ 0 and h > 18000: u16 +0x9E = u16 +0xE6 − 18.
   Else: u16 +0x9E = s16 +0xE6 − cdiv(h, 1000 if +0x1C9 ≠ 0 else 400).
6. q = cdiv(h, 1000). If q ≤ 2: +0x1C5 = 0. Else if q ≤ 22: +0x1C5 = 0xD2 + (q − 3) / 2. (Else unchanged.)

## Fire now (`ja1_fire_now`, op 39 fire)

The attack ends (an attack that timed out, or a shot that left the map).
1. If +0x1D6 ≠ 0 and +0x1D1 is 0x0B, 0x0C, 0x0E or 0x10: +0x1D1 = 0; u16 +0x132 = NONE; +0x1D6 = 0.
2. u16 +0xDC = NONE. active_slot = 0x40.
3. If turn_moves ≠ 0: op 114 next_turn (1). Else if interrupt_go ≠ 0: op 113 merc_interrupt ().

## Projectile impact (`ja1_projectile_impact`)

Arguments: tile t (s16), point x, y (32-bit), hit (32-bit: the projectile test's result; −1 = a direct hit).
1. active_slot = slot. it = +0x1F5. vol = 0x58. loud = impact_noise[it] if it < 33, else random(3). Host op 127
   make_noise (slot, t, t0_class[t0 of t], u8 loud, 4).
2. o = occupant(t). c = s8 +0x1E4.
   - **Roll**: true when c ≥ 10 and random(100) − random(50) < c (no random draws when c < 10).
   - **Effect**: r = random(6500); f = random(2); host op 91 effect (t, +0x1C9, f, r − 2000 × s8 combat, slot, 0).
   - **Explosive**: if hit = −1 or Roll: host op 90 explode (slot, t, s8 +0x1C9, hit); done. Else: host op 176
     land_item (slot, t, it, c, x, y); then if Roll: Effect.
3. If o ≠ 0x40 (a soldier on the tile; record O):
   a. If combat ≠ 0: effect_tile = t; host op 66 tile_effect (t, 0).
   b. in_flight = in_flight + 1.
   c. If O +0x17A ≥ 0x28 or slot ≥ 0x28: if replay_m = 0: replay_m = 1 and host op 89 replay_event (s8
      cur_sector, s8 cur_sector, 0x4D). Then if O +0x17A ≥ 0x28 and s8 first_hit = 1: first_hit = 2.
   d. If +0x1C9 ≠ 0: if s8 +0x1C9 > 1: Explosive. Else: host op 110 take_damage (o, 2500, random(4) + 1, 1, slot,
      NONE); host op 176 land_item (slot, t, 0x5D, 100, x, y). Done.
   e. d = s16 of host op 175 bullet_damage (slot). Host op 110 take_damage (o, s16 +0xF2, d, 1, slot, NONE). If
      slot < 8: host op 105 stat_practice (slot, 8, 1, 0). Done.
4. No soldier: st = tile_standable(t).
   a. If s8 +0x1C9 > 0: if s8 +0x1C9 > 1: Explosive, then if combat ≠ 0: effect_tile = t and host op 66
      tile_effect (t, 0). Else: host op 176 land_item (slot, t, 0x5D, 100, x, y). Done.
   b. If slot ≥ 0x28 and +0x1D8 < 8: host op 105 stat_practice (+0x1D8, 2, 1, 0).
   c. If cur_sector = s8 story[0x17] and story[0x15] = 0 and story[0x16] = 0 and t1 of t = 0xAE: host op 177
      story_hit ().
   d. If t2 of t ≥ 0x10 and s16 +0x116 ≤ 0: fx_id = 0. Else:
      - If t1 of t = 0 and t0_class[t0 of t] ≥ 8: r = low byte of host op 178 puff (t, x, y, 0, 5, s8 +0x1C9, 1);
        sound = 0x11.
      - Else: if st < 0: x = px(t) − 8, y = py(t) − 7. r = low byte of host op 178 puff (t, x, y, 1, 5, 0, 1). If
        t1 of t ≠ 0: sound = random(3) + 0x14 and vol = 0x28; else sound = 0x13.
      - fx_id = r. anim_signal = s8 r. Host op 60 sfx (sound, 0x2B11, vol, 1).
   e. If st < 0: v = host op 179 door_lookup (t) (i32); if v > 100: v = v − 70. If v < s16 of host op 175
      bullet_damage (slot), and then random(3) = 0: host op 107 break_object (t, t1 of t, 1); +0x9D = 1; host op
      105 stat_practice (slot, 8, 2, 0).

*Evidence (2026-10-22): mirrors tools/ai.py launch_projectile (LaunchProjectile 0x58DE4), shot_end (ShotEnd
0x56DD4), projectile_step (ProjectileStep 0x57BB8), fire_now (FireNow? 0x57B34), projectile_impact
(ProjectileImpact 0x5713C), use_throw (UseThrow? 0x535E8), diff-tested in ai_diff.py (launch, w_56dd4, pstep,
fire, impact: 12,000 cases each, 0 mismatches).*

## Batch 2: taking damage, the hit reaction, the hit alert

combat.md "Taking damage" and "Reaction to a hit". A hit soldier loses health (**Take damage**, op 110), shows the
damage, plays a hit animation and loses AP; when the attacker is on the other side, the hit soldier may notice it
(**Hit reaction** → **Hit alert** → **Mark seen**).

```c
void    ja1_take_damage(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage, int32_t kind,
                        int32_t attacker, int32_t tile);    /* = library op 110 take_damage; kind, attacker: low byte;
                                                               tile: low 16 bits */
/* also entry points (tests): */
void    ja1_hit_reaction(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t attacker, int32_t flag, int32_t damage);
void    ja1_hit_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t flag);
void    ja1_mark_seen(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent);
```

(attacker, flag, damage of Hit reaction and flag of Hit alert: low bytes. opponent = a record index 0..47.)
Library op 110 take_damage = Take damage (host_ops bit 63, as every op ≥ 63). Hit reaction, Hit alert and Mark
seen are the library's own procedures, never host calls.

New world fields (port_api.md, "shooting batch 2"): `dmg_text` (240 bytes, 5 per slot: the damage number shown
over the soldier, a text with a 0 byte), `marked_slot` (u8: a soldier that the front-end marks; 0x40 = none),
`knockdown` (u8: the next bullet hit on a standing soldier knocks it down), `focus_last` (u8: the soldier the
view went to last), `hit_scale` (u8: damage / 10 while a hit reaction runs, else 0). Existing fields used:
`path_preview`, `selected`, `follow_all`, `spotted`, `focus_slot`, `knowledge`, `remember_m`, `remember_e`,
`last_seen_m`, `last_seen_e`, `anim_rows`, `soldiers`.

New host ops: 180 hit_location (slot, int height, int damage): result (i32) = the damage after the hit location
and the armour (combat.md "Hit location and armour"). 181 knockdown_anim (slot, byte 0x28, byte 0x2B, byte 4):
none (the soldier starts the knockdown animation). 182 interrupt_check (slot, slot opponent, byte old flag): none
(combat.md "Interrupts": the soldier may take an interrupt turn). Existing host ops used: 6 direction, 7
tile_dist, 49 sight_line, 50 end_task, 52 set_anim, 55 ui_refresh, 58 reserve_ap, 60 sfx, 65 effect_slot, 99
focus, 100 cry, 101 voice, 105 stat_practice, 127 make_noise, 135 portrait, 153 wound_bar, 163 cursor_reset;
library op 31 spend_ap and the procedure Rest (turns.md), Frame delay (ai.md), Forget the attack
(soldier_update.md Helpers).

### Take damage (`ja1_take_damage`, op 110 take_damage)

Arguments: height (32-bit), damage d (i32), kind k (1 = a bullet, 2, 3, 4, 6: other hits), attacker a (0x40 =
none), tile (u16). cur = the current animation word u16 +0x1E8 + 2 × +0x1F3, read once at the start.
1. extra = 0 (a byte). Host op 55 ui_refresh (1 if path_preview ≠ 0, else 0).
2. If s8 +0xBA = 4: done (this soldier takes no damage).
3. prev = s8 +0x184 − s8 +0x183 − s8 +0x185. +0x1BE = 0. If marked_slot = slot: marked_slot = 0x40.
4. Queued aim. If +0x1E1 = 0:
   - cur 0x1B → +0x1E1 = 0x1A; cur 0x1F → 0x1E; cur 0x23 → 0x22; cur 5 → 6; cur 0x0E or 0x0F: t = s16 +0x114;
     u16 +0x126 = u16 +0x116 = px(t); u16 +0x128 = u16 +0x118 = py(t); +0x1E1 = 0x0E. Any other cur: nothing.
   - If +0x1E1 ≠ 0 now: q = +0x1E1; if anim_rows[8 × q] ≠ 0x63: +0x1E3 = +0x1E3 + anim_rows[8 × q] (8-bit).
5. If k = 1 or k = 4: v = 10000 − s16 +0x10C. r = cdiv(50 × d × v, 10000) (the product in 32 bits, wrapping).
   Rest (slot, −(s16 of r), 1). If height ≠ 0x1356: d = host op 180 hit_location (slot, height, d) (i32).
6. +0x195 = 1.
7. If d ≠ 0:
   a. Write the decimal text of −d (a '-' first when −d < 0) and then a 0 byte at dmg_text[5 × slot], one byte
      each. (Bytes past the end of dmg_text are not written.)
   b. +0x1D2 = 0x28. u32 +0xC4 = 10. u16 +0xD8 = 0xFFFC. u16 +0xDA = 2.
   c. If s8 +0x183 < d: d = s8 +0x183. +0x183 = +0x183 − d (8-bit). If slot ≥ 0x28 and s8 +0x183 < 15 and +0x183 ≠
      0: +0x183 = 0.
   d. If slot < 8: host op 105 stat_practice (slot, 9, s16 of d, 0).
   e. +0x185 = +0x184 − (+0x183 + prev) (8-bit). u16 +0x1F0 = cdiv(s8 +0x183, s8 +0x1F2 + 10) + 1 (div_zero when
      s8 +0x1F2 = −10). +0x189 = 7. Host op 135 portrait (slot, 1).
   f. If s8 +0x183 < 15: u16 +0xD4 = 0; Forget the attack (0); if s8 +0x186 ≥ 0: +0x186 = 1; +0x181 = 0; +0x1AC =
      1 if s8 +0x183 < 10, else 0; if selected = slot: host op 163 cursor_reset ().
   g. Host op 153 wound_bar (slot, 1).
   Else (d = 0): dmg_text[5 × slot] = 0.
8. +0x1C1 = 1. u16 +0xDC = NONE. If +0x19C ≠ 3: u16 +0xD0 = NONE.
9. Hit animation. hk = 2. By s16 of cur ("anim (c, keep)" = op 52 set_anim (slot, c, keep, 1)):
   - s16 cur > 0x31: if k = 2: +0x1E1 = 0, +0x195 = 0, anim (0x3C, 0), hk = 3. Else if s16 cur ≤ 0x35: anim
     (0x35, 1). Else if ≤ 0x38: anim (0x38, 1). Else if < 0x40: anim (0x3B, 1). Else nothing.
   - cur 0, 2, 0x29, 0x2A or 0x2F: +0x1E1 = 0; if k = 1: anim (0x2F, 0); else anim (0x29, 0) and hk = 3.
   - cur 0x2B: hk = 3; +0x1E1 = 0.
   - cur 1, 3 or 0x0D: +0x1E1 = 0; if k = 1: anim (0x30, 0); else anim (3, 0) and hk = 3.
   - Any other cur (a standing soldier; also a negative s16 cur):
     a. k = 1: if knockdown ≠ 0 and s8 +0x183 ≥ 15: host op 181 knockdown_anim (slot, 0x28, 0x2B, 4); extra = 4;
        knockdown = 0. Else: if s8 +0x183 < 15 and a < 0x40: A = the record of a (this record itself when a =
        slot); +0x1A2 = low byte of host op 6 direction (s16 +0x116, s16 +0x118, s16 A +0x116, s16 A +0x118). Then
        anim (0x28, 1).
     b. k = 3: +0x195 = 0.
     c. Any other k: if d > 0x28 or tile = u16 +0x114: anim (0x2C, 0); else if d > 5: anim (0x2A, 0); else +0x195
        = 0. extra = 4. If d > 5 and +0x1B9 ≠ 0: host op 50 end_task (slot, 0).
     d. Then, only if k = 3 or k = 6: hk = 3; and if +0x183 = 0 and the current animation word (read again now) ≥
        4, or k = 6 and d = 1: anim (0x2B, 1), and if +0x1B9 ≠ 0: host op 50 end_task (slot, 0).
10. If k ≠ 3: host op 65 effect_slot (slot, hk).
11. q = the integer part of √d when d ≥ 0, else −0x80000000. v = low byte of (q − cdiv(s8 +0x192, 2)). If s8 v >
    0: host op 127 make_noise (slot, s16 +0x114, +0x1B3, s8 v, 7).
12. If hk = 2: host op 60 sfx (0x20, 0x2B11, 0x32, 1).
13. Frame delay (ja1_anim_delay). u32 +0xB0 = s16 +0xD2 (sign-extended).
14. ap = low byte of (cdiv(d, 4) + extra). If s8 ap > s8 +0x1A5: ap = +0x1A5. Op 31 spend_ap (slot, s8 ap, 0).
15. If slot < 8: op 58 reserve_ap (slot).
16. If a < 0x40: +0x1DA = a; Hit reaction (slot, a, 1, low byte of d).

### Hit reaction (`ja1_hit_reaction`)

Arguments: attacker a (record index), flag b, damage (byte).
1. mine = slot < 0x28; theirs = soldiers[a] +0x17A < 0x28. If mine = theirs, or s8 +0x183 < 15, or +0xBA = 2:
   done.
2. If s8 byte (+0x112 if mine, else +0x13A) + a = 1: done.
3. hit_scale = damage / 10 (unsigned). Hit alert (slot, a, b). hit_scale = 0.

### Hit alert (`ja1_hit_alert`)

Arguments: opponent i (record index; O = its record, o = O +0x17A), flag (byte).
1. If slot < 8: if flag ≠ 0: host op 99 focus (slot, 0); host op 100 cry (+0x179, −1, 0 = "BAD", 1, 1); host op
   101 voice (0x2B11, 0x58, 1). Then host op 105 stat_practice (slot, 9, 1, 0).
2. mine = slot < 0x28. If mine = (o < 0x28): done.
3. f = byte (+0x112 if mine, else +0x13A) + o of this record; old = f. tell = 0 if mine and s8 knowledge[o] is 1,
   2 or −1; else 1.
4. seen = 0. If host op 7 tile_dist (s16 +0x114, s16 O +0x114) (i32) ≤ 14: seen = (host op 49 sight_line (slot,
   s16 +0x116 + 2, s16 +0x118 + 4, s16 O +0x114, s16 O +0x116 + 2, s16 O +0x118 + 4, 1, 0x0E, o) ≠ 0) (32 bits).
5. If seen: Mark seen (slot, i). Then if mine: if s8 O +0x9C > 0 (read after Mark seen) or follow_all ≠ 0:
   spotted = o, focus_last = o, focus_slot = o, host op 99 focus (o, 0). knowledge[o] = 1 (when mine).
6. Else: +0x1C1 = 1. If s8 +0xBB < 2: +0xBB = 2 and Orders bump. If s8 f ≠ 2: f = 0xFF. If not mine: u16
   last_seen_e[(slot − 0x28) × 0x50 + 2 × o] = u16 O +0x114. If mine: u16 last_seen_m[slot × 16 + 2 × (o − 0x28)]
   = u16 O +0x114; then if tell = 1 and slot < 8: knowledge[o] = 0xFF.
7. Host op 182 interrupt_check (slot, i, old).

**Orders bump** (BumpOrders 0x6F29C; +0x1D4 = debug page "orders"; was "Orders bump"): if +0x1D4 < 2, and not (s8 +0xBA = 3 and +0x1D4 = 0 and s8 +0x18F = 99): +0x1D4 = +0x1D4 + 1.

### Mark seen (`ja1_mark_seen`)

The soldier sees opponent i now (ai.md "mark seen"). O = record i, b = O +0x17A.
1. Only if slot < 0x30 and b < 0x30, +0x182, +0x138 and +0x183 of both records are ≠ 0, one of slot and b is
   below 0x28 and the other is 0x28 or more, and s8 +0xBA ≠ 2. Else: done.
2. If b < 0x28: j = b, R = remember_e byte (slot − 0x28) × 0x28 + b, L = last_seen_e u16 at (slot − 0x28) × 0x50
   + 2 × b. Else: j = b − 0x28, R = remember_m byte slot × 8 + j, L = last_seen_m u16 at slot × 16 + 2 × j. (The
   opponent's slot byte b, not the record index i; the tile in step 3 comes from record i.)
3. old = +0x13A + j; +0x13A + j = 1. last = L; L = u16 O +0x114.
4. If old ≠ 1: +0x1D5 = +0x1D5 + 1; if s8 +0xBB < 2: Orders bump; +0xBB = 3; if s8 old ≠ 2 or last ≠ u16 O +0x114:
   +0x1C1 = 1; if +0x1D1 is 3 or 4: +0x1C1 = 1.
5. R = 0xFF if R = 0, else 1. If s8 O +0x9C ≤ 0: O +0x9C = 1.

*Evidence (2026-10-22): mirrors tools/ai.py take_damage (TakeDamage 0x392FC), hit_reaction (0x74304), hit_alert
(0x743E8), mark_seen (MarkSeen 0x732FC), _bump_trait (0x6F29C), diff-tested in ai_diff.py (take_damage,
hit_reaction, hit_alert, mark_seen; 0 mismatches).*

## Batch 3: bullet damage, hit location, shot alert, interrupt check

combat.md "Damage of a bullet", "Hit location and armour" and "Interrupts". These four were host ops until now;
they become library ops (the library runs them unless their host_ops bit is set; all four use bit 63).

```c
int32_t ja1_bullet_damage(Ja1World *w, const Ja1Host *h, int32_t slot);                     /* = library op 175 */
int32_t ja1_hit_location(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t height, int32_t damage);  /* op 180 */
void    ja1_shot_alert(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);           /* = library op 173 */
void    ja1_interrupt_check(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t opponent, int32_t old);   /* op 182 */
```

(flag, old: low bytes; opponent = a record index.) New host ops: 183 loss_msg (slot, byte amount, int part): none
(the stat-loss message: part 0 = no text, 1 = "head" / "wisdom", 2 = "arm" / "dexterity", 3 = "leg" /
"agility"). 184 merc_sees (byte slot, byte how, byte near): none (the merc reacts to what it sees; how 1 = a
shooter it does not see). Host ops 116 ask_interrupt and 128 ask_merc_interrupt now have a result: low byte ≠ 0
= the soldier wins the interrupt. Other ops used: 7 tile_dist, 13 move_ap, 34 share_sightings, 49 sight_line, 105
stat_practice; library ops 113 merc_interrupt and 114 next_turn (turns.md); Mark seen and Orders bump (batch 2).
Fields used: `item_types`, `weapons`, `absorb`, `knockdown`, `knowledge`, `last_seen_m`, `last_seen_e`,
`interrupted`, `interrupt_go`, `turn_moves`, `selected`, `kept_merc`, `tiles`, `soldiers`.

### Bullet damage (`ja1_bullet_damage`, op 175 bullet_damage)

Result (i32, the low 16 bits are read): the damage of the soldier's projectile.
1. g = +0x1F5. If g < 1 or g > 14: result 0.
2. r = s16 of (item_types[12 × g + 0x0B] × 15). If r = 0: result 0.
3. fall = s16 of cdiv(s16 +0xC8 × 50, r). roll = s8 of (random(0x33) − 0x19). m = s8 of (+0x1FC / 2) (unsigned
   division).
4. d = s16 of cdiv(weapons[6 × g + 5] × (100 − fall + roll + m), 100). Result max(d, 1).

### Hit location (`ja1_hit_location`, op 180 hit_location)

Arguments: height (i32), damage d (i32). Result (i32): the damage after the body part and the armour.
1. r = random(100) + 1 (always drawn first). If d = 0: result 0.
2. a0 = s16 of the current animation word. If (a0 ≤ 3 or a0 ≥ 0x39) and height ≤ 1000: height = height × 6.
3. If height > 6000 or height < 0: result 0.
4. **Armour piece** (entry e, absorb offset A, wear offset W; t = byte 12 × e of the record, c = byte 12 × e + 2):
   a = s16 absorb at byte A; o = r − s8 c; if o > 0: a = max(a − o, 0). a = min(a, d). If a ≠ 0: wear = s16
   absorb at byte W; c = c − cdiv(wear × cdiv(a × 50, 100), 100) (8-bit). d = d − a. The piece absorbed a.
   A helmet: A = 0x1E + 2 × t, W = 2 + 2 × t. Body armour: A = 2 × t, W = 2 × t − 10.
5. **Stat loss** (byte offset f): if random(s16 of d) + 1 ≤ 20: no loss. Else n = random(s16 of cdiv(d, 2)) + 1;
   if s8 byte f − n < 1: n = s8 byte f − 1. Byte f = byte f − n (8-bit). The loss is n (it may be 0 or less: it
   still counts as a loss).
6. **Message** (part, amount): if part = 0 or slot < 8: host op 183 loss_msg (slot, amount, part).
7. Head (height ≥ 5000): a = 0; if +0x18 ≠ 0: a = Armour piece (2, helmet). If a = 0: d = d + cdiv(d × 50, 100);
   Stat loss (0x18D); with a loss n: Message (1, n); else Message (0, 0xFF).
8. Torso (height ≥ 2500): total = 0. If +0x48 = 0x31: total += Armour piece (6, body). If +0x3C ≠ 0: total +=
   Armour piece (5, body). If total = 0: Stat loss (0x18C); with a loss n: Message (2, n).
9. Legs (below 2500): d = d − cdiv(d × 50, 100). Stat loss (0x18B); with a loss n: Message (3, n), and if 4 ≤ a0
   < 0x29 and a0 ≠ 0x0D: knockdown = 1.
10. Result max(d, 0).

### Shot alert (`ja1_shot_alert`, op 173 shot_alert)

The target of the soldier's shot (k = +0x1D8, record O, o = O +0x17A) notices it. Argument flag (byte).
1. If k ≥ 0x40: done. If O +0x182 = 0 or O +0x138 = 0 or s8 O +0x183 < 15: done.
2. If s8 O +0xBA = 2: O +0x1C4 = 1; done.
3. If slot < 0x28 and o < 0x28, or slot ≥ 0x28 and o > 0x28: done.
4. news = 1. If o < 0x28: if s8 knowledge[slot] is 1, 2 or −1: news = 0; f = byte O +0x112 + slot. Else: f = byte
   O +0x13A + slot. old = f.
5. q = 0x40. If flag = 0: q = occupant(vt(s16 +0xE4, s16 +0xE6) as a 16-bit tile); if q ≠ 0x40: record q +0x1C4 =
   1.
6. t = s16 O +0x114. near = s16 of max(|s16 +0x116 + 2 − (px(t) + 2)|, |s16 +0x118 + 4 − (py(t) + 4)|). If s16
   +0xC8 < near − 0x2D, or flag = 0 and q ≠ 0x40: done.
7. O +0x1C4 = 1. seen = 0. If host op 7 tile_dist (t, s16 +0x114) (i32) ≤ 14: seen = (host op 49 sight_line (k,
   s16 O +0x116 + 2, s16 O +0x118 + 4, s16 +0x114, s16 +0x116 + 2, s16 +0x118 + 4, 1, 0x0E, slot) ≠ 0).
8. If seen: Mark seen (k, slot) (O sees this soldier). If o < 0x28: host op 34 share_sightings (k, 1).
9. Else: O +0x1C1 = 1. If s8 O +0xBB < 2: O +0xBB = 2 and Orders bump (O). If s8 f ≠ 2: f = 0xFF. If o ≥ 0x28: u16
   last_seen_e[(o − 0x28) × 0x50 + 2 × slot] = u16 +0x114. Else: u16 last_seen_m[o × 16 + 2 × (slot − 0x28)] =
   u16 +0x114; then if news = 1 and o < 8: host op 105 stat_practice (k, 9, 1, 0); host op 184 merc_sees (o, 1,
   0x40); knowledge[slot] = 0xFF.
10. Interrupt check (k, slot, old).

### Interrupt check (`ja1_interrupt_check`, op 182 interrupt_check)

Arguments: opponent i (record O, o = O +0x17A), old (byte: the old sight flag on the opponent).
1. If s8 +0xBA = 2, or +0x18A = 0, or the current animation word < 4: done.
2. If o ≥ 0x28: mine = byte +0x112 + o, theirs = byte O +0x13A + slot. Else: mine = byte +0x13A + o, theirs = byte
   O +0x112 + slot.
3. If s8 mine = 1: if s8 old is 1 or 2: done; need = low byte of host op 13 move_ap (slot). Else: if s8 theirs ≠
   1, or s8 +0x1D5 > 0, or s8 old = −1: done; need = 1.
4. If s8 +0x1A5 < need (need unsigned): done.
5. interrupted = 1.
6. If slot < 0x28: if the low byte of host op 128 ask_merc_interrupt (slot, i) = 0: done. If interrupt_go ≠ 0: op
   113 merc_interrupt (); done. If turn_moves = 0: done.
   Else (slot ≥ 0x28): if the low byte of host op 116 ask_interrupt (slot, i) = 0, or turn_moves = 0: done. If
   selected < 8: kept_merc = selected.
7. Op 114 next_turn (1).

*Evidence (2026-10-22): mirrors tools/combat.py bullet_damage (BulletDamage 0x39FD8), hit_location_damage
(HitLocationDamage 0x38D80; damage_diff.py, 4000 cases, 0 mismatches) and tools/ai.py shot_alert (0x746D4),
interrupt_check (0x73AB4) (ai_diff.py, 12,000 cases each, 0 mismatches).*

## Batch 4: death

combat.md "Death". **Kill** (op 78 kill) ends a soldier's life (or its stay on the board): its items fall, the
others forget it, the death animation starts. **Merc dies** (op 82 merc_dies, at the end of the death animation or
at once for a quiet death) takes it off the board.

```c
void    ja1_kill(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);        /* = library op 78 kill */
void    ja1_merc_dies(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t quiet);   /* = library op 82 merc_dies */
/* also entry points (tests): */
void    ja1_forget_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
```

(quiet: 32 bits, ≠ 0 = quiet.) New world fields (port_api.md, "death"): `dead_mercs`, `dead_allies`, `dead_npc`,
`enemy_deaths` (i32 each: deaths of this day: mercs, other allies, guards and native mercs, enemies),
`squad_lost` (u8: no merc of the squad is left in the sector), `last_killer` (u8: the merc that killed last).
Existing fields used: `selected`, `view_hold`, `cur_sector`, `sector_state`, `sectors`, `team_on_duty`,
`speaker`, `merc_to_show`, `mission_slot`, `mission_target`, `mission_step`, `listed`, `ground_items`, `notes`,
`knowledge`, `remember_m`, `remember_e`, `last_seen_m`, `last_seen_e`, `last_spotter`, `tiles`, `soldiers`.

New host ops: 185 click_panel (slot, int 0): none (the merc panel updates). 186 death_show (slot, int quiet): none
(the merc's death portrait, the death sound of sound.md "Requests" and, for quiet = 0, the message box
"NAME is dead!" for 800 ms as op 57 merc_text; the front-end also hides its path marks). 187
sector_won (): none (the last enemy of the sector died). 188 squad_lost (): none (the squad in the sector is
gone). Other ops used: 1 idle, 19 mission_cancel, 49 sight_line, 50 end_task, 52 set_anim, 53 stop_anim, 85
cursor_back, 89 replay_event, 103 quote_event, 105 stat_practice, 126 move_sound, 143 deselect, 166 item_panel;
the procedures Rest (turns.md), Put item and Listed (sector_start.md batch 3 and 4).

**Drop entry** E (12 bytes of a record): all 12 bytes = 0.

### Kill (`ja1_kill`, op 78 kill)

1. If selected = slot: host op 85 cursor_back ().
2. If s8 +0x186 = −1: done.
3. u16 +0x126 = u16 +0x116; u16 +0x128 = u16 +0x118. If selected = slot: view_hold = 0.
4. +0x185 = 0, +0x184 = 0, +0x183 = 0. +0x1A5 = 0. u16 +0x10C = 10000.
5. If quiet = 0: op 53 stop_anim (slot, 1); Rest (slot, 0, 0); host op 50 end_task (slot, 1). Else: +0x1B9 = 0.
6. +0x17F = +0x181 = +0x189 = +0x1D5 = 0.
7. If selected = slot and quiet = 0: host op 185 click_panel (slot, 0).
8. Forget soldier (slot).
9. c = cur_sector (u8); sc = s8 cur_sector; row = 9 × c (sector_state); E_i = record bytes 12 i .. 12 i + 11.
   - slot < 8: u32 +0x1DC = 0. +0x189 = 0. If quiet = 0: host op 126 move_sound (slot, 1). sector_state[row + 2]
     −= 1 (8-bit). Host op 89 replay_event (sc, s8 +0x17A, s8 +0x179). t = s16 +0x114. For i = 0..11: if quiet
     = 0: Put item (t, E_i, b6 1, exact 0, note 0, near 0), then Drop entry E_i; else: if Listed (byte 0 of E_i)
     = 0: Drop entry E_i. Then if quiet = 0 and (selected = slot, or selected ≥ 8 and slot = merc_to_show): host
     op 166 item_panel (slot, 1, 1). team_on_duty −= 1. If +0x179 ≥ 0x3C: speaker = 0x40 and dead_npc += 1. Else:
     dead_mercs += 1.
   - 8 ≤ slot ≤ 15: sector_state[row + 4] −= 1. dead_npc += 1. Host op 89 replay_event (sc, sc, 0x52).
   - 16 ≤ slot ≤ 0x27: sector_state[row + 3] −= 1. dead_allies += 1. Host op 89 replay_event (sc, sc, 0x53).
   - slot ≥ 0x28: if mission_slot = slot: mission_slot = 0x40, and if s8 mission_step > 0 and s16 mission_target
     ≠ NONE: host op 19 mission_cancel (). t = s16 +0x114. For i = 7..11: if i = 9 and +0x6C = sectors[0xB1 × c +
     0x9E]: sectors[0xB1 × c + 0x9E] = 0. Put item (t, E_i, b6 1, exact 0, note 1, near 0); Drop entry E_i. Then
     **Enemy died**: for k = 0..7: if record k has +0x182 ≠ 0, s8 +0x183 ≥ 15, its current animation word ≥ 4 and
     +0x138 ≠ 0: record k +0x9D = 1.
10. +0x186 = 0xFF.
11. If quiet ≠ 0: Merc dies (slot, 1) (op 82); done.
12. +0x195 = 0. Op 52 set_anim (slot, 0x40 if In water else 0x43, 0, 1) (In water: +0x1B2 ≥ 0x6C and +0x1B4 is
    neither 0x1C nor 0x1F).
13. k = +0x1DA. If k < 8: record k +0x1D9 += 1 (8-bit; this record when k = slot); host op 105 stat_practice (k,
    9, s16 of (s8 +0x192 × 5), 0); last_killer = k.

### Forget soldier (`ja1_forget_soldier`)

The soldier leaves the board; everybody forgets it.
1. u16 +0x108 = NONE.
2. For k = 0..47: for f in +0x1D8, +0x1F7, +0x1C2 of record k: if it = slot: it = 0x40.
3. If slot < 0x28: for k = 0x28..0x2F (record K): if s8 K +0x13A + slot = 1: K +0x1D5 −= 1 (8-bit). K +0x13A +
   slot = 0. u16 last_seen_e[0x50 × (k − 0x28) + 2 × slot] = NONE. remember_e[0x28 × (k − 0x28) + slot] = 0. Then
   if slot < 16: Forget tiles (this record).
4. Else: for k = 0..15 (record K): if s8 K +0x112 + slot = 1: K +0x1D5 −= 1. K +0x112 + slot = 0.
   remember_m[8 × k + slot − 0x28] = 0. u16 last_seen_m[16 × k + 2 × (slot − 0x28)] = NONE. Then Forget tiles (this
   record). If last_spotter = slot: last_spotter = 0x40.
5. knowledge[slot] = 0.

**Forget tiles** (record S): if s8 S +0xBA = 2: nothing. s = S +0x17A. If s < 0x28: u16 last_seen_m[16 × s + 2 j]
= NONE for j = 0..7. Else: u16 last_seen_e[0x50 × (s − 0x28) + 2 j] = NONE for j = 0..0x27.

### Merc dies (`ja1_merc_dies`, op 82 merc_dies)

1. If +0x188 = 1: done.
2. +0x195 = 0. +0x1AC = 4. +0x185 = +0x184 = +0x183 = 0. +0x1A5 = 0. u16 +0x10C = 10000.
3. If slot < 8: host op 186 death_show (slot, quiet). If sector_state[9 × cur_sector + 2] = 0: squad_lost = 1;
   then for k = 0..7, k ≠ slot: if record k +0x182 ≠ 0 and +0x138 ≠ 0: squad_lost = 0, stop.
4. Else: +0x182 = 0. c = cur_sector. If slot ≥ 0x28 and sector_state[9 × c + 1] ≠ 0:
   a. enemy_deaths += 1. n = sector_state[9 × c + 1] − 1 (8-bit); sector_state[9 × c + 1] = n.
   b. If n = 0: host op 187 sector_won (). Else if +0x1DA < 8 (record K = record +0x1DA): if host op 49
      sight_line (+0x1DA, s16 K +0x116 + 2, s16 K +0x118 + 4, s16 +0x114, s16 +0x116 + 2, s16 +0x118 + 4, 1, 0x0E,
      0x40) ≠ 0 (32 bits): host op 103 quote_event (+0x1DA, 0xFA, 0, NONE, 0); then for j = 0..15: if record j
      +0x182 ≠ 0, +0x138 ≠ 0, +0x1D5 = 0 and +0x1FE ≠ 0: op 1 idle (j).
   c. c = cur_sector (read again). sectors[0xB1 × c + 0x6E + slot] = 0xFE. Host op 89 replay_event (s8 c, s8 c,
      0x54).
5. +0x188 = 1. +0x138 = 0. +0x17E = 0. t = s16 +0x114; if 0 ≤ t < 3136: tile byte 5 of t = (byte 5 & 0x80) |
   0x40.
6. u16 +0x114 = NONE, u16 +0x118 = 0, u16 +0x124 = NONE, u16 +0x116 = 0, u16 +0x128 = 0, u16 +0x126 = 0. +0x1A8 =
   0.
7. If slot < 8 and squad_lost ≠ 0 and quiet = 0: host op 143 deselect (1); selected = slot; host op 188
   squad_lost ().

*Evidence (2026-10-22): mirrors tools/ai.py kill (SoldierDies? 0x3A8C0), forget_soldier (0x3A148), forget_tiles
(0x74BD4), merc_dies (MercDies? 0x3A4A4), the enemy-died loop (0x3A2D4), DropItem 0x470B0 (12-byte clear),
diff-tested in ai_diff.py (kill, merc_dies).*

## Batch 5: aiming and firing, the chance to hit, the hit test

combat.md "Starting an attack", "Chance to hit" and "Bullet flight". The attack animation reaches its start word:
**Start attack** (op 79) faces the target and raises the weapon; **Attack go** (op 80) fires: **Fire gun** pays
the AP, picks the bullet's end point and height and rolls the **Chance to hit** (op 46); the projectile then flies
(Launch, Projectile step) and each new tile runs the **Hit test** (op 48). These four ops were host ops until now;
they become library ops (the library runs them unless their host_ops bit is set: 46 and 48 have their own bits,
79 and 80 use bit 63).

```c
void    ja1_start_attack(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);   /* = library op 79 */
void    ja1_attack_go(Ja1World *w, const Ja1Host *h, int32_t slot);                  /* = library op 80 (its tile
                                                                                         argument is not used) */
int32_t ja1_chance_to_hit(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t aim);  /* op 46 */
int32_t ja1_hit_test(Ja1World *w, const Ja1Host *h, int32_t height, int32_t x, int32_t y, int32_t flown,
                     int32_t direction, int32_t skip, int32_t cap, int32_t target, int32_t roll,
                     uint8_t *out);                                                 /* = library op 48; out: 8 B */
/* also an entry point (tests): */
void    ja1_fire_gun(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);
```

(tile, target: low 16 bits, signed; aim, direction, skip: low bytes; the others 32 bits.) ja1_hit_test writes its
out record like host op 48 (+0 the new skip byte, +4 the pass-through chance, i32) and returns the result. As a
library op inside Projectile step the record starts as skip = the skip argument, chance = 0.

New world fields (port_api.md, "aiming"): read-only tables `posture` (256 bytes, one per animation code: ≥ 6 = a
soldier in that animation can block a shot not aimed at it), `window` (2056 bytes: window[8 × object + direction],
signed: how a wall object lets a projectile through in that direction), `attack_sound` (5 × i16, one per weapon
class 0..4: the sound argument of a shot's Spend AP; guns 1..0x10 have classes 1..4), `diff_mod` (3 × i16, one per difficulty: the enemies' to-hit
bonus). Existing fields used: `combat`, `difficulty`, `effect_tile`, `selected`, `weapons` (byte 0 = class, byte 4
= tracking), `item_types` (byte 11 = range in tiles), `anim_table` (column 1 = can attack), `t0_class`,
`t1_class`, `tiles`, `soldiers`.

Aim direction (soldier_update.md; formerly host op 189 aim_dir): (direction 1..8 from the first point to the
second, the u16 the soldier keeps at +0x10A). 190 reloaded (slot): a library op (combat.md Reload): result (i32) 0 = the gun was reloaded, ≠ 0 = no clip. Other ops
used: 7 tile_dist, 45 shot_ap, 49 sight_line, 52 set_anim, 54 merc_message, 99 focus; library op 31 spend_ap; the
procedures Shot end (batch 1), Forget the attack (soldier_update.md), Max AP and World distance (ai.md), Throw
scatter (soldier_update.md).

**Blast kind** of an item type t (u8): t ≤ 0x15 → 0; 0x16..0x1B → t − 0x12; 0x1C, 0x1D → 2; 0x5D → 1; else 3.

**Current animation** of a record r = u16 r[+0x1E8 + 2 × r +0x1F3] (the whole word unless written "low byte").

### Start attack (`ja1_start_attack`, op 79 start_attack)

1. If +0x1BE ≠ 1: done.
2. me = s16 +0x114. (d, word) = Aim direction (px(me), py(me), px(tile), py(tile)); u16 +0x10A = word;
   +0x1A2 = d.
3. Aim posture (slot, tile).
4. occ = occupant(tile).
5. If +0x01 ≠ 0: +0x1BE = 2; u16 +0xDC = tile.
6. If s8 +0x00 ≤ 0xE and +0x01 = 0: if host op 190 reloaded (slot) ≠ 0 (32 bits): Shot end (slot, 1, 0); Forget
   the attack (1); done.
7. +0x1C9 = Blast kind (+0x00).
8. If occ = 0x40, or slot ≥ 8, or occ ≥ 8: done.
9. If +0x162 + occ = 0 and +0x19C ≠ 3: host op 54 merc_message (slot, 8, occ, 0, 1).
10. +0x162 + occ = 1. Record occ: +0x162 + slot = 1 (occ may be the soldier itself). +0x1D8 = 0x40.

**Aim posture** (slot, tile):
1. a = the current animation (as s16). If anim_table[8 × a + 1] = 0, or Water tile (+0x1B2, +0x1B4): done.
2. it = +0x00 if +0x01 ≠ 0, else 0. (d, word) = Aim direction (px(me), py(me), px(tile), py(tile)) with me
   = s16 +0x114; u16 +0x10A = word.
3. code = 0. If it ≠ 0 and s8 it ≤ 0x10: if s8 it ≥ 0xF: code = 0x22 unless 0x22 ≤ a ≤ 0x24; else if s8 it ≥ 8:
   code = 0x1E unless 0x1E ≤ a ≤ 0x20; else code = 0x1A unless 0x1A ≤ a ≤ 0x1C (code stays 0 where "unless"
   holds).
4. If code ≠ 0: op 52 set_anim (slot, code, 0, 1).
5. If d ≠ +0x1A2: if u8 +0x00 < 0xF and code = 0: Spend AP (slot, 1, 0xA). +0x1A2 = d.

### Attack go (`ja1_attack_go`, op 80 attack_go)

1. If +0x1BE ≠ 2, or the current animation is not 0x1B, 0x1F or 0x23, or s16 +0xDC = NONE: done.
2. Fire gun (slot, s16 +0xDC).
3. t = s16 +0xDC (Fire gun may have changed it). occ = occupant(t). x = s16 +0x116, y = s16 +0x118, ex = s16
   +0xDE, ey = s16 +0xE0.
4. The skip distance:
   - if occ ≥ 0x30 (nobody): if host op 49 sight_line (slot, x + 2, y + 4, t, ex, ey, 0x58, 14, 0x40) ≠ 0
     (32 bits): +0x1FF = low byte of max(|x − ex|, |y − ey|);
   - else if occ < 0x28 and slot < 0x28: the same with sight_line (slot, x + 2, y + 4, t, ex, ey, 0x58, 14, occ);
   - else: i = occ − 0x28 if occ ≥ 0x28, else occ; if s8 +0x13A + i = 1: +0x1FF = low byte of max(|x − ox|, |y −
     oy|) with ox = s16 record occ +0x116, oy = s16 record occ +0x118.
   (0x58 is the original's leftover register value, the low byte of the soldier record's address: pass it as is.)
5. The fire animation, by u8 +0x00 = it:
   - it ≤ 7: code = 0x1C; if op 7 tile_dist (t, s16 +0x114) < 2 (32 bits, signed) and (occ ≥ 0x30, or the current
     animation of record occ < 4): code = 0x2D;
   - it ≤ 0xE: code = 0x20;
   - it ≤ 0x10: code = 0x24; if combat ≠ 0 and slot ≥ 8: host op 99 focus (slot, 0);
   - else: done (no animation).
6. Op 52 set_anim (slot, code, 0, 1).

### Fire gun (`ja1_fire_gun`)

1. If combat ≠ 0: if slot ≥ 8 and s8 +0x9C > 0: host op 99 focus (slot, 0). effect_tile = tile.
2. it = s8 +0x00. If it = 0xF or it = 0x10: done.
3. c = low byte of op 45 shot_ap (slot, tile). Spend AP (slot, s8 of (c + s8 +0x1FD), attack_sound[weapons[6 × u8
   it + 0]]) (the sound as a signed 16-bit value).
4. occ = occupant(tile). If occ < 0x40 (record o = occ, which may be the soldier itself):
   a. u16 +0xDE = u16 o +0x116 + 2; u16 +0xE0 = u16 o +0x118 + 4.
   b. r = s8 of random(20). If r < 16: u16 +0xE2 = random(2500) + 2500; else if r < 19: u16 +0xE2 = random(1001)
      + 5000; else u16 +0xE2 = random(2500).
   c. If the current animation of o is ≤ 3 or ≥ 0x39: u16 +0xE2 = cdiv(s16 +0xE2, 6).
   Else (nobody on the tile):
   a. u16 +0xDE = px(tile); u16 +0xE0 = py(tile). By +0x1A2: 1, 2 or 8: u16 +0xE0 += 5; 4, 5 or 6: u16 +0xE0 −= 5;
      other values: no change.
   b. k = t1_class[t1 of tile]. If 6 ≤ k ≤ 8, or k = 9 and t1 is 0xE2 or 0xE3: u16 +0xE2 = 5000; else if k = 9:
      u16 +0xE2 = 2000; else u16 +0xE2 = 0.
5. ch = s8 of op 46 chance_to_hit (slot, tile, +0x1FD). r = s8 of (random(100) + 1).
6. If r ≤ ch: +0x1BC = 1; u16 +0xDC = tile; +0x1FC = ch − r (8-bit).
7. Else: +0x1BC = 0; m = r − ch (8-bit); +0x1FC = 0; d = World distance (s16 +0x116, s16 +0x118, s16 +0xDE, s16
   +0xE0); Throw scatter (slot, m, s16 of d); v = cdiv(s16 +0xE2 × s8 m, 100) (+0xE2 read after Throw scatter); if
   random(2) ≠ 0: v = −v; u16 +0xE2 = u16 +0xE2 + v − d (16-bit); if s16 +0xE2 < 0: u16 +0xE2 = 0.
8. u16 +0xD0 = tile.

### Chance to hit (`ja1_chance_to_hit`, op 46 chance_to_hit)

Result 1..99 (the low byte is read). Arguments: slot (the shooter S), tile, aim (a byte).
1. mk = s8 S +0x191. If u8 S +0x02 ≥ mk: c = mk; else c = cdiv(mk + u8 S +0x02, 2).
2. If tile = s16 S +0xD0: c = c + 10. c = c + 10 × aim.
3. If slot ≥ 0x28: c = c + diff_mod[s8 difficulty].
4. If S +0x1B9 = 4: c = max(c − 20, 1).
5. d = World distance (s16 S +0x116, s16 S +0x118, px(tile), py(tile)).
6. If S +0x24 = 0x27 (sun goggles): d = d − cdiv(d, 10). If S +0x03 = 0x42 (sniper scope) and d > 150 and aim ≠ 0:
   d = d − cdiv(s16 of cdiv((d − 150) × aim × 15, 100) × s8 S +0x05, 100).
7. c = c + cdiv(75 − d, 3).
8. If occupant(tile) = o < 0x40 (record T, which may be S): a = the low byte of T's current animation.
   a. If (a ≤ 3 or a ≥ 0x39) and d > 20: c = c − cdiv(d − 20, 3).
   b. If 5 ≤ a ≤ 7 or 14 ≤ a ≤ 16: c = c − 20.
   c. If d > 75 and T +0x1B1 ≠ 0 and T +0x1B3 is 1, 2, 4, 5, 6 or 7: c = c − 5.
   d. If s8 T +0x183 ≥ 15: p = Max AP of T; used = u8(p − T +0x1A5) if p > s8 T +0x1A5, else 0; m = u8(p × T
      +0x1F2 / used) if used ≠ 0 (unsigned division), else T +0x1F2. If s8 T +0x1D5 > 0: f = T +0x13A + slot if
      T +0x17A ≥ 0x28; else T +0x112 + slot if slot ≥ 0x28; else 0. If f = 1: m = u8(m + cdiv(s8 T +0x18B, 20) +
      cdiv(s8 T +0x192, 2)). If m > weapons[6 × u8 S +0x00 + 4]: c = c − (m − that byte) × 5.
9. hp = s8 S +0x183, mh = s8 S +0x184. If hp < mh: lost = u8(u8 S +0x184 − u8 S +0x183 − u8 S +0x185) / 2 + (mh −
   hp); c = c − cdiv(u8(cdiv(c × 2 × lost, mh × 3)) × (100 − (s8 S +0x192 − 1) × 10), 100) (div_zero when mh = 0).
10. e = s8 S +0x18A. If e < 100: c = c − cdiv((100 − (s8 S +0x18C − 10)) × u8(cdiv((100 − e) × c, 200)), 100).
11. If d > item_types[12 × u8 S +0x00 + 11] × 15: c = cdiv(c, 2).
12. Result min(max(c, 1), 99).

### Hit test (`ja1_hit_test`, op 48 hit_test)

Arguments: height h, point x, y (pixels), flown (pixels), direction (byte), skip (byte), cap, target (tile), roll
(≠ 0: the to-hit roll succeeded). Result: 1 = the projectile stops here, 0 = it flies on, −1 = it stops at a
roof-high wall. Out: skip (unchanged unless written), chance (unchanged unless written).
1. If h < 1: result 1.
2. t = vt(x, y). b = t1 of t; k = t1_class[b]. ob = 9000 if k = 6; 10000 if k = 7 or 8; if k = 9: 7000 when b is
   0xE2 or 0xE3, else 3000; otherwise 0.
3. If cap > 9000 and t0_class[t0 of t] = 3 and t3 of t ≥ 0x52: ob = 10001 (a roof).
4. If occupant(t) = o < 0x40 (record T):
   a. If target ≠ u16 T +0x114 and flown < 0x4B: if random(30) ≠ 0, or s8 posture[T's current animation] < 6:
      result 0. (random(30) is drawn only here.)
   b. a = T's current animation. ob = ob + 1000 if a < 4 or a > 0x37, else ob + 6000.
   c. If ob ≥ h and max(|x − (s16 T +0x116 + 2)|, |y − (s16 T +0x118 + 4)|) ≤ 1: result 1.
   d. If target = u16 T +0x114: skip = 0. Result 0.
5. If ob < h: result 0.
6. If ob = 9000 (a tree):
   - if flown ≥ 30 or t = target: n = max(25 − cdiv(flown, 15), 1); if random(s16 n) = 0 and chance (its value
     on entry) = 0: result 1; else chance = 100 − cdiv(100, n), result 0;
   - else if roll ≠ 0: chance = 100, result 0;
   - else if random(100) = 0 and chance = 0: result 1; else chance = 99, result 0.
   (The random number is drawn in every case. Line of fire calls with chance 100, so a tree never stops its
   estimate; Projectile step calls with chance 0.)
7. If ob < 10000: result 1 if max(|x − px(t)|, |y − py(t)|) ≤ 2, else 0.
8. If skip ≠ 0: result 0.
9. wn = s8 window[8 × b + direction]. If wn ≥ 0x62 and h < 9000: p = 100 − cdiv(flown − 15, 5); if random(100) ≥ p
   and chance (its value on entry) = 0: result 1; else chance = p, result 0. (Line of fire enters with chance 100,
   so a window never stops its estimate and only scales it; Projectile step enters with 0. Asm 0x57AB7..0x57AC0.)
10. If wn < 4 and cap ≤ 9000: result 0.
11. If cap ≤ 9000 or ob ≤ 10000: result 1. Else result −1.

## Batch 6: the melee blow

combat.md "A melee blow". The knife attack animation reaches its blow word (0x1EB): **Melee hit** (op 77) rolls
the blow, deals the damage and pays the AP. Fire gun pays nothing for a knife (step 2), so this op is the only
place that pays a stab. Op 77 was a host op until now; it becomes a library op (bit 63 of host_ops).

```c
void    ja1_melee_hit(Ja1World *w, const Ja1Host *h, int32_t slot);   /* = library op 77 melee_hit */
```

Existing fields used: `active_slot`, `anim_signal`, `cur_sector`, `story`, `weapons`, `attack_sound`, `tiles`,
`soldiers`. Ops used: 45 shot_ap, 105 stat_practice, 110 take_damage, 177 story_hit (as Projectile impact step
4c), Spend AP, Hit alert, Melee hit chance (ai.md), random.

### Melee hit (`ja1_melee_hit`, op 77 melee_hit)

1. active_slot = slot; anim_signal = −2. t = s16 +0xDC (the attack tile). occ = occupant(t).
2. If occ = 0x40 (nobody): if cur_sector = s8 story[0x17] and story[0x15] = 0 and story[0x16] = 0 and t1 of t =
   0xAE: story_hit (as Projectile impact step 4c). Go to step 6.
3. Record O = occ (the soldier itself when occ = slot). O +0x1C4 = 1. If slot < 8: op 105 stat_practice (slot, 3,
   1, 0). sure = 0; if occ < 0x28 and slot ≥ 0x28: sure = (s8 O +(0x112 + slot) ≠ 1); if occ ≥ 0x28 and slot <
   0x28: sure = (s8 O +(0x13A + slot) ≠ 1). If s8 O +0x183 < 15: sure = 1. If sure: height = 0x1356, chance = 99.
   Else: chance = low byte of Melee hit chance (O, aim = +0x1FD); height = 16-bit (random(2500) + 2500).
4. roll = low byte of random(100). If roll < chance (a hit), with every step in 16 bits, read as signed:
   d = weapons[6 × it + 5] + random(31) − 15 (it = u8 +0x00, the byte read unsigned); d = d + (s8 +0x192 − 1) × 2;
   d = d + cdiv(((u8 +0x184 − u8 +0x183 − u8 +0x185) low byte) / 2 + s8 +0x183 − 50, 5); d = d + cdiv(chance −
   roll, 5); d = cdiv(s8 +0x02 × d, 100); if d < 1: d = 1. Op 110 take_damage (occ, height, d, 1, slot, none).
   If slot < 8: op 105 stat_practice (slot, 3, 1, 0). If s8 +0x02 > 10: q = low byte of cdiv(d, 10); if it = 0x10:
   q = q / 2; if q > cdiv(s8 +0x02, 10): q = that value; if q > 1: +0x02 = +0x02 − random(q) (8-bit).
5. Else (a miss): if sure and s8 O +0x183 ≥ 15 and s8 O +0xBA ≠ 2: Hit alert run as soldier occ (opponent slot,
   flag 1). If occ < 8: op 105 stat_practice (occ, 2, 1, 0).
6. c = low byte of op 45 shot_ap (slot, t). Spend AP (slot, s8 of (c + s8 +0x1FD), attack_sound[weapons[6 × it +
   0]]) (the sound as a signed 16-bit value, it = u8 +0x00 read now). +0x1BE = 0.

## Tests

- `ai_diff.py --only launch --port`, `--only pstep --port`, `--only fire --port`, `--only w_56dd4 --port`,
  `--only impact --port`: 0 mismatches.
- Batch 2: `ai_diff.py --only take_damage --port`, `--only hit_reaction --port`, `--only hit_alert --port`, `--only
  mark_seen --port`: 0 mismatches.
- Batch 4: `ai_diff.py --only kill --port`, `--only merc_dies --port`: 0 mismatches.
- Batch 3: `damage_diff.py --port`, `ai_diff.py --only shot_alert --port`, `--only interrupt_check --port`: 0
  mismatches.
- Batch 5: `ai_diff.py --only start_attack --port`, `--only attack_go --port`, `--only fire_gun --port`, `--only
  chance_to_hit --port`, `flight_diff.py --port`, `walk_diff.py --shoot`: 0 mismatches.
- Batch 6: `ai_diff.py --only melee_hit --port`: 0 mismatches.
- `ai_diff.py --only ticks --port` (Timer tick: busy_timer), `world_diff.py` and `world_diff.py --port`,
  `walk_diff.py`, `walk_diff.py --combat`: 0 mismatches.
