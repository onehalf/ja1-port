//! The tactical click (docs/spec/click.md; C interface in port_api.md): Tactical click (library op 30), Attack
//! ready (library op 15), Reach tile (library op 14), Gun jam (library op 68) and Jam check. A child module of
//! `ai`: every op goes through `Ai::call` (the host_ops rule). World data is read again after every host call.
use super::damage::get8;
use super::search::{px, py, s16, step, world_dist};
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT};

const ITEM_TYPES_LEN: usize = 136 * 12;
const CLASS_LEN: usize = 256;
const GROUND_LEN: usize = 200 * 12;

/// Water tile (t0, t1) (soldier_update.md; IsWaterTile 0x23A10).
fn water_tile(t0: u8, t1: u8) -> bool {
    t0 >= 0x6C && t1 != 0x1C && t1 != 0x1F
}
/// Tile parts (ai.md; C division on the signed tile).
fn row(t: i32) -> i32 {
    t / 56
}
fn column(t: i32) -> i32 {
    t - row(t) * 56
}

/// The steps of "Use" (click.md step 10): AP cost, sound of Spend AP (None = no Spend AP), task, animation,
/// reset byte of Set animation, facing (None = no facing written).
// SPEC GAP: "if a sound is given" — the spec writes "sound 0" for the hand and 0x76; a written sound (0 included) is
// taken as given (Spend AP with sound 0 runs). The move order of item 0x4D passes confirm 1 as written; the library
// ignores confirm (port_api.md op 140). The window uses ja1_click_detector: it runs the wait of the confirm. A library
// click with 0x4D (the walk end of a far detector click) leaves detector_slot / detector_tile when the option is on.
struct Use {
    cost: u8,
    sound: Option<i32>,
    task: u8,
    anim: i32,
    reset: i32,
    face: Option<u8>,
}

impl Ai {
    // ---- small definitions (click.md "Conventions") ----
    /// Byte b of item type it; SPEC GAP: an item type of 136 and up (or a null table) reads as 0.
    fn c_item(&self, it: u8, b: usize) -> u8 {
        get8(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, it as i64 * 12 + b as i64)
    }
    fn c_t1_class(&self, b: u8) -> u8 {
        get8(unsafe { (*self.w).t1_class }, CLASS_LEN, b as i64)
    }
    // SPEC GAP: off the map (e.g. tile NONE) the tile bytes read as 0, so the occupant is slot 0 there, as in
    // the other tile reads of the port.
    fn c_occupant(&self, t: i32) -> u8 {
        let o = self.tb(t, 5) & 0x7F;
        if o >= 0x30 { NO_SLOT } else { o }
    }
    fn c_selected(&self) -> u8 {
        unsafe { (*self.w).selected }
    }
    fn c_key_ctrl(&self) -> u8 {
        unsafe { (*self.w).key_ctrl }
    }
    fn c_click_panel(&self) {
        self.call(op::CLICK_PANEL, &[self.slot(), 1]);
    }
    fn c_message(&self, text: i32) {
        self.call(op::MESSAGE, &[text]);
    }
    /// The current animation word.
    fn c_anim(&self) -> u16 {
        self.rd16(0x1E8 + 2 * self.rd8(0x1F3) as usize)
    }

