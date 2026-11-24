# Enemy and guard AI: decisions

Enemies and hired guards (every soldier except the player's eight mercs) decide by a fixed procedure, run once per
game tick for each of them. All arithmetic is integer; divisions truncate toward zero. "Roll *c*" means: draw a
random number 0..99 and succeed if it is below *c* (so *c* ≤ 0 never succeeds, *c* ≥ 100 always does). Values the
decisions read about a soldier, by their record names in `defs/types.toml`:

- **aiType** 0..4: type 2 never acts; type 4 is Santino (see below); only type 3 reacts to noises; type 1 changes
  its orders (+0x1D4) when the fight ends (below).
- **alert** state: 0 calm, 1 alerted (heard something), 2 combat, 3 engaged (the soldier's counter at record
  byte +0x1D5 is above 0: set by the combat code, it also enables the dodge bonus in the chance to hit). The game's
  debug page (cursor.md:551) calls +0x1D5 "# opponents" (types.toml `numOpponents`; MarkSeen adds 1 for each newly
  seen opponent).
- **orders** (0..4) and **attitude** (0..5): values that shift every chance below. Other values change nothing unless
  stated. Names from the game's debug page (cursor.md:551): "orders" = +0x1D4, "attitude" = +0x1D3 (until
  2026-11-06 this spec called them trait1 and trait2). Orders sets the patrol leash (1/5/15/99 tiles).
- **bypassToGreen** (+0x131): debug page "bypass To Green" (was calmBonus). Added to the calm rolls. While it is not 0 and the alert state is below 3, the soldier behaves as
  if calm.
- energy, health / max health, AP left, facing (1..8), postFacing (the direction a guard returns to).

"Tired" below means (max health − health) + (100 − energy). Several decisions need a target or a tile, which other
procedures supply (the noise heard, a calling ally's position, a search position, an advance position, a
patrol tile, a retreat tile, the nearest dry tile), specified under Positions. Each returns "none" or
a tile, and the decision falls through to the next option on "none".

## One tick

Record names below are those of `defs/types.toml` (struct soldier). "Signed" bytes and words are two's complement;
a comparison with a negative constant or between two record bytes is signed unless stated. The **current
animation** is the 16-bit word at record offset 0x1E8 + 2 × animIdx. **Walk target** = the word at +0x120,
**step tile** = the word at +0x124 (the tile the soldier stands on as far as walking is concerned; +0x126 / +0x128
are its pixel x / y). **No tile** = 0xC41. The **occupant** of a tile = tile byte 5 & 0x7F, where 0x30 and up count
as 0x40 (none).

1. Slots below 8: nothing.
2. Not turn-based: nothing while busy > 0 (signed) or the active slot is below 0x40.
3. aiType (signed) 2: turnDone = 1, done. aiType 4 (Santino): if his byte +0x9C (signed) is above 0 and Santino
   has not spoken: while his timer (signed s16 turn_timer2: soldier_update.md Loop head step 1 counts it up to 20 while the screen is not drawn, i.e. about 2 s of the enemy turn) is below 20 nothing happens (turnDone stays). Otherwise: redraw (only
   with show_ai), delay 1500, Santino's speech, delay 300, santino_spoke = 1, santino_word = 0. In every case that
   gets this far: turnDone = 1, done.
4. If inCurSector is 0, inPlay is 0, hp (signed) is below 15, or the current animation is below 4 and energy
   (signed) is below 10: turnDone = 1, done.
5. If forcedToStop is set or column 0 ("can act") of the current animation's table row is 0: nothing (wait).
6. Current animation below 4 (down): turnDone = 1 if apLeft (signed) is below 7; done either way.
7. Host perceive(slot, 0), then the alert state (next section).
8. If rethink is set: rethink = 0, bypassToGreen = 0, and if aiAction is not 0: walk target = no tile, `stopped` (+0x1B8) = 1,
   aiAction = 0, aiTarget = no tile, actionRunning = 0, desdir = facing, step tile = tile, +0x126 = x2, +0x128 = y2.
9. If `stopped` (+0x1B8) is 0: done.
10. If aiAction and actionRunning are both set (an action still runs):
    - Walk target not "no tile" and step tile ≠ walk target: host walk_to(slot, walk target); if the step tile
      still differs from the walk target afterwards, rethink = 1. Done.
    - aiAction 1 and desdir (signed byte) ≠ aiTarget (signed word): desdir = aiTarget's low byte. Done.
    - Otherwise the tick counter actionTicks runs: it is first set to 350 if aiAction is 14 and the aiTarget tile's
      occupant is none, and set to 350 in turn-based mode; then it counts up by 1 (so a forced one becomes 351). Below
      350: done. At 350 or more: aiAction 11, 12, 14 or 16 → host fire(slot); any other → apLeft = 0, turnDone = 1.
      Then actionTicks = 0, rethink = 1. Done.
11. actionTicks = 0. If aiAction is 0: actionRunning = 0, aiAction = the low byte of "choose an action" (fallback
    0); if that is 0: turnDone = 1, done.
12. If the action is not affordable (AP cost below): turnDone = 1, done.
13. Redraw (only with show_ai); actionRunning = the result of starting the action (Starting an action, end).

## Alert state

Let *old* = alert (signed). By aiType as an unsigned byte:

- **0..2**: alert = 3 if numOpponents (signed) is above 0; else 2 if the current sector's enemies-left byte is not 0; else
  0, and then a soldier with aiType 1 coming from *old* ≥ 2 also gets orders = 3 if its word +0x12E is exactly
  0xC41 (else 1), bypassToGreen = 0, inTheWay = 0, and host idle(slot).
- **3..4**: alert = 3 if numOpponents > 0; else from alert 3: 2; from 0: 2 if the alarm is raised or underFire is set, else 1 if
  host noise_tile(slot) is not "no tile"; from 1: 2 if the alarm is raised or underFire is set, else 0 if noise_tile
  gives "no tile" and actionRunning is 0 (noise_tile is asked before actionRunning is looked at). Any other
  state stays. The noise is only asked when the alarm test fails.
- **Other types**: alert stays.

Then with *new* = alert (signed):

- *old* = *new*: rethink = 1 when (*new* 0 and energy (signed) < 75, or *new* 1 and energy < 50) and the soldier
  is not in water.
- Else: rethink = 1 if *old* < 2 or *new* < 2. Also, *old* < 2 and *new* ≥ 2 and orders (unsigned) < 2: orders + 1,
  except for aiType 3 with orders 0 and explosives (signed) exactly 99.

**In water**: groundUnder ≥ 0x6C and objectUnder is not 0x1C or 0x1F. **In gas**: bit 0x80 of byte 5 of the
soldier's tile, unless item entry 3 is 0x28 (mask).

**Idle** (the host op idle; spec for when it is ported): if the soldier's record byte +0x1FE is set, it forgets its
attack: attack target slot and the slot at +0x1C2 become "none" (0x40), byte +0x1BE 0, the last target tile and
the word at +0xDC "no tile"; then, if the current animation is one that steps on (column 2 of the per-animation
table, 8 bytes per animation), the animation state advances one step (not yet specified).

## Choosing an action

Action codes: 1 turn (target = direction 1..8), walks 2 (patrol / dry tile / noise), 3 (buddy / calling ally),
4 (advance), 5 (search / attack position), 8 (retreat), 13 (scripted spot); 6 shout for help, 7 raise the
alarm, 9 operate a scripted target, 10 radio, 11 shoot, 12 throw, 14 stab, 16 use the hand weapon on the scripted
target. Each procedure returns the code and leaves its target in aiTarget. **"Set target T"** means aiTarget = the
low 16 bits of T even when T is "no tile" (the next try overwrites it); **"walk to T with code C"** means: set
target T; if T is not "no tile": b1A8 = 0, return C; else go on. "Nothing" = aiTarget = no tile, return 0.
Host tile results are read as signed 16-bit values. "Roll *c*" draws random(100) and succeeds if it is below *c*,
where *c* is first cut to a signed 16-bit value.

**Choose**: bypassToGreen not 0 and alert (signed) < 3 → Calm. Else by alert (unsigned): 0 Calm, 1 Alerted, 2 Combat,
3 Engaged, any other value → return the fallback unchanged.

**AP cost** (affordable when the cost as a signed byte ≤ apLeft as a signed byte), by aiAction: 1 → 1; walks → the
low byte of host move_ap(slot); 6, 7 → 5; 9, 10 → 2; 11, 12, 14, 16 → the low byte of host action_ap(slot,
aiTarget, 1); others 0.

**Orders/attitude shifts** used below (unlisted values: 0). Shout/alarm: orders 0: +20, 2: −10, 3: +10, 4: −20; attitude 0: +20,
1: −10, 3: −5, 5: −20. **Tired** = (hpMax − hp) + (100 − energy), all signed bytes.

### Calm

1. Unless combat mode, justArrived, inTheWay, in water or in gas: draw random(10); if it is not 0: host idle(slot), nothing.
2. In water or in gas: walk to host find_dry_tile(slot) with code 2.
3. Unless energy (signed) ≥ 75, inTheWay or in water: nothing.
4. Patrol: *c* = bypassToGreen (unsigned) + 25 + orders (0: −20, 1: +15, 2: +25, 4: −10) + attitude (0: −10, 1: +5,
   3: +5, 5: +10) − tired. If justArrived, inTheWay or in water (no roll drawn then), or roll *c*: set target host
   patrol_tile(slot); if found: b1A8 = 0, inTheWay = 0, return 2.
5. Buddy: *c* = bypassToGreen + 25 + orders (0: −20, 1: +10, 2: +20, 4: −10); attitude 1 or 3: *c* = *c* / 2 (16-bit,
   truncated); attitude 2 or 4: +10; then − tired. Roll *c*, and only then host buddy_tile(slot): low byte not 0 →
   b1A8 = 0, return 3 (the host stored the target).
6. Turn: roll bypassToGreen + 25 + (25 if orders is 0) + (25 if attitude is 0). On success repeat: if postFacing (signed)
   is 1..8 and differs from facing, draw random(2): not 0 → direction = postFacing; in every other case direction
   = random(8) + 1. Set target direction; if facing (signed) differs from it, return 1; else repeat (no limit).
7. Nothing.

### Alerted

1. aiType (signed) not 3: return 0, aiTarget unchanged.
2. Host noise_tile(slot): tile and loudness (32-bit). "No tile": return 0, aiTarget unchanged.
3. *d* = host direction from the soldier's tile to the noise (low byte). If facing (signed) ≠ *d* and then host
   tile_dist(tile, noise) ≤ 14: roll (60, or 35 if orders ≠ 0) + (15 if attitude is 0) → set target *d*, return 1.
4. Enemies (slot ≥ 0x28) with apLeft (signed) ≥ 5: *c* = (low byte of host news_value(slot)) × 5; if *c* is not 0
   and groundUnder < 0x9A: *v* = host sector_value(current sector, 1) (signed 16-bit), −100 → 0, else at most 100;
   *c* + *v* / 2 (truncated) + the shout shifts; roll → return 6, aiTarget unchanged.
5. Unless energy (signed) ≥ 50 or in water: nothing.
6. Here "tired" means only 100 − energy. Roll loudness + 75 + orders (0: −20, 1: −10, 2: +10, 4: +25) + attitude (0:
   −10, 1: +10, 2: +5, 3: +5, 5: +20) − tired (cut to 16 bits) → walk to host approach_tile(slot, noise) with code 2.
7. *a* = host ally_call_tile(slot); if not "no tile": roll 50 − host tile_dist(tile, *a*) + orders (0: −20, 1: −10,
   3: +20, 4: +10) + attitude (0: −10, 2: +20, 4: +20, 5: −20) − tired → walk to host approach_tile(slot, *a*) with
   code 3.
8. Roll orders base (0: 45, 1: 35, 2: 20, 4: 5, any other: 25) + attitude (0: +10, 1: −15, 2: −20, 3: +20, 4: +15, 5:
   −10) − tired → walk to host search_tile(slot, 0) with code 5.
9. random(100) < 50: bypassToGreen = 15, return Calm.
10. Nothing.

### Combat

"AP" = apLeft as a signed byte.

1. apLeft 0: nothing.
2. **Can walk** = AP ≥ host move_ap(slot) (32-bit signed); asked once, here, before anything else.
3. Scripted mission (slot ≥ 0x28 and mission_step (signed) > 0):
   - The current sector's mission is not done and either hand (item entry 1 first, then 0) holds item 0x40 (the Detonator; JA.EXE item names: 0x2A = Radio, 0x40 = Detonator; the action
     code 10 keeps its old label "radio"):
     AP ≥ 2 → return 10 (aiTarget unchanged), else nothing.
   - If mission_slot is this soldier and the mission target is not "no tile": byte +0x9C is set to 0xFF during
     host reach_ap(slot, mission target, 0) and restored after it. A result of 0: go to the cancel below.
     Otherwise, with the spot = mission_spot (read after the call):
     - Spot = the soldier's tile: at mission step ≠ 1, AP ≥ 2 → set target mission target, return 9. At step 1:
       *r* = host attack_ready(slot, mission target) (signed low byte); −6 or −5 → host swap_hand(slot, 0, 6) and
       ask again. *r* = 1: if the second hand's item kind is 0x96 or the main hand's kind is neither 0x92 nor 0x96,
       swap_hand; then if the main hand's kind is 0x92 or 0x96 and host action_ap(slot, mission target, 1)
       (32-bit signed) ≤ AP: set target mission target, return 16. Any other outcome at the spot: nothing.
     - Not at the spot: cannot walk → nothing. Host can_stand_at(slot, spot) low byte not 0 → set target spot,
       return 13.
     - Cancel (reach_ap 0, or the spot not standable): mission_slot = 0x40, host mission_cancel(); go on with step 4.
4. Energy (signed) < 25, not in water and underFire clear: nothing.
5. *s* = low byte of host opp_strength(slot). *s* = 0 and can walk: walk to host retreat_tile(slot) with code 8.
6. Alarm: enemies (slot ≥ 0x28) with AP ≥ 5: *c* = (low byte of news_value) × 10 (16-bit); *c* not 0 and
   groundUnder < 0x9A: while no alarm is raised *c* = 50 / 80 / 95 by difficulty; once raised *c* + the shout
   shifts (16-bit), then ×3 at *s* 0, ×2 at 1 (16-bit), / 2 at 3, / 3 at 4 (truncated). Roll *c* → return 7,
   aiTarget unchanged.
7. Can walk: weights (advance, approach, attack) by *s*: 0 → (−99, −99, +1), 1 → (−1, 0, +1), 3 or 4 → (+1, 0,
   −1), else 0; plus orders (0: −1/−1/+1, 3: 0/+1/−1, 4: +1/0/−1), then plus attitude (0: −1/0/+1, 1: +1/−1/−1, 2:
   +1/+1/−1, 3: 0/−1/+1, 4: 0/+1/+1, 5: +1/0/−1), each sum kept as a signed byte. While not all three are ≤ −90,
   one pass checks the three options in order, each with the weights as they are at that moment:
   - advance, if its weight > −90 and ≥ both others: *t* = host advance_tile(slot, 1); if not "no tile": walk to
     host approach_tile(slot, *t*) with code 4. Then advance weight = −99.
   - approach, if > −90 and ≥ both others: *t* = host ally_call_tile(slot); if not "no tile": walk to
     approach_tile(slot, *t*) with code 3. Then approach weight = −99.
   - attack, if > −90 and ≥ both others: *t* = host target_tile(slot); if not "no tile": walk to host
     search_tile(slot, mode) with code 5, mode 1 for *s* < 2, 2 for *s* 3 or 4, else 0. Then attack weight = −99.
8. *t* = host target_tile(slot); if not "no tile": *d* = host direction to *t*; if facing (signed) ≠ *d* and then
   tile_dist(tile, *t*) ≤ 14: roll (50, or 25 if orders ≠ 0) + (25 if attitude is 0) → set target *d*, return 1.
9. Can walk: bypassToGreen = 30, return Calm.
10. Nothing.

**Attack-ready check** (the host op attack_ready; spec for when it is ported): −3 if the target is the soldier's own
tile; −2 in water; −4 if its current animation does not allow attacking (column 1 of the per-animation table); −6
if the main hand holds neither a gun (items 1..16) nor a throwable (item kind 0x95); −5 if the main hand's count
byte is 0; −1 if the animation does not allow acting (column 0); else 1 (ready). A gun counts as a gun whatever its
condition byte.

