# Tactical map: data files and how the static scene is drawn

The tactical map is 56 × 56 tiles. A tile is 16 px wide and 14 px tall (top-down view, no isometry), so a whole
map is 896 × 784 px. The play window shows 14 × 12 tiles (224 × 168 px) and always starts on a tile boundary.

## Files

**TERRAIN.DAT**: 60 records of 15681 bytes, one per sector (sector n = record n − 1).
Each record = 3136 tiles × 5 bytes (t0 t1 t2 t3 t4, meaning below), stored **from the last tile to the first**
(the first 5 bytes are tile 3135), then 1 byte: the record's **tileset** (0..9). Tile index = row · 56 + column.
When loading, the game also sets each tile's occupant to "none", its ground item to "none" and a flags byte to 0.

**TILES*n*.DAT** (one per tileset): a sprite bank.
- u32 count N (1327 in every tileset), then N u32 offsets, each relative to the end of the offset table.
- Entries 0..1292 are **compiled sprites**: 32-bit x86 machine code that writes the sprite's opaque pixels.
  A sprite is drawn at (x, y) of a buffer with pitch P by calling it with EDI = buffer + y·P + x and EBX = P.
  The code uses only `mov byte/word/dword [edi + k], imm` (k = column offset), `add edi, ebx` (next row) and `ret`;
  a 5-byte trailer (`nop`, u16 width, u16 height) usually follows the `ret` but is not needed for drawing.
  A port can translate each entry once at load time into a run list (row, column, bytes) by interpreting these three
  instructions (or by executing it in an emulator, which is how the reference renderer checks itself).
  Pixels never written are transparent; there is no transparent colour.
- Entries 1293..1326 are **raw 16 × 14 bitmaps** (224 bytes, row-major): shadow masks.
- Groups: 0..156 ground (t0), 157 + t1 objects, 413 + v overlays and roofs (v = t2 or t3), 1293 + k shadows.
  The bank also holds ~450 4 × 3 pixel tiles and UI icons (not used by the map drawing below).

**MISC\\TILE*n*.PAL**: 256 × RGB, 6-bit VGA values (0..63).

