//! The front-end as the library's host (docs/spec/world_load.md sections 5 and 7): the C structs of
//! docs/spec/port_api.md ("World", "Host") pointing into a `World`'s buffers, the game's random numbers, and the
//! answers to the host calls of a sector entry.
use crate::iso::Iso;
use crate::world::World;
use std::ffi::c_void;

/// port_api.md `Ja1World`, field for field (include/ja1logic.h).
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
    pub remember_m: *mut u8,
    pub remember_e: *mut u8,
    pub story: *mut u8,
    pub plants: *mut u8,
    pub crates: *const u8,
    pub day: i32,
    pub weapons: *const u8,
    pub absorb: *const u8,
    pub anim_rows: *const u8,
    pub selected: u8,
    pub anim_scripts: *const u8,
    pub anim_free: *const u8,
    pub anim_sounds: *const u8,
    pub path_preview: i32,
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
    pub supplies: *mut u8,
    pub equip_skill: *const u8,
    pub attach: *const u8,
    pub guard_skill: u8,
    pub ground_items: *mut u8,
    pub notes: *mut u8,
    pub move_codes: *mut u8,
    pub held: *mut u8,
    pub blast_list: *mut u8,
    pub listed: *const u8,
    pub t2_class: *const u8,
    pub enemy_watch: *mut u8,
    pub text_buf: *mut u8,
    pub item_buf: *mut u8,
    pub crate_state: *mut u8,
    pub crate_shown: *mut u8,
    pub fixed_items: *const u8,
    pub sectors: *mut u8,
    pub bomb_count: i32,
    pub scratch: *mut u8,
    pub team_on_duty: i32,
    pub purifier: u8,
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
    // turns.md "Spend AP":
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
    // shooting.md:
    pub silencer: *const u8,
    pub shot_sound: *const u8,
    pub impact_noise: *const u8,
    pub shot_range: *const u8,
    pub busy_timer: i32,
    pub replay_m: u8,
    pub first_hit: u8,
    pub fx_id: u8,
    // shooting.md batch 2:
    pub dmg_text: *mut u8,
    pub marked_slot: u8,
    pub knockdown: u8,
    pub focus_last: u8,
    pub hit_scale: u8,
    // shooting.md batch 4 (death):
    pub dead_mercs: i32,
    pub dead_allies: i32,
    pub dead_npc: i32,
    pub enemy_deaths: i32,
    pub squad_lost: u8,
    pub last_killer: u8,
    // shooting.md batch 5 (aiming), read-only:
    pub posture: *const u8,
    pub window: *const u8,
    pub attack_sound: *const u8,
    pub diff_mod: *const u8,
    // click.md:
    pub key_ctrl: u8,
    // turns.md:
    pub fast_timer: i32,
    // select.md:
    pub marks_shown: i32,
    pub marks_n: i32,
    pub marks: *mut u8,
    pub opt_path_marks: u8,
    pub sees_slot: u8,
    pub preview_slot: u8,
    // soldier_update.md Update end / Fuses / Timer tick:
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
    // soldier_update.md Loop head:
    pub turn_tick: i32,
    // soldier_update.md Leaving the sector:
    pub quit_tactical: u8,
    // inventory.md (world_load.md section 8 "Inventory screen"):
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
    pub accounts: *mut u8,
    pub prev_balance: i32,
    pub sector_order: *mut u8,
    pub profiles: *mut u8,
    pub stash_counts: *mut u8,
    pub stat_progress: *mut u8,
    pub greed: *const u8,
    pub stat_changed: *mut u8,
    pub duty7_merc: u8,
    pub sector_replay: *mut u8,
    pub guide_queue: *mut u8,
    pub clock_minute: u8,
    pub clock_hour: u8,
    pub day_hours: i8,
    pub day_minutes: i32,
    pub day_bar: i32,
    pub hard_day: u8,
    pub night_warned: u8,
    pub guard_eta: *mut u8,
    pub guard_rec: *mut u8,
    pub guides_hired: u8,
    pub guide_wait: u8,
    pub idle_day: u8,
    pub enemy_counter: u8,
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
    // strategic.md The day replay: Day Replays (switch 12) and the carry.
    pub opt_replays: u8,
    pub replay_carry: u8,
    // MoveMercTo 0x1C608 confirm: option byte, and the detector wait the library left (tile < 0 = none).
    pub opt_confirm_move: u8,
    pub detector_slot: u8,
    pub detector_tile: i16,
}

pub type RandomFn = unsafe extern "C" fn(ctx: *mut c_void, n: i32) -> i32;
pub type CallFn =
    unsafe extern "C" fn(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, out: *mut u8, out_size: i32) -> u32;

/// port_api.md `Ja1Host`.
#[repr(C)]
pub struct Ja1Host {
    pub ctx: *mut c_void,
    pub random: RandomFn,
    pub call: CallFn,
    pub host_ops: u64,
}

