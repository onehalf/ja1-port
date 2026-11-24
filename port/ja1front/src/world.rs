//! The library's world (docs/spec/port_api.md "World") built from the player's own files, as
//! docs/spec/world_load.md sections 1-3 say, and the world image of section 6 (the diff test interface).
//! Every buffer is owned here at its full size; a later play path points a `Ja1World` at these buffers.
use crate::iso::Iso;
use std::io;

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// `n` bytes of `src` from `off`, or an error naming the file.
fn slice<'a>(src: &'a [u8], off: usize, n: usize, what: &str) -> io::Result<&'a [u8]> {
    src.get(off..off + n).ok_or_else(|| bad(format!("{} is {} bytes, need {}", what, src.len(), off + n)))
}

/// A buffer of `size` bytes: the first `n` bytes from `src` at `off`, the rest 0 (section 1).
fn table(src: &[u8], off: usize, n: usize, size: usize, what: &str) -> io::Result<Vec<u8>> {
    let mut v = vec![0u8; size];
    v[..n].copy_from_slice(slice(src, off, n, what)?);
    Ok(v)
}

/// Sizes of the buffer fields (section 6).
pub mod size {
    pub const SOLDIERS: usize = 24576;
    pub const TILES: usize = 25088;
    pub const SECTOR_STATE: usize = 720;
    pub const SECTORS: usize = 10620;
    pub const SCRATCH: usize = 0x200;
}

/// Sizes of the turns.md buffers `itq`, `bled`, `timed_fx` (port_api.md).
pub const ITQ: usize = 50;
pub const BLED: usize = 48;
pub const TIMED_FX: usize = 65;
/// Size of the turns.md `sector_spoken` buffer (one byte per sector).
pub const SECTOR_SPOKEN: usize = 60;
/// Size of the `fx_anims` buffer (port_api.md: 16 x 0x12 bytes).
pub const FX_ANIMS: usize = 16 * 0x12;

/// The inventory screen's buffers and scalars (section 8 "Inventory screen"; inventory.md). The cursor item is
/// `held`. Kept between screens; not in the world image.
#[derive(Clone, Debug, PartialEq)]
pub struct InvData {
    pub swap: Vec<u8>,
    pub picked: Vec<u8>,
    pub give: Vec<u8>,
    pub item_rects: Vec<u8>,
    pub slot_rects: Vec<u8>,
    pub vest_owner: u8,
    pub tip_item: u8,
    pub face_left: u8,
    pub cursor_drop: u8,
    pub drop_answer: i32,
    pub panel_dy: i32,
    pub voice_test: u8,
    /// panel.md "Clicks on the bottom strip": the strip box of the last left click that held the mouse.
    pub strip_box: u8,
}

/// The day clock fields (turns.md "The day clock"; world_load.md sections 2-4). Not in the world image.
#[derive(Clone, Debug, PartialEq)]
pub struct Clock {
    pub minute: u8,
    pub hour: u8,
    pub hours: i8,
    pub minutes: i32,
    pub bar: i32,
    pub hard_day: u8,
    pub night_warned: u8,
    /// 60 x 8 s16: the minute a hired guard reaches the sector (9999 = none).
    pub guard_eta: Vec<u8>,
    /// 60 x 8: the guard's record number per guard_eta entry.
    pub guard_rec: Vec<u8>,
}

impl Clock {
    /// Section 2: guard_eta every entry 9999, guard_rec 0; the clock at the start of the day (section 4 step 0).
    pub fn new() -> Clock {
        Clock {
            minute: 0,
            hour: 7,
            hours: 0,
            minutes: 0,
            bar: 0,
            hard_day: 0,
            night_warned: 0,
            guard_eta: 9999i16.to_le_bytes().repeat(480),
            guard_rec: vec![0; 480],
        }
    }
}

/// inventory.md "Item rects": the default table (the tactical view's).
pub const DEFAULT_ITEM_RECTS: [(i16, i16); 14] = [
    (108, 73), (203, 89), (161, 8), (108, 31), (200, 28), (108, 52), (257, 18),
    (257, 37), (257, 56), (257, 75), (257, 94), (257, 113), (108, 92), (159, 115),
];

impl Default for InvData {
    fn default() -> Self {
        let pairs = |v: &mut Vec<u8>, xy: &[(i16, i16)]| {
            for &(x, y) in xy {
                v.extend_from_slice(&x.to_le_bytes());
                v.extend_from_slice(&y.to_le_bytes());
            }
        };
        let mut item_rects = Vec::with_capacity(56);
        pairs(&mut item_rects, &DEFAULT_ITEM_RECTS);
        // inventory.md "Merc slots": (0, 14 + 42 k) for k = 0..3, (272, 14 + 42 (k - 4)) for k = 4..7.
        let slots: Vec<(i16, i16)> = (0..8).map(|k| if k < 4 { (0, 14 + 42 * k) } else { (272, 14 + 42 * (k - 4)) }).collect();
        let mut slot_rects = Vec::with_capacity(32);
        pairs(&mut slot_rects, &slots);
        InvData {
            swap: vec![0; 12],
            picked: vec![0; 8],
            give: vec![0x40; 2],
            item_rects,
            slot_rects,
            vest_owner: 0x40,
            tip_item: 0,
            face_left: 0,
            cursor_drop: 0,
            drop_answer: 0,
            panel_dy: 0,
            voice_test: 0,
            strip_box: 0,
        }
    }
}

/// The soldier update scalars of port_api.md (soldier_update.md), with their start values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UpdateScalars {
    pub screen_drawn: u8,
    pub interrupt_go: u8,
    pub turn_moves: u8,
    pub kept_merc: u8,
    pub next_selected: u8,
    pub quote_slot: u8,
    pub quote_tile: u16,
    pub quote_focus: u8,
    pub quote_delay: i8,
    pub quote_event: u8,
    pub opt_half_delay: u8,
    pub opt_follow: u8,
    pub opt_reload_msg: u8,
    pub alone_warned: u8,
    pub follow_all: u8,
    pub stop_timer: i32,
    pub interrupted: u8,
    pub walk_ap: i32,
    pub spotted: i32,
    pub opt_step_text: u8,
    // move order, one frame, timer tick (soldier_update.md); in the world image after opt_step_text
    pub key_shift: u8,
    pub key_alt: u8,
    /// click.md: Ctrl is held (start 0, world_load.md section 2); in the world image after key_alt.
    pub key_ctrl: u8,
    /// turns.md Turn start: ms of real time left (start 0, world_load.md section 2); in the world image as the last scalar (after last_killer).
    pub fast_timer: i32,
    pub opt_move_fx: u8,
    pub seen_enemies: u16,
    pub in_flight: u8,
    pub merc_shown: u8,
    pub interrupt_turn: u8,
    pub tick_step: i32,
    // merc view (soldier_update.md "Merc view"); in the world image after tick_step
    pub story_finder: u8,
    pub spot_stop: u8,
    // turns (turns.md); in the world image after spot_stop
    pub itq_len: u8,
    pub new_round: u8,
    pub turn_timer: i16,
    pub turn_timer2: i16,
    pub shown_slot: i32,
    pub view_tile: i32,
    pub focus_slot: u8,
    pub turn_shown: u8,
    pub merc_to_show: u8,
    pub view_hold: u8,
    // turns.md "Spend AP"; in the world image after view_hold
    pub weather_tire: i32,
    pub merc_tire: i8,
    // turns.md "New turn"; in the world image after merc_tire
    pub round_lock: u8,
    pub round_lock2: u8,
    pub handed_over: u8,
    pub turn_flag: u8,
    pub interrupter: u8,
    pub speaker: u8,
    pub speech_wait: u8,
    pub stage: u8,
    pub redraw_next: u8,
    pub redraw_pending: i32,
    pub turn_clock: i32,
    pub show_timer: i32,
    // shooting.md; in the world image after show_timer
    pub busy_timer: i32,
    pub replay_m: u8,
    pub first_hit: u8,
    pub fx_id: u8,
    // shooting.md batch 2; in the world image after fx_id
    pub marked_slot: u8,
    pub knockdown: u8,
    pub focus_last: u8,
    pub hit_scale: u8,
    // shooting.md batch 4 (death); in the world image after hit_scale
    pub dead_mercs: i32,
    pub dead_allies: i32,
    pub dead_npc: i32,
    pub enemy_deaths: i32,
    pub squad_lost: u8,
    pub last_killer: u8,
    // select.md; in the world image after fast_timer
    pub marks_shown: i32,
    pub marks_n: i32,
    pub opt_path_marks: u8,
    pub sees_slot: u8,
    pub preview_slot: u8,
    // soldier_update.md Update end / Fuses / Timer tick (the last world image scalars)
    // ripple_timer from the EXE, opt_ambient and enemies_warned from the day save (world_load.md sections 2-3).
    pub ripple_timer: i32,
    pub ripple_tile: i32,
    pub ambient_t1: i32,
    pub ambient_t2: i32,
    pub ambient_count: u8,
    pub opt_ambient: u8,
    pub time_compress: u8,
    pub enemies_warned: u8,
    pub leave_reason: u8,
    pub fuse_beep_timer: i32,
    pub fuse_beep: u8,
    // soldier_update.md Loop head
    pub turn_tick: i32,
    /// soldier_update.md Leaving the sector: != 0 = the tactical screen ends with no end of day (the front-end
    /// sets it; the sector setup clears it). The last scalar of the world image.
    pub quit_tactical: u8,
}

impl Default for UpdateScalars {
    fn default() -> Self {
        UpdateScalars {
            screen_drawn: 0,
            interrupt_go: 0,
            turn_moves: 0,
            kept_merc: 0,
            next_selected: 0,
            quote_slot: 0,
            quote_tile: 0xC41,
            quote_focus: 0,
            quote_delay: 0,
            quote_event: 0,
            opt_half_delay: 0,
            opt_follow: 0,
            opt_reload_msg: 0,
            alone_warned: 0,
            follow_all: 0,
            stop_timer: 0,
            interrupted: 0,
            walk_ap: 0,
            spotted: 0,
            opt_step_text: 0,
            key_shift: 0,
            key_alt: 0,
            key_ctrl: 0,
            fast_timer: 0,
            opt_move_fx: 0,
            seen_enemies: 0,
            in_flight: 0,
            merc_shown: 0,
            interrupt_turn: 0,
            tick_step: 0,
            story_finder: 0,
            spot_stop: 0,
            itq_len: 0,
            new_round: 0,
            turn_timer: 0,
            turn_timer2: 0,
            shown_slot: 0,
            view_tile: 0,
            focus_slot: 0,
            turn_shown: 0,
            merc_to_show: 0,
            view_hold: 0,
            weather_tire: 0,
            merc_tire: 0,
            round_lock: 0,
            round_lock2: 0,
            handed_over: 0,
            turn_flag: 0,
            interrupter: 0,
            speaker: 0,
            speech_wait: 0,
            stage: 0,
            redraw_next: 0,
            redraw_pending: 0,
            turn_clock: 0,
            show_timer: 0,
            busy_timer: 0,
            replay_m: 0,
            first_hit: 0,
            fx_id: 0,
            marked_slot: 0,
            knockdown: 0,
            focus_last: 0,
            hit_scale: 0,
            dead_mercs: 0,
            dead_allies: 0,
            dead_npc: 0,
            enemy_deaths: 0,
            squad_lost: 0,
            last_killer: 0,
            marks_shown: 0,
            marks_n: 0,
            opt_path_marks: 0,
            sees_slot: 0,
            preview_slot: 0,
            ripple_timer: 0,
            ripple_tile: 0,
            ambient_t1: 0,
            ambient_t2: 0,
            ambient_count: 0,
            opt_ambient: 0,
            time_compress: 0,
            enemies_warned: 0,
            leave_reason: 0,
            fuse_beep_timer: 0,
            fuse_beep: 0,
            turn_tick: 0,
            quit_tactical: 0,
        }
    }
}