**MISC\\TILESET*n*.LST**: 256 bytes, replaces colour remap table 28 (below); a value 255 keeps the table's own byte
(and the overrides below are then not applied to that entry). Fixed overrides applied to each value taken from the
file, in this order, chosen by the ENTRY NUMBER i (the position 0..255 in the file), not by the value: if i is 0xA2
or 0xA3 the value becomes 0xA3; tileset 7: if i is 0xB9 the value becomes 0xBD; tileset 9: if i is 0xF9 the value
becomes 0xFA, and if i is 0xB9 it becomes 0xBD; then if i is 0x2E the value becomes 0x2F; then if i is 0xF5, 0xF7 or
0xF9 the value becomes 0xF9 (so tileset 9's entry 0xF9 ends as 0xF9). Remap table 29 gets
[0x27] = 0x2B, [0xF1] = 0xF2, [0x1B] = 0xC6.

**JAGGED.DAT**, after the sector tables, from byte 13424: 34 colour remap tables × 256 bytes, then three class tables:
157 bytes per t0 value, 256 bytes per t1 value (*t1 class*), 155 bytes per t2 value (*t2 class*, missing values = 0).

Two more per-value tables are fixed in the game (not in a file; the port needs them as data):
*ground-visible-by-t1* and *ground-visible-by-t3* (256 bytes each), the *shadow table* (per shadow id: raw bitmap
number, dx, dy as signed bytes) and the *2 × 2 composite table* (per t1 value from 0x27: three overlay numbers for the
tile above-left, above and left). The port reads them from the user's JA.EXE (v1.13) at these file offsets:

| table | JA.EXE file offset | layout |
|---|---|---|
| ground-visible-by-t1 | 0xDABF0 | 256 bytes, by t1 |
| ground-visible-by-t3 | 0xDACF0 | 256 bytes, by t3 |
| 2 × 2 composite | 0xDAE04 | 3 bytes per t1 value, starting at t1 = 0x27: above-left, above, left (overlay numbers) |
| shadow | 0xDCF08 | 3 bytes per shadow id 0..98: raw bitmap number (1293 + it), dx, dy (signed) |

## Where the files are (for a front-end)

The GOG install holds the CD image `JA1.gog` (ISO 9660, volume label JAGGED113; plain 8.3 upper-case names, a ";1"
version suffix may follow) and `JA.EXE`. Paths inside the image: `DAT/TERRAIN.DAT`, `DAT/TILES0.DAT` ..
`DAT/TILES9.DAT`, `DAT/JAGGED.DAT`, `MISC/TILE0.PAL` .. `MISC/TILE9.PAL`, `MISC/TILESET0.LST` ..
`MISC/TILESET9.LST`. JAGGED.DAT's remap tables start at byte 13424 (0x3470); the t0 class table follows the 34
tables (157 bytes), then the t1 class table (256 bytes), then the t2 class table (155 bytes; values 155..255 = 0).
The overrides of remap table 29 apply after loading, whatever the tileset.

**Palette**: each 6-bit value v of TILE*n*.PAL becomes the 8-bit value min(255, v × 4 + v / 16) (v << 2 | v >> 4).

**Back buffer**: one byte per pixel (a palette index), pitch 384, height 28 + 14 × 57 + 28 = 854 rows, all 0
before a redraw. A sprite pixel at buffer position (x, y) goes to offset y × 384 + x; a pixel whose offset is
outside 0 .. 384 × 854 − 1 is dropped (there is no other clipping: a pixel with x ≥ 384 lands on the next row).
Shadows use the same offsets. The view window is the 16 × cols by 14 × rows block starting at (32, 28).

## Tile bytes

| byte | meaning for drawing |
|---|---|
| t0 | ground sprite number |
| t1 | object (0 = none): sprite 157 + t1; the t1 class decides the layer |
| t2 | overlay (0 = none): sprite 413 + t2 |
| t3 | roof / interior: above 0x50 (except 0x73) = roof sprite 413 + t3; also hides what is under it |
| t4 | shadow: id = t4 & 0x7F, drawn when id < 99; ids below 0x40 cover 2 × 2 tiles |

## Drawing order (static scene)

Positions are relative to the window: tile (c, r) of the window has its top-left corner at (16c, 14r). "Window
tiles" means c = 0..13, r = 0..11. For another window size cols × rows, 13 means cols − 1, 14 means cols, 11 means
rows − 1 and 12 means rows. Later steps paint over earlier ones. **Roofs** is a game flag: 1 at program start, changed only by the debug key
Alt+Q (cursor.md "Debug keys (level 4)"), kept in the quick save (savegame.md) and loaded back with it. Shadows are
on. With roofs off (flag 0): step 1 draws every ground sprite t0, step 3 draws every tall object (and its ground
item) whatever *ground-visible-by-t3*[t3] is, and step 4 draws no roof sprite.

1. **Ground**, window tiles, row by row:
   - ground sprite t0 if (*ground-visible-by-t1*[t1] ≠ 0 and *ground-visible-by-t3*[t3] ≠ 0) or t3 = 0x73 or
     roofs are off;
   - object 157 + t1 if t1 ≠ 0, t1 class < 6 and t3 ≠ 0x73;
   - overlay 413 + t2 if t2 ≠ 0 and t2 class < 12;
   - the **ground item** of the tile (see "Ground items" below) if t1 < 0x75, at the tile corner;
   - the **path marks** of tile byte 7 (select.md): n = byte 7 >> 4: if n ≠ 0, sprite 644 + n when n < 10, else
     sprite 595; m = byte 7 & 0x0F: if m ≠ 0, sprite 652 + m. (Evidence: DrawGroundPass 0x16B42..0x16BAB, tables
     0xA9658, 0xA9594, 0xA9678 in the sprite pointer table 0xA8C48.)
2. **Shadows** (only while the Control Panel switch Shadows, save byte 21542, is on; off = no pass 2), for rows −1..12 and columns −1..13 (row −1 / column −1 only when the view is not at the map's top /
   left edge; row 12 only when it is on the map): id = t4 & 0x7F; if id < 99:
   - id < 0x40: shadow id at (+2, −2) from the tile corner, then ids id−1, id−2, id−3 at (−16, 0), (0, −14),
     (−16, −14) from that point (each only if its position is not left of / above the back buffer origin);
   - otherwise: shadow id at the tile corner.

   A shadow = raw bitmap (1293 + table bitmap number) placed at its table offset (dx, dy). Per pixel: 0 = leave the
   destination; 0x17 = darken: destination ← table28[destination]; other values v: destination ← table0[v].
   Quirk: if the shadow starts above the buffer top, the rows above are skipped but 14 rows are still drawn, reading
   on into the next bitmap.
3. **Objects and soldiers**, for rows −1..12 and columns −1..14 (only tiles on the map): if the tile's occupant
   byte (t5 & 0x7F) is below 0x30: the soldier of that slot (see "Soldiers" below, whether or not it is drawn),
   then, only for c ≥ 0 and r ≥ 0, if the up-right neighbour tile (map index − 55, plain linear index; nothing when
   it is below 0) has t1 = 0x41 or 0x60, that tile's object 157 + t1 again at (+16, −14) from this tile's corner
   (a wall piece in front of the soldier). A tile without a soldier gets no redraw. Then the object:
   - t1 class ≥ 7 (tall objects, e.g. trees), for c ≥ 0 and r ≥ 0 (window tiles AND column 14 / row 12; not
     column −1 / row −1): if *ground-visible-by-t3*[t3] ≠ 0 or roofs are off, sprite 157 + t1 at the tile corner, or at (−2, −1) for
     t1 > 0x82; then, only when t1 ≥ 0x75, the tile's ground item at (−3, −3) from where the object went (so
     (−3, −3) or (−5, −4) from the tile corner). An item on a tile with t1 ≥ 0x75 and a class below 7 is not drawn;
   - t1 class = 6 (any tile of the range): 2 × 2 composite: sprite 157 + t1 at (+2, −2), then from that point the
     composite table's overlays: "above" at (0, −14), "above-left" at (−16, −14), "left" at (−16, 0) (each skipped when
     its position would be left of or above the back buffer origin).
