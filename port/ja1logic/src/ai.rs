//! Slice 2a: the enemy AI decision layer (docs/spec/ai.md "One tick", "Alert state", "Choosing an action",
//! "Starting an action"; C interface in docs/spec/port_api.md).
//!
//! Everything the host table lists (positions, sightings, scores, AP costs, turning, walking, ...) is a host call,
//! except the library ops of slice 2b (module `search`): `Ai::call` is the one dispatch point that runs them here
//! unless the host claims them in host_ops.
//! World memory belongs to the caller and a host call may change any of it, so every field is read through a raw
//! pointer at the point of use; no Rust reference into world memory is held across a host call.
use std::os::raw::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};

#[path = "search.rs"]
mod search;
#[path = "strength.rs"]
mod strength;
#[path = "positions.rs"]
mod positions;
#[path = "attack.rs"]
mod attack;
#[path = "fire.rs"]
mod fire;
#[path = "sight.rs"]
mod sight;
#[path = "anim.rs"]
mod anim;
pub use anim::set_keep_anim_loop;
#[path = "update.rs"]
mod update;
#[path = "turns.rs"]
pub(crate) mod turns;
#[path = "shoot.rs"]
pub(crate) mod shoot;
#[path = "damage.rs"]
pub(crate) mod damage;
#[path = "death.rs"]
pub(crate) mod death;
#[path = "aim.rs"]
pub(crate) mod aim;
#[path = "click.rs"]
pub(crate) mod click;
#[path = "select.rs"]
pub(crate) mod select;
#[path = "doors.rs"]
pub(crate) mod doors;
#[path = "noise.rs"]
pub(crate) mod noise;
#[path = "leave.rs"]
pub(crate) mod leave;
#[path = "traverse.rs"]
pub(crate) mod traverse;
#[path = "home.rs"]
pub(crate) mod home;
#[path = "inventory.rs"]
pub(crate) mod inventory;
#[path = "ledger.rs"]
pub(crate) mod ledger;
#[path = "pay.rs"]
pub(crate) mod pay;
#[path = "payday.rs"]
pub(crate) mod payday;
#[path = "duties.rs"]
pub(crate) mod duties;
#[path = "stats.rs"]
pub(crate) mod stats;
#[path = "morning.rs"]
pub(crate) mod morning;
#[path = "complain.rs"]
pub(crate) mod complain;
#[path = "corpse.rs"]
pub(crate) mod corpse;
#[path = "dismiss.rs"]
pub(crate) mod dismiss;
#[path = "join.rs"]
pub(crate) mod join;
#[path = "raise.rs"]
pub(crate) mod raise;
#[path = "dawn.rs"]
pub(crate) mod dawn;
#[path = "mapclick.rs"]
pub(crate) mod mapclick;
#[path = "daystart.rs"]
pub(crate) mod daystart;
#[path = "sunrise.rs"]
pub(crate) mod sunrise;
#[path = "stratopen.rs"]
pub(crate) mod stratopen;
#[path = "storyday.rs"]
pub(crate) mod storyday;
#[path = "storyfind.rs"]
pub(crate) mod storyfind;
#[path = "clock.rs"]
pub(crate) mod clock;
#[path = "guide.rs"]
pub(crate) mod guide;
#[path = "sightings.rs"]
pub(crate) mod sightings;
#[path = "guard.rs"]
pub(crate) mod guard;
#[path = "enemy.rs"]
pub(crate) mod enemy;
#[path = "verdict.rs"]
pub(crate) mod verdict;
#[path = "replay.rs"]
pub(crate) mod replay;
#[path = "ending.rs"]
pub(crate) mod ending;
#[path = "newgame.rs"]
pub(crate) mod newgame;
#[path = "battle.rs"]
pub(crate) mod battle;
#[path = "practice.rs"]
pub(crate) mod practice;
#[path = "firstaid.rs"]
pub(crate) mod firstaid;
#[path = "blast.rs"]
pub(crate) mod blast;
#[path = "usetile.rs"]
pub(crate) mod usetile;
#[path = "explode.rs"]
pub(crate) mod explode;
#[cfg(test)]
#[path = "null_rules.rs"]
mod null_rules;
#[path = "aimhire.rs"]
pub(crate) mod aimhire;

/// StepTile 0x46FF4: tile `t` + step `k`, or `t` when the step leaves the map (OffMapStep 0x46F70).
pub fn step_tile(t: i32, k: i32) -> i32 {
    search::step(t, k)
}

/// The shared world (port_api.md "World"). Field order and types are the C layout.
#[repr(C)]
pub struct Ja1World {
    pub soldiers: *mut u8,
    pub tiles: *mut u8,
    pub item_types: *const u8,
    pub anim_table: *const u8,
    pub sector_state: *mut u8,
    pub mission_done: *mut u8,
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
    // slice 2b (Search procedures in ai.md):
    pub knowledge: *mut u8,
    pub fresher: *const u8,
    pub last_seen_m: *mut u8,
    pub last_seen_e: *mut u8,
    pub calls: *mut u8,
    pub paths: *mut u8,
    pub dir_steps: *const u8,
    pub t0_class: *const u8,
    pub t1_class: *const u8,
    pub last_spotter: u8,
    // slice 2b part 2 (Situation score in ai.md):
    pub remember_m: *mut u8,
    pub remember_e: *mut u8,
    // slice 2b part 2, batch 2 (Sector value in ai.md):
    pub story: *mut u8,
    pub plants: *mut u8,
    pub crates: *const u8,
    pub day: i32,
    // slice 2c part 1 (Attack options in ai.md):
    pub weapons: *const u8,
    pub absorb: *const u8,
    // animation (Animation script in ai.md):
    pub anim_rows: *const u8,
    pub selected: u8,
    // animation slice 3 (Starting an animation in ai.md):
    pub anim_scripts: *const u8,
    pub anim_free: *const u8,
    pub anim_sounds: *const u8,
    pub path_preview: i32,
    // animation slice 4 (Frame routine in ai.md):
    pub anim_steps_tile: *const u8,
    pub anim_turn_while: *const u8,
    pub opt_sound: u8,
    pub opt_onscreen: u8,
    pub view_x: i32,
    pub view_y: i32,
    pub view_w: i32,
    pub view_h: i32,
    pub anim_signal: i32,
    pub effect_tile: i16,
    // sector start batch 1 (sector_start.md):
    pub supplies: *mut u8,
    pub equip_skill: *const u8,
    pub attach: *const u8,
    // sector start batch 2:
    pub guard_skill: u8,
    // sector start batches 3 and 4 (ground items):
    pub ground_items: *mut u8,
    pub notes: *mut u8,
    pub move_codes: *mut u8,
    pub held: *mut u8,
    pub blast_list: *mut u8,
    pub listed: *const u8,
    // sector start batch 5 (soldier start):
    pub t2_class: *const u8,
    pub enemy_watch: *mut u8,
    // sector start batch 6 (map patches, supply drops, plants):
    pub text_buf: *mut u8,
    pub item_buf: *mut u8,
    pub crate_state: *mut u8,
    pub crate_shown: *mut u8,
    // sector start batch 7 (fixed items, tmp file, ground blast):
    pub fixed_items: *const u8,
    pub sectors: *mut u8,
    pub bomb_count: i32,
    // sector start batch 8 (the whole sector start):
    pub scratch: *mut u8,
    pub team_on_duty: i32,
    pub purifier: u8,
    // sector start batch 11 (quest check):
    pub safe_opened: u8,
    // soldier update (soldier_update.md):
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
    pub ap_warned: *mut u8,
    pub splash: *const u8,
    pub spotted: i32,
    pub opt_step_text: u8,
    // move order (soldier_update.md):
    pub key_shift: u8,
    pub key_alt: u8,
    pub opt_move_fx: u8,
    // one frame and the timer tick (soldier_update.md):
    pub seen_enemies: u16,
    pub in_flight: u8,
    pub merc_shown: u8,
    pub interrupt_turn: u8,
    pub fx_anims: *const u8,
    pub tick_step: i32,
    // merc view (soldier_update.md "Merc view"):
    pub view_rays: *const u8,
    pub obj_block: *const u8,
    pub story_finder: u8,
    pub spot_stop: u8,
    // turns (turns.md):
    pub itq: *mut u8,
    pub itq_len: u8,
    pub bled: *mut u8,
    pub timed_fx: *mut u8,
    pub new_round: u8,
    pub turn_timer: i16,
    pub turn_timer2: i16,
    pub shown_slot: i32,
    pub view_tile: i32,
    pub focus_slot: u8,
    pub turn_shown: u8,
    pub merc_to_show: u8,
    pub view_hold: u8,
    // turns (turns.md "Spend AP"):
    pub weather_tire: i32,
    pub merc_tire: i8,
    // new turn (turns.md "New turn"):
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
    pub sector_spoken: *mut u8,
    // shooting (shooting.md):
    pub silencer: *const u8,
    pub shot_sound: *const u8,
    pub impact_noise: *const u8,
    pub shot_range: *const u8,
    pub busy_timer: i32,
    pub replay_m: u8,
    pub first_hit: u8,
    pub fx_id: u8,
    // shooting batch 2 (shooting.md):
    pub dmg_text: *mut u8,
    pub marked_slot: u8,
    pub knockdown: u8,
    pub focus_last: u8,
    pub hit_scale: u8,
    // death (shooting.md batch 4):
    pub dead_mercs: i32,
    pub dead_allies: i32,
    pub dead_npc: i32,
    pub enemy_deaths: i32,
    pub squad_lost: u8,
    pub last_killer: u8,
    // aiming (shooting.md batch 5):
    pub posture: *const u8,
    pub window: *const u8,
    pub attack_sound: *const u8,
    pub diff_mod: *const u8,
    // click (click.md):
    pub key_ctrl: u8,
    // turn start (turns.md):
    pub fast_timer: i32,
    // selecting a merc (select.md):
    pub marks_shown: i32,
    pub marks_n: i32,
    pub marks: *mut u8,
    pub opt_path_marks: u8,
    pub sees_slot: u8,
    pub preview_slot: u8,
    // the rest of the update pass (soldier_update.md Update end, Fuses, Timer tick):
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
    // the tactical loop head (soldier_update.md Loop head):
    pub turn_tick: i32,
    // leaving the sector (soldier_update.md Leaving the sector):
    pub quit_tactical: u8,
    // the inventory screen (inventory.md):
    pub inv_cursor: *mut u8,
    pub inv_swap: *mut u8,
    pub inv_picked: *mut u8,
    pub inv_give: *mut u8,
    pub item_rects: *const u8,
    pub slot_rects: *const u8,
    pub vest_owner: u8,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub mouse_buttons: u8,
    pub key_flags: u16,
    pub tip_item: u8,
    pub face_left: u8,
    pub cursor_drop: u8,
    pub drop_answer: i32,
    pub panel_dy: i32,
    pub voice_test: u8,
    // the day accounts (strategic.md Day accounts, Worker pool):
    pub accounts: *mut u8,
    pub prev_balance: i32,
    pub sector_order: *mut u8,
    // morning pay (strategic.md Morning pay); accounts is 0xA8 bytes for it (+0xA0 = resignations):
    pub profiles: *mut u8,
    pub stash_counts: *mut u8,
    pub stat_progress: *mut u8,
    pub greed: *const u8,
    // morning stat changes (strategic.md Morning stat changes):
    pub stat_changed: *mut u8,
    // back on duty (strategic.md Back on duty):
    pub duty7_merc: u8,
    // day start (strategic.md Day start):
    pub sector_replay: *mut u8,
    // story events at the day start (strategic.md Story events at the day start):
    pub guide_queue: *mut u8,
    // the day clock (turns.md The day clock):
    pub clock_minute: u8,
    pub clock_hour: u8,
    pub day_hours: i8,
    pub day_minutes: i32,
    pub day_bar: i32,
    pub hard_day: u8,
    pub night_warned: u8,
    pub guard_eta: *mut u8,
    pub guard_rec: *mut u8,
    // the guide's advice (strategic.md The guide's advice):
    pub guides_hired: u8,
    pub guide_wait: u8,
    // world_load.md section 4 step 3:
    pub idle_day: u8,
    // strategic.md Enemy moves:
    pub enemy_counter: u8,
    // strategic.md The day's verdict:
    pub start_trees: i32,
    pub start_workable: i32,
    pub start_tapped: i32,
    pub start_sectors: i32,
    pub start_cap: i32,
    pub start_plants: i32,
    pub item_told: *mut u8,
    pub bonus_due: u8,
    pub bonus: u16,
    // combat.md Blast, exact steps:
    pub blaster: u8,
    pub gas_clouds: *mut u8,
    pub tileset: u8,
    pub no_break: *const u8,
    // combat.md The listening device at a door, exact steps:
    pub walter_said: *mut u8,
    pub walter_next: *mut u8,
    pub walter_sector: u8,
    // panel.md Clicks on the bottom strip:
    pub strip_box: u8,
    // strategic.md The day replay:
    pub opt_replays: u8,
    pub replay_carry: u8,
    // MoveMercTo 0x1C608 confirm (cursor.md "The walk confirm"): option byte 0x1163C6, and the detector wait that
    // a library TacticalClick with item 0x4D left for the front-end (slot, tile; tile < 0 = none).
    pub opt_confirm_move: u8,
    pub detector_slot: u8,
    pub detector_tile: i16,
}

pub type RandomFn = unsafe extern "C" fn(ctx: *mut c_void, n: i32) -> i32;
pub type CallFn =
    unsafe extern "C" fn(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, out: *mut u8, out_size: i32) -> u32;

/// The host (port_api.md "Host").
#[repr(C)]
pub struct Ja1Host {
    pub ctx: *mut c_void,
    pub random: Option<RandomFn>,
    pub call: Option<CallFn>,
    /// bit n set: op n goes to the host even when the library implements it.
    pub host_ops: u64,
}

/// Host call op numbers (port_api.md host table).
pub mod op {
    pub const IDLE: i32 = 1;
    pub const NOISE_TILE: i32 = 2;
    pub const FIND_DRY_TILE: i32 = 3;
    pub const PATROL_TILE: i32 = 4;
    pub const BUDDY_TILE: i32 = 5;
    pub const DIRECTION: i32 = 6;
    pub const TILE_DIST: i32 = 7;
    pub const NEWS_VALUE: i32 = 8;
    pub const SECTOR_VALUE: i32 = 9;
    pub const APPROACH_TILE: i32 = 10;
    pub const ALLY_CALL_TILE: i32 = 11;
    pub const SEARCH_TILE: i32 = 12;
    pub const MOVE_AP: i32 = 13;
    pub const REACH_AP: i32 = 14;
    pub const ATTACK_READY: i32 = 15;
    pub const SWAP_HAND: i32 = 16;
    pub const ACTION_AP: i32 = 17;
    pub const CAN_STAND_AT: i32 = 18;
    pub const MISSION_CANCEL: i32 = 19;
    pub const OPP_STRENGTH: i32 = 20;
    pub const RETREAT_TILE: i32 = 21;
    pub const ADVANCE_TILE: i32 = 22;
    pub const TARGET_TILE: i32 = 23;
    pub const GUN_OPTION: i32 = 24;
    pub const THROW_OPTION: i32 = 25;
    pub const KNIFE_OPTION: i32 = 26;
    pub const BUSY: i32 = 27;
    pub const TURN: i32 = 28;
    pub const WALK_TO: i32 = 29;
    pub const CLICK: i32 = 30;
    pub const SPEND_AP: i32 = 31;
    /// combat.md Mission blast (library op).
    pub const MISSION_BLAST: i32 = 32;
    pub const RADIO_RESULT: i32 = 33;
    pub const SHARE_SIGHTINGS: i32 = 34;
    pub const PERCEIVE: i32 = 35;
    pub const REDRAW: i32 = 36;
    pub const DELAY: i32 = 37;
    pub const SANTINO_SPEECH: i32 = 38;
    pub const FIRE: i32 = 39;
    pub const PATH_AP: i32 = 40;
    pub const FIND_PATH: i32 = 41;
    pub const FIRE_LINE: i32 = 42;
    pub const CONNECTED_HOME: i32 = 43;
    pub const QUEST_CHECK: i32 = 44;
    pub const SHOT_AP: i32 = 45;
    pub const CHANCE_TO_HIT: i32 = 46;
    pub const DIV_ZERO: i32 = 47;
    pub const HIT_TEST: i32 = 48;
    pub const SIGHT_LINE: i32 = 49;
    pub const END_TASK: i32 = 50;
    pub const REPLACE: i32 = 51;
    pub const SET_ANIM: i32 = 52;
    pub const STOP_ANIM: i32 = 53;
    pub const MERC_MESSAGE: i32 = 54;
    pub const UI_REFRESH: i32 = 55;
    pub const ANIM_TURN: i32 = 56;
    pub const MERC_TEXT: i32 = 57;
    pub const RESERVE_AP: i32 = 58;
    pub const WALK_SOUND: i32 = 59;
    pub const SFX: i32 = 60;
    pub const ANIM_FRAME: i32 = 61;
    pub const REVEAL_VIEW: i32 = 62;
    pub const ENEMY_TURNED: i32 = 63;
    pub const GUARD_TURNED: i32 = 64;
    pub const EFFECT_SLOT: i32 = 65;
    pub const TILE_EFFECT: i32 = 66;
    pub const QUOTE: i32 = 67;
    pub const GUN_JAM: i32 = 68;
    pub const USE_SPECIAL: i32 = 69;
    pub const DOOR_FX: i32 = 70;
    pub const POPUPS: i32 = 71;
    pub const STOP_WALK: i32 = 72;
    pub const PICKUP_HERE: i32 = 73;
    pub const USE_ON_TILE: i32 = 74;
    pub const BURY_MINE: i32 = 75;
    pub const REVIVE_ROLL: i32 = 76;
    pub const MELEE_HIT: i32 = 77;
    pub const KILL: i32 = 78;
    pub const START_ATTACK: i32 = 79;
    pub const ATTACK_GO: i32 = 80;
    pub const LAUNCH: i32 = 81;
    pub const MERC_DIES: i32 = 82;
    pub const ANIM_END: i32 = 83;
    pub const ANIM_STEP: i32 = 84;
    pub const CURSOR_BACK: i32 = 85;
    pub const OPEN_PATCH: i32 = 86;
    pub const READ: i32 = 87;
    pub const CLOSE: i32 = 88;
    pub const REPLAY_EVENT: i32 = 89;
    pub const EXPLODE: i32 = 90;
    pub const EFFECT: i32 = 91;
    pub const OPEN_TMP: i32 = 92;
    pub const SECTOR_RESETS: i32 = 93;
    pub const KEEP_SELECTED: i32 = 94;
    pub const SECTOR_TAIL: i32 = 95;
    pub const SEEK: i32 = 96;
    pub const ANIM_NEXT: i32 = 97;
    pub const WALK_BLOCKED: i32 = 98;
    pub const FOCUS: i32 = 99;
    pub const CRY: i32 = 100;
    pub const VOICE: i32 = 101;
    pub const RELOAD_CHECK: i32 = 102;
    pub const QUOTE_EVENT: i32 = 103;
    pub const CLOSE_DOOR: i32 = 104;
    pub const STAT_PRACTICE: i32 = 105;
    pub const CONSUME_TRIGGER: i32 = 106;
    pub const BREAK_OBJECT: i32 = 107;
    pub const TRIGGER_ITEM: i32 = 108;
    pub const TAKE_GROUND_ITEM: i32 = 109;
    pub const TAKE_DAMAGE: i32 = 110;
    pub const ENEMY_STEP: i32 = 111;
    pub const TAC_REDRAW: i32 = 112;
    pub const MERC_INTERRUPT: i32 = 113;
    pub const NEXT_TURN: i32 = 114;
    pub const SIGHT_CHECK: i32 = 115;
    pub const ASK_INTERRUPT: i32 = 116;
    pub const STEP_NOISE: i32 = 117;
    pub const MINE_FLAG: i32 = 118;
    pub const AI_THINK: i32 = 119;
    pub const LOW_AP_STOP: i32 = 120;
    pub const USE_COST: i32 = 121;
    pub const THROW_FOLLOW: i32 = 122;
    pub const TURN_BANNER: i32 = 123;
    pub const NEAREST_MERC: i32 = 124;
    pub const MESSAGE: i32 = 125;
    pub const MOVE_SOUND: i32 = 126;
    pub const MAKE_NOISE: i32 = 127;
    pub const ASK_MERC_INTERRUPT: i32 = 128;
    pub const STEP_TEXT: i32 = 129;
    pub const TILE_MARKER: i32 = 130;
    pub const ITQ_POP: i32 = 131;
    pub const TURN_HAND_OVER: i32 = 132;
    pub const ROUND_END: i32 = 133;
    pub const NEW_TURN: i32 = 134;
    pub const PORTRAIT: i32 = 135;
    pub const PROJECTILE_STEP: i32 = 136;
    pub const ACTIVE_FRAME: i32 = 137;
    pub const SOLDIER_FRAME: i32 = 138;
    pub const STEP_SOUND_STOP: i32 = 139;
    pub const MOVE_ORDER: i32 = 140;
    pub const START_TURN: i32 = 141;
    pub const MOUSE_HIDE: i32 = 142;
    pub const PANEL_REDRAW: i32 = 144;
    pub const MOUSE_OFF: i32 = 145;
    pub const MOUSE_MOVE: i32 = 146;
    pub const MOUSE_SAVE: i32 = 147;
    pub const PANEL_SLOTS: i32 = 148;
    pub const PORTRAIT_REFRESH: i32 = 149;
    pub const TIMED_EFFECT: i32 = 150;
    pub const AID_TICK: i32 = 151;
    pub const TURN_REST: i32 = 152;
    pub const WOUND_BAR: i32 = 153;
    pub const ENERGY_BAR: i32 = 154;
    pub const MERC_PANEL: i32 = 155;
    pub const PANEL_UPDATE: i32 = 156;
    pub const MOUSE_REFRESH: i32 = 157;
    pub const TURN_UI: i32 = 158;
    pub const GUARDS_ARRIVE: i32 = 159;
    pub const SECTOR_TICK: i32 = 160;
    pub const GAS_TICK: i32 = 161;
    pub const MOUSE_CHECK: i32 = 162;
    pub const CURSOR_RESET: i32 = 163;
    pub const VIEW_SAVE: i32 = 164;
    pub const VIEW_RESTORE: i32 = 165;
    pub const ITEM_PANEL: i32 = 166;
    pub const SPEECH_READY: i32 = 167;
    pub const SECTOR_SPEECH: i32 = 168;
    pub const STAGE_EFFECTS: i32 = 169;
    pub const STAGE_FIVE: i32 = 170;
    pub const KEPT_TURN_START: i32 = 171;
    pub const ENEMY_FOCUS: i32 = 172;
    pub const SHOT_ALERT: i32 = 173;
    pub const ITEM_COND: i32 = 174;
    pub const BULLET_DAMAGE: i32 = 175;
    pub const LAND_ITEM: i32 = 176;
    pub const STORY_HIT: i32 = 177;
    pub const PUFF: i32 = 178;
    pub const DOOR_LOOKUP: i32 = 179;
    pub const HIT_LOCATION: i32 = 180;
    pub const KNOCKDOWN_ANIM: i32 = 181;
    pub const INTERRUPT_CHECK: i32 = 182;
    pub const LOSS_MSG: i32 = 183;
    pub const MERC_SEES: i32 = 184;
    pub const CLICK_PANEL: i32 = 185;
    pub const DEATH_SHOW: i32 = 186;
    pub const SECTOR_WON: i32 = 187;
    pub const SQUAD_LOST: i32 = 188;
    pub const RELOADED: i32 = 190;
    pub const FIRST_AID: i32 = 191;
    pub const ATTACK_CURSOR: i32 = 192;
    pub const VOC: i32 = 193;
    pub const BANNER: i32 = 194;
    // select.md
    pub const SELECT_LIGHT: i32 = 195;
    pub const PANEL_VIEW: i32 = 196;
    pub const SEES_MSG: i32 = 197;
    pub const MOUSE_WAIT: i32 = 198;
    pub const CURSOR_STATE: i32 = 199;
    pub const CURSOR_SELECT: i32 = 200;
    // soldier_update.md Update end
    pub const SECTOR_GAINED: i32 = 201;
    pub const WON_QUOTE: i32 = 202;
    pub const ENEMIES_MSG: i32 = 203;
    pub const MUSIC: i32 = 204;
    pub const TILE_ANIM: i32 = 205;
    pub const PAL_CYCLE: i32 = 206;
    pub const PAL_SET: i32 = 207;
    pub const STRIP: i32 = 208;
    // combat.md Doors
    pub const DOOR_SND: i32 = 209;
    pub const DOOR_ANIM: i32 = 210;
    // ai.md Noise
    pub const HEARS: i32 = 211;
    pub const NOISE_INT: i32 = 212;
    pub const HEARS_MSG: i32 = 213;
    // soldier_update.md Leaving the sector
    pub const LEAVE_SECTOR: i32 = 214;
    pub const FADE: i32 = 215;
    pub const MUSIC_STOP: i32 = 216;
    pub const END_OF_DAY: i32 = 217;
    pub const RESOLVE_BATTLE: i32 = 218;
    pub const SAVE_TMP: i32 = 219;
    pub const TIMER_OFF: i32 = 220;
    // soldier_update.md End of day
    pub const EOD_BEGIN: i32 = 254;
    pub const EOD_WAIT: i32 = 255;
    pub const EOD_TEXT: i32 = 256;
    pub const EOD_POLL: i32 = 257;
    pub const EOD_SOUND_STOP: i32 = 258;
    pub const OVERHEAD_SOUNDS: i32 = 259;
    pub const SHOW_SCREEN: i32 = 260;
    // strategic.md Worker pool and natives who quit
    pub const NATIVES_QUIT_MSG: i32 = 261;
    pub const MAP_REDRAW: i32 = 262;
    // strategic.md Sector click on the island map
    pub const GUIDE_SAY: i32 = 302;
    pub const DAY_ASK: i32 = 303;
    // strategic.md Sunrise
    pub const SUNRISE_SHOW: i32 = 304;
    // strategic.md Strategic screen opening
    pub const MERC_GONE: i32 = 305;
    pub const MAP_SHOW: i32 = 306;
    pub const QUEST_RECORD: i32 = 307;
    pub const STASH_TAKE: i32 = 308;
    // turns.md The day clock, Time compression
    pub const DAY_BAR: i32 = 309;
    pub const ENEMY_PICK: i32 = 310;
    pub const ENEMY_MOVE: i32 = 311;
    pub const GUARD_ENTER: i32 = 312;
    pub const MODE_BANNER: i32 = 313;
    pub const GUARDS_MSG: i32 = 314;
    pub const MOUSE_RELEASE: i32 = 315;
    pub const REFRESH: i32 = 316;
    // strategic.md The guide's advice
    pub const BRENDA_SAY: i32 = 317;
    pub const PANEL_DRAW: i32 = 318;
    pub const GUIDE_OFFER: i32 = 319;
    pub const MERC_JOIN: i32 = 320;
    // ai.md Sightings as steps
    pub const GUARD_SEES_MSG: i32 = 326;
    pub const CLEAR_ROOMS: i32 = 327;
    // strategic.md Morning pay
    pub const FACE_CLEAR: i32 = 263;
    pub const STASH_PUT: i32 = 264;
    pub const FACE_MOVE: i32 = 265;
    pub const TEAM_REDRAW: i32 = 266;
    pub const RUNS_OFF: i32 = 267;
    pub const ISLAND_MAP: i32 = 268;
    pub const RUNS_OFF_END: i32 = 269;
    pub const FIRE_QUOTE: i32 = 328;
    /// strategic.md The A.I.M. screen: the named merc's voice remark (profile, m); A.I.M. merc removed (slot).
    pub const AIM_REMARK: i32 = 337;
    pub const AIM_REMOVED: i32 = 338;
    /// combat.md Springing a ground item, exact steps: the trigger click (host); call_enemies, take_patch and
    /// toggle_doors are library ops; pick_up (host).
    pub const CALL_ENEMIES: i32 = 340;
    pub const TAKE_PATCH: i32 = 341;
    pub const TOGGLE_DOORS: i32 = 342;
    pub const PICK_UP: i32 = 343;
    pub const MINE_SPOTTED: i32 = 247;
    pub const DISARM: i32 = 248;
    pub const CURSOR_TO_SOLDIER: i32 = 344;
    pub const DISARM_ASK: i32 = 345;
    pub const GROUND_ITEM_RESET: i32 = 346;
    pub const MENU_REDRAW: i32 = 347;
    pub const WALTER_SAY: i32 = 348;
    pub const REPLAY_COUNT: i32 = 353;
    pub const REPLAY_RECORD: i32 = 354;
    pub const REPLAY_ANIM: i32 = 355;
    pub const REPLAY_DRAW: i32 = 356;
    /// SlideViewToTile? 0x3F578 (tile): only fan fix F15 calls it from the library.
    pub const SLIDE_VIEW: i32 = 357;
    /// DrawProjectilePass? 0x17548, its effect-record part (blast.rs effect_pass): the frame's pass and the one that
    /// MercDies? runs through TacticalRedraw(1) (#70 row 30); arg = the count limit (0x13, overview 0x16). Library op.
    pub const EFFECT_PASS: i32 = 358;
    /// EffectHit? 0x173B4 (tile, kind, power) as effect_pass calls it. Library op.
    pub const EFFECT_HIT: i32 = 359;
    // strategic.md Enemy moves:
    pub const ENEMY_WALK_IN: i32 = 329;
    pub const HOME_LOST_MSG: i32 = 330;
    pub const RATING_WORD: i32 = 331;
    // strategic.md Sector battles
    pub const BATTLE_UI: i32 = 332;
    pub const BATTLE_WAIT: i32 = 333;
    pub const BATTLE_BOX: i32 = 334;
    pub const BATTLE_MSG: i32 = 335;
    pub const EXPLODE_ART: i32 = 336;
    // strategic.md Corpse export
    pub const CORPSE_ASK: i32 = 292;
    pub const CORPSE_PAID: i32 = 293;
    pub const CORPSE_END: i32 = 294;