    /// AP check (ap, flag) → true = affordable.
    fn c_ap_check(&self, ap: u8, flag: u8) -> bool {
        let slot = self.slot();
        if (self.c_selected() as i32 == slot || flag as i8 == 99)
            && unsafe { (*self.w).combat } != 0
            && self.s8(0x1E2) + self.s8(0x1A5) < ap as i32
        {
            if slot < 8 {
                self.call(op::MERC_TEXT, &[slot, 1, 0x4B0]);
            } else {
                self.wr8(0x1B0, 1);
            }
            return false;
        }
        true
    }
    /// Face point (tile): the direction from the soldier's pixel point to the tile's centre.
    fn face_point(&self, tile: i32) -> u8 {
        let args = [s16(self.rd16(0x116) as i32), s16(self.rd16(0x118) as i32), column(tile) * 16 + 8, row(tile) * 14 + 7];
        self.call(op::DIRECTION, &args) as u8
    }
    /// Clear the task (flag).
    fn c_clear_task(&self, flag: bool) {
        self.wr8(0x1A6, 0);
        self.wr16(0x134, NONE);
        self.wr8(0x1A7, 0);
        if flag {
            self.wr8(0x95, 0);
            self.wr16(0x96, NONE);
        }
    }
    /// combat.md "Use cost, exact steps" (library op 121 use_cost): AP byte of walking to `spot` and using `kind`
    /// at `tile` (g = ground index byte).
    pub(super) fn use_cost(&self, spot: i32, tile: i32, g: u8, kind: u8) -> u8 {
        // 1.
        let p = self.path_ap(spot) as u8;
        // 2.
        if kind == 0 || kind >= 0xFE {
            let d = self.tile_standable(tile) as i8;
            let add = if d > 0 {
                4
            } else if d < 0 {
                3
            } else if g >= 200 {
                5
            } else if get8(unsafe { (*self.w).ground_items }, GROUND_LEN, 12 * g as i64 + 6) != 0 {
                4
            } else if self.slot() >= 0x28 && get8(unsafe { (*self.w).ground_items }, GROUND_LEN, 12 * g as i64) == 0x72 {
                2
            } else {
                5
            };
            return p.wrapping_add(add);
        }
        // 3.
        p.wrapping_add(match kind {
            0x76 | 0x9B => 10,
            0x96 => self.call(op::SHOT_AP, &[self.slot(), tile]) as u8,
            0x93 | 0x9E => 5,
            0x9D => 6,
            _ => 0,
        })
    }
    /// ai.md "Soldier busy" (library op 27 busy).
    pub(super) fn busy(&self) -> i32 {
        matches!(self.cur_anim(), 0x1B | 0x1F | 0x23) as i32
    }
    /// ai.md "Swap hands" (library op 16 swap_hand): items table[a] and table[b] change places.
    pub(super) fn swap_hand(&self, a: i8, b: i8) {
        const PLACE: [u8; 7] = [0x00, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x01];
        // 1. (SPEC GAP: indexes outside 0..6 are never passed; nothing happens then.)
        let (Some(&ia), Some(&ib)) = (PLACE.get(a as usize), PLACE.get(b as usize)) else { return };
        let (ia, ib) = (12 * ia as usize, 12 * ib as usize);
        let kept: Vec<u8> = (0..12).map(|i| self.rd8(ia + i)).collect();
        for i in 0..12 {
            self.wr8(ia + i, self.rd8(ib + i));
        }
        for (i, v) in kept.into_iter().enumerate() {
            self.wr8(ib + i, v);
        }
        // 2.
        let slot = self.rd8(0x17A);
        if slot == unsafe { (*self.w).selected } {
            self.call(op::ITEM_PANEL, &[slot as i32, 0, 1]);
        }
    }
    /// Ground index of a tile: its byte 6, 0xFF if that is 200 or more.
    fn ground_index(&self, t: i32) -> u8 {
        let g = self.tb(t, 6);
        if g >= 200 { 0xFF } else { g }
    }
    fn c_spend(&self, cost: u8, sound: i32) {
        self.call(op::SPEND_AP, &[self.slot(), cost as i8 as i32, sound]);
    }
    /// Use (click.md step 10); sets done.
    fn c_use(&self, tile: i32, u: Use, done: &mut bool) {
        if self.c_ap_check(u.cost, 0) {
            self.wr8(0x1A6, u.task);
            self.wr16(0x134, tile as u16);
            self.call(op::SET_ANIM, &[self.slot(), u.anim, 0, u.reset]);
            if let Some(snd) = u.sound {
                self.c_spend(u.cost, snd);
            }
            if let Some(d) = u.face {
                self.wr8(0x1A2, d);
            }
        }
        *done = true;
    }
    fn c_action_ap(&self, tile: i32) -> u8 {
        self.call(op::ACTION_AP, &[self.slot(), tile, 1]) as u8
    }

