# Selecting a merc (library op 120)

The player (or the turn code) makes a merc the selected one: its selection light, its portrait, the cursor, a
queued merc quote and, for a merc with a walk order still open, the **path preview** (marks on the tiles of the
walk). Library op 120 (`low_ap_stop` in the op table; the host_ops bit of op 120 = bit 63): turns.md Mercs' turn
and soldier_update.md call it.

```c
int32_t ja1_select_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t force, int32_t quote);   /* op 120 */
int32_t ja1_select_soldier(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t show, int32_t quote);
int32_t ja1_path_ap_marks(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t mark,
                          int32_t extra, int32_t sneak, int32_t byte177, int32_t alt);
void    ja1_hide_path_marks(Ja1World *w, const Ja1Host *h, int32_t force);
```

`slot` is an int (0..0x2F); `force`, `show`, `quote`, `mark`, `extra`, `sneak`, `byte177`, `alt`: their low byte
(≠ 0 = set). Results: 1 = selected, 0 = not.

New world fields (port_api.md, "select"):

| field | type | meaning |
|---|---|---|
| `marks_shown` | i32 | ≠ 0: path marks are on the map |
| `marks_n` | i32 | number of tiles in `marks` |
| `marks` | 256 bytes | 128 × u16: the marked tiles, in order |
| `opt_path_marks` | u8 | an option: ≠ 0 = the path preview draws its marks |
| `sees_slot` | u8 | a merc whose "sees enemies" message waits (0x40 = none) |
| `preview_slot` | u8 | the merc whose next selection shows its path preview (0x40 = none) |

Existing fields used: `screen_drawn`, `selected`, `shown_slot`, `opt_onscreen`, `view_hold`, `view_y`, `combat`,
`quote_slot`, `quote_delay`, `path_preview` (> 0: slot + 1 of the merc whose preview stays on), `spot_stop`,
`walk_ap`, `merc_shown`, `tiles`, `paths`, `move_codes`.

The path marks live in byte 7 of each tile (`tiles`, 8 bytes per tile): the high nibble is the direction the
walk comes in by, the low nibble the direction it leaves by, each as direction + 1 (1..8); a high nibble 0xA marks
a tile the walk crosses on a special ground (byte 0 ≥ 0x9A, byte 1 = 0). The front-end draws them; 0 = no mark.

New host ops (none of them reads the world unless it says so):

