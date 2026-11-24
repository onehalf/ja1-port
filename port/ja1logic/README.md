# ja1logic

Rust port of Jagged Alliance (1994) game logic. Plain C interface, see `include/ja1logic.h`. A separate harness
compares it with the original through ctypes.

**How to port: see CONTRIBUTING.md "How the port was written" (the maintainer 2026-10-31, issue #37).** Understand the original function from the
asm and the decomp first; the tests check that understanding.

## Build and test

    ~/.cargo/bin/cargo build --release && ~/.cargo/bin/cargo test

Output: `target/release/libja1logic.so` (cdylib; also an rlib that `port/ja1front` links). The diff tests load the
.so through `validate/diff/port_lib.py` (ctypes; `JA1_PORT_LIB` points them at another copy) with `--port`;
`./t check` builds it first. The C interface is `include/ja1logic.h`, described in `docs/spec/port_api.md`.

## History (slice notes, oldest first)

The first slices (to 2026-10-29) were written only from
`docs/spec/` and `defs/types.toml`, with no decompiler output; `// SPEC GAP:` marks places where the spec was silent.
The first slice was terrain move codes, step AP cost and A* path search.

Slice 2a (`src/ai.rs`): the enemy AI decision layer from `docs/spec/ai.md` ("One tick", "Alert state", "Choosing an
action" with Calm / Alerted / Combat / Engaged and AP cost, "Starting an action") behind the C API of
`docs/spec/port_api.md` (`Ja1World`, `Ja1Host`, nine `ja1_ai_*` entry points). Everything in the port_api host
table (positions, sightings, situation score, sector value, news value, attack options and attack-ready check,
walking and action AP, turning, walking, clicking, perceiving, redraw, waits) is a host call with the listed op
and arguments; the library reads world memory through raw pointers again after every host call. Guesses are
marked `// SPEC GAP:` in `src/ai.rs`.

Slice 2b part 2, first half (`src/strength.rs`): the "Situation score" of `docs/spec/ai.md` (Max AP, Armour,
Soldier power, Situation score). Op 20 opp_strength is now a library op (the combat and engaged decisions run the
library's own score unless bit 20 is set in host_ops). New world fields `remember_m` / `remember_e` at the end of
`Ja1World`, new host op 42 fire_line, new entry points `ja1_ai_opp_strength` and `ja1_ai_soldier_power`. Inner
calls direction (6), sector_value (9) and fire_line (42) are always host calls. Guesses are marked `// SPEC GAP:`.

Slice 2b part 2, batch 2 (`src/positions.rs`): "Positions" (Box, Retreat tile op 21, Position score, Search
position op 12) and "Sector value" (op 9) of `docs/spec/ai.md`. Ops 9, 12 and 21 are now library ops (same
host_ops rule). New world fields `story`, `plants`, `crates`, `day` at the end of `Ja1World`, new host ops 43
connected_home and 44 quest_check, new entry points `ja1_ai_retreat_tile`, `ja1_ai_search_tile`,
`ja1_ai_sector_value`. Position score writes the soldier records (tile, px, py) before its fire_line calls and
restores them after, as the spec says.

Slice 2c part 1 (`src/attack.rs`): "Attack options" of `docs/spec/ai.md` (Expected damage, Gun option op 24,
Melee hit chance, Knife damage, Knife option op 26). Ops 24 and 26 are now library ops (same host_ops rule); op 25
throw_option was still a host call then. New world fields `weapons`, `absorb` at the end of `Ja1World`, new host ops 45
shot_ap and 46 chance_to_hit, new entry points `ja1_ai_gun_option`, `ja1_ai_knife_option` (write the 10-byte
option record to `out`). Every division whose divisor can be 0 makes host call 47 div_zero first, then goes on
with quotient 0 (port_api.md "Division by zero").

Slice 2c part 2 (`src/attack.rs`): Throw damage, Throw AP, Throw chance and the Throw option (op 25, now a
library op like 24 and 26) of `docs/spec/ai.md` "Attack options"; entry point `ja1_ai_throw_option`. ground(T) =
t0_class[tile byte 0]; div_zero (op 47) at the divisions the spec marks.

Slice 2c part 3 (`src/fire.rs`): the "Line of fire" exact procedure of `docs/spec/ai.md`. Op 42 fire_line is now
a library op, so Soldier power (F = 1), Position score and the gun / throw options run it unless bit 42 is set in
host_ops. New host ops 48 hit_test (8-byte out record) and 49 sight_line; entry point `ja1_ai_line_of_fire`. The
leave-map test reuses the "Step from a tile" code of `src/search.rs`.

Animation slice 3 (`src/anim.rs`): "Starting an animation" of `docs/spec/ai.md`; entry point `ja1_anim_set`. Op 52
set_anim is now a library op: `ja1_anim_step` runs its own `ja1_anim_set` unless bit 52 is set in host_ops. Four new
world fields at the end of `Ja1World` (`anim_scripts`, `anim_free`, `anim_sounds`, `path_preview`); new host ops 55
ui_refresh .. 61 anim_frame; op 31 spend_ap gets a third argument (the sound). Table reads outside their table give 0.

Animation slice 4 (`src/anim.rs`): "Frame routine", "Next word" and "Turn done" of `docs/spec/ai.md`; entry points
`ja1_anim_frame` and `ja1_anim_turn`. Ops 56 anim_turn, 61 anim_frame and 84 anim_step are library ops like 52, so
`ja1_anim_set`, `ja1_anim_step` and the frame routine run the library's own code unless the op's bit is set in
host_ops (host_ops bit 63 stands for op 63 and every higher op, so it covers op 84). Ten new world fields after
`path_preview`; host ops 62..84. Tile standability and tile steps reuse `src/search.rs`.

Sector start batch 2 (`src/sector.rs`): "Batch 2: placing soldiers" of `docs/spec/sector_start.md` (XRA enemy,
Tapper, Guard, Guard tile, Place merc, Merc entry); entry points `ja1_xra_enemy`, `ja1_init_tapper`,
`ja1_init_guard`, `ja1_guard_tile`, `ja1_place_merc`, `ja1_merc_entry`. Host ops 51 replace and 58 reserve_ap are
always host calls. New world field `guard_skill` (u8) at the end of `Ja1World`. step, dirStep, occupant,
tile_standable and Occupy reuse the `ai` code through `ai::shared`; item resets reuse batch 1's Item reset.

Sector start batches 3 and 4 (`src/sector.rs`): "Putting an item on the ground" and "Taking items off the ground" of
`docs/spec/sector_start.md`; entry points `ja1_put_item` (no host call, no random number), `ja1_take_ground_item`
and `ja1_take_ground_patch`. New host op 85 cursor_back (no arguments). Six new world fields at the end of
`Ja1World` (`ground_items`, `notes`, `move_codes`, `held`, `blast_list`, `listed`). Take ground item rebuilds
`move_codes` with the crate's `rebuild` (lib.rs). A tile outside 0..3135 has byte 6 = 0xFF and is never indexed.
The patch recursion stops at depth 4096 (a listed type recurses forever in the original).

Sector start batch 5 (`src/sector.rs`): "Soldier start" of `docs/spec/sector_start.md`; entry point
`ja1_soldier_start` (no host call; random through the host). New world fields `t2_class` and `enemy_watch` at the
end of `Ja1World`. Max AP and Frame delay run the crate's own code (`strength.rs`, `anim.rs`) through `ai::shared`,
with the record index as the slot. The step writes `remember_m` / `remember_e` although the world declares them
const.

Sector start batch 6 (`src/sector.rs`): "Map patches, supply drops, plants" of `docs/spec/sector_start.md`; entry
points `ja1_tile_patch`, `ja1_item_patch` (int32 results), `ja1_crate_lost`, `ja1_crate_drops`,
`ja1_plant_sector`, `ja1_plant_done`. The library reads no files: new host ops 86 open_patch, 87 read (only the
bytes the host reports are copied), 88 close, 89 replay_event. The path text (`MISC\CRATE07.BIN` etc.) goes to the
new world field `text_buf` before op 86; other new fields `item_buf`, `crate_state`, `crate_shown`; `plants` is now
writable. Tile patch rebuilds one tile's move code with the crate's `move_code` (previous = `move_codes[t]`); Item
patch reuses batch 3's Put item and batch 4's Take ground item.

Sector start batch 7 (`src/sector.rs`): "Fixed items, the sector tmp file, ground blast" of
`docs/spec/sector_start.md`; entry points `ja1_fixed_items`, `ja1_tmp_file` (int32 result 1 / 0),
`ja1_ground_blast`. New world fields at the end of `Ja1World`: `fixed_items` (read only), `sectors`, `bomb_count`
(sizeof 472). New host ops 90 explode, 91 effect, 92 open_tmp (the tmp file is read with 87 and closed with 88).

Sector start batch 8 (`src/sector.rs`): "The whole sector start" of `docs/spec/sector_start.md`; entry point
`ja1_setup_sector(w, h, tile, facing)`, which runs batches 1-7 in the game's order. New world fields at the end:
`scratch`, `team_on_duty` (read only), `purifier` (sizeof 488); `story`, `remember_m` and `remember_e` are now
writable. New host ops 93 sector_resets, 94 keep_selected, 95 sector_tail, 96 seek; kind 4 of op 86 open_patch is
`DAT\XRA.DAT` and does not write `text_buf`.

Sector start batch 9 (`src/sector.rs`): "Replace and reserve AP" of `docs/spec/sector_start.md`; entry points
`ja1_replace_soldier` and `ja1_reserve_ap`. Ops 51 replace and 58 reserve_ap are now library ops: batch 2 (Place
merc, guard, tapper, XRA enemy, Merc entry) and the animation code run the library procedures unless the op's bit
is set in `host_ops`. Replace soldier reuses Frame delay (`anim.rs`) and the `t0_class` / `t1_class` / `t2_class`
tables; Reserve AP keeps host op 17 action_ap(slot, 0xC41, 1). No new world field.

Sector start batch 10 (`src/sector.rs`): "Radio result" of `docs/spec/sector_start.md`; entry point
`ja1_radio_result` (int32 result, the new mission step 0..4; it also writes `mission_target`). Op 33 radio_result
is now a library op: batch 8 step 6 and the AI's radio action run it unless bit 33 is set in `host_ops`. Host op
44 quest_check(0x1C9) stays a host call. No new world field.

Sector start batch 11 (`src/sector.rs`): "Quest check" of `docs/spec/sector_start.md`; entry point
`ja1_quest_check(w, h, quest)` (int32 result 1 / 0; no host call, no write). Op 44 quest_check is now a library op
(Radio result step 3, Sector value, the animation script's door code) unless bit 44 is set in `host_ops`. New last
world field `safe_opened` (u8, read only; sizeof stays 488).

Sector start batch 12 (`src/attack.rs`): "Action AP" of `docs/spec/sector_start.md`; entry point
`ja1_action_ap(w, h, slot, tile, flag)` (int32 result, a byte). Op 17 action_ap is now a library op (Reserve AP,
the attack options, the AI's AP checks) unless bit 17 is set in `host_ops`. It reuses Throw AP; its inner calls 45
shot_ap, 7 tile_dist, 14 reach_ap, 40 path_ap and 6 direction stay host calls. No new world field.

Sector start batch 13 (`src/attack.rs`): "Shot AP" of `docs/spec/sector_start.md`; entry point
`ja1_shot_ap(w, h, slot, tile)` (int32 result, a byte). Op 45 shot_ap is now a library op (Action AP, the gun and
knife options) unless bit 45 is set in `host_ops`. It reuses Max AP; its only host call is 47 div_zero. No new world
field.

Soldier update per frame (`src/update.rs`): `docs/spec/soldier_update.md`; entry points `ja1_soldier_frame`,
`ja1_active_frame`, `ja1_walk_step`, `ja1_walk_blocked`, `ja1_walk_end`, `ja1_stand_up`, `ja1_frame_checks`,
`ja1_merc_quote`, `ja1_enemies_see`. New library ops 97 anim_next (Next word), 98 walk_blocked, 109
take_ground_item and 119 ai_think (One tick), all on `host_ops` bit 63; host ops 99..126 are host calls. New last
world fields `screen_drawn` ... `splash` (port_api.md "soldier update"; sizeof 536).

Sector click (`src/mapclick.rs`): `ja1_map_click` and `ja1_map_start` from `docs/spec/strategic.md` "Sector click on the
island map"; new host op 302 guide_say. Reuses Connected to home and All accounts. Null pointers or a sector outside
0..59 give 0 (marked `// SPEC GAP:`).