### Engaged

1. apLeft 0: nothing. mission_slot is this soldier: return Combat.
2. AP as in Combat. *s* = low byte of host opp_strength(slot); then can walk = AP ≥ host move_ap(slot) (asked
   once, here).
3. In gas, or energy (signed) < 5: can walk → walk to host retreat_tile(slot) with code 8; then *s* = 0.
4. *r* = host attack_ready(slot, no tile) (signed low byte); −6 or −5 → swap_hand(slot, 0, 6) and ask again. In gas,
   *r* = 0 (the calls are still made). *L* = lastTargetTile (signed).
5. Only if *r* = 1, the options (each starts "not valid"):
   - Gun: if the main hand's item kind is not 0x92, swap_hand; if it is still not 0x92: nothing. If AP ≥ the low
     byte of host action_ap(slot, *L*, 0), read unsigned (0..255): gun = host gun_option(slot); then if the hp (signed) of the soldier in
     the gun record's slot is below 15 and attitude is not 5, and host advance_tile(slot, 0) is not "no tile":
     return Combat (whether or not the gun option is valid).
   - Then throw (item kind 0x95, host throw_option) and knife (kind 0x96, host knife_option), each: the hand is
     the main hand if its kind matches, else the second hand, else skip. Second hand: swap_hand first. If AP ≥ the
     low byte of action_ap(slot, *L*, 0) (unsigned, as for the gun), the option is taken from the host. Second hand: swap_hand back.
   - Pick: gun and throw valid → throw if the gun's score < the throw's score, else shoot; else gun and knife valid
     → stab if the gun's score < the knife's, else shoot; else the first valid of gun (11), throw (12), knife (14).
     A throw or stab from the second hand swaps the hands once more.
   **Attack** = set target the pick's tile, aim = its aim clicks, curOppnum = its slot, return its code.
6. A pick whose hit chance (unsigned byte) is at least 75 / 10 / 25 / 25 / 50 for *s* 0..4: attack.
7. Can walk: *s* 0 → walk to host retreat_tile(slot) with code 8; then walk to host search_tile(slot, mode) with
   code 5, mode 0 in water, else 1 for *s* < 2, 2 for *s* 3 or 4, else 0.
8. A pick exists: attack.
9. In water or gas and can walk: walk to find_dry_tile(slot) with code 2; then walk to retreat_tile(slot) with
   code 8; then *s* = 0.
10. *s* > 0 or can walk: if host busy(slot) is 0: *t* = host target_tile(slot); if not "no tile": *d* = direction
    to *t*; facing (signed) ≠ *d* → set target *d*, return 1.
11. Nothing.

## Situation score

The score *s* used by the combat and engaged decisions says how the soldier's side stands, 0 (badly outmatched)
to 4 (clearly stronger). Exact procedures (port slice 2b part 2); the conventions of "Search procedures" apply
(s8 / u8 / s16 / u16, C division = truncation toward zero, `cdiv`). World data used: knowledge, fresher,
remember_m, remember_e, item_types (port_api.md).

**Max AP** of a record R (also combat.md "Action points per turn"; this is the exact form):
1. If u8 health (+0x183) is 0: the result is 0.
2. ap = u8(cdiv(s8 level (+0x192) × 10 + s8 agility (+0x18B) × 2 + s8 maxHealth (+0x184) + s8 dexterity (+0x18C)
   + 10, 20)). There is no upper limit.
3. If s8 health < s8 maxHealth: lost = u8(u8 maxHealth − u8 health − u8 wounds (+0x185)) / 2 (an unsigned value,
   halved) + (s8 maxHealth − s8 health); ap = u8(ap − cdiv(ap × 2 × lost, s8 maxHealth × 3)).
4. If s8 energy (+0x18A) < 100: ap = u8(ap − cdiv((100 − s8 energy) × ap, 200)).
5. The result is ap, at least 8.

**Armour** of R: the sum over three inventory entries of cdiv(s8 condition × value, 100), where the entry's item
type is its byte +0 and its condition its byte +2; an item type not in the entry's list adds nothing:
- body armour, entry at +0x3C: item 0x2D = 20, 0x2E = 30, 0x2F = 40, 0x30 = 60;
- helmet, entry at +0x18: 0x23 = 5, 0x24 = 7, 0x25 = 10, 0x26 = 15;
- vest pocket, entry at +0x48: 0x31 = 20 (Shielded Vest).

**Soldier power** of record R, optionally rated against a tile T (s16; NONE = no tile) with a flag F (byte):
1. If u8 inCurSector (+0x182), u8 inPlay (+0x138) or u8 health (+0x183) is 0: the result is −999.
2. v = Max AP + cdiv(s8 apLeft (+0x1A5), 2) + cdiv(Armour, 4) + cdiv(s8 marksmanship (+0x191), 5)
   − cdiv(100 − s8 health, 10) − cdiv(100 − s8 energy (+0x18A), 10).
3. Main-hand item t = u8 byte +0 of R: t 1..14: v += 3 × t; else t 0x5D or 0x0F: v += 2; else t 0x10: v += 5;
   else, if item_types[12 × t + 2] (the kind) is 0x95: v += 15.
4. If T is not NONE: if u16(T) equals u16 lastTargetTile (+0xD0), v += cdiv(v, 10) (no host call). Otherwise call
   host direction (op 6) with px(me), py(me), px(T), py(T), where me = s16 tile (+0x114) of R; if s8 facing
   (+0x1A1) equals the full 32-bit result, v += cdiv(v, 20).
5. v = v × s8 level (+0x192).
6. If s8 health < 15: if v > 0, v = cdiv(v, 19 − s8 health). Otherwise, if F is not 0 and T is not NONE: call
   host fire_line (op 42) with slot = u8 slot (+0x17A) of R, T, byte 1; v = cdiv(i32(v × result), 100), where the
   result is the full 32-bit value and the product wraps to 32 bits.
7. The result is v, at least 1.

**Situation score** of soldier S (slot `me`):
1. Sides. For me ≥ 0x28 (an enemy): the opponents are slots 0..15 (index j = slot), the allies slots 0x28..0x2F
   (index k = slot − 0x28), and S's remember byte for opponent j is remember_e[(me − 0x28) × 40 + j]. Otherwise:
   the opponents are slots 0x28..0x2F (j = slot − 0x28), the allies slots 0..15 (k = slot), and the remember
   byte is remember_m[me × 8 + j]. Note: an enemy looks at only 16 opponents here, not at 0x28.
2. "Counts" (a record): u8 inCurSector ≠ 0, u8 inPlay ≠ 0 and s8 health ≥ 15.
3. level[k] = s8 knowledge[ally slot] for every ally k (all allies, counted or not). oppSum = 0, allySum = 0.
   my = s16 tile (+0x114) of S.
4. For each opponent O in slot order, if O counts:
   a. a = s8 sight flag of S on O (S +0x13A + j), b = s8 knowledge[O's slot].
   b. If a = 0 and b = 0: skip O (go on with the next) when the remember byte is 0; else lvl = 0.
      Otherwise lvl = a if fresher(b, a) ≠ 0 or a = b, else b.
   c. oppSum = s16(oppSum + cdiv(weight(lvl) × s16(Soldier power of O against my, F = 0), 100)).
   d. For every ally k in order: c = s8 sight flag of O on ally k (O +0x13A + k). If c ≠ 0, the ally record counts,
      and (fresher(c, level[k]) ≠ 0 or c = level[k]): level[k] = c. (The first argument of fresher is c, so a
      flag replaces the level only when the level is fresher than it or equal: the level only moves to staler
      values, and a level of 0 stays 0. This is what the original does.)
5. For each ally A (index k) in slot order, if A counts: w = 100 if A's slot is me, else weight(level[k]);
   allySum = s16(allySum + cdiv(w × s16(Soldier power of A, no tile, F = 0), 100)). No host calls happen here.
6. ratio = 500 if oppSum = 0, else s16(cdiv(allySum × 100, oppSum)). s = 0 if ratio ≤ 25, 1 if ≤ 50, 2 if ≤ 150,
   3 if ≤ 300, else 4.
7. sv = s16(host sector_value (op 9) with byte cur_sector, int 1). s += 1 if sv > 50; s −= 1 if sv < 35.
8. t = u8 attitude (+0x1D3) — attitude, not orders: s += 1 if t is 1 or 2; s −= 1 if t is 0.
9. e = s8 energy (+0x18A): s += 1 if e > 75; otherwise s −= 1 if e < 40, and s −= 1 more if e < 10.
10. h = s8 health (+0x183): s += 1 if h > 75; otherwise s −= 1 if h < 40, and s −= 1 more if h < 20.
11. s += 1 if u8 underFire (+0x1C4) is 0.
12. s −= 2 if santino_spoke ≠ 0 and me ≥ 0x28.
13. The result is s clamped to 0..4.

weight(lvl) for lvl −3, −2, −1, 0, 1, 2, 3, 4 = 60, 70, 80, 50, 100, 90, 80, 70 (levels are always in −3..4).
Freshness order, for reading: 1 > 2 > −1 > 3 > −2 > 4 > −3 > 0 (the table `fresher` is the authority).

## Sector value

How much the enemy wants a sector (the strategic layer uses it too; the AI reads it for the current sector with
the bonus flag set). Exact procedure (port slice 2b part 2, op 9) for a sector number n (a byte, below 80 here)
and a flag F (byte). R = the sector's row in sector_state (9 bytes, layout sectorState). World data used:
sector_state, day, story, plants, crates, difficulty (port_api.md). story[k] = byte k of the story block (the
offsets below are into it). "Never" = −100. A comparison "n = s8(x)" compares the sector number with the byte
read as signed, so a byte of 0x80 and up never matches.

1. If u8 R[8] ≠ 0: never.
2. If n = 0x3B (home) and homeDay ≥ day (signed 32-bit), where homeDay = 6, 4, 2 for difficulty 0, 1, 2: never.
3. Story condition C: s8 story[0x09] = 2 and s8 story[0x06] = 2 and s8 story[0x0B] < 1 and n is 0x11 or 0x1B.
4. If n = 0x1B and (u8 R[0] & 1) ≠ 0 and not C: never.
5. If n = 0x35 and s8 story[0x0D] = 1: 1000.
6. If C: 1000 for n = 0x1B, else 250.
7. v = 30 + u8 R[6] × 5 + u8 R[7] × 2 + (u8 R[3] / 3) × 3.
8. v += u8(host connected_home (op 43) with byte n) × 5.
9. v += 1 for each of the bits 0x02, 0x04, 0x08, 0x10 that is clear in u8 R[0].
10. v += 15 if n is one of 0x1C, 0x1D, 0x26, 0x30, 0x31.
11. v += 20 if n = 5.
12. If n = 0x0B: q = u8(host quest_check (op 44) with int 0x1C9). If q ≠ 0: never if u8 R[2] ≠ 0, else v += 25.
    (No host call for other sectors.)
13. v += 25 for each k = 0..3 with plants[k] = n (plants[k] = i32 at byte 12 × k + 4 of the plant block).
14. v += 25 if some k = 0..3 has n = s8(crates[4 × k]) and day + 1 = s8(crates[4 × k + 1]) (32-bit sum).
15. v += 50 if s8 story[0x0B] = 1 and n = s8(story[0x17]) and u8 story[0x16] = 0 and u8 story[0x15] = 0.
16. If n = 0x3B: v = i32(v + day).
17. v += 20 if n = 0x15.
18. v += 100 if n = 0 and u8 R[2] ≠ 0.
19. If F = 0: v = i32(v − 25).
20. The result is v clamped to 20..100 (signed).

## Sightings and the opponent's position

**Updating sightings.** For every opponent in the sector and alive (any health) on which the soldier has a non-zero
sight flag, the sight of that opponent is re-checked (Sight check, below; it may change the flag and counts new
sightings). Opponents are slots 0..0x27 for an enemy (40 flags) and the 8 enemy slots for a merc or guard. When
aging is asked for, the flag then gets older: 2, 3, 4 step up by one (past 4: forgotten, 0); −1, −2, −3 step down
by one (past −3: 0); 1 stays. A merc or guard with new sightings shares them with its side (Sharing sightings, below, without the reaction flag). AI type
2 skips all of this. The AI tick runs it without aging; another per-soldier routine (0x11FB4) runs it with aging.

**Sight check** (one soldier, one opponent; result: a new sighting or not). Nothing happens unless both are in
the sector, in play and alive, they are on opposite sides (an enemy and a merc or guard), the soldier is not AI
type 2, its current animation code is 4 or more, and, for Santino (type 4), its record byte +0x9C is above 0.
"Old flag" is the soldier's sight flag on the opponent before the check.
- Santino sees the opponent when the opponent's own flag on enemy slot 0x28 is 1 (he sees whoever sees him); this
  also clears tile byte 3 in two fixed rectangles of the map (tiles 232 + 7 rows × 6 columns and 625 + 3 rows × 5
  columns).
- Anyone else sees the opponent when the tile distance (larger of the row and column difference, low byte only)
  is within the sight range and a sight line exists from the soldier's (x + 2, y + 4) to the opponent's
  (x + 2, y + 4) (Sight line, below). The range is 14 tiles while the old flag is not 0; otherwise
  it depends on the direction from the soldier to the opponent (world pixels) relative to its facing: 14 tiles
  straight ahead, 12 at 45°, 8 at 90°, 3 at 135°, 0 behind.

When seen, **mark seen**: the flag becomes 1 and the soldier's last-seen tile for the opponent becomes the
opponent's tile. If the old flag was not 1 (newly seen): the counter +0x1D5 rises by 1, orders rises (as when
entering combat, see Alert state) if the alert state is below 2, the alert state becomes 3, and the soldier
re-thinks unless the old flag was 2 with the opponent still on the last-seen tile; a soldier whose current action
is 3 or 4 always re-thinks. The soldier's remember byte for the opponent becomes 0xFF if it was 0, else 1, and the
opponent's byte +0x9C becomes 1 if it was 0 or negative. Result: a new sighting only if the old flag was neither 1
nor 2; otherwise, for a merc or guard, the side's knowledge of the opponent becomes 1.

When not seen and the old flag was 1 (sight lost): +0x1D5 falls by 1 (at 0 the alert state becomes 2), the flag
becomes 2, a remember byte of 0xFF becomes 1, and if the side's knowledge of the opponent is 1 and no other soldier
of the soldier's side (in play, health 15 or more) has flag 1 on it, the knowledge becomes 2. Mercs and guards
count slots 0..15 as their side here, enemies (AI type 3 and 4) slots 0x28..0x2F.

**Sight line** (library op 49 sight_line, `ja1_sight_line`; arguments: the looker S, pixel (x, y), tile T, pixel
(tx, ty), the old sight flag F (low byte), a range R in tiles, the slot V seen (low byte; 0x40 and up = none); result
1 = seen, 0 = not seen). All counters below are 16-bit values compared as signed (s16) unless said otherwise.
1. (The callers pass a real slot 0..47 or 0x40 as V; the original reads past the soldier records for 48..63.)
   If V < 0x40, F = 0, V's byte +0x1B1 ≠ 0 and V's byte +0x1B3 is 1, 2, 4, 5, 6 or 7: R = R − R × 30 / 100
   (division truncated toward 0, 32-bit). Limit L = R × 15.
2. D = direction (op 6) from (x, y) to (tx, ty), low byte (1..8).
3. dx = tx − x, dy = ty − y; sx = −1 if s16 dx < 0 else 1, sy the same for dy; dx, dy become their absolute
   values. If s16 dx < s16 dy: major = dy, minor = dx, the straight step is (0, sy); else major = dx, minor = dy,
   the straight step is (sx, 0). err = 2 × minor − major, diag = err − major. The diagonal step is (sx, sy).
4. prev = the view tile of (x, y) (view tile = x / 16 + (y / 14) × 56, divisions truncated toward 0). steps = sum
   = count = opacity = 0, brush = no.