    /// The ops the library implements itself (port_api.md "Library ops").
    pub const LIBRARY: u64 = 1 << NOISE_TILE
        | 1 << IDLE
        | 1 << DIRECTION
        | 1 << TILE_DIST
        | 1 << STOP_ANIM
        | 1 << WALK_TO
        | 1 << PATH_AP
        | 1 << FIND_PATH
        | 1 << FIND_DRY_TILE
        | 1 << PATROL_TILE
        | 1 << BUDDY_TILE
        | 1 << NEWS_VALUE
        | 1 << SECTOR_VALUE
        | 1 << CONNECTED_HOME
        | 1 << APPROACH_TILE
        | 1 << ALLY_CALL_TILE
        | 1 << CAN_STAND_AT
        | 1 << SEARCH_TILE
        | 1 << OPP_STRENGTH
        | 1 << RETREAT_TILE
        | 1 << ADVANCE_TILE
        | 1 << TARGET_TILE
        | 1 << GUN_OPTION
        | 1 << THROW_OPTION
        | 1 << KNIFE_OPTION
        | 1 << FIRE_LINE
        | 1 << SIGHT_LINE
        | 1 << ACTION_AP
        | 1 << SHOT_AP
        | 1 << RADIO_RESULT
        | 1 << MISSION_CANCEL
        | 1 << QUEST_CHECK
        | 1 << MISSION_BLAST
        | 1 << REPLACE
        | 1 << SET_ANIM
        | 1 << ANIM_TURN
        | 1 << RESERVE_AP
        | 1 << ANIM_FRAME
        | 1 << SPEND_AP
        | 1 << FIRE
        | 1 << CHANCE_TO_HIT
        | 1 << HIT_TEST
        | 1 << CLICK
        | 1 << ATTACK_READY
        | 1 << REACH_AP
        | 1 << REVEAL_VIEW
        | 1 << PERCEIVE
        | 1 << SHARE_SIGHTINGS
        | 1 << END_TASK
        | 1 << TURN
        | 1 << MOVE_AP
        | 1 << SWAP_HAND
        | 1 << BUSY;
}

/// Soldier record offsets (defs/types.toml struct.soldier; ai.md "One tick" for +0x120 / +0x124).
mod f {
    pub const ITEMS: usize = 0x000; // item[12], 12 bytes each, byte 0 = type
    pub const B09C: usize = 0x09C;
    pub const AI_TYPE: usize = 0x0BA;
    pub const ALERT: usize = 0x0BB;
    pub const LAST_TARGET_TILE: usize = 0x0D0;
    pub const ACTION_TICKS: usize = 0x0F0;
    pub const WALK_FAIL_TILE: usize = 0x10E;
    pub const TILE: usize = 0x114;
    pub const X2: usize = 0x116;
    pub const Y2: usize = 0x118;
    pub const WALK_TARGET: usize = 0x120;
    pub const STEP_TILE: usize = 0x124;
    pub const X: usize = 0x126;
    pub const Y: usize = 0x128;
    pub const HOME_TILE: usize = 0x12E;
    pub const CALM_BONUS: usize = 0x131;
    pub const AI_TARGET: usize = 0x132;
    pub const IN_PLAY: usize = 0x138;
    pub const IN_CUR_SECTOR: usize = 0x182;
    pub const HP: usize = 0x183;
    pub const HP_MAX: usize = 0x184;
    pub const ENERGY: usize = 0x18A;
    pub const EXPLOSIVES: usize = 0x18F;
    pub const B197: usize = 0x197;
    pub const FACING: usize = 0x1A1;
    pub const TURN_TO: usize = 0x1A2;
    pub const AP_LEFT: usize = 0x1A5;
    pub const B1A8: usize = 0x1A8;
    pub const B1AB: usize = 0x1AB;
    pub const TURN_DONE: usize = 0x1B0;
    pub const GROUND_UNDER: usize = 0x1B2;
    pub const OBJECT_UNDER: usize = 0x1B4;
    pub const B1B8: usize = 0x1B8;
    pub const RETHINK: usize = 0x1C1;
    pub const B1C4: usize = 0x1C4;
    pub const POST_FACING: usize = 0x1CF;
    pub const AI_ACTION: usize = 0x1D1;
    pub const ATTITUDE: usize = 0x1D3;   // debug page "attitude" (cursor.md:551)
    pub const ORDERS: usize = 0x1D4;    // debug page "orders" (cursor.md:551)
    pub const B1D5: usize = 0x1D5;
    pub const ACTION_RUNNING: usize = 0x1D6;
    pub const B1D7: usize = 0x1D7;
    pub const ATTACK_SLOT: usize = 0x1D8;
    pub const ANIM: usize = 0x1E8;
    pub const ANIM_IDX: usize = 0x1F3;
    pub const AIM: usize = 0x1FD;
}

const REC: usize = 0x200;
const SLOTS: usize = 48;
const TILES: usize = 3136;
const SECTORS: usize = 80;
const ITEM_TYPES: usize = 136;
const NONE: u16 = 0xC41;
pub(crate) const NO_SLOT: u8 = 0x40;
const KIND_GUN: u8 = 0x92;
const KIND_THROW: u8 = 0x95;
const KIND_KNIFE: u8 = 0x96;

/// One attack option record (port_api.md "Option record").
#[derive(Clone, Copy)]
struct Opt {
    valid: bool,
    slot: u8,
    aim: u8,
    chance: u8,
    score: i32,
    tile: u16,
}

fn is_walk(code: u8) -> bool {
    matches!(code, 2 | 3 | 4 | 5 | 8 | 13)
}
fn is_attack(code: u8) -> bool {
    matches!(code, 11 | 12 | 14 | 16)
}

/// Tile number as a signed 16-bit host argument.
fn t16(t: u16) -> i32 {
    t as i16 as i32
}

/// Search / attack position mode by situation score (Combat step 7, Engaged step 7).
fn search_mode(s: i32) -> i32 {
    if s < 2 {
        1
    } else if s == 3 || s == 4 {
        2
    } else {
        0
    }
}

/// True during a per-frame draw (test only: the front-end's script line `framefree`, validate/replay_week.py).
static FRAME_DRAW: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn ja1_frame_draw() -> i32 {
    FRAME_DRAW.load(std::sync::atomic::Ordering::Relaxed) as i32
}

struct Ai {
    w: *mut Ja1World,
    h: *const Ja1Host,
    s: usize,
}

