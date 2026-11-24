# Sector start

What the game does when a tactical sector is entered: every soldier slot is reset, the sector's enemies and ground
items come from the data files, enemies get their equipment, supply drops and processing plants change the map,
then the mercs, tappers and guards are placed. This page is written in batches; each batch is a set of library
entry points that a differential test runs against the original.

Notation as in [ai.md](ai.md): record fields are bytes (u8) unless a width is given; "s8 x" = the byte x read as a
signed value; "random(n)" = the host's random number 0..n-1 (n ≤ 0: 0, no other effect); NONE = tile 0xC41. A
soldier record is 512 bytes (layout "soldier" in defs/types.toml); its 12 items are 12-byte entries at +0x00,
+0x0C, ... +0x84 (item k at 12 × k). Item entry bytes: +0 type, +1 count, +2 condition, +3 attachment, +4
attachment count, +5 attachment condition, +6 a flag byte, +8..+11 the conditions of units 2..5 of a stack.

World fields used (port_api.md): `soldiers`, `item_types` (136 × 12 bytes: +5 max count, +6 stack class),
`weapons` (256 × 6 bytes: +1 the ammo item type of a gun), `day`, `difficulty`, and the batch-1 fields
`supplies`, `equip_skill`, `attach`.

## Batch 1: slot reset, item reset, equipment roll

```c
void ja1_slot_reset(Ja1World *w, const Ja1Host *h, int32_t record, int32_t slot);
void ja1_item_init(Ja1World *w, const Ja1Host *h, uint8_t *item, int32_t cond_mode, int32_t count_mode);
void ja1_equip(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t flag);
```

### Slot reset (`ja1_slot_reset`)

Runs at sector start for every slot k = 0..47 on record k; the test also calls it with a record r different from
the slot number k. s = soldier record r (the `record` argument), k = the `slot` argument (int32).

1. Bytes +0x1AB, +0xB9, +0x1BD, +0x1F3, +0x17F, +0x1A4, +0x1C7, +0x1AD, +0x1B9, +0x189, +0x1D2, +0x181, +0x1A6,
   +0x1A7 = 0.
2. +0x9C = 0xFF, +0x188 = 1, +0x1C8 = 0x78.
3. u16 +0x12A, +0xDE, +0xE0, +0x134 = NONE; u16 +0x1E8 = 4; u16 +0x10A = 0.
4. +0x17A = k (low byte).
5. u16 +0x124 = u16 +0x12A; u16 +0x114 = u16 +0x12A.
6. +0x180 = 1 if k > 3 (signed), else 0.
7. u16 +0xE4 = u16 +0xDE; u16 +0xE6 = u16 +0xE0.
8. If k ≥ 8 (signed): on record k (not s): +0x182 = 0 and +0x17E = 1; then on s: +0x1B1 = 0.

### Item reset (`ja1_item_init`)

`item` points to one 12-byte item entry (in a soldier record or anywhere else). DL = `cond_mode` and BL =
`count_mode`, low bytes only. t = +0 (item type); cls = item_types[12 t + 6], top = item_types[12 t + 5].

1. +3, +4, +5 = 0 (always, also for an empty entry).
2. If t = 0: +2, +1, +8, +9, +10, +11 = 0; stop.
3. DL = 1: +2 = random(31) + 70. DL = 2: +2 = 100. Any other DL: +2 unchanged.
4. If DL ≠ 0: +8, +9, +10, +11 = 0.
5. If cls = 1: if top = 99 or BL = 2, +1 = top; else if BL = 0, +1 = top when s8 +1 > s8 top; else if BL = 1, +1 =
   random(s8 top − 1) + 1 (a byte); any other BL: +1 unchanged. Stop.
6. Other classes: if s8 +1 > cls (cls unsigned 0..255), +1 = cls. Then if +1 = 0, +1 = 1.
7. For i = 0, 1, ... while i < s8 +1 − 1 (+1 read again each time): DL = 1: +8+i = random(31) + 70; DL = 2: +8+i =
   100; else: if +8+i = 0, +8+i = +2.

### Stack count (part of the equipment roll)

New stack count for item type t: if item_types[12 t + 6] ≠ 5, the count is 1 (no random number). Else r =
random(100): r < 1 → 5, r < 3 → 4, r < 10 → 3, r < 25 → 2, else 1.

### Equipment roll (`ja1_equip`)

Rolls an enemy's equipment. s = soldier record `slot`; flag = low byte of `flag` (the game passes 2 for an enemy
marked so in the sector's enemy list, else 0). "Item k" = the entry at s + 12 k; "item reset of item k with (d, b)"
= the Item reset procedure on that entry with DL = d, BL = b. "Swap old → new" = in items 7..11 (entries at +0x54,
+0x60, +0x6C, +0x78, +0x84), every item whose type (+0) is old gets type new.

Supplies: A = supplies[0], B = supplies[1], C = supplies[3] (u8; supplies[2] is not read here).

Tables (constants of the game):

| name | values |
|---|---|
| GUNS (index 0..7) | 1, 2, 3, 4, 8, 9, 10, 11 |
| THROWN (index 0..6) | 0x1C, 0x1C, 0x1C, 0x16, 0x16, 0x18, 0x1A |
| COUNTS (index 0..5) | 1, 1, 1, 2, 2, 3 |

1. If +0xBA = 4: item 2's type (+0x18) = 0 and item 5's type (+0x3C) = 0; stop.
2. If flag = 0: for k = 0..11: m = 1 if item k's condition (+2) is 0, else 0; item reset of item k with (m, m).
   If flag ≠ 0: bytes +0x00..+0x8F = 0.
3. Chance base c: lv = (+0x192 − 4) × 5 (+0x192 unsigned); d = (day < 5, signed) ? day × 5 − 25 : 0; c = the low 8
   bits of equip_skill[2 × difficulty] + d + lv, read as a signed byte (−128..127).