impl Ja1World {
    /// Scalars copied from `w`, buffer pointers into `w`'s buffers. Valid while `w` is not moved or resized.
    pub fn view(w: &mut World) -> Ja1World {
        Ja1World {
            soldiers: w.soldiers.as_mut_ptr(),
            tiles: w.tiles.as_mut_ptr(),
            item_types: w.item_types.as_ptr(),
            anim_table: w.anim_table.as_ptr(),
            sector_state: w.sector_state.as_mut_ptr(),
            mission_done: w.mission_done.as_mut_ptr(),
            cur_sector: w.cur_sector,
            alarm: w.alarm,
            combat: w.combat,
            difficulty: w.difficulty,
            mission_step: w.mission_step,
            mission_slot: w.mission_slot,
            mission_target: w.mission_target,
            mission_spot: w.mission_spot,
            turn_based: w.turn_based,
            busy: w.busy,
            active_slot: w.active_slot,
            santino_spoke: w.santino_spoke,
            santino_timer: w.santino_timer,
            show_ai: w.show_ai,
            santino_word: w.santino_word,
            knowledge: w.knowledge.as_mut_ptr(),
            fresher: w.fresher.as_ptr(),
            last_seen_m: w.last_seen_m.as_mut_ptr(),
            last_seen_e: w.last_seen_e.as_mut_ptr(),
            calls: w.calls.as_mut_ptr(),
            paths: w.paths.as_mut_ptr(),
            dir_steps: w.dir_steps.as_ptr(),
            t0_class: w.t0_class.as_ptr(),
            t1_class: w.t1_class.as_ptr(),
            last_spotter: w.last_spotter,
            remember_m: w.remember_m.as_mut_ptr(),
            remember_e: w.remember_e.as_mut_ptr(),
            story: w.story.as_mut_ptr(),
            plants: w.plants.as_mut_ptr(),
            crates: w.crates.as_ptr(),
            day: w.day,
            weapons: w.weapons.as_ptr(),
            absorb: w.absorb.as_ptr(),
            anim_rows: w.anim_rows.as_ptr(),
            selected: w.selected,
            anim_scripts: w.anim_scripts.as_ptr(),
            anim_free: w.anim_free.as_ptr(),
            anim_sounds: w.anim_sounds.as_ptr(),
            path_preview: w.path_preview,
            anim_steps_tile: w.anim_steps_tile.as_ptr(),
            anim_turn_while: w.anim_turn_while.as_ptr(),
            opt_sound: w.opt_sound,
            opt_onscreen: w.opt_onscreen,
            view_x: w.view_x,
            view_y: w.view_y,
            view_w: w.view_w,
            view_h: w.view_h,
            anim_signal: w.anim_signal,
            effect_tile: w.effect_tile,
            supplies: w.supplies.as_mut_ptr(),
            equip_skill: w.equip_skill.as_ptr(),
            attach: w.attach.as_ptr(),
            guard_skill: w.guard_skill,
            ground_items: w.ground_items.as_mut_ptr(),
            notes: w.notes.as_mut_ptr(),
            move_codes: w.move_codes.as_mut_ptr(),
            held: w.held.as_mut_ptr(),
            blast_list: w.blast_list.as_mut_ptr(),
            listed: w.listed.as_ptr(),
            t2_class: w.t2_class.as_ptr(),
            enemy_watch: w.enemy_watch.as_mut_ptr(),
            text_buf: w.text_buf.as_mut_ptr(),
            item_buf: w.item_buf.as_mut_ptr(),
            crate_state: w.crate_state.as_mut_ptr(),
            crate_shown: w.crate_shown.as_mut_ptr(),
            fixed_items: w.fixed_items.as_ptr(),
            sectors: w.sectors.as_mut_ptr(),
            bomb_count: w.bomb_count,
            scratch: w.scratch.as_mut_ptr(),
            team_on_duty: w.team_on_duty,
            purifier: w.purifier,
            safe_opened: w.safe_opened,
            screen_drawn: w.update.screen_drawn,
            interrupt_go: w.update.interrupt_go,
            turn_moves: w.update.turn_moves,
            kept_merc: w.update.kept_merc,
            next_selected: w.update.next_selected,
            quote_slot: w.update.quote_slot,
            quote_tile: w.update.quote_tile,
            quote_focus: w.update.quote_focus,
            quote_delay: w.update.quote_delay,
            quote_event: w.update.quote_event,
            opt_half_delay: w.update.opt_half_delay,
            opt_follow: w.update.opt_follow,
            opt_reload_msg: w.update.opt_reload_msg,
            alone_warned: w.update.alone_warned,
            follow_all: w.update.follow_all,
            stop_timer: w.update.stop_timer,
            interrupted: w.update.interrupted,
            walk_ap: w.update.walk_ap,
            ap_warned: w.ap_warned.as_mut_ptr(),
            splash: w.splash.as_ptr(),
            spotted: w.update.spotted,
            opt_step_text: w.update.opt_step_text,
            key_shift: w.update.key_shift,
            key_alt: w.update.key_alt,
            opt_move_fx: w.update.opt_move_fx,
            seen_enemies: w.update.seen_enemies,
            in_flight: w.update.in_flight,
            merc_shown: w.update.merc_shown,
            interrupt_turn: w.update.interrupt_turn,
            // From the mutable buffer: the front-end's animation timers (play.rs) write the counts through it.
            fx_anims: w.fx_anims.as_mut_ptr() as *const u8,
            tick_step: w.update.tick_step,
            view_rays: w.view_rays.as_ptr(),
            obj_block: w.obj_block.as_ptr(),
            story_finder: w.update.story_finder,
            spot_stop: w.update.spot_stop,
            itq: w.itq.as_mut_ptr(),
            itq_len: w.update.itq_len,
            bled: w.bled.as_mut_ptr(),
            timed_fx: w.timed_fx.as_mut_ptr(),
            new_round: w.update.new_round,
            turn_timer: w.update.turn_timer,
            turn_timer2: w.update.turn_timer2,
            shown_slot: w.update.shown_slot,
            view_tile: w.update.view_tile,
            focus_slot: w.update.focus_slot,
            turn_shown: w.update.turn_shown,
            merc_to_show: w.update.merc_to_show,
            view_hold: w.update.view_hold,
            weather_tire: w.update.weather_tire,
            merc_tire: w.update.merc_tire,
            round_lock: w.update.round_lock,
            round_lock2: w.update.round_lock2,
            handed_over: w.update.handed_over,
            turn_flag: w.update.turn_flag,
            interrupter: w.update.interrupter,
            speaker: w.update.speaker,
            speech_wait: w.update.speech_wait,
            stage: w.update.stage,
            redraw_next: w.update.redraw_next,
            redraw_pending: w.update.redraw_pending,
            turn_clock: w.update.turn_clock,
            show_timer: w.update.show_timer,
            sector_spoken: w.sector_spoken.as_mut_ptr(),
            silencer: w.silencer.as_ptr(),
            shot_sound: w.shot_sound.as_ptr(),
            impact_noise: w.impact_noise.as_ptr(),
            shot_range: w.shot_range.as_ptr(),
            busy_timer: w.update.busy_timer,
            replay_m: w.update.replay_m,
            first_hit: w.update.first_hit,
            fx_id: w.update.fx_id,
            dmg_text: w.dmg_text.as_mut_ptr(),
            marked_slot: w.update.marked_slot,
            knockdown: w.update.knockdown,
            focus_last: w.update.focus_last,
            hit_scale: w.update.hit_scale,
            dead_mercs: w.update.dead_mercs,
            dead_allies: w.update.dead_allies,
            dead_npc: w.update.dead_npc,
            enemy_deaths: w.update.enemy_deaths,
            squad_lost: w.update.squad_lost,
            last_killer: w.update.last_killer,
            posture: w.posture.as_ptr(),
            window: w.window.as_ptr(),
            attack_sound: w.attack_sound.as_ptr(),
            diff_mod: w.diff_mod.as_ptr(),
            key_ctrl: w.update.key_ctrl,
            fast_timer: w.update.fast_timer,
            marks_shown: w.update.marks_shown,
            marks_n: w.update.marks_n,
            marks: w.marks.as_mut_ptr(),
            opt_path_marks: w.update.opt_path_marks,
            sees_slot: w.update.sees_slot,
            preview_slot: w.update.preview_slot,
            ripple_timer: w.update.ripple_timer,
            ripple_tile: w.update.ripple_tile,
            ambient_t1: w.update.ambient_t1,
            ambient_t2: w.update.ambient_t2,
            ambient_count: w.update.ambient_count,
            opt_ambient: w.update.opt_ambient,
            time_compress: w.update.time_compress,
            enemies_warned: w.update.enemies_warned,
            leave_reason: w.update.leave_reason,
            fuse_beep_timer: w.update.fuse_beep_timer,
            fuse_beep: w.update.fuse_beep,
            turn_tick: w.update.turn_tick,
            quit_tactical: w.update.quit_tactical,
            // world_load.md section 8 "Inventory screen": the cursor item is `held`.
            inv_cursor: w.held.as_mut_ptr(),
            inv_swap: w.inv.swap.as_mut_ptr(),
            inv_picked: w.inv.picked.as_mut_ptr(),
            inv_give: w.inv.give.as_mut_ptr(),
            item_rects: w.inv.item_rects.as_ptr(),
            slot_rects: w.inv.slot_rects.as_ptr(),
            vest_owner: w.inv.vest_owner,
            mouse_x: 0,
            mouse_y: 0,
            mouse_buttons: 0,
            key_flags: 0,
            tip_item: w.inv.tip_item,
            face_left: w.inv.face_left,
            cursor_drop: w.inv.cursor_drop,
            drop_answer: w.inv.drop_answer,
            panel_dy: w.inv.panel_dy,
            voice_test: w.inv.voice_test,
            // the strategic fields (world_load.md sections 2, 3 and 9).
            accounts: w.accounts.as_mut_ptr(),
            prev_balance: w.prev_balance,
            sector_order: w.sector_order.as_mut_ptr(),
            profiles: w.profiles.as_mut_ptr(),
            stash_counts: w.stash_counts.as_mut_ptr(),
            stat_progress: w.stat_progress.as_mut_ptr(),
            greed: w.greed.as_ptr(),
            stat_changed: w.stat_changed.as_mut_ptr(),
            duty7_merc: w.duty7_merc,
            sector_replay: w.sector_replay.as_mut_ptr(),
            guide_queue: w.guide_queue.as_mut_ptr(),
            // the day clock (turns.md The day clock).
            clock_minute: w.clock.minute,
            clock_hour: w.clock.hour,
            day_hours: w.clock.hours,
            day_minutes: w.clock.minutes,
            day_bar: w.clock.bar,
            hard_day: w.clock.hard_day,
            night_warned: w.clock.night_warned,
            guard_eta: w.clock.guard_eta.as_mut_ptr(),
            guard_rec: w.clock.guard_rec.as_mut_ptr(),
            // the guide's advice (strategic.md The guide's advice).
            guides_hired: w.guides_hired,
            guide_wait: w.guide_wait,
            idle_day: w.idle_day,
            enemy_counter: w.enemy_counter,
            // the day's verdict (strategic.md The day's verdict).
            start_trees: w.start_trees,
            start_workable: w.start_workable,
            start_tapped: w.start_tapped,
            start_sectors: w.start_sectors,
            start_cap: w.start_cap,
            start_plants: w.start_plants,
            item_told: w.item_told.as_mut_ptr(),
            bonus_due: w.bonus_due,
            bonus: w.bonus,
            blaster: w.blaster,
            gas_clouds: w.gas_clouds.as_mut_ptr(),
            tileset: w.tileset,
            no_break: w.no_break.as_ptr(),
            walter_said: w.walter_said.as_mut_ptr(),
            walter_next: w.walter_next.as_mut_ptr(),
            walter_sector: w.walter_sector,
            strip_box: w.inv.strip_box,
            opt_replays: crate::ctlpanel::get(w, 12),
            replay_carry: w.replay_carry,
            opt_confirm_move: w.opt_confirm_move,
            detector_slot: 0xFF,
            detector_tile: -1,
        }
    }