5. Repeat:
   a. If s16 err < 0: take the straight step, err += 2 × minor; else take the diagonal step, err += diag.
      cur = the view tile of the new (x, y). steps += 1. If steps > s16 L: result 0.
   b. If cur ≠ prev (a new tile): sum = count = 0; (opacity, brush) = the **tile opacity** of cur (below) at the
      new (x, y) with this steps count. If opacity ≥ 0x62 (a window): when steps > 15, L = steps + 30 for 0x62 and
      L = steps for any higher value; when steps ≤ 15, L = steps + 30 for 0x63 and L stays for the others; then
      opacity = 3. If brush: count += 1.
   c. If opacity ≠ 0 or brush:
      - if brush and count ≥ 0: count += 1; if count > 4: L = L − 15 (0 if that is below 0) and count = −1 (no
        more cuts on this tile). (So the fourth step on a brush tile cuts the line.)
      - sum += (cur = prev and brush: the tile opacity of cur recomputed at the new (x, y) with this steps count;
        else opacity).
      - If sum > 56, the line is blocked and the walk ends: with F = 0 the result is 0; with F ≠ 0 the result is
        1 when fire_line (op 42) (S, T, thrown 1) ≥ 25 (32-bit signed), else 0. If sum ≤ 56, go on with step d.
   d. If (x, y) = (tx, ty): result 1. prev = cur.

**Tile opacity** of tile t at point (x, y) with a steps count n: by the tile's object o (tile byte 1):
- 0x37 ≤ o ≤ 0x82: (the window table byte [8 × o + D], no brush).
- 0x27 ≤ o ≤ 0x36 (brush, trees): brush; r = n / 4 clamped to 2..8; if max(|x − (tx₀ + 2)|, |y − (ty₀ + 4)|) < r
  with (tx₀, ty₀) = the tile's point ((t mod 56) × 16 + 5, (t / 56) × 14 + 5): opacity (n / 15) × 2 (low byte),
  else 0.
- o = 0xE2 or 0xE3: n / 30, no brush. Any other object: 0, no brush.

**Sharing sightings** (one soldier; a reaction flag). An enemy becomes the side's latest spotter. For each
opponent in play on which the soldier has a sight flag that is fresher than or equal to the side's knowledge of it
(freshness table), the side's knowledge takes the flag and the opponent's knownTile takes the soldier's last-seen
tile for it. Along the way a merc or guard (slots 0..15) that sees the opponent now (flag 1, opponent health 15
or more) counts:
- a **new sighting** when the side did not know the opponent (knowledge 0). A merc (slots 0..7) also practises
  stat 9 (2 rolls, see the merc stats) for each new sighting that does not see the merc back (the opponent's flag
  on the merc is not 1);
- a **new noise** when the side's knowledge was not 1 or 2.
The nearest of them by world distance (from the soldier's pixel position to the opponent's tile) is remembered; a
first new sighting replaces any noise found before it, and noises are not considered once a sighting exists;
ties keep the earlier slot.

With at least one new sighting or noise: a merc reacts (speech and interrupt handling, not yet specified) with
the nearest opponent and a code: none seen 0xFF; seen and none sees the merc back 0; else 2 with the reaction
flag, 0xFF without; if it is the selected merc on the tactical screen it also stops (two routines) and sets its
byte +0x197 to 1. A guard (slots 8..15) with a new sighting focuses the view on itself and, if the nearest is
visible to the player (byte +0x9C > 0) or a global option (0x93458) is set, on the nearest too; then the message
"Guard sees a new enemy!" shows.

Last, in every case, the side's call takes the soldier's call tile and level if the soldier's level is higher.

**Target tile, noise, calls.** When a soldier calls out a sighting, its side remembers the call's tile and level
(one record for the mercs' side, one for the enemies'), and for an enemy also which soldier called. The target
tile, the noise heard, the news value and the calling ally are exact procedures under Search procedures.

### Sightings as steps (library ops 35 perceive, 115 sight_check, 34 share_sightings, 63 enemy_turned, 64 guard_turned, 327 clear_rooms)

The paragraphs above, as steps. S = the soldier (slot = its +0x17A), O = record i. **In play**(r) = record r has
+0x182, +0x138 and +0x183 all ≠ 0. **Opponents** of S: slots 0..0x27 for an enemy (slot ≥ 0x28), with flag index
j = i; slots 0x28..0x2F for anyone else, with j = i − 0x28. S's **sight flag** on O = s8 S +0x13A + j. **Sides
differ** = one of the two slots is below 0x28 and the other is 0x28 or more.

**Sight range table** (73 bytes, index 8 × facing + d; out of 0..72 reads 0):
`00 37 00 ff ff c7 ff c7 ff`, then for each facing 1..8 the 8 bytes for d = 1..8: 14 when d = facing, 12 one
direction away, 8 two away, 3 three away, 0 opposite (around the circle of 8; facing 1 row: `0e 0c 08 03 00 03
08 0c`).

**Sight check** (op 115 sight_check (slot S, slot i); result low byte 1 = a new sighting, else 0):
1. If slot ≥ 0x30 or O +0x17A ≥ 0x30: 0. If not (in play(S) and in play(O), sides differ, s8 S +0xBA ≠ 2): 0. If
   u16 S +0x1E8 + 2 × S +0x1F3 < 4: 0. If s8 S +0xBA = 4 and s8 S +0x9C ≤ 0: 0.
2. old = S's sight flag on O (signed). seen = 0.
3. If s8 S +0xBA = 4: if s8 O +0x13A = 1: host op 327 clear_rooms (); seen = 1.
4. Else: r = 14 if old ≠ 0; else d = host op 6 direction (s16 S +0x116, s16 S +0x118, s16 O +0x116, s16 O +0x118)
   & 0xFF and r = range table [8 × s8 S +0x1A1 + d]. If (host op 7 tile_dist (s16 S +0x114, s16 O +0x114) & 0xFF)
   ≤ r and host op 49 sight_line (slot, s16 S +0x116 + 2, s16 S +0x118 + 4, s16 O +0x114, s16 O +0x116 + 2, s16 O
   +0x118 + 4, old, r, i) ≠ 0 (32 bits): seen = 1.
5. If seen: Mark seen (S, i) (shooting.md). Then if old is 1 or 2: if slot < 0x28: knowledge[i] = 1; result 0.
   Else result 1.
6. Not seen: if old = 1: +0x1D5 = +0x1D5 − 1 (byte); if it is now 0: +0xBB = 2. Sight flag = 2. Remember byte
   (Mark seen step 2, indexed by i) 0xFF → 1. If s8 knowledge[i] = 1 and No ally sees (S, i): knowledge[i] = 2.
   Result 0.

**No ally sees** (S, i): b = O +0x17A. If s8 S +0xBA < 3: slots 0..15 and index b − 0x28; else slots 0x28..0x2F
and index b (low byte). Result 0 if a slot k ≠ slot of those, in play with s8 +0x183 ≥ 15, has byte +0x13A +
index = 1; else 1.

**Perceive** (op 35 perceive (slot, int age)):
1. If s8 +0xBA = 2: done. new = 0 (byte).
2. For each opponent i with in play(i) and a sight flag ≠ 0: new = new + low byte of host op 115 sight_check
   (slot, i). If age & 0xFF ≠ 0: v = the flag (signed, read again); v ≥ 2: v + 1, past 4 → 0; v ≤ −1: v − 1,
   below −3 → 0; the flag = v.
3. If new ≠ 0 and slot < 0x28: host op 34 share_sightings (slot, 0).

**Share sightings** (op 34 share_sightings (slot, int flag)):
1. If slot ≥ 0x28: last_spotter = slot. best = 1500, near = 0x40, seen = 0, heard = 0 (bytes), unseen = 1.
2. For each opponent i, in order, with in play(i) and flag f = sight flag ≠ 0, known = s8 knowledge[i]: skip it
   unless fresher(known, f) ≠ 0 or f = known. Then:
   a. If slot < 0x10, s8 O +0x183 ≥ 15 and f = 1: if known = 0: seen + 1; d = World distance (s16 S +0x116, s16 S
      +0x118, px(O tile), py(O tile)) (O tile = s16 O +0x114); if seen = 1 or s16 d < s16 best: best = d, near = i.
      Then if slot < 8: if s8 O +0x13A + slot = 1: unseen = 0; else host op 105 stat_practice (slot, 9, 2, 0).
      Else if known is not 1 or 2: heard + 1; if seen = 0: d as above; if heard = 1 or s16 d < s16 best: best = d,
      near = i.
   b. knowledge[i] = the flag byte. u16 O +0x108 = last_seen_m u16 [slot × 8 − 40 + O +0x17A] if slot < 0x28,
      else last_seen_e u16 [(slot − 0x28) × 40 + O +0x17A]. (SPEC GAP (not seen: the game always has O +0x17A = i, so the index stays in the table): an entry outside the table reads 0;
      the game always has O +0x17A = i.)
3. If heard ≠ 0 or seen ≠ 0:
   - slot < 8: how = 0xFF if seen = 0; else 0 if unseen = 1; else 2 if flag & 0xFF ≠ 0; else 0xFF. Host op 184
     merc_sees (slot, how, near). If screen_drawn ≠ 0 and slot = selected: host op 72 stop_walk (slot, 0), op 53
     stop_anim (slot, 0), +0x197 = 1.
   - else if seen ≠ 0: host op 99 focus (slot, 0). If s8 near record +0x9C > 0 or follow_all ≠ 0: spotted = near,
     focus_last = near, focus_slot = near, host op 99 focus (near, 0). Host op 326 guard_sees_msg ().
4. Side call c = calls +0 if slot < 0x28, else calls +4: if c +2 (u8) < S +0x1C0: u16 c = u16 S +0x122, c +2 =
   S +0x1C0.

**Enemy turned** (op 63 enemy_turned (slot)): for i = 0..0x27 with in play(i): host op 115 sight_check (slot, i).

**Guard turned** (op 64 guard_turned (slot)): for i = 0x28..0x2F with in play(i): host op 115 sight_check (slot,
i). Then if sees_slot = S +0x17A: host op 197 sees_msg (slot).

**Clear rooms** (op 327 clear_rooms ()): tile byte 3 = 0 for tiles 232 + 56 × r + c (r = 0..6, c = 0..5) and
625 + 56 × r + c (r = 0..2, c = 0..4).

*Evidence (2026-10-24): read in the asm of SightCheck 0x72F50, 0x73268, UpdateSightings 0x740FC, ShareSightings
0x736BC, 0x73578, 0x7351C, 0x61744 and the range table JA.EXE 0x92FC3; matches the diff-tested mirrors
tools/ai.py sight_check / no_ally_sees / update_sightings / share_sightings.*

## Positions

Leash, standable tiles, dry / patrol / buddy / approach / advance tiles: see Search procedures (exact steps).

Exact procedures (port slice 2b part 2) for the retreat tile, the position score and the search position follow.
Conventions as in "Search procedures"; dist(a, b) for two tiles = s16(D(px(a), py(a), px(b), py(b))) (World
distance). "Counts" = u8 inCurSector ≠ 0, u8 inPlay ≠ 0 and s8 health ≥ 15. Opponents here: for me ≥ 0x28 the
slots 0..15 only (j = slot), else the slots 0x28..0x2F (j = slot − 0x28). "Known" = u8 sight flag of S on the
opponent ≠ 0 or u8 knowledge[opponent slot] ≠ 0; its place is the Known place (Search procedures).
power(O) = s16(Soldier power of O against my, F = 0) (Situation score), where my = s16 tile (+0x114) of S; it may
make the host call direction.

**Box** (shared by the retreat tile and the search position):
1. r = 3, 5, 7 for difficulty 0, 1, 2. If s8 alertState (+0xBB) = 3: half = u8(cdiv(s8 apLeft (+0x1A5), 2));
   r = the smaller of r and half (unsigned).
2. col = my − cdiv(my, 56) × 56, row = cdiv(my, 56). left = min(r, col), right = min(r, 55 − col),
   up = min(r, row), down = min(r, 55 − row) (signed). Each is then kept as a byte (u8).
3. The scan: for dy from s8(u8(−up)) to s8(down), and inside for dx from s8(u8(−left)) to s8(right), both
   inclusive and ascending: candidate c = s16(my + dx + dy × 56).

**Retreat tile** (op 21) of soldier S:
1. near = 1500; an empty list K.
2. For each opponent O in slot order: skip O unless O counts and is known. p = power(O); skip O if p = −999.
   t = its Known place (u16). d = s16(D(s16 px (+0x116 of S), s16 py (+0x118 of S), px(s16 t), py(s16 t))) — the
   soldier's own pixel position, not its tile's. near = min(near, d). Append t to K.
3. If K is empty: the result is NONE.
4. best = near, result = NONE. For each candidate c of the Box:
   a. Skip c unless can_stand_at(S, c) (op 18, a library op) gives a non-zero low byte.
   b. ap = Path AP (op 40) of c. Skip c if ap = 0, or if s8 alertState = 3 and ap > s8 apLeft.
   c. far = 1500; for each t in K in order: far = min(far, dist(c, s16 t)).
   d. If far > best: best = far, result = u16(c).
5. The result (low 16 bits).

**Position score** of soldier S (slot m) standing on tile A against opponent O (slot o) on tile B, with a power P
and a distance D (all three as given by the caller):
1. For (m, A) and then (o, B): let old = u16 tile of that record. If u16 of the given tile equals old, nothing
   happens for this record. Otherwise remember old and the record's u16 px (+0x116) and py (+0x118), then write
   tile = the given tile, px = px(s16 tile), py = py(s16 tile) (16 bits each).
2. theirs = u8(100 − host fire_line (op 42) with o, s16 A, byte 1).
3. raw = u8(host fire_line with m, s16 B, byte 1). mine = 100 if s8(raw) < 25, else u8(100 − raw).
4. Restore, first S, then O: a record changed in step 1 gets its old tile, px and py back, unless its old tile was
   NONE (0xC41): then it keeps the new values (this is what the original does).
5. The result = i32((300 − s16 D) × i32(s16 P × s8(u8(theirs − mine)))).

