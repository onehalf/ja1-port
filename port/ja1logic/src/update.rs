//! Soldier update per frame (docs/spec/soldier_update.md; C interface in docs/spec/port_api.md "Soldier update
//! entry points"). A child module of `ai`: it extends the `Ai` view of one soldier record.
//! Library ops made here (1, 6, 7, 17, 29, 40, 41, 52, 53, 62, 84, 97, 98, 106, 109, 111, 117, 119, 122, 137, 138) go through `Ai::call`, the one dispatch point that applies
//! the host_ops rule. Every other op is a host call. World data is read again after every host call.
use super::fire::vt;
use super::search::{px, py, s16, step, world_dist};
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT, SLOTS, TILES};

const NONE_I: i32 = NONE as i32;
const ITEM_TYPES: usize = 136;
const GROUND: usize = 200;
const SECTOR_ROWS: usize = 60;
const SECTOR_ROW: usize = 0xB1;
const CLASS_LEN: usize = 256;

/// Byte i of a world buffer of `len` bytes; null or out of range reads as 0.
fn buf8(p: *const u8, len: usize, i: usize) -> u8 {
    if p.is_null() || i >= len {
        return 0;
    }
    unsafe { p.add(i).read() }
}

/// Tile parts (C division).
fn row(t: i32) -> i32 {
    t / 56
}
fn column(t: i32) -> i32 {
    t - row(t) * 56
}

/// Water tile (t0, t1).
fn water_tile(t0: u8, t1: u8) -> bool {
    t0 >= 0x6C && t1 != 0x1C && t1 != 0x1F
}

/// Step energy of a move code.
fn step_energy(m: u16) -> i32 {
    match m {
        1000 => 5,
        1200 => 10,
        1500 => 5,
        1600 => 20,
        2000 => 35,
        3000 => 50,
        3600 => 75,
        5000 => 100,
        25300 => 5,
        25400 => 5,
        _ => 0,
    }
}

impl Ai {
    // ---- access ----
    fn u_s8_of(&self, slot: usize, off: usize) -> i32 {
        self.rd8_of(slot, off) as i8 as i32
    }
    fn u_wr8_of(&self, slot: usize, off: usize, v: u8) {
        if slot >= SLOTS || off >= 0x200 {
            return;
        }
        unsafe {
            let p = (*self.w).soldiers;
            if !p.is_null() {
                p.add(slot * 0x200 + off).write(v);
            }
        }
    }
    /// The current animation word of record r.
    fn u_cur_of(&self, r: usize) -> u16 {
        self.rd16_of(r, 0x1E8 + 2 * self.rd8_of(r, 0x1F3) as usize)
    }
    fn u_cur(&self) -> u16 {
        self.cur_anim()
    }
    fn u_code(&self) -> u8 {
        self.cur_anim() as u8
    }
    /// still(a) = anim_rows[8 a + 3].
    fn u_still(&self, a: u16) -> u8 {
        self.row_byte(a as i32, 3)
    }
    /// can act(a) = anim_table[8 a + 0].
    fn u_can_act(&self, a: u16) -> u8 {
        self.anim_col(a, 0)
    }
    /// turns(a) = anim_table[8 a + 2].
    fn u_turns(&self, a: u16) -> u8 {
        self.anim_col(a, 2)
    }
    pub(super) fn u_class(&self, p: *const u8, v: u8) -> u8 {
        buf8(p, CLASS_LEN, v as usize)
    }
    pub(super) fn u_move_code(&self, t: i32) -> u16 {
        // Off-map tiles read 0.
        if !(0..TILES as i32).contains(&t) {
            return 0;
        }
        let p = unsafe { (*self.w).move_codes } as *const u8;
        let i = 2 * t as usize;
        buf8(p, 2 * TILES, i) as u16 | (buf8(p, 2 * TILES, i + 1) as u16) << 8
    }
    /// Byte b of ground entry g (0..199); any other index reads 0.
    fn u_ground(&self, g: u8, b: usize) -> u8 {
        if g as usize >= GROUND {
            return 0;
        }
        buf8(unsafe { (*self.w).ground_items }, GROUND * 12, 12 * g as usize + b)
    }
    pub(super) fn u_ap_warned(&self) -> u8 {
        // SPEC GAP: ap_warned has 8 entries (mercs); another slot reads 0.
        if self.s >= 8 {
            return 0;
        }
        buf8(unsafe { (*self.w).ap_warned }, 8, self.s)
    }
    fn u_sel(&self) -> u8 {
        unsafe { (*self.w).selected }
    }
    fn u_pp(&self) -> i32 {
        (unsafe { (*self.w).path_preview } != 0) as i32
    }
    fn u_follow_all(&self) -> bool {
        unsafe { (*self.w).follow_all != 0 }
    }
    fn u_turn_moves(&self) -> bool {
        unsafe { (*self.w).turn_moves != 0 }
    }
    fn u_seen_i(&self) -> i32 {
        self.seen() as i32
    }

    // ---- helpers (soldier_update.md "Helpers") ----
    pub(super) fn u_tile_point_stop(&self, flag: i32) {
        let t = self.s16_at(0x114);
        self.wr16(0x126, px(t) as u16);
        self.wr16(0x128, py(t) as u16);
        self.call(op::STOP_ANIM, &[self.slot(), flag]);
    }
    fn u_at_point(&self) -> bool {
        self.rd16(0x126) == self.rd16(0x116) && self.rd16(0x128) == self.rd16(0x118)
    }
    fn u_stored_flag(&self) -> i32 {
        (self.rd16(0x120) == self.rd16(0x114) || self.s16_at(0x120) >= NONE_I) as i32
    }
    /// At the waypoint of record r: 1 / 0.
    pub(super) fn u_at_waypoint(&self, r: usize) -> bool {
        !(self.rd8_of(r, 0x1B9) < 5
            && self.u_cur_of(r) >= 4
            && (self.rd16_of(r, 0x114) != self.rd16_of(r, 0x124)
                || self.rd16_of(r, 0x116) != self.rd16_of(r, 0x126)
                || self.rd16_of(r, 0x118) != self.rd16_of(r, 0x128)))
    }
    /// Ground index of tile t: 0xFF if byte 6 >= 200.
    fn u_ground_index(&self, t: i32) -> u8 {
        let g = self.tb(t, 6);
        if g >= 200 { 0xFF } else { g }
    }
    /// Step AP of tile t (low byte).
    fn u_step_ap(&self, t: i32) -> u8 {
        crate::step_ap(self.u_move_code(t), self.rd8(0x1A8), self.rd8(0x177), self.rd8(0x196)) as u8
    }
    /// Low-AP stop; `with_16b`: stop_timer only if +0x16B != 0.
    fn u_low_ap_stop(&self, with_16b: bool) {
        // SPEC GAP: ap_warned has 8 entries; for a slot of 8 and up the write is skipped.
        if self.s < 8 {
            let p = unsafe { (*self.w).ap_warned };
            if !p.is_null() {
                unsafe { p.add(self.s).write(1) };
            }
        }
        if !with_16b || self.rd8(0x16B) != 0 {
            unsafe { (*self.w).stop_timer = 2000 };
        }
        self.u_tile_point_stop(0);
        self.wr8(0x1A6, 0);
        self.wr16(0x134, NONE);
        self.wr8(0x1A7, 0);
        self.wr8(0x16D, 1);
        self.call(op::LOW_AP_STOP, &[self.slot(), 1, 0]);
    }
    pub(super) fn u_forget_attack(&self, flag: bool) {
        self.wr8(0x1D8, NO_SLOT);
        self.wr8(0x1C2, NO_SLOT);
        self.wr8(0x1BE, 0);
        self.wr16(0xD0, NONE);
        self.wr16(0xDC, NONE);
        if flag && self.u_turns(self.u_cur()) != 0 {
            self.call(op::ANIM_STEP, &[self.slot()]);
        }
    }
    fn u_exhausted_fall(&self) {
        let c = self.u_cur();
        if self.s8(0x183) < 15 && (4..0x28).contains(&c) {
            self.u_tile_point_stop(1);
            self.call(op::UI_REFRESH, &[self.u_pp()]);
            let code = if self.in_water() { 0x40 } else { 0x2B };
            self.call(op::SET_ANIM, &[self.slot(), code, 0, 0]);
        }
    }
    fn u_soak_items(&self) {
        for k in 0..12 {
            let ty = self.rd8(12 * k);
            if ty == 0 || buf8(unsafe { (*self.w).item_types }, ITEM_TYPES * 12, 12 * ty as usize + 8) == 0 {
                continue;
            }
            let r = self.random(100);
            if r < 10 {
                // SPEC GAP: "if s8 byte 2 < 0" is read as part of the r < 10 step.
                self.wr8(12 * k + 2, self.rd8(12 * k + 2).wrapping_sub(r as u8));
                if (self.rd8(12 * k + 2) as i8) < 0 {
                    let v = self.random(8) + 2;
                    self.wr8(12 * k + 2, v as u8);
                }
            }
        }
    }
    fn u_fire_trigger(&self, index: i32, keep: bool) {
        let slot = self.slot();
        self.call(op::MERC_MESSAGE, &[slot, 4, 0x40, 0, 1]);
        self.u_tile_point_stop(1);
        if !keep {
            self.call(op::CONSUME_TRIGGER, &[index]);
        }
    }
    fn u_sector_row(&self, i: usize) -> u8 {
        // SPEC GAP: a current sector of 60 and up (no row) reads 0.
        let s = unsafe { (*self.w).cur_sector } as usize;
        if s >= SECTOR_ROWS {
            return 0;
        }
        buf8(unsafe { (*self.w).sectors }, SECTOR_ROWS * SECTOR_ROW, s * SECTOR_ROW + i)
    }
    fn u_tile_triggers(&self) {
        let slot = self.slot();
        for i in 0..10usize {
            let tile = self.u_sector_row(0x58 + 4 * i) as u16 | (self.u_sector_row(0x59 + 4 * i) as u16) << 8;
            let value = (self.u_sector_row(0x5A + 4 * i) as u16 | (self.u_sector_row(0x5B + 4 * i) as u16) << 8)
                as i16 as i32;
            if self.rd16(0x114) != tile {
                continue;
            }
            let skill = self.s8(0x192);
            if value < 0 {
                if skill <= value.abs() {
                    self.u_fire_trigger(i as i32, false);
                    self.call(op::STAT_PRACTICE, &[slot, 4, 5, 0]);
                }
            } else if skill >= value {
                self.u_fire_trigger(i as i32, false);
                self.call(op::STAT_PRACTICE, &[slot, 4, 10, 0]);
            }
        }
    }

    // ---- Walk move ----
    fn u_walk_move(&self) -> i32 {
        let (mut x, mut y) = (self.s16_at(0x116), self.s16_at(0x118));
        let (tx, ty) = (self.s16_at(0x126), self.s16_at(0x128));
        let v = self.rd8(0x19F) as i32;
        if x < tx {
            x = (x + v).min(tx);
        } else if x > tx {
            x = (x - v).max(tx);
        }
        if y < ty {
            y = (y + v).min(ty);
        } else if y > ty {
            y = (y - v).max(ty);
        }
        self.wr16(0x116, x as u16);
        self.wr16(0x118, y as u16);
        match self.rd8(0x1A2) {
            2 => x += 5,
            4 => x += 1,
            6 => y += 3,
            _ => {}
        }
        s16(x) / 16 + s16(y) / 14 * 56
    }

