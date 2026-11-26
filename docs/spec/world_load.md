# World from the game files, and entering a sector

How a front-end builds the library's world (port_api.md) from the player's own files, and how the game enters a
tactical sector. Verified against the original: validate/diff/world_diff.py with three memory snapshots of a real
game (QICKSTRT save restored; the moment before the sector entry; the moment after it).

Files: `JA.EXE` (the GOG install), the CD image `JA1.gog` (paths inside it as in tactical_map.md: `DAT/JAGGED.DAT`,
`DAT/TERRAIN.DAT`, `DAT/XRA.DAT`, `MISC/...`), and a **day save** (savegame.md: `GAME0.SAV`..`GAME9.SAV`,
`DAY.SAV`, `OFFICE.SAV`, the GOG sample `QICKSTRT.SAV`). Numbers are little-endian.

## 1. Read-only tables

Each read-only world field is a copy of bytes from a file: `size` bytes from `offset`. When "bytes" is smaller than
the field, the rest of the field is 0. (Some JA.EXE copies run past the end of the real table; the library never
reads those bytes.)

| field | field size | file | offset | bytes |
|---|---|---|---|---|
| `item_types` | 1632 | JA.EXE | 900764 (0xDBE9C) | 1632 |
| `anim_table` | 2048 | JA.EXE | 893450 (0xDA20A) | 2048 |
| `anim_rows` | 2048 | JA.EXE | 893446 (0xDA206) | 2048 |
| `anim_free` | 256 | JA.EXE | 893241 (0xDA139) | 256 |
| `anim_sounds` | 512 | JA.EXE | 893310 (0xDA17E) | 512 |
| `anim_steps_tile` | 256 | JA.EXE | 893173 (0xDA0F5) | 256 |
| `anim_turn_while` | 256 | JA.EXE | 893990 (0xDA426) | 256 |
| `fresher` | 2304 | JA.EXE | 903691 (0xDCA0B) | 2304 |
| `dir_steps` | 514 | JA.EXE | 893020 (0xDA05C) | 514 |
| `listed` | 32 | JA.EXE | 892758 (0xD9F56) | 32 |
| `crates` | 16 | JA.EXE | 904548 (0xDCD64) | 16 |
| `fixed_items` | 88 | JA.EXE | 905276 (0xDD03C) | 88 |
| `greed` | 256 | JA.EXE | 902801 (0xDC691) | 64 |
| `weapons` | 1536 | JA.EXE | 900309 (0xDBCD5) | 1536 |
| `absorb` | 576 | JA.EXE | 892866 (0xD9FC2) | 576 |
| `silencer` | 17 | JA.EXE | 892836 (0xD9FA4) | 17 |
| `shot_sound` | 34 | JA.EXE | 900736 (0xDBE80) | 34 |
| `impact_noise` | 33 | JA.EXE | 892869 (0xD9FC5) | 33 |
| `shot_range` | 9 | JA.EXE | 906223 (0xDD3EF) | 9 |
| `posture` | 256 | JA.EXE | 893105 (0xDA0B1) | 256 |
| `window` | 2056 | JA.EXE | 903212 (0xDC82C) | 2056 |
| `attack_sound` | 10 | JA.EXE | 892682 (0xD9F0A) | 10 |
| `diff_mod` | 6 | JA.EXE | 904434 (0xDCCF2) | 6 |
| `equip_skill` | 6 | JA.EXE | 904428 (0xDCCEC) | 6 |
| `attach` | 328 | JA.EXE | 899918 (0xDBB4E) | 328 |
| `anim_scripts` | 5440 | JAGGED.DAT | 7984 (0x1F30) | 5440 |
| `t0_class` | 256 | JAGGED.DAT | 22128 (0x5670) | 157 |
| `t1_class` | 256 | JAGGED.DAT | 22285 (0x570D) | 256 |
| `t2_class` | 256 | JAGGED.DAT | 22541 (0x580D) | 155 |
| `splash` | 60 | JA.EXE | 892698 (0xD9F1A) | 60 |
| `view_rays` | 306 | JA.EXE | 894456 (0xDA5F8) | 306 |
| `obj_block` | 685 | JA.EXE | 902968 (0xDC738) | 685 |

`window` bytes 0x63B and 0x63C (object 199 at directions 3 and 4) are two variables of the original's user
interface; the port uses the JA.EXE copy.

Also from JA.EXE: **entry tiles**, 60 × i32 at 904564 (0xDCD74): entry tile of sector n (0..59) = i32 at
904564 + 4·n. (Used in section 4.)

## 2. Start values of the other fields

A field that no day save holds (section 3) starts with these values. "EXE n" = the field's bytes (its size in the
port_api.md struct) from JA.EXE at offset n; scalars are read with their struct type.

| field | start value |
|---|---|
| `calls` | EXE 892510 (0xD9E5E), 8 bytes |
| `combat` | EXE 892527 (0xD9E6F) |
| `mission_slot` | EXE 892492 (0xD9E4C) |
| `mission_target` | EXE 892494 (0xD9E4E) |
| `mission_spot` | EXE 892660 (0xD9EF4) |
| `turn_based` | EXE 892496 (0xD9E50) |
| `busy` | EXE 892636 (0xD9EDC) |
| `duty7_merc` | EXE 892542 (0xD9E7E) (0x40 = none) |
| `stat_changed` | 640 × 0 |
| `prev_balance` | 0 (set at each sector entry, section 4) |
| `show_ai` | EXE 892529 (0xD9E71) |
| `opt_onscreen` | EXE 892547 (0xD9E83) |
| `effect_tile` | EXE 892656 (0xD9EF0) |
| `bomb_count` | EXE 892640 (0xD9EE0) |
| `screen_drawn` | EXE 892528 (0xD9E70) |
| `interrupt_go` | EXE 892531 (0xD9E73) |
| `turn_moves` | EXE 892532 (0xD9E74) |
| `interrupted` | EXE 892535 (0xD9E77) |
| `quote_slot` | EXE 892560 (0xD9E90) |
| `quote_tile` | EXE 892562 (0xD9E92) |
| `follow_all` | EXE 894204 (0xDA4FC) |
| `stop_timer` | EXE 904812 (0xDCE6C) |
| `in_flight` | EXE 892520 (0xD9E68) |
| `merc_shown` | EXE 892524 (0xD9E6C) |
| `interrupt_turn` | EXE 892544 (0xD9E80) |
| `tick_step` | EXE 892932 (0xDA004) |
| `busy_timer` | EXE 892504 (0xD9E58) (i32) |
| `story_finder` | EXE 892551 (0xD9E87) |
| `spot_stop` | EXE 892680 (0xD9F08) |
| `merc_to_show` | EXE 892523 (0xD9E6B) |
| `focus_slot` | EXE 892662 (0xD9EF6) |
| `round_lock` | EXE 892548 (0xD9E84) |
| `round_lock2` | EXE 892549 (0xD9E85) |
| `handed_over` | EXE 892545 (0xD9E81) |
| `redraw_pending` | EXE 892488 (0xD9E48) |
| `turn_flag` | EXE 892525 (0xD9E6D) |
| `interrupter` | EXE 892534 (0xD9E76) |
| `show_timer` | EXE 896504 (0xDADF8) |
| `speaker` | EXE 892537 (0xD9E79) |
| `marked_slot` | EXE 904880 (0xDCEB0) |
| `knockdown` | EXE 892694 (0xD9F16) |
| `focus_last` | EXE 892663 (0xD9EF7) |
| `hit_scale` | EXE 892521 (0xD9E69) |
| `sees_slot` | EXE 892522 (0xD9E6A) |
| `preview_slot` | EXE 892668 (0xD9EFC) |
| `ripple_timer` | EXE 892828 (0xD9F9C), 4 bytes (250) |
| `view_w`, `view_h` | 224, 168 (the view window: 14 × 12 tiles of 16 × 14 pixels) |
| `guard_eta` | every s16 entry 9999 (no hired guard on the way; strategic.md The guard screen fills it) |
| `guard_rec` | 0 (480 bytes) |
| `tiles` | each tile 0x20, 0, 0, 0xFF, 0, 0x40, 0xFF, 0 (InitProgramWorld? 0x377B0); section 7 writes over it at the first sector |
| every other buffer and scalar not in section 3 | 0 (`knowledge`, `last_seen_m`, `last_seen_e`, `paths`, `remember_m`, `remember_e`, `ground_items`, `move_codes`, `held`, `blast_list`, `enemy_watch`, `text_buf`, `item_buf`, `active_slot`, `santino_timer`, `selected`, `path_preview`, `view_x`, `view_y`, `anim_signal`, `kept_merc`, `next_selected`, `quote_focus`, `quote_delay`,
`quote_event`, `walk_ap`, `spotted`, `key_shift`, `key_alt`, `key_ctrl`, `fast_timer`, `seen_enemies`, `fx_anims`, `speech_wait`, `turn_clock`, `dmg_text`, `dead_mercs`, `dead_allies`, `dead_npc`, `enemy_deaths`, `marks_shown`, `marks_n`, `marks`) |

## 3. Day save

A day save (savegame.md) is a 28-byte header, fixed blocks, then the sector files and the item stash. The game
skips the header. World fields at fixed save offsets (count = bytes copied from the save; the rest of the field
keeps its start value, which is 0 here):

| field | save offset | count |
|---|---|---|
| `soldiers` | 28 (0x1C) | 4096 (records 0..7, 512 bytes each) |
| `sector_state` | 9524 (0x2534) | 540 (sectors 0..59; entries 60..79 stay 0) |
| `sectors` | 10064 (0x2750) | 10620 (then see the reset below) |
| `day` | 20688 (0x50D0) | 4 |
| `team_on_duty` | 20696 (0x50D8) | 4 |
| `santino_word` | 20788 (0x5134) | 4 |
| `guard_skill` | 20828 (0x515C) | 1 |
| `cur_sector` | 20837 (0x5165) | 1 |
| `story` | 20844 (0x516C) | 43 (the world image of section 6 keeps the first 35) |
| `crate_state` | 20887 (0x5197) | 4 |
| `crate_shown` | 20891 (0x519B) | 4 |
| `guides_hired` | 20895 (0x519F) | 1 |
| `guide_wait` | 20896 (0x51A0) | 1 |
| `supplies` | 20970 (0x51EA) | 4 |
| `enemy_counter` | 20969 (0x51E9) | 1 |
| `item_told` | 21282 (0x5322) | 3 |
| `walter_said` | 20957 (0x51DD) | 12 |
| `walter_next` | 20975 (0x51EF) | 8 |
| `santino_spoke` | 20974 (0x51EE) | 1 |
| `purifier` | 20983 (0x51F7) | 1 |
| `difficulty` | 20984 (0x51F8) | 1 |
| `alarm` | 20986 (0x51FA) | 1 |
| `mission_step` | 20988 (0x51FC) | 1 |
| `opt_reload_msg` | 20826 (0x515A) | 1 |
| `ap_warned` | 21016 (0x5218) | 8 |
| `merc_tire` | 20869 (0x5185) | 1 |
| `sector_spoken` | 20897 (0x51A1) | 60 |
| `stage` | 21278 (0x531E) | 1 |
| `redraw_next` | 21279 (0x531F) | 1 |
| `weather_tire` | 21036 (0x522C) | 4 |
| `replay_m` | 20989 (0x51FD) | 1 |
| `last_killer` | 20990 (0x51FE) | 1 |
| `squad_lost` | 21047 (0x5237) | 1 |
| `first_hit` | 21281 (0x5321) | 1 |
| `alone_warned` | 21035 (0x522B) | 1 |
| `last_spotter` | 21044 (0x5234) | 1 |
| `safe_opened` | 21045 (0x5235) | 1 |
| `plants` | 21078 (0x5256) | 48 |
| `mission_done` | 21287 (0x5327) | 80 |
| `opt_step_text` | 21543 (0x5427) | 1 |
| `opt_half_delay` | 21547 (0x542B) | 1 |
| `opt_follow` | 21548 (0x542C) | 1 |
| `opt_sound` | 21549 (0x542D) | 1 |
| `opt_move_fx` | 21553 (0x5431) | 1 |
| `opt_confirm_move` | 21552 (0x5430) | 1 |
| `opt_path_marks` | 21545 (0x5429) | 1 |
| `opt_ambient` | 21546 (0x542A) | 1 |
| `enemies_warned` | 20987 (0x51FB) | 1 |
| `notes` | 21619 (0x5473) | 30 |
| `accounts` | 20684 (0x50CC) | 168 (the first 0xA8 bytes of the game state block) |
| `sector_order` | 21558 (0x5436) | 61 |
| `profiles` | 4124 (0x101C) | 5400 (records 0..59; records 60..63: below) |
| `stat_progress` | 21126 (0x5286) | 144 |
| `hard_day` | 21048 (0x5238) | 1 |
| `last_quote` | 21270 (0x5316) | 6 |
| `hinted` | 21280 (0x5320) | 1 |

