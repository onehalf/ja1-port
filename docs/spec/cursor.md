# The tactical cursor

Over the tactical map the game hides the mouse arrow. The only cursor there is a **tile cursor**: a tile sprite
drawn at the tile under the mouse, with an AP number beside it in combat. Outside the map the mouse arrow is drawn
(panel element 78; element 62 while a mouse button is held). This page says how the tile cursor is drawn
("Drawing") and how its value is chosen while the mouse moves without a button ("Hover").

Numbers are signed 32-bit unless stated; "/" and "%" truncate toward zero. Sprite n = sprite n of the tileset's
TILES bank (tactical_map.md). The back buffer is the 384-pixel-wide buffer of tactical_map.md "Back buffer"; the map
window starts at (32, 28).

## Cursor state

| name | meaning |
|---|---|
| over_map | 1 while the mouse is over the map (the hover step sets it); 0 = no tile cursor |
| tactical_on | 1 while the tactical screen is up; 0 = nothing of this page is drawn |
| overlay_up | 1 while a full-screen overlay hides the map; 1 = nothing of this page is drawn |
| value | the cursor value (byte), see "Values" |
| mode | the cursor mode (byte): 0 none, 1 walk, 2 weapon, 3 hand |
| tile | the mouse tile (map index row × 56 + column) |
| out_of_range | 1 = the target tile is past the weapon range (mode 2 / 3) |
| ap | the AP number (0 = none) and its decimal text |
| ap_fg, ap_shadow | the AP number colours: (1, 0) = not enough AP, else (0xFF, 0xFE) |
| combat | 1 in turn-based combat (the AP number is shown only then); the same byte as the world field `combat` |
| blink_timer, blink_phase | the blink clock: the game's timer lowers blink_timer by 10 per tick while round_lock2 = 0; phase 0 / 1 |
| no_blink | 1 = the AP number does not blink |
| wobble | door wobble clock (≤ 0 = off; −1000 = "not started") |
| wobble_dir | the tile's standable result (signed byte, see "Hover"); −2 = the door wobbles in x |
| wobble_dx, wobble_dy | the wobble offsets the drawing writes (16-bit) |
| under_soldier | 1 when this frame's target mark could not go under a soldier (cleared at each full redraw) |
| label, label_x, label_y | the hover label text (up to 9 characters + 0) and its place; label_x = −1 = none |
| marker_kind, marker_tile | the tile marker's picture and tile ("Tile marker"); marker_tile = 0xC41 = none (the start value) |
| marker_life, marker_blink, marker_phase | the tile marker's life and blink clocks (start 0, 200, 0) and its blink phase |
| shown_slot, spotted, view_tile | the world fields of the same names (turns.md): the soldier the view goes to, the enemy that was spotted (0x40 = none) and a tile the view goes to (0xC41 = none); "Soldier and tile marks" draws them |
| mark_timer, mark_phase | the clock and the picture step (0..4, signed byte) of the soldier and tile marks; the sector load sets mark_timer = 100 (LoadTacticalSector? 0x2AEA6), mark_phase starts at 0 |

## Values

The cursor picture is sprite 413 + P, P = the **cursor table** byte for (value, out_of_range): the table in JA.EXE
at linear 0x9545A (file offset 0xDC4FE), 182 pairs of bytes, pair v = (P in range, P out of range). The pair
for value v is at 2 v. Used values:

| value | picture |
|---|---|
| 0x8E | walk square |
| 0x8F | red square (confirm walk) |
| 0x90 | "ALL" |
| 0x91 | yellow brackets (another merc) |
| 0xB5 | X (cannot go there) |
| 0xA1 | "?" (cannot act) |
| 0x0A, 0x0B, 0x0C, 0x0D | edge arrows: north, east, south, west (a drag to an edge) |
| 0xA4 | door |
| 0x94 | hand |
| 0x77 | red hand (an item lies here) |
| 0x8C | grave |
| 0x92, 0x95, 0x96, 0x97, 0x98 | weapon kinds: circle, brackets, knife, syringe, key |
| 0x74, 0x75, 0x76 | red circle, brackets, knife (the weapon cursor on a soldier) |
| 0x93 | first aid |
| 0x8B, 0xA2 | red syringe, red key |
| 0x9C | fire confirm |

## Drawing

The full redraw of the tactical screen (tactical_map.md "Drawing order") clears under_soldier and sets label_x = −1
before it starts. Two cursor steps are inside it:

**Target mark** (after the shadows, before soldiers and objects). Only when over_map = 1, tactical_on = 1 and the
value is 0x8E..0x91 or 0xA4: if the occupant of the mouse tile (tile byte 5 & 0x7F) is below 0x30, draw the
**cursor sprite** (below) with offset (0, 0); it is then under the soldier. Else set under_soldier = 1.

**Cursor pass** (the last step, after the projectiles). Nothing when tactical_on = 0 or overlay_up = 1. When
over_map = 1:
1. If under_soldier = 0: when the value is not 0x8E..0x91, draw the cursor sprite with offset (−2, −2) (a value
   in 0x8E..0x91 was drawn by the target mark or is not drawn).
   If under_soldier = 1: draw the cursor sprite with offset (wobble_dx, wobble_dy) when the value is 0xA4, else
   offset (0, 0). (wobble_dx, wobble_dy are read before this draw changes them.)
2. If label_x ≥ 0: Text (label, label_x, label_y, centred, fg 0xFF, shadow 1, font 0, mode 300) into the back
   buffer with clip width 384 (see "Text").

Then, also when over_map = 0: the **soldier and tile marks** (below): the soldier mark of shown_slot when
shown_slot < 0x40 (signed), the soldier mark of spotted when spotted < 0x40 (signed), the tile mark of view_tile
when view_tile ≠ 0xC41; then 3. the **tile
marker** (below) when marker_tile ≠ 0xC41, then the path marks of the selected soldiers (not described here), then
the clock (panel.md "Pause (key P) and the clock").