    /// Copies the scalars back into `w` (the buffers were changed in place).
    pub fn store(&self, w: &mut World) {
        w.cur_sector = self.cur_sector;
        w.alarm = self.alarm;
        w.combat = self.combat;
        w.difficulty = self.difficulty;
        w.mission_step = self.mission_step;
        w.mission_slot = self.mission_slot;
        w.mission_target = self.mission_target;
        w.mission_spot = self.mission_spot;
        w.turn_based = self.turn_based;
        w.busy = self.busy;
        w.active_slot = self.active_slot;
        w.santino_spoke = self.santino_spoke;
        w.santino_timer = self.santino_timer;
        w.show_ai = self.show_ai;
        w.santino_word = self.santino_word;
        w.last_spotter = self.last_spotter;
        w.day = self.day;
        w.selected = self.selected;
        w.path_preview = self.path_preview;
        w.opt_sound = self.opt_sound;
        w.opt_onscreen = self.opt_onscreen;
        w.view_x = self.view_x;
        w.view_y = self.view_y;
        w.view_w = self.view_w;
        w.view_h = self.view_h;
        w.anim_signal = self.anim_signal;
        w.effect_tile = self.effect_tile;
        w.guard_skill = self.guard_skill;
        w.bomb_count = self.bomb_count;
        w.team_on_duty = self.team_on_duty;
        w.purifier = self.purifier;
        w.safe_opened = self.safe_opened;
        w.update.screen_drawn = self.screen_drawn;
        w.update.interrupt_go = self.interrupt_go;
        w.update.turn_moves = self.turn_moves;
        w.update.kept_merc = self.kept_merc;
        w.update.next_selected = self.next_selected;
        w.update.quote_slot = self.quote_slot;
        w.update.quote_tile = self.quote_tile;
        w.update.quote_focus = self.quote_focus;
        w.update.quote_delay = self.quote_delay;
        w.update.quote_event = self.quote_event;
        w.update.opt_half_delay = self.opt_half_delay;
        w.update.opt_follow = self.opt_follow;
        w.update.opt_reload_msg = self.opt_reload_msg;
        w.update.alone_warned = self.alone_warned;
        w.update.follow_all = self.follow_all;
        w.update.stop_timer = self.stop_timer;
        w.update.interrupted = self.interrupted;
        w.update.walk_ap = self.walk_ap;
        w.update.spotted = self.spotted;
        w.update.opt_step_text = self.opt_step_text;
        w.update.key_shift = self.key_shift;
        w.update.key_alt = self.key_alt;
        w.update.key_ctrl = self.key_ctrl;
        w.update.fast_timer = self.fast_timer;
        w.update.marks_shown = self.marks_shown;
        w.update.marks_n = self.marks_n;
        w.update.opt_path_marks = self.opt_path_marks;
        w.update.sees_slot = self.sees_slot;
        w.update.preview_slot = self.preview_slot;
        w.update.ripple_timer = self.ripple_timer;
        w.update.ripple_tile = self.ripple_tile;
        w.update.ambient_t1 = self.ambient_t1;
        w.update.ambient_t2 = self.ambient_t2;
        w.update.ambient_count = self.ambient_count;
        w.update.opt_ambient = self.opt_ambient;
        w.update.time_compress = self.time_compress;
        w.update.enemies_warned = self.enemies_warned;
        w.update.leave_reason = self.leave_reason;
        w.update.fuse_beep_timer = self.fuse_beep_timer;
        w.update.fuse_beep = self.fuse_beep;
        w.update.turn_tick = self.turn_tick;
        w.update.quit_tactical = self.quit_tactical;
        w.inv.vest_owner = self.vest_owner;
        w.inv.tip_item = self.tip_item;
        w.inv.face_left = self.face_left;
        w.inv.cursor_drop = self.cursor_drop;
        w.inv.drop_answer = self.drop_answer;
        w.inv.panel_dy = self.panel_dy;
        w.inv.voice_test = self.voice_test;
        w.prev_balance = self.prev_balance;
        w.duty7_merc = self.duty7_merc;
        w.clock.minute = self.clock_minute;
        w.clock.hour = self.clock_hour;
        w.clock.hours = self.day_hours;
        w.clock.minutes = self.day_minutes;
        w.clock.bar = self.day_bar;
        w.clock.hard_day = self.hard_day;
        w.clock.night_warned = self.night_warned;
        w.guides_hired = self.guides_hired;
        w.guide_wait = self.guide_wait;
        w.idle_day = self.idle_day;
        w.enemy_counter = self.enemy_counter;
        w.start_trees = self.start_trees;
        w.start_workable = self.start_workable;
        w.start_tapped = self.start_tapped;
        w.start_sectors = self.start_sectors;
        w.start_cap = self.start_cap;
        w.start_plants = self.start_plants;
        w.bonus_due = self.bonus_due;
        w.bonus = self.bonus;
        w.blaster = self.blaster;
        w.tileset = self.tileset;
        w.walter_sector = self.walter_sector;
        w.inv.strip_box = self.strip_box;
        w.replay_carry = self.replay_carry;
        w.update.opt_move_fx = self.opt_move_fx;
        w.update.seen_enemies = self.seen_enemies;
        w.update.in_flight = self.in_flight;
        w.update.merc_shown = self.merc_shown;
        w.update.interrupt_turn = self.interrupt_turn;
        w.update.tick_step = self.tick_step;
        w.update.story_finder = self.story_finder;
        w.update.spot_stop = self.spot_stop;
        w.update.itq_len = self.itq_len;
        w.update.new_round = self.new_round;
        w.update.turn_timer = self.turn_timer;
        w.update.turn_timer2 = self.turn_timer2;
        w.update.shown_slot = self.shown_slot;
        w.update.view_tile = self.view_tile;
        w.update.focus_slot = self.focus_slot;
        w.update.turn_shown = self.turn_shown;
        w.update.merc_to_show = self.merc_to_show;
        w.update.view_hold = self.view_hold;
        w.update.weather_tire = self.weather_tire;
        w.update.merc_tire = self.merc_tire;
        w.update.round_lock = self.round_lock;
        w.update.round_lock2 = self.round_lock2;
        w.update.handed_over = self.handed_over;
        w.update.turn_flag = self.turn_flag;
        w.update.interrupter = self.interrupter;
        w.update.speaker = self.speaker;
        w.update.speech_wait = self.speech_wait;
        w.update.stage = self.stage;
        w.update.redraw_next = self.redraw_next;
        w.update.redraw_pending = self.redraw_pending;
        w.update.turn_clock = self.turn_clock;
        w.update.show_timer = self.show_timer;
        w.update.busy_timer = self.busy_timer;
        w.update.replay_m = self.replay_m;
        w.update.first_hit = self.first_hit;
        w.update.fx_id = self.fx_id;
        w.update.marked_slot = self.marked_slot;
        w.update.knockdown = self.knockdown;
        w.update.focus_last = self.focus_last;
        w.update.hit_scale = self.hit_scale;
        w.update.dead_mercs = self.dead_mercs;
        w.update.dead_allies = self.dead_allies;
        w.update.dead_npc = self.dead_npc;
        w.update.enemy_deaths = self.enemy_deaths;
        w.update.squad_lost = self.squad_lost;
        w.update.last_killer = self.last_killer;
    }
}