In each of the 8 records, bytes +0x90..+0x93 (the animation script position) are set to 0 after the copy: the
save holds a memory address of the original there, which means nothing to the port; the sector entry sets it.

**Strategic fields.** `profiles` records 60..63 (the four natives; the save does not hold them) are bytes
5400..5759 of `EDT\NEWPROF.EDT` (speech.md "Initial merc profiles") in the directory of JA.EXE (SPEC GAP (not seen: the file is part of
every install, 5760 bytes in the GOG files; only a damaged install lacks it): when the file is missing they stay 0). `stash_counts` (136 bytes) = the first 136 bytes of the item stash in the tail (after
the sector files, below). `sector_replay` (540 bytes) = a copy of `sector_state` bytes 0..539 after the load.
`team_on_duty`, `cur_sector` and `day` are also inside `accounts` (+0x0C, +0x99, +0x04): the library reads the
world fields for them; a front-end that changes one keeps the other copy equal.

**The last quote** (`last_quote`, 6 bytes): the speaker (byte 0), the category (byte 1), the row (u16, bytes 2..3)
and the quote number (u16, bytes 4..5) of the last quote that the game played; the original's "say it again" box
reads it (panel.md "Say it again (key L)"). A quote of category 4, 5 or 10 does not change it. The
front-end sets it in these host ops (row 0 in all):
- op 302 guide_say (q, c): speaker 0x40, category c, quote q; then, when q < 0x28 and c = 6, quote q + 1 (the
  second half of the guide's quote).
- op 317 brenda_say (q): speaker 0x41, category 7, quote q; then, when q < 10, quote q + 1.
- op 38 santino_speech: speaker 0x42, category 9, quote 0.
- the A.I.M. screen's "P says quote q" (strategic.md "The A.I.M. screen"): speaker p, category 0, row 14 p, quote
  q; when the refusal reason 13, 7 or 6 named a profile m < 64, also `quote_member` = m (save byte 21276).
The merc quotes set it too (speaker = the merc's profile p; row 14 p for category 0 AIM, 33 p for 1 DAY, 15 p for
2 PST; k = a soldier, whose profile is the speaker):
- op 54 merc_message / op 184 merc_sees: (p, 1, 33 p, how), only when the merc speaks (soldier_update.md "Merc
  message": past the gates slot < 8, how ≥ 0, radio, the two health tests); the cry of how = −1 does not.
- op 252 merge_quote (p): (p, 1, 33 p, 6).
- op 267 runs_off (k, slot, p): (p, 2, 15 p, 12).
- op 276 pd_quote (p): (p, 2, 15 p, 0).
- op 289 cp_quote (slot, q, p, first): (p, 2, 15 p, q) (a first complaint says quote 7 before, overwritten); then
  `quote_member` = the profile of soldier `slot`.
- op 291 hate_quote (k, d): (p, 2, 15 p, 6); `quote_member` = d.
- op 295 raise_voice (k, g): no quote record; `quote_member` = g.
- op 296 raise_ask (k, reason, g): (p, 2, 15 p, reason).
- op 298 raise_answer (k, a): (p, 2, 15 p, 13 when a = 2, else 14).
- op 328 fire_quote (k, x, y, p): (p, 0, 14 p, 9).
- op 100 (the cry) does not change it.
Ops 67 (the listening device) and 103 (the queued quote, soldier_update.md "Queue a merc quote") reach the
merc message through op 54, so they set it there. Op 202 won_quote runs Merc message for each merc that passes
the gates of soldier_update.md "Team quote", in a random order: the last one stays. Evidence (2026-10-28, asm): PlayQuote 0x24F08 (AL speaker, DX row, BX
quote, stack category) at 0x29B1F, 0x4A646, 0x447E1, 0x453F6, 0x44D73 / 0x44DF2 (member 0x44E2B), 0x44CBE (member
0x44CE6), 0x4472D, 0x509F9; smoke step say_again_merc.

**`hinted`**: the game flag of strategic.md "Start the day" (the hint about the ASSIGNMENT screen shows once per game).

**Profile fixes after loading.** After the sector reset (below), the three profile fixes of section 11 apply to the
loaded `profiles` (profile 41 bytes 0x3A / 0x3C, profile 24 byte 0x39, profile 26 byte 0x39).

**The program option "allow save"** (the original's command-line word ALLOWSAVE; in the front-end the environment
variable `JA1_ALLOWSAVE` = 1): when it is on and save byte 21286 (the quick-save mode was chosen) is 0, the loaded
game gets byte 21286 = 1 and byte 21285 (quick save in combat) = 1. The front-end keeps both bytes in the base
(section 10). **The notice**: when the option is on, the original prints this text on its DOS text screen at
program start (main_ 0x1024D..0x102BE, after the title banner, before the graphics start), then waits for a key
(GetKey 0x560F4); the eight strings at 0x900C4, 0x90110, 0x9015C, 0x901A8, 0x901C4, 0x90218, 0x90264, 0x902A0:

```text


QUICK SAVES TO BE ALLOWED DURING COMBAT: Each game restored (including
quick saves) during this session will be "patched" to allow quick saves
during combat providing that the game was started with a version of Jagged
Alliance prior to v1.14.

Starting with v1.14, beginning a new game allows you to decide which quick-save
mode you prefer (immediately following the difficulty mode choice). Like
the difficulty level, once decided it cannot be changed.
Press any key to start Jagged Alliance...
```

The front-end has no text screen: it writes the same text to stderr at program start and does not wait.

**Sector reset after loading.** Then, for every sector i = 0..59, the game overwrites part of record i of `sectors`
(177 bytes per record) from JAGGED.DAT: record bytes 0..47 = JAGGED.DAT bytes 184 + 48·i .. (48 bytes), record
bytes 48..87 = JAGGED.DAT bytes 3064 + 40·i .. (40 bytes). (So a used lock or trigger tile in those parts comes back
on every load; this is the original's behaviour.)

**Sector files.** After the fixed blocks (offset 21649, 0x5491) come the saved sector files: one 18,080-byte file
for every sector i (in order 0..59) whose `sectors` record byte 169 (0xA9) is not 0, as loaded above. The front-end
keeps them: file i is the sector's tmp file for host op 92 open_tmp (sector_start.md batch 7). A sector without one
has no tmp file (op 92 returns −1).

## 4. Entering a sector

The front-end:

1. **Map.** Fills `tiles` from TERRAIN.DAT record number `cur_sector` (0-based: the record of sector `cur_sector` + 1, tactical_map.md: 3136 × 5 bytes, the first 5 bytes
   are tile 3135) as bytes 0..4 of each tile; byte 5 = 0x40, byte 6 = 0xFF, byte 7 = 0. The record's last byte is
   the tileset (drawing only).
2. Sets `prev_balance` = the i32 at `accounts` +0x24 (g_balance: the funds at the start of the tactical day), the
   day-start copies `start_trees` .. `start_cap` and `dead_mercs`, `dead_allies`, `dead_npc`, `enemy_deaths` = 0
   (strategic.md The day's verdict, Day-start copies) and `first_hit` = 0 (so each tactical day starts with no enemy
   hit: strategic.md "The day-end steps" step 2 counts the day as quiet unless a hit sets it to 2; evidence
   2026-10-28: LoadTacticalSector? 0x2AD41 writes 0 to 0x22A1F5 on every path, before any branch), then calls `ja1_enter_sector(world, host, entry_tile)` with the entry tile of `cur_sector` (section 1).
3. Draws the view once (tactical_map.md). Drawing a soldier writes its drawAnchorX / drawAnchorY (soldier +0x11A /
   +0x11C), which the AI reads later. Every full draw of the tactical screen sets `opt_reload_msg` = 1 (the
   original's flag "the screen was drawn", which Frame checks reads).

### `ja1_enter_sector` (library)

```c
void ja1_enter_sector(Ja1World *w, const Ja1Host *h, int32_t entry_tile);
```

cur = `cur_sector`; state = `sector_state` entry cur (9 bytes).

0. kept_merc = 0x40. last_killer = 0x40 (shooting.md batch 4). ambient_t1 = 200, ambient_t2 = 300
   (soldier_update.md Update end). turn_tick = 10 (soldier_update.md Loop head).
   The day clock starts (turns.md "The day clock", Start of the day): day_minutes = 0, day_bar = 0,
   weather_tire = 0, redraw_next = 0, night_warned = 0, day_hours = 0, clock_minute = 0, clock_hour = 7.
1. For k = 0..7, record k: +0x138 = 1 if +0x17E = 0, else 0.
2. selected = the first k in 0..7 whose record has +0x182 ≠ 0, +0x17E = 0 or 8, and +0x183 ≠ 0. None: selected = 8.
3. `idle_day` = 0 if selected < 8, else 1 (the day's score is capped at 0 with it; original byte 0x22A10A,
   LoadTacticalSector? 0x2B081 / 0x2B0B1, read by DayScore 0x61346 and DayEndVerdict 0x60A8C).
   If selected < 8 and state byte 2 ≠ 0: host op 89 replay_event(cur, cur, 0x58). Then (always) Time compression
   (0) (turns.md): time_compress = 0, tick_step = 10, host op 198 mouse_wait, host op 315 mouse_release. (So a
   new sector always starts at normal speed, also when the last day ended with the time compressed; evidence
   2026-10-24: LoadTacticalSector? 0x2B0C1 calls TimeCompress 0x11D40 with AL = 0.)
4. ja1_setup_sector(entry_tile, 1) (sector_start.md batch 8).
5. **View.** If state byte 2 ≠ 0 (read again now): t = s16 of record `selected` +0x114; px = (t mod 56) × 16,
   py = (t div 56) × 14. Only if `opt_onscreen` ≠ 0 and the point is outside the view (px < view_x, or
   px ≥ view_x + view_w, or py < view_y, or py ≥ view_y + view_h): view_x = px − m + 16 with m = 0x70 when
   `opt_onscreen` < 2, else 0xA0; view_y = py − (view_h / 2) + 14 (division truncated toward zero), then while
   view_y is not a multiple of 14 (C remainder ≠ 0): view_y = view_y − 1. Then ClampView 0x24830: view_x into
   0 .. 0x380 − view_w, view_y into 0 .. 0x310 − view_h.
   Else (state byte 2 = 0): view_x = 336, view_y = 308.

The host calls during the entry are those of batch 8 plus ops 89, 198 and 315. Random numbers come from the host.

## 5. The game's random numbers

The original's random(n) (the host's `random` callback) is Watcom's rand over a 32-bit seed: seed = seed ×
0x41C64E6D + 0x3039 (modulo 2³²); r = (seed >> 16) & 0x7FFF; with m = n as a signed 16-bit value, v = (r × m) /
0x7FFF and the result = v − (v / m) × m (both divisions truncated toward zero; m ≤ 0: result 0, and the seed still moves forward). A front-end keeps
one seed for the whole game. (A real sector entry from the original's seed draws the same 215 numbers in the same
order as the library with this host.)

## 6. World image (test interface)

For the diff test the front-end writes a world as one file: every buffer field in this order, raw, at its full
size, then every scalar in this order as a 4-byte little-endian signed value (unsigned struct types zero-extended,
signed ones sign-extended):

buffers: `soldiers` 24576, `tiles` 25088, `item_types` 1632, `anim_table` 2048, `sector_state` 720,
`mission_done` 80, `knowledge` 64, `fresher` 2304, `last_seen_m` 256, `last_seen_e` 640, `calls` 8, `paths` 3840,
`dir_steps` 514, `t0_class` 256, `t1_class` 256, `remember_m` 128, `remember_e` 320, `story` 35, `plants` 48,
`crates` 16, `weapons` 1536, `absorb` 576, `anim_rows` 2048, `anim_scripts` 5440, `anim_free` 256,
`anim_sounds` 512, `anim_steps_tile` 256, `anim_turn_while` 256, `supplies` 4, `equip_skill` 6, `attach` 328,
`ground_items` 2400, `notes` 30, `move_codes` 6272, `held` 12, `blast_list` 60, `listed` 32, `t2_class` 256,
`enemy_watch` 48, `text_buf` 128, `item_buf` 12, `crate_state` 4, `crate_shown` 4, `fixed_items` 88,
`sectors` 10620, `ap_warned` 8, `splash` 60, then the buffers of the later pages in the order the front-end
already writes them (the last one `dmg_text`), then `marks` 256 (select.md).

scalars: `cur_sector`, `alarm`, `combat`, `difficulty`, `mission_step`, `mission_slot`, `mission_target`,
`mission_spot`, `turn_based`, `busy`, `active_slot`, `santino_spoke`, `santino_timer`, `show_ai`, `santino_word`,
`last_spotter`, `day`, `selected`, `path_preview`, `opt_sound`, `opt_onscreen`, `view_x`, `view_y`, `view_w`,
`view_h`, `anim_signal`, `effect_tile`, `guard_skill`, `bomb_count`, `team_on_duty`, `purifier`, `safe_opened`,
`screen_drawn`, `interrupt_go`, `turn_moves`, `kept_merc`, `next_selected`, `quote_slot`, `quote_tile`,
`quote_focus`, `quote_delay`, `quote_event`, `opt_half_delay`, `opt_follow`, `opt_reload_msg`, `alone_warned`,
`follow_all`, `stop_timer`, `interrupted`, `walk_ap`, `spotted`, `opt_step_text`, then the later scalars in port_api.md struct order (the last ones: ..., `squad_lost`, `last_killer`, `fast_timer`).

Command: `ja1-port ISO EXE --world SAVE OUT` loads the world from the files (sections 1-3, no sector entry) and
writes it to OUT.

## 7. The front-end as host (sector entry)

From sector_start.md batches 9..13 on, the library itself runs every game rule of a sector entry. The host calls
that remain are file access and per-sector resets of the front-end's own state. A front-end that enters a sector
(section 4) answers them as follows. Any other op during the entry is an error: stop and name the op.

| op | name | answer |
|---|---|---|
| random | (callback) | section 5, with the front-end's seed |
| 86 | open_patch(kind, number, count) | open a file the original opens with OpenCDFile (the game-folder file first, the CD file only when it is absent; the official 1.13b patch puts its DAT\XRA.DAT in the game folder): kind 0..3 = `MISC\` + CRATE / ITEM / DEADPL / LIVEPL + number as 2 decimal digits (at least 2, zero-padded; "%02d") + `.BIN`; kind 4 = `DAT\XRA.DAT`. Result 0 if the file exists, else −1 (as int32, 0xFFFFFFFF). That file is now the open file |
| 87 | read(n) | the next n bytes of the open file (fewer at its end); result = the count; out = the bytes |
| 88 | close | close the open file; result 0 |
| 96 | seek(pos) | the open file goes to byte pos; result 0 |
| 92 | open_tmp(sector) | when the front-end keeps a tmp file for the sector (as an unsigned byte; section 3, or made by op 219 save_tmp, section 8): that file is now the open file (position 0; op 87 reads it, op 88 closes it), result 0. Else −1 (as int32). A sector whose sectors row byte 0xA9 ≠ 0 but that has no tmp file: the front-end refuses it (SPEC GAP (not seen: a sector with byte 0xA9 set always has its tmp file while the game runs, see "The tail" in section 10); the original game would go on without the file) |
| 93 | sector_resets | 0 (no random draw: Setup sector step 1 draws it for speech_wait) |
| 89 replay_event, 94 keep_selected, 95 sector_tail | — | 0 (nothing to reset in a viewer) |
| 47 | div_zero | stop with an error (the original game crashes here) |

The host must not change the world during these calls.

**Test command.** `ja1-port ISO EXE --enter SAVE SEED OUT`: sections 1-3 (load), section 4 (map, ja1_enter_sector
with this host, view; opt_reload_msg = 1 as after the step 3 draw, also when nothing is drawn), then write the world image (section 6) to OUT, then one more line of text to stdout:
`seed S draws D` (the end seed as a decimal u32, D = the count of random draws, the host's own op 93 draw included). SEED = the start seed (a
decimal u32).

**Viewer.** `ja1-port ISO EXE --play SAVE [SEED] [--png OUT]` enters the sector of the save (SEED default 1) and
shows it as the plain viewer does, inside the tactical screen of panel.md (or writes that 320 x 200 screen to OUT), with the soldiers drawn per tactical_map.md "Soldiers":
tile map = the world's `tiles`, soldier records = `soldiers`, script word of slot k = the u16 at byte offset
(u32 soldier +0x90) in `anim_scripts`, show all = the "show all enemies" byte `follow_all` (0 unless the debug key Alt+E set it, cursor.md "Debug keys (level 4)"), roofs = the roofs flag (tactical_map.md "Drawing order"), sight viewer = 0xFF while `sees_slot` = 0x40 (so step 4 never hides
a soldier), else `sees_slot` with the sight table of panel.md "Tactical messages", selected merc = `selected`, shade table 28, the tileset of the TERRAIN.DAT
record. The first view = the world's view_x / view_y (pixels; top-left tile c0 = view_x / 16, r0 = view_y / 14, both clamped so that the window stays on the map).

## 8. Play (front-end)

`ja1-port ISO EXE --play SAVE [SEED]` (the window, not `--png`) runs the sector after the viewer setup of
section 7:

- **Game time.** Every 10 ms of real time is one tick: `ja1_tick` (soldier_update.md "Timer tick"). A front-end
  that falls behind runs the missed ticks first, at most 10 per pass; the rest run in the next passes.
- **Loop head.** Every pass starts with `ja1_loop_head` (soldier_update.md "Loop head"; turn timers, combat start
  and end, the busy and fast timer starts), before the inputs of the pass. Its result counts like the results of
  `ja1_frame` and `ja1_update_end` below. (It replaces the front-end's own fast-timer restart.) Host op 208 strip:
  result 0, no world change, as every other op; the weapon status text of the loop head is not drawn yet.
- **Frames.** After the ticks of one pass, first **Santino's step**: if s8 `santino_spoke` = 1 (his quote was
  spoken this sector), `santino_spoke` = 2, host op 99 focus (0x28, 1), then `ja1_click` (0x28, 291, 0): Santino
  uses tile 291 by hand (the use animation reaches op 74 use_on_tile, combat.md, which springs the trigger there).
  Then `ja1_frame` once, then `ja1_update_end` (soldier_update.md), then
  `ja1_fuses`, then draw the map and the soldiers. (The original game alternates one update pass and one draw pass;
  the soldiers' own timers keep the speed. The front-end runs Fuses once per pass of its own.) Then `ja1_leave_tests` (soldier_update.md
  "Leave tests"). The results of `ja1_loop_head`, `ja1_frame`, `ja1_update_end` and `ja1_leave_tests` add up (8-bit);
  every step of the pass runs, also after a result ≠ 0, and the sum is tested only at the end of the pass. A sum ≠ 0
  ends the sector: `ja1_leave_path` runs once (soldier_update.md "Leaving the sector"; host ops 215..220 answer 0
  with no world change, except that op 219 save_tmp makes the **tmp file** of sector `cur_sector` (as an unsigned
  byte; it replaces an older one): for each tile t = 0..3135 the bytes 1, 2, 3, 4, 6 of tile t (5 bytes per tile,
  in this order), then `ground_items` bytes 0..0x95F; 18,080 bytes, the layout that op 92 open_tmp gives back
  (sector_start.md "Tmp file") and that the save files hold (section 3). The front-end keeps it with the save's
  sector files, so the next entry of that sector reads it (the sector setup sets the sectors row byte 0xA9); library op 217 end_of_day shows the **end-of-day screen**, below, and "end of day" in the
  window title), then the window shows "sector left" in its title and play stops
  (no more ticks, frames or clicks; Escape still closes the window).
- **Hover.** While the mouse is over the map and no mouse button is held, every pass runs one cursor.md "Hover"
  step at the mouse tile T, after the inputs of the pass and before its ticks (the original runs it from its input
  handler, TacticalKeyHandler 0x13490 → 0x1D9B8). Shift / Alt = the keys held now. The cursor state and the hover
  state belong to play (start values: cursor.md); the cursor's combat = the world's `combat` at the end of each
  pass; the tactical screen draws the cursor from that cursor state. When
  the mouse leaves the map: over_map = 0, no hover step. In a play script, `hover T` puts the mouse over tile T
  (no pass of its own; the next passes run the hover step), `hover -1` takes it off the map; the mouse starts off
  the map. JA1_SCREEN_PNG draws the script's cursor state (JA1_CURSOR=T still overrides the tile).
- **The front-end's animation timers** (soldier_update.md) run after each `ja1_tick`, and host op 205 tile_anim
  (kind 0) starts a water ripple in them as soldier_update.md "Update end" says (frame table: the ripple's, count
  0x12).
- **Click.** A mouse button press over the map at tile T runs cursor.md "Clicks" (left or right button) in that
  pass's input order, in place of the pass's hover step; Shift / Alt = the keys at that moment (they also go to
  the world's key_shift / key_alt). The aim and the confirm of cursor.md are **waits**: while one is open the pass
  runs only its inputs (no hover, ticks, frame, update end, fuses or leave tests; no game time passes), and the
  next click or Escape answers it. Other keys do nothing while a wait is open (the original reads them and drops
  them). The mouse off the tile of an open confirm (walk or hand; not the aim, whose mouse stays on its tile)
  answers it at once as "anything else" does, before any later input: off the map, or a press on another tile
  (the mouse came there first; the press then acts as a new click). In a play script, `click T` (left) and `rclick T` (right) first put the mouse
  over T as `hover T` does; when the mouse was not over T before, one hover step at T runs first in the same pass
  (the mouse came there one frame before the press). `key Escape` answers an open wait with Escape (else it is the Escape of
  "Quit" below). The status line shows the click's text ("Not available!", "Can't use that!", the merc's "doesn't
  have enough Action Points!").
- **Quit.** Escape on the tactical screen does not end the game: it stops the selected merc, `ja1_escape_stop`
  (TacticalKeyHandler 0x13FF3). Only when selected < 8 and (combat = 0 or screen_drawn ≠ 0), on record selected,
  in this order: shown_slot = 0x40; Forget the attack (1); +0x1F7 = 0x40; Clear the task (+0x95 / +0x96 too); u16
  +0x120 = u16 +0x114; host op 50 end_task (slot, 1); +0x197 = 0; tile point stop (1); host op 84 anim_step (slot)
  when At the waypoint; host op 72 stop_walk (slot, 0) when view_hold (the busy display, byte 0x92DCE) ≠ 0. Else
  nothing. Escape while a confirm or aim is open only answers it (section 8 "Click"). Alt-X asks "Exit to DOS?" in the question box (panel.md "Abort and restart questions")
  (the original's ExitToDosPrompt 0x38084, extended key 0x2D); yes closes the window.
- **Focus (host op 99 focus (slot, flag)).** The view goes to soldier slot (FocusOn 0x3F724; slide 0x3F578; jump
  0x3F490; bounds 0x24830). Record = soldier slot.
  1. If +0x182 = 0, +0x183 = 0, +0x138 = 0, or (s8 +0x9C ≤ 0 and follow_all = 0): if slot = focus_slot, focus_slot
     = 0x40. Nothing else.
  2. t = s16 +0x114; x = (t mod 56) × 16, y = (t div 56) × 14 (tile pixel point). New view: half = 0x70 if
     opt_onscreen < 2, else 0xA0; nx = x − (half − 16); ny = y − (view_h / 2 − 14) (the division truncates); while
     ny mod 14 ≠ 0 (truncating remainder): ny − 1. Then nx = 0 if nx < 0, else 0x380 − view_w if nx is above that;
     ny = 0 if ny < 0, else 0x310 − view_h if ny is above that.
  3. If slot = focus_slot and opt_onscreen ≠ 0 (slide): only when x ≤ view_x, x ≥ view_x + view_w, y ≤ view_y or
     y ≥ view_y + view_h: up to 51 steps, each one stops when (view_x, view_y) = (nx, ny), else view_x moves 16
     towards nx (when they differ) and view_y moves 14 towards ny (when they differ). The original first sets
     ripple_timer = 20; before each step it waits until ripple_timer ≤ 0 (10 off per 10 ms tick), then sets
     ripple_timer = 50, moves and redraws the screen: 50 ms per step ("The slide in time" below). When 16 / 14 pixel
     steps cannot reach (nx, ny) the view swings around it until the 51 steps are done.
     Else (jump): only when opt_onscreen ≠ 0 and (x < view_x, x ≥ view_x + view_w, y < view_y or y ≥ view_y +
     view_h): view_x = nx, view_y = ny.
  4. focus_slot = 0x40. If flag = 0: shown_slot = slot, show_timer = 5000.
  In words: the view moves only when the soldier is outside it (or on its left or top edge for the slide), and then
  puts the soldier about in the middle. Callers: a selected merc (select.md), each step of an enemy that the mercs
  see in its own turn (soldier_update.md Active frame step 4), a seen enemy that shoots or throws (shooting.md,
  soldier_update.md Throw aim), a new sighting (ai.md), the start of a turn (turns.md).

  **The slide in time.** The timer runs while the view slides, but no frame runs and the game takes no input:
  the slide takes 2 ticks for the first step (ripple_timer = 20 at the start, whatever it was) and 5 ticks for each
  later step; after it ripple_timer = 50. Front-end rule: the library moves the view at once; after
  the pass the window draws step i at 10 × (2 + 5 (i − 1)) ms, then runs the slide's ticks as plain timer ticks
  (no frames) and sets ripple_timer = 50; script mode runs the same ticks and logs `slide X0 Y0 X1 Y1 STEPS TICKS` (X1 Y1 = the target).

- **Choose a merc.** Keys 1..8 = merc k = 0..7: if record k has +0x182 ≠ 0, +0x138 ≠ 0, s8 +0x183 > 0 and
  +0x188 = 0: `ja1_select_merc` (k, 1, 0) (select.md).
- **End turn.** Key D, only while combat ≠ 0, screen_drawn ≠ 0 and turn_shown ≠ 0: `ja1_end_turn` (turns.md).
  (The original first puts an item on the mouse cursor back, op 85; the front-end has no cursor item.)
- **Time compression.** Key C and the "any key or click turns it off" rule: turns.md "Time compression" (the
  front-end's key handling). The pass that holds the input does nothing else with that key or click. The window
  shows the clock in its status line as `H:MM` (`clock_hour`, `clock_minute`, the minutes with two digits) and
  "C" while time_compress ≠ 0. With time compression on, a day of 12 hours takes 144 new turns of 20 ticks each
  (about 29 seconds of real time). The day ends when `ja1_day_clock` (op 159, turns.md) answers 1: the leave
  tests see New turn's result and the end-of-day screen follows.
- **Abort or restart the day.** Key A (only through the same key path as key C, turns.md "Time compression": while
  time_compress ≠ 0 the key only turns time compression off), or a click on the abort button of the top strip
  (inventory.md Buttons, button 5 at x 234, y 1; the tactical top strip is not drawn yet). The question box asks
  "ABORT this day?" (panel.md "Abort and restart questions"; keys Y / O / Enter = yes, N / Escape = no, or the
  buttons). No: nothing more. Yes: the question box asks "RESTART this day?". Then play stops at once: the rest of the pass
  does not run, and the leave path, the end-of-day screen and the day loop's morning do not run.
  - **Restart** (Y): the world becomes the copy kept when the day started (section 9 step 4; before the first day
    start of a run: the world as loaded at the start of the run, the original's DAY.SAV), then the sector entry of
    section 4 runs for `cur_sector` and play goes on (the same day again from its start).
  - **Abort** (N or Escape): the world becomes the copy kept after the office (section 9 step 2, at Sleep: the
    original's OFFICE.SAV, so `day` is the day before the sunrise and `cur_sector` is the value of the office), and
    the day loop goes on at step 2: the office again (no verdict, no morning), then Sleep, the sunrise (`day` + 1
    again), a new strategic screen opening and the island map. Before the first office of a run there is no such
    copy: the abort then works as a restart (front-end choice; the original loads the OFFICE.SAV file of the last
    night on the disk). Evidence: PlayLoop 0x4FB94 (g_dayAborted set: LoadGame OFFICE.SAV, then OfficeScreen).
  The random numbers go on from where they are in both cases.
- **Host ops of the day clock** (turns.md "The day clock", "Time compression"): 309 day_bar: the front-end keeps
  the bar length for its status line; 310 enemy_pick and 311 enemy_move are library ops (strategic.md Enemy moves;
  with their op 329 enemy_walk_in) and 312 guard_enter are library ops too (strategic.md Walk-ins); 313 mode_banner, 316 refresh: the
  window draws anyway; 314 guards_msg: the status line shows the text (count 1: "1 Guard arrives in sector N");
  315 mouse_release: nothing to wait for.
- **Stat practice.** Host op 105 stat_practice (slot, stat, n, flag): the front-end calls `ja1_stat_practice` with
  the same arguments on the world of the running call (combat.md Stat practice), so the mercs' practice counters grow
  in play as in the original (the morning turns them into stat gains). Host op 281 practice (team slot, stat,
  amount, flag) of the morning duties (strategic.md Morning duties) is the same work: the front-end calls
  `ja1_stat_practice` with those four arguments (the amount as a signed 16-bit number), so training, doctoring and
  repairs give practice.
- **Sector battles.** Ops 218 and 201 are library ops (strategic.md Sector battles). 332 battle_ui, 333 battle_wait
  (result 0: no skip, no wait) and 334 battle_box: nothing yet. 335 battle_msg (t, n): t = 0 the text goes to the
  status line, else a message box with a 1500 ms limit (panel.md "Tactical messages"; a play script writes
  `msgbox TEXT`).
- **Inventory screen.** Key I, with selected < 8 and s = record `selected`: only when s +0x138 ≠ 0, (combat = 0 or
  screen_drawn ≠ 0), s8 +0x183 ≥ 15 and s +0x188 ≠ 1 (inventory.md). The game time stops while the screen is open
  (no ticks, no passes; the time that passes meanwhile does not count). Steps:
  1. **Draw** (the front-end's "body"): the inventory screen drawing (inventory.md "Drawing the screen", the
     ja1f_draw_inventory code) of S = s with opt = 1 (opt_onscreen is 1 while the screen is open and gets its old
     value back after it), small panel 0, time meter width = `day_bar` (the length of the last op 309 day_bar), the
     enemies byte = byte 1 of the current sector's sector_state record (9 bytes per sector), blink = the
     front-end's own blink byte (start 0, kept between drawings), sight answers: for k = 0..7 in this order, sight[k] =
     `ja1_sight_line` (port_api.md; ai.md Sight line) with the arguments of inventory.md "Drawing the screen" step
     12 (S, S's x + 2, y + 4, O's tile, O's x + 2, y + 4, flag 1, range 14, seen 0x40; O = merc k) when step 12
     asks for it (O not "out" by the other tests and k ≠ S's record index), else 0 (no call: a sight line can draw
     random numbers through its line of fire), the panel buffer = a fresh 320 x 200 buffer,
     table 0 and the faces of the tactical screen. `item_rects` = the table the drawing gives back; the picture
     it leaves on the screen is the **inventory picture**.
  2. `ja1_inventory_screen` (selected, 0). Host calls meanwhile (each new op number still goes once to stderr):
     - op 221 inv_poll: show the inventory picture with the **cursor item** on it (when `held` byte 0 ≠ 0: item
       pointer shape of item type `held[0]`, panel.md "The mouse pointer": the item picture (item type byte 4,
       colour 0) and the hand), take the window's input, then write
       mouse_x, mouse_y (tactical screen pixels), mouse_buttons (bit 0 left, bit 1 right; a press and release
       between two polls is seen as down at the first poll and up at the next one), key_flags (0x1 Shift, 0x4
       Ctrl). A key goes to a queue: letters as upper-case ASCII, Escape 0x1B. Result 1 when the window closes
       (then play ends as after Escape), else 0.
     - op 222 key_pressed: 1 when the queue holds a key; op 223 get_key: the next key of the queue (0 = none).
     - op 232 inv_body (slot): step 1 again for that slot (the merc shown can change); result = the armour value.
     - op 238 draw_slot (S, k): item place k of S (inventory.md "Drawing the screen", with the current item places
       table) straight onto the screen. Op 239 vestmap (S): strategic.md "The vest map" (panel buffer, then the
       rectangle to the screen). Op 244 inv_moved (S, d, c): if S's slot < 8 and (opt < 3 or the small panel flag ≠
       0): strip box k = 0..6 into the panel buffer, darkened as in the drawing when d ≠ 0; then, when c ≠ 0, panel
       rows 182..199 to the screen. The panel buffer is the one the last step 1 drew (these ops do not redraw it).
     - op 241 rest (slot, amount, flag): `ja1_rest` (slot, amount, flag). Op 242 inv_select (slot, show, quote):
       the result of `ja1_select_soldier` (slot, show, quote).
     - ops 247 mine_spotted and 248 disarm are library ops (host_ops 0): the library answers them itself
       (combat.md "Mine spotted" and "Disarming an explosive", with the disarm prompt). Op 249 pick_up (slot, tile, 0): `ja1_take_ground_item`
       (tile, 1) (the ground item goes to `held`; inventory.md Pick up; the story-item quotes are left out).
     - op 250 inv_put_item (tile, d0, d1, d2, b6, exact, note, near): `ja1_put_item` (tile, the 12 bytes of
       d0..d2 little-endian, b6, exact, note, near).
     - every other op: result 0 and no world change (no sound, text, tooltips or prompts; merge_prompt 0 = no).
  3. After the screen: +0x1BB and +0x130 of the 8 mercs = 0; `ja1_reserve_ap` (S slot) for the merc shown last;
     opt_onscreen gets its old value. An item still on the cursor (the right click under the panel) stays in
     `held` (the front-end does not drop it).

  World data of the screen (the front-end keeps them between screens): `inv_cursor` = the same 12 bytes as `held`,
  `inv_swap` 12 bytes, `inv_picked` 8 bytes, `inv_give` 2 bytes, `item_rects` 56 bytes (start: the default table,
  inventory.md), `slot_rects` = the 8 merc slot corners of inventory.md "Merc slots", vest_owner start 0x40, the
  other inventory scalars start 0. None of them is in the world image (section 6).
- **End-of-day screen** (soldier_update.md "End of day"; the library's End of day makes these host calls). The
  screen is **open** from host op 254 until the next host op 215 fade; while it is open the front-end keeps a
  320 x 200 **picture** P, a **screen** S (320 x 200, what the window shows), a **work palette** W and a **shown
  palette** D (256 colours each, 6-bit VGA values 0..63; the window scales a value v to v × 255 / 63). The window
  shows S with D, scaled as the play window. Ops:
  - 254 eod_begin (day): P = MAIN.DAT member `SUNSET.PCX` (PCX, 320 x 200, decoded as the panel's PCX members).
    W = the PCX palette: the file's last 769 bytes are 0x0C and then 256 × (R, G, B); each value is shifted right
    by 2. D = W. S = P. (If the member is missing: P all 0 and W all 0; SPEC GAP (not seen: MAIN.DAT of the GOG files holds SUNSET.PCX, 22800 bytes; only a damaged install lacks it).)
  - 206 pal_cycle (first, last), while the screen is open: the entries first..last of W rotate by one toward
    the lower index: W[first] .. W[last − 1] = the old W[first + 1] .. W[last], W[last] = the old W[first]. (Not
    open: result 0, nothing, as before.)
  - 207 pal_set (first, last), while open: D[first..last] = W[first..last]; then S is shown with D as one frame.
    The original waits for the screen's vertical retrace here (70 per second): the window waits until 1/70 s
    after the previous pal_set frame. (Not open: nothing.)
  - 255 eod_wait (): a **clock wait** of 2 ms ("Game speed", The game clock): the first op 255 of the screen does not
    wait; each later one waits until the game clock reads at least 2 ms more than at the end of the previous op 255
    (in practice: until the next clock step, ~55 ms). This sets the sunset pace (119 frames, ~6.5 s); op 207 adds
    nothing more. A play script does not wait; its sound time (sound.md "Output") counts 54925 µs per frame for
    ops 255 and 207 together (40639 µs for op 255, 14286 µs for op 207).
  - 256 eod_text (day): draw the text "End of Day " followed by `day` in decimal (signed, a minus sign when
    negative) on S with panel.md "Text" (centred, x 159, y 64, fg 0xFF, shadow 1, font 2 = JA_BIG.FNT, table 0
    of the tactical screen). P stays as it was.
  - 257 eod_poll (): S is shown with D; result 1 when a key press or a mouse button press came since the last
    poll (window), else 0.
  - 260 show_screen (): S = P (the texts go away); S is shown with D.
  - 37 delay (ms), while open: S is shown with D and the window waits ms milliseconds (taking no input).
  - 57 merc_text (k, n, ms), while open, n = 3 or 4: draw the text "<nickname of merc k> is shot retreating!"
    (n = 3) or "<nickname> dies retreating!" (n = 4) (nickname = the bytes at record k +0x16E up to the first 0,
    at most 9) on S as plain text (DrawMercText 0x27B38 → DrawWrappedText? 0x3625C: fg 0xFF, shadow 0, centre x 161,
    y 63, wrap 190, font 2, line height 11), no box; show S with D and wait ms milliseconds, with no key read. The
    text stays on S until the next 260.
  - 215 fade (while open): show S with D one last time; the screen closes (the window shows the map again).
  - 60 sfx, 258 eod_sound_stop, 259 overhead_sounds: result 0, nothing (no sound yet).
  - every op answers 0 and changes no world data, except the results of 257 above.
  Play script: a line `eod N` (N ≥ 0) before the pass that ends the sector makes poll number N (counted from 0
  for this screen) answer 1; every other poll answers 0; nothing waits. The environment variable JA1_EOD_PNG=PATH
  writes the last frame shown (S with D) as a PNG, and stderr gets one line when the screen closes:
  `eod frames F polls Q texts T` (F = frames shown by op 207, Q = polls, T = texts drawn by ops 256 and 57).
- **Host calls during play.** random = section 5, going on from the seed at the end of the sector entry. Ops 6, 7, 40, 41, 53 and the other library ops run in the
  library. Every other op: result 0 and no world change (a viewer has no sound, text or panel yet); the front-end
  writes each new op number once to stderr (`host op N`). The out buffer of such an op stays as it is.

**Play script (test command).** `ja1-port ISO EXE --script SAVE SEED SCRIPT OUT` runs play without a window and
without real time: sections 1-4 as `--enter`, then the passes of the play loop above, each pass with exactly one
tick. SCRIPT is a text file, one command per line (`#` starts a comment line):
- `wait N`: N passes with no input.
- `click T` / `rclick T`: a left / right click on tile T (no Shift / Alt; section 8 "Click"), then one pass.
  `key Escape` answers an open aim or confirm (else it stops the selected merc, "Quit"), then one pass.
- `key K`: key K (`D`, `I` or `1`..`8`), then one pass. For `I` the inventory screen takes its input from the
  `inv` lines that follow it.
- `inv X Y B [K]`: one inv_poll of the open inventory screen: the mouse at (X, Y) with buttons B (bit 0 left, bit 1
  right) and, when given, key K (one character, or `Esc`) in the queue. When the screen polls and no `inv` line is
  left, op 221 answers 1 (stop). `inv` lines with no screen open are ignored. The environment variable
  JA1_INV_PNG=PATH writes the last picture the screen showed (inventory picture + cursor item) as a PNG.
- `eod N`: the end-of-day screen (section 8) answers 1 at its poll number N; no pass of its own.
- `hover T`: the mouse over map tile T (section 8 "Hover"); `hover -1` = off the map. No pass of its own.
- `key PgUp` / `key PgDn` / `key Insert` / `key Tab`: the view-mode keys (panel.md "View modes"), run after the pass's
  other inputs; each writes `view mode M x X y Y filter F`. The end of a right-button look (cursor.md "Clicks" right
  step 4) does Set mode (0) the same way. The script's mouse point is (160, 100) until Set mode moves it.
- `key C`: the key C (turns.md "Time compression"), then one pass. While time_compress ≠ 0, each pass of a
  `wait` line makes as many ticks as the window does (one tick per pass in the script; tick_step = 1000 makes the
  turn clock run 100 times faster). `clock` prints a line `clock H:MM hours D minutes M` (clock_hour,
  clock_minute, day_hours, day_minutes) to stdout; no pass of its own.
- `abort R`: key A answered Y, then R (1 = Y = restart, 0 = N = abort) for "RESTART this day?" (section 8, Abort
  or restart the day). stdout gets `abort R day D sector S` (D = `day`, S = `cur_sector` after it). After a
  restart the play lines go on in the new sector entry; after an abort the day loop lines (`office`, `day`, `map`,
  `start`) go on from step 2 (the next `day` line runs the office and step 3: no verdict and no morning; its stdout
  line is `day morning - day D`).
The environment variable JA1_SCREEN_PNG=PATH writes, after the script, the tactical screen of the last pass as a
320 x 200 PNG with the sector's palette: the picture the window shows (panel.md "The front-end's tactical screen":
slots, map view at (48, 14), the interface strips), drawn from the world as the last pass left it. When the sector
was left, it is the last tactical frame before the leave.
Each pass happens as in the window (loop head, then the input, then the tick, frame, update end, fuses, leave tests). After the script: the
world image (section 6) goes to OUT and one line per soldier 0..47 with +0x182 ≠ 0 to stdout:
`slot K tile T hp H ap A` (T = s16 +0x114, H = s8 +0x183, A = s8 +0x1A5), then, while the mouse is over the map
(`hover`), `cursor tile T value V mode M ap A over O label "TEXT"` (the cursor state), then `passes P left L` (L = 1
when the sector was left).

**Test command.** `ja1-port ISO EXE --walk SAVE SEED SLOT TILE TICKS OUT`: sections 1-4 as `--enter`, then
`ja1_move_order` (SLOT, TILE, 0, 0, 0, 0, 0), then TICKS times: `ja1_tick`, `ja1_frame`. Host calls as in "Host
calls during play". Then the world image (section 6) goes to OUT, and one line to stdout: `walk R at T` (R = the
move order's result, T = s16 +0x114 of SLOT at the end).

## 9. The day loop (front-end)

The original runs one tactical day per sector entry. When play (section 8) stops because the sector was left (the
leave path ran, with the end-of-day screen), the next day comes in this order:

0. **The day's verdict** (not after an abort): first the A.I.M. link (`accounts` +0xA3) = 1 (strategic.md "The A.I.M.
   screen": a new day allows a new visit) and guards still on the way are lost (b5 = 0); then `ja1_day_verdict` (strategic.md The day's verdict). Host ops 330
   home_lost_msg: nothing (kept); 331 rating_word (score): the front-end keeps the score; while the island map of
   step 4 is open the window title gets " | rating W" after "Day N" (W = the word for the score, −3..3 = TERRIBLE,
   LOUSY, POOR, FAIR, GOOD, EXCELLENT, OUTSTANDING); the word is also drawn on the screen of the day replay (its last
   picture, or the start picture with Day Replays off) under the panel's RATING label: centred at (276, 161), font 0,
   colour 0xFF, shadow 1 (strategic.md "The day's verdict" step 5; not a message box, so it takes no click); 302 guide_say, 317 brenda_say, 308 stash_take as in
   step 3. A result other than 0 ends the game (ending.md "Game end": the end show of the code, the game-over
   screen, then the main menu). Steps 1..4 do not run. Result 0: `ja1_day_bonus`, then
   step 1.
1. **The morning**: `ja1_morning` (strategic.md The morning). Host ops as in "Host calls during play" (result 0, no
   world change), except 301 payroll_show: the payroll screen (strategic.md "The payroll screen") over the island
   map of the world (the window shows its screens and plays its sounds; a script prints `payroll steps N sounds A
   B`), and the ops of the **Not enough funds** dialog (strategic.md):
   - 221 inv_poll: the window shows the payroll screen at its end with its rows 1..142 drawn again from the payroll
     picture of the world as it is now (without a payroll screen: the assignment picture, drawn on a copy of the
     world because the drawing can change duties) with the message box (panel.md) of the
     newest op 270 text over it (message 1 right after message 0: both texts in one box, 0 first). The picture is
     made again when the texts change or op 268 came. The mouse position goes to `mouse_x` / `mouse_y`;
     `mouse_buttons` bit 0 = a left press since the last poll. Window closed: 1 (stop). Script: the next `pay` line
     (below); none left: 1 (stop: the morning stops after that step, as it did before the dialog had a front-end).
   - 270 pd_message (n): text 0 "NOT ENOUGH FUNDS TO PAY ALL MERCENARIES!", 1 "Choose someone to... er... be
     the... ah... charitable one...", 2 "The native guide is helping you for free!" (the strings of JA.EXE).
   - 271 pd_timer_start / 272 pd_elapsed: milliseconds since op 271 (script: always 0).
   - 275 pd_ask (k, family): the merc-face question box (panel.md "Question box", as "Fire NICK?" on the assignment
     screen) with "Don't pay NICK?" (family 0) or "Don't pay NICK's family?" (family 1); NICK = the nickname of
     soldier k (+0x16E, up to 9 bytes); OK, Enter, O, Y = 1; NO, N, Escape = 0. Script: the answer of the poll.
   - 268 island_map: the next poll makes the picture again. 233 / 234 (the "fire" tooltip), 274 (the blink), 269
     and 278..280 (the payroll numbers again, PAID on the faces at the end), 276 / 277 (the dismissed merc's voice
     and face balloon): as strategic.md "Not enough funds", What the player sees. The window draws them; the script
     keeps them for its pictures and lines. Ops 145, 224, 226, 246 and 273: nothing (nothing stays on the screen).
   Then `ja1_day_end` (strategic.md "The day-end screen", day-end steps 1 and 2; host op 302 guide_say as in step
   0; the script line `day end R`): a result 3 ends the game as the verdict's result 3 (the office does not open).
2. **The office** (strategic.md "The office"). After an abort (section 8) the day loop starts here. When the office
   ends (Sleep) the front-end keeps a copy of the whole world (the original's OFFICE.SAV, written after the office;
   for "Abort this day", section 8). Window: the office picture with its hot spots, labels and
   highlights; the step waits for Sleep (a click on the bed with a team; window closed = the step ends as after
   Sleep). Script: the `office` lines (below) are its polls; when none are left the step ends as after Sleep (so a
   script without `office` lines goes on at once). After Sleep the front-end writes `OFFICE.SAV` (section 10).
3. **The sunrise**: `ja1_sunrise` (strategic.md Sunrise). Host op 304 sunrise_show: the window title shows
   "Day N" (N = `day`), and the front-end runs the sunrise screen (strategic.md "The sunrise screen") with the
   random numbers of the library's sequence. Window: the frames and waits as given there, sounds on the screen
   sound. Script: nothing waits and no input skips, except that a line `sunrise F` (F ≥ 0) skips at poll F of
   that screen (polls counted from 0: the colour cycle polls once per frame f, then the helicopter once per frame
   i); the script output gets `sunrise sound NAME` per sound started and, when the screen ends,
   `sunrise frames F texts T birds B heli H skip K` (F = colour-cycle frames, T = text draws, B = bird calls, H =
   helicopter frames, K = 1 after a skip). JA1_SUNRISE_PNG=PATH writes the last frame shown before the fade out (S
   with the shown palette).
   Then **the strategic screen opening**: `ja1_strategic_open` (strategic.md Strategic screen opening). Host ops:
   306 map_show: nothing (the map opens in step 4); 305 merc_gone (profile p): the front-end keeps the text
   "<nickname of p> seems to have disappeared!" (nickname = profile bytes from +0x00 up to the first 0, at most
   15); the last such text shows on the status line when the map opens; 302 guide_say: nothing (no speech yet);
   307 quest_record (n): bytes 250 (n − 5) .. + 249 of `DAT/QUESTS.DAT` (the game-folder file first, else the CD: the 1.13b patch file; bytes past the file end, or
   no file: 0); 308 stash_take (type, take): the item store records (port_api.md; the first record of that type, take ≠ 0 takes it out); the guide_queue stays in the world (no guide yet);
   264 stash_put (type, count, condition): the sorted insert into the item store (strategic.md "Store an item"); every other op as in "Host calls during play" (result 0).
4. **The island map** opens (the map view of section 8 with its status line, mode 0 = team). Clicks work as in the map view. Key T is the
   Subtitles switch (MapKeyHandler? 0x426D0, mapkey_diff); P, G, Escape and M do nothing here (the day must start). Enter: the start checks
   `ja1_map_start` (button 0); a message shows in a message box (panel.md); answer 0: `ja1_day_start` (strategic.md
   Day start). Host op 303 day_ask: the question box (panel.md "Question box") with "Get to work?" (kind 1, with
   the travel animation of panel.md "The travel animation": wait_row, then go_row with number) or "Start day without anyone assigned to duty?" and Q_MARK.PCC (kind 0); OK,
   Enter, O, Y = answer 1; NO, N, Escape = answer 0. Result 0:
   the map stays open. Result 1: the front-end keeps a copy of the whole world (the original's DAY.SAV; for
   "Restart this day", section 8) and writes `DAY.SAV` from it (section 10), closes the map, runs the sector entry of section 4 for `cur_sector` (the
   random numbers go on from where they are), and play (section 8) goes on in that sector.

The world that play uses has the strategic fields of sections 2 and 3 (`accounts`, `profiles`, ...) from the
start, so the library's day-end code (leave path, end of day) sees the same data as the morning.

**Play script.** After the sector was left, these lines run the day loop (other lines are ignored until the sector
is entered again):
- `day`: steps 0..3; stdout gets `verdict R rating W` (step 0 ran; R = its result, W = the rating word or `-`),
  then, when R ≠ 0, `game over R` and nothing more (the later day lines do nothing); else `day morning M day D`
  (M = the result of `ja1_morning`, D = `day` after it), then one line `gone P` per op 305 merc_gone, in order
  (P = its profile).
- `pay X Y B A [MS]` (before `day`): one poll of the Not enough funds dialog: `mouse_x` X, `mouse_y` Y,
  `mouse_buttons` B; A (0 / 1) = the answer to an op 275 in that poll; MS (default 0) = what op 272 pd_elapsed
  answers in that poll (MS > 200: the blink). After the `day` lines, stdout gets `pay message N` per op 270,
  `pay ask K F A` per op 275 (team slot, family, answer), `pay balloon K X P` per op 277 (team slot, face x,
  profile), in order, and `pay end frames [K, ...]` when op 278 came (the slots of op 279). Pictures:
  JA1_PAY_BLINK_PNG (the first picture with the blink on, without the message box), JA1_PAY_BALLOON_PNG (the first
  balloon at full size), JA1_PAY_END_PNG (the picture of op 280).
- `office X Y B` (before `day`): one poll of the office (step 2): the mouse at (X, Y), B bit 0 = a left press, B
  bit 1 = a key V. After the `day` lines, stdout gets `office click K` per left press on hot spot K (0..7; 8 = none),
  `office msg` per message box (the Sleep message with no team; the next poll with a press or a key closes it and
  does nothing else) and `office sleep` when Sleep ended the step. JA1_OFFICE_PNG=path writes the last office
  picture (palette of the room) as a PNG.
- `slots X Y B [K,K,...]` (before `day`): while the slot list (strategic.md "Save Game and Restore Game") is open,
  every `slots` or `office` line is one poll of the list: the mouse at (X, Y), B = the buttons held (bit 0 left,
  bit 1 right), K = key bytes in decimal typed before the poll (an extended key = 0 and its scan code). stdout gets
  `slots open M` when a click on hot spot 0 or 1 opens the list (M = 1 Save Game, 0 Restore Game), then one of
  `slots none` (Restore Game with no saved game; the message box opens: `office msg`), `slots cancel`,
  `slots saved N` (GAMEn.SAV written) or `slots restored N day D` (GAMEn.SAV loaded; D = its `day`); with no save
  directory (no JA1_SAVE_DIR) the line ends with ` (no save directory)` and no file is written or read. A load error
  gives `slots error N` (the world stays). JA1_SLOTS_PNG=path writes the slot list after each of its polls.
- `map S MODE B`: `ja1_map_click` on sector S with map mode MODE and buttons B (process warning flag: one per map
  opening, 0 at the start); stdout gets `map S msg N` (N = its result).
- `start A [B]`: Enter on the map (B = 1: a click on the start button instead, strategic.md "Start the day");
  op 303 answers A. stdout gets `start M R` (M = the result of `ja1_map_start`, R = the
  result of `ja1_day_start`, or `-` when it did not run). When R = 1 the sector entry runs, as in step 4, and the
  script's play lines (`wait`, `click`, `key`, ...) go on in the new sector; "left" of the last stdout line is 0
  again.

## 10. Writing a day save (front-end)

The original writes a day save (savegame.md "Day save") twice per day loop: `OFFICE.SAV` when the day loop
reaches the office (before `day` goes up), and `DAY.SAV` when the day starts (after the strategic screen, before
the sector entry). Before it writes `OFFICE.SAV` it deletes `OFFICE.SAV`, `DAY.SAV` and `QUICK.SAV` if they exist.
The save is a copy of memory; the front-end makes the same file from the world.

**The base.** The front-end keeps bytes 0..21648 (the fixed part, 0x5491 bytes) of the day save that it loaded
(section 3). The file it writes starts as a copy of the base; then, in this order:
1. `accounts` at 20684 (168 bytes).
2. Every other field of the table in section 3, at its save offset, with the same byte count (the first `count`
   bytes of the field): `soldiers` (records 0..7), `sector_state` (540), `sectors` (10620, all bytes of every
   record: the sector reset of section 3 happens on loading only), `day` (4), ..., `profiles` (records 0..59),
   `stat_progress`, `hard_day`.
3. Bytes 0..1: the low 16 bits of `day` (u16). Bytes 2..27 (the save name) stay as in the base.
4. In each of the 8 soldier records, bytes +0x90..+0x93 are the base's bytes (the original writes a memory address
   there; a loader ignores them, section 3).
Bytes that no field covers keep the base's values (the port does not change them).

**The tail.** Then, for every sector i = 0..59 in order whose `sectors` record byte 169 (0xA9) is not 0: the
18,080 bytes of the sector's tmp file (the file that host op 92 open_tmp opens; section 3 "Sector files", op 219
save_tmp writes it). SPEC GAP (not seen: byte 169 is set only when a sector with mercs is set up; every way out
of the tactical screen (LeaveSectorCleanup? 0x30D6C ends with SaveSectorTmp) and every quick save write the
current sector's tmp file first; a program start and a new game delete all tmp files (InitTempFiles 0x38038, a
new game also clears every byte 169), and each load writes the file of every sector with byte 169 set
(CopySaveChunks? 0x4E840); so the file is missing only when it is deleted outside the game): a sector with byte 169 set and no tmp file: the original stops with an error
message; the front-end writes 18,080 zero bytes. Then the item stash: `stash_counts` (136 bytes), then the item
store's records (strategic.md "The item store") in the store's order, 3 bytes each (type, quantity, condition).