4. **Overlays and roofs**, window tiles: overlay 413 + t2 if t2 > 0x3F; roof 413 + t3 if t3 > 0x50, t3 ≠ 0x73 and roofs are on
   (evidence: g_roofsOn 0x92DDB tests at 0x16A67, 0x16F39, 0x17160).

**Ground items**: tile byte 6 = g. The tile shows an item when g < 200 and byte 6 of ground item entry g (12 bytes
per entry, 200 entries, the `ground_items` of the world) is above 0 as a signed byte (1 = seen; 0 = not yet seen by
a merc, soldier_update.md "Spotted item"; 0xFF = buried or in combat: hidden). The picture is compiled sprite
s of **DAT/SINV.DAT** (on the CD; same container as TILES*n*.DAT, 118 entries), s = byte 4 of the item type entry
(`item_types`[12 t + 4], t = byte 0 of the ground item entry). Drawn with the tileset's palette, no remap. *Evidence
(2026-10-27): DrawGroundPass 0x16B02..0x16B3A, DrawTileObjects 0x16FA6..0x16FD9; the bank 0xA6C48 is loaded by
LoadOffsetBank("DAT\\SINV.DAT", 0xA6C48, 0x1E98) in LoadTacticalSector? 0x2AD94.*

**Animated tiles** (t3 = 1..9, the front-end's tile animations, soldier_update.md "The front-end's part of the
update pass"): in pass 1, after the path marks of byte 7, a window tile with 1 ≤ t3 ≤ 9 draws tile animation k = t3:
if its count (s8) is above 0, sprite 413 + frames[count] (the animation's frame table, indexed by the count) at the
tile corner; else nothing is drawn and the tile's t3 becomes 0. *Evidence (2026-10-25): DrawGroundPass 0x16BAF..0x16BF4
(tile animations 0x229E24, 11 bytes each: s8 count, +7 frame table).*

Not covered yet (drawn by the game but not specified): soldier names and floating numbers (text),
tile flag marks, projectiles, cursors and banners.

## Soldiers

Soldier sprites are the raw bitmaps of `DAT/SH.DAT` (an offset bank like TILES*n*.DAT: u32 count = 1456, then the
offsets relative to the end of the offset table). Bitmap number = (facing − 1) × 182 + frame. A frame below 0x43
is 20 × 20 pixels, anchor (9, 12); a frame 0x43..0xB5 is 32 × 28 pixels, anchor (15, 16). Rows are stored top
down, the row length is the width.

Inputs: the soldier record (fields by name from defs/types.toml `soldier`), its **script word** (the animation
script word the soldier's script position points at: the frame it shows), and five game values: **show all**
(a flag: draw soldiers the player does not see), **sight viewer** v (0..255) and the **sight table** seen[v][slot]
(bytes), the **selected merc** slot, and the **shade table** number. Drawing one soldier:

1. If deadAndRemoved = 1 (dead), draw nothing.
2. f = (s8 facing − 1) as an unsigned 16-bit value. frame = script word.
3. If s8 `visible` (+0x9C) ≤ 0 and show all is off, draw nothing.
4. If slot ≥ 0x28 and v < 0x40 and seen[v][slot] ≠ 1, draw nothing.
5. If frameOverride ≠ 0, frame = frameOverride.
6. If frame ≥ 0x43: if frame ≥ 0xB6 draw nothing, else size 32 × 28, anchor (15, 16); otherwise size 20 × 20,
   anchor (9, 12). Write the anchor to drawAnchorX / drawAnchorY (the record keeps it).
7. x = s16 x2 − anchor x − view x + 32; y = s16 y2 − anchor y − view y + 28 (view x, view y = the view's top-left
   pixel on the map, 16 × c0 and 14 × r0). Plain 32-bit integer arithmetic from here on: x and y do not wrap
   at 16 bits.
8. code = anim[animIdx] (u16, the current animation code). If code is 1..3, 0xD or 0x2B and frame > 0x30:
   x + 1, y + 4.
9. If x < 32 − width, x > 16 × cols + 32, y < 28 − height or y > 14 × rows + 28: draw nothing.
10. Colour table k: 0x1B if shaded ≠ 0; else colour + 8 (8-bit) if slot < 8 and slot = selected merc; else colour.
11. If frame is 0x48 or 0x50: frame = frame + frameAdd (16 bits).
12. Draw bitmap f × 182 + frame at (x, y), width × height pixels: 0x18 = leave the destination; 0x1C = darken:
    destination ← shade table[destination]; other values v: destination ← table k [v] (k as a signed byte). Both
    table numbers name the remap tables 0..33 exactly as the static scene uses them: table 28 is the tileset's
    table 28 (TILESET*n*.LST overrides applied) and table 29 has its overrides. The shade table is usually 28.
    Same buffer rules as the static scene.

### Soldier texts

After step 12 (also when the bitmap is missing), with x, y after step 8 and the width w of step 6. Text = panel.md
"Text" into the back buffer, clip width 384, fg 0xFF, shadow 1, mode 300 (cursor.md "Text"):
1. If +0x1D2 = 0: when slot = lit or slot = named (cursor.md "Hover"): label_y = y + 8 when u16 tile (+0x114) is
   below 0x38 (the top tile row), else y − 3; label_x = x + w / 2 (C division). The cursor pass draws the label
   there (cursor.md "Drawing"; the full redraw set label_x = −1 before, so a soldier not drawn shows no label).
2. Else (a **hit number**, shooting.md TakeDamage step 7): hx = x + s16 +0xD8, hy = y + s16 +0xDA; when hy <
   14 × rows + 28, hx > 32 and hy > 28: Text (dmg_text[5 × slot], zero-ended, hx, hy, not centred, font 1). The
   soldier update moves it (soldier_update.md: +0xD8 and +0xDA − 1 every 4 counts of +0x1D2).
3. If slot < 8 and +0x195 ≠ 0: Text (the nickname +0x16E, x + w / 2, y + 0x12, centred, font 0).

Evidence (2026-10-29, asm DrawSoldier? 0x165E8 at 0x168BC..0x16990: lit 0x284F0E, named 0x95E0C, label_x / label_y
0x22A480 / 0x22A482, dmg_text 0x10E290 + 5 × slot (written by TakeDamage 0x392FC with itoa at 0x3951A)).

The back buffer has a margin: the window starts at (32, 28) inside a 384-pixel-wide buffer, so objects of the
border tiles may stick into the window by a few pixels (composites and shadows of column −1 reach 2 px in).
A port with a different buffer must keep that margin or clip the same way.

## Front-end test interface

The front-end library exports one call that draws a view with soldiers from a given game state (the tests feed it
states taken from the original):

```c
typedef struct Ja1fScene {
    const uint8_t  *tilemap;       /* 3136 tiles x 8 bytes (map index = row * 56 + column): t0, t1, t2, t3, t4,
                                      occupant t5 (slot | flag 0x80; 0x40 = none), t6 ground item, t7 marks */
    uint8_t        *soldiers;      /* 48 records x 512 bytes, layout "soldier"; drawAnchorX / drawAnchorY written */
    const uint16_t *script_words;  /* 48: each soldier's script word */
    const uint8_t  *seen;          /* sight table, 64 x 512 bytes: seen[v][slot] at v * 512 + slot */
    uint8_t show_all, viewer, selected, shade;
    const uint8_t  *ground_items;  /* 200 x 12 bytes (the world's ground_items) or NULL = no ground items drawn */
} Ja1fScene;

int32_t ja1f_render_scene(const char *iso_path, const char *exe_path, int32_t tileset, const Ja1fScene *scene,
                          int32_t c0, int32_t r0, int32_t cols, int32_t rows, uint8_t *out);
```

It draws steps 1..4 from `scene->tilemap` (not from TERRAIN.DAT) with tileset `tileset`'s files and tables, the
soldiers included, and writes the window like `ja1f_render_view` (same return codes; −2 = tileset not 0..9).

## Evidence

Soldiers: DrawSoldier 0x165E8 (its text calls: "Soldier texts"), bitmap table 0xA5588 (built by 0x381C0), SH.DAT
loaded by LoadTacticalSector 0x2AD8A; verified by `validate/diff/soldier_diff.py` (the original object pass with
soldiers on vs tools/sectormap.py, fuzzed soldier fields, 5 dumps).

Traced in the original: TacticalRedraw 0x176C8 and its passes 0x169E8, 0x16C48 (returns at once when g_shadowsOn 0x1163BC = 0, 0x16C54), 0x17028/0x16E3C, 0x17098; loaders
0x2A9FC, 0x36EA0/0x52E00, 0x371BC; tables 0x93B4C, 0x93C4C, 0x95E64, 0x93D60. Verified 2026-10-18 by
`validate/diff/tiles_diff.py`: the reference renderer `tools/sectormap.py` equals the original passes (run in
Unicorn on a tactical memory snapshot, soldiers stubbed) byte for byte on 600 random windows covering all 60 sectors
and all 10 tilesets.