**Search position** (op 12; mode byte: 0, 1 = do not get nearer, 2 = do not get farther) of soldier S:
1. near = 1500, nearTile = NONE, cur = 0, an empty target list.
2. For each opponent O in slot order: skip O unless O counts and is known. t = its Known place (u16);
   d = dist(my, s16 t). Skip O if d > 300. p = power(O) (only now, after the distance test); skip O if p = −999.
   Append (t, O's slot, p). cur = i32(cur + Position score(S on my, O on t, P = p, D = d)). If d < near: near = d,
   nearTile = t.
3. If there are no targets: the result is NONE.
4. Box steps 1 and 2. Then, if s8 alertState ≠ 3 and leash r < 99 (Leash, Search procedures; post = s16 +0x12E):
   with over(t) = u8(host tile_dist (op 7) with t, post) − r, in this order:
   a. v = over(my − s8(left)); if v > 0: left = u8(left − v).
   b. v = over(my + s8(right)); if v > 0: right = u8(right − v).
   c. v = over(my − s8(up) × 56); if v > 0: up = u8(up − v).
   d. v = over(my + s8(down) × 56); if v > 0: down = u8(down − v).
   The tile_dist arguments are passed as computed (s16).
5. best = cur, result = NONE. For each candidate c of the Box scan (Box step 3):
   a. Skip c unless can_stand_at(S, c) gives a non-zero low byte.
   b. If u8 mode ≠ 0: dm = dist(c, s16 nearTile); with mode 1 skip c if dm < near; with mode 2 skip c if dm > near.
   c. ap = Path AP of c; skip c if ap = 0, or if s8 alertState = 3 and ap > s8 apLeft.
   d. total = 0; for each target (t, o, p) in order: total = i32(total + Position score(S on c, O on t, P = p,
      D = dist(c, s16 t))).
   e. If total > best: best = total, result = u16(c).
6. If result is NONE: NONE. m = max(|best|, |cur|) as i32 (|−2³¹| stays −2³¹). If m ≤ 0, or
   s16(cdiv(i32((best − cur) × 100), m)) < 20: NONE. Else the result (low 16 bits).

## Search procedures

Exact procedures for the searches the decisions call (port slice 2b). Each step runs in the order written; every
host call and every random draw is part of the observable behaviour. Unless stated, comparisons are signed and
arithmetic is 32-bit; "s16(v)" = the low 16 bits of v read as signed, "u16(v)" = read as unsigned, "s8" / "u8" the
same for the low byte. NONE = tile 0xC41 (3137). World data used here (port_api.md): knowledge, fresher, last_seen_m,
last_seen_e, calls, last_spotter, paths, dir_steps, t0_class, t1_class.

### Shared definitions

- **Tile parts.** For a tile number t (signed): row(t) = t / 56, column(t) = t − row(t) × 56 (C division, so a
  negative t gives a negative row and column). Pixel point: px(t) = column(t) × 16 + 5, py(t) = row(t) × 14 + 5.
  Tile bytes: byte 0 = ground (t0), 1 = object (t1), 3 = t3, 5 = occupant (low 7 bits; 0x30 and up = nobody) with
  flag 0x80. occupant(t) = byte 5 & 0x7F when below 0x30, else 0x40 (nobody).
- **World distance** D(x1, y1, x2, y2) = floor(sqrt(dx² + dy²)) with dx = |x1 − x2| × 15 / 16 and
  dy = |y1 − y2| × 15 / 14 (each truncated; the square root is the exact integer square root, rounded down).
- **Tile distance** = host tile_dist(a, b) (op 7). Where a step says "low byte", only u8 of the result counts;
  otherwise the full 32-bit signed result.
- **Step from a tile.** step(t, k) for a step value k: the step leaves the map when column(t) is 0 or 55, or
  t ≤ 56, or t ≥ 0xC08, and one of these holds for n = t + k: n < 0; column(t) = 0 and column(n) = 55;
  column(t) = 55 and column(n) = 0; t ≥ 0xC08 and n ≥ 0xC40. A step that leaves the map gives t, else n.
  Direction steps: dirStep(d) = dir_steps[d] (signed 16-bit entry d of the table; d = 1..8 are N, NE, E, SE, S,
  SW, W, NW = −56, −55, +1, +57, +56, +55, −1, −57; other indexes read the table as it is).
- **Alive** (an opponent or ally record): inCurSector (+0x182) ≠ 0, inPlay (+0x138) ≠ 0 and health (+0x183) ≠ 0.
- **Opponents** of soldier S (slot `me`): for me ≥ 0x28 the slots i = 0..0x27 with index j = i; else the slots
  i = 0x28..0x2F with index j = i − 0x28. S's sight flag on opponent j = s8(sees[j]) (record +0x13A + j); the side's
  knowledge of i = s8(knowledge[i]).
- **Freshness**: fresher(b, a) = fresher[1152 + 8 × b + a] (u8; b = the older level, a = the newer, both signed
  bytes). Non-zero = a is fresher than b.
- **Last-seen tile** of opponent j for S: for me ≥ 0x28 the u16 last_seen_e[(me − 0x28) × 40 + j], else
  last_seen_m[me × 8 + j] (S a merc or guard, slot < 16).
- **Known place** of opponent i (index j): with a = sight flag, b = knowledge: none when a = 0 and b = 0;
  otherwise the last-seen tile when fresher(b, a) ≠ 0 or a = b, else the opponent's knownTile (+0x108). (u16)
- **Side call**: for me < 0x28 the mercs' side call (calls +0 tile u16, +2 level u8), else the enemies' (calls +4
  tile, +6 level). S's own call = noiseGridno (+0x122, u16) and noiseVolume (+0x1C0, u8).
- **Leash** r by orders (+0x1D4, unsigned byte): 0 → 1, 1 → 5, 2 → 15, 3 or 4 → 99, any other value → 0.
  post = s16(+0x12E).
- **Path AP** = s16(host path_ap(slot, tile)) (op 40); 0 means unreachable. **Find path** = host find_path(slot,
  tile, flag) (op 41); the full 32-bit result ≠ 0 means found (the host stores the path in `paths` and pathLen).

### Soldier busy

Host op 27 busy (slot); a library op. Result (int) 1 when the soldier's animation word u16 +0x1E8 + 2 × +0x1F3
is 0x1B, 0x1F or 0x23, else 0. Original: SoldierBusy 0x5AD1C.

### Swap hands

Host op 16 swap_hand (slot, byte a, byte b); a library op. The item-place table is the 7 bytes
`00 07 08 09 0A 0B 01` (index 0..6; a and b are signed bytes, the library only passes a = 0 and b = 6, so the main
hand, item 0, and the second hand, item 1, change places; SPEC GAP (not seen: the library passes only a = 0 and b = 6): other indexes are not used).
1. ia = table[a], ib = table[b]. The 12-byte items ia and ib of the soldier's record change places (copy item ia
   aside, copy item ib onto item ia, copy the kept item onto item ib).
2. If the soldier's slot (+0x17A) = the selected soldier: host op 166 item_panel (slot, 0, 1).
Original: SwapHand 0x55B0C (table at 0x93D59, copies with 0x47090, panel 0x55FD8).

### Standable tile

tile_standable(t), used by can_stand_at:
1. o = byte 5 of t & 0x7F. If o < 0x30 and s8(record o +0x9C) > 0: result 0.
2. If t1_class[t1] ≤ 5 (unsigned): result 1.
3. By object t1: 0x3B, 0x41, 0x5A, 0x60 → 1; 0x3D, 0x3E, 0x5C, 0x5D → −2; 0x43, 0x45, 0x62, 0x64 → −1; any
   other object: 1 if t3 = 0x73, else 0.

can_stand_at(S, t) (op 18; t signed 16-bit), result 1 or 0. Defined for map tiles 0..3135 only (the original
reads past the map for other values; the library reads those bytes as 0):
1. If s8(aiType +0xBA) = 1, combat = 0 and t0_class[t0] = 3: 0.
2. If tile_standable(t) ≤ 0: 0.
3. If t0 ≥ 0x6C and t1 is neither 0x1C nor 0x1F: 0.
4. If byte 5 has flag 0x80 and S's item entry 3 type (+0x24) is not 0x28: 0.
5. If occupant(t) ≠ 0x40, or u16(t) = S's tile (+0x114): 0.
6. Find path(S, t, 0). If it is 0, or u16(t) = walkFailTile (+0x10E): 0. (The host call happens before the
   walkFailTile test.)
7. Result 1.

### Patrol tile (op 4)

1. r = leash. boxed = r < 55.
2. If boxed: col = column(post), row = row(post); left = min(r, col), right = min(r, 55 − col), top = min(r, row),
   bottom = min(r, 55 − row); wide = s8(left + right + 1), high = s8(top + bottom + 1).
3. Up to 10 times:
   - boxed: dx = random(wide), then dy = random(high); t = s8(dx − left) + post + s8(dy − top) × 56.
   - not boxed: t = random(0xC40).
   - If can_stand_at(S, s16(t)) ≠ 0: result t (the full 32-bit value).
4. Result NONE.

### Buddy tile (op 5)

1. aiType (unsigned byte) ≤ 2: n = 0x28, base = 0; ≤ 4: n = 8, base = 0x28. Other types: result 0, nothing else
   happens.
2. r = leash. Mark S's own slot as tried (when it lies in base..base + n − 1); no other slot is tried yet.
3. Repeat n − 1 times:
   a. Draw k = u8(random(n) + base) until k is not tried (at most 100000 draws; if every draw hits a tried slot,
      result 0).
   b. Buddy B = record k. If B is not alive: mark k tried and continue with the next repeat.
   c. If r < 99 and (post = NONE, or tile distance(post, s16(B tile)) − 1 > r): continue (k stays untried; the
      host call is skipped when post = NONE).
   d. 8 times: nb = step(s16(B tile), dirStep(random(8) + 1)). If r < 99 and tile distance(post, nb) > r: next
      try. If can_stand_at(S, nb) ≠ 0: aiTarget (+0x132) = u16(nb); result 1.
4. Result 0.

### Approach tile (op 10; a target tile)

1. r = leash; t = u16(target). If S's tile = t: result NONE.
2. If can_stand_at(S, s16(t)) = 0: 8 times: d = random(8); nb = step(s16(t), dirStep(d + 1)); if
   can_stand_at(S, nb) ≠ 0: t = u16(nb) and stop. When all 8 fail: result NONE.
3. If r < 99 and tile distance(post, s16(t)) > r: t = S's tile; k = 0; while k < s16(pathLen +0xD4) (pathLen and
   `paths` are read again before each pass, as a host call may change them): nb = step(s16(t), dirStep(paths[me ×
   80 + k] + 1)); if tile distance(post, nb) > r: stop; if can_stand_at(S, nb) ≠ 0: t = u16(nb); k = k + 1.
4. Result NONE if t = S's tile, else t.

### Dry tile (op 3)