**When and where.** Day loop (section 9): step 2 (the office) deletes `OFFICE.SAV`, `DAY.SAV` and `QUICK.SAV`, then
writes `OFFICE.SAV` from the world as it is there (the same world as the office copy). Step 4 with result 1 writes
`DAY.SAV` from the world of the day start copy (before the sector entry). The directory is the one of the day save
that the front-end loaded (front-end rule; the original uses its working directory). The environment variable
`JA1_SAVE_DIR`, when set, gives the directory instead. In the play script (section 8, `--script`) the front-end
writes and deletes files only when `JA1_SAVE_DIR` is set (tests run in parallel on the same saves). A write error:
the front-end goes on (stderr gets one line `save: PATH: ERROR`).

**Loading it again.** The written file is a day save as in section 3: `ja1front` starts from it as from any other
save (`--world`, `--script`, the window).

**Test command.** `ja1-port ISO EXE --save-copy SAVE OUT`: loads SAVE as in sections 1..3 (no sector entry) and at
once writes the day save of this section to OUT. Its bytes equal SAVE except the sector reset and the profile
fixes of section 3 (record bytes 0..87 of each sector record come from JAGGED.DAT) and, when SAVE is longer than the tail above,
the extra bytes at the end.

## 11. A new game (front-end + library)