    // ---- Walk step ----
    pub(super) fn walk_step(&self) {
        let slot = self.slot();
        // 1.
        self.wr8(0x193, 1);
        let old = self.s16_at(0x114);
        let (ox, oy, speed) = (self.rd16(0x116), self.rd16(0x118), self.rd8(0x19F));
        let a = self.u_code();
        // 2.
        let new = if self.u_still(a as u16) == 0 {
            self.u_walk_move()
        } else {
            if a < 4 && self.s8(0x1A5) < 5 {
                self.wr8(0x1B8, 1);
                self.call(op::STOP_WALK, &[slot, 0]);
                if self.s8(0xBA) >= 1 {
                    self.wr8(0x1B0, 1);
                }
            }
            old
        };
        // 3.
        if new == old {
            self.call(op::ANIM_NEXT, &[slot]);
            return;
        }
        // 4.
        let b5 = self.tb(new, 5) & 0x7F;
        let o = if b5 >= 0x30 { NO_SLOT } else { b5 };
        if o != NO_SLOT {
            self.wr16(0x116, ox);
            self.wr16(0x118, oy);
            self.wr8(0x19F, speed);
            self.call(op::WALK_BLOCKED, &[slot, s16(new), o as i32]);
            return;
        }
        // 5.
        let c = self.u_cur();
        if self.s < 8
            && self.s8(0x18A) < 0x14
            && self.rd8(0x1BA) == 0
            && (0x15..=0x17).contains(&c)
            && self.rd8(0x1D5) == 0
        {
            self.wr8(0x1BA, 1);
            self.call(op::MERC_MESSAGE, &[slot, 0x1E, 0x40, 0, 1]);
        }
        // 6.
        self.vacate(self.s16_at(0x114));
        self.wr16(0x114, new as u16);
        let (b0, b1, b2) = (self.tb(new, 0), self.tb(new, 1), self.tb(new, 2));
        self.wr8(0x1B2, b0);
        self.wr8(0x1B3, self.u_class(unsafe { (*self.w).t0_class }, self.rd8(0x1B2)));
        self.wr8(0x1B4, b1);
        self.wr8(0x1B5, self.u_class(unsafe { (*self.w).t1_class }, self.rd8(0x1B4)));
        self.wr8(0x1B6, b2);
        self.wr8(0x1B7, self.u_class(unsafe { (*self.w).t2_class }, self.rd8(0x1B6)));
        // 7.
        self.anim_delay();
        // 8.
        if self.s16_at(0x1F8) != NONE_I
            && (self.call(op::TILE_DIST, &[self.s16_at(0x114), self.s16_at(0x1F8)]) as i32) > 8
        {
            self.wr16(0x1F8, NONE);
        }
        // 9.
        self.wr8(0x1AE, 0);
        self.wr8(0x1F2, self.rd8(0x1F2).wrapping_add(1));
        self.call(op::ANIM_NEXT, &[slot]);
        // 10.
        let sel = self.u_sel();
        if sel as usize == self.s
            && (self.seen() || self.u_follow_all())
            && self.w_combat()
            && (self.s >= 8 || unsafe { (*self.w).opt_follow } != 0)
        {
            self.call(op::FOCUS, &[sel as i32, 1]);
        }
        // 11.
        if self.rd8(0x1B2) < 0x97 || self.rd8(0x1B4) != 0 || self.s8(0x183) <= 0 {
            return;
        }
        // SPEC GAP: a current sector of 60 and up (or a null table) gives n = 0; random(n) is drawn for any n.
        let cs = unsafe { (*self.w).cur_sector } as usize;
        let n = buf8(unsafe { (*self.w).splash }, SECTOR_ROWS, cs) as i8 as i32;
        if self.random(n) != 0 {
            return;
        }
        if self.u_cur() >= 0x3C || self.rd8(0x179) >= 0x3C {
            if self.random(5) != 1 {
                return;
            }
        }
        // 12.
        self.call(op::SET_ANIM, &[slot, 0x3D, 0, 1]);
        if self.rd8(0x179) < 0x40 {
            self.call(op::FOCUS, &[slot, 1]);
            self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 0, 1, 1]);
            self.call(op::VOICE, &[0x2B11, 0x32, 1]);
        }
    }

    // ---- Walk blocked (op 98) ----
    pub(super) fn walk_blocked(&self, tile: i32, occupant: u8) {
        let slot = self.slot();
        // SPEC GAP: target is read once at the start ("target = s16 +0x124"), not again after host calls.
        let target = self.s16_at(0x124);
        // 1.
        if self.w_combat() {
            if tile == target {
                self.u_tile_point_stop(1);
                self.call(op::ANIM_STEP, &[slot]);
            } else {
                self.u_tile_point_stop(0);
            }
            return;
        }
        if occupant == NO_SLOT {
            // 2.
            if self.tile_standable(target) != 1 {
                if tile == target || self.call(op::FIND_PATH, &[slot, target, 1]) == 0 {
                    self.u_tile_point_stop(1);
                } else {
                    self.start_leg();
                }
                return;
            }
        } else {
            // 3.
            if self.tile_standable(target) < 1
                && (self.call(op::TILE_DIST, &[self.s16_at(0x114), target]) as i32) < 4
            {
                self.u_tile_point_stop(1);
                return;
            }
            self.wr8(0x1AE, self.rd8(0x1AE).wrapping_add(1));
            let patience = if self.s < 8 { 0x32 } else { 0 };
            if self.rd8(0x1AE) > patience {
                self.u_tile_point_stop(1);
                return;
            }
            if occupant >= NO_SLOT || !self.u_at_waypoint(occupant as usize) {
                return;
            }
            if tile == target {
                self.u_tile_point_stop(1);
                return;
            }
        }
        // 4.
        self.wr16(0x12A, self.rd16(0x124));
        self.start_leg();
    }

    // ---- Walk end ----
    pub(super) fn walk_end(&self, code: u8) {
        let slot = self.slot();
        // 1.
        if self.s8(0x18A) <= 0 && (4..0x32).contains(&code) {
            self.call(op::UI_REFRESH, &[self.u_pp()]);
            self.call(op::SET_ANIM, &[slot, 0x2B, 0, 0]);
        } else if self.rd16(0x114) == self.rd16(0x120) || self.s16_at(0x120) == NONE_I {
            if code < 0x32 {
                if self.s16_at(0xDC) == NONE_I {
                    self.call(op::SET_ANIM, &[slot, 4, 0, 0]);
                }
            } else {
                let g = self.rd8(0x1B2);
                let n = if g < 0x97 { 0x33 } else if g < 0x9A { 0x36 } else { 0x39 };
                self.call(op::SET_ANIM, &[slot, n, 0, 0]);
            }
        }
        // 2.
        self.call(op::STOP_ANIM, &[slot, self.u_stored_flag()]);
        self.call(op::STOP_WALK, &[slot, 0]);
        self.wr8(0x1B8, 1);
        // 3.
        if self.s >= 0x28 {
            let p = unsafe { (*self.w).enemy_watch };
            if !p.is_null() {
                unsafe { p.add(self.s).write(1) };
            }
        }
        if self.s >= 8 && self.rd8(0x197) != 0 {
            self.wr8(0x1B0, 1);
        }
    }

    // ---- Stand up ----
    pub(super) fn stand_up(&self) {
        let slot = self.slot();
        // 1.
        if unsafe { (*self.w).screen_drawn } != 0 {
            if self.s >= 0x28 || (self.s >= 8 && self.w_combat()) {
                return;
            }
        } else if self.s < 8 {
            return;
        }
        // 2.
        let a = self.u_cur();
        if a >= 3 || self.rd8(0x1B9) != 0 {
            return;
        }
        self.wr16(0xD0, NONE);
        // 3.
        if self.s8(0x18A) < 10 || a > 1 || self.s8(0x1A5) < 5 {
            return;
        }
        // 4.
        self.wr8(0x1CC, 1);
        self.call(op::SET_ANIM, &[slot, 4, 0, 0]);
        self.u_tile_point_stop(1);
        self.wr8(0x1B8, 0);
        self.wr8(0x1C1, 1);
    }

    // ---- Merc quote ----
    pub(super) fn merc_quote(&self) {
        let slot = self.slot();
        // 1.
        if self.s as u8 != unsafe { (*self.w).quote_slot } || self.s >= 8 {
            return;
        }
        // 2.
        let d = unsafe { (*self.w).quote_delay };
        if d > 0 {
            unsafe { (*self.w).quote_delay = d.wrapping_sub(1) };
            return;
        }
        // 3.
        let e = unsafe { (*self.w).quote_event };
        if e < 0x21 {
            let qt = unsafe { (*self.w).quote_tile } as i16 as i32;
            self.call(op::MERC_MESSAGE, &[slot, e as i32, qt, 0, 1]);
            if e == 5 {
                self.u_forget_attack(true);
            }
        } else if (0xF9..=0xFD).contains(&e) {
            self.call(op::CRY, &[self.rd8(0x179) as i32, -1, (e - 0xF9) as i32, 1, 1]);
            if (self.rd32(0xC0) as i32) <= 0 && self.rd8(0x16C) == 0 {
                if unsafe { (*self.w).quote_focus } != 0 {
                    self.call(op::FOCUS, &[slot, 1]);
                }
                self.call(op::VOICE, &[0x2B11, 0x58, 1]);
                self.wr32(0xC0, 400);
            }
        }
        // 4.
        unsafe {
            (*self.w).quote_focus = 0;
            (*self.w).quote_slot = NO_SLOT;
            (*self.w).quote_delay = 0;
            (*self.w).quote_tile = NONE;
        }
    }

    // ---- Frame checks ----
    pub(super) fn frame_checks(&self) {
        let slot = self.slot();
        // 1.
        let code = self.u_code();
        if self.s8(0x18A) <= 0 && (4..0x28).contains(&code) && self.rd8(0x186) == 0 {
            self.call(op::UI_REFRESH, &[self.u_pp()]);
            self.call(op::END_TASK, &[slot, 1]);
            self.call(op::SET_ANIM, &[slot, 0x2B, 0, 0]);
        }
        // 2.
        if self.rd32(0xB4) == 0
            && unsafe { (*self.w).opt_reload_msg } != 0
            && self.rd8(0x01) == 0
            && self.rd8(0x00) != 0
            && self.rd8(0x1BF) == 0
            && (self.call(op::RELOAD_CHECK, &[slot]) as i32) != 0
            && self.rd8(0x1CD) == 0
        {
            self.call(op::QUOTE_EVENT, &[slot, 5, 1, NONE_I, 1]);
            self.wr8(0x1CD, 1);
            self.wr32(0xB4, 0xFFFF_FFFF);
        }
        // 3.
        self.merc_quote();
    }

    // ---- Enemies see ----
    pub(super) fn enemies_see(&self) {
        let slot = self.slot();
        // 1.
        unsafe { (*self.w).interrupted = 1 };
        // 2.
        for k in 0x28..0x30usize {
            if self.rd8_of(k, 0x182) != 0 && self.rd8_of(k, 0x138) != 0 && self.u_s8_of(k, 0x183) >= 15 {
                if self.call(op::SIGHT_CHECK, &[k as i32, slot]) & 0xFF != 0 {
                    self.call(op::ASK_INTERRUPT, &[k as i32, slot]);
                }
            }
        }
    }

    // ---- Enemy step seen (op 111 enemy_step) ----
    pub(super) fn enemy_step_seen(&self) {
        let slot = self.slot();
        // 1.
        if self.s8(0x9C) > 0 {
            self.wr8(0x9C, 0);
        }
        // 2.
        unsafe { (*self.w).interrupted = 1 };
        // 3.
        for k in 0..16usize {
            if self.rd8_of(k, 0x182) != 0 && self.rd8_of(k, 0x138) != 0 && self.u_s8_of(k, 0x183) >= 15 {
                if self.call(op::SIGHT_CHECK, &[k as i32, slot]) & 0xFF != 0 {
                    self.call(op::SHARE_SIGHTINGS, &[k as i32, 0]);
                    self.call(op::ASK_MERC_INTERRUPT, &[k as i32, slot]);
                }
            }
        }
        // 4.
        if self.s8(0x9C) < 1 && unsafe { (*self.w).spotted } == slot {
            unsafe { (*self.w).spotted = NO_SLOT as i32 };
        }
    }

    // ---- Step noise (op 117 step_noise) ----
    pub(super) fn step_noise(&self) {
        let slot = self.slot();
        // 1.
        let mut q = self.s8(0x192) * 5 + self.s8(0x18C) / 2;
        // 2.
        let h = ((self.rd8(0x184) as i32 - self.rd8(0x183) as i32 - self.rd8(0x185) as i32) & 0xFF) / 2
            + self.s8(0x183);
        let h = h & 0xFF;
        if h < 50 {
            let q8 = q & 0xFF;
            q = q8 - ((50 - h) * q8) / 100;
        }
        // 3.
        let e = self.s8(0x18A);
        if e < 50 {
            let q8 = q & 0xFF;
            q = q8 - ((50 - e) * q8) / 100;
        }
        // 4.
        q &= 0xFF;
        if self.in_water() {
            q = (q - 10) & 0xFF;
        }
        if (self.s < 8 && self.rd8(0x19C) == 3) || (self.s >= 8 && self.rd8(0x1D3) == 5) {
            q = (q - 10) & 0xFF;
        }
        let loud;
        if self.rd8(0x1A8) == 0 {
            // 5. Walking.
            let n = 6 - q / 20 + self.in_water() as i32;
            loud = (self.random(n & 0xFF) + 1) & 0xFF;
        } else {
            // 6. Sneaking.
            let mut r = self.random(100) & 0xFF;
            if r >= q {
                r = self.random(100) & 0xFF;
            }
            if r < q {
                loud = 0;
            } else {
                loud = ((r - q + 1) / 20 + 1) & 0xFF;
                if self.s < 8 {
                    let (fx, text) = if self.in_water() {
                        (if self.random(2) == 0 { 0x1F } else { 0x1E }, if loud >= 3 { 1 } else { 0 })
                    } else if self.rd8(0x1B3) == 3 {
                        (0x2A, if loud >= 3 { 3 } else { 2 })
                    } else {
                        (if self.random(2) != 0 { 0x1C } else { 0x2B }, if loud >= 3 { 5 } else { 4 })
                    };
                    const TEXTS: [&[u8]; 6] = [b"splash", b"SPLASH!", b"clunk", b"CLUNK!", b"snap", b"SNAP!"];
                    let p = unsafe { (*self.w).text_buf };
                    // SPEC GAP: a null text_buf is not written.
                    if !p.is_null() {
                        let t = TEXTS[text as usize];
                        unsafe {
                            for (i, &b) in t.iter().enumerate() {
                                p.add(i).write(b);
                            }
                            p.add(t.len()).write(0);
                        }
                    }
                    let v = ((loud * 15 + 0x23) & 0xFF) as u8 as i8 as i32;
                    self.call(op::SFX, &[fx, 0x2B11, v, 1]);
                    if unsafe { (*self.w).opt_step_text } != 0 {
                        self.call(op::STEP_TEXT, &[text]);
                    }
                }
            }
        }
        // 7.
        if loud != 0 {
            let kind = if self.in_water() { 3 } else { 1 };
            self.call(op::MAKE_NOISE, &[slot, self.s16_at(0x114), self.rd8(0x1B3) as i32, loud, kind]);
        }
    }

    // ---- Throw follow-up (op 122 throw_follow) ----
    pub(super) fn throw_follow(&self) {
        let slot = self.slot();
        // 1.
        let t = self.s16_at(0xDC);
        if t == NONE_I || self.item_kind(self.rd8(0x00)) != 0x95 {
            return;
        }
        // 2.
        self.throw_aim(t);
        // 3.
        let d = self.call(
            op::DIRECTION,
            &[self.s16_at(0x116) + 2, self.s16_at(0x118) + 4, self.s16_at(0xDE), self.s16_at(0xE0)],
        );
        self.wr8(0x1A2, d as u8);
        // 4.
        self.call(op::SET_ANIM, &[slot, 0x27, 0, 1]);
    }

    // ---- Throw aim ----
    pub(super) fn throw_aim(&self, t: i32) {
        let slot = self.slot();
        let t = s16(t);
        // 1.
        if self.w_combat() {
            if self.s >= 8 && self.s8(0x9C) > 0 {
                self.call(op::FOCUS, &[slot, 0]);
            }
            unsafe { (*self.w).effect_tile = t as i16 };
        }
        // 2.
        let ty = self.rd8(0x00);
        if self.item_kind(ty) != 0x95 {
            return;
        }
        // 3.
        let a = ((self.call(op::ACTION_AP, &[slot, t, 1]) as u8 as i32) + self.rd8(0x1FD) as i32) & 0xFF;
        self.call(op::SPEND_AP, &[slot, a, 0]);
        // 4.
        self.wr16(0xDE, px(t) as u16);
        self.wr16(0xE0, py(t) as u16);
        self.wr16(0xE2, 0);
        // 5.
        let d = s16(world_dist(self.s16_at(0x116), self.s16_at(0x118), self.s16_at(0xDE), self.s16_at(0xE0)));
        // 6.
        let ch = self.throw_chance(t, self.rd8(0x1FD)) as u8;
        let r = ((self.random(100) + 1) & 0xFF) as u8;
        if (r as i8) > (ch as i8) {
            self.wr8(0x1BC, 0);
            self.throw_scatter(r.wrapping_sub(ch), d);
        } else {
            self.wr8(0x1BC, 1);
            self.wr16(0xDC, t as u16);
        }
        // 7.
        let mut reach = self.range(ty) * 15 + self.s8(0x183) - 50;
        reach -= self.cdiv((100 - self.s8(0x18A)) * reach, 200);
        if reach < 0 {
            reach = 0;
        }
        // 8.
        if reach < d {
            let dx = s16(self.rd16(0xDE).wrapping_sub(self.rd16(0x116)) as i32);
            let dy = s16(self.rd16(0xE0).wrapping_sub(self.rd16(0x118)) as i32);
            let x = self.cdiv(dx * reach, d) + self.s16_at(0x116);
            self.wr16(0xDE, x as u16);
            let y = self.cdiv(dy * reach, d) + self.s16_at(0x118);
            self.wr16(0xE0, y as u16);
            self.wr16(0xDC, vt(self.s16_at(0xDE), self.s16_at(0xE0)) as u16);
        }
        // 9.
        self.wr16(0xD0, t as u16);
    }

    // ---- Throw scatter ----
    pub(super) fn throw_scatter(&self, margin: u8, dist: i32) {
        let dist = s16(dist);
        let f = self.rd8(0x1A1);
        let xm = matches!(f, 1 | 2 | 4 | 5 | 6 | 8);
        let ym = matches!(f, 2 | 3 | 4 | 6 | 7 | 8);
        // 1.
        let mut q1 = self.cdiv(dist, 30);
        if s16(q1) < 1 {
            q1 = 1;
        }
        let mut q2 = margin as i32 / 15;
        if q2 < 1 {
            q2 = 1;
        }
        let size = q1 * q2;
        // 2.
        let (mut x, mut y);
        if xm && ym {
            x = self.cdiv(s16(size), 2);
            y = x;
        } else {
            x = if xm { size } else { 0 };
            y = if ym { size } else { 0 };
        }
        // 3.
        if xm && s16(x) < 4 {
            x = 4;
        }
        if ym && s16(y) < 4 {
            y = 4;
        }
        // 4.
        if xm && ym {
            let r = self.random(2);
            match f {
                2 | 6 => {
                    if r != 0 {
                        x = -x;
                        y = -y;
                    }
                }
                _ => {
                    // f = 4 or 8.
                    if r != 0 {
                        x = -x;
                    } else {
                        y = -y;
                    }
                }
            }
        } else {
            if xm && self.random(2) != 0 {
                x = -x;
            }
            if ym && self.random(2) != 0 {
                y = -y;
            }
        }
        // 5.
        self.wr16(0xDE, self.rd16(0xDE).wrapping_add(x as u16));
        self.wr16(0xE0, self.rd16(0xE0).wrapping_add(y as u16));
        self.wr16(0xDC, vt(self.s16_at(0xDE), self.s16_at(0xE0)) as u16);
    }

    // ---- Move to the patrol tile ----
    // ---- Path ops (ops 1, 29, 40, 41) ----
    /// Find path (op 41): 1 = a path exists (stored when the low 16 bits of `flag` are not 0), 0 = no path.
    pub(super) fn find_path_op(&self, tile: i32, flag: i32) -> i32 {
        let w = self.w;
        let start = s16(self.rd16(0x114) as i32);
        // SPEC GAP: a start (s16 +0x114) off the map also gives "no path" (the page names only the destination).
        if !(0..TILES as i32).contains(&start) || !(0..TILES as i32).contains(&tile) {
            return 0;
        }
        let mut codes = vec![0u16; TILES];
        let mut ground = vec![0u8; TILES];
        let mut occ = vec![0u8; TILES];
        for t in 0..TILES {
            codes[t] = self.u_move_code(t as i32);
            ground[t] = self.tile_byte(t as u16, 0);
            // occupied = (byte 5 & 0x7F) < 0x30; find_path treats a value below 0x40 as occupied.
            occ[t] = self.occupant(t as u16);
        }
        let p = crate::PathIn {
            codes: &codes,
            ground: &ground,
            occ: &occ,
            b177: self.rd8(0x177),
            bb9: self.rd8(0xB9),
            slot: self.s as u8,
            combat: (unsafe { (*w).combat } != 0) as u8,
        };
        let Some(dirs) = crate::find_path(start as usize, tile as usize, &p) else { return 0 };
        if flag as u16 != 0 {
            let n = dirs.len().min(80);
            let paths = unsafe { (*w).paths };
            if !paths.is_null() {
                for (k, &d) in dirs[..n].iter().enumerate() {
                    unsafe { paths.add(80 * self.s + k).write(d) };
                }
            }
            self.wr16(0xD4, n as u16);
            self.wr8(0x1AF, 0);
        }
        1
    }
    /// Path AP (op 40 = sneak, byte177, alt all 0): the AP price of a walk to `tile`; stores walk_ap.
    pub(super) fn path_ap_op(&self, tile: i32, sneak: u8, b177: u8, alt: u8) -> i32 {
        self.path_ap_marks(tile, 0, 0, sneak, b177, alt, false)
    }
    /// Path AP with marks (select.md): `mark` draws the path preview, `extra` prices one step more. `own_find`:
    /// Find path is the library's own procedure (Select merc), else op 41 (a host call when host_ops claims it).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn path_ap_marks(&self, tile: i32, mark: u8, extra: u8, sneak: u8, b177: u8, alt: u8, own_find: bool)
        -> i32 {
        let w = self.w;
        let set_walk = move |v: i32| unsafe { (*w).walk_ap = v };
        let m = mark != 0 && unsafe { (*w).opt_path_marks } != 0;
        let put_mark = move |i: i32, v: u16| {
            // SPEC GAP: a list index outside the 128 entries is not written.
            let p = unsafe { (*w).marks };
            if !p.is_null() && (0..128).contains(&i) {
                unsafe {
                    p.add(2 * i as usize).write(v as u8);
                    p.add(2 * i as usize + 1).write((v >> 8) as u8);
                }
            }
        };
        // 1.
        if mark != 0 && unsafe { (*w).marks_shown } != 0 {
            return 0;
        }
        // 2.
        set_walk(0);
        let mut c: i32 = 0;
        let mut up: i32 = 0;
        // 3. Find path runs as the library procedure (or the host's op 41 when host_ops claims it).
        let found = if own_find { self.find_path_op(tile, 1) as u32 } else { self.call(op::FIND_PATH, &[self.slot(), tile, 1]) };
        if found != 0 {
            let n = s16(self.rd16(0xD4) as i32) + (extra != 0) as i32;
            let mut t = s16(self.rd16(0x114) as i32);
            let paths = unsafe { (*w).paths };
            let pd = |k: i32| buf8(paths, SLOTS * 80, 80 * self.s + k as usize) as i32;
            // SPEC GAP: a negative pathLen gives no steps; a pathLen above 80 (possible when the host claims op 41)
            // reads on into the next slots' paths, and past the 48 x 80 bytes as 0.
            for k in 0..n.max(0) {
                // 3.1
                let d = (pd(k) + 1) & 0xFF;
                t = step(t, self.dir_step(d));
                c = c.wrapping_add(crate::step_ap(self.u_move_code(t), sneak, b177, alt));
                // 3.2
                if k == 0 {
                    set_walk(c);
                    if m && self.tb(t, 0) < 0x9A {
                        let st = self.rd16(0x114);
                        put_mark(0, st);
                        let s0 = s16(st as i32);
                        let b7 = self.tb(s0, 7);
                        self.u_tile_put(s0, 7, (b7 & 0xF0) | (d as u8 & 0xF));
                        unsafe { (*w).marks_n = 1 };
                    }
                }
                // 3.3
                if m && k < n - 1 {
                    let mn = unsafe { (*w).marks_n }.wrapping_add(1);
                    unsafe { (*w).marks_n = mn };
                    put_mark(mn.wrapping_sub(1), t as u16);
                    let mut b = self.tb(t, 7);
                    if self.tb(t, 0) >= 0x9A && self.tb(t, 1) == 0 {
                        b = (b & 0xF) | 0xA0;
                    } else {
                        b = ((d as u8 & 0xF) << 4) | (b & 0xF);
                        let o = if extra != 0 && k >= n - 2 { d } else { pd(k + 1) + 1 };
                        b = (b & 0xF0) | (o as u8 & 0xF);
                    }
                    self.u_tile_put(t, 7, b);
                }
            }
            up = match self.u_cur() {
                0 | 3 => 7,
                1 | 2 => 5,
                0xE | 0xF => 3,
                _ => 0,
            };
        }
        // 4.
        unsafe { (*w).walk_ap = (*w).walk_ap.wrapping_add(up) };
        // 5.
        if mark != 0 {
            unsafe { (*w).marks_shown = 1 };
            if extra != 0 {
                unsafe { (*w).path_preview = self.slot() + 1 };
            }
        }
        // 6.
        c.wrapping_add(up)
    }
    /// Walk to (op 29).
    pub(super) fn walk_to(&self, tile: i32) {
        let slot = self.slot();
        // 1.
        self.wr16(0x12A, tile as u16);
        self.wr16(0x120, tile as u16);
        self.wr16(0xD0, NONE);
        // 2. SPEC GAP: turns(cur) of a word 256 and up reads 0 (anim_table has 256 rows), as everywhere else.
        if (self.u_turns(self.u_cur()) != 0 || self.s8(0x1D5) > 0) && self.rd8(0x1FE) != 0 {
            self.call(op::IDLE, &[slot]);
        } else {
            self.call(op::ANIM_STEP, &[slot]);
        }
    }
    /// Idle (op 1).
    pub(super) fn idle(&self) {
        if self.rd8(0x1FE) != 0 {
            self.u_forget_attack(true);
        }
    }
    // ---- Move order (ja1_move_order) ----
    /// Move order: 1 = walking, 0 = not. Flags are low bytes (≠ 0 = set).
    pub(super) fn move_order(&self, t: i32, marker: u8, sneak: u8, b177: u8, alt: u8, squad: u8) -> i32 {
        if !self.move_gates(t, true) {
            return 0;
        }
        self.move_go(t, marker, sneak, b177, alt, squad, None)
    }
    /// Step 1 of the move order (the gates): true = the order goes on. `ops` = false: no host op (the messages of a
    /// refused order do not show; ja1_move_gate).
    pub(super) fn move_gates(&self, t: i32, ops: bool) -> bool {
        let slot = self.slot();
        // 1. Steps 1, 2 and 3 of "Move to the patrol tile".
        if self.s >= 0x28
            || self.rd8(0x182) == 0
            || self.s8(0x183) < 15
            || self.rd8(0x17E) != 0
            || self.u_cur() > 0x3C
        {
            return false;
        }
        let d = self.s16_at(0xD0);
        if self.rd8(0x19C) == 3 && self.s8(0x1D5) > 0 && d != NONE_I {
            let o = (self.tb(d, 5) & 0x7F) as usize;
            let ty = self.rd8(0x00);
            // SPEC GAP: the hand gun state, +0x00 and +0x01 are read on the soldier's own record (as in Move to the
            // patrol tile).
            if o < 0x30 && self.u_s8_of(o, 0x183) > 0 && (1..=0x10).contains(&ty) && self.rd8(0x01) != 0 {
                let me = self.s16_at(0x114);
                if ty < 0xF
                    || ((self.call(op::TILE_DIST, &[me, d]) as i32) == 1
                        && column(self.s16_at(0x114)) == column(d)
                        && row(self.s16_at(0x114)) == row(d))
                {
                    if ops {
                        self.call(op::MERC_MESSAGE, &[slot, 8, 0x40, 0, 1]);
                    }
                    return false;
                }
            }
        }
        if self.tile_standable(t) <= 0 && self.s8(0xBA) != 2 {
            if ops && self.s16_at(0x114) != t && t != NONE_I {
                self.call(op::MESSAGE, &[1]);
            }
            return false;
        }
        true
    }
    /// The move order after its gates; `walk` = Some(walk AP) after a confirm click (the AP the marked path priced:
    /// the path is not priced again).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn move_go(&self, t: i32, marker: u8, sneak: u8, b177: u8, alt: u8, squad: u8, walk: Option<i32>) -> i32 {
        let slot = self.slot();
        // 2. Squad.
        if squad != 0 {
            let (ks, ka) = unsafe { ((*self.w).key_shift, (*self.w).key_alt) };
            for i in 0..8usize {
                if i == self.s {
                    continue;
                }
                if let Some(o) = Ai::new(self.w, self.h, i as i32) {
                    o.move_order(t, 1, ks, 0, ka, 0);
                }
            }
        }
        // 3. The library's Path AP (never a host call here).
        if let Some(r) = walk {
            unsafe { (*self.w).walk_ap = r };
        } else {
            self.wr16(0xD4, 0);
            self.path_ap_op(t, sneak, b177, alt);
        }
        // 4. Not enough AP.
        // SPEC GAP: the low byte of walk_ap is compared unsigned (as in Move to the patrol tile).
        let walk = unsafe { (*self.w).walk_ap } as u8 as i32;
        if self.u_sel() as usize == self.s && self.w_combat() && self.s8(0x1E2) + self.s8(0x1A5) < walk {
            if self.s < 8 {
                self.call(op::MERC_TEXT, &[slot, 1, 0x4B0]);
            } else {
                self.wr8(0x1B0, 1);
            }
            self.wr16(0x120, t as u16);
            self.wr8(0x177, b177);
            self.wr8(0x196, alt);
            self.wr8(0x1A8, sneak);
            self.call(op::MOVE_SOUND, &[slot, 1]);
            return 0;
        }
        // 5.
        self.wr8(0x197, 0);
        self.call(op::STOP_WALK, &[slot, 1]);
        // 6.
        if marker != 0 {
            if unsafe { (*self.w).opt_move_fx } != 0 {
                self.call(op::EFFECT_SLOT, &[slot, 1]);
            }
            self.call(op::TILE_MARKER, &[0x8A, t]);
        }
        // 7.
        if self.u_can_act(self.u_cur()) != 0 {
            self.wr16(0x126, self.rd16(0x116));
            self.wr16(0x128, self.rd16(0x118));
        }
        // 8.
        self.wr8(0x177, b177);
        self.wr8(0x1A8, sneak);
        self.wr8(0x196, alt);
        self.wr8(0x1FA, 1);
        self.call(op::WALK_TO, &[slot, t]);
        self.call(op::MOVE_SOUND, &[slot, 1]);
        self.wr32(0xB0, 0);
        1
    }

    // ---- Teleport (cursor.md "Debug keys (level 4)", Ctrl+T) ----
    pub(super) fn teleport(&self, t: i32) {
        self.vacate(self.s16_at(0x114));
        self.wr16(0x114, t as u16);
        self.stop_anim(1);
        self.wr16(0x120, 0xC41);
        let (x, y) = ((t % 56) * 16 + 5, (t / 56) * 14 + 5);
        self.wr16(0x116, x as u16);
        self.wr16(0x118, y as u16);
        self.wr16(0x126, x as u16);
        self.wr16(0x128, y as u16);
        self.occupy_for(t, self.rd8(0x17A));
        let (b0, b1, b2) = (self.tb(t, 0), self.tb(t, 1), self.tb(t, 2));
        self.wr8(0x1B4, b1);
        self.wr8(0x1B5, self.u_class(unsafe { (*self.w).t1_class }, b1));
        self.wr8(0x1B2, b0);
        self.wr8(0x1B3, self.u_class(unsafe { (*self.w).t0_class }, b0));
        self.wr8(0x1B6, b2);
        self.wr8(0x1B7, self.u_class(unsafe { (*self.w).t2_class }, b2));
    }

    // ---- Stop animation (op 53 stop_anim) ----
    pub(super) fn stop_anim(&self, flag: u8) {
        // 1.
        self.wr16(0x12A, NONE);
        self.wr8(0x1F6, 0);
        self.wr16(0x124, self.rd16(0x114));
        // 2.
        if flag == 0 {
            let a = self.rd8(0x1A6);
            if a != 0 {
                self.wr8(0x95, a);
                self.wr16(0x96, self.rd16(0x134));
            }
            return;
        }
        // 3.
        self.wr16(0x120, NONE);
        self.wr8(0x1A8, 0);
        self.wr8(0x196, 0);
        self.call(op::STEP_SOUND_STOP, &[self.slot(), 1]);
        let a = self.rd8(0x1D1);
        if a != 0 && self.rd8(0x1D6) != 0 && !(a == 0xE && self.rd8(0x1A6) != 0) && matches!(a, 2 | 3 | 4 | 5 | 8 | 0xD) {
            self.wr8(0x1D1, 0);
            self.wr16(0x132, NONE);
            self.wr8(0x1D6, 0);
        }
    }

    // ---- One frame: the part of one record (step 2 a) ----
    fn frame_record(&self) {
        let i = self.s;
        let w = self.w;
        // 1.
        if i >= 0x28 && self.s8(0x9C) > 0 {
            unsafe { (*w).seen_enemies = (*w).seen_enemies.wrapping_add(1) };
        }
        // 2.
        let add = self.rd8(0x195);
        unsafe { (*w).in_flight = (*w).in_flight.wrapping_add(add) };
        // 3.
        if self.rd32(0xB0) as i32 <= 0 {
            let sel = self.u_sel();
            if (self.rd8(0x1B0) == 0 && sel == self.rd8(0x17A)) || (i < 0x10 && !self.w_combat()) {
                self.call(op::ACTIVE_FRAME, &[i as i32]);
            } else if self.rd32(0xB0) as i32 <= 0 {
                self.call(op::SOLDIER_FRAME, &[i as i32]);
            }
        }
        // 4.
        if self.rd32(0xC4) == 0 {
            let c = self.rd8(0x1D2).wrapping_sub(1);
            self.wr8(0x1D2, c);
            if (c as i8 as i32) % 4 == 0 {
                self.wr16(0xD8, self.rd16(0xD8).wrapping_sub(1));
                self.wr16(0xDA, self.rd16(0xDA).wrapping_sub(1));
            }
            // SPEC GAP: "then u32 +0xC4 = ..." is read as following the whole step 4 test (not only the multiple-of-4
            // case).
            let v: i32 = if self.s8(0x1D2) > 0 { 10 } else { -1 };
            self.wr32(0xC4, v as u32);
        }
        // 5.
        if i < 8 {
            if self.rd32(0xBC) as i32 <= 0 && self.s8(0x189) > 0 {
                self.wr8(0x189, self.rd8(0x189).wrapping_sub(1));
                self.call(op::PORTRAIT, &[self.rd8(0x17A) as i32, 1]);
                self.wr32(0xBC, 0x96);
            }
            if self.rd32(0xA8) as i32 <= 0 {
                self.wr8(0x1AC, self.rd8(0x1AC).wrapping_add(1));
                let mut r = 0;
                if self.s8(0x1AC) > 0x14 {
                    self.wr8(0x1AC, 0);
                }
                let shown = unsafe { (*w).merc_shown };
                if self.s8(0x1AC) < 3 && shown != self.rd8(0x17A) && self.rd8(0x194) == 0 && self.rd8(0x1A5) != 0 {
                    self.call(op::PORTRAIT, &[i as i32, 1]);
                } else {
                    r = self.frame_random(0x28);
                }
                self.wr32(0xA8, (r.wrapping_add(100)) as u32);
            }
        }
        // 6.
        if self.rd8(0x1BF) == 3 {
            self.call(op::PROJECTILE_STEP, &[i as i32]);
        }
    }

    /// One frame step 2 b: the hand-over. Returns the new_turn result to add (low byte), 0 when none.
    fn frame_hand_over(&self) -> u8 {
        let w = self.w;
        let out = self.rd8(0x1B0) != 0 || self.rd8(0x182) == 0 || self.rd8(0x138) == 0 || self.rd8(0x188) != 0;
        if !(out && self.rd8(0x17A) == self.u_sel()) {
            return 0;
        }
        let r = self.call(op::ITQ_POP, &[]) as u8;
        if r < 8 {
            self.call(op::TURN_HAND_OVER, &[]);
        } else if r < 0x40 {
            unsafe { (*w).selected = r };
            self.call(op::TURN_BANNER, &[]);
        } else if unsafe { (*w).kept_merc } < 8 {
            self.call(op::TURN_HAND_OVER, &[]);
        } else {
            let sel = self.u_sel();
            if sel < 0x2F {
                if sel >= 8 {
                    unsafe { (*w).selected = sel.wrapping_add(1) };
                    self.call(op::TURN_BANNER, &[]);
                }
            } else if unsafe { (*w).interrupt_turn } == 0 {
                self.call(op::ROUND_END, &[]);
            } else if self.w_combat() {
                return self.call(op::NEW_TURN, &[1]) as u8;
            }
        }
        0
    }

    // ---- Timer tick: the part of one record ----
    fn tick_record(&self) {
        let w = self.w;
        let step = unsafe { (*w).tick_step } as u32;
        self.wr32(0xB0, self.rd32(0xB0).wrapping_sub(step));
        self.wr32(0xCC, self.rd32(0xCC).wrapping_sub(step));
        if self.s8(0x183) >= 15 && unsafe { (*w).screen_drawn } != 0 {
            self.wr32(0xA8, self.rd32(0xA8).wrapping_sub(10));
        }
        if self.rd32(0xC0) as i32 > 0 {
            self.wr32(0xC0, self.rd32(0xC0).wrapping_sub(10));
        }
        if self.rd32(0xC4) as i32 > 0 {
            self.wr32(0xC4, self.rd32(0xC4).wrapping_sub(10));
            if (self.rd32(0xC4) as i32) < 0 {
                self.wr32(0xC4, -1i32 as u32);
            }
        }
        if self.rd32(0xB4) as i32 > 0 {
            self.wr32(0xB4, self.rd32(0xB4).wrapping_sub(10));
            if self.rd32(0xB4) as i32 <= 0 {
                self.wr32(0xB4, 0);
            }
        }
        if self.s < 8 {
            // SPEC GAP: the +0x1DC test is read as a second, independent test (not inside the +0x189 one).
            if self.s8(0x189) > 0 {
                self.wr32(0xBC, self.rd32(0xBC).wrapping_sub(10));
            }
            if self.rd32(0x1DC) as i32 > 0 {
                self.wr32(0x1DC, self.rd32(0x1DC).wrapping_sub(10));
            }
        }
    }
    fn u_move_to(&self, t: i32) -> i32 {
        let slot = self.slot();
        // 1.
        if self.s >= 0x28
            || self.rd8(0x182) == 0
            || self.s8(0x183) < 15
            || self.rd8(0x17E) != 0
            || self.u_cur() > 0x3C
        {
            return 0;
        }
        // 2. Not done with the target: a relentless merc (+0x19C 3) refuses while its target +0xD0 lives
        //    (DAY quote 8 "I'm not done yet!", docs/spec/personality.md).
        let d = self.s16_at(0xD0);
        if self.rd8(0x19C) == 3 && self.s8(0x1D5) > 0 && d != NONE_I {
            let o = (self.tb(d, 5) & 0x7F) as usize;
            let ty = self.rd8(0x00);
            // SPEC GAP: the hand gun state, +0x00 and +0x01 are read on the soldier's own record.
            if o < 0x30 && self.u_s8_of(o, 0x183) > 0 && (1..=0x10).contains(&ty) && self.rd8(0x01) != 0 {
                let me = self.s16_at(0x114);
                if ty < 0xF
                    || ((self.call(op::TILE_DIST, &[me, d]) as i32) == 1
                        && column(self.s16_at(0x114)) == column(d)
                        && row(self.s16_at(0x114)) == row(d))
                {
                    self.call(op::MERC_MESSAGE, &[slot, 8, 0x40, 0, 1]);
                    return 0;
                }
            }
        }
        // 3.
        if self.tile_standable(t) <= 0 && self.s8(0xBA) != 2 {
            if self.s16_at(0x114) != t && t != NONE_I {
                self.call(op::MESSAGE, &[1]);
            }
            return 0;
        }
        // 4. (ClearPath 0x1A460)
        self.wr16(0xD4, 0);
        self.call(op::PATH_AP, &[slot, t]);
        // 5.
        // SPEC GAP: the low byte of walk_ap is compared unsigned.
        let walk = unsafe { (*self.w).walk_ap } as u8 as i32;
        if self.u_sel() as usize == self.s && self.w_combat() && self.s8(0x1E2) + self.s8(0x1A5) < walk {
            if self.s < 8 {
                self.call(op::MERC_TEXT, &[slot, 1, 0x4B0]);
            } else {
                self.wr8(0x1B0, 1);
            }
            self.wr16(0x120, t as u16);
            self.wr8(0x177, 0);
            self.wr8(0x196, 0);
            self.wr8(0x1A8, 0);
            self.call(op::MOVE_SOUND, &[slot, 1]);
            return 0;
        }
        // 6.
        self.wr8(0x197, 0);
        self.call(op::STOP_WALK, &[slot, 1]);
        // 7.
        if self.u_can_act(self.u_cur()) != 0 {
            self.wr16(0x126, self.rd16(0x116));
            self.wr16(0x128, self.rd16(0x118));
        }
        // 8.
        self.wr8(0x177, 0);
        self.wr8(0x1A8, 0);
        self.wr8(0x196, 0);
        self.wr8(0x1FA, 1);
        self.call(op::WALK_TO, &[slot, t]);
        self.call(op::MOVE_SOUND, &[slot, 1]);
        self.wr32(0xB0, 0);
        1
    }

    // ---- Soldier frame ----
    pub(super) fn soldier_frame(&self) {
        let slot = self.slot();
        // 1.
        let a = self.u_code();
        // 2.
        if self.rd8(0x1B8) == 0 && self.s8(0x183) >= 15 {
            if self.u_at_point() {
                self.call(op::STOP_ANIM, &[slot, self.u_stored_flag()]);
                self.wr8(0x1B8, 1);
                self.call(op::ANIM_STEP, &[slot]);
            } else {
                self.walk_step();
            }
        }
        // 3.
        if self.rd8(0x9D) != 0 {
            self.call(op::REVEAL_VIEW, &[slot, self.rd8(0x1A1) as i32, self.rd8(0x19E) as i32, 0]);
        }
        // 4.
        if self.u_still(a as u16) == 0 && self.rd8(0xBA) != 2 {
            return;
        }
        // 5.
        self.call(op::ANIM_NEXT, &[slot]);
        self.wr32(0xB0, self.s16_at(0xD2) as u32);
        self.frame_checks();
        // 6.
        if self.s8(0x183) >= 15 {
            self.stand_up();
            if self.rd8(0xBA) == 2 && a >= 4 && self.u_can_act(a as u16) != 0 {
                let p = self.rd16(0x12E);
                // SPEC GAP: "Else if At the waypoint" is read as the else of the p test.
                if self.rd16(0x114) != p && p != self.rd16(0x124) {
                    self.wr8(0x1B8, 0);
                    self.u_move_to(p as i16 as i32);
                } else if self.u_at_waypoint(self.s) && a != 0x11 {
                    self.call(op::SET_ANIM, &[slot, 0x11, 1, 0]);
                    self.wr8(0x1A2, self.rd8(0x1A4));
                }
            }
        }
        if self.s8(0x183) < 15 {
            self.wr16(0x120, NONE);
        }
        // 7.
        if self.rd8(0x182) != 0 && self.rd8(0x138) != 0 && self.rd8(0x188) == 0 {
            self.occupy(self.s16_at(0x114));
        }
    }

    // ---- Step start ----
    fn u_step_start(&self) -> bool {
        let slot = self.slot();
        // 1.
        let c0 = self.u_cur();
        let mut d = self.rd8(0x1A2);
        if self.rd8(0x196) != 0 {
            d = d.wrapping_add(4);
            if d > 8 {
                d -= 8;
            }
        }
        let me = self.s16_at(0x114);
        let nt = step(me, self.dir_step(d as i32));
        let (t0, t1) = (self.tb(nt, 0), self.tb(nt, 1));
        let cls = self.t0_class(t0);
        // 2.
        if self.rd8(0x196) != 0 && water_tile(t0, t1) {
            self.wr8(0x196, 0);
        }
        // 3.
        let ap = if nt != me { self.u_step_ap(nt) } else { 0 };
        let mut have = self.rd8(0x1A5);
        if self.s < 8 && self.u_ap_warned() == 0 && self.rd8(0x16B) == 1 {
            have = have.wrapping_sub(self.rd8(0xEF));
        }
        // 4.
        if self.w_combat() && ap > have {
            self.call(op::STOP_WALK, &[slot, 0]);
            self.wr8(0x197, 1);
            if ap as i32 > self.s8(0x1A5) {
                let f = (ap >= 100 && nt == self.s16_at(0x124)) as i32;
                self.call(op::STOP_ANIM, &[slot, f]);
            } else {
                self.u_low_ap_stop(true);
            }
            return false;
        }
        // 5.
        let mut go = false;
        let st = self.tile_standable(nt);
        if st >= 1 || self.rd8(0xBA) == 2 {
            go = true;
        } else if st < 0 && self.rd8(0xB9) != 0 {
            self.call(op::BREAK_OBJECT, &[nt, t1 as i32, self.u_seen_i()]);
            for k in 0..8usize {
                if self.rd8_of(k, 0x138) != 0 {
                    self.u_wr8_of(k, 0x9D, 1);
                }
            }
            self.wr16(0x106, nt as u16);
            go = true;
        } else {
            self.call(op::WALK_BLOCKED, &[slot, nt, (self.tb(nt, 5) & 0x7F) as i32]);
        }
        // 6. Mine.
        let g = self.tb(nt, 6);
        if self.s < 8
            && g < 0xFF
            && matches!(self.u_ground(g, 0), 0x13 | 0x2C)
            && self.s8(0x192) >= (self.u_ground(g, 2) as i8 as i32).abs()
        {
            go = false;
            self.u_fire_trigger(0xFF, true);
            self.call(op::MINE_FLAG, &[slot, nt]);
        }
        // 7. Forgetful merc (+0x19C 2): forgets where it was going.
        let lim = if self.w_combat() { 5 } else { 15 };
        if self.rd8(0x19C) == 2 && self.s < 8 && self.rd8(0x1AF) > lim && self.random(15) == 0 {
            unsafe {
                (*self.w).quote_slot = self.s as u8;
                (*self.w).quote_event = 0xD;
                (*self.w).quote_delay = 2;
            }
            go = false;
            self.call(op::STOP_ANIM, &[slot, 1]);
            if self.w_combat() && self.rd8(0x1A5) != 0 {
                let r = self.random(self.s8(0x1A5)) + 1;
                self.wr8(0x1A5, self.rd8(0x1A5).wrapping_sub(r as u8));
            }
        }
        // 8.
        if !go {
            return false;
        }
        // 9.
        self.wr16(0x126, px(nt) as u16);
        self.wr16(0x128, py(nt) as u16);
        let wading = t1 > 0 && t1 < 0x21;
        // 10.
        if !self.in_water() && self.u_cur() < 0x32 {
            if cls >= 8 && !wading {
                self.call(op::SET_ANIM, &[slot, 0x32, 1, 1]);
            }
            return true;
        }
        // 11.
        let c = c0 as i16 as i32;
        let n = if cls < 8 || wading {
            Some(0x31)
        } else if t0 >= 0x9A {
            (c < 0x39).then_some(0x3A)
        } else if t0 >= 0x97 {
            (c < 0x36 || c >= 0x39).then_some(0x37)
        } else if c > 0x34 {
            Some(0x34)
        } else if c < 0x32 {
            Some(0x32)
        } else {
            None
        };
        if let Some(n) = n {
            self.call(op::SET_ANIM, &[slot, n, 1, 1]);
        }
        true
    }

    // ---- Drown ----
    fn u_drown(&self) {
        let slot = self.slot();
        // 1.
        if self.u_cur() < 0x39 {
            return;
        }
        // 2.
        if self.seen() || self.u_follow_all() {
            self.call(op::FOCUS, &[slot, 0]);
        }
        // 3.
        if self.rd8(0x1BA) == 0 {
            self.call(op::MERC_MESSAGE, &[slot, 0xA, 0x40, 0, 1]);
            self.wr8(0x1BA, 1);
            self.call(op::STAT_PRACTICE, &[slot, 4, 5, 0]);
            return;
        }
        // 4.
        let mut d = (self.random(10) + 2) as u8;
        d = d.wrapping_add(self.random(10) as u8);
        if self.rd8(0x19C) == 5 {
            d = d.wrapping_mul(2);
        }
        self.call(op::TAKE_DAMAGE, &[slot, 0x1356, d as i32, 2, 0x40, NONE_I]);
        if self.rd8(0x183) == 0 {
            self.call(op::KILL, &[slot, 0]);
        }
    }

    /// Turn ends (with or without the target test).
    fn u_turn_ends(&self, target_test: bool) {
        let slot = self.slot();
        let sel = self.u_sel();
        if sel < 8 {
            unsafe { (*self.w).kept_merc = sel };
        }
        if !target_test || self.rd16(0x114) != self.rd16(0x124) {
            self.call(op::STOP_ANIM, &[slot, 0]);
        }
        self.wr8(0x197, 1);
        self.call(op::NEXT_TURN, &[0]);
    }

    // ---- Active frame ----
    pub(super) fn active_frame(&self) {
        let slot = self.slot();
        let c0 = self.u_cur();
        let mut ended = false;
        let mut stepped = false;
        if self.rd8(0x1B8) == 0 && self.s8(0x183) >= 15 {
            // A. Walking.
            if !self.u_at_point() {
                self.walk_step();
            } else {
                self.u_active_at_point(c0, &mut ended, &mut stepped);
            }
        } else {
            // B. Still.
            // 1.
            if self.u_still(self.u_cur()) != 0 {
                self.call(op::ANIM_NEXT, &[slot]);
            }
            // 2.
            if self.u_sel() as usize == self.s {
                self.call(op::AI_THINK, &[slot]);
            }
            // 3.
            if self.s8(0x183) >= 15 || self.in_water() {
                if !self.in_water() && self.rd8(0x00) > 0x10 && self.rd8(0x1BE) == 1 {
                    self.call(op::START_ATTACK, &[slot, self.s16_at(0xDC)]);
                    self.call(op::THROW_FOLLOW, &[slot]);
                }
                if self.s16_at(0x12A) != NONE_I {
                    self.call(op::ANIM_STEP, &[slot]);
                }
            } else {
                self.u_exhausted_fall();
            }
        }
        // C. Every case.
        // 1.
        if self.s < 8 && self.rd8(0x9D) != 0 {
            self.call(op::GUARD_TURNED, &[slot]);
            self.enemies_see();
            self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
            if self.u_turn_moves() {
                self.u_turn_ends(false);
            }
            self.call(op::REVEAL_VIEW, &[slot, self.rd8(0x1A1) as i32, self.rd8(0x19E) as i32, 0]);
        }
        // 2.
        self.frame_checks();
        // 3.
        if self.s8(0x183) >= 15 {
            self.stand_up();
            if self.rd8(0x19C) == 4
                && unsafe { (*self.w).alone_warned } == 0
                && (self.call(op::NEAREST_MERC, &[slot]) as i32) > 5
            {
                self.call(op::MERC_MESSAGE, &[slot, 4, 0x40, 0, 1]);
                unsafe { (*self.w).alone_warned = 1 };
            }
        }
        if self.s8(0x183) < 15 {
            self.wr16(0x120, NONE);
        }
        // 4. Timer.
        if self.s >= 0x28 && self.s8(0x9C) == -1 {
            self.wr32(0xB0, 0);
        } else {
            let v = self.s16_at(0xD2);
            let half = (self.rd8(0xBA) == 1 && self.w_combat())
                || (unsafe { (*self.w).opt_half_delay } != 0 && self.u_still(self.u_cur()) == 0);
            self.wr32(0xB0, if half { v / 2 } else { v } as u32);
        }
        // 5.
        if self.rd8(0x182) != 0 && self.rd8(0x138) != 0 && self.rd8(0x188) == 0 {
            self.occupy(self.s16_at(0x114));
        }
        // 6.
        let ns = unsafe { (*self.w).next_selected };
        if ns != NO_SLOT {
            unsafe { (*self.w).selected = ns };
            self.call(op::TURN_BANNER, &[]);
            unsafe { (*self.w).next_selected = NO_SLOT };
        }
    }

    /// Active frame A, at the step point (steps 1..11).
    fn u_active_at_point(&self, c0: u16, ended: &mut bool, stepped: &mut bool) {
        let slot = self.slot();
        // 1. Door.
        let door = self.s16_at(0x106);
        if door != NONE_I && self.rd16(0x114) != self.rd16(0x106) {
            self.call(op::CLOSE_DOOR, &[door, self.tb(door, 1) as i32, self.u_seen_i()]);
            self.wr16(0x106, NONE);
        }
        // 2. Sneak practice.
        if self.s < 8 && self.rd8(0x1A8) != 0 && self.s8(0x1D5) > 0 {
            for j in 0..8usize {
                if self.rd8(0x13A + j) == 1 && self.rd8_of(0x28 + j, 0x13A + self.s) != 1 {
                    self.call(op::STAT_PRACTICE, &[slot, 2, 1, 0]);
                }
            }
        }
        // 3.
        let me = self.s16_at(0x114);
        if self.rd8(0x193) != 0 {
            self.wr8(0x193, 0);
            // a.
            if self.rd16(0xF4) != self.rd16(0x114) {
                let m = self.u_move_code(me);
                let ap = self.u_step_ap(me) as i32;
                self.call(op::SPEND_AP, &[slot, ap, step_energy(m)]);
            }
            // b.
            self.wr16(0xF4, self.rd16(0x114));
            self.wr8(0x17D, 0);
            self.u_tile_triggers();
            *stepped = true;
            // c.
            let g = self.u_ground_index(me);
            if g < 0xFF {
                let k = self.u_ground(g, 0);
                if self.s < 0x28 || k == 0x70 {
                    self.call(op::TRIGGER_ITEM, &[slot, me, g as i32, 0]);
                } else if k == 0x11 && self.u_ground(g, 2) as i8 >= 10 {
                    self.call(op::EXPLODE, &[slot, me, 2, 1]);
                    self.call(op::TAKE_GROUND_ITEM, &[me, 0]);
                }
            }
            // d.
            if self.rd8(0x18A) == 0 && self.in_water() && self.rd8(0x183) != 0 {
                self.u_drown();
                self.wr16(0x124, self.rd16(0x114));
                self.walk_end(self.u_code());
            }
        }
        // 4.
        if self.s >= 0x28 {
            self.call(op::ENEMY_TURNED, &[slot]);
            self.call(op::ENEMY_STEP, &[slot]);
            if self.seen() {
                self.call(op::FOCUS, &[slot, 1]);
                if unsafe { (*self.w).opt_onscreen } != 0 {
                    self.call(op::TAC_REDRAW, &[1]);
                }
            }
            if unsafe { (*self.w).interrupt_go } != 0 {
                self.call(op::MERC_INTERRUPT, &[]);
                *ended = true;
            } else if self.u_turn_moves() {
                self.call(op::NEXT_TURN, &[0]);
                *ended = true;
            }
        } else {
            if self.s < 0x10 {
                self.call(op::GUARD_TURNED, &[slot]);
                self.enemies_see();
                self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
                self.call(op::REVEAL_VIEW, &[slot, self.rd8(0x1A1) as i32, self.rd8(0x19E) as i32, 0]);
            }
            // SPEC GAP: "Then if turn_moves" is read as part of every slot below 0x28, not only below 0x10.
            if self.u_turn_moves() {
                *ended = true;
                self.u_turn_ends(true);
            }
        }
        // 5.
        if *stepped {
            self.call(op::STEP_NOISE, &[slot]);
        }
        // 6.
        if self.w_combat() && self.u_sel() as usize != self.s && self.rd16(0x114) != self.rd16(0x124) {
            self.call(op::STOP_ANIM, &[slot, 0]);
        }
        // 7.
        let mf = self.s16_at(0xEC);
        if mf != NONE_I {
            self.call(op::MINE_FLAG, &[slot, mf]);
            self.wr16(0xEC, NONE);
        }
        // 8.
        if self.u_sel() as usize == self.s {
            self.call(op::AI_THINK, &[slot]);
        }
        // 9.
        if self.s < 8 && self.in_water() && self.rd8(0x1B2) >= 0x9A {
            self.u_soak_items();
        }
        // 10.
        if self.rd16(0x114) == self.rd16(0x124) {
            self.walk_end(c0 as u8);
            if (!self.w_combat() || self.u_sel() as usize == self.s) && self.rd8(0x1A6) != 0 && self.rd8(0x1B0) == 0 {
                let v = self.rd8(0x1A6);
                let act = if v >= 0xFE {
                    Some(0u8)
                } else if self.rd8(0x00) == v {
                    Some(v)
                } else {
                    None
                };
                match act {
                    Some(act) if !self.in_water() => {
                        let target = self.s16_at(0x134);
                        let mut warn = false;
                        if self.s < 8 {
                            // SPEC GAP: "me" is the own tile read at step 3.
                            let c = (self.call(op::USE_COST, &[slot, me, target, self.tb(target, 6) as i32, 0]) & 0xFF)
                                as u8;
                            let mut have = self.rd8(0x1A5);
                            if self.u_ap_warned() == 0 && self.rd8(0x16B) != 0 {
                                have = have.wrapping_sub(self.rd8(0xEF));
                            }
                            warn = self.w_combat() && c > have && self.rd8(0x16B) != 0;
                        }
                        if warn {
                            self.u_low_ap_stop(false);
                        } else {
                            self.call(op::CLICK, &[slot, target, act as i32]);
                        }
                    }
                    _ => self.clear_task(),
                }
            }
        } else if self.s8(0x18A) > 0 || self.in_water() {
            // SPEC GAP: Step start (and what follows it) is read as part of "if ended = 0".
            if !*ended {
                let mut d = self.next_path_direction();
                self.wr8(0x1A2, d);
                if self.rd8(0x196) != 0 {
                    d = d.wrapping_add(4);
                    self.wr8(0x1A2, if d > 8 { d - 8 } else { d });
                }
                if self.u_step_start() {
                    self.wr16(0xD4, self.rd16(0xD4).wrapping_sub(1));
                    self.wr8(0x1AF, self.rd8(0x1AF).wrapping_add(1));
                    self.walk_step();
                } else if self.rd16(0x124) == self.rd16(0x114) {
                    self.walk_end(self.u_code());
                }
            }
        } else if c0 as i16 >= 4 {
            self.u_tile_point_stop(1);
            self.call(op::UI_REFRESH, &[self.u_pp()]);
            self.call(op::SET_ANIM, &[slot, 0x2B, 0, 0]);
        }
        // 11.
        self.u_exhausted_fall();
    }
}