    // ---- Tactical click (op 30) ----
    pub(super) fn click(&self, tile: i32, item: i32) -> i32 {
        self.click_body(tile, item, false)
    }
    /// `defer`: the move order of item 0x4D does not run and the result is 2. The front-end runs it (ja1_click_detector).
    pub(super) fn click_body(&self, tile: i32, item: i32, defer: bool) -> i32 {
        let slot = self.slot();
        let tile = s16(tile);
        // 1.
        for k in 0..4 {
            self.wr8(0xB0 + k, 0);
        }
        let occ = self.c_occupant(tile);
        let g = self.ground_index(tile);
        // 2.
        if self.anim_col(self.c_anim(), 0) == 0 {
            return 0;
        }
        // 3.
        let it = item as u8;
        let mut k = self.c_item(it, 2);
        let (range, reach): (u8, u16);
        if it == 0 {
            range = 1;
            reach = 15;
            k = 0;
        } else if k == 0x95 {
            let mut r = self.c_item(it, 11) as i32 * 15 + self.s8(0x183) - 50;
            r = (r - ((100 - self.s8(0x18A)) * r) / 200).max(0);
            reach = r as u16;
            range = (reach as i16 as i32 / 15) as u8;
        } else {
            range = self.c_item(it, 11);
            reach = if range > 1 { range as u16 * 15 } else { 0x18 };
        }
        // 4.
        let me = s16(self.rd16(0x114) as i32);
        let mut walk = false;
        let dist = world_dist(s16(self.rd16(0x116) as i32), s16(self.rd16(0x118) as i32), px(tile), py(tile));
        if dist > reach as i16 as i32 {
            if it == 0 || (it > 0x23 && it != 0x5D) || range == 1 {
                walk = true;
                if it != 0 {
                    self.wr8(0x1A6, it);
                }
            }
        } else if range == 1
            && ((column(me) != column(tile) && row(me) != row(tile)) || (self.tile_standable(tile) > 0 && me != tile))
        {
            walk = true;
            if it != 0 {
                self.wr8(0x1A6, it);
            }
        }
        // 5.
        if unsafe { (*self.w).combat } == 0 {
            self.wr16(0x134, tile as u16);
        }
        // 6.
        if walk {
            // a.
            self.wr8(0x1A7, range);
            // b.
            if range != 1 {
                self.call(op::WALK_TO, &[slot, tile]);
                self.wr8(0x1A6, self.rd8(0));
                self.wr8(0x1A7, range);
                return 0;
            }
            // c.
            let task = self.rd8(0x1A6);
            let mut stop = false;
            if matches!(task, 0xF | 0x10 | 0x49 | 0x4A) {
                if occ == NO_SLOT {
                    self.c_message(2);
                    stop = true;
                    self.wr8(0x1A6, 0);
                }
            } else if task == 0x45 {
                if matches!(self.c_t1_class(self.tb(tile, 1)), 7 | 8) {
                    self.wr8(0x1A6, 0x45);
                } else {
                    stop = true;
                    self.wr8(0x1A6, 0);
                }
            }
            // d.
            if self.call(op::REACH_AP, &[slot, tile, self.rd8(0x1A6) as i32]) == 0 {
                self.c_message(3);
                self.wr8(0x1A6, 0);
                return 0;
            }
            // e.
            if stop {
                return 0;
            }
            // f.
            let spot = unsafe { (*self.w).mission_spot } as i16 as i32;
            let c = self.call(op::USE_COST, &[slot, spot, tile, g as i32, k as i32]) as u8;
            // g.
            if self.c_ap_check(c, 0) && !water_tile(self.tb(spot, 0), self.tb(spot, 1)) {
                self.wr8(0x1A6, if k == 0 { 0xFF } else { it });
                self.wr16(0x134, tile as u16);
                self.call(op::WALK_TO, &[slot, spot]);
                if self.c_key_ctrl() == 0 {
                    self.c_click_panel();
                }
                return 1;
            }
            // h.
            self.wr16(0x134, NONE);
            self.wr8(0x1A6, 0);
            return 0;
        }
        // 7.
        let mut done = false;
        let finish = |done: bool| -> i32 {
            if done && self.c_key_ctrl() == 0 {
                self.c_click_panel();
            }
            0
        };
        // 8.
        if (1..=0xE).contains(&it) {
            // a.
            self.aim_posture(tile);
            // b.
            if self.jam_check() == 0 {
                self.c_click_panel();
                return 0;
            }
            // c.
            if self.call(op::ATTACK_READY, &[slot, tile]) == 1 {
                self.wr8(0x1D8, occ);
                if occ < NO_SLOT {
                    if self.s8_of(occ as usize, 0x9C) > 0 {
                        self.wr8(0x1C2, occ);
                        if slot < 0x28 && occ < 0x28 {
                            self.wr8(0x1C3, 1);
                        }
                    }
                }
                if occ == NO_SLOT {
                    self.wr8(0x1C2, 0xFE);
                }
                self.wr8(0x1BE, 1);
                self.wr16(0xDC, tile as u16);
                self.call(op::ATTACK_CURSOR, &[]);
                done = true;
            } else if self.rd8(1) == 0 && self.call(op::RELOADED, &[slot]) != 0 && slot < 8 {
                // d.
                self.call(op::MERC_MESSAGE, &[slot, 5, 0x40, 0, 1]);
                self.wr8(0x1CD, 1);
            }
            // e.
            return finish(done);
        }
        // 9.
        if it != 0 && self.jam_check() == 0 {
            self.c_click_panel();
            return 0;
        }
        if self.in_water() {
            self.c_click_panel();
            return 0;
        }
        // 11.
        match it {
            0 => {
                if tile == me {
                    self.c_use(tile, Use { cost: 4, sound: Some(0), task: 0xFF, anim: 0xB, reset: 0, face: None }, &mut done);
                } else {
                    let d = self.face_point(tile);
                    if self.tile_standable(tile) < 0 {
                        self.c_use(tile, Use { cost: 3, sound: Some(0x1E), task: 0xFF, anim: 0x26, reset: 0, face: Some(d) }, &mut done);
                    } else {
                        let gi = get8(unsafe { (*self.w).ground_items }, GROUND_LEN, 12 * g as i64);
                        let c = if slot >= 0x28 && g < 200 && gi == 0x72 { 2 } else { 5 };
                        self.c_use(tile, Use { cost: c, sound: Some(0), task: 0xFF, anim: 0xC, reset: 0, face: Some(d) }, &mut done);
                    }
                }
                finish(done)
            }
            0x4C | 0x57 | 0x58 => {
                let (cost, sound) = if it == 0x4C { (10, -250) } else { (3, 0x1E) };
                let d = self.face_point(tile);
                if self.tile_standable(tile) < 0 || (it == 0x4C && self.call(op::QUEST_CHECK, &[tile]) as u8 != 0) {
                    self.c_use(tile, Use { cost, sound: Some(sound), task: it, anim: 0x26, reset: 0, face: Some(d) }, &mut done);
                }
                done = true;
                finish(done)
            }
            0x4D => {
                if defer {
                    return 2;
                }
                // MoveMercTo 0x1C7A8: a merc (slot < 8) with the confirm option on waits for a second click. The
                // library cannot wait: the walk end's TacticalClick (ActiveFrame 0x1BFD9) leaves the move order to
                // the front-end, which opens the wait after the pass (the gates run there, as in "Fire").
                if slot < 8 && unsafe { (*self.w).opt_confirm_move } != 0 {
                    unsafe {
                        (*self.w).detector_slot = slot as u8;
                        (*self.w).detector_tile = tile as i16;
                    }
                } else {
                    self.call(op::MOVE_ORDER, &[slot, tile, 1, 0, 1, 0, 1, 0]);
                }
                finish(done)
            }
            0xF | 0x10 => {
                if tile == me {
                    return finish(done);
                }
                let c = self.c_action_ap(tile);
                if self.c_ap_check(c, 0) {
                    self.aim_posture(tile);
                    if self.call(op::ATTACK_READY, &[slot, tile]) == 1 {
                        if occ < NO_SLOT {
                            self.wr8(0x1D8, occ);
                            self.wr8(0x1C2, occ);
                            if slot < 0x28 && occ < 0x28 {
                                self.wr8(0x1C3, 1);
                            }
                        }
                        self.wr8(0x1BE, 1);
                        self.wr16(0xDC, tile as u16);
                        done = true;
                    }
                }
                finish(done)
            }
            0x14 | 0x16 | 0x18 | 0x1A | 0x1C | 0x1D | 0x5D => {
                let c = self.c_action_ap(tile);
                if self.c_ap_check(c, 0) {
                    let d = self.face_point(tile);
                    self.wr8(0x1C2, 0xFE);
                    self.wr8(0x1A2, d);
                    self.wr16(0xDC, tile as u16);
                    done = true;
                    self.wr8(0x1BE, 1);
                    self.c_click_panel();
                }
                finish(done)
            }
            0x49 | 0x4A => {
                if self.c_ap_check(5, 0) {
                    let (d, r) = if tile == me {
                        (5u8, self.call(op::FIRST_AID, &[slot, me, 6, 5]) as u8)
                    } else {
                        let d = self.face_point(tile);
                        (d, self.call(op::FIRST_AID, &[slot, tile, 5, d as i32]) as u8)
                    };
                    done = r != 0;
                    self.c_clear_task(true);
                    self.c_click_panel();
                    if r == 0 {
                        self.wr8(0x1A2, d);
                    } else {
                        self.wr16(0x120, NONE);
                    }
                }
                finish(done)
            }
            0x11 | 0x17 | 0x19 | 0x1B | 0x1E | 0x1F | 0x20 => {
                if it == 0x11 && tile != me && self.tile_standable(tile) <= 0 {
                    self.c_message(4);
                    self.c_clear_task(true);
                    done = true;
                    return finish(done);
                }
                let (cost, sound) = if it == 0x11 { (10u8, 0xFA) } else { (6u8, 0x14) };
                let d = if tile != me { self.face_point(tile) } else { 0 };
                if self.c_ap_check(cost, 0) {
                    self.wr8(0x1A6, it);
                    self.wr16(0x134, tile as u16);
                    if tile == me {
                        self.call(op::SET_ANIM, &[slot, 0x13, 0, 1]);
                    } else {
                        self.wr8(0x1A2, d);
                        self.call(op::SET_ANIM, &[slot, 0x12, 0, 1]);
                    }
                    self.c_spend(cost, sound);
                }
                done = true;
                finish(done)
            }
            0x76 => {
                if tile == me {
                    return finish(done);
                }
                let d = self.face_point(tile);
                self.c_use(tile, Use { cost: 10, sound: Some(0), task: 0x76, anim: 0xC, reset: 0, face: Some(d) }, &mut done);
                finish(done)
            }
            0x45 => {
                if tile == me {
                    return finish(done);
                }
                if !matches!(self.c_t1_class(self.tb(tile, 1)), 7 | 8) {
                    self.wr8(0x1A6, 0);
                    return finish(done);
                }
                self.wr8(0x1A6, 0x45);
                self.wr16(0x134, tile as u16);
                let d = self.face_point(tile);
                if self.c_ap_check(5, 0) {
                    self.wr8(0x1A2, d);
                    self.call(op::SET_ANIM, &[slot, 0x2E, 0, 1]);
                }
                done = true;
                finish(done)
            }
            0x87 => {
                if tile == me {
                    return finish(done);
                }
                let d = self.face_point(tile);
                if self.c_ap_check(8, 0) {
                    self.wr8(0x1A6, 0x87);
                    self.wr8(0x1A2, d);
                    self.wr16(0x134, tile as u16);
                    self.call(op::SET_ANIM, &[slot, 0xC, 0, 1]);
                    self.c_spend(8, -50);
                } else {
                    self.c_clear_task(true);
                }
                done = true;
                finish(done)
            }
            _ => {
                if it == 0x46 && self.s8(1) > 0 && self.random(100) < self.s8(2) {
                    self.call(op::SFX, &[0x2C, 0x2B11, 0x58, 1]);
                    if unsafe { (*self.w).opt_step_text } != 0 {
                        self.c_message(5);
                    }
                    self.wr8(1, self.rd8(1).wrapping_sub(1));
                    if self.c_selected() as i32 == slot {
                        self.call(op::ITEM_COND, &[0, slot, 0, 1]);
                    }
                }
                done = true;
                self.c_clear_task(true);
                finish(done)
            }
        }
    }