A new game builds its world without a day save: the front-end makes the **new-game save** (a day save as in
savegame.md, bytes below) and loads it as in sections 1..3; that file is also the base of section 10. Then the
library's `ja1_new_game` finishes the world, and the day loop (section 9) starts at step 2 (the office), as after an
abort (no verdict, no morning). Inputs: the difficulty d (0 = the easiest .. 2, from the Set Difficulty screen) and
the quick-save mode q (a byte, from the Set Quick-Save Mode screen).

```c
void ja1_new_game(Ja1World *w, const Ja1Host *h);   /* after the load of the new-game save */
```

**The new-game save** (offsets in the file; "EXE n" = JA.EXE bytes from file offset n; d is read as a signed byte
where the text says so). Every byte not named here is 0.

| bytes | value |
|---|---|
| 0..1 | `day` 0 (u16) |
| 2..27 | the save name: 26 × 0 |
| 28..4123 | soldiers 0..7: 0 |
| 4124..9523 | profiles 0..59 = `EDT\NEWPROF.EDT` bytes 0..5399 (the file beside JA.EXE, section 3), then the three fixes below |
| 9524..10063 | `sector_state`: for sector i = 0..59, entry byte 6 (fallow trees) = JAGGED.DAT byte 64 + 2i, byte 0 = JAGGED.DAT byte 65 + 2i; the other 7 bytes 0 |
| 10064..20683 | `sectors`: record i bytes 0..47 = JAGGED.DAT 184 + 48i (48 bytes), bytes 48..87 = JAGGED.DAT 3064 + 40i (40 bytes), bytes 88..127 = JAGGED.DAT 5464 + 40i (40 bytes), byte 128 = JAGGED.DAT 7864 + 2i, byte 129 = JAGGED.DAT 7865 + 2i; bytes 130..176 = 0 (`ja1_new_game` sets them) |
| 20684..21537 | the game state block (854 bytes; the first 168 are `accounts`): 0 except the table below |
| 21538..21557 | options: 1 1 0 0 1 0 1 1 1 0 1 1 1 0 1 1 0 0 0 0 (the program-start values) |
| 21558..21618 | `sector_order` = EXE 892564 (0xD9E94), 61 bytes |
| 21619..21648 | `notes` = EXE 892790 (0xD9F76), 30 bytes |
| 21649.. | no sector files; `stash_counts` (136 bytes), then the records of the item store (below) |