// ---- Merc view (op 62 reveal_view) ----
const VIEW_RAYS_LEN: usize = 306;
const OBJ_BLOCK_LEN: usize = 685;

/// Five directions D(f) for the facing f (any other value: all 0).
fn five_dirs(f: u8) -> [u8; 5] {
    match f {
        1 => [8, 2, 1, 7, 3],
        2 => [1, 3, 2, 8, 4],
        3 => [2, 4, 3, 1, 5],
        4 => [3, 5, 4, 2, 6],
        5 => [4, 6, 5, 3, 7],
        6 => [5, 7, 6, 4, 8],
        7 => [6, 8, 7, 5, 1],
        8 => [7, 1, 8, 6, 2],
        _ => [0; 5],
    }
}

impl Ai {
    /// Byte k of tile t is written only on the map (soldier_update.md "Roof tiles outside the map").
    fn u_tile_put(&self, t: i32, k: usize, v: u8) {
        if !(0..TILES as i32).contains(&t) {
            return;
        }
        unsafe {
            let p = (*self.w).tiles;
            if !p.is_null() {
                p.add(t as usize * 8 + k).write(v);
            }
        }
    }
    fn u_move_code_put(&self, t: i32, v: u16) {
        if !(0..TILES as i32).contains(&t) {
            return;
        }
        unsafe {
            let p = (*self.w).move_codes;
            if !p.is_null() {
                let [lo, hi] = v.to_le_bytes();
                p.add(2 * t as usize).write(lo);
                p.add(2 * t as usize + 1).write(hi);
            }
        }
    }
    fn u_ground_put(&self, g: u8, b: usize, v: u8) {
        if g as usize >= GROUND {
            return;
        }
        unsafe {
            let p = (*self.w).ground_items;
            if !p.is_null() {
                p.add(12 * g as usize + b).write(v);
            }
        }
    }
    /// Object block (t, d, side); side None = NONE.
    pub(super) fn u_obj_block(&self, t: i32, d: u8, side: Option<i32>) -> u8 {
        let o = self.tb(t, 1);
        if o < 0x27 {
            return 0;
        }
        if o >= 0x83 {
            return if o == 0xE2 || o == 0xE3 { 2 } else { 0 };
        }
        if o <= 0x36 {
            return 2;
        }
        let ob = unsafe { (*self.w).obj_block };
        let row = (o as usize - 0x37) * 9;
        let v = buf8(ob, OBJ_BLOCK_LEN, row + d as usize);
        let side = match side {
            None => return v,
            Some(s) => s,
        };
        if v > 2 {
            return v;
        }
        let m = buf8(ob, OBJ_BLOCK_LEN, row + 9) as i8;
        if (m == 1 && (side == 3 || side == 7)) || (m == 2 && (side == 1 || side == 5)) { 0 } else { v }
    }
    /// Merc busy.
    pub(super) fn u_merc_busy(&self) -> bool {
        (0..8).any(|k| {
            self.rd8_of(k, 0x182) != 0
                && self.rd8_of(k, 0x138) != 0
                && (self.u_s8_of(k, 0x1D5) >= 1 || self.rd8_of(k, 0x1C4) != 0)
        })
    }
    fn u_item_type_byte(&self, i: usize) -> u8 {
        buf8(unsafe { (*self.w).item_types }, ITEM_TYPES * 12, i)
    }