    // ---- Escape (world_load.md "Quit"): stop the selected merc ----
    pub(super) fn escape_stop(&self) {
        let slot = self.slot();
        unsafe { (*self.w).shown_slot = 0x40 };
        self.u_forget_attack(true);
        self.wr8(0x1F7, 0x40);
        self.c_clear_task(true);
        self.wr16(0x120, self.rd16(0x114));
        self.call(op::END_TASK, &[slot, 1]);
        self.wr8(0x197, 0);
        self.u_tile_point_stop(1);
        if self.u_at_waypoint(self.s) {
            self.call(op::ANIM_STEP, &[slot]);
        }
        if unsafe { (*self.w).view_hold } != 0 {
            self.call(op::STOP_WALK, &[slot, 0]);
        }
    }

    // ---- Jam check ----
    pub(super) fn jam_check(&self) -> i32 {
        let slot = self.slot();
        let it = self.rd8(0);
        let c = self.s8(2);
        if self.c_item(it, 7) != 0 && c > 15 && it != 0x11 && self.random(0x50) + 20 > c + 50 {
            let v = self.random(10) + 1;
            self.wr8(2, v as u8);
            if self.c_selected() as i32 == slot {
                self.call(op::ITEM_PANEL, &[slot, 0, 1]);
            }
            self.call(op::MERC_MESSAGE, &[slot, 0xF, 0x40, 0, 1]);
            return 0;
        }
        1
    }

