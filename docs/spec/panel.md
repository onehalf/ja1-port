# Merc panel (tactical view)

The tactical screen shows the 8 merc slots of the player's team at the sides of the map: 4 slots in the left column,
4 in the right column. Each slot (48 x 42 pixels) shows the merc's face, name, three bars (health, energy, action
points), and some marks (tint, icon, AP number, assignment text, frames). This page says how one slot is drawn, from
which game files, with which numbers. The front-end draws it; the only game-state write is the eye state (+0x1AC).

Verified: validate/diff/panel_diff.py (the original slot drawing in Unicorn vs the analysis mirror: 5 dumps,
600 fuzzed cases, 0 mismatches) and validate/diff/panel_data_diff.py (the data below, built from the game files,
vs 132 memory dumps of the original: 0 mismatches).

## Buffers and data

- **Panel buffer**: 320 x 200 bytes, pitch 320 (a full-screen layer the slots are drawn into). Slot k has its
  top-left corner at the **slot origin** (X, Y): slots 0..3 = (0, 14), (0, 56), (0, 98), (0, 140); slots 4..7 =
  (272, 14), (272, 56), (272, 98), (272, 140).
- **Panel sheet**: MAIN.DAT member `PANELS.PCC` (PCX, 320 x 160), kept at pitch 320. An **element** is a rectangle of
  the sheet (x, y, w, h). Elements used here:

  | element | x, y | w x h | use |
  |---|---|---|---|
  | 0 / 7 | (0, 0) / (0, 42) | 48 x 42 | slot background (7 = selected merc) |
  | 4 / 11 | (229, 0) / (209, 29) | 11 x 27 | bar frame (11 = selected merc) |
  | 17 | (242, 2) | 28 x 25 | face tint mask |
  | 80, 81 | (105, 117), (105, 116) | 3 x 1 | health bar: body row, top row |
  | 82, 83 | (108, 117), (108, 116) | 3 x 1 | energy bar: body row, top row |
  | 84, 85 | (111, 117), (111, 116) | 3 x 1 | AP bar: body row, top row |
  | 86 | (155, 95) | 3 x 1 | health bar: empty row |
  | 87 | (155, 96) | 3 x 1 | health bar: wound row |
  | 88, 89, 90 | (230, 1), (233, 1), (236, 1) | 3 x 25 | empty part of the health / energy / AP bar |
  | 91, 92, 93 | (210, 30), (213, 30), (216, 30) | 3 x 25 | the same for the selected merc |
  | 104 | (242, 2) | 24 x 22 | shade mask |
  | 117 (0x75) | (300, 133) | 16 x 16 | the guard screen's picked number (strategic.md "The guard screen") |
  | 120 (0x78) | (198, 147) | 37 x 12 | strip button face (interface strips, keep = 1) |

  Frame lines take their pixels from the sheet row at (78, 159) (horizontal) and the sheet column at (319, 0)
  (vertical, downwards).
- **Face buffer** per slot: 32 x 32 bytes, pitch 32. Rows 2..26, columns 2..29 hold the face (28 x 25); rows 28..29
  and 30..31 (columns 0..27) hold two eye strips. Filled when a game is loaded (0x2884C; a record with `inCurSector` = 0 gets a cleared buffer) and when a merc joins (face number n = the merc's
  `profileId` +0x179):
  - alive (hp > 0): MAIN.DAT `FACES.PCX` (320 x 200) holds 11 faces per row in cells of 28 x 29: cell n starts
    at (28 (n mod 11), 29 (n div 11)). Cell rows 0..24 go to face buffer (2, 2); cell rows 25..28 go to buffer
    rows 28..31 at column 0. The merc's **eye row** +0x1AA = byte n of CD `DAT/JAGGED.DAT`.
  - dead (hp ≤ 0): MAIN.DAT `DEADFACE.PCC` (28 x 25) to (2, 2).
  - when a merc dies in the sector, the death morph `MORF<n>.PCC` (CD `DAT/MF1.DAT` for n < 30, else `DAT/MF2.DAT`;
    10 frames of 28 x 25 side by side) plays in the face: frame i (x = 28 i) goes to (2, 2), so the buffer keeps
    frame 9.
- **Remap tables**: 34 tables of 256 bytes from CD `DAT/JAGGED.DAT` offset 0x3470 (table k at 0x3470 + 256 k).
  After loading, byte 0x1B of table 29 is set to 0xC6. Each sector load (after its tileset table, table 28, is
  built) sets table 29 bytes 0x27 = 0x2B and 0xF1 = 0xF2. Here **table 0** is the drawing table (the slot code
  changes some of its bytes for a moment, see below) and **table 29** is the darken table.
- **Fonts**: three raw sheets on the hard disk: font 0 = `JA_SMALL.FNT` (pitch 442, glyph height 7), font 1 =
  `JA_M_ENG.FNT` (pitch 659, height 6), font 2 = `JA_BIG.FNT` (pitch 789, height 11). Each font has 131 glyphs for
  the codes 0x20..0xA2. Glyph c starts at x = the sum of the widths of glyphs 0..c-1, row 0. Widths (glyph 0 first):

  ```
  font 0:
    4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4
    4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4
    4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 0 0 0 0 0 0 0 0
    0 0 0 0 0 0 0 0 0 0 0 0 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4 4
  font 1:
    5 6 6 6 6 6 7 5 4 4 6 6 6 5 5 6 6 6 6 6 6 6 6 6 6 6 5 6 6 6 6 6 6
    6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6
    6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 0 0 0 0 0 0 0 0
    0 0 0 0 0 0 0 0 0 0 0 0 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6 6
  font 2:
    3 3 5 8 6 7 6 3 4 4 7 6 3 5 3 6 6 6 6 6 6 6 6 6 6 6 3 3 5 6 5 6 6
    6 6 6 6 5 5 6 6 3 6 6 5 8 6 6 6 6 6 6 7 6 6 9 6 7 6 9 6 5 6 6 3 6
    6 6 6 6 5 6 6 3 5 6 3 9 6 6 6 6 5 6 5 6 6 9 6 6 5 9 7 6 6 7 7 9 8
    7 9 10 6 9 8 7 10 7 7 9 6 6 6 6 6 6 6 7 6 6 6 6 4 6 6 6 6 6 4 6 5
  ```
- **Side offsets**: per column (soldier `+0x180`: 0 = left, 1 = right), five signed (dx, dy) pairs from the slot
  origin: face (14, 3) / (3, 3); pair 1 (16, 5) / (5, 5) (not used here); health bar (3, 4) / (35, 4); energy bar
  (7, 5) / (39, 5); AP bar (10, 5) / (42, 5).
- **Dying tint colours** (by `+0x189` = 0, 1, 2, 3): 112, 118, 117, 116.
- **Assignment texts** (by `+0x17E` = 0..7): `ON DUTY`, `REST`, `TRAIN`, `DOCTOR`, `PATIENT`, `REPAIR`, `FIRE`,
  `ESCORT` (index 0 is never drawn).

## Inputs

Soldier record fields (layout "soldier" in defs/types.toml; all bytes, "signed" where stated): `slot` +0x17A,
`nickname` +0x16E, column +0x180, icon +0x181, `inCurSector` +0x182, `hp` +0x183 (signed), `hpMax` +0x184 (signed),
`wounds` +0x185 (signed), dying state +0x189 (signed), `energy` +0x18A (signed), `apLeft` +0x1A5 (signed),
`reservedAp` +0xEF (signed), `inPlay` +0x138, assignment +0x17E, whole-slot frame +0x17F, frame colour flag +0x1A8,
`eyeRow` +0x1AA, eye state +0x1AC (signed), shaded flags +0x194 and +0x1BB, and for the frame test `tile` +0x114,
+0x116, +0x118, +0x120 (signed 16-bit), +0x124, +0x126, +0x128, +0x1B9, `anim` +0x1E8, `animIdx` +0x1F3.

World and front-end values: `selected` (the selected merc's slot), `combat`, `opt_onscreen` (the panel draws only
while it is below 2), the **speaker slot** (the merc whose speech balloon is up; 0xFF none), the **hover slot**
(the slot whose bar area, 11 x 27 at the health-bar offset, is under the mouse; a click there opens the inventory popup; 0xFF none), and a per-slot **AP warned**
flag (set when the low-AP stop happened this turn).

## Drawing primitives

All drawing goes into the panel buffer (pitch 320). Numbers are signed 32-bit; "/" truncates toward zero.

- **Element** e at (x, y), optional row count r: copy the element's w x (r or h) pixels from the sheet; a sheet
  pixel 0 is not written.
- **Bar rows** of element e at (x, y), n rows: copy the 3 pixels at the element's (x, y) in the sheet to rows
  y .. y + n - 1 (all pixels written, also 0).
- **Remapped draw** of element e with table T: for each sheet pixel p, v = T[p]; v is written unless v = 0.
- **Text** (string s, x, y, centred, fg, shadow, font, mode 300): if centred, x -= (sum of the widths of the
  glyphs s[i] - 0x20) / 2. Font 0: y -= 1. Save table 0 bytes 0xFF and 0xFE, set them to fg and shadow. For each
  byte b of s: c = b - 0x20; if c < 0 or c ≥ 131, stop at once (the two table bytes then stay changed). w = width;
  if x + w > 320 (unsigned) then w = 320 - x. Draw the glyph's w x height pixels at (x, y): if shadow ≠ 0, a glyph
  pixel p ≠ 0 is written as table0[p]; if shadow = 0, v = table0[p] is written unless v = 0. x += w; if x > 319,
  stop. At the end restore the two table bytes. A glyph with w = 0 (fonts 0 and 1: the codes 0x7B..0x8E) is a
  **stop point** (port_api.md "Division by zero"): the original's column copy counts down and stops at 0, so it
  runs 2^32 columns and crashes (strategic.md, Delete in the save name).
  **Text with a background** (mode b ≠ 300; the map panel uses b = 13 and 56): the same, but each glyph is drawn with
  table 0 byte 0 = b: every glyph pixel p (also 0) gives v = table0[p], written unless v = 0; after each glyph table 0
  byte 0 = 0 (not the old value). So the glyph box gets colour b where the glyph has no pixel, and the shadow test
  is not made.
- **Number** v with d digits (d = 2 unless stated): the decimal text of v (a `-` first when v < 0), padded on the
  left with `^` to d characters (a longer text is not cut), drawn as text (not centred).
- **Frame** (x0, y0, x1, y1, colour): table 0 byte 1 = colour. Using table 0 (written unless the result is 0):
  top line from (x0, y0), x1 - x0 pixels long; bottom line from (x0, y1), x1 - x0 + 1 long (both from the sheet
  row at (78, 159), offset 0xC70E: 194 pixels of 1, then 0); left column from (x0, y0) and right column from (x1, y0), each y1 - y0 high (from the sheet
  column at (319, 0)). Then table 0 byte 1 = 1 (not the old value). Frames wider than 194 or higher than 83 are
  drawn in two parts: when x1 - x0 > 194, ew = (x1 - x0 - 194) as a signed 8-bit number and x1 = x0 + 194; when
  y1 - y0 > 83, eh = (y1 - y0 - 83) as a signed 8-bit number and y1 = y0 + 83 (else ew, eh = 0). The order: top line
  (x0, y0) x1 - x0 long; bottom line (x0, y1 + eh) x1 - x0 + 1 long; if ew > 0: top line (x1, y0) ew long and bottom
  line (x1, y1 + eh) ew + 1 long (each line again from the start of the sheet row); left column (x0, y0) and right
  column (x1 + ew, y0), y1 - y0 high; if eh > 0: columns (x0, y1) and (x1 + ew, y1), eh high. Evidence: DrawFrame
  0x46C44, tools/panel.py frame; msgbox_diff.py (boxes 206 + 2 wide).
- **Tint** at (x, y) with colour c: save table 0 byte 0x1B, set it to c, draw element 17 remapped with table 0 at
  (x + 2, y + 2), restore the byte.
- **Darken** at (x, y): every pixel p of the 28 x 25 box at (x + 2, y + 2) becomes table29[p].

## Drawing one slot

Nothing is drawn unless `opt_onscreen` < 2 and the slot is 0..7. Let s = the merc in the slot, (X, Y) = the slot
origin, k = 7 when the slot is `selected` (else 0), and (fx, fy) = (X, Y) + the face offset of s's column.

1. Element k at (X, Y). Element k + 4 at (X, Y) + the health-bar offset.
2. **Name**: the nickname with its first byte + 0x20 (mod 256), font 1, at (X + 3, Y + 32), not centred; colours
   fg 0x16 / shadow 0x15 for the selected merc, else 0x0E / 0xFE.
3. If `inCurSector`: the health bar, the energy bar and the AP bar (below).
4. If +0x194 = 0 or the slot is not the speaker slot: the **portrait** (below).
5. If +0x194 ≠ 0 or +0x1BB ≠ 0: **shade**: save table 0 byte 0x1B, set it to 0x70, draw element 104 remapped
   with table 0 at (X, Y), (X + 24, Y), (X, Y + 20) and (X + 24, Y + 20), restore the byte. Then, if the slot is
   the speaker slot, the portrait again.

**Health bar** (only when `opt_onscreen` < 2 and `inCurSector`): kh = 3 for the selected merc, else 0.
(x, y) = (X, Y) + the health-bar offset + (1, 1). gap = (100 - hpMax) / 4. nh = 0 if hp = 0, else (hp - 1) / 4 + 1.
nw = 0 if wounds = 0, else (wounds - 1) / 4 + 1. If (100 - hpMax) rem 4 < (100 - (hpMax - hp)) rem 4 (C remainder,
sign of the dividend) and nh > 1: nh -= 1. rest = 25 - gap - nh - nw.
If gap > 0: element 88 + kh at (x, y) with gap rows. y += gap. If nw > 0: nw bar rows of element 87 at (x, y).
y += nw. If rest > 0: rest bar rows of element 86. y += rest (also when rest is negative). If nh > 0: element 81 at
(x, y), then nh - 1 bar rows of element 80 at (x, y + 1).

**Energy bar** (only when `inPlay`): v = energy / 4; if v = 0 and energy > 0, v = 1. Then the **bar shape** with
empty element 89 + kh, top element 83, body element 82, at (X, Y) + the energy-bar offset.

**AP bar** (only when `opt_onscreen` < 2 and `inPlay`): v = min(apLeft, 25) (apLeft is 0..127 in the game; a negative value would read past the sheet). Bar shape with empty element
90 + kh, top element 85, body element 84, at (X, Y) + the AP-bar offset.

**Bar shape** (v, at (x, y)): gap = 25 - v. If gap > 0: the empty element at (x, y) with gap rows. If v > 0:
y += gap; the top element at (x, y); if v > 1: v - 1 bar rows of the body element at (x, y + 1).

**Portrait** (only when `opt_onscreen` ≤ 1 and the slot ≤ 7):
1. Copy the face buffer's top-left 31 x 28 pixels to (fx, fy) (all pixels).
2. Eye state: if the dying state ≠ 0, +0x1AC = 0; else if apLeft = 0 and hp ≥ 15, +0x1AC = 3. Let e = +0x1AC. If
   e < 2: copy 28 x 2 pixels from face buffer row 28 + 2 e, column 0, to (fx + 2, fy + 2 + eyeRow).
3. Tint: if the dying state ≠ 0 or hp < 15: tint with the dying tint colour of the dying state; else if
   `inCurSector` and not `inPlay`: tint with colour 2.
4. Icon: if +0x181 ≠ 0: draw tile sprite 413 + icon of the sector's tileset (tactical_map.md) at (fx + 2, fy + 13).
   Icons 0xB7..0xB9 are the first-aid marks.
5. AP number: if hp ≥ 15, `combat` ≠ 0 and `inPlay`: a = apLeft. If a < reservedAp: fg 0x72, shadow 1; else fg =
   0x16 if the slot's AP-warned flag is set, else 0xFF, shadow 0xFE. If fg ≠ 0xFF, first fill a black (0) box of
   7 rows: 5 wide at (fx + 25, fy + 20) when a < 10, else 8 wide at (fx + 22, fy + 20). Then the number a with
   font 0 at (fx + 22, fy + 21).
6. If the assignment ≠ 0 or apLeft = 0: darken at (fx, fy).
7. If the assignment is 1..7: its text, centred, font 0, at (fx + 16, fy + 12), fg 0x27, shadow 1.
8. **Frames** (only when `inCurSector` and `inPlay`): if the slot is the speaker slot: face frame with colour 0xFF,
   and nothing more in this step. Else, if s is not at its waypoint, or +0x120 < 0xC41: the slot frame.
   - at its waypoint: false only when +0x1B9 < 5, the animation word anim[2 animIdx] (u16) ≥ 4, and one of the
     pairs (+0x114, +0x124), (+0x116, +0x126), (+0x118, +0x128) (u16) differs.
   - slot frame (only when `opt_onscreen` < 2): colour 0xA2 if +0x1A8 = 0, else 0x16. If +0x17F = 0: face frame;
     else frame (X + 1, Y + 1, X + 46, Y + 40).
   - face frame: frame (fx + 1, fy + 1, fx + 30, fy + 27).
9. If the slot is the hover slot and hp > 0: frame (bx, by, bx + 10, by + 26) with colour 0x27, (bx, by) =
   (X, Y) + the health-bar offset.

Evidence: REVERSE.md "Merc panel" (DrawMercSlot? and its callees, the loaders LoadPanels, LoadMercFace,
LoadSmallFont, LoadJaggedDat, LoadTerrainTileset, MercDies?).

## Stat popup (left button held on a bar area)

While the left mouse button is held on the bar area of slot k, the game shows a small picture with 10 numbers of
merc k next to the slot (display only; nothing in the game state changes).

1. Load the picture POPINV.PCC (MAIN.DAT, 100 x 42, PCX as the faces) into the panel buffer at (173, 18). The
   loader's result (0 = loaded) goes to a scratch variable of the game (no other use).
2. For n = 0..9, with (dx, dy) from the table below, (x, y) = (173 + dx, 18 + dy) and v = value n of merc k:
   draw the number v with 4 digits (the decimal text of v, padded on the left with `^` to 4 characters; a
   longer text is drawn whole) in the panel buffer at (x + 1, y + 1) with fg 7 (the shadow pass), then at (x, y)
   with fg = the colour of the table; both with shadow 0, font 0, mode 300 (Text in Drawing primitives).
3. Copy the 100 x 42 block at (173, 18) of the panel buffer to the screen at (slot x + 48, slot y) for slots 0..3,
   at (slot x - 100, slot y) for slots 4..7 (slot origin from Buffers and data).

| n | value | dx, dy | colour |
|---|---|---|---|
| 0 | +0x112 (u16) | 77, 32 | 237 |
| 1 | hp +0x183 (signed byte) | 28, 32 | 237 (the table gives 41, but the game takes the colour of n = 0) |
| 2 | +0x18B (signed byte) | 28, 4 | 237 |
| 3 | +0x18C | 28, 11 | 41 |
| 4 | +0x18D | 28, 18 | 237 |
| 5 | +0x18E | 28, 25 | 41 |
| 6 | +0x18F | 72, 4 | 237 |
| 7 | +0x190 | 72, 11 | 41 |
| 8 | +0x191 | 72, 18 | 237 |
| 9 | +0x192 | 72, 25 | 41 |