    pub(super) fn merc_view(&self, facing: u8, range: u8, think: u8) {
        let slot = self.slot();
        // 1.
        if self.s < 8 {
            // 2.
            let f = facing;
            let base = if f % 2 == 0 { 0x99 } else { 0 };
            let dirs = five_dirs(f);
            let mut steps = [0i32; 6];
            for i in 0..5 {
                steps[i] = self.dir_step(dirs[i] as i32);
            }
            let mut found = false;
            // 3.
            for r in (0..17).map(|i| i * 9) {
                let mut budget: i32 = 99;
                let mut after1 = false;
                let mut t = self.s16_at(0x114);
                for k in 0..range as usize {
                    // a.
                    let rays = unsafe { (*self.w).view_rays };
                    let ia = base + r + k;
                    let a = if ia >= VIEW_RAYS_LEN { 5 } else { buf8(rays, VIEW_RAYS_LEN, ia) };
                    let b = if k < 8 { buf8(rays, VIEW_RAYS_LEN, ia + 1) } else { 99 };
                    // b. SPEC GAP: a ray step above 5 is treated as step 0 (the ray ends).
                    let st = steps.get(a as usize).copied().unwrap_or(0);
                    let n = step(t, st);
                    if n == t {
                        break;
                    }
                    t = n;
                    // c.
                    let g = self.tb(t, 6);
                    if self.rd8(0x177) != 0
                        && k <= 1
                        && g < 200
                        && self.u_ground(g, 6) == 0xFF
                        && self.u_item_type_byte(12 * self.u_ground(g, 0) as usize + 12) != 0
                    {
                        if self.rd8(0x000) == 0x4D && self.s8(0x002) >= 10 {
                            if self.random(100) < self.s8(0x002) && self.tb(t, 1) == 0 {
                                self.call(op::MINE_FLAG, &[slot, t]);
                                if self.u_ground(g, 0) == 0x11 && (self.u_ground(g, 2) as i8) < 0 {
                                    self.call(op::STAT_PRACTICE, &[slot, 6, 2, 0]);
                                }
                            }
                        } else {
                            self.wr8(0x177, 0);
                        }
                    }
                    // d. SPEC GAP: a side byte of 6..98 or above 99 (no D(f) entry) is taken as 0.
                    let side = match b {
                        99 => None,
                        5 => Some(t),
                        _ => Some(dirs.get(b as usize).copied().unwrap_or(0) as i32),
                    };
                    let da = dirs.get(a as usize).copied().unwrap_or(0);
                    let mut blk = self.u_obj_block(t, da, side) as i8 as i32;
                    // e.
                    if blk == 0xB && k < 2 {
                        if k == 0 {
                            blk = 0;
                            budget = 2;
                        } else {
                            blk = 1;
                        }
                    }
                    // f.
                    if blk < 3 {
                        self.u_tile_put(t, 4, self.tb(t, 4) | 0x80);
                        // Spotted item.
                        let (tm, sd) = unsafe { ((*self.w).turn_moves, (*self.w).screen_drawn) };
                        if self.rd8(0x1D5) == 0 && tm == 0 && sd != 0 && g < 200 && self.u_ground(g, 6) == 0 {
                            self.u_ground_put(g, 6, 1);
                            if !found {
                                self.call(op::QUOTE_EVENT, &[slot, 3, 0, t, 1]);
                                self.u_tile_point_stop(0);
                                self.wr8(0x1A6, 0);
                                self.wr16(0x134, NONE);
                                self.wr8(0x1A7, 0);
                                if self.rd8(0x16D) == 0
                                    && self.s16_at(0x120) != NONE_I
                                    && self.rd16(0x120) != self.rd16(0x114)
                                {
                                    unsafe { (*self.w).spot_stop = 1 };
                                    self.wr8(0x16D, 1);
                                    self.call(op::LOW_AP_STOP, &[slot, 1, 0]);
                                }
                                self.call(op::STAT_PRACTICE, &[slot, 4, 1, 0]);
                                found = true;
                            }
                        }
                        // Roofs.
                        let c = self.u_class(unsafe { (*self.w).t2_class }, self.tb(t, 3));
                        if c >= 0x12 {
                            if self.tb(t, 3) == 0x73 {
                                self.u_tile_put(t, 3, 0);
                                if self.u_move_code(t) == 0x4B1 {
                                    let (gc, oc) = unsafe {
                                        (self.u_class_table((*self.w).t0_class), self.u_class_table((*self.w).t1_class))
                                    };
                                    let code = crate::move_code(self.tb(t, 0), self.tb(t, 1), 0, 0x4B1, &gc, &oc);
                                    self.u_move_code_put(t, code);
                                }
                            }
                            let p = match c {
                                0x12 => Some((0x53u8, 0x52u8, 0x54u8, 0x53u8, 9u8)),
                                0x13 => Some((0x5E, 0x5D, 0x5F, 0x5E, 0x14)),
                                0x14 => Some((0x69, 0x68, 0x6A, 0x69, 0x1F)),
                                _ => None,
                            };
                            if let Some((keep, south, above, own, add)) = p {
                                if self.tb(t - 56, 3) <= keep {
                                    self.u_tile_put(t - 56, 3, 0);
                                }
                                let t2 = self.tb(t, 2);
                                if matches!(t2, 0x49 | 0x4A | 0x4C) && self.tb(t + 56, 3) >= south {
                                    if self.tb(t, 3) > above {
                                        let v = if t2 == 0x4C { own } else { t2.wrapping_add(add) };
                                        self.u_tile_put(t, 3, v);
                                    }
                                } else {
                                    self.u_tile_put(t, 3, 0);
                                }
                            }
                        }
                        if self.tb(t, 1) == 0x57 && matches!(self.tb(t - 56, 3), 0x53 | 0x5E | 0x69) {
                            self.u_tile_put(t - 56, 3, 0);
                        }
                        // Story tiles.
                        let (so, cs, ss) = (self.story(0x16), unsafe { (*self.w).cur_sector }, self.story(0x17));
                        if so == 0 && cs as i32 == ss as i8 as i32 {
                            if (0xAC..=0xAF).contains(&self.tb(t, 1)) && !self.u_merc_busy() && self.s8(0x183) >= 15 {
                                unsafe { (*self.w).story_finder = slot as u8 };
                            }
                        } else if cs == 0 && unsafe { (*self.w).santino_word } != 0 && t == 0x279 {
                            unsafe { (*self.w).santino_word = 0 };
                        }
                        budget -= 1;
                    }
                    // g.
                    if budget <= 0 || after1 {
                        break;
                    }
                    if blk > 1 {
                        break;
                    }
                    if blk == 1 {
                        after1 = true;
                    }
                }
            }
        }
        // 4.
        self.wr8(0x09D, 0);
        if think != 0 {
            self.call(op::AI_THINK, &[slot]);
        }
    }
    /// A 256-byte class table copied out of the world (null: all 0).
    fn u_class_table(&self, p: *const u8) -> [u8; 256] {
        let mut a = [0u8; 256];
        for (i, e) in a.iter_mut().enumerate() {
            *e = buf8(p, CLASS_LEN, i);
        }
        a
    }
}