    // ---- Attack ready (op 15) ----
    pub(super) fn attack_ready_op(&self, tile: i32) -> i32 {
        let a = self.c_anim();
        // 1.
        if s16(tile) == s16(self.rd16(0x114) as i32) {
            return -3;
        }
        if water_tile(self.rd8(0x1B2), self.rd8(0x1B4)) {
            return -2;
        }
        if self.anim_col(a, 1) == 0 {
            return -4;
        }
        // 2.
        let it = self.rd8(0);
        if !(1..=0x10).contains(&it) && self.c_item(it, 2) != 0x95 {
            return -6;
        }
        if self.rd8(1) == 0 {
            return -5;
        }
        // 3.
        if self.anim_col(a, 0) == 0 { -1 } else { 1 }
    }

    // ---- Reach tile (op 14) ----
    pub(super) fn reach_tile(&self, tile: i32, flag: u8) -> i32 {
        let slot = self.slot();
        let tile = s16(tile);
        // 1.
        let st = self.tile_standable(tile);
        if st > 0 {
            if self.call(op::FIND_PATH, &[slot, tile, 0]) != 0 {
                unsafe { (*self.w).mission_spot = tile as u16 };
                return 1;
            }
            // SPEC GAP: "Else u16 +0xD4 = 0" is read as the else of the path search (st > 0 and no path).
            self.wr16(0xD4, 0);
        }
        // 2.
        let b = self.tb(tile, 1);
        let mut best = 0x100;
        let mut pick = 0;
        for (i, d) in [1, 3, 5, 7].into_iter().enumerate() {
            let n = step(tile, self.dir_step(d));
            if self.tile_standable(n) <= 0 {
                continue;
            }
            if self.call(op::FIND_PATH, &[slot, n, 1]) == 0 {
                continue;
            }
            if water_tile(self.tb(n, 0), self.tb(n, 1)) {
                continue;
            }
            let c = s16(self.rd16(0xD4) as i32);

            if c >= best {
                continue;
            }
            if !((flag > 0 && flag < 0xFE) || st < 0 || b < 0x37 || b >= 0x83 || n > tile) {
                continue;
            }
            pick = n;
            let _ = i;
            best = c;
        }
        // 3.
        if best >= 0x100 {
            return 0;
        }
        unsafe { (*self.w).mission_spot = pick as u16 };
        1
    }