/// Section 5: Watcom's rand over a 32-bit seed, scaled to 0..n-1 (the original's Random 0x53538 and rand_
/// 0x76929; the seed is the global at 0x9BBBC in the game).
pub struct Rng {
    pub seed: u32,
    pub draws: u64,
}

impl Rng {
    pub fn new(seed: u32) -> Rng {
        Rng { seed, draws: 0 }
    }
    pub fn random(&mut self, n: i32) -> i32 {
        self.draws += 1;
        // SPEC GAP: section 5 does not say if n <= 0 steps the seed; here it does (r is computed before m is read).
        self.seed = self.seed.wrapping_mul(0x41C6_4E6D).wrapping_add(0x3039);
        let r = ((self.seed >> 16) & 0x7FFF) as i32;
        let m = n as i16 as i32;
        // The original's idiv by 0 stops the program (rng_diff shows it); the port gives 0 here. A negative n runs on.
        if m == 0 {
            return 0;
        }
        let v = (r * m) / 0x7FFF;
        v - (v / m) * m
    }
}

/// Host state for one sector entry (section 7).
pub struct EnterHost<'a> {
    pub rng: Rng,
    iso: &'a mut Iso,
    /// JA.EXE in the game folder: op 86 reads the game-folder file first (iso::game_file); None = the CD only.
    pub exe: Option<String>,
    /// The open file: its bytes and the read position.
    file: Option<(Vec<u8>, usize)>,
    /// The tmp files the front-end keeps, by sector (op 92 open_tmp; world_load.md section 3 "Sector files").
    pub tmp_files: Vec<Option<Vec<u8>>>,
}