/// cursor.md "Debug keys (level 4)" Ctrl+T, up to ReplaceSoldier (the caller runs ja1_replace_soldier).
#[no_mangle]
pub extern "C" fn ja1_teleport(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    if (0..3136).contains(&tile) {
        run(w, h, slot, (), |a| a.teleport(tile))
    }
}

#[no_mangle]
pub extern "C" fn ja1_merc_view(w: *mut Ja1World, h: *const Ja1Host, slot: i32, facing: i32, range: i32, think: i32) {
    run(w, h, slot, (), |a| a.merc_view(facing as u8, range as u8, think as u8))
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens.
#[no_mangle]
pub extern "C" fn ja1_soldier_frame(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.soldier_frame())
}
#[no_mangle]
pub extern "C" fn ja1_active_frame(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.active_frame())
}
#[no_mangle]
pub extern "C" fn ja1_walk_step(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.walk_step())
}
#[no_mangle]
pub extern "C" fn ja1_walk_blocked(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, occupant: i32) {
    run(w, h, slot, (), |a| a.walk_blocked(s16(tile), occupant as u8))
}
#[no_mangle]
pub extern "C" fn ja1_walk_end(w: *mut Ja1World, h: *const Ja1Host, slot: i32, code: i32) {
    run(w, h, slot, (), |a| a.walk_end(code as u8))
}
#[no_mangle]
pub extern "C" fn ja1_stand_up(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.stand_up())
}
#[no_mangle]
pub extern "C" fn ja1_frame_checks(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.frame_checks())
}
#[no_mangle]
pub extern "C" fn ja1_merc_quote(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.merc_quote())
}
#[no_mangle]
pub extern "C" fn ja1_enemies_see(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.enemies_see())
}
#[no_mangle]
pub extern "C" fn ja1_enemy_step_seen(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.enemy_step_seen())
}
#[no_mangle]
pub extern "C" fn ja1_step_noise(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.step_noise())
}
#[no_mangle]
pub extern "C" fn ja1_throw_follow(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.throw_follow())
}
#[no_mangle]
pub extern "C" fn ja1_throw_aim(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    run(w, h, slot, (), |a| a.throw_aim(s16(tile)))
}
#[no_mangle]
pub extern "C" fn ja1_throw_scatter(w: *mut Ja1World, h: *const Ja1Host, slot: i32, margin: i32, distance: i32) {
    run(w, h, slot, (), |a| a.throw_scatter(margin as u8, s16(distance)))
}
#[no_mangle]
pub extern "C" fn ja1_find_path_op(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) -> i32 {
    run(w, h, slot, 0, |a| a.find_path_op(s16(tile), flag))
}
#[no_mangle]
pub extern "C" fn ja1_path_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, sneak: i32, byte177: i32,
    alt: i32) -> i32 {
    run(w, h, slot, 0, |a| a.path_ap_op(s16(tile), sneak as u8, byte177 as u8, alt as u8))
}
#[no_mangle]
pub extern "C" fn ja1_path_ap_marks(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, mark: i32, extra: i32,
    sneak: i32, byte177: i32, alt: i32) -> i32 {
    run(w, h, slot, 0, |a| a.path_ap_marks(s16(tile), mark as u8, extra as u8, sneak as u8, byte177 as u8, alt as u8, false))
}
#[no_mangle]
pub extern "C" fn ja1_walk_to(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    run(w, h, slot, (), |a| a.walk_to(s16(tile)))
}
#[no_mangle]
pub extern "C" fn ja1_idle(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.idle())
}
// ---- Small ops (soldier_update.md): Direction (op 6), Tile distance (op 7) ----
/// Direction 1..8 from pixel point (x1, y1) toward (x2, y2).
pub(super) fn direction(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    const T: [i32; 24] = [3, 4, 5, 6, 7, 8, 1, 2, 3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8];
    let dx = x2.wrapping_sub(x1);
    let dy = y2.wrapping_sub(y1);
    let fx = if dx == 0 { 0.00390625f64 } else { dx as f64 };
    let a = (1.2f64 * dy as f64).atan2(fx);
    let v = (a + 0.39269908175) * 1.2732395445689122;
    let i = v.trunc() as i32;
    // SPEC GAP: an index outside the table (cannot happen for a = -pi..pi) reads T[0].
    let at = |k: i32| if (0..24).contains(&k) { T[k as usize] } else { T[0] };
    if v > 0.0 {
        at(i)
    } else if v < 0.0 {
        at(i + 7)
    } else {
        T[0]
    }
}

