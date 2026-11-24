# Save files

Two formats, both little-endian, no header magic, no checksum. A loader tells them apart by size (or by trying
the day layout first, as `./t savefile` does: only one layout accounts for every byte).

| File | Format | Written when |
|---|---|---|
| `game<n>.sav` | day save | calendar "Save Game" in the office |
| `OFFICE.SAV` | day save | entering the office (autosave) |
| `DAY.SAV` | day save | start of a day, after Sleep |
| `QICKSTRT.SAV` | day save | shipped "Quick Start" game; renamed to `game0.sav` on first use |
| `QUICK.SAV` | quick save | Control Panel "Quick Save" during a day |

## Day save

| Offset | Size | Contents |
|---|---|---|
| 0x0000 | 2 | day number (u16; 1 = Day 1, 0 before the first day) |
| 0x0002 | 26 | save name (NUL-padded) |
| 0x001C | 8 × 0x200 | soldier records 0–7 (the player's mercs; `struct soldier`) |
| 0x101C | 60 × 0x5A | merc profiles 0–59 (`struct profile`; the 4 native profiles 60–63 are not saved) |
| 0x2534 | 60 × 9 | per-sector state (`struct sectorState`) |
| 0x2750 | 60 × 0xB1 | sector records (`struct sector`) |
| 0x50CC | 0x356 | game state block (below) |
| 0x5422 | 0x14 | options block (below) |
| 0x5436 | 61 | sector processing order (below) |
| 0x5473 | 30 | notes table (below) |
| 0x5491 | variable | tail (below) |

## Quick save

The fixed part is the list below, in order (blocks 0–8 are the same records as in the day save, but with all 48
soldier slots and without the save name). Blocks marked "unnamed" are game variables whose meaning is not yet
known; a port must still store and restore them byte for byte. Soldier records in a quick save hold a pointer
turned into an offset at +0x98 (the game subtracts the address of its base table before writing).

| # | Offset | Size | Contents |
|---|---|---|---|
| 0 | 0x00000 | 2 | g_day |
| 1 | 0x00002 | 24576 | g_soldiers |
| 2 | 0x06002 | 5400 | g_profiles |
| 3 | 0x0751A | 540 | g_sectorTbl9 |
| 4 | 0x07736 | 10620 | g_sectors |
| 5 | 0x0A0B2 | 854 | g_mercDeathsTotal |
| 6 | 0x0A408 | 20 | g_options |
| 7 | 0x0A41C | 61 | g_sectorOrder |
| 8 | 0x0A459 | 30 | g_notesFound |
| 9 | 0x0A477 | 32 | unnamed |
| 10 | 0x0A497 | 2 | unnamed |
| 11 | 0x0A499 | 2 | unnamed |
| 12 | 0x0A49B | 1 | unnamed |
| 13 | 0x0A49C | 1 | unnamed |
| 14 | 0x0A49D | 4 | unnamed |
| 15 | 0x0A4A1 | 1 | unnamed |
| 16 | 0x0A4A2 | 4 | unnamed |
| 17 | 0x0A4A6 | 65 | unnamed |
| 18 | 0x0A4E7 | 30 | unnamed |
| 19 | 0x0A505 | 1 | unnamed |
| 20 | 0x0A506 | 1 | unnamed |
| 21 | 0x0A507 | 1 | unnamed |
| 22 | 0x0A508 | 1 | unnamed |
| 23 | 0x0A509 | 1 | unnamed |
| 24 | 0x0A50A | 1 | unnamed |
| 25 | 0x0A50B | 1 | unnamed |
| 26 | 0x0A50C | 1 | unnamed |
| 27 | 0x0A50D | 60 | unnamed |
| 28 | 0x0A549 | 6272 | unnamed |
| 29 | 0x0BDC9 | 4 | unnamed |
| 30 | 0x0BDCD | 4 | unnamed |
| 31 | 0x0BDD1 | 1 | unnamed |
| 32 | 0x0BDD2 | 4 | unnamed |
| 33 | 0x0BDD6 | 960 | unnamed |
| 34 | 0x0C196 | 60 | unnamed |
| 35 | 0x0C1D2 | 480 | unnamed |
| 36 | 0x0C3B2 | 2 | g_replayCount |
| 37 | 0x0C3B4 | 60 | g_prevBalance |
| 38 | 0x0C3F0 | 1 | g_timeCompression |
| 39 | 0x0C3F1 | 4 | g_viewX |
| 40 | 0x0C3F5 | 4 | g_viewY |
| 41 | 0x0C3F9 | 4 | unnamed |
| 42 | 0x0C3FD | 4 | unnamed |
| 43 | 0x0C401 | 1 | unnamed |
| 44 | 0x0C402 | 1 | unnamed |
| 45 | 0x0C403 | 1 | unnamed |
| 46 | 0x0C404 | 1 | unnamed |
| 47 | 0x0C405 | 7 | unnamed |
| 48 | 0x0C40C | 4 | unnamed |
| 49 | 0x0C410 | 4 | unnamed |
| 50 | 0x0C414 | 4 | unnamed |
| 51 | 0x0C418 | 1 | unnamed |
| 52 | 0x0C419 | 1 | unnamed |
| 53 | 0x0C41A | 1 | unnamed |
| 54 | 0x0C41B | 1 | unnamed |
| 55 | 0x0C41C | 2 | unnamed |
| 56 | 0x0C41E | 4 | unnamed |
| 57 | 0x0C422 | 1 | unnamed |
| 58 | 0x0C423 | 50 | unnamed |
| 59 | 0x0C455 | 320 | unnamed |
| 60 | 0x0C595 | 128 | unnamed |
| 61 | 0x0C615 | 48 | unnamed |
| 62 | 0x0C645 | 256 | unnamed |
| 63 | 0x0C745 | 640 | unnamed |
| 64 | 0x0C9C5 | 1 | cursor mode 0x284F1B (`cursor_mode`) |
| 65 | 0x0C9C6 | 1 | cursor picture 0x284F1D (`cursor_value`) |
| 66 | 0x0C9C7 | 1 | 0x92DD4: always 0x4F, only QuickRestore writes it |
| 67 | 0x0C9C8 | 2 | unnamed |
| 68 | 0x0C9CA | 2 | wanted theme 0x96360 (`music_wanted`) |
| 69 | 0x0C9CC | 8 | unnamed |
| 70 | 0x0C9D4 | 2 | unnamed |
| 71 | 0x0C9D6 | 1 | unnamed |
| 72 | 0x0C9D7 | 4 | unnamed |
| 73 | 0x0C9DB | 1 | unnamed |
| 74 | 0x0C9DC | 1 | unnamed |
| 75 | 0x0C9DD | 1 | unnamed |
| 76 | 0x0C9DE | 480 | unnamed |
| 77 | 0x0CBBE | 1 | unnamed |
| 78 | 0x0CBBF | 110 | unnamed |
| 79 | 0x0CC2D | 288 | unnamed |
| 80 | 0x0CD4D | 540 | unnamed |
| 81 | 0x0CF69 | 3840 | unnamed |
| 82 | 0x0DE69 | 1 | unnamed |
| 83 | 0x0DE6A | 64 | unnamed |
| 84 | 0x0DEAA | 1 | unnamed |
| 85 | 0x0DEAB | 1 | unnamed |
| 86 | 0x0DEAC | 1 | unnamed |
| 87 | 0x0DEAD | 1 | unnamed |
| 88 | 0x0DEAE | 4 | unnamed |
| 89 | 0x0DEB2 | 512 | unnamed |
| 90 | 0x0E0B2 | 4 | unnamed |
| 91 | 0x0E0B6 | 1 | unnamed |
| 92 | 0x0E0B7 | 1 | unnamed |
| 93 | 0x0E0B8 | 1 | unnamed |
| 94 | 0x0E0B9 | 1 | unnamed |
| 95 | 0x0E0BA | 2 | unnamed |
| 96 | 0x0E0BC | 1 | unnamed |
| 97 | 0x0E0BD | 1 | g_roofsOn |
| 98 | 0x0E0BE | 2 | unnamed |
| 99 | 0x0E0C0 | 1 | unnamed |
| 100 | 0x0E0C1 | 1 | unnamed |
| 101 | 0x0E0C2 | 4 | first view tile 0x229CA4, ViewTile of the view |
| 102 | 0x0E0C6 | 4 | unnamed |
| 103 | 0x0E0CA | 4 | unnamed |
| 104 | 0x0E0CE | 4 | unnamed |
| 105 | 0x0E0D2 | 4 | unnamed |
| 106 | 0x0E0D6 | 4 | unnamed |
| 107 | 0x0E0DA | 4 | g_viewCols |
| 108 | 0x0E0DE | 4 | g_viewRows |
| 109 | 0x0E0E2 | 4 | unnamed |
| 110 | 0x0E0E6 | 4 | unnamed |
| 111 | 0x0E0EA | 4 | unnamed |
| 112 | 0x0E0EE | 4 | unnamed |
| 113 | 0x0E0F2 | 1 | unnamed |
| 114 | 0x0E0F3 | 1 | unnamed |
| 115 | 0x0E0F4 | 1 | unnamed |
| 116 | 0x0E0F5 | 1 | unnamed |
| 117 | 0x0E0F6 | 1 | unnamed |
| 118 | 0x0E0F7 | 4 | unnamed |
| 119 | 0x0E0FB | 4 | unnamed |
| 120 | 0x0E0FF | 1 | unnamed |
| 121 | 0x0E100 | 16 | unnamed |
| 122 | 0x0E110 | 16 | unnamed |
| 123 | 0x0E120 | 1 | unnamed |
| 124 | 0x0E121 | 1 | unnamed |
| 125 | 0x0E122 | 2 | unnamed |
| 126 | 0x0E124 | 1 | unnamed |
| 127 | 0x0E125 | 2 | unnamed |
| 128 | 0x0E127 | 1 | unnamed |
| 129 | 0x0E128 | 240 | unnamed |
| 130 | 0x0E218 | 4 | unnamed |
| 131 | 0x0E21C | 1 | unnamed |
| 132 | 0x0E21D | 2 | unnamed |
| 133 | 0x0E21F | 1 | unnamed |
| 134 | 0x0E220 | 1 | unnamed |
| 135 | 0x0E221 | 4 | unnamed |
| 136 | 0x0E225 | 1 | unnamed |
| 137 | 0x0E226 | 4 | unnamed |
| 138 | 0x0E22A | 4 | unnamed |
| 139 | 0x0E22E | 4 | unnamed |
| 140 | 0x0E232 | 4 | unnamed |
| 141 | 0x0E236 | 4 | unnamed |
| 142 | 0x0E23A | 2400 | g_groundItems |
| 143 | 0x0EB9A | 25088 | g_tileMap |

Fixed part total: 85402 bytes.

### Quick Save and Quick Restore: the steps

**Quick Save** (Control Panel result 100, panel.md):

1. The question box "Save day in progress?" with the picture `q_mark.pcc` (panel.md question box). No: nothing more.
2. The message box "Saving..." (panel.md "Message box", tactical colours) with no wait: the wait time is 0 ms
   (one key or button press that is waiting then is used up) and step 4 of the box (put the rectangle back) is
   skipped, so the box stays on the screen while the file is written; the next draw of the map covers it. The
   front-end shows it the same way.
3. The current sector's map state goes to its sector file first (as when the sector is left: tiles and ground items).
4. Each of the 48 soldier records whose bytes +0x182 and +0x138 are both not 0 gets +0x98 = its 32-bit value at
   +0x90 minus 0x106C18 (records where one of them is 0 keep their +0x98). The original keeps a pointer into the
   animation script table (`anim_scripts`, at 0x106C18) at +0x90, so +0x98 = the script position as a byte offset;
   the +0x90 bytes in a file depend on where the program was loaded (QUICK_t10.SAV: 0x2F848A with +0x98 = 0x872).
   A port that keeps the offset at +0x90 (port_api.md) writes +0x98 = that offset and +0x90 = 0x106C18 + it.
5. `QUICK.SAV` is written: the 143 blocks of the table above, then the tail (sector files, item stash, replay log).

**Quick Restore** (Control Panel result 101, only shown when `QUICK.SAV` exists):

1. The question box "Quick Restore?" with `q_mark.pcc`. No: nothing more.
2. When a theme plays, the music stops. The sound effects stop. The merc faces count as not loaded.
3. The message box "Restoring..." (no wait, as "Saving..." in Quick Save step 2).
4. Every block of the file is read back into its variable. Each soldier record whose +0x182 and +0x138 are both not
   0 gets +0x90 = 0x106C18 + its +0x98 (a port that keeps the offset: +0x90 = +0x98). The sector files, the item stash and the replay log are read back.
5. As after loading a day save: the profile fix-up, the sector reset from `JAGGED.DAT` (world_load.md section 3).
6. The terrain tileset of `cur_sector` loads (its palette too); the faces of the 8 merc slots load again; the strips
   are drawn; the tactical screen is drawn again; the palette fades in.
7. The saved playing theme (block 67) is taken: the playing theme becomes -1, and when the saved one is > 0 it plays
   (music.md "Play theme t", flag 0). Sound FX and Music now have the file's values (see "Options block").

**Quick save keys.** The tactical screen has two keys for the same steps (the key handler reads them as extended
keys, ASCII 0, then the scan code):

- Alt+S (scan code 0x1F): Quick Save when `combat` = 0 or the hidden save-anywhere byte ≠ 0. Unlike the Control
  Panel, save byte 21285 (quick save in combat) is not read: in a fight the key does nothing. The save-anywhere byte
  is 0 at the start of the program and only a cheat key changes it (the port keeps it 0).
- Alt+R (scan code 0x13): Quick Restore (with argument 1, as the Control Panel) when the file `QUICK.SAV` exists;
  else nothing.

Front-end: the window has both keys (smoke step window_quick).

Evidence: TacticalKeyHandler 0x13490: scan compares 0x13689 (0x13 -> 0x138A5: FileExists? 0x4707C on
"quick.sav" 0x902FC, QuickRestore 0x2C898 with EAX = 1) and 0x13655 (0x1F -> 0x138C6: 0x92DCB = 0 or 0x92DC3 ≠ 0,
QuickSave 0x2D880); ShowMessage? 0x27B98 called at 0x2D8CC / 0x2C90B with CL = 1 and the stack arguments 0, 0, 0
(no restore, 0 ms); save-anywhere byte 0x92DC3 (initial 0, xor 1 at 0x139B2 under the cheat mode 0x955E9 = 4).

### The blocks in the port's world

A port keeps the blocks below in the named world fields (port_api.md names; `u.` = the soldier update scalars,
`clock.` = the day clock). Write: field to block; restore: block to field. Fields not named here keep their value at a
restore (the original keeps those variables as they are). Blocks not named here (9, 34, 35, 48, 57, 64-66,
68, 70, 71, 75, 78, 83, 92, 97, 99, 101, 105-113, 115, 117-119, 121, 122, 134, 137-141) have no world field: a port
writes 0 there and ignores them at a restore.

| Block | World field |
|---|---|
| 0 | `day` (16 bits) |
| 1 | `soldiers` (all 48 records) |
| 2, 3, 4, 6, 7, 8 | as in the day save (`profiles`, `sector_state`, `sectors`, options, `sector_order`, `notes`) |
| 5 | the game state block as in the day save, plus the fields listed below the table |
| 10 / 11 | `u.turn_timer` / `santino_timer` (the same variable as `u.turn_timer2`) |
| 12 / 13 / 14 / 15 / 16 | `show_ai` / `active_slot` / `anim_signal` / `u.in_flight` / `bomb_count` |
| 17 / 18 | `u.timed_fx` (65) / `gas_clouds` (30) |
| 19-26 | `u.merc_to_show`, `u.interrupt_turn`, `u.handed_over`, `u.speech_wait`, `u.turn_moves`, `u.interrupt_go`, `u.itq_len`, `u.interrupter` |
| 27 / 28 | `blast_list` (60) / `move_codes` (6272) |
| 29 | byte 3 = `u.fx_id` |
| 30 / 31 / 32 | `busy` / `u.turn_flag` / `u.turn_clock` |
| 33 | `clock.guard_eta` (960); bytes 920..959 are also `enemy_watch` bytes 0..39 (one variable in the original; `enemy_watch` is written only for slots 0x28..0x2F, so these bytes are always the `guard_eta` entries of sectors 57..59: write them from `guard_eta`. Zeros there make guards arrive in sectors 57..59 after a restore) |
| 36 | the replay log record count; the records (3 bytes each, strategic.md "The day replay" events in order) are the last 3 × count bytes of the tail. Restore: the day's replay log becomes these records |
| 37 | at offsets 0 `prev_balance`, 4 `start_trees`, 12 `start_workable`, 20 `start_tapped`, 24 `start_sectors`, 32 `u.dead_mercs`, 36 `u.dead_allies`, 40 `u.dead_npc`, 48 `u.enemy_deaths`, 52 `start_cap`, 56 `start_plants` |
| 38 / 39 / 40 | `u.time_compress` / `view_x` / `view_y` |
| 41 / 42 | the tile marker's tile / kind (cursor.md "Tile marker"; its life and blink clocks are not saved) |
| 43 / 44 / 45 / 46 / 47 | `selected` / `u.next_selected` / `u.kept_merc` / `opt_onscreen` / `guide_queue` (7) |
| 49 / 50 | `u.spotted` / `u.view_tile` |
| 51-56 | `u.quote_slot`, `u.quote_event`, `u.quote_delay`, `u.quote_focus`, `u.quote_tile`, `u.show_timer` |
| 58 / 59 / 60 / 61 | `itq` (50) / `remember_e` (320; its bytes 0..15 are also `knowledge` bytes 48..63) / `remember_m` / `knowledge` bytes 0..47 |
| 62 / 63 | `last_seen_m` / `last_seen_e` |
| 67 | the playing theme (16 bits, music.md) |
| 69 | `enemy_watch` bytes 40..47 |
| 72 / 73 / 74 | `small_panel` / `full_shown` / `panel_ends` (panel.md "The small panel in the full view"); block 75 = the strip flag (panel.md "The interface strips") |
| 76 / 77 | `clock.guard_rec` (480) / `walter_sector` |
| 79 / 80 / 81 | `fx_anims` (288) / `sector_replay` (540) / `paths` (3840) |
| 82, 84-88 | `u.sees_slot`, `u.merc_shown`, `u.turn_shown`, `u.screen_drawn`, `u.view_hold`, `path_preview` |
| 89 / 90 / 91 | `marks` (bytes 0..255 of the 512) / `u.marks_n` / `u.interrupted` |
| 93-96, 98 | `u.speaker`, `bonus_due`, `bonus`, `duty7_merc`, `u.seen_enemies` |
| 100 / 102 / 103 / 104 | `inv.face_left` / `u.shown_slot` / `view_w` / `view_h` |
| 114 / 116 / 120 | `u.round_lock` / `u.story_finder` / `blaster` |
| 123, 125-128 | `calls` bytes 7, 0..1, 2, 4..5, 6 (byte 3 is not saved) |
| 124 / 129 / 130 / 131 | `u.ambient_count` / `dmg_text` (240) / `u.fuse_beep_timer` / `u.fuse_beep` |
| 132 / 133 / 135 / 136 | `mission_target` / `mission_slot` / `u.redraw_pending` / `combat` |
| 142 / 143 | `ground_items` / `tiles` |

Block 5 holds these world fields beyond the day save's own list, at block offsets: 152 `tileset`, 153
`cur_sector`, 185 `u.merc_tire`, 301 `u.new_round`, 307 `clock.night_warned`, 340 `clock.hours`, 341
`clock.minute`, 342 `clock.hour`, 343 `clock.minutes` (32 bits), 347 `clock.bar` (32 bits), 351 `u.alone_warned`,
352 `u.weather_tire` (32 bits), 362 `idle_day`, 363 `u.squad_lost`, 364 `clock.hard_day`.

## Tail (both formats)

1. **Sector files**: for each sector, in sector order, whose sector record has `hasTmp` (+0xA9) set: 18,080 bytes =
   that sector's map state (3136 tiles × 5 bytes, then 200 ground items × 12 bytes). These are copies of the
   game's `sec<nn>.tmp` working files.
2. **Item stash**: 136 bytes, one count per item type, then 3 bytes (type, quantity, condition) per counted item,
   grouped by type.
3. **Quick save only**: the day's replay log, 3 bytes per record; the record count is the 16-bit value saved in the
   fixed part (block 36, g_replayCount).

## Game state block (0x356 bytes)

Offsets relative to the block. Integers are little-endian; "i32" = signed 32-bit.

| Offset | Type | Meaning |
|---|---|---|
| 0x000 | i32 | mercs killed over the whole game |
| 0x004 | u16 | day number (same as the day-save header) |
| 0x008 | i32 | mercs on the team |
| 0x00C | i32 | team members on duty |
| 0x010 | i32 | income per harvested tree (500) |
| 0x014 | i32 | workable trees (owned sectors connected to home) |
| 0x018 | i32 | trees owned |
| 0x01C | i32 | trees harvested per day |
| 0x020 | i32 | today's income = price × trees harvested |
| 0x024 | i32 | funds |
| 0x028 | i32 | projected funds after today |
| 0x02C | i32 | today's net (income − expenses) |
| 0x030 | i32 | sum of half salaries |
| 0x034 | i32 | sum of full salaries |
| 0x038 | i32 | salaries charged today (half for mercs in states 1 and 4) |
| 0x03C | i32 | natives available to hire |
| 0x040 | i32 | tappers hired |
| 0x044 | i32 | guards hired |
| 0x048 | i32 | natives hired (tappers + guards) |
| 0x04C | i32 | idle natives |
| 0x050 | i32 | natives killed over the whole game |
| 0x054 | i32 | daily wage per native (35) |
| 0x058 | i32 | tappers' wages |
| 0x05C | i32 | guards' wages |
| 0x060 | i32 | natives' wages |
| 0x064 | i32 | total expenses today |
| 0x06C | i32 | enemies killed over the whole game |
| 0x074 | i32 | sectors owned |
| 0x078 | i32 | sectors gained |
| 0x07C | i32 | sectors lost |
| 0x080 | i32 | last sector gained (index) |
| 0x084 | i32 | processing capacity (trees per day) |
| 0x088 | i32 | processing plants owned |
| 0x098 | u8 | tileset of the current sector map |
| 0x099 | u8 | current sector index (0xFF = none) |
| 0x09A | u8 | AIM "Success Points" |
| 0x09B | u8 | consecutive bad days |
| 0x09C | u8 | AIM "# Terminations" |
| 0x09D | u8 | AIM "# Hirings" |
| 0x09F | u8 | AIM "Mercenary Sorties" |
| 0x0A0 | u8 | AIM "# Resignations" |
| 0x0A1 | u8 | AIM "# Non-Payments" |
| 0x0CB | u8 | Micro-Purifier pending (next enemy sector set up gets it) |
| 0x0CC | u8 | difficulty (0–2) |
| 0x18A | 4 × 12 | processing plants: i32 capacity, i32 sector index, u8 state, u8 and u8 record counts for the plant's tile and item patch files, 1 pad byte |
| 0x1BA | 8 × 18 | per merc slot: nine i16 practice counters (max HP, agility, dexterity, wisdom, medical, explosives, mechanical, marksmanship, level) |
| 0x259 | u8 | quick save allowed in combat |
| 0x25A | u8 | quick-save mode chosen |

All other bytes: not yet identified; preserve.

## Options block (0x14 bytes)

Sixteen on/off switches from the Control Panel, one byte each, in this order: Sound FX, Music, Fast Scroll,
Alt Scroll, Shadows, Subtitles, Item Info, Show Paths, Pal Cycling, Fast Anims, Follow move, Step Sounds, Day
Replays, Fast Firing, Safety Move, Merc OKs. Four further bytes follow (not yet identified).

**Which loads take Sound FX and Music from the file.** Loading a day save (`OFFICE.SAV` and `DAY.SAV` in the day
loop, Restore Game, "Abort this day") reads the whole block, then puts the first two switches (Sound FX and Music)
back to the values they had before the load: these two follow the running program, not the file. At program start
both are on (world_load.md section 10, new game values). Quick Restore reads all 20 bytes and keeps them, Sound FX
and Music too: the sound effects are heard only when Sound FX is on, and the saved theme of step 7 plays only when
Music is on (music.md "Play theme t" does nothing while Music is off; the theme is still the wanted one).

## Sector processing order (61 bytes)

Sector indices in the order the strategic code visits sectors; reshuffled at random during play. Initial
contents: 0x3B, 0x31, 0x27 … (each column of the 10-wide grid from the bottom, right to left), ending with 0x00.

## Notes table (30 bytes)

Fifteen pairs (item type, flag) for the one-off "Note" items (types 0x60–0x6D, 0x59, 0x5A). Flag 1 = that note
has already been placed in the world, so it is not placed again.

## Evidence

Block lists read from the original's save functions (SaveGame and QuickSave fwrite_ sequences) by
`tools/savefile.py`; 19 real saves (9 day, 10 quick) parse and re-serialise byte-identically
(`validate/diff/save_diff.py`). Day saves compared block by block with live memory dumps (`./t savecmp`).
Field meanings traced in the binary (REVERSE.md, labels.csv); funds, team salaries, natives' wages and sectors
gained also checked against the End of Day screen in a live session.
Sound FX and Music at a load (2026-10-26, asm): LoadGame 0x4E964 saves g_options 0x1163B9 / 0x1163B8 on the stack
at entry and writes them back after the reads and ResetSectorsFromDat 0x3712C (0x4EB26 / 0x4EB2F); its callers are
PlayLoop 0x4FC33 / 0x4FD0E (OFFICE.SAV / DAY.SAV, EDX = 1) and RestoreGameDialog? 0x4E734. QuickRestore 0x2C898
freads the 0x14 bytes to 0x1163B8 at 0x2C9CB and never writes them back. SetMusic 0x5C5EC tests 0x1163B9 first.