Evidence: tools/popup.py (mirror of DrawPopupInventory? 0x3FF3C, 0x3FE50, the value switch 0x47350; tables
0x93FB0 positions, 0x93F10 colours), validate/diff/popup_diff.py: 200 cases on 5 dumps (fuzzed values incl.
negative and 5-digit, noise in the panel buffer), panel buffer + screen + loader result compared, 0 mismatches;
4 hand mutants caught.

**Key S: all stat popups.** Key S (or s) on the tactical screen shows the stat popups of all mercs at once:

1. Lock with clock switch 1 ("Other users of Lock").
2. For each slot k = 0..7 in order: when merc k is in the current sector (+0x182 ≠ 0) and on the map
   (+0x138 ≠ 0), draw its stat popup (steps 1 to 3 above; the hp is not checked, unlike the held button).
3. The pointer hide flag 0x284F13 = 0, show the pointer.
4. Wait until a key is pressed, or a mouse button goes down (the wait then also waits until it comes up). The key
   or the click is used up: it has no other effect.
5. Unlock. Nothing puts the screen back: the popups lie over the map view and the slots, and the next draw of
   those parts covers them.

Front-end: ja1front's window shows the popups until a key or a button release and runs that time as locked
ticks (clock switch 1); the script line `statkey MS` = key S held for MS ms (stdout `stat popups [k, ...]`;
JA1_STATS_PNG = the screen with the popups). Smoke step stat_key.

Evidence: TacticalKeyHandler 0x13490, case 0x53 at 0x13E9E (the key goes through the upper-case step 0x754D7
first): Lock? 0x4B730 (1), FUN_00040024 0x40024 (the loop over the slot table 0x100958, DrawPopupInventory?
0x3FF3C, 0x284F13 = 0, ShowMouse, wait loop 0x67F34 = MousePoll? 0x678E8 + key-or-click test 0x67E44), Unlock?
0x4B764.

## The switch view (right button on a slot)

Each merc has three **switches**, bytes of its record: **max aim** +0x16A (Reserve AP adds 4 AP for the aim,
sector_start.md "Reserve AP"), **rsv pts** +0x16B (the low-AP stop, soldier_update.md) and **mute** +0x16C (no
merc sounds, sound.md op 65). The **view byte** +0x17F of a merc is 0 for the normal slot and 1 for the switch view.

**Elements** (panel sheet x, y, w, h): 1 (48, 0, 48, 42), 2 (96, 0, 48, 42), 3 (144, 0, 48, 42), 8 (48, 42, 48, 42),
9 (96, 42, 48, 42), 10 (144, 42, 48, 42); switch faces 0x45 (155, 84, 9, 8), 0x46 (164, 84), 0x47 (173, 84),
0x48 (182, 84), 0x49 (172, 110), 0x4A (181, 110), 0x4B (190, 110), 0x4C (199, 110), all 9 x 8.

**Drawing the switch view** of slot k (only when `opt_onscreen` < 2 and k ≤ 7; s = record k, (X, Y) = the slot
origin, sel = k is `selected`):
1. Element 7 (sel) or 0 at (X, Y).
2. For i = 0..2, y = Y + 5 + 9 i: the label i (`max aim`, `rsv pts`, `mute`) with font 0 at (X + 3, y), not
   centred, fg 0x1E / shadow 0x15 (sel) or fg 0x0D / shadow 0x07; then the **switch face** i: element 0x48 when
   byte +0x16A + i ≠ 0, else 0x45, plus 4 when sel, at (X + 0x23, y − 1).