impl Ai {
    fn new(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> Option<Ai> {
        // SPEC GAP: a null world, host or soldier buffer, or a slot outside 0..47, does nothing (the entry
        // points return 0, choose returns the fallback).
        if w.is_null() || h.is_null() || !(0..SLOTS as i32).contains(&slot) {
            return None;
        }
        if unsafe { (*w).soldiers.is_null() } {
            return None;
        }
        Some(Ai { w, h, s: slot as usize })
    }

    fn slot(&self) -> i32 {
        self.s as i32
    }

    // ---- soldier record access (raw pointer, little-endian) ----
    fn rd8_of(&self, slot: usize, off: usize) -> u8 {
        // SPEC GAP: a read outside the 48 records (bad slot from a host record, offset past 0x1FF) gives 0.
        if slot >= SLOTS || off >= REC {
            return 0;
        }
        unsafe {
            let p = (*self.w).soldiers;
            if p.is_null() { 0 } else { p.add(slot * REC + off).read() }
        }
    }
    fn rd8(&self, off: usize) -> u8 {
        self.rd8_of(self.s, off)
    }
    /// Signed byte, widened.
    fn s8(&self, off: usize) -> i32 {
        self.rd8(off) as i8 as i32
    }
    fn rd16(&self, off: usize) -> u16 {
        self.rd8(off) as u16 | (self.rd8(off + 1) as u16) << 8
    }
    fn wr8(&self, off: usize, v: u8) {
        if off >= REC {
            return;
        }
        unsafe {
            let p = (*self.w).soldiers;
            if !p.is_null() {
                p.add(self.s * REC + off).write(v);
            }
        }
    }
    fn wr16(&self, off: usize, v: u16) {
        self.wr8(off, v as u8);
        self.wr8(off + 1, (v >> 8) as u8);
    }

    // ---- world access: copies of scalar fields, no reference kept ----
    fn w_turn_based(&self) -> i32 { unsafe { (*self.w).turn_based } }
    fn w_busy(&self) -> i32 { unsafe { (*self.w).busy } }
    fn w_active_slot(&self) -> u8 { unsafe { (*self.w).active_slot } }
    fn w_combat(&self) -> bool { unsafe { (*self.w).combat != 0 } }
    fn w_alarm(&self) -> bool { unsafe { (*self.w).alarm != 0 } }
    fn w_difficulty(&self) -> u8 { unsafe { (*self.w).difficulty } }
    fn w_cur_sector(&self) -> u8 { unsafe { (*self.w).cur_sector } }
    fn w_mission_step(&self) -> i32 { unsafe { (*self.w).mission_step as i32 } }
    fn w_mission_slot(&self) -> u8 { unsafe { (*self.w).mission_slot } }
    fn w_mission_target(&self) -> u16 { unsafe { (*self.w).mission_target } }
    fn w_mission_spot(&self) -> u16 { unsafe { (*self.w).mission_spot } }

    fn tile_byte(&self, tile: u16, i: usize) -> u8 {
        // SPEC GAP: a tile outside 0..3135 reads as all-zero bytes.
        unsafe {
            let p = (*self.w).tiles;
            if p.is_null() || tile as usize >= TILES {
                return 0;
            }
            p.add(tile as usize * 8 + i).read()
        }
    }
    /// Occupant of a tile: byte 5 & 0x7F, 0x30 and up = none (0x40).
    fn occupant(&self, tile: u16) -> u8 {
        let o = self.tile_byte(tile, 5) & 0x7F;
        if o >= 0x30 { NO_SLOT } else { o }
    }
    fn item_kind(&self, item: u8) -> u8 {
        // SPEC GAP: a null item table or an item number of 136 and up has kind 0.
        unsafe {
            let p = (*self.w).item_types;
            if p.is_null() || item as usize >= ITEM_TYPES {
                return 0;
            }
            p.add(item as usize * 12 + 2).read()
        }
    }
    fn anim_col(&self, code: u16, col: usize) -> u8 {
        // SPEC GAP: an animation word of 256 and up (past the 256-row table) or a null table reads as 0.
        unsafe {
            let p = (*self.w).anim_table;
            if p.is_null() || code as usize >= 256 {
                return 0;
            }
            p.add(code as usize * 8 + col).read()
        }
    }
    fn sector_byte(&self, p: *mut u8, size: usize, i: usize) -> u8 {
        // SPEC GAP: a current sector of 80 and up (or a null table) reads as 0.
        let s = self.w_cur_sector() as usize;
        if p.is_null() || s >= SECTORS {
            return 0;
        }
        unsafe { p.add(s * size + i).read() }
    }
    fn enemies_left(&self) -> u8 {
        self.sector_byte(unsafe { (*self.w).sector_state }, 9, 1)
    }
    fn mission_done(&self) -> bool {
        self.sector_byte(unsafe { (*self.w).mission_done }, 1, 0) != 0
    }

    // ---- derived soldier values ----
    fn item(&self, entry: usize) -> u8 {
        self.rd8(f::ITEMS + entry * 12)
    }
    /// Current animation: the word at 0x1E8 + 2 × animIdx.
    fn anim(&self) -> u16 {
        self.rd16(f::ANIM + 2 * self.rd8(f::ANIM_IDX) as usize)
    }
    fn own_tile(&self) -> u16 {
        self.rd16(f::TILE)
    }
    fn ap(&self) -> i32 {
        self.s8(f::AP_LEFT)
    }
    fn energy(&self) -> i32 {
        self.s8(f::ENERGY)
    }
    fn facing(&self) -> i32 {
        self.s8(f::FACING)
    }
    fn orders(&self) -> u8 {
        self.rd8(f::ORDERS)
    }
    fn attitude(&self) -> u8 {
        self.rd8(f::ATTITUDE)
    }
    fn calm_bonus(&self) -> i32 {
        self.rd8(f::CALM_BONUS) as i32
    }
    fn tired(&self) -> i32 {
        (self.s8(f::HP_MAX) - self.s8(f::HP)) + (100 - self.energy())
    }
    fn in_water(&self) -> bool {
        self.rd8(f::GROUND_UNDER) >= 0x6C && !matches!(self.rd8(f::OBJECT_UNDER), 0x1C | 0x1F)
    }
    /// Gas on the own tile and no gas mask. Fan fix F43 (#70 row 93; 1.15m CalmAction 0x6DB37 / EngagedAction
    /// 0x6EBB1 -> tails 0x8AD93 / 0x8AD80): the mask needs condition >= 10 (signed byte).
    fn in_gas(&self) -> bool {
        self.tile_byte(self.own_tile(), 5) & 0x80 != 0
            && (self.item(3) != 0x28 || (crate::fanfix::on(43) && (self.rd8(f::ITEMS + 3 * 12 + 2) as i8) < 10))
    }
    fn set_target(&self, t: u16) {
        self.wr16(f::AI_TARGET, t);
    }
    /// "Nothing": aiTarget = no tile, return 0.
    fn nothing(&self) -> i32 {
        self.set_target(NONE);
        0
    }
    /// "Walk to T with code C": set target T; Some(C) (and b1A8 = 0) when T is a tile.
    fn walk(&self, t: u16, code: i32) -> Option<i32> {
        self.set_target(t);
        if t == NONE {
            return None;
        }
        self.wr8(f::B1A8, 0);
        Some(code)
    }

    // ---- host ----
    /// A draw of TacticalLoop's per-frame draws: as `random`, with the mark for the front-end's test line `framefree`.
    fn frame_random(&self, n: i32) -> i32 {
        FRAME_DRAW.store(true, std::sync::atomic::Ordering::Relaxed);
        let r = self.random(n);
        FRAME_DRAW.store(false, std::sync::atomic::Ordering::Relaxed);
        r
    }

    fn random(&self, n: i32) -> i32 {
        unsafe {
            let h = &*self.h;
            match h.random {
                Some(r) => r(h.ctx, n),
                None => 0,
            }
        }
    }
    /// "Roll c": c cut to a signed 16-bit value, then random(100) < c.
    fn roll(&self, c: i32) -> bool {
        self.random(100) < c as i16 as i32
    }
    /// The one dispatch point for every op ("Library ops", port_api.md): a library op runs the library's own
    /// procedure unless its bit is set in host_ops; every other op (and a claimed one) is a host call.
    fn dispatch(&self, opc: i32, args: &[i32], out: Option<&mut [u8]>) -> u32 {
        // Library membership: LIBRARY bits for ops below 63, plus ops 83 anim_end, 84 anim_step, 97 anim_next, 98 walk_blocked,
        // 106 consume_trigger, 109 take_ground_item, 111 enemy_step, 117 step_noise, 119 ai_think, 122 throw_follow,
        // 131 itq_pop, 132 turn_hand_over, 133 round_end, 140 move_order, 141 start_turn, 134 new_turn, 114 next_turn and
        // 113 merc_interrupt, 81 launch, 136 projectile_step, 110 take_damage, 175 bullet_damage, 180 hit_location, 173 shot_alert,
        // 182 interrupt_check, 128 ask_merc_interrupt and 116 ask_interrupt (combat.md Interrupt contests), 78 kill, 82 merc_dies, 79 start_attack, 80 attack_go, 68 gun_jam, 123 turn_banner,
        // 120 low_ap_stop (select.md Select merc), 70 door_fx, 104 close_door, 107 break_object, 179 door_lookup
        // (combat.md Doors), 190 reloaded and 102 reload_check (combat.md Reload), 127 make_noise, 211 hears and
        // 212 noise_int (ai.md Noise), 214 leave_sector (soldier_update.md Leaving the sector), 217 end_of_day
        // (soldier_update.md End of day), 310 enemy_pick and 311 enemy_move (strategic.md Enemy moves), 336 explode_art (combat.md Craters), 77
        // melee_hit (shooting.md batch 6), 76 revive_roll (combat.md Getting up after a collapse), 177 story_hit (StoryHit? 0x61D28) (all on host_ops
        // bit 63).
        let lib = (0..63).contains(&opc) && op::LIBRARY & (1u64 << opc) != 0
            || matches!(
                opc,
                op::ANIM_END
                    | op::ANIM_STEP
                    | op::ANIM_NEXT
                    | op::WALK_BLOCKED
                    | op::CONSUME_TRIGGER
                    | op::TAKE_GROUND_ITEM
                    | op::ENEMY_STEP
                    | op::STEP_NOISE
                    | op::THROW_FOLLOW
                    | op::AI_THINK
                    | op::ACTIVE_FRAME
                    | op::SOLDIER_FRAME
                    | op::ITQ_POP
                    | op::TURN_HAND_OVER
                    | op::ROUND_END
                    | op::MOVE_ORDER
                    | op::START_TURN
                    | op::TURN_REST
                    | op::AID_TICK
                    | op::GAS_TICK
                    | op::EXPLODE
                    | op::SECTOR_TICK
                    | op::EXPLODE_ART
                    | op::FIRST_AID
                    | op::SECTOR_WON
                    | op::KEPT_TURN_START
                    | op::NEW_TURN
                    | op::NEXT_TURN
                    | op::MERC_INTERRUPT
                    | op::KNOCKDOWN_ANIM
                    | op::LAUNCH
                    | op::PROJECTILE_STEP
                    | op::TAKE_DAMAGE
                    | op::BULLET_DAMAGE
                    | op::HIT_LOCATION
                    | op::SHOT_ALERT
                    | op::INTERRUPT_CHECK
                    | op::ASK_MERC_INTERRUPT
                    | op::ASK_INTERRUPT
                    | op::MELEE_HIT
                    | op::REVIVE_ROLL
                    | op::KILL
                    | op::MERC_DIES
                    | op::START_ATTACK
                    | op::ATTACK_GO
                    | op::GUN_JAM
                    | op::TURN_BANNER
                    | op::LOW_AP_STOP
                    | op::DOOR_FX
                    | op::CLOSE_DOOR
                    | op::BREAK_OBJECT
                    | op::DOOR_LOOKUP
                    | op::RELOADED
                    | op::RELOAD_CHECK
                    | op::MAKE_NOISE
                    | op::HEARS
                    | op::NOISE_INT
                    | op::LEAVE_SECTOR
                    | op::END_OF_DAY
                    | op::GUARDS_ARRIVE
                    | op::SIGHT_CHECK
                    | op::ENEMY_TURNED
                    | op::GUARD_TURNED
                    | op::CLEAR_ROOMS
                    | op::ENEMY_PICK
                    | op::ENEMY_MOVE
                    | op::GUARD_ENTER
                    | op::ENEMY_WALK_IN
                    | op::RESOLVE_BATTLE
                    | op::SECTOR_GAINED
                    | op::USE_COST
                    | op::EFFECT
                    | op::STAGE_EFFECTS
                    | op::STAGE_FIVE
                    | op::TRIGGER_ITEM
                    | op::USE_ON_TILE
                    | op::BURY_MINE
                    | op::PICKUP_HERE
                    | op::LAND_ITEM
                    | op::USE_SPECIAL
                    | op::QUOTE
                    | op::MINE_FLAG
                    | op::PICK_UP
                    | op::MINE_SPOTTED
                    | op::DISARM
                    | op::GROUND_ITEM_RESET
                    | op::TIMED_EFFECT
                    | op::CALL_ENEMIES
                    | op::TAKE_PATCH
                    | op::TOGGLE_DOORS
                    | op::STORY_HIT
                    | op::EFFECT_PASS
                    | op::EFFECT_HIT
            );
        // host_ops bit 63 stands for op 63 and every higher op (port_api.md).
        let hbit = if opc < 0 { 0 } else { 1u64 << opc.min(63) };
        let host_ops = unsafe { (*self.h).host_ops };
        if lib && host_ops & hbit == 0 && opc == op::STORY_HIT {
            // StoryHit? 0x61D28 takes no slot (storyfind.rs story_hit).
            storyfind::story_hit_op(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::SECTOR_WON {
            // SectorVictory? 0x3A0B0 takes no slot.
            update::sector_won(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::SECTOR_VALUE {
            // sector_value takes (byte sector, int flag), not a slot.
            let n = args.first().copied().unwrap_or(0) as u8;
            let fl = args.get(1).copied().unwrap_or(0) as u8;
            return self.sector_value(n, fl) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::RESOLVE_BATTLE {
            // strategic.md Sector battles: op 218 (byte sector, byte f), no slot.
            let s = args.first().copied().unwrap_or(0) as u8;
            let f = args.get(1).copied().unwrap_or(0) as u8;
            battle::resolve_battle_op(self.w, self.h, s, f);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::SECTOR_GAINED {
            // strategic.md Sector battles: op 201 sector_gained (s8 sector), no slot.
            battle::sector_gained_op(self.w, self.h, args.first().copied().unwrap_or(0) as u8);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::ENEMY_PICK {
            // enemy_pick takes no slot (strategic.md Enemy moves).
            return enemy::enemy_pick(self.w, self.h) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::ENEMY_MOVE {
            // enemy_move takes a byte sector, not a slot (strategic.md Enemy moves).
            let n = args.first().copied().unwrap_or(0) as u8;
            return enemy::enemy_move(self.w, self.h, n) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::GUARD_ENTER {
            // guard_enter takes the edge, not a slot (strategic.md Walk-ins).
            return crate::sector::guard_enter(self.w, self.h, args.first().copied().unwrap_or(0)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::ENEMY_WALK_IN {
            // enemy_walk_in takes (slot, edge) (strategic.md Walk-ins).
            let a = |i: usize| args.get(i).copied().unwrap_or(0);
            return crate::sector::enemy_walk_in(self.w, self.h, a(0), a(1)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::CONNECTED_HOME {
            // connected_home takes a byte sector, not a slot (soldier_update.md Joined to home).
            let n = args.first().copied().unwrap_or(0);
            return home::connected_home(self.w, n) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::MISSION_CANCEL {
            // mission_cancel takes no slot (MissionCancel 0x72670, sector.rs).
            crate::sector::ja1_mission_cancel(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::RADIO_RESULT {
            // radio_result takes no slot (sector_start.md batch 10).
            return crate::sector::ja1_radio_result(self.w, self.h) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::QUEST_CHECK {
            // quest_check takes an int quest, not a slot (sector_start.md batch 11).
            let q = args.first().copied().unwrap_or(0);
            return crate::sector::ja1_quest_check(self.w, self.h, q) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::TAKE_GROUND_ITEM {
            // take_ground_item takes (int tile, int keep), not a slot (sector_start.md batch 4).
            let t = args.first().copied().unwrap_or(0);
            let keep = args.get(1).copied().unwrap_or(0);
            crate::sector::ja1_take_ground_item(self.w, self.h, t, keep);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::DIRECTION {
            // direction takes four ints (pixel points), no slot (soldier_update.md Direction).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return update::direction(g(0), g(1), g(2), g(3)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::HIT_TEST {
            // hit_test takes no slot (shooting.md batch 5); the out record gives the starting skip / chance.
            let mut none = [0u8; 0];
            return self.hit_test_op(args, out.unwrap_or(&mut none)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::TILE_DIST {
            // tile_dist takes two tiles, no slot (soldier_update.md Tile distance).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return update::tile_dist(g(0), g(1)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::TURN_BANNER {
            // turns.md: op 123 turn_banner (Turn start) takes no arguments.
            turns::turn_start(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::ITQ_POP | op::TURN_HAND_OVER | op::ROUND_END) {
            // turns.md: ops 131, 132, 133 take no arguments.
            return match opc {
                op::ITQ_POP => turns::itq_pop(self.w, self.h) as u32,
                op::TURN_HAND_OVER => {
                    turns::mercs_turn(self.w, self.h);
                    0
                }
                _ => {
                    turns::enemies_turn(self.w, self.h);
                    0
                }
            };
        }
        if lib && host_ops & hbit == 0 && opc == op::SECTOR_TICK {
            // turns.md Sector tick: op 160 (none).
            blast::sector_tick(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::EFFECT_PASS {
            blast::effect_pass(self.w, self.h, args.first().copied().unwrap_or(0x13));
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::EFFECT_HIT {
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            blast::effect_hit_op(self.w, self.h, g(0), g(1), g(2) as u8 as i8);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::EXPLODE {
            // combat.md Explosion, exact steps: op 90 explode (byte thrower, tile, int kind, int hit); the thrower
            // may be 0x40 and up (no soldier), so it does not take the slot path.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            blast::explode(self.w, self.h, g(0) as u8, g(1), g(2), g(3));
            return 0;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::EFFECT | op::STAGE_EFFECTS | op::STAGE_FIVE) {
            // turns.md: op 91 effect (tile, byte kind, byte timer, int power, byte owner, byte flag) -> 1 when a free
            // entry took it; op 169 stage_effects and op 170 stage_five (none). No slot path.
            return match opc {
                op::EFFECT => turns::effect_start(self.w, self.h, args),
                op::STAGE_EFFECTS => {
                    turns::stage_effects(self.w, self.h);
                    0
                }
                _ => {
                    turns::stage_five(self.w, self.h);
                    0
                }
            };
        }
        if lib && host_ops & hbit == 0 && opc == op::USE_ON_TILE {
            // combat.md Using the hand item on a tile, exact steps: op 74 (slot, tile, byte flag).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            usetile::use_on_tile(self.w, self.h, g(0), g(1), g(2) as u8);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::BURY_MINE {
            // combat.md Burying a mine, exact steps: op 75 (slot, tile).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            usetile::bury_mine(self.w, self.h, g(0), g(1));
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::MINE_FLAG {
            // combat.md A detected mine gets a blue flag, exact steps: op 118 (slot, tile).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            usetile::mine_flag(self.w, self.h, g(0), g(1) as i16 as i32);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::QUOTE {
            // combat.md The listening device at a door, exact steps: op 67 (slot, tile).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            usetile::walter(self.w, self.h, g(0), g(1) as i16 as i32);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::USE_SPECIAL {
            // combat.md Using the lock kit on the mission safe, exact steps: op 69 (slot).
            usetile::use_special(self.w, self.h, args.first().copied().unwrap_or(0));
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::LAND_ITEM {
            // combat.md A thrown item lands, exact steps: op 176 (slot, tile, byte item, int condition, int x, int y).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return usetile::land_item(self.w, self.h, g(0), g(1), g(2) as u8, g(3), g(4), g(5)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::PICKUP_HERE {
            // combat.md Picking up at the own tile, exact steps: op 73 (slot).
            usetile::pickup_here(self.w, self.h, args.first().copied().unwrap_or(0));
            return 0;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::MINE_SPOTTED | op::DISARM | op::GROUND_ITEM_RESET) {
            // combat.md Mine spotted (slot, tile, byte g) -> byte; Disarming an explosive (slot, tile, byte g, byte
            // armed, byte menu) -> int; sector_start.md Item reset on ground entry g (byte g).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return match opc {
                op::MINE_SPOTTED => usetile::mine_spotted(self.w, self.h, g(0), g(1), g(2) as u8) as u32,
                op::DISARM => usetile::disarm(self.w, self.h, g(0), g(1), g(2) as u8, g(3) as u8, g(4) as u8) as u32,
                _ => {
                    usetile::ground_item_reset(self.w, self.h, g(0) as u8);
                    0
                }
            };
        }
        if lib && host_ops & hbit == 0 && opc == op::PICK_UP {
            // combat.md Picking up a ground item, exact steps: op 343 (slot, tile, byte cursor).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            usetile::pick_up(self.w, self.h, g(0), g(1), g(2) as u8);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::TRIGGER_ITEM {
            // combat.md Springing a ground item, exact steps: op 108 (slot, tile, byte g, byte armed) -> int.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return blast::trigger_item(self.w, self.h, g(0), g(1), g(2) as u8, g(3) as u8) as u32;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::MISSION_BLAST | op::TIMED_EFFECT) {
            // combat.md Mission blast: op 32 (int power); sector_start.md Ground blast: op 150 (byte type, int flag,
            // int power, byte thrower, int last). No slot path.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            if opc == op::MISSION_BLAST {
                blast::mission_blast(self.w, self.h, g(0));
            } else {
                crate::sector::ja1_ground_blast(self.w, self.h, g(0), g(1), g(2), g(3), g(4));
            }
            return 0;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::CALL_ENEMIES | op::TAKE_PATCH | op::TOGGLE_DOORS) {
            // combat.md Enemy call (tile, int kind), Taking a patch (tile, byte type), Toggling doors by value (byte
            // mode, byte value). No slot path.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            match opc {
                op::CALL_ENEMIES => blast::call_enemies(self.w, g(0)),
                op::TAKE_PATCH => crate::sector::ja1_take_ground_patch(self.w, self.h, g(0), g(1) as u8 as i32),
                _ => doors::toggle_doors(self.w, self.h, g(0) as u8, g(1) as u8),
            }
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::EXPLODE_ART {
            // combat.md Craters and blast leftovers: op 336 explode_art (tile, int big).
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            explode::explode_dat(self.w, self.h, g(0), g(1));
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::GAS_TICK {
            // combat.md Blast, exact steps: op 161 gas_tick (none).
            blast::gas_tick(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::KEPT_TURN_START {
            turns::kept_turn_start(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::NEW_TURN | op::NEXT_TURN | op::MERC_INTERRUPT) {
            // turns.md: op 134 new_turn (int focus), op 114 next_turn (int mode), op 113 merc_interrupt (none).
            let a0 = args.first().copied().unwrap_or(0);
            return match opc {
                op::NEW_TURN => turns::new_turn(self.w, self.h, a0) as u32,
                op::NEXT_TURN => {
                    turns::next_turn(self.w, self.h, a0);
                    0
                }
                _ => {
                    turns::interrupt_turn(self.w, self.h);
                    0
                }
            };
        }
        if lib && host_ops & hbit == 0 && matches!(opc, op::DOOR_LOOKUP | op::BREAK_OBJECT | op::CLOSE_DOOR) {
            // combat.md Doors: op 179 door_lookup (tile), 107 break_object and 104 close_door (tile, byte object,
            // int sound) take no slot.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            return match opc {
                op::DOOR_LOOKUP => doors::door_lookup(self.w, g(0)) as u32,
                op::BREAK_OBJECT => {
                    doors::break_object(self.w, self.h, g(0), g(1) as u8, g(2));
                    0
                }
                _ => {
                    doors::close_door(self.w, self.h, g(0), g(1) as u8, g(2));
                    0
                }
            };
        }
        if lib && host_ops & hbit == 0 && opc == op::MAKE_NOISE {
            // ai.md Make noise: op 127 (byte src, tile, byte class, int loudness, byte kind); src may be 0x40 and up
            // (no soldier), so it does not take the slot path.
            let g = |k: usize| args.get(k).copied().unwrap_or(0);
            noise::make_noise(self.w, self.h, g(0) as u8, g(1), g(2) as u8, g(3), g(4) as u8);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::LEAVE_SECTOR {
            // soldier_update.md Leaving the sector: op 214 (int flag), no slot.
            leave::leave_sector(self.w, self.h, args.first().copied().unwrap_or(0));
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::END_OF_DAY {
            // soldier_update.md End of day: op 217, no arguments.
            leave::end_of_day(self.w, self.h);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::GUARDS_ARRIVE {
            // turns.md The day clock: op 159 (int minutes), no slot.
            return clock::day_clock(self.w, self.h, args.first().copied().unwrap_or(0)) as u32;
        }
        if lib && host_ops & hbit == 0 && opc == op::CLEAR_ROOMS {
            // ai.md Sightings as steps: op 327 clear_rooms takes no arguments.
            sightings::clear_rooms(self.w);
            return 0;
        }
        if lib && host_ops & hbit == 0 && opc == op::CONSUME_TRIGGER {
            // consume_trigger takes an int index, not a slot (soldier_update.md Consume trigger).
            let i = args.first().copied().unwrap_or(0);
            update::consume_trigger(self.w, i);
            return 0;
        }
        if lib && host_ops & hbit == 0 {
            // SPEC GAP: a library op whose slot argument is not 0..47 goes to the host (never happens from
            // the library's own calls, which always pass the soldier's slot).
            if let Some(&slot) = args.first() {
                if let Some(sub) = Ai::new(self.w, self.h, slot) {
                    let mut none = [0u8; 0];
                    return sub.lib_op(opc, args, out.unwrap_or(&mut none));
                }
            }
        }
        unsafe {
            let h = &*self.h;
            match h.call {
                Some(c) => match out {
                    Some(o) => c(h.ctx, opc, args.as_ptr(), args.len() as i32, o.as_mut_ptr(), o.len() as i32),
                    None => c(h.ctx, opc, args.as_ptr(), args.len() as i32, std::ptr::null_mut(), 0),
                },
                None => 0,
            }
        }
    }
    /// cdiv(n, d) with the "Division by zero" rule (port_api.md): d = 0 makes host call 47 div_zero (no args),
    /// then the quotient is 0.
    fn cdiv(&self, n: i32, d: i32) -> i32 {
        if d == 0 {
            self.call(op::DIV_ZERO, &[]);
            0
        } else {
            n.wrapping_div(d)
        }
    }
    fn call_out(&self, op: i32, args: &[i32], out: &mut [u8]) -> u32 {
        self.dispatch(op, args, Some(out))
    }
    fn call(&self, op: i32, args: &[i32]) -> u32 {
        self.dispatch(op, args, None)
    }
    /// A host call whose result is a tile (low 16 bits).
    fn tile_call(&self, op: i32, args: &[i32]) -> u16 {
        self.call(op, args) as u16
    }
    fn option(&self, op: i32) -> Opt {
        // Empty option: not valid, slot 0x40, tile none.
        let mut o = [0u8; 10];
        o[1] = NO_SLOT;
        o[8..10].copy_from_slice(&NONE.to_le_bytes());
        self.call_out(op, &[self.slot()], &mut o);
        Opt {
            valid: o[0] != 0,
            slot: o[1],
            aim: o[2],
            chance: o[3],
            score: i32::from_le_bytes([o[4], o[5], o[6], o[7]]),
            tile: u16::from_le_bytes([o[8], o[9]]),
        }
    }
    fn swap_hands(&self) {
        self.call(op::SWAP_HAND, &[self.slot(), 0, 6]);
    }
    /// Host direction between the pixel points of two tiles: x = (t % 56) × 16 + 5, y = (t / 56) × 14 + 5 on the
    /// signed 16-bit tile (truncating / and %).
    fn direction_to(&self, to: u16) -> i32 {
        let (a, b) = (t16(self.own_tile()), t16(to));
        let args = [(a % 56) * 16 + 5, (a / 56) * 14 + 5, (b % 56) * 16 + 5, (b / 56) * 14 + 5];
        (self.call(op::DIRECTION, &args) & 0xFF) as i32
    }
    fn tile_dist(&self, a: u16, b: u16) -> i32 {
        self.call(op::TILE_DIST, &[t16(a), t16(b)]) as i32
    }
    fn move_ap(&self) -> i32 {
        self.call(op::MOVE_AP, &[self.slot()]) as i32
    }
    fn news(&self) -> i32 {
        (self.call(op::NEWS_VALUE, &[self.slot()]) & 0xFF) as i32
    }
    fn opp_strength(&self) -> i32 {
        (self.call(op::OPP_STRENGTH, &[self.slot()]) & 0xFF) as i32
    }
    fn attack_ready(&self, tile: u16) -> i32 {
        self.call(op::ATTACK_READY, &[self.slot(), t16(tile)]) as u8 as i8 as i32
    }
    /// attack_ready, and on −6 or −5 swap the hands and ask again.
    fn attack_ready_swap(&self, tile: u16) -> i32 {
        let r = self.attack_ready(tile);
        if r == -6 || r == -5 {
            self.swap_hands();
            return self.attack_ready(tile);
        }
        r
    }
    fn shout_shift(&self) -> i32 {
        (match self.orders() { 0 => 20, 2 => -10, 3 => 10, 4 => -20, _ => 0 })
            + match self.attitude() { 0 => 20, 1 => -10, 3 => -5, 5 => -20, _ => 0 }
    }

    // ================= One tick =================
    fn tick(&self) {
        // 1.
        if self.s < 8 {
            return;
        }
        // 2.
        if self.w_turn_based() == 0 && (self.w_busy() > 0 || self.w_active_slot() < NO_SLOT) {
            return;
        }
        // 3. Type 2; type 4 (Santino).
        let ty = self.s8(f::AI_TYPE);
        if ty == 2 {
            self.wr8(f::TURN_DONE, 1);
            return;
        }
        if ty == 4 {
            let spoke = unsafe { (*self.w).santino_spoke != 0 };
            if self.s8(f::B09C) > 0 && !spoke {
                // (his timer is turn_timer2: the Loop head counts it up while the screen is not drawn)
                if unsafe { (*self.w).turn_timer2 } < 20 {
                    return;
                }
                Ai::enemy_panel(self.w, self.h);
                self.call(op::DELAY, &[1500]);
                self.call(op::SANTINO_SPEECH, &[]);
                self.call(op::DELAY, &[300]);
                unsafe {
                    (*self.w).santino_spoke = 1;
                    (*self.w).santino_word = 0;
                }
            }
            self.wr8(f::TURN_DONE, 1);
            return;
        }
        // 4.
        if self.rd8(f::IN_CUR_SECTOR) == 0
            || self.rd8(f::IN_PLAY) == 0
            || self.s8(f::HP) < 15
            || (self.anim() < 4 && self.energy() < 10)
        {
            self.wr8(f::TURN_DONE, 1);
            return;
        }
        // 5.
        if self.rd8(f::B197) != 0 || self.anim_col(self.anim(), 0) == 0 {
            return;
        }
        // 6.
        if self.anim() < 4 {
            if self.ap() < 7 {
                self.wr8(f::TURN_DONE, 1);
            }
            return;
        }
        // 7.
        self.call(op::PERCEIVE, &[self.slot(), 0]);
        self.update_alert();
        // 8.
        if self.rd8(f::RETHINK) != 0 {
            self.wr8(f::RETHINK, 0);
            self.wr8(f::CALM_BONUS, 0);
            if self.rd8(f::AI_ACTION) != 0 {
                self.wr16(f::WALK_TARGET, NONE);
                self.wr8(f::B1B8, 1);
                self.wr8(f::AI_ACTION, 0);
                self.set_target(NONE);
                self.wr8(f::ACTION_RUNNING, 0);
                self.wr8(f::TURN_TO, self.rd8(f::FACING));
                self.wr16(f::STEP_TILE, self.own_tile());
                self.wr16(f::X, self.rd16(f::X2));
                self.wr16(f::Y, self.rd16(f::Y2));
            }
        }
        // 9.
        if self.rd8(f::B1B8) == 0 {
            return;
        }
        // 10. An action still runs.
        if self.rd8(f::AI_ACTION) != 0 && self.rd8(f::ACTION_RUNNING) != 0 {
            let wt = self.rd16(f::WALK_TARGET);
            if wt != NONE && self.rd16(f::STEP_TILE) != wt {
                self.call(op::WALK_TO, &[self.slot(), t16(wt)]);
                if self.rd16(f::STEP_TILE) != self.rd16(f::WALK_TARGET) {
                    self.wr8(f::RETHINK, 1);
                }
                return;
            }
            let code = self.rd8(f::AI_ACTION);
            let target = self.rd16(f::AI_TARGET);
            if code == 1 && self.s8(f::TURN_TO) != target as i16 as i32 {
                self.wr8(f::TURN_TO, target as u8);
                return;
            }
            let mut ticks = self.rd16(f::ACTION_TICKS);
            if code == 14 && self.occupant(target) == NO_SLOT {
                ticks = 350;
            }
            if self.w_turn_based() != 0 {
                ticks = 350;
            }
            // SPEC GAP: actionTicks is an unsigned 16-bit counter (wraps, compared unsigned).
            ticks = ticks.wrapping_add(1);
            self.wr16(f::ACTION_TICKS, ticks);
            if ticks < 350 {
                return;
            }
            if is_attack(code) {
                self.call(op::FIRE, &[self.slot()]);
                // #70 F36 (row 34): 1.15m AiThink 0x6D4DE jumps to 0x6D47A, which stores edx (0, the cdq of
                // 0x6D48A) into 0x92DAC: turn-based mode ends after the shot forced by the action timeout.
                if crate::fanfix::on(36) {
                    unsafe { (*self.w).turn_based = 0 };
                }
            } else {
                self.wr8(f::AP_LEFT, 0);
                self.wr8(f::TURN_DONE, 1);
            }
            self.wr16(f::ACTION_TICKS, 0);
            self.wr8(f::RETHINK, 1);
            return;
        }
        // 11.
        self.wr16(f::ACTION_TICKS, 0);
        if self.rd8(f::AI_ACTION) == 0 {
            self.wr8(f::ACTION_RUNNING, 0);
            let code = self.choose(0) as u8;
            self.wr8(f::AI_ACTION, code);
            if code == 0 {
                self.wr8(f::TURN_DONE, 1);
                return;
            }
        }
        // 12.
        if !self.affordable() {
            self.wr8(f::TURN_DONE, 1);
            return;
        }
        // 13.
        Ai::enemy_panel(self.w, self.h);
        let r = self.start();
        self.wr8(f::ACTION_RUNNING, r as u8);
    }

    // ================= Alert state =================
    fn update_alert(&self) {
        let old = self.s8(f::ALERT);
        let ty = self.rd8(f::AI_TYPE);
        let engaged = self.s8(f::B1D5) > 0;
        match ty {
            0..=2 => {
                if engaged {
                    self.wr8(f::ALERT, 3);
                } else if self.enemies_left() != 0 {
                    self.wr8(f::ALERT, 2);
                } else {
                    self.wr8(f::ALERT, 0);
                    if ty == 1 && old >= 2 {
                        let t1 = if self.rd16(f::HOME_TILE) == NONE { 3 } else { 1 };
                        self.wr8(f::ORDERS, t1);
                        self.wr8(f::CALM_BONUS, 0);
                        self.wr8(f::B1D7, 0);
                        self.call(op::IDLE, &[self.slot()]);
                    }
                }
            }
            3 | 4 => {
                let forced = || self.w_alarm() || self.rd8(f::B1C4) != 0;
                let noise = || self.tile_call(op::NOISE_TILE, &[self.slot()]) != NONE;
                let new = if engaged {
                    3
                } else {
                    match old {
                        3 => 2,
                        0 => if forced() { 2 } else if noise() { 1 } else { 0 },
                        1 => {
                            if forced() {
                                2
                            } else if !noise() && self.rd8(f::ACTION_RUNNING) == 0 {
                                0
                            } else {
                                1
                            }
                        }
                        o => o,
                    }
                };
                self.wr8(f::ALERT, new as u8);
            }
            _ => {}
        }
        let new = self.s8(f::ALERT);
        if old == new {
            let e = self.energy();
            if ((new == 0 && e < 75) || (new == 1 && e < 50)) && !self.in_water() {
                self.wr8(f::RETHINK, 1);
            }
        } else {
            if old < 2 || new < 2 {
                self.wr8(f::RETHINK, 1);
            }
            let t1 = self.orders();
            if old < 2 && new >= 2 && t1 < 2 {
                let keep = self.rd8(f::AI_TYPE) == 3 && t1 == 0 && self.s8(f::EXPLOSIVES) == 99;
                if !keep {
                    self.wr8(f::ORDERS, t1 + 1);
                }
            }
        }
    }

    // ================= Choosing an action =================
    fn choose(&self, fallback: i32) -> i32 {
        if self.calm_bonus() != 0 && self.s8(f::ALERT) < 3 {
            return self.calm();
        }
        match self.rd8(f::ALERT) {
            0 => self.calm(),
            1 => self.alerted(),
            2 => self.combat(),
            3 => self.engaged(),
            _ => fallback,
        }
    }

    /// "AP cost": affordable when the cost (signed byte) ≤ apLeft (signed byte).
    fn affordable(&self) -> bool {
        let code = self.rd8(f::AI_ACTION);
        let cost: i32 = match code {
            1 => 1,
            c if is_walk(c) => self.move_ap() as u8 as i8 as i32,
            6 | 7 => 5,
            9 | 10 => 2,
            c if is_attack(c) => {
                let t = self.rd16(f::AI_TARGET);
                self.call(op::ACTION_AP, &[self.slot(), t16(t), 1]) as u8 as i8 as i32
            }
            _ => 0,
        };
        cost <= self.ap()
    }

    // ---- Calm ----
    fn calm(&self) -> i32 {
        // 1.
        if !(self.w_combat()
            || self.rd8(f::B1AB) != 0
            || self.rd8(f::B1D7) != 0
            || self.in_water()
            || self.in_gas())
            && self.random(10) != 0
        {
            self.call(op::IDLE, &[self.slot()]);
            return self.nothing();
        }
        // 2.
        if self.in_water() || self.in_gas() {
            if let Some(c) = self.walk(self.tile_call(op::FIND_DRY_TILE, &[self.slot()]), 2) {
                return c;
            }
        }
        // 3.
        if !(self.energy() >= 75 || self.rd8(f::B1D7) != 0 || self.in_water()) {
            return self.nothing();
        }
        // 4. Patrol.
        let c = self.calm_bonus()
            + 25
            + (match self.orders() { 0 => -20, 1 => 15, 2 => 25, 4 => -10, _ => 0 })
            + (match self.attitude() { 0 => -10, 1 | 3 => 5, 5 => 10, _ => 0 })
            - self.tired();
        let forced = self.rd8(f::B1AB) != 0 || self.rd8(f::B1D7) != 0 || self.in_water();
        if forced || self.roll(c) {
            let t = self.tile_call(op::PATROL_TILE, &[self.slot()]);
            self.set_target(t);
            if t != NONE {
                self.wr8(f::B1A8, 0);
                self.wr8(f::B1D7, 0);
                return 2;
            }
        }
        // 5. Buddy.
        let mut c = self.calm_bonus() + 25 + match self.orders() { 0 => -20, 1 => 10, 2 => 20, 4 => -10, _ => 0 };
        match self.attitude() {
            1 | 3 => c = (c as i16 / 2) as i32,
            2 | 4 => c += 10,
            _ => {}
        }
        c -= self.tired();
        if self.roll(c) && self.call(op::BUDDY_TILE, &[self.slot()]) & 0xFF != 0 {
            self.wr8(f::B1A8, 0);
            return 3;
        }
        // 6. Turn.
        let c = self.calm_bonus()
            + 25
            + if self.orders() == 0 { 25 } else { 0 }
            + if self.attitude() == 0 { 25 } else { 0 };
        if self.roll(c) {
            // SPEC GAP: the spec repeats without limit; a host whose random never leaves the facing would hang,
            // so the loop stops after 100000 tries (direction then kept as drawn).
            for _ in 0..100_000 {
                let post = self.s8(f::POST_FACING);
                let dir = if (1..=8).contains(&post) && post != self.facing() && self.random(2) != 0 {
                    post
                } else {
                    self.random(8) + 1
                };
                self.set_target(dir as u16);
                if self.facing() != dir {
                    return 1;
                }
            }
            return 1;
        }
        // 7.
        self.nothing()
    }

    // ---- Alerted ----
    fn alerted(&self) -> i32 {
        // 1.
        if self.s8(f::AI_TYPE) != 3 {
            return 0;
        }
        // 2.
        let mut out = [0u8; 4];
        let noise = self.call_out(op::NOISE_TILE, &[self.slot()], &mut out) as u16;
        let loud = i32::from_le_bytes(out);
        if noise == NONE {
            return 0;
        }
        // 3.
        let d = self.direction_to(noise);
        if self.facing() != d && self.tile_dist(self.own_tile(), noise) <= 14 {
            let c = (if self.orders() != 0 { 35 } else { 60 }) + if self.attitude() == 0 { 15 } else { 0 };
            if self.roll(c) {
                self.set_target(d as u16);
                return 1;
            }
        }
        // 4. Shout for help.
        if self.s >= 0x28 && self.ap() >= 5 {
            let c = self.news() * 5;
            if c != 0 && self.rd8(f::GROUND_UNDER) < 0x9A {
                let sector = self.w_cur_sector() as i32;
                let v = self.call(op::SECTOR_VALUE, &[sector, 1]) as u16 as i16 as i32;
                let v = if v == -100 { 0 } else { v.min(100) };
                if self.roll(c + v / 2 + self.shout_shift()) {
                    return 6;
                }
            }
        }
        // 5.
        if !(self.energy() >= 50 || self.in_water()) {
            return self.nothing();
        }
        // 6. Walk toward the noise ("tired" = 100 − energy here).
        let c = loud
            + 75
            + (match self.orders() { 0 => -20, 1 => -10, 2 => 10, 4 => 25, _ => 0 })
            + (match self.attitude() { 0 => -10, 1 => 10, 2 | 3 => 5, 5 => 20, _ => 0 })
            - (100 - self.energy());
        if self.roll(c) {
            if let Some(c) = self.walk(self.tile_call(op::APPROACH_TILE, &[self.slot(), t16(noise)]), 2) {
                return c;
            }
        }
        // 7. Walk toward a calling ally.
        let a = self.tile_call(op::ALLY_CALL_TILE, &[self.slot()]);
        if a != NONE {
            let c = 50 - self.tile_dist(self.own_tile(), a)
                + (match self.orders() { 0 => -20, 1 => -10, 3 => 20, 4 => 10, _ => 0 })
                + (match self.attitude() { 0 => -10, 2 | 4 => 20, 5 => -20, _ => 0 })
                - (100 - self.energy());
            if self.roll(c) {
                if let Some(c) = self.walk(self.tile_call(op::APPROACH_TILE, &[self.slot(), t16(a)]), 3) {
                    return c;
                }
            }
        }
        // 8. Search.
        let c = (match self.orders() { 0 => 45, 1 => 35, 2 => 20, 4 => 5, _ => 25 })
            + (match self.attitude() { 0 => 10, 1 => -15, 2 => -20, 3 => 20, 4 => 15, 5 => -10, _ => 0 })
            - (100 - self.energy());
        if self.roll(c) {
            if let Some(c) = self.walk(self.tile_call(op::SEARCH_TILE, &[self.slot(), 0]), 5) {
                return c;
            }
        }
        // 9.
        if self.random(100) < 50 {
            self.wr8(f::CALM_BONUS, 15);
            return self.calm();
        }
        // 10.
        self.nothing()
    }

    // ---- Combat ----
    fn combat(&self) -> i32 {
        // 1.
        if self.rd8(f::AP_LEFT) == 0 {
            return self.nothing();
        }
        // 2.
        let can_walk = self.ap() >= self.move_ap();
        // 3. Scripted mission.
        if self.s >= 0x28 && self.w_mission_step() > 0 {
            if !self.mission_done() && (self.item(1) == 0x40 || self.item(0) == 0x40) {
                if self.ap() >= 2 {
                    return 10;
                }
                return self.nothing();
            }
            if self.w_mission_slot() as usize == self.s && self.w_mission_target() != NONE {
                if let Some(c) = self.mission(can_walk) {
                    return c;
                }
            }
        }
        // 4.
        if self.energy() < 25 && !self.in_water() && self.rd8(f::B1C4) == 0 {
            return self.nothing();
        }
        // 5.
        let s = self.opp_strength();
        if s == 0 && can_walk {
            if let Some(c) = self.walk(self.tile_call(op::RETREAT_TILE, &[self.slot()]), 8) {
                return c;
            }
        }
        // 6. Raise the alarm.
        if self.s >= 0x28 && self.ap() >= 5 {
            let c = (self.news() * 10) as i16 as i32;
            if c != 0 && self.rd8(f::GROUND_UNDER) < 0x9A {
                let c = if !self.w_alarm() {
                    // SPEC GAP: a difficulty above 2 uses the difficulty 2 chance (95).
                    match self.w_difficulty() { 0 => 50, 1 => 80, _ => 95 }
                } else {
                    let c = (c + self.shout_shift()) as i16 as i32;
                    match s {
                        0 => (c * 3) as i16 as i32,
                        1 => (c * 2) as i16 as i32,
                        3 => c / 2,
                        4 => c / 3,
                        _ => c,
                    }
                };
                if self.roll(c) {
                    return 7;
                }
            }
        }
        // 7. Advance / approach / attack position.
        if can_walk {
            let base: [i8; 3] = match s { 0 => [-99, -99, 1], 1 => [-1, 0, 1], 3 | 4 => [1, 0, -1], _ => [0, 0, 0] };
            let a: [i8; 3] = match self.orders() { 0 => [-1, -1, 1], 3 => [0, 1, -1], 4 => [1, 0, -1], _ => [0; 3] };
            let b: [i8; 3] = match self.attitude() {
                0 => [-1, 0, 1],
                1 => [1, -1, -1],
                2 => [1, 1, -1],
                3 => [0, -1, 1],
                4 => [0, 1, 1],
                5 => [1, 0, -1],
                _ => [0; 3],
            };
            let mut wt = [0i8; 3];
            for i in 0..3 {
                wt[i] = base[i].wrapping_add(a[i]).wrapping_add(b[i]);
            }
            while !wt.iter().all(|&x| x <= -90) {
                for i in 0..3 {
                    let top = wt[i] > -90 && (0..3).all(|j| wt[i] >= wt[j]);
                    if !top {
                        continue;
                    }
                    let found = match i {
                        0 => {
                            let t = self.tile_call(op::ADVANCE_TILE, &[self.slot(), 1]);
                            if t != NONE {
                                self.walk(self.tile_call(op::APPROACH_TILE, &[self.slot(), t16(t)]), 4)
                            } else {
                                None
                            }
                        }
                        1 => {
                            let t = self.tile_call(op::ALLY_CALL_TILE, &[self.slot()]);
                            if t != NONE {
                                self.walk(self.tile_call(op::APPROACH_TILE, &[self.slot(), t16(t)]), 3)
                            } else {
                                None
                            }
                        }
                        _ => {
                            let t = self.tile_call(op::TARGET_TILE, &[self.slot()]);
                            if t != NONE {
                                let m = search_mode(s);
                                self.walk(self.tile_call(op::SEARCH_TILE, &[self.slot(), m]), 5)
                            } else {
                                None
                            }
                        }
                    };
                    if let Some(c) = found {
                        return c;
                    }
                    wt[i] = -99;
                }
            }
        }
        // 8. Turn toward the current opponent.
        let t = self.tile_call(op::TARGET_TILE, &[self.slot()]);
        if t != NONE {
            let d = self.direction_to(t);
            if self.facing() != d && self.tile_dist(self.own_tile(), t) <= 14 {
                let c = (if self.orders() != 0 { 25 } else { 50 }) + if self.attitude() == 0 { 25 } else { 0 };
                if self.roll(c) {
                    self.set_target(d as u16);
                    return 1;
                }
            }
        }
        // 9.
        if can_walk {
            self.wr8(f::CALM_BONUS, 30);
            return self.calm();
        }
        // 10.
        self.nothing()
    }

    /// Combat step 3, the soldier ordered to the mission target. Some(code) returns; None goes on with step 4.
    fn mission(&self, can_walk: bool) -> Option<i32> {
        let target = self.w_mission_target();
        let saved = self.rd8(f::B09C);
        self.wr8(f::B09C, 0xFF);
        // SPEC GAP: "a result of 0" = the whole 32-bit result is 0.
        let r = self.call(op::REACH_AP, &[self.slot(), t16(target), 0]);
        self.wr8(f::B09C, saved);
        if r != 0 {
            let spot = self.w_mission_spot();
            if spot == self.own_tile() {
                let target = self.w_mission_target();
                if self.w_mission_step() != 1 {
                    if self.ap() >= 2 {
                        self.set_target(target);
                        return Some(9);
                    }
                    return Some(self.nothing());
                }
                if self.attack_ready_swap(target) == 1 {
                    let k0 = self.item_kind(self.item(0));
                    if self.item_kind(self.item(1)) == KIND_KNIFE || !(k0 == KIND_GUN || k0 == KIND_KNIFE) {
                        self.swap_hands();
                    }
                    let k0 = self.item_kind(self.item(0));
                    if (k0 == KIND_GUN || k0 == KIND_KNIFE)
                        && self.call(op::ACTION_AP, &[self.slot(), t16(target), 1]) as i32 <= self.ap()
                    {
                        self.set_target(target);
                        return Some(16);
                    }
                }
                return Some(self.nothing());
            }
            if !can_walk {
                return Some(self.nothing());
            }
            if self.call(op::CAN_STAND_AT, &[self.slot(), t16(spot)]) & 0xFF != 0 {
                self.set_target(spot);
                return Some(13);
            }
        }
        // Cancel.
        unsafe { (*self.w).mission_slot = NO_SLOT; }
        self.call(op::MISSION_CANCEL, &[]);
        None
    }

    // ---- Engaged ----
    fn engaged(&self) -> i32 {
        // 1.
        if self.rd8(f::AP_LEFT) == 0 {
            return self.nothing();
        }
        if self.w_mission_slot() as usize == self.s {
            return self.combat();
        }
        // 2.
        let mut s = self.opp_strength();
        let can_walk = self.ap() >= self.move_ap();
        // 3.
        if self.in_gas() || self.energy() < 5 {
            if can_walk {
                if let Some(c) = self.walk(self.tile_call(op::RETREAT_TILE, &[self.slot()]), 8) {
                    return c;
                }
            }
            s = 0;
        }
        // 4.
        let mut r = self.attack_ready_swap(NONE);
        if self.in_gas() {
            r = 0;
        }
        let l = self.rd16(f::LAST_TARGET_TILE);
        // 5. Options and pick: (code, option, from the second hand).
        let mut pick: Option<(i32, Opt)> = None;
        if r == 1 {
            // Gun.
            if self.item_kind(self.item(0)) != KIND_GUN {
                self.swap_hands();
                if self.item_kind(self.item(0)) != KIND_GUN {
                    return self.nothing();
                }
            }
            let mut gun: Option<Opt> = None;
            if self.ap() >= self.action_ap_byte(l) {
                let o = self.option(op::GUN_OPTION);
                if o.valid {
                    gun = Some(o);
                }
                // SPEC GAP: the weak-target check runs inside the "AP cover a shot" branch (after gun_option).
                if self.s8_of(o.slot as usize, f::HP) < 15
                    && self.attitude() != 5
                    && self.tile_call(op::ADVANCE_TILE, &[self.slot(), 0]) != NONE
                {
                    return self.combat();
                }
            }
            let throw = self.hand_option(KIND_THROW, op::THROW_OPTION, l);
            let knife = self.hand_option(KIND_KNIFE, op::KNIFE_OPTION, l);
            let chosen = match (gun, throw, knife) {
                (Some(g), Some(t), _) => Some(if g.score < t.0.score { (12, t.0, t.1) } else { (11, g, false) }),
                (Some(g), None, Some(k)) => Some(if g.score < k.0.score { (14, k.0, k.1) } else { (11, g, false) }),
                (Some(g), None, None) => Some((11, g, false)),
                (None, Some(t), _) => Some((12, t.0, t.1)),
                (None, None, Some(k)) => Some((14, k.0, k.1)),
                (None, None, None) => None,
            };
            if let Some((code, o, second)) = chosen {
                if second {
                    self.swap_hands();
                }
                pick = Some((code, o));
            }
        }
        // 6.
        if let Some((code, o)) = pick {
            // SPEC GAP: no threshold is defined for s above 4; such a pick is not taken here.
            let need = match s { 0 => 75, 1 => 10, 2 | 3 => 25, 4 => 50, _ => i32::MAX };
            if o.chance as i32 >= need {
                return self.attack(code, o);
            }
        }
        // 7.
        if can_walk {
            if s == 0 {
                if let Some(c) = self.walk(self.tile_call(op::RETREAT_TILE, &[self.slot()]), 8) {
                    return c;
                }
            }
            let mode = if self.in_water() { 0 } else { search_mode(s) };
            if let Some(c) = self.walk(self.tile_call(op::SEARCH_TILE, &[self.slot(), mode]), 5) {
                return c;
            }
        }
        // 8.
        if let Some((code, o)) = pick {
            return self.attack(code, o);
        }
        // 9.
        if (self.in_water() || self.in_gas()) && can_walk {
            if let Some(c) = self.walk(self.tile_call(op::FIND_DRY_TILE, &[self.slot()]), 2) {
                return c;
            }
            if let Some(c) = self.walk(self.tile_call(op::RETREAT_TILE, &[self.slot()]), 8) {
                return c;
            }
            s = 0;
        }
        // 10.
        if (s > 0 || can_walk) && self.call(op::BUSY, &[self.slot()]) == 0 {
            let t = self.tile_call(op::TARGET_TILE, &[self.slot()]);
            if t != NONE {
                let d = self.direction_to(t);
                if self.facing() != d {
                    self.set_target(d as u16);
                    return 1;
                }
            }
        }
        // 11.
        self.nothing()
    }

    fn s8_of(&self, slot: usize, off: usize) -> i32 {
        self.rd8_of(slot, off) as i8 as i32
    }

    /// Low byte of action_ap(slot, tile, 0), unsigned (0..255).
    fn action_ap_byte(&self, tile: u16) -> i32 {
        (self.call(op::ACTION_AP, &[self.slot(), t16(tile), 0]) & 0xFF) as i32
    }

    /// Throw / knife option (Engaged step 5): main hand if its kind matches, else the second hand (swapped in
    /// and back), else none. Returns (option, from the second hand) when the option is valid.
    fn hand_option(&self, kind: u8, opcode: i32, l: u16) -> Option<(Opt, bool)> {
        let second = if self.item_kind(self.item(0)) == kind {
            false
        } else if self.item_kind(self.item(1)) == kind {
            true
        } else {
            return None;
        };
        if second {
            self.swap_hands();
        }
        let mut o = None;
        if self.ap() >= self.action_ap_byte(l) {
            let r = self.option(opcode);
            if r.valid {
                o = Some(r);
            }
        }
        if second {
            self.swap_hands();
        }
        o.map(|o| (o, second))
    }

    /// "Attack": set target the pick's tile, aim, attackSlot, return its code.
    fn attack(&self, code: i32, o: Opt) -> i32 {
        self.set_target(o.tile);
        self.wr8(f::AIM, o.aim);
        self.wr8(f::ATTACK_SLOT, o.slot);
        code
    }

    // ================= Starting the chosen action =================
    fn start(&self) -> i32 {
        let code = self.rd8(f::AI_ACTION);
        let target = self.rd16(f::AI_TARGET);
        let slot = self.slot();
        let end = || {
            self.set_target(NONE);
            self.wr8(f::AI_ACTION, 0);
            0
        };
        match code {
            0 => 1,
            1 => {
                if self.call(op::TURN, &[slot, (target & 0xFF) as i32, 30]) != 0 {
                    return 1;
                }
                self.wr8(f::AP_LEFT, 0);
                self.wr8(f::RETHINK, 1);
                self.wr8(f::AI_ACTION, 0);
                self.set_target(NONE);
                0
            }
            c if is_walk(c) => {
                self.call(op::WALK_TO, &[slot, t16(target)]);
                let target = self.rd16(f::AI_TARGET);
                if self.rd16(f::STEP_TILE) != target {
                    self.wr8(f::RETHINK, 1);
                    self.wr16(f::WALK_FAIL_TILE, target);
                    self.set_target(NONE);
                    self.wr8(f::AI_ACTION, 0);
                    return 0;
                }
                self.wr16(f::WALK_FAIL_TILE, NONE);
                self.wr8(f::B1AB, 0);
                1
            }
            c if is_attack(c) => {
                self.call(op::CLICK, &[slot, t16(target), self.item(0) as i32]);
                1
            }
            9 => {
                self.call(op::CLICK, &[slot, t16(target), 0]);
                end()
            }
            10 => {
                self.call(op::SPEND_AP, &[slot, 2, 0]);
                self.call(op::MISSION_BLAST, &[1000]);
                let m = self.call(op::RADIO_RESULT, &[]) as u8;
                unsafe { (*self.w).mission_step = m as i8; }
                if m as i8 != 1 {
                    unsafe { (*self.w).mission_slot = NO_SLOT; }
                }
                end()
            }
            6 | 7 => {
                if code == 7 && self.s >= 0x28 && !self.w_alarm() {
                    unsafe { (*self.w).alarm = 1; }
                    if self.w_mission_step() > 0 && self.w_mission_target() != NONE {
                        self.call(op::MISSION_CANCEL, &[]);
                    }
                }
                self.call(op::SPEND_AP, &[slot, 5, 0]);
                self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
                end()
            }
            _ => 0,
        }
    }
}

/// Shared tile helpers (ai.md "Shared definitions", "Standable tile", "Frame routine" helpers) for modules outside
/// `ai` (sector start). Each one views the world through an `Ai` on slot 0 (the slot is not used by these helpers);
/// a null world, host or soldier buffer gives 0 / no effect.
pub(crate) mod shared {
    use super::{Ai, Ja1Host, Ja1World};

    /// The buffer and index that byte i of the ground entries is in (combat.md Mine spotted, "G for g of 200 or more"):
    /// the 200 entries, then the 128-byte text buffer, then the 64 profiles (SPEC GAP: past them, none).
    fn ground_at(w: &Ja1World, i: usize) -> Option<(*mut u8, usize)> {
        const GROUND_LEN: usize = 200 * 12;
        let (p, len, i) = if i < GROUND_LEN {
            (w.ground_items, GROUND_LEN, i)
        } else if i - GROUND_LEN < 0x80 {
            (w.text_buf, 0x80, i - GROUND_LEN)
        } else {
            (w.profiles, 64 * 0x5A, i - GROUND_LEN - 0x80)
        };
        (!p.is_null() && i < len).then_some((p, i))
    }
    /// Byte i of the ground entries and of the bytes after them; SPEC GAP: a null buffer or a byte past the profiles
    /// reads 0.
    pub(crate) fn ground_byte(w: *const Ja1World, i: usize) -> u8 {
        if w.is_null() {
            return 0;
        }
        ground_at(unsafe { &*w }, i).map_or(0, |(p, i)| unsafe { *p.add(i) })
    }
    /// The write of `ground_byte`; SPEC GAP: a null buffer or a byte past the profiles is not written.
    pub(crate) fn ground_byte_put(w: *const Ja1World, i: usize, v: u8) {
        if w.is_null() {
            return;
        }
        if let Some((p, i)) = ground_at(unsafe { &*w }, i) {
            unsafe { *p.add(i) = v }
        }
    }

    /// step(t, k).
    pub(crate) fn step(t: i32, k: i32) -> i32 {
        super::search::step(t, k)
    }
    /// dirStep(d) = signed entry d of dir_steps.
    pub(crate) fn dir_step(w: *mut Ja1World, h: *const Ja1Host, d: i32) -> i32 {
        Ai::new(w, h, 0).map_or(0, |a| a.dir_step(d))
    }
    /// occupant(t): byte 5 & 0x7F when below 0x30, else 0x40.
    pub(crate) fn occupant(w: *mut Ja1World, h: *const Ja1Host, t: i32) -> u8 {
        Ai::new(w, h, 0).map_or(0, |a| {
            let o = a.tb(t, 5) & 0x7F;
            if o >= 0x30 { super::NO_SLOT } else { o }
        })
    }
    /// An op through the library's own dispatch (a library op runs here unless its host_ops bit is set).
    pub(crate) fn lib_call(w: *mut Ja1World, h: *const Ja1Host, opc: i32, args: &[i32]) -> u32 {
        Ai::new(w, h, 0).map_or(0, |a| a.call(opc, args))
    }
    /// tile_standable(t).
    pub(crate) fn tile_standable(w: *mut Ja1World, h: *const Ja1Host, t: i32) -> i32 {
        Ai::new(w, h, 0).map_or(0, |a| a.tile_standable(t))
    }
    /// t0_class[t0 of t].
    pub(crate) fn ground_class(w: *mut Ja1World, h: *const Ja1Host, t: i32) -> u8 {
        Ai::new(w, h, 0).map_or(0, |a| a.t0_class(a.tb(t, 0)))
    }
    /// Max AP (ai.md "Max AP") of record r (0..47). Its s8 maxHealth x 3 = 0 division makes host call 47 div_zero
    /// as everywhere else.
    pub(crate) fn max_ap(w: *mut Ja1World, h: *const Ja1Host, r: usize) -> i32 {
        Ai::new(w, h, r as i32).map_or(0, |a| a.max_ap(r))
    }
    /// Frame delay (ai.md "Frame delay", `ja1_anim_delay`) of record r, with r as the slot.
    pub(crate) fn frame_delay(w: *mut Ja1World, h: *const Ja1Host, r: usize) {
        if let Some(a) = Ai::new(w, h, r as i32) {
            a.anim_delay();
        }
    }
    /// Action AP (sector_start.md batch 12) of record r: a byte 0..255.
    pub(crate) fn action_ap(w: *mut Ja1World, h: *const Ja1Host, r: usize, tile: i32, flag: u8) -> i32 {
        Ai::new(w, h, r as i32).map_or(0, |a| a.action_ap(tile, flag))
    }
    /// Occupy(t, slot): 1 = done, 0 = fails.
    pub(crate) fn occupy(w: *mut Ja1World, h: *const Ja1Host, t: i32, slot: u8) -> bool {
        Ai::new(w, h, 0).is_some_and(|a| a.occupy_for(t, slot))
    }
}

fn run<T>(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dflt: T, f: impl FnOnce(&Ai) -> T) -> T {
    match Ai::new(w, h, slot) {
        None => dflt,
        Some(ai) => catch_unwind(AssertUnwindSafe(|| f(&ai))).unwrap_or(dflt),
    }
}

#[no_mangle]
pub extern "C" fn ja1_ai_tick(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.tick())
}
#[no_mangle]
pub extern "C" fn ja1_ai_update_alert(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.update_alert())
}
#[no_mangle]
pub extern "C" fn ja1_ai_choose_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32, fallback: i32) -> i32 {
    run(w, h, slot, fallback, |a| a.choose(fallback))
}
#[no_mangle]
pub extern "C" fn ja1_ai_calm_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.calm())
}
#[no_mangle]
pub extern "C" fn ja1_ai_alerted_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.alerted())
}
#[no_mangle]
pub extern "C" fn ja1_ai_combat_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.combat())
}
#[no_mangle]
pub extern "C" fn ja1_ai_engaged_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.engaged())
}
#[no_mangle]
pub extern "C" fn ja1_ai_action_affordable(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.affordable() as i32)
}
#[no_mangle]
pub extern "C" fn ja1_ai_start_action(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.start())
}

// ---- Search procedures (ai.md "Search procedures"); host_ops applies to their inner calls ----
// SPEC GAP: on a null world / host or a slot outside 0..47 the tile searches return NONE (0xC41), the others 0;
// *loudness is then left unchanged.
#[no_mangle]
pub extern "C" fn ja1_ai_noise_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, loudness: *mut u16) -> i32 {
    run(w, h, slot, NONE as i32, |a| {
        let (t, loud) = a.noise_tile();
        if !loudness.is_null() {
            unsafe { loudness.write_unaligned(loud) };
        }
        t as i32
    })
}
#[no_mangle]
pub extern "C" fn ja1_ai_find_dry_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.find_dry_tile())
}
#[no_mangle]
pub extern "C" fn ja1_ai_patrol_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.patrol_tile())
}
#[no_mangle]
pub extern "C" fn ja1_ai_buddy_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.buddy_tile())
}
#[no_mangle]
pub extern "C" fn ja1_ai_news_value(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.news_value())
}
#[no_mangle]
pub extern "C" fn ja1_ai_approach_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.approach_tile(tile))
}
#[no_mangle]
pub extern "C" fn ja1_ai_ally_call_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.ally_call_tile())
}
#[no_mangle]
pub extern "C" fn ja1_ai_can_stand_at(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, 0, |a| a.can_stand_at(tile))
}
#[no_mangle]
pub extern "C" fn ja1_ai_advance_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, weak: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.advance_tile(weak))
}
#[no_mangle]
pub extern "C" fn ja1_ai_target_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.target_tile())
}

// ---- Situation score (ai.md "Situation score"); inner calls (6, 9, 42) are always host calls ----
// SPEC GAP: on a null world / host or a slot outside 0..47 both return 0.
#[no_mangle]
pub extern "C" fn ja1_ai_opp_strength(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.situation_score())
}
#[no_mangle]
pub extern "C" fn ja1_ai_soldier_power(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) -> i32 {
    run(w, h, slot, 0, |a| a.soldier_power(a.s, tile as i16 as i32, flag as u8))
}

// ---- Positions and sector value (ai.md); host_ops applies to inner library ops (18) ----
// SPEC GAP: on a null world / host or a slot outside 0..47 the tile searches return NONE (0xC41).
#[no_mangle]
pub extern "C" fn ja1_ai_retreat_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.retreat_tile())
}
#[no_mangle]
pub extern "C" fn ja1_ai_search_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, mode: i32) -> i32 {
    run(w, h, slot, NONE as i32, |a| a.search_tile(mode))
}
// ---- Attack options (ai.md); the 10-byte option record is written to out ----
// SPEC GAP: on a null world / host or a slot outside 0..47, out is left unchanged; a null out writes nothing.
#[no_mangle]
pub extern "C" fn ja1_ai_gun_option(w: *mut Ja1World, h: *const Ja1Host, slot: i32, out: *mut u8) {
    run(w, h, slot, (), |a| {
        let b = a.gun_option().bytes();
        if !out.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), out, 10) };
        }
    })
}
// ---- Line of fire (ai.md "Line of fire"); library op 42 ----
// SPEC GAP: on a null world / host or a slot outside 0..47 the result is 0.
#[no_mangle]
pub extern "C" fn ja1_ai_line_of_fire(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, thrown: i32) -> i32 {
    run(w, h, slot, 0, |a| a.line_of_fire(tile, thrown as u8))
}
#[no_mangle]
pub extern "C" fn ja1_ai_throw_option(w: *mut Ja1World, h: *const Ja1Host, slot: i32, out: *mut u8) {
    run(w, h, slot, (), |a| {
        let b = a.throw_option().bytes();
        if !out.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), out, 10) };
        }
    })
}
#[no_mangle]
pub extern "C" fn ja1_ai_knife_option(w: *mut Ja1World, h: *const Ja1Host, slot: i32, out: *mut u8) {
    run(w, h, slot, (), |a| {
        let b = a.knife_option().bytes();
        if !out.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), out, 10) };
        }
    })
}