4. +0x1B1 = 1 if random(100) < A + c + 15, else 0.
5. If flag ≠ 0 or A ≠ 0:
   a. Gun: old = +0 (item 0's type). u = 7. Repeat: if random(100) < (7 − u) × 5 + c + B, stop; else u = u − 1, and
      stop when u = 0. new = GUNS[u].
   b. If new > old (unsigned): if old ≠ 0: swap old → new, and if 1 ≤ old ≤ 14 also swap weapons[6 old + 1] →
      weapons[6 new + 1] (the ammo types). Then +0 = new and item reset of item 0 with (1, 1).
   c. If item 1's type (+0xC) = 0: if random(100) < C + c + 15: +0xC = THROWN[random(7)], then +0xD =
      COUNTS[random(6)] (the two random numbers in this order), item reset of item 1 with (1, 0). Else: b = 25 if
      +0x1D3 = 5, else 0; if random(100) < A + b + 25: +0xC = 0x10 if random(100) < A + b + 10, else 0xF; item reset
      of item 1 with (1, 0). (Else item 1 stays empty.)
6. Armour and helmet: for (item 2, top 0x26, field +0x18) and then (item 5, top 0x30, field +0x3C): base = top − 3;
   for t = top down to base: if t − base < +0x1B4 (unsigned) and random(100) < A + c + 50, pick t and stop (no
   random number when the first test fails). If a t was picked: if the field ≠ 0, swap field → t; field = t; item
   reset of the item with (1, 0). If none: field = 0.
7. Item 6: if +0x1B4 = 4 and random(100) < A + c (no random number when +0x1B4 ≠ 4): +0x48 = 0x31; else +0x48 =
   0x32 + random(4). Item reset of item 6 with (1, 0).
8. For (item 4, field +0x30, high 0x2B, low 0x2A) and then (item 3, field +0x24, high 0x28, low 0x27), only when the
   field = 0: if random(100) < A + c: field = high; else if random(100) < c + 25 + A: field = low, and for item 3
   also +0x25 = 1; else nothing more for this item. When a type was set: item reset of the item with (1, 0).
9. Attachment, only when +3 (item 0's attachment) = 0: over the 82 attach entries e = 0..81 (4 bytes each): if
   attach[4e + 1] = +0, note its kind attach[4e]: kind 0x42 ('B') sets haveB, kind 0x5B ('[') sets haveL. Then:
   if haveB and random(100) < c − 5 + A: +3 = 0x42, +4 = 1, +5 = random(31) + 70. Then, if haveL and +3 ≠ 0x42 and
   random(100) < c − 10 + A: +3 = 0x5B, +4 = 1, +5 = random(31) + 70. (The random number of each line is drawn only
   when the tests before it pass.)
10. If flag = 0: for k = 7, 8, 9: if 0x36 ≤ item k's type ≤ 0x3E: its count (+1) = the stack count of that type,
    then item reset of item k with (1, 0). Then if +0xC = 0x1C and +0xE = 100: +0xE = random(50) + 50.
    If flag ≠ 0: dst = 7; for k = 0..5: if item k's type ≠ 0 and random(100) < 8 (no random number for an empty
    item): copy item k's bytes +0, +1, +2, +8, +9, +10, +11 to item dst; item reset of item dst with (0, 0);
    dst = dst + 1; stop the loop when dst > 9.
11. Last: if 1 ≤ +0 ≤ 14 and random(100) < (45 if flag ≠ 0 else 25): +0x84 = weapons[6 × (+0) + 1]; +0x85 = the
    stack count of type +0x84; item reset of item 11 with (1, 0).

All "random(100) < X" tests compare as plain integers (X may be negative or above 99).

Evidence: SlotReset 0x2B250, FUN_53708, 0x30A88, 0x2FCE4; validate/diff/ai_diff.py sections slot_reset and equip
(the original in Unicorn against the mirror tools/ai.py, 0 mismatches, mutation-tested).

## Batch 2: placing soldiers

```c
void    ja1_xra_enemy(Ja1World *w, const Ja1Host *h, int32_t slot);
void    ja1_init_tapper(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_init_guard(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing, int32_t orders,
                       int32_t anchor);
int32_t ja1_guard_tile(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t dir, int32_t dist);
void    ja1_place_merc(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t facing);
void    ja1_merc_entry(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t facing);
```

`slot` = the soldier record the procedure works on (s). Tile arguments: written to u16 fields as their low 16 bits
and read back as signed 16-bit values where the steps say s16; facing = its low byte. Helpers from ai.md "Shared
definitions": step(t, k), dirStep(d), occupant(t), tile_standable(t) ("Standable tile"), Occupy(t, slot) (in
"Frame routine" helpers: 0 if t > 0xC40; o = byte 5 of t & 0x7F, 0x40 if that is 0x30 or more; if o ≠ slot: if o
< 0x30, fail, else byte 5 of t = slot | (byte 5 & 0x80)). "Item reset of item k with (d, b)" = batch 1's Item reset
on the item entry k of the named record. New world field: `guard_skill` (u8 scalar, see port_api.md). The host op
51 replace takes the slot of the record (the `slot` argument, or the record named in the step).

### XRA enemy (`ja1_xra_enemy`)

Starts an enemy whose record was just loaded from the sector file.

1. +0x1E0 = random(4); +0x182 = 1; +0x138 = 1.
2. t = s16 of the u16 field picked by random(3): 0 → +0x114, 1 → +0x116, 2 → +0x118 (the three start tiles of
   the record).
3. If item 5's type (+0x3C) = 0x31: +0x3C = 0x30.
4. For items 7, 8, 9 (entries at +0x54, +0x60, +0x6C): type 0 → bytes +1 and +2 of the entry = 0; else, when the
   attachment byte +3 is 0 → bytes +4 and +5 = 0.
5. u16 +0x124, +0x114, +0x12E = t.
6. +0x185 = 0; +0x1BD = 0x6D; u16 +0x10C = 0; +0x18A = 100; +0x184 = +0x183; +0x17B = +0x17B + 4 (byte); +0x17C =
   random(8) + 2.
7. Occupy(t, +0x17A) (the result is not used).
8. Host op 51 replace (slot).

### Tapper (`ja1_init_tapper`)

1. +0x182, +0x138, +0x9C = 1; +0x17E = 0; +0x1BD = 0x6C; +0x17B = 0x19.
2. u16 +0x124, +0x114, +0x12E = tile.
3. +0x1A2, +0x1A1, +0x1A4 = facing.
4. +0x17C = random(8) + 2; then r = random(4).
5. +0x1D4 = 0, +0x1D3 = 0; +0x19C = 7; +0x1E0 = r; +0x192 = 1.
6. Stats, in this order, each = v + random(n) (a byte) with v = random(n) + base drawn first: +0x18B (base 30, n
   26), +0x18C (base 40, n 26), +0x183 (base 40, n 21), +0x18D (base 20, n 26).
7. +0x185 = 0; u16 +0x10C = 0; +0x18A = 100; +0x1F3 = 0; +0x184 = +0x183; u16 +0x1E8 = 0x11.
8. Occupy(tile, +0x17A). (No host call.)

### Guard (`ja1_init_guard`)

orders = low byte of `orders`; anchor = the anchor tile.

1. +0x182, +0x9C, +0x138 = 1; +0x17E = 0; +0x1BD = 0x6B; +0x17B = 0x17.
2. u16 +0x124 = tile; u16 +0x114 = tile; +0x1A2 = +0x1A1 = facing.
3. +0x17C = random(8) + 2; +0x1E0 = random(4); u16 +0x12E = anchor; +0x1D4 (orders) = orders; +0x1D3 (attitude) = random(6); +0x19C =
   7.
4. Level: n = s16 of ((low 16 bits of day) + 5); +0x192 = random(n) / 10 + 1 (a byte); if s8 +0x192 > 9: +0x192 = 9.
5. Stats, in this order, each = v + random(26) (a byte) with v = random(26) + base drawn first: +0x18B (base 40),
   +0x18C (40), +0x183 (40), +0x18D (20), +0x191 (30). Then +0x18E = v + random(16) with v = random(16).
6. a = ((+0x192 − 1) × 5) low 8 bits. If a ≠ 0: for +0x18B, +0x18C, +0x183, +0x18D, +0x191, +0x18E: field = field +
   a (byte); if s8 field > 100: field = 100.