3. The name ("Drawing one slot" step 2), then the frames ("Drawing one slot" step 8, with the face position of
   s's column; the view byte is 1, so the slot frame is the whole-slot frame).

A **pressed switch face** i with the new value v: element 0x46 + (v ≠ 0), plus 4 when sel, at (X + 0x23, Y + 5 +
9 i − 1). An **open / close frame** e: element e (+7 when sel) at (X, Y).

**Opening and using it.** In the tactical key handler, while `opt_onscreen` < 2 and the mercs' input is on, for
each slot k under the mouse: if merc k is in the sector (+0x182), not shaded (+0x194 = 0) and on the map (+0x138),
and the left button is not down, a right button press with +0x17E (assignment) = 0 and hp ≠ 0:
1. Lock (clock switch 1); the tactical screen is drawn; view byte = 1.
2. The open frames e = 1, 2, 3: each drawn into the slot and copied to the screen, then a wait of 20 ms. Then the
   switch view, copied to the screen. Wait until the right button is up.
3. The mouse stays inside (X + 2, Y) .. (X + 0x2F, Y + 0x29).
4. Loop: when the left button is down, for i = 0..2 with the mouse in the switch rectangle (X + 0x23, Y + 5 + 9 i)
   .. (X + 0x2B, Y + 10 + 9 i) (edges included): byte +0x16A + i ^= 1; the pressed face with the new value; effect
   0 (IN) at rate 11025, volume 0x32, once (op 60 rules); the slot to the screen; wait until the left button is up;
   effect 1 (OUT), same rate and volume; for i ≤ 1 Reserve AP of s; the switch view again, copied to the screen.
   Else, when the right button is down: wait until it is up and leave the loop. Keys do nothing meanwhile.
5. View byte = 0; the close frames e = 3, 2, 1 as in step 2; then "Drawing one slot" of slot k, copied.
6. The mouse may go anywhere again; Unlock.

While the view byte is 1 every slot redraw (the interface strips step 5, the inventory screen's slot redraw)
draws the switch view instead of the slot; a view byte above 1 draws nothing there.

Evidence: 0x11714 (the slot mouse routine of TacticalKeyHandler 0x1468D / 0x146D3: right branch 0x11B37..0x11D34),
0x332E4 (view byte + open / close frames, busy delay 0x67250(20)), 0x330C0 (drawing), 0x33018 (labels 0x965BE),
0x32FFC (pressed face), 0x35858 (label colours). validate/diff/panel_diff.py --port --switch: the drawing, the
open / close frames and the pressed faces vs the original (0 mismatches); strips_diff / invdraw_diff --port give
some mercs view byte 1 or 2.

## Message box

The game shows a short message (for example "This sector is in enemy hands!" on the island map) in a dark box in
the middle of the screen and waits. Steps, with the message text s (a zero-ended byte string):

1. Save the screen rectangle x 52 .. 267, y 37 .. 116 (216 x 80 pixels).
2. Draw the box (below) on the screen with darkening table D and frame colour F: on the strategic screen D = 32
   and F = 0x68 (the table index in use there is 32). In general F = 0xFF when D = 31, else 0x68.
3. Wait until a key is pressed, or a mouse button goes down (a button that is already down when the box shows
   counts only after it comes up and goes down again; the wait then also waits for that button to come up), or
   25 000 ms pass. The key or the click is used up: it has no other effect.
4. Put the saved rectangle back on the screen. Pixels of the box outside the rectangle (a message of 5 lines or
   more) stay on the screen until the next full draw.

**The box.** Font 2, text colour 0xFF, shadow colour 8, line height 11, wrap width 189.

- Width: t = the sum of the font 2 widths of all bytes of s (byte b: width index b - 0x20). The box is W = t + 16
  pixels wide when t ≤ 190, else W = 206. (Width index 131 and up lies past the font's table: the byte
  0xB1 counts 0 there; other such bytes are not described.) Its left column is x0 = (320 - W) / 2 (the box is centred).
- Lines: take the words of s one by one (a word = the bytes up to the next space or the end; spaces before a word
  are skipped). The word width = the sum of its byte widths, but a byte 0xB1, 0xB3 or below 0x20 sets the sum to 1
  (it does not add). A word of width 0 (the end of s) ends the text. Keep the current line and its width L (start:
  empty, 0). For each word of width u:
  - if L + u > 189: the current line is finished (draw it, below; it can be empty when the first word of a line is
    wider than 189); the new line = the word, L = u. The word "\xB1" (the byte 0xB1 alone) also finishes the
    current line, but the new line is then empty (L = 0).
  - else: when L ≠ 0, add one space (L += the width of a space, 3 in font 2); add the word (L += u).
  (The control word 0xB3 + byte is not used by the messages and is not described here.)
  At the end, the current line is drawn when L ≠ 0.
- First (before any line) the top border rows: the pixels of x0 .. x0 + W - 1, y 58 .. 62 become D[pixel]
  (darkening table D of the 34 remap tables).
- Drawing line i (0, 1, 2 ...; finished lines and the last line alike) at y = 63 + 11 i: first its box rows
  (x0 .. x0 + W - 1, y .. y + 10 become D[pixel]), then Text (Drawing primitives) of the line, centred at x = 161,
  y, fg 0xFF, shadow 8, font 2, mode 300 (an empty line draws no text).
- Then, with k = the number of finished lines: the bottom border rows x0 .. x0 + W - 1, y_b .. y_b + 4 with
  y_b = 63 + 11 (k + 1) become D[pixel] (so a text that ends with the 0xB1 word leaves an undarkened band of 11 rows
  above the bottom border; an empty text gives only the borders). Then Frame (x0, 58, x0 + W + 1, y_b + 6, colour 1) and Frame (x0 - 1, 57, x0 + W, y_b + 5,
  colour F).

Evidence: StatusMessage? 0x27D24 = ShowMessage? 0x27B98 (text, 0, 0, 1, 1, 1, 25000): save area 0 (0x467E4 with
table 0x936BA entry 0 = back-buffer (0, 78), 215 x 79 → screen (52, 37)), WrapText? 0x362AC (text, fg 0xFF, shadow
8, screen, pitch 320, centre x 161, y 63, wrap 190 (189 = 190 - 1), centred, font 2, line height byte 0x949EC = 11,
not encoded, box 1, 0, 0, 1), word reader WrapNextWord 0x36184, TextWidth 0x35880, box rows 0x36DFC = RemapRect
0x52481 with g_remapTables[byte 0x284F08] (InitStrategicMap 0x43F48 sets 32), DrawFrame 0x46C44 ×2 (colour 0x68,
0xFF when the table is 0x1F), wait 0x67E44 (key, or a new press of the right / left button: waits for release),
restore 0x468B0. Calls traced in Unicorn on tac60; validate/diff/msgbox_diff.py --port: 1000 cases (the island
map's messages + fuzzed texts, darkening tables except 28), 0 mismatches (port/ja1front msgbox.rs, ja1f_message_box).

## Tactical messages

**The message box on the tactical screen.** Three messages of the tactical screen use the box of "Message box"
(steps 1..4; text centred at x = 161) with darkening table D = 28 (the sector's tileset table; the table index in
use on the tactical screen is 28) and frame colour F = 0x68. They differ only in the time limit of step 3:

| message | shown by | time limit |
|---|---|---|
| "Enemies nearby!" | host op 203 enemies_msg | 1500 ms |
| "Guard sees a new enemy!" | host op 326 guard_sees_msg | 25 000 ms |
| "Enemy spotted!" | a merc's sighting reaction, only while option byte 0x1163BD ≠ 0 (0 in a new game) | 700 ms |
| "NAME TEXT" | host op 57 merc_text (slot, text, ms), only for a merc (record +0x17A < 8) | ms |

For op 57, NAME = the soldier's nickname (+0x16E up to the zero byte), one space, and TEXT by number: 1 "doesn't
have enough Action Points!", 2 "has reloaded.", 3 "is shot retreating!", 4 "dies retreating!", 5 "abandoned first
aid", 6 "is all bandaged!", 7 "ran out of bandages!", 8 "is already performing first aid! Press ESC to abort that
first.", 9 "doesn't need first aid". (Front-end rule: ja1front shows it on the tactical screen only, not while its
inventory screen is open; two boxes in one game step show only the last one.)

The game does nothing else while the box shows: the game is Locked ("Other users of Lock": the soldiers and the
turn stand still, the screen timers run), and the key or click that closes the box has no other effect. The front-end shows the box over the whole 320 × 200 screen as it is at that moment.

**Status texts.** The game has no status line in the picture: every text that other sections send to "the status
line" (host op 125 message, op 314 guards_msg, the click texts of cursor.md such as "Can't move there!" and "Not
available!", "Can't compress time, there are enemies nearby!", the island map's message numbers of strategic.md)
shows in the message box of "Message box" with the time limit 25 000 ms. On the tactical screen the table is
D = 28 as above. Each text is a box of its own; the next one shows after the previous one went away. (Front-end
rule: ja1front shows only the last text of one game step and keeps a copy in its window title.)

Evidence: all 68 calls of StatusMessage? 0x27D24 = ShowMessage? 0x27B98 (text, 0, 0, 1, 1, 1, 25000), among them
TraversePrompt 0x1CF9C ("Can't traverse here!"), MoveMercTo 0x1C88C ("Can't move there!"), TacticalCursorTile?
0x2473F ("Not available!"), TacticalKeyHandler 0x13DB2 ("Can't compress time ..."), TacticalLoop 0x11325 ("Home
sector LOST!"), TeamNearby? 0x312F8, UseCrowbar? 0x3B62F, BuryMine 0x3CC8B, TacticalClick 0x59EB3 / 0x59F17 /
0x5A6AC, StrategicScreen 0x43A6E / 0x43C2D; GuardsArriveMsg 0x1F9DD → MsgStrInt 0x27CE0 (sprintf "%s%d" →
ShowMessage, 25000).

## Abort and restart questions

Key A or the abort button of the top strip (world_load.md section 8 "Abort or restart the day") asks two questions
in the question box of "Question box" (panel picture `PANEL_P.PCC`, byte 0x284F0C = 0 on the tactical screen):

1. Text "\xB1 ABORT this day?" (the 0xB1 word makes the first line empty: the text stands on line 1) with the
   picture `!MARK.PCC`. No: nothing more (the game goes on).
2. Yes: the day is aborted, then text "\xB1 RESTART this day?" with the picture `Q_MARK.PCC`. Yes = restart, no =
   abort (back to the office).

Alt-X on the tactical screen asks "Exit to DOS?" with the picture `Q_MARK.PCC` in the same box (yes = quit,
see "Exit to DOS").

**Exit to DOS.** The quit question of Alt-X (tactical screen: argument 1; A.I.M. screen, island map, assignment
screen, main menu prompt: argument 0):

1. The question box "Exit to DOS?" with `Q_MARK.PCC`. No: nothing more (the screen goes on).
2. Argument 1: Quick Save (savegame.md "Quick Save and Quick Restore: the steps", from its question "Save day in
   progress?"; its no only skips the save), then step 4. Argument 0: no Quick Save, go on with step 3.
3. When `QUICK.SAV` does not exist: `OFFICE.SAV`, `DAY.SAV` and `QUICK.SAV` are deleted (each only when it exists).
4. The program ends. The text screen shows the **quit line**: one of 23 lines (random(23), the game's random
   number, one draw), then an empty line. The 23 lines are 80-byte records from 0x9AF0B (the first: "Lucas Santino
   is a very patient man.", the second: "Is life really that important?").

Evidence (2026-10-28, asm): ExitToDosPrompt 0x38084 (EAX = argument; 0x278A0 (0x9AE39, 0x908E4 "q_mark.pcc");
argument ≠ 0: QuickSave 0x2D880, Random 0x53538(0x17) × 80 + 0x9AF0B, 0x382AC; else FileExists? 0x4707C(0x908C0
"quick.sav") = 0 → DeleteDaySaves 0x37FF0, then the same random line + 0x382AC). 0x382AC copies the line to 0x10AAD8,
appends 0x90900 "\n" and calls exit_(0); 0x38144 (read as the end-of-program step, not traced to its registration) prints it with printf_("%s\n") 0x908F0. Callers:
TacticalKeyHandler 0x138EB (1), AimScreen 0x20270 (0), MapKeyHandler? 0x42712, AssignScreen? 0x48A37, DayEndScreen?
0x65CB2 (0).

Evidence: AskAbortDay 0x11DC0: 0x278A0 (0x99A1D "\xB1 ABORT this day?", 0x902E4 "!mark.pcc", item 0, save 1,
restore 1); yes → byte 0x92E74 = 1 and 0x278A0 (0x99A37 "\xB1 RESTART this day?", 0x902F0 "q_mark.pcc"); yes →
byte 0x92E75 = 1. ExitToDosPrompt 0x38084: 0x278A0 (0x9AE39 "Exit to DOS?", 0x908E4 "q_mark.pcc"). Bytes 0x284F0C
= 0 and 0x284F08 = 28 in the dumps tac60 and fight59. validate/diff/question_diff.py --port has both questions as
fixed cases (0 mismatches).

**The "sees enemies" line.** Host op 197 sees_msg (slot) makes the line for soldier S = slot: `sees_slot` = S
+0x17A, and the line text = "NAME sees N enemies!" with NAME = S's nickname (+0x16E, up to the zero byte) and
N = S +0x1D5 (signed, written in decimal); when N = 1 the text is "NAME sees 1 enemy!". The text is made at that
moment and stays until the next op 197 (a later change of +0x1D5 does not change it; the sight steps call op
197 again for the soldier whose line shows, ai.md "Sightings as steps").

At each full draw of the tactical screen, after the cursor pass (cursor.md "Drawing"), while `screen_drawn` ≠ 0
and `sees_slot` < 0x40: Text (the line, x = view_w / 2 + 32, y = 34, centred, fg 0xFF, shadow 0xFE, font 2,
mode 300) into the back buffer (clip width 384; cursor.md "Text"). With view_w = 224 this is the top of the map
view, centred.

While `sees_slot` < 0x40, the soldier drawing (tactical_map.md "Soldiers") uses sight viewer v = `sees_slot` and the
sight table seen[v][k] = byte +0x112 + k of record v (for an enemy k = 0x28..0x2F: +0x13A + k − 0x28): an enemy
that merc does not see (flag ≠ 1) is not drawn. With `sees_slot` = 0x40 the viewer is 0xFF (no such test).

**Key F9** (scan code 0x43) on the tactical screen, when `selected` < 8: if `sees_slot` = 0x40, op 197 for the
selected merc; else `sees_slot` = 0x40 (the line goes away).

Evidence: MsgEnemiesNearby 0x1B0EC = ShowMessage? 0x27B98 ("Enemies nearby!", 0, 0, 1, 1, 0, 1500, 1); ShareSightings
0x736BC → StatusMessage? 0x27D24 ("Guard sees a new enemy!") = ShowMessage (…, 25000); ShowMercMessage? 0x299B8
("Enemy spotted!", …, 700) under byte 0x1163BD; MsgMercText 0x15910 (record +0x17A < 8: sprintf "%s %s" (nickname
+0x16E, text) → ShowMessage (…, 0, 0, 1, 1, 0, ms, 1)); ShowMessage takes the centre x from EBX (0 → 0xA1) and the box rows
from g_remapTables[byte 0x284F08] = 28 in the dumps tac60, fight59, combat1. MsgSeesEnemies 0x11448 (sees_slot
0x92DC6 = slot byte, sprintf into 0x10DA50 with "%s sees %d enemy!" / "%s sees %d enemies!"), TacticalRedraw
0x177A7 (DrawText 0x358D0 (0x10DA50, back buffer, pitch 384, x 0x229CBC / 2 + 32, y 0x22, 1, 0xFF, 0xFE, 2, 300)
under 0x92DCC and 0x92DC6 < 0x40), DrawSoldier? 0x16638 (slot ≥ 0x28: the int at 0xFAA67 + 0x200 × sees_slot +
slot, top byte = record byte +0x112 + slot), TacticalKeyHandler 0x13785 (key 0x43, selected 0x284F11 < 8).

**The turn banner and the other view texts.** The banner text and colour are the latest ones that host op 194
banner asked for (turns.md "Turn start"). The two turn timers of the Loop head (soldier_update.md, every 100 ms)
move it: while `screen_drawn` = 0 (the enemies' or the guards' turn) `turn_timer2` counts up to 20, so the banner
moves down one pixel per 100 ms and then stays; when the mercs' turn starts, turns.md sets `turn_timer2` = 0 and the
banner goes away. At each full draw of the tactical screen, right after the "sees enemies" line (same x, centred,
shadow 0xFE, font 2, mode 300, into the back buffer):

1. If s16 `turn_timer2` ≥ 1: Text (the banner text, y = `turn_timer2` + 28, fg = the banner colour). Done (step 3
   is not done).
2. Else, if s16 `turn_timer` ≥ 1: Text (`INTERRUPT!`, y = 48, fg 0x16). Else, if `time_compress` ≠ 0 and the darken flag
   (below) is 0: Text (`TIME COMPRESSION...`, y = view_h / 2 + 18, fg 0xFF).
3. If s32 `stop_timer` ≥ 1 and `screen_drawn` ≠ 0: Text (`RESERVE WARNING...`, y = view_h + 8, fg 0x16).

**The dark view.** Before step 1 (after the "sees enemies" line), when `time_compress` ≠ 0 or the darken flag is
set, every pixel of the map view (16 × view columns by 14 × view rows, the overview picture 224 × 168, at (32, 28)
of the back buffer) becomes D[pixel] with the darkening table of the screen (28 on the tactical screen). The
darken flag is a one-draw flag: the Control Panel, the speech balloon and the guards' arrival message set it to 1
before they redraw the view under their box, and each full draw sets it back to 0 after the texts. Evidence:
TacticalRedraw 0x177EB..0x17836 (RemapRect 0x52481 (back buffer + 0x2A20, view_cols × 16, view_rows × 14, pitch
384, g_remapTables[byte 0x284F08])), 0x178B9..0x178FF (text 0x9A9A7, y = [0x229CC0] / 2 + 0x12), flag byte 0x93DBE
cleared at 0x17959; OverviewRedraw 0x241F0 (224 × 168), 0x242F5, cleared at 0x24357; flag set to 1 at 0x4B7F2,
0x4BA78 (ControlPanel), 0x295A5 (SpeechBalloon?), 0x1F976 (GuardsArriveMsg). Front-end: Scene `dark`, smoke_play
step sighting (`_compress` picture).

With a banner kind never asked for, step 1 draws nothing. Evidence: TacticalRedraw 0x17839..0x17954 (s16
0x22A484 = turn_timer2, s16 0x22A486 = turn_timer, text 0x10E380 + colour 0x92DDE from ShowTurnBanner? 0x1054C,
strings 0x9AC99 / 0x9A9A7 / 0x9ACB3, 0x95DC8 = stop_timer, 0x229CC0 = view_h); TacticalLoop 0x108D8 (the timers);
MercsTurn? 0x12DD4 (turn_timer2 = 0).

## Question box

The game asks a yes / no question (for example "Start day without anyone assigned to duty?" on the island map) in
a box with a picture, a text and two buttons, NO and OK. Steps, with the question text s and the picture P (a PCC
member of MAIN.DAT, `Q_MARK.PCC` for the island map's question):

1. Save the screen rectangle x 52 .. 267, y 37 .. 116 (216 x 80 pixels), as for the message box.
2. Build the box in a 216 x 80 picture B (box pixel (x, y) = screen pixel (52 + x, 37 + y)):
   - B = the panel picture `PANEL_P.PCC` (216 x 80; on the island map. The office uses `PANEL_B.PCC`).
   - Copy the 64 x 68 block of the panel picture at (69, 6) to B at (6, 6) (the picture frame moves to the left).
   - P at (12, 13) (all its pixels).
   - With an item type t ≠ 0 (only the discard question of the assignment screen, strategic.md "Item moves on the
     item panel"): the item picture of t (item type byte 4; inventory.md "Drawing the screen (body)", the picture
     with a colour) at (26, 18) with colour 0. (The original can draw a second item picture with colour 0x0A at a
     place it is given; no caller in the port uses it.)
   - The text lines of s at x 140 (centred), y 13 + 11 i for line i: the line rule of the message box with the
     wrap width 129 (not 189), Text (Drawing primitives) with fg 0xED, shadow 8, font 2, mode 300. No box rows, no
     frames; an empty line draws nothing.
   - Button 5 (NO) at (90, 55) and button 6 (OK) at (156, 55), up faces: element 0x12 (sheet (192, 77), 38 x 16)
     at the button, then its label centred at (button x + 19, button y + 5), fg 0x2A, shadow 3, font 1. Labels:
     button 5 `NO`, button 6 `OK`.
3. Copy B to the screen at (52, 37). When the mouse is outside x 52 .. 267, y 37 .. 116, it moves to (237, 102).
4. Wait for the answer. Each round: a left button down with the mouse on button 5 (screen x 142 .. 179,
   y 92 .. 107) or button 6 (x 208 .. 245, y 92 .. 107) plays the press sound (sound.md "Button sounds") and shows the pressed face of that
   button: element 0x13 (sheet (230, 77), 38 x 16) at its screen position, then its label centred at (x + 18, y + 4),
   fg 0x2B, shadow 2, font 1. When the left button comes up, the up face comes back, the release sound plays and the answer is given: button
   5 = no, button 6 = yes (also when the mouse left the button meanwhile). Button 5 is tested first. Keys (upper
   case of the key's character): Enter, `O` and `Y` = yes; Escape and `N` = no; other keys do nothing.
5. Put the saved rectangle back on the screen. The answer is 1 for yes, 0 for no.

Evidence: 0x278A0 (text, picture name, item 0, ..., save 1, restore 1, mouse back 0) for "Start day without anyone
assigned to duty?" (StrategicScreen 0x432F4, q_mark.pcc): save area 0 (0x467E4), LoadPanelPcc 0x46B1C (`panel_` +
`p.pcc` when byte 0x284F0C = 0, set by InitStrategicMap 0x43F48; `b.pcc` when 1, InitOfficeScreen 0x4ED00) into
the panel buffer *0x93E68 (pitch 216), 0x46B98 (BlitCopy 216 x 80 into *0x93E6C pitch 228 + the 64 x 68 block
panel + 0x555 → buffer + 0x55E), PCC loader 0x532BC (picture into buffer + 0xBA0 = (12, 13)), 0x681D0 mouse move,
0x3625C = WrapText 0x362AC without box (fg 0xED, shadow 8, pitch 228, x 140, y 13, wrap 130, centred, font 2,
line height byte 0x949EC = 11), buttons 0x35B60 (5 / 6: button table 0x9345A entries x 142 / 208, y 92, up element
0x12, pressed 0x13, kind 6, label dx 20, dy 8; label 0x35AB8 text 0x966C9 + 6 k at (x + dx − 1 − p, y + dy − 3 − p),
fg 0x2A + p, shadow 3 − p, font 1), BlitCopy to the screen + 0x2E74, input 0x2538C (0x67CD8(5, 1) then (6, 1): hit
area = the button + the pressed element's w − 1, h − 1 of table 0x93732; 0x35D44 = press 0x35DA4, wait for the up
button, up 0x35C0C; keys via toupper 0x754D7), restore 0x468B0. Calls traced in Unicorn on tac60.

The same box serves ConfirmDialog? 0x275E4 ("Get to work?" before the day, "Raise natives' salary? ...") with an
animation in the picture frame instead of P, and a second animation with its own text after the answer: see
"The travel animation" below.

### The travel animation

The "Get to work?" box (strategic.md "Day start", host op 303 kind 1 with a wait row and a go row) plays two
animations from the archive `DAT\SANIM2.DAT` (on the hard disk, else on the CD).

**Rows.** JA.EXE holds a table of 12-byte animation rows at file offset 0xDB4DE (row r at 0xDB4DE + 12 r): byte 0
= the text number i, byte 1 = the first picture number m, byte 2 = frames per picture c, byte 3 = the frame count
n, byte 4 = frame width w, byte 5 = frame height h (all unsigned), u16 at 6 = the frame time in ms, i16 at 8 = the
sound slot (−1 = none), u16 at 10 = the sound's loop value. The texts are 40-byte strings at file offset 0xDD8C3 +
40 i (empty = no text). The day start uses rows 6 / 7, 12 / 13, 14 / 15 and 16 / 17 (wait / go): every one has w =
60, h = 50, c = 5, 52 ms, no sound; the wait rows have n = 5, the go rows n = 15, 15, 25 and 30, and the go rows'
text is "Destination: ± Sector   " (± = the line break of the message box rule), in which the host writes the
sector number into characters 22 and 23.

**Frames.** Frame k (k = 0, 1, ...) of a row is the w x h block at (w × (k mod c), 0) of the picture member
`SANIM.<m + k div c>` (three digits, PCX; its palette is not used: the screen keeps the island map's palette).
The pictures are 300 x 51; their last row is never shown.

**Start of a row** (both rows): when the sound slot is not −1, the effect of that slot plays (rate 0x2B11, volume
20, the loop value). Frame 0 is copied into the box picture B at (12, 13) (it replaces the picture P of step 2).

**Playing.** A frame step happens when at least the frame time passed since the last step (the first step comes at
once): frame k is copied to the screen at (64, 50) (B's (12, 13) at the box place), then k = k + 1. A wait row
loops: when k reaches n − 1 it starts again at 0. A go row stops when k reaches n − 1. Frame n − 1 of a row is
therefore never shown. (For a looping row the frame rule holds while its frames fit into four pictures, n ≤ 20;
the day start's wait rows have 5 frames.)

**The box with the animation** (a wait row, a yes row and a no row; a missing row = none). Steps 1 to 3 of the
question box with no picture, then frame 0 of the wait row at (12, 13) of B, before the box goes to the screen.
During step 4 the wait row plays. After the answer, the row of that answer (yes row or no row): when there is none,
step 5 at once. Else B is built again from the panel picture (the panel picture and the 64 x 68 block of step 2,
no buttons, no question text), frame 0 of that row at (12, 13), the row's text (when not empty) by the text rule of
step 2, and B goes to the screen at (52, 37). Then the row plays until it stops or a key or a click comes; then
step 5.

**The mouse in this box** (ConfirmDialog 0x275E4). At the start, a mouse outside x 0x34..0x10B, y 0x25..0x74 moves to
(237, 102). The third flag brings the mouse back at the end to where it was at the start, also when the player moved
it: 1 on the island map's callers (day start, wage, corpse export), 0 on A.I.M.

**End of the box.** After the answer's row (or at once when there is none) every sound stops (all effect channels,
the cry and the speech channel), then step 5. So a looping wait-row sound (loop value 0 = until stopped) ends there.

**Callers.** The archive is `DAT\SANIM.DAT` for all but the day start:
- the day start (strategic.md "Day start"): `DAT\SANIM2.DAT`, wait / yes rows as given there, no row none;
- the A.I.M. screen's helicopter charge question: wait row 0 (sound slot 23 = main-bank IDL, loop value 0), yes row
  1 ("On the way. ± ± E.T.A.: Daybreak...", sound slot 24 = LIF, once), no row none;
- the A.I.M. screen's terminate question (all three places): wait row 2, yes row 3 ("± Disconnecting..."), no row
  none;
- the natives' salary question (strategic.md "The map panel"): wait row 4, yes row 5 ("Happy campers!", sound slot
  17 = NAT, once), no row none;
- the corpse export question (strategic.md "Corpse export"): wait row 8; can_pay = 1: yes row 9 ("It'll be a closed
  coffin for sure!"), can_pay = 0: yes row 11 ("Sorry, you don't have the funds"); no row 10 ("OK..."). The box,
  its text and its buttons are the same for both values of can_pay.
The sound slots are effects of the main bank (sound.md "Main bank"), loaded on these screens.

Evidence (2026-10-27, asm read): ConfirmDialog? 0x275E4 (EDX = wait row pointer, EBX = yes row, ECX = no row (0 =
none); callers AimScreen 0x20FC3 (rows 0x9443A / 0x94446 = 0 / 1, then AddFunds) and 0x2107B / 0x210BA / 0x210EB
(0x94452 / 0x9445E = 2 / 3, text 0x975B7 "Terminate..."), CorpseExportPrompt? 0x463E7 (balance >= cost: 0x9449A /
0x944A6 / 0x944B2 = 8 / 9 / 10) and 0x464B2 (else: 8 / 11 / 10, same text 0x229B1C); the box end 0x2781D calls
StopAllSounds? 0x5CE08 (AIL_end_sample on every sample handle), StrategicScreen 0x436C6 (salary) / 0x43D93 (day);
LoadSanimDat? names 0x90AA0 `dat\sanim.dat` / 0x90ADC `dat\sanim2.dat`; the wait row
plays inside the box input 0x2538C as PlaySanim?(1) (ECX = 1 = loop); after yes FormatSanimName (go row), WrapText
0x3625C of text 0x9681F + 40 × byte 0, BlitCopy 216 x 80 to the screen + 0x2E74, loop PlaySanim?(0) / MousePoll /
0x67E44 while n − 1 > frame 0x229B04; then 0x5CE08), FormatSanimName 0x27494 (PlaySfxSlot(slot, 0x2B11, 0x14, loop);
loads `sanim.%03d` into the panel buffer, picture j at row 50 j while j × h + h − 1 < 200; frame 0 BlitCopy to the box
buffer + 0xBA0), PlaySanim? 0x258D0 (TimerElapsedMs 0x22A3FC ≥ u16 +6, BlitCopy w x h to the screen + 0x3EC0,
counters 0x229B04 frame, 0x229B08 frames left in the buffer, 0x229B14 / 0x229B18 x / y in the buffer; when the
buffer runs out the next picture loads at row 0, so frame k always comes from picture m + k div c), rows table
0x9443A, texts 0x9681F, LoadSanimDat? 0x5E8E0 (OpenFileOrCD). PlaySanim? first calls the sound streaming service
0x5CCF8. Rows 0..5 and 8..11 take their pictures from `DAT\SANIM.DAT` (pictures 0..41; both archives hold 37..41).
validate/diff/sanim_diff.py --port runs FormatSanimName + PlaySanim? in Unicorn for all 18 rows (541 frame steps,
with and without loop) against the port's frames: 0 mismatches.

## Merc-face question box

A yes / no question with the face of a merc (FIRE on the assignment screen: "Fire <nickname>?"; the guide offer
"Accept this native guide?"). Input: the text s, the profile p and the nickname n. The box does not save or put back
the screen: the caller saves the screen part 0 (x 52 .. 267, y 37 .. 116) first and puts it back after.

1. Build the box in a 216 x 80 picture B (box pixel (x, y) = screen pixel (52 + x, 37 + y)):
   - B = the panel picture (as the question box step 2: `PANEL_P.PCC` on the island map, `PANEL_B.PCC` in the
     office), all 216 x 80. No 64 x 68 block move.
   - The face: the MAIN.DAT member named by p in decimal digits ("12" for profile 12; a PCC picture 148 x 69); its
     top-left 56 x 50 pixels (all pixels, also 0) at B (8, 7).
   - n with Text (Drawing primitives) centred at (36, 60), fg 0x2A, shadow 3, font 2, mode 300.
   - The text lines of s at x 140 (centred), y 13 + h i for line i with h = (line height byte, 11) − 1 = 10: the
     line rule of the message box with the wrap width 129, Text with fg 0x28, shadow 8, font 2, mode 300.
   - Buttons 5 (NO) and 6 (OK) as the question box step 2 (up faces at (90, 55) and (156, 55)).
2. Copy B to the screen at (52, 37). When the option Subtitles is on and the mouse is outside x 52 .. 267,
   y 37 .. 116, the mouse moves to (237, 102).
3. Wait for the answer as the question box step 4 (the same buttons, presses and keys). The answer is 1 for yes,
   0 for no.

Evidence: YesNoPrompt? 0x28C84 (EAX s, DL p, EBX n, ECX 1 = with text and buttons, stack 0): LoadPanelPcc 0x46B1C,
itoa_ 0x7574F (radix 10) + PCC loader 0x532BC into *0x93E70 (pitch 0x94), BlitCopy 216 x 80 panel *0x93E68 →
*0x93E6C (pitch 228), BlitCopy 56 x 50 face → + 0x644, DrawText 0x358D0 (n, x 0x24, y 0x3C, centred, 0x2A, 3, font 2,
0x12C), byte 0x1163BD and mouse (0x229D00, 0x229D04) → 0x681D0(0xED, 0x66), WrapText 0x3625C (fg 0x28, shadow 8, x
0x8C, y 0xD, wrap 0x82, centred, font 2, line height byte 0x949EC − 1), buttons 0x35B60 (5 / 6, table positions −
(0x34, 0x25)), BlitCopy to the screen + 0x2E74, input 0x2538C. ECX 0 (the balloon with Subtitles) draws the text
only, at (75, 7), fg 0xED, line height − 2, and waits for the speech. validate/diff/question_diff.py --face (ECX 1 and
ECX 0 cases).
Mouse (question_diff --mouse): with Subtitles on, a mouse outside x 0x34..0x10B, y 0x25..0x74 moves to (237, 102) (Subtitles
off: no move, also for key L). ShowMouse 0x28F1B runs before the wait of ECX 0, so the pointer shows during a Subtitles quote. Stack 1
(only the A.I.M. quote box 0x2900C): at the end the mouse goes back to the place before the move when it is less than 10
pixels from the place after the move on both axes.

**Text-only form** (ECX 0: a merc, the guide or Brenda speaks with the option Subtitles on): step 1 without the text
lines and without the buttons: the panel picture, the face and the nickname n (as above), then the text s as it is
stored (+1 encoding: every byte but the space is the shown byte + 1) decoded, as lines of the message box rule (wrap
width 129) at x 75 (not centred), y 7 + 9 i, fg 0xED, shadow 8, font 2. Step 2 as above. Then the wait of "Speech
balloon" step 7 with 80 ms per byte of s (SpeechWait? 0x28034 (0x3B, 0x2B, 80 × strlen, 0, 1, 0x90494, 0)); when
byte 0x92EF0 ≠ 0 instead until a key or a click (0x67F34). The answer is 0.

## Say it again (key L)

Key L shows the text of the last quote again (world_load.md "The last quote", `last_quote`: speaker, category,
row, quote). The key works on the tactical screen, the island map (also the island map between two days, after the
day replay and the verdict), the assignment screen, the office's View Team page (the box shows over the page; the
key or click that ends the box does not close the page) and the A.I.M. screen. No voice plays; no game time passes.

1. The name n by the speaker byte k: k < 0x40: the nickname of profile k (profile +0, up to its NUL); k = 0x40:
   `Jack`; k = 0x41: `Brenda`; k = 0x42: `Santino`; any other k (0xFF after a new game, before the first quote):
   nothing happens.
2. The text s: quote `quote` of row `row` of the category's `<CAT>QUOTE.EDT` text (speech.md "Quote categories":
   record row + quote, 200 bytes, +1 encoding), with no voice.
3. If the byte after `last_quote` (game state byte 0x24A + 6, save byte 21276: the profile m that an A.I.M. refusal
   named, strategic.md "Refusal reason") is below 60: the first `MEMBER` in s becomes the nickname of profile m.
4. The text-only merc-face box ("Merc-face question box", text-only form) with s, profile k (the face: MAIN.DAT
   member k in decimal, so "64" / "65" / "66" for Jack / Brenda / Santino) and the nickname n, on the panel
   picture `PANEL_P.PCC` (`PANEL_B.PCC` after the office or the A.I.M. screen opened, until the island map or the
   day replay opens again: so `PANEL_B.PCC` on the View Team page and the A.I.M. screen, `PANEL_P.PCC` elsewhere). It waits
   until a key or a click (not the 80 ms per byte). The key or click does nothing else.
5. The screen part under the box comes back (save area 0); the tactical path marks go off (path marks off).

Evidence: SpecialNpcSpeech? 0x29D54 (key L 0x4C in TacticalKeyHandler 0x13490, also called by AimScreen 0x202A6,
MapKeyHandler? 0x4271F, AssignScreen? 0x48A6D (= the assignment screen 0x4891C), DrawOfficeLedger 0x4CE6C,
DayEndScreen? 0x65C8F (strategic.md "The day-end screen"));
the panel byte 0x284F0C read by LoadPanelPcc 0x46B1C: 1 = PANEL_B (ShowAimScreen 0x1FECA, InitOfficeScreen? 0x4ED4B),
0 = PANEL_P (InitStrategicMap 0x43EE9, ReplayScreenInit? 0x642D9); speaker byte
0x22A1EA (< 0x40 profile nickname copy; 0x40 0x90640 "Jack", 0x41 0x90648 "Brenda", 0x42 0x90650 "Santino"; other:
byte 0x92E4F = 1, return), LoadPanelPcc 0x46B1C, save area 0x467E4 (0, 0x34, 0x25), PlayQuote 0x24F08 (speaker,
s16 0x22A1EC row, s16 0x22A1EE quote, 200, 0x2299BC, category 0x22A1EB, 0, play 0), byte 0x22A1F0 < 0x3C:
0x35810 (+1 encode the nickname) + StrReplace ("MEMBER" in +1 encoding: "NFNCFS" 0x90658), byte 0x92EF0 = 1
around YesNoPrompt? 0x28C84 (text, DL speaker, EBX name, ECX 0, stack 0) = wait for a key or a click, restore
0x468B0 (0), PathMarksOff. NewGameInit 0x37DEB writes speaker 0xFF.

## Speech balloon (merc face over its strip)

When a merc of the team speaks in the tactical view (the sighting message with a radio, soldier_update.md "Merc
sighting"), its face pops up over its strip portrait, the quote text shows beside it, and the face goes away.
Input: the soldier record R of the merc (slot k = R +0x17A, side c = R +0x180, profile p = R +0x179), the quote q
(0 .. 32) and the Text option T (the option byte of the "Enemy spotted!" box).

1. **Face.** The MAIN.DAT member named by p in decimal digits (as the merc-face question box) is a PCC picture; its
   pixel (x, y) goes to the face sheet F (148 x 69, pitch 148) at (x, y) for x < picture width, y < picture height.
   When q = 9 the block x 56 .. 101, y 0 .. 22 of F is then copied to x 5 .. 50, y 5 .. 27 of F (the hurt face).
   The face shown is F x 0 .. 55, y 0 .. 49 (56 x 50).
2. **Text.** The DAYQUOTE.EDT record 33 p + q (200 bytes) of the EDT folder (the CD's EDT\ first, then the game's).
3. **Place.** The strip origin (ox, oy): ox = 0 when k < 4, else 272; oy = 14 + 42 (k mod 4). The face offset
   (fx, fy) = (14, 3) when c = 0, else (3, 3). Face point: X = ox + fx + 2, Y = oy + fy + 2. The balloon rectangle
   is 58 x 52 at bx = X − 15 limited to 1 .. 261, by = Y − 14. The text centre is tx = 140 when k < 4, else 178;
   the text top is ty = Y − d − 1 with d = 4 when k mod 4 = 0, 14 when k mod 4 = 3, else 10.
4. **Save** the 58 x 52 screen pixels of the balloon rectangle (S).
5. **Slide in**: 4 frames with (w, h) = (35, 32), (42, 38), (49, 44), (56, 50). One frame (w, h):
   - a 58 x 52 picture P = S;
   - when w > 28: x0 = (58 − w) / 2, y0 = (52 − h) / 2 (rounded toward 0); P (x0 + i, y0 + j) = F (⌊xs⌋, ⌊ys⌋)
     for i < w, j < h, scaled as below; then Frame (x0 − 1, y0 − 1, x0 + w, y0 + h, colour 0xFF) on P (Drawing
     primitives; it sets entry 1 of remap table 0 to 0xFF as every Frame);
   - P goes to the screen at (bx, by); then a wait of 30 ms.
   Scaling (32-bit unsigned, fixed point with 22 fraction bits): sx = (56 · 2^22) / w, sy = (50 · 2^22) / h
   (integer division); the source row of row j is (sy / 2 + j · sy) >> 22, the source column of column i is
   (sx / 2 + i · sx) >> 22. At w = 56, h = 50 this is a plain copy at (1, 1) with the frame (0, 0) .. (57, 51).
6. **Text** (only when T ≠ 0): the decoded quote (every byte but the space taken − 1, up to the first 0) as lines of
   the message box rule with wrap width 159 (font 2), line i drawn on the screen centred at tx, y = ty + 10 i, fg
   0xFF, shadow 1, font 2. The text is not saved: it stays until the next full draw.
7. **Wait** (SpeechWait): the merc's voice plays; the wait ends at Enter, Escape, Space or a click, or when the
   voice ends; without a voice after 80 ms per byte of the quote text. With Subtitles on (byte 0x1163BD) the voice's end and the text
   time end only the talk: the wait stays until the player ends it (0x283AD; faceanim::SpeechWait, speechwait_diff). A right click also sets "skip the focus"
   (byte 0x92F65: the sighting then does not move the view). The face blinks and the mouth moves during
   the wait ("Face animation" below). The **face rectangle** of the wait is (X0, Y0) .. (X0 + 56, Y0 + 50), the
   ends included, with (X0, Y0) one pixel up and left of the face (the face shows at (X0 + 1, Y0 + 1)). A left
   click ends the wait at once when the mouse is in the face rectangle (it also sets byte 0x92F64 = 1, which
   nothing reads); outside it, only when more than 1000 ms passed since the wait's timer started (the start of the
   wait; with Subtitles off and a hold time it starts again when the talk ends). **Wait text**: the merc message's
   text (the weapon text of "Merc sighting": JAMMED, NO AMMO, ITEM BROKEN, NO WEAPON; the fire and dismiss
   balloons of 0x50908 pass an empty text) shows only while the mouse is in the face rectangle: each face redraw then draws it centred at face
   point (28, 20) (fg 0xFF, shadow 8, font 2) and only the 56 x 50 face goes to the screen, so the text is cut at
   the face edges; when the mouse leaves the rectangle the face is drawn again without it.
8. **Slide out**: 4 frames with (w, h) = (49, 44), (42, 38), (35, 32), (28, 26) as in step 5 (the last frame is S
   only: the screen is back as it was, but for the text).

Evidence: ShowMercMessage? 0x299B8 (how ≥ 0 path): face loader 0x532BC (archive *0x92F44 = MAIN.DAT, member
itoa(R +0x179), sheet *0x93E70 pitch 0x94), q = 9: BlitCopy 0x52AE0 (F + 0x2E9 ← F + 0x38, 46 × 23), PlayQuote
0x24F08 (p, 33 p, q, 200, buffer 0x2299BC, play 1, p, category byte 0x92F88), face point from g_slotPos 0x93686 and
g_slotOffsets 0x936A6 (+ 10 c), text x 0x8C / 0xB2, d from k mod 4. SpeechBalloon? 0x29544: clamp (1, 0x105), save
BlitCopy into *0x93E6C + 0x40 (pitch 228), 0x1CB78 TacticalRedraw first when its flag is set, frames 0x29428
(BlitCopy 59 × 53 to the back buffer 0x22A4EF pitch 0x180, scaled blit 0x52B7C / row copy 0x524B9, DrawFrame
0x46C44 colour 0xFF, BlitCopy 58 × 52 to the screen, delay 0x67250(30)), text WrapText 0x3625C (fg 0xFF, shadow 1,
x, y − 1, wrap 0xA0, centred, font 2, line height byte 0x949EC − 1), SpeechWait? 0x28034 (time 80 × strlen, keys
0x0D / 0x1B / 0x20, right button → 0x92F65 = 1; text = stack arg 2 (SpeechBalloon? stack arg 7: 0x50908 passes ""
0x90FAC), its length test at 0x280B5, face rectangle MouseInRect? 0x67C0C (x, y, x + 0x38, y + 0x32) at 0x28412,
text DrawText 0x358D0 (text, panel 0x93E6C, pitch 0xE4, x 0x24, y 0x1B, centred 1, fg 0xFF, shadow 8, font 2, 300)
at 0x284A8 after the face copy to panel + 0x644 (= (8, 7)), in-rectangle flag [esp + 0x40], left click 0x284F2 /
0x285C4 (TimerElapsedMs > 0x3E8)). validate/diff/balloon_diff.py --port; front-end: tests/rom.rs
weapon_text_stays_on_the_face, jammed_gun_report_carries_its_text.

## Face animation (eyes and mouth)

While a face waits for its speech (the speech balloon step 7, the text-only merc-face box, the merc-face question
box with a voice) and while the A.I.M. screen shows an available merc, the face blinks and (only while the merc
talks) the mouth moves. The face sheet F (148 x 69, "Speech balloon" step 1) holds the face (x 0 .. 55, y 0 .. 49)
and the animation blocks: **eye block** k = 1, 2, 3 at x 56 .. 101, y 23 (k − 1) .. 23 (k − 1) + 22 (46 x 23);
**mouth block** k = 1, 2, 3 at x 102 .. 147, y 20 (k − 1) .. 20 (k − 1) + 19 (46 x 20).

**Eye frame** of the eye state e (0 .. 12): f = e, but 3 when e ≥ 4. f = 0 is the face's own eyes, F block x 5 .. 50,
y 5 .. 27; else eye block f. The block goes to the face picture at (5, 5) (46 x 23, all pixels, also 0).
**Mouth frame** of the mouth state m (0 .. 3): m = 0 is the face's own mouth, F block x 5 .. 50, y 30 .. 49; else
mouth block m. The block goes to the face picture at (5, 30) (46 x 20, all pixels).

**Eye step** (state e, talking flag d, mode b; "R(n)" = the game's random number 0 .. n − 1, drawn in this order):
- e = 0: r = R(2); e = 3 when r ≠ 0, d ≠ 0 and b ≠ 1; else e = 1.
- e = 1: e = 1 when b = 1, else 2 (no random number).
- e = 2: r = R(2); e = 3 when r ≠ 0 and d ≠ 0; else 3 when b = 2; else 0.
- e = 3, 4, 5, 6: e + 1.
- e = 7: r = R(2); e = 8 when r ≠ 0 and b ≠ 1; else 0.
- e = 8 .. 11: e + 1. e = 12: 0.
So a quiet face blinks (0, 1, 2, 0); a talking face sometimes looks aside (frame 3 for the states 3 .. 12); with
b = 1 (the hurt face, quote 9) the eyes stay at frame 1.

**Mouth step** (state m): when the voice is in a **silence** (below), m = 0. Else repeat r = R(4) until r is allowed,
then m = r. Allowed r: for m = 0 only 1; for m = 1 every r but 1; for m = 2 only 1 and 3; for m = 3 only 1 and 2.

**The wait** (SpeechWait; mode b = 1 for the merc message with quote 9, else 0; the timers count real ms):
1. Start: m = 0; e = b (1 when b = 1, else 0); talking = 1. Mouth time tm = R(150) + 25, then eye time te =
   R(1000) + 500; both timers start.
2. Each pass, before the redraw test: when the eye timer reaches te: eye step (e, talking, b), the eye timer starts
   again, redraw; te = R(2000) + 1000 when e = 0 and talking = 0, else R(300) + 100.
3. Not talking: when m ≠ 0: m = 0, redraw. Talking: the talking test ("Speech balloon" step 7: the voice ends, or
   without a voice the text time ends) may set talking = 0; then, when talking is still 1 and the mouth timer reaches
   tm: mouth step, the mouth timer starts again, tm = R(100) + 50, redraw.
4. Redraw: the face picture = F x 0 .. 55, y 0 .. 49 with the eye frame of e and the mouth frame of m, to the screen
   at the face point of the box (the balloon rectangle + (1, 1); the merc-face box: screen (60, 44)).
When the wait ends the plain face (frames 0) is drawn once more.

**Silence** (lip sync of the streamed voice): the voice samples (8-bit unsigned, 22050 Hz) are scanned with a clock
P that grows by 45 per sample (µs). A sample 0x7B .. 0x85 is **quiet**. A run of quiet samples starts at the clock
P / 1000 (ms, toward 0) of its first sample (the run counter is 16 bits: at 65536 quiet samples it starts again);
the first loud sample after a run of 1100 or more quiet samples ends it at its P / 1000, and the run becomes silence
i (at most 100 silences; later runs are lost). A run that the voice end cuts is not a silence. Playback time T (real
µs from the voice start, in steps of 2500): each step T = T + 2500, then: in silence i and T / 1000 > end i: not in
silence, i = i + 1; else not in silence, i < the count and T / 1000 ≥ start i: in silence i. (One change per step.)
A voice that does not stream (the first part of a two-part quote with the Alt mode, sound.md) keeps the silence
state of the last streamed voice. Each streamed part of a quote (part A, part B) is its own voice for this scan:
SpeechStream resets the table, the run and T (LipSyncReset 0x66D50), so the quiet run at the end of part A is never
a silence, and part B's T starts at 0 at its own start. The port plays the parts as one sample: its lip view makes
part A's end run loud (sound.rs lip_bytes; faceanim_diff parts cases).

**A.I.M. screen** (strategic.md A.I.M.): only when the shown merc is available (alive and not on a contract): the
eye state e starts at 0 with te = R(1000) + 500. Each pass of the screen while the mouse is not on the face rect
(9, 21) .. (64, 70): when the eye timer reaches te: eye step (e, 0, 0), the eye frame of e goes to the screen at
(14, 26) (face at (9, 21), the mouth is not touched), the eye timer starts again, te = R(2000) + 1000 when e = 0,
else R(300) + 100. While the mouse is on the face rect no eye frame is drawn (the eyes stay as they are) and the
state stays. A gear view item click (strategic.md "The gear view item click") draws eye frame 0.

Evidence (asm 2026-10-28): eye step 0x27E38 (EAX &e, DL d, BL b; jump table 0x27E04), eye frame 0x27D64 (EAX e,
EDX 1 = screen + 0x208E pitch 320, else panel *0x93E6C + 0xABD pitch 228; source *0x93E70 pitch 0x94), mouth step
0x27FB8 (silence byte 0x337C9F, R(4) retry, jump table 0x27FA8), mouth frame 0x27F40 (panel + 0x2101). SpeechWait?
0x28034: R(0x96) + 0x19, R(1000) + 500, timers 0x47020 / 0x47040, CL = b (ShowMercMessage? 0x29AB8: 1 for q = 9;
the other callers 0, checked with balloon_diff / question_diff), face to the panel + 0x644 then to the screen. Silence
scan 0x66DB8 (called by the stream feeder 0x5CCF8), reset 0x66D50 (SpeechStream with lip sync), AIL timer 0x66CC4
(period 2500 µs; table count 0x337970, run 0x337972, P 0x337974, starts 0x337978, ends 0x337B08, T 0x337C98, i
0x337C9C). A.I.M.: AimScreen 0x2098B (byte 0x95F6D = available, set by 0x21648; reached only with the mouse off the face, 0x202FF), 0x20D6A
(gear view item click; the only other caller).
validate/diff/faceanim_diff.py --port.

## Santino's balloon

When Santino speaks (ai.md Think step 3, op 38), his voice starts (speech.md) and, with the Subtitles option off, his
face slides in as a speech balloon over the tactical screen. With the Subtitles option on, the face-and-text box of
the question box shows instead (question box face text: speaker 0x42, name "Santino", the quote text; the port keeps
Subtitles off, so this branch is not drawn).

1. **Face.** The MAIN.DAT member "66" goes to the face sheet F as in "Speech balloon" step 1 (no hurt face).
2. **Place.** The face point is fixed: (X, Y) = (134, 50), so the balloon rectangle is 58 x 52 at (119, 36).
3. **Steps 4, 5, 7, 8** of "Speech balloon" with this rectangle (save, slide in, wait, slide out). No text is drawn
   (step 6 is skipped). The wait without a voice is 80 ms per byte of the quote text (SANQUOTE.EDT record 0, the
   text the voice speaks). The work buffer of the frames is the panel sheet, which is loaded again afterwards; the
   screen is back as it was after the last frame.

Evidence: SantinoSpeech? 0x5F4C4: PlayQuote 0x24F08 (0x42, record 0, 200, buffer 0x2299BC, 9, 0x42, 1); byte
0x1163BD = 0: face loader 0x28898 (itoa(0x42) = "66", 0x532BC into *0x93E70 pitch 0x94), SpeechBalloon? 0x29544
(EAX text, EDX 0x86, EBX 0x32, ECX 0x42; stack 0, 0, 0, redraw 0, g_panelSheet, pitch 0x140, 2, 0x91104, slide 1,
second quote 0, 9, 0), LoadPanels 0x46EC4; else YesNoPrompt? 0x28C84 (0x2299BC, 0x42, 0x91108, 0).
validate/diff/balloon_diff.py --port (20 Santino cases).

## The guide's and Brenda's balloons

Host op 302 guide_say (q, c) and op 317 brenda_say (q) speak as Santino does ("Santino's balloon"), with these
differences:
- **Speaker and quote.** The guide: speaker 0x40, category c (6 = QST in most calls, 3 = JAC at the day's
  verdict), quote q. Brenda: speaker 0x41, category 7 (BRE), quote q. Voice and text: speech.md "Quote categories"
  (archive `<CAT>\<speaker>`, member `<speaker><CAT><q>.8` or its two halves; text record q of the category's EDT
  file).
- **Face.** The MAIN.DAT member "64" (the guide) or "65" (Brenda), at the same face point (134, 50).
- **Second quote.** For the guide when q < 0x28 and c = 6, and for Brenda when q < 10, the quote has a second
  part, quote q + 1 of the same category: after the wait of the first part (step 7 of "Speech balloon") ends, a
  further 1000 ms pass (the wait still ends early at Enter, Escape, Space or a click), then the voice of quote
  q + 1 starts and the wait runs again (also with the 1000 ms after the voice); then the face slides out. Without
  the second part the wait has no extra time. Each wait is its own SpeechWait?: the face animation starts again
  (fresh timers, talking flag on, the clean face picture), and in the 1000 ms after a voice the face is quiet
  (talking flag off: slow eyes, mouth closed). The text time of both waits (the wait without a voice) is 80 ms per
  byte of the FIRST quote's text: 0x29544 measures the text once, before the first wait.
- **Subtitles on** (`opt_step_text` ≠ 0, byte 0x1163BD): no face slides in. After the voice of quote q starts, the
  text-only merc-face box ("Merc-face question box", text-only form) shows with the text of quote q, profile p =
  the speaker (0x40 / 0x41: MAIN.DAT member "64" / "65") and the nickname `Jack` (the guide) or `Brenda`, on the
  panel picture of the screen (`PANEL_P.PCC` on the island map, `PANEL_B.PCC` in the office), and waits. With a
  second part: the voice of quote q + 1 starts and the box shows again with its text (no extra 1000 ms). The caller
  puts the screen back after (save area 0, 0x28698).

Evidence: 0x5F5AC (guide) and BrendaSpeech? 0x5F76C: PlayQuote 0x24F08 (speaker, record 0, q, 200, buffer
0x2299BC, category, speaker, play byte 0x92F88), face loader 0x28898 (0x40 / 0x41), SpeechBalloon? 0x29544 at
(0x86, 0x32) with the stack arguments second quote q + 1, its category, extra wait 1000 (else 0, 0, 0); in it,
after SpeechWait? 0x28034 a second quote ≠ 0 plays PlayQuote and SpeechWait again; SpeechWait's last argument =
the ms that pass after the voice ended (the wait ends once that much time passed since the voice ended).
validate/diff/balloon_diff.py --port (20 guide / Brenda cases; with a second part the original's second wait shows
the same screen as the first).

## Item info box

A right click on an item (inventory, assignment screen) shows the item info box: a box with the item's picture,
name, numbers and description; some items (letters, photos, the map pieces) show a full picture instead. Inputs:
the merc's profile P, the item type t, the count n (a signed byte), the value v (a signed 16-bit number: the
condition, or the amount for money), the numbers flag a, the voice flag b, the mouse-back flag m (host op 231
item_info, port_api.md), and the Control Panel options Item Info (option 6) and Subtitles (option 5). Item table
bytes (inventory.md): byte 4 = picture, 5 = clip, 6 = stack class, 7 and 9 = repair bytes, 10 = has a condition.

1. t = 0: nothing; the result is 0. When t = 0x59 and P ≥ 60 (a native), t = 0x5A from here on.
2. The item text: record t of `EDT\INVQUOTE.EDT` (270 bytes per record): the name N = bytes 0..29, the description
   D = bytes 30..269, both zero-ended and stored with every byte + 1 (as the quote texts).
3. The voice starts (the item's name; "The item's voice" below).
4. A picture item is one whose picture byte is 0x1A, 0x74 or 0x52. For it (Item Info on or off):
   - A 320 x 160 picture S, all 0. The MAIN.DAT member `item89.pcc` (t = 0x5A) or `item<t>.pcc` (t in decimal)
     at (0, 0) of S (pitch 320); a missing member leaves S at 0.
   - With Subtitles on and t = 0x59 or 0x5A: D wrapped into S (the line rule of the message box with every byte of
     a word − 1, wrap width 144) at x 28, y 80 + 11 i for line i, Text fg 1, shadow 0, font 2, mode 300.
   - The 200 x 150 pixels of S at (0, 0) go to the screen at (60, 20); a pixel 0 is not written.
   - Go to step 7. The result is 1 (the caller draws the screen again).
5. Item Info off (not a picture item): no box; go to step 7. The result is 0.
6. The box (as the question box: the screen rectangle x 52 .. 267, y 37 .. 116 is saved; box pixel (x, y) =
   screen pixel (52 + x, 37 + y)):
   - The text T: for money (t = 0x6F), D with its first `BNPVO` (`AMOUNT` + 1) replaced by `$` and the decimal
     text of v (only `$` when v = 0), each byte of that + 1; else T = D.
   - B = `PANEL_P.PCC` and its frame block, as step 2 of the question box.
   - The item picture of t (inventory.md) at (10, 10) with colour 0x0A.
   - When v < 10 and repair byte 9 or 7 of t ≠ 0: `useless` centred at x 31, y 16, Text fg 0xED, shadow 1, font 0.
     Else when n < 0: `jammed` there, the same way.
   - The name: every byte of N up to its end that is above 0x20 as a signed byte gets − 1; Text of N at (60, 14),
     not centred, fg 0xFF, shadow 8, font 1.
   - When a = 1, the clip byte (signed) ≠ 99 and the stack class = 1: Number |n| (2 digits) at (138, 14), `/` at
     (150, 14), Number of the clip byte (signed, 2 digits) at (156, 14); all Text fg 0x28, shadow 8, font 1.
   - When a = 1 and byte 10 of t ≠ 0: element 0x42 at (174, 11), the condition bar (inventory.md) of the low byte of
     v as a signed byte at (176, 13), Number v (3 digits) at (181, 14), `%` at (199, 14); Text fg 0x28, shadow 8,
     font 1.
   - T wrapped (the line rule of the message box with every byte of a word − 1, wrap width 194) at x 10 (not
     centred), y 30 + 8 i for line i, Text fg 0x28, shadow 8, font 0.
   - Copy B to the screen at (52, 37). When m ≠ 0 the mouse position is kept; when the mouse is outside x 52 .. 267,
     y 37 .. 116, it moves to (237, 102).
7. Wait until the right button and then the left button are up. Then wait until a mouse button goes down (Item
   Info off and not a picture item: also until the voice ends). When m ≠ 0 (box shown) and the mouse is less than
   10 pixels from where it was after step 6 (in x and in y), it goes back to the kept position. The voice stops.
   With Item Info on: wait until the left and the right button are up, and (not a picture item) put the saved
   rectangle back on the screen.

### The item's voice

Inputs as above: t (after the native swap of step 1), n, v, the voice flag b. Item table bytes of t: byte 3 =
the **voice kind** k (0..6), byte 5 = clip, byte 6 = stack class. Member names match without case. "Speech M"
below = look up member M in the open archive; when it exists, the speech channel plays it (22050 Hz, volume 70,
once); when it does not exist, nothing plays and the steps go on.

1. The text record (step 2) changes no "last quote" bytes and plays nothing.
2. The open archive becomes `INV\` + A (the hard-disk folder INV): A = `MONEY` for t = 0x6F, else `1_54` for
   t < 55, `55_81` for t < 82, else `82_135`.
3. Money (t = 0x6F): the **and flag** = (v > 99). The step S = 1.
4. Not money: the name member = t in decimal, then (stack class > 1 only) `_` and (n = 99: `1`, else n in decimal,
   signed), then `.8` (`21.8`, `20_3.8`). When it exists: speech of it, and S = 1. When it does not exist: S = 0:
   no voice at all, and the voice is never **done**.
5. The box or the picture is drawn (steps 4..6) while the voice plays.
6. In the wait, each pass, while S ≠ 0 and neither the speech channel nor the cry channel plays: when S ≥ 7 the
   voice is **done**; else the code C = row k, entry S of this table runs, then S = S + 1 (except for code 0):

   | k | items (t) | codes for S = 1, 2, 3, ... |
   |---|---|---|
   | 0 | 0, 136 | 0 |
   | 1 | 50-63, 67, 68, 79, 81-90, 92, 93, 95-110, 112-119, 121-135 | 0 |
   | 2 | 15-49, 64, 66, 69, 70, 75-78, 91, 120 | 2, 0 |
   | 3 | 1-14 | 2, 5, 0 |
   | 4 | 71-74, 80 | 4, 0 |
   | 5 | 65, 94 | 3, 0 |
   | 6 | 111 (money) | 6, 7, 11, 8, 9, 10 |

   The grade G of the low byte of v (signed): 100 or more `F`; 76..99 `E`; 51..75 `D`; 26..50 `C`; 11..25 `B`;
   else `A`. R starts as v.
   - Code 0: the voice is **done** (nothing more plays).
   - Code 2 (condition): when the stack class > 1 and n > 1, b = 0 for the rest of the box. Then, when b ≠ 0 and
     n ≥ 0: the open archive becomes `INV\EXTRA`, speech `C` G `.8`.
   - Code 3: when b ≠ 0 and n = 1: the open archive becomes `INV\EXTRA`, speech `L` G `.8`.
   - Code 4: the same with `U` G `.8`.
   - Code 5 (rounds): when b ≠ 0: speech `b` + (n = 99: the clip byte, signed; else n) + `.8` in the archive that
     is open now (only EXTRA holds these: when code 2 did not open it, nothing plays).
   - Code 6: q = R / 1000 (toward 0); when q > 0: speech `M` (q × 1000) `.8`, R = R − q × 1000.
   - Code 7: the same with 100. Code 8: the same with 10, but only when q > 1. Code 9: when R > 0: speech `M` R `.8`.
   - Code 11: when R > 0 and the and flag is set: the cry channel plays `and.voc` of MONEY (22050 Hz, volume 70, once).
   - Code 10: the cry channel plays `dollars.voc` of MONEY (22050 Hz, volume 70, once).
   So 2345 = M2000, M300, and, M40, M5, dollars; 0 = dollars only. MONEY has no thousands member above
   M30000, so that part of 31000..32767 is silent. A VOC member's sound = its bytes from offset 0x21 to the end
   minus 1 byte (sound.md "Op 101 voice").
7. With Item Info off and t not a picture item there is no box; the wait ends when the voice is **done** or a mouse
   button goes down (with S = 0: the button only).
8. When the wait ends (a button down): every sound stops (the speech channel, the effect channels, the cry
   channel). With Sound FX off (or no sound driver) nothing is heard, but the steps run the same.

Evidence: 0x25D20 (ItemInfoBox): PlayQuote 0x24F08 (0x40, 0, t, 270, 0x229E92, cat 4 = INV, 99, 0);
picture kinds at 0x25D73; options 0x1163BE (Item Info) and 0x1163BD (Subtitles) of g_options 0x1163B8; the picture
path 0x26429..0x2651C (memset sheet 0xC800, PCC loader 0x532BC, WrapText 0x3625C (fg 1, shadow 0, pitch 320, 28,
80, 0x91, 0, font 2, line height byte 0x949EC = 11, encoded 1), BlitKey (screen + 0x193C, sheet, 200, 150, 320,
320, 0)); money 0x26009..0x26067 (0x36C5C = concatenation of strings and non-zero numbers, StrReplace 0x252B0);
the box 0x2608D..0x26424 (save area 0x467E4 (0, 52, 37), LoadPanelPcc + 0x46B98, 0x67E18 keeps the mouse when the
third stack argument ≠ 0, 0x681D0 (237, 102), 0x46750 (pic, buf, 228, 10, 10, 0x0A), DrawText 0x358D0, DrawNumber
0x36090, DrawPanelElem 0x469F0, bar 0x4B064, WrapText 0x3625C (0x229B1C, fg 0x28, shadow 8, pitch 228, 10, 30, 0xC3,
0, font 0, line height byte 0x949E4 = 8, encoded 1; WrapNextWord 0x36184 subtracts 1 per byte when encoded),
BlitCopy 216 x 80 to screen + 0x2E74); the wait 0x26521..0x26C31 (0x67C68 = wait right up, 0x67C54 = wait left
up, exit when g_mouseButtons & 3 ≠ 0; mouse back 0x67E30 when |dx| < 10 and |dy| < 10; restore 0x468B0). The
condition bar of a low byte 110..127 never ends in the original (crash); the test does not feed it.
validate/diff/iteminfo_diff.py runs 0x25D20 in Unicorn on tac60 (voice stubbed, INVQUOTE.EDT, MAIN.DAT) and
compares the screen at the wait with ja1f_item_info.
The voice (2026-10-26, asm): PlayQuote 0x24F08 called with play 0 and category 4 (no last-quote bytes for 4, 5, 10);
archive names 0x25DEC..0x25E73 (0x90524 `INV\`, 0x90534 .. 0x9054C), open 0x5C0EC (close 0x5C144, open 0x53338);
name 0x25E81..0x25FB3 (0x90554 `_`, 0x904A8 `.8`, stack class 0x94DFE, ArcSeek 0x534A4, SpeechStream 0x5CC6C
(0x5622, 0x46) checks 0x96399 and Sound FX 0x1163B8); money 0x25FB5..0x25FEB (LoadVocSample 0x5C960, `dollars.voc`
0x9055C, `and.voc` 0x90568); wait loop 0x26534..0x26B95 (speech pump 0x5CCF8, flag 0x96372, cry status 0x5CA74,
codes 0x94C00 + 7 k + S, jump table 0x25CF0); 0x25CA4 opens `INV\EXTRA`; 0x25BDC = prefix C / L / U + grade + `.8`;
`b` 0x904C4, `M` 0x90598; PlayVocSample 0x5C9C0; done flag tested with Item Info 0x1163BE at 0x26B70..0x26B85;
stop 0x5CE08 at 0x26BD4. Port: iteminfo::voice (cargo test voice_steps), smoke step item_info (voice line, WAV
not silent, Item Info off). Live (2026-10-28, tac60, key I, right press on an item): the original is in 0x25D20's wait
19 ms after the press, so the box has no delay of its own.

## Tooltip box

A small framed label over a face or a button (assignment screen "gear" / "exit", inventory, pay day). State: the
number T of the tooltip that shows (0 = none; one byte, kept for the whole game) and the kept position (kx, ky).
Rectangle size by number: 64 x 53 for number 9, else 36 x 28.

**Show** (number n ≠ 0, x0, y0, x1, y1, texts t1, t2, t3, font f):
1. When n = T: nothing.
2. Hide (below; it uses the old T). T = n.
3. kx = x0 − 5 when n = 9, else x0 − 4; ky = y0 − 1. Save the screen rectangle of n's size at (kx, ky).
4. The sound: effect 0 of the main bank (sound.md "Button sounds") at rate 11025, volume 5, once (PlaySfxSlot
   0x5CB34 (0, 0x2B11, 5, ECX 1) at 0x232D9 in the show routine 0x23264).
5. Frame (x0 − 1, y0 − 1, x1, y1, colour 0xFF) on the screen.
6. cx = x0 + (x1 − x0) / 2 and cy = y0 + (y1 − y0) / 2 (both divisions rounded toward 0). Text is centred, fg 0xFF,
   shadow 0xFE, font f, mode 300:
   - t2 empty: t1 at (cx, cy − 3).
   - Else: t1 at (cx − 1, cy − 12), t2 at (cx − 1, cy − 4) and, when t3 is not empty, t3 at (cx − 1, cy + 4).

**Hide**: when T ≠ 0: put the saved rectangle (T's size) back at (kx, ky); T = 0.

Evidence: 0x23264 (show), 0x23420 (hide), save / restore 0x467E4 / 0x468B0 through the back buffer (slot 8 or 9 of
the table 0x936BA: 36 x 28 and 64 x 53), tooltip number 0x93EA8, kx 0x229C84, ky 0x229C80.
Diff-tested: validate/diff/assign_hover_diff.py (direct calls of show / hide with numbers 1..9, any texts and fonts).

## The mouse pointer

The game draws its own mouse pointer: a **shape** of at most 42 x 16 pixels, drawn with remap table 0 where colours
0 and 0x17 are not drawn (table 0 byte 0x17 is set to 0 first). The shape's top-left corner goes to (mouse x + hx,
mouse y + hy), cut at x 320 and y 200 (and at x 0 on the left).

| shape | picture | (hx, hy) | when |
|---|---|---|---|
| 0 | element 78 (27, 146) 11 x 11, a red arrow | (−3, 0) | the normal pointer |
| 200 | element 62 (38, 146) 11 x 11, the arrow turned | (−3, 0) | shape 0 while a mouse button is held |
| 201 | element 63 (192, 29) 12 x 15 | (−3, 0) | from the payroll screen to the end of the morning stat changes (strategic.md "The morning") |
| 202 | element 97 (0, 146) 11 x 11, a grey arrow | (−2, 0) | the new game after its intro (Set Difficulty, Set Quick-Save Mode), and the office |
| 203 | element 98 (11, 146) 11 x 11, the grey arrow turned | (−2, 0) | shape 202 while a mouse button is held |
| 1..199 | item type s: its item picture (inventory.md, colour 0) at x 0, 6 or 13 of the shape (42-, 30- or 16-wide picture), then element 61 (218, 131) 18 x 16 (a hand) at x 14 | (−24, −8) | an item is held (the cursor item) |

The mouse poll changes the shape at a button press: 0 → 200, 202 → 203; at the release: 200 → 0, 203 → 202
(other shapes stay). The main menu sets no shape: it shows shape 0 at the program start (the game sets 0 before
the first menu) and after a game (the office and the game-over screen end with 0). The morning (strategic.md "The
morning"): the payroll screen sets 201; the "Not enough funds" dialog sets 0 after its first two messages and
201 at its end (also when the funds are enough); the morning stat changes set 0 at their end, so the later morning
boxes show 0 and the office then sets 202. Screens that hold a cursor item (the assignment screen, the inventory
screen) set the shape of the cursor item (0 when none) each time it changes, so they show 0 with no item.
Shape 0 is also set by: the program start, the end of day merc update,
the merge, the office ledger, the "Continue" prompt, the game-over screen, the
Santino / guide / Brenda balloons and the inventory screen. The office ledger page sets shape 0 when it opens and
202 when it closes. Seen in the original's memory dumps (byte 0x229C54 = the shown shape): every tactical state 0,
every office state 202, the payroll at the end of a day 201; live (2026-10-28, DOSBox-X): main menu 0, office
202, island map 0, assignment screen 0.
The front-end's window: the system pointer is shape 202 in the office (0 on the ledger page) and after the new-game
intro, 201 in the morning as above, else shape 0 (SCALE times as big, tileset 0 colours), shape 200 while a
mouse button is held (203 for 202). While an item is held, the screen picture holds the item shape and the system pointer is
hidden. The original has one drawn pointer on every screen; the window hides the system pointer
only over the tactical map (the tile cursor shows the mouse there) and while an item is held, so each screen that
sets a shape (the office after a day that ended with the mouse over the map) shows it again (the maintainer play test #20 C,
D: no pointer in the office; arrow and hand together on the assignment screen). The tactical screen also draws a held item (an item taken from a strip box) at the mouse (#20 B). Evidence (2026-10-28): SetMouseShape? 0x674B0 (element table 0x93732 = x, y, w − 1, h − 1 as i16; BlitCopy
14 rows into the shape buffer [0x93E74], pitch 42; hot spot words 0x229C70 / 0x229C48; item: picture from 0x46AE4
and the bank table 0x1163CC, then BlitRemap of element 61 at shape + 14), DrawMousePointer? 0x67A28 (BlitRemap
key 0), MousePoll? 0x6792A..0x67969, callers: GameMain 0x502E2 (0xCA after LoadPanels), InitOfficeScreen? 0x4EDB1
(0xCA), PayDayDialog? 0x4556A (0xC9) and 0x4519F (0 by 0x67478 = the shape with the mouse hidden), PayrollScreen
0x44439 (0xC9), ExperienceChangeMsg? end 0x462C2 (0), GameMain start (0, before ContinueDayPrompt / MainMenu) and
0x502E2 (0xCA, new game only, after SINTRO), OfficeScreen end 0x4F532 (0), PlayLoop order OfficeScreen ->
StrategicScreen -> TacticalLoop -> DayEndScreen? (the morning); 0x67478 callers AssignScreen? 0x489A7 (each round,
the cursor item) and AssignPanelFlipIn? 0x47CE4 (its 201 at 0x47BD2 shows only during the flip); remap table 0 in tac60 = identity; unit test pointer_shapes (tests/rom.rs).

## The tactical screen

The panel buffer is the whole tactical screen. The map view (14 x 12 tiles = 224 x 168 pixels) is copied into it
at (48, 14), between the two slot columns. When the sector starts, all 8 slots are drawn (slot k = the record whose
`slot` is k): a slot whose +0x17F is 0 with "Drawing one slot"; a slot whose +0x17F is 1 shows the switch view
("The switch view"). The rows above and below the map (y 0..13 and 182..199) are the **interface strips**
(next section).

### The interface strips (rows 0..13 and 182..199)

The **top strip** holds the sector box (the sector number, blinking while enemies are in the sector), six buttons
(`done`, `trav`, `inv`, `map`, `opt`, `abort`) and the time meter; the **bottom strip** holds seven boxes with the
items of one merc. The original draws both when the sector starts, after the inventory screen and after a quick
restore. Inputs: `top` (0 / 1), `keep` (0 / 1: 1 = the top strip picture is already in the panel buffer, only the
button faces are drawn again), `opt` (opt_onscreen, as in inventory.md), the combat byte, the current sector and its
mercs byte (byte 2 of its 9-byte sector_state record) and enemies byte (byte 1), the **strip flag** (u8; 1 in every
play save seen; start 1; set to 1 by the sector load, by Set mode to 0, 1 or 2 ("View modes"; mode 3 leaves it) and
by the small panel's Show; set to 0 only by the redraw after the full view showed ("The small panel in the full
view" step 4); the quick save keeps it, block 75), `selected`, the **turn slot** (u8, the merc whose turn it is in combat; 64 = none), the soldier
records, the time meter width (= world field `day_bar`), the small panel flag, the blink byte, the **combat seen**
byte (u8). The panel parts used (elements, texts, number, "button label", "button shade", "time meter", "sector box",
"strip box") are the ones of inventory.md "Drawing the screen": **button label** i = the first half of its step 4
(the label text), **button shade** i = the second half (the two remapped 19 x 12 / 18 x 12 pieces). Button i = 0..5
in inventory.md "Buttons" order.

1. If top ≠ 0 and opt < 3:
   1. The last mouse x that the mouse poll keeps = 3137 (no map tile under the mouse until the next poll).
   2. If keep = 0: loader result = load MAIN.DAT `TSTRIP.PCC` at (0, 0) (inventory.md step 1).
   3. For button i = 0..5: if keep ≠ 0, draw element 0x78 at the button's (x, 1). Then the button label i. Then
      the button shade i when (combat = 0 and i = 0) or (the sector's mercs byte = 0 and i ≤ 2): `done` is greyed
      out outside combat; `done`, `trav` and `inv` are greyed out when no merc of the team is in the sector.
   4. Copy the panel rows 0..13 (320 x 14 pixels) to the screen.
   5. The time meter (inventory.md step 2), then the sector box (inventory.md step 3 with "on" = 1: drawn even
      when the enemies byte is 0). Both go into the panel buffer only (the screen gets them with the next copy).
2. Loader result = load `BSTRIP.PCC` at (0, 182).
3. If the strip flag ≠ 0 (if it is 0, nothing of the bottom strip goes to the screen):
   - if selected < 8 and record `selected` +0x138 (in play) ≠ 0: the strip boxes k = 0..6 of S = record
     `selected` **not darkened** (inventory.md step 13 and "Strip box" without the last darken step);
   - else if the turn slot < 8 and record `selected` +0x138 ≠ 0 (original bug kept: it tests the selected
     record, not the turn slot's; with selected ≥ 8 that is a byte outside the 8 mercs, see below): the strip
     boxes of S = record `turn slot` **darkened** (the full "Strip box", as in the inventory);
   - else copy the panel rows 182..199 to the screen.
   In the two merc branches the boxes and the copy of rows 182..199 to the screen happen only when S's slot
   +0x17A < 8 and (opt < 3 or the small panel flag ≠ 0); else nothing is drawn and nothing is copied.
4. If combat = 0: combat seen = 0.
5. If opt < 2: for record k = 0..7, with j = its slot +0x17A: when j < 8, draw slot n = record j's slot +0x17A
   (record j's +0x17F = 0: "Drawing one slot" of slot n; +0x17F = 1: "The switch view"), then copy
   the 48 x 42 pixels of slot j (not n) from the panel buffer to the screen at slot j's origin. Note: j is a slot
   number but is used as a record number for the second read; in a save every merc k has slot k, so n = j = k.

Selected ≥ 8 in step 3 (second test): the original reads the in-play byte of soldier record `selected` (records
8..47 are the other people; 64 and 255 point past the 48 records). For 64 that byte is byte 0 (the ground
sprite) of tile 1039 of the tile map, which lies after the soldier records (record 64 +0x138 = the tile map +0x2078,
checked 2026-10-28 in the EXE's addresses). For 255 it lies in the copy of the CD file DAT\INV.DAT that the
original loads whole when it enters a sector (and for the island map and A.I.M.): byte 0x44C8 (17608) of that file,
a picture byte, 0xDF, so not 0 (evidence: LoadInvDat 0x38190 loads the 55100 bytes at 0x1163C8, and record 255
+0x138 = 0x11A890; the byte is 0xDF in 131 of 136 memory dumps, 0 only in dumps from before the first load).

Evidence: tools/invdraw.py tac_strips (mirror of LoadStripPccs? 0x2F928, the slot copy 0x33204),
validate/diff/strips_diff.py: 1000 cases on 5 dumps (top / keep, opt 0..3, combat, mercs byte, strip flag,
selected 0..8 / 255, turn slot, in play, item entries, time meter width, small panel flag, blink, noise in the
panel and screen buffers), compared: panel, screen, 48 soldier records, table 0, blink, combat seen, the loader
result, the mouse x: 0 mismatches; 3 of 3 hand mutants caught (shade rule, dark flag, the top copy size). `--port` (port/ja1front ja1f_draw_strips): 911 cases, 0 mismatches (2026-10-23).
Addresses: mouse x copy 0x229CF0, strip flag 0x92E36, turn slot 0x284F0F, combat seen 0x92DCA, buttons 0x13..0x18
of the button table 0x9345A.

The original redraws a slot only when something in it changes. Redrawn from the dump's state, the 8 slots of the
panel buffer come out the same in 100 of 131 memory dumps; the rest are dumps of other screens (office, AIM,
end of day: the buffer is scratch there) or slots with 2..9 old pixels (a change the original did not redraw
yet). So a front-end that redraws every slot on every frame shows the same picture, only sooner.

**Mouse areas** (screen pixels): the **bar area** of slot k = 11 x 27 at the slot origin + the health-bar offset
of the merc's column; the rest of the 48 x 42 slot is the **portrait area**; (48, 14) .. (271, 181) is the map
view (view pixel = screen pixel - (48, 14)).

### Clicks on the top strip (button faces)

Button i (0..5, inventory.md "Buttons" order) has the mouse area x .. x + 36, 1 .. 12 (the 37 x 12 pressed face).
**Up** face: element 0x78 (198, 147, 37 x 12) at (x, 1), then the label centred at (x + 19, 4), fg 0x2A, shadow 3,
font 1 (= the strip's button label). **Pressed** face: element 0x79 (235, 147, 37 x 12) at (x, 1), then the label
centred at (x + 18, 3), fg 0x2B, shadow 2, font 1. A left click on a button that reacts: the press sound (sound.md "Button
sounds"), the pressed face, wait until the left button is up, the up face, the release sound, then the action. Which buttons react and their actions
(TacticalKeyHandler 0x13490, mouse y ≥ 1 and above the map view, `screen_drawn` ≠ 0): `done` only when `turn_shown`
≠ 0 (end the turn when `combat` ≠ 0 and selected < 8); `trav` and `inv` when selected < 8 and the sector's mercs byte
≠ 0 (traverse prompt; inventory when the selected merc's hp > 14); `map` (the guard screen 0x1328C); the sector box
(286, 3, 306, 11) when `map` is not hit (toggle 0x22A129, not described); `opt` (control panel); `abort`.
Evidence: button table 0x9345A entries 0x13..0x18 (kind 6), press 0x35DA4, up 0x35C0C,
label 0x35AB8 (x + dx − 1 − p, y + dy − 3 − p, fg 0x2A + p, shadow 3 − p); mappanel_diff.py "strip" cases (port
ja1f_map_field 28 + 2 i + p).

### Clicks on the bottom strip (item boxes)

While the tactical screen is up (`tactical_on`) and the mouse y is 182..198, each pass of the tactical input step
runs the **strip click** for S = the record of `selected` (library `ja1_strip_click (slot, mode)`, mode = the
cursor mode of cursor.md). Box k of the mouse x: the largest k of 0..6 with x ≥ edge[k] − 1, edges 0, 49, 94, 139,
184, 229, 272 (−1 when none; it then uses entry 0). Box k shows **entry** e(k) of S: 0, 7, 8, 9, 10, 11, 1 (the main
hand, the five vest pockets, the second hand). "Cursor" = the 12-byte `held` item; "missing" = inventory.md's
missing vest pocket test; **flag** = the cursor-under-panel flag (`cursor_drop`).

1. If hp (+0x183, signed) < 15 or `inPlay` (+0x138) = 0: done (nothing else).
2. h0 = the hand type (byte +0x00 of S); changed = 0.
3. Left button held: e = e(k). If inventory.md "May it go" with X = the cursor item onto place e gives a value
   other than 0 (−1 counts as go on here) and e is not a missing pocket:
   - Ctrl held: inventory.md "Unstack" on entry e; changed = 1 when it acted.
   - Else: the cursor and entry e trade their 12 bytes; changed = 1.
   Then, when the cursor holds an item (type ≠ 0) and flag = 0: `drop_answer` = w_682ac(1, 190, 318, 197) (the mouse
   is held inside x 1..318, y 190..197); `drop_answer` = mouse_to(mouse x, 191, 318) (the third value is left over from the call before); flag = 1; **strip box** = k
   (byte). Then hide, cursor(cursor type), w_55fd8(S, 0, 1), show, wait until the left button is up.
4. Else right button held: changed = 1; open = 0.
   - k = 0: entry 0 empty: open = 1. Else the first j of 6, 5 .. 1 whose entry e(j) is empty and not a missing
     pocket gets entry 0's 12 bytes, and entry 0 is cleared; no such j: open = 1.
   - k = 6 and entry 1 holds an item: entries 0 and 1 trade their 12 bytes.
   - Other k (and k = 6 with entry 1 empty): open = 1.
   open = 1: inv_open(S) (the inventory screen of S runs, as port_api.md op 349). Else w_55fd8(S, 0, 1) and wait
   until the right button is up.
5. If the cursor is empty and flag ≠ 0: `drop_answer` = w_682ac(0, 0, 319, 199) (the mouse is free again); flag = 0.
6. If mode = 2 and flag = 0: weapon_cursor(S) (cursor.md: the cursor value from the hand item).
7. If the hand type ≠ h0: when S +0x177 ≠ 0 and the hand type ≠ 0x4D, S +0x177 = 0; then ClearTarget(S, 0) and
   Next animation of S (ai.md).
8. If changed: reserve_ap(S); slot_redraw(slot +0x17A, 1).

Note: the left branch runs only when the left button is held; the right branch only when it is not. With the
mouse held in the strip (flag 1) the player can only put the item into a box; a right click opens the screen.
Evidence: 0x54F3C (strip click, called from TacticalKeyHandler 0x13490 at 0x14635 when mouse y > [0x229CDC] = 181
and ≤ 198 and 0x92DCC ≠ 0), box finder 0x4671C (edges 0x93B14), entries 0x93D59, MergeItemsPrompt 0x4A838,
missing 0x4A464, Unstack 0x53580, item copy 0x47090, DropItem 0x470B0, strip box byte 0x92E73, flag 0x955C9,
inventory screen runner 0x54D50 (it returns at once when S +0x188 = 1), mouse limit 0x682AC, mouse move 0x681D0,
waits 0x67C54 / 0x67C68, WeaponCursor 0x1C39C, cursor mode 0x284F1B. Test: inv_diff.py --port --strip.

**Cursor item back** (host op 85 cursor_back; key F10 before the Control Panel; also the game's own callers: the
tactical loop, other tactical keys, a merc's death, taking a ground item). When `tactical_on` ≠ 0, the flag
(`cursor_drop`) ≠ 0 and the cursor holds an item (type ≠ 0): the 12 bytes of the cursor go into entry e(strip box)
of the record of `selected` (the box the item came from in step 3 above), the cursor's 12 bytes become 0, the mouse
shape becomes the (now empty) cursor type, `drop_answer` = w_682ac(0, …) (the mouse is free again) and the flag =
0. Else nothing. Evidence: CursorItemBack? 0x53660 (0x92DCC, 0x955C9, g_cursorItem 0x222B50, strip box byte
0x92E73 → entry byte 0x93D59 + box, CopyBytes12 0x47090 (EAX destination), DropItem 0x470B0, SetMouseShape?
0x674B0, 0x682AC → 0x229D18). Front-end: play.rs cursor_back_raw (stderr `cursor back slot S entry E`); smoke step
strip (`opt back`).

### The Control Panel (key F10, `opt` button)

The tactical Control Panel shows 16 switches as text straight over the tactical screen. It has no picture of its
own. The office has a second Control Panel (the switch box, office hot spot 6), with the same 16 switches.

**The 16 switches** are 16 bytes of the world image, offsets 21538 + k (0 = off, 1 = on). Names (font 1):
0 `Sound FX`, 1 `Music`, 2 `Fast Scroll`, 3 `Alt Scroll`, 4 `Shadows`, 5 `Subtitles`, 6 `Item Info`, 7 `Show Paths`,
8 `Pal Cycling`, 9 `Fast Anims`, 10 `Follow move`, 11 `Step Sounds`, 12 `Day Replays`, 13 `Fast Firing`,
14 `Safety Move`, 15 `Merc OKs`. The world fields `opt_step_text` (5), `opt_path_marks` (7), `opt_ambient` (8),
`opt_half_delay` (9), `opt_follow` (10), `opt_sound` (11), `opt_confirm_move` (14) and `opt_move_fx` (15) are
these bytes. Byte 21554 (the first of the four bytes after them) is used once, below ("Continue Game").

**When it opens.** Key F10 (the cursor item goes back first, as before other screens) and a left click on the
`opt` button ("Clicks on the top strip"). Both do nothing while `screen_drawn` = 0.

**Opening.**
1. S (Quick Save shown) = 1 when `combat` = 0, or the hidden save-anywhere byte ≠ 0, or save byte 21285 (quick save
   in combat) ≠ 0. R (Quick Restore shown) = 1 when the file `QUICK.SAV` exists.
2. The tactical screen is drawn again (a full draw) with the darken flag set, so the map view under the panel is
   the dark view ("The dark view"; seen in DOSBox-X 2026-10-28: tac60, F10). The game stops: no turns, no animation, no time, until the
   panel closes. The mouse stays inside the map view (x 48..271, y 14..181) while the panel is open.
3. Item k = 0..15 has the place (x, y) = (55, 25 + 13 k) for k < 8 and (170, 25 + 13 (k − 8)) for k ≥ 8. Item 16
   (`Continue Game`) has (120, 142). Button j = 0 (`Quick Save`) has (55, 165), button j = 1 (`Quick Restore`)
   (170, 165).
4. Draw (Text, font 1, not centred, mode 300) for k = 0..15: the name at (x, y), fg 0xFF, shadow 8; then the value
   at (x + 75, y): `ON` with fg 0xFF when the byte is 1, else `OFF` with fg 0x6F; the shadow is the byte itself
   (so `OFF` has no shadow). Then `Continue Game` at (120, 142), fg 0xFF, shadow 8. Then `Quick Save` when S and
   `Quick Restore` when R, fg 0xFF, shadow 8.

**Each pass** (the hover item h starts at −1; "hit" = the mouse inside the rectangle, edges included):
1. The search: the first item k = 0..16 whose rectangle (x, y − 4) .. (x + 95, y + 9) holds the mouse ends the
   search. It is a valid target except: k = 0 (`Sound FX`) when the switch is on and `Subtitles` is off; k = 5
   (`Subtitles`) when the switch is on and `Sound FX` is off. (The game has a sound driver and a music driver; a
   game without them also refuses k = 0 / k = 1.) No item hit: no valid target.
2. Then, in each case, button j = 0, 1: its rectangle (x, y − 4) .. (x + 80, y + 5) holds the mouse and (j = 0
   and S) or (j = 1 and R): the target is 100 + j. (The rectangles of items and buttons do not overlap.)
3. No valid target: t = −1. When t ≠ h: the old hover item is drawn again in its normal colours (as in
   Opening step 4; for a switch, name and value). Then the new one is drawn with fg 0x16: a switch's name with
   shadow 8 and its value text with shadow = the byte; `Continue Game` with shadow = byte 21554 (a quirk: normally
   0, so no shadow; the shadow pixels of the normal draw stay on the screen); a button with shadow 8. h = t.
4. The left button is down and h ≠ −1: wait until the button is up, then the click sound: tactical effect
   slot 0 at 11025 Hz, volume 20, once. h ≥ 16 (Continue Game, Quick Save, Quick Restore): the panel closes with
   the result h. Else switch h toggles (below).
5. Escape: the panel closes with the result −1. Other keys do nothing.

**Toggle switch k** (at its place (x, y)):
1. k = 7 (`Show Paths`) and the switch is on: the library's Hide path marks (select.md) with force = 1 when
   path_preview ≠ 0 (select.md "Path AP with marks" step 5: the preview of a walk with one step more), else 0.
   This runs before the flip, so the switch is still on and the marked tiles lose their byte 7.
2. The byte flips (0 ↔ 1).
3. The value area, 20 x 13 pixels at (x + 75, y), gets the pixels of the map picture under it: the map view of
   the full draw of Opening step 2 (dark, with its view texts, no mouse pointer, no panel text). The original
   copies them from its back buffer, point (x + 75 − left + 32, y − top + 28) (ControlPanelInput 0x4BEC9..0x4BF17,
   BlitCopy 0x52AE0, view left 0x229CB8 / top 0x229CC4); the map view on the screen is a copy of that buffer, and
   every value area lies inside the map view in all four view modes. Then the new value text with fg 0x16, shadow
   = the new byte.
4. k = 1 (`Music`) now on: when the wanted theme > 0, play the wanted theme (music.md "Play theme t", flag 0).
   `Music` now off: when the playing theme > 0, stop the music (music.md "Stop the music").
   k = 0 (`Sound FX`) now on: the click sound plays once more (step 4 of the pass).
5. Every other switch changes nothing at once: the game reads the byte where it uses it.

**Closing.** The mouse may go anywhere again. Result 100: the tactical screen is drawn again, then the quick save
(savegame.md "Quick save"). Result 101: drawn again, then the quick restore. Result 16 or −1: nothing more; the next
frame of the tactical loop covers the panel.

**Sound FX off**: no sound effect starts (effect slots; speech and music are not effects). **Music** off: music.md.
Evidence: ControlPanel? 0x4B788 (S/R 0x4B79E..0x4B7E9, places 0x4B859..0x4B9BB, button places 0x96248), input
0x4BAA4 (search 0x4BAC4..0x4BB84, buttons 0x4BB88..0x4BBF3, hover draws 0x4BBFF..0x4BE42, click 0x4BE47..0x4BFC0,
Escape 0x4BFC5), sound effect gate 0x5CB47 (PlaySfxSlot: no effect when the sound driver is missing or byte 0 is
0), names 0x9675F (12 bytes each), ON/OFF 0x9A11F / 0x9A139, Continue Game 0x9A153, buttons 0x9641C / 0x9643C;
callers TacticalKeyHandler 0x137BE (F10, after 0x53660) and 0x145E6 (`opt`, button 0x17). Live check (DOSBox-X,
tac60, 2026-10-26): F10 opens it (ControlPanel? on the stack), two clicks on `Music` stop the theme (playing −1)
and start theme 1 again.

### View modes (keys PgUp, PgDn, Insert, Tab)

The tactical screen has four **view modes** (`opt_onscreen`, 0..3). Each mode has a **view window**: `cols` x `rows`
tiles at screen point (`left`, `top`); its size in pixels is `view_w` = 16 x cols, `view_h` = 14 x rows.

| mode | what the player sees | cols x rows | left, top |
|------|----------------------|-------------|-----------|
| 0 | the **overview**: the whole 56 x 56 map, 4 x 3 pixels per tile, in the map area (48, 14)..(271, 181), with the slots and strips | 14 x 12 (kept for the view values) | 48, 14 |
| 1 | the normal screen: the slot columns left and right, the map view between them, both strips | 14 x 12 | 48, 14 |
| 2 | the wide view: no slot columns, the map view over the whole width, both strips | 20 x 12 | 0, 14 |
| 3 | the full view: no slots, no top strip; the map view from the top of the screen | 20 x 14 (20 x 13 while the small panel shows) | 0, 0 |

World values: `opt_onscreen` (the mode), `view_x` / `view_y` (the view's map pixel origin), `view_w` / `view_h`, the
**small panel** flag (op 196 "Panel view" sets it; the strip drawing reads it), the **last wide mode** (signed byte,
start 0: the last mode 2 or 3 minus 2), the **old mode** (byte), the **overview filter** (byte, start 0: 1 = the
overview shows only the tiles whose byte 4 has bit 0x80) and the mouse position (x, y).

**Set mode (m)** (m = a signed byte):
1. If m = the current mode (compared as a number, before step 2): nothing happens.
2. The path marks go away (world_load.md "Path marks", force 0). If m < 0: m = 3; else if m > 3: m = 0.
   old mode = the current mode; the slot frame slot = 0x40 (none).
3. m = 2 or 3: the "panel moved" byte = 0x40 (none).
   - m = 3: view_y −= 14. Window 20 x 13 at (0, 0) when the small panel flag ≠ 0; else 20 x 14 at (0, 0) and the
     "full view shown" byte = 1.
   - m = 2: window 20 x 12 at (0, 14); small panel = 0. If the current mode is 3: view_y += 14, the current mode
     = 2 now (for the next step), and the interface strips are drawn (top 1, keep 0).
   - Then, if the current mode (as it is now) is 1: view_x −= 48. last wide mode = m − 2.
4. m = 1: if the current mode is 0: window 14 x 12 at (48, 14). Else (2 or 3): window 14 x 12 at (48, 14),
   view_x += 48, view_y += 14 when the current mode is 3, small panel = 0, and the strips are drawn (top 1, keep 0).
5. m = 0: if the old mode (as a signed byte) ≥ 2, the strips are drawn (top 1, keep 0). The mouse moves to the overview point of the
   tile under it: if mouse y > the old window's bottom row (top + view_h − 1), mouse y = that bottom row − 1; with
   the old window, col = (view_x + mouse x − left) / 16 and row = (view_y + mouse y − top) / 14 (both divisions
   toward zero); the new mouse place = (48 + 4 x col, 14 + 3 x row). Then window 14 x 12 at (48, 14), small panel = 0.
   In steps 3 (m = 2), 4 and 5 the strip flag ("The interface strips") = 1 first, before the strips are drawn;
   m = 3 leaves it.
6. mode = m; the view is clamped: view_x = 0 if < 0, else at most 896 − view_w; view_y = 0 if < 0, else at most
   784 − view_h (signed compares). The whole screen is drawn again on the next frame.

Note: the steps change view_x / view_y by 48 / 14 only on some paths (1 → 2 or 3: −48; 2 or 3 → 1: +48; → 3: −14;
3 → 1 or 2: +14), so a full round PgUp 1 → 2 → 3 → 0 → 1 leaves the view 48 pixels left and 14 pixels up of where
it was (original quirk, issue #2).

**Keys** (tactical screen, after the time-compression and message tests of the key handler):
- PgUp: mode < 3: Set mode (mode + 1); else Set mode (0).
- PgDn: Set mode (mode − 1) (from 0 this is −1: mode 3).
- Insert: in mode 0 overview filter = filter and 1; else overview filter = 1, Set mode (0).
- Tab: mode ≥ 2: Set mode (1); mode 1: Set mode (last wide mode + 2); mode 0: overview filter = filter xor 1.
- The end of a right-button look (cursor.md "Clicks" right step 4): Set mode (0), the overview.

**The overview click** (mode 0, a left click; the mouse point (x, y)): x is held in 48..270, y in 15..181 (y < 14
becomes 15); the mouse goes there. col = (x − 48) / 4, row = (y − 14) / 3 (toward zero), tile t = 56 x row + col;
view_x = 16 x (col − 7), view_y = 14 x (row − 7), each 0 if negative. t is brought into 0..3136 (+56 while < 0, −56
while > 3136). The mouse goes to (48 + 4 x (t mod 4), 14 + 3 x (t / 3)) (these formulas do not give the tile's overview
point, but the last step moves the mouse again), the cursor tile = t, then Set mode (old mode); last, the mouse goes
to the tile's corner in the view that came back: (16 x (t mod 56) − view_x + left, 14 x (t / 56) − view_y + top).
(Each of the three mouse moves is made whatever the mouse flag.)

**The overview mouse step** (mode 0: each pass of the tactical key handler with no key and the mouse inside the view
window, in place of the normal view's hover and clicks; cursor.md):
1. If the "mouse shown" byte is 0: show the mouse; mouse flag = 1.
2. T = 56 x ((y − 14) / 3) + (x − 48) / 4 (both toward zero), at most 3135 (no lower bound). S = the selected
   soldier. (Hide path marks first sets the overview cost, which is the cursor's AP number, to 0 and the cursor
   tile to 0xC41; cursor.md.)
3. If T ≠ the cursor tile: ap_timer = 800, Hide path marks (force 1), cursor tile = T.
4. If S +0x1B8 = 0: ap_timer = 0, Hide path marks (force 1).
5. o = the occupant of T.
6. **Left button** down: if o < 8 and S hp (+0x183) ≠ 0: when the hp of o is ≥ 15 (signed byte), Select merc (o,
   force 0, quote 1); else nothing. Else if o < 0x40: the status line "Not available!". Else **Walk (S, T)**
   (cursor.md "Walk": the same routine as the normal view's walk, with its walk confirm). Done.
7. Else **right button** down: wait until the right button is up, then the overview click above. Done.
8. Else (no button): overview colour = 0xAF when o < 8 and o ≠ the selected slot, else 0x4F. If S hp ≥ 15 (signed
   byte), ap_timer > 0 and o ≥ 0x40: ap_timer −= 10; when it is now ≤ 0: ap_timer = 0; overview cost = PathApCost (S,
   cursor tile, mark 1, extra 0, sneak = key_shift, byte177 = S +0x177, alt = key_alt) (the overview cost is the
   cursor's AP number; the path marks of the walk
   show in the overview picture); overview over = 1 when cost > S apLeft (signed byte +0x1A5), else 0xFF.
   So the path to a free tile shows after the mouse rests on it for 80 passes.

**The overview picture** (mode 0; drawn instead of the map view at each full draw of the tactical screen). The
picture goes into the back buffer (pitch 384) at (32, 28): tile t = 56 x row + col has its **overview point**
(32 + 4 x col, 28 + 3 x row). The pictures are the **mini tiles** of the sector's tile bank (tactical_map.md:
compiled sprites, 4 x 3 pixels): mini ground g = entry 669 + g, mini object o = entry 826 + o, mini overlay v =
entry 1082 + v. The **marks** are panel sheet elements of 2 x 2 pixels (sheet x, y): 0x6A (162, 138), 0x6B (182,
138), 0x6C (184, 138), 0x6D (186, 138), 0x6E (188, 138), 0x6F and 0x7A (both (190, 138)); sheet pixel 0 is not
written. The picture does not clear the buffer first. `roofs` = the game's roofs flag (on in normal play,
tactical_map.md "Drawing order"); `gv1`, `gv3` = the ground-visible tables of t1 and t3 (as the ground pass).

1. For i = 0 .. 3247 (56 places per row, 58 rows; the overview point of place i is that of tile i):
   a. If i < 3136, tile i (bytes t0..t4) is drawn at its point (x, y):
      - Filter on and t4 bit 0x80 clear: only mini ground 0.
      - Else: o = t1, except o = 0 when gv3 [t3] = 0 and roofs are on (the roof hides the object).
        Mini ground t0 when (gv1 [o] ≠ 0 and gv3 [t3] ≠ 0) or t3 = 0x73 or roofs are off.
        If o ≠ 0: when the t1 class of o (JAGGED.DAT `g_t1Class`) is 6, the **2 x 2 composite**: mini object o at
        (x, y); with the three overlay numbers (above-left, above, left) of the composite table entry o − 0x27
        (tactical_map.md), mini overlay "above" at (x, y − 3) if y − 3 ≥ 0, mini overlay "above-left" at
        (x − 4, y − 3) if x − 4 ≥ 0 and y − 3 ≥ 0, mini overlay "left" at (x − 4, y) if x − 4 ≥ 0 (a number 0 is drawn
        too). Other classes: mini object o at (x, y).
        Mini overlay t2 if t2 ≠ 0. Mini overlay t3 if t3 ≠ 0, t3 ≠ 0x73 and (t3 < 0x50 or roofs are on).
   b. The marks of tile j = i − 112 (two rows up), when i ≥ 112, at (x + 1, y − 5) of place i = (x_j + 1, y_j + 1):
      element 0x6F when the tile's ground item number n (byte 6) is below 200 and byte 6 of ground item record n
      (12-byte records) is above 0 (signed); then element 0x7A when the high four bits of byte 7 (the path mark
      number, select.md) are not 0.
      (Original bug, not visible: places 0 .. 111 read their path byte from address 7 of the machine's memory, not
      from a tile, and can draw element 0x7A at rows 23 .. 27 of the buffer, above the picture.)
2. For k = 0 .. 47: first soldier k, then (for k = 1 .. 15) effect record k:
   - Soldier k gets its mark when it is in the sector (+0x182 ≠ 0) and alive (+0x183 ≠ 0), and: k < 40: +0x9C > 0
     (signed) and +0x138 ≠ 0; k ≥ 40: (+0x9C > 0 or the "show all" option is on) and (sees_slot ≥ 8 or byte
     +0x112 + k of soldier sees_slot is 1). The mark = element (+0x1BD as a signed byte) at
     X = 32 + x / 4 (x = s16 +0x116, the division toward zero), Y = ⌊y × 0.2127659574468085 + 28⌋ toward zero
     (y = s16 +0x118; the factor is a 64-bit float a little above 10 / 47, so Y = 28 + ⌊10 y / 47⌋ for y ≥ 0).
   - Effect record k (combat.md "Drawing the effect records", count c = byte 0, kind = byte 11, x = s16 +5,
     y = s16 +7), when c ≠ 0: kind 0 draws mini overlay (byte c of its frame table) at (X, Y) of (x, y) as above
     (the library makes none). Other kinds: the **small explosion**: by kind, source S, source size sw x sh, pitch,
     drawn size W x H and key:
     | kind | source | sw x sh | W x H | key |
     |---|---|---|---|---|
     | 3 | E, column (c − 1) x 76 | 76 x 60 | 19 x 15 | 0x7B |
     | 2 | E, column (c − 1) x 76 | 76 x 60 | 9 x 7 | 0x7B |
     | other | S, column (c − 1) x 42 | 42 x 34 | 10 x 8 | 0xD3 |
     The W x H block at (0, 0) of the back buffer is set to 0; the source is scaled into it (the speech balloon's
     nearest scaling: step sx = (sw x 2^22) / W, sy = (sh x 2^22) / H, 32-bit unsigned; row j reads source row
     (sy / 2 + j x sy) >> 22, column i reads (sx / 2 + i x sx) >> 22); then the block goes to (X, Y) with
     X = 32 + x / 4, Y as for soldiers; pixels equal to the key are not written. (The block at (0, 0) stays: it is
     outside the picture.) After the drawing, the effect's hit step and the fire start of the normal view run
     (combat.md); they are game logic, not pictures.
3. The clock at (32, 28) when the clock switch and the blink byte are both ≠ 0 ("Pause (key P) and the clock").
4. The **overview cursor**. The game's draw of the tactical screen gives the overview colour (the overview mouse
   step) as `p` and no point; other callers (traverse prompts) can give a point (px, py) with px ≠ 0. The window
   rectangle is left .. right = left + view_w − 1, top .. bottom = top + view_h − 1 ("View modes").
   a. Nothing when px = 0 and the mouse is outside the rectangle (edges inside).
   b. With a point: X = px − 16 clamped to left .. right, Y = py + 14 clamped to top .. bottom. Else X = mouse x
      − 16, Y = mouse y + 14 (not clamped).
   c. If the overview cost ≠ 0 and combat is on: the cost as a number of 2 digits (left padded with `^`, as in the
      panel's numbers) by Text (not centred, x = X − 5, y = Y − 3, fg = the overview "over" value (1 or 0xFF),
      shadow = fg − 1, font 0, mode 300) with the back buffer as target (cursor.md "Text": clip width 384).
      (SPEC GAP (not seen: Set mode puts the mode-0 window at left 48 every time ("View modes", m = 0), so X ≥ 32
      and x ≥ 27): a number at x < 0, possible only with a window left edge below 21, is not described.)
   d. Else, when tactical_on (cursor.md) ≠ 0 and overlay_up = 0: sprite 413 + p of the tile bank at (X − 4, Y − 4).
5. Then the texts of the normal view (the "sees enemies" line and the banners, "Tactical messages") at the same
   places.

The screen shows the 224 x 168 block at (32, 28) in the map area (48, 14).

Evidence (2026-10-27): asm of 0x23B4C (was ShowTacticalModeBanner?, now OverviewRedraw) read: tile loop 0x23B8E ..
0x23E71 (marks pointer starts at 0, set to the tile map at place 0x70), banks 0xA96BC / 0xA9930 / 0xA9D30 =
g_tileSprites 0xA8C48 + 4 x 669 / 826 / 1082, composite 0x23A80 (table 0x93D60), soldiers and records 0x23E77 ..
0x2407A (constants 0x90468 = 10 / 47 as a double, 0x90470 = 28.0; __CHP before the store), the records' zoomed-out
path of 0x153C8 (0x155EA .. 0x156B6: FillRect 0x52AB4, ScaledBlit? 0x52B7C, BlitKey 0x522A0). Elements: table
0x93732.
Overview cursor (asm 0x240A1 .. 0x241A1 read 2026-10-27): arguments AL (1 from 0x1CB78) / DL (p = caller's AL,
[0x92DD4] from TacticalLoop 0x11205) / EBX px / ECX py; MouseInRect? 0x67C0C (inclusive edges; mouse 0x229D00 /
0x229D04); cost 0x229DEC, combat 0x92DCB, DrawNumber 0x36090 (fg dword [0x92E5C]), tactical_on 0x92DCC,
overlay_up 0x92DCE, sprite pointers 0xA92BC + 4 x p, DrawCompiledSprite 0x52DC8. Clock: 0x2409C DrawClock?
0x1632C (32, 28) when 0x92DE5 and 0x284F06 (set by 0x4B730).

Evidence: Set mode 0x248E4 (window 0x24880: cols 0x229CC8, rows 0x229CD0, view_w 0x229CBC, view_h 0x229CC0, left
0x229CB8, top 0x229CC4, right 0x229CD8, bottom 0x229CDC; clamp 0x24830; small panel 0x92E30, last wide mode
0x92F8A, old mode 0x92DB9, full view shown 0x92E34, slot frame slot 0x284F0E, panel moved 0x93448, redraw
0x284F14; path marks 0x1D8B8; strips 0x2F928 (1, 0); mouse set 0x681D0, done only when the mouse flag 0x284F13 ≠ 0,
which is 1 in tactical play). Keys: TacticalKeyHandler 0x13490, scan codes 0x49 → 0x1386A, 0x51 → 0x13891, 0x52
→ 0x137C8; Tab = jump table 0x13328 entry 9 → 0x13F7B; overview filter 0x92DC1 (read by the overview drawing
0x23B4C). Overview tile under the mouse 0x24488, overview click 0x244C8. Overview mouse step 0x24630 (was TacticalCursorTile?; called from TacticalKeyHandler's mouse branch when the mode byte 0x92DDF = 0, else UseItemAt? 0x1D9B8): mouse shown 0x284F0B, show 0x67BF0, buttons 0x229CF4 (1 left, 2 right), release wait 0x67C68, cursor tile 0x229D2C, ap_timer 0x229DD4, colour 0x92DD4, cost 0x229DEC, over 0x92E5C, SelectMerc? 0x1148C, StatusMessage? 0x27D24 ("Not available!" 0x9ABFD), AllMoveTraverse? 0x1CFEC (BL 0), PathApCost 0x1D32C. Live check (DOSBox-X, tac60, 2026-10-26):
PgUp from mode 1 gives modes 2, 3, 0, 1 with (view_x, view_y) = (256, 168) → (208, 168) → (208, 154) → (208, 154)
→ (208, 154).

### The small panel in the full view

In view mode 3 (the full view) a merc selection shows the bottom interface strip for a short time: the **small
panel**. World values: the **small panel** timer (i32, start 0; the flag of "View modes"), the **panel ends** byte
(signed, start 0), the **full view shown** byte (start 0) and the strip flag (start 0).

1. **Show** (select.md "Panel view (op 196)", in mode 3): window 20 x 13 at (0, 0); small panel = 3000; strip
   flag = 1; then the view scroll and mouse move of op 196.
2. **Countdown** (the timer, on each timer tick): if small panel > 0 and no merc portrait is lit (cursor.md
   `lit` = 0x40): small panel −= 10; if it is now ≤ 0: small panel = 0 and panel ends = 1. While the mouse lights a
   merc, the panel stays.
3. **End** (the key handler's mouse step in modes 1..3, while the mouse is inside the view window, after the hover
   step of cursor.md): if panel ends = 1: full view shown = 1, panel ends = 0, window 20 x 14 at (0, 0), then the
   view is clamped as in Set mode step 6. So the panel goes away at the first mouse step inside the map after the
   countdown, not at the countdown itself.
4. **Drawing**: while the window is 20 x 13 the bottom strip (BSTRIP.PCC at y 182) shows below the map; its boxes
   follow the strip rule (opt < 3 or small panel ≠ 0). The screen drawing (the tactical redraw) tests full view
   shown after the copy of the view: if it is ≠ 0, the rows 196..199 of the screen get colour 0, full view shown = 0
   and the strip flag = 0. So in the full view the 4 rows under the 196-pixel map are black.
5. When the inventory screen closes in mode 3, panel ends = 1, so the panel goes away at the next mouse step in
   the map (asm: the inventory screen wrapper 0x54D50 keeps the mode byte, runs the screen 0x53A9C and sets it at
   0x54E41; 0x54D1C, reached only by pointer, sets it too).

Set mode (above) sets small panel = 0 for modes 0, 1 and 2, and leaves it in mode 3 (window 20 x 13 while it is
≠ 0). The quick save keeps the four values (blocks 72..75, savegame.md).

Evidence (asm read 2026-10-27): Show = SelectSoldier? 0x17E86..0x17EF1 (SetViewWindow 0x24880 (0, 20, 13), small
panel 0x92E30 = 0xBB8, strip flag 0x92E36 = 1). Countdown = timer 0x5EDE0 at 0x5EF30..0x5EF6F (lit 0x284F0E, panel
ends 0x92E35). End = TacticalKeyHandler 0x142A4..0x142D6 (MouseInRect? 0x67C0C on the view rectangle at 0x14218;
full view shown 0x92E34; SetViewWindow (0, 20, 14); ClampView 0x24830). Drawing = TacticalRedraw 0x17B22..0x17B4E
(memset of screen 0x95678 + 0xF500, 0x500 bytes). Quick save / restore: 0x2E020..0x2E063 / 0x2D026..0x2D069.

### Arrow-key scroll

TacticalKeyHandler 0x13490 (asm read 2026-10-30; port viewmode::Scroll, diff test scroll_diff.py). State: steps dy
0x9344C / dx 0x9344D (signed bytes), hover timer 0x229D28 and delay 0x229D44 (ms; TacticalTimer? 0x5EE98 takes 10 from
both on each 10 ms tick, in the same gate as the blink timer), the handler's local "key scroll" byte.

1. Arrow key cases (scan codes): 0x48 up 0x137F1 dy = -14, 0x4B left 0x1382F dx = -16, 0x4D right 0x13850 dx = 16,
   0x50 down 0x13815 dy = 14 (14 / 16 = 0x229D10 / 0x229D0C, set once by 0x377B0). Each sets the key-scroll byte (and
   the direction byte 0x284F17: no reader). The mouse at a screen edge also sets dx / dy (0x14250..0x146A5; not in
   the port yet).
2. End of the handler 0x14721: no step -> hover = 200, done. Else go = Alt Scroll 0x1163BB ? Alt held : (Alt up and
   hover <= 0). If go or key scroll: when delay <= 0 or Fast Scroll 0x1163BA is on, step 0x1EDAC; else the steps wait.
   If neither: both steps = 0.
3. Step 0x1EDAC: cursor tile 0x229D2C = StepTile(tile, dx/16) then StepTile(tile, dy/14*56) (only for a step that is
   not 0), view x += dx, view y += dy, ClampView 0x24830, delay = 70, steps = 0, redraw 0x284F14 = 1.

### Mouse-edge scroll

TacticalKeyHandler 0x14201..0x14721 (asm read 2026-10-30; port viewmode::Scroll::mouse_edge + Play::scroll_pass,
diff test scroll_diff.py mouse cases). Runs after the key cases and only while 0x92DCC (tactical_on) is set (else the
handler returns at 0x141F5 and the end block does not run either). Window rect = left 0x229CB8, top 0x229CC4, right
0x229CD8 = left + 16 cols - 1, bottom 0x229CDC = top + 14 rows - 1 (SetViewWindow 0x24880); MouseInRect 0x67C0C is
inclusive. Mouse x / y = 0x229D00 / 0x229D04. Step sizes 0x229D0C (16) and 0x229D10 (14).

1. Mouse in the rect: mode 0 (overview) -> OverviewMouse 0x24630, no edge. Else UseItemAt? 0x1D9B8, then x < 1 ->
   dx = -16, x > 318 -> dx = 16; y < 1 -> dy = -14.
2. Mouse outside the rect (after the "left the map" and side-strip mouse moves): y < top -> y < 1 gives dy = -14 (else
   the top-bar clicks); y > bottom -> y >= 199 gives dy = 14 (else StripClick). Then x > left -> x >= 319 gives
   dx = 16 (else slot popups 4..7); x <= left -> x < 1 gives dx = -16 (else slot popups 0..3).
3. A step that is not set keeps its key value. Then the end block of "Arrow-key scroll": no key, so a mouse step runs
   only after the 200 ms hover time (Alt Scroll: only while Alt is held), then every 70 ms (Fast Scroll: every pass).

Port: the window sets `mouse_out` when the mouse leaves the window (a DOS mouse cannot leave the screen), so the
view does not scroll at the last edge point; the window log writes `window scroll x X y Y` for edge steps too.
Script line `pointer X Y` (`pointer -1` = the mouse leaves the window).

### Pause (key P) and the clock

State: `paused`, the **clock switch**, `lock_owned`, the **blink byte** and `blink_timer` (s32). The timer lowers
blink_timer by 10 per tick (as the other screen timers); the game loop does the **blink step** once per pass: if
blink_timer ≤ 0, the blink byte flips (0 ↔ 1) and blink_timer = 800 when it is now 1, else 400.

**Pause (on)**:
1. on ≠ 0 and paused = 0: **Lock**: if round_lock = 0: round_lock = 1, lock_owned = 1, clock switch = 1. Then (in
   any case) blink byte = 1, blink_timer = 800. The tactical screen is drawn again; paused = 1.
2. on = 0 and paused ≠ 0: **Unlock**: if lock_owned ≠ 0: round_lock = 0. Clock switch = 0, lock_owned = 0;
   paused = 0.
3. Then (also when nothing changed) the game waits for the mouse, then until the left and the right button are up.

While round_lock ≠ 0 the timer's game steps do not run (turns.md: soldiers do not move, the animations stop).

**Inputs** (tactical key handler): key `P` = Pause (1). While paused, any key = Pause (0) and nothing else (an
extended key's second code is read and dropped), and a left or right button press = Pause (0) and nothing else;
these tests come before the time compression ones. A left click on the sector box of the top strip (286, 3, 306,
11; "Clicks on the top strip") = Pause (1 when paused = 0; a press while paused ends the pause first).

**The clock** at back buffer point (x, y): for k = 0..7 the tile bank sprite 413 + n with n = 0xE1 + k (k < 7) or
0xD1 (k = 7), at (x, y + 14k) for k < 4 and (x + 16, y + 14(k − 4)) for k ≥ 4 (a wristwatch of 32 x 56). Then the
text `%2d:%02d` (clock_hour, clock_minute as signed bytes; turns.md "Day clock") by Text (not centred, x + 5,
y + 27, fg 1, shadow 0, font 0, mode 300) with the back buffer as target (clip width 384).

Where: the overview picture draws it at (32, 28) (step 3, before the cursor). The normal view draws it last in the
cursor pass at (16 × (T % 56) − view_x + 32, 14 × (T / 56) − view_y + 28) with T = the top-left tile of the view
(view_y / 14 × 56 + view_x / 16), = (32, 28) when view_x and view_y are multiples of 16 and 14. Both draw it
only when the clock switch ≠ 0 and the blink byte ≠ 0.

**Other users of Lock.** These boxes and waits call Lock (with the clock switch given below) before they show and
Unlock after they close; between the two the game loop does not run (no blink step, no drawing of the map):

| user | clock switch | between Lock and Unlock |
|---|---|---|
| the message box (every box of "Message box" and "Tactical messages") | 0 | the box and its wait; after Unlock the cursor tile (cursor.md) = 0xC41 |
| the speech balloon of a merc (both kinds of "Speech balloon") | 0 | the face, the text, the voice |
| Santino's balloon | 0 | his face and text |
| the battle box (strategic.md "The battle show") | 0 | the show |
| the inventory screen (inventory.md "The loop": its w_4b730 / w_4b764 = Lock / Unlock, host ops 228 / 229) | 0 | the whole screen |
| the stat popup (left button held on a bar area, "Stat popup") | 1 | the wait until the left button is up |
| key S of the tactical key handler ("Key S: all stat popups") | 1 | the popups and the wait for a key or a click |

So while one of them shows, round_lock = 1 (when it was 0; the Lock owns it then) and the timer goes on every
10 ms with that lock: the soldiers, the turn clock, the busy and fast timers and the front-end's animation timers
stand still (soldier_update.md "Timer tick" stops after its show timer step), but the show timer, the tile
marker's life (cursor.md "Tile marker", only round_lock2 gates it), blink_timer and the other screen timers run.
A walk order's marker of 1 s is therefore gone after a box that stays 1 s or more, and the walk then goes on from
the same tile. Lock also sets the blink byte = 1 and blink_timer = 800. A clock switch of 1 shows the clock only
when the map is drawn, which none of these users does.

Front-end: ja1front's window runs the time a box, balloon, battle show or stat popup hold stays on the screen
(and the inventory screen stays open) as that many timer ticks under Lock / Unlock when it closes; the script line `boxwait MS` does the same for MS ms.

Evidence: Pause? 0x1002C (paused 0x22A129), Lock? 0x4B730 (AL = clock switch 0x92DE5; round_lock 0x92DE0,
lock_owned 0x92DE2, blink byte 0x284F06, blink_timer 0x229D48), Unlock? 0x4B764, blink step TacticalLoop 0x11014,
timer 0x5EDE0 (−10), DrawClock 0x1632C, callers 0x2409C (overview) and 0x17387 (cursor pass), key 0x50 and the
sector box in TacticalKeyHandler 0x13490 (0x13E8F, 0x13513, 0x141C7, 0x145B6). Test: overview_diff.py (clock switch,
blink byte and time fuzzed), smoke step pause. Other Lock users (asm read 2026-10-27): ShowMessage?
0x27B98 (Lock 0x27BDA, Unlock 0x27CCA, then 0x229D2C = 0xC41), speech balloons FUN_000297D4 (0x29806 / 0x29997) and
ShowMercMessage? (0x29A24 / 0x29C23), ResolveSectorBattle (0x3E533 / 0x3EC7F), the inventory screen UseInventoryItem?
(0x53B06 / 0x54D0E), SantinoSpeech? (0x5F4D4 / 0x5F59E), stat popup FUN_00011714 (DrawPopupInventory? then Lock 1,
WaitLeftUp, Unlock: 0x119B8 / 0x119C2; also 0x11B63 / 0x11D29), TacticalKeyHandler (Lock 1, call 0x40024, Unlock:
0x13EA3 / 0x13EAD = key S, "Key S: all stat popups"). Test: smoke step box_lock (script `boxwait`).

### The front-end's tactical screen (port/ja1front)

- At sector entry each slot k's face buffer is filled from record k (face number +0x179): alive face (and the
  eye row +0x1AA) when hp > 0, else the dead face; a record with `inCurSector` = 0 (for example the empty place
  after a merc left the team) keeps a cleared buffer (all 0), as the original's game load (0x2884C: records not in
  the sector get a cleared face, 0x46AC4) and its face_clear (op 263) leave it. When a slot's face number changes
  (the team closed up), the same rule fills it again. Remap table 0 starts as loaded and keeps the state the
  drawing leaves.
- Each drawn frame: the slots are drawn first, then the map view goes on top at (48, 14). A merc whose face
  buffer holds an alive face and whose hp is now 0 or less gets the death-morph face (its last frame). A loud death
  (op 186 death_show with quiet 0) first plays the 10 frames in the merc's strip face: frame i into the face
  buffer, the screen drawn, then a wait of 10 ms (MercDies? 0x3A4A4: BlitCopy per frame, the slot redraw
  0x331A4 / 0x320A0, busy delay 0x67250(10)); the "is dead!" box follows (MsgMercText 0x3A65C, 800 ms), before
  the next death: two loud deaths in one batch of ticks show morph, box, morph, box (the port keeps a queue). Speaker slot = 0xFF (no speech balloons yet); AP warned = the world's
  per-slot flags. A slot whose +0x17F is 1 shows the switch view ("The switch view"; right button on a slot).
  Then the interface strips ("The interface strips", top = 1, keep = 0) with opt = opt_onscreen, combat, the
  current sector's bytes, strip flag 1, `selected`, turn slot = the world's turn slot when the port keeps one, else
  64, time meter width = `day_bar`, small panel 0, the front-end's blink byte (the one of the inventory screen),
  combat seen = a front-end byte (start 0). The drawing flips the blink byte while enemies are in the sector; the
  front-end keeps the new value only on the first frame after a blink step of the loop ("Pause (key P) and the
  clock": blink_timer ≤ 0, then 800 or 400 ms) and else puts the old value back, so the sector box changes at each
  blink step, as in the original: its blink step first calls the sector box step (0x33768 with AL = 0: while the
  sector's enemies byte ≠ 0 the blink byte 0x96068 flips and the sector box is drawn with element 0x63 (blink byte
  now 0) or 0x64 and the sector number; with no enemies nothing), then flips the clock's blink byte (TacticalLoop
  0x11014 .. 0x1103E, asm 2026-10-28). So the sector box changes its face after 800 ms and 400 ms in turn. Top-strip buttons: "Clicks on the top strip". The pressed face is drawn on
  each frame while the left button is down after a press on a reacting button; the action runs when the left
  button comes up (also when the mouse left the button meanwhile): `done` = the end-turn key D, `inv` = key I, `map`
  = key M, `abort` = key A (world_load.md section 8), `trav` = the traverse popup, `opt` = key F10 (the Control
  Panel). The sector box does nothing.
- Control Panel (key F10 / `opt`, "The Control Panel"): over a fresh frame; one pass each 10 ms; a left press acts
  when the button comes up, at the place of the press. The window holds the real mouse in the map view (it warps
  the mouse back while the window has the input focus) and the pass clamps the mouse position into it. Key F10
  puts the cursor item back first ("Cursor item back"). Quick Save and
  Quick Restore close the panel, then run savegame.md "Quick Save and Quick Restore: the steps". The Sound FX switch stops
  the tactical effects only (the screens' own sounds still play). Script line `opt X Y [L]` (one pass, L = left
  click; the first line opens it) and `opt esc` write `opt X Y hover H switch K value V close R` (K = −1: no
  switch changed, R = −2: still open); JA1_PANEL_PNG (with JA1_SCREEN_PNG) = the end screen with the last panel. Clicks on the strip boxes: "Clicks on the bottom strip"; while `cursor_drop` ≠ 0 the window holds the mouse inside x 1..318, y 190..197.
- Mouse: moving over a bar area makes that slot the hover slot (else 0xFF). A left click on a portrait area =
  choose merc k (world_load.md section 8 "Choose a merc", the same as key k + 1: SelectMerc (k, force = Alt held
  0x284F20, quote 1), so a merc not shown yet is first only shown, #29 L). SlotMouse 0x11714: the mouse over the
  face of the selected merc with +0x16D ≠ 0 (an open walk order whose path shows, select.md step 6) and the merc in
  the view (OnScreen 0x1FC34) draws "cont" (font 0, centred, face + (15, 12), fg 0x27, shadow 1); a left click
  then (0x119CC): hide the path marks, +0x16D = 0, +0x1A6 = +0x95, u16 +0x134 = u16 +0x96; if +0x1A6 ≠ 0 and the
  merc is at +0x120 (or +0x120 = 0xC41): Use cost (+0x114, +0x134, tile byte 6, +0x1A6), Not enough AP, the
  tactical click on +0x134; else the move order (+0x120, marker 1, sneak +0x1A8, +0x177, alt +0x196) (#29 M); a left button press on the bar area of slot k
  (k = the hover slot) when merc k is on the map (+0x138 not 0) and its hp (+0x183, signed) is above 0: the screen
  as drawn so far gets the stat popup of slot k (Stat popup, steps 1..3) and the game stops (no turns, no
  animation, no other input) until the left button is up; then the play goes on with the next normal frame (it
  covers the popup). Otherwise a bar-area press does nothing; a left click on the map view is the map click of section 8.
- `--play SAVE SEED --png OUT` writes the 320 x 200 screen of the entered sector.

## Front-end interface (test entry points of ja1front)

The front-end (port/ja1front) exports two C functions so that validate/diff/panel_diff.py `--port` and
panel_data_diff.py `--port` can test it against the original. `iso` = the CD image, `exe` = JA.EXE in the game
directory; MAIN.DAT is `DAT/MAIN.DAT` and the fonts are in that same game directory (file names in upper case:
`DAT/MAIN.DAT`, `JA_SMALL.FNT`, `JA_M_ENG.FNT`, `JA_BIG.FNT`). Return 0 = done; a negative value = an error (the
front-end's usual codes: bad argument, missing or short file).

```c
typedef struct {
    uint8_t slot;              /* 0..7 */
    uint8_t selected;          /* world selected */
    uint8_t combat;            /* world combat */
    uint8_t opt_onscreen;      /* world opt_onscreen */
    uint8_t speaker;           /* speaker slot, 0xFF none */
    uint8_t hover;             /* hover slot, 0xFF none */
    uint8_t ap_warned[8];      /* per slot */
    const uint8_t *faces;      /* 8 x 1024: the face buffers (32 x 32 each), slot order */
    uint8_t *soldiers;         /* 48 x 512 soldier records, in/out (the eye state +0x1AC) */
    uint8_t *table0;           /* 256 bytes, in/out: remap table 0 as the drawing leaves it */
} Ja1fPanel;

/* Draw one slot ("Drawing one slot") into panel (320 x 200, in/out). tileset 0..9 = the sector's tileset (icons). */
int32_t ja1f_draw_slot(const char *iso, const char *exe, int32_t tileset, const Ja1fPanel *p, uint8_t *panel);

/* Fill a face buffer for face number n ("Face buffer"): kind 0 = alive (also writes *eye_row), 1 = dead,
   2 = after the death morph. Only the bytes the loading writes change in out (1024 bytes, in/out). */
int32_t ja1f_face(const char *iso, const char *exe, int32_t n, int32_t kind, uint8_t *out, uint8_t *eye_row);

/* The stat popup of slot k ("Stat popup" steps 1..3): soldiers = 48 x 512 records (read only; merc k = record k,
   as in Drawing one slot); panel and screen are 320 x 200 each, in/out: the picture and
   the numbers go into panel at (173, 18), then the 100 x 42 block goes to screen. The loader's result of step 1 is
   not returned (the test does not compare it). */
int32_t ja1f_draw_popup(const char *iso, const char *exe, int32_t k, const uint8_t *soldiers, uint8_t *panel,
                        uint8_t *screen);
```

Table 29 (darken) is the loaded table 29 with all its overrides (the panel is drawn only in a sector).

```c
typedef struct {
    Ja1fPanel panel;           /* slot unused; selected, combat, opt_onscreen, faces, soldiers, table0 as above */
    int32_t tileset;           /* the sector's tileset (slot icons) */
    uint8_t top, keep;         /* "The interface strips" inputs */
    uint8_t sector;            /* current sector 0..59 */
    uint8_t mercs, enemies;    /* bytes 2 and 1 of its sector_state record */
    uint8_t strip_flag;
    uint8_t turn_slot;
    uint8_t blink;             /* in */
    uint8_t combat_seen;       /* in */
    int32_t small_panel;
    int32_t time_w;            /* the time meter width (day_bar) */
    uint8_t sel_in_play;       /* record `selected` +0x138, read by the caller (0 when selected >= 48) */
} Ja1fStrips;

typedef struct {
    int32_t load_result;
    uint8_t blink, combat_seen;
    int32_t mouse_x;           /* the stored mouse x: 3137 when step 1.1 ran, else the value passed in mouse_x_in */
} Ja1fStripsOut;

/* "The interface strips" into panel and screen (320 x 200 each, in/out); soldiers in/out (the pull-up of a worn-out
   item, as in the inventory strip boxes). mouse_x_in = the stored mouse x before the call. */
int32_t ja1f_draw_strips(const char *iso, const char *exe, const Ja1fStrips *v, int32_t mouse_x_in,
                         uint8_t *panel, uint8_t *screen, Ja1fStripsOut *out);
```