**Soldier and tile marks** (a bouncing arrow over the soldier the view went to, over the enemy that was spotted,
and over the tile the view went to; state: mark_timer, mark_phase). The game's timer lowers mark_timer by 10 per
tick while round_lock2 = 0, with blink_timer (soldier_update.md "Timer tick" gates). One mark:
1. Soldier mark of slot k (S = soldier record k): if s16 S +0x114 (its tile) = the mouse tile `tile`: stop here
   (no picture and no step 3). Else x = s16 S +0x116 − view_x + 22, y = s16 S +0x118 − view_y + 19 (the soldier's
   pixel place; the call passes the offset (−2, −2)); base = 0xBB, or 0xDC when k ≥ 0x28 (an enemy).
   (SPEC GAP (not seen: the game sets shown_slot and spotted only to a soldier slot 0..0x2F or to 0x40): a slot
   0x30..0x3F reads past the 48 soldier records in the original; the port draws no mark there.)
   Tile mark of tile t: x = 16 × (t % 56) − view_x + 32, y = 14 × (t / 56) − view_y + 28; if tile byte 1 (the
   object) of t ≠ 0: x = x − 3, y = y − 3. base = 0xBB. (The call's offset is not used.)
   (SPEC GAP (not seen: view_tile is a map tile 0..3135 or 0xC41): another tile reads outside the tile map in the original; the port
   takes its object as 0.)
2. If 16 < x < view_w + 32 and 14 < y < view_h + 28: sprite 413 + base + mark_phase (signed) at (x, y).
3. If mark_timer ≤ 0: mark_phase = mark_phase + 1; if it is then above 4 (signed): mark_phase = 0; mark_timer =
   100. (So the picture steps through the five frames base..base + 4, one step per 0.1 s; the marks share the
   clock, so in one pass only the first mark that reaches step 3 moves it.)
Evidence: DrawCursorPass? 0x171E8 at 0x172B7..0x17313 calls DrawTileCursor? 0x15E8C kind 1 with (shown_slot
0x229CB4, −2, −2), (spotted 0x229C94, −2, −2), (view_tile 0x229C8C, dx 99 = the tile form, 0); kind-1 path
0x16172..0x16263 (soldier fields +0x114 / +0x116 / +0x118, tile byte 1 at 0x100A19, phase byte 0x95F5A, clock
0x229D38 = 100); TacticalTimer? 0x5EEC5..0x5EF3C lowers 0x229D38 and 0x229D4C by 10 behind the 0x92DE1 test.

**Tile marker** (the mark a move order or first aid leaves on its target tile; state: marker_kind, marker_tile,
marker_life, marker_blink, marker_phase):
- Set (`tile marker` host op 130 (kind, tile); the ALL-move puts kind 0x0F, see "ALL-move"): marker_kind = kind,
  marker_tile = tile, marker_blink = 200, marker_life = 1000. Kinds used: 0x8A (the walk target of a move order
  with a marker), 0x0F (ALL-move), 0x93 (first aid).
- Timer (the game's 10 ms timer, front-end part, soldier_update.md "Timer tick" gates): while round_lock2 = 0:
  when marker_tile ≠ 0xC41: marker_life = marker_life − 10; if it is then ≤ 0: marker_tile = 0xC41. While
  round_lock2 = 0 and round_lock = 0: marker_blink = marker_blink − 10 (always, also with no marker).
- Drawing: x = 16 × (marker_tile % 56) − view_x + 32, y = 14 × (marker_tile / 56) − view_y + 28. If marker_blink
  ≤ 0: marker_phase ^= 1, marker_blink = 200 (also when the tile is off the window). Then, when marker_phase ≠ 0
  and 16 < x < view_w + 32 and 14 < y < view_h + 28: sprite 413 + marker_kind (the kind itself, not the cursor
  table) at (x, y). So the marker blinks (0.2 s on, 0.2 s off) and goes away after 1 s.
- (The quick save keeps marker_kind and marker_tile: savegame.md blocks 41 / 42; not their clocks.)

**Cursor sprite** with offset (dx, dy) (dx, dy are 16-bit signed):
- x = 16 × (tile % 56) − view_x + 32, y = 14 × (tile / 56) − view_y + 28 (view_x, view_y = the pixel position of
  the window's top-left map corner, 16 × c0 and 14 × r0). Nothing is drawn unless 16 < x < view_w + 32 and
  14 < y < view_h + 28 (view_w, view_h = the window size in pixels, 16 × columns and 14 × rows).
- (The game saves the 16 × 14 back-buffer pixels at (x, y) first, to restore them later; a front-end that redraws
  the whole frame need not.)
- Blink: if blink_timer ≤ 0: blink_phase ^= 1, blink_timer = 200.
- Colours: if no_blink = 0 and blink_phase = 1, (fg, sh) = (0xFF, 0xFE); else (fg, sh) = (ap_fg, ap_shadow).
- Red phase R = 1 when out_of_range = 1 and blink_phase = 1, else 0.
- If wobble ≤ 0 and R = 0: draw sprite 413 + P(value, out_of_range) at (x + dx, y + dy). Then, if ap ≠ 0 and
  combat = 1: Text (ap text, x + dx + 9 − mode, y + dy + 4, centred, fg, sh, font 0, mode 300).
- Else: the picture value p = value, offsets (ox, oy) = (dx, dy). If wobble > 0: p = 0xA4 and, when wobble_dir =
  −2, (ox, oy) = (−4, 0) and (wobble_dx, wobble_dy) = (−4, 0); else (ox, oy) = (0, −4) and (wobble_dx, wobble_dy)
  = (0, −4). Draw sprite 413 + p (p itself, NOT the cursor table) at (x + ox, y + oy). Then, if ap ≠ 0, combat = 1
  and p ≠ 0xA4: Text (ap text, x + ox + 9 − mode, y + oy + 4, centred, fg, sh, font 0, mode 300).
  So an out-of-range cursor blinks between its table picture and the raw sprite 413 + value.

**Text** is panel.md "Text" with the back buffer as target: the clip width is 384 instead of 320 (w = 384 − x when
x + w > 384, unsigned) and the stop test is x > 383.

Evidence: DrawTargetMark? 0x16DD0, DrawCursorPass? 0x171E8 (flags 0x92DCC tactical_on, 0x92DCE overlay_up,
0x284F13 over_map, 0x284F03 under_soldier; label 0x22A444 at 0x22A480 / 0x22A482, wobble offsets 0x22A47E /
0x22A47C), DrawTileCursor? 0x15E8C kind 0 (view 0x229CF8 / 0x229CFC, size 0x229CBC / 0x229CC0, save buffer
[0x93E78], blink 0x229D4C / 0x95F5C, no_blink 0x95E0D, colours 0x92E5C / 0x92E60, out_of_range 0x955C8, ap
0x229DEC + text 0x229DE0, combat 0x92DCB, mode 0x284F1B, value 0x284F1D, tile 0x229D2C, wobble 0x229D3C,
wobble_dir 0x284F05, sprite pointers 0xA92BC = sprite 413), DrawText 0x358D0 (EBX = pitch = clip width); tile marker: kind 0x229CA8, tile 0x229CAC, life
0x229D20, blink 0x229D40, phase 0x284F30, draw FUN_0001626C (TileMarkerDraw?) at 0x17328, set by MoveMercTo
0x1C99D, AllMoveTraverse? 0x1D286 and SetTileMarker? 0x1F634, timer TacticalTimer? 0x5EF75 (life) / 0x5F0A1
(blink, after the round_lock 0x92DE0 test at 0x5EFFD).
Tests: validate/diff/cursor_diff.py (original 0x16DD0 + 0x171E8 in Unicorn on mem_fight59 vs ja1front
ja1f_cursor_draw).

## Hover

The hover step runs once per tactical frame while the mouse is over the map and no mouse button is held, with
the mouse tile T. Besides the "Cursor state" it uses the **hover state**:

| name | meaning |
|---|---|
| ap_timer | AP number delay (start 800 at each new tile, −10 per hover step) |
| item_timer | item hand delay (start 2000 at each new tile, −10 per hover step) |
| lit | the merc slot whose portrait is lit (0x40 = none) |
| named | the soldier whose health word is in the label (0x40 = none) |
| light_wait, light_timer | portrait light delay used while panel redraws are off |
| target | the soldier under a red weapon cursor (0x40 = none) |
| redraw_off | the panel redraw switch (2 or more = panel redraws off) |
| strip_hover | the bar area under the mouse (interface strips; "none" when the mouse is over the map) |

World data: selected (the selected merc slot), path_preview, soldiers (S = the selected merc's record), tiles,
ground items, item types; shift / alt = the Shift / Alt keys held. Record fields: hp = s8 +0x183, slot = +0x17A,
side = s8 +0xBA, apLeft = s8 +0x1A5, hand type = +0x00, hand condition = s8 +0x02; item type fields kind = +2,
range = +11 (item type entries 12 bytes). Ground item of a tile: index = tile byte 6 (none when ≥ 200); there is a
**ground item** when the index is below 200, its entry's type byte (+0) ≠ 0 and s8 entry +6 > 0.

Routines used by the steps:
- **Hide marks**: ap = 0, tile = 0xC41 (off the map), then the library's Hide path marks (0) (select.md).
- **Unlight (p)**: label = "" (first byte 0); if p < 8: lit = 0x40 (and the portrait p is drawn plain again).
- **Light (o)**: if redraw_off ≠ 1 or o ≥ 8: if redraw_off ≥ 2 and o < 8: lit = o; label = nickname of o.
  Else: if lit < 0x40 and o ≠ lit: Unlight (lit); label = nickname of o; lit = o (the portrait o is drawn lit).
  (Nickname = the zero-ended text at record +0x16E.)
- **Name (o)**: named = o. If side of record o = 4: label = "SANTINO". Else label = the health word with number
  (hp of record o / 15) & 0xFF: 0 DYING, 1 CRITICAL, 2 WOUNDED, 3 POOR, 4 HEALTHY, 5 STRONG, 6 EXCELLENT, 7 FAINT.
- **Dead fix**: if S hp ≠ 0: value = 0xA1. Else if mode = 1: value = 0x8E. Else if S slot < 8: value = 0x8C.
- **Weapon cursor**: if S hp > 14, S does not stand in water (Water tile of +0x1B2 / +0x1B4, ai.md), the anim
  word u16 at record +0x1E8 + 2 × (+0x1F3) is above 3, and (hand condition > 9, or byte 7 and byte 9 of the hand
  item type are both 0): value = kind of the hand item type. Else value = 0xA1.
- **Mode cursor**: Hide marks. Mode 0: if S hp ≠ 0: value = 0xA1, mode = 1; else if S slot < 8: value = 0x8C.
  Mode 1: if S hp > 14: value = 0x8E; else if S hp ≠ 0: value = 0xA1; else mode = 0, value = 0x8C. Mode 2: Weapon
  cursor. Mode 3: value = 0x94.
- **AP colours**: (ap_fg, ap_shadow) = (1, 0) if apLeft < ap, else (0xFF, 0xFE).
- **Target AP**: only when the kind of the hand item type is 0x92 or 0x95: ap_timer −= 10; if ap_timer ≤ 0:
  ap_timer = 0; ap = s8 of the library's action AP (S, tile, 1) (ja1_action_ap); if +0x16A ≠ 0, ap < apLeft: ap += 4 if
  ap + 4 ≤ apLeft, else ap = apLeft. AP colours.

Steps (tile = the cursor state's tile, which step 3 sets to T and Hide marks can set to 0xC41):
1. over_map = 1. If overlay_up = 1, tactical_on = 0 or selected ≥ 8: stop.
2. If strip_hover is not "none": strip_hover = none (that slot's bar area is drawn again).
3. If T ≠ tile: ap_timer = 800, item_timer = 2000; if mode = 1: value = 0x8E; Hide marks; tile = T.
4. If S +0x1B8 = 0: ap_timer = 0, and if ap ≠ 0: Hide marks.
5. o = tile byte 5 of T & 0x7F. wobble_dir = 0.
6. If ap_timer > 0 and value is 0x74, 0x75, 0x92 or 0x95: Target AP.
7. If o < 0x40 and s8 record o +0x9C > 0: **soldier tile** (7a). Else **empty tile** (7b).
8. If wobble_dir ≥ 0: wobble = −1000.
9. If mode = 1 and item_timer > 0: item_timer −= 10; if item_timer ≤ 0: item_timer = 0, and if T has a ground
   item: value = 0x94, ap = 0.
10. out_of_range = 0. If mode ≥ 2: limit (s16) = 15 for mode 3; else, when the hand item kind is 0x95, the throw
    reach (click.md step 3, k = 0x95); else range × 15. If World distance (s16 +0x116, s16 +0x118, px(tile),
    py(tile)) (ai.md) > limit: out_of_range = 1.

7a. Soldier tile:
1. wobble = −1000.
2. If o < 8 (a merc):
   1. named = 0x40.
   2. If o ≠ lit: if redraw_off < 2: Light (o). Else: lit = 0x40; if light_wait = 0: light_wait = 1; else if
      light_timer < 1: Light (o), light_wait = 0, light_timer = 300; else light_timer −= 10.
   3. If mode < 2: if o = selected: if value is 0x91, 0xB5 or 0xA4: Dead fix. Else (another merc): if record o
      +0x194 = 0: value = 0x91.
      Else if o = selected, mode = 2 and value ≠ 0x93: value = 0xB5.
      Else if value = 0xB5: mode 2: Weapon cursor; mode 3: value = 0x94.
      Else if S hp < 15: value = 0xA1.
3. Else (o ≥ 8):
   1. light_wait = 0, light_timer = 300. If lit < 0x40: Unlight (lit), then if value = 0x91: Dead fix.
   2. If mode = 1: if o ≠ named: Name (o), then if value = 0xA4: Mode cursor; else if S hp < 15: value = 0xA1.
   3. If mode is 1 or 2: value 0x92 → 0x74, 0x95 → 0x75, 0x96 → 0x76 (and then target = o); value 0xB5 →
      Weapon cursor; other values stay.
4. If mode = 3: value = 0x77 if T has a ground item, else 0x94.

7b. Empty tile:
1. light_timer = 300.
2. If S hp < 15 and mode ≠ 0: value = 0xA1.
   Else if mode = 1, ap_timer > 0, value is 0x8E or 0xA4, S +0x1BE = 0 and S +0x1BF = 0: ap_timer −= 10; if
   ap_timer ≤ 0: ap_timer = 0; ap = the library's path AP marks (selected, tile, mark = 1 if path_preview = 0
   else 0, 0, shift, s8 S +0x177, alt) (ja1_path_ap_marks); AP colours.
3. named = 0x40, target = 0x40. If lit < 0x40: Unlight (lit). Else if value = 0x91: Dead fix.
4. d = tile_standable (T) (ai.md "Standable tile"; a signed byte); wobble_dir = d.
5. If d < 1:
   1. If mode = 1: if d = 0: value = 0xB5 unless it is 0x94. If d < 0: value = 0xA4 if wobble > 0, else 0xB5;
      ap = 0.
      Else (mode ≠ 1): if value = 0xB5: Mode cursor.
   2. If wobble = −1000: wobble = 550 when t3 of T ≤ 0x50, or (d = −2 and t3 of T − 112 ≤ 0x50), or (d ≠ −2 and
      t3 of T − 1 ≤ 0x50). Else if mode ≠ 1: wobble −= 10.
   If d ≥ 1: if mode = 2, t0_class[t0 of T] ≠ 3, t3 of T = 0, or (gv_t3[t3] ≠ 0 and t3 is not 0x54, 0x5F or
   0x6A): if value is 0xB5 or 0xA4: Mode cursor. Else: value = 0xB5.
6. If mode = 3: value = 0x77 if T has a ground item, else 0x94. Else: value 0x74 → 0x92, 0x75 → 0x95,
   0x76 → 0x96, 0x8D → 0x93; 0x8B → 0x97 and 0xA2 → 0x98 when d ≥ 0; 0x97 → 0x8B and 0x98 → 0xA2 when d < 0.

Notes:
- Hide marks in the original also runs inside many library routines (select, walk end, animation changes ...):
  each sets ap = 0 and tile = 0xC41 there. The library's Hide path marks does not touch the cursor state; a
  front-end that wants the same look clears ap and tile itself when the selected merc changes or walks.
- Mode cursor sets tile = 0xC41, so the tile cursor is not drawn in that frame; the next hover step sees a new
  tile and starts the delays again.
- An occupant 0x30..0x3F is past the 48 records (the original reads other memory); the front-end takes it as an
  empty tile.

Evidence: UseItemAt? 0x1D9B8 (no-button path 0x1D9B8..0x1DA9x + 0x1E436..0x1ED9x), TargetApNumber? 0x1D690,
ApNumberText? 0x1D644, MoveCursorState? 0x1C464, WeaponCursor? 0x1C39C, HidePathMarks? 0x1D8B8 (0x229DEC = 0,
0x229D2C = 0xC41 first), SetSpeakerName? 0x336E8 (health words 0x964EC, 10 bytes each; "SANTINO" 0x907E8), light
0x3349C, unlight 0x33570 (label byte 0x907E4 = 0), TileStandable 0x23538, t3 0x23A78, TileItemIndex 0x23768,
throw reach 0x58AB8 (EAX soldier, EDX hand type), PathApCost 0x1D32C (stack: shift 0x284F21, +0x177, alt
0x284F20). State: ap_timer 0x229DD4, item_timer 0x229DDC, lit 0x284F0E, named 0x95E0C, light_wait 0x93D4C,
light_timer 0x93D50, target 0x229C4C, redraw_off 0x92DDF, strip_hover 0x93448 (0x40 = none), path_preview
0x229DD8, selected 0x284F11, wobble_dir 0x284F05.

## Clicks

A mouse button press over the map runs one **click** at the mouse tile T in place of that frame's hover step. The
click reads the cursor state that the hover steps left (above all `value` and `mode`): a click acts on what the
cursor shows. World data as in "Hover"; more record fields: inCur = +0x182, turnDone = +0x1B0, aim = +0x1FD, tile
= u16 +0x114, hand count = +0x01. **Anim word** = the u16 at record +0x1E8 + 2 × (+0x1F3) (as in Weapon cursor);
**in water** as in Weapon cursor; occupant (t) = tile byte 5 of t & 0x7F (0x40 = nobody).

Routines:
- **Not enough AP (S, c)**: inventory.md "Not enough AP" (the merc is the selected one, flag 0): true when combat
  ≠ 0 and s8 S +0x1A5 + s8 S +0x1E2 < c, with c read as an unsigned byte (a cost below 0, such as −1, is never
  enough); then the merc says "doesn't have enough Action Points!" (merc_text, 1200).
- **Target memory (S, T)**, after a weapon click that started: only when the hand type (+0x00) is 1..16. o =
  occupant (T); S +0x1C2 = o. If o = 0x40: S +0x1F7 = 0x40, S +0x1BE = 1, u16 S +0xDC = T, S +0x1C2 = 0x3F.
  Else if o ≥ 0x28: if s8 record o +0x9C > 0: S +0x1F7 = o, u16 S +0x1F8 = T; else S +0x1F7 = 0x40, S +0x1C2 =
  0x3F.
- **Walk (S, T)**: S +0x177 = 0, +0x1A8 = 0, +0x196 = 0, +0x16D = 0; clear task: +0x1A6 = 0, u16 +0x134 =
  0xC41, +0x1A7 = 0, +0x95 = 0, u16 +0x96 = 0xC41. The world's key_shift = shift, key_alt = alt.
  **Walk confirm** (the option confirm_move, save byte 21552, on; no ALL-move; S < 8; `ja1_move_gate` (S, T) = 1, which is
  steps 1-3 of the move order without their messages): keep a copy of S's 80 path bytes (paths + 80 × S); r =
  `ja1_path_ap_marks` (S, T, mark 1, extra 0, sneak = shift, byte177 0, alt). If r ≤ 0: the status line shows
  "Can't move there!" and the walk ends. Else value = 0x8F, tile = T, and the game waits for a **walk confirm**.
  The answer: anything but a left click on T gives tile = 0xC41 and, except for Escape, item_timer = 2000. Then
  always Mode cursor. A left click on T then runs `ja1_move_confirmed` (S, T, marker 1, sneak = key_shift, byte177
  0, alt = key_alt, walk AP = the world's walk_ap): the move order from its step 5 on with that walk AP (no second
  pricing, path length not cleared). Any other answer puts the 80 path bytes back.
  Without the walk confirm: the library's move order `ja1_move_order` (selected, T, marker 1, sneak = shift,
  byte177 0, alt, squad 0) (its gates show their messages).
  **Drag to an edge**: Walk (S, T) starts at the left press and follows the mouse until the left button is up.
  (px, py) = the mouse point at the press. On each mouse poll while the round is not locked (Pause) and no ALL-move
  came: dir = 10 (north) when py − y ≥ 17; else 12 (south) when y − py ≥ 17; else 13 (west) when px − x ≥ 17;
  else 11 (east) when x − px ≥ 17; else none (the mouse is back within 16 pixels: the walk follows). With a dir:
  value = dir (the arrow cursor of that edge) when `ja1_traverse_edge` (S, dir) ≠ 0, and the "edge failed" flag
  goes off; else value = 0xA1 (the "no" cursor), the flag goes on, dir = none and (px, py) = the mouse point now (a
  new drag starts from here). At the release the value comes back to what it was at the press and tile = 0xC41.
  Then: with a dir, the traverse check (soldier_update.md "Traverse check") runs for S: −1: nothing more (the AP
  text came); 1: the squad leaves over that edge (the move of soldier_update.md); 0: the status line shows "Can't
  traverse here!". Without a dir: when the "edge failed" flag is on, the status line shows "Can't traverse here!";
  else the walk above (walk confirm or move order) runs. So `ja1_traverse` (S, dir) gives the same results for a
  straight drag (the port may use it: the edge test is the same at the release).
  Only a press that reaches Walk starts this loop: mode 1, a tile with no merc (occupant >= 8), value not 0xA4, 0x94
  or 0xB5, and no blocked tile. Every other press acts at the press (a merc is selected while the button is down). In
  the loop no key is read: a key waits in the queue for the release. Only a change of the arrow redraws the picture.
  The loop does not run the key flag step 0x1409B either: the release ends the pass of the press, and the next pass
  reads the held key with the Alt latch 0x284F20 of the press (an Alt+F1..F8 held in the loop selects with force 0
  when Alt was up at the press, also with Alt still down at the release). The key arms of TacticalKeyHandler read
  no Shift or Ctrl latch (their xrefs in it are the flag step only).
  The overview mouse step's left press runs the same Walk (panel.md "The overview mouse step" step 6), so the drag
  works there too. The overview picture shows no arrow: the drag's redraw in mode 0 is the overview picture, which
gets the value but never reads it (OverviewRedraw 0x23B4C stores DL at [esp + 0x30] only), so its own cross cursor
stays.
  **ALL-move**: the right button pressed while the left one is held, at a poll with no dir yet (once the ALL flag is
  on, the polls make no dir any more). In combat the status line shows "ALL-MOVE not available during Turn-based",
  the game waits until the right button is up and Walk ends at once (no walk and no traverse at the release). Else
  the ALL flag goes on, value = 0x0F (the "ALL" picture) until the right button is up, then the value of the press
  again. At the release with the ALL flag (no dir): the "edge failed" flag on gives "Can't traverse here!" as above;
  else the move order runs with squad = 1 (soldier_update.md "Move order" step 2: the other mercs 0..7 get the same
  order to T first). When it gives 1, the world's tile marker becomes kind 0x0F on T (in place of the order's 0x8A,
  same timers 200 and 1000; see "Tile marker"). The ALL-move walk never asks for the
  walk confirm (the original's confirm test inside the move order needs squad = 0).

Common steps (both buttons): Hover steps 1, 2, 3 and 4 (with their stops). o = occupant (T). Then the world's
`spotted` = 0x40, `shown_slot` = 0x40 and `view_tile` = 0xC41 (so the soldier and tile marks of "Drawing" go
away; evidence: 0x1DB40..0x1DB4C left, 0x1E132..0x1E144 right).

**End of a click**: every click that passed the common steps ends with Hover step 10 (the range test), after its
wait when it opened one (the aim, the confirm), whichever way the click ended.

**Left button** (with debug level 4 and Shift held on a soldier o < 0x30 the click opens debug pages in place
of steps 1..3: see "Debug keys (level 4)"):
1. **Mode 3**: if S inCur ≠ 0 and S hp > 14: `ja1_click` (selected, T, 0). Done.
2. **Mode 2** (weapon):
   1. Done when S inCur = 0, S hp < 15, the anim word < 4 or > 0x31, or value = 0xA1.
   2. If s8 hand condition (+0x02) < 10 and byte 9 of the hand item type ≠ 0: the status line shows "Can't use
      that!". Done.
   3. k = kind of the hand item type. If k is not 0x92, 0x95 or 0x96: **Fire**. Else (a gun or a thrown weapon):
      1. If the hand count = 0 and `ja1_reload` (selected) = 1 (no clip): merc message 5 (host op 54
         merc_message (S, 5, 0x40): the merc's balloon with quote 5), S +0x1CD = 1. Done. (Reload result 0: the gun is loaded now, go on.)
      2. If S +0x19C = 3, u = s16 S +0xD0, u ≠ 0xC41, u ≠ T, occupant (u) < 0x40 and s8 hp of that record > 0:
         merc message 8 (host op 54 merc_message (S, 8, 0x40)). Done.
      3. cost = s8 of `ja1_action_ap` (selected, T, 1). ap = cost, then if S +0x16A ≠ 0 and ap < apLeft: ap += 4
         when ap + 4 ≤ apLeft, else ap = apLeft (as in Target AP).
      4. If S tile = T, or Not enough AP (S, ap) (the AP number of step 3, not the cost): ap = 0. Done.
      5. If the Control Panel switch 13 **Fast Firing** (0x1163C5, 0 in a new game) is on: S aim = 0 and **Fire** at once,
         also in combat (no aim step, no Weapon cursor; ap keeps its step 3 value until Fire).
         Else if combat = 0: S aim = 4, Weapon cursor, ap = 0, **Fire**.
      6. Else the **aim** starts: value = 0x9C, tile = T, wobble = 0, no_blink = 1, ap_shadow = 0xFE, ap_fg = 0xC2 when apLeft
         = ap or cost < ap, else 0xFF; aim_cost = cost, aim_max = cost + 4 (8-bit), or apLeft when s8 apLeft is
         smaller; S aim = 0. The game waits (no game time passes, the mouse stays on T) for:
         - a left click: S aim = ap − aim_cost; no_blink = 0; Weapon cursor; ap = 0; **Fire**;
         - a right click: ap += 1; if ap > aim_max: ap = aim_cost. ap_fg = 0xC2 when ap = aim_max, else 0xFF;
           ap_shadow = 0xFE;
         - Escape: no_blink = 0, tile = 0xC41; Weapon cursor; ap = 0. Done (no shot).
   **Fire**: r = `ja1_click` (selected, T, hand type). If r ≠ 0: Target memory (S, T), S +0x197 = 0. Then ap =
   0. (The original also plays the merc's "yes" sound; the front-end has no sound.)
3. **Mode 0 or 1**:
   1. Hide marks; the library's hide gets force = 1 when path_preview ≠ 0.
   2. **Roof**: if t0_class[t0 of T] = 3, t3 of T ≠ 0, value ≠ 0xA4, and (gv_t3[t3] = 0 or t3 is 0x54, 0x5F
      or 0x6A): done (nothing).
   3. If value is 0xA4 (door) or 0x94 (hand): if S inCur ≠ 0 and S hp > 14: S turnDone = 0; if value = 0x94 and
      the object byte t1 of T < 0x21: value = 0x77, tile = T, and the game waits for a **confirm**: a second left
      click on T goes on; anything else (Escape, the right button, the mouse off T) gives tile = 0xC41 and
      (except for Escape) item_timer = 2000, and done. Then `ja1_click` (selected, T, 0). Done.
   4. If o ≥ 8 (no merc on T): if mode ≠ 0: if value = 0xB5: value = 0x94, tile = T (the next click uses the
      hand); else S turnDone = 0 and Walk (S, T). Done.
   5. (A merc on T.) If s8 hp of record o ≠ 0: mode = 1. **Alt posture**: if Alt is held, o = the selected slot,
      S's anim word ≥ 4 and S is not in water: if the anim word is 5: Set anim (S, 7, 1, 0); else if Not enough AP
      (S, 2) is false: Set anim (S, 6, 1, 0) (when it is true, its merc text shows). No confirm (unlike the right
      hold's posture). Then r = `ja1_select_merc` (o, 1, 1); if r ≠ 0: Mode cursor with S = record o; done.
      Else, or when r = 0: the status line shows "Not available!".

**Right button**:
1. Hide marks (force = 1 when path_preview ≠ 0).
2. If mode = 3 or value = 0x94: the **tile popup** of T with the right-button wait ("Tile popups" below). Done.
3. Else the **hold**: saved = value; press y = mouse y; o = the occupant of T (the same o as the left button);
   turn, look and posture = 0. The game does not run while the right button is held (no ticks; only the mouse is
   polled). Each poll (the button state and the mouse y of this poll):
   1. With the left button held and S hp ≥ 15 (signed byte): **turn**: value = 0x0E, cursor tile = T, turn = 1.
      Then the **wait for the left button**: the following polls only read the mouse until a poll has the left
      button up (that poll's state is the state of this poll from here on). Then d = Direction (x(S), y(S), x(T),
      y(T)) with x(t) = (t % 56) × 16 + 5, y(t) = (t / 56) × 14 + 5 on S's signed 16-bit tile +0x114. If d = S's
      facing (signed byte +0x1A1): +0x9D = 1. Else, if Not enough AP (S, 1) is false: Turn (S, d, 30)
      (soldier_update.md, library `ja1_turn`); when it gives 1: +0x1D6 = 1 and +0x1D1 = 1. So one left press
      gives one Turn, when the left button comes up; the merc turns when the game runs again (after the hold).
      (Live check 2026-10-27, DOSBox-X fight59: a hold of both buttons for 1 s
      changes nothing while held; after the release the merc turns one step for 1 AP.)
   2. Else, when mouse y − press y > 10: **look**: look = 1, cursor tile = T, value = 0x46.
   3. Else, when o = the selected slot, S's anim word ≥ 4 and S is not in water: **posture**: value = 0x8F, then
      the confirm (as left step 3.3: cursor tile = T; a left click on T = yes; Escape: cursor tile = 0xC41; any
      other answer: cursor tile = 0xC41 and item_timer = 2000). Yes: if the anim word = 5: Set anim (S, 7, keep 1,
      reset 0); else if Not enough AP (S, 2) is false: Set anim (S, 6, 1, 0); posture = 1 (also when the AP were
      not enough). No: Mode cursor (posture stays 0).
   4. The hold ends when the right button is up in this poll's state (after a posture: the state when the confirm
      ended); else the next poll.
4. After the hold: if turn, look and posture are all 0, mode ≠ 0, the hand type ≠ 0 and S hp ≥ 15: mode += 1; if
   mode > 2: mode = 1 (walk ↔ weapon); then Mode cursor. Then: look = 1: value = saved, then the look panel
   routine Set mode (0) (panel.md "View modes": the overview); else turn = 1: value = saved. (A
   posture does not give the value back: after a yes it stays 0x8F, after a no Mode cursor set it.)

A right click (press and release with no left button and no drag) is a hold of one poll with both buttons up: on
the selected merc's own tile (anim word ≥ 4, not in water) it opens the posture confirm, else it runs step 4.

Evidence: UseItemAt? 0x1D9B8 (right hold turn: WaitLeftUp 0x67C54 at 0x1E1D6 before the direction; left 0x1DACE: mode 3 0x1E0C1, mode 2 0x1DDEF..0x1E0BC, mode 0/1 0x1DB69, Alt posture 0x1DCD7..0x1DD6A (g_altHeld 0x284F20, InWater 0x23A34, MsgNotEnoughAP 0x15960 (S, 2, 0)); right
0x1E108..0x1E431), AimLoop 0x1D744 (+1 per right click, cap min(cost + 4, apLeft), wraps to cost; Escape 0x1B;
aim = shown − cost), WaitConfirmClick? 0x1C514, AllMoveTraverse? 0x1CFEC (drag and ALL-move read 2026-10-27: press point 0x229D00 / 0x229D04, round lock
0x92DE0, value 0x284F1D, CanTraverseEdge 0x30FA0, TraverseCheck 0x31344, TraverseMove? 0x315DC, "Can't traverse here!"
0x99F4B, ALL-move combat test 0x92DCB, ALL-move state 0x229CA8 / 0x229CAC / 0x229D40 / 0x229D20; MoveMercTo 0x1C608 with BL 1, CL shift
0x284F21, stack 0, alt 0x284F20, 1, all-move flag), target memory 0x1CAD0, ClearTask 0x3DDB8 (EDX 1),
MsgNotEnoughAP 0x15960, MsgReloaded? 0x40094 (= Reload), mouse y 0x229D04 (press y 0x229C44), right hold loop 0x1E18C..0x1E3A8 (MousePoll? 0x678E8, buttons 0x229CF4: 1 left, 2 right; flags [esp+0x14] turn, [esp+0x10] look, [esp+0xC] posture), TurnTo 0x1AA18, SetAnim 0x17F90, DirectionTo? 0x52560, value 0x284F1D,
mode 0x284F1B.

## Tile popups

A right click in mode 3 or on a sign value (Clicks, right step 2), and the key Z in the normal view (redraw_off ≠ 0)
at the mouse tile, show the popup of tile T: a ground item zooms up from its tile, or a sign shows its text. The
game does not run meanwhile. `wait` = the end wait: the right click waits until the right button is up, the key Z
until a key or a click.

**Which popup.** g = tile byte 6 (the ground entry; 0xFF, and 200 or more, = none).
- If g < 200: if the entry's type ≠ 0 and s8 entry +6 > 0: the **item popup**. Else nothing.
- Else, if the tile's object (t1) is 0x94: the **sign popup** with the current sector. Else nothing.

Common terms: X = 16 × (T % 56), Y = 14 × (T / 56) (the tile's map pixel); the popup shows only when view_x ≤ X <
view_x + view_w and view_y ≤ Y < view_y + view_h (else nothing at all). sx = X − view_x + view left, sy = Y − view_y
+ view top (the tile's screen point; view left and top as in panel.md "View modes", 32 and 28 in the normal view).
First the 16 × 14 pixels that the cursor sprite saved go back to the screen at (sx, sy) (Drawing, "Cursor sprite":
the cursor goes away). A **step wait** = the step timer is set to 20 and the game waits until it is ≤ 0: 20 ms on
the game's 10 ms timer (it is the water ripple timer, world_load.md section 8, so the ripples step right after).
**Clipped copy** of a w0 × h0 block to the screen at (bx, by): if bx < 0: w = bx + w0, the block's columns start at
w0 − w, bx = 0; else if bx + w0 > 319: w = 320 − bx. If by < 0: h = by + h0, by = 0, and the block's rows start at
its row 0 for the item popup but at row h0 − h for the sign popup (an original-game difference; neither happens in
the normal view). Else if by + h0 > 199: h = 200 − by. **Scaled copy** (sw × sh source to dw × dh): nearest point,
10.22 fixed point: fx = (sw × 2^22) / dw, fy = (sh × 2^22) / dh (unsigned); destination point (i, j) takes source
column (fx / 2 + i × fx) >> 22 and row (fy / 2 + j × fy) >> 22 (all pixels copied).

**Item popup** (entry type t, count c = s8 entry +1):
1. x = sx, y = sy; if t1 of T > 0x74 (a raised object): x −= 3, y −= 3. The box is 42 × 16 at (x − 13, y − 1).
2. B = the box under the popup: the 42 × 16 screen pixels at (x − 13, y − 1) (no clipping; the same rectangle is put
   back at the end). P = a 42 × 16 picture filled with 0, then item picture (pic = item table +4 of t, colour 0)
   at (0, 0) (inventory.md "Item picture").
3. w = 2, h = 0. Repeat: if w < 42: w += 5; if h < 16: h += 2; a **frame** (w, h); a step wait; until h ≥ 16
   (8 frames, the last one at 42 × 16).
4. If c > 1 and the item table's stackClass (+6) of t > 1: the count as text, left padded with '^' to 3 characters,
   at (x + 16, y + 10) on the screen, fg 0xFF, shadow 8, font 0, not centred (panel.md "Text").
5. The end wait.
6. While h > 2: w −= 5, h −= 2, a frame (w, h), a step wait (7 frames).
7. B goes back to the screen at (x − 13, y − 1).
A **frame** (w, h): F = a copy of B; P scaled to w × h, then copied into F at ((42 − w) / 2, (16 − h) / 2)
(divisions truncate) skipping the pixels of value 0; then the clipped copy of F (42 × 16) to (x − 13, y − 1).

**Sign popup** (sector n):
1. x0 = sx − 42, y0 = sy − 24; the box is 94 × 55 at (x0, y0).
2. B = the 94 × 55 screen pixels at (x0, y0) (no clipping, put back at the end). S = 94 × 55: the picture
   `sign.pcc` of MAIN.DAT at (0, 0); then the quote text of category 5 (SIG), record n (speech.md, no voice), as
   wrapped text (wrap 85, font 1, line height 12, centred; panel.md "Question box" text) twice: fg 0x29, shadow 0
   with the centre at x = 47, then fg 7, shadow 0 at x = 46; the first line at y = 14.
3. w = 4, h = 1. Repeat: if w < 94: w += 10; if h < 55: h += 6; a frame (w, h); a step wait; until h ≥ 55
   (9 frames, the last one at 94 × 55).
4. The end wait.
5. While h > 7: w −= 10, h −= 6, a frame (w, h), a step wait (8 frames).
6. B goes back to the screen at (x0, y0).
A **frame** (w, h): F = a copy of B; S scaled to w × h, then copied into F at ((94 − w) / 2, (55 − h) / 2) with all
pixels (the sign is not keyed); then the clipped copy of F (94 × 55) to (x0, y0).

Evidence: the right click 0x1E14F..0x1E166 (mode 3 / value 0x94 → 0x1D950 (tile, 0)); TacticalKeyHandler key 0x5A
(g_redrawOff ≠ 0 → 0x1D950 (mouse tile, 1)); 0x1D950 (TileItemIndex 0x23768, g_groundItems type + s8 +6,
TileObject 0x23760 = 0x94 → DrawSectorSign 0x27204 (tile, g_curSector, flag)); item popup 0x26DB0 (TileColPx /
TileRowPx, view 0x229CF8 / 0x229CFC + size 0x229CBC / 0x229CC0, TileScreenX 0x1FB28 / TileScreenY 0x1FB50, cursor
save [0x93E78], scratch [0x93E6C] pitch 228: B at 0, P at row 16, scaled at column 42, F at row 16 column 42;
picture 0x46750, FillRect 0x52AB4, DrawNumber 0x36090 (3, fg 0xFF, shadow 8, font 0, mode 300), step timer
0x92EF8 = 20, end wait WaitRightUp 0x67C68 / WaitKeyOrClick 0x67F34), frame 0x26C40 (ScaledBlit? 0x52B7C, BlitKey
0x522A0 key 0, BlitCopy 0x52AE0); sign 0x27204 (PCC loader 0x532BC MAIN.DAT "sign.pcc" into [0x93E6C] + 94,
PlayQuote 0x24F08 (0x40, 0, n, 200, buffer, 5, 100, 0), WrapText 0x3625C (fg 0x29 / 7, shadow 0, pitch 228, x 141
/ 140, y 14, wrap 85, centred, font 1, line height byte 0x949E8 + 5 = 12, encoded)), frame 0x270A0 (work buffer
[0x93E68] pitch 216).
Tests: validate/diff/tilepopup_diff.py (0x1D950 in Unicorn on tac60 vs ja1front ja1f_tile_popup).

## Debug keys (level 4)

The tactical key handler keeps a **debug level** L (one byte, 0 at program start, never saved, kept for the whole
program run). A player can raise it with four Ctrl keys typed in this order: Ctrl+B, Ctrl+A, Ctrl+L, Ctrl+D.

**Level keys** (each key works at every level; the key handler's usual gates apply: during time compression a key
only stops it, see turns.md):
- Ctrl+B: L = 1 when L = 0, else L = 0.
- Ctrl+A: L = 2 when L = 1, else L = 0.
- Ctrl+L: L = 3 when L = 2, else L = 0.
- Ctrl+D: L = 4 when L = 3, else L = 0 (so Ctrl+D at level 4 turns the debug keys off again).
No other key changes L. Nothing shows when L changes.

**Keys that work only at L = 4.** S = the record of `selected` (the key handler reads it even when selected ≥ 8),
T = the tile under the mouse (in the overview view mode the overview tile under the mouse). A message below =
the status message box (panel.md "Tactical messages", 25 000 ms); "%s%d" texts are the text then the number in
decimal.
1. **Ctrl+R (full clips)**: for each record k = 0..7 with +0x182 ≠ 0, +0x183 ≠ 0 and +0x138 ≠ 0: when its hand
   item type t (+0x00) is 1..14, the hand item's count +0x01 = item table entry t byte 5 (clip).
2. **Alt+L (heal)**: S +0x183 = S +0x184, S +0x185 = 0, S +0x186 = 0; Rest (S, amount = S fatigue (i16 +0x10C),
   redraw 1); S +0x1A5 = MercMaxAP (S); the bottom strip redraws S's portrait.
3. **K (kill)**: o = occupant (T); when o < 0x40: TakeDamage (record o, height 0x1356, damage 99, kind 1 bullet,
   attacker 0x40, tile 0xC41) (combat.md TakeDamage).
4. **@ (break)**: BreakObject (T, object = T's object byte t1, sound 1) (doors.md).
5. **Ctrl+T (teleport)**: S moves to T: S's old tile is vacated; S's tile = T; S's animation stops (StopAnim
   (S, 1)); S +0x120 = 0xC41; S x2 / y2 and x / y = the column / row coordinates of T (TileToX / TileToY); T gets
   S as its occupant; S's object / ground bytes under it and their classes (+0x1B1.. +0x1B7) are read again from T
   as at sector entry; ReplaceSoldier (S). Then TurnSightings (1). Then, when the world's turn flag 0x92DD0 ≠ 0:
   when selected < 8 the turn's "first merc" byte 0x284F0F = selected; NextTurn (1).
   No step code runs, so the tile triggers of a walk do not fire (port probe 2026-11-19, #61 c12761 slice 1, each
   case once by a walk and once by Ctrl+T): a set mine on T (smoke_play step_mine_step, 1371) does not spring and
   stays; a merc put on 977 does not find the story object on 1090 (step_story_find: the walk finds it); a merc put
   on P 0 tile 685 (quest_scenarios sapling_by_play save) gets no Santino scene until the next turn end (key D),
   where the walk to 685 starts it at once. The cheat bot's C5 rule: Ctrl+T only to a tile next to the goal, then
   real walk steps.
   **K** victims (3.) fall at hp 0 and die (MercDies, the sector's enemies byte − 1) only when their fall ends: on
   DAY_fight48d, 2 of 4 enemies killed with K in one turn were still falling after `wait 300` and dead (combat off,
   sector owned) after `wait 1500`.
6. **Ctrl+F1 (save anywhere)**: the save-anywhere byte flips (0 ↔ 1). While it is 1, Alt+S saves also with
   enemies near (savegame.md "Quick save keys").
7. **Alt+Q (roofs)**: the roofs switch g_roofsOn flips; message "Show Roofs  on" / "Show Roofs  off" (the text
   "Show Roofs " then " on" or " off").
8. **Alt+E (enemies)**: the "show all enemies" byte (FocusOn's test, 0x93458) flips; message "Enemies on" /
   "Enemies off".
9. **E**: message "overlay is N" with N = T's byte t2 (tile record byte 2), then a second message
   "overlay2 is M" with M = T's byte t3; the second box shows after the first one closed.
10. **F**: message "struc is N" with N = T's object byte t1.
11. **G**: message "gridno is T".
12. **Ctrl+P**: view focus on selected (FocusOn (selected, 0)), the screen redraws, message
    "CHARACTER SELECTED = n" with n = selected; then, when selected < 0x30, the debug pages of S.
13. **Ctrl+O**: o = occupant (T); when o < 0x30: the opponent lists of record o (three messages, see below).
14. **Shift + left click** on a tile whose occupant o < 0x30 (only at L = 4): wait until the left button is up,
    open the inventory screen of record o (any soldier, also an enemy), then the debug pages of record o; the
    click then ends (no other click step runs).

**Opponent lists (Ctrl+O)**, for record o (s = o's slot +0x17A): three or four status messages (the message box
above; each " %d" below is a space and the number, signed bytes):
1. "OPPCNT = n" with n = i8 o +0x1D5.
2. "PERSONAL:" then n entries " %d": i8 o +0x13A + i for i = 0 .. n − 1, n = 8 when s < 40, else 40.
3. "PUBLIC:" then " %d" of `g_known`[i] (i8 per slot, the world's `knowledge`) for i = 40 .. 47 when s < 40, else
   for i = 0 .. 39.
4. "SEEN:" then: s < 16: 8 entries " %d" of `remember_m`[8 s + i] (i = 0..7); 16 ≤ s < 40: no entry ("SEEN:"
   alone); s ≥ 40: 40 entries " %d" of `remember_e`[40 (s − 40) + i] (i = 0..39).
The four messages show one after the other (the next one after the box before it closed).

**Debug pages** (Ctrl+P, Shift+left click), for soldier record S: four pages, one after the other. Each page:
first the screen redraws (pages 1 and 2: the map view with a full redraw (TacticalRedraw (1)), whatever the view
mode; pages 3 and 4: the screen of the current view mode, as each frame draws it); then over that picture, row k
= 0..16 at y = 15 + 10 k: the label text at x 50, the text ":" at x C, the number at x N (all font 1, colour 0xFF,
shadow 8, not centred, mode 300; the number has at least 4 characters: on its left '^' (the font's blank glyph) as
needed, so "-5" → "^^-5"). Pages 1 and 2: C = 148, N = 165; pages 3 and 4: C = 118, N = 125. Then the game waits
for a key or a mouse click (that key or click does nothing else) and the next page shows; after page 4 the game
goes on. Draw order on a page: the 17 labels, the 17 ":" texts, the 17 numbers, k = 0 first.

Row values (i8 / u8 / i16 / u16 = S's field of that type at that offset, signed or not):

| k | page 1 label | value | page 2 label | value | page 3 label | value | page 4 label | value |
|---|---|---|---|---|---|---|---|---|
| 0 | active | u8 +0x182 | service | u8 +0x1B9 | guynum | u8 +0x17A | # opponents | i8 +0x1D5 |
| 1 | in sector | i8 +0x138 | serv grid | i16 +0xD6 | health | i8 +0x183 | cur oppnum | u8 +0x1D8 |
| 2 | dead And Removed | i8 +0x188 | serv cnt | i8 +0x1CA | breath | i8 +0x18A | `target   ` (3 spaces) | i16 +0xDC |
| 3 | visible | i8 +0x9C | busy Being Hit | i8 +0x195 | protect % | ArmorRating (S) (ai.md "Armour") | override | u8 +0x1C2 |
| 4 | stopped | u8 +0x1B8 | bypass To Green | u8 +0x131 | actionPts | i8 +0x1A5 | moved | i8 +0x1B0 |
| 5 | moved | i8 +0x1B0 | made Roll By | u8 +0x1FC | orders | u8 +0x1D4 | # enemies | u8 sector table byte "enemies" of the current sector |
| 6 | forced to Stop | i8 +0x197 | has Keys | i8 +0xB9 | attitude | u8 +0x1D3 | artillery | i8 +0x1C9 |
| 7 | new Situation | u8 +0x1C1 | gassed | u8 +0x1DB | reacheddest | AtWaypoint (S): 1 or 0 | anitype | u16 +0x1E8 + 2 × (u8 +0x1F3) |
| 8 | action | u8 +0x1D1 | underFire | i8 +0x1C4 | nextdest | i16 +0x12A | frame | S's script word (u16, tactical_map.md "Soldiers") |
| 9 | action Data | i16 +0x132 | in the Way | u8 +0x1D7 | desdest | i16 +0x120 | firing | u8 +0x1BF |
| 10 | action in Progr. | i8 +0x1D6 | where I Was | i16 +0x12E | nextact | u8 +0x1A6 | OLD oppnum | u8 +0x1F7 |
| 11 | action Timeout | u16 +0xF0 | tiles Moved | i8 +0x1F2 | actgrid | i16 +0x134 | action pts | i8 +0x1A5 |
| 12 | alert Status | i8 +0xBB | last Known Loc | i16 +0x108 | destination | i16 +0x124 | DestFirst | i8 +0x1F6 |
| 13 | noise Gridno | i16 +0x122 | just Arrived | i8 +0x1AB | destx | i16 +0x126 | desdir | i8 +0x1A2 |
| 14 | noise Volume | u8 +0x1C0 | --- | 0 | desty | i16 +0x128 | aiming | u8 +0x1BE |
| 15 | Pub Noise Gridno | i16 tile of the side's latest call (mercs' side `g_callM` when +0x17A < 40, else `g_callE`) | --- | 0 | x | i16 +0x116 | ammo | i8 +0x01 |
| 16 | Pub Noise Volume | u8 level of that call | --- | 0 | y | i16 +0x118 | oldgridno | i16 +0x1F8 |

AtWaypoint (S) = 0 when u8 +0x1B9 < 5, the animation word u16 +0x1E8 + 2 × (u8 +0x1F3) ≥ 4 and (i16 +0x114,
+0x116, +0x118) ≠ (+0x124, +0x126, +0x128) (any of the three); else 1.

Evidence: TacticalKeyHandler 0x13490: level byte 0x955EC (the high byte of the dword 0x955E9) 0x139D8..0x13A3E;
toupper (key) − 1 indexes the jump table 0x13328 (Ctrl+O 0x13AA6, Ctrl+P 0x13A43, Ctrl+R 0x13ADC, Ctrl+T 0x13C87,
@ 0x13BC9, E 0x13BF6, F 0x13C3A, G 0x13C65, K 0x13B3B); extended keys (first GetKey = 0): Alt+Q scan 0x10
0x1391A, Alt+E 0x12 0x138F5 (toggle + text 0x3F994: "on" 0x9A04F / "off" 0x9A069), Alt+L 0x26 0x1393F, Ctrl+F1
0x5E 0x139A1 (byte 0x92DC3); messages = MsgStrInt 0x27CE0 (sprintf "%s%d", same box as StatusMessage 0x27D24).
Teleport 0x3F8A4. Shift+left click: UseItemAt? 0x1DACE..0x1DB2C (inventory screen 0x54D50 (o, 0), then
DebugShowSoldierFlags 0x6BC28 (pages 1, 2) → DebugShowSoldier 0x6A964 (page 3) → DebugShowSoldierAI? 0x6B2BC (page 4);
DrawText 0x358D0, DrawNumber 0x36090, map redraw TacticalRedraw 0x176C8 (1), screen redraw 0x1CB78, wait 0x67F34).
Opponent lists 0x6CF88 (g_known 0x10D860, g_rememberE 0x10D890, g_rememberM 0x10D9D0). Rows and lists: dbgpage_diff.