Profile fixes (profile p, byte b = file byte 4124 + 90p + b): profile 41: if byte 0x3A = 99, byte 0x3A = 25 and byte
0x3C = 99. Profile 24: if byte 0x39 = 2, byte 0x39 = 101. Profile 26: if byte 0x39 = 101, byte 0x39 = 2.

Game state block, byte k at file offset 20684 + k (i16 / i32 little-endian):

| k | value |
|---|---|
| 0x10 | i32 500 (the tree price) |
| 0x24 | i32 = the s16 at EXE 904392 (0xDCCC8) + 2·d (d signed): 25000, 20000, 15000 for d = 0, 1, 2 (the funds) |
| 0x54 | i32 35 (the native wage) |
| 0x68 | i32 = the s16 at EXE 904422 (0xDCCE6) + 2·d (d signed): 100, 200, 300 |
| 0x80 | i32 59 (the last sector gained) |
| 0x99 | 0xFF (`cur_sector`: none) |
| 0xA3, 0xB2, 0xDD, 0xDF, 0xE5, 0xE6, 0xFA, 0xFC, 0x110 | 1 |
| 0xB7, 0xCF, 0xD0, 0xD1, 0xD2, 0x24A | 0xFF |
| 0x111..0x11C | 12 × 99 |
| 0x11D | 60 |
| 0x123, 0x124, 0x125, 0x126, 0x127, 0x128, 0x129, 0x12A | 90, 70, 40, 20, 65, 60, 10, 60 |
| 0x12C | d (`difficulty`) |
| 0x18A..0x1B9 | `plants` (48 bytes) = EXE 904264 (0xDCC48): capacities 20, 35, 25, 30 in sectors 59, 26, 53, 30, states 0, 2, 2, 2 |
| 0x259 | q (the quick-save mode) |
| 0x25A | 1 |