/// Aim direction: the direction 1..8 and the turn word for +0x10A (soldier_update.md "Aim direction").
pub(super) fn aim_dir(x1: i32, y1: i32, x2: i32, y2: i32) -> (u8, u16) {
    const D: [u8; 24] = [3, 3, 4, 4, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 1, 1, 1, 2, 2, 2, 3];
    let dx = x2.wrapping_sub(x1);
    let dy = y2.wrapping_sub(y1);
    let fx = if dx == 0 { 0.00390625f64 } else { dx as f64 };
    let a = (1.2f64 * dy as f64).atan2(fx);
    let v = (a + 0.1308996939166667) * 3.819718633706735;
    let i = v.trunc() as i32;
    let k = if v > 0.0 { i } else if v < 0.0 { i + 23 } else { 0 };
    // SPEC GAP: an index outside the table (cannot happen for a = -pi..pi) reads k = 0.
    let k = if (0..24).contains(&k) { k as usize } else { 0 };
    (D[k], [0u16, 1, 0xFFFF][k % 3])
}

/// Tile distance: max of the row and column differences (C division for the parts).
pub(super) fn tile_dist(a: i32, b: i32) -> i32 {
    let dr = row(a).wrapping_sub(row(b)).wrapping_abs();
    let dc = column(a).wrapping_sub(column(b)).wrapping_abs();
    dr.max(dc)
}