/// Sector value needs no soldier: it runs without the soldier-buffer check.
// SPEC GAP: on a null world / host the result is -100 ("never").
#[no_mangle]
pub extern "C" fn ja1_ai_sector_value(w: *mut Ja1World, h: *const Ja1Host, sector: i32, flag: i32) -> i32 {
    if w.is_null() || h.is_null() {
        return -100;
    }
    let ai = Ai { w, h, s: 0 };
    catch_unwind(AssertUnwindSafe(|| ai.sector_value(sector as u8, flag as u8))).unwrap_or(-100)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Fake host: records every call ("random" as op 0), answers from a queue / a table.
    #[derive(Default)]
    struct Fake {
        rnd: Vec<i32>,
        ret: HashMap<i32, u32>,
        log: Vec<(i32, Vec<i32>)>,
        /// Soldier buffer address (0 = off): at each fire_line call, record slot 2's (tile, px, py).
        peek: usize,
        seen: Vec<(u16, u16, u16)>,
        /// Bytes written to `out` per op (as far as they fit).
        outs: HashMap<i32, Vec<u8>>,
    }
    unsafe extern "C" fn f_random(ctx: *mut c_void, n: i32) -> i32 {
        let f = &mut *(ctx as *mut Fake);
        f.log.push((0, vec![n]));
        if f.rnd.is_empty() { 0 } else { f.rnd.remove(0) }
    }
    unsafe extern "C" fn f_call(ctx: *mut c_void, op: i32, a: *const i32, n: i32, o: *mut u8, sz: i32) -> u32 {
        let f = &mut *(ctx as *mut Fake);
        let args = if n > 0 { std::slice::from_raw_parts(a, n as usize).to_vec() } else { vec![] };
        f.log.push((op, args));
        if let Some(b) = f.outs.get(&op) {
            if !o.is_null() {
                let k = b.len().min(sz.max(0) as usize);
                std::ptr::copy_nonoverlapping(b.as_ptr(), o, k);
            }
        }
        if op == 42 && f.peek != 0 {
            let r = |off: usize| {
                let p = (f.peek + 2 * REC + off) as *const u8;
                p.read() as u16 | (p.add(1).read() as u16) << 8
            };
            f.seen.push((r(f::TILE), r(f::X2), r(f::Y2)));
        }
        *f.ret.get(&op).unwrap_or(&(NONE as u32))
    }

    struct Env {
        soldiers: Vec<u8>,
        tiles: Vec<u8>,
        items: Vec<u8>,
        anims: Vec<u8>,
        sectors: Vec<u8>,
        done: Vec<u8>,
        knowledge: Vec<u8>,
        fresher: Vec<u8>,
        last_m: Vec<u8>,
        last_e: Vec<u8>,
        calls: Vec<u8>,
        paths: Vec<u8>,
        dir_steps: Vec<u8>,
        t0c: Vec<u8>,
        t1c: Vec<u8>,
        rem_m: Vec<u8>,
        rem_e: Vec<u8>,
        story: Vec<u8>,
        plants: Vec<u8>,
        crates: Vec<u8>,
        day: i32,
        weapons: Vec<u8>,
        absorb: Vec<u8>,
        anim_rows: Vec<u8>,
        host_ops: u64,
        fake: Box<Fake>,
    }
    impl Env {
        fn new() -> Env {
            let mut tiles = vec![0u8; TILES * 8];
            for t in 0..TILES {
                tiles[t * 8 + 5] = 0x40;
            }
            Env {
                soldiers: vec![0u8; SLOTS * REC],
                tiles,
                items: vec![0u8; ITEM_TYPES * 12],
                anims: vec![1u8; 256 * 8],
                sectors: vec![0u8; SECTORS * 9],
                done: vec![0u8; SECTORS],
                knowledge: vec![0u8; 64],
                fresher: vec![0u8; 2304],
                last_m: vec![0u8; 256],
                last_e: vec![0u8; 640],
                calls: {
                    let mut c = vec![0u8; 8];
                    c[0..2].copy_from_slice(&NONE.to_le_bytes());
                    c[4..6].copy_from_slice(&NONE.to_le_bytes());
                    c
                },
                paths: vec![0u8; SLOTS * 80],
                dir_steps: {
                    let mut d = vec![0u8; 257 * 2];
                    for (i, k) in [-56i16, -55, 1, 57, 56, 55, -1, -57].iter().enumerate() {
                        d[(i + 1) * 2..(i + 1) * 2 + 2].copy_from_slice(&k.to_le_bytes());
                    }
                    d
                },
                t0c: vec![0u8; 256],
                t1c: vec![0u8; 256],
                rem_m: vec![0u8; 128],
                rem_e: vec![0u8; 320],
                story: vec![0u8; 32],
                plants: vec![0xFFu8; 48],
                crates: vec![0xFFu8; 16],
                day: 10,
                weapons: vec![0u8; 256 * 6],
                absorb: vec![0u8; 288 * 2],
                anim_rows: vec![0u8; 256 * 8],
                // The slice 2a tests script every op: all library ops go to the host.
                host_ops: op::LIBRARY,
                fake: Box::default(),
            }
        }
        fn set16(&mut self, slot: usize, off: usize, v: u16) {
            self.set(slot, off, v as u8);
            self.set(slot, off + 1, (v >> 8) as u8);
        }
        fn set(&mut self, slot: usize, off: usize, v: u8) {
            self.soldiers[slot * REC + off] = v;
        }
        fn get(&self, slot: usize, off: usize) -> u8 {
            self.soldiers[slot * REC + off]
        }
        fn get16(&self, slot: usize, off: usize) -> u16 {
            self.get(slot, off) as u16 | (self.get(slot, off + 1) as u16) << 8
        }
        fn run<T>(&mut self, f: impl FnOnce(*mut Ja1World, *const Ja1Host) -> T) -> T {
            let mut w = Ja1World {
                soldiers: self.soldiers.as_mut_ptr(),
                tiles: self.tiles.as_mut_ptr(),
                item_types: self.items.as_ptr(),
                anim_table: self.anims.as_ptr(),
                sector_state: self.sectors.as_mut_ptr(),
                mission_done: self.done.as_mut_ptr(),
                cur_sector: 0,
                alarm: 0,
                combat: 0,
                difficulty: 0,
                mission_step: 0,
                mission_slot: 0x40,
                mission_target: NONE,
                mission_spot: NONE,
                turn_based: 1,
                busy: 0,
                active_slot: 0x40,
                santino_spoke: 0,
                santino_timer: 0,
                show_ai: 0,
                santino_word: 0,
                knowledge: self.knowledge.as_mut_ptr(),
                fresher: self.fresher.as_ptr(),
                last_seen_m: self.last_m.as_mut_ptr(),
                last_seen_e: self.last_e.as_mut_ptr(),
                calls: self.calls.as_mut_ptr(),
                paths: self.paths.as_mut_ptr(),
                dir_steps: self.dir_steps.as_ptr(),
                t0_class: self.t0c.as_ptr(),
                t1_class: self.t1c.as_ptr(),
                last_spotter: 0x40,
                remember_m: self.rem_m.as_ptr() as *mut u8,
                remember_e: self.rem_e.as_ptr() as *mut u8,
                story: self.story.as_ptr() as *mut u8,
                plants: self.plants.as_mut_ptr(),
                crates: self.crates.as_ptr(),
                day: self.day,
                weapons: self.weapons.as_ptr(),
                absorb: self.absorb.as_ptr(),
                anim_rows: self.anim_rows.as_ptr(),
                selected: 0,
                anim_scripts: std::ptr::null(),
                anim_free: std::ptr::null(),
                anim_sounds: std::ptr::null(),
                path_preview: 0,
                anim_steps_tile: std::ptr::null(),
                anim_turn_while: std::ptr::null(),
                opt_sound: 0,
                opt_onscreen: 0,
                view_x: 0,
                view_y: 0,
                view_w: 0,
                view_h: 0,
                anim_signal: 0,
                effect_tile: 0,
                supplies: std::ptr::null_mut(),
                equip_skill: std::ptr::null(),
                attach: std::ptr::null(),
                guard_skill: 0,
                ground_items: std::ptr::null_mut(),
                notes: std::ptr::null_mut(),
                move_codes: std::ptr::null_mut(),
                held: std::ptr::null_mut(),
                blast_list: std::ptr::null_mut(),
                listed: std::ptr::null(),
                t2_class: std::ptr::null(),
                enemy_watch: std::ptr::null_mut(),
                text_buf: std::ptr::null_mut(),
                item_buf: std::ptr::null_mut(),
                crate_state: std::ptr::null_mut(),
                crate_shown: std::ptr::null_mut(),
                fixed_items: std::ptr::null(),
                sectors: std::ptr::null_mut(),
                bomb_count: 0,
                scratch: std::ptr::null_mut(),
                team_on_duty: 0,
                purifier: 0,
                safe_opened: 0,
                screen_drawn: 0,
                interrupt_go: 0,
                turn_moves: 0,
                kept_merc: 0,
                next_selected: 0x40,
                quote_slot: 0x40,
                quote_tile: NONE,
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
                ap_warned: std::ptr::null_mut(),
                splash: std::ptr::null(),
                spotted: 0x40,
                opt_step_text: 0,
                key_shift: 0,
                key_alt: 0,
                opt_move_fx: 0,
                seen_enemies: 0,
                in_flight: 0,
                merc_shown: 0,
                interrupt_turn: 0,
                fx_anims: std::ptr::null(),
                tick_step: 10,
                view_rays: std::ptr::null(),
                obj_block: std::ptr::null(),
                story_finder: 0x40,
                spot_stop: 0,
                itq: std::ptr::null_mut(),
                itq_len: 0,
                bled: std::ptr::null_mut(),
                timed_fx: std::ptr::null_mut(),
                new_round: 0,
                turn_timer: 0,
                turn_timer2: 0,
                shown_slot: 0x40,
                view_tile: NONE as i32,
                focus_slot: 0x40,
                turn_shown: 0,
                merc_to_show: 0x40,
                view_hold: 0,
                weather_tire: 0,
                merc_tire: 0,
                round_lock: 0,
                round_lock2: 0,
                handed_over: 0,
                turn_flag: 0,
                interrupter: 0x40,
                speaker: 0x40,
                speech_wait: 0,
                stage: 0,
                redraw_next: 0,
                redraw_pending: 0,
                turn_clock: 0,
                show_timer: 0,
                sector_spoken: std::ptr::null_mut(),
                silencer: std::ptr::null(),
                shot_sound: std::ptr::null(),
                impact_noise: std::ptr::null(),
                shot_range: std::ptr::null(),
                busy_timer: 0,
                replay_m: 0,
                first_hit: 0,
                fx_id: 0,
                dmg_text: std::ptr::null_mut(),
                marked_slot: 0x40,
                knockdown: 0,
                focus_last: 0,
                hit_scale: 0,
                dead_mercs: 0,
                dead_allies: 0,
                dead_npc: 0,
                enemy_deaths: 0,
                squad_lost: 0,
                last_killer: 0x40,
                posture: std::ptr::null(),
                window: std::ptr::null(),
                attack_sound: std::ptr::null(),
                diff_mod: std::ptr::null(),
                key_ctrl: 0,
                fast_timer: 0,
                marks_shown: 0,
                marks_n: 0,
                marks: std::ptr::null_mut(),
                opt_path_marks: 0,
                sees_slot: 0x40,
                preview_slot: 0x40,
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
                inv_cursor: std::ptr::null_mut(),
                inv_swap: std::ptr::null_mut(),
                inv_picked: std::ptr::null_mut(),
                inv_give: std::ptr::null_mut(),
                item_rects: std::ptr::null(),
                slot_rects: std::ptr::null(),
                vest_owner: 0x40,
                mouse_x: 0,
                mouse_y: 0,
                mouse_buttons: 0,
                key_flags: 0,
                tip_item: 0,
                face_left: 0,
                cursor_drop: 0,
                drop_answer: 0,
                panel_dy: 0,
                voice_test: 0,
                accounts: std::ptr::null_mut(),
                prev_balance: 0,
                sector_order: std::ptr::null_mut(),
                profiles: std::ptr::null_mut(),
                stash_counts: std::ptr::null_mut(),
                stat_progress: std::ptr::null_mut(),
                greed: std::ptr::null(),
                stat_changed: std::ptr::null_mut(),
                duty7_merc: 0x40,
                sector_replay: std::ptr::null_mut(),
                guide_queue: std::ptr::null_mut(),
                clock_minute: 0,
                clock_hour: 7,
                day_hours: 0,
                day_minutes: 0,
                day_bar: 0,
                hard_day: 0,
                night_warned: 0,
                guard_eta: std::ptr::null_mut(),
                guard_rec: std::ptr::null_mut(),
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
                item_told: std::ptr::null_mut(),
                bonus_due: 0,
                bonus: 0,
                blaster: 0x40,
                gas_clouds: std::ptr::null_mut(),
                tileset: 0,
                no_break: std::ptr::null(),
                walter_said: std::ptr::null_mut(),
                walter_next: std::ptr::null_mut(),
                walter_sector: 99,
                strip_box: 0,
                opt_replays: 0,
                replay_carry: 0,
                opt_confirm_move: 0,
                detector_slot: 0xFF,
                detector_tile: -1,
            };
            let h = Ja1Host {
                ctx: &mut *self.fake as *mut Fake as *mut c_void,
                random: Some(f_random),
                call: Some(f_call),
                // ops 6 direction, 7 tile_dist, 53 stop_anim stay scripted in these tests (library ops since the
                // soldier_update.md Small ops; tested in update.rs).
                host_ops: self.host_ops | 1 << op::DIRECTION | 1 << op::TILE_DIST | 1 << op::STOP_ANIM,
            };
            f(&mut w, &h)
        }
    }

    #[test]
    fn calm_idles_unless_random10_is_0() {
        let mut e = Env::new();
        e.fake.rnd = vec![3];
        e.set(40, f::AI_TARGET, 7);
        assert_eq!(e.run(|w, h| ja1_ai_calm_action(w, h, 40)), 0);
        assert_eq!(e.fake.log, vec![(0, vec![10]), (op::IDLE, vec![40])]);
        assert_eq!(e.get16(40, f::AI_TARGET), NONE);
        // 0 goes on; energy 0 then stops at step 3 with no more calls.
        let mut e = Env::new();
        e.fake.rnd = vec![0];
        assert_eq!(e.run(|w, h| ja1_ai_calm_action(w, h, 40)), 0);
        assert_eq!(e.fake.log, vec![(0, vec![10])]);
    }

    #[test]
    fn calm_patrol_walk() {
        let mut e = Env::new();
        e.set(40, f::ENERGY, 100);
        e.set(40, f::HP, 50);
        e.set(40, f::HP_MAX, 50);
        e.set(40, f::ORDERS, 2); // +25
        e.set(40, f::ATTITUDE, 5); // +10 -> chance 60
        e.set(40, f::B1A8, 1);
        e.fake.rnd = vec![0, 59];
        e.fake.ret.insert(op::PATROL_TILE, 123);
        assert_eq!(e.run(|w, h| ja1_ai_calm_action(w, h, 40)), 2);
        assert_eq!(e.fake.log, vec![(0, vec![10]), (0, vec![100]), (op::PATROL_TILE, vec![40])]);
        assert_eq!(e.get16(40, f::AI_TARGET), 123);
        assert_eq!(e.get(40, f::B1A8), 0);
    }

    #[test]
    fn type2_ends_turn() {
        let mut e = Env::new();
        e.set(9, f::AI_TYPE, 2);
        e.run(|w, h| ja1_ai_tick(w, h, 9));
        assert_eq!(e.get(9, f::TURN_DONE), 1);
        assert!(e.fake.log.is_empty());
        e.set(3, f::AI_TYPE, 2);
        e.run(|w, h| ja1_ai_tick(w, h, 3));
        assert_eq!(e.get(3, f::TURN_DONE), 0);
    }

    #[test]
    fn ap_cost_table() {
        let mut e = Env::new();
        let cases: [(u8, u8, i32); 9] =
            [(1, 1, 1), (1, 0, 0), (6, 5, 1), (6, 4, 0), (7, 4, 0), (9, 2, 1), (10, 1, 0), (0, 0, 1), (1, 0xFF, 0)];
        for (code, ap, want) in cases {
            e.set(40, f::AI_ACTION, code);
            e.set(40, f::AP_LEFT, ap);
            assert_eq!(e.run(|w, h| ja1_ai_action_affordable(w, h, 40)), want, "code {code} ap {ap}");
        }
        assert!(e.fake.log.is_empty());
        e.set(40, f::AI_ACTION, 2);
        e.set(40, f::AP_LEFT, 0);
        e.fake.ret.insert(op::MOVE_AP, 0x1FF); // low byte -1
        assert_eq!(e.run(|w, h| ja1_ai_action_affordable(w, h, 40)), 1);
        e.fake.log.clear();
        e.set(40, f::AI_ACTION, 11);
        e.set(40, f::AI_TARGET, 0x10);
        e.set(40, f::AP_LEFT, 8);
        e.fake.ret.insert(op::ACTION_AP, 0x109); // low byte 9 > 8
        assert_eq!(e.run(|w, h| ja1_ai_action_affordable(w, h, 40)), 0);
        assert_eq!(e.fake.log, vec![(op::ACTION_AP, vec![40, 0x10, 1])]);
    }

    #[test]
    fn alert_enters_combat() {
        let mut e = Env::new();
        e.sectors[1] = 3;
        e.set(9, f::AI_TYPE, 0);
        e.set(9, f::ORDERS, 1);
        e.run(|w, h| ja1_ai_update_alert(w, h, 9));
        assert_eq!(e.get(9, f::ALERT), 2);
        assert_eq!(e.get(9, f::ORDERS), 2);
        assert_eq!(e.get(9, f::RETHINK), 1);
        assert!(e.fake.log.is_empty());
    }

    #[test]
    fn start_walk_stopped_short() {
        let mut e = Env::new();
        e.set(40, f::AI_ACTION, 2);
        e.set(40, f::AI_TARGET, 0x20);
        assert_eq!(e.run(|w, h| ja1_ai_start_action(w, h, 40)), 0);
        assert_eq!(e.fake.log, vec![(op::WALK_TO, vec![40, 0x20])]);
        assert_eq!(e.get16(40, f::WALK_FAIL_TILE), 0x20);
        assert_eq!(e.get16(40, f::AI_TARGET), NONE);
        assert_eq!(e.get(40, f::RETHINK), 1);
    }

    #[test]
    fn c_layout() {
        assert_eq!(std::mem::size_of::<Ja1World>(), 1152); // gcc sizeof on x86-64
        assert_eq!(std::mem::offset_of!(Ja1World, walter_said), 1128);
        assert_eq!(std::mem::offset_of!(Ja1World, walter_sector), 1144);
        assert_eq!(std::mem::offset_of!(Ja1World, strip_box), 1145);
        assert_eq!(std::mem::offset_of!(Ja1World, start_trees), 1060);
        assert_eq!(std::mem::offset_of!(Ja1World, start_plants), 1080);
        assert_eq!(std::mem::offset_of!(Ja1World, item_told), 1088);
        assert_eq!(std::mem::offset_of!(Ja1World, bonus_due), 1096);
        assert_eq!(std::mem::offset_of!(Ja1World, bonus), 1098);
        assert_eq!(std::mem::offset_of!(Ja1World, gas_clouds), 1104);
        assert_eq!(std::mem::offset_of!(Ja1World, no_break), 1120);
        assert_eq!(std::mem::offset_of!(Ja1World, guides_hired), 1056);
        assert_eq!(std::mem::offset_of!(Ja1World, guide_wait), 1057);
        assert_eq!(std::mem::offset_of!(Ja1World, idle_day), 1058);
        assert_eq!(std::mem::offset_of!(Ja1World, enemy_counter), 1059);
        assert_eq!(std::mem::offset_of!(Ja1World, clock_minute), 1024);
        assert_eq!(std::mem::offset_of!(Ja1World, day_hours), 1026);
        assert_eq!(std::mem::offset_of!(Ja1World, day_minutes), 1028);
        assert_eq!(std::mem::offset_of!(Ja1World, day_bar), 1032);
        assert_eq!(std::mem::offset_of!(Ja1World, night_warned), 1037);
        assert_eq!(std::mem::offset_of!(Ja1World, guard_eta), 1040);
        assert_eq!(std::mem::offset_of!(Ja1World, guard_rec), 1048);
        assert_eq!(std::mem::offset_of!(Ja1World, profiles), 960);
        assert_eq!(std::mem::offset_of!(Ja1World, stash_counts), 968);
        assert_eq!(std::mem::offset_of!(Ja1World, stat_progress), 976);
        assert_eq!(std::mem::offset_of!(Ja1World, greed), 984);
        assert_eq!(std::mem::offset_of!(Ja1World, stat_changed), 992);
        assert_eq!(std::mem::offset_of!(Ja1World, duty7_merc), 1000);
        assert_eq!(std::mem::offset_of!(Ja1World, sector_replay), 1008);
        assert_eq!(std::mem::offset_of!(Ja1World, guide_queue), 1016);
        assert_eq!(std::mem::offset_of!(Ja1World, accounts), 936);
        assert_eq!(std::mem::offset_of!(Ja1World, prev_balance), 944);
        assert_eq!(std::mem::offset_of!(Ja1World, sector_order), 952);
        assert_eq!(std::mem::offset_of!(Ja1World, inv_cursor), 856);
        assert_eq!(std::mem::offset_of!(Ja1World, item_rects), 888);
        assert_eq!(std::mem::offset_of!(Ja1World, vest_owner), 904);
        assert_eq!(std::mem::offset_of!(Ja1World, mouse_x), 908);
        assert_eq!(std::mem::offset_of!(Ja1World, mouse_buttons), 916);
        assert_eq!(std::mem::offset_of!(Ja1World, key_flags), 918);
        assert_eq!(std::mem::offset_of!(Ja1World, tip_item), 920);
        assert_eq!(std::mem::offset_of!(Ja1World, drop_answer), 924);
        assert_eq!(std::mem::offset_of!(Ja1World, panel_dy), 928);
        assert_eq!(std::mem::offset_of!(Ja1World, voice_test), 932);
        assert_eq!(std::mem::offset_of!(Ja1World, quit_tactical), 848);
        assert_eq!(std::mem::offset_of!(Ja1World, ripple_timer), 812);
        assert_eq!(std::mem::offset_of!(Ja1World, ambient_count), 828);
        assert_eq!(std::mem::offset_of!(Ja1World, leave_reason), 832);
        assert_eq!(std::mem::offset_of!(Ja1World, fuse_beep_timer), 836);
        assert_eq!(std::mem::offset_of!(Ja1World, fuse_beep), 840);
        assert_eq!(std::mem::offset_of!(Ja1World, turn_tick), 844);
        assert_eq!(std::mem::offset_of!(Ja1World, marks_shown), 792);
        assert_eq!(std::mem::offset_of!(Ja1World, marks_n), 796);
        assert_eq!(std::mem::offset_of!(Ja1World, marks), 800);
        assert_eq!(std::mem::offset_of!(Ja1World, opt_path_marks), 808);
        assert_eq!(std::mem::offset_of!(Ja1World, sees_slot), 809);
        assert_eq!(std::mem::offset_of!(Ja1World, preview_slot), 810);
        assert_eq!(std::mem::offset_of!(Ja1World, key_ctrl), 784);
        assert_eq!(std::mem::offset_of!(Ja1World, fast_timer), 788);
        assert_eq!(std::mem::offset_of!(Ja1World, posture), 752);
        assert_eq!(std::mem::offset_of!(Ja1World, diff_mod), 776);
        assert_eq!(std::mem::offset_of!(Ja1World, dead_mercs), 732);
        assert_eq!(std::mem::offset_of!(Ja1World, enemy_deaths), 744);
        assert_eq!(std::mem::offset_of!(Ja1World, squad_lost), 748);
        assert_eq!(std::mem::offset_of!(Ja1World, last_killer), 749);
        assert_eq!(std::mem::offset_of!(Ja1World, dmg_text), 720);
        assert_eq!(std::mem::offset_of!(Ja1World, marked_slot), 728);
        assert_eq!(std::mem::offset_of!(Ja1World, hit_scale), 731);
        assert_eq!(std::mem::offset_of!(Ja1World, silencer), 680);
        assert_eq!(std::mem::offset_of!(Ja1World, shot_range), 704);
        assert_eq!(std::mem::offset_of!(Ja1World, busy_timer), 712);
        assert_eq!(std::mem::offset_of!(Ja1World, replay_m), 716);
        assert_eq!(std::mem::offset_of!(Ja1World, fx_id), 718);
        assert_eq!(std::mem::offset_of!(Ja1World, round_lock), 649);
        assert_eq!(std::mem::offset_of!(Ja1World, redraw_next), 657);
        assert_eq!(std::mem::offset_of!(Ja1World, redraw_pending), 660);
        assert_eq!(std::mem::offset_of!(Ja1World, show_timer), 668);
        assert_eq!(std::mem::offset_of!(Ja1World, sector_spoken), 672);
        assert_eq!(std::mem::offset_of!(Ja1World, weather_tire), 644);
        assert_eq!(std::mem::offset_of!(Ja1World, merc_tire), 648);
        assert_eq!(std::mem::offset_of!(Ja1World, itq), 592);
        assert_eq!(std::mem::offset_of!(Ja1World, new_round), 624);
        assert_eq!(std::mem::offset_of!(Ja1World, shown_slot), 632);
        assert_eq!(std::mem::offset_of!(Ja1World, view_hold), 643);
        assert_eq!(std::mem::offset_of!(Ja1World, view_rays), 568);
        assert_eq!(std::mem::offset_of!(Ja1World, obj_block), 576);
        assert_eq!(std::mem::offset_of!(Ja1World, story_finder), 584);
        assert_eq!(std::mem::offset_of!(Ja1World, spot_stop), 585);
        assert_eq!(std::mem::offset_of!(Ja1World, key_shift), 541);
        assert_eq!(std::mem::offset_of!(Ja1World, seen_enemies), 544);
        assert_eq!(std::mem::offset_of!(Ja1World, interrupt_turn), 548);
        assert_eq!(std::mem::offset_of!(Ja1World, fx_anims), 552);
        assert_eq!(std::mem::offset_of!(Ja1World, tick_step), 560);
        assert_eq!(std::mem::offset_of!(Ja1World, screen_drawn), 486);
        assert_eq!(std::mem::offset_of!(Ja1World, quote_tile), 492);
        assert_eq!(std::mem::offset_of!(Ja1World, follow_all), 501);
        assert_eq!(std::mem::offset_of!(Ja1World, stop_timer), 504);
        assert_eq!(std::mem::offset_of!(Ja1World, interrupted), 508);
        assert_eq!(std::mem::offset_of!(Ja1World, walk_ap), 512);
        assert_eq!(std::mem::offset_of!(Ja1World, ap_warned), 520);
        assert_eq!(std::mem::offset_of!(Ja1World, splash), 528);
        assert_eq!(std::mem::offset_of!(Ja1World, spotted), 536);
        assert_eq!(std::mem::offset_of!(Ja1World, opt_step_text), 540);
        assert_eq!(std::mem::offset_of!(Ja1World, text_buf), 416);
        assert_eq!(std::mem::offset_of!(Ja1World, crate_shown), 440);
        assert_eq!(std::mem::offset_of!(Ja1World, fixed_items), 448);
        assert_eq!(std::mem::offset_of!(Ja1World, sectors), 456);
        assert_eq!(std::mem::offset_of!(Ja1World, bomb_count), 464);
        assert_eq!(std::mem::offset_of!(Ja1World, scratch), 472);
        assert_eq!(std::mem::offset_of!(Ja1World, team_on_duty), 480);
        assert_eq!(std::mem::offset_of!(Ja1World, purifier), 484);
        assert_eq!(std::mem::offset_of!(Ja1World, safe_opened), 485);
        assert_eq!(std::mem::offset_of!(Ja1World, t2_class), 400);
        assert_eq!(std::mem::offset_of!(Ja1World, enemy_watch), 408);
        assert_eq!(std::mem::offset_of!(Ja1World, guard_skill), 344);
        assert_eq!(std::mem::offset_of!(Ja1World, ground_items), 352);
        assert_eq!(std::mem::offset_of!(Ja1World, listed), 392);
        assert_eq!(std::mem::size_of::<Ja1Host>(), 32);
    }

    #[test]
    fn bad_input_does_not_panic() {
        let mut e = Env::new();
        for slot in [-1, 48, 1000] {
            e.run(|w, h| ja1_ai_tick(w, h, slot));
            assert_eq!(e.run(|w, h| ja1_ai_choose_action(w, h, slot, 7)), 7);
        }
        assert_eq!(ja1_ai_start_action(std::ptr::null_mut(), std::ptr::null(), 9), 0);
        e.set(40, f::ANIM_IDX, 0xFF);
        e.set(40, f::TILE, 0xFF);
        e.set(40, f::TILE + 1, 0xFF);
        e.set(40, f::AI_ACTION, 14);
        e.set(40, f::ALERT, 3);
        e.set(40, f::AP_LEFT, 9);
        e.fake.ret.insert(op::ATTACK_READY, 1);
        e.items[2] = KIND_GUN;
        e.run(|w, h| ja1_ai_tick(w, h, 40));
        e.run(|w, h| ja1_ai_engaged_action(w, h, 40));
        e.run(|w, h| ja1_ai_start_action(w, h, 40));
        e.host_ops = 0;
        for slot in [0usize, 9, 20, 40, 47] {
            e.set(slot, f::TILE, 0xFF);
            e.set(slot, f::TILE + 1, 0x7F);
            e.set(slot, f::AI_TYPE, slot as u8 % 6);
            e.set(slot, 0xD4, 0xFF); // pathLen 0x7FFF: reads past the paths of this slot
            e.set(slot, 0xD5, 0x7F);
            e.set(slot, f::HOME_TILE, 0x00);
            e.set(slot, f::HOME_TILE + 1, 0x80);
            let s = slot as i32;
            e.fake.ret.insert(op::TILE_DIST, 0);
            e.fake.ret.insert(op::FIND_PATH, 1);
            e.run(|w, h| {
                ja1_ai_noise_tile(w, h, s, std::ptr::null_mut());
                ja1_ai_find_dry_tile(w, h, s);
                ja1_ai_patrol_tile(w, h, s);
                ja1_ai_buddy_tile(w, h, s);
                ja1_ai_news_value(w, h, s);
                ja1_ai_approach_tile(w, h, s, -1);
                ja1_ai_ally_call_tile(w, h, s);
                ja1_ai_can_stand_at(w, h, s, 40000);
                ja1_ai_advance_tile(w, h, s, 1);
                ja1_ai_target_tile(w, h, s);
            });
        }
    }

    // ---------------- slice 2b: search procedures ----------------

    /// A soldier at `tile` on an empty floor (t1 class 0 = standable everywhere).
    fn floor(slot: usize, tile: u16) -> Env {
        let mut e = Env::new();
        // fire_line (42), action_ap (17) and shot_ap (45) are scripted in these tests; the line-of-fire tests
        // clear the bits. find_path (41) and path_ap (40) are scripted too.
        e.host_ops = 1 << op::FIRE_LINE | 1 << op::ACTION_AP | 1 << op::SHOT_AP | 1 << op::FIND_PATH | 1 << op::PATH_AP
            | 1 << op::CHANCE_TO_HIT | 1 << op::HIT_TEST // (library ops 46, 48 since shooting.md batch 5: scripted here)
            | 1 << op::ATTACK_READY | 1 << op::REACH_AP | 1 << op::CLICK; // (library ops 15, 14, 30 since click.md)
        e.set16(slot, f::TILE, tile);
        e.set16(slot, f::WALK_FAIL_TILE, NONE);
        e.set16(slot, f::HOME_TILE, NONE);
        e.set16(slot, 0x122, NONE);
        e
    }

    #[test]
    fn can_stand_at_steps() {
        let mut e = floor(40, 100);
        e.fake.ret.insert(op::FIND_PATH, 1);
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 200)), 1);
        assert_eq!(e.fake.log, vec![(op::FIND_PATH, vec![40, 200, 0])]);
        // Own tile: 0, no host call.
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 100)), 0);
        assert!(e.fake.log.is_empty());
        // walkFailTile is tested after the path call.
        e.set16(40, f::WALK_FAIL_TILE, 200);
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 200)), 0);
        assert_eq!(e.fake.log, vec![(op::FIND_PATH, vec![40, 200, 0])]);
        // Water and an occupied tile.
        e.fake.log.clear();
        e.tiles[300 * 8] = 0x6C;
        e.tiles[301 * 8 + 5] = 9;
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 300)), 0);
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 301)), 0);
        // A blocking object class: t1 0x3D gives -2.
        e.t1c[0x3D] = 9;
        e.tiles[302 * 8 + 1] = 0x3D;
        assert_eq!(e.run(|w, h| ja1_ai_can_stand_at(w, h, 40, 302)), 0);
        assert!(e.fake.log.is_empty());
    }

    /// Ledger row c2aea50:ja1logic:3750: library op 3 (FindDryTile?) from the AI's own calls (water and gas moves,
    /// AiThink) runs the port's search when its host_ops bit is clear: same tile as the direct entry point.
    #[test]
    fn library_op_3_runs_the_dry_tile_search() {
        let mut e = floor(40, 100);
        e.fake.ret.insert(op::PATH_AP, 5);
        let direct = e.run(|w, h| ja1_ai_find_dry_tile(w, h, 40));
        assert_ne!(direct, NONE as i32);
        e.fake.log.clear();
        let via_op = e.run(|w, h| Ai::new(w, h, 40).unwrap().tile_call(op::FIND_DRY_TILE, &[40]));
        assert_eq!(via_op as i32, direct);
        assert!(e.fake.log.iter().all(|(o, _)| *o != op::FIND_DRY_TILE));
    }

    #[test]
    fn host_ops_bit_sends_the_op_to_the_host() {
        let mut e = floor(40, 100);
        e.fake.ret.insert(op::FIND_PATH, 1);
        e.fake.ret.insert(op::CAN_STAND_AT, 0);
        e.host_ops = 1 << op::CAN_STAND_AT | 1 << op::FIRE_LINE;
        e.fake.rnd = vec![0; 10];
        // Patrol with leash 0 (orders 5): boxed, post NONE; can_stand_at goes to the host 10 times.
        e.set(40, f::ORDERS, 5);
        let t = e.run(|w, h| ja1_ai_patrol_tile(w, h, 40));
        assert_eq!(t, NONE as i32);
        let stands = e.fake.log.iter().filter(|c| c.0 == op::CAN_STAND_AT).count();
        assert_eq!(stands, 10);
        assert!(e.fake.log.iter().all(|c| c.0 != op::FIND_PATH));
        // Without the bit the library runs its own can_stand_at (find_path calls instead).
        e.host_ops = 0;
        e.fake.log.clear();
        e.fake.rnd = vec![0, 0];
        let t = e.run(|w, h| ja1_ai_patrol_tile(w, h, 40));
        // post = 3137: column 1, row 56; leash 0: wide = high = 1; t = 0 + 3137 + 0 -> outside the map: 0.
        assert_eq!(t, NONE as i32);
        assert!(e.fake.log.iter().all(|c| c.0 != op::CAN_STAND_AT));
    }

    #[test]
    fn patrol_boxed_draws() {
        let mut e = floor(40, 100);
        e.fake.ret.insert(op::FIND_PATH, 1);
        e.set(40, f::ORDERS, 1); // leash 5
        e.set16(40, f::HOME_TILE, 2 * 56 + 3); // col 3, row 2: left 3, right 5, top 2, bottom 5
        e.fake.rnd = vec![4, 6];
        let t = e.run(|w, h| ja1_ai_patrol_tile(w, h, 40));
        assert_eq!(t, (4 - 3) + 115 + (6 - 2) * 56);
        assert_eq!(e.fake.log[0], (0, vec![9]));
        assert_eq!(e.fake.log[1], (0, vec![8]));
        assert_eq!(e.fake.log[2], (op::FIND_PATH, vec![40, t, 0]));
    }

    #[test]
    fn decision_uses_library_patrol() {
        // The calm decision's patrol step now runs the library's patrol_tile.
        let mut e = floor(40, 100);
        e.set(40, f::ENERGY, 100);
        e.set(40, f::HP, 50);
        e.set(40, f::HP_MAX, 50);
        e.set(40, f::B1D7, 1); // forced: no random(10), no roll
        e.set(40, f::ORDERS, 3); // leash 99: not boxed
        e.fake.ret.insert(op::FIND_PATH, 1);
        e.fake.rnd = vec![500];
        assert_eq!(e.run(|w, h| ja1_ai_calm_action(w, h, 40)), 2);
        assert_eq!(e.fake.log, vec![(0, vec![0xC40]), (op::FIND_PATH, vec![40, 500, 0])]);
        assert_eq!(e.get16(40, f::AI_TARGET), 500);
    }

    #[test]
    fn noise_tile_offers() {
        let mut e = floor(40, 100);
        // Opponent slot 3: alive, heard (-2) at last-seen 700.
        e.set(3, f::IN_CUR_SECTOR, 1);
        e.set(3, f::IN_PLAY, 1);
        e.set(3, f::HP, 50);
        e.set(40, 0x13A + 3, 0xFE);
        e.last_e[3 * 2..3 * 2 + 2].copy_from_slice(&700u16.to_le_bytes());
        e.fake.ret.insert(op::TILE_DIST, 0x105); // low byte 5 -> v = -10
        let mut loud = 0u16;
        let t = e.run(|w, h| ja1_ai_noise_tile(w, h, 40, &mut loud));
        assert_eq!(t, 700);
        assert_eq!(loud as i16, -10);
        assert_eq!(e.fake.log, vec![(op::TILE_DIST, vec![100, 700])]);
        // Own call tile at the own tile is cleared.
        e.set16(40, 0x122, 100);
        e.set(40, 0x1C0, 7);
        e.run(|w, h| ja1_ai_noise_tile(w, h, 40, std::ptr::null_mut()));
        assert_eq!(e.get16(40, 0x122), NONE);
        assert_eq!(e.get(40, 0x1C0), 0);
    }

    #[test]
    fn news_value_sums_freshness() {
        let mut e = floor(40, 100);
        e.set(40, 0x1C0, 9);
        e.calls[6] = 4;
        e.fresher[1152] = 2; // fresher(0, 0) = 2 for each of 40 opponents
        assert_eq!(e.run(|w, h| ja1_ai_news_value(w, h, 40)), ((9 - 4 + 80) as u8) as i32);
        assert!(e.fake.log.is_empty());
    }

    #[test]
    fn target_tile_nearest_known() {
        let mut e = floor(40, 0);
        for (slot, tile) in [(1usize, 56u16 * 10), (2, 3)] {
            e.set(slot, f::IN_CUR_SECTOR, 1);
            e.set(slot, f::IN_PLAY, 1);
            e.set(slot, f::HP, 1);
            e.knowledge[slot] = 2;
            e.set16(slot, 0x108, tile);
        }
        assert_eq!(e.run(|w, h| ja1_ai_target_tile(w, h, 40)), 3);
    }

    fn fighter(e: &mut Env, slot: usize) {
        e.set(slot, f::IN_CUR_SECTOR, 1);
        e.set(slot, f::IN_PLAY, 1);
        e.set(slot, f::HP, 50);
        e.set(slot, f::HP_MAX, 50);
        e.set(slot, f::ENERGY, 100);
        e.set(slot, 0x192, 2); // level
        e.set(slot, 0x18B, 50); // agility
        e.set(slot, 0x18C, 50); // dexterity
        e.set(slot, 0x191, 50); // marksmanship
        e.set(slot, f::AP_LEFT, 10);
        e.set(slot, 0x17A, slot as u8);
    }

    #[test]
    fn soldier_power_steps() {
        let mut e = floor(3, 100);
        fighter(&mut e, 3);
        // Max AP 11, + 10/2 + 50/5 - 50/10 = 21; x level 2.
        assert_eq!(e.run(|w, h| ja1_ai_soldier_power(w, h, 3, NONE as i32, 1)), 42);
        assert!(e.fake.log.is_empty());
        // Last target tile: + 21/10, no host call.
        e.set16(3, f::LAST_TARGET_TILE, 200);
        assert_eq!(e.run(|w, h| ja1_ai_soldier_power(w, h, 3, 200, 0)), 46);
        assert!(e.fake.log.is_empty());
        // Facing the tile: + 21/20; then fire line 50 %.
        e.set(3, f::FACING, 4);
        e.fake.ret.insert(op::DIRECTION, 4);
        e.fake.ret.insert(op::FIRE_LINE, 50);
        assert_eq!(e.run(|w, h| ja1_ai_soldier_power(w, h, 3, 300, 1)), 22);
        assert_eq!(e.fake.log, vec![(op::DIRECTION, vec![709, 19, 325, 75]), (op::FIRE_LINE, vec![3, 300, 1])]);
        // Wounded below 15: divided by 19 - health; not in play: -999.
        e.fake.log.clear();
        e.set(3, f::HP, 10);
        // Max AP: 11 - 11*2*(20+40)/150 = 11 - 8 = 3 -> 8; v = 8 + 5 + 10 - 9 = 14, x2 = 28, / 9 = 3.
        assert_eq!(e.run(|w, h| ja1_ai_soldier_power(w, h, 3, NONE as i32, 0)), 3);
        e.set(3, f::IN_PLAY, 0);
        assert_eq!(e.run(|w, h| ja1_ai_soldier_power(w, h, 3, NONE as i32, 0)), -999);
    }

    #[test]
    fn situation_score_alone() {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        e.set(40, f::ENERGY, 0);
        e.host_ops = 1 << op::SECTOR_VALUE; // scripted: returns 3137
        // ratio 500 -> 4; sector value 3137 > 50 -> 5; attitude 0 -> 4; energy 0 -> 2; b1C4 0 -> 3.
        assert_eq!(e.run(|w, h| ja1_ai_opp_strength(w, h, 40)), 3);
        assert_eq!(e.fake.log, vec![(op::SECTOR_VALUE, vec![0, 1])]);
    }

    #[test]
    fn situation_score_counts_seen_opponent() {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        e.host_ops = 1 << op::SECTOR_VALUE;
        fighter(&mut e, 2); // opponent slot 2, seen (+0x13A + 2 = 1)
        e.set(2, 0x192, 9); // level 9: power 21 x 9 = 189 (no direction: own tile 0 vs my 100 -> call)
        e.set(40, 0x13A + 2, 1);
        e.set(40, f::ATTITUDE, 3);
        e.fake.ret.insert(op::DIRECTION, 0x100); // never equals facing 0
        e.fake.ret.insert(op::SECTOR_VALUE, 40);
        e.fresher[1152 + 1] = 1; // fresher(0, 1): the sight flag 1 is fresher than knowledge 0 -> lvl 1
        // oppSum = 100 * 189 / 100 = 189; allySum = 42; ratio = 4200 / 189 = 22 -> 0; sector value 40: no
        // change; energy 100 -> +1; health 50: none; b1C4 0 -> +1 = 2.
        assert_eq!(e.run(|w, h| ja1_ai_opp_strength(w, h, 40)), 2);
        assert_eq!(e.fake.log, vec![(op::DIRECTION, vec![5, 5, 709, 19]), (op::SECTOR_VALUE, vec![0, 1])]);
        // An unseen, unknown opponent is skipped unless remembered (then weight 50).
        e.set(40, 0x13A + 2, 0);
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| ja1_ai_opp_strength(w, h, 40)), 4); // ratio 500 -> 4, +2 -> clamped
        assert_eq!(e.fake.log, vec![(op::SECTOR_VALUE, vec![0, 1])]);
        e.rem_e[2] = 1;
        e.fake.log.clear();
        // lvl 0: oppSum = 50 * 189 / 100 = 94; ratio = 4200 / 94 = 44 -> 1; +2 = 3.
        let s = e.run(|w, h| ja1_ai_opp_strength(w, h, 40));
        assert_eq!(e.fake.log[0].0, op::DIRECTION);
        assert_eq!(s, 3);
    }

    #[test]
    fn opp_strength_is_a_library_op() {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        e.set(40, f::ENERGY, 0);
        e.host_ops = 1 << op::SECTOR_VALUE; // scripted: returns 3137
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.opp_strength())), 3);
        assert_eq!(e.fake.log, vec![(op::SECTOR_VALUE, vec![0, 1])]);
        e.fake.log.clear();
        e.host_ops = 1 << op::OPP_STRENGTH;
        e.fake.ret.insert(op::OPP_STRENGTH, 0x102);
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.opp_strength())), 2);
        assert_eq!(e.fake.log, vec![(op::OPP_STRENGTH, vec![40])]);
    }

    #[test]
    fn connected_home_library() {
        let mut e = Env::new();
        let mut ch = |e: &mut Env, c: i32| e.run(|w, _| home::ja1_connected_home(w, std::ptr::null(), c));
        assert_eq!(ch(&mut e, 59), 1);
        assert_eq!(ch(&mut e, 60), 0);
        assert_eq!(ch(&mut e, 0x13B), 1); // low byte 59
        assert_eq!(ch(&mut e, 49), 0); // unowned start
        // Isolated owned sector.
        let mut e = Env::new();
        e.sectors[22 * 9] = 1;
        assert_eq!(ch(&mut e, 22), 0);
        // Straight chain 49 -> 59 (south).
        let mut e = Env::new();
        e.sectors[49 * 9] = 1;
        e.sectors[59 * 9] = 1;
        assert_eq!(ch(&mut e, 49), 1);
        // Chain along row 5: 55..59.
        let mut e = Env::new();
        for n in 55..=59 {
            e.sectors[n * 9] = 1;
        }
        assert_eq!(ch(&mut e, 55), 1);
        // Gap at 57.
        e.sectors[57 * 9] = 0;
        assert_eq!(ch(&mut e, 55), 0);
    }

    #[test]
    fn sector_value_steps() {
        let mut e = Env::new();
        e.host_ops |= 1 << op::CONNECTED_HOME;
        e.fake.ret.insert(op::CONNECTED_HOME, 1);
        // 30 + connected 5 + four clear sides 4 + sector 5 bonus 20.
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 5, 1)), 59);
        assert_eq!(e.fake.log, vec![(op::CONNECTED_HOME, vec![5])]);
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 5, 0)), 34);
        // Home: day 10 > homeDay 6 -> + day; day 5 -> never (no host call).
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 0x3B, 1)), 49);
        e.day = 5;
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 0x3B, 1)), -100);
        assert!(e.fake.log.is_empty());
        // Story condition: 0x11 -> 250; a crate tomorrow and a plant add 25 each.
        e.story[0x09] = 2;
        e.story[0x06] = 2;
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 0x11, 1)), 250);
        e.story[0x09] = 0;
        e.crates[4] = 7;
        e.crates[5] = 6;
        e.plants[12 + 4..12 + 8].copy_from_slice(&7i32.to_le_bytes());
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 7, 1)), 30 + 5 + 4 + 50);
        // Quest sector 0x0B with mercs there: never.
        e.sectors[0x0B * 9 + 2] = 1;
        e.fake.ret.insert(op::QUEST_CHECK, 1);
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| ja1_ai_sector_value(w, h, 0x0B, 1)), -100);
        assert_eq!(e.fake.log, vec![(op::CONNECTED_HOME, vec![0x0B]), (op::QUEST_CHECK, vec![0x1C9])]);
    }

    #[test]
    fn situation_score_uses_library_sector_value() {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        e.set(40, f::ENERGY, 0);
        e.host_ops |= 1 << op::CONNECTED_HOME;
        e.fake.ret.insert(op::CONNECTED_HOME, 0);
        // Sector 0: 30 + 4 = 34 < 35 -> s - 1: 4 - 1 - 1 (attitude 0) - 2 (energy 0) + 1 = 1.
        assert_eq!(e.run(|w, h| ja1_ai_opp_strength(w, h, 40)), 1);
        assert_eq!(e.fake.log, vec![(op::CONNECTED_HOME, vec![0])]);
    }

    /// S (slot 40) at the middle of the map, opponent slot 2 known (knowledge 1) at 3 tiles east.
    fn retreat_env() -> Env {
        let my = 28 * 56 + 28;
        let mut e = floor(40, my);
        e.host_ops = 1 << op::CAN_STAND_AT | 1 << op::FIRE_LINE;
        fighter(&mut e, 40);
        fighter(&mut e, 2);
        e.set16(40, f::X2, 28 * 16 + 5);
        e.set16(40, f::Y2, 28 * 14 + 5);
        e.knowledge[2] = 1;
        e.set16(2, 0x108, my + 3);
        e.fake.ret.insert(op::CAN_STAND_AT, 1);
        e.fake.ret.insert(op::PATH_AP, 5);
        e.fake.ret.insert(op::DIRECTION, 0x100);
        e
    }

    #[test]
    fn retreat_tile_farthest() {
        let mut e = retreat_env();
        // near = 45; the first candidate at the far corner (dx -3, dy -3) gives 100.
        assert_eq!(e.run(|w, h| ja1_ai_retreat_tile(w, h, 40)), 1596 - 3 - 3 * 56);
        assert_eq!(e.fake.log[0].0, op::DIRECTION);
        assert_eq!(e.fake.log.iter().filter(|c| c.0 == op::CAN_STAND_AT).count(), 49);
        // Nobody known: NONE, no calls.
        e.knowledge[2] = 0;
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| ja1_ai_retreat_tile(w, h, 40)), NONE as i32);
        assert!(e.fake.log.is_empty());
    }

    #[test]
    fn search_tile_moves_records_around_fire_line() {
        let mut e = retreat_env();
        e.set16(2, f::TILE, 7);
        e.set16(2, f::X2, 1);
        e.set16(2, f::Y2, 2);
        e.set(40, f::ORDERS, 3); // leash 99: no tile_dist
        e.fake.ret.insert(op::FIRE_LINE, 0);
        e.fake.peek = e.soldiers.as_ptr() as usize;
        assert_eq!(e.run(|w, h| ja1_ai_search_tile(w, h, 40, 0)), NONE as i32);
        // First position score: O moved to 1599 during both fire_line calls, then restored.
        let t = 1599u16;
        let want = (t, ((t % 56) * 16 + 5) as u16, ((t / 56) * 14 + 5) as u16);
        assert_eq!(&e.fake.seen[..2], &[want, want]);
        assert_eq!((e.get16(2, f::TILE), e.get16(2, f::X2), e.get16(2, f::Y2)), (7, 1, 2));
        let fl: Vec<_> = e.fake.log.iter().filter(|c| c.0 == op::FIRE_LINE).take(2).cloned().collect();
        assert_eq!(fl, vec![(op::FIRE_LINE, vec![2, 1596, 1]), (op::FIRE_LINE, vec![40, 1599, 1])]);
        // An old tile of NONE keeps the new values.
        e.set16(2, f::TILE, NONE);
        e.run(|w, h| ja1_ai_search_tile(w, h, 40, 0));
        assert_eq!(e.get16(2, f::TILE), 1599);
    }

    /// S (slot 40, aiType 3, AP 10) at tile 100 with a gun; opponent slot 2 seen, at tile `ot`.
    fn attack_env(ot: u16) -> Env {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        fighter(&mut e, 2);
        e.set(40, f::AI_TYPE, 3);
        e.set16(40, f::X2, 709);
        e.set16(40, f::Y2, 19);
        e.set16(2, f::TILE, ot);
        e.set16(2, f::X2, ((ot % 56) * 16 + 5) as u16);
        e.set16(2, f::Y2, ((ot / 56) * 14 + 5) as u16);
        e.set(40, 0x13A + 2, 1);
        e.items[5 * 12 + 11] = 20; // range 20
        e.weapons[5 * 6 + 5] = 30;
        e.weapons[15 * 6 + 5] = 10;
        e.fake.ret.insert(op::ACTION_AP, 4);
        e.fake.ret.insert(op::SHOT_AP, 3);
        e.fake.ret.insert(op::DIRECTION, 0x100);
        e
    }
    fn rec_of(b: &[u8; 10]) -> (u8, u8, u8, u8, i32, u16) {
        (b[0], b[1], b[2], b[3], i32::from_le_bytes([b[4], b[5], b[6], b[7]]), u16::from_le_bytes([b[8], b[9]]))
    }

    #[test]
    fn gun_option_scores() {
        let mut e = attack_env(103);
        e.set(40, 0, 5);
        e.fake.ret.insert(op::FIRE_LINE, 80);
        e.fake.ret.insert(op::CHANCE_TO_HIT, 50);
        let mut out = [0u8; 10];
        e.run(|w, h| ja1_ai_gun_option(w, h, 40, out.as_mut_ptr()));
        // best 250 (a 0), eff 40, power 42 x 80 % = 33, expected damage 31: 250 x 31 x 40 x 33.
        assert_eq!(rec_of(&out), (1, 2, 0, 40, 10230000, 103));
        let mut want = vec![(op::ACTION_AP, vec![40, 103, 1]), (op::FIRE_LINE, vec![40, 103, 0]), (op::SHOT_AP, vec![40, 3137])];
        for a in 0..5 {
            want.push((op::CHANCE_TO_HIT, vec![40, 103, a]));
        }
        want.push((op::DIRECTION, vec![757, 19, 709, 19]));
        want.push((op::FIRE_LINE, vec![2, 100, 1]));
        assert_eq!(e.fake.log, want);
        // Armour: helmet 0x23 (absorb 10 at [0x23 + 15]) at 100 %, body 0x2D (absorb 50) at 100 %.
        e.absorb[2 * (0x23 + 15)] = 10;
        e.absorb[2 * 0x2D] = 50;
        e.set(2, 0x18, 0x23);
        e.set(2, 0x1A, 100);
        e.set(2, 0x3C, 0x2D);
        e.set(2, 0x3E, 100);
        // 31 - (10 x 15 + 50 x 80) / 100 = 31 - 41 -> s8(u8(-10)) = -10 -> 1.
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.expected_damage(2, 50))), 1);
    }

    #[test]
    fn knife_option_sneak() {
        let mut e = attack_env(101);
        e.set(40, 0, 0x0F);
        e.set(40, 2, 100);
        e.fake.ret.insert(op::TILE_DIST, 1);
        let mut out = [0u8; 10];
        e.run(|w, h| ja1_ai_knife_option(w, h, 40, out.as_mut_ptr()));
        // Sneak (O does not see S, distance 1, not diagonal): chance 99, best 495, power 42, damage 21.
        assert_eq!(rec_of(&out), (1, 2, 0, 99, 21 * 495 * 99 * 42, 101));
        assert_eq!(
            e.fake.log,
            vec![
                (op::ACTION_AP, vec![40, 101, 1]),
                (op::SHOT_AP, vec![40, 3137]),
                (op::TILE_DIST, vec![100, 101]),
                (op::DIRECTION, vec![725, 19, 709, 19]),
            ]
        );
        // O sees S: no tile_dist call, melee hit chance instead.
        e.set(2, 0x13A, 1);
        e.fake.log.clear();
        e.run(|w, h| ja1_ai_knife_option(w, h, 40, out.as_mut_ptr()));
        assert!(e.fake.log.iter().all(|c| c.0 != op::TILE_DIST));
        // skill: S (50 x 3 + 50 + 50 + 20) / 6 = 45 both sides -> 50; enemy at difficulty 0: 40.
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.melee_hit_chance(2, 0))), 40);
    }

    #[test]
    fn clear_rooms_is_a_library_op() {
        // Op 327 = ClearRoomFlags 0x61744 on the world's tiles: byte 3 of tiles 232 + 56 r + c (7 x 6) and
        // 625 + 56 r + c (3 x 5) becomes 0; the tiles beside the two rectangles keep it.
        let mut e = attack_env(103);
        for t in 0..3136 {
            e.tiles[t * 8 + 3] = 0x5A;
        }
        e.host_ops = 0;
        e.run(|w, h| run(w, h, 40, (), |a| { a.call(op::CLEAR_ROOMS, &[]); }));
        let b = |t: usize| e.tiles[t * 8 + 3];
        assert_eq!((b(231), b(232), b(237), b(238), b(232 + 56 * 6 + 5), b(232 + 56 * 7)), (0x5A, 0, 0, 0x5A, 0, 0x5A));
        assert_eq!((b(624), b(625), b(629), b(630), b(625 + 56 * 2 + 4), b(625 + 56 * 3)), (0x5A, 0, 0, 0x5A, 0, 0x5A));
        assert_eq!(e.tiles.chunks(8).filter(|t| t[3] == 0).count(), 42 + 15);
    }

    #[test]
    fn options_are_library_ops() {
        let mut e = attack_env(103);
        e.host_ops = 1 << op::GUN_OPTION;
        let o = e.run(|w, h| run(w, h, 40, None, |a| Some(a.option(op::GUN_OPTION)))).unwrap();
        assert!(!o.valid);
        assert_eq!(e.fake.log, vec![(op::GUN_OPTION, vec![40])]);
        e.host_ops = 1 << op::ACTION_AP | 1 << op::SHOT_AP; // scripted (batches 12, 13 test the library ones)
        e.fake.log.clear();
        e.set(40, 0, 0x0F);
        e.fake.ret.insert(op::TILE_DIST, 1);
        let o = e.run(|w, h| run(w, h, 40, None, |a| Some(a.option(op::KNIFE_OPTION)))).unwrap();
        assert!(o.valid && o.slot == 2 && o.tile == 103);
        assert!(e.fake.log.iter().all(|c| c.0 != op::KNIFE_OPTION));
        // Op 25 runs the throw option (EngagedAction 0x6EB54 calls ThrowOption 0x717F8 for a thrown item in a
        // hand, KnifeOption 0x71DD4 only for a blade): the same record as the direct call, not valid here (the
        // hand holds a blade, so no thrown item scores), where op 26 gives a valid stab.
        let t = e.run(|w, h| run(w, h, 40, None, |a| Some((a.option(op::THROW_OPTION), a.throw_option().bytes())))).unwrap();
        assert_eq!((t.0.valid, t.0.slot, t.0.tile), (t.1[0] != 0, t.1[1], u16::from_le_bytes([t.1[8], t.1[9]])));
        assert!(!t.0.valid);
    }

    #[test]
    fn div_zero_calls_the_host() {
        // Range 0 in the expected damage: op 47, then quotient 0.
        let mut e = attack_env(103);
        e.set(40, 0, 6); // item 6: range 0
        let v = e.run(|w, h| run(w, h, 40, 0, |a| a.expected_damage(2, 50)));
        assert_eq!(e.fake.log, vec![(op::DIV_ZERO, vec![])]);
        assert_eq!(v, 1);
        // Melee hit chance with a + d = 0.
        let mut e = floor(40, 100);
        e.set(40, 0, 0x0F);
        e.set(40, f::ENERGY, 100);
        e.set(2, f::ENERGY, 100);
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.melee_hit_chance(2, 0))), 1);
        assert_eq!(e.fake.log, vec![(op::DIV_ZERO, vec![])]);
    }

    #[test]
    fn throw_option_scores() {
        let my = 1593u16;
        let mut e = attack_env(1596);
        e.set16(40, f::TILE, my);
        e.set16(40, f::X2, 405);
        e.set16(40, f::Y2, 397);
        e.set(40, 0, 0x18); // grenade: blast 10, gas 60
        e.set(40, 1, 1); // one unit left: condition = byte +2
        e.set(40, 2, 100);
        e.items[0x18 * 12 + 2] = 0x95;
        e.items[0x18 * 12 + 11] = 10;
        e.fake.ret.insert(op::FIRE_LINE, 80);
        e.fake.ret.insert(op::TILE_DIST, 0x103); // full 259 > 4 for the ally; low byte 3 for the target
        let mut out = [0u8; 10];
        e.run(|w, h| ja1_ai_throw_option(w, h, 40, out.as_mut_ptr()));
        // First candidate 1482: hit 45 -> 18 x power 42 = 756; throw AP 7; chance 100 at aim 0 -> best 142;
        // eff 80. Later candidates tie at best (not strictly higher).
        assert_eq!(rec_of(&out), (1, 2, 0, 80, 80 * 142 * 756, 1482));
        assert_eq!(
            &e.fake.log[..5],
            &[
                (op::DIRECTION, vec![453, 397, 405, 397]),
                (op::ACTION_AP, vec![40, 1482, 1]),
                (op::FIRE_LINE, vec![40, 1482, 0]),
                (op::TILE_DIST, vec![1593, 1482]),
                (op::TILE_DIST, vec![1596, 1482]),
            ]
        );
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.throw_damage(2, 1482))), 45);
        // Ground class 8 and up: halved only.
        e.t0c[0] = 8;
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.throw_damage(2, 1482))), 15);
        // Throw AP with a divisor of 0 (dexterity -100): div_zero, then (0 + 1) / 2 = 0 -> 1.
        e.set(40, 0x18C, 0x9C);
        e.fake.log.clear();
        assert_eq!(e.run(|w, h| run(w, h, 40, 0, |a| a.throw_ap())), 1);
        assert_eq!(e.fake.log, vec![(op::DIV_ZERO, vec![])]);
        // Library op 25 unless claimed.
        e.host_ops = 1 << op::THROW_OPTION;
        e.fake.log.clear();
        e.run(|w, h| run(w, h, 40, (), |a| { a.option(op::THROW_OPTION); }));
        assert_eq!(e.fake.log, vec![(op::THROW_OPTION, vec![40])]);
    }

    #[test]
    fn line_of_fire_bullet() {
        let mut e = floor(40, 100);
        e.host_ops = 1 << op::HIT_TEST | 1 << op::SIGHT_LINE; // hit_test scripted (library op 48 since shooting.md batch 5)
        e.set16(40, f::X2, 709);
        e.set16(40, f::Y2, 19);
        e.fake.ret.insert(op::HIT_TEST, 0);
        e.fake.outs.insert(op::HIT_TEST, vec![32, 0, 0, 0, 90, 0, 0, 0]);
        // Own tile: 100, no calls.
        assert_eq!(e.run(|w, h| ja1_ai_line_of_fire(w, h, 40, 100, 0)), 100);
        assert!(e.fake.log.is_empty());
        // Two tiles east: 16 pixels on tile 101 and one on 102, 90 % each (truncated): 13.
        assert_eq!(e.run(|w, h| ja1_ai_line_of_fire(w, h, 40, 102, 0)), 13);
        assert_eq!(e.fake.log[0], (op::DIRECTION, vec![711, 23, 743, 23]));
        assert_eq!(e.fake.log[1], (op::SIGHT_LINE, vec![40, 711, 23, 102, 743, 23, 0, 0x0E, 0x40]));
        assert_eq!(e.fake.log[2], (0, vec![1001]));
        // h 4500, slope (-1000 / 32 - 1) / 2 = -16; first hit test at x 720 after 9 pixels; d 0x41, skip 32.
        assert_eq!(e.fake.log[3], (op::HIT_TEST, vec![4500 - 16 * 9, 720, 23, 9, 0x41, 32, 0, 102, 1]));
        assert_eq!(e.fake.log.len(), 3 + 17);
        // Stopped before the target: 0.
        e.fake.ret.insert(op::HIT_TEST, 1);
        assert_eq!(e.run(|w, h| ja1_ai_line_of_fire(w, h, 40, 102, 0)), 0);
        // Soldier power with F = 1 now runs the library line of fire (hit_test calls, no fire_line call).
        e.fake.log.clear();
        fighter(&mut e, 40);
        e.run(|w, h| ja1_ai_soldier_power(w, h, 40, 102, 1));
        assert!(e.fake.log.iter().any(|c| c.0 == op::HIT_TEST));
        assert!(e.fake.log.iter().all(|c| c.0 != op::FIRE_LINE));
    }

    #[test]
    fn line_of_fire_throw_div_zero() {
        // A throw with dist 0 is not reachable from a different tile on the map; a pixel position that
        // equals the target point gives it: half = 0 -> div_zero.
        let mut e = floor(40, 100);
        e.host_ops = 1 << op::HIT_TEST;
        e.set16(40, f::X2, 741); // px(102)
        e.set16(40, f::Y2, 19);
        e.items[1 * 12 + 2] = 0x95;
        e.fake.ret.insert(op::HIT_TEST, 1);
        e.run(|w, h| ja1_ai_line_of_fire(w, h, 40, 102, 1));
        assert_eq!(e.fake.log[2], (op::DIV_ZERO, vec![]));
    }

    // ---- sector_start.md batch 12: Action AP ----
    fn ap_env(kind: u8) -> Env {
        let mut e = floor(40, 100);
        // shot_ap scripted (batch 13 tests the library one); path_ap and find_path scripted too.
        e.host_ops = 1 << op::SHOT_AP | 1 << op::PATH_AP | 1 << op::FIND_PATH | 1 << op::REACH_AP; // (reach_ap: library op 14 since click.md, scripted here)
        e.set(40, 0, 5);
        e.items[5 * 12 + 2] = kind;
        e.weapons[5 * 6 + 2] = 7;
        e.fake.ret.insert(op::SHOT_AP, 0x103);
        e
    }
    fn ap(e: &mut Env, tile: i32, flag: i32) -> i32 {
        e.fake.log.clear();
        e.run(|w, h| crate::ai::attack::ja1_action_ap(w, h, 40, tile, flag))
    }

    #[test]
    fn action_ap_gun() {
        let mut e = ap_env(0x92);
        e.set16(40, f::ANIM, 4);
        assert_eq!(ap(&mut e, 103, 1), 10); // not aiming: sap 3 + weapons[5].2 7
        assert_eq!(e.fake.log, vec![(op::SHOT_AP, vec![40, 103])]);
        e.set16(40, f::ANIM, 0x1F);
        assert_eq!(ap(&mut e, 103, 0x100), 3); // flag = low byte 0
        e.fake.ret.insert(op::DIRECTION, 3);
        e.set(40, f::FACING, 3);
        assert_eq!(ap(&mut e, 103, 1), 3);
        // me 100 = (44, 1), tile 103 = (47, 1).
        assert_eq!(e.fake.log[1], (op::DIRECTION, vec![44 * 16 + 5, 14 + 5, 47 * 16 + 5, 14 + 5]));
        e.set(40, f::FACING, 2);
        assert_eq!(ap(&mut e, 103, 1), 4);
        assert_eq!(ap(&mut e, NONE as i32, 1), 4);
        assert_eq!(e.fake.log, vec![(op::SHOT_AP, vec![40, NONE as i32])]);
    }

    #[test]
    fn action_ap_knife_and_other() {
        let mut e = ap_env(0x96);
        e.fake.ret.insert(op::TILE_DIST, 1);
        assert_eq!(ap(&mut e, 101, 1), 3); // near, same row
        assert_eq!(e.fake.log, vec![(op::TILE_DIST, vec![100, 101]), (op::SHOT_AP, vec![40, 101])]);
        // Distance 1 but another row and column: not near; reach_ap 1 -> path_ap to mission_spot, tile 0xC41.
        e.fake.ret.insert(op::REACH_AP, 1);
        e.fake.ret.insert(op::PATH_AP, 0x105);
        assert_eq!(ap(&mut e, 157, 1), 8);
        assert_eq!(
            e.fake.log,
            vec![
                (op::TILE_DIST, vec![100, 157]),
                (op::REACH_AP, vec![40, 157, 0xF]),
                (op::PATH_AP, vec![40, NONE as i16 as i32]),
                (op::SHOT_AP, vec![40, NONE as i32]),
            ]
        );
        e.fake.ret.insert(op::REACH_AP, 0);
        assert_eq!(ap(&mut e, 157, 1), 0x66);
        // tile 0xC41 and up: c = 0, no tile_dist.
        assert_eq!(ap(&mut e, NONE as i32, 1), 3);
        assert_eq!(e.fake.log, vec![(op::SHOT_AP, vec![40, NONE as i32])]);
        // Other kinds: 0x63, no host call.
        let mut e = ap_env(0x10);
        assert_eq!(ap(&mut e, 101, 1), 0x63);
        assert!(e.fake.log.is_empty());
        // Thrown: Throw AP.
        let mut e = ap_env(0x95);
        let want = e.run(|w, h| run(w, h, 40, 0, |a| a.throw_ap())) & 0xFF;
        assert_eq!(ap(&mut e, 101, 1), want);
    }

    // ---- sector_start.md batch 13: Shot AP ----
    #[test]
    fn shot_ap_steps() {
        let mut e = floor(40, 100);
        fighter(&mut e, 40);
        e.host_ops = 0;
        e.set(40, 0, 5);
        e.set(40, 0x191, 61); // marksmanship: k / 2 = 30
        e.set16(40, 0xD0, 103);
        e.weapons[5 * 6 + 3] = 10;
        let m = e.run(|w, h| run(w, h, 40, 0, |a| a.max_ap(40))) as u8 as i32;
        assert!(m > 2);
        let shot = |e: &mut Env, t: i32| {
            e.fake.log.clear();
            e.run(|w, h| crate::ai::attack::ja1_shot_ap(w, h, 40, t))
        };
        // d = (30 + 50) x 10 / 2 = 400; q = 200 m / 400.
        let q = 200 * m / 400;
        assert_eq!(shot(&mut e, 103), ((q + 1) / 2).min(m).max(1)); // c = 0: tile = +0xD0
        assert_eq!(shot(&mut e, 104), (1 + (q + 1) / 2).min(m)); // c = 1
        assert!(e.fake.log.is_empty());
        // weapons[6y] = 4: k = (agility 50 + dexterity 51) / 2 = 50 -> d = 75 x 10 / 2 = 375.
        e.weapons[5 * 6] = 4;
        e.set(40, 0x18C, 51);
        assert_eq!(shot(&mut e, 104), (1 + (200 * m / 375 + 1) / 2).min(m));
        // d = 0: div_zero, q = 0, b = c, then at least 1.
        e.weapons[5 * 6 + 3] = 0;
        assert_eq!(shot(&mut e, 103), 1);
        assert_eq!(e.fake.log, vec![(op::DIV_ZERO, vec![])]);
        // +0xD0 = 0xC41: c = 1 even for tile 0xC41.
        e.set16(40, 0xD0, NONE);
        assert_eq!(shot(&mut e, NONE as i32), 1);
        // y outside 1..0x10: 0.
        e.set(40, 0, 0x11);
        assert_eq!(shot(&mut e, 104), 0);
        e.set(40, 0, 0);
        assert_eq!(shot(&mut e, 104), 0);
    }

    #[test]
    fn action_ap_runs_library_shot_ap() {
        let mut e = ap_env(0x92);
        fighter(&mut e, 40);
        e.host_ops = 0;
        e.set16(40, f::ANIM, 4);
        e.weapons[5 * 6 + 3] = 10;
        let sap = e.run(|w, h| crate::ai::attack::ja1_shot_ap(w, h, 40, 103));
        e.fake.log.clear();
        assert_eq!(ap(&mut e, 103, 1), (sap + 7) & 0xFF);
        assert!(e.fake.log.is_empty());
    }

    #[test]
    fn reserve_ap_runs_library_action_ap() {
        let mut e = ap_env(0x92);
        e.set16(40, f::ANIM, 4);
        e.run(|w, h| crate::sector::ja1_reserve_ap(w, h, 40));
        assert_eq!(e.soldiers[40 * REC + 0xEF], 10);
        assert_eq!(e.fake.log, vec![(op::SHOT_AP, vec![40, NONE as i32])]);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;
    fn host() -> Ja1Host {
        Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 }
    }

    #[test]
    fn guard_sector_byte_range() {
        // Port rule (SPEC GAP): a current sector of 80 and up reads 0; the row after sector 79 holds 5.
        let mut tab = vec![0u8; (SECTORS + 1) * 9];
        tab[79 * 9 + 1] = 3;
        tab[80 * 9 + 1] = 5;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        let h = host();
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        w.cur_sector = 79;
        assert_eq!(ai.sector_byte(tab.as_mut_ptr(), 9, 1), 3);
        w.cur_sector = 80;
        assert_eq!(ai.sector_byte(tab.as_mut_ptr(), 9, 1), 0);
    }
}