1. m = s16(S's tile); col = column(m), row = row(m); best = 1000; result = NONE.
2. For R = 5, 10, 15, 20, 25:
   a. left = min(R, col), right = min(R, 55 − col), up = min(R, row), down = min(R, 55 − row).
   b. For dy = −s8(up) .. s8(down), and inside it dx = −s8(left) .. s8(right) (both inclusive):
      t = s16(dx + m + dy × 56); if can_stand_at(S, t) ≠ 0: ap = path AP(t); if ap ≠ 0 and ap < best:
      best = ap, result = u16(t).
   c. If result ≠ NONE: stop.
3. Result (best carries over from one R to the next).

### Target tile (op 23)

1. m = s16(S's tile); best = 1500; result = NONE.
2. For each opponent i (index j) that is alive, with sight flag a and knowledge b not both 0: t = known place;
   skip it if t = u16(S's tile); d = s16(D(px(m), py(m), px(s16(t)), py(s16(t)))); if d < best: best = d,
   result = t.
3. Result.

### Noise tile (op 2; also gives a loudness)

1. m = s16(S's tile); best = −10000; result = NONE. "Offer (target, factor, value)": v = s16(u8(tile
   distance(m, target)) × factor); if v > best: best = v, result = value.
2. For each opponent i (index j) that is alive: if a = sight flag < 0: offer(s16(last-seen tile), a, last-seen
   tile). Then if b = knowledge < 0: offer(s16(knownTile of i), b, knownTile of i).
3. If s16(noiseGridno) ≠ NONE: if noiseGridno = S's tile: noiseGridno = NONE, noiseVolume = 0. Else offer(s16(noiseGridno),
   (noiseVolume >> 1) − 6, noiseGridno).
4. t = s16(side call tile); if t ≠ NONE and u16(t) ≠ S's tile: offer(t, (side call level >> 1) − 6, u16(t)).
5. Result = result; loudness = u16(best) (16 bits; readers use it as signed 16-bit).

### News value (op 8)

1. side = side call level (u8). v = noiseVolume − side when noiseVolume > side (unsigned bytes), else 0.
2. For every opponent i (index j), alive or not: v += fresher(knowledge b, sight flag a).
3. Result u8(v).

### Calling ally (op 11)

1. Allies: slots 0..0x27 for me < 0x28, else 0x28..0x2F. best = 1000; result = NONE.
2. For each ally A (slot order), not S and alive: skip it when its underFire (+0x1C4) = 0 and last_spotter ≠ A's slot
   byte (+0x17A). ap = path AP(s16(A tile)); if ap ≠ 0 and ap < best: best = ap, result = A tile (u16, read again after the
   path_ap call).
3. Result (only the low 16 bits are defined).

### Advance tile (op 22; a byte "weak")

1. strong = (1000, NONE), weakest = (1000, NONE).
2. For each opponent i (index j) that is alive: skip it when its sight flag and the knowledge are both 0 (no known place); t = known place
   (a stored 0xC41 is a tile like any other here); skip when t = u16(S's tile). ap = path
   AP(s16(t)); if ap ≠ 0: the pair is strong when s8(health of i) ≥ 15, else weakest; if ap < its AP: it becomes
   (ap, t).
3. best = 1000, result = NONE. If strong's tile ≠ NONE: (best, result) = strong. Else if u8(weak) ≠ 0 and
   weakest's tile ≠ NONE: (best, result) = weakest.
4. If s16(noiseGridno) ≠ NONE: if noiseGridno = S's tile: noiseGridno = NONE, noiseVolume = 0. Else ap = path AP(s16(noiseGridno));
   if ap ≠ 0 and ap < best: best = ap, result = noiseGridno.
5. t = side call tile (u16): if s16(t) ≠ NONE and t ≠ S's tile: ap = path AP(s16(t)); if ap ≠ 0 and ap < best:
   result = t.
6. Result.

## Attack options

Each option fills a record: valid, target slot, aim clicks, hit chance %, score (32-bit), target tile; empty =
not valid, slot 0x40, tile none.

Exact procedures for the expected damage and the gun option (port slice 2c part 1) follow; the conventions of
"Search procedures" apply. World data used: item_types (byte 11 of an item type = its range in tiles), weapons
(6 bytes per item type, byte 5 = damage base "b5"), absorb (s16 per index: armour absorb of item t = absorb[t],
helmet absorb of item t = absorb[t + 15]) (port_api.md).

**Expected damage** of soldier S's main-hand gun against opponent O, for a hit chance c (byte):
1. d = s16(D(s16 px S (+0x116), s16 py S (+0x118), s16 px O, s16 py O)) (the pixel positions in the records).
2. t = u8 S byte +0 (the main-hand item). q = cdiv(d × 50, s16(range(t) × 15)) (the original divides by zero
   when the range is 0).
3. base = u8(cdiv(b5(t) × (100 − s16(q) + u8(c) / 4), 100)).
4. helm = 0; if u8 O byte +0x18 (helmet item) ≠ 0: helm = u8(cdiv(s8 O byte +0x1A × helmet absorb(O byte +0x18),
   100)).
5. body = 0; if u8 O byte +0x48 = 0x31: body = u8(body + cdiv(s8 O byte +0x4A × armour absorb(0x31), 100)).
   Then if u8 O byte +0x3C (body armour item) ≠ 0: body = u8(body + cdiv(s8 O byte +0x3E × armour absorb(O byte
   +0x3C), 100)).
6. v = s8(u8(base − (helm × 15 + body × 80) / 100)). The result is v, at least 1.

**Gun option** (op 24) of soldier S (slot me) fills the 10-byte option record:
1. Start: valid 0, slot 0x40, aim 0, chance 0, score 0, tile NONE (0xC41).
2. Opponents: if s8 aiType (+0xBA) ≥ 3, the slots 0..0x27 (j = slot); else the slots 0x28..0x2F (j = slot −
   0x28). my = s16 tile of S. ap = s8 apLeft (+0x1A5) of S, read again where used.
3. For each opponent O in slot order:
   a. Skip O unless u8 inCurSector, u8 inPlay and u8 health of O are all ≠ 0, and s8 sight flag of S on O
      (+0x13A + j) = 1. ot = s16 tile of O.
   b. cost = u8(host action_ap (op 17) with me, ot, byte 1). Skip O if cost > ap.
   c. line = u8(host fire_line (op 42) with me, ot, byte 0). Skip O if line = 0.
   d. sap = u8(r − 1), where r = host shot_ap (op 45) with me, NONE.
   e. top = min(ap − cost, 4) (signed). best = 0, aim = 0, chance = 0. For a = 0 .. top: c = u8(host
      chance_to_hit (op 46) with me, ot, byte a); v = s16(cdiv(c × ap, a + sap)) (the original divides by zero
      when a + sap = 0); if v > best: best = v, aim = a, chance = c.
   f. Skip O if best = 0. eff = u8(line × chance / 100). Skip O if eff = 0.
   g. p = s16(Soldier power of O against my, F = 1) (Situation score; it calls fire_line with O's slot).
   h. score = i32(i32(i32(best × Expected damage(S, O, chance)) × eff) × p).
   i. prev = the record's chance byte. Skip O if eff ≤ prev. If prev ≠ 0 and s16(cdiv(eff × 100, prev) − 100) < 50
      and score < the record's score (i32): skip O.
   j. Record: valid 1, slot = O's slot, aim, chance = eff, score, tile = u16 tile of O.
4. The record is the result.

**Melee hit chance** of soldier S against opponent O with aim clicks k (byte) (exact):
1. If u8 S byte +0 (main-hand item) is neither 0x0F nor 0x10: 0.
2. skill(R, first, second, k): v = s16(cdiv(s16(s8 R[first] × 3 + s8 R[second] + s8 maxHealth + s8 level × 10),
   6)); if u8 k ≠ 0: v = s16(v + u8 k × 5); if u8 R byte +0x1B9 = 4: v = s16(v − 20), and if v < 1 then v = 1.
   If s8 health < s8 maxHealth: lost = u8(u8 maxHealth − u8 health − u8 wounds) / 2 + s8 maxHealth − s8 health;
   v = s16(v − cdiv(v × 2 × lost, s8 maxHealth × 3)). If s8 energy < 100: v = s16(v − cdiv((100 − s8 energy) × v,
   200)). The result is v.
3. a = skill(S, dexterity +0x18C, agility +0x18B, k); d = skill(O, agility +0x18B, dexterity +0x18C, 0).
4. v = cdiv(a × 100, a + d) (the original divides by zero when a + d = 0). If S's slot ≥ 0x28: v += −10, 0, +10
   for difficulty 0, 1, 2. v = s16(v). The result is v clamped to 1..99.

**Knife damage** of S against O for a hit chance c and a sneak flag (exact):
1. b = u8(b5(t) + (u8 level − 1) × 2), t = u8 S byte +0 (b5: weapons, as for the expected damage).
2. lost = s8(u8(u8 maxHealth − u8 health − u8 wounds)); b = u8(s8 b + cdiv(cdiv(lost, 2) + s8 health − 50, 5)).
3. b = u8(s8 b + u8 c / 10).
4. b = u8(cdiv(s8 S byte +2 (the main-hand item's condition) × s8 b, 100)).
5. Unless sneak: if u8 O byte +0x48 = 0x31: b = u8(s8 b − cdiv(s8 O byte +0x4A × armour absorb(0x31), 100)). Then
   if u8 O byte +0x3C ≠ 0: b = u8(s8 b − cdiv(armour absorb(O byte +0x3C) × s8 O byte +0x3E, 100)).
6. The result is s8 b, at least 1.

**Knife option** (op 26) of soldier S (slot me) fills the option record:
1. Start as for the gun option. If s8 aiType ≥ 3: the opponents are slots 0..0x27 and own = 0x28; else the slots
   0x28..0x2F and own = 0. j = the opponent's index as for the gun option. my = s16 tile of S; ap = s8 apLeft.
2. For each opponent O in slot order:
   a. Skip O unless u8 inCurSector, u8 inPlay and u8 health of O are all ≠ 0, and s8 sight flag of S on O = 1.
      ot = s16 tile of O.
   b. cost = u8(host action_ap (op 17) with me, ot, byte 1). Skip O if cost > ap.
   c. sap = u8(r − 1), r = host shot_ap (op 45) with me, NONE. (No line of fire for a knife.)
   d. sneak = 0. If s8 O's sight flag on S (O byte +0x13A + me − own) ≠ 1, call host tile_dist (op 7) with my, ot;
      if the full result = 1: sneak = 1 when u8(|my − ot|) is 1 or 56 (not diagonal). No tile_dist call when the
      flag is 1.
   e. top = min(ap − cost, 4). best = 0, aim = 0, chance = 0. For a = 0 .. top: c = 99 if sneak, else Melee hit
      chance(S, O, a); v = s16(cdiv(ap × c, sap + a)); if v > best: best = v, aim = a, chance = c.
   f. Skip O if best = 0.
   g. p = s16(Soldier power of O against my, F = 0).
   h. score = i32(i32(i32(Knife damage(S, O, chance, sneak) × best) × chance) × p).
   i. prev = the record's chance byte. Skip O if chance ≤ prev. If prev ≠ 0 and s16(cdiv(chance × 100, prev) −
      100) < 50 and score < the record's score: skip O.
   j. Record: valid 1, slot = O's slot, aim, chance, score, tile = u16 tile of O.

Exact procedures for the throw option (port slice 2c part 2) follow; conventions as above. ground(T) =
t0_class[byte 0 of tile T] (the ground class).

**Throw damage** of soldier S's main-hand thrown item to opponent O at tile T:
1. t = u8 S byte +0. (blast, gas) = (10, 60) for t 0x18, (22, 50) for 0x1A, (37, 0) for 0x1C, (0, g) for 0x16 with
   g = 75 if ground(T) = 3 else 37, and (0, 0) for any other item.
2. If blast ≠ 0: if u8 O byte +0x48 = 0x31: blast = u8(s8 blast − cdiv(armour absorb(0x31) × s8 O byte +0x4A,
   100)). Then if u8 O byte +0x3C ≠ 0: blast = u8(s8 blast − cdiv(s8 O byte +0x3E × armour absorb(O byte +0x3C),
   100)). Then if s8 blast < 1: blast = 1.
3. If gas ≠ 0 and u8 O byte +0x24 = 0x28 (gas mask): gas = u8(cdiv((100 − s8 O byte +0x26) × s8 gas, 100)).
4. v = cdiv(s8 gas, 3) + s8 blast.
5. cond = s8 S byte +2 if s8 S byte +1 (units left) = 1, else s8 S byte number ((6 + s8 S byte +1) & 0x1FF).
6. r = s8(u8(cdiv(s8(u8 v) × cond, 100))).
7. The result is cdiv(r, 2) if ground(T) ≥ 8, else cdiv(r, 2) + r.

**Throw AP** of S: 0 unless the kind of item u8 S byte +0 (item_types byte 2) is 0x95. Else m = u8(Max AP of S);
v = u8(cdiv(cdiv(m × 200, cdiv(s8 dexterity, 2) × 2 + 100) + 1, 2)) (div_zero when the divisor is 0); v = min(v,
m); the result is v, or 1 when v = 0.

**Throw chance** of S at tile T (s16) with aim clicks k (byte):
1. v = cdiv(s8 marksmanship + s8 dexterity, 2). v += 10 if T = s16 lastTargetTile (+0xD0). v += u8 k × 10 if
   u8 k ≠ 0. For S's slot ≥ 0x28: v += −10, 0, +10 for difficulty 0, 1, 2.
2. d = s16(D(s16 px S, s16 py S, px(T), py(T))). If u8 S byte +0x24 = 0x27: d −= cdiv(d, 10).
3. reach = range(u8 S byte +0) × 15 + s8 health − 50; reach = max(reach − cdiv((100 − s8 energy) × reach, 200), 0).
4. v += cdiv(reach, 2) − d. If d > reach: v = cdiv(v, 2).
5. If s8 health < s8 maxHealth: lost as in Max AP step 3; q = u8(cdiv(v × 2 × lost, s8 maxHealth × 3))
   (div_zero when 0); v −= cdiv((100 − (s8 level − 1) × 10) × q, 100).
6. If s8 energy < 100: q = u8(cdiv((100 − s8 energy) × v, 200)); v −= cdiv((100 − (s8 dexterity − 10)) × q, 100).
7. The result is v clamped to 1..99.

**Throw option** (op 25) of soldier S (slot me) fills the option record:
1. Start as for the gun option. Sides: for me ≥ 0x28 the allies are slots 0x28..0x2F and the opponents slots
   0..0x27 (j = slot); else the allies are slots 0..0x27 and the opponents 0x28..0x2F (j = slot − 0x28).
2. allyTiles = the u16 tiles of the allies whose u8 inCurSector, u8 inPlay and u8 health are all ≠ 0, in slot
   order (S itself is among them when it qualifies). my = s16 tile of S. ap = s8 apLeft.
3. targets = for each opponent O in slot order with u8 inCurSector, inPlay and health ≠ 0 and s8 sight flag of S
   on O = 1: (u16 tile of O, O's slot, s16(Soldier power of O against my, F = 0)).
4. For each target (t, k, ·) in order, with tt = s16 t, col = tt − cdiv(tt, 56) × 56, row = cdiv(tt, 56):
   left = min(col, 2), right = min(2, 55 − col), up = min(row, 2), down = min(2, 55 − row), each kept as a byte;
   for dy from s8(u8(−up)) to s8(down), inside for dx from s8(u8(−left)) to s8(right): c = s16(tt + dx + dy × 56).
   a. cost = u8(host action_ap (op 17) with me, c, byte 1). Skip c if cost > ap.
   b. line = u8(host fire_line (op 42) with me, c, byte 0). Skip c if line = 0.
   c. For each a in allyTiles in order: host tile_dist (op 7) with s16 a, c; if the full result ≤ 4: skip c
      (the remaining allies are not asked).
   d. dmg = 0. For each target (ot, ok, p) in order: dist = u8(host tile_dist with s16 ot, c); if dist ≤ 3:
      hit = u8(Throw damage(S, record of slot ok, c)); dmg = u16(dmg + u8(cdiv((100 − dist × 20) × hit, 100)) × p).
   e. tac = Throw AP of S.
   f. best = 0, aim = 0, chance = 0. For a = 0 .. min(ap − cost, 4): ch = u8(Throw chance(S, c, a));
      ch = min(u8(ch + ch / 2), 100); v = s16(cdiv(ap × ch, tac + a)) (div_zero when tac + a = 0);
      if v > best: best = v, aim = a, chance = ch.
   g. Skip c if best = 0. eff = u8(chance × line / 100). Skip c if eff = 0.
   h. score = i32(i32(eff × best) × s16 dmg). Skip c if score ≤ the record's score (i32).
   i. Record: valid 1, slot k, aim, chance = eff, score, tile = u16 c.

## Line of fire

The chance (%) that a shot from the soldier reaches a tile; 100 for its own tile. A throw is assumed when asked
for or when the main-hand item is of kind 0x95. The path runs pixel by pixel (Bresenham, major axis first; on a tie
the x axis is major) from the soldier's pixel position + (2, 4) to the tile's pixel position + (2, 4).
Height: a bullet starts at 4500 + a random 0..1000 and changes each pixel by ((4000 − start) × 2 / distance ± 1)
/ 2 (distance = the larger of |dx|, |dy|), never below 0. A throw starts at 8000, rises by ((cap − 8000) × 2 /
(distance / 2) + 1) / 2 per pixel up to the cap (9000 when +0x1B3 is 3, else 22000), then falls by (−cap × 2 /
(distance / 2) − 1) / 2. At every pixel on a tile other than the soldier's own, the projectile test (the same as
for real shots, with an initial "ignore first part" distance when the sight-line test sees a line) runs: if it
stops the projectile, the result is the chance so far when on the target tile, else 0; otherwise chance = chance ×
the tile's pass-through % / 100. A path leaving the map gives 0 (cannot happen for paths inside the map).

**Exact procedure** (port slice 2c part 3, op 42) for soldier S (slot me), tile T (s16) and a thrown flag (byte).
Conventions as in "Search procedures"; vt(x, y) = cdiv(x, 16) + cdiv(y, 14) × 56 (the tile under a pixel).
1. my = s16 tile of S. If my = T: the result is 100 (no calls).
2. tx = px(T) + 2, ty = py(T) + 4; x = s16 px S (+0x116) + 2, y = s16 py S (+0x118) + 4.
3. d = u8(host direction (op 6) with x, y, tx, ty) — these pixel values, not tile points.
4. dist = max(|x − tx|, |y − ty|).
5. skip = 0. If sight_line (op 49) with me, x, y, T, tx, ty, 0, 0x0E, 0x40 gives a full result ≠ 0:
   skip = u8(dist).
6. item = 1 if u8 thrown ≠ 0, else u8 S byte +0. throw = (the kind of item, item_types byte 2, is 0x95).
7. If throw: cap = 9000 if u8 S byte +0x1B3 = 3, else 22000; h = 8000; half = cdiv(dist, 2) (div_zero when 0);
   slope = cdiv(cdiv((cap − 8000) × 2, half) + 1, 2); down = cdiv(cdiv(−cap × 2, half) − 1, 2).
   Else: cap = 0, down = 0; h = random(1001) + 4500; e = 4000 − h; slope = cdiv(cdiv(e × 2, dist) + (1 if e ≥ 0
   else −1), 2) (div_zero when dist = 0).
8. ax = tx − x, ay = ty − y; sx = −1 if ax < 0 else 1, sy = −1 if ay < 0 else 1; ax = |ax|, ay = |ay|. If ax ≥ ay:
   major = ax, minor = ay, (mx, my') = (sx, 0); else major = ay, minor = ax, (mx, my') = (0, sy).
   err = 2 × minor − major.
9. prev = vt(x, y); flown = 0; pct = 100.
10. Repeat:
    a. If err < 0: x += mx, y += my', err += 2 × minor. Else: x += sx, y += sy, err += 2 × minor − 2 × major.
    b. t = vt(x, y); flown += 1; h = max(h + slope, 0). If cap ≠ 0 and h > cap: h = cap, slope = down.
    c. If t ≠ prev and t ≠ my: if the step from prev to t leaves the map ("Step from a tile", the leave-map test
       with n = t): the result is 0. prev = t.
    d. If t ≠ my: host hit_test (op 48) with h, x, y, flown, byte d, byte skip, cap, T, 1; it returns res and,
       through the out record, the new skip (u8 at +0) and a chance (i32 at +4). Before each call the out record
       holds skip at +0 and chance 100 at +4 (hit_test leaves them unchanged unless it writes them). skip = the new skip. If res
       ≠ 0 (all 32 bits): the result is pct if t = T, else 0. Otherwise pct = cdiv(pct × chance, 100).
    e. If t = T: the result is pct.


## Walking cost

"Can walk" compares AP left with the cheapest single step: the lowest step cost (the movement rules' cost to enter
a tile, walking normally: no sneaking, no Alt, no crouch flag) over the 8 neighbouring tiles, 99 if none. A
neighbour that would leave the map is skipped: from the left column a step that lands in the right column (and the
reverse), any step to a negative tile, and from the bottom row area (tile ≥ 0xC08) any step to tile ≥ 0xC40.
Directions 1..8 = N, NE, E, SE, S, SW, W, NW (tile steps −56, −55, +1, +57, +56, +55, −1, −57).

Library op 13 move_ap (slot) (C export `ja1_move_ap`) gives this cost as an int: start value 99; for d = 1..8 the
neighbour n = step(s16 +0x114, dir_steps[d]) (n = the own tile when the step leaves the map); if n ≠ the own tile,
the cost = the step AP of n's move code (combat.md Movement, sneak, byte 177 and Alt all 0) as a signed byte; the
result is the lowest. *Evidence (2026-10-25): MoveApCost 0x1B118 asm; mirror tools/ai.py move_ap_cost (ai_diff
move_ap_cost; --port).*

## Turning toward a direction

Library op 28 turn (slot, byte d, int ticks) (C export `ja1_turn`; the exact steps of Turning in Starting an
action below); result 1 when the soldier turns (or stops for lack of AP), else 0. a = the low byte of the animation word u16 +0x1E8 + 2 × +0x1F3; face = s8 +0x1A1.
1. If d = face or a < 4: result 0.
2. +0x1A3 = the low byte of turn way (face, d): 0 if face = d; else with k = |face − d|: 1 if k = 4; if face > d:
   −1 if k ≤ d + 8 − face, else 1; if face < d: 1 if k ≤ 8 − d + face, else −1.
3. a ≤ 10: it turns, with the turn animation. Else if anim_table column 0 of a = 0: result 0. Else if column 2 of
   a ≠ 0: it turns, no turn animation. Else a < 0x32: it turns with the turn animation; a < 0x3D: it turns, no turn
   animation; else result 0.
4. Short of AP: when slot < 8, +0x16B ≠ 0, combat ≠ 0 and 1 > s8 +0x1A5 (less s8 +0xEF while ap_warned[slot] = 0):
   stop_timer = 2000, ap_warned[slot] = 1, result 1 (nothing else changes).
5. Else: +0x1A2 = d; with the turn animation: op 52 set_anim (slot, 8, 0, 0); op 31 spend_ap (slot, 1, 10);
   u16 +0xD2 = s8 ticks (the low byte, sign-extended). Result 1.

*Evidence (2026-10-25): TurnTo? 0x1AA18, ApShort 0x1A9A8, TurnStep 0x19540 asm; mirror tools/ai.py turn_to (ai_diff
turn_to; --port).*

## Starting an action

**Turning** toward direction *d* (with a tick count): nothing if the soldier already faces *d* or its current
animation code (low byte) is below 4. The turn way is set: clockwise or counter-clockwise, whichever is shorter
(exactly opposite: clockwise). Animations 4..10 always turn and first switch to animation 8; others turn only if
the animation allows acting, and then only if it is a stepping animation, or below code 0x3D (below 0x32 also
switching to animation 8). A merc in combat mode short of 1 AP (keeping its AP reserve until its first stop of
the turn) stops instead; the turn still counts as started. Otherwise the turn target becomes *d*, sound 1 plays
(volume 10) and the tick count is stored (as a signed byte).

**Walking** to a tile: the walk target and the stored target become the tile and the last target tile is
cleared. If the current animation is a stepping one or the engaged counter is above 0, and record byte +0x1FE is
set, the soldier idles instead (see Idle); otherwise one animation step runs (below).

**Animation step** (every tick of a walking or standing soldier; it picks an animation code and starts it):
- The turn way is cleared when the soldier faces its turn target.
- The soldier is "at its step point" when its task state is 5 or more, its animation is below 4, or its tile and
  pixel position equal the step target's. Not at it: it walks on (and with a walk target set, the next leg starts).
  At it: with a walk target whose animation allows walking (the animation's "can act" flag, always for 0x31/0x32,
  never for animations 0..1 when health < 15 or energy is 0), it first faces the target if the animation is a
  stepping one, then the next leg starts and it walks; otherwise it stands.
- **Next leg**: an occupied walk target within 2 tiles (unless flag +0x1A7) stops the soldier on its own tile.
  Otherwise the walk target becomes the step target and the pathfinder runs to it (unless +0x1FA); no path: a merc
  shows message 13 and its task fields are cleared, the soldier stops on its tile and faces the target. With a
  path the turn target is the path's first direction + 1.
- Walking: animation 2 from animation 0, 0x0D from 1..2, else 0x15 (0x16 with +0x1A8, 0x18 with +0x177); in water
  (animation 0x32 and up) 0x34 / 0x37 / 0x3A by the ground (below 0x97, below 0x9A, else). The task step ends.
- Standing: AI type 2 uses 0x11 and turns to +0x1A4; others end a task of state 5+ (unless hit), use 4 (keeping
  animation 5), and get re-placed on their tile if +0x1CC is set; in water 0x33 / 0x36 / 0x39.
- A soldier hit since the last step (+0x195; cleared while alive) fires back when: not in combat mode, slots 0..15,
  hit by a merc, not the selected merc, health 15+, on land, animation 4+, a gun (items 1..16) in hand, and the
  standing code 4 was chosen: it starts animation 4 and attacks the attacker's tile with the gun.
- A queued animation (+0x1E1, signed) replaces the code; +0x1E3 is added to +0x1E2 and cleared.
- Codes below 1000 start (no code: nothing).

**Starting an animation** (code, "keep" and "reset" flags). Per animation the tables give: a fixed frame delay (0 =
computed), an AP cost (99 = free), a "no AP check" flag, a sound argument and a frame table (0x50 bytes; its first
word is the frame count).
- Nothing happens if the current animation cannot act, its frame count is at most 0x15E and "reset" is clear.
- The same animation again: only the frame delay is recomputed (and with "keep" the frame table is set again).
- Standing up (codes 4 or 0x15) from a lower posture first plays a transition: from 0: 0x0D then 2 (the AP check
  looks at 2); from 1: 0x0D; from 5: 7; from 0x0F: 0x10; from 0x17..0x18: 0x19 (clears +0x177); from 0x1A..0x1C:
  0x1D and from 0x1E..0x20: 0x21 (both also run a posture routine); from 0x22..0x24: 0x25. The checked
  transition must pass the AP check (below) or nothing happens; the queue then holds code, transition(s).
- Code 0x43 from animation 1 turns the soldier around (facing + 4, wrapping past 8).
- Other codes: "reset" empties the queue; codes below 2 or 0x3C and up clear the hit flag; the animation must pass
  the AP check unless its cost is 99, then it starts with its sound (code 0x24 seen by the player also plays
  sound effect 11).
- **AP check**: passes for "no AP check" animations; else it fails only for the selected soldier (or any, when asked
  with 99) in combat mode whose AP left (+0x1A5 + +0x1E2) is below the cost: a merc says "doesn't have enough
  Action Points!", others end their turn.
- Walking codes 0x15, 0x34, 0x37, 0x3A start the step sound; then the frame delay is computed and the frame table
  of the (new) current animation is set; then the frame routine runs (Animation script, below).

**Frame delay** (ticks per frame): the table value if not 0. Standing (4) and wading (0x33, 0x36): (100 − health) ×
10 + energy × 10, at least 50. Deep water (0x39): 4 × max(25, 100 − energy) + max(25, 100 − health) (byte
arithmetic). Others: base (50 for 2, 0x0D, 0x1A, 0x1D, 0x1E, 0x21, 0x22, 0x25, 0x3A; 100 for 0x17; 40 for 0x34;
80 for 0x37; record word +0xA4 for 0x16; else 0) + (40 while +0x1B8 is set, else 2 × the terrain extra: class 1–2:
1, 4–5: 2, 6–7: 4, 8–9: 6) + (100 − health, clamped 15..50) + for mercs (100 − energy, at most 50) + (100 − byte
+0x18B, clamped 20..70), for others 20.

**Starting the chosen action** (the last step of a tick; returns 1 while the action goes on, else 0). "End" =
aiTarget = no tile, aiAction = 0, return 0.

- aiAction 0: return 1.
- 1 (turn): host turn(slot, aiTarget low byte, 30) not 0 → return 1. Else apLeft = 0, rethink = 1, aiAction = 0,
  aiTarget = no tile, return 0.
- Walks (2, 3, 4, 5, 8, 13): host walk_to(slot, aiTarget). If the step tile differs from aiTarget afterwards (the
  walk stopped short): rethink = 1, walkFailTile = aiTarget, aiTarget = no tile, aiAction = 0, return 0. Else
  walkFailTile = no tile, justArrived = 0, return 1.
- Attacks (11, 12, 14, 16): host click(slot, aiTarget, main hand item), return 1.
- 9 (operate): host click(slot, aiTarget, 0), end.
- 10 (radio): host spend_ap(slot, 2, 0), wait(1000), *m* = low byte of radio_result(); mission_step = *m*; *m* ≠ 1
  (signed) → mission_slot = 0x40. End.
- 6 (shout) and 7 (alarm): for 7 from an enemy (slot ≥ 0x28) while no alarm is raised: alarm = 1, and if the
  mission step (signed) is above 0 and the mission target is not "no tile", host mission_cancel(). Then (both)
  host spend_ap(slot, 5, 0), share_sightings(slot, 0). End.
- Any other code: return 0, nothing changed.

*Evidence: differential test validate/diff/ai_diff.py runs every decision procedure of the original in an
emulator against tools/ai.py with fuzzed soldier records and scripted random numbers / sub-searches (12,000 cases,
every action code reached, 0 mismatches; the situation score, soldier power, sighting update, target tile, noise, news value, calling ally, patrol / buddy / approach / advance
tiles, standable / dry / retreat / search tile and walking cost with
every soldier slot and the knowledge tables fuzzed too). Mutating a single constant of the port is caught. The sight
check, mark seen, sharing sightings, sector value, idle, reaching the target, attack-ready check and sight line are
diff-tested the same way (2026-10-19); every single-point mutation of them is caught except a few that cannot change
any result.*

*Port evidence (2026-10-19): the clean-room port's 9 AI entry points (port/ja1logic, written from this page and
port_api.md) give 0 mismatches against the original in ai_diff.py --port (12,000 cases) and compose_diff.py --port
(117 dumps x 300 ticks, the mirror as host; 12 dumps outside a tactical sector skipped).*

## Animation script

Every animation has a script of 16-bit words (up to 40 per animation in one table, 0x50 bytes each); the soldier
keeps a pointer into it. Running the script ("frame routine") looks at the current word:
- **Frame numbers** (below 0x1B6) are shown and the routine stops, except: 5 and 10 (a sound when the option at
  0x1163C3 is on and the player sees the soldier), 0x2D, 0x93, 0x96, 0x8F, 0x98 (sounds when seen), 0x57 and 0x85
  (a sound always), 0x54 (an effect for the soldier's slot) — these go on to the next word. Frame 0x37 turns one
  step if the turn way is counter-clockwise and stops.
- **Commands** (0x1B6..0x1F3) do something and go on, unless noted "stops":
  0x1B6 a global flag (0x229E04) = −1; 0x1B7 in combat mode an effect for the tile at 0x92E4C; 0x1B8..0x1BC sounds
  when seen and on screen; 0x1BD clears +0x1BE; 0x1BE sets +0x1FE; 0x1BF speech (skips a word, stops); 0x1C0, 0x1E4
  and 0x1ED nothing; 0x1C1 an effect (skips a word, stops); 0x1C2 uses the target (+0x134): the special item when
  +0x1A6 is 0x4C and the quest check holds, else a door effect; clears the task (skips a word, stops); 0x1C3 skips
  5 words unless the turn target is 3 or 5; 0x1C4 sets +0x9D; 0x1C5 skips a word if the turn way is −1, 0x1C8 skips
  two, 0x1C9 skips one if it is +1; 0x1C6 jumps to word *n* (the next word) of the current animation (stops); 0x1C7
  at the turn target runs an animation step and finishes an AI turn, else starts animation 8 (stops); 0x1CA turns
  one step if the turn way is +1; 0x1CB fidget: 1 in 12, energy above 70, no task, no wounds: animation 9 (even
  facing) or 10 (stops); 0x1CC stops; 0x1CD..0x1D4 set facing and turn target to 1..8; 0x1D5 / 0x1D6 a popup and
  the walk stops; 0x1D7..0x1DA step 1..4 pixels forward, 0x1DB..0x1DE back; 0x1DF the turn target becomes the next
  path direction; 0x1E0 / 0x1E2 turn clockwise one step now, 0x1E1 / 0x1E3 counter-clockwise; 0x1E5 starts
  animation 0x2B (stops) if the tile behind or ahead is not standable or is water; 0x1E6 turn to a free way: back at the
  starting facing → animation 1 (stops); a blocked tile ahead (or facing 4 with an object 0x83+ east) turns
  clockwise and repeats the command 40 times (stops); 0x1E7 turns around; 0x1E8 an effect (skips a word, stops);
  0x1E9 animation 0xC: a use effect on the target, else buries a mine there (skips a word, stops); 0x1EA (stops),
  0x1EB effects; 0x1EC the walk ends, a dead soldier falls (stops); 0x1EE runs on, then fires again at +0xDC with a
  loaded gun (stops); 0x1EF launches a projectile; 0x1F0 toggles +0x1AD; 0x1F1 a soldier with health below 15 drops:
  forgets all sightings, ends AI and target, stops in place, falls (animation 0x2A) or, in water, drowns (stops);
  0x1F2 dies (stops); 0x1F3 an effect (stops). Unknown words stop.
- **Going on** advances the pointer one word (frame numbers below 0xB6 and 0x1CC are remembered) and runs the next
  word; a word of 500 or more starts animation word − 500 instead. Afterwards a soldier at its turn target finishes
  an AI turn, else animations flagged "turn while" turn one step toward the turn target.
- **A pixel step** moves along the facing (diagonals move 65 % of the step on the y axis). If the new point lies in
  another map tile, the soldier takes that tile only if it is standable and free (else the step is undone); the old
  tile is freed; animations flagged "steps tile" then aim at the new tile's point, others stop where they are.

### Frame delay (`ja1_anim_delay`)

Writes the ticks per frame of the soldier's current animation to +0xD2 (u16). a = the current animation code
(u16 at +0x1E8 + 2 × animIdx). Values "s8(u8(100 − v))" mean: 100 − v kept to its low 8 bits, then read as a
signed byte.

1. d = anim_rows[8 × a + 2] (u8). +0xD2 = d. If d ≠ 0, stop.
2. If a is 4, 0x33 or 0x36: +0xD2 = low 16 bits of (100 − s8 hp) × 10 + s8 energy × 10; then if s16 +0xD2 < 50,
   +0xD2 = 50. Stop.
3. If a is 0x39: h = s8(u8(100 − hp)), n = s8(u8(100 − energy)); +0xD2 = 4 × max(n, 25) + max(h, 25). Stop.
4. base = 50 if a is 2, 0xD, 0x1A, 0x1D, 0x1E, 0x21, 0x22, 0x25 or 0x3A; the i32 at +0xA4 if a is 0x16; 100 if a
   is 0x17; 40 if a is 0x34; 80 if a is 0x37; else 0.
5. +0x19F (u8) = 0 if 0x1A ≤ s16 a ≤ 0x27, else 2.
6. extra = 40 if `stopped` (+0x1B8) ≠ 0; else 2 × T[b1B3] with T[1] = T[2] = 1, T[4] = T[5] = 2, T[6] = T[7] = 4,
   T[8] = T[9] = 6 and T = 0 for every other value.
7. If slot ≥ 0x28: tired = 0, agil = 20. Else tired = min(s8(u8(100 − energy)), 50) and
   agil = max(min(s8(u8(100 − agility)), 70), 20).
8. hurt = min(max(s8(u8(100 − hp)), 15), 50).
9. +0xD2 = low 16 bits of hurt + tired + agil + base + extra (32-bit sum).

### Next animation (`ja1_anim_step`)

Picks the soldier's next animation when the current one ends and starts it. a = the current animation code (u16
at +0x1E8 + 2 × animIdx; read it again where noted). NONE = 0xC41. Record fields are u8 unless a width is given;
tiles in +0x114, +0x124, +0x12A, +0x134, +0x96 are u16 (signed where marked s16).

**Tile point:** for a tile t (s16), q = t / 56 truncated toward zero; px(t) = (t − 56 × q) × 16 + 5,
py(t) = q × 14 + 5.

**Direction** from (x1, y1) to (x2, y2) (32-bit): dx = x2 − x1, dy = y2 − y1. If dx = 0: 1 if dy < 0, 5 if dy > 0,
else 0. If dy = 0: 7 if dx < 0, else 3. Otherwise ax = |dx|, ay = |dy|; r = (1 if ax / ay > 2 else 0) when
ax > ay, else (2 if ay / ax > 2 else 0) (truncating division); then by the signs: dx < 0, dy < 0 → (8, 7, 1)[r];
dx < 0, dy > 0 → (6, 7, 5)[r]; dx > 0, dy < 0 → (2, 3, 1)[r]; dx > 0, dy > 0 → (4, 3, 5)[r].

**At the step point:** true unless +0x1B9 < 5 and a ≥ 4 and (u16 +0x114 ≠ u16 +0x124 or u16 +0x116 ≠ u16 +0x126
or u16 +0x118 ≠ u16 +0x128).

**Can walk** in animation code c: v = s8 anim_rows[8 × s16 c + 4]; if v = 0 and c is 0x31 or 0x32, v = 1; if
s16 c ≤ 1 and (s8 hp < 15 or energy = 0), v = 0. Can walk = v ≠ 0.

**Stop on the own tile:** u16 +0x126 = px(s16 +0x114), u16 +0x128 = py(s16 +0x114); host op 53 stop_anim
(slot, 1).

**Start a leg:**
1. t = s16 +0x12A. If t = NONE: +0x1FA = 0, done.
2. o = the occupant of tile t (byte 5 & 0x7F; 0x40 when that is 0x30 or more). If o < 0x40 and +0x1A7 < 1 and
   tile_dist(s16 +0x114, t) (host op 7, read as i32) < 3: stop on the own tile, done.
3. u16 +0x124 = u16 +0x12A; u16 +0x12A = NONE.
4. If +0x1FA = 0 and find_path(slot, s16 +0x124, 1) (host op 41) returns 0: if slot < 8, host op 54
   merc_message (slot, 0xD, 0x40, 0, 1) and clear the task: +0x1A6 = 0, u16 +0x134 = NONE, +0x1A7 = 0, +0x95 = 0,
   u16 +0x96 = NONE. Then stop on the own tile and +0x1A2 = direction(s16 +0x116, s16 +0x118, px(t2), py(t2))
   with t2 = s16 +0x124. Done.
5. +0x1A2 = paths[80 × slot] + 1 (8-bit); +0x1FA = 0; +0x1E1 = 0.

**Water animation** w(b): g = +0x1B2; b if g < 0x97, b + 3 if g < 0x9A, else b + 6.

**Procedure:**
1. code = 1000 (signed 32-bit). If +0x1A2 = +0x1A1: +0x1A3 = 0.
2. If the soldier is at the step point: moving = no; t = s16 +0x12A; if t ≠ NONE and it can walk in a: if
   anim_rows[8 × s16 a + 6] ≠ 0, +0x1A2 = direction(s16 +0x116, s16 +0x118, px(t), py(t)); start a leg; moving =
   yes. Otherwise (not at the step point): moving = yes; if s16 +0x12A ≠ NONE, start a leg.
3. If moving: +0x1B8 = 0; if s16 a < 0x32: code = 2 if a = 0, else 0xD if a ≤ 2 (a as u16: a code 0x8000 or more is not ≤ 2), else 0x18 if +0x177 ≠ 0, else
   0x16 if +0x1A8 ≠ 0, else 0x15; if s16 a ≥ 0x32: code = w(0x34). Then host op 50 end_task (slot, 0).
   Not moving and s16 a < 0x32: if s8 +0xBA = 2: code = 0x11 and +0x1A2 = +0x1A4. Otherwise: if +0x1B9 ≥ 5 and
   +0x195 = 0, host op 50 end_task (slot, 1); if s16 a ≠ 5, code = 4; if +0x1CC ≠ 0: +0x1CC = 0, host op 51
   replace (slot).
   Not moving and s16 a ≥ 0x32: code = w(0x33).
4. If +0x195 ≠ 0:
   - if hp ≠ 0: +0x195 = 0; item = byte +0x000; if combat = 0, slot < 0x10, +0x1DA < 8, selected ≠ slot,
     s8 hp ≥ 15, +0x1B2 < 0x6C, the current animation code (read again) ≥ 4, 0 < item ≤ 0x10 and code = 4: host op
     52 set_anim (slot, 4, 1, 0); +0x1E1 = 0; code = 1000; host op 30 click (slot, s16 +0x114 of the soldier in
     slot +0x1DA, item);
   - then if +0x1E1 ≠ 0: code = s8 +0x1E1; +0x1E1 = 0; +0x1E2 = +0x1E2 + +0x1E3 (8-bit); +0x1E3 = 0.
5. If code < 1000: host op 52 set_anim (slot, code low 16 bits, 1, 0).

### Starting an animation (`ja1_anim_set`; host op 52 set_anim)

Starts animation `code` (low 16 bits) with the flags `keep` and `reset` (low bytes). The animation queue is the
u16 array at +0x1E8 with the index animIdx = byte +0x1F3; cur = the current code = u16 at +0x1E8 + 2 × animIdx,
read again at every use (steps below change it). Record fields are u8 unless a width is given. The **script
position** +0x90 (u32) is a byte offset into `anim_scripts` (68 animations × 0x50 bytes; animation c starts at
c × 0x50). Table reads by code use the code as an unsigned index (the game only uses codes below 256).

**AP check** for animation a: passes if anim_free[a] ≠ 0. Else c = anim_rows[8 × a] (u8); it fails only if
selected = slot, combat ≠ 0 and s8 +0x1E2 + s8 +0x1A5 < c (signed sum against the unsigned byte). On a fail: if
slot < 8, host op 57 merc_text (slot, 1, 0x4B0); else +0x1B0 = 1.

**Start sound** for animation a: host op 31 spend_ap (slot, anim_rows[8 × a] as a byte, sound) with sound = the
i16 anim_sounds[a] sign-extended to 32 bits.

**Procedure:**
1. If anim_rows[8 × cur + 4] = 0 and the u16 word at anim_scripts[+0x90] ≤ 0x15E (unsigned) and reset = 0: stop.
2. If code = cur: Frame delay (ja1_anim_delay); if keep ≠ 0, +0x90 = cur × 0x50; +0x1AD = 0; host op 61
   anim_frame (slot). Stop.
3. If selected = slot and code is not 9 or 10: host op 55 ui_refresh (1 if path_preview ≠ 0, else 0).
4. Transition: first = second = 1000, chain = no, posture = no. If code is 4 or 0x15, by cur:
   0 → chain, first = 0xD, second = 2; 1 → chain, first = 0xD; 5 → chain, first = 7; 0xF → chain, first = 0x10;
   0x17 or 0x18 → chain, first = 0x19 and +0x177 = 0; 0x1A..0x1C → chain, first = 0x1D, posture; 0x1E..0x20 →
   chain, first = 0x21, posture; 0x22..0x24 → chain, first = 0x25; any other cur: no chain.
   Else if code = 0x43 and cur = 1: d = (+0x1A1 + 4) as a byte; if d > 8, d = d − 8 (byte); +0x1A2 = d,
   +0x1A1 = d; host op 56 anim_turn (slot, 0). (No chain.)
5. With a chain: if second ≠ 1000, pick = second when the AP check of second passes; else pick = first when the AP
   check of first passes (only one check runs). No pick: stop. Then: queue[animIdx] = code; animIdx += 1 (byte);
   queue[animIdx] = first; +0x1FE = 0. If second ≠ 1000: queue[animIdx] = first; animIdx += 1 (byte);
   queue[animIdx] = second; +0x1FE = 0. Start sound for pick. If posture: host op 58 reserve_ap (slot).
6. Without a chain: if reset ≠ 0, animIdx = 0. If code < 2 or code ≥ 0x3C: +0x195 = 0. If anim_rows[8 × code] =
   99: queue[animIdx] = code, +0x1FE = 0, start sound for code. Else: the AP check of code fails → stop; else
   queue[animIdx] = code, +0x1FE = 0, start sound for code, and if code = 0x24 and s8 +0x9C > 0: host op 60 sfx
   (11, 0x2B11, 0x58, 1).
7. If code is 0x15, 0x34, 0x37 or 0x3A: host op 59 walk_sound (slot, 1).
8. Frame delay (ja1_anim_delay); +0x90 = cur × 0x50 (u32); +0x1AD = 0; host op 61 anim_frame (slot).

### Frame routine (`ja1_anim_frame`; host op 61 anim_frame)

Runs the soldier's animation script from the script position (u32 +0x90, a byte offset into `anim_scripts`).
"Word at p" = the u16 at anim_scripts[p]. cur = the current animation code (u16 at +0x1E8 + 2 × animIdx, read again
at every use). Record fields are u8 unless a width is given; NONE = 0xC41. Tables indexed by an animation code use
it as an unsigned index (the game only uses codes below 256).

**Helpers**

- **Seen** = s8 +0x9C > 0.
- **On screen**: 1 if opt_onscreen = 0. Else (32-bit signed): y = s16 +0x118 if s16 +0x114 < 56, else s16 +0x118 −
  s16 +0x11C; x = s16 +0x116 − s16 +0x11A. 0 if x + 20 < view_x, or y + 20 < view_y, or x > view_x + view_w − 1, or
  y > view_y + view_h − 1; else 1.
- **Sound** (e, v, n) = host op 60 sfx (e, 0x2B11, v, n).
- **Blocked** tile t = tile_standable(t) < 1 (Standable tile) or (t0 ≥ 0x6C and t1 is neither 0x1C nor 0x1F).
- **Wrap** a direction value d (a byte read as s8): d > 8 → d − 8; d < 1 → d + 8; else d (all as bytes).
- **Turn way** from a to b (s8 values): 0 if a = b; else d = |a − b|; d = 4 → +1; a > b → −1 if d ≤ b + 8 − a,
  else +1; a < b → +1 if d ≤ 8 − b + a, else −1.
- **Turn one step**: +0x1A1 = +0x1A1 + turn way(s8 +0x1A1, s8 +0x1A2) (byte). Then if s8 +0x1A1 > 8: +0x1A1 = 1;
  else if s8 +0x1A1 < 1: +0x1A1 = 8. If +0x1A1 = +0x1A2 now: host op 62 reveal_view (slot, +0x1A1, +0x19E, 0).
- **Rotate** clockwise (counter-clockwise): +0x1A2 = wrap(+0x1A1 + 1) (wrap(+0x1A1 − 1)); +0x1A1 = +0x1A2; Turn done
  (slot, 1) (op 56).
- **Turn around**: d = +0x1A1 + 4 (byte); if d > 8, d = d − 8; +0x1A2 = d, +0x1A1 = d.
- **Clear the task**: +0x1A6 = 0, u16 +0x134 = NONE, +0x1A7 = 0, +0x95 = 0, u16 +0x96 = NONE.
- **In water**: +0x1B2 ≥ 0x6C and +0x1B4 is neither 0x1C nor 0x1F.
- **Forget tiles**: nothing if s8 +0xBA = 2. Slot < 0x28: the 8 u16 entries last_seen_m[8 × slot + j] = NONE.
  Else the 40 u16 entries last_seen_e[40 × (slot − 0x28) + j] = NONE.
- **Next path direction** (a byte): if s16 +0xD4 > 0: paths[80 × slot + +0x1AF] + 1. Else if the soldier is not at
  the step point (Next animation): if u16 +0x114 ≠ u16 +0x124: u16 +0x12A = u16 +0x124, Start a leg (Next
  animation), then paths[80 × slot + +0x1AF] + 1 (+0x1AF read after the leg); else the low byte of host op 6
  direction (s16 +0x116, s16 +0x118, s16 +0x126, s16 +0x128). At the step point: +0x1A1.
- **Occupy** tile t for the slot (1 = done, 0 = fails): 0 if t > 0xC40. o = byte 5 of t & 0x7F (0x40 if that is
  0x30 or more). If o ≠ slot: if o < 0x30, 0; else byte 5 of t = slot | (byte 5 & 0x80). Result 1.
- **Vacate** tile t: byte 5 of t = (byte 5 & 0x80) | 0x40.

**Procedure:**
1. p = u32 +0x90; w = word at p. go = yes, step = 0.
2. By w (a word not listed: go = no):

| word | action |
|---|---|
| 5, 0xA | if opt_sound ≠ 0 and seen: sound (0x1B, 0xF, 1) |
| 0x2D | if seen: sound (0x12, 0x32, 1) |
| 0x57, 0x85 | sound (0xD, 0x32, 1) |
| 0x8F, 0x98 | if seen: sound (0x1C, 0x1E, 1) |
| 0x93 | if seen: sound (0x1A, 0x1E, 1) |
| 0x96 | if seen: sound (0x1B, 0x32, 1) |
| 0x1B8, 0x1B9, 0x1BA, 0x1BB, 0x1BC | if seen and on screen: sound (0x1F, 0x32, 1), (0x21, 0x32, 1), (0x1E, 0x32, 1), (0x1F, 0x32, 2), (0x19, 0x32, 1) in that order |
| 0x37 | if s8 +0x1A3 = −1: turn one step. go = no |
| 0x54 | host op 65 effect_slot (slot as a byte, 3) |
| 0x1B6 | anim_signal = −1 |
| 0x1B7 | if combat ≠ 0: host op 66 tile_effect (effect_tile sign-extended, 0) |
| 0x1BD | +0x1BE = 0 |
| 0x1BE | +0x1FE = 1 |
| 0x1BF | +0x90 = p + 2; go = no; host op 67 quote (slot, s16 +0x134) |
| 0x1C0, 0x1E4, 0x1ED | nothing |
| 0x1C1 | +0x90 = p + 2; go = no; host op 68 gun_jam (slot) |
| 0x1C2 | go = no; +0x90 = p + 2. If +0x1A6 = 0x4C and the low byte of host op 44 quest_check (s16 +0x134, sign-extended) ≠ 0: host op 69 use_special (slot); else host op 70 door_fx (slot, s16 +0x134, +0x1A6). Then clear the task |
| 0x1C3 | if s8 +0x1A2 is neither 3 nor 5: +0x90 = p + 10 |
| 0x1C4 | +0x9D = 1 |
| 0x1C5 | if s8 +0x1A3 = −1: +0x90 = p + 2 |
| 0x1C6 | +0x90 = cur × 0x50 + 2 × (word at p + 2); go = no |
| 0x1C7 | if +0x1A1 = +0x1A2: Next animation (op 84 anim_step, slot), then Turn done (slot, 1) (op 56); else Starting an animation (slot, 8, 1, 0) (op 52). go = no |
| 0x1C8 | if s8 +0x1A3 = −1: +0x90 = p + 4 |
| 0x1C9 | if s8 +0x1A3 = 1: +0x90 = p + 2 |
| 0x1CA | if s8 +0x1A3 = 1: turn one step |
| 0x1CB | r = random(12). If r = 0 and s8 +0x18A > 70 and +0x1B9 = 0 and +0x185 = 0: Starting an animation (slot, 9 if s8 +0x1A1 is even, else 10; 0, 0); go = no |
| 0x1CC | go = no |
| 0x1CD..0x1D4 | +0x1A1 = +0x1A2 = w − 0x1CC |
| 0x1D5, 0x1D6 | host op 71 popups (slot, 1 for 0x1D5, else 0); host op 72 stop_walk (slot, 0) |
| 0x1D7..0x1DA | step = w − 0x1D6 (1..4) |
| 0x1DB..0x1DE | step = −(w − 0x1DA) (−1..−4) |
| 0x1DF | +0x1A2 = next path direction |
| 0x1E0, 0x1E2 | rotate clockwise |
| 0x1E1, 0x1E3 | rotate counter-clockwise |
| 0x1E5 | me = s16 +0x114; back = turn-around value of +0x1A1 (as in Turn around, without writing). If blocked(step(me, dirStep(back))) or blocked(step(me, dirStep(+0x1A1))): Starting an animation (slot, 0x2B, 0, 1); go = no |
| 0x1E6 | if +0x17D = +0x1A1: +0x17D = 0, u16 +0x136 = 0, Starting an animation (slot, 1, 1, 1), go = no. Else: if +0x17D = 0, +0x17D = +0x1A1; me = s16 +0x114; if blocked(step(me, dirStep(+0x1A1))) or (s8 +0x1A1 = 4 and byte 1 of tile me + 1 ≥ 0x83): rotate clockwise, then u16 +0x136 = 0x28, go = no; else u16 +0x136 = 0 |
| 0x1E7 | turn around; Turn done (slot, 0) (op 56) |
| 0x1E8 | go = no; +0x90 = p + 2; host op 73 pickup_here (slot) |
| 0x1E9 | go = no; +0x90 = p + 2; if cur = 0xC: host op 74 use_on_tile (slot, s16 +0x134, +0x1A6); else host op 75 bury_mine (slot, s16 +0x134) |
| 0x1EA | host op 76 revive_roll (slot, 0); go = no |
| 0x1EB | host op 77 melee_hit (slot) |
| 0x1EC | u16 +0x124 = u16 +0x114; u16 +0x120 = NONE; host op 72 stop_walk (slot, 0); if s8 hp (+0x183) ≤ 0: host op 78 kill (slot, 0), go = no |
| 0x1EE | Next word; then t = s16 +0xDC; if t ≠ NONE and byte +0x001 ≠ 0: host op 79 start_attack (slot, t), host op 80 attack_go (slot, t). go = no |
| 0x1EF | host op 81 launch (slot) |
| 0x1F0 | +0x1AD = +0x1AD xor 1 |
| 0x1F1 | if s8 hp < 15: u16 +0x120 = NONE; the 40 bytes +0x13A..+0x161 = 0; forget tiles; +0x1D5 = 0; +0x1D8 = 0x40, +0x1C2 = 0x40, +0x1BE = 0, u16 +0xD0 = NONE, u16 +0xDC = NONE; u16 +0x126 = u16 +0x116, u16 +0x128 = u16 +0x118, u16 +0x124 = u16 +0x114; if not in water: if cur ≠ 0, Starting an animation (slot, 0x2A, 0, 1) and host op 50 end_task (slot, 0); in water: if cur < 0x40, host op 78 kill (slot, 0). go = no. (hp ≥ 15: nothing, go on) |
| 0x1F2 | host op 82 merc_dies (slot, 0); go = no |
| 0x1F3 | op 83 anim_end (slot, 0) (Animation end); go = no |

   Only these words go on with go = yes and step = 0: the sound frames in the first rows (also when no sound
   plays), 0x54, 0x1B6, 0x1B7, 0x1BD, 0x1BE, 0x1C0, 0x1C3..0x1C5, 0x1C8..0x1CA, 0x1CB when no animation starts,
   0x1CD..0x1D6, 0x1DF..0x1E5 (unless 0x1E5 starts an animation), 0x1E6 in its last case, 0x1E7, 0x1EB, 0x1EC with
   hp > 0, 0x1EF, 0x1F0, 0x1F1 with hp ≥ 15.
3. If step = 0: if go, Next word. Stop.
4. Pixel step: ox = u16 +0x116, oy = u16 +0x118; g = step × 65 / 100 (truncated toward zero). By f = +0x1A1:
   1 → (0, −step), 2 → (step, −g), 3 → (step, 0), 4 → (step, g), 5 → (0, step), 6 → (−step, g), 7 → (−step, 0),
   8 → (−step, −g); any other f: no move. With a move (dx, dy): u16 +0x116 = ox + dx, u16 +0x118 = oy + dy.
5. t = (s16 +0x116) / 16 + ((s16 +0x118) / 14) × 56 (each division truncated toward zero). If t = s16 +0x114: Next
   word. Stop.
6. If tile_standable(t) < 1 or Occupy(t) fails: u16 +0x116 = ox, u16 +0x118 = oy; Next word. Stop.
7. Vacate(s16 +0x114); u16 +0x114 = t. If anim_steps_tile[cur] ≠ 0: u16 +0x126 = px(t), u16 +0x128 = py(t)
   (Tile point, Next animation); host op 53 stop_anim (slot, 1); Next word. Else: host op 53 stop_anim (slot, 1);
   u16 +0x126 = u16 +0x116, u16 +0x128 = u16 +0x118; Next word.

Tiles: t is always a map tile here (the host keeps soldiers so that a pixel step stays on the map).

### Next word (part of the frame routine)

Frame routine and Next word call each other directly: these calls (and the entry point `ja1_anim_frame` itself)
always run the library's code, never op 61. Op 61 (and host_ops bit 61) applies only where Starting an animation
runs the frame routine. The same holds for Turn done: `ja1_anim_turn` runs its code; op 56 applies where another
procedure calls it.

1. If u16 +0x136 ≠ 0: Frame routine. Stop.
   Original bug (issue #2 row 60): an Escape stop in the fall search (word 0x1E6) leaves +0x136 set; a later
   knock-back that reaches word 0x1E5 with free tiles then runs Frame routine -> Next word -> Frame routine on the
   same word without end (stack overflow). **Port fix** (the maintainer ruling on #59, 2026-11-07): when this step would run
   the Frame routine again for the same soldier at the same script position, the port sets u16 +0x136 = 0 and stops
   (the frame holds for this tick; the next tick steps on). `ai::set_keep_anim_loop(true)` keeps the original
   repeat. Test: validate/gate_campaign.py step 18 (`santino_loop`, script seeds 1 and 5).
2. p = u32 +0x90; v = word at p. If v < 0xB6 or v = 0x1CC: +0x1F4 = low byte of v.
3. p = p + 2; +0x90 = p; v = word at p. If v > 0x1F3: Starting an animation (slot, v − 0x1F4 (low 16 bits), 1, 0)
   (op 52). Else: Frame routine.
4. Afterwards: if +0x1A1 = +0x1A2: Turn done (slot, 1) (op 56). Else if anim_turn_while[cur] ≠ 0: turn one step.

### Animation end (`ja1_anim_end`; host op 83 anim_end)

Script word 0x1F3 ends every animation script. The soldier goes back to the animation queued before the current one,
or picks the next animation. Arguments: slot, a byte that is not used. Record fields are u8 unless a width is given;
queue[i] = the u16 at +0x1E8 + 2 × i; idx = +0x1F3.

1. If idx = 0: Next animation (op 84 anim_step, slot). Stop.
2. prev = queue[idx − 1] (u16, used as an unsigned table index). c = anim_rows[8 × prev] (u8); if c = 99, c = 0.
   If anim_free[prev] = 0 and the AP check fails for the cost c (the check of Starting an animation with c in place
   of anim_rows[8 × a]: it fails only if selected = slot, combat ≠ 0 and s8 +0x1E2 + s8 +0x1A5 < c; on a fail: if
   slot < 8, host op 57 merc_text (slot, 1, 0x4B0); else +0x1B0 = 1): stop.
3. fromD = (queue[idx] = 0xD). queue[idx] = 1000. idx = idx − 1 (byte, stored in +0x1F3).
4. a = the low byte of queue[idx] (the new idx). +0x90 = a × 0x50.
5. If a is 0x15, 0x34, 0x37 or 0x3A: host op 59 walk_sound (slot, 1).
6. If a = 4 and fromD: op 51 replace (slot).
7. +0x1FE = 0. Frame delay (ja1_anim_delay; it reads queue[idx] as the whole u16). +0x1AD = 0. Op 61 anim_frame
   (slot).
8. Host op 31 spend_ap (slot, anim_rows[8 × a] as a byte, the i16 anim_sounds[a] sign-extended to 32 bits).
9. If anim_rows[8 × a + 3] ≠ 0 and (u16 +0x126 ≠ u16 +0x116 or u16 +0x128 ≠ u16 +0x118): Next animation (op 84
   anim_step, slot).

Ops 61 and 84 are library ops: the library runs its own code unless their host_ops bit is set (as everywhere).
If the host answers op 83 itself, the script goes on to the word after 0x1F3 (0x3E7 in every script), which
starts animation 0x3E7 − 0x1F4 = 499: no such animation exists. The original never gets there.

*Evidence (2026-10-21): mirror tools/ai.py anim_end, ai_diff anim_end 1000 cases 0 mismatches; mutants 111/113
(the 2 survivors change the unused AP-check flag, equivalent).*

### Turn done (`ja1_anim_turn`; host op 56 anim_turn)

Arguments: slot, flag (low byte).
1. If +0x1D1 = 1 and +0x1D6 ≠ 0: +0x1D6 = 0, +0x1D1 = 0, u16 +0x132 = NONE. Else stop.
2. If flag ≠ 0: host op 62 reveal_view (slot, +0x1A1, +0x19E, 0). Then if slot ≥ 0x28: host op 63 enemy_turned
   (slot); else if slot < 0x10: host op 64 guard_turned (slot), then host op 34 share_sightings (slot, 0).

*Port evidence (2026-10-20): ja1_anim_set, ja1_anim_frame and ja1_anim_turn (port/ja1logic src/anim.rs, clean-room)
give 0 mismatches against the original in ai_diff.py --port (12,000 cases each; the test host answers op 62
reveal_view with the analysis mirror on the port's world). Rust mutants of ja1_anim_set: 87 of 90 killed (3 are
table-size limits that cannot change a result).*


## Noise

A noise has a source (a soldier slot, or 0x40 and above for no soldier: 0x41 is heard by the mercs' side, 0x42 by
the enemies), a tile, a class, a loudness and a kind. Nothing is heard outside combat, with loudness 0, or from a
dead soldier.
- **Who listens**: the enemies for a merc or guard source (and 0x42); the mercs and guards for an enemy source (and
  0x41); everyone otherwise.
- **Class**: the given class when 1..9, else the source soldier's class (+0x1B3) when 1..9, else the class of the
  ground under the tile.
- **Each listener** in play, conscious, with 15 hp or more and not in state 2, skips a source its side already sees
  (side knowledge 1) or that it sees itself (sight flag 1). It hears at: loudness + its hearing bonus (+0x1D0),
  −5 when exactly one of the listener and the noise is class 3 (water), −(Distance to the tile / 15 − 1), −+0x1D5.
  Above 0 the listener reacts with that strength (below); for kind 4 from a soldier the reaction gets the source's
  own tile instead of the noise tile.

**Hearing a noise.** The noise's soldier is the one standing on the tile (none for kinds 6, 8 and 9); a soldier of
the listener's own side is ignored. Level: 0 below strength 3, 1 below 6, 2 below 10, else 3.
- A merc shows a 25 s status box (StatusMessage? 0x27D24; host op 213 in the port) "<name> hears a  <LEVEL> <KIND> noise coming from the <direction>" (levels FAINT, DEFINITE, LOUD,
  VERY LOUD; kinds by number: UNKNOWN, MOVEMENT, CREAKING, SPLASHING, IMPACT, GUNFIRE, EXPLOSION, SCREAM, IMPACT,
  IMPACT; the direction from its tile to the noise tile), except for kinds 4–6 and 8–9 and when its side already
  knows the soldier.
- Within 14 tiles it looks: a sight line (range 14) to the soldier (or the tile) means seen; it is "turned away"
  when its facing is not the direction to that point.
- A soldier seen: MarkSeen, and a merc shares its sightings. Not seen: re-think; kinds 4 and 5 above level 0 raise
  the alert to 2 (Orders bump); the sight flag becomes "heard" unless it is 2; the soldier's tile becomes the
  last-seen tile; a merc's side marks it heard unless known.
- No soldier: kind 6 above level 0 raises the alert to 2. When unseen (or kind 8 while turned away) a noise at least
  as strong as the remembered one (+0x1C0) becomes the noise to check (+0x122 = tile, +0x1C0 = strength, at most
  12) and the soldier re-thinks; otherwise it re-thinks with alert 2.
- Last: kind 8 runs a follow-up check (0x73C00); with a soldier on the tile, the interrupt check (old sight flag).

**Interrupt on a kind-8 noise**: a soldier that hears a kind-8 noise takes the turn at once when it is not in alert
state 2, is on the map, stands (animation 4 or later) and has AP left. Mercs, guards and allies do so only while the
tactical screen is not drawn. The currently selected soldier is kept (a merc as the one to return to, others in the
interrupt queue). A merc acts immediately; any other soldier joins the interrupt queue and the turn moves on.

**Being shot at**: after a shot (not a silent one) the target, if in play with 15+ hp, may notice the shooter. A target
in alert state 2 only marks itself (+0x1C4). Shots within a side are ignored. When the shot ended on a soldier other
than the target (the soldier at the end point), that soldier is marked and the target does not react. Otherwise the
target reacts only when the bullet flew at least the pixel distance between them minus 45: it is marked; within 14
tiles with a clear sight line it sees the shooter (as in sighting, and a merc's side shares it). Unseen, it re-thinks,
becomes alert (state 2), sets its sight flag on the shooter to "heard" unless it already had it as 2 and remembers
the shooter's tile; a merc target whose side did not know the shooter practises perception, reacts and its side now
knows the shooter was heard. Last, the target may interrupt (as for sighting) with its old sight flag.

### Make noise (library op 127 make_noise, `ja1_make_noise`)

Arguments: source src (byte: a soldier slot, or 0x40 and up for none), tile (s16), class cls (byte), loudness loud
(int), kind (byte). No soldier argument: "src record" = soldiers[src] when src < 0x30.
1. If combat = 0 or loud = 0 (32 bits): done. If src < 0x40 and src record +0x183 = 0: done. (SPEC GAP (not seen: no
   caller passes 0x30..0x3F: StepNoise, TakeDamage, PlayDoorFx?, UseOnTile?, ReviveRoll?, ShotEnd, LandItem? and
   ProjectileImpact pass a record's own slot 0..47; Explode passes a record's slot or the owner byte of the effect
   table, which only FxStart writes: 0x41 from TriggerItem? and StageEffects?, 0x42 from BuryMine, a slot from
   ProjectileImpact, or the owner of the flare 0x355D0 = 0x40, 0x41 or an effect table owner): src 0x30..0x3F
   reads past the 48 records in the original; the port treats such a source as alive and with class 0.)
2. Listeners k: 0x28..0x2F when src < 0x28 or src = 0x42; else 0..0x0F when src < 0x30 or src = 0x41; else 0..0x2F.
3. Class c: when src < 0x30: c = cls, and when c is not 1..9, c = src record +0x1B3. When c is still not 1..9 (or
   src ≥ 0x30): c = t0_class[tile byte 0 of tile].
4. For each listener k (record K), in slot order:
   a. Skip it when K +0x182 = 0, K +0x138 = 0, K +0x188 ≠ 0, s8 K +0x183 < 15 or s8 K +0xBA = 2.
   b. When src < 0x40: skip it when s8 knowledge[src] = 1 or s8 K byte (+0x13A when src < 0x28, else +0x112) + src = 1.
   c. v = (loud + s8 K +0x1D0) as 16 bits. When (K +0x1B3 = 3) ≠ (c = 3): v = v − 5 (16 bits).
      d = s16 World distance (s16 K +0x116, s16 K +0x118, px(tile), py(tile)).
      v = s16 (v − (d / 15 − 1) − s8 K +0x1D5) (division truncated toward zero, 16-bit wrap). Skip it when v ≤ 0.
   d. where = s16 src record +0x114 when kind = 4 and src < 0x30; else tile.
      Library op 211 hears (k, where, low byte of v, kind).

### Hearing a noise (library op 211 hears, `ja1_hears`)

Arguments: tile (s16), strength (byte), kind (byte). The soldier is the listener (slot, record S).
1. occ = 0x40 when kind is 6, 8 or 9; else the occupant of tile. tell = 1; old = 0. When occ < 0x30 (record O):
   - If (slot < 0x28) = (occ < 0x28): done.
   - If slot < 0x28: tell = 0 when s8 knowledge[occ] is 1, 2 or −1; old = S byte +0x112 + occ.
     Else: old = S byte +0x13A + occ.
2. st = s8 strength. level = 0 if st < 3, 1 if st < 6, 2 if st < 10, else 3. me = s16 S +0x114.
3. If slot < 8, tell = 1 and kind is not 4, 5, 6, 8 or 9: d = low byte of library op 6 direction (px(me), py(me),
   px(tile), py(tile)); host op 213 hears_msg (slot, level, kind, d).
4. seen = 0, turned = 0. If library op 7 tile_dist (me, tile) (i32) ≤ 14:
   - Target point: when occ < 0x30: x = s16 (s16 O +0x116 + 2), y = s16 (s16 O +0x118 + 4); else x = px(tile),
     y = py(tile). Own point: sx = s16 S +0x116 + 2, sy = s16 S +0x118 + 4.
   - turned = (s8 S +0x1A1 ≠ direction (sx, sy, x, y) (i32)); then seen = (op 49 sight_line (slot, sx, sy,
     tile, x, y, 1, 0x0E, occ) ≠ 0).
5. When occ < 0x30:
   - seen: Mark seen (slot, occ) (shooting.md). Then when slot < 0x28: host op 34 share_sightings (slot, 0).
   - not seen: S +0x1C1 = 1. If kind is 4 or 5, level ≠ 0 and s8 S +0xBB < 2: S +0xBB = 2 and Orders bump
     (shooting.md). Then when slot ≥ 0x28: if s8 S byte +0x13A + occ ≠ 2 it becomes 0xFF; u16 last_seen_e
     [(slot − 0x28) × 0x50 + 2 × occ] = u16 O +0x114. When slot < 0x28: if s8 S byte +0x112 + occ ≠ 2 it becomes
     0xFF; u16 last_seen_m[slot × 16 + 2 × (occ − 0x28)] = u16 O +0x114; then when tell = 1 and slot < 8:
     knowledge[occ] = 0xFF.
6. When occ ≥ 0x30:
   - If kind = 6, level ≠ 0 and s8 S +0xBB < 2: S +0xBB = 2 and Orders bump.
   - If seen = 0, or kind = 8 and turned = 1: when st ≥ S +0x1C0 (unsigned byte): u16 S +0x122 = tile (16 bits),
     S +0x1C0 = strength when st < 12, else 12; S +0x1C1 = 1.
   - Else: S +0x1C1 = 1; if s8 S +0xBB < 2: S +0xBB = 2 and Orders bump.
7. Last: when kind = 8: library op 212 noise_int (slot). Else when occ < 0x30: library op 182 interrupt_check (slot,
   occ, old).

### Noise interrupt (library op 212 noise_int, `ja1_noise_interrupt`)

The soldier heard a kind-8 noise.
1. If s8 +0xBA = 2, +0x18A = 0, the animation word u16 +0x1E8 + 2 × +0x1F3 < 4 or s8 +0x1A5 < 1: done.
2. If slot < 0x28 and screen_drawn ≠ 0: done.
3. +0x1B0 = 0. sel = selected. If sel ≥ 8: itq[itq_len] = sel, itq_len = itq_len + 1 (8-bit). Else: kept_merc = sel.
   (itq has 50 entries: a push when itq_len ≥ 50 is a **stop point**, port_api.md "Division by zero": the original
   writes past the queue into other game data; the library makes host call 47 and does not push.)
4. If slot < 8: interrupter = slot; library op 113 merc_interrupt (); done.
5. itq[itq_len] = slot, itq_len = itq_len + 1 (8-bit) (the same stop point); library op 114 next_turn (1).

Evidence: MakeNoise, MsgHearsNoise? and NoiseInterrupt? diff-tested in Unicorn against the original
(validate/diff/ai_diff.py `make_noise`, `hears`, `noise_int`), mutation-tested.

## Knowledge ageing at a turn change

At a turn change the game ages what each side knows. With the side flag set, the mercs' latest call counts down
(its level byte drops by 1 if above 0; at 0 the call's tile is cleared) and enemies 40..47 age; with the flag clear,
the enemies' call counts down and soldiers 0..39 age. A soldier ages only while present (three status flags set):
the other side's knowledge level of it moves one step away from 0: levels 2 and up rise (past 4 they become 0,
"lost"), levels −1 and down fall (past −3 they become 0); levels 0 and 1 stay. The level is a byte: 127 rises to −128 and stays.

Evidence: AgeKnowledge? diff-tested in Unicorn against the original (validate/diff/ai_diff.py `age`), mutation-tested.