7. c = (guard_skill + a) low 8 bits (unsigned 0..255). +0x1B1 = 1 if random(100) < c, else 0.
8. R = the record of slot +0x17A (normally s itself).
9. +0 (item 0's type, on s) = 1 + (1 if random(2) ≠ 0 else 0) + c / 5, at most 0xE (low 8 bits). Item reset of R's
   item 0 with (1, 2).
10. If random(100) < c + 50: +0xC = 0xF + (1 if random(100) < c + 25 else 0); item reset of R's item 1 with (1, 2).
11. If c ≠ 0: for (field +0x18, base 0x22, top 0x26, item 2) and then (field +0x3C, base 0x2C, top 0x30, item 5): q =
    (random(c) / 10) low 8 bits; if q ≠ 0: v = (q + base) low 8 bits; field = top if v > top, else v; item reset of
    R's item with (1, 0).
12. +0x185 = 0; u16 +0x10C = 0; +0x184 = +0x183; +0x18A = 100.
13. Occupy(tile, +0x17A); host op 51 replace (slot).

### Guard tile (`ja1_guard_tile`)

Finds a free tile `dist` steps from `tile`. d = d0 = low byte of `dir`; misses = 0; dist and tile as int32.

1. If dist > 27: result tile.
2. Loop:
   a. t = tile, then dist times (none when dist ≤ 0): t = step(t, dirStep(d)).
   b. c = t0_class[t0 of t].
   c. If tile_standable(t) > 0 and occupant(t) = 0x40 and c ≠ 3 and c < 8: result t.
   d. d = d + 1 (byte); if d > 8: d = 1. If d = d0: dist = dist + 1.
   e. If c > 7: misses = misses + 1; if misses ≥ 150: result tile.
   f. If dist > 27: result tile.

### Place merc (`ja1_place_merc`)

1. u16 +0x124 = tile; u16 +0x114 = tile; t = s16 +0x114.
2. +0x1E0 = +0x17A. Tile t: byte 5 = +0x17A (the whole byte, the 0x80 flag is lost); byte 4 = byte 4 | 0x80.
3. +0x1CF = 0; +0x9C = 1; +0x1A2 = +0x1A1 = facing.
4. Host op 51 replace (slot).
5. +0x1BD = 0x6A; +0x138 = 1.
6. Orders/attitude: if +0x179 > 0x3B (unsigned): +0x1D4 = 2, +0x1D3 = 2. Else if s8 +0x18E > s8 +0x191: +0x1D4 = 3, +0x1D3
   = 0. Else +0x1D4 = 4 and +0x1D3 by +0x19C: 3 → 5; 4 or 8 → 0; 2 or 6 → random(5); any other value → random(6).

### Merc entry (`ja1_merc_entry`)

Puts the mercs (records 0..7) on the entry tile and around it. sel = `selected`. placed[0..7] = no; last = −1;
ring2 = no; lead = unset (a tile).

1. If tile_standable(tile) > 0: Place merc (record sel, tile, facing); placed[sel] = yes; lead = s16 of tile.
   placed[] is an 8-byte stack array (issue #2 row 48). For sel 8 or more the byte 1 goes over other values before
   lead is set: sel 8..11 a byte of tile; 12..15 a byte of last; 24, 25 ring2 = yes; 32 facing = 1. Sel 16..23, 26..31
   and 33..47 change nothing that is read. LoadTacticalSector? gives sel 8 with no merc on duty.
2. Host op 58 reserve_ap (sel). Sel 48 or more: the port skips this step.
3. For k = 0..7, record S = k:
   a. If +0x182 = 0 or +0x138 = 0 or +0x183 = 0: +0x138 = 0; next k.
   b. base = tile. While placed[S +0x17A] is no:
      - For d = 1..8: c = step(base, dirStep(d)); with ring2 also c = step(c, dirStep(d)) once more. If
        tile_standable(c) > 0 and occupant(c) = 0x40: Place merc (S, c, facing); placed[S +0x17A] = yes; if S
        +0x17A = sel, lead = s16 of c; leave the d loop.
      - If a tile was found: go on with the while test (it ends the loop).
      - Else: j = last + 1; while j < 8 and placed[j] is no: j = j + 1. If j < 8: base = s16 of record j's +0x114;
        last = j; go on with the while loop. Else if ring2: give up on S (it stays unplaced). Else ring2 = yes and
        go on with the while loop (base unchanged).
      (last and ring2 are kept from one merc to the next.)
4. For i = 7 down to 0: the first i with placed[i] = yes and i ≠ sel: ti = s16 of record i's +0x114; Place merc
   (record sel, ti, facing); Place merc (record i, lead, facing); stop. (When lead is unset here the original
   uses an undefined value: such a case does not occur in the tests.)

## Batch 3: putting an item on the ground

```c
void ja1_put_item(Ja1World *w, const Ja1Host *h, int32_t tile, const uint8_t *item, int32_t b6, int32_t exact,
                  int32_t note, int32_t near);
```

`item` = a 12-byte item entry (read only); `tile` = a map tile 0..3135; b6, exact, note, near = their low bytes.
New world fields (port_api.md): `ground_items` (200 × 12 bytes: the ground item entries, same byte layout as a
soldier's item entry; entry k is free when its byte 0 is 0) and `notes` (15 × 2 bytes: item type, placed flag).
Tile bytes used: 0 ground (t0), 1 object (t1), 6 the ground item entry on the tile (0xFF = none). No host call
and no random number.

Helpers:
- **Water**(t): t0 of t ≥ 0x6C and t1 of t is neither 0x1C nor 0x1F.
- **Surface**(t): c = t1_class[t1 of t]. If c ≤ 5 or c ≥ 9: 1. Else 1 if t1 is 0x3B, 0x41, 0x5A or 0x60, else 0.
- **Item tile ok**(t, flag): g = byte 6 of t. If g ≠ 0xFF and (s8 byte 2 of ground entry g > 0 or s8 byte 6 of
  ground entry g ≠ −1): no. If Water(t): no. If flag ≠ 0: yes. Else yes when Surface(t) = 1.
- **Find item tile**(t, flag): if Item tile ok(t, flag): t. Else for r = 1..11, for d = 1..8: u = t, then r times
  u = step(u, dirStep(d)); the first u with Item tile ok(u, flag) is the result. None found: NONE.
- **Note placed**(type): for k = 0..14: if notes[2k] = type: if notes[2k + 1] = 1, the result is 1; else
  notes[2k + 1] = 1 and the result is 0 (the search stops at the first match). No match: 0.

### Put item (`ja1_put_item`)

1. type = item[0]. If type = 0: done.
2. If note ≠ 0 and Note placed(type) = 1: done.
3. If Water(tile) and item_types byte 12 × type + 13 ≠ 0 (the item sinks): done. (That is byte 1 of the next
   type's entry; type 135 does not occur here.)
4. k = the first free ground entry (0..199), or 200 when none is free.
5. dest = tile if exact ≠ 0, else Find item tile(tile, near).
6. If k < 200 and dest ≠ NONE:
   a. Byte 6 of dest = k.
   b. Ground entry k = the 12 item bytes; then its byte 6 = b6.
   c. If byte 6 of item type (entry byte 0) > 1 (unsigned) and s8 entry byte 1 > 1: for j = 0, 1, ...
      while j < s8 entry byte 1 − 1 (read again each time): if byte 8 + j of the entry is 0, it becomes entry byte
      2. (Byte 8 + j runs past the entry's 12 bytes for counts above 5; such counts do not occur in the tests.)
7. If dest ≠ tile: g = byte 6 of tile. If g < 0xFF and s8 byte 6 of ground entry g ≠ −1: byte 6 of tile = k
   (also when k = 200 or step 6 did not run), and byte 6 of dest = g, except when dest = NONE (the original
   writes past the map there; nothing to do).

## Batch 4: taking items off the ground

```c
void ja1_take_ground_item(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t keep);
void ja1_take_ground_patch(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t type);
```

keep, type = their low bytes. New world fields (port_api.md): `move_codes` (3136 × u16, the move code per tile, as
`ja1_rebuild_move_codes` fills it), `held` (12 bytes: the item entry on the mouse cursor), `blast_list` (30 × u16
tiles), `listed` (32 bytes: item types). Item type byte 0xA ≠ 0: the item keeps its condition when it is picked
up. New host op 85 cursor_back (no arguments, no result): the host puts back the item the cursor holds.

Helpers:
- **Listed**(type): type ≠ 0 and one of listed[0..31] = type.
- **Blast list remove**(t): the first k (0..29) with s16 blast_list[k] = t: blast_list[k] = 0xC41 (only that one).
- A tile outside 0..3135 has byte 6 = 0xFF here (the tests make it so: the original reads memory around the map).

### Take ground item (`ja1_take_ground_item`)

1. If tile < 0 or tile > 0xC40, or byte 6 of tile = 0xFF: done.
2. g = byte 6 of tile. If t1 of tile = 0x86: t1 of tile = 0, then all move codes are rebuilt (combat.md
   "Rebuilding all move codes", into `move_codes`).
3. Blast list remove(tile).
4. If keep ≠ 0: host op 85 cursor_back; then `held` = the 12 bytes of ground entry g; type = held[0]; if item
   type byte 0xA of `type` is 0 and type is not 0x6F, 0x57 or 0x58: held[2] = 100.
   Else (keep = 0): if Listed(byte 0 of ground entry g): done (the item stays).
5. Byte 6 of tile = 0xFF. If g < 200: bytes 0..6 of ground entry g = 0 (bytes 7..11 stay).

A ground entry g of 200 to 254 (a tile byte 6 of 200..254: Put item into a full table writes entry 200) is read
as combat.md Mine spotted, "G for g of 200 or more": the text buffer, then the profiles. This holds in Take ground
item (steps 4 and the Listed test), Take ground patch and Ground blast.

### Take ground patch (`ja1_take_ground_patch`)

Takes away a connected patch of ground items of one type.

1. Take ground item(tile, keep 0).
2. For d = −56, +1, +56, −1 in this order: t = s16 of the low 16 bits of tile + d (no map or row check). If byte
   6 of t ≠ 0xFF and byte 0 of ground entry (byte 6 of t) = type: Take ground patch(t, type).
   (With a listed type the item stays and the recursion never ends in the original: such cases do not occur in
   the tests.)

## Batch 5: soldier start

```c
void ja1_soldier_start(Ja1World *w, const Ja1Host *h, int32_t record);
```

The per-sector start of one soldier record (all of it, also for empty slots). s = the record; slot = s +0x17A
(u8). No host call. New world fields (port_api.md): `t2_class` (256 bytes: class per tile byte 2 value),
`enemy_watch` (48 bytes, one per slot). `remember_m`, `remember_e`, `last_seen_m`, `last_seen_e` are written here.
Width notes: "u16 f = v" writes the low 16 bits; "u32 f" a 32-bit little-endian field; all other fields are
bytes. NONE = 0xC41.

**Shot end (silent)**: u16 +0xDC, +0xDE, +0xE0, +0xE4, +0xE6, +0x1E6 = NONE; +0x1BF = +0x1C7 = +0x1FF = 0;
+0x1BC = 1; if busy > 0 (signed): busy = busy − 1.
**Clear target**: +0x1D8 = +0x1C2 = 0x40; +0x1BE = 0; u16 +0xD0 = u16 +0xDC = NONE.
**Armour bonus**: +0x1D0 = 1 if s8 +0x192 > 3, else 0. If +0x30 = 0x2B: +0x1D0 = s8 +0x1D0 + 5 − cdiv(100 − s8 +0x32,
20) (low 8 bits; cdiv = division truncated toward 0).
**Wakes**: if cur_sector = 0: 1. Else base by +0x1D4: 0 → 5, 1 → 10, 2 → 50, 3 or 4 → 25, any other value → 0;
the result is 1 if random(100) < s16 of the low 16 bits of (base + (s8 +0x192 − 1) × 5), else 0.

1. u16 +0x1EA, +0x1EC, +0x1EE, +0x1F0 = 1000.
2. Type and name: slot ≥ 0x28: if cur_sector = 0 and slot = 0x28: +0xBA = 4, name "SANTINO"; else +0xBA = 3, name
   "ENEMY %d". Slot 0x10..0x27: +0xBA = 2, name "TAPPER%2d". Slot 8..0xF: +0xBA = 1, name "GUARD %d". Slot 0..7:
   +0xBA = 0, no name (nothing written). The name (%d = slot in decimal, %2d = the same right-aligned in 2
   places with a space) and then one 0 byte go to +0x16E onwards.
3. These bytes = 0: +0x1CC, +0x1FA, +0x1D7, +0x130, +0x1DB, +0x1FC, +0x1E1, +0x9D, +0x16D, +0x1D1, +0x1D6, +0x1C1,
   +0x1C0, +0x197, +0x196, +0x193, +0x1D2. u16 +0x10E, +0x108, +0x132, +0x122, +0x120 = NONE.
4. +0x1BC = 1; +0x1E3 = +0x1E2 = +0x1E1; u32 +0xC0 = u32 +0xC4 = 0xFFFFFFFF; +0x1F7 = 0x40; u16 +0xEC = NONE;
   +0x1FF = +0x1BA = +0x1FE = 0; +0x198 = 0xFF; u16 +0x106 = NONE.
5. If +0x183 ≠ 0: +0x188 = 0. +0x1A8 = 0; +0x1AC = 2; +0x1A3 = 0; u32 +0xA4 = (10 − s8 +0x192) × 10; +0x177 = 0.
6. Shot end (silent); Clear target.
7. +0x1D5 = 0; u16 +0xDC = u16 +0x1E6 = NONE; +0x1F6 = 0; u16 +0x1F8 = NONE; +0x195 = 0; +0x1DA = 0x40; u16 +0xD0 =
   NONE; +0x1F2 = +0x1FD = 0.
8. If +0x182 ≠ 0 (the soldier is in the sector):
   a. +0x131 = 0; u16 +0xF0 = 0; +0x1C4 = +0x194 = 0; +0x1A5 = low 8 bits of Max AP (ai.md "Max AP"); Armour bonus.
   b. u16 +0x11A = 0xA; u16 +0x11C = 0xD. t = s16 +0x114 (a map tile). x = (t mod 56) × 16 + 5, y = (t div 56) × 14
      + 5; u16 +0x116 = u16 +0x126 = x; u16 +0x118 = u16 +0x128 = y.
   c. +0x1B4 = t1 of t; +0x1B5 = t1_class[+0x1B4]; +0x1B2 = t0 of t; +0x1B3 = t0_class[+0x1B2]; +0x1B6 = tile byte 2
      of t; +0x1B7 = t2_class[+0x1B6].
   d. +0x186 = +0x187 = 1 if s8 +0x183 < 15, else 0. +0x1B9 = 0; u16 +0xD6 = NONE; +0x1CA = +0x1A9 = +0x1AE = 0;
      u16 +0xD4 = 0; u32 +0xB0 = 100; u32 +0xBC = 0x32; u32 +0xA8 = random(300); u32 +0xAC = 100; u32 +0x1DC = 0; u32
      +0xB4 = 0xFFFFFFFF.
   e. If slot < 0x28: +0xB9 = 0, and if slot < 8 and +0x17E = 8: +0x17E = 0. Else: +0xB9 = Wakes; then
      enemy_watch[slot] = 1 if +0x1D4 is 3 or 4, else 0.
   f. u32 +0xCC = 100; +0x19F = 2; +0x1B8 = 1; u16 +0x136 = 0; +0x1A6 = +0x1AD = +0x1F3 = 0.
   g. p = 0x50 × u16 +0x1E8; u32 +0x90 = p (the script position, port_api.md); u16 +0xC8 = 0; +0x1C9 = 0; +0x1F4 =
      anim_scripts byte p; +0x1AF = 0; then Frame delay (ai.md "Frame delay") of this record.
   h. +0x1CD = 0; +0x162..+0x169 = 0; u32 +0xA0 = 500; +0x19E = 7 if +0x192 ≤ 2 (unsigned), else 8; if +0x24 =
      0x27: +0x19E = +0x19E + 1.
   i. If slot ≥ 8: +0x179 = 0x40, +0xEF = 0, +0x1B0 = 1. Else +0x1B0 = 0.
   Else (+0x182 = 0): +0x194 = 1.
9. +0x13A..+0x161 = 0.
10. If s8 +0xBA ≠ 2: the soldier's last-seen row = NONE (slot < 0x28: the 8 u16 of last_seen_m row slot; else the
    40 u16 of last_seen_e row slot − 0x28); then its remember row = 0 (slot < 0x10: the 8 bytes of remember_m row
    slot; else the 40 bytes of remember_e row slot − 0x28).
11. +0x1CF = +0x1A1 if slot ≥ 0x28, else 0.
12. For each item k = 0..11 whose type (byte 12 k) is 0: all 12 bytes of the entry = 0.

## Batch 6: map patches, supply drops, plants

```c
int32_t ja1_tile_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
int32_t ja1_item_patch(Ja1World *w, const Ja1Host *h, int32_t kind, int32_t number, int32_t count, int32_t bl,
                       int32_t cl);
void    ja1_crate_lost(Ja1World *w, const Ja1Host *h, int32_t drop);
void    ja1_crate_drops(Ja1World *w, const Ja1Host *h);
void    ja1_plant_sector(Ja1World *w, const Ja1Host *h);
void    ja1_plant_done(Ja1World *w, const Ja1Host *h);
```

bl, cl = their low bytes; count = a signed 32-bit record count. The library does not read files itself: a **patch
file** is opened, read and closed through host calls. New host ops (port_api.md):

| op | name | arguments | result |
|---|---|---|---|
| 86 | open_patch | int kind, int number, int count | int: −1 = no such file, else 0 |
| 87 | read | int n | int: the number of bytes read (0..n); the bytes go to `out` (n bytes room) |
| 88 | close | (none) | none |
| 89 | replay_event | int sector, int 0, int event | none |

kind: 0 = `MISC\CRATE<nn>.BIN`, 1 = `MISC\ITEM<nn>.BIN`, 2 = `MISC\DEADPL<nn>.BIN`, 3 = `MISC\LIVEPL<nn>.BIN`, with
<nn> = number as two decimal digits (more when it is 100 or more). Before op 86 the library writes this path text
and one 0 byte to the new world field `text_buf` (at least 32 bytes). Other new world fields: `item_buf` (12
bytes: the item entry a patch record reads), `crate_state` (4 bytes, one per supply drop: 1 dropped, 2 picked up,
3 lost to the enemy, 4 the enemy keeps it, 5 the sector is owned), `crate_shown` (4 bytes, one per drop: 0 / 1;
0xFF = not yet). `plants` becomes writable (byte 8 of a record = its state). `crates` (4 × 4 bytes): +0 sector
(s8), +1 day (s8), +2 patch record count (s8), +3 the enemy keeps it (u8). `supplies` [0..3] as in batch 1. A
sector is **owned** when bit 0 of byte 0 of its `sector_state` row is set. story[5] (s8) = the plant story stage.

### Tile patch (`ja1_tile_patch`)

1. Write the path to `text_buf`. If op 86 open_patch(kind, number, count) gives −1 (as int32): the result is −1.
2. rec = 4 bytes, all 0. done = 0.
3. count times (none when count ≤ 0):
   a. Read 2 bytes into rec[0..1], then read 2 bytes into rec[2..3] (each with op 87 read(2); a short read
      changes only the bytes it got, the others keep their old values).
   b. obj = u16 rec[2..3]. If cl ≠ 0 and 0x9E ≤ s16 obj ≤ 0xAA: u16 rec[2..3] = obj + 1 (this stays in rec).
   c. t = s16 rec[0..1]. If t ≠ 0 and t < 0xC40 (signed; negative tiles do not occur in the game files): t1 of t
      = rec[2]; then the move code of t is rebuilt from the tile (combat.md "Movement", the per-tile rule as `ja1_move_code`: ground t0, object t1,
      roof byte 3, previous code = move_codes[t]) into move_codes[t]; then if bl ≠ 0: byte 3 of t = 0x73. done =
      done + 1.
4. Op 88 close. The result is done.

### Item patch (`ja1_item_patch`)

1. Write the path to `text_buf`. If op 86 open_patch(kind, number, count) gives −1: the result is −1.
2. rec = 2 bytes, all 0. done = 0.
3. count times:
   a. Read 2 bytes into rec (op 87 read(2), short read as above); then read 12 bytes (op 87 read(12)) into
      item_buf (only the bytes it got are written).
   b. t = s16 rec. If t = 0 or t ≥ 0xC40 or item_buf[0] = 0: next record (not counted).
   c. If bl ≠ 0: Take ground item(t, keep 0) (batch 4).
      Else: if cl ≠ 0 and byte 6 of t ≠ 0xFF: g = byte 6 of t; Put item(t, a copy of ground entry g, b6 = byte 6
      of that entry, exact 0, note 0, near 0) (batch 3); then byte 0 of ground entry (byte 6 of t, read again) =
      0. Then Put item(t, item_buf, b6 = item_buf[6], exact 1, note 1, near 0).
   d. done = done + 1.
4. Op 88 close. The result is done.

### Crate lost (`ja1_crate_lost`)

k = drop (0..3). If s8 crate_state[k] = 3: done. Else crate_state[k] = 3 and the enemy gains supplies (each a
byte add): drop 0: supplies[0] + 10, [1] + 15, [2] + 10, [3] + 5; drop 1: [0] + 10, [1] + 25, [2] + 25; drop 2:
[0] + 10, [1] + 5, [2] + 5, [3] + 20; drop 3: nothing more.

### Supply drops (`ja1_crate_drops`)

"Crate tiles (B, C)" = Tile patch(kind 0, number sec, count n, B, C); "crate items (B, C)" = Item patch(kind 1,
number sec, count n, B, C). For each drop k = 0..3 with sec = s8 crates +0 equal to cur_sector (compared as
int32): dday = s8 +1, n = s8 +2, owned = sector sec owned.

1. If dday = day:
   a. crate_state[k] = 0: if owned: crate_state[k] = 5. Else crate tiles (1, 0), crate items (0, 1),
      crate_state[k] = 1, crate_shown[k] = 0. Next drop.
   b. s8 crate_state[k] = 1: if owned: crate_state[k] = 2. Else if crates +3 ≠ 0: crate_state[k] = 4. Else crate
      tiles (0, 1), crate items (1, 0), Crate lost(k), crate_shown[k] = 1. Next drop.
   (Any other state goes on to step 2.)
2. If dday ≥ day: next drop. Else, with s = s8 crate_state[k], f = s8 crate_shown[k]:
   - s = 4 and f = −1: crate tiles (1, 0), crate items (0, 1), crate_shown[k] = 0.
   - s = 3 and f = −1: crate tiles (1, 1), crate_shown[k] = 1.
   - s = 3 and f = 0: crate tiles (0, 1), crate items (1, 0), crate_shown[k] = 1.

### Plant sector (`ja1_plant_sector`)

cur = cur_sector. For k = 1..3: P = plant record k (12 bytes); sec = the u32 at P + 4. The first k with sec = cur:

1. If s8 P + 8 ≠ 2: done. nt = s8 P + 9, ni = s8 P + 10.
2. If s8 story[5] < 2 (the dead plant): Tile patch(kind 2, number sec, nt, 0, 0); Item patch(kind 1, number sec,
   ni, 1, 0); P + 8 = 0; op 89 replay_event(s8 cur as int32, 0, 0x51). Done.
3. Else (the live plant): Tile patch(kind 3, number sec, nt, 0, 0); owned = sector cur owned (0 or 1); Item
   patch(kind 1, number sec, ni, owned, 0); if not owned: P + 8 = 3. Done.

No k matches: nothing.

### Plant done (`ja1_plant_done`)

The first k = 1..3 with the u32 at plant record k + 4 = cur_sector: if s8 P + 8 = 3: P + 8 = 0 and op 89
replay_event(s8 cur_sector as int32, 0, 0x51). Any other state, or no match: nothing.

## Batch 7: fixed items, the sector tmp file, ground blast

```c
void    ja1_fixed_items(Ja1World *w, const Ja1Host *h);
int32_t ja1_tmp_file(Ja1World *w, const Ja1Host *h);
void    ja1_ground_blast(Ja1World *w, const Ja1Host *h, int32_t type, int32_t flag, int32_t power, int32_t thrower,
                         int32_t last);
```

type, flag, thrower, last = their low bytes; power = a signed 32-bit value. New world fields (port_api.md):
`fixed_items` (read only, 11 × 8 bytes: +0 tile (s16), +2 item type, +4 count, +6 condition; the other bytes are
not read), `sectors` (60 × 0xB1 bytes, one row per sector; byte 0xB0 of a row = the sector's count of set mines;
cur_sector < 60 here), `bomb_count` (int32: type 0x3F ground items set off so far). New host ops:

| op | name | arguments | result |
|---|---|---|---|
| 90 | explode | byte thrower, int tile, int kind, int 1 | none (an explosion at the tile) |
| 91 | effect | int 0xC41, byte type, byte flag, int power, byte thrower, int last | none (the blast effect starts) |
| 92 | open_tmp | int sector | int: −1 = no such file, else 0 |

The tmp file (op 92) is the saved state of a sector the player left (`sec<nn>.tmp`). It is read with op 87 read
and closed with op 88 close (batch 6). The host serves a whole file: a short read does not occur here.

### Fixed items (`ja1_fixed_items`)

For i = 0..10, F = fixed_items + 8 i:

1. item = 12 bytes: [0] = F[2], [1] = F[4], [2] = F[6], [3..11] = 0.
2. Item reset(item, cond_mode 0, count_mode 0) (batch 1). The count loop of step 7 there can write past byte 11 for
   counts above 5: give `item` room for 12 + 256 bytes; only bytes 0..11 go on.
3. Put item(s16 F + 0, item, b6 0, exact 0, note 0, near 0) (batch 3).

### Tmp file (`ja1_tmp_file`)

1. If op 92 open_tmp(cur_sector, as an unsigned byte) gives −1 (as int32): the result is 0.
2. For each tile t = 0..3135, for each byte o = 1, 2, 3, 4, 6 in this order: op 87 read(1); byte o of tile t = the
   byte read.
3. Op 87 read(0x960): the 0x960 bytes are ground_items bytes 0..0x95F (all 200 entries).
4. Op 88 close. The result is 1.

### Ground blast (`ja1_ground_blast`)

Sets off (or, for mines, takes away) the ground items of one type.

1. If power < 100 (signed) and type = 0x11: power = random(1000) + 500.
2. more = 0. For each tile t = 0..3135 in order:
   a. g = byte 6 of t. If g = 0xFF: next tile. E = ground entry g (g of 200 to 254: past the 200 entries as
      combat.md Mine spotted, "G for g of 200 or more"; reads and the E[1] write). If E[0] ≠ type: next tile.
   b. Type 0x11 (a mine): Take ground item(t, keep 0) (batch 4); m = byte 0xB0 of sectors row cur_sector; m =
      m − 1 (a byte), written back to the row; more = 1 when m ≠ 0,
      else 0.
   c. Any other type:
      - chain = 0. If the type is 0x29, 0x3F, 0x6E or 0x71: if 0x6E: E[1] = 1. If 0x6E or 0x71: Plant done
        (batch 6). chain = 1 unless the type is 0x6E.
      - Op 90 explode(thrower, t, s8 E[1] + 1 (as int32), 1). (E[1] is read after the write above.)
      - If the type is 0x3F: bomb_count = bomb_count + 1.
      - If chain = 1: if bomb_count ≥ 3 (signed) and s8 flag < 1: flag = 1. If the type is 0x29: flag = 0. Op 91
        effect(0xC41, type, flag, power, thrower, s8 last (as int32)).
   d. If more = 0: stop (so every type but 0x11 stops after its first tile).

## Batch 8: the whole sector start

```c
void ja1_setup_sector(Ja1World *w, const Ja1Host *h, int32_t tile, int32_t facing);
```

tile = the merc entry tile, facing = its low byte. This runs every earlier batch in the game's order. The host
keeps the per-sector state that is not in the world (screen, sound, effect and user interface state): ops 93..95
tell it when to reset that state. New world fields (port_api.md): `purifier` (u8, read and written: the next
sector the player does not own gets the Micro-Purifier), `team_on_duty` (int32, read only: mercs on duty),
`scratch` (0x200 bytes: where the templates of absent enemies go). `story` becomes writable. New host ops:

| op | name | arguments | result |
|---|---|---|---|
| 93 | sector_resets | (none) | none. The host resets its own per-sector state (no random draw: Setup sector step 1 draws it for speech_wait) |
| 94 | keep_selected | byte slot | none (the host notes which merc is selected) |
| 95 | sector_tail | (none) | none. The host resets the rest of its per-sector state |
| 96 | seek | int pos | none (the open file goes to byte pos) |

Kind 4 of op 86 open_patch = `DAT\XRA.DAT` (number 0, count 0); for kind 4 the library does NOT write
`text_buf` (the original passes a fixed text). XRA.DAT holds one record of 0x2E98 bytes per sector (sectors
0..58): 16 soldier templates of 0x200 bytes, then 3136 tile bytes, then 50 ground items of 12 bytes. The host opens the game-folder file first and the CD file only when it is absent (OpenCDFile 0x38304:
FileExists_ on the bare path; the port target 1.13b puts its own DAT\XRA.DAT in the game folder, 4 bytes differ
from the CD: records 1, 22 and 23). The open result is not checked (the file is always there). GUARD_DIRS = 2, 6, 4, 8, 1, 5, 3, 7. NO_TILE = 0xC41. cur =
cur_sector, S = sectors row cur (0xB1 bytes), T = sector_state row cur (9 bytes). Bytes are unsigned unless
marked s8 / s16.

### Setup sector (`ja1_setup_sector`)

1. **Resets.** Op 93 sector_resets. Then in the world:
   - turn_based = 0, bomb_count = 0, show_ai = 0, santino_timer = 0, anim_signal = −1.
   - mission_slot = 0x40, last_spotter = 0x40, active_slot = 0x40.
   - calls: u16 +0 = NO_TILE, +2 = 0, u16 +4 = NO_TILE, +6 = 0 (+3 and +7 stay).
   - knowledge[0..47] = 0 (bytes 48..63 stay). remember_e (all 320 bytes) = 0. remember_m (all 128 bytes) = 0.
   - blast_list: all 30 entries = NO_TILE.
   - interrupt_go = 0, turn_moves = 0, quote_focus = 0, quote_delay = 0, ap_warned (all 8 bytes) = 0.
   - next_selected = 0x40, quote_slot = 0x40, spotted = 0x40.
   - Turns (turns.md fields): itq (all 50 bytes) = 0x40, itq_len = 0, bled (all 48 bytes) = 0, i32 +0 of each of
     the 5 timed_fx entries = −1 (their other bytes stay), new_round = 1, turn_timer = 0, turn_timer2 = 0,
     shown_slot = 0x40, view_tile = NO_TILE, focus_slot = 0x40, view_hold = 0, interrupt_turn = 0.
   - Update end fields (soldier_update.md): ambient_count = 0xF8.
   - New turn fields (turns.md, New turn): turn_clock = 20000, show_timer = 0, handed_over = 0, interrupter = 0x40,
     speech_wait = random(3) + 1 (8-bit). (This is the one random(3) of the resets: op 93 no longer draws it.)
   - Shooting fields (shooting.md): in_flight = 0, replay_m = 0, marked_slot = 0x40.
   - Blast fields (combat.md Blast, exact steps): blaster = 0x40; the tile word of each of the 5 gas_clouds = NO_TILE
     (their other bytes stay).
   - alarm = 1 if bit 0 of T[0] is set, else 0.
2. For k = 0..47: Slot reset(record k, slot k) (batch 1).
3. tmp = Tmp file (batch 7). Then all move codes are rebuilt (combat.md "Rebuilding all move codes").
4. If bit 0 of T[0] is 0 (the player does not own the sector):
   a. **XRA load.** Op 86 open_patch(4, 0, 0); op 96 seek(cur × 0x2E98). Pass 1 (want = 2), then pass 2
      (want = 1): for k = 0x28..0x2F: read 0x200 bytes (op 87 read(0x200)); if s8 S[0x6E + k] = want, they
      become soldier record k, else they go to `scratch` (each overwrites the one before). In pass 1 only: then bytes 0x54..0x5F and 0x60..0x6B
      of record k = 0 (its items 7 and 8). Then (both passes) record k +0x182 = 0 and +0x17A = k.
   b. If tmp = 0: **ground items load.**
      - For t = 0xC3F down to 0: byte 6 of t = one byte read (op 87 read(1)).
      - For i = 0..49: E = ground entry i = 12 bytes read (op 87 read(12)). If E[3] = 0: E[5] = 0, E[4] = 0.
        If E[0] ≠ 0 and item_types[12 E[0] + 6] > 1 and s8 E[1] > 1: for j = 0, 1, ... while j < s8 E[1] − 1
        (E[1] read again each time), with U = E[8 + j] and base = s8 E[2]:
        - base < 0: U = (random(10) − 5 + E[2]) as a byte; if s8 U > −1: U = 0xFF; if s8 U ≤ −100: U = E[2].
        - else: if base < 100: U = (random(10) − 5 + E[2]) as a byte. Then if s8 U > 100: U = 100; if s8 U ≤ 0:
          U = E[2].
      - For t = 0..0xC3F: g = byte 6 of t; if g ≠ 0xFF and Note placed(byte 0 of ground entry g) = 1 (batch
        3; it marks a note that is not yet placed): Take ground item(t, keep 0) (batch 4).
   c. Op 88 close.
   d. If purifier ≠ 0 and cur ≠ 0x3B: S[0x9E] = 0x79, purifier = 0.
   e. given = 0. For i = 0..7, with k = 0x28 + i: if s8 S[0x96 + i] > 0: XRA enemy(k) (batch 2); Equipment
      roll(k, flag 1 if S[0x96 + i] = 2 else 0) (batch 1); then if it = S[0x9E] ≠ 0 and given = 0: record k
      +0x6C = it; q = item_types[12 it + 5]; +0x6D = q if s8 q < 99 else 99; +0x6E = 100; +0x6F, +0x70, +0x71,
      +0x72 = 0; given = 1.
5. Supply drops (batch 6). Plant sector (batch 6).
6. mission_step = the low byte of op 33 radio_result.
7. **Story objects and items.** For k = 0..4:
   a. t = s16 S[0x82 + 2k], b = S[0x9F + k]. If t ≠ NO_TILE and b ≠ 0: if b is 0xAC or 0xAD and story[11] = 2:
      t1 of t = 0, S[0x9F + k] = 0, u16 S[0x82 + 2k] = NO_TILE, story[0x17] = 0xFF. Else t1 of t = b. Then the
      move code of t is rebuilt (the per-tile rule, as in batch 6 Tile patch step 3c).
   b. t = s16 S[0x8C + 2k]. If t ≠ NO_TILE: item_buf[0] = S[0xA4 + k] (bytes 1..11 keep their old values);
      Item reset(item_buf, cond_mode 1, count_mode 2); if item_buf[0] = 0x86: item_buf[3] = 0x69, item_buf[4]
      = 99, item_buf[5] = 0x64. Put item(t, item_buf, b6 0xFF if item_buf[0] = 0x5E else 0, exact 1, note 1,
      near 0). S[0xA4 + k] = 0; u16 S[0x8C + 2k] = NO_TILE.
8. While S[0xAE] ≠ 0 and S[0xAD] ≠ 0: S[0xAE] − 1, S[0xAD] − 1.
9. If S[0xB0] ≠ 0: Ground blast(0x11, 0, 0, 0x40, 0) (batch 7).
10. **Trees.** For t = 0..0xC3F, b = t1 of t, when b is 0x35 or 0x36 (tree tiles lie at t < 3080 here):
   - If S[0xAC] ≠ 0: t1 of t = 0x34; byte 4 of tile t + 0x38 = (that byte) AND 0xB7; S[0xAC] − 1; if b = 0x36:
     S[0xAD] − 1.
   - Else if b = 0x35: if S[0xAE] ≠ 0: t1 of t = 0x36, S[0xAE] − 1.
   - Else (b = 0x36): if S[0xAD] ≠ 0: t1 of t = 0x35, S[0xAD] − 1.
   (All counts are bytes.)
11. If team_on_duty ≠ 0: Merc entry(tile, facing) (batch 2).
12. merc_to_show = selected. (This replaces the former host op 94 keep_selected, which is no longer called.)
13. **Tappers.** groups = (T[3] + 2) / 3 (integer). done = 0, done3 = 0, slot = 0x10. For t = 0xC3F down to 0:
    if t1 of t = 0x36 and done < groups: r = random(4) (low byte); d = 1; for j = 0 .. min(T[3] − done3, 3) − 1:
    if j = r: d = d + 2; Tapper(slot, step(t, dirStep(d)), (d + 3) mod 8 + 1) (batch 2); slot + 1; d = d + 2.
    Then done3 + 3, done + 1. (All values here stay small: d ≤ 9.)
14. **Guards.** gd = T[4]. per = 0 if gd = 0 or groups = 0; else 1 if gd < groups; else gd / groups (integer).
    per = min(per, 3). slot = 8, placed = 0.
    a. If per ≠ 0: for t = 0xC3F down to 0 with t1 of t = 0x36: for m = 0..per − 1: d = random(3) + 2; c =
       Guard tile(t, GUARD_DIRS[m], d); f = random(8) + 1; Guard(slot, c, f, orders 1, anchor t) (batch 2);
       slot + 1; placed + 1. After the m loop of a tile: if placed ≥ groups × per: stop the tile loop.
    b. For m = 0 .. gd − placed − 1 (none when gd ≤ placed; m < 8 here): d = random(11) + 5; c = Guard
       tile(0x63C, GUARD_DIRS[m], d); f = random(8) + 1; Guard(slot, c, f, orders 3, anchor NO_TILE); slot + 1.
15. For k = 0..47: Soldier start(record k) (batch 5).
16. busy = 0. fx_id = 0xFF (shooting.md). ripple_timer = 250 (soldier_update.md Update end). If team_on_duty ≠ 0: soldier record `selected` +0x1BD = 0x6E. Op 95 sector_tail.
17. If cur = 0x3B and byte 0xA9 of sectors row 0x3B = 0: Fixed items (batch 7).
18. If T[2] ≠ 0: S[0xA9] = 1.

## Batch 9: replace and reserve AP (ops 51 and 58 become library ops)

```c
void ja1_replace_soldier(Ja1World *w, const Ja1Host *h, int32_t slot);
void ja1_reserve_ap(Ja1World *w, const Ja1Host *h, int32_t slot);
```

From this batch on, ops 51 replace and 58 reserve_ap are **library ops** (port_api.md): wherever the library's
code makes op 51 or op 58 (batch 2, ai.md), it runs the procedure below itself, unless the op's bit is set in
`host_ops`; then it makes the host call as before. `slot` = the soldier record s (0..47). No new world field.

### Replace soldier (`ja1_replace_soldier`)

Puts the soldier back on its tile as a standing soldier. t = s16 +0x114.

1. u16 +0xF4 = t. +0x1B2 = byte 0 of tile t; +0x1B4 = byte 1 of tile t.
2. animIdx (+0x1F3) = 0.
3. u16 +0x1E8 = 4, unless the tile is water (+0x1B2 ≥ 0x6C and +0x1B4 is not 0x1C or 0x1F): then 0x33 if +0x1B2 <
   0x97, else 0x36 if +0x1B2 < 0x9A, else 0x39.
4. u32 +0x90 = u16 +0x1E8 × 0x50.
5. Frame delay (ja1_anim_delay, ai.md). It reads the OLD ground class +0x1B3 (step 7 writes the new one after it).
6. +0x138 = 1.
7. +0x1B2 = byte 0 of t, +0x1B3 = t0_class[+0x1B2]; +0x1B4 = byte 1 of t, +0x1B5 = t1_class[+0x1B4]; +0x1B6 =
   byte 2 of t, +0x1B7 = t2_class[+0x1B6].

No host call, no random draw. t is a tile of the map here (0..3135).

### Reserve AP (`ja1_reserve_ap`)

Writes the AP the soldier holds back for the item in its hand (item entry 0: type = byte +0x00) to +0xEF.
Blast kind of a type y (a byte): 0 if y ≤ 0x15; y − 0x12 if y ≤ 0x1B; 2 if y ≤ 0x1D; 1 if y = 0x5D; else 3.

1. k = blast kind of +0x00. If k is 3, 5, 7 or 9: +0xEF = 6. Stop.
2. If k = 0 and +0x00 is 0 or above 0xE: +0xEF = 0. Stop.
3. ap = low byte of host op 17 action_ap(slot, tile 0xC41, flag 1). +0xEF = ap.
4. If s8 +0x1A5 > s8 ap and +0x16A ≠ 0: v = (ap + 4) as a byte; +0xEF = v; if s8 v > s8 +0x1A5: +0xEF = +0x1A5.

Op 17 stays a host call (the AP cost of every item use is not yet in the library).

## Batch 10: radio result (op 33 becomes a library op)

```c
int32_t ja1_radio_result(Ja1World *w, const Ja1Host *h);   /* the new mission step: a byte 0..4 */
```

From this batch on, op 33 radio_result is a **library op** (same rule as batch 9). The result is the new mission
step; batch 8 step 6 and the ai.md callers keep its low byte. No new world field. S = sectors row cur (cur =
cur_sector). New host call inside: op 44 quest_check (port_api.md).

### Radio result (`ja1_radio_result`)

1. **Mission target.** mission_target = 0xC41. For t = 0 .. 0xC3F: g = byte 6 of tile t; if g < 0xFF and byte 0 of
   ground entry g = 0x72: mission_target = t; stop the loop.
2. **Story slot.** If s8 story[0x0B] = 1 and cur = s8 story[0x17] and story[0x16] = 0 and story[0x15] = 0: for k =
   0..4: if S[0x9F + k] = 0xAE: mission_target = u16 S[0x82 + 2k]; the result is 1.
3. If cur = 0x0B: the result is 3 if the low byte of op 44 quest_check(0x1C9) ≠ 0, else 0.
4. **Plants.** For k = 1, 2, 3 (plant 0 is NOT read): if the i32 at plants + 12 k + 4 = cur: the result is 2 if s8
   (byte plants + 12 k + 8) = 3, else 0.
5. If mission_done[cur] ≠ 0: the result is 0.
6. If s16 mission_target ≠ 0xC41: the result is 4.
7. For slot k = 0x28 .. 0x2F (the enemies): if +0x182 ≠ 0 and +0x183 ≠ 0 and +0x188 = 0 and (+0x0C = 0x40 or
   +0x00 = 0x40, the radio in item 1 or item 0): the result is 4.
8. The result is 0.

No random draw. Byte 6 of a tile is 0..199 or 0xFF in the game (ground_items has 200 entries).

## Batch 11: quest check (op 44 becomes a library op)

```c
int32_t ja1_quest_check(Ja1World *w, const Ja1Host *h, int32_t quest);   /* 1 = the condition holds, else 0 */
```

From this batch on, op 44 quest_check is a **library op** (same rule as batch 9). Callers keep the low byte of the
result. New world field (port_api.md, the last field of the struct): `safe_opened` (u8 scalar:
the safe in sector 0x0B is open).

### Quest check (`ja1_quest_check`)

1. If cur_sector ≠ 0x0B, or s16 quest ≠ 0x1C9 (the low 16 bits of quest, read as signed), or s8 story[0x0E] ≠ 1,
   or safe_opened ≠ 0, or story[0x1C] ≠ 0, or story[0x1E] ≠ 0: the result is 0.
2. If a merc carries item type 0x6E: the result is 0. Else, if a merc carries item type 0x7C: the result is 0.
   "A merc carries type y" = for slot k = 0..7 with +0x182 ≠ 0: byte 0 of one of its 12 item entries (+0x00,
   +0x0C, ..., +0x84) = y.
3. The result is 1.

No host call, no random draw, no write.

## Batch 12: action AP (op 17 becomes a library op)

```c
int32_t ja1_action_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile, int32_t flag);   /* a byte */
```

From this batch on, op 17 action_ap is a **library op** (same rule as batch 9; its slot argument names the
record). The result is a byte 0..255 (callers keep the low byte). tile = a signed 32-bit value, flag = its low byte.
New host calls inside (all exist in port_api.md): 45 shot_ap, 7 tile_dist, 14 reach_ap, 40 path_ap, 6 direction.
No new world field. s = the record of `slot`; me = s16 +0x114; kind = item_types[12 × (+0x00) + 2] (the main-hand
item's kind byte). "ShotAP(t)" = the low byte of host op 45 shot_ap(slot, t).

### Action AP (`ja1_action_ap`)

1. kind = 0x95 (thrown): the result is the low byte of Throw AP of s (ai.md, Throw AP).
2. kind = 0x96 (knife): c = 0.
   a. If tile < 0xC41 (signed): near = (host op 7 tile_dist(me, tile), as int32, ≤ 1) and (me and tile lie in the
      same column or the same row: (me % 56) = (tile % 56) or (me / 56) = (tile / 56), with % and / truncated toward
      zero on the signed values). If not near: if host op 14 reach_ap(slot, tile, 0xF) ≠ 0 (int32): c = the low
      byte of host op 40 path_ap(slot, s16 mission_spot) (mission_spot read after the reach_ap call), and tile =
      0xC41; else c = 0x63.
   b. The result is (c + ShotAP(tile)) as a byte.
3. kind ≠ 0x92: the result is 0x63.
4. kind = 0x92 (gun): sap = ShotAP(tile). a = u16 +0x1E8 + 2 × (+0x1F3) (the current animation).
   a. If a is not 0x1B, 0x1F or 0x23 (aiming): the result is (sap + weapons[6 × (+0x00) + 2]) as a byte.
   b. If flag = 0: the result is sap.
   c. If tile ≠ 0xC41 and host op 6 direction(x(me), y(me), x(tile), y(tile)) (as int32) = s8 +0x1A1: the result is
      sap. x(t) = (t % 56) × 16 + 5, y(t) = (t / 56) × 14 + 5 (truncated toward zero, port_api.md op 6).
   d. The result is (sap + 1) as a byte.

No random draw, no write (reach_ap may write mission_spot through the host).

## Batch 13: shot AP (op 45 becomes a library op)

```c
int32_t ja1_shot_ap(Ja1World *w, const Ja1Host *h, int32_t slot, int32_t tile);   /* a byte */
```

From this batch on, op 45 shot_ap is a **library op** (same rule as batch 9; its slot argument names the record).
The result is a byte (callers keep the low byte). tile = a signed 32-bit value. s = the record of `slot`, y = +0x00
(the main-hand item type). No new world field, no host call except div_zero (op 47).

### Shot AP (`ja1_shot_ap`)

1. If y < 1 or y > 0x10: the result is 0.
2. c = 1 if tile ≠ s16 +0xD0 (compared as int32), or if s16 +0xD0 = 0xC41; else c = 0.
3. m = the low byte of Max AP of s (ai.md, Max AP), read as unsigned.
4. k = the low byte of trunc((s8 +0x18B + s8 +0x18C) / 2) if weapons[6y + 0] = 4; else k = +0x191. Read k as
   unsigned.
5. d = trunc(((k / 2) + 50) × weapons[6y + 3] / 2) (k / 2 = integer half of the unsigned k).
6. If d = 0: host op 47 div_zero, then q = 0. Else q = trunc(200 × m / d).
7. b = (c + trunc((q + 1) / 2)) as a byte. If b > m (unsigned): b = m. If b = 0: b = 1. The result is b.