const NEG1: u32 = 0xFFFF_FFFF;

/// SaveSectorTmp 0x2F800 calls OpenMiscFile 0x38370 for `secNN.tmp`; the port keeps this file's bytes in memory.
/// Op 219 save_tmp (world_load.md section 8): the tmp file of the tiles and the ground items, 18,080 bytes: for each
/// tile 0..3135 its bytes 1, 2, 3, 4, 6, then `ground_items` bytes 0..0x95F (sector_start.md "Tmp file").
/// SaveSectorTmp 0x2F800.
pub fn make_tmp_file(tiles: &[u8], ground_items: &[u8]) -> Vec<u8> {
    let mut f = Vec::with_capacity(crate::world::TMP_FILE);
    for t in tiles.chunks_exact(8).take(3136) {
        f.extend_from_slice(&[t[1], t[2], t[3], t[4], t[6]]);
    }
    f.extend_from_slice(&ground_items[..0x960]);
    f
}

/// Host op 189 aim_dir (x1, y1, x2, y2) (shooting.md batch 5): the direction 1..8 (1 = north, clockwise; combat.md)
/// from the first pixel point to the second, from the arc tangent of the pixel difference (screen y grows down).
// SPEC GAP: the original's exact arc-tangent rounding is not described; this takes the nearest of the 8 directions
// (a tie goes clockwise) and gives 1 for the same point.
pub fn aim_dir(x1: i32, y1: i32, x2: i32, y2: i32) -> u32 {
    let (dx, dy) = ((x2 - x1) as f64, (y2 - y1) as f64);
    if dx == 0.0 && dy == 0.0 {
        return 1;
    }
    let deg = dx.atan2(-dy).to_degrees().rem_euclid(360.0);
    ((deg / 45.0 + 0.5).floor() as u32 % 8) + 1
}