    // ---- Gun jam (op 68) ----
    pub(super) fn gun_jam(&self) {
        let slot = self.slot();
        // 1.
        if slot >= 8 || self.rd8(0x1C9) != 0 {
            return;
        }
        let c = self.s8(1);
        if c == 0 {
            return;
        }
        // 2.
        if c > 0 {
            let v = self.s8(2) + self.random(0x32);
            if self.random(0x64) < v {
                return;
            }
        }
        // 3.
        if c < 0 {
            let m = self.s8(0x190);
            let p = if m > 0 { m / 2 + 0x14 } else { 0x14 };
            if self.random(0x64) < p {
                self.wr8(1, (-c) as u8);
                self.call(op::ITEM_PANEL, &[slot, 0, 1]);
                self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 1, 1, 1]);
                self.call(op::VOC, &[0x2B11, 0x32, 1]);
                self.call(op::STAT_PRACTICE, &[slot, 7, 2, 0]);
                return;
            }
        }
        // 4.
        self.call(op::FOCUS, &[slot, 1]);
        self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 0, 1, 1]);
        self.call(op::VOC, &[0x2B11, 0x32, 1]);
        // 5.
        if c > 0 {
            self.wr8(1, (-c) as u8);
            self.call(op::ITEM_PANEL, &[slot, 0, 1]);
            self.u_forget_attack(true);
            self.call(op::MERC_MESSAGE, &[slot, 0x10, 0x40, 0, 1]);
        } else {
            self.u_forget_attack(true);
        }
    }
}

// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing and returns 0.
#[no_mangle]
pub extern "C" fn ja1_click(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, item: i32) -> i32 {
    run(w, h, slot, 0, |a| a.click(tile, item))
}
/// Tactical click with the mine detector 0x4D in hand, for the window. MoveMercTo 0x1C7A8..0x1C7C5 waits for a confirm
/// click when the option is on, and the library cannot wait: result 2 = the move order of step 11 is for the caller.
#[no_mangle]
pub extern "C" fn ja1_click_detector(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, 0, |a| a.click_body(tile, 0x4D, true))
}
/// Escape on the tactical screen: only when selected < 8 and (combat = 0 or screen_drawn != 0).
#[no_mangle]
pub extern "C" fn ja1_escape_stop(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    let (sel, combat, drawn) = unsafe { ((*w).selected, (*w).combat, (*w).screen_drawn) };
    if sel < 8 && (combat == 0 || drawn != 0) {
        run(w, h, sel as i32, (), |a| a.escape_stop());
    }
}
#[no_mangle]
pub extern "C" fn ja1_attack_ready(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) -> i32 {
    run(w, h, slot, 0, |a| a.attack_ready_op(tile))
}
#[no_mangle]
pub extern "C" fn ja1_reach_tile(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, flag: i32) -> i32 {
    run(w, h, slot, 0, |a| a.reach_tile(tile, flag as u8))
}
#[no_mangle]
pub extern "C" fn ja1_gun_jam(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.gun_jam())
}
#[no_mangle]
pub extern "C" fn ja1_jam_check(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.jam_check())
}

/// combat.md Use cost, exact steps (library op 121).
#[no_mangle]
pub extern "C" fn ja1_use_cost(w: *mut Ja1World, h: *const Ja1Host, slot: i32, spot: i32, tile: i32, g: i32, kind: i32) -> i32 {
    run(w, h, slot, 0, |a| a.use_cost(s16(spot), s16(tile), g as u8, kind as u8) as i32)
}
#[no_mangle]
pub extern "C" fn ja1_swap_hand(w: *mut Ja1World, h: *const Ja1Host, slot: i32, a: i32, b: i32) {
    run(w, h, slot, (), |x| x.swap_hand(a as i8, b as i8))
}
#[no_mangle]
pub extern "C" fn ja1_busy(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |x| x.busy())
}
