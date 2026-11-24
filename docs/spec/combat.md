# Combat: action points, movement, shot cost, chance to hit, damage

All arithmetic is integer; divisions truncate toward zero (C semantics). Stats are 0..100 unless noted.
A *tile distance* d is measured in units of 15 per tile (5 tiles = 75).

## Action points per turn

1. base = (10·level + 2·agility + maxHealth + dexterity + 10) / 20, kept as a byte (no upper limit; exact
   form: ai.md "Max AP").
2. Wounded (health < maxHealth): lost = (maxHealth − health − woundTotal)/2 + (maxHealth − health);
   AP −= AP·2·lost / (3·maxHealth).
3. Energy below 100: AP −= (100 − energy)·AP / 200.
4. At least 8. A soldier with health 0 has 0.

## Cost of a shot

cost = (target tile ≠ last target tile ? 1 : 0) + (AP·200 / ((skill/2 + 50)·speed/2) + 1) / 2, clamped to 1..AP,
where AP is the shooter's turn AP, speed is the weapon's speed byte and skill is marksmanship (knives: the average
of agility and dexterity). A shot therefore costs a fixed share of the turn: faster soldiers pay more points.
Weapon-specific extra AP is added on top.

## Aiming

Each extra aim click costs 1 AP and adds 1 aim, up to min(base cost + 4, AP left); one more click wraps back to
no aim. Shots in real time get 4 aim.

## Chance to hit (1..99 %)

Start from c = marksmanship (averaged with the gun's condition when that is lower), then adjust c in order:

- +10 if the target tile is the same as the shooter's last target tile;
- +10 per aim click;
- enemy shooters: −10 / 0 / +10 by difficulty;
- shooter state byte = 4 (meaning not identified yet): −20;
- range: + (75 − d)/3 (0 at 5 tiles, −5 per tile beyond);
- sun goggles: d shortened by 10 %; sniper scope with aim > 0: d shortened beyond 10 tiles;
- target animation ≤ 3 or ≥ 0x39 (prone?): −(d − 20)/3 more; animation 5–7 or 14–16 (crouched? not confirmed): −20;
- target beyond 5 tiles whose two state bytes b1B1 (set) and b1B3 (1, 2 or 4–7) match (meaning not identified): −5;
- conscious target (health ≥ 15): tiles moved m = `tilesMoved` +0x1F2 (scaled by the AP it has spent this turn), plus agility/20 + level/2
  when it sees the shooter; every point of m above the gun's tracking value costs 5;
- shooter wounds: −(c·2·lost/(3·maxHealth))·(100 − (level − 1)·10)/100;
- shooter energy below 100: −((100 − energy)·c/200)·(110 − dexterity)/100;
- target beyond the weapon's range (range in tiles × 15): halved.

The shot hits when Random(100) + 1 ≤ chance. **A successful roll is not yet a hit**: it fixes the bullet's end
point and height; the bullet then flies and can be stopped on the way (see Bullet flight).

Weapon ranges (tiles): .38 10, .45 11, 9 mm 12, .357 13, shotgun 16, M14 20, M16 25, modified M16 30.

## Bullet flight

A bullet starts at height 4500 + Random(1001) and moves one pixel per step, 15 steps per frame, its height changing
linearly toward the end height chosen by the to-hit roll. Each time it enters a new tile it is tested against that
tile (x, y = its pixel position; flown = pixels so far):

1. Height ≤ 0: it stops.
2. The tile's object (layer t1) gives an obstacle height by the object's class (JAGGED.DAT): class 6 (trees) 9000,
   7–8 (walls) 10000, 9: 7000 for objects 0xE2/0xE3, else 3000; other classes 0. A roof (ground class 3, roof layer
   t3 ≥ 0x52) counts as 10001, but only for projectiles whose maximum height is above 9000 (thrown items).
3. **A soldier on the tile:** considered only if it stands on the target tile, or the bullet has flown 75+ pixels,
   or (1 in 30) when its animation's posture value is ≥ 6; otherwise the bullet flies on. It is hit when its height
   (obstacle height + 1000 lying, i.e. animation < 4 or > 0x37, else + 6000) is at least the bullet's height and the
   bullet is within 1 pixel (larger of |dx|, |dy|) of its centre (position + (2, 4)). If missed and it was the
   target, the "skip" flag of the flight is cleared.
4. **No soldier**, obstacle lower than the bullet: flies on. Otherwise:
   - **Tree (9000):** after 30+ pixels, or on the target tile: stops with chance 1/n, n = max(1, 25 − flown/15)
     (the farther the shot, the likelier). Closer than that: passes if the to-hit roll succeeded, else stops 1 time
     in 100.
   - **Low object (3000 / 7000):** stops only if the bullet is within 2 pixels of the tile's centre.
   - **Wall (10000+):** passes while the flight's skip flag is set. A wall piece whose window value for this object
     and direction is ≥ 0x62 lets a bullet below 9000 through with chance 100 − (flown − 15)/5 %. Otherwise: window
     value < 4 and maximum height ≤ 9000 → passes; else it stops.

When it stops on a soldier the damage rules above apply.

## Random numbers

Random(n) = (rand() × n / 32767) mod n with C integer arithmetic, where rand() is the Watcom C library generator
(15-bit result). n = 0 would divide by zero; no rule below can pass 0.

## Damage of a bullet

When a bullet reaches a soldier:

damage = weaponDamage × (100 − flown × 50 / (range × 15) + (Random(51) − 25) + margin / 2) / 100, at least 1,

where flown = pixels the bullet travelled, range = the weapon's range in tiles, margin = how far the to-hit roll
beat the chance (0 on a miss), divisions truncating. Weapon damage:

| Weapon | Damage | | Weapon | Damage |
|---|---|---|---|---|
| .38 Revolver | 20 | | .12g Shotgun | 36 |
| Colt .45 | 22 | | .12g Rifle | 32 |
| 9mm Beretta | 24 | | M14 Rifle | 34 |
| .357 Magnum | 26 | | M16 Rifle | 36 |
| Modified .45 | 26 | | Mod. 12g Rifle | 36 |
| Modified 9mm | 28 | | Modified M14 | 38 |
| Modified .357 | 30 | | Modified M16 | 40 |
| Knife | 20 | | Combat Knife | 30 |

## Hit location and armour

The bullet's height when it arrives picks the body part. If the target is lying down (animation ≤ 3 or ≥ 0x39)
and the height is at most 1000, the height is multiplied by 6 first. One roll r = Random(100) + 1 is made at the
start and used for every armour piece.

- **Above 6000 or below 0:** no damage.
- **Head (5000–6000):** a helmet absorbs a = min(damage, absorb − max(0, r − condition)) (not below 0). If it
  absorbs anything, damage −= a and the helmet's condition drops by wear × (a / 2) / 100. If nothing was absorbed,
  damage × 1.5, then a wisdom-loss check.
- **Torso (2500–4999):** a Shielded Vest in the vest-pocket slot, then the body armour, each absorb in the same
  way (the second piece sees the damage left by the first). If neither absorbed anything: dexterity-loss check.
- **Legs (below 2500):** damage halved (damage − damage / 2), then an agility-loss check; if agility is lost and the
  target is standing (animation 4–0x28 except 0xD) it is knocked down.

Stat-loss check: if Random(damage) + 1 > 20, the stat loses Random(damage / 2) + 1, but never drops below 1.

| Armour | Absorb | Wear |
|---|---|---|
| Helmet | 10 | 50 |
| Treated Helmet | 15 | 40 |
| Kevlar Helmet | 20 | 30 |
| Trt. Kev. Helm | 30 | 20 |
| Kevlar Vest | 10 | 50 |
| Trt. Kev. Vest | 15 | 40 |
| Spectra Shield | 20 | 30 |
| Trt. Spectra | 30 | 20 |
| Shielded Vest | 10 | 50 |

The returned damage is then subtracted from health (never below 0). An enemy left with less than 15 health dies
at once; a merc below 15 collapses.

## Movement

Every tile has a terrain move code, rebuilt from its layers whenever they change:

- With an object (t1 ≠ 0), by the object's class (JAGGED.DAT table): class 1 → 2000; 2–3 → 25400; 4–5 → 25300
  (door) for objects 0x21, 0x23, 0x24, 0x26, else 25500 (wall); any other class → 1500 for objects 0x3D, 0x3E,
  0x43, 0x45, 0x5C, 0x5D, 0x62, 0x64, else 1201 if the roof layer t3 is 0x73, else 25500.
- Without an object, by the ground's class: 1, 4, 5 → 1200; 2, 3 → 1000; 6, 7 → 1600; 8 → 3000 (ground values
  below 0x80) or 3600; 9 → 5000 (ground values above 0x96) or 3600; class 0 keeps the previous code.
- A tile that had the door code 25300 never gets a lower code.

AP to enter a tile = base + extra, where base = 2 when sneaking (Shift held), else 3 when the soldier's crouch-like
flag (record byte +0x177, not yet confirmed) is set, else 0; +1 with Alt held. Extra by move code: 1000, 1500, 25300,
25400 → 2; 1200 → 3; 1201, 1600 → 4; 2000 → 5; 3000 → 6; 5000 → 7; 3600 → 8; any other code → 0. Code 25500 costs
100 (impassable) regardless.

A walk costs the sum over the tiles of the path plus getting up first: 7 if the soldier's current animation is
0 or 3, 5 for 1–2, 3 for 0xE–0xF.

### Rebuilding all move codes

When an object goes away (a blue flag removed, a tree burnt, a mine set off), the whole table is rebuilt: every code
is cleared, every tile gets its code by the rules above (previous code 0), then a door pass goes over the tiles in
order. Open doors 0x3B, 0x5A, 0x41 and 0x60 set their own tile to 25400 (0x6338). Each door object then marks
neighbours (directions 1 = north, clockwise to 8 = north-west) with 25300 (0x62D4), unless the neighbour holds
25500, 1201 or 1500 at that moment (earlier marks count):

| Objects | Neighbours marked |
|---|---|
| 0x1B | 7, 8, 6 |
| 0x1D | 3, 2, 4 |
| 0x1E | 1, 8, 2 |
| 0x20 | 5, 6, 4 |
| 0x3B, 0x5A (open), 0x3D, 0x3E, 0x5C, 0x5D | 1, 8, 2, 5, 6 |
| 0x41, 0x60 (open), 0x43, 0x45, 0x62, 0x64 | 7, 8, 6, 3, 2 |

A neighbour step that leaves the map stays on the tile itself.

### Starting a walk (the move order)

A move order (tile, show-target marker, sneak, crouch-like byte +0x177, Alt, confirm, whole squad) returns 1 when
the soldier starts to walk:

1. Refused (0) for slots 0x28 and up, a soldier not in the sector, hp below 15, record byte +0x17E set, or a
   current animation code above 0x3C.
2. A soldier in posture 3 with +0x1D5 above 0 and a living soldier on its target tile (+0xD0), who holds a loaded
   gun (item below 0xF with ammo): message 8, refused.
3. A tile that is not standable is refused unless record byte +0xBA is 2; "Can't move there!" shows unless it is the
   soldier's own tile or no tile (0xC41).
4. Confirm (a merc, the confirm option on, not a squad order): the path is priced with the path marked on the map;
   no path (AP 0 or less) shows "Can't move there!" and refuses. The cursor becomes 0x8F and the game waits for a
   second click on the same tile (Escape, the right button or a click elsewhere cancels); a cancel puts the old path
   back and refuses. (The library leaves this step to the front-end: `ja1_move_gate` = steps 1-3 without
   messages, `ja1_move_confirmed` = steps 5-7 with the walk AP of the marked pricing; cursor.md "Walk (S, T)".)
5. A squad order sends the same order to every other merc 0..7 (marker on, sneak = Shift, Alt = Alt, no confirm,
   no squad).
6. Without confirm the path is priced again, unmarked, from zero steps.
7. Not enough AP for the priced walk (the usual check, in combat only): the tile is kept as the soldier's goal
   (+0x120) with the three option bytes, refused. Else the soldier stops its current walk, the target marker is set
   when asked, its step point moves to its position when the animation allows actions, the option bytes are stored
   (+0x177, +0x1A8 sneak, +0x196 Alt, +0x1FA = 1), and it walks to the tile.

## A merc's view

When a merc faces a new direction (turn end, start of its turn) it looks: nothing happens for slots 8 and up. 17
rays leave the merc's tile; each step of a ray takes one of five directions (left, right, ahead, left-left,
right-right of the facing) from a fixed 17 × 9 table, one for odd and one for even facings, and may name a second
direction as the side the object test uses. A ray ends at the map edge, after the merc's view range in steps, when
its budget (99) runs out, after an object that blocks fully (2 or more), or one step after an object that blocks
half (1). An object of kind 0xB right next to the merc counts as open with a budget of 2; one step further it
counts as half.

On the first two steps a merc with the "search" byte (+0x177) and a working mine detector in hand (item 0x4D,
condition 10+) finds a hidden armed mine on a tile without an object with chance = the detector's condition %: the
tile gets a blue flag and the merc's path is planned again; a mine of type 0x11 trains stat 6. Without a working
detector the search byte clears.