pub const SECTOR_COUNT: usize = 60;
pub const SECTOR_ROW: usize = 177;
/// Bytes per saved sector file (section 3).
pub const TMP_FILE: usize = 18080;
/// Offset of the first saved sector file in a day save.
pub const SAVE_FIXED_END: usize = 21649;
/// The map before the first sector (InitProgramWorld? 0x377B0): each tile 0x20, 0, 0, 0xFF, 0, no occupant 0x40, no
/// ground item 0xFF; byte 7 stays 0. LoadTacticalSector writes over it (data::fill_map_record).
pub fn start_tile_map() -> Vec<u8> {
    [0x20u8, 0, 0, 0xFF, 0, 0x40, 0xFF, 0].repeat(size::TILES / 8)
}

/// The random seed of a game. The original seeds from the clock at each program start (SrandTime? 0x53564: time_, then
/// srand_), so two runs give other numbers. JA1_SEED=N (tests only) sets it.
pub fn start_seed() -> u32 {
    if let Some(n) = std::env::var("JA1_SEED").ok().and_then(|s| s.parse().ok()) {
        return n;
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    let seed = clock_seed(now.map_or(0, |d| d.as_secs()));
    eprintln!("start seed {} (clock)", seed);
    seed
}

#[cfg(test)]
mod seed_tests {
    /// Ledger row c2aea50:ja1front:22654: with no JA1_SEED the seed is the clock (SrandTime? 0x53564: time_, srand_),
    /// so each program start differs.
    #[test]
    fn the_seed_comes_from_the_clock() {
        if std::env::var_os("JA1_SEED").is_some() {
            return;
        }
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
        let s = super::start_seed();
        assert!(s >= super::clock_seed(now) && s <= super::clock_seed(now + 2), "{s}");
    }
}

/// srand_ takes the low 32 bits of time_'s value.
pub fn clock_seed(secs: u64) -> u32 {
    secs as u32
}

/// JA.EXE offset of the 60 entry tiles (i32 each).
const ENTRY_TILES: usize = 904564;

/// InitTempFiles 0x38038: the original deletes sec00.tmp .. sec59.tmp and replay.rec (main_, QuickRestore, NewGameInit,
/// the quit line). The port keeps these files in the world.
pub fn init_temp_files(tmp: &mut Vec<Option<Vec<u8>>>, replay: &mut Vec<[u8; 3]>) {
    *tmp = vec![None; SECTOR_COUNT];
    replay.clear();
}

/// The world fields (port_api.md) plus what the front-end keeps beside them.
#[derive(Clone)]
pub struct World {
    // buffers
    pub soldiers: Vec<u8>,
    pub tiles: Vec<u8>,
    pub item_types: Vec<u8>,
    pub anim_table: Vec<u8>,
    pub sector_state: Vec<u8>,
    pub mission_done: Vec<u8>,
    pub knowledge: Vec<u8>,
    pub fresher: Vec<u8>,
    pub last_seen_m: Vec<u8>,
    pub last_seen_e: Vec<u8>,
    pub calls: Vec<u8>,
    pub paths: Vec<u8>,
    pub dir_steps: Vec<u8>,
    pub t0_class: Vec<u8>,
    pub t1_class: Vec<u8>,
    pub remember_m: Vec<u8>,
    pub remember_e: Vec<u8>,
    pub story: Vec<u8>,
    pub plants: Vec<u8>,
    /// strategic.md Day accounts: the first 0xA8 bytes of the day save's game state block (not in `image`).
    pub accounts: Vec<u8>,
    /// The 61 sector numbers of the day save's sector processing order (not in `image`).
    pub sector_order: Vec<u8>,
    /// world_load.md section 3 "Strategic fields" (none of them is in `image`):
    /// 64 profile records of 0x5A bytes (60..63 = the natives from EDT\NEWPROF.EDT).
    pub profiles: Vec<u8>,
    /// 136 bytes: the first 136 bytes of the save's item stash.
    pub stash_counts: Vec<u8>,
    /// The item store records (type, count, condition) in list order (strategic.md "The item store").
    pub stash_records: Vec<[u8; 3]>,
    /// 144 bytes from the save.
    pub stat_progress: Vec<u8>,
    /// 256 bytes, JA.EXE (section 1).
    pub greed: Vec<u8>,
    /// 640 bytes, start 0 (section 2).
    pub stat_changed: Vec<u8>,
    /// Section 2 (0x40 = none).
    pub duty7_merc: u8,
    /// The funds at the start of the tactical day (section 4 step 2).
    pub prev_balance: i32,
    /// 540 bytes: copy of `sector_state` bytes 0..539 (after the load; again at each day start).
    pub sector_replay: Vec<u8>,
    /// The guide's messages of the day (7 bytes; strategic.md Story events at the day start).
    pub guide_queue: Vec<u8>,
    /// Native guides hired by the guide's offer (strategic.md The guide's advice; day save 20895).
    pub guides_hired: u8,
    /// Days in a row without a guide merc since the last offer (day save 20896).
    pub guide_wait: u8,
    /// world_load.md section 4 step 3: 1 = no merc could be chosen at the sector entry.
    pub idle_day: u8,
    /// strategic.md Enemy moves: the enemy commander's place in sector_order.
    pub enemy_counter: u8,
    /// strategic.md The day's verdict: the day-start copies (set at each sector entry, world_load.md section 4 step 2).
    pub start_trees: i32,
    pub start_workable: i32,
    pub start_tapped: i32,
    pub start_sectors: i32,
    pub start_cap: i32,
    pub start_plants: i32,
    /// The guide spoke of the ransom items 0x86, 0x7A, 0x5C (day save 21282..21284).
    pub item_told: Vec<u8>,
    pub bonus_due: u8,
    pub bonus: u16,
    /// combat.md Blast, exact steps (not in `image`): the last explosion's thrower, the 5 gas clouds (30 bytes),
    /// the current map's tileset and the 10-byte no_break table from JA.EXE (section 1).
    pub blaster: u8,
    pub gas_clouds: Vec<u8>,
    pub tileset: u8,
    pub no_break: Vec<u8>,
    /// combat.md The listening device at a door (not in `image`): walter_said (12 bytes, day save 20957),
    /// walter_next (8 bytes, day save 20975), walter_sector (99 at program start, not saved).
    pub walter_said: Vec<u8>,
    pub walter_next: Vec<u8>,
    pub walter_sector: u8,
    /// strategic.md "The day replay": the count kept from one replay event to the next (not in the saves).
    pub replay_carry: u8,
    /// The day clock (turns.md "The day clock"; not in `image`).
    pub clock: Clock,
    pub crates: Vec<u8>,
    pub weapons: Vec<u8>,
    pub absorb: Vec<u8>,
    pub anim_rows: Vec<u8>,
    pub anim_scripts: Vec<u8>,
    pub anim_free: Vec<u8>,
    pub anim_sounds: Vec<u8>,
    pub anim_steps_tile: Vec<u8>,
    pub anim_turn_while: Vec<u8>,
    pub supplies: Vec<u8>,
    pub equip_skill: Vec<u8>,
    pub attach: Vec<u8>,
    pub ground_items: Vec<u8>,
    pub notes: Vec<u8>,
    pub move_codes: Vec<u8>,
    pub held: Vec<u8>,
    pub blast_list: Vec<u8>,
    pub listed: Vec<u8>,
    pub t2_class: Vec<u8>,
    pub enemy_watch: Vec<u8>,
    pub text_buf: Vec<u8>,
    pub item_buf: Vec<u8>,
    pub crate_state: Vec<u8>,
    pub crate_shown: Vec<u8>,
    pub fixed_items: Vec<u8>,
    pub sectors: Vec<u8>,
    /// SPEC GAP: world_load.md does not list `scratch` (port_api.md batch 8, 0x200 bytes); it starts at 0 and is
    /// not part of the world image.
    pub scratch: Vec<u8>,
    pub ap_warned: Vec<u8>,
    pub splash: Vec<u8>,
    /// 16 x 0x12 bytes, start 0 (section 2); in the world image after splash.
    pub fx_anims: Vec<u8>,
    /// turns.md: the interrupt queue (50), bled (48), timed effects (65); start 0; in the world image after fx_anims.
    pub itq: Vec<u8>,
    pub bled: Vec<u8>,
    pub timed_fx: Vec<u8>,
    /// turns.md New turn: 60 bytes, one per sector, from the day save (section 3); in the world image after timed_fx.
    pub sector_spoken: Vec<u8>,
    /// 306 bytes from JA.EXE (section 1); in the world image after fx_anims.
    pub view_rays: Vec<u8>,
    /// 685 bytes from JA.EXE (section 1); in the world image after view_rays.
    pub obj_block: Vec<u8>,
    /// shooting.md tables from JA.EXE (section 1): 17, 34, 33, 9 bytes; in the world image after obj_block.
    pub silencer: Vec<u8>,
    pub shot_sound: Vec<u8>,
    pub impact_noise: Vec<u8>,
    pub shot_range: Vec<u8>,
    /// shooting.md batch 5 tables from JA.EXE (section 1): posture 256, window 2056, attack_sound 10, diff_mod 6
    /// bytes; in the world image after shot_range.
    pub posture: Vec<u8>,
    pub window: Vec<u8>,
    pub attack_sound: Vec<u8>,
    pub diff_mod: Vec<u8>,
    /// shooting.md batch 2: 240 bytes, start 0 (section 2); in the world image after shot_range.
    pub dmg_text: Vec<u8>,
    /// select.md: 128 x u16, the path preview's marked tiles; start 0 (section 2); the last buffer of the world image.
    pub marks: Vec<u8>,
    // scalars (port_api.md struct types)
    pub cur_sector: u8,
    pub alarm: u8,
    pub combat: u8,
    pub difficulty: u8,
    pub mission_step: i8,
    pub mission_slot: u8,
    pub mission_target: u16,
    pub mission_spot: u16,
    pub turn_based: i32,
    pub busy: i32,
    pub active_slot: u8,
    pub santino_spoke: u8,
    pub santino_timer: i16,
    pub show_ai: u8,
    pub santino_word: u32,
    pub last_spotter: u8,
    pub day: i32,
    pub selected: u8,
    pub path_preview: i32,
    pub opt_sound: u8,
    /// The confirm-move option (save byte 21552): a walk click needs a second click (cursor.md "Clicks").
    pub opt_confirm_move: u8,
    pub opt_onscreen: u8,
    /// panel.md "View modes": the last wide mode (2 or 3, minus 2), the old mode, the overview filter.
    pub view_last_wide: i8,
    pub view_old_mode: u8,
    pub overview_filter: u8,
    /// panel.md "The small panel in the full view": the small panel timer, the panel ends byte, the full view
    /// shown byte.
    /// InitSound 0x5CED4: the drivers SOUND.CFG names (program-run values, not in a save).
    pub sound_setup: crate::startup::SoundSetup,
    pub small_panel: i32,
    pub panel_ends: u8,
    pub full_shown: u8,
    /// panel.md "The interface strips": the strip flag (0 = the bottom strip does not go to the screen).
    pub strip_flag: u8,
    /// cursor.md "Tile marker": kind and tile as the quick save keeps them (blocks 42 / 41); Play's cursor state
    /// holds the live marker and copies it here before a quick save, and back after a quick restore.
    /// Quick save blocks 64, 65: the cursor mode 0x284F1B and the cursor picture 0x284F1D (copies of `CursorState`
    /// at a quick save and a quick restore, as `marker_kind`).
    /// 0x229E00..2, a copy for Quick Save and Quick Restore: the queued sighting reaction (slot, how, near; `sighting::Sight`).
    pub sight_queue: [u8; 3],
    pub cursor_mode: u8,
    pub cursor_value: u8,
    /// Quick save block 68: the wanted music theme 0x96360 (a copy of the music player's at the same two points).
    pub music_wanted: i16,
    pub marker_kind: i32,
    pub marker_tile: i32,
    /// Quick save blocks 118, 119: the mouse place of MouseSavePos2? 0x67E18 (pointer::mouse_saved holds it during play).
    pub mouse_saved_x: i32,
    pub mouse_saved_y: i32,
    /// savegame.md "Tail" step 3: the day's replay log (strategic.md "The day replay") as the quick save keeps it;
    /// Play's host holds the live log and copies it here before a quick save, and back after a quick restore.
    pub replay_log: Vec<[u8; 3]>,
    pub view_x: i32,
    pub view_y: i32,
    pub view_w: i32,
    pub view_h: i32,
    pub anim_signal: i32,
    pub effect_tile: i16,
    pub guard_skill: u8,
    pub bomb_count: i32,
    pub team_on_duty: i32,
    pub purifier: u8,
    pub safe_opened: u8,
    /// Soldier update scalars (soldier_update.md); in the world image after `safe_opened`.
    pub update: UpdateScalars,
    /// The inventory screen's data (section 8 "Inventory screen"); not in the world image.
    pub inv: InvData,
    // front-end data
    /// Entry tile of sector n (0..59), JA.EXE (section 1).
    pub entry_tiles: [i32; SECTOR_COUNT],
    /// The saved sector files (section 3): the tmp file of sector i for host op 92 open_tmp, None = no tmp file.
    pub tmp_files: Vec<Option<Vec<u8>>>,
    /// Bytes 0..21648 of the day save that was loaded (section 10 "The base").
    pub base: Vec<u8>,
    /// The last quote (world_load.md section 3, save bytes 21270..21275): speaker, category, row (u16), quote (u16).
    pub last_quote: [u8; 6],
    /// The profile an A.I.M. refusal named (save byte 21276; strategic.md "Refusal reason"): "MEMBER" in the
    /// say-again text (panel.md "Say it again (key L)").
    pub quote_member: u8,
    /// The game flag "hinted" (strategic.md "Start the day"; save byte 21280).
    pub hinted: u8,
    /// g_lossSlot 0x22A1F1 (save byte 21277): the soldier of the stat-loss message (play::loss_set, play::loss_pass);
    /// its amount g_lossAmount 0x22A02F is `accounts` +0x8F.
    pub loss_slot: u8,
    /// g_lossTimer 0x92DE8 (quick save only): 500 or 7500 when set, 10 less per timer tick down to 0, then -1 once shown.
    pub loss_timer: i32,
    /// g_lossPart 0xFA908 (quick save only): "head" / "arm" / "leg".
    pub loss_part: [u8; 16],
    /// g_lossStat 0xFA918 (quick save only): "wisdom" / "dexterity" / "agility".
    pub loss_stat: [u8; 16],
    /// The "sees enemies" line of host op 197 (panel.md "Tactical messages"); drawn while sees_slot < 0x40.
    pub sees_text: Vec<u8>,
    /// tactical_map.md "Animated tiles": the frame of each tile animation 0..9 (None = count 0), set by play.
    pub tile_frames: [Option<u8>; 10],
    /// Quick save block 78, tile animations 0x229E24 as the file holds them (`AnimTimers::to_block`, `from_block`).
    pub tile_anim_block: [u8; 110],
}

fn u16_at(b: &[u8], off: usize) -> io::Result<u16> {
    let s = slice(b, off, 2, "JA.EXE")?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}
fn i32_at(b: &[u8], off: usize, what: &str) -> io::Result<i32> {
    let s = slice(b, off, 4, what)?;
    Ok(i32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}
fn u8_at(b: &[u8], off: usize, what: &str) -> io::Result<u8> {
    Ok(slice(b, off, 1, what)?[0])
}

impl World {
    /// Sections 1-3 from the bytes of JA.EXE, DAT/JAGGED.DAT and a day save. No sector entry.
    pub fn from_files(exe: &[u8], jagged: &[u8], save: &[u8]) -> io::Result<World> {
        let e = |off: usize, n: usize, size: usize| table(exe, off, n, size, "JA.EXE");
        let j = |off: usize, n: usize, size: usize| table(jagged, off, n, size, "JAGGED.DAT");
        let mut entry_tiles = [0i32; SECTOR_COUNT];
        for (n, t) in entry_tiles.iter_mut().enumerate() {
            *t = i32_at(exe, ENTRY_TILES + 4 * n, "JA.EXE")?;
        }
        let mut w = World {
            // section 1
            item_types: e(900764, 1632, 1632)?,
            anim_table: e(893450, 2048, 2048)?,
            anim_rows: e(893446, 2048, 2048)?,
            anim_free: e(893241, 256, 256)?,
            anim_sounds: e(893310, 512, 512)?,
            anim_steps_tile: e(893173, 256, 256)?,
            anim_turn_while: e(893990, 256, 256)?,
            fresher: e(903691, 2304, 2304)?,
            dir_steps: e(893020, 514, 514)?,
            listed: e(892758, 32, 32)?,
            crates: e(904548, 16, 16)?,
            fixed_items: e(905276, 88, 88)?,
            weapons: e(900309, 1536, 1536)?,
            absorb: e(892866, 576, 576)?,
            equip_skill: e(904428, 6, 6)?,
            attach: e(899918, 328, 328)?,
            anim_scripts: j(7984, 5440, 5440)?,
            t0_class: j(22128, 157, 256)?,
            t1_class: j(22285, 256, 256)?,
            t2_class: j(22541, 155, 256)?,
            splash: e(892698, 60, 60)?,
            view_rays: e(894456, 306, 306)?,
            obj_block: e(902968, 685, 685)?,
            silencer: e(892836, 17, 17)?,
            shot_sound: e(900736, 34, 34)?,
            impact_noise: e(892869, 33, 33)?,
            shot_range: e(906223, 9, 9)?,
            posture: e(893105, 256, 256)?,
            window: e(903212, 2056, 2056)?,
            attack_sound: e(892682, 10, 10)?,
            diff_mod: e(904434, 6, 6)?,
            dmg_text: vec![0; DMG_TEXT],
            marks: vec![0; MARKS],
            // section 2
            calls: e(892510, 8, 8)?,
            combat: u8_at(exe, 892527, "JA.EXE")?,
            mission_slot: u8_at(exe, 892492, "JA.EXE")?,
            mission_target: u16_at(exe, 892494)?,
            mission_spot: u16_at(exe, 892660)?,
            turn_based: i32_at(exe, 892496, "JA.EXE")?,
            busy: i32_at(exe, 892636, "JA.EXE")?,
            show_ai: u8_at(exe, 892529, "JA.EXE")?,
            opt_onscreen: u8_at(exe, 892547, "JA.EXE")?,
            // 0x92F8A, 0x92DB9, 0x92DC1 (file offset = address + 0x470A4, as opt_onscreen 0x92DDF)
            view_last_wide: u8_at(exe, 0x92F8A + 0x470A4, "JA.EXE")? as i8,
            view_old_mode: u8_at(exe, 0x92DB9 + 0x470A4, "JA.EXE")?,
            overview_filter: u8_at(exe, 0x92DC1 + 0x470A4, "JA.EXE")?,
            // 0x92E30, 0x92E35, 0x92E34
            sound_setup: Default::default(),
            small_panel: i32_at(exe, 0x92E30 + 0x470A4, "JA.EXE")?,
            panel_ends: u8_at(exe, 0x92E35 + 0x470A4, "JA.EXE")?,
            full_shown: u8_at(exe, 0x92E34 + 0x470A4, "JA.EXE")?,
            strip_flag: u8_at(exe, 0x92E36 + 0x470A4, "JA.EXE")?,
            sight_queue: [0x40, 0x7F, 0x40],
            cursor_mode: 1,
            cursor_value: 0x8E,
            music_wanted: -1,
            marker_kind: 0,
            marker_tile: 0xC41,
            mouse_saved_x: 0,
            mouse_saved_y: 0,
            replay_log: Vec::new(),
            effect_tile: u16_at(exe, 892656)? as i16,
            bomb_count: i32_at(exe, 892640, "JA.EXE")?,
            view_w: 224,
            view_h: 168,
            soldiers: vec![0; size::SOLDIERS],
            tiles: start_tile_map(),
            sector_state: vec![0; size::SECTOR_STATE],
            mission_done: vec![0; 80],
            knowledge: vec![0; 64],
            last_seen_m: vec![0; 256],
            last_seen_e: vec![0; 640],
            paths: vec![0; 3840],
            remember_m: vec![0; 128],
            remember_e: vec![0; 320],
            story: vec![0; 43],
            plants: vec![0; 48],
            accounts: vec![0; 0xA8],
            sector_order: vec![0; 61],
            profiles: vec![0; 64 * 0x5A],
            stash_counts: vec![0; 136],
            stash_records: Vec::new(),
            stat_progress: vec![0; 144],
            greed: {
                let mut g = vec![0u8; 256];
                g[..64].copy_from_slice(slice(exe, 902801, 64, "JA.EXE")?);
                g
            },
            stat_changed: vec![0; 640],
            duty7_merc: u8_at(exe, 892542, "JA.EXE")?,
            prev_balance: 0,
            sector_replay: vec![0; 540],
            guide_queue: vec![0; 7],
            guides_hired: 0,
            guide_wait: 0,
            idle_day: 0,
            enemy_counter: 0,
            start_trees: 0,
            start_workable: 0,
            start_tapped: 0,
            start_sectors: 0,
            start_cap: 0,
            start_plants: 0,
            item_told: vec![0; 3],
            bonus_due: 0,
            bonus: 0,
            blaster: 0x40,
            gas_clouds: [0x41u8, 0x0C, 0, 0, 0, 0].repeat(5),
            tileset: 0,
            no_break: e(892645, 10, 10)?,
            walter_said: vec![99; 12],
            walter_next: vec![0; 8],
            walter_sector: 99,
            replay_carry: 0,
            clock: Clock::new(),
            supplies: vec![0; 4],
            ground_items: vec![0; 2400],
            notes: vec![0; 30],
            move_codes: vec![0; 6272],
            held: vec![0; 12],
            blast_list: vec![0; 60],
            enemy_watch: vec![0; 48],
            text_buf: vec![0; 128],
            item_buf: vec![0; 12],
            crate_state: vec![0; 4],
            crate_shown: vec![0; 4],
            sectors: vec![0; size::SECTORS],
            scratch: vec![0; size::SCRATCH],
            inv: InvData::default(),
            cur_sector: 0,
            alarm: 0,
            difficulty: 0,
            mission_step: 0,
            active_slot: 0,
            santino_spoke: 0,
            santino_timer: 0,
            santino_word: 0,
            last_spotter: 0,
            day: 0,
            selected: 0,
            path_preview: 0,
            opt_sound: 0,
            opt_confirm_move: 0,
            view_x: 0,
            view_y: 0,
            anim_signal: 0,
            guard_skill: 0,
            team_on_duty: 0,
            purifier: 0,
            safe_opened: 0,
            ap_warned: vec![0; 8],
            fx_anims: vec![0; FX_ANIMS],
            itq: vec![0; ITQ],
            bled: vec![0; BLED],
            timed_fx: vec![0; TIMED_FX],
            sector_spoken: vec![0; SECTOR_SPOKEN],
            update: UpdateScalars {
                screen_drawn: u8_at(exe, 892528, "JA.EXE")?,
                interrupt_go: u8_at(exe, 892531, "JA.EXE")?,
                turn_moves: u8_at(exe, 892532, "JA.EXE")?,
                interrupted: u8_at(exe, 892535, "JA.EXE")?,
                quote_slot: u8_at(exe, 892560, "JA.EXE")?,
                quote_tile: u16_at(exe, 892562)?,
                follow_all: u8_at(exe, 894204, "JA.EXE")?,
                stop_timer: i32_at(exe, 904812, "JA.EXE")?,
                in_flight: u8_at(exe, 892520, "JA.EXE")?,
                merc_shown: u8_at(exe, 892524, "JA.EXE")?,
                interrupt_turn: u8_at(exe, 892544, "JA.EXE")?,
                tick_step: i32_at(exe, 892932, "JA.EXE")?,
                story_finder: u8_at(exe, 892551, "JA.EXE")?,
                spot_stop: u8_at(exe, 892680, "JA.EXE")?,
                merc_to_show: u8_at(exe, 892523, "JA.EXE")?,
                focus_slot: u8_at(exe, 892662, "JA.EXE")?,
                round_lock: u8_at(exe, 892548, "JA.EXE")?,
                round_lock2: u8_at(exe, 892549, "JA.EXE")?,
                handed_over: u8_at(exe, 892545, "JA.EXE")?,
                redraw_pending: i32_at(exe, 892488, "JA.EXE")?,
                turn_flag: u8_at(exe, 892525, "JA.EXE")?,
                interrupter: u8_at(exe, 892534, "JA.EXE")?,
                show_timer: i32_at(exe, 896504, "JA.EXE")?,
                speaker: u8_at(exe, 892537, "JA.EXE")?,
                busy_timer: i32_at(exe, 892504, "JA.EXE")?,
                marked_slot: u8_at(exe, 904880, "JA.EXE")?,
                knockdown: u8_at(exe, 892694, "JA.EXE")?,
                focus_last: u8_at(exe, 892663, "JA.EXE")?,
                hit_scale: u8_at(exe, 892521, "JA.EXE")?,
                sees_slot: u8_at(exe, 892522, "JA.EXE")?,
                preview_slot: u8_at(exe, 892668, "JA.EXE")?,
                ripple_timer: i32_at(exe, 892828, "JA.EXE")?,
                ..UpdateScalars::default()
            },
            entry_tiles,
            tmp_files: vec![None; SECTOR_COUNT],
            base: Vec::new(),
            last_quote: [0; 6],
            quote_member: 0,
            hinted: 0,
            loss_slot: 0,
            loss_timer: -1, // the image's value at 0x92DE8
            loss_part: [0; 16],
            loss_stat: [0; 16],
            sees_text: Vec::new(),
            tile_frames: [None; 10],
            tile_anim_block: [0; 110],
        };
        w.load_save(jagged, save)?;
        Ok(w)
    }

    /// Section 3: the day save's fixed blocks, the sector reset from JAGGED.DAT, the saved sector files.
    pub(crate) fn load_save(&mut self, jagged: &[u8], save: &[u8]) -> io::Result<()> {
        let s = |off: usize, n: usize| slice(save, off, n, "the day save");
        let copy = |dst: &mut [u8], off: usize, n: usize| -> io::Result<()> {
            dst[..n].copy_from_slice(s(off, n)?);
            Ok(())
        };
        let b = |off: usize| -> io::Result<u8> { Ok(s(off, 1)?[0]) };
        let d = |off: usize| -> io::Result<[u8; 4]> {
            let v = s(off, 4)?;
            Ok([v[0], v[1], v[2], v[3]])
        };
        self.base = s(0, SAVE_FIXED_END)?.to_vec();
        copy(&mut self.soldiers, 28, 4096)?;
        copy(&mut self.sector_state, 9524, 540)?;
        copy(&mut self.sectors, 10064, 10620)?;
        self.day = i32::from_le_bytes(d(20688)?);
        self.team_on_duty = i32::from_le_bytes(d(20696)?);
        self.santino_word = u32::from_le_bytes(d(20788)?);
        self.guard_skill = b(20828)?;
        self.cur_sector = b(20837)?;
        // SPEC GAP: world_load.md lists `story` as 35 bytes; the guide's advice reads 43 (S[0x23..0x2A] are the
        // save bytes 20879..20886, just before crate_state), so the port copies 43.
        copy(&mut self.story, 20844, 43)?;
        // SPEC GAP: world_load.md does not list guides_hired / guide_wait; their offsets come from the addresses in
        // strategic.md (S + 0x33, S + 0x34, as hard_day = S + 0xCC = 21048).
        self.guides_hired = b(20895)?;
        self.guide_wait = b(20896)?;
        copy(&mut self.crate_state, 20887, 4)?;
        copy(&mut self.crate_shown, 20891, 4)?;
        copy(&mut self.supplies, 20970, 4)?;
        self.enemy_counter = b(20969)?;
        // SPEC GAP: world_load.md does not list item_told; its offset comes from the address in strategic.md
        // (0x22A1F6 = S + 0x1B6).
        copy(&mut self.item_told, 21282, 3)?;
        copy(&mut self.walter_said, 20957, 12)?;
        copy(&mut self.walter_next, 20975, 8)?;
        self.santino_spoke = b(20974)?;
        self.purifier = b(20983)?;
        self.difficulty = b(20984)?;
        self.alarm = b(20986)?;
        self.mission_step = b(20988)? as i8;
        self.update.opt_reload_msg = b(20826)?;
        copy(&mut self.ap_warned, 21016, 8)?;
        self.update.alone_warned = b(21035)?;
        self.last_spotter = b(21044)?;
        self.safe_opened = b(21045)?;
        copy(&mut self.plants, 21078, 48)?;
        copy(&mut self.accounts, 20684, 0xA8)?;
        copy(&mut self.sector_order, 21558, 61)?;
        copy(&mut self.profiles, 4124, 5400)?;
        copy(&mut self.stat_progress, 21126, 144)?;
        copy(&mut self.mission_done, 21287, 80)?;
        self.update.opt_step_text = b(21543)?;
        self.update.opt_half_delay = b(21547)?;
        self.update.opt_follow = b(21548)?;
        self.opt_sound = b(21549)?;
        self.opt_confirm_move = b(21552)?;
        self.update.opt_move_fx = b(21553)?;
        self.update.opt_path_marks = b(21545)?;
        self.update.merc_tire = b(20869)? as i8;
        self.update.weather_tire = i32::from_le_bytes(d(21036)?);
        copy(&mut self.sector_spoken, 20897, 60)?;
        self.update.stage = b(21278)?;
        self.update.redraw_next = b(21279)?;
        self.update.replay_m = b(20989)?;
        self.update.first_hit = b(21281)?;
        self.update.last_killer = b(20990)?;
        // LoadGame 0x4E964 reads the whole block: these two also come from the file (loadgame_diff noise cases).
        self.clock.night_warned = b(20991)?;
        self.idle_day = b(21046)?;
        self.update.squad_lost = b(21047)?;
        self.update.opt_ambient = b(21546)?;
        self.update.enemies_warned = b(20987)?;
        self.clock.hard_day = b(21048)?;
        // The day clock and the new-round flag (block 5 bytes 301, 340..350): LoadGame 0x4E964 reads them; the next
        // sector load resets the clock (#39, loadgame_diff).
        self.update.new_round = b(20985)?;
        self.clock.hours = b(21024)? as i8;
        self.clock.minute = b(21025)?;
        self.clock.hour = b(21026)?;
        self.clock.minutes = i32::from_le_bytes([b(21027)?, b(21028)?, b(21029)?, b(21030)?]);
        self.clock.bar = i32::from_le_bytes([b(21031)?, b(21032)?, b(21033)?, b(21034)?]);
        copy(&mut self.notes, 21619, 30)?;
        copy(&mut self.last_quote, 21270, 6)?;
        self.quote_member = b(21276)?;
        self.hinted = b(21280)?;
        self.loss_slot = b(21277)?;
        // The animation script position of records 0..7 means nothing to the port.
        for k in 0..8 {
            self.soldiers[k * 512 + 0x90..k * 512 + 0x94].fill(0);
        }
        // Sector reset after loading.
        reset_sectors(jagged, &mut self.sectors)?;
        // Profile fixes after loading (the same three as a new game, section 11).
        let pb = |p: usize, b: usize| 90 * p + b;
        if self.profiles[pb(41, 0x3A)] == 99 {
            self.profiles[pb(41, 0x3A)] = 25;
            self.profiles[pb(41, 0x3C)] = 99;
        }
        if self.profiles[pb(24, 0x39)] == 2 {
            self.profiles[pb(24, 0x39)] = 101;
        }
        if self.profiles[pb(26, 0x39)] == 101 {
            self.profiles[pb(26, 0x39)] = 2;
        }
        // The program option "allow save" (JA1_ALLOWSAVE=1, the original's command-line word ALLOWSAVE).
        // Original 0x4E93C, called by QuickRestore 0x2C898.
        set_allow_save_mode(&mut self.base, allow_save());
        // Saved sector files, one per sector whose row byte 169 is not 0.
        // SPEC GAP: a save that ends before a listed sector file is an error (the spec does not say).
        let mut at = SAVE_FIXED_END;
        for i in 0..SECTOR_COUNT {
            if self.sectors[i * SECTOR_ROW + 169] != 0 {
                self.tmp_files[i] = Some(s(at, TMP_FILE)?.to_vec());
                at += TMP_FILE;
            }
        }
        // The item stash follows the sector files. LoadGame 0x4E964 empties the store first (ClearItemStash?
        // 0x515DC), then LoadItemStash 0x51508 reads it.
        clear_stash(&mut self.stash_counts, &mut self.stash_records);
        read_stash(&save[at..], &mut self.stash_counts, &mut self.stash_records);
        self.sector_replay.copy_from_slice(&self.sector_state[..540]);
        Ok(())
    }

    /// Section 10: the day save made from the world: the base, the fields of section 3 at their save offsets, the
    /// header day, the sector tmp files, the item stash. `load_save` reads the same offsets and counts.
    pub fn save_bytes(&self) -> Vec<u8> {
        self.save_bytes_from(self.base.clone())
    }

    /// `save_bytes` with every byte that no world field holds set to `fill`: a byte that comes out the same
    /// under two fills is a byte the port models (`loadgame_diff.py` compares those with the original).
    pub fn save_bytes_fill(&self, fill: u8) -> Vec<u8> {
        self.save_bytes_from(vec![fill; SAVE_FIXED_END])
    }

    fn save_bytes_from(&self, mut o: Vec<u8>) -> Vec<u8> {
        o.resize(SAVE_FIXED_END, 0);
        let put = |o: &mut Vec<u8>, off: usize, v: &[u8], n: usize| {
            let m = n.min(v.len());
            o[off..off + m].copy_from_slice(&v[..m]);
        };
        // Step 1: accounts first, so that the fields below that lie inside it (day, team_on_duty, cur_sector) win.
        put(&mut o, 20684, &self.accounts, 0xA8);
        // Step 2: every other field of section 3 (the ones load_save reads, in its order).
        put(&mut o, 28, &self.soldiers, 4096);
        put(&mut o, 9524, &self.sector_state, 540);
        put(&mut o, 10064, &self.sectors, 10620);
        put(&mut o, 20688, &self.day.to_le_bytes(), 4);
        put(&mut o, 20696, &self.team_on_duty.to_le_bytes(), 4);
        put(&mut o, 20788, &self.santino_word.to_le_bytes(), 4);
        o[20828] = self.guard_skill;
        o[20837] = self.cur_sector;
        put(&mut o, 20844, &self.story, 43);
        // The first 5 story bytes lie on accounts +0xA0..+0xA4. Bytes 0, 1, 3, 4 (resignations, non-payments, A.I.M.
        // link, A.I.M. page) only `accounts` updates (replay_week day 2: a stale link byte). Byte 2 (0x22A042) is a
        // story byte only: its users are DayEndVerdict, NativeGuidePrompt and NewGameInit (`./t varmap 0x22A040 5
        // --funcs`), the port's day_verdict and guide_advice write it in `story` (docs/story_map.md).
        for k in [0usize, 1, 3, 4] {
            o[20844 + k] = self.accounts[0xA0 + k];
        }
        // SPEC GAP: section 10 says "every other field of the table in section 3"; the fields that load_save reads
        // from offsets outside the table (guides_hired, guide_wait, item_told, opt_confirm_move) are written too.
        o[20895] = self.guides_hired;
        o[20896] = self.guide_wait;
        put(&mut o, 20887, &self.crate_state, 4);
        put(&mut o, 20891, &self.crate_shown, 4);
        put(&mut o, 20970, &self.supplies, 4);
        o[20969] = self.enemy_counter;
        put(&mut o, 21282, &self.item_told, 3);
        put(&mut o, 20957, &self.walter_said, 12);
        put(&mut o, 20975, &self.walter_next, 8);
        o[20974] = self.santino_spoke;
        o[20983] = self.purifier;
        o[20984] = self.difficulty;
        o[20986] = self.alarm;
        o[20988] = self.mission_step as u8;
        o[20826] = self.update.opt_reload_msg;
        put(&mut o, 21016, &self.ap_warned, 8);
        o[21035] = self.update.alone_warned;
        o[21044] = self.last_spotter;
        o[21045] = self.safe_opened;
        put(&mut o, 21078, &self.plants, 48);
        put(&mut o, 21558, &self.sector_order, 61);
        put(&mut o, 4124, &self.profiles, 5400);
        put(&mut o, 21126, &self.stat_progress, 144);
        put(&mut o, 21287, &self.mission_done, 80);
        // Switches 0 and 1 (Sound FX, Music) have no field of their own: ctlpanel::get reads them from `base`.
        for k in 0..2 {
            o[crate::ctlpanel::OPT_BASE + k] = self.base.get(crate::ctlpanel::OPT_BASE + k).copied().unwrap_or(0);
        }
        o[21543] = self.update.opt_step_text;
        o[21547] = self.update.opt_half_delay;
        o[21548] = self.update.opt_follow;
        o[21549] = self.opt_sound;
        o[21552] = self.opt_confirm_move;
        o[21553] = self.update.opt_move_fx;
        o[21545] = self.update.opt_path_marks;
        o[20869] = self.update.merc_tire as u8;
        put(&mut o, 21036, &self.update.weather_tire.to_le_bytes(), 4);
        put(&mut o, 20897, &self.sector_spoken, 60);
        o[21278] = self.update.stage;
        o[21279] = self.update.redraw_next;
        o[20989] = self.update.replay_m;
        o[21281] = self.update.first_hit;
        o[20990] = self.update.last_killer;
        o[20991] = self.clock.night_warned;
        o[21046] = self.idle_day;
        o[21047] = self.update.squad_lost;
        o[21546] = self.update.opt_ambient;
        o[20987] = self.update.enemies_warned;
        o[21048] = self.clock.hard_day;
        // The day clock and the new-round flag lie in the same block (#39, loadgame_diff: a day save holds the
        // minutes of the day that ended; the guard moves refuse at 720 until the next sector load resets them).
        o[20985] = self.update.new_round;
        o[21024] = self.clock.hours as u8;
        o[21025] = self.clock.minute;
        o[21026] = self.clock.hour;
        put(&mut o, 21027, &self.clock.minutes.to_le_bytes(), 4);
        put(&mut o, 21031, &self.clock.bar.to_le_bytes(), 4);
        put(&mut o, 21619, &self.notes, 30);
        put(&mut o, 21270, &self.last_quote, 6);
        o[21276] = self.quote_member;
        o[21280] = self.hinted;
        o[21277] = self.loss_slot;
        // Step 3: the header day (u16); bytes 2..27 stay.
        o[0..2].copy_from_slice(&(self.day as u16).to_le_bytes());
        // Step 4: +0x90..+0x93 of the 8 soldier records: the live pointer, or the base's bytes before any sector.
        for k in 0..8 {
            let a = 28 + k * 512 + 0x90;
            // A sector ran (replay_week day 2): the original saves its live pointer, the port holds the table offset.
            let off = u32::from_le_bytes(self.soldiers[k * 512 + 0x90..k * 512 + 0x94].try_into().unwrap());
            if off != 0 {
                o[a..a + 4].copy_from_slice(&(crate::quicksave::PTR_BASE + off).to_le_bytes());
            } else {
                o[a..a + 4].copy_from_slice(&self.base[a..a + 4]);
            }
        }
        // The tail: the tmp files of the sectors whose row byte 169 is not 0, then the stash.
        for i in 0..SECTOR_COUNT {
            if self.sectors[i * SECTOR_ROW + 169] != 0 {
                let at = o.len();
                // SPEC GAP: a set byte 169 with no tmp file writes 18,080 zero bytes (section 10).
                o.resize(at + TMP_FILE, 0);
                if let Some(t) = &self.tmp_files[i] {
                    let m = t.len().min(TMP_FILE);
                    o[at..at + m].copy_from_slice(&t[..m]);
                }
            }
        }
        o.extend_from_slice(&write_stash(&self.stash_counts, &self.stash_records));
        o
    }

    /// Loads JA.EXE from `exe_path`, DAT/JAGGED.DAT from the CD image and the day save from `save_path`.
    pub fn load(iso_path: &str, exe_path: &str, save_path: &str) -> io::Result<World> {
        let exe = std::fs::read(exe_path)?;
        let jagged = Iso::open(iso_path)?.read("DAT/JAGGED.DAT")?;
        let save = std::fs::read(save_path)?;
        World::from_parts(exe_path, &exe, &jagged, &save)
    }

    /// `load` from the bytes of JA.EXE, DAT/JAGGED.DAT and the day save (the diff tests feed the save in
    /// memory, and the fuzz cases need no file).
    pub fn from_parts(exe_path: &str, exe: &[u8], jagged: &[u8], save: &[u8]) -> io::Result<World> {
        let mut w = World::from_files(exe, jagged, save)?;
        // Strategic fields: profiles 60..63 (the natives) = bytes 5400..5759 of EDT\NEWPROF.EDT beside JA.EXE.
        // SPEC GAP: when the file is missing (or shorter) they stay 0. The file name is tried as given and in
        // upper / lower case (the GOG install keeps the DOS names).
        let dir = std::path::Path::new(exe_path).parent().unwrap_or(std::path::Path::new("."));
        let names = ["EDT/NEWPROF.EDT", "edt/newprof.edt", "EDT/newprof.edt", "edt/NEWPROF.EDT"];
        if let Some(edt) = names.iter().find_map(|n| std::fs::read(dir.join(n)).ok()) {
            if edt.len() >= 5760 {
                w.profiles[5400..5760].copy_from_slice(&edt[5400..5760]);
            }
        }
        Ok(w)
    }

    /// Section 11: the new-game save for difficulty `d` and quick-save mode `q` (JA.EXE, JAGGED.DAT and
    /// EDT\NEWPROF.EDT bytes; a file that is too short gives zeros for the missing bytes).
    pub fn new_game_save(exe: &[u8], jagged: &[u8], edt: &[u8], d: u8, q: u8) -> Vec<u8> {
        let mut o = vec![0u8; SAVE_FIXED_END];
        let copy = |o: &mut Vec<u8>, at: usize, src: &[u8], from: usize, n: usize| {
            for k in 0..n {
                o[at + k] = src.get(from + k).copied().unwrap_or(0);
            }
        };
        // Profiles 0..59, then the three fixes.
        copy(&mut o, 4124, edt, 0, 5400);
        let pb = |p: usize, b: usize| 4124 + 90 * p + b;
        if o[pb(41, 0x3A)] == 99 {
            o[pb(41, 0x3A)] = 25;
            o[pb(41, 0x3C)] = 99;
        }
        if o[pb(24, 0x39)] == 2 {
            o[pb(24, 0x39)] = 101;
        }
        if o[pb(26, 0x39)] == 101 {
            o[pb(26, 0x39)] = 2;
        }
        for i in 0..SECTOR_COUNT {
            copy(&mut o, 9524 + 9 * i + 6, jagged, 64 + 2 * i, 1);
            copy(&mut o, 9524 + 9 * i, jagged, 65 + 2 * i, 1);
            let r = 10064 + SECTOR_ROW * i;
            copy(&mut o, r, jagged, 184 + 48 * i, 48);
            copy(&mut o, r + 48, jagged, 3064 + 40 * i, 40);
            copy(&mut o, r + 88, jagged, 5464 + 40 * i, 40);
            copy(&mut o, r + 128, jagged, 7864 + 2 * i, 2);
        }
        // The game state block.
        let b = 20684;
        let s16 = |off: i64| -> i32 {
            let lo = if off >= 0 { exe.get(off as usize).copied().unwrap_or(0) } else { 0 };
            let hi = if off + 1 >= 0 { exe.get(off as usize + 1).copied().unwrap_or(0) } else { 0 };
            i16::from_le_bytes([lo, hi]) as i32
        };
        let ds = d as i8 as i64;
        o[b + 0x10..b + 0x14].copy_from_slice(&500i32.to_le_bytes());
        o[b + 0x24..b + 0x28].copy_from_slice(&s16(904392 + 2 * ds).to_le_bytes());
        o[b + 0x54..b + 0x58].copy_from_slice(&35i32.to_le_bytes());
        o[b + 0x68..b + 0x6C].copy_from_slice(&s16(904422 + 2 * ds).to_le_bytes());
        o[b + 0x80..b + 0x84].copy_from_slice(&59i32.to_le_bytes());
        o[b + 0x99] = 0xFF;
        for k in [0xA3, 0xB2, 0xDD, 0xDF, 0xE5, 0xE6, 0xFA, 0xFC, 0x110, 0x25A] {
            o[b + k] = 1;
        }
        for k in [0xB7, 0xCF, 0xD0, 0xD1, 0xD2, 0x24A] {
            o[b + k] = 0xFF;
        }
        for k in 0x111..=0x11C {
            o[b + k] = 99;
        }
        o[b + 0x11D] = 60;
        o[b + 0x123..b + 0x12B].copy_from_slice(&[90, 70, 40, 20, 65, 60, 10, 60]);
        o[b + 0x12C] = d;
        copy(&mut o, b + 0x18A, exe, 904264, 48);
        o[b + 0x259] = q;
        // Options, sector order, notes. The defaults are program start's (0x377B0: 0x1163B8).
        o[21538..21558].copy_from_slice(&[1, 1, 0, 0, 1, 0, 1, 1, 1, 0, 1, 1, 1, 0, 1, 1, 0, 0, 0, 0]);
        copy(&mut o, 21558, exe, 892564, 61);
        copy(&mut o, 21619, exe, 892790, 30);
        // The tail: no sector files; the item store.
        let mut types = vec![0x36u8; 30];
        types.extend_from_slice(&[0x32, 0x32, 0x33, 0x2A, 0x49, 0x41, 0x41]);
        types.sort();
        let mut counts = [0u8; 136];
        let mut recs = Vec::new();
        for t in types {
            let it = |k: usize| exe.get(900764 + 12 * t as usize + k).copied().unwrap_or(0);
            let c = if it(6) == 1 { it(5) } else { 1 };
            counts[t as usize] = counts[t as usize].wrapping_add(1);
            recs.extend_from_slice(&[t, c, 100]);
        }
        o.extend_from_slice(&counts);
        o.extend_from_slice(&recs);
        o
    }

    /// Section 11: a new game (the new-game save loaded as in sections 1..3, then `ja1_new_game`).
    pub fn new_game(iso_path: &str, exe_path: &str, d: u8, q: u8) -> io::Result<World> {
        Self::new_game_after(iso_path, exe_path, d, q, None)
    }

    /// Section 11 for a new game after another game of the same run (`old`): the original keeps the save name
    /// (save bytes 2..27), the options, `sector_order` and `notes` (save bytes 21538..21648) of the game before.
    pub fn new_game_after(iso_path: &str, exe_path: &str, d: u8, q: u8, old: Option<&World>) -> io::Result<World> {
        let exe = std::fs::read(exe_path)?;
        let jagged = Iso::open(iso_path)?.read("DAT/JAGGED.DAT")?;
        let edt = Self::newprof(exe_path).unwrap_or_default();
        let mut save = Self::new_game_save(&exe, &jagged, &edt, d, q);
        if let Some(old) = old {
            let ob = old.save_bytes();
            save[2..28].copy_from_slice(&ob[2..28]);
            save[21538..21649].copy_from_slice(&ob[21538..21649]);
        }
        let mut w = World::from_files(&exe, &jagged, &save)?;
        if edt.len() >= 5760 {
            w.profiles[5400..5760].copy_from_slice(&edt[5400..5760]);
        }
        crate::mapui::new_game_finish(&mut w);
        // world_load.md section 11: InitSound 0x5CED4 runs once per program, so its driver bytes stay for each later
        // new game (a SOUND.CFG with no driver keeps the sound and music switches off).
        if let Some(old) = old {
            w.sound_setup = old.sound_setup;
        }
        Ok(w)
    }

    /// Section 12: the world at the program start (Continue Game before any game): a new game with d = 0, q = 0,
    /// then the A.I.M. link (`accounts` +0xA3) = 0.
    pub fn program_start(iso_path: &str, exe_path: &str) -> io::Result<World> {
        let mut w = Self::new_game(iso_path, exe_path, 0, 0)?;
        w.accounts[0xA3] = 0;
        w.sound_cfg_options(exe_path);
        Ok(w)
    }

    /// Section 11 (the sound setup at the program start): option 5 (Subtitles) = byte 80 of SOUND.CFG beside
    /// JA.EXE (the original stops when the file is missing; the port then keeps the option).
    pub fn sound_cfg_options(&mut self, exe_path: &str) {
        let dir = std::path::Path::new(exe_path).parent().unwrap_or(std::path::Path::new("."));
        let cfg = ["SOUND.CFG", "sound.cfg"].iter().find_map(|n| std::fs::read(dir.join(n)).ok());
        if let Some(c) = cfg {
            self.apply_sound_cfg(&c);
        }
    }

    /// The Subtitles option before any game (the film shows of the title and the first intro): the switch default
    /// (0, g_options 0x1163B8 + 5) unless SOUND.CFG beside JA.EXE gives byte 80, as `sound_cfg_options`.
    pub fn program_start_subtitles(exe_path: &str) -> bool {
        let dir = std::path::Path::new(exe_path).parent().unwrap_or(std::path::Path::new("."));
        let cfg = ["SOUND.CFG", "sound.cfg"].iter().find_map(|n| std::fs::read(dir.join(n)).ok());
        cfg.and_then(|c| crate::startup::SoundSetup::from_cfg(&c).subtitles).is_some_and(|b| b != 0)
    }

    /// The options and driver bytes of InitSound 0x5CED4 + InitAil 0x5C1DC.
    pub fn apply_sound_cfg(&mut self, cfg: &[u8]) {
        let s = crate::startup::SoundSetup::from_cfg(cfg);
        self.sound_setup = s;
        if let Some(b) = s.subtitles {
            crate::ctlpanel::set(self, 5, b);
        }
        if !s.sound_driver {
            crate::ctlpanel::set(self, 0, 0);
        }
        if !s.music_driver {
            crate::ctlpanel::set(self, 1, 0);
        }
    }

    /// EDT\NEWPROF.EDT beside JA.EXE (the name as given and in upper / lower case: the GOG install keeps the DOS names).
    fn newprof(exe_path: &str) -> Option<Vec<u8>> {
        let dir = std::path::Path::new(exe_path).parent().unwrap_or(std::path::Path::new("."));
        let names = ["EDT/NEWPROF.EDT", "edt/newprof.edt", "EDT/newprof.edt", "edt/NEWPROF.EDT"];
        names.iter().find_map(|n| std::fs::read(dir.join(n)).ok())
    }

    /// Section 6: every buffer raw at its full size, then every scalar as a 4-byte little-endian signed value.
    pub fn image(&self) -> Vec<u8> {
        // SPEC GAP: section 6 does not name the move order / one frame fields; they are appended in port_api.md struct
        // order (fx_anims, view_rays, obj_block after splash; the 8 move order / frame scalars after opt_step_text, the 4
        // merc view scalars after tick_step), as "every buffer field ... every scalar".
        let bufs: [&[u8]; 64] = [
            &self.soldiers,
            &self.tiles,
            &self.item_types,
            &self.anim_table,
            &self.sector_state,
            &self.mission_done,
            &self.knowledge,
            &self.fresher,
            &self.last_seen_m,
            &self.last_seen_e,
            &self.calls,
            &self.paths,
            &self.dir_steps,
            &self.t0_class,
            &self.t1_class,
            &self.remember_m,
            &self.remember_e,
            // world_load.md section 6 keeps `story` at 35 bytes in the image (the buffer holds 43).
            &self.story[..35],
            &self.plants,
            &self.crates,
            &self.weapons,
            &self.absorb,
            &self.anim_rows,
            &self.anim_scripts,
            &self.anim_free,
            &self.anim_sounds,
            &self.anim_steps_tile,
            &self.anim_turn_while,
            &self.supplies,
            &self.equip_skill,
            &self.attach,
            &self.ground_items,
            &self.notes,
            &self.move_codes,
            &self.held,
            &self.blast_list,
            &self.listed,
            &self.t2_class,
            &self.enemy_watch,
            &self.text_buf,
            &self.item_buf,
            &self.crate_state,
            &self.crate_shown,
            &self.fixed_items,
            &self.sectors,
            &self.ap_warned,
            &self.splash,
            &self.fx_anims,
            &self.itq,
            &self.bled,
            &self.timed_fx,
            &self.sector_spoken,
            &self.view_rays,
            &self.obj_block,
            &self.silencer,
            &self.shot_sound,
            &self.impact_noise,
            &self.shot_range,
            &self.posture,
            &self.window,
            &self.attack_sound,
            &self.diff_mod,
            &self.dmg_text,
            &self.marks,
        ];
        // Unsigned types zero-extend, signed ones sign-extend (`as i32` does both).
        let u = &self.update;
        let scalars: [i32; SCALARS] = [
            self.cur_sector as i32,
            self.alarm as i32,
            self.combat as i32,
            self.difficulty as i32,
            self.mission_step as i32,
            self.mission_slot as i32,
            self.mission_target as i32,
            self.mission_spot as i32,
            self.turn_based,
            self.busy,
            self.active_slot as i32,
            self.santino_spoke as i32,
            self.santino_timer as i32,
            self.show_ai as i32,
            self.santino_word as i32,
            self.last_spotter as i32,
            self.day,
            self.selected as i32,
            self.path_preview,
            self.opt_sound as i32,
            self.opt_onscreen as i32,
            self.view_x,
            self.view_y,
            self.view_w,
            self.view_h,
            self.anim_signal,
            self.effect_tile as i32,
            self.guard_skill as i32,
            self.bomb_count,
            self.team_on_duty,
            self.purifier as i32,
            self.safe_opened as i32,
            u.screen_drawn as i32,
            u.interrupt_go as i32,
            u.turn_moves as i32,
            u.kept_merc as i32,
            u.next_selected as i32,
            u.quote_slot as i32,
            u.quote_tile as i32,
            u.quote_focus as i32,
            u.quote_delay as i32,
            u.quote_event as i32,
            u.opt_half_delay as i32,
            u.opt_follow as i32,
            u.opt_reload_msg as i32,
            u.alone_warned as i32,
            u.follow_all as i32,
            u.stop_timer,
            u.interrupted as i32,
            u.walk_ap,
            u.spotted,
            u.opt_step_text as i32,
            u.key_shift as i32,
            u.key_alt as i32,
            u.key_ctrl as i32,
            u.opt_move_fx as i32,
            u.seen_enemies as i32,
            u.in_flight as i32,
            u.merc_shown as i32,
            u.interrupt_turn as i32,
            u.tick_step,
            u.story_finder as i32,
            u.spot_stop as i32,
            u.itq_len as i32,
            u.new_round as i32,
            u.turn_timer as i32,
            u.turn_timer2 as i32,
            u.shown_slot,
            u.view_tile,
            u.focus_slot as i32,
            u.turn_shown as i32,
            u.merc_to_show as i32,
            u.view_hold as i32,
            u.weather_tire,
            u.merc_tire as i32,
            u.round_lock as i32,
            u.round_lock2 as i32,
            u.handed_over as i32,
            u.turn_flag as i32,
            u.interrupter as i32,
            u.speaker as i32,
            u.speech_wait as i32,
            u.stage as i32,
            u.redraw_next as i32,
            u.redraw_pending,
            u.turn_clock,
            u.show_timer,
            u.busy_timer,
            u.replay_m as i32,
            u.first_hit as i32,
            u.fx_id as i32,
            u.marked_slot as i32,
            u.knockdown as i32,
            u.focus_last as i32,
            u.hit_scale as i32,
            u.dead_mercs,
            u.dead_allies,
            u.dead_npc,
            u.enemy_deaths,
            u.squad_lost as i32,
            u.last_killer as i32,
            u.fast_timer,
            u.marks_shown,
            u.marks_n,
            u.opt_path_marks as i32,
            u.sees_slot as i32,
            u.preview_slot as i32,
            u.ripple_timer,
            u.ripple_tile,
            u.ambient_t1,
            u.ambient_t2,
            u.ambient_count as i32,
            u.opt_ambient as i32,
            u.time_compress as i32,
            u.enemies_warned as i32,
            u.leave_reason as i32,
            u.fuse_beep_timer,
            u.fuse_beep as i32,
            u.turn_tick,
            u.quit_tactical as i32,
        ];
        let mut out = Vec::with_capacity(IMAGE_SIZE);
        for b in bufs {
            out.extend_from_slice(b);
        }
        for v in scalars {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out
    }
}

/// Count of the section 6 scalars.
pub const SCALARS: usize = 120;

/// Byte size of the section 6 world image: the buffer sizes listed there (plus fx_anims, view_rays, obj_block) plus 64 scalars x 4.
pub const IMAGE_SIZE: usize = 24576 + 25088 + 1632 + 2048 + 720 + 80 + 64 + 2304 + 256 + 640 + 8 + 3840 + 514 + 256
    + 256 + 128 + 320 + 35 + 48 + 16 + 1536 + 576 + 2048 + 5440 + 256 + 512 + 256 + 256 + 4 + 6 + 328 + 2400 + 30
    + 6272 + 12 + 60 + 32 + 256 + 48 + 128 + 12 + 4 + 4 + 88 + 10620 + 8 + 60 + FX_ANIMS + ITQ + BLED + TIMED_FX + SECTOR_SPOKEN + 306
    + 685
    + SHOOT_TABLES
    + AIM_TABLES
    + DMG_TEXT
    + MARKS
    + SCALARS * 4;

/// Bytes of the select.md buffer marks (the last buffer of the image, after dmg_text).
pub const MARKS: usize = 256;

/// Bytes of the shooting.md batch 2 buffer dmg_text (in the image after shot_range).
pub const DMG_TEXT: usize = 240;

/// Bytes of the shooting.md tables silencer, shot_sound, impact_noise, shot_range (in the image after obj_block).
pub const SHOOT_TABLES: usize = 17 + 34 + 33 + 9;

/// Bytes of the shooting.md batch 5 tables posture, window, attack_sound, diff_mod (in the image after shot_range).
pub const AIM_TABLES: usize = 256 + 2056 + 10 + 6;


/// ResetSectorsFromDat 0x3712C (LoadGame, QuickRestore): for every sector i, record bytes 0..47 = JAGGED.DAT
/// bytes 184 + 48 i, record bytes 48..87 = JAGGED.DAT bytes 3064 + 40 i (world_load.md section 3 "Sector reset
/// after loading"). A used lock or trigger tile in those parts comes back on every load - the original's behavior.
pub fn reset_sectors(jagged: &[u8], sectors: &mut [u8]) -> io::Result<()> {
    for i in 0..SECTOR_COUNT {
        let row = &mut sectors[i * SECTOR_ROW..(i + 1) * SECTOR_ROW];
        row[..48].copy_from_slice(slice(jagged, 184 + 48 * i, 48, "JAGGED.DAT")?);
        row[48..88].copy_from_slice(slice(jagged, 3064 + 40 * i, 40, "JAGGED.DAT")?);
    }
    Ok(())
}

/// LoadItemStash 0x51508 on a save's stash tail: the first 136 bytes are the counts, then `counts[i]` 3-byte
/// records (type, quantity, condition) for each type i in turn, each put into the store in sorted order (type
/// ascending, condition descending as a signed byte, after equals; a canteen's condition is 100). The caller
/// empties the store first (ClearItemStash? 0x515DC). Returns the bytes read.
///
/// SPEC GAP: a record of a stackable type with a quantity other than 1, or a type-0 record, is stored as written;
/// the original's AddStashItem? splits the first into singles and refuses the second. SaveItemStash never writes
/// either, and a save that ends early gives fewer records.
pub fn read_stash(tail: &[u8], counts: &mut [u8], records: &mut Vec<[u8; 3]>) -> usize {
    let n = tail.len().min(136);
    counts.fill(0);
    counts[..n].copy_from_slice(&tail[..n]);
    let mut store = crate::assign_items::ItemStore {
        view: 1, records: Vec::new(), cur: crate::assign_items::NONE, top: 0, shown: [0; 4], v: 0x38,
    };
    let mut r = 136;
    for i in 0..136 {
        for _ in 0..counts[i] {
            if let Some(b) = tail.get(r..r + 3) {
                store.put(b[0], b[1], b[2]);
            }
            r += 3;
        }
    }
    *records = store.records;
    r
}

/// SaveItemStash 0x51590: the save's stash tail = the 136 counts, then the store's records in list order.
pub fn write_stash(counts: &[u8], records: &[[u8; 3]]) -> Vec<u8> {
    let mut o = Vec::with_capacity(136 + 3 * records.len());
    o.extend_from_slice(&counts[..136.min(counts.len())]);
    for r in records {
        o.extend_from_slice(r);
    }
    o
}

/// ClearItemStash? 0x515DC (NewGameInit, LoadGame, QuickRestore): for each of the 136 item types, take a record
/// of that type out of the store (TakeStashItem? 0x51A24, which lowers the count; with Shift held a whole stack)
/// while the count is not 0, at most 1000 times per type. The counts always match the records, so the store ends
/// empty with every count 0. The original also leaves the last record in its item buffer 0x222B68 (the port has
/// none). SPEC GAP: a count without records (never in a game) makes the original walk past the list end.
pub fn clear_stash(counts: &mut [u8], records: &mut Vec<[u8; 3]>) {
    counts.fill(0);
    records.clear();
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Synthetic files: every byte = a function of its offset, so each copy can be checked by position.
    fn pat(len: usize, salt: u32) -> Vec<u8> {
        (0..len).map(|i| ((i as u32).wrapping_mul(2654435761).wrapping_add(salt) >> 13) as u8).collect()
    }

    pub(crate) fn files(tmp_rows: &[usize]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let exe = pat(907_000, 1);
        let jag = pat(23_000, 2);
        let mut save = pat(SAVE_FIXED_END, 3);
        // byte 169 of each sectors row (save offset 10064 + 177 i + 169): set only for tmp_rows
        for i in 0..SECTOR_COUNT {
            save[10064 + SECTOR_ROW * i + 169] = tmp_rows.contains(&i) as u8;
        }
        for (n, _) in tmp_rows.iter().enumerate() {
            save.extend(std::iter::repeat(n as u8 + 1).take(TMP_FILE));
        }
        (exe, jag, save)
    }

    #[test]
    fn image_layout_and_size() {
        let (exe, jag, save) = files(&[]);
        let w = World::from_files(&exe, &jag, &save).unwrap();
        let img = w.image();
        assert_eq!(img.len(), IMAGE_SIZE);
        // soldiers first: records 0..7 from the save with +0x90..+0x93 cleared, records 8..47 = 0
        assert_eq!(&img[0..0x90], &save[28..28 + 0x90]);
        assert_eq!(&img[0x90..0x94], &[0, 0, 0, 0]);
        assert_eq!(img[7 * 512 + 0x94], save[28 + 7 * 512 + 0x94]);
        assert!(img[4096..24576].iter().all(|&b| b == 0));
        // item_types right after tiles
        assert_eq!(&img[24576 + 25088..24576 + 25088 + 1632], &exe[900764..900764 + 1632]);
        // the scalar tail: view_w / view_h are scalars 23 and 24
        let tail = &img[IMAGE_SIZE - SCALARS * 4..];
        let sc = |k: usize| i32::from_le_bytes(tail[4 * k..4 * k + 4].try_into().unwrap());
        assert_eq!((sc(23), sc(24)), (224, 168));
        assert_eq!(sc(0), save[20837] as i32);
        assert_eq!(sc(4), save[20988] as i8 as i32);
        assert_eq!(sc(16), i32::from_le_bytes(save[20688..20692].try_into().unwrap()));
        assert_eq!(sc(26), i16::from_le_bytes(exe[892656..892658].try_into().unwrap()) as i32);
        assert_eq!(sc(31), save[21045] as i32); // safe_opened, the last scalar before the soldier update ones
        assert_eq!(sc(37), exe[892560] as i32); // quote_slot
        assert_eq!(sc(38), u16::from_le_bytes(exe[892562..892564].try_into().unwrap()) as i32); // quote_tile
        assert_eq!(sc(44), save[20826] as i32); // opt_reload_msg
        assert_eq!(sc(47), i32::from_le_bytes(exe[904812..904816].try_into().unwrap())); // stop_timer
        assert_eq!(sc(50), 0); // spotted
        assert_eq!(sc(51), save[21543] as i32); // opt_step_text
        assert_eq!(sc(54), 0); // key_ctrl (click.md)
        assert_eq!(sc(55), save[21553] as i32); // opt_move_fx
        assert_eq!(sc(60), i32::from_le_bytes(exe[892932..892936].try_into().unwrap())); // tick_step
        assert_eq!((sc(61), sc(62)), (exe[892551] as i32, exe[892680] as i32)); // story_finder, spot_stop
        assert_eq!((sc(71), sc(69)), (exe[892523] as i32, exe[892662] as i32)); // merc_to_show, focus_slot
        let rays_end = IMAGE_SIZE - SCALARS * 4 - MARKS - DMG_TEXT - AIM_TABLES - SHOOT_TABLES - 685;
        assert_eq!(&img[rays_end - 306..rays_end], &exe[894456..894456 + 306]);
        assert_eq!(&img[rays_end..rays_end + 685], &exe[902968..902968 + 685]);
        assert_eq!(sc(6), u16::from_le_bytes(exe[892494..892496].try_into().unwrap()) as i32);
        // sectors, ap_warned, splash, fx_anims, itq, bled, timed_fx, sector_spoken, view_rays, obj_block, then the scalars
        let spoken_end = IMAGE_SIZE - SCALARS * 4 - MARKS - DMG_TEXT - AIM_TABLES - SHOOT_TABLES - 685 - 306;
        assert_eq!(&img[spoken_end - SECTOR_SPOKEN..spoken_end], &save[20897..20897 + 60]);
        // New turn scalars: round_lock (74) .. show_timer (85)
        assert_eq!((sc(75), sc(82), sc(83)), (exe[892548] as i32, save[21278] as i32, save[21279] as i32));
        assert_eq!(sc(85), 0); // turn_clock
        assert_eq!(sc(86), i32::from_le_bytes(exe[896504..896508].try_into().unwrap())); // show_timer
        // shooting.md scalars: busy_timer (86), replay_m, first_hit, fx_id (89)
        assert_eq!(sc(87), i32::from_le_bytes(exe[892504..892508].try_into().unwrap()));
        assert_eq!((sc(88), sc(89), sc(90)), (save[20989] as i32, save[21281] as i32, 0));
        // shooting.md tables after obj_block
        let tab = IMAGE_SIZE - SCALARS * 4 - MARKS - DMG_TEXT - AIM_TABLES - SHOOT_TABLES;
        assert_eq!(&img[tab..tab + 17], &exe[892836..892836 + 17]);
        assert_eq!(&img[tab + 17..tab + 51], &exe[900736..900736 + 34]);
        assert_eq!(&img[tab + 51..tab + 84], &exe[892869..892869 + 33]);
        assert_eq!(&img[tab + 84..tab + 93], &exe[906223..906223 + 9]);
        // shooting.md batch 2: dmg_text (240 zero bytes) after shot_range; marked_slot, knockdown, focus_last, hit_scale (90..93)
        // shooting.md batch 5: posture, window, attack_sound, diff_mod after shot_range
        assert_eq!(&img[tab + 93..tab + 349], &exe[893105..893105 + 256]);
        assert_eq!(&img[tab + 349..tab + 2405], &exe[903212..903212 + 2056]);
        assert_eq!(&img[tab + 2405..tab + 2415], &exe[892682..892682 + 10]);
        assert_eq!(&img[tab + 2415..tab + 2421], &exe[904434..904434 + 6]);
        let dmg = tab + 93 + AIM_TABLES;
        assert!(img[dmg..dmg + DMG_TEXT].iter().all(|&b| b == 0));
        assert_eq!((sc(91), sc(92), sc(93), sc(94)), (exe[904880] as i32, exe[892694] as i32, exe[892663] as i32, exe[892521] as i32));
        // shooting.md batch 4: dead_mercs, dead_allies, dead_npc, enemy_deaths (0), squad_lost, last_killer (save) (94..99)
        assert_eq!((sc(95), sc(96), sc(97), sc(98)), (0, 0, 0, 0));
        assert_eq!((sc(99), sc(100)), (save[21047] as i32, save[20990] as i32));
        assert_eq!(sc(101), 0); // fast_timer (turns.md)
        // select.md: marks (256 zero bytes) the last buffer; marks_shown, marks_n (0), opt_path_marks (save),
        // sees_slot, preview_slot (EXE) the last scalars
        let mk = IMAGE_SIZE - SCALARS * 4 - MARKS;
        assert!(img[mk..mk + MARKS].iter().all(|&b| b == 0));
        assert_eq!(&img[mk - DMG_TEXT..mk], &w.dmg_text[..]);
        assert_eq!((sc(102), sc(103)), (0, 0));
        assert_eq!((sc(104), sc(105), sc(106)), (save[21545] as i32, exe[892522] as i32, exe[892668] as i32));
        let end = IMAGE_SIZE - SCALARS * 4 - MARKS - DMG_TEXT - AIM_TABLES - SHOOT_TABLES - 685 - 306 - SECTOR_SPOKEN - TIMED_FX - BLED - ITQ - FX_ANIMS;
        assert_eq!(&img[end - 60..end], &exe[892698..892698 + 60]);
        assert_eq!(&img[end - 68..end - 60], &save[21016..21024]);
        let secs = &img[end - 68 - 10620..end - 68];
        assert_eq!(secs, &w.sectors[..]);
    }

    #[test]
    fn partial_tables_and_sector_reset() {
        let (exe, jag, save) = files(&[]);
        let w = World::from_files(&exe, &jag, &save).unwrap();
        assert_eq!(&w.t0_class[..157], &jag[22128..22128 + 157]);
        assert!(w.t0_class[157..].iter().all(|&b| b == 0));
        assert!(w.t2_class[155..].iter().all(|&b| b == 0));
        assert_eq!(&w.sector_state[..540], &save[9524..9524 + 540]);
        assert!(w.sector_state[540..].iter().all(|&b| b == 0));
        for i in [0usize, 7, 59] {
            let row = &w.sectors[i * 177..(i + 1) * 177];
            assert_eq!(&row[..48], &jag[184 + 48 * i..232 + 48 * i]);
            assert_eq!(&row[48..88], &jag[3064 + 40 * i..3104 + 40 * i]);
            assert_eq!(&row[88..], &save[10064 + 177 * i + 88..10064 + 177 * (i + 1)]);
        }
        assert_eq!(w.entry_tiles[3], i32::from_le_bytes(exe[904576..904580].try_into().unwrap()));
        // move order / one frame / tick start values (sections 2 and 3)
        assert_eq!(w.update.opt_move_fx, save[21553]);
        assert_eq!(w.update.in_flight, exe[892520]);
        assert_eq!(w.update.merc_shown, exe[892524]);
        assert_eq!(w.update.interrupt_turn, exe[892544]);
        assert_eq!(w.update.tick_step, i32::from_le_bytes(exe[892932..892936].try_into().unwrap()));
        assert_eq!((w.update.key_shift, w.update.key_alt, w.update.seen_enemies), (0, 0, 0));
        assert!(w.fx_anims.len() == FX_ANIMS && w.fx_anims.iter().all(|&b| b == 0));
    }

    /// Story byte 2 shares its address with accounts +0xA2; the story view wins there, accounts wins at 0, 1, 3, 4.
    #[test]
    fn story_byte_2_survives_the_save() {
        let (exe, jag, save) = files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.story[2] = w.story[2].wrapping_add(1);
        w.accounts[0xA3] = w.accounts[0xA3].wrapping_add(1);
        let o = w.save_bytes();
        assert_eq!(o[20844 + 2], w.story[2]);
        assert_eq!(o[20844 + 3], w.accounts[0xA3]);
        let v = World::from_files(&exe, &jag, &o).unwrap();
        assert_eq!((v.story[2], v.accounts[0xA3]), (w.story[2], w.accounts[0xA3]));
    }

    #[test]
    fn save_copy_equals_input() {
        let (exe, jag, mut save) = files(&[3, 30, 59]);
        save[0] = 0x34; // header day: the low 16 bits of `day` (bytes 20688..)
        save[1] = 0x12;
        save[20688..20692].copy_from_slice(&0x1234i32.to_le_bytes());
        // the stash: one record of type 5; the counts are 136 bytes, so type 5 has count 1
        let at = save.len();
        save.extend((0..136).map(|i| (i == 5) as u8));
        save.extend([5, 7, 9]);
        let w = World::from_files(&exe, &jag, &save).unwrap();
        let out = w.save_bytes();
        assert_eq!(out.len(), save.len());
        assert_eq!(&out[at..], &save[at..]);
        let mut diff = 0;
        for i in 0..save.len() {
            if out[i] != save[i] {
                // only the sector reset: record bytes 0..87
                assert!(i >= 10064 && i < 10064 + 60 * SECTOR_ROW && (i - 10064) % SECTOR_ROW < 88, "byte {}", i);
                diff += 1;
            }
        }
        assert!(diff > 0);
        // a sector with row byte 169 set and no tmp file: zeros
        let mut w2 = w.clone();
        w2.tmp_files[30] = None;
        let o2 = w2.save_bytes();
        assert!(o2[SAVE_FIXED_END + TMP_FILE..SAVE_FIXED_END + 2 * TMP_FILE].iter().all(|&b| b == 0));
    }

    #[test]
    fn tmp_files_in_sector_order() {
        let (exe, jag, save) = files(&[4, 30, 59]);
        let w = World::from_files(&exe, &jag, &save).unwrap();
        let have: Vec<usize> = (0..60).filter(|&i| w.tmp_files[i].is_some()).collect();
        assert_eq!(have, vec![4, 30, 59]);
        assert_eq!(w.tmp_files[30].as_ref().unwrap()[0], 2);
        assert_eq!(w.tmp_files[59].as_ref().unwrap().len(), TMP_FILE);
        // A save cut short of a listed file is an error.
        assert!(World::from_files(&exe, &jag, &save[..save.len() - 1]).is_err());
    }
}

/// World_load.md section 3 "The program option allow save": the notice the original prints at program start.
pub const ALLOW_SAVE_NOTICE: &str = "\n\nQUICK SAVES TO BE ALLOWED DURING COMBAT: Each game restored (including\n\
quick saves) during this session will be \"patched\" to allow quick saves\n\
during combat providing that the game was started with a version of Jagged\n\
Alliance prior to v1.14.\n\n\
Starting with v1.14, beginning a new game allows you to decide which quick-save\n\
mode you prefer (immediately following the difficulty mode choice). Like\n\
the difficulty level, once decided it cannot be changed.\n\
Press any key to start Jagged Alliance...\n";

/// World_load.md section 3: the program option "allow save" is on when JA1_ALLOWSAVE is 1.
pub fn allow_save() -> bool {
    ALLOW_SAVE_WORD.load(std::sync::atomic::Ordering::Relaxed) || std::env::var("JA1_ALLOWSAVE").map(|v| v == "1").unwrap_or(false)
}

/// The word ALLOWSAVE on the command line (0x955C6 and 0x955C7).
static ALLOW_SAVE_WORD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What one word of the command line does: ParseCmdArg 0x1008C.
#[derive(Debug, PartialEq, Eq)]
pub enum CmdWord {
    /// NOCRITICAL (0x229E14 = 0): no DOS critical error handler. There is none in the port, only the line stays.
    NoCritical,
    /// RESET (0x229E10 = 1): a keyboard reset at the program end for DOS. Only the line stays.
    Reset,
    AllowSave,
    /// Any other word: the line, then exit 0. The text has the word in upper case.
    Unknown(Vec<u8>),
}

/// ParseCmdArg 0x1008C: the word in upper case (bytes up to the first 0), then the three compares.
pub fn cmd_word(word: &[u8]) -> CmdWord {
    let up: Vec<u8> = word.iter().take_while(|&&c| c != 0).map(|c| c.to_ascii_uppercase()).collect();
    match &up[..] {
        b"NOCRITICAL" => CmdWord::NoCritical,
        b"RESET" => CmdWord::Reset,
        b"ALLOWSAVE" => CmdWord::AllowSave,
        _ => CmdWord::Unknown(up),
    }
}

/// The line that a word prints (empty for ALLOWSAVE), from the format strings at 0x90010, 0x90038, 0x9005C.
pub fn cmd_word_line(w: &CmdWord) -> Vec<u8> {
    match w {
        CmdWord::NoCritical => b"No Critical error handling...\n".to_vec(),
        CmdWord::Reset => b"Resetting keyboard...\n".to_vec(),
        CmdWord::AllowSave => Vec::new(),
        CmdWord::Unknown(up) => [&b"Unknown argument: "[..], up, b"\n"].concat(),
    }
}

/// main_ 0x10150: only the first 3 words of the command line go to ParseCmdArg 0x1008C.
pub fn first_word(count: &mut u32) -> bool {
    *count += 1;
    *count <= 3
}

/// The text of the words of main_ 0x10150 before its call chain: the line of each word it reads, up to the end of the
/// program at an unknown word (true), then the notice when one of them is ALLOWSAVE.
pub fn main_words_text(words: &[&[u8]]) -> (Vec<u8>, bool) {
    let (mut out, mut count, mut allow) = (Vec::new(), 0, false);
    for w in words {
        if !first_word(&mut count) {
            continue;
        }
        let k = cmd_word(w);
        out.extend(cmd_word_line(&k));
        match k {
            CmdWord::Unknown(_) => return (out, true),
            CmdWord::AllowSave => allow = true,
            _ => {}
        }
    }
    if allow {
        out.extend(ALLOW_SAVE_NOTICE.as_bytes());
    }
    (out, false)
}

/// Does the word for the program: prints its line; ALLOWSAVE turns the option on; an unknown word ends the program.
pub fn run_cmd_word(w: &CmdWord) {
    use std::io::Write;
    let _ = std::io::stdout().write_all(&cmd_word_line(w));
    match w {
        CmdWord::AllowSave => ALLOW_SAVE_WORD.store(true, std::sync::atomic::Ordering::Relaxed),
        CmdWord::Unknown(_) => std::process::exit(0),
        _ => {}
    }
}

/// 0x4E93C (called by QuickRestore 0x2C898): when the save's quick-save mode is not chosen yet (byte 21286 = 0)
/// and the program option "allow save" is on, byte 21286 = 1 and the mode (byte 21285) = 1 (the original copies
/// 0x955C7, which is 1 when ALLOWSAVE is set).
pub fn set_allow_save_mode(base: &mut [u8], allow: bool) {
    if base[21286] == 0 && allow {
        base[21286] = 1;
        base[21285] = 1;
    }
}