// ---- One frame (ja1_frame) and Timer tick (ja1_tick) ----
fn frame(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let Some(a0) = Ai::new(w, h, 0) else { return 0 };
    // 1.
    unsafe {
        (*w).in_flight = 0;
        (*w).seen_enemies = 0;
    }
    let mut sum: u8 = 0;
    // 2.
    for i in 0..SLOTS {
        let Some(a) = Ai::new(w, h, i as i32) else { continue };
        if a.rd8(0x182) != 0 && a.rd8(0x138) != 0 && a.rd8(0x188) == 0 {
            a.frame_record();
        }
        sum = sum.wrapping_add(a.frame_hand_over());
    }
    // 3.
    let sig = unsafe { (*w).anim_signal };
    if sig >= 0 {
        // SPEC GAP: an entry past the 16 effect animations (or a null fx_anims) reads its count as 0.
        let c = if sig < 16 { buf8(unsafe { (*w).fx_anims }, 16 * 0x12, 0x12 * sig as usize) as i8 } else { 0 };
        if c <= 0 {
            unsafe { (*w).anim_signal = -1 };
        }
    }
    // 4.
    let (act, fl, sig) = unsafe { ((*w).active_slot, (*w).in_flight, (*w).anim_signal) };
    if act < 0x40 && fl == 0 && sig == -1 {
        a0.call(op::FIRE, &[act as i32]);
    }
    // 5.
    sum as i32
}

fn tick(w: *mut Ja1World, h: *const Ja1Host) {
    if Ai::new(w, h, 0).is_none() {
        return;
    }
    // 0.
    unsafe {
        if (*w).round_lock2 != 0 {
            return;
        }
        (*w).ambient_t1 = (*w).ambient_t1.wrapping_sub(10);
        (*w).ambient_t2 = (*w).ambient_t2.wrapping_sub(10);
        if (*w).show_timer > 0 {
            (*w).show_timer = (*w).show_timer.wrapping_sub((*w).tick_step);
            if (*w).show_timer <= 0 {
                (*w).show_timer = 0;
                (*w).shown_slot = 0x40;
                (*w).spotted = 0x40;
                (*w).view_tile = 0xC41;
            }
        }
        if (*w).round_lock != 0 {
            return;
        }
        // Busy timer (shooting.md).
        if (*w).busy_timer > 0 {
            (*w).busy_timer = (*w).busy_timer.wrapping_sub(10);
            if (*w).busy_timer <= 0 {
                (*w).busy_timer = 0;
                (*w).busy = 0;
            }
        }
        // Fast timer (turns.md Turn start).
        if (*w).fast_timer > 0 {
            (*w).fast_timer = (*w).fast_timer.wrapping_sub(10);
            if (*w).fast_timer <= 0 {
                (*w).fast_timer = 0;
                (*w).turn_based = 1;
            }
        }
        if (*w).turn_clock > 0 {
            (*w).turn_clock = (*w).turn_clock.wrapping_sub((*w).tick_step);
            if (*w).turn_clock < 0 {
                (*w).turn_clock = 0;
            }
        }
        (*w).turn_tick = (*w).turn_tick.wrapping_sub(10);
        (*w).ripple_timer = (*w).ripple_timer.wrapping_sub(10);
    }
    // 1.
    unsafe {
        if (*w).stop_timer > 0 {
            (*w).stop_timer = (*w).stop_timer.wrapping_sub(10);
            if (*w).stop_timer < 0 {
                (*w).stop_timer = 0;
            }
        }
    }
    // 2.
    for i in 0..SLOTS {
        let Some(a) = Ai::new(w, h, i as i32) else { continue };
        if a.rd8(0x182) != 0 && a.rd8(0x138) != 0 {
            a.tick_record();
        }
    }
    // 3. Fuses.
    let fx = unsafe { (*w).timed_fx };
    for f in 0..5usize {
        let b = 13 * f;
        let e = fx_i32(fx, b);
        if e <= 0 {
            continue;
        }
        if buf8(fx, FX_LEN, b + 9) as i8 <= 0 {
            let n = e.wrapping_sub(10);
            fx_put_i32(fx, b, if n <= 0 { 0 } else { n });
        }
        unsafe {
            if buf8(fx, FX_LEN, b + 0xC) != 0 && (*w).fuse_beep_timer > 0 {
                (*w).fuse_beep_timer = (*w).fuse_beep_timer.wrapping_sub(10);
                if (*w).fuse_beep_timer <= 0 {
                    (*w).fuse_beep = 1;
                    (*w).fuse_beep_timer = 700;
                }
            }
        }
    }
}

// ---- Update end, Fuses ----
const FX_LEN: usize = 5 * 13;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SPLASH_LEN: usize = 60;

fn fx_i32(fx: *const u8, b: usize) -> i32 {
    i32::from_le_bytes([buf8(fx, FX_LEN, b), buf8(fx, FX_LEN, b + 1), buf8(fx, FX_LEN, b + 2), buf8(fx, FX_LEN, b + 3)])
}
fn fx_put_i32(fx: *mut u8, b: usize, v: i32) {
    if fx.is_null() || b + 4 > FX_LEN {
        return;
    }
    for (o, x) in v.to_le_bytes().iter().enumerate() {
        unsafe { fx.add(b + o).write(*x) };
    }
}

fn update_end(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let Some(a0) = Ai::new(w, h, 0) else { return 0 };
    let mut result = 0;
    // 1. Sector won.
    let cs = unsafe { (*w).cur_sector } as usize;
    let ss = unsafe { (*w).sector_state };
    let b0 = buf8(ss, SECTOR_STATE_LEN, 9 * cs);
    let b1 = buf8(ss, SECTOR_STATE_LEN, 9 * cs + 1);
    if b0 & 1 == 0 {
        if b1 == 0 {
            a0.call(op::SECTOR_GAINED, &[cs as u8 as i8 as i32]);
            for i in 0..8usize {
                if a0.rd8_of(i, 0x182) != 0 && a0.rd8_of(i, 0x183) as i8 >= 15 && a0.rd8_of(i, 0x138) != 0 {
                    a0.call(op::STAT_PRACTICE, &[i as i32, 9, 10, 0]);
                }
            }
            if unsafe { (*w).cur_sector } == 0 {
                a0.call(op::WON_QUOTE, &[0x1D, 0x63, 1]);
                unsafe { (*w).leave_reason = 1 };
                result = 1;
            }
        }
    } else if b1 > 0 && unsafe { (*w).enemies_warned } == 0 {
        a0.call(op::ENEMIES_MSG, &[]);
        unsafe { (*w).enemies_warned = 1 };
        a0.call(op::MUSIC, &[2, 0]);
    }
    // 2. Water ripples.
    if unsafe { (*w).ripple_timer } <= 0 {
        let t = a0.frame_random(3136);
        unsafe { (*w).ripple_tile = t };
        let cs = unsafe { (*w).cur_sector } as usize;
        let n = buf8(unsafe { (*w).splash }, SPLASH_LEN, cs) as i8 as i32;
        if a0.tb(t, 0) >= 0x97 && a0.tb(t, 1) == 0 && a0.tb(t, 5) & 0x7F >= 0x30 && a0.frame_random(n) == 0 {
            a0.call(op::TILE_ANIM, &[t, 0]);
        }
        unsafe { (*w).ripple_timer = n.wrapping_mul(12) };
    }
    // 3. Ambient.
    if unsafe { (*w).ambient_t1 } <= 0 && unsafe { (*w).opt_ambient } != 0 {
        a0.call(op::PAL_CYCLE, &[0xAC, 0xB2]);
        unsafe { (*w).ambient_t1 = 200 };
        if unsafe { (*w).ambient_t2 } <= 0 {
            a0.call(op::PAL_CYCLE, &[0xA5, 0xAB]);
            unsafe {
                (*w).ambient_t2 = 300;
                let c = (*w).ambient_count;
                (*w).ambient_count = if c < 0xFE { c + 1 } else { 0xF8 };
            }
            let r = a0.frame_random(20);
            if r == 0 && unsafe { (*w).time_compress } == 0 {
                let s = a0.frame_random(6) + 0x24;
                a0.call(op::SFX, &[s, 0x2B11, 0x23, 1]);
            }
        }
        a0.call(op::PAL_SET, &[0xA5, 0xB2]);
    }
    result
}

fn fuses(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a0) = Ai::new(w, h, 0) else { return };
    for f in 0..5usize {
        let b = 13 * f;
        let fx = unsafe { (*w).timed_fx };
        if fx_i32(fx, b) != 0 || buf8(fx, FX_LEN, b + 9) as i8 > 0 {
            continue;
        }
        fx_put_i32(fx, b, -1);
        let tile = i16::from_le_bytes([buf8(fx, FX_LEN, b + 6), buf8(fx, FX_LEN, b + 7)]) as i32;
        let k = buf8(fx, FX_LEN, b + 8);
        let owner = buf8(fx, FX_LEN, b + 0xB) as i32;
        if tile < NONE_I {
            // SPEC GAP: a tile off the map reads tile byte 6 as 0 (soldier_update.md "Notes for the port").
            let g = a0.tb(tile, 6) as usize;
            let item = g < GROUND
                && crate::sector::blast_kind(buf8(unsafe { (*w).ground_items }, GROUND * 12, 12 * g)) == k;
            if item || k == 0xA {
                a0.call(op::EXPLODE, &[owner, tile, k as i32, 1]);
            }
        } else {
            let a = buf8(fx, FX_LEN, b + 0xA) as i8 as i32;
            let bb = i16::from_le_bytes([buf8(fx, FX_LEN, b + 4), buf8(fx, FX_LEN, b + 5)]) as i32;
            let c = buf8(fx, FX_LEN, b + 0xC) as i8 as i32;
            a0.call(op::TIMED_EFFECT, &[k as i32, a, bb, owner, c]);
        }
    }
}

#[no_mangle]
pub extern "C" fn ja1_update_end(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| update_end(w, h))).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn ja1_fuses(w: *mut Ja1World, h: *const Ja1Host) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fuses(w, h)));
}

// ---- Loop head (ja1_loop_head) ----
fn loop_head(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let Some(a0) = Ai::new(w, h, 0) else { return 0 };
    let mut sum: u8 = 0;
    let new_turn = |sum: &mut u8| *sum = sum.wrapping_add(a0.call(op::NEW_TURN, &[0]) as u8);
    // 1. Turn timers.
    unsafe {
        if (*w).turn_tick < 1 {
            let t2 = (*w).turn_timer2;
            if (*w).screen_drawn == 0 {
                if t2 < 20 {
                    (*w).turn_timer2 = t2.wrapping_add(1);
                }
            } else if t2 >= 1 {
                (*w).turn_timer2 = t2.wrapping_sub(1);
            } else if (*w).turn_timer > 0 && (*w).new_round == 1 {
                (*w).turn_timer = (*w).turn_timer.wrapping_sub(1);
            }
            (*w).turn_tick = 10;
        }
    }
    // 2.
    let cs = unsafe { (*w).cur_sector } as usize;
    let ss = unsafe { (*w).sector_state };
    let enemies = buf8(ss, SECTOR_STATE_LEN, 9 * cs + 1);
    let mercs = buf8(ss, SECTOR_STATE_LEN, 9 * cs + 2);
    if enemies != 0 {
        if unsafe { (*w).combat } == 0 && mercs != 0 {
            // Combat starts.
            unsafe { (*w).combat = 1 };
            a0.call(op::MOUSE_SAVE, &[]);
            if unsafe { (*w).merc_to_show } >= 8 {
                let m = super::turns::first_merc(w, h) as u8;
                unsafe { (*w).merc_to_show = m };
            }
            new_turn(&mut sum);
            let a = if unsafe { (*w).opt_onscreen } < 3 { 1 } else { 0 };
            a0.call(op::STRIP, &[a, 1]);
            unsafe { (*w).turn_shown = 1 };
        } else if unsafe { (*w).redraw_pending } != 0 {
            new_turn(&mut sum);
        }
        unsafe { (*w).turn_clock = -1 };
    } else if unsafe { (*w).combat } != 0 {
        // Combat ends.
        unsafe {
            (*w).combat = 0;
            (*w).turn_clock = 20000;
            (*w).turn_shown = 0;
        }
        combat_end(w, h, &a0);
        if unsafe { (*w).redraw_pending } != 0 {
            new_turn(&mut sum);
        }
    } else if unsafe { (*w).turn_clock } == 0 {
        if unsafe { (*w).busy } < 1 {
            unsafe { (*w).busy = 0 };
            new_turn(&mut sum);
        }
    } else if unsafe { (*w).redraw_pending } != 0 {
        new_turn(&mut sum);
    }
    unsafe {
        // 3.
        if (*w).busy != 0 && (*w).busy_timer == 0 {
            (*w).busy_timer = 5000;
        }
        // 4.
        if (*w).selected >= 8 && (*w).turn_based == 0 && (*w).fast_timer <= 0 {
            (*w).fast_timer = 15000;
        }
    }
    // 5.
    sum as i32
}