Every tile a ray reaches that blocks less than fully is revealed and spends 1 budget. In turn-based combat (flag
0x92DD0 clear) and with record byte +0x1D5 at 0, a hidden ground item on it becomes seen; the first one stops the
merc (speech 3, stop, task cleared; when +0x16D is 0 and its goal +0x120 is another tile: 0x92E64 = 1, +0x16D = 1,
0x1148C) and trains stat 4. Roofs open: by the roof value's class 0x12 / 0x13 /
0x14 (a value 0x73 also clears with the tile's move code redone), the north tile's roof clears up to 0x53 / 0x5E /
0x69, and the tile's own roof clears unless its overlay is 0x49, 0x4A or 0x4C and the south tile's roof is at least
0x52 / 0x5D / 0x68 (then a roof above 0x54 / 0x5F / 0x6A becomes 0x53 / 0x5E / 0x69 for overlay 0x4C, else
overlay + 9 / 0x14 / 0x1F). Object 0x57 clears a north roof of 0x53, 0x5E or 0x69. In the story sector the objects
0xAC..0xAF note the merc as the finder (hp 15+). In sector 0 the tile 0x279 clears a story flag.

### Finding the path

The path is an A* search over the 56 × 56 grid from the soldier's tile, 8 directions numbered 0 = north, then
clockwise (north-east, east, south-east, south, south-west, west, north-west); a step never leaves the map and never
wraps from column 0 to column 55 or back.

**Step cost.** For each neighbour n of the tile t that is expanded, the tests run in this order (the first that
applies decides; "code" starts as n's move code, "from" is t's move code):

1. n is occupied (any soldier slot below 0x40 on it, the walker included) and n is not the destination: blocked in
   combat (enemies near); otherwise code becomes 20000. (The other tests then see 20000, not the tile's code.)
2. Else, with record byte +0x177 set and code below 3000: code becomes 1000.
3. Code 1500 is blocked unless n is the destination, or from is 0x62D4 and record byte +0xB9 is set.
4. Code 0x639C is blocked (also as the destination); code 1201 is blocked for slots 8 and up (also as the destination).
5. From 0x6338, a step is blocked unless code is 0x6338 or 0x62D4; a step onto code 0x6338 is blocked unless from
   is 0x6338 or 0x62D4 (no destination exemption; an occupied tile counts as 20000 here).
6. Code 0x6338 costs 1000; code 0x62D4 costs 1000 on ground values below 0x6C, else 6000.
7. The destination costs 0.
8. When start and destination both have move code 3600 or 5000, a remaining cost of 3600 or 5000 becomes 500.
9. A diagonal step (odd direction) adds 1, also onto the destination.

**Search.** The estimate to the goal is the longer axis distance plus 14 × the shorter one / 10 (truncated), in
tiles (it is small against step costs; the search is close to uniform-cost). The best known cost per tile starts
"infinite", 0 at the start. The open list holds candidates (tile, cost, estimate, step record), sorted by cost +
estimate. A new candidate is placed before the first candidate whose cost + estimate is greater; among those with an
equal value it goes behind every one strictly nearer the goal by Manhattan distance and before the rest.

The search starts with the start tile as the only candidate. Each round takes the first open candidate and moves it
to the end of the finished list. If its cost is not above the best known cost of its tile, every neighbour is tried:
a neighbour whose new cost (candidate cost + step cost) is not below its best known cost is dropped, otherwise it
becomes a new candidate with that best cost. After the round, the search stops when the open list is empty or its
first candidate stands on the destination. A first candidate on the destination is "found". An empty open list is
"found" with 0 steps when the start is the destination, else "no path". So a start equal to the destination (never
added again, its best cost is 0) explores until the open list runs empty (found, 0 steps) or a limit below ends the
search (no path).

**Limits.** Candidates are kept in a pool of 510, the start candidate included. When the pool
is full, the candidate that finished earliest (the head of the finished list) is taken out of the finished list and
reused; when the finished list is empty there is nothing to reuse and the search ends with "no path". The reused
candidate can be the one now expanded (when it is the only finished one): its tile and cost stay in use for the rest
of the round, but its step record is now the new candidate's, so the following neighbours of the round link their
step records to that new record. A reused
candidate loses nothing of a path, because paths are kept apart from candidates: every new candidate writes one step
record (its direction and the step record of the candidate it came from); the start has no record. The search
creates at most 4094 step records: the 4095th ends the search with "no path", before that candidate is added.

**Result.** A found path is the chain of step records from the destination's candidate back to the start, read in
walking order. The soldier keeps at most the first 80 directions, and the stored step count is the number kept
(at most 80). After "no path" the stored directions and count stay as they were.

*Evidence: differential tests against the original code: validate/diff/combat_diff.py (3,100 AP cases from 100
live soldiers + 7,525 chance-to-hit cases), damage_diff.py (4,000 bullet-damage and hit-location cases with every
Random call compared), move_diff.py (2,961 step costs, the move code of every tile of all 60 sectors + 20,000 fuzzed
tiles), flight_diff.py (20,000 tile tests: trees, walls, windows, roofs, soldiers), path_diff.py (300 searches on the real
map, fuzzed move codes and occupants, an open field and a maze: node reuse, the step-record limit and paths of
over 1,400 steps), all 0 mismatches. The bullet-flight rule was observed in a live fight. Key meanings (Shift = sneak, Alt) from
the BIOS keyboard flags the code reads and the game's quick-reference card.*

## Using an item at a tile (the click)

The same procedure serves the player's click and the AI's attacks. Input: a soldier, a target tile, the item to
use (0 = the empty hand). Nothing happens while the soldier's current animation cannot act. The soldier's record
bytes +0xB0..+0xB3 are cleared first.

**Reach.** The empty hand reaches 15 world units with range 1. A throwable (item kind 0x95) reaches range × 15 +
health − 50, less (100 − energy) / 200 of that (not below 0), and its range is reach / 15. Other items reach range ×
15 (24 when the range is below 2). The distance is the world distance from the soldier's pixel point to the
tile's point.

**Walking first.** A walk is needed when the target is beyond reach and the item is the hand, an item above 0x23
other than 0x5D, or has range 1; or, within reach with range 1, when the target is a diagonal neighbour or any
other standable tile. The item becomes the soldier's task (+0x1A6, with the range in +0x1A7). Outside combat mode
the target tile is stored (+0x134). With a range above 1 the soldier simply walks to the tile (the task becomes
the main-hand item) and the procedure returns. With range 1: knives and first aid kits need someone on the
target ("No one there" otherwise, nothing more happens); item 0x45 needs a door-class object there (class 7–8);
then the target must be reachable ("Can't get there!" otherwise). The use spot's AP cost is the walking cost
there plus a use cost (hand: +4 on a standable tile, +3 on an unstandable one, else by the ground item: +5 none,
+4 when the item's byte 6 is set, +2 for an enemy at item 0x72, else +5; item kinds 0x76 and 0x9B +10, 0x96 + a
shot, 0x93 and 0x9E +5, 0x9D +6). If affordable (AP check) and the spot is dry, the soldier walks there (task 0xFF
for the hand) and the merc panel updates unless Ctrl is held; this is the only case that returns "started".

**Use cost, exact steps** (host op 121 use_cost (slot, spot, tile, byte g, byte kind); a library op). The result
is a byte (all sums wrap to 8 bits):
1. p = low byte of Path AP (op 40 path_ap (slot, spot): sneak, byte 0x177 and alt all 0).
2. If kind = 0 or kind ≥ 0xFE (the hand): d = tile_standable (tile) as a signed byte (ai.md "Standable tile").
   - d > 0: p + 4. d < 0: p + 3.
   - d = 0: if g ≥ 200: p + 5. Else if byte 6 of ground item g ≠ 0: p + 4. Else if the slot is 0x28 or above (an
     enemy) and byte 0 of ground item g (its type) = 0x72: p + 2. Else p + 5.
3. kind 0x76 or 0x9B: p + 10. kind 0x96: p + low byte of Shot AP (op 45 shot_ap (slot, tile)). kind 0x93 or 0x9E:
   p + 5. kind 0x9D: p + 6. Any other kind: p.
Original: UseCost 0x59AE8 (EAX soldier, DX spot, BX tile, CL g, kind on the stack).

**In reach.**
- Guns (items 1..14): the soldier raises the weapon toward the tile (aiming animation 0x1A for pistols, 0x1E for
  rifles, 0x22 for knives, unless already in that posture; the turn target becomes the direction, with a turn
  sound for guns when no animation started). A jamming gun (item flag, condition above 15) jams when Random(80)
  + 20 > condition + 50: its condition drops to Random(10) + 1, the merc says message 15, and nothing more
  happens. A ready soldier (attack-ready check = 1) records the attack: target slot (none: 0x40) and, for a
  visible soldier target, the second target byte +0x1C2 (no soldier: 0xFE) and, merc side to merc side, +0x1C3 =
  1; +0x1BE = 1, the attack tile +0xDC, global 0x229DD4 = 40000. A gun without ammo that can be reloaded makes a
  merc say message 5 (+0x1CD = 1).
- Other items need the jam check too; nothing happens in water.
- Knives (0x0F, 0x10) on another tile: the AP cost of the attack must be affordable; then the weapon is raised and
  a ready soldier records target slot / second target / +0x1C3 like a gun (no "none" values), +0x1BE, the tile.
- Throwables (0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D) and 0x5D: if the AP cost is affordable, the soldier turns to the
  tile, second target 0xFE, attack tile, +0x1BE = 1.
- The hand on its own tile: 4 AP, animation 0x0B (sound 4). On another tile: facing it, 3 AP and animation 0x26 for
  an unstandable tile (sound 3/30), else 5 AP (2 for an enemy picking up item 0x72) and animation 0x0C.
- 0x4C and 0x57–0x58: facing it; on an unstandable tile (or 0x4C where the quest check holds) 10 / 3 AP and
  animation 0x26 (sounds −250 / 30).
- 0x4D: a group move to the tile.
- First aid (0x49, 0x4A): 5 AP; first aid starts on the soldier itself or on the tile; if it does not start the
  soldier turns to it (its own tile: direction 5).
- 0x11 (a mine): not on another unstandable tile ("Can't bury mine there!"); 10 AP. 0x17, 0x19, 0x1B, 0x1E..0x20:
  6 AP. Animation 0x13 on the own tile, else 0x12 facing it.
- 0x76: another tile, 10 AP, animation 0x0C. 0x87: another tile, 8 AP, animation 0x0C (sound −50); not
  affordable clears the task. 0x45 on a door-class tile: 5 AP, animation 0x2E.
- 0x46 (a drink with charges): with charges left, Random(100) below its condition plays a sound and uses a charge.
- Any other item clears the task.
After a click that did something the merc panel updates unless Ctrl is held.

## Launching a projectile

When the attack animation reaches its launch command, the projectile leaves from the shooter's pixel point + (2,
4) toward the shot end point. Nothing happens with a negative item count or when the end lies within 1 pixel. The
shooter records the flight: flown 0, the item, the target tile, its facing; a "no soldier" second target or a
merc-side flag becomes "none".
- Throwables start at height 8500 and rise to a cap of 22000 (9000 on terrain class 3): the up slope is
  (sign + 2 × (cap − 8500) / (distance / 2)) / 2 and the down slope the same toward the end height (C division,
  sign = −1 for a negative difference, else +1). Tracking 2. Mercs practise stats 3 and 6.
- Bullets start at height 4500 + Random(1001) with no cap; slope (sign + 2 × (end height − start) / distance) / 2;
  tracking from the weapon table; +0xCC = 15. Mercs practise stat 8.
- The pixel walk uses Bresenham (larger axis steps every time). The busy counter rises by 1 and a 5000 timer
  starts. The shot sound plays: with a 0x5B attachment its volume is ((100 − the attachment's condition) /
  (100 / (silencer quality − 1)) + 1) × 10 + 20 (byte arithmetic, at most 120); else the weapon's sound if its id
  is below 0x63.
- Mercs, and anyone throwing, wear their gear: guns (items 1..14) lose one condition point when Random(2 × wear)
  is 0; the count drops by one (throwables through their own routine); the last round leaves a gun empty (reload
  timer 2000) or drops the item. Enemies firing guns spend no ammo and cause no wear.
- Within 20 pixels the projectile lands at once.

## Projectile flight (per frame)

A projectile moves only on frames where its timer (+0xCC) is 0; the timer is then set to 15. Each pixel step
(Bresenham) adds one to the flown count and the slope to the height (not below 0). With a cap, a throwable that
climbs past it switches to its down slope; a bullet levels off. Each new map tile other than the shooter's raises
the tile count; leaving the map ends the shot and the attack. The obstacle test (Bullet flight) runs on every
pixel outside the shooter's tile, with the skip distance (+0x1FF, cleared once flown past); a stop ends the shot
and the projectile lands at (x − 8, y − 6 − height / 600). A frame moves the projectile through up to "tracking"
tiles (the shooter's own tile does not count). A bullet still flying beyond its facing's low range over open
ground (object below 0x27, byte 2 below 0x10), and any throwable, casts a shadow at x, y − height / 400 (thrown:
/ 1000; above 18000: y − 18) with a sprite by height / 1000 (0–2: value 0, 3–4 the first (0xD2), … 21–22 the tenth;
value v = sprite 413 + v, so value 0 still draws sprite 413, a one-pixel dot). The drawing: each full draw, before the
effect records, slots 0..15 and 40..47 with +0x1C7 set draw sprite 413 + (+0x1C5) at world (+0xF6, +0x9E)
(DrawSpriteAtWorld 0x1698C: X = x − view_x + 32, Y = y − view_y + 28, drawn only when 16 < X < view_w + 32 and
14 < Y < view_h + 28).

**Shot end**: unless silent, the target soldier reacts and a gun makes noise at the shooter's tile (loudness from
the silencer table, or from a 0x5B attachment's condition as in Launching); all shot points are cleared, the
shooter's roll flag is set and the busy counter drops by one.

## Projectile impact

Where a projectile stops (tile, point, and the obstacle test's result), the shooter becomes the acting soldier and
the impact makes noise (loudness from the item's table entry for items below 0x21, else 0..2; the ground class).
- **A soldier on the tile**: in combat mode the view centres there; a hit counter rises; the first hit involving an
  enemy records a replay event 'M', and an enemy target moves story flag 0x22A1F5 from 1 to 2. A bullet deals
  BulletDamage (Damage of a bullet) at the projectile's height to the soldier (damage kind 1, attacker the
  shooter); a merc shooter practises stat 8. An explosive (+0x1C9 above 1) explodes when the obstacle test said
  "always" (−1) or, with condition 10 or more, when Random(100) − Random(50) is below the condition; otherwise it
  lands as an item and the same roll may set off its effect. Other throwables deal Random(4) + 1 at height 2500
  and land as item 0x5D.
- **No soldier**: thrown items behave as above (the view centres in combat mode after an explosive). A bullet:
  an enemy shooting at a merc makes the merc practise stat 2; a story object 0xAE in the story sector may react;
  unless the tile's byte 2 is 0x10+ with the shooter at x ≤ 0, a puff (or a splash on open water) shows with a sound
  (objects: 0x14–0x16 at volume 40; open ground 0x13; water 0x11; volume 88 otherwise); on an unstandable tile a
  breakable object (value from the sector's door table, minus 70 above 100) breaks when that value is below the
  bullet's damage and Random(3) is 0 (the shooter practises stat 8).

## Taking damage

A soldier takes damage at a projectile height, with a kind (1 bullet, 2, 3, 4, 6), an attacker slot and a tile.
- A soldier in state 4 (+0xBA) takes nothing. The wound total before the hit is hpMax − hp − wounds. A queued aim
  animation (0x1B, 0x1F, 0x23, 5; 0xE / 0xF also snaps the screen position to the tile centre) is remembered in
  +0x1E1 with its AP cost given back in +0x1E3.
- Bullets and kind 4 tire the soldier: fatigue rises by 50 × damage × (10000 − fatigue) / 10000 (see Fatigue and
  energy), and go through the hit-location rule
  (head / torso / legs, armour) unless the height is 0x1356; that rule gives the damage left.
- **With damage left**: the number shows over the soldier as "−N"; hp drops by the damage (never below 0); an enemy
  left with 1–14 hp dies outright (hp 0); a merc practises stat 9; wounds = hpMax − hp − the old wound total;
  +0x1F0 = hp / (+0x1F2 + 10) + 1; the portrait flashes. Below 15 hp the soldier collapses: attack forgotten,
  +0x186 = 1 unless negative, +0x1AC set below 10 hp, and when it is the selected merc the cursor resets.
- After the hit the soldier re-thinks, its attack tile is cleared, and its last target too unless +0x19C is 3.
- **Hit animation** by the current animation (posture): water (above 0x31) plays 0x35 / 0x38 / 0x3B, or 0x3C for
  kind 2; lying (0, 2, 0x29, 0x2A, 0x2F) plays 0x2F for a bullet, else 0x29; crouched (1, 3, 0xD) plays 0x30 for a
  bullet, else 3; unconscious (0x2B) plays nothing. Standing: a bullet plays the knock-down (0x28 then 0x2B) when
  the hit-location rule set the knock-down flag and hp is 15 or more, else 0x28 (turned towards the attacker when
  hp is below 15 and the attacker is a soldier; when the attacker is the soldier itself it faces its own new
  position); kind 3 plays nothing; others play 0x2C when the damage is above 40 or the blast tile is the soldier's
  tile, 0x2A above 5, and add 4 to the AP lost (a running task ends above 5). Kinds 3 and 6 play 0x2B (out) when hp
  is 0 or, for kind 6, when the damage is exactly 1.
- Every kind but 3 cries out. Noise = √damage (rounded down) − +0x192 / 2, when above 0. The ordinary hit plays a
  sound. AP lost = damage / 4 + the extra 4, at most the AP left. A merc's posture shows. A soldier attacker is
  recorded as the killer (+0x1DA) and reacts to the hit.

## Reaction to a hit

After damage from a soldier (slot below 0x40), the hit soldier reacts only when the attacker is on the other side,
the hit soldier has 15 hp or more, is not in state 2, and does not see the attacker already (sight flag not 1).
While the reaction runs, a scale byte holds damage / 10 (it is 0 otherwise).
- A merc hit with the flag set (TakeDamage always sets it) brings the view to itself, cries out and plays a sound;
  any merc practises stat 9.
- Within 14 tiles and with a clear sight line (range 14) the soldier now sees the attacker (see MarkSeen). A merc
  then brings the view to the attacker when it is visible (or a global flag is set) and its side knows it (1).
- Otherwise it only knows that someone shot: it re-thinks, its alert rises to 2 (with an Orders bump) if lower, its
  sight flag on the attacker becomes "heard" (0xFF) unless it is 2, it remembers the attacker's tile as last seen,
  and a merc's side marks the attacker heard (0xFF) unless the side already knew it (1, 2 or 0xFF).
- Last a follow-up routine (0x73AB4) gets the soldier, the attacker and the old sight flag.

## Interrupts

After the reaction to a hit, the hit soldier may interrupt the attacker's side.
- **Who qualifies**: a soldier on the map, not in state 2, and standing (animation 4 or above). Either it now sees
  the attacker and did not see it before (old sight flag not 1 or 2), and then it needs as many AP as its cheapest
  step; or the attacker sees it, it has seen nothing new (+0x1D5 at most 0), and the old flag was not "heard", and
  then it needs 1 AP. With enough AP left a "qualified" flag is set and the side's contest runs.
- **Merc or guard contest**: never while the tactical screen draws. It needs the same AP again. An enemy that does
  not see it loses at once. Otherwise compare (signed bytes, the merc must be strictly higher):
  - merc: level, +1 when it remembers the enemy, −(+0x1D5 − 1) when +0x1D5 is above 1, −1 when it only heard the
    enemy, − the hit scale (damage / 10);
  - enemy: level + the difficulty bonus (table per difficulty), +1 when it remembers the merc, −(+0x1D5 − 1) when
    above 1, +1 when the merc's side is known to the enemies.
- **Enemy contest**: a merc that does not see it, or an enemy in state 4, wins at once. Otherwise the enemy's
  level + difficulty bonus (+1 remembers, −(+0x1D5 − 1), +1 known, −1 only heard, − hit scale) must beat the merc's
  level (+1 remembers, −(+0x1D5 − 1)).
- **A won interrupt**: the winner's turn-done flag clears. During an enemy turn with the qualified flag set, the flag
  clears and the selected merc goes into the interrupt queue. A merc winner becomes the soldier that acts (the
  "keep this merc" slot instead when a game flag says so) and a merc interrupt mode starts; a guard or enemy winner
  goes into the queue and the queue mode starts. A visible attacker (or a global flag) sets a 5000 timer and
  remembers the attacker for the view. Then the turn hands over.

### Interrupt contests, exact steps (library ops 128 ask_merc_interrupt, 116 ask_interrupt)

Both are game logic (MercInterrupt? and EnemyInterrupt? in the original). Library entries `ja1_ask_merc_interrupt`
(w, h, slot, opponent) and `ja1_ask_interrupt` (w, h, slot, opponent): slot = the asking soldier, opponent = the
record index of the other soldier. Result (i32): 1 = the asker wins the interrupt, else 0. Notation: s = the
asker's record, o = the opponent's record, m = the merc's (or guard's) slot byte o / s +0x17A, j = the enemy's
index (its slot byte +0x17A − 0x28, 8-bit). sees (r, x) = s8 r +0x13A + x (1 = sees, −1 = only heard). BONUS
(difficulty) = the low byte of the s16 table (−1, 0, 1) for difficulty 0, 1, 2 (SPEC GAP (not seen: the Set Difficulty screen (world_load.md) gives 0, 1 or 2 only; another value needs an edited save): another difficulty reads
the original's neighbouring data; the port uses 0). push (x): itq[itq_len] = x, itq_len + 1 (8-bit; the same
stop point at itq_len ≥ 50 as Noise interrupt in ai.md). All sums are 8-bit and compared as signed bytes.

*Grant* (both contests, when the asker wins): s +0x1B0 = 0. If selected ≥ 8 and s8 interrupted = 1: interrupted =
0, push (selected).

*Merc contest* (op 128; the asker is a merc or guard, m = s +0x17A, j = o +0x17A − 0x28):
1. s +0x1C1 = 1. need = low byte of op 13 move_ap (slot) if sees (s, j) = 1, else 1.
2. If s8 need > s8 s +0x1A5, or screen_drawn ≠ 0: result 0.
3. If sees (o, m) ≠ 1: the merc wins (go to 5).
4. a = s +0x192 (level) + 1 if remember_m [8 m + j] = 1; − (s +0x1D5 − 1) if s8 s +0x1D5 > 1; − 1 if sees (s, j) =
   −1; − hit_scale. b = o +0x192 + BONUS (difficulty) + 1 if remember_e [0x28 j + m] = 1; − (o +0x1D5 − 1) if s8 o
   +0x1D5 > 1; + 1 if knowledge [m] ≠ 0. If s8 a ≤ s8 b: result 0.
5. Grant. If s8 o +0x9C > 0 or follow_all ≠ 0: show_timer = 5000, spotted = o +0x17A.
6. If m < 8: interrupter = m, interrupt_go = 1; if new_round ≠ 0 and kept_merc < 8: interrupter = kept_merc. Else
   (a guard): push (m), turn_moves = 1. Result 1.

*Enemy contest* (op 116; the asker is an enemy, j = s +0x17A − 0x28, m = o +0x17A):
1. s +0x1C1 = 1. need = low byte of op 13 move_ap (slot) if sees (s, m) = 1, else 1.
2. If s8 need > s8 s +0x1A5: result 0.
3. If sees (o, j) ≠ 1, or s8 s +0xBA = 4: the enemy wins (go to 5).
4. a = s +0x192 + BONUS (difficulty) + 1 if remember_e [0x28 j + m] = 1; − (s +0x1D5 − 1) if s8 s +0x1D5 > 1; + 1 if
   knowledge [m] ≠ 0; − 1 if sees (s, m) = −1; − hit_scale. b = o +0x192 + 1 if remember_m [8 m + j] = 1; − (o
   +0x1D5 − 1) if s8 o +0x1D5 > 1. If s8 a ≤ s8 b: result 0.
5. Grant. push (s +0x17A), turn_moves = 1. Result 1.

Fields used: `soldiers`, `remember_m`, `remember_e`, `knowledge`, `hit_scale`, `difficulty`, `screen_drawn`,
`selected`, `interrupted`, `itq`, `itq_len`, `follow_all`, `show_timer`, `spotted`, `interrupter`, `interrupt_go`,
`new_round`, `kept_merc`, `turn_moves`.

Evidence: MercInterrupt? 0x73D28 and EnemyInterrupt? 0x73F3C diff-tested in Unicorn against the original
(validate/diff/ai_diff.py `merc_interrupt`, `enemy_interrupt`, with --port).

## Stat loss message

A merc that loses a stat from a hit (see the hit-location rule) gets an on-screen message for a timer of 500: its
slot, the points lost, the body part ("head", "arm" for the torso, "leg") and the stat ("wisdom", "dexterity",
"agility"). An unarmoured head hit without a stat loss sets the message for any soldier with points 0xFF and no
texts. When the timer reaches 0 (10 less per timer tick), the next tactical pass shows it once: "NAME receives a
critical PART injury, losing N point(s) of STAT" for 2000 ms, points 0xFF "NAME hit in head, causing extra damage!"
for 800 ms (mercs only), and points 0xFE (stage 0 home warning, timer 7500) team quote 0x1C. Port: play::loss_set,
loss_timer_tick, loss_pass (lossmsg_diff).

## Fatigue and energy

Fatigue (+0x10C, 0..10000) falls by a rest amount (a negative amount tires). Energy (+0x18A) is 100 − fatigue / 100.
When energy changes, energy 50 or more clears +0x1BA and a merc's energy bar is redrawn (unless panel redraws are
off); when it does not change, a redraw request redraws the merc's panel.

## Explosion

An explosion has a thrower, a tile, a kind (10 counts as 3) and the impact test's result.
- Kind 2: size 2, loudness 25, volume 30. Kind 3: size 3, loudness 50, volume 50. Kinds 4–9: size 4, loudness 10,
  volume 30. (Other kinds leave these unset in the original.)
- The blast picture sits at the tile's top-left pixel plus a per-size offset; when it is wholly outside the view the
  sound is 20 quieter. The sound plays (sample per size).
- A size-2 blast on a tile no one can stand on breaks the object there, unless the current sector forbids it; a
  merc thrower is then marked (+0x9D).
- The blast effect starts (its id is kept), a kind-6 noise goes out from the thrower at the loudness, the thrower
  is remembered as the last blaster, and the tile is removed from a list of 30 tiles.
- The damage itself comes later from the blast effect (see Blast).

**Explosion, exact steps.** Library op 90 explode (byte thrower, int tile, int kind, int hit): wherever the
library's code makes it, it runs Explode below, unless bit 63 of host_ops is set. C exports `ja1_explode` and
`ja1_effect_hit` (port_api.md). s8 / s16 = the byte / word as a signed number; cdiv / cmod = C division and
remainder (cut toward zero). no_break, blaster: as in Blast, exact steps.

*Explode* (thrower = low byte of the first argument, t = tile, kind, hit):
1. If kind = 10: kind = 3. By kind: 2: size 2, loud 25, vol 30. 3: size 3, loud 50, vol 50. 4..9: size 4, loud
   10, vol 30. Other kinds: the original does not set size, loud and vol; it reads left-over stack bytes, whose
   values change with the calls made before. The port uses the 4..9 values (size 4, loud 10, vol 30) and runs all
   the steps below. Where another kind comes from: Ground blast (sector_start.md) passes s8 E[1] + 1. The XRA.DAT
   ground items of types 0x29 and 0x3F have E[1] = 1 or 2 (kinds 2, 3); type 0x6E gets E[1] = 1 first; type 0x71
   has E[1] = 2 except three entries with E[1] = 0x63 (kind 100): sector 23 tiles 2790 and 1968, sector 26 tile
   1491. Sector 26 also holds the mission trigger 0x72 (tile 2198), so its mission blast reaches tile 1491. No
   other source makes a ground item of type 0x29, 0x3F or 0x71: the fixed items, the MISC ITEM patches (0x71 with
   E[1] = 2 only), the XRA soldier templates and the equipment roll hold none, and Take ground item refuses them.
   Seen live (DOSBox-X, 2026-10-28, tac60 with a 0x71 ground item of E[1] = 0x63 and a timed effect of type 0x71):
   the original made an effect record of kind 100 with look 3 (the gas cloud, count 0x0A), at picture offset
   (−19, 0) instead of (−16, −18); at its hit the ground item was taken away (Effect hit step 4) and no soldier lost
   hit points. With the port's values the only difference is that offset and the sound and noise strength.
2. x = s16 of (cmod(t, 56) × 16 + ox), y = s16 of (cdiv(t, 56) × 14 + oy), with (ox, oy) = (−14, −15) for size 2,
   (−36, −34) for size 3, (−16, −18) for size 4.
3. If x + 40 < view_x, or y + 30 < view_y, or x > view_x + view_w − 1, or y > view_y + view_h − 1 (the picture is
   wholly outside the view): vol = (vol − 20) & 0xFF.
4. Host op 60 sfx (0x0C, 0x2B11, s8 vol, 1).
5. If size = 2 and Tile standable (t) < 0 and no_break is 0: host op 107 break_object (t, t1 of t, 1); then if
   thrower < 8: byte +0x9D of the thrower's record = 1.
6. fx = low byte of host op 178 puff (t, x, y, look, count, s8 of the low byte of kind, s8 of the low byte of hit)
   with look 2 (the explosion) and count 0x15 when kind < 4, else look 3 (the gas cloud) and count 0x0A. fx_id =
   fx; anim_signal = s8 fx.
7. Host op 127 make_noise (thrower, t, 0, loud, 6).
8. blaster = thrower if it is below 0x30, else 0x40.
9. The first of the 30 words of blast_list that equals t becomes 0xC41.

*Gas start* (t, kind = low byte) → cloud index or −1: turns = 3 if kind = 4, else 5. If t0_class[t0 of t] ≠ 3:
turns −= Random(2) + 1. The first cloud k (0..4) of gas_clouds whose s16 tile is 0xC41 gets tile t, turns and radius
3 (kind 4) or 5 (other kinds); the result is k. When no cloud is free the result is −1 and nothing is written.

*Effect hit* (t = tile, kind, power = s8 of the low byte of the third argument; kind compared as a 32-bit value):
1. If t1 of t = 0x86: t1 of t = 0, then all move codes are rebuilt (Rebuilding all move codes).
2. obj = t1 of t, s = cur_sector. If kind = 3 and obj is 0x35 or 0x36: sector_state byte 6 of s −= 1 (8-bit); if obj
   = 0x35 also byte 8 of s −= 1. Host op 89 replay_event (s8 s, s8 s, 0x56 if obj = 0x35 else 0x55). Then the
   workable count of s is recomputed (strategic.md, Workable trees).
3. If kind is 4 or 5: Gas start (t, kind); if it gives −1, done.
4. If kind is 2 or 3 and power > 0: host op 336 explode_art (t, 1 if kind = 3 else 0) (Craters and
   blast leftovers, below). Then if s = s8 story byte
   0x17: for j = 0..4, if the s16 word at byte 0x82 + 2j of row s of `sectors` equals t: host op 177 story_hit ().
   Otherwise (kind not 2 or 3, or power ≤ 0): host op 109 take_ground_item (t, 0).
5. If power > 0: Blast (t, radius, kind) with radius 2 for kinds 2 and 4, 4 for kinds 3, 5, 7 and 9, else 3.

**The explosion effect record (front-end).** The front-end keeps the effect records (the `fx_anims` entries 1..15;
entry 0 is never used). Host op 178 puff takes the first entry whose count (byte 0) is 0 and sets count = the count
argument − 1, its picture point (x, y), its tile, its look, its kind (byte), its power (the last argument, a byte)
and a "hit" flag 0; the result is the entry number, or −1 when all are in use. The effect timer counts it down
(soldier_update.md, The front-end's animation timers). At each drawn frame, for each entry in use whose kind is not
0: when its count (s8) is 0x13 or less and its hit flag is 0, the front-end calls `ja1_effect_hit` (tile, kind,
power) and sets the hit flag. (So an explosion of kind 2 or 3 hits one timer step after it starts, a gas cloud at
the first frame.)

**Drawing the effect records.** At each full draw of the tactical screen, after the overlays and roofs and before
the cursor pass, each entry 1..15 with count c ≠ 0 and kind ≠ 0 is drawn. An entry of kind 0 (a bullet's puff or splash, a
landed item's splash) draws sprite 413 + table[c] at its (x, y) as DrawSpriteAtWorld; the table pointer is the
record's +0xC: puff 0x9566B = 0, 0x78, 0x79, 0x7A, 0x7B; splash 0x95670 = 0, 0x7C, 0x7D, 0x7E, 0x7F (host op 178
look 1 / look 0). The pictures come from MAIN.DAT: E = EXPLODE.PCX (1596 × 60, 21
frames of 76 × 60 side by side) and S = SMOKBOMB.PCC (420 × 34, 10 frames of 42 × 34). By kind:

| kind | source column of frame c − 1 | drawn size | key | margins (mw, mh) |
|---|---|---|---|---|
| 3 (big explosion) | E, (c − 1) × 76 | 76 × 60, clipped (below) | 0x7B | (76, 60) |
| 2 (small explosion) | E, (c − 1) × 76 | 38 × 30: every second pixel of every second row of the 76 × 60 frame | 0x7B | (38, 30) |
| any other | S, (c − 1) × 42 | 42 × 34 | 0xD3 | (42, 34) |

1. X = x − view_x + 32, Y = y − view_y + 28 (back buffer, pitch 384). w × h = 76 × 60 (kinds 2, 3) or 42 × 34;
   sx = sy = 0.
2. If X < 0: sx = −X, w = w + X, X = 0. If Y < 0: sy = −Y, h = h + Y, Y = 0.
3. Nothing is drawn if w ≤ 0, h ≤ 0, X ≥ 384 − mw or Y > 252 − mh.
4. Kind 3: the w × h block at column (c − 1) × 76 + sx, row sy of E goes to (X, Y); source pixels equal to the key
   are not written. Kind 2: the 38 × 30 half-size picture read from column (c − 1) × 76 + sx, row 0 of E goes to
   (X, Y); a source pixel v ≠ key is written as remap table 0 [v]. Other kinds: the 42 × 34 block at column
   (c − 1) × 42 + sx, row 0 of S goes to (X, Y); key pixels are not written. (Kinds 2 and others ignore w, h and sy:
   the original's quirk.)

Then the hit step above runs. After it (or when the entry does not hit now): an entry of kind 3 with count 5 whose
tile has byte 3 = 0 and byte 0 below 0x6C starts the fire: a tile animation on that tile with the fire frames
(frames[0..43] = 0, 0x89, 0x88, … 0x80, then 0x83, 0x82, 0x81, 0x80 eight times, then 0) and count 0x2B (started as
host op 205 starts the ripple: the first free animation of 1..9, count 0x2B − 1, the tile's byte 3 = its number; no
free one: no fire), and sfx (0x1D, 0x2B11, 0x32, 5) (also when no animation was free). The tactical map draws it (tactical_map.md Animated tiles).

Evidence: DrawProjectilePass? 0x17548 (records 0xEB002, 18 bytes; byte +0xB = 0 → DrawSpriteAtWorld of the frame
table +0xC; else 0x153C8 (count, x, y, kind), then EffectHit or the fire: StartTileAnim (tile, 0x9562E, 0x2B),
PlaySfxSlot (0x1D, 0x2B11, 0x32, 5)); 0x153C8 (normal view, redraw_off ≠ 0: kind 3 BlitKey 0x522A0 (…, 384, 0x63C,
0x7B), kind 2 0x523CE half-size remap blit (…, 76, 60, 384, 0x63C, *0xE8C48 = remap table 0, 0x7B), else BlitKey
(…, 42, 34, 384, 0x1A4, 0xD3) from sheet + 0x17700); LoadTacticalSector? 0x2ADB9 (0x532BC loads "explode.pcx" with
pitch 0x63C and "smokbomb.pcc" at +0x17700 with pitch 0x1A4 into the sheet *0x93E5C). The zoomed-out path of
0x153C8 (redraw_off = 0) is in panel.md "The overview picture" (the small explosion), diff-tested by overview_diff.

### Craters and blast leftovers (`ja1_explode_dat`, host op 336 explode_art)

Host op 336 explode_art (tile, int big) is game logic, not a picture: it breaks the objects round the tile by the
crater patterns of DAT\EXPLODE.DAT, changes or takes the ground item on the tile and, in sector 0, can end the story
fight. Effect hit calls it with big = 1 for kind 3 (the big explosion), else 0.

**Exact steps.** t = s16 tile. t0 (x), t1 (x), t2 (x), t4 (x) = bytes 0, 1, 2, 4 of tile x (x can be a little off the
map; the original then reads and writes the bytes before or after the tile map). "Set c1 (x) = v" writes byte 1, "set
c2 (x) = v" writes byte 2. mark (x, v): t4 (x) = (t4 (x) & 0x80) | v. The 9 cells: D = −57, −56, −55, −1, 0, 1, 55,
56, 57 (k = 0..8, row by row); the object of cell k is tile a = t + D[k], its byte 2 sits one row higher at b = a −
56. FILL = 0, 0, 0, 0, 0, 7, 0, 8, 9. PIECE = 10 records of 12 bytes (i32 offset d, object p at +4, byte 2 q at +8):
(−1, 0x71, 0x50), (−1, 0x52, 0x47), (−56, 0x72, 0), (−56, 0x53, 0), (1, 0x0C, 0x48), (1, 0x0E, 0x51), (0x37, 0x52,
0x47), (0x37, 0x71, 0x50), (0x38, 0x0B, 0), (0x38, 0x0D, 0). CACHE = 12 records of 7 bytes (u16 tile at +0,
difficulty at +6): (0x15B, 0), (0x192, 0), (0x1C8, 0), (0x1C6, 0), (0x1FC, 0), (0x194, 1), (0x202, 2), (0x274, 0),
(0x203, 2), (0x2AA, 1), (0x15D, 0), (0x159, 0) (the original's table, read only).

*Object class* (o): 0 if o = 0; 6 if o is 0x27..0x36; 7 if o is 1..0x55, 0x7F or 0x80; 8 if o is 0x56..0x7E, 0x81 or
0x82; 9 above 0x82.

*Marks* (x, c) (byte-4 marks round a broken object; q (y) = 1 if t4 (y) ≠ 0, else 0):
1. By o = t1 (x): 6: at = x + 56, d = 0; if q (x + 56) = 0x50: d = 7; else if q (x + 57) = 0x50: mark (x + 57, 0x4D);
   else if t1 (x − 1) ≠ 0x52: d = 0x1E. Then if q (x + 1) = 0x51: mark (x + 1, 0x4E). If q (x + 57) is 0x49, 0x4E,
   0x46 or 0x4C: mark (x + 57, 0x63). If q (x + 1) is 0x4F or 0x4B: mark (x + 1, 0x63). If q (x + 57) = 0x51: mark
   (x + 57, 0x4F). (q is only 0 or 1, so of these tests only "t1 (x − 1) ≠ 0x52" can pass: the original's quirk.)
   0x0B: at = x + 1, d = 0x0C if q (x + 1) = 0x4E else 0x0A. 0x0C: at = x + 56, d = 0x0B if q (x + 56) = 0x4C else 8.
   0x0F: if the low byte of c = 6: mark (x + 56, 0x63); else mark (x, 0x63), (x − 1, 0x63), (x − 56, 0x63), (x − 57,
   0x63); d = 0. 0x22 or 0x25: at = x, d = 0x1E. 0x52: at = x + 57, d = 7 unless q (x + 57) = 0x50. 0x53: at = x + 57,
   d = 9. Others: d = 0.
2. If d ≠ 0: mark (at, d + 0x45).

*Explode dat* (t, big):
1. If cur_sector = 0 and s8 santino_spoke < 2: if t is 0x122..0x124, done; for each CACHE record whose tile = t and
   whose difficulty ≤ s8 difficulty: done.
2. Open DAT\EXPLODE.DAT: op 86 open_patch (kind 5, number 0, count 0); the result is not checked (the
   original passes a leftover register as its seek value, which has no effect: every read below follows a seek). The file holds 256 rows of 36 bytes: row = an object byte, 9
   cells of two i16 words (new object, new byte 2).
3. O = t1 (t), G = t0_class [t0 (t)]. If G = 3: t0 (t) = Random (3) + 0x1D (random = the host's random).
4. Ground item: i = byte 6 of t (none if 200 or more). If there is one, by its type (byte 0 of record i):
   - 0x5F: type = 0x7B, byte +1 = 0x63, byte +6 = 0.
   - 0x6E: type = 0x7C, byte +1 = 0x63, story byte 0x1E = 1 (world `story` + 0x1E, 0x22A05E in the original), byte
     +6 = 0.
   - otherwise: if the type is 0x3F, cur_sector = 0, t = 0x15B and santino_spoke ≠ 0: Forget soldier (slot 0x28)
     (shooting.md; a library procedure, no host op), byte 1 of sector_state row cur_sector += 1 (8-bit), op 82
     merc_dies (0x28, 0), op 170 stage_five (). Then op 109 take_ground_item (t, 0).
5. c = 0 (the class word). If big ≠ 0: c = Object class (O); if c = 9: big = 0.
6. If O < 0x1B: if O = 0 and G is 1..7: set c1 (t) = Random (2) + 3 if G = 3, else Random (2) + 1. Go to step 8.
7. For k = 0..8 (a = t + D[k], b = a − 56):
   a. o = t1 (a) (the old object). Op 96 seek (4k + 36 × o). v = s16 of op 87 read (2).
   b. If v < 0xFF: c = Object class (o); if c is not 6..8, or big ≠ 0: set c1 (a) = low byte of v. Marks (a, c). v =
      s16 of read (2). If v ≠ 0xFF: c = t2_class [t2 (b)]; if c is neither 0x10 nor 0x11, or big ≠ 0: set c2 (b) =
      low byte of v.
   c. If k is 0 or 3, o < 7 and t2 (b − 1) = 0x4A: set c2 (b) = 0x50.
   d. If o is 1..0x82 and big ≠ 0, in this order ("a/b := v1/v2" = set c1 (a) = v1, set c2 (b) = v2):
      1. If O is 0x56 or 0x66: k = 2 and o = 0x67: a/b := 0x72/0. k = 5: o = 0x57 or 0x75..0x79: a/b := 0x0E/0x51;
         else o = 0x71: a/b := 6/0; else o = 0x5B: a/b := 6/0 and set c1 (a + 1) = 0x0E, c2 (b + 1) = 0x51. k = 7: o =
         0x6D: set c1 (a − 56) = 0x0D, c2 (b − 56) = 0; else o = 0x61: a/b := 6/0, set c1 (a + 56) = 0x0D, c2 (b + 56)
         = 0. Then if k = 7 and (o = 0x58 or o is 0x7A..0x7E): a/b := 0x0D/0.
      2. If O = 0x37: k = 7 and o = 0x39: a/b := 0x0B/0. Else k = 5: o = 0x38: a/b := 0x0C/0; o = 0x52: a/b := 6/0;
         o = 0x3C: a/b := 6/0, set c1 (a + 1) = 0x0C, c2 (b + 1) = 0x48.
      3. If O = 0x57, o = 0x67 and k = 6: a/b := 0x71/0x50.
      4. If O = 0x67: if o = 0x65, k = 7 and t1 (a − 1) = 6: a/b := 6/0; else if o = 0x57, k = 6 and t1 (a + 1) ≠
         0x65: a/b := 0x71/0x50.
      5. If O is 0x38 or 0x52, k = 3, o = 0x37 and t1 (a − 1) = 0: a/b := 6/0; then if t1 (a − 1) = 0 and t2 (b − 1)
         = 0x4A: set c2 (b − 1) = 0x50.
      6. If O = 0x4B, k = 3 and o = 0x37: a/b := 0x0B/0.
      7. If o = 0x56 and k = 1: if t1 (a − 56) = 0x4E: set c1 (a − 56) = 0x53, c2 (b − 56) = 0; else if it is 0x50:
         set c1 (a − 56) = 0x52, c2 (b − 56) = 0x47.
      8. If (o = 0x56, k = 3 and t1 (t − 2) = 0) or (O = 0x56, k = 5 and t1 (t + 1) = 0x71) or (o = 0x72, k = 7 and
         O = 0x56) or (O = 0x39, k = 3 and o = 0x71): a/b := 6/0.
      9. If O = 0x39, k is 3 or 6 and o = 0x57: a/b := 0x71/0x50; set c1 (t) = 6, c2 (t − 56) = 0; Marks (t, c); set
         c1 (t + 56) = 6 if t1 (t + 56) = 0x53, else 0x0B; set c2 (t) = 0; Marks (t + 56, c).
      10. If O = 0x4A and k = 5: o = 0x38: a/b := 0x0C/0x48; o = 0x52: a/b := 6/0. Then Marks (a, c).
      11. If k = 3 and ((o = 0x5D and t1 (t − 2) = 0x5B) or (o = 0x5A and t1 (t − 2) = 0x59)): set c1 (t − 2) = 0x71,
          c2 (t − 58) = 0x50.
      12. If k = 3 and ((o is 0x3D or 0x3E and t1 (t − 2) = 0x3C) or (o = 0x3B and t1 (t − 2) = 0x3A)): set c1 (t − 2)
          = 0x52, c2 (t − 58) = 0x47; Marks (t − 2, c).
      13. If k = 1, o is 0x64 or 0x62 and t1 (t − 112) = 0x63: set c1 (t − 112) = 0x72.
      14. If k = 1 and ((o = 0x41 and t1 (t − 112) = 0x42) or (o is 0x43 or 0x45 and t1 (t − 112) = 0x44)): set c1 (t
          − 112) = 0x53; Marks (t − 112, c).
      15. If k = 0, o = 0x57 and t1 (t − 112) = 0x39: set c1 (t − 112) = 0x53; Marks (t − 112, c); set c1 (t − 56) =
          6, c2 (t − 112) = 0; Marks (t − 56, c).
      16. If O is 0x58, 0x72 or 0x39, t1 (t − 112) is 0x58 or 0x6D and t1 (t − 56) = 6: set c1 (t − 112) = 0x72.
      17. If O = 0x58, t1 (t − 112) = 0x0D and t1 (t − 56) = 6: set c1 (t − 112) = 6.
      18. If O is 0x58 or 0x72 and t1 (t − 112) = 0x4E: set c1 (t − 112) = 0x53.
      19. If O = 0x49 and k = 7: o = 0x39: set c1 (a) = 0x0B; o = 0x53: set c1 (a) = 6. Then Marks (a, c).
      20. If k = 1, o is 0x66 or 0x47 and t1 (a − 56) = 0: set c1 (a) = 6.
   e. If o is 0x1B..0x1D: set c2 (t) = 0. If o is 0x1E..0x20: set c2 (t − 1) = 0.
   f. If v ≥ 1000 and big ≠ 0: v = v − 1000. With x = t1 (a − 1), y = t1 (a − 56):
      - v < 10: a/b := 6/0; then with PIECE[v] = (d, p, q): set c1 (a + d) = p, c2 (b + d) = q.
      - 10: x = 0x57, 0x0E, 0x5C, 0x5D or 0x75..0x79: a/b := 0x71/0x50; else x = 0x0C: set c1 (a − 1) = 6, c2 (b −
        1) = 0, a/b := 6/0; else a/b := 6/0.
      - 11: a/b := (0x72 if y = 0x6D else 6)/0. 12: if y = 6: a/b := 6/0, set c1 (a + 56) = 0x0D, c2 (b + 56) = 0.
      - 13: if x is 0x56..0x58 or 0x75..0x79: set c1 (a − 1) = 0x71, c2 (b − 1) = 0x50; else set c1 (a − 2) = 0x71,
        c2 (b − 2) = 0x50, c1 (a − 1) = 6, c2 (b − 1) = 0. Then a/b := 0x53/0.
      - 14: a/b := 0x52/0x47 if x is 0x37..0x39 or 0x7F, else 0x71/0x50. 15: a/b := (6 if x = 0 else 0x53)/0.
      - 16: if y = 0x67: set c1 (a − 56) = 0x72, c2 (b − 56) = 0. Then a/b := 0x0E/0x51.
      - 17: a/b := 6/0 if y = 0x72, else 0x0E/0x51. 18: a/b := 6/0 if t1 (a + 56) ≠ 0x72, else 0x0D/0.
      - 19 and above: nothing.
   g. c = t1_class [O]. If c is 7 or 8 and t1 (a) < 10: if k is 5 or 7: set c1 (a) = FILL[k]; else if k = 8, t1 (a −
      56) < 15 and t1 (a − 1) < 15: set c1 (a) = FILL[8].
8. Op 88 close (). If t1_class [O] is 2 or 3: move code of t = 25300 (0x62D4). Otherwise, unless the move code of t is
   0x62D4 or 0x6338, the move code of t is computed again by the rules of Movement, with its old code as the
   previous code (the original calls MoveCode for the tile; ai_diff explode_dat --port runs it real: 162 of 600
   cases change a move code).

Notes. The class word c keeps its last value from one cell to the next (step 7g sets it at the end of each cell), and
step 7d's Marks calls use it. v in steps 7e..7f is the second word when the first was below 0xFF, else the first.

*Evidence (2026-10-25): LoadExplodeDat 0x33BD4..0x35204 (cells 0x31BF0, fill 0x31C02, CACHE 0x95E0E, PIECE 0x95FF0,
file "dat\explode.dat" 0x907F0, close 0x351A7, move code 0x351AC..0x351F4), Marks = ExplodeMarks? 0x33820 (with
0x23938 = byte 4 ≠ 0), Object class = ExplodeClass? 0x33B60. Mirror tools/ai.py explode_dat / explode_marks /
explode_class; test `ai_diff.py build/mem_tac60.bin 3000 --only explode_dat` (3000 cases, 0 mismatches; Forget
soldier, merc_dies, stage_five, take_ground_item and MoveCode stubbed on both sides, the call log compared).*

## A thrown item lands

The thrower, the tile, the item and its condition, and the landing point are given. An item already lying on the
tile is sprung first (see Springing a ground item, not armed), then the item (count 1, the condition) is put on the
tile. On water a splash effect starts. r = Random(4) + 4: the landing sound plays at volume r × 10 + 20 (a splash
sample on water), and a noise goes out from the thrower at loudness r with the ground's class: kind 8 on water or for
item 0x5D, else kind 9.

**A thrown item lands, exact steps** (library op 176 land_item (slot, tile, byte item, int condition, int x, int y);
S = the soldier record of the slot; the result is none):
1. E = 12 zero bytes; E[0] = item, E[1] = 1, E[2] = the low byte of condition.
2. If the tile's byte 6 is below 200 (a ground item lies there): op 108 trigger_item (slot, tile, byte 6, 0); its
   result is not used.
3. Put item (tile, E, b6 1, exact 0, note 0, near 0) (Putting an item on the ground; `ja1_put_item`).
4. Water = the tile's byte 0 ≥ 0x6C and its byte 1 is neither 0x1C nor 0x1F (both read after step 3).
   - Water: fx = the low byte of op 178 puff (tile, x, y, 0 (splash), 5, 0, 1); fx_id = fx, anim_signal = fx as a
     signed byte. r = Random(4) + 4 (a byte); sample 0x1F; kind 8.
   - Else: r = Random(4) + 4; sample 0x10; kind 8 if item = 0x5D, else 9.
5. op 60 sfx (sample, 0x2B11, the signed byte (r × 10 + 20) as an int, 1).
6. op 127 make_noise (S[0x17A], tile, t0 class of the tile's byte 0, r, kind).

## Start of a soldier's turn

At its side's turn start each soldier:
- flips its +0x1C4 marker (1 becomes 0xFF, 0xFF becomes 0), forgets a queued aim and its AP, clears +0x16D;
- continues first aid when it is bandaging someone (below), and rests on its spare AP (below);
- bleeds (below), and drowns when it has no energy left in deep water (below);
- if alive: clears +0x1F2. In combat: its sightings age, a remembered noise to check ages out with its counter,
  and its AP become its maximum per turn plus up to 5 saved from the last turn (at most 25); a merc that was set
  to state 8 (+0x17E) leaves it, may act again, and is marked when it is off its stored tile; an enemy may act
  again. Out of combat its AP are simply its maximum. A merc with 15+ hp in the current sector faces again.
- its animation delay is recomputed.

**Bleeding**: only with more than 12 wound points or the bleeding mark (+0x186 = 1), and not while doing first
aid. A timer counts down each turn; when it runs out: if hp is short of hpMax − wounds and the mark is not set, the
wounds grow by 1; otherwise the soldier takes 1 damage (kind 3, no attacker) — with the mark set a merc with 10+
hp says a line once, and hpMax above 15 drops by 1 together with the wounds. The timer restarts at
hp / (+0x1F2 + 10) + 1.

**Drowning**: in a deep-water animation with energy 0. The first time the soldier only says a line (and
practises stat 4); after that it takes Random(10) + 2 + Random(10) damage per turn (doubled for +0x19C = 5), kind 2;
when it dies a death routine runs (not yet specified).

## Getting up after a collapse

A collapsed soldier (hp above 0, in play, conscious) rolls to get up when its animation reaches that step. It makes
a small noise (kind 3, loudness 7). A medical item (0x0F or 0x10) in hand helps; otherwise each pocket holding one
is moved to the hand when two Random(100) rolls both fall below dexterity, or always for soldiers that are not hired
mercs (profile 0x3C or above) or of AI type 1. One time in five (Random(5) = 0) it only plays a struggle animation.
Otherwise the chance is ((hpMax − hp − wounds) / 2 + hp) × energy / 100 − (100 − dexterity) / 4, then +80 % with a
medical item, +40 % for non-mercs or AI type 1, at most 95. A Random(100) at or above the chance kills the soldier
(death routine). Below it the soldier gets up: with a medical item it practises stats 4, 3 and 2 by 25; without one
by 10 and then takes Random(30) damage (kind 2) and continues its animation.

Port: library op 76 revive_roll (`aim.rs` `ja1_revive_roll`, ReviveRoll? 0x401B4), tested by `ai_diff.py --port
--only revive_roll`. The step is word 0x1EA at the end of script 0x3E (deep water: script 0x3D runs on into 0x3E; no other
script has word 0x1EA); the next word is 999 (= SET_ANIM 499). Each path after the first test sets a new animation or kills the
soldier, so the script never reaches 999. Until 2026-11-12 the port had no handler: soldiers in deep water got
animation 499 and could never move again (#52 c12347).

## A melee blow

When the attack animation reaches the blow, the attacker becomes the acting soldier.
- **No soldier on the attack tile**: in the story sector a blow at object 0xAE starts a story event. Nothing else.
- **A soldier**: it is marked as attacked; a merc attacker practises stat 3. The blow cannot miss (chance 99, height
  0x1356) when the target is on the attacker's side, does not see the attacker, or has less than 15 hp. Otherwise
  the chance comes from the melee chance rule (not yet specified) with the attacker's aim, and the blow lands at
  height Random(2500) + 2500.
- Random(100) below the chance hits. Damage = (weapon damage + Random(31) − 15 + (level − 1) × 2 +
  ((hpMax − hp − wounds) / 2 + hp − 50) / 5 + (chance − roll) / 5) × weapon condition / 100, at least 1; it is dealt
  as a bullet-kind hit from the attacker (Taking damage). A merc practises stat 3. A weapon above condition 10 wears
  by Random(n), n = damage / 10 (halved for item 0x10), at most condition / 10, only when n is above 1.
- A missed sure blow makes a target with 15+ hp (not in state 2) react (Reaction to a hit); a merc target
  practises stat 2.
- Always: the attack sound plays with the AP of the action (shot AP + aim), and +0x1BE clears.

## Death

A soldier that dies (optionally "quietly", e.g. for scripted deaths) — nothing happens to one already dead. Its
picture point is fixed at its position, its hp, maximum hp, wounds and AP become 0 and its fatigue full; a normal
death also stops its animation and running task; a quiet one only forgets its first-aid task. Several per-soldier
flags clear.
- **Leaving the board** (every death, before the side-specific part): the soldier's known tile clears. Every soldier
  whose three target / follow fields (+0x1D8, +0x1F7, +0x1C2) name it gets "none" (0x40) there. The other side
  forgets it: for a merc or ally, each enemy drops its sight flag on it (a flag of 1, "sees now", also lowers that
  enemy's count of seen opponents), its last-seen tile and its memory of it; for an enemy, mercs and guards 0..15 do
  the same. A merc or guard (slots 0..15) and an enemy also clear their own row of last-known enemy tiles, unless
  their AI kind is 2 ("never acts"); an enemy that was the last spotter stops being it. Last, its knowledge level is 0.
- **A merc**: the sector loses one merc from its count; a replay event records the death; every item it carries is
  put on its tile (normal death) and removed from it (a quiet death keeps some items, not yet specified); the item
  panel refreshes when it was the selected merc; one fewer merc is on duty; the death counts as a merc death, or as
  a team-member death for non-merc profiles (which also sets a flag).
- **A guard** (slots 8–15) or **another ally** (16–39): the sector's matching count drops, a death counter rises
  and a replay event ('R' / 'S') records it.
- **An enemy**: a mission runner is released (and the mission is cancelled while it runs with a target); its items
  in entries 7–11 drop on its tile (one may clear a sector item marker); a follow-up check runs.
- Then it is marked dead. A quiet death ends with the merc-death routine; a normal one plays the death animation
  (a water one in deep water) and a merc killer gets a kill, practises stat 9 by the victim's level × 5 and is
  remembered as the last killer.

## Blast

A blast has a centre tile, a radius and a kind. The centre takes the blast at ring 0. Then, for each of the 8
directions (north first, clockwise), the blast moves out ring by ring up to the radius. A step stops that direction
when it would leave the map, when the object there blocks movement in that direction (walls by a per-wall,
per-direction table), or when it meets an object of class 6 (which also shortens the reach by one ring for every
such object); a kind-3 blast stopped at ring 1 by a tile no one can stand on breaks that object (unless the sector
forbids it). Each tile reached takes the blast at its ring, and from it a side fan of (radius − ring) tiles in the
next direction clockwise also takes it at the same ring, skipping (without moving) where an object blocks.

**One tile**: by kind: 2 — damage Random(26) + 25; 3 — damage Random(51) + 50 (both explosions); 4 and 5 — the tile
is marked gassed and stun Random(1001) + 2000; 6 and 7 — stun Random(2001) + 5000 and damage 1; 8 and 9 — stun
Random(2001) + 4000 and damage Random(16) + 15 (gas); 99 clears the gas mark. For a soldier in play and alive on the
tile: rings 1–4 take 20 % less per ring, farther rings a tenth. Any effect makes a merc blaster practise stat 6. A
standing soldier off the centre turns towards it. Damage (not for AI type 4) is dealt at height 2500 as the blast's
effect kind from the blaster; explosions also wear the soldier's items (below). Stun (not for AI type 4; a gas mask
in pocket 3 lets only 100 − its condition per cent through) adds to fatigue; without damage the soldier coughs; gas
on water (or kind 4) sets a flag; the soldier re-thinks. Without damage, a soldier blaster becomes the killer (if
there was stun) and a soldier with 15+ hp reacts with strength stun / 300.

**Item wear**: each item (and each of up to four stacked items when it stacks and the count is above 1, and its
attachment) loses Random(n) condition, n = the damage minus the item's absorb value for helmets and armour, halved for
some types, only when n is above 1 and the type wears at all; condition never drops below 1.

**Gas clouds**: up to five clouds (tile, turns left, radius). Once per turn each cloud in use gasses its area again
(a kind-4 blast); its turns count down, and when they run out the gas is cleared (kind 99) and the cloud is freed.

**Blast, exact steps.** Library op 161 gas_tick: wherever the library's code makes it, it runs Gas tick below,
unless bit 63 of host_ops is set. C exports `ja1_blast`, `ja1_blast_tile`, `ja1_gas_tick` (port_api.md). s8 / s16 =
the byte / word as a signed number; cdiv = C division (cut toward zero). occupant(t) = byte 5 of tile t & 0x7F, none
(0x40) if that is 0x30 or more. cur(R) = the animation word u16 +0x1E8 + 2 × +0x1F3 of record R. blaster = the world
byte `blaster` (the slot of the last explosion's thrower, 0x40 = none). no_break = byte s8 `tileset` of the world
table `no_break` (10 bytes, one per tileset; an index outside 0..9 reads 0): when it is not 0 the tileset's objects
do not break in blasts. Item type bytes: item_types[12 × i + b]; a byte past the table end reads 0.

*Blast tile* (c = centre, t = tile, ring, kind; kind compared as an unsigned 32-bit value):
1. dmg = 0, stun = 0, fx = none, flag = 1. By kind: 2: dmg = Random(26) + 25, fx = 4. 3: dmg = Random(51) + 50,
   fx = 4. 4 or 5: byte 5 of t |= 0x80, then stun = Random(1001) + 2000, fx = 6. 6 or 7: stun = Random(2001) + 5000,
   dmg = 1, fx = 6. 8 or 9: stun = Random(2001) + 4000, then dmg = Random(16) + 15, fx = 6. 0x63: byte 5 of t &=
   0x7F. Other kinds: nothing here.
2. o = occupant(t). Done if o is none, or +0x182 of o is 0, or +0x138 is 0, or +0x183 is 0. P = record o.
3. ring (signed) below 5: dmg −= cdiv(dmg × ring × 20, 100), stun −= cdiv(stun × ring × 20, 100). Else dmg =
   cdiv(dmg, 10), stun = cdiv(stun, 10). Then a negative dmg or stun becomes 0.
4. If dmg or stun is not 0 and blaster < 8: host op 105 stat_practice (blaster, 6, 1, 0).
5. If cur(P) ≥ 4 and s16 +0x114 of P ≠ c: +0x1A2 of P = low byte of host op 6 direction (s16 +0x116, s16 +0x118,
   (c % 56) × 16 + 8, (c / 56) × 14 + 7).
6. If dmg ≠ 0 and s8 +0xBA of P ≠ 4: host op 110 take_damage (o, 2500, dmg, fx, blaster, s16 of c). If fx = 4: Blast
   wear (P, dmg). (P is read again after the call.)
7. If +0x183 of P ≠ 0 and stun ≠ 0 and s8 +0xBA ≠ 4: if +0x24 (item entry 3) is 0x28 (gas mask): stun = cdiv((100 −
   s8 +0x26) × stun, 100). If then stun ≠ 0: Rest (o, −stun, 1) (turns.md); if dmg = 0: host op 65 effect_slot (+0x17A
   of P, 3) and flag = 0; if fx = 6 and (t0_class[t0 of tile s16 +0x114 of P] = 3 or kind = 4): +0x1DB = 1; +0x1C1 = 1.
8. +0x1C4 of P = 1.
9. If dmg = 0 and blaster < 0x40: if stun ≠ 0: +0x1DA = blaster. If s8 +0x183 ≥ 15: Hit reaction (o, attacker =
   blaster, flag, damage = low byte of cdiv(stun, 300)) (shooting.md).

*Wear amount* (i = item type, d = damage) → condition lost: 0 if item_types[12i + 7] is 0. If i is 0x23..0x26: d =
d − s16 absorb[0x1E / 2 + i] (the helmet values, i.e. the word at byte 0x1E + 2i of `absorb`), not below 0. If i is
0x2D..0x31: d = d − s16 absorb[i], not below 0. If item_types[12i + 12] (byte 0 of the next type) is not 0: d =
cdiv(d, 2). Result = low byte of Random(s16 of d) when d > 1, else 0 (no Random call).

*Blast wear* (P, d): for each item entry e = 0..11 (12 bytes at 12e): if byte 0 ≠ 0: if s8 byte 2 < 0, go to the
next entry (its attachment too is skipped); byte 2 −= Wear amount (byte 0, d) (8-bit), and becomes 1 if s8 below 1;
if item_types[12 × byte 0 + 6] > 1 and s8 byte 1 > 1: for c = 0..3, if s8 byte 8 + c > 0: it loses Wear amount
(byte 0, d), and becomes 1 if s8 below 1. Then (also for an empty entry) if byte 3 ≠ 0: byte 5 −= Wear amount
(byte 3, d), and becomes 1 if s8 below 1. Last, if +0x17A of P < 8 and = the world's `selected`: host op 166
item_panel (+0x17A, 0, 1).

*Blast* (c = centre, radius r, kind): Blast tile (c, c, 0, kind). Then for each direction d = 1..8:
1. lim = r (32-bit). If s16 of lim < 1: next direction. side = low byte of lim − 1. cur = c, i = 1.
2. nxt = step(cur, dirStep(d)) (s16). If nxt = cur: stop. Else if t1_class[t1 of nxt] = 6: lim −= 1, stop if s16 i
   > s16 lim. Else stop if Object block (nxt, d, NONE) ≠ 0 (soldier_update.md). On a stop: if kind = 3 and s16 i < 2
   and tile_standable(nxt) < 0 and no_break is 0: host op 107 break_object (nxt, t1 of nxt, 1). Then the next
   direction.
3. Blast tile (c, nxt, i, kind). cur = nxt.
4. Side fan: if side ≠ 0: d2 = d + 1, or 1 when d = 8. f = nxt; repeat side times: t2 = step(f, dirStep(d2)); if t2 ≠
   f and Object block (t2, d2, NONE) = 0: Blast tile (c, t2, i, kind), f = t2.
5. side = side − 1 (8-bit), i = i + 1 (16-bit). If s16 i ≤ s16 lim: back to 2.

*Gas tick*: for each cloud k = 0..4 of the world's `gas_clouds` (6 bytes each: u16 tile, s16 turns left, s16 radius;
tile 0xC41 = free): skip a free one. Blast (s16 tile, s16 radius, 4); turns −= 1; if s16 turns ≤ 0: Blast (s16 tile,
s16 radius, 0x63) and the tile word becomes 0xC41.

## A side's new turn

When the turn changes: the bleed marks clear and a pending merc remark plays. Unless the previous call already did
so, arriving guards end the call here (result 1), and a waiting interrupt turn hands over instead of starting a new
turn. A new turn resets its timers and flags; in combat the low-AP warnings and the interrupt queue reset; outside
combat the guards (slots 8–15) each think once. Then the gas clouds tick, combat ends when the sector has no mercs
left, every living soldier below slot 40 starts its turn (Start of a soldier's turn), and enemies remember their
visibility. In combat the view and panel go to the merc to show (another one when that merc is down). Last, five
story counters and a speech delay count down, a waiting speaker may talk, Santino's countdown advances in sector 0
(at 5 the sector loses an enemy and his scene runs), bleeding soldiers cough, and pending redraws happen.

## Handing the turn over

**Next turn** (nothing while a blocking counter runs): the queued-interrupt flag clears; if no merc is set to be
shown, the selected merc (or another one during an enemy turn) is. A drawn tactical screen redraws once and stops
drawing. In mode 1 the next interrupter from the queue becomes the selected soldier and the turn banner shows;
otherwise it is stored aside.

**The interrupt queue**: the last usable entry is taken — a soldier in play with 15+ hp whose turn is not done. Free
entries are passed over without shortening the queue; an unusable soldier is dropped from it (and, as a quirk,
returned when it was the first entry). An empty queue gives "nobody".

**A merc interrupt turn** (nothing while a blocking counter runs): the tactical screen is drawn again; every merc or
guard in play waiting in state 8 leaves it if its turn is not done (else it is marked); all but the interrupter get
a refresh; the interrupter (the first merc when none is set) is shown and the view moves to it.

## Spending AP and fatigue

Every action spends AP and fatigue through one routine (nothing happens for a dead or unconscious soldier).
- **AP** (an amount of 0 or 99 means none): at most the AP left — except for the selected soldier during an animation
  that needs no AP check. In combat the AP kept back for a queued aim pays first and the rest comes off the AP
  left; outside combat AP are not spent. The selected soldier's AP never go below 0.
- **Fatigue**: a non-zero amount is scaled by a factor = (200 − energy) + (100 − ((hpMax − hp − wounds) / 2 + hp))
  + the conditions' modifier (only a fifth of it for soldiers of class 3, on ground 0x97 and above, or in state 7)
  + a modifier for mercs and guards, ×5 while swimming. Positive amounts become amount × factor / 100, negative
  ones amount × 100 / factor. The result is added to fatigue (Fatigue and energy) after the panel refreshes.

## Stat practice

Only mercs (slots 0–7) practise, and only in a practice mode (mode 2 ignores flagged practice). Unflagged practice
of any stat but hpMax rolls three times as often. Stats 1–9: hpMax, agility, dexterity, wisdom, (+0x18E), (+0x18F),
(+0x190), marksmanship, level. Each stat has a progress counter.
- **Mode 1 (gain)**, per roll: chance = 100 − (progress / 100 + stat); for the level 100 − (progress / (level ×
  1000) + level) × 10. A positive chance is scaled by (wisdom progress / 100 + wisdom − 50) %; marksmanship in state
  6 gets a fifth of it; at most 99. Random(100) below the chance adds one to the progress.
- **Other modes (loss)**: chance = progress / 100 + stat (minus 1 for stats 1–4; for the level (progress / (level
  × 1000) + level − 1) × 10), reduced by the same wisdom percentage, at least 1; Random(100) below it removes one.
- The total change goes to the stat's progress and, for unflagged practice of any stat but the level, to the
  level's progress too.

Exact steps (host op 105 stat_practice (slot, stat, n, flag); C export `ja1_stat_practice`, see port_api.md). World
fields: `soldiers` (record S = the soldier in `slot`), `stat_progress` (row P = 0x12 bytes at 0x12 × S +0x17A: nine
signed 16-bit counters, counter k − 1 for stat k). stat and flag are bytes, n is a signed 16-bit number. Divisions
are C divisions (the quotient is cut toward zero); "s16" = the low 16 bits as a signed number.
1. Stop if S +0x17A > 7, if S +0x19B (the practice mode) = 0, or if flag ≠ 0 and +0x19B = 2.
2. If flag = 0 and stat ≠ 1: n = s16(3 × n). Stop if stat is not 1..9.
3. v = the stat's byte of S (unsigned): stat 1 +0x184, 2 +0x18B, 3 +0x18C, 4 +0x18D, 5 +0x18E, 6 +0x18F, 7 +0x190,
   8 +0x191, 9 +0x192. p = counter stat − 1 of P; wp = counter 3 of P (wisdom). b = wp / 100 + s8 S +0x18D − 50.
   delta = 0.
4. n times (none when n ≤ 0), each time:
   - Mode 1 (+0x19B = 1): c = 100 − (p / 100 + v); for stat 9: c = 100 − (p / (v × 1000) + v) × 10. c = s16(c).
     If c > 0: c = s16(c + (b × c) / 100). If stat = 8 and S +0x19C = 6: c = c / 5. If c > 99: c = 99.
     If Random(100) < c: delta + 1.
   - Other modes: c = p / 100 + v − 1 for stats 1..4, p / 100 + v for stats 5..8, (p / (v × 1000) + v − 1) × 10
     for stat 9. c = s16(c). If c > 0: c = s16(c − (c × b) / 100), and c = 1 if that is below 1. If Random(100) <
     c: delta − 1.
   Random(100) is drawn on every roll. (Stat 9 with level v = 0 divides by zero in the original; the port then
   uses 0 for p / (v × 1000).)
5. Counter stat − 1 of P + delta (16-bit wrap). If stat ≠ 9 and flag = 0: counter 8 (the level) of P + delta.

**Resting on spare AP** (start of turn): each AP above 5 left from the last turn counts. By the soldier's current
animation, a rate applies: most standing, crouching and lying animations rest (−200, −100 or −50 fatigue per spare
point), swimming and some others tire (25, 50 or 100 per point, scaled by 25 − the spare points). Resting does not
happen with +0x1DB set or on a gassed tile without a gas mask.

**Thrown items**: throwing takes one item off the stack: a non-stacking item or the last one clears the pocket;
otherwise the next stacked item's condition becomes the pocket's and the count drops by one.

**First aid at the turn start**: a medic in a bandaging task (+0x1B9 5 or 6) on the bandaging animation (0xF)
treats the soldier on its partner tile (+0xD6). With no one there, or a patient that is dying, dead or not in a
patient task (4 or 6), the medic says "abandoned first aid" and its task ends. A patient with no wounds and 15+ hp
only ends the medic's task. Otherwise the medic's main-hand item supplies charges (the sum of its units'
conditions):
- Points = the medic's AP × ((dexterity + 2 × min(charges, 100) + 3 × medical + 10 × level) / 7) / 50, all C
  integer division; 0 points do nothing. A medical kit (item 0x4A) gives half again (first aid kit 0x49 does not).
- The points are capped by the need: 2 per hp below 15, plus 1 per wound beyond those.
- Charges used: with the medical kit half the points (at least 1), else the points; never more than the charges
  (then the points become 2 × or 1 × the charges).
- Hp below 15 heals first at 2 points per hp (wounds drop by the same amount, not below 0); at 15 the patient stops
  bleeding (+0x186, +0x187 cleared). Points left close one wound each, unless +0x186 is set.
- The medic spends (points used × its AP / uncapped points) AP, two thirds of that with the medical kit, with 50
  fatigue per AP; it practises medical (points used / 3 rolls) and dexterity (points used / 2 / 3 rolls).
- A patient now without wounds at 15+ hp says "is all bandaged!" and its task ends. The used charges come off the
  last units first; an emptied unit drops out of the stack, an emptied first unit removes the item. The selected
  merc's item panel refreshes. With no charges left the medic says "ran out of bandages!" and its task ends.

**Starting first aid** (a first aid kit or medical kit used on a tile; the medic's own tile means itself): a medic
already in a task gets "is already performing first aid! Press ESC to abort that first." — except a patient (task 4)
may start bandaging itself. With no soldier on the tile: "No one there". A patient with no wounds that is not
bleeding (+0x186 = 1) says "doesn't need first aid". Otherwise the medic forgets its target, pays 5 AP, takes the
task (5 for another soldier, 6 for itself) with the patient's tile, plays animation 0xE, and the patient's medic
count (+0x1CA) rises. Bandaging another: the medic faces the patient; unless the patient is bandaging itself it
becomes a patient (task 4) and, if it has 15+ hp and is not in a moving animation, it stops, sits (animation 4) and
faces the medic. Bandaging itself: the medic faces direction 5. Both portraits redraw and a marker shows on the tile.

**First aid, exact steps.** Library ops 191 first_aid and 151 aid_tick: wherever the library's code makes one of
them, it runs the procedure below, unless bit 63 of host_ops is set. C exports `ja1_first_aid` and `ja1_aid_tick`
(port_api.md). s8 / s16 = the byte / word as a signed number; cdiv = C division (cut toward zero; a divisor of 0:
port_api.md, Division by zero). cur(R) = the animation word u16 +0x1E8 + 2 × +0x1F3 of record R. occupant(t) = byte
5 of tile t & 0x7F, none (0x40) if that is 0x30 or more (a tile outside 0..3135 reads 0). Texts of host op 57
merc_text (slot, text, ms): 5 "abandoned first aid", 6 "is all bandaged!", 7 "ran out of bandages!", 8 "is already
performing first aid! Press ESC to abort that first.", 9 "doesn't need first aid".

*Charges* of record R = the sum over the units k = 0..s8 +0x01 − 1 of item entry 0 (the main hand) of s8 (+0x02
for k = 0, +0x07 + k for k ≥ 1); none when the count is 0 or less.

*Use charges* (R, n): for k = s8 +0x01 − 1 down to 0, while n > 0: o = +0x02 for k = 0, else +0x07 + k; c = s8 o.
If c > n: o = o − n (8-bit), n = 0. Else if k = 0: n = n − c, the 12 bytes of item entry 0 become 0. Else: o = 0,
n = n − c, +0x01 = +0x01 − 1 (8-bit).

*Apply first aid* (P = patient, M = medic, sup, cap); result = charges used:
1. If s8 P +0x185 < 1 and s8 P +0x183 ≥ 15: result 0.
2. ap = s8 M +0x1A5. full = cdiv(ap × cdiv(s8 M +0x18C + 2 × cap + 3 × s8 M +0x18E + 10 × s8 M +0x192, 7), 50).
   If full = 0: result 0.
3. kit = (M byte +0x00 = 0x4A). base = full + cdiv(full, 2) with kit, else full. pts = base.
4. short = (15 − P +0x183) & 0xFF if s8 P +0x183 < 15, else 0. need = 2 × short; if s8 P +0x185 − short > 0:
   need = need + s8 P +0x185 − short. If need < pts: pts = need.
5. With kit: used = cdiv(pts, 2); if used = 0 and pts > 0: used = 1; if used > sup: used = sup, pts = 2 × sup.
   Without: used = pts; if used > sup: used = pts = sup.
6. left = pts & 0xFF. If left ≠ 0 and s8 P +0x183 < 15:
   - if left ≥ 2 × short: P +0x183 = 15; P +0x185 = P +0x185 − short (8-bit); left = left − 2 × short (8-bit);
   - else: half = left / 2; P +0x183 = s8 P +0x183 + half, P +0x185 = s8 P +0x185 − half (8-bit each); left = left
     mod 2;
   - if s8 P +0x185 < 0: P +0x185 = 0. If s8 P +0x183 ≥ 15: P +0x187 = P +0x186 = 0.
   - host op 149 portrait_refresh (P +0x17A, 1).
7. If left ≠ 0 and P +0x185 ≠ 0 and P +0x186 = 0: if s8 P +0x185 ≤ left: left = left − P +0x185 (8-bit), P +0x185
   = 0; else P +0x185 = P +0x185 − left (8-bit), left = 0. Host op 153 wound_bar (P slot, 1).
8. pts = pts − left. spent = cdiv(pts × ap, base); with kit spent = cdiv(spent × 2, 3).
9. Spend AP (M, amount = s8 low byte of spent, tire = s16 (spent × −50)) = op 31 spend_ap.
10. If pts ≠ 0: op 105 stat_practice (M slot, 5, s16 cdiv(pts, 3), 0). If cdiv(pts, 2) ≠ 0: op 105 stat_practice
    (M slot, 3, s16 cdiv(cdiv(pts, 2), 3), 0).
11. Result used.

*First aid tick* (op 151 aid_tick (slot); S = record slot):
1. If S +0x1B9 < 5 or cur(S) ≠ 0xF: done.
2. o = occupant(s16 S +0xD6). text = 5.
3. If o ≠ 0x40: P = record o. If P +0x188 = 0 and P +0x183 ≠ 0 and P +0x1B9 is 4 or 6:
   a. If P +0x185 = 0 and s8 P +0x183 ≥ 15: op 50 end_task (slot, 1). Done.
   b. c = Charges (S). used = Apply first aid (P, S, c, min(c, 100)).
   c. If P +0x185 = 0 and s8 P +0x183 ≥ 15: op 57 merc_text (o, 6, 1000); op 50 end_task (o, 1).
   d. Use charges (S, used). c = Charges (S).
   e. If combat = 0 and S +0x17A = selected: host op 166 item_panel (slot, 0, 1).
   f. If c > 0: done. text = 7.
4. Op 57 merc_text (slot, text, 1000); op 50 end_task (slot, 1).

*First aid start* (op 191 first_aid (slot, tile, byte task, byte b); M = record slot; b is not used); result 1 when
first aid starts, else 0:
1. If M +0x1B9 ≠ 0 and not (M +0x1B9 = 4 and task = 6): op 57 merc_text (slot, 8, 2000); result 0.
2. If task is not 5 or 6: result 0.
3. o = occupant(tile). If o ≥ 0x40: host op 125 message (2) ("No one there"); result 0. P = record o.
4. If P +0x185 = 0 and P +0x186 ≠ 1: op 57 merc_text (o, 9, 1000); result 0.
5. Clear target of M (sector_start.md). Op 31 spend_ap (slot, 5, 0). M +0x1B9 = task; u16 M +0xD6 = tile; op 52
   set_anim (slot, 0xE, 0, 0); P +0x1CA = P +0x1CA + 1 (8-bit).
6. If task = 5: M +0x181 = 0xB8; M +0x1A2 = op 6 direction (M's pixel point, P's pixel point) & 0xFF, the pixel
   points (s16 +0x116, s16 +0x118) of each record. If P +0x1B9 ≠ 6: P +0x1B9 = 4; P +0x181 = 0xB9; if s8 P +0x183
   ≥ 15 and cur(P) ≥ 4 and anim_table column 2 of cur(P) = 0 (a word of 256 and up reads 0): Clear target of P; op
   52 set_anim (o, 4, 0, 0); P +0x1A2 = op 6 direction (P's pixel point, M's pixel point) & 0xFF. Then (P +0x1B9 ≠ 6
   only) host op 135 portrait (o, 1).
7. If task = 6: M +0x181 = 0xB7; M +0x1A2 = 5.
8. Host op 135 portrait (slot, 1). Host op 130 tile_marker (0x93, tile). Result 1.

*Evidence (2026-10-25): mirrors tools/ai.py apply_first_aid (ApplyFirstAid? 0x39CA4), first_aid_tick (0x15260),
first_aid_start (0x3D750), item_charges 0x151BC, use_charges 0x151FC (ai_diff apply_aid, aid_tick, first_aid).*

**End of a task, exact steps** (library op 50 end_task (slot, byte flag); C export `ja1_end_task`). A task is a
merc's shared job: +0x1B9 = 4 (the patient, or a lone task), 5 (bandaging the soldier on tile u16 +0xD6), 6
(bandaging itself); +0x1CA counts the medics working on this soldier. flag = 1 ends the whole task (any value ≠ 0
counts as 1). S = record slot, occupant(t) as in First aid. Point stop (S) = u16 +0x126 / +0x128 = the pixel
point (x, y) of S's own tile t = s16 +0x114 (x = (t % 56) × 16 + 5, y = (t / 56) × 14 + 5, port_api.md op 6), then op 53
stop_anim (slot, 0).
1. If +0x1B9 ≠ 0: +0x1C1 = 1 (re-think).
2. If +0x1B9 is 5 or 6: o = occupant(s16 +0xD6). If o < 0x40 (P = record o): P +0x1CA = P +0x1CA − 1 (8-bit); if
   s8 P +0x1CA ≤ 0: P +0x1CA = 0, P +0x1B9 = 0, P +0x181 = 0, host op 135 portrait (P +0x17A, 1). Then
   +0x1B9 = 4 if s8 +0x1CA > 0, else 0. u16 +0xD6 = NONE (0x0C41); +0x1CA = 0; +0x181 = 0. If flag: Point stop
   (S) and op 52 set_anim (slot, 0x10, 0, 0). Host op 135 portrait (+0x17A, 1).
3. If +0x1B9 = 4 (also when step 2 just set it): for each record k = 0..7 in order with +0x182 ≠ 0, +0x138 ≠ 0,
   +0x183 ≠ 0, +0x1B9 = 5 and u16 +0xD6 = S's u16 +0x114: run these steps on k with flag 1. Then, if S +0x1B9 is
   still 4: +0x181 = 0, +0x1B9 = 0, +0x1CA = 0, host op 135 portrait (+0x17A, 1), and if flag: Point stop (S).
   Last, if +0x17A < 8: op 58 reserve_ap (slot).

*Evidence (2026-10-25): EndTask? 0x3D98C read in the asm; mirror tools/ai.py end_task (ai_diff end_task; --port).*

**After death** (MercDies; run at the end of the death animation, or at once for a quiet death): a soldier already
marked dead is left alone. Otherwise hp, max hp, wounds and AP become 0, fatigue 10000.
- A merc: its death portrait (an animation of 10 frames) is drawn with the overhead death sound and its own death
  remark; a loud death also refreshes the panel per frame, prints "<name> is dead!" and restores the mouse. When the
  sector's merc count is 0 and no other merc (slots 0–7) is still in play, the squad is lost: after the cleanup
  below (loud deaths only) the merc becomes the selected one and the squad-lost sequence starts.
- An enemy while the sector still counts enemies: the kill counter rises and the sector's enemy count drops. The last
  one wins the sector. Otherwise, when the killer is a merc that can see the body (a sight line from its point to the
  body's), it gets a kill remark and mercs and guards (slots 0–15) in play that are not busy go idle. The enemy's
  entry in the sector record is marked gone (0xFE) and the day replay records a kill (T).
- Every soldier then is dead (+0x188 = 1), off the board, its tile is vacated and its position cleared.

**Explosion effect step**: an explosion leaves an effect record; when its animation plays (once per record) the effect
hits its tile with its kind and power:
- A burning object (0x86) on the tile is cleared.
- Kind 3 on a tree (0x35 or 0x36) costs the sector one tree (and, for 0x35, one more counter); the day replay records
  it and the sector's workability is recomputed.
- Kinds 4 and 5 start a gas cloud: it lasts 3 turns (kind 4) or 5, one or two fewer unless the ground is class 3,
  with radius 3 or 5. With all five clouds in use nothing else happens.
- Kinds 2 and 3 with power above 0 show the explosion art; in the story sector, story tiles on the tile are hit.
  Other effects (or no power) run the tile's object reaction (0x3C05C).
- With power above 0 the blast follows with a radius by kind: 2 → 2, 3 → 4, 4 → 2, 5 → 4, 7 → 4, 9 → 4, else 3.

**Putting an item on the ground** (dropped, thrown, or left by the dead): an item of type 0 does nothing; a one-off
note that was placed before is not placed again; on water an item of a sinking type is lost. The item takes the first
free ground item entry (200 in all). Unless the exact tile is required, it lands on the nearest tile that takes items:
the tile itself, else the first such tile 1 to 11 steps out, trying the eight directions N, NE, … NW at each distance.
A tile takes items when it holds no ground item (or only an empty one marked free), is not water and its object lets
items lie on it (any object of class 5 or below or 9 or above; of classes 6–8 only objects 0x3B, 0x41, 0x5A, 0x60).
With an entry and a tile the item is stored there; a stacking item with more than one unit gets the first unit's
condition for every empty unit condition. When the item moved off a tile that already held a ground item, the two
tiles exchange their item references.

**Springing a ground item** (an item lands on, or a soldier steps on, a tile with a ground item): by the item type
- a set mine (0x11, condition below 0) clicks and explodes (kind 2);
- 0x13 clicks and sets off the 0x29 bombs (a timed effect when its count is above 0, else at once); 0x2C (the
  trigger of the 0x3F bombs in Santino's base) clicks and sets off the 0x3F bombs (a soldier in state 4 instead starts
  the home-sector stages); sprung by a merc, either becomes a used item (type 0x40, condition 1–100) and puts out a
  burning object on the tile;
- 0x72 clicks, sets off the mission bombs (0x6E in sector 11, else 0x71) and cancels the current mission; sprung by a
  merc it becomes a used item too;
- 0x44, 0x52, 0x70 run their own tile events (0x52 with a click);
- 0x29, 0x3F, 0x71 explode (kind 3) when armed;
- any other set explosive (condition below 0) explodes when armed with its blast kind (0x16–0x1B → 4–9, 0x1C/0x1D → 2,
  0x5D → 1, else 3), most with a click.
When something happened the tile's object reacts (or, for a reset item, the item is re-placed for the merc).

**Springing a ground item, exact steps** (library op 108 trigger_item (slot, tile, byte g, byte armed) → int; S =
the soldier record of the slot, E = ground entry g, the 12 bytes at ground_items + 12 g (g of 200 or more: past the
200 entries as in Mine spotted, "G for g of 200 or more"), armed = the low byte):
1. r = 1, reset = 0. T = E[0].
2. By T:
   - 0x11: if s8 E[2] < 0: op 339 click (slot); op 90 explode (S[0x17A], tile, 2, 1). Else r = 0.
   - 0x13 or 0x2C: op 339 click (slot). For 0x13: if s8 E[1] > 0: op 91 effect (0xC41, 0x29, E[1], 500, 0x41, 0),
     else op 150 ground_blast (0x29, 0, 500, 0x41, 0). For 0x2C: if s8 S[0xBA] = 4: op 169 stage_effects, else
     op 150 ground_blast (0x3F, s8 E[1] (as int32), 800, 0x41, 0). Then **spent** (below).
   - 0x72: op 339 click (slot); op 32 mission_blast (1000); mission_step = 0, mission_slot = 0x40, mission_target =
     0xC41. Then, if armed ≠ 0 and S[0x17A] < 8: **spend** E (no fire step).
   - 0x44: op 340 call_enemies (tile, 0x44); op 341 take_patch (tile, 0x44).
   - 0x52: op 339 click (slot); op 342 toggle_doors (0, E[2]); op 341 take_patch (tile, 0x52).
   - 0x70: op 342 toggle_doors (1, E[2]); op 341 take_patch (tile, 0x70).
   - 0x29, 0x3F, 0x71: if armed ≠ 0: op 339 click (slot); op 90 explode (S[0x17A], tile, 3, 1). Else r = 0.
   - Any other type: k = the blast kind of T (0x16..0x1B → T − 0x12, 0x1C / 0x1D → 2, 0x5D → 1, up to 0x15 → 0,
     else 3). If s8 E[2] < 0 and armed ≠ 0: op 339 click (slot) unless T is 0x14, 0x16, 0x18, 0x1A, 0x1C or 0x1D;
     op 90 explode (S[0x17A], tile, k, 1). Else r = 0.
   **Spent** (0x13, 0x2C): if armed ≠ 0 and S[0x17A] < 8: **spend** E, then if byte 1 of the tile is 0x86 it
   becomes 0 and every move code is rebuilt. **Spend** E: E[0] = 0x40; v = random(100); E[1] = 1; E[2] = v + 1;
   E[6] = 1; reset = 1.
3. If r = 1: reset = 1: op 343 pick_up (slot, tile, 1); else op 109 take_ground_item (tile, 0).
4. Result r.

**Mission blast** (library op 32 mission_blast (int power)): op 150 ground_blast (0x6E when cur_sector = 11, else
0x71; 0, power, 0x41, 0); then mission_done[cur_sector] = 1.

**Enemy call** (library op 340 call_enemies (tile, int kind)): the enemies' side call (calls +4) = the low 16 bits
of the tile, its level (calls +6) = 0xC. kind is not used.

**Taking a patch** (library op 341 take_patch (tile, byte type)): sector_start.md "Take ground patch".

**Toggling doors by value** (library op 342 toggle_doors (byte mode, byte value)): for each tile t = 0..3135 in order
whose object (byte 1) is 0x3B..0x64: mode 1: if Tile standable (t) < 0 (ai.md) and the door value of t = value:
op 107 break_object (t, object, 0); any other mode: if the door value of t = value and byte 5 of t = 0x40 exactly:
op 104 close_door (t, object, 1).

**Taking a ground item off a tile** (to the cursor, or away for good): nothing happens off the map (tiles 0..3136
only) or on a tile without a ground item. A tile marker 0x86 on the tile is cleared and the move codes are rebuilt;
the tile leaves the list of pending blast tiles. To the cursor: what the cursor holds first goes back, the 12-byte
item is copied to the cursor and its condition (byte 2) becomes 100 unless its type keeps it (item table byte 0xA)
or it is 0x6F, 0x57 or 0x58. Away for good: a type on the game's 32-entry list of special items stays where it is.
Then the tile has no ground item, and for entries below 200 the first 7 bytes of the entry are cleared (bytes 7..11
keep their old values). **Removing a patch**: the item is taken away, then the same happens on each of the four
neighbours (north −56, east +1, south +56, west −1, no map-edge or row checks) whose ground item has the same type,
so a connected patch of one type goes. On a special type the patch never ends (the original runs out of stack).

**A soldier picks up an item**: the item goes to the cursor; when asked, the cursor item then goes to the soldier.
For the four story items 0x79, 0x86, 0x5F, 0x6E (by the type picked up): if the cursor now holds something else, one
of the eight mercs present must carry the type (first match in the 12 pockets); then, once per game, the soldier
says the item's quote (0x12, 0x15, 0x1B, 0x1F). **Squad search**: over one merc (slot 0..7) or all eight (any
higher slot), present mercs only, the first pocket whose type matches gives that merc's slot (optionally the pocket
is dropped); with no match an optional second search may give 0x63; else none (−1).
**Item 0x44 sprung** (its own tile event): the enemies' latest call goes to that tile with level 12, and the patch of
0x44 items there is removed.

**Doors and windows.** Each sector has 12 (tile, value) pairs; the **door value** of a tile is the value of the first
pair with that tile, else 0 (the same pairs give door ids and the strength of breakable objects). Opening or
breaking an object changes its object number and the object of one neighbour, and sets the tile's move code to
0x6338 (a sound plays when asked):

| Object | Tile | Neighbour |
|---|---|---|
| 0x3E, 0x5D | −3 | west (tile − 1): −2 |
| 0x3D, 0x5C | −2 | west: −2 |
| 0x45, 0x64 | −4 | north (tile − 56): −2, its byte 2 = 0x45 if it is now 0x42, else 0x4E |
| 0x43, 0x62 | −2 | north: −2, byte 2 as above |

Closing does nothing while a soldier stands on the tile; doors 0x3B / 0x5A gain 3 (west neighbour +2), doors 0x41 /
0x60 gain 2 (north neighbour +2, its byte 2 = 0); the move code becomes 0x5DC. **Doors by id** (an item event):
over every tile of the map whose object is 0x3B..0x64 and whose door value equals the id, the open
mode opens those a soldier cannot stand on (silently); the other mode closes those with no occupant (with sound).

**Reserved AP** (soldier +0xEF): explosives of blast kind 3, 5, 7 or
9 reserve 6 AP. Other items of blast kind 0 reserve nothing unless their type is 1..0xE. Otherwise the reserve is
the AP cost of using the item (no target tile); when the soldier has more AP left than that and its flag +0x16A is
set, 4 more, but never more than the AP left (signed byte comparisons).

**Gun jams** (an animation step of firing, mercs only, not while the soldier's flag +0x1C9 is set): the hand item's
byte 1 above 0 means loaded, below 0 jammed. A loaded item jams unless a roll 0..99 is below its condition (signed)
plus a roll 0..49: byte 1 is negated, the merc says the jam line, the panel refreshes, the attack is dropped and
message 0x10 shows. A jammed item clears when a roll 0..99 is below 20 plus half the merc's mechanical skill (when
above 0): byte 1 is negated back and the merc practises stat 7; otherwise the attack is dropped.

**Stage effects**: a fixed table of 12 effects (tile, effect, stage, a parameter, lowest difficulty) belongs to a
staged event in sector 0. Starting a stage starts every effect of that stage whose lowest difficulty is at most the
game's difficulty. Stage 0 also shows the stat-loss style message for merc 0 (amount code 0xFE) for 0x1D4C ticks.
When the stage counter reaches 5, the sector's enemy count drops by one and the remaining enemies 0x29..0x2F (not
0x28) that are present, have 15 or more health, a Santino-speech byte (+0x9C) of 0 or less and the forced-alert flag
(+0x1C4) clear leave the board (see "Leaving the board" under Death) and die normally.

**Story objects**: each sector has 5 story slots (an object code and a tile). Hitting a story object marks the hit,
turns slot code 0xAE into 0xAC and 0xAF into 0xAD, puts the 0xAC / 0xAD object on the slot's tile, sets the mission
step from the radio result and clears the mission order.

**Starting an attack** (stage 1 only): the soldier faces the target tile and raises its weapon. A loaded hand item
moves the attack to stage 2 and stores the target tile. An empty item of type up to 0xE asks for a reload; when the
reload message answers, the shot ends and the attack is dropped. Otherwise the item's blast kind is noted. A merc
that aims at another merc gets the "friendly target" message 8 once per pair (not while +0x19C is 3); both mercs
then remember the pair, and the attack target is cleared.

**Burying a mine** (the animation step after the bury action, current animation not 0xC): when the target tile is
not the soldier's own tile, some objects on it push the item one more tile in the same direction (table 0x95694,
the ObjBlock rows from 0x37, value 0xB: objects 0x7F / 0x81 at directions 1 and 5, 0x80 / 0x82 at 3 and 7).
The original does not bound the object: objects below 0x37 or above 0x82 read bytes outside the table (object 0xC9
at direction 4 also hits 0xB). A port must keep those bytes or decide the case.
A mine (type 0x11) of condition 10 or more is armed (byte 1 = 1), put down with byte 6 = -1, adds 1 to the sector's
mine count (+0xAF) and shows "Mine buried"; a weaker mine is only put down. Any other item is set off in place:
condition 0xFF, its blast kind noted, effect 0x157BC at the tile with strength combat x (roll 0..1 + 1) as a byte
and parameter 0x2134 - 6000 x combat, and the item is put down. Both practise stat 6 (5 rolls), not the weak mine.
The hand slot is then emptied and the panel of the selected merc refreshes.

**Picking up at the own tile** (animation step 0x1E8): the soldier takes the ground item of its own tile, but not
types 0x44, 0x52, 0x70, 0x29, 0x3F or 0x71 (then the "nothing here" quote event 0xFC). A merc on an armed buried
mine (type 0x11, byte 6 = -1, condition above 0) notes the tile in the 30-entry blast list and the mine's condition
is negated (made safe). An active timed effect on the tile (one of 5 records filled by the effect starter) lets the
soldier note the tile when a roll 0..7 is below level + explosives / 3, and negates the tile item's condition either
way. Then the attack is dropped. With an item, the disarm prompt decides: yes picks the item up, frees the tile's
effect and, with an effect, lowers the sector's mine count; no (with an effect) negates the condition back. The
soldier's pending item action (+0x1A6, +0x134, +0x1A7, +0x95, +0x96) is cleared.

**Using the hand item on a tile** (animation step 0x1E9 with the "use" animation 0xC): off the map nothing happens.
A merc standing on the tile gets message 0xE. The ground item counts only when the soldier may use the tile from
its own side; an item already handled (byte 6 above 0) skips the object checks. Object 0x8E is a container that
needs no crowbar and is silent except in tileset 7; in sector 0xB it opens only with item 0x87 (once: voice, sound,
a 1-second delay, practice of stat 4, and the mission step and order reset). Objects 0x9A, 0x9C, 0x9E, 0xA0, 0xA2,
0xA4 and 0xA6, 0xA8, 0xAA are locked when the tileset's flag says so. A locked object with an item needs the disarm
prompt, then the crowbar (not for 0x8E); success raises the object byte by 1 (the opened form) with a sound and
noise 3; without an item the same gives quote event 0xFB. Any other object on an unstandable tile with flag 0x76 is
broken open by the crowbar (quote event 0xFA, else 0xF9). Items of type 0x44, 0x52, 0x70, and any non-merc user,
trigger the item. A merc otherwise checks for an armed mine, then the disarm prompt picks the item up (answer -1
marks it), or a new item is marked handled with quote event 0xFD. With no item and nothing locked, flag 0 or 0xFE+
gives quote event 0xFC. The merc practises stat 4 while the disarm path is open; the pending action is cleared.

**Disarming an explosive** (the disarm prompt; returns handled, cancelled or set off): only an armed explosive
counts — condition below −100 when called as armed, below 0 otherwise; anything else is "handled". A non-merc user
always sets it off. For a merc, an explosive whose tile is in the blast list is known; otherwise the merc spots it
when explosives / 30 + level reaches its strength (the condition's size, less 100 above 100): the tile joins the
blast list, the merc practises stats 4 and 6 by the strength and says message 0x13. A blue flag (object 0x86) on the
tile also counts as spotted, but without the prompt. A known or newly spotted explosive asks "Attempt to disarm
explosive?"; no clears the merc's pending action and, for a known one on a blue flag, asks "Remove blue flag?"
(yes removes it) — "cancelled". Called from the menu, the menu refreshes after the message and after a yes. A merc
that goes on disarms it when a 0..99 roll is below explosives × 70 / 100 + dexterity × 20 / 100 + level −
(100 − wisdom) / 5 (practice of stat 6 by 10); otherwise it goes off (practice by 5, "set off"). An unspotted one
also goes off. A disarmed item by type: a mine (0x11) gets condition 1..9 and is marked handled; types 0x14, 0x16,
0x18, 0x1A, 0x1C, 0x1D have their condition negated; 0x13, 0x2C, 0x72 become item 0x40 and 0x29, 0x3F, 0x71 item
0x21 (condition 1..10, count 1, handled, blue flag removed); any other gets condition 70..100. A stack of more than
one item whose type is stackable (table byte 6 above 1) gives each further unit the item's condition − 5..+4, kept
in 1..100.

**Mine spotted, exact steps** (library op 247 mine_spotted (slot, tile, byte g) → byte; S = the soldier, G = ground
entry g). G for g of 200 or more: entries g of 200 or more lie past the 200 entries: entry g, byte o is byte 12 g + o − 2400 after the table: bytes 0..127 = the text buffer (`text_buf`, 128 bytes: a status message runs past byte 31; 33 of 136 dumps hold text there), from 128 on the profiles (`profiles`, byte 128 + 0x5A p + f = profile p byte f). Entry 0xFF ("none") thus reads profile 5 bytes 0x52..0x59 (kit places 2..9: item types below 136) and profile 6 bytes 0..3 (the nickname); its byte 6 is a kit item type, never 0xFF, so step 1 returns 0 for g = 0xFF.
1. If G[0] ≠ 0x11, or s8 G[6] ≠ −1, or S[0x17A] ≥ 8, or s8 G[2] ≤ 0: return 0.
2. If none of the 30 words of `blast_list` (signed) equals tile: Blast list add (tile).
3. G[2] = −G[2] (8-bit); return 1.

**Disarming an explosive, exact steps** (library op 248 disarm (slot, tile, byte g, byte armed, byte menu) → int;
S = the soldier, G = ground entry g, c = s8 G[2]; merc = S[0x17A] < 8; "ask (kind)" = host op 345 disarm_ask
(slot, kind), yes when the low byte of the answer ≠ 0; kind 0 = "Attempt to disarm explosive?", kind 1 = "Remove
blue flag?"; "refresh" = when menu ≠ 0: host op 347 menu_redraw; "cancel" = u16 S[0x134] = 0xC41, u16 S[0x96] = 0xC41,
S[0x1A6] = S[0x1A7] = S[0x95] = 0; "set off" = op 108 trigger_item (slot, tile, g, 1), return −1. Signed divisions
truncate. (SPEC GAP (not seen: checked 2026-10-28, both callers, "Using the hand item on a tile" (steps 8 and 9) and "Picking up at the own tile" (step 6), call op 248 only when g < 0xFF, and g comes from tile byte 6, which holds an entry number below 200 or 0xFF)): g of 200 or more as in Mine spotted.)
1. Unless (c < −100 and armed ≠ 0) or (c < 0 and armed = 0): return 1.
2. known = (not merc) or one of the 30 words of `blast_list` (signed) equals tile. spotted = known.
3. If known and merc: if not ask (0): cancel; if tile byte 1 = 0x86 and ask (1): tile byte 1 = 0 and all move codes
   are rebuilt; return 0. Else refresh.
4. If not known: v = |c|; if v > 100: v = v − 100. If tile byte 1 = 0x86: spotted = 1. If s8 S[0x18F] / 30 + s8
   S[0x192] ≥ v: Blast list add (tile); spotted = 1; op 105 stat_practice (slot, 4, v, 0); op 105 (slot, 6, v, 0);
   op 54 merc_message (slot, 0x13, 0x40, 0, s8 menu); refresh; if not ask (0): cancel, return 0; refresh.
5. If not spotted or not merc: set off.
6. chance = s8 S[0x18F] × 70 / 100 + s8 S[0x18C] × 20 / 100 + s8 S[0x192] − (100 − s8 S[0x18D]) / 5. If random(100)
   (signed) ≥ chance: op 105 (slot, 6, 5, 0); set off.
7. op 105 (slot, 6, 10, 0). By the type t = G[0]:
   - 0x11: r = random(9); G[6] = 1; G[2] = r + 1; host op 346 ground_item_reset (g).
   - 0x14, 0x16, 0x18, 0x1A, 0x1C, 0x1D: G[2] = −G[2].
   - 0x13, 0x2C, 0x72 (G[0] = 0x40) and 0x29, 0x3F, 0x71 (G[0] = 0x21): G[2] = random(10) + 1; G[1] = 1; G[6] = 1;
     op 346 (g); then if tile byte 1 = 0x86: tile byte 1 = 0 and all move codes are rebuilt.
   - else: G[2] = random(0x1F) + 0x46.
8. If s8 G[1] > 1 and byte 6 of item type G[0] > 1: for j = 0 .. s8 G[1] − 2 (the bound read again each round):
   x = G[2] + random(10) − 5 (8-bit); G[8 + j] = x when s8 x > 0, else 1; if s8 G[8 + j] > 100: G[8 + j] = 100.
   Byte 8 + j past 11 is a byte of the next ground entry; past the 200 entries the writes land as in Mine spotted
   ("G for g of 200 or more"; entry 199 with a big stack reaches the text buffer).
9. Return 1.

**The crowbar** (forcing a lock or a door; returns "opened"): the lock strength is twice the door value of the tile
when nobody may stand on it, else the tile's lock value (12 tile / value pairs from +0x30 of the sector record, the
last two of which overlap the tile triggers). Strength 0 opens at once for 50 fatigue. Else the force is
strength (+0x18A) × ((hp + half of (max hp − hp − wounds) when hurt) / 2) / 100, all signed and truncated. With
flag 0x76 and a real crowbar (item 0x76 with condition 10 or more) in hand the force gains half again; the bar
breaks when a 0..99 roll is below (min(force, lock) as a byte − its condition): its condition becomes 1..9, the
merc says "BAD", a sound plays, "Crowbar breaks!" shows and the force drops to half the unaided force. A bar that
holds wears by a roll below min(force, lock) / 10 when that is 20 or more. The selected merc's item panel
refreshes. The attempt costs fatigue 50 × (force when below the lock, else lock² / force with an unbroken bar,
else the lock), taken as a byte. When the force reaches the lock the merc practises strength (stat 1) by lock / 10
and the lock opens. A negative lock with zero force divides by zero in the original.

**Using the hand item on a tile, exact steps** (library op 74 use_on_tile (slot, tile, byte flag) → none; S = the
soldier record of the slot, me = S[0x17A], flag = the low byte; animation step 0x1E9 calls it with S +0x134 and
S +0x1A6). Ground entry g = the 12 bytes at ground_items + 12 g; "type of g" = byte 0 of entry g.
1. If tile (signed) > 0xC40: done (nothing at all). (SPEC GAP (not seen: checked 2026-10-28, the tile is S +0x134, which only the task clear (NONE = 0xC41) and click.md "Tactical click" write; that click gets the tile under the mouse, a map tile 0..0xC3F, or 3137 = 0xC41 when no tile is under the mouse (panel.md)): a negative tile, and tile 0xC40 (one past the
   map, which the original reads from the bytes after the map), read their tile bytes as 0)
2. obj = tile byte 1. g = tile byte 6, or 0xFF when it is 200 or more. occ = the occupant of the tile (ai.md). st =
   Tile standable (tile) (ai.md). practise = locked = breaks = nocrow = handled = 0; noisy = 1; spotted = 0.
3. If not (st < 0 or obj < 0x37 or obj > 0x82 or s16 S[0x114] > tile): g = 0xFF. (The soldier may not use the tile
   from its north or west side when it is standable and holds an object 0x37..0x82.) If g < 0xFF: handled = (s8
   byte 6 of g > 0).
4. **Finish** (used below): if practise ≠ 0: op 105 stat_practice (slot, 4, 1, 0). Then S[0x1A6] = 0, u16 S[0x134]
   = 0xC41, S[0x1A7] = 0, S[0x95] = 0, u16 S[0x96] = 0xC41.
5. If occ < 0x40: if occ < 8 and occ ≠ me: op 54 merc_message (occ, 0xE, 0x40, 0, 1). Finish; done.
6. ts = s8 tileset. If handled = 0, by obj:
   - 0x8E: locked = nocrow = 1; noisy = (ts = 7). If cur_sector = 0xB: if S[0] ≠ 0x87: if safe_opened = 0: op 54
     merc_message (slot, 0x17, 0x40, 0, 1); Finish; done. If S[0] = 0x87: op 99 focus (me, 0); op 100 cry (S[0x179],
     −1, 1, 1, 1); op 101 voice (0x2B11, 0x32, 1); op 37 delay (1000); safe_opened = 1; op 105 stat_practice (slot,
     4, 0x32, 0); mission_step = 0; mission_slot = 0x40.
   - 0x9A, 0x9C, 0x9E, 0xA0, 0xA2, 0xA4: locked = (LOCK_A[ts] ≠ 0); 0xA6, 0xA8, 0xAA: locked = (LOCK_B[ts] ≠ 0).
     LOCK_A = 1 1 1 1 0 1 1 0 1 1, LOCK_B = 1 1 1 1 0 0 0 0 1 1 (tilesets 0..9; JA.EXE 0x95681 and 0x9568B).
     (SPEC GAP (not seen: the last byte of each of the 60 TERRAIN.DAT records is 0..9): a tileset outside 0..9 is not locked.)
   - any other object: if st < 0 and flag = 0x76: breaks = 1.
7. **Sound** (used below): if noisy ≠ 0: op 60 sfx (0x22, 0x2B11, 0x32, 1); op 127 make_noise (me, tile, S[0x1B3], 3,
   2).
8. If locked ≠ 0:
   - g < 0xFF: if s32 op 248 disarm (slot, tile, g, 1, 1) ≤ 0: g = 0xFF. Else, if nocrow = 0 and Crowbar (tile,
     flag) ≠ 1: op 54 merc_message (slot, 0x17, 0x40, 0, 1); g = 0xFF. Else: tile byte 1 = tile byte 1 + 1 (8-bit);
     Sound.
   - g = 0xFF: if nocrow = 0 and Crowbar (tile, flag) ≠ 1: op 54 merc_message (slot, 0x17, 0x40, 0, 1). Else: Sound;
     tile byte 1 = tile byte 1 + 1 (8-bit); op 103 quote_event (me, 0xFB, 6, 0xC41, 0).
   Else, if g < 0xFF and the type of g is 0x44, 0x52 or 0x70: op 108 trigger_item (slot, tile, g, 1).
   Else, if me ≥ 8: op 108 trigger_item (slot, tile, g, 1); Finish; done. (g may be 0xFF here.)
   Else: spotted = low byte of op 247 mine_spotted (slot, tile, g); practise = 1. If breaks ≠ 0: if Crowbar (tile,
   flag) = 1: op 107 break_object (tile, obj, 1); op 103 quote_event (me, 0xFA, 2, 0xC41, 1); S[0x9D] = 1. Else: op
   103 quote_event (me, 0xF9, 2, 0xC41, 1).
   With g = 0xFF the type test reads entry 0xFF as in Mine spotted ("G for g of 200 or more"): profile 5's kit place 2. When that place holds a Strobe (0x44) or a Plastic Bag (0x52) (the merc of profile 5 left the team with it there), a use with no ground item on the tile gives op 108 trigger_item (slot, tile, 0xFF, 1) instead of the mine check.
9. If g < 0xFF:
   - practise ≠ 0: r = s32 op 248 disarm (slot, tile, g, 0, 1). r > 0: op 343 pick_up (slot, tile, 1); Free effect
     (tile); if spotted ≠ 0: Mines down. Else: if r = −1: byte 6 of g = 0xFF; practise = 0. Then if spotted ≠ 0: j =
     tile byte 6 (0xFF when 200 or more); if j < 0xFF: byte 2 of j = −byte 2 of j (8-bit); else Mines down and Free
     effect (tile).
   - practise = 0, the type of g is not 0x44, 0x52 or 0x70, and me < 8: op 105 stat_practice (slot, 4, 1, 0); byte 6
     of g = 1; op 103 quote_event (me, 0xFD, 4, 0xC41, 1).
   g = 0xFF: if locked = 0 and (flag = 0 or flag ≥ 0xFE): op 103 quote_event (me, 0xFC, 4, 0xC41, 0).
10. Finish.

**Free effect** (tile): the first of the 5 `timed_fx` entries f (turns.md) whose u16 +6 equals the low 16 bits of the
tile: i32 +0 = −1, u16 +6 = 0xC41, byte +0xC = 0, fuse_beep = 0; then stop. **Mines down**: byte 0xAF of the
current sector's `sectors` row − 1 (8-bit).

**Crowbar, exact steps** (tile, flag) → 0 or 1 (S = the user, flag = the low byte):
1. v = 2 × door value (tile) (Doors, below) if Tile standable (tile) < 0; else the lock value: the s16 value of the
   first of the 12 pairs (s16 tile, s16 value) at bytes 0x30 + 4 k of the current sector's `sectors` row whose tile
   equals the tile; none 0.
2. If v = 0: op 31 spend_ap (slot, 0, 0x32); result 1.
3. hp = s8 S[0x183]. missing = s8 S[0x184] − hp − s8 S[0x185] if hp < s8 S[0x184], else 0. f = s8 S[0x18A] × ((hp +
   missing / 2) / 2) / 100 (all signed, C division).
4. bar = (flag = 0x76 and S[0] = 0x76 and s8 S[2] ≥ 10).
5. If bar: bonus = f × 50 / 100; f = f + bonus; m = the low byte of (f if f < v, else v); risk = s8 (m − S[2])
   (8-bit), 0 when negative. If risk ≠ 0 and random(100) < risk: bar = 0; f = (f − bonus) / 2; S[2] = random(9) +
   1; op 99 focus (me, 1); op 100 cry (S[0x179], −1, 0, 1, 1); op 101 voice (0x2B11, 0x32, 1); op 125 message (13,
   "Crowbar breaks!"). Else, if m ≥ 0x14: S[2] = S[2] − random(m / 10) (8-bit). Then (bar still in hand or not), if
   me = selected: op 166 item_panel (slot, 0, 1).
6. a = f if f < v; else v × v / f (v × v wraps at 32 bits) if bar ≠ 0; else v. op 31 spend_ap (slot, 0, s16 ((low
   byte of a) × 0x32)). (SPEC GAP (not seen: f = 0 there needs v < 0; JAGGED.DAT door and lock values are 0..199, the 12-pair lock scan also reads triggers 0 and 1, and the only negative trigger values there are sector 39 trigger 0 (tile 1149, no object in TERRAIN.DAT, in the 10 MISC patch files or after a crater) and sector 59 trigger 0 (tile 3136, off the map), while the scan runs only on a tile with a locked door object): f = 0 there divides by zero in the original; the port uses a = 0.)
7. If v ≤ f: op 105 stat_practice (slot, 1, s16 (v / 10), 0); result 1. Else result 0.

**Using the lock kit on the mission safe, exact steps** (library op 69 use_special (slot) → none; S = the soldier
record of the slot, me = S[0x17A]; the task step 0x1C2 calls it when the hand item is 0x4C and the quest check of
the task tile is non-zero, ai.md):
1. If S[0] ≠ 0x4C: done.
2. op 99 focus (me, 0).
3. skill = the low byte of (s8 S[0x18C] × s8 S[0x190] × s8 S[0x18D]) / 10000 (dexterity × mechanical × wisdom;
   signed, truncated), plus the low byte of S[0x192] × 5 (level), as a byte. Then skill = the low byte of
   (skill × s8 S[2]) / 100 (the kit's condition; signed, truncated). need = 80 / 90 / 100 by s8 difficulty
   (0 / 1 / 2; JA.EXE 0x95CB4, 16-bit words).
4. If skill (0..255) ≥ need (the safe opens): op 100 cry (S[0x179], −1, 1 = "GOOD", 1, 1); op 101 voice (0x2B11,
   0x32, 1); op 37 delay (1000); safe_opened = 1; op 105 stat_practice (slot, 7, 100, 0), (slot, 4, 100, 0),
   (slot, 3, 100, 0); op 74 use_on_tile (slot, 0x1C9, 0x4C) (the safe's tile); mission_step = 0; mission_slot =
   0x40.
5. Else: op 100 cry (S[0x179], −1, 0 = "BAD", 1, 1); op 101 voice (0x2B11, 0x32, 1); if s8 S[2] > 5: S[2] =
   S[2] − (Random(5) + 1) (a byte; the kit wears).

**A detected mine gets a blue flag, exact steps** (library op 118 mine_flag (slot, tile) → none; S = the soldier
record of the slot, me = S[0x17A]; called when a merc's mine detector finds a hidden mine in its view ("A merc's
view"), when a walking soldier's next tile holds a mine it knows of, and for the deferred tile S[0xEC]
(soldier_update.md)):
1. The tile's object byte (tile byte 1) = 0x86 (the blue flag); the tile's move code is built again from the tile
   ("Movement", the per-tile rule, previous code = the tile's code), so the flag blocks the way.
2. The path to the soldier's goal (s16 S[0x124]) is searched again and stored (op 41 find_path (slot, goal, 1)).
3. If no path is found and me < 8 (a merc): the task is cleared (S[0x1A6] = 0, S[0x134] = NONE, S[0x1A7] = 0,
   S[0x95] = 0, S[0x96] = NONE); the goal point (S[0x126], S[0x128]) = the point of the soldier's own tile
   (s16 S[0x114]); op 53 stop_anim (slot, 0); op 84 anim_step (slot); op 72 stop_walk (slot, 0).
4. Always: S[0x1A2] = the first byte of the soldier's path (paths[0x50 × me]) + 1 (a byte).

**Burying a mine, exact steps** (library op 75 bury_mine (slot, tile) → none; S = the soldier, H = its hand entry =
the 12 bytes S[0x00..0x0B], me = S[0x17A]; animation step 0x1E9 when the animation is not "use" 0xC):
1. If tile ≠ s16 S[0x114]: d = op 6 direction from S's tile to tile (as in the click: the pixel points of the two
   tiles), low byte. o = byte 1 of tile. If o is 0x37..0x82 and s8 object_block[(o − 0x37) × 9 + d] = 0xB: tile =
   the neighbour of tile in step d (the step table; tile itself when the step leaves the map). Any other o reads
   outside the object table in the original (other data of the program): the only 0xB there is at o = 0xC9, d = 4,
   so that pair also takes the step; no other o takes it. (The bytes it reads are constants, two start-option bytes
   that hold 0 or 1, and the day replay's carry byte, which is 0 outside the replay: each carry record is followed
   at once by the march record that uses it. Checked against the program image and 135 memory dumps, 2026-10-28.)
2. If H[0] = 0x11 (mine):
   - s8 H[2] < 10: Put item (sector_start.md, tile, H, b6 1, exact 1, note 0, near 1).
   - else: H[1] = 1 (armed); Put item (tile, H, b6 0xFF, exact 1, note 0, near 1); sectors[cur_sector × 0xB1 +
     0xAF] = that byte + 1 (8-bit; nothing when cur_sector ≥ 60); op 125 message (14, "Mine buried"); op 105
     stat_practice (slot, 6, 5, 0).
3. Any other H[0] (an explosive set off in place): H[2] = 0xFF; S[0x1C9] = the blast kind of H[0] (shooting.md);
   c = s8 combat; op 91 effect (s16 tile, S[0x1C9], the low byte of combat (unsigned) × (random(2) + 1),
   0x2134 − 6000 × c, owner 0x42, flag 1); Put item (tile, H, b6 1, exact 1, note 0, near 1); op 105
   stat_practice (slot, 6, 5, 0).
4. op 65 effect_slot (me, 0); H = 12 zero bytes (the hand is empty); if selected = me: op 166 item_panel (slot, 0,
   1).

**Picking up at the own tile, exact steps** (library op 73 pickup_here (slot) → none; S = the soldier, me =
S[0x17A], t = s16 S[0x114]; animation step 0x1E8). "Ground index of x" = tile byte 6 of x, 0xFF when it is 200 or
more; "type of g" = byte 0 of ground entry g.
1. g = ground index of t. If the type of g is 0x44, 0x52 or 0x70: g = 0xFF. If g < 0xFF and the type of g is 0x29,
   0x3F or 0x71: g = 0xFF.
2. If g = 0xFF: op 103 quote_event (me, 0xFC, 1, 0xC41, 0).
3. op 247 mine_spotted (slot, t, g) (the result is not used).
4. **Effect here**: c = 0. If one of the 5 `timed_fx` records f (in order) has u16 +6 = the low 16 bits of t and
   i32 +0 ≥ 0: c = 1; if random(8) < s8 S[0x192] + s8 S[0x18F] / 3 (C division) and none of the 30 words of
   `blast_list` (all of them, signed) equals t: Blast list add (t); then byte 2
   of ground entry (tile byte 6 of t, not checked) = −that byte (8-bit). With tile byte 6 = 200 or more the write lands past
   the 200 entries as in Mine spotted ("G for g of 200 or more"; bytes 32..127 there are not kept); for 0xFF (no
   ground item) it negates profile 5 byte 0x54 (kit place 4).
5. Drop the attack: S[0x1D8] = 0x40, S[0x1C2] = 0x40, S[0x1BE] = 0, u16 S[0xD0] = 0xC41, u16 S[0xDC] = 0xC41.
6. If g < 0xFF: r = op 248 disarm (slot, t, g, 0, 1) (signed).
   - r > 0: op 343 pick_up (slot, t, 1); Free effect (t); if c ≠ 0: Mines down.
   - else if c ≠ 0: j = ground index of t; if j < 0xFF: byte 2 of j = −byte 2 of j (8-bit); else Mines down and
     Free effect (t).
7. S[0x1A6] = 0, u16 S[0x134] = 0xC41, S[0x1A7] = 0, S[0x95] = 0, u16 S[0x96] = 0xC41.

**Picking up a ground item, exact steps** (library op 343 pick_up (slot, tile, byte cursor) → none; S = the
soldier, me = S[0x17A]; also `ja1_pick_up`). The four story items and their quotes and once-per-game flags:
0x79 → quote 0x12, `story` +0x1A; 0x86 → 0x15, +0x1B; 0x5F → 0x1B, +0x1D; 0x6E → 0x1F, +0x1C.
1. sector_start.md "Take ground item" (tile, keep 1).
2. typ = `held` byte 0 (after step 1, also when nothing was taken).
3. If cursor ≠ 0: host op 344 cursor_to_soldier (slot) (port_api.md: the cursor item goes to the soldier through
   the inventory screen).
4. If typ is not one of the four story items: done.
5. If `held` byte 0 ≠ typ (the cursor now holds something else) and Squad search (typ, no drop, all eight mercs, no
   second search) gives none (−1): done.
6. If the story flag of typ ≠ 0: done. Else op 103 quote_event (me, the quote of typ, 0, 0xC41, 1); the flag = 1.

**Blast list add** (t): scan the 30 words of `blast_list` in order: a word (signed) equal to t: done (nothing); stop
at the first word 0xC41 and write t there (16 bits). No 0xC41 word and no t: nothing.

**The listening device at a door, exact steps** (library op 67 walter (slot, tile) → none; S = the soldier, me =
S[0x17A]; the animation 0x2E (the click on a door with item 0x45 in the hand, click.md) runs its script step 0x1BF
with tile = s16 S[0x134]). The device (item 0x45) plays a sound clip of what it hears behind the door, with a
caption. Fields: `walter_said` (12 bytes W[0..11]: the sector where each overheard story line was said, 99 = not
yet), `walter_next` (8 bytes C[0..7]: the next caption numbers of the plain sounds), `walter_sector` (the sector
of the last story line; 99 at program start, not saved), `story` (S'[k] = story byte k), cur_sector (unsigned),
alarm, day (signed), the crate drops (`crates`, day of drop k = s8 byte 4k + 1). All W, C and S' bytes are
compared as signed bytes. q is the caption number (−1 = none yet).
1. If S[0] ≠ 0x45: done.
2. If s8 S[2] ≤ 10 (the device is worn out): q = 0 (static); go to step 8.
3. If Jam check (click.md "Jam check": the hand item can fail) gives 0: done.
4. c = t1_class[object byte of tile]. If c < 7: done.
5. If c = 9: g = tile byte 6 (none when 200 or more). If g is not none and s8 byte 2 of ground entry g < 0 (an armed
   trap): q = 1 (ticking); else q = 2 (silence). Go to step 8.
6. Else (a door, c = 7, 8 or above 9): count the enemies behind it: for each slot 40..47 with S+0x182 ≠ 0 (in the
   sector), s8 hp (+0x183) ≥ 15, t0_class[S+0x1B2] = 3 and s8 S+0x9C ≤ 0: d = op 7 tile_dist (s16 S+0x114, tile);
   d < 3: near + 1; 3 ≤ d < 6: far + 1. (near and far as bytes: near = count mod 100, far = count / 100.)
7. Pick q:
   - near ≥ 2: the **story lines**, in this order, each only while q = −1:
     a. Story (0, 7, last). b. Story (7, 8, last). c. Story (1, 9, last). d. Story (2, 9, last).
     e. If walter_sector ≠ cur_sector and S'[0xB] = 1: if W[0] = 99: q = 0x6E, W[0] = cur_sector. Else if W[1] =
        99: if cur_sector ≠ W[0]: q = 0x6F, W[1] = cur_sector. Else if W[2] = 99 and cur_sector ≠ W[1]: q = 0x70,
        W[2] = cur_sector. If q ≠ −1 now: walter_sector = cur_sector.
     f. If S'[0xA] = 1: Story (5, 3, last), then Story (5, 4, 3).
     g. Crate talk: if walter_sector ≠ cur_sector: for k = 0, 1, 2: d = the drop day of crate k; if d > day and d − 5
        ≤ day: if W[0xA] = 99: q = 0x78, W[0xA] = cur_sector, walter_sector = cur_sector; else if W[0xA] ≠
        cur_sector and W[0xB] = 99: q = 0x79, W[0xB] = cur_sector, walter_sector = cur_sector; stop the loop at
        this k in every case.
     h. If S'[0xE] = 1 and day > 10: Story (9, 5, last), then Story (9, 6, 5).
     i. Still −1: if alarm ≠ 0: q = C[2]; C[2] + 1; if C[2] > 0x3B: C[2] = 0x28. Else q = C[3]; C[3] + 1; if C[3] >
        0x27: C[3] = 0x14.
   - else near + far ≥ 2: if alarm ≠ 0: q = C[0]; C[0] + 1; if C[0] > 0x6D: C[0] = 0x5A. Else q = C[1]; C[1] + 1;
     if C[1] > 0x59: C[1] = 0x46.
   - else near = 1: if alarm ≠ 0: q = C[6]; C[6] + 1; if C[6] > 0x13: C[6] = 0xA. Else q = C[7]; C[7] + 1; if C[7]
     > 0x40: C[7] = 0x3C.
   - else far = 1: if alarm ≠ 0: q = C[0]; C[0] + 1; if C[0] > 0x45: C[0] = 0x41. Else q = C[1]; C[1] + 1; if C[1] >
     0x40: C[1] = 0x3C.
   - else: q = 2 (silence).
   (q = the counter's value before the + 1, as a signed byte; the + 1 is 8-bit.)
8. Host op 348 walter_say (q): the clip and its caption.
9. If q = 1: Blast list add (tile); op 54 merc_message (slot, 0x13, 0x40, 0, 1); op 105 stat_practice (slot, 4,
   10, 0). If q ≥ 0x6E: op 105 stat_practice (slot, 4, 5, 0).

**Story** (a, b, last): if q ≠ −1 or walter_sector = cur_sector or S'[5 + a] ≠ 1 or W[b] ≠ 99: nothing. If last =
"last" (no partner): q = b + 0x6E, W[b] = cur_sector, walter_sector = cur_sector. Else (last = a W index p): if
W[p] = 99 or cur_sector = W[p]: nothing; else q = b + 0x6E, W[b] = cur_sector, walter_sector = cur_sector. (The
story line b is said only once, in a sector other than the one of its partner line p.)

**walter_say** (host op 348, q): the voice clip `wp<q>.8` from the CD archive `WPRO\WPRO1` (q < 0x3C) or
`WPRO\WPRO2` (q ≥ 0x3C) (speech.md: raw 8-bit PCM, played at rate 0x2B11 = 11025 Hz, volume 0x7F); if the Subtitles option
is on: caption = record q of `EDT\WALQUOTE.EDT` (200 bytes at 200 × q) in a message box (panel.md "Tactical
messages"; it stays (1 − Sound FX option) × 2000 ms, so 0 with sound: until the clip ends). The game waits until the
clip ends or a mouse button is pressed, then draws the tactical screen again (op 347's refresh).

Evidence: PlayWalterQuote? 0x3C574 (enemy count 0x3C3D4, Story 0x3C454, crate talk 0x3C4E0; W = 0x22A0B1, C =
0x22A0C3, walter_sector = 0x92E2D, alarm 0x22A0CE, sounds via 0x5C0EC / ArcSeek / SpeechStream, caption via
ShowMessage? 0x27B98 when g_options[5], wait on 0x96372, then 0x5C144 and 0x1CB78 (0x92DD4)).

## Doors

A sector's door table is the first 48 bytes of its `sectors` row: 12 pairs (tile, value) of signed 16-bit words,
pair k at +4k. Move codes: 1500 = closed door, 25400 = open door (Movement above). "Object" = tile byte 1.

**Door value** (library op 179 door_lookup (tile); entry point `ja1_door_lookup`): the value of the first pair
(k = 0..11) of the current sector whose tile word, as a signed number, equals the tile argument (the whole 32-bit
argument, so a tile with bits above 16 matches nothing); no such pair: 0. The result is the value as a signed
32-bit number. A value above 100 marks a big door (lock strength value − 100).

**Opening or breaking an object** (library op 107 break_object (tile, byte object, int sound); `ja1_break_object`).
The object argument decides, not the tile's current object. All object changes wrap at 8 bits.
- Objects 0x3E and 0x5D: the tile's object − 3; objects 0x3D and 0x5C: the tile's object − 2. For these four the
  tile's move code becomes 25400 and the object of the tile to the west (tile − 1) − 2.
- Objects 0x45 and 0x64: the tile's object − 4; objects 0x43 and 0x62: the tile's object − 2. For these four the
  tile to the north (tile − 56) gets object n = its object − 2 and byte 2 = 0x45 when n is 0x42, else 0x4E; the
  tile's move code becomes 25400.
- Any other object: nothing at all.
- After a change, when the low byte of sound is not 0: host op 60 sfx (0x18, 0x2B11, 0x32, 1).

**Closing a door** (library op 104 close_door (tile, byte object, int sound); `ja1_close_door`): nothing when a
soldier stands on the tile (byte 5 & 0x7F below 0x30). Objects 0x3B and 0x5A: the tile's object + 3, its move code
1500, the object of the tile to the west + 2. Objects 0x41 and 0x60: the tile to the north gets object + 2 and
byte 2 = 0; the tile's object + 2 and its move code 1500. Any other object: nothing. The sound as above.

**Using a key** (part of the door effect; input: the soldier, big 0 / 1, the lock value v as a signed 16-bit
number; result 1 = a key fitted). The key item is 0x57, or 0x58 for a big door. The soldier's item entries are
searched in pocket order: entry indexes 0 (hand), 7, 8, 9, 10, 11, 1 (pocket number i = 0..6 in that order;
entry = the 12 bytes at record + 12 × index). An entry whose type (byte 0) is not the key item is skipped. A key
entry holds count = byte 1 (signed) key ids: id 0 in byte 2, id k (1..) in byte k + 7. For k = 0, 1, ... while k
is below the count (read again at each step), with ids compared as signed bytes against v:
- k = 0 and id 0 = v: when the count is above 1, byte 2 = byte (count + 6) (the last id), that byte = 0 and the
  count drops by 1; otherwise all 12 bytes of the entry become 0. Result 1 (after the panel step below).
- k ≥ 1 and byte (k + 7) = v: for k below 4 the bytes 9 + m move to 8 + m for m = k − 1 .. 2, and then byte 0x0B
  of the SOLDIER RECORD (the hand entry's byte 11, whichever entry held the key) becomes 0 — the original game's
  mistake (bug list row 9; the ring's own byte 11 keeps its old value); for k ≥ 4 byte (k + 7) of the entry = 0
  (this lies past the entry for k > 4). The count drops by 1. Result 1 (after the panel step).
- Panel step: when the world's `selected` slot is this soldier, host op 174 item_cond (i, slot, 0, 1).
No id matched in any entry: result 0.

**The door effect** (library op 70 door_fx (slot, tile, byte task); `ja1_door_fx`): a soldier works the door or
window on the tile (the Use animation's step, ai.md word 0x1C2). The tile is taken as a signed 16-bit number.
1. obj = the tile's object. Host op 209 door_snd (0) (the front-end stops all sound and loads the door sounds).
2. v = the door value of the tile (computed here as a plain procedure, never a host call) as a signed 16-bit
   number; big = v > 100, and then v = v − 100.
3. The result r: v = 0 gives 11 (open). Otherwise the item it is 0 when task is 0xFF, else the hand item (record
   byte 0).
   - it = 0x4C (lock pick): skill = (wisdom +0x18D × mechanical +0x190) / 100 × dexterity +0x18C / 100 + level
     +0x192 × 5, all signed bytes and truncating divisions. When (condition +2 (signed) × skill) / 100 ≥ v: r = 10
     and host op 105 stat_practice (slot, 7, 2, 0), then (slot, 4, 2, 0), then (slot, 3, 1, 0). Otherwise
     Random(100) is drawn; when it is below v and the condition byte is not 0, the condition drops by 1; r = 7.
   - any other it: Using a key (big, v) gives 1: r = 9; else r = 6 when it is 0x57 or 0x58, else 8.
4. Host op 210 door_anim (big, r, 0, x, y) (the try). (x, y) = the film's screen place, computed at the start
   (PlayDoorFx 0x3B03C): x = TileScreenX − 32, y = TileScreenY − 14 (TileScreenX = 16 × (tile mod 56) − view x +
   left, TileScreenY = 14 × (tile / 56) − view y + top, truncating); then x < left gives left, else x + 79 > right
   gives right − 79; y < top gives top, else y + 49 > bottom gives bottom − 49. The view window (SetViewWindow
   0x24880): left = 48 when the view is narrower than 320, else 0; top = 0 in mode 3, else 14; right = left + view
   width − 1, bottom = top + view height − 1.
5. r = 8 with a big door: host op 209 door_snd (1); done.
6. r ≥ 9: break_object (tile, obj, 1) (library op 107); then l = Random(3); host op 127 make_noise (slot, tile,
   record byte +0x1B3, l, 2); record byte +0x9D = 1.
7. Host op 210 door_anim (big, r, 1, x, y) (the outcome, same place).
8. When `selected` is this soldier: host op 174 item_cond (0, slot, 0, 1).
9. Host op 209 door_snd (1) (the front-end stops the door sounds and loads the overhead sounds).

The front-end shows door_anim as a short film and sound: the door kind (big: "I_PD", else "I_DR") and the action
by r (6 / 9 key "KEY", 7 / 10 lock pick "PCK", 8 / 11 hand "HAN"); phase 0 is the try, phase 1 the outcome
(success for r ≥ 9, failure otherwise). The films (sound.md "Op 210") play at (x, y) on the screen with the game
stopped: the try film, then the outcome film; the next game frame draws over them.

## Reload

**Reload** (library op 190 reloaded (slot), and op 102 reload_check (slot) of soldier_update.md, the same procedure; entry point `ja1_reload`; result 0 = the gun was reloaded, 1 = no clip):
the soldier's gun in hand (type t = record byte 0) takes the ammo item a = byte 1 of the `weapons` row of t
(6 bytes per type).
1. The clip: the first entry in pocket order (entry indexes 0 (hand), 7, 8, 9, 10, 11, 1; pocket number p = 0..6,
   as in Doors "Using a key") whose type (byte 0) is a. None: result 1, nothing else.
2. Record byte 1 (the rounds in the gun) += byte 5 of the `item_types` row of t (the gun's clip size; 8-bit wrap).
3. When s8 record +0x1D5 > 0: host op 105 stat_practice (slot, 3, 1, 0).
4. Host op 60 sfx (0x0F when t < 8, else 0x0E; 0x2B11, 0x32, 1).
5. One clip leaves the entry e (12 bytes): c = s8 byte 1 of e; when c > 1, byte c + 6 of e = 0. Byte 1 of e drops
   by 1 (8-bit wrap); when it is now 0, all 12 bytes of e become 0.
6. When the world's `selected` slot is this soldier: host op 174 item_cond (0, slot, 0, 1), then item_cond (p, slot,
   0, 1).
7. When opt_step_text ≠ 0: host op 57 merc_text (slot, 2, 800) (text 2 = "has reloaded.").
8. Result 0.