**The item store** of the new game: one record (t, c, 100) for each t of this list of 37 types, in this order:
0x36 thirty times, 0x32, 0x32, 0x33, 0x2A, 0x49, 0x41, 0x41 (the original's list also holds three empty entries,
type 0, which store nothing). c = `item_types`[12t + 5] when `item_types`[12t + 6] = 1 (99 for 0x2A, 0x32, 0x33),
else 1. The records go in as strategic.md "The item store" says (so the file holds them sorted by type, and
`stash_counts`[t] = the number of records of type t).

**`ja1_new_game`.** d = `difficulty` (a byte); state(i) = `sector_state` entry i, rec(i) = `sectors` record i.
1. Set owned (59): state(59) byte 0 bit 0 = 1, then Workable for sector 59 and its neighbours, as at a sector
   gained (strategic.md Territory, without the gain count and the replay event).
2. For each sector i = 0..59: rec(i) bytes 0x96..0x9D (the enemy list) = 0xFF.
   - i = 59: state byte 4 (guards) = 8, state byte 1 (enemies) = 0, rec byte 0xAA = 1.
   - Else: state byte 1 = 4, 6 or 8 when d = 0, 1 or 2 (other d: unchanged, so 0). Then i = 28 or 37: + 2 when d
     as a signed byte < 2; i = 48, 49 or 58: − 2 (a byte, so 0 − 2 = 254). Then a value above 8 becomes 8. Then rec
     bytes 0x96 .. 0x96 + n − 1 = 1 (n = state byte 1) and rec byte 0xAA = 0.
   - Then rec bytes 0x9E..0xA9 and 0xAB..0xB0 = 0, and the ten u16 at rec bytes 0x82, 0x84, ..., 0x94 = 0x0C41.