/// SectorVictory? 0x3A0B0 (library op 187; MercDies at the death of the sector's last enemy). The nearby warning can
/// come again, the last killer says quote 0x16 (when it is at 15 hp or more; else the first merc on duty, when slot 0
/// holds a merc), combat stops here and not in the next loop head. BlinkUi(1) has no call here: with no enemy the
/// strips of each frame draw the plain sector box.
pub(super) fn sector_won(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a0) = Ai::new(w, h, 0) else { return };
    unsafe { (*w).enemies_warned = 0 };
    let k = unsafe { (*w).last_killer } as usize;
    if k < 0x40 && a0.rd8_of(k, 0x183) as i8 >= 15 {
        a0.call(op::MERC_MESSAGE, &[k as i32, 0x16, 0x40, 0, 1]);
    } else if a0.rd8_of(0, 0x182) != 0 {
        // TeamSize 0x513B8 is not 0 exactly when slot 0 holds a merc.
        // With no merc on duty the original takes entry 0x40 of the soldier table, which is map bytes (#2): no quote.
        let m = super::turns::first_merc(w, h);
        if m < 8 {
            a0.call(op::MERC_MESSAGE, &[m, 0x16, 0x40, 0, 1]);
        }
    }
    unsafe {
        (*w).combat = 0;
        (*w).turn_clock = 20000;
        (*w).turn_shown = 0;
    }
    combat_end(w, h, &a0);
}

#[no_mangle]
pub extern "C" fn ja1_sector_won(w: *mut Ja1World, h: *const Ja1Host) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sector_won(w, h)));
}

/// Combat end (original 0x106F4; soldier_update.md Loop head). Called when the last enemy leaves combat.
fn combat_end(w: *mut Ja1World, h: *const Ja1Host, a0: &Ai) {
    for i in 0..0x28usize {
        let Some(a) = Ai::new(w, h, i as i32) else { continue };
        if a.rd8(0x182) != 0 && a.rd8(0x138) != 0 && a.rd8(0x183) != 0 {
            a.wr8(0x1A5, 1);
            a.wr8(0x1B0, 0);
            a.wr8(0x194, 0);
            a.wr8(0x17E, 0);
            a.wr8(0x197, 0);
            a0.call(op::IDLE, &[i as i32]);
        }
    }
    unsafe {
        (*w).screen_drawn = 1;
        (*w).new_round = 1;
        if (*w).selected >= 8 {
            (*w).selected = super::turns::first_merc(w, h) as u8;
        }
    }
    let sel = unsafe { (*w).selected };
    if sel < 8 {
        a0.call(op::SELECT_LIGHT, &[sel as i32, 0xC]);
    }
    unsafe {
        (*w).interrupt_turn = 0;
        (*w).view_hold = 0;
    }
    a0.call(op::STRIP, &[1, 1]);
}

// ---- Leave tests (ja1_leave_tests) ----
const STORY_LEN: usize = 32;
fn leave_tests(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let Some(a0) = Ai::new(w, h, 0) else { return 0 };
    let mut r = 0;
    // 1. The home sector (entry 59).
    if buf8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, 9 * 59) & 1 == 0 {
        a0.call(op::MESSAGE, &[6]);
        r += 1;
        unsafe { (*w).leave_reason = 4 };
    }
    // 2.
    let st = unsafe { (*w).story };
    if buf8(st, STORY_LEN, 0x1C) != 0 && (buf8(st, STORY_LEN, 0x0E) as i8) < 2 {
        r += 1;
    }
    // 3.
    r
}

#[no_mangle]
pub extern "C" fn ja1_leave_tests(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| leave_tests(w, h))).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ja1_loop_head(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop_head(w, h))).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ja1_move_order(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, marker: i32, sneak: i32,
    byte177: i32, alt: i32, squad: i32) -> i32 {
    run(w, h, slot, 0, |a| a.move_order(s16(tile), marker as u8, sneak as u8, byte177 as u8, alt as u8, squad as u8))
}
#[no_mangle]
pub extern "C" fn ja1_move_gate(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, 0, |a| a.move_gates(s16(tile), false) as i32)
}
#[no_mangle]
pub extern "C" fn ja1_move_confirmed(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, marker: i32, sneak: i32,
    byte177: i32, alt: i32, walk_ap: i32) -> i32 {
    run(w, h, slot, 0, |a| a.move_go(s16(tile), marker as u8, sneak as u8, byte177 as u8, alt as u8, 0, Some(walk_ap)))
}
#[no_mangle]
pub extern "C" fn ja1_direction(x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
    direction(x1, y1, x2, y2)
}
#[no_mangle]
pub extern "C" fn ja1_aim_dir(x1: i32, y1: i32, x2: i32, y2: i32, word: *mut u16) -> i32 {
    let (d, wd) = aim_dir(x1, y1, x2, y2);
    if !word.is_null() {
        unsafe { *word = wd };
    }
    d as i32
}
#[no_mangle]
pub extern "C" fn ja1_tile_dist(a: i32, b: i32) -> i32 {
    tile_dist(a, b)
}
#[no_mangle]
pub extern "C" fn ja1_stop_anim(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    run(w, h, slot, (), |a| a.stop_anim(flag as u8))
}
#[no_mangle]
pub extern "C" fn ja1_frame(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| frame(w, h))).unwrap_or(0)
}
#[no_mangle]
pub extern "C" fn ja1_tick(w: *mut Ja1World, h: *const Ja1Host) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tick(w, h)));
}
/// Consume trigger needs no soldier and makes no host call: the host pointer is only checked for null like the
/// other entry points.
#[no_mangle]
pub extern "C" fn ja1_consume_trigger(w: *mut Ja1World, h: *const Ja1Host, index: i32) {
    // SPEC GAP: a null world or host does nothing.
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume_trigger(w, index)));
}

// ---- Consume trigger (op 106 consume_trigger) ----
/// The library stops the chain after this many nested levels (soldier_update.md).
const TRIGGER_DEPTH: u32 = 40;

/// The tile of trigger i in the sector row (None when there is no row or no sectors buffer).
fn trigger_tile_ptr(w: *mut Ja1World, i: i32) -> Option<*mut u8> {
    let (cs, p) = unsafe { ((*w).cur_sector as usize, (*w).sectors) };
    // SPEC GAP: an index outside 0..9 does nothing (the spec gives index 0..9 only).
    if cs >= SECTOR_ROWS || p.is_null() || !(0..10).contains(&i) {
        return None;
    }
    Some(unsafe { p.add(cs * SECTOR_ROW + 0x58 + 4 * i as usize) })
}

pub(super) fn consume_trigger(w: *mut Ja1World, index: i32) {
    if w.is_null() {
        return;
    }
    consume_level(w, index, 1);
}

fn consume_level(w: *mut Ja1World, index: i32, depth: u32) {
    if depth > TRIGGER_DEPTH {
        return;
    }
    let rd = |i: i32| trigger_tile_ptr(w, i).map(|p| unsafe { p.read() as u16 | (p.add(1).read() as u16) << 8 });
    // 1.
    let Some(p) = trigger_tile_ptr(w, index) else { return };
    let t = rd(index).unwrap_or(0);
    unsafe {
        p.write(NONE as u8);
        p.add(1).write((NONE >> 8) as u8);
    }
    // 2.
    for d in [-56i32, 1, 56, -1] {
        let v = (t as i32 + d) as u16;
        for i in 0..10 {
            if rd(i) == Some(v) {
                consume_level(w, i, depth + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_compass() {
        // screen y grows to the south; 1..8 = N, NE, E, SE, S, SW, W, NW; the same point gives 3.
        let c = (100, 100);
        let to = |dx: i32, dy: i32| direction(c.0, c.1, c.0 + dx, c.1 + dy);
        assert_eq!([to(0, -50), to(50, -50), to(50, 0), to(50, 50), to(0, 50), to(-50, 50), to(-50, 0), to(-50, -50)],
            [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(to(0, 0), 3);
    }

    #[test]
    fn aim_dir_same_point() {
        assert_eq!(aim_dir(5, 5, 5, 5), (3, 0));
    }

    #[test]
    fn tile_dist_parts() {
        assert_eq!(tile_dist(0, 57), 1);
        assert_eq!(tile_dist(0, 3135), 55);
        assert_eq!(tile_dist(56 * 3 + 10, 56 * 5 + 2), 8);
        // C division: tile -1 is row 0, column -1.
        assert_eq!(tile_dist(-1, 1), 2);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    unsafe extern "C" fn rec_n(ctx: *mut std::os::raw::c_void, n: i32) -> i32 {
        (*(ctx as *mut Vec<i32>)).push(n);
        0
    }

    unsafe extern "C" fn rec_op(ctx: *mut std::os::raw::c_void, op: i32, _a: *const i32, _n: i32, _o: *mut u8, _s: i32) -> u32 {
        (*(ctx as *mut Vec<i32>)).push(op);
        0
    }

    /// Fuses: a tile whose byte 6 is 200 or more holds no ground item (TileItemIndex 0x23774 gives 0xFF for it, the
    /// test turns.md End turn step 1 uses), so a run-out fuse of kind 0 there does not go off; kind 0xA does on any
    /// tile. Ledger row c2aea50:ja1logic:7963 (g < GROUND).
    #[test]
    fn fuse_on_tile_byte_200_is_no_item() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut tiles = vec![0u8; TILES * 8];
        tiles[100 * 8 + 6] = 200;
        let mut ground = vec![0u8; GROUND * 12];
        let mut fx = vec![0u8; FX_LEN];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        w.ground_items = ground.as_mut_ptr();
        w.timed_fx = fx.as_mut_ptr();
        let mut ops: Vec<i32> = vec![];
        let h = Ja1Host { ctx: &mut ops as *mut Vec<i32> as *mut std::os::raw::c_void, random: None, call: Some(rec_op),
                          host_ops: !0 };
        for (k, want) in [(0u8, 0usize), (0xA, 1)] {
            fx.iter_mut().for_each(|b| *b = 0);
            fx[6..8].copy_from_slice(&100i16.to_le_bytes());
            fx[8] = k;
            for f in 1..5 {
                fx[13 * f..13 * f + 4].copy_from_slice(&(-1i32).to_le_bytes());
            }
            ops.clear();
            ja1_fuses(&mut *w, &h);
            assert_eq!(ops.iter().filter(|&&o| o == op::EXPLODE).count(), want, "kind {k}");
        }
    }

    /// Port rule (SPEC GAP, no original counterpart): fuses with a null `timed_fx` reads every entry as 0 and drops
    /// the write of −1 (fx_put_i32 null rule) instead of a write through a null pointer. Ledger row
    /// c2aea50:ja1logic:7882.
    #[test]
    fn fuses_null_timed_fx_writes_nothing() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut tiles = vec![0u8; 0x10000 * 8];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        let mut ops: Vec<i32> = vec![];
        let h = Ja1Host { ctx: &mut ops as *mut Vec<i32> as *mut std::os::raw::c_void, random: None, call: Some(rec_op),
                          host_ops: !0 };
        ja1_fuses(&mut *w, &h);
        assert!(w.timed_fx.is_null());
    }

    /// StepNoise 0x19A6C (asm 0x19B4C cmp +0x17A, 8 / jge): the personality 3 (relentless) noise cut is for mercs
    /// only; a soldier of record 8 keeps q = 20, so the walk roll is Random(6 - 20/20 = 5).
    #[test]
    fn step_noise_relentless_mercs_only() {
        let mut sol = vec![0u8; 48 * 0x200];
        let r = 8 * 0x200;
        sol[r + 0x17A] = 8;
        sol[r + 0x192] = 4;
        sol[r + 0x183] = 50;
        sol[r + 0x18A] = 50;
        sol[r + 0x19C] = 3;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let mut ns: Vec<i32> = vec![];
        let h = Ja1Host { ctx: &mut ns as *mut Vec<i32> as *mut std::os::raw::c_void, random: Some(rec_n), call: None,
                          host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Ai { w: wp, h: &h, s: 8 }.step_noise()));
        assert_eq!(ns.first(), Some(&5));
    }

    /// Port rule (SPEC GAP): ap_warned has 8 entries (one per merc); the low-AP stop of slot 8 and up writes none.
    #[test]
    fn guard_low_ap_stop_warned() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut warned = [0u8; 10];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.ap_warned = warned.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Ai { w: wp, h: &h, s: 8 }.u_low_ap_stop(false)));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Ai { w: wp, h: &h, s: 7 }.u_low_ap_stop(false)));
        assert_eq!(warned, [0, 0, 0, 0, 0, 0, 0, 1, 0, 0]);
    }

    /// Port rule (SPEC GAP): a start or destination tile off the map gives "no path" (0).
    #[test]
    fn guard_find_path_op_off_map() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (ai.find_path_op(TILES as i32, 0), ai.find_path_op(-1, 0))
        }));
        assert_eq!(r.ok(), Some((0, 0)));
        sol[0x114] = 0xFF;
        sol[0x115] = 0xFF;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ai.find_path_op(5, 0)));
        assert_eq!(r.ok(), Some(0));
    }

}