| op | name | arguments | result |
|---|---|---|---|
| 195 | select_light | byte slot, int kind | none (the front-end draws the soldier's selection light: kind 5 = off, 0xC = on) |
| 196 | panel_view | int view_y | the new view_y (i32), below |
| 197 | sees_msg | slot | none (the "… sees N enemies!" message) |
| 198 | mouse_wait | (none) | none (the front-end waits until the left mouse button is up) |
| 199 | cursor_state | slot | none (the front-end updates its cursor for the soldier; it first hides the path marks: Hide path marks (0)) |
| 200 | cursor_select | (none) | none (the front-end's cursor goes to "a merc is selected": state 1, cursor 0x8E) |

Existing host ops used: 55 ui_refresh (force: the front-end calls Hide path marks (force)), 65 effect_slot
(slot, 0), 99 focus, 149 portrait_refresh (byte slot, int show), 163 cursor_reset, 166 item_panel (slot, 0, 1).
Library ops used: 58 reserve_ap. Library procedures used: Merc quote (soldier_update.md), On screen (ai.md
helpers), Find path (op 41), "AP to enter a tile" (combat.md), "Step from a tile" (ai.md). Path AP with marks
(below) is always the library's own procedure here, never host op 40, and the Find path inside it is the
library's own too, whatever the host_ops bit of op 41 says.

**Panel view (op 196).** In view mode 3 (`opt_onscreen` = 3) the selection moves the front-end's panel: its view
window becomes 20 columns × 13 rows (182 pixels high) and its panel timer starts at 3000. Then, if the mouse
pointer's y ≥ 182 and view_y + 14 < 0x310 − 182: the view scrolls 14 pixels down (result view_y + 14) and the
mouse pointer moves 14 pixels up. Else the result is view_y. How the panel counts down and goes away: panel.md "The small panel in the
full view".

## Select merc (`ja1_select_merc`, op 120)

s = record slot.

1. If screen_drawn = 0: result 0.
2. If s8 +0x183 ≤ 0: result 0.
3. If view_hold = 0 and (force ≠ 0, or (slot = shown_slot or opt_onscreen = 0) and On screen (s)):
   1. r = Select soldier (slot, 1, quote).
   2. Host op 199 cursor_state (slot).
   3. If r = 0: result 0.
   4. If quote_slot < 0x40: quote_delay = 0; Merc quote (slot).
   5. If sees_slot < 0x40 and +0x17A < 8: host op 197 sees_msg (slot).
   6. If path_preview ≠ 0: host op 55 ui_refresh (1).
   7. t = s16 +0x120. If t ≠ NONE and u16 +0x120 ≠ u16 +0x114 and +0x16D ≠ 0:
      If preview_slot = slot or spot_stop ≠ 0: m = 1 and spot_stop = 0 (in both cases). Else m = 0.
      Path AP with marks (slot, t, mark m, extra m, sneak s8 +0x1A8, byte177 s8 +0x177, alt s8 +0x196).
      If s8 +0x1A5 < walk_ap (signed): host op 55 ui_refresh (the low byte of path_preview, sign-extended);
      +0x16D = 0.
   8. Result r.
4. If view_hold ≠ 0: result 0.
5. Host op 149 portrait_refresh (slot, 1); host op 99 focus (slot, 0).
6. If combat ≠ 0 and +0x16D = 0 and t = s16 +0x120 ≠ NONE and u16 +0x120 ≠ u16 +0x114: +0x16D = 1; Path AP with
   marks (slot, t, 1, 1, s8 +0x1A8, s8 +0x177, s8 +0x196); if s8 +0x1A5 < walk_ap (signed): host op 55
   ui_refresh (the low byte of path_preview, sign-extended); merc_shown = 0x40; +0x16D = 0.
7. Host op 198 mouse_wait (). Result 0.

The comparison slot = shown_slot is on the whole int; preview_slot = slot compares the byte with the int.

## Select soldier (`ja1_select_soldier`)

k = low byte of slot; s = record k; sh = the low byte of show, sign-extended to 32 bits.

1. If k > 7 or +0x182 = 0 or +0x17E ≠ 0: result 0.
2. o = selected. If k ≠ o:
   1. If o < 8: record o +0x1BD = 0x6A; host op 195 select_light (o, 5); selected = 0x40; host op 149
      portrait_refresh (o, sh).
   2. selected = k; +0x1BD = 0x6E; host op 195 select_light (k, 0xC).
   3. If s8 +0x183 ≥ 15: host op 200 cursor_select (); then, only if quote ≠ 0, host op 65 effect_slot (k, 0);
      then (whatever quote is) op 58 reserve_ap (k). Else: host op 163 cursor_reset ().
   4. Host op 149 portrait_refresh (k, sh).
3. If show ≠ 0: if opt_onscreen = 3: view_y = host op 196 panel_view (view_y). Host op 166 item_panel (k, 0, 1).
4. Result 1.

## Path AP with marks (`ja1_path_ap_marks`)

Path AP (soldier_update.md, op 40) with two more arguments: `mark` (draw the path preview) and `extra` (price one
step more). Path AP (slot, tile, sneak, byte177, alt) = Path AP with marks (slot, tile, 0, 0, sneak, byte177,
alt). M = mark ≠ 0 and opt_path_marks ≠ 0.

1. If mark ≠ 0 and marks_shown ≠ 0: result 0 (nothing is written, walk_ap keeps its value).
2. walk_ap = 0; c = 0; up = 0.
3. If Find path (slot, tile, 1) ≠ 0: n = s16 +0xD4 (+ 1 if extra ≠ 0); t = s16 +0x114; p = 80 × slot. For k = 0
   .. n − 1:
   1. d = (paths[p + k] + 1) & 0xFF; t = step(t, dirStep(d)); c = c + "AP to enter a tile" for move_codes[t]
      (as in Path AP). (With extra, step n − 1 reads the direction byte just after the stored ones.)
   2. If k = 0: walk_ap = c. If M and byte 0 of tile t < 0x9A: u16 marks[0] = u16 +0x114; byte 7 of tile s16
      +0x114 = (byte 7 & 0xF0) | (d & 0xF); marks_n = 1. (Else marks and marks_n keep their values.)
   3. If M and k < n − 1 (signed): marks_n = marks_n + 1; u16 marks[marks_n − 1] = t. b = byte 7 of tile t. If byte 0 of
      tile t ≥ 0x9A and byte 1 of tile t = 0: b = (b & 0xF) | 0xA0. Else: b = ((d & 0xF) << 4) | (b & 0xF); then
      o = d if extra ≠ 0 and k ≥ n − 2, else paths[p + k + 1] + 1; b = (b & 0xF0) | (o & 0xF). Byte 7 of tile t
      = b.
   Then up = the getting-up cost as in Path AP.
4. walk_ap = walk_ap + up.
5. If mark ≠ 0 (also when no path was found): marks_shown = 1; if extra ≠ 0 too: path_preview = slot + 1.
6. Result c + up (32-bit).

A walk has at most 80 steps, so with marks_n starting at 0 or 1 the list stays inside its 128 entries.

So the start tile has only a "leaves by" mark, the tiles of steps 0 .. n − 2 get both, and without extra the
destination tile gets none; with extra the destination's "leaves by" is its own "comes in by".

## Hide path marks (`ja1_hide_path_marks`)

The front-end calls it for host ops 55 and 199 (op 199 with force 0). force: its low byte.

1. If marks_shown = 0: done.
2. If path_preview > 0 (signed) and force = 0: done.
3. path_preview = 0; marks_shown = 0.
4. If opt_path_marks ≠ 0: for i = 0 .. marks_n − 1: byte 7 of tile s16 marks[i] = 0; marks_n = 0; all 256 bytes
   of marks = 0.

(The original also clears its AP number text and its cursor tile here: those are the front-end's.)

## Tests

`validate/diff/ai_diff.py --only select_merc --port` runs Select merc and Select soldier against the original on
fuzzed worlds (Path AP with marks and Find path real on both sides; the test host answers op 200 and op 196 by
writing the original's cursor bytes, panel timer and mouse position). `--only path_ap --port` runs Path AP with
marks (ja1_path_ap_marks) with mark and extra fuzzed. `validate/diff/walk_diff.py --fight` plays a whole fight; the
port selects the mercs itself (op 120 a library op).

*Evidence (2026-10-22): mirrors tools/ai.py select_merc, select_soldier, path_ap_cost, hide_path_marks (ai_diff
select_merc, path_ap, hide_path_marks).*