3. Processing capacity (strategic.md Day accounts: CalcProcessCap), then Worker pool (`worker_pool`, strategic.md).

The original keeps the options, `sector_order`, `notes` and the save name of the game played before in the same run
of the program; the values above are those of a fresh start (FUN_000377b0 sets the options), except option 5
(Subtitles): at the program start the sound setup (InitSound 0x5CED4, after the program-start new game) sets it to
byte 80 of SOUND.CFG (the GOG file: 1). So the first game of a run starts with that value; the front-end reads
SOUND.CFG beside JA.EXE (World::sound_cfg_options). A new game after another game of the same run starts from the save bytes 2..27 and
21538..21648 of the game before (World::new_game_after); the rest is as above. Evidence (2026-10-28): Unicorn on
tac60 with noise in the options 0x1163B8 (20 bytes), `sector_order` 0x92DF0, `notes` 0x92ED2 and the save name
0x28EDFE: NewGameInit 0x37A0C leaves all four unchanged for d = 0, 1, 2 (build/probe/ngcarry/p.py); the only other
writers are FUN_000377b0 (main_ only), LoadGame / QuickRestore, the switch screens, ShuffleSectorOrder?,
NoteAlreadyPlaced and SaveSlotNameInput?; MainMenu, NewGameSetup, SetQuickSaveMode and GameMain write none of them.

Evidence (2026-10-25): NewGameInit 0x37A0C (asm; called by GameMain 0x4FFEC after NewGameSetup 0x4F548 and
SetQuickSaveMode 0x4F898 with EAX = difficulty [0x22A0CC], EDX = quick-save mode [0x22A1F9], then PlayLoop(0) starts
at OfficeScreen), LoadJaggedDat 0x371BC, LoadNewProfEdt 0x379B4 + 0x503F8 (profile fixes), 0x521B4 (40 types at
0x514E0, ItemInit?(2, 2), AddStashItem? 0x51D50), SetSectorOwned 0x3DDE4, CalcProcessCap 0x41574, WorkerPool 0x50BC4,
FUN_000377b0 (options). validate/diff/newgame_diff.py runs NewGameInit and SaveGame 0x4EBC8 in Unicorn.

**Test command.** `ja1-port ISO EXE --new-game D Q OUT` writes the day save of a new game (section 10, after
`ja1_new_game`) to OUT.

## 12. The main menu (front-end)

At the program start (after the title part, ending.md "The title part (program start)") the main menu opens. Before it, when `QUICK.SAV`
exists in the save folder:

**Continue last day in progress?**
1. The palette of AIMSCRN.PCX (MAIN.DAT) on a black screen (fade in), the pointer shows.
2. The question box (panel.md "Question box") "Continue last day in progress?" with the picture `Q_MARK.PCC`.
   Yes: go to step 4 with the answer 1.
3. No: the question box "Quick save about to be deleted!" with the picture `!MARK.PCC`. Yes: the files
   `OFFICE.SAV`, `DAY.SAV` and `QUICK.SAV` are removed when they exist (no other file), go to step 4 with the
   answer 0. No: back to step 2.
4. The pointer hides, the palette fades out.

With the answer 1 the main menu does not open first: the day goes on from `QUICK.SAV` (the play loop starts at the
tactical screen with savegame.md "Quick Restore" steps 2 to 7, with no question; then the day goes on as usual,
and its end leads to the office as always). When that play loop ends (Leave), the main menu opens. With the
answer 0 (or no `QUICK.SAV`) the main menu opens.

Front-end: the window from the menu (`--menu DIR`) asks both questions; after the yes, "Restart this day" takes
`DAY.SAV` of the save folder (else the restored world) and "Abort this day" `OFFICE.SAV` (else none). Smoke step
window_quick.

Evidence: GameMain (FileExists? on "quick.sav" 0x90C40, ContinueDayPrompt 0x4FF00, its answer to PlayLoop's
argument), ContinueDayPrompt 0x4FF00 (archive 0x92F44 member "aimscrn.pcx" 0x90EA0, palette 0xEDA08, memset of the
screen, question boxes 0x278A0 with 0x997EF / "q_mark.pcc" 0x90EAC and 0x99867 / "!mark.pcc" 0x90EB8,
DeleteDaySaves 0x37FF0), PlayLoop 0x4FB94 argument 1: no office and day start, TacticalLoop(1) ->
LoadTacticalSector? 0x2B1FC QuickRestore(0).

 **C** (Continue allowed) = 1 when a file `GAMEn.SAV` (n = 0..9) exists in the
save directory (section 10), else 0.

**Pictures.** `DAT/OFFICE.DAT` members MAINMENU.PCX (320 x 200) and STARTEXT.PCC (190 x 132); both have the same
palette, the palette of the menu. STARTEXT holds the three menu rows k = 0 (START NEW GAME), 1 (CONTINUE GAME),
2 (EXIT TO DOS), each 190 x 22: the plain row k at its (0, 22k), the lit row k at its (0, 66 + 22k).
**Draw row k plain / lit**: copy that 190 x 22 part to the screen at (60, 43 + 22k) (all pixels).

**Open the menu with row R** (R = 0 or 1): MAINMENU.PCX on the screen, rows 0, 1, 2 plain, row R lit; the mouse
moves to (240, 52 + 22R); the palette fades in (below). At the program start R = C.

**Fades of the menu screens.** Each fade is the fade of ending.md op 24 / 25 (levels n · from / 256 .. n · to / 256,
one per vertical retrace) on the colours of the screen at that moment:
- Open the menu: before the picture, the screen is cleared to 0 and the palette of MAINMENU.PCX fades out
  (24 steps, 256 → 0; so colour 0 of the menu palette goes to black); after the rows and the mouse move it fades in
  (24 steps, 0 → 256).
- Start New Game: after the intro the screen is cleared to 0 and the colours fade out (24 steps, 256 → 0). Set
  Difficulty and Set Quick-Save Mode each fade in (24 steps, 0 → 256) after their text and fade out (24 steps,
  256 → 0) after the choice.
- Continue Game and Exit to DOS: the menu fades out over **56** steps (256 → 0) before the day loop or the end.
Evidence (2026-10-27, asm; the fade 0x52908 takes EAX = colour slot 0xEDA08, EBX = n, ECX = from, one stack argument
= to): MainMenu 0x4FE55 (24, 256, 0, after the menu palette went into the slot and the 64000-byte screen clear) and
0x4FEF5 (24, 0, 256); GameMain 0x50327 (choice 0: clear, 24, 256, 0), 0x5037E (choice 1: 56, 256, 0), 0x503DF
(the end: 56, 256, 0); NewGameSetup 0x4F64A (24, 0, 256) / 0x4F85C (24, 256, 0); SetQuickSaveMode 0x4F95A
(24, 0, 256) / 0x4FB75 (24, 256, 0).

**The menu loop.** State: L = the lit row of the loop (0 at the program start; −1 = none), V = the row under the
mouse in this poll, V' = V of the poll before (V and V' start at 99 = none). Each poll (mouse position, buttons held,
at most one key):
1. V' = V, V = 99. For k = 0, 1, 2: the mouse is in row k when 60 ≤ x ≤ 250 and 43 + 22k ≤ y ≤ 65 + 22k; row 1
   counts only when C = 1. At the first such k: when k ≠ L, draw row L plain (when L ≠ −1, with the menu sound,
   sound.md "Menu sound") and row k lit, L = k. V = k. When the left button is held: wait until it is up; the choice is k.
   Stop at this k.
2. When V = 99 and V' < 99: draw row L plain, L = −1.
3. A key: an extended key (0, then a scan code s): s = 0x48 (Up): draw row L plain (L ≥ 0); L = L − 1, a value
   below 0 becomes 2; when L = 1 and C = 0, L = 0. s = 0x50 (Down): draw row L plain (L ≥ 0); L = L + 1, a value
   above 2 becomes 0; when L = 1 and C = 0, L = 2. After Up or Down: draw row L lit (with the menu sound) and move
   the mouse to (240, 52 + 22L). The key 13 (Enter) when V < 99: the choice is L. Other keys do nothing.
4. The choice:
   - 0 (**Start New Game**): the intro (ending.md "The new-game intro"), the
     screen goes black, then **Set Difficulty** and **Set Quick-Save Mode** (below), then the new game (section 11)
     with the difficulty d and the quick-save mode q, and the day loop from the office (section 9 step 2). When the
     game ends with Leave (the office's hot spot 7, result 7): C = 1, open the menu with row 0.
   - 1 (**Continue Game**): the day loop from the office with the world in memory (the last game of this run; at
     the program start: the **program-start world** = the world of a new game (section 11) with d = 0 and q = 0,
     then the A.I.M. link (`accounts` +0xA3) = 0, so the office shows no A.I.M. hot spots until a game is
     restored). After Leave: C = 1, open the menu with row 1.
   - 2 (**Exit to DOS**): the program ends ("Thanks for playing" on the text screen).
   So after the first game of a run, Continue is always allowed.

**Set Difficulty.** The screen is black (all 0); the palette is the palette of ROOM.PCX (`DAT/OFFICE.DAT`).
Text (panel.md "Drawing primitives") "Set Difficulty Level:" at x 159, y 60, centred, fg 0x28, shadow 1, font 2,
mode 300; the rows k = 0, 1, 2 "EASIER", "NORMAL", "HARDER" at x 159, y_k = 80, 100, 120, centred, fg 0x20 (plain),
shadow 1, font 2, mode 300. **Lit row k**: the same text with fg 0xFF (drawn over the plain text). Fade in. S = −1
(the lit row), K = the last key byte (it keeps its value from poll to poll; at the start not 13). Each poll:
1. A key: K = its byte; an extended key (0, then s): K = s. s = 0x48 (Up): S > 0: S − 1; S = 0: 2; S < 0: 0.
   s = 0x50 (Down): S = −1: 2; S < 2: S + 1; else 0. After Up or Down the mouse moves to (180, y_S + 2); step 2 of this poll uses that position.
2. S' = S. S = the first row k with 125 ≤ x ≤ 195 and y_k ≤ y ≤ y_k + 12, else −1 (so the mouse decides: a key
   lights a row only through the mouse it moved).