impl<'a> EnterHost<'a> {
    pub fn new(iso: &'a mut Iso, seed: u32) -> EnterHost<'a> {
        EnterHost { rng: Rng::new(seed), iso, exe: None, file: None, tmp_files: Vec::new() }
    }

    /// The CD path of op 86 open_patch(kind, number, ...).
    pub fn patch_path(kind: i32, number: i32) -> Option<String> {
        let name = match kind {
            0 => "CRATE",
            1 => "ITEM",
            2 => "DEADPL",
            3 => "LIVEPL",
            4 => return Some("DAT\\XRA.DAT".to_string()),
            _ => return None,
        };
        Some(format!("MISC\\{}{:02}.BIN", name, number))
    }

    /// One host call; Err = stop the entry (the text names the op).
    pub fn call(&mut self, op: i32, args: &[i32], out: &mut [u8]) -> Result<u32, String> {
        let arg = |i: usize| args.get(i).copied().ok_or_else(|| format!("op {}: argument {} missing", op, i));
        match op {
            86 => {
                // SPEC GAP: a kind outside 0..4 is not described; it stops the entry.
                let path = Self::patch_path(arg(0)?, arg(1)?).ok_or_else(|| format!("op 86 open_patch: kind {}", args[0]))?;
                // SPEC GAP: a failed open leaves no open file (later reads give 0 bytes).
                match crate::iso::game_file(self.exe.as_deref(), self.iso, &path) {
                    Ok(b) => {
                        self.file = Some((b, 0));
                        Ok(0)
                    }
                    Err(_) => {
                        self.file = None;
                        Ok(NEG1)
                    }
                }
            }
            87 => {
                let n = arg(0)?.max(0) as usize;
                // SPEC GAP: n larger than the out buffer copies only out_size bytes; the result is the count copied.
                let Some((b, pos)) = self.file.as_mut() else { return Ok(0) };
                let k = n.min(out.len()).min(b.len().saturating_sub(*pos));
                out[..k].copy_from_slice(&b[*pos..*pos + k]);
                *pos += k;
                Ok(k as u32)
            }
            88 => {
                self.file = None;
                Ok(0)
            }
            96 => {
                // SPEC GAP: a negative position is taken as 0; past the end, reads give 0 bytes.
                let p = arg(0)?.max(0) as usize;
                if let Some((_, pos)) = self.file.as_mut() {
                    *pos = p;
                }
                Ok(0)
            }
            92 => {
                // Op 92 open_tmp(sector): the kept tmp file becomes the open file (position 0), else -1.
                let sector = arg(0)? as u8 as usize;
                match self.tmp_files.get(sector).and_then(|f| f.as_ref()) {
                    Some(b) => {
                        self.file = Some((b.clone(), 0));
                        Ok(0)
                    }
                    None => Ok(NEG1),
                }
            }
            // Op 93 sector_resets: no random draw (the library draws speech_wait itself, sector_start.md Setup sector).
            // Ops 198 mouse_wait, 315 mouse_release: Time compression (0) at the entry (world_load.md section 4
            // step 3); no mouse button is down at a sector entry, so nothing to wait for.
            89 | 93 | 94 | 95 | 198 | 315 => Ok(0),
            47 => Err("op 47 div_zero: the original game crashes here".to_string()),
            _ => Err(format!("op {} is not a host call of a sector entry", op)),
        }
    }
}

unsafe extern "C" fn cb_random(ctx: *mut c_void, n: i32) -> i32 {
    (*(ctx as *mut EnterHost)).rng.random(n)
}

unsafe extern "C" fn cb_call(ctx: *mut c_void, op: i32, args: *const i32, nargs: i32, out: *mut u8, out_size: i32) -> u32 {
    let h = &mut *(ctx as *mut EnterHost);
    let args = if args.is_null() || nargs <= 0 { &[][..] } else { std::slice::from_raw_parts(args, nargs as usize) };
    let out = if out.is_null() || out_size <= 0 { &mut [][..] } else { std::slice::from_raw_parts_mut(out, out_size as usize) };
    match h.call(op, args, out) {
        Ok(v) => v,
        Err(e) => {
            // "Any other op during the entry is an error: stop and name the op." Unwinding out of a C callback is
            // not possible, so the process stops here.
            eprintln!("ja1front: sector entry stopped: {}", e);
            std::process::exit(1)
        }
    }
}

extern "C" {
    fn ja1_enter_sector(w: *mut Ja1World, h: *const Ja1Host, entry_tile: i32);
    fn ja1_setup_sector(w: *mut Ja1World, h: *const Ja1Host, tile: i32, facing: i32);
}

/// Runs the library's `ja1_setup_sector (tile, facing)` on `w` with this host (host_ops = 0): the sector setup of a
/// traverse (soldier_update.md "Traverse to a neighbour sector", the move step 2).
pub fn setup_sector(w: &mut World, host: &mut EnterHost, tile: i32, facing: i32) {
    let mut cw = Ja1World::view(w);
    let h = Ja1Host { ctx: host as *mut EnterHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
    // SAFETY: as in enter_sector.
    unsafe { ja1_setup_sector(&mut cw, &h, tile, facing) };
    cw.store(w);
}

/// Runs the library's `ja1_enter_sector` on `w` with this host (host_ops = 0).
pub fn enter_sector(w: &mut World, host: &mut EnterHost, entry_tile: i32) {
    let mut cw = Ja1World::view(w);
    let h = Ja1Host { ctx: host as *mut EnterHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 };
    // SAFETY: every pointer in `cw` points into a buffer of `w` at its full port_api.md size; `w` is borrowed
    // mutably for the whole call, and the host callbacks do not touch it.
    unsafe { ja1_enter_sector(&mut cw, &h, entry_tile) };
    cw.store(w);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aim_directions() {
        // 1 = north (up on screen), clockwise to 8 = north-west.
        assert_eq!(aim_dir(100, 100, 100, 50), 1);
        assert_eq!(aim_dir(100, 100, 150, 50), 2);
        assert_eq!(aim_dir(100, 100, 150, 100), 3);
        assert_eq!(aim_dir(100, 100, 100, 150), 5);
        assert_eq!(aim_dir(100, 100, 50, 100), 7);
        assert_eq!(aim_dir(100, 100, 50, 50), 8);
    }

    /// MoveMercTo 0x1C7A8 leaves a detector move order only inside a TacticalClick with the detector in hand: a new
    /// view of the world holds none (slot 0xFF, tile -1), so the pass after it opens no confirm wait.
    #[test]
    fn a_new_view_has_no_detector_order() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut w = World::new_game(iso.to_str().unwrap(), exe.to_str().unwrap(), 0, 0).unwrap();
        let v = Ja1World::view(&mut w);
        assert_eq!((v.detector_slot, v.detector_tile), (0xFF, -1));
    }

    #[test]
    fn world_struct_size() {
        // The library's struct is 1024 bytes (guide_queue at 1016; strategic fields profiles .. sector_replay from 960; day accounts accounts .. sector_order from 936; inventory.md fields from 856: inv_cursor .. voice_test at 932; quit_tactical at 848; Update end fields from 812: ripple_timer .. fuse_beep at 840; select.md fields from 792: marks_shown, marks_n, marks at 800, the three bytes at 808..810; turns.md fast_timer at 788, click.md key_ctrl at 784, shooting.md batch 5 tables from 752, batch 4 fields from 732, batch 2 from 720, batch 1 from 680) (turns.md fields from 592, weather_tire at 644, merc_tire at 648, New turn
        // fields from 649, sector_spoken at 672) on a 64-bit target (ja1logic ai.rs layout test; soldier update fields
        // at the end, spotted at 536, opt_step_text at 540, seen_enemies at 544, fx_anims at 552, tick_step at 560, view_rays at 568, spot_stop at 585).
        if std::mem::size_of::<usize>() == 8 {
            assert_eq!(std::mem::size_of::<Ja1World>(), 1152);
            assert_eq!(std::mem::offset_of!(Ja1World, no_break), 1120);
            assert_eq!(std::mem::offset_of!(Ja1World, start_trees), 1060);
            assert_eq!(std::mem::offset_of!(Ja1World, item_told), 1088);
            assert_eq!(std::mem::offset_of!(Ja1World, bonus), 1098);
            assert_eq!(std::mem::offset_of!(Ja1World, guides_hired), 1056);
            assert_eq!(std::mem::offset_of!(Ja1World, guide_wait), 1057);
            assert_eq!(std::mem::offset_of!(Ja1World, idle_day), 1058);
            assert_eq!(std::mem::offset_of!(Ja1World, enemy_counter), 1059);
            assert_eq!(std::mem::offset_of!(Ja1World, clock_minute), 1024);
            assert_eq!(std::mem::offset_of!(Ja1World, day_minutes), 1028);
            assert_eq!(std::mem::offset_of!(Ja1World, guard_eta), 1040);
            assert_eq!(std::mem::offset_of!(Ja1World, guard_rec), 1048);
            assert_eq!(std::mem::offset_of!(Ja1World, accounts), 936);
            assert_eq!(std::mem::offset_of!(Ja1World, prev_balance), 944);
            assert_eq!(std::mem::offset_of!(Ja1World, profiles), 960);
            assert_eq!(std::mem::offset_of!(Ja1World, stash_counts), 968);
            assert_eq!(std::mem::offset_of!(Ja1World, stat_progress), 976);
            assert_eq!(std::mem::offset_of!(Ja1World, greed), 984);
            assert_eq!(std::mem::offset_of!(Ja1World, stat_changed), 992);
            assert_eq!(std::mem::offset_of!(Ja1World, duty7_merc), 1000);
            assert_eq!(std::mem::offset_of!(Ja1World, sector_replay), 1008);
            assert_eq!(std::mem::offset_of!(Ja1World, guide_queue), 1016);
            assert_eq!(std::mem::offset_of!(Ja1World, sector_order), 952);
            assert_eq!(std::mem::offset_of!(Ja1World, inv_cursor), 856);
            assert_eq!(std::mem::offset_of!(Ja1World, vest_owner), 904);
            assert_eq!(std::mem::offset_of!(Ja1World, key_flags), 918);
            assert_eq!(std::mem::offset_of!(Ja1World, drop_answer), 924);
            assert_eq!(std::mem::offset_of!(Ja1World, voice_test), 932);
            assert_eq!(std::mem::offset_of!(Ja1World, quit_tactical), 848);
            assert_eq!(std::mem::offset_of!(Ja1World, ripple_timer), 812);
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
            assert_eq!(std::mem::offset_of!(Ja1World, fx_id), 718);
            assert_eq!(std::mem::offset_of!(Ja1World, round_lock), 649);
            assert_eq!(std::mem::offset_of!(Ja1World, redraw_pending), 660);
            assert_eq!(std::mem::offset_of!(Ja1World, show_timer), 668);
            assert_eq!(std::mem::offset_of!(Ja1World, sector_spoken), 672);
            assert_eq!(std::mem::offset_of!(Ja1World, weather_tire), 644);
            assert_eq!(std::mem::offset_of!(Ja1World, merc_tire), 648);
            assert_eq!(std::mem::offset_of!(Ja1World, itq), 592);
            assert_eq!(std::mem::offset_of!(Ja1World, view_hold), 643);
            assert_eq!(std::mem::offset_of!(Ja1World, view_rays), 568);
            assert_eq!(std::mem::offset_of!(Ja1World, spot_stop), 585);
            assert_eq!(std::mem::offset_of!(Ja1World, key_shift), 541);
            assert_eq!(std::mem::offset_of!(Ja1World, opt_move_fx), 543);
            assert_eq!(std::mem::offset_of!(Ja1World, seen_enemies), 544);
            assert_eq!(std::mem::offset_of!(Ja1World, interrupt_turn), 548);
            assert_eq!(std::mem::offset_of!(Ja1World, fx_anims), 552);
            assert_eq!(std::mem::offset_of!(Ja1World, tick_step), 560);
            assert_eq!(std::mem::offset_of!(Ja1World, splash), 528);
            assert_eq!(std::mem::offset_of!(Ja1World, spotted), 536);
            assert_eq!(std::mem::offset_of!(Ja1World, opt_step_text), 540);
        }
    }

    #[test]
    fn watcom_rand() {
        let mut r = Rng::new(1);
        // seed 1 -> 0x41C67EA6, r = 0x41C6 = 16838 (Watcom's first rand() with seed 1).
        assert_eq!(r.random(0x7FFF), (16838i32 * 0x7FFF / 0x7FFF) % 0x7FFF);
        assert_eq!(r.seed, 0x41C6_7EA6);
        assert_eq!(r.random(0), 0);
        assert_eq!(r.random(-5), -1); // a negative n runs the same division (rng_diff)
        assert_eq!(r.draws, 3);
        let mut a = Rng::new(1793863554);
        for _ in 0..1000 {
            let v = a.random(3);
            assert!((0..3).contains(&v));
        }
    }

    #[test]
    fn patch_paths() {
        assert_eq!(EnterHost::patch_path(0, 7).unwrap(), "MISC\\CRATE07.BIN");
        assert_eq!(EnterHost::patch_path(3, 123).unwrap(), "MISC\\LIVEPL123.BIN");
        assert_eq!(EnterHost::patch_path(1, 0).unwrap(), "MISC\\ITEM00.BIN");
        assert_eq!(EnterHost::patch_path(4, 9).unwrap(), "DAT\\XRA.DAT");
        assert!(EnterHost::patch_path(5, 0).is_none());
    }

    #[test]
    fn tmp_file_layout() {
        let mut tiles = vec![0u8; 3136 * 8];
        for (i, b) in tiles.iter_mut().enumerate() {
            *b = (i % 8) as u8 + 10 * ((i / 8) % 7) as u8;
        }
        let ground: Vec<u8> = (0..2400).map(|i| (i * 7) as u8).collect();
        let f = make_tmp_file(&tiles, &ground);
        assert_eq!(f.len(), 18080);
        for t in [0usize, 1, 500, 3135] {
            let want: Vec<u8> = [1, 2, 3, 4, 6].iter().map(|&o| tiles[t * 8 + o]).collect();
            assert_eq!(&f[t * 5..t * 5 + 5], &want[..]);
        }
        assert_eq!(&f[3136 * 5..], &ground[..]);
    }
}