3. When S ≠ S': row S' plain (S' ≥ 0), row S lit (S ≥ 0), the menu sound.
4. When the left button is held or K = 13: S ≥ 0: the difficulty d = S, the screen ends (fade out). S = −1: wait
   until the left button is up.
Then: when the save directory has no `GAME0.SAV` and has `QICKSTRT.SAV`, `QICKSTRT.SAV` is renamed `GAME0.SAV`.

**Set Quick-Save Mode.** As Set Difficulty with: the screen black, the palette kept; "Set Quick-Save Mode:" at
y 65; two rows k = 0, 1 "No Quick-Save in Combat", "Quick-Save Anytime!" at y_k = 90, 110; Up: S > 0: S − 1;
S = 0: 1; S < 0: 0. Down: S = −1: 1; S < 1: S + 1; else 0. The choice is the quick-save mode q = S. No file rename.

**Leave** (the office, strategic.md "The office", hot spot 7): the office ends with result 7; the day loop stops
(no sunrise) and the main menu opens as above.

**Play script.** `ja1-port ISO EXE --script menu SEED SCRIPT OUT` starts at the main menu (no save). Lines
`menu X Y B [K,K,...]` are polls of the menu screens (main menu, Set Difficulty, Set Quick-Save Mode): the mouse at
(X, Y), B bit 0 = the left button held, then the key bytes (the screen takes one key per poll: an extended key is
two bytes in one poll, the rest wait for the next polls). stdout: `menu open R C` when the menu opens, `menu
choice K`, `menu difficulty D`, `menu quick-save Q`, `menu exit`. After a New Game or Continue choice the other
lines run as with a save, the day loop starting at the office (a `day` line runs it); `office leave` when the office
ends with Leave, then the next `menu` lines go to the menu again. JA1_MENU_PNG = the picture of the menu screen
after the last menu poll. In the window: `ja1-port ISO EXE --menu [SAVE_DIR]`.

Evidence (2026-10-25): GameMain 0x4FFEC (asm + pseudocode: FormatSaveName 0x4FDB8 = any GAMEn.SAV, rows at
MouseInRect 0x67C0C (60, 43 + 22k, 250, 65 + 22k), keys 0x48 / 0x50 / 13, mouse to (0xF0, 0x34 + 22k) by 0x681D0,
NewGameSetup → SetQuickSaveMode → NewGameInit → PlayLoop(0), C = 1 after each game), MainMenu 0x4FDF4, row copy
0x4FD48 (STARTEXT loaded at pitch 380: source (0, 22k + 66 lit), BlitCopy 190 x 22 to (60, 43 + 22k)), NewGameSetup
0x4F548 (y table 0x96252, texts 0x96474 + 8k, hit (125, y, 195, y + 12), fg 0x20 / 0xFF from the stack bytes,
QICKSTRT.SAV rename), SetQuickSaveMode 0x4F898 (y table 0x96258, texts 0x9648C + 24k, also sets byte 0x22A1FA = 1),
PlayLoop 0x4FB94 (office result 7 ends it), MouseInRect 0x67C0C (inclusive), 0x67C54 (wait for the left button up).
Screens seen in DOSBox: validate/shots/14..33, 36 (2026-10-17).
Program-start world (2026-10-28): main_ 0x102D2..0x102E7 runs InitTempFiles, FUN_000377b0 (the options),
NewGameInit(0, 0), then writes 0 to the A.I.M. link 0x22A043. Live in DOSBox-X: Continue Game right after the
start opens the office at day 0; SaveGame of that memory (build/probe/cont/save.py on mem_cont0) and the port's
`--new-game 0 0` save differ in the A.I.M. link and in option 5 only (the sound setup, section 11).

## Game speed (speed survey 2026-10-27)

The original under DOSBox-X 2026.10.01 (validate/speed_survey.py, live) at `cycles=fixed 23880` (the DOSBox-X menu
entry "486DX2 66MHz", src/gui/menu_callback.cpp) and at `cycles=max` (the GOG setting), and the port window:

| event | 486DX2-66 | cycles=max | port window | what sets the speed |
|---|---|---|---|---|
| merc walk, tac60 (22,17) → (29,17), 8 tile steps | 165 ms per tile (1650 ms per 10 tiles) | 166 ms per tile | 166 ms per tile (window_speed: 1160 ms vs 1163 ms for the 7 steps) | the soldier's own timer +0xB0 (10 ms ticks), not the pass rate |
| view slide (focus), 34 steps | 50.0 ms per step, 1700 ms | 50.0 ms per step, 1700 ms | was 0 ms (one jump); now 50 ms per step | ripple_timer (10 ms ticks) |
| new-game intro (INTRO\SAT.DAT island.SAB), START NEW GAME → Set Difficulty | 239.7 s | 238.0 s | 237.7 s | film frame times (FLC speed), script waits by the timer |
| quick message box ("Enemies nearby!") | 1500 ms (asm) | 1500 ms live (tac60 with the enemies byte of the home sector set to 1: 1531, 1499, 1496 ms; polled every ~3 ms) | 1500 ms | TimerElapsedMs 0x47040 (timer ticks) |
| merc message box ("Hector doesn't have enough Action Points!", merc_text 1200) | | 1175 ms live (fight59, 2 runs: 1173, 1192 ms from the press; the button held, polled every ~5 ms) | 1200 ms | TimerElapsedMs 0x47040 (timer ticks; the box start falls inside a 10 ms tick) |
| day with time compression, tac60 key C → clock 720 minutes | 29.3 s (2 runs: 29335, 29368 ms; key held 0.1 s or 0.6 s) | 28.8 s (28836 ms) | 144 turns × 20 ticks = 28.8 s | the turn timer, 100 times faster (turns.md) |

| sunset (end-of-day screen), 119 frames, tac60 key C | 6.5 s (the day end to the day replay: 7.97 s) | 54.7 ms per frame (median of 119 palette changes, 50.7 .. 57.9 ms; build/probe/sunsettime.py), the day end to the day replay 7.75 s | was 14.3 ms per frame (1.7 s); now one clock step per frame | the game clock (below): each frame waits "2 ms" |
| sunrise colour cycle, 90 frames | | | was 14.3 ms per frame (1.3 s); now one clock step per frame (4.9 s) | the game clock: each frame waits "1 ms" (asm 0x2A501; not timed live) |
| day replay flag animation, tac60 key C: records (59,59,`X`) and (59,59,`Y`) | ~1.94 s (stack polls every 50 ms stop the emulator, so an upper bound; Music off and the busy-wait count of a fresh 486 boot written in, see below) | 1308 and 1464 ms (upper bounds as at 486; the count 11764 of tac60 is from this speed) | 1620 ms (the busy-wait counts) | busy wait 0x67250: n × the loops per ms that 0x67294 counts at start (2 000 000 loops / their ms) |

**The game clock.** The original's timer routines (TimerStart / TimerElapsedMs) read the DOS time of day: seconds
and hundredths, where the hundredths come from the BIOS tick count (18.2 ticks per second). So the clock moves in
steps of one BIOS tick, 54.925 ms (65536 / 1193180 s); the elapsed milliseconds it gives are 10 × the hundredths
difference (steps of 50 or 60 ms). A wait "until at least t ms passed" ends at the first clock step where the
difference reaches t: a wait of 1 or 2 ms waits for the next step (0 .. 55 ms, ~55 ms when the wait comes right
after the previous one), a wait of 60 ms takes one or two steps. The front-end models it: a tick = floor(real time
since the program start × 1193180 / 65536); clock ms = 10 × floor(tick × 6553600 / 1193180) (the
hundredths of the DOS time: seconds × 100 + hundredths). (The 10 ms timer of the walk and the view slide is another clock: those ride the sound
driver's timer, not this one.)

Other short clock waits of the original (each TimerElapsedMs 0x47040 ≥ t: the wait ends at the first clock step where the
difference reaches t): t = 60 in the strategic
screen (0x4377C, 0x43837), the payroll screen (0x443EC), the assignment panel flip (0x47BAF), the inventory
screen (0x4A424) and the flip animation (0x51371); t = 1 in the payroll screen (0x4449B), the assignment panel flip
(0x47C1C) and the inventory screen (0x4A34F); t = 90 in the office highlight frame (0x4C0A0) and the switch box
(0x4D2B8); t = 80 at 0x4C273; t = 200 in the A.I.M. screen (0x20965) and PayDayDialog (0x45249). The front-end
waits on its game clock in every one of them: the payroll screen, the office (frames and palette cycle), the switch
box frames, the finance page flips (0x4377C, 0x43837, PlayFlipAnim 0x51371), the assignment screen's open (0x47BAF,
0x47C1C) and close (0x4A34F, 0x4A424) slides, the A.I.M. right-button repeat and the PayDayDialog blink. (The
tactical inventory has no clock wait: 0x4A34F and 0x4A424 belong to the assignment screen's close.)

**State loads and the busy wait.** A save state of DOSBox-X keeps the busy-wait count (loops per ms, 0x933C8) of
the run that recorded it: tac60 holds 11764 (recorded at cycles=max); a fresh boot at 23880 cycles calibrates 4081.
Timing a busy wait at 23880 cycles after a tac60 load therefore needs 4081 written in (else the flag animation took
5.6 s). With Music on, the day end of a tac60 load at either speed hangs in SetMusic 0x5C5EC → AIL_init_sequence →
AIL_call_driver (sbpro1.mdi, `nosound=true`); with the Music switch 0x1163B9 set to 0 after the load it runs on.

In words: the original paces nearly everything by its 10 ms timer or by a busy wait that it calibrates against the
timer at start, so a 486DX2-66 and a fast PC play at the same speed. The one exception: the tactical hover delays
(ap_timer 0x229DD4 800, item_timer, light_timer; cursor.md "Hover") step once per loop pass, and the loop has no
speed limit, so at cycles=max they end ~20x sooner (AP delay 800 -> 0 in ~0.04 s live; port: Play::hover_per_tick,
c5a75ed). An asm sweep (#29 O, 2026-10-29) of every screen loop in validate/screen_calls.py to call depth 5 found no
other per-pass counter: each other global or soldier counter that a loop steps is either gated by a timer counter
(0x229D24 turn bar 0x22A484/0x22A486, 0x229D48 blink 0x284F04, 0x229D38 tile cursor 0x95F5A, soldier +0xC4 / +0xA8
from ja1_tick) or counts game data (list positions, stats, day results); the credits wait for the retrace. The tactical pass takes ~60 ms at the 486 setting
(one update pass and one draw pass), but a walk frame waits longer than that, so the walk does not slow down. The
port matched every event but the view slide, which it now shows step by step. (The replay was
read in asm, not timed live; "Enemies nearby!" was timed live with build/probe/nearbytime.py; a merc message box was timed live in fight59 with build/probe/msgtime.py: right click,
then a held click on a far tile with Hector at 0 AP.)

**The speed gate.** (The new-game intro part runs only by hand: `validate/gate_speed.py intro`.) validate/gate_speed.py times each event in the port window (JA1_SPEED_LOG=1 stderr lines:
`speed MS SLOT TILE` per tile change, `window slide end MS ms`, `speed msgbox MS ms WANT`, `speed clock MS MINUTES
COMPRESS` per day clock move, `speed sunset|sunrise MS ms N frames`, `speed replay MS ms WAITS ms waits` when the
replay ran to its end, `speed intro MS ms`) and asserts each within 15 % of the original: the walk 1163 ms, a view
slide 10 ms per tick (first step 2 ticks, then 5 per step), the merc message box 1175 ms, Time compression 200 ms
per 5-minute turn (the turn that reaches 720 minutes holds the sunset, so it does not count), the sunset 6481 ms,
the day replay the sum of its waits, the sunrise 4888 ms, the battle result box 1500 ms and the intro 238.0 s
(START NEW GAME after the title part).
