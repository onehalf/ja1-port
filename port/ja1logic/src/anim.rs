//! Animation script, frame delay (docs/spec/ai.md "Frame delay (`ja1_anim_delay`)"; C interface in
//! docs/spec/port_api.md). A child module of `ai`: it extends the `Ai` view of one soldier record.
//! Later animation procedures go here too.
use super::search::{px, py, step};
use super::{op, run, Ai, Ja1Host, Ja1World, NONE};
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

/// #59 switch: true = the original endless AnimFrame/AnimNext repeat (#2 row 60), false (default) = the port's fix.
static KEEP_ANIM_LOOP: AtomicBool = AtomicBool::new(false);
thread_local! {
    /// (slot, script position) of the AnimNext -> AnimFrame repeat that runs now (+0x136 != 0), for the #59 fix.
    static REPEAT: Cell<Option<(i32, u32)>> = const { Cell::new(None) };
}

/// Turn the original game's endless animation repeat (#2 row 60) back on (true) or off (false, the default).
pub fn set_keep_anim_loop(on: bool) {
    KEEP_ANIM_LOOP.store(on, Ordering::Relaxed);
}

/// Soldier record offsets used here (defs/types.toml struct.soldier).
const A4: usize = 0x0A4; // i32
const DELAY: usize = 0x0D2; // u16, ticks per frame
const SLOT_BYTE: usize = 0x17A;
const HP: usize = 0x183;
const ENERGY: usize = 0x18A;
const AGILITY: usize = 0x18B;
const B1B3: usize = 0x1B3;
const B1B8: usize = 0x1B8;
const ANIM: usize = 0x1E8;
const ANIM_IDX: usize = 0x1F3;
const FLAG_19F: usize = 0x19F;
// Next animation (`ja1_anim_step`):
const CODE_FIXED: i32 = 1000;
const ITEM0: usize = 0x000;
const TASK_95: usize = 0x095;
const TASK_96: usize = 0x096; // u16
const TILE: usize = 0x114; // u16
const POS_X: usize = 0x116; // u16
const POS_Y: usize = 0x118; // u16
const GOAL: usize = 0x124; // u16 (+0x126, +0x128 = its point)
const GOAL_X: usize = 0x126;
const GOAL_Y: usize = 0x128;
const NEXT: usize = 0x12A; // u16
const TASK_134: usize = 0x134; // u16
const B_BA: usize = 0x0BA;
const B_177: usize = 0x177;
const B_195: usize = 0x195;
const B_1A1: usize = 0x1A1;
const B_1A2: usize = 0x1A2;
const B_1A3: usize = 0x1A3;
const B_1A4: usize = 0x1A4;
const B_1A6: usize = 0x1A6;
const B_1A7: usize = 0x1A7;
const B_1A8: usize = 0x1A8;
const B_1B2: usize = 0x1B2;
const B_1B9: usize = 0x1B9;
const B_1CC: usize = 0x1CC;
const B_1DA: usize = 0x1DA;
const B_1E1: usize = 0x1E1;
const B_1E2: usize = 0x1E2;
const B_1E3: usize = 0x1E3;
const B_1FA: usize = 0x1FA;

/// s8(u8(100 - v)): 100 - v kept to its low 8 bits, then read as a signed byte.
fn inv(v: u8) -> i32 {
    (100i32 - v as i32) as u8 as i8 as i32
}

impl Ai {
    pub(super) fn anim_delay(&self) {
        let hp = self.rd8(HP);
        let energy = self.rd8(ENERGY);
        let agility = self.rd8(AGILITY);
        // a = current animation code (u16 at +0x1E8 + 2 x animIdx).
        let a = self.rd16(ANIM + 2 * self.rd8(ANIM_IDX) as usize) as i32;

        // 1. fixed ticks per frame from the row.
        // SPEC GAP: a code >= 256 has no row; d = 0 (computed).
        let d = if a < 256 {
            let p = unsafe { (*self.w).anim_rows };
            if p.is_null() { 0 } else { unsafe { p.add(8 * a as usize + 2).read() } }
        } else {
            0
        };
        self.wr16(DELAY, d as u16);
        if d != 0 {
            return;
        }
        // 2.
        if matches!(a, 4 | 0x33 | 0x36) {
            let v = (100 - hp as i8 as i32) * 10 + (energy as i8 as i32) * 10;
            let mut v = v as u16;
            if (v as i16) < 50 {
                v = 50;
            }
            self.wr16(DELAY, v);
            return;
        }
        // 3.
        if a == 0x39 {
            let h = inv(hp);
            let n = inv(energy);
            self.wr16(DELAY, (4 * n.max(25) + h.max(25)) as u16);
            return;
        }
        // 4.
        let base: i32 = match a {
            2 | 0xD | 0x1A | 0x1D | 0x1E | 0x21 | 0x22 | 0x25 | 0x3A => 50,
            0x16 => {
                (self.rd16(A4) as u32 | (self.rd16(A4 + 2) as u32) << 16) as i32
            }
            0x17 => 100,
            0x34 => 40,
            0x37 => 80,
            _ => 0,
        };
        // 5.
        self.wr8(FLAG_19F, if (0x1A..=0x27).contains(&(a as u16 as i16 as i32)) { 0 } else { 2 });
        // 6.
        let extra: i32 = if self.rd8(B1B8) != 0 {
            40
        } else {
            2 * match self.rd8(B1B3) {
                1 | 2 => 1,
                4 | 5 => 2,
                6 | 7 => 4,
                8 | 9 => 6,
                _ => 0,
            }
        };
        // 7. SPEC GAP: "slot" is read as the record's own slot byte (+0x17A, u8), not the call argument.
        let (tired, agil) = if self.rd8(SLOT_BYTE) >= 0x28 {
            (0, 20)
        } else {
            (inv(energy).min(50), inv(agility).min(70).max(20))
        };
        // 8.
        let hurt = inv(hp).max(15).min(50);
        // 9. 32-bit sum, low 16 bits stored.
        let sum = hurt.wrapping_add(tired).wrapping_add(agil).wrapping_add(base).wrapping_add(extra);
        self.wr16(DELAY, sum as u16);
    }
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens.
#[no_mangle]
pub extern "C" fn ja1_anim_delay(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.anim_delay())
}

/// Direction from (x1, y1) to (x2, y2) (Next animation, "Direction"; DirectionPx 0x15A90, AxisRatio 0x383E0).
fn direction(x1: i32, y1: i32, x2: i32, y2: i32) -> u8 {
    let (dx, dy) = (x2.wrapping_sub(x1), y2.wrapping_sub(y1));
    if dx == 0 {
        return if dy < 0 { 1 } else if dy > 0 { 5 } else { 0 };
    }
    if dy == 0 {
        return if dx < 0 { 7 } else { 3 };
    }
    let (ax, ay) = (dx.unsigned_abs() as i64, dy.unsigned_abs() as i64);
    let r = if ax > ay {
        (ax / ay > 2) as usize
    } else if ay / ax > 2 {
        2
    } else {
        0
    };
    let t: [u8; 3] = match (dx < 0, dy < 0) {
        (true, true) => [8, 7, 1],
        (true, false) => [6, 7, 5],
        (false, true) => [2, 3, 1],
        (false, false) => [4, 3, 5],
    };
    t[r]
}

impl Ai {
    pub(super) fn s16_at(&self, off: usize) -> i32 {
        self.rd16(off) as i16 as i32
    }
    pub(super) fn row_byte(&self, row: i32, col: usize) -> u8 {
        // SPEC GAP: a row outside 0..255 (or a null table) reads as 0.
        if !(0..256).contains(&row) {
            return 0;
        }
        unsafe {
            let p = (*self.w).anim_rows;
            if p.is_null() { 0 } else { p.add(8 * row as usize + col).read() }
        }
    }
    /// The current animation code (u16 at +0x1E8 + 2 x animIdx), read now.
    pub(super) fn cur_anim(&self) -> u16 {
        self.rd16(ANIM + 2 * self.rd8(ANIM_IDX) as usize)
    }
    fn at_step_point(&self) -> bool {
        !(self.rd8(B_1B9) < 5
            && self.cur_anim() >= 4
            && (self.rd16(TILE) != self.rd16(GOAL)
                || self.rd16(POS_X) != self.rd16(GOAL_X)
                || self.rd16(POS_Y) != self.rd16(GOAL_Y)))
    }
    fn can_walk(&self, c: u16) -> bool {
        let c16 = c as i16 as i32;
        let mut v = self.row_byte(c16, 4) as i8 as i32;
        if v == 0 && (c == 0x31 || c == 0x32) {
            v = 1;
        }
        if c16 <= 1 && ((self.rd8(HP) as i8 as i32) < 15 || self.rd8(ENERGY) == 0) {
            v = 0;
        }
        v != 0
    }
    pub(super) fn stop_own_tile(&self) {
        let t = self.s16_at(TILE);
        self.wr16(GOAL_X, px(t) as u16);
        self.wr16(GOAL_Y, py(t) as u16);
        self.call(op::STOP_ANIM, &[self.slot(), 1]);
    }
    pub(super) fn start_leg(&self) {
        let slot = self.slot();
        // 1.
        let t = self.rd16(NEXT);
        if t == NONE {
            self.wr8(B_1FA, 0);
            return;
        }
        // 2.
        let o = self.occupant(t);
        if o < 0x40 && self.rd8(B_1A7) < 1 {
            let own = self.s16_at(TILE);
            if (self.call(op::TILE_DIST, &[own, t as i16 as i32]) as i32) < 3 {
                self.stop_own_tile();
                return;
            }
        }
        // 3.
        self.wr16(GOAL, self.rd16(NEXT));
        self.wr16(NEXT, NONE);
        // 4.
        if self.rd8(B_1FA) == 0 {
            let g = self.s16_at(GOAL);
            if self.call(op::FIND_PATH, &[slot, g, 1]) == 0 {
                if slot < 8 {
                    self.call(op::MERC_MESSAGE, &[slot, 0xD, 0x40, 0, 1]);
                    self.wr8(B_1A6, 0);
                    self.wr16(TASK_134, NONE);
                    self.wr8(B_1A7, 0);
                    self.wr8(TASK_95, 0);
                    self.wr16(TASK_96, NONE);
                }
                self.stop_own_tile();
                let t2 = self.s16_at(GOAL);
                let d = direction(self.s16_at(POS_X), self.s16_at(POS_Y), px(t2), py(t2));
                self.wr8(B_1A2, d);
                return;
            }
        }
        // 5.
        // SPEC GAP: paths of a slot are read as 0 when the paths pointer is null.
        let p0 = unsafe {
            let p = (*self.w).paths;
            if p.is_null() { 0 } else { p.add(80 * self.s).read() }
        };
        self.wr8(B_1A2, p0.wrapping_add(1));
        self.wr8(B_1FA, 0);
        self.wr8(B_1E1, 0);
    }
    fn water(&self, b: i32) -> i32 {
        let g = self.rd8(B_1B2);
        if g < 0x97 { b } else if g < 0x9A { b + 3 } else { b + 6 }
    }

    pub(super) fn anim_step(&self) {
        let slot = self.slot();
        let mut code: i32 = CODE_FIXED;
        // 1.
        if self.rd8(B_1A2) == self.rd8(B_1A1) {
            self.wr8(B_1A3, 0);
        }
        // 2.
        let a = self.cur_anim();
        let a16 = a as i16 as i32;
        let moving;
        if self.at_step_point() {
            let mut m = false;
            let t = self.s16_at(NEXT);
            if self.rd16(NEXT) != NONE && self.can_walk(a) {
                // SPEC GAP: only the direction update depends on the row byte; the leg starts either way.
                if self.row_byte(a16, 6) != 0 {
                    let d = direction(self.s16_at(POS_X), self.s16_at(POS_Y), px(t), py(t));
                    self.wr8(B_1A2, d);
                }
                self.start_leg();
                m = true;
            }
            moving = m;
        } else {
            moving = true;
            if self.rd16(NEXT) != NONE {
                self.start_leg();
            }
        }
        // 3.
        if moving {
            self.wr8(B1B8, 0);
            if a16 < 0x32 {
                code = if a == 0 {
                    2
                } else if a <= 2 {
                    0xD
                } else if self.rd8(B_177) != 0 {
                    0x18
                } else if self.rd8(B_1A8) != 0 {
                    0x16
                } else {
                    0x15
                };
            } else {
                code = self.water(0x34);
            }
            self.call(op::END_TASK, &[slot, 0]);
        } else if a16 < 0x32 {
            if self.rd8(B_BA) as i8 == 2 {
                code = 0x11;
                self.wr8(B_1A2, self.rd8(B_1A4));
            } else {
                if self.rd8(B_1B9) >= 5 && self.rd8(B_195) == 0 {
                    self.call(op::END_TASK, &[slot, 1]);
                }
                if a16 != 5 {
                    code = 4;
                }
                if self.rd8(B_1CC) != 0 {
                    self.wr8(B_1CC, 0);
                    self.call(op::REPLACE, &[slot]);
                }
            }
        } else {
            code = self.water(0x33);
        }
        // 4.
        if self.rd8(B_195) != 0 {
            if self.rd8(HP) != 0 {
                self.wr8(B_195, 0);
                let item = self.rd8(ITEM0) as i32;
                let sel = unsafe { (*self.w).selected } as i32;
                if !self.w_combat()
                    && slot < 0x10
                    && self.rd8(B_1DA) < 8
                    && sel != slot
                    && (self.rd8(HP) as i8 as i32) >= 15
                    && self.rd8(B_1B2) < 0x6C
                    && self.cur_anim() >= 4
                    && item > 0
                    && item <= 0x10
                    && code == 4
                {
                    self.call(op::SET_ANIM, &[slot, 4, 1, 0]);
                    self.wr8(B_1E1, 0);
                    code = CODE_FIXED;
                    let other = self.rd8(B_1DA) as usize;
                    let ot = self.rd16_of(other, TILE) as i16 as i32;
                    self.call(op::CLICK, &[slot, ot, item]);
                }
            }
            // SPEC GAP: the "then if +0x1E1 != 0" step is read as independent of the hp test (same list level).
            if self.rd8(B_1E1) != 0 {
                code = self.rd8(B_1E1) as i8 as i32;
                self.wr8(B_1E1, 0);
                self.wr8(B_1E2, self.rd8(B_1E2).wrapping_add(self.rd8(B_1E3)));
                self.wr8(B_1E3, 0);
            }
        }
        // 5.
        if code < CODE_FIXED {
            self.call(op::SET_ANIM, &[slot, code & 0xFFFF, 1, 0]);
        }
    }
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens.
#[no_mangle]
pub extern "C" fn ja1_anim_step(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.anim_step())
}

// ---- Starting an animation (`ja1_anim_set`) ----
const SCRIPT_POS: usize = 0x090; // u32, byte offset into anim_scripts
const B_09C: usize = 0x09C;
const B_1A5: usize = 0x1A5;
const B_1AD: usize = 0x1AD;
const B_1B0: usize = 0x1B0;
const B_1F3: usize = ANIM_IDX;
const B_1FE: usize = 0x1FE;

impl Ai {
    /// Table read by code (unsigned index). SPEC GAP: a code >= 256, or a null table, reads as 0.
    fn table_byte(&self, which: u8, code: u16, off: usize, len: usize) -> u8 {
        if code >= 256 {
            return 0;
        }
        unsafe {
            let p = match which {
                0 => (*self.w).anim_free,
                1 => (*self.w).anim_sounds,
                _ => (*self.w).anim_scripts,
            };
            if p.is_null() || off >= len { 0 } else { p.add(off).read() }
        }
    }
    /// The u16 word at a byte offset of anim_scripts. SPEC GAP: outside the 68 x 0x50 byte table (or a null
    /// table) it reads as 0.
    fn script_word(&self, off: u32) -> u16 {
        let p = unsafe { (*self.w).anim_scripts };
        let off = off as usize;
        if p.is_null() || off + 2 > 68 * 0x50 {
            return 0;
        }
        unsafe { p.add(off).read() as u16 | (p.add(off + 1).read() as u16) << 8 }
    }
    pub(super) fn rd32(&self, off: usize) -> u32 {
        self.rd16(off) as u32 | (self.rd16(off + 2) as u32) << 16
    }
    pub(super) fn wr32(&self, off: usize, v: u32) {
        self.wr16(off, v as u16);
        self.wr16(off + 2, (v >> 16) as u16);
    }
    /// queue[animIdx] = v, with animIdx read now.
    fn queue_set(&self, v: u16) {
        self.wr16(ANIM + 2 * self.rd8(B_1F3) as usize, v);
    }
    fn idx_inc(&self) {
        self.wr8(B_1F3, self.rd8(B_1F3).wrapping_add(1));
    }
    /// QueueTwoAnims 0x17F5C: queue[animIdx] = cur, animIdx + 1, queue[animIdx] = next, +0x1FE = 0.
    pub(super) fn queue_two_anims(&self, next: u16, cur: u16) {
        self.queue_set(cur);
        self.idx_inc();
        self.queue_set(next);
        self.wr8(B_1FE, 0);
    }
    /// AP check for animation a: true = passes.
    fn ap_check(&self, a: u16) -> bool {
        if self.table_byte(0, a, a as usize, 256) != 0 {
            return true;
        }
        self.ap_cost_check(self.row_byte(a as i32, 0) as i32)
    }
    /// The cost part of the AP check (Starting an animation) for cost c: true = passes.
    fn ap_cost_check(&self, c: i32) -> bool {
        let (sel, combat) = unsafe { ((*self.w).selected as usize, (*self.w).combat) };
        if sel == self.s && combat != 0 && self.s8(0x1E2) + self.s8(B_1A5) < c {
            if self.s < 8 {
                self.call(op::MERC_TEXT, &[self.slot(), 1, 0x4B0]);
            } else {
                self.wr8(B_1B0, 1);
            }
            return false;
        }
        true
    }
    fn start_sound(&self, a: u16) {
        let lo = self.table_byte(1, a, 2 * a as usize, 512);
        let hi = self.table_byte(1, a, 2 * a as usize + 1, 512);
        let snd = (lo as u16 | (hi as u16) << 8) as i16 as i32;
        self.call(op::SPEND_AP, &[self.slot(), self.row_byte(a as i32, 0) as i32, snd]);
    }

    pub(super) fn anim_set(&self, code: u16, keep: u8, reset: u8) {
        let slot = self.slot();
        // 1.
        if self.row_byte(self.cur_anim() as i32, 4) == 0
            && self.script_word(self.rd32(SCRIPT_POS)) <= 0x15E
            && reset == 0
        {
            return;
        }
        // 2.
        if code == self.cur_anim() {
            self.anim_delay();
            if keep != 0 {
                self.wr32(SCRIPT_POS, self.cur_anim() as u32 * 0x50);
            }
            self.wr8(B_1AD, 0);
            self.call(op::ANIM_FRAME, &[slot]);
            return;
        }
        // 3.
        let sel = unsafe { (*self.w).selected } as usize;
        if sel == self.s && code != 9 && code != 10 {
            let pv = unsafe { (*self.w).path_preview } != 0;
            self.call(op::UI_REFRESH, &[pv as i32]);
        }
        // 4.
        let (mut first, mut second) = (1000i32, 1000i32);
        let (mut chain, mut posture) = (false, false);
        if code == 4 || code == 0x15 {
            match self.cur_anim() {
                0 => { chain = true; first = 0xD; second = 2; }
                1 => { chain = true; first = 0xD; }
                5 => { chain = true; first = 7; }
                0xF => { chain = true; first = 0x10; }
                0x17 | 0x18 => { chain = true; first = 0x19; self.wr8(B_177, 0); }
                0x1A..=0x1C => { chain = true; first = 0x1D; posture = true; }
                0x1E..=0x20 => { chain = true; first = 0x21; posture = true; }
                0x22..=0x24 => { chain = true; first = 0x25; }
                _ => {}
            }
        } else if code == 0x43 && self.cur_anim() == 1 {
            let mut d = self.rd8(B_1A1).wrapping_add(4);
            if d > 8 {
                d = d.wrapping_sub(8);
            }
            self.wr8(B_1A2, d);
            self.wr8(B_1A1, d);
            self.call(op::ANIM_TURN, &[slot, 0]);
        }
        if chain {
            // 5. SPEC GAP: "only one check runs": with a second animation only it is checked (a fail = no pick,
            // first is not tried); without one, first is checked.
            let pick = if second != 1000 { second } else { first };
            if !self.ap_check(pick as u16) {
                return;
            }
            self.queue_set(code);
            self.idx_inc();
            self.queue_set(first as u16);
            self.wr8(B_1FE, 0);
            if second != 1000 {
                self.queue_set(first as u16);
                self.idx_inc();
                self.queue_set(second as u16);
                self.wr8(B_1FE, 0);
            }
            self.start_sound(pick as u16);
            if posture {
                self.call(op::RESERVE_AP, &[slot]);
            }
        } else {
            // 6.
            if reset != 0 {
                self.wr8(B_1F3, 0);
            }
            if code < 2 || code >= 0x3C {
                self.wr8(B_195, 0);
            }
            if self.row_byte(code as i32, 0) == 99 {
                self.queue_set(code);
                self.wr8(B_1FE, 0);
                self.start_sound(code);
            } else {
                if !self.ap_check(code) {
                    return;
                }
                self.queue_set(code);
                self.wr8(B_1FE, 0);
                self.start_sound(code);
                if code == 0x24 && self.s8(B_09C) > 0 {
                    self.call(op::SFX, &[11, 0x2B11, 0x58, 1]);
                }
            }
        }
        // 7.
        if matches!(code, 0x15 | 0x34 | 0x37 | 0x3A) {
            self.call(op::WALK_SOUND, &[slot, 1]);
        }
        // 8.
        self.anim_delay();
        self.wr32(SCRIPT_POS, self.cur_anim() as u32 * 0x50);
        self.wr8(B_1AD, 0);
        self.call(op::ANIM_FRAME, &[slot]);
    }
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens.
#[no_mangle]
pub extern "C" fn ja1_anim_set(w: *mut Ja1World, h: *const Ja1Host, slot: i32, code: i32, keep: i32, reset: i32) {
    run(w, h, slot, (), |a| a.anim_set(code as u16, keep as u8, reset as u8))
}

/// QueueTwoAnims 0x17F5C (op 181 knockdown_anim): queue cur, then next, and clear +0x1FE.
#[no_mangle]
pub extern "C" fn ja1_knockdown_anim(w: *mut Ja1World, h: *const Ja1Host, slot: i32, next: i32, cur: i32) {
    run(w, h, slot, (), |a| a.queue_two_anims(next as u16, cur as u16))
}

// ---- Animation end (`ja1_anim_end`, op 83) ----
impl Ai {
    pub(super) fn anim_end(&self) {
        let slot = self.slot();
        let q = |i: usize| self.rd16(ANIM + 2 * i);
        // 1.
        let idx = self.rd8(B_1F3) as usize;
        if idx == 0 {
            self.call(op::ANIM_STEP, &[slot]);
            return;
        }
        // 2.
        let prev = q(idx - 1);
        let mut c = self.row_byte(prev as i32, 0);
        if c == 99 {
            c = 0;
        }
        if self.table_byte(0, prev, prev as usize, 256) == 0 && !self.ap_cost_check(c as i32) {
            return;
        }
        // 3.
        let from_d = q(idx) == 0xD;
        self.wr16(ANIM + 2 * idx, 1000);
        let idx = idx - 1;
        self.wr8(B_1F3, idx as u8);
        // 4.
        let a = q(idx) as u8;
        self.wr32(SCRIPT_POS, a as u32 * 0x50);
        // 5.
        if matches!(a, 0x15 | 0x34 | 0x37 | 0x3A) {
            self.call(op::WALK_SOUND, &[slot, 1]);
        }
        // 6.
        if a == 4 && from_d {
            self.call(op::REPLACE, &[slot]);
        }
        // 7.
        self.wr8(B_1FE, 0);
        self.anim_delay();
        self.wr8(B_1AD, 0);
        self.call(op::ANIM_FRAME, &[slot]);
        // 8.
        self.start_sound(a as u16);
        // 9.
        if self.row_byte(a as i32, 3) != 0
            && (self.rd16(GOAL_X) != self.rd16(POS_X) || self.rd16(GOAL_Y) != self.rd16(POS_Y))
        {
            self.call(op::ANIM_STEP, &[slot]);
        }
    }
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens. The byte argument is not used.
#[no_mangle]
pub extern "C" fn ja1_anim_end(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.anim_end())
}

// ---- Frame routine, Next word, Turn done (ai.md) ----
impl Ai {
    pub(super) fn seen(&self) -> bool {
        self.s8(B_09C) > 0
    }
    pub(super) fn on_screen(&self) -> bool {
        let (os, vx, vy, vw, vh) = unsafe {
            let w = &*self.w;
            (w.opt_onscreen, w.view_x, w.view_y, w.view_w, w.view_h)
        };
        if os == 0 {
            return true;
        }
        let y = if self.s16_at(TILE) < 56 {
            self.s16_at(POS_Y)
        } else {
            self.s16_at(POS_Y).wrapping_sub(self.s16_at(0x11C))
        };
        let x = self.s16_at(POS_X).wrapping_sub(self.s16_at(0x11A));
        !(x.wrapping_add(20) < vx
            || y.wrapping_add(20) < vy
            || x > vx.wrapping_add(vw).wrapping_sub(1)
            || y > vy.wrapping_add(vh).wrapping_sub(1))
    }
    fn sound(&self, e: i32, v: i32, n: i32) {
        self.call(op::SFX, &[e, 0x2B11, v, n]);
    }
    fn blocked(&self, t: i32) -> bool {
        self.tile_standable(t) < 1 || (self.tb(t, 0) >= 0x6C && !matches!(self.tb(t, 1), 0x1C | 0x1F))
    }
    fn turn_way(a: i32, b: i32) -> i32 {
        if a == b {
            return 0;
        }
        let d = (a - b).abs();
        if d == 4 {
            return 1;
        }
        if a > b {
            if d <= b + 8 - a { -1 } else { 1 }
        } else if d <= 8 - b + a {
            1
        } else {
            -1
        }
    }
    fn turn_one_step(&self) {
        let way = Self::turn_way(self.s8(B_1A1), self.s8(B_1A2));
        self.wr8(B_1A1, (self.rd8(B_1A1) as i32).wrapping_add(way) as u8);
        if self.s8(B_1A1) > 8 {
            self.wr8(B_1A1, 1);
        } else if self.s8(B_1A1) < 1 {
            self.wr8(B_1A1, 8);
        }
        if self.rd8(B_1A1) == self.rd8(B_1A2) {
            self.call(op::REVEAL_VIEW, &[self.slot(), self.rd8(B_1A1) as i32, self.rd8(0x19E) as i32, 0]);
        }
    }
    fn wrap_dir(d: u8) -> u8 {
        let v = d as i8;
        if v > 8 { d.wrapping_sub(8) } else if v < 1 { d.wrapping_add(8) } else { d }
    }
    fn rotate(&self, clockwise: bool) {
        let f = self.rd8(B_1A1);
        let n = Self::wrap_dir(if clockwise { f.wrapping_add(1) } else { f.wrapping_sub(1) });
        self.wr8(B_1A2, n);
        self.wr8(B_1A1, self.rd8(B_1A2));
        self.call(op::ANIM_TURN, &[self.slot(), 1]);
    }
    fn around_value(&self) -> u8 {
        let d = self.rd8(B_1A1).wrapping_add(4);
        if d > 8 { d.wrapping_sub(8) } else { d }
    }
    fn turn_around(&self) {
        let d = self.around_value();
        self.wr8(B_1A2, d);
        self.wr8(B_1A1, d);
    }
    pub(super) fn clear_task(&self) {
        self.wr8(B_1A6, 0);
        self.wr16(TASK_134, NONE);
        self.wr8(B_1A7, 0);
        self.wr8(TASK_95, 0);
        self.wr16(TASK_96, NONE);
    }
    fn forget_tiles(&self) {
        if self.s8(B_BA) == 2 {
            return;
        }
        // SPEC GAP: a slot of 16..0x27 indexes past the 16 x 8 last_seen_m table; those writes are skipped.
        let (p, base, n, len) = unsafe {
            if self.s < 0x28 {
                ((*self.w).last_seen_m, 8 * self.s, 8, 256)
            } else {
                ((*self.w).last_seen_e, 40 * (self.s - 0x28), 40, 640)
            }
        };
        if p.is_null() {
            return;
        }
        for j in 0..n {
            let o = 2 * (base + j);
            if o + 2 <= len {
                unsafe {
                    p.add(o).write(NONE as u8);
                    p.add(o + 1).write((NONE >> 8) as u8);
                }
            }
        }
    }
    fn path_dir_at(&self, k: usize) -> u8 {
        let i = self.s * 80 + k;
        unsafe {
            let p = (*self.w).paths;
            if p.is_null() || i >= 48 * 80 { 0 } else { p.add(i).read() }
        }
    }
    /// NextPathDir 0x195F8.
    pub(super) fn next_path_direction(&self) -> u8 {
        if self.s16_at(0xD4) > 0 {
            return self.path_dir_at(self.rd8(0x1AF) as usize).wrapping_add(1);
        }
        if !self.at_step_point() {
            if self.rd16(TILE) != self.rd16(GOAL) {
                self.wr16(NEXT, self.rd16(GOAL));
                self.start_leg();
                return self.path_dir_at(self.rd8(0x1AF) as usize).wrapping_add(1);
            }
            let a = [self.s16_at(POS_X), self.s16_at(POS_Y), self.s16_at(GOAL_X), self.s16_at(GOAL_Y)];
            return self.call(op::DIRECTION, &a) as u8;
        }
        self.rd8(B_1A1)
    }
    fn tile_write5(&self, t: i32, v: u8) {
        if !(0..3136).contains(&t) {
            return;
        }
        unsafe {
            let p = (*self.w).tiles;
            if !p.is_null() {
                p.add(t as usize * 8 + 5).write(v);
            }
        }
    }
    pub(super) fn occupy(&self, t: i32) -> bool {
        self.occupy_for(t, self.s as u8)
    }
    /// Occupy tile t for the given slot number (also used by the sector start, where the slot is a record byte).
    pub(super) fn occupy_for(&self, t: i32, slot: u8) -> bool {
        if t > 0xC40 {
            return false;
        }
        let b5 = self.tb(t, 5);
        let mut o = b5 & 0x7F;
        if o >= 0x30 {
            o = 0x40;
        }
        if o != slot {
            if o < 0x30 {
                return false;
            }
            self.tile_write5(t, slot | (b5 & 0x80));
        }
        true
    }
    pub(super) fn vacate(&self, t: i32) {
        self.tile_write5(t, (self.tb(t, 5) & 0x80) | 0x40);
    }
    fn flag_table(&self, which: u8, code: u16) -> bool {
        if code >= 256 {
            return false;
        }
        unsafe {
            let p = if which == 0 { (*self.w).anim_steps_tile } else { (*self.w).anim_turn_while };
            !p.is_null() && p.add(code as usize).read() != 0
        }
    }

    pub(super) fn anim_frame(&self) {
        let slot = self.slot();
        // 1.
        let p = self.rd32(SCRIPT_POS);
        let w = self.script_word(p);
        let mut go = true;
        let mut stp: i32 = 0;
        let opt_sound = unsafe { (*self.w).opt_sound } != 0;
        // 2.
        match w {
            5 | 0xA => {
                if opt_sound && self.seen() {
                    self.sound(0x1B, 0xF, 1);
                }
            }
            0x2D => {
                if self.seen() {
                    self.sound(0x12, 0x32, 1);
                }
            }
            0x57 | 0x85 => self.sound(0xD, 0x32, 1),
            0x8F | 0x98 => {
                if self.seen() {
                    self.sound(0x1C, 0x1E, 1);
                }
            }
            0x93 => {
                if self.seen() {
                    self.sound(0x1A, 0x1E, 1);
                }
            }
            0x96 => {
                if self.seen() {
                    self.sound(0x1B, 0x32, 1);
                }
            }
            0x1B8..=0x1BC => {
                // SPEC GAP: "seen and on screen" is tested when the word is reached; on screen reads the record fresh.
                if self.seen() && self.on_screen() {
                    let (e, v, n) = [(0x1F, 0x32, 1), (0x21, 0x32, 1), (0x1E, 0x32, 1), (0x1F, 0x32, 2), (0x19, 0x32, 1)]
                        [(w - 0x1B8) as usize];
                    self.sound(e, v, n);
                }
            }
            0x37 => {
                if self.s8(B_1A3) == -1 {
                    self.turn_one_step();
                }
                go = false;
            }
            0x54 => {
                self.call(op::EFFECT_SLOT, &[slot & 0xFF, 3]);
            }
            0x1B6 => unsafe { (*self.w).anim_signal = -1 },
            0x1B7 => {
                if self.w_combat() {
                    let t = unsafe { (*self.w).effect_tile } as i32;
                    self.call(op::TILE_EFFECT, &[t, 0]);
                }
            }
            0x1BD => self.wr8(0x1BE, 0),
            0x1BE => self.wr8(B_1FE, 1),
            0x1BF => {
                self.wr32(SCRIPT_POS, p.wrapping_add(2));
                go = false;
                self.call(op::QUOTE, &[slot, self.s16_at(TASK_134)]);
            }
            0x1C0 | 0x1E4 | 0x1ED => {}
            0x1C1 => {
                self.wr32(SCRIPT_POS, p.wrapping_add(2));
                go = false;
                self.call(op::GUN_JAM, &[slot]);
            }
            0x1C2 => {
                go = false;
                self.wr32(SCRIPT_POS, p.wrapping_add(2));
                if self.rd8(B_1A6) == 0x4C
                    && self.call(op::QUEST_CHECK, &[self.s16_at(TASK_134)]) & 0xFF != 0
                {
                    self.call(op::USE_SPECIAL, &[slot]);
                } else {
                    self.call(op::DOOR_FX, &[slot, self.s16_at(TASK_134), self.rd8(B_1A6) as i32]);
                }
                self.clear_task();
            }
            0x1C3 => {
                if !matches!(self.s8(B_1A2), 3 | 5) {
                    self.wr32(SCRIPT_POS, p.wrapping_add(10));
                }
            }
            0x1C4 => self.wr8(0x9D, 1),
            0x1C5 => {
                if self.s8(B_1A3) == -1 {
                    self.wr32(SCRIPT_POS, p.wrapping_add(2));
                }
            }
            0x1C6 => {
                let v = (self.cur_anim() as u32 * 0x50).wrapping_add(2u32.wrapping_mul(self.script_word(p.wrapping_add(2)) as u32));
                self.wr32(SCRIPT_POS, v);
                go = false;
            }
            0x1C7 => {
                if self.rd8(B_1A1) == self.rd8(B_1A2) {
                    self.call(op::ANIM_STEP, &[slot]);
                    self.call(op::ANIM_TURN, &[slot, 1]);
                } else {
                    self.call(op::SET_ANIM, &[slot, 8, 1, 0]);
                }
                go = false;
            }
            0x1C8 => {
                if self.s8(B_1A3) == -1 {
                    self.wr32(SCRIPT_POS, p.wrapping_add(4));
                }
            }
            0x1C9 => {
                if self.s8(B_1A3) == 1 {
                    self.wr32(SCRIPT_POS, p.wrapping_add(2));
                }
            }
            0x1CA => {
                if self.s8(B_1A3) == 1 {
                    self.turn_one_step();
                }
            }
            0x1CB => {
                let r = self.frame_random(12);
                if r == 0 && self.s8(ENERGY) > 70 && self.rd8(B_1B9) == 0 && self.rd8(0x185) == 0 {
                    let c = if self.s8(B_1A1) % 2 == 0 { 9 } else { 10 };
                    self.call(op::SET_ANIM, &[slot, c, 0, 0]);
                    go = false;
                }
            }
            0x1CC => go = false,
            0x1CD..=0x1D4 => {
                let d = (w - 0x1CC) as u8;
                self.wr8(B_1A1, d);
                self.wr8(B_1A2, d);
            }
            0x1D5 | 0x1D6 => {
                self.call(op::POPUPS, &[slot, (w == 0x1D5) as i32]);
                self.call(op::STOP_WALK, &[slot, 0]);
            }
            0x1D7..=0x1DA => stp = (w - 0x1D6) as i32,
            0x1DB..=0x1DE => stp = -((w - 0x1DA) as i32),
            0x1DF => {
                let d = self.next_path_direction();
                self.wr8(B_1A2, d);
            }
            0x1E0 | 0x1E2 => self.rotate(true),
            0x1E1 | 0x1E3 => self.rotate(false),
            0x1E5 => {
                let me = self.s16_at(TILE);
                let back = self.around_value() as i32;
                let f = self.rd8(B_1A1) as i32;
                if self.blocked(step(me, self.dir_step(back))) || self.blocked(step(me, self.dir_step(f))) {
                    self.call(op::SET_ANIM, &[slot, 0x2B, 0, 1]);
                    go = false;
                }
            }
            0x1E6 => {
                if self.rd8(0x17D) == self.rd8(B_1A1) {
                    self.wr8(0x17D, 0);
                    self.wr16(0x136, 0);
                    self.call(op::SET_ANIM, &[slot, 1, 1, 1]);
                    go = false;
                } else {
                    if self.rd8(0x17D) == 0 {
                        self.wr8(0x17D, self.rd8(B_1A1));
                    }
                    let me = self.s16_at(TILE);
                    let f = self.rd8(B_1A1) as i32;
                    if self.blocked(step(me, self.dir_step(f)))
                        || (self.s8(B_1A1) == 4 && self.tb(me + 1, 1) >= 0x83)
                    {
                        self.rotate(true);
                        self.wr16(0x136, 0x28);
                        go = false;
                    } else {
                        self.wr16(0x136, 0);
                    }
                }
            }
            0x1E7 => {
                self.turn_around();
                self.call(op::ANIM_TURN, &[slot, 0]);
            }
            0x1E8 => {
                go = false;
                self.wr32(SCRIPT_POS, p.wrapping_add(2));
                self.call(op::PICKUP_HERE, &[slot]);
            }
            0x1E9 => {
                go = false;
                self.wr32(SCRIPT_POS, p.wrapping_add(2));
                if self.cur_anim() == 0xC {
                    self.call(op::USE_ON_TILE, &[slot, self.s16_at(TASK_134), self.rd8(B_1A6) as i32]);
                } else {
                    self.call(op::BURY_MINE, &[slot, self.s16_at(TASK_134)]);
                }
            }
            0x1EA => {
                self.call(op::REVIVE_ROLL, &[slot, 0]);
                go = false;
            }
            0x1EB => {
                self.call(op::MELEE_HIT, &[slot]);
            }
            0x1EC => {
                self.wr16(GOAL, self.rd16(TILE));
                self.wr16(0x120, NONE);
                self.call(op::STOP_WALK, &[slot, 0]);
                if self.s8(HP) <= 0 {
                    self.call(op::KILL, &[slot, 0]);
                    go = false;
                }
            }
            0x1EE => {
                self.next_word();
                let t = self.s16_at(0xDC);
                if t != NONE as i32 && self.rd8(0x001) != 0 {
                    self.call(op::START_ATTACK, &[slot, t]);
                    self.call(op::ATTACK_GO, &[slot, t]);
                }
                go = false;
            }
            0x1EF => {
                self.call(op::LAUNCH, &[slot]);
            }
            0x1F0 => self.wr8(B_1AD, self.rd8(B_1AD) ^ 1),
            0x1F1 => {
                if self.s8(HP) < 15 {
                    self.wr16(0x120, NONE);
                    for o in 0x13A..=0x161 {
                        self.wr8(o, 0);
                    }
                    self.forget_tiles();
                    self.wr8(0x1D5, 0);
                    self.wr8(0x1D8, 0x40);
                    self.wr8(0x1C2, 0x40);
                    self.wr8(0x1BE, 0);
                    self.wr16(0xD0, NONE);
                    self.wr16(0xDC, NONE);
                    self.wr16(GOAL_X, self.rd16(POS_X));
                    self.wr16(GOAL_Y, self.rd16(POS_Y));
                    self.wr16(GOAL, self.rd16(TILE));
                    if !self.in_water() {
                        // SPEC GAP: both calls are read as under "if cur != 0".
                        if self.cur_anim() != 0 {
                            self.call(op::SET_ANIM, &[slot, 0x2A, 0, 1]);
                            self.call(op::END_TASK, &[slot, 0]);
                        }
                    } else if self.cur_anim() < 0x40 {
                        self.call(op::KILL, &[slot, 0]);
                    }
                    go = false;
                }
            }
            0x1F2 => {
                self.call(op::MERC_DIES, &[slot, 0]);
                go = false;
            }
            0x1F3 => {
                self.call(op::ANIM_END, &[slot, 0]);
                go = false;
            }
            _ => go = false,
        }
        // 3.
        if stp == 0 {
            if go {
                self.next_word();
            }
            return;
        }
        // 4.
        let ox = self.rd16(POS_X);
        let oy = self.rd16(POS_Y);
        let g = stp * 65 / 100;
        let mv = match self.rd8(B_1A1) {
            1 => Some((0, -stp)),
            2 => Some((stp, -g)),
            3 => Some((stp, 0)),
            4 => Some((stp, g)),
            5 => Some((0, stp)),
            6 => Some((-stp, g)),
            7 => Some((-stp, 0)),
            8 => Some((-stp, -g)),
            _ => None,
        };
        if let Some((dx, dy)) = mv {
            self.wr16(POS_X, ox.wrapping_add(dx as u16));
            self.wr16(POS_Y, oy.wrapping_add(dy as u16));
        }
        // 5.
        let t = self.s16_at(POS_X) / 16 + (self.s16_at(POS_Y) / 14) * 56;
        if t == self.s16_at(TILE) {
            self.next_word();
            return;
        }
        // 6.
        if self.tile_standable(t) < 1 || !self.occupy(t) {
            self.wr16(POS_X, ox);
            self.wr16(POS_Y, oy);
            self.next_word();
            return;
        }
        // 7.
        self.vacate(self.s16_at(TILE));
        self.wr16(TILE, t as u16);
        if self.flag_table(0, self.cur_anim()) {
            self.wr16(GOAL_X, px(t) as u16);
            self.wr16(GOAL_Y, py(t) as u16);
            self.call(op::STOP_ANIM, &[slot, 1]);
            self.next_word();
        } else {
            self.call(op::STOP_ANIM, &[slot, 1]);
            self.wr16(GOAL_X, self.rd16(POS_X));
            self.wr16(GOAL_Y, self.rd16(POS_Y));
            self.next_word();
        }
    }

    pub(super) fn next_word(&self) {
        let slot = self.slot();
        // 1.
        if self.rd16(0x136) != 0 {
            // ORIGINAL BUG (#2 row 60, fixed in the port: the maintainer ruling on #59, 2026-11-07): +0x136 stays set after an
            // Escape stop in the fall search (word 0x1E6); a later knock-back reaches word 0x1E5 with free tiles, and
            // AnimFrame -> AnimNext -> AnimFrame runs the same word again with no step, without end (stack overflow).
            // The port holds the frame for this tick when the same soldier would run the same word again, and clears
            // the stale +0x136 (as word 0x1E6 does at the end of a blocked fall), so the next tick steps on and the
            // merc gets up. The switch `set_keep_anim_loop(true)` keeps the original endless repeat.
            let p = self.rd32(SCRIPT_POS);
            let again = REPEAT.with(|r| r.get() == Some((slot, p)));
            if again && !KEEP_ANIM_LOOP.load(Ordering::Relaxed) {
                self.wr16(0x136, 0);
                return;
            }
            let old = REPEAT.with(|r| r.replace(Some((slot, p))));
            self.anim_frame();
            REPEAT.with(|r| r.set(old));
            return;
        }
        // 2.
        let mut p = self.rd32(SCRIPT_POS);
        let v = self.script_word(p);
        if v < 0xB6 || v == 0x1CC {
            self.wr8(0x1F4, v as u8);
        }
        // 3.
        p = p.wrapping_add(2);
        self.wr32(SCRIPT_POS, p);
        let v = self.script_word(p);
        if v > 0x1F3 {
            self.call(op::SET_ANIM, &[slot, v.wrapping_sub(0x1F4) as i32, 1, 0]);
        } else {
            self.anim_frame();
        }
        // 4.
        if self.rd8(B_1A1) == self.rd8(B_1A2) {
            self.call(op::ANIM_TURN, &[slot, 1]);
        } else if self.flag_table(1, self.cur_anim()) {
            self.turn_one_step();
        }
    }

    pub(super) fn anim_turn_done(&self, flag: u8) {
        let slot = self.slot();
        // 1.
        if self.rd8(0x1D1) == 1 && self.rd8(0x1D6) != 0 {
            self.wr8(0x1D6, 0);
            self.wr8(0x1D1, 0);
            self.wr16(0x132, NONE);
        } else {
            return;
        }
        // 2.
        if flag != 0 {
            self.call(op::REVEAL_VIEW, &[slot, self.rd8(B_1A1) as i32, self.rd8(0x19E) as i32, 0]);
            if self.s >= 0x28 {
                self.call(op::ENEMY_TURNED, &[slot]);
            } else if self.s < 0x10 {
                self.call(op::GUARD_TURNED, &[slot]);
                self.call(op::SHARE_SIGHTINGS, &[slot, 0]);
            }
        }
    }
}

// SPEC GAP: on a null world / host or a slot outside 0..47 nothing happens.
#[no_mangle]
pub extern "C" fn ja1_anim_frame(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.anim_frame())
}

#[no_mangle]
pub extern "C" fn ja1_anim_turn(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    run(w, h, slot, (), |a| a.anim_turn_done(flag as u8))
}

impl Ai {
    /// Turning toward a direction (library op 28 turn; ai.md "Turning toward a direction"): 1 = it turns.
    pub(super) fn turn_to(&self, d: u8, ticks: i32) -> i32 {
        let a = (self.cur_anim() & 0xFF) as u16;
        let face = self.s8(0x1A1);
        // 1.
        if d as i32 == face || a < 4 {
            return 0;
        }
        // 2.
        self.wr8(0x1A3, Self::turn_way(face, d as i32) as u8);
        // 3.
        let anim = if a <= 10 {
            true
        } else if self.anim_col(a, 0) == 0 {
            return 0;
        } else if self.anim_col(a, 2) != 0 {
            false
        } else if a < 0x32 {
            true
        } else if a < 0x3D {
            false
        } else {
            return 0;
        };
        // 4.
        if self.s < 8 && self.rd8(0x16B) != 0 && self.w_combat() {
            let warned = unsafe { (*self.w).ap_warned };
            let mut ap = self.s8(0x1A5);
            if self.u_ap_warned() == 0 {
                ap -= self.s8(0xEF);
            }
            if 1 > ap {
                unsafe { (*self.w).stop_timer = 2000 };
                if !warned.is_null() {
                    unsafe { warned.add(self.s).write(1) };
                }
                return 1;
            }
        }
        // 5.
        let slot = self.slot();
        self.wr8(0x1A2, d);
        if anim {
            self.call(op::SET_ANIM, &[slot, 8, 0, 0]);
        }
        self.call(op::SPEND_AP, &[slot, 1, 10]);
        self.wr16(0xD2, (ticks as u8 as i8 as i16) as u16);
        1
    }
}

#[no_mangle]
pub extern "C" fn ja1_turn(w: *mut Ja1World, h: *const Ja1Host, slot: i32, d: i32, ticks: i32) -> i32 {
    run(w, h, slot, 0, |a| a.turn_to(d as u8, ticks))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::raw::c_void;

    fn rec(f: impl FnOnce(&mut [u8]), rows: &[u8]) -> (u16, u8) {
        let mut soldiers = vec![0u8; 48 * 0x200];
        f(&mut soldiers[3 * 0x200..4 * 0x200]);
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = soldiers.as_mut_ptr();
        w.anim_rows = rows.as_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<c_void>(), random: None, call: None, host_ops: 0 };
        ja1_anim_delay(&mut w, &h, 3);
        let r = &soldiers[3 * 0x200..4 * 0x200];
        (r[DELAY] as u16 | (r[DELAY + 1] as u16) << 8, r[FLAG_19F])
    }

    #[test]
    fn fixed_row_stops() {
        let mut rows = vec![0u8; 256 * 8];
        rows[8 * 5 + 2] = 7;
        assert_eq!(rec(|r| r[ANIM] = 5, &rows).0, 7);
    }

    #[test]
    fn standing_uses_hp_energy_and_floor() {
        let rows = vec![0u8; 256 * 8];
        // a = 4: (100-90)*10 + 100*10 = 1100.
        assert_eq!(rec(|r| { r[ANIM] = 4; r[HP] = 90; r[ENERGY] = 100; }, &rows).0, 1100);
        // hp 100, energy 0 -> 0 -> floor 50.
        assert_eq!(rec(|r| { r[ANIM] = 4; r[HP] = 100; r[ENERGY] = 0; }, &rows).0, 50);
    }

    #[test]
    fn computed_sum() {
        let rows = vec![0u8; 256 * 8];
        // a = 2 (base 50); hp 100, energy 100, agility 100 -> hurt 15, tired 0, agil 20; b1B3 = 6 -> extra 8.
        let (d, f) = rec(|r| { r[ANIM] = 2; r[HP] = 100; r[ENERGY] = 100; r[AGILITY] = 100; r[B1B3] = 6; }, &rows);
        assert_eq!((d, f), (15 + 20 + 50 + 8, 2));
        // slot >= 0x28, b1B8 set, a = 0x1A: flag 0, extra 40.
        let (d, f) = rec(|r| { r[ANIM] = 0x1A; r[HP] = 100; r[ENERGY] = 0; r[SLOT_BYTE] = 0x28; r[B1B8] = 1; }, &rows);
        assert_eq!((d, f), (15 + 20 + 50 + 40, 0));
    }

    #[test]
    fn a4_and_a39() {
        let rows = vec![0u8; 256 * 8];
        let (d, _) = rec(|r| { r[ANIM] = 0x16; r[A4] = 0x10; r[HP] = 100; r[ENERGY] = 100; r[AGILITY] = 100; }, &rows);
        assert_eq!(d, 15 + 20 + 16);
        // a = 0x39: h = 50 (hp 50), n = 0 -> 4*25 + 50.
        assert_eq!(rec(|r| { r[ANIM] = 0x39; r[HP] = 50; r[ENERGY] = 100; }, &rows).0, 150);
    }

    use std::cell::RefCell;
    thread_local! { static LOG: RefCell<Vec<(i32, Vec<i32>)>> = RefCell::new(vec![]); }
    unsafe extern "C" fn fcall(_c: *mut c_void, op: i32, a: *const i32, n: i32, _o: *mut u8, _s: i32) -> u32 {
        let v = std::slice::from_raw_parts(a, n as usize).to_vec();
        LOG.with(|l| l.borrow_mut().push((op, v)));
        if op == 7 { 5 } else { 1 }
    }
    fn step(f: impl FnOnce(&mut [u8]), rows: &[u8]) -> (Vec<(i32, Vec<i32>)>, Vec<u8>) {
        let mut soldiers = vec![0u8; 48 * 0x200];
        f(&mut soldiers[3 * 0x200..4 * 0x200]);
        let mut w: Ja1World = unsafe { std::mem::zeroed() };
        w.soldiers = soldiers.as_mut_ptr();
        w.anim_rows = rows.as_ptr();
        w.selected = 9;
        let h = Ja1Host { ctx: std::ptr::null_mut::<c_void>(), random: None, call: Some(fcall), host_ops: 1 << op::SET_ANIM | 1 << op::FIND_PATH | 1 << op::TILE_DIST };
        LOG.with(|l| l.borrow_mut().clear());
        ja1_anim_step(&mut w, &h, 3);
        (LOG.with(|l| l.borrow().clone()), soldiers[3 * 0x200..4 * 0x200].to_vec())
    }

    #[test]
    fn direction_table() {
        assert_eq!(direction(0, 0, 0, -5), 1);
        assert_eq!(direction(0, 0, 5, 0), 3);
        assert_eq!(direction(0, 0, -5, -5), 8);
        assert_eq!(direction(0, 0, 10, 2), 3);
        assert_eq!(direction(0, 0, 2, 10), 5);
        assert_eq!(direction(0, 0, 5, 5), 4);
    }

    #[test]
    fn idle_stands_and_moving_walks() {
        let rows = vec![0u8; 256 * 8];
        // Idle at the step point, a = 5: code stays 1000 -> no set_anim.
        let (log, _) = step(|r| { r[ANIM] = 5; r[HP] = 100; r[ENERGY] = 100; r[NEXT] = 0x41; r[NEXT + 1] = 0x0C; }, &rows);
        assert!(log.is_empty());
        // Idle, a = 6 -> code 4.
        let (log, _) = step(|r| { r[ANIM] = 6; r[NEXT] = 0x41; r[NEXT + 1] = 0x0C; }, &rows);
        assert_eq!(log, vec![(op::SET_ANIM, vec![3, 4, 1, 0])]);
        // Walking: row col 4 set for a = 2, next tile set, open path: leg starts, code 0xD, end_task 0.
        let mut rows = rows;
        rows[8 * 2 + 4] = 1;
        let (log, r) = step(|r| { r[ANIM] = 2; r[HP] = 100; r[ENERGY] = 100; r[NEXT] = 100; }, &rows);
        assert_eq!(log[0].0, op::TILE_DIST);
        assert_eq!(log[1].0, op::FIND_PATH);
        assert_eq!(log.last().unwrap(), &(op::SET_ANIM, vec![3, 0xD, 1, 0]));
        assert_eq!(r[B_1A2], 1); // paths missing -> 0 + 1
        assert_eq!(r[NEXT], 0x41);
    }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    /// Port rule (SPEC GAP): a path step past the 48 x 80 table or a script word past 68 x 0x50 bytes reads 0.
    #[test]
    fn guard_path_dir_and_script_word() {
        let mut paths = vec![0u8; 48 * 80 + 4];
        paths[48 * 80 - 1] = 3;
        paths[48 * 80] = 5;
        let mut scripts = vec![0u8; 68 * 0x50 + 4];
        scripts[68 * 0x50 - 2] = 1;
        scripts[68 * 0x50 - 1] = 2;
        scripts[68 * 0x50] = 4;
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.paths = paths.as_mut_ptr();
        w.anim_scripts = scripts.as_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        assert_eq!(ai.path_dir_at(48 * 80 - 1), 3);
        assert_eq!(ai.path_dir_at(48 * 80), 0);
        assert_eq!(ai.script_word(68 * 0x50 - 2), 0x0201);
        assert_eq!(ai.script_word(68 * 0x50 - 1), 0);
    }

    /// Ledger row ja1logic:5967: OccupyTile 0x23974 refuses only tiles >= 0xC41 (asm 0x23975 `cmp eax,0xc41; jl`).
    /// Tile 0xC40 reads the byte past the 56 x 56 map (0x106A1D, another global: 0x40 in all 136 build/mem_*.bin
    /// dumps), so slot 0 takes it (returns 1). The port reads past-map tile bytes as 0 (ai.rs tile_byte SPEC GAP), which
    /// also gives 1 for slot 0; other slots differ (the original takes the tile, the port refuses) and stay untested.
    #[test]
    fn occupy_tile_c40_is_inside_the_bound() {
        let mut tiles = vec![0u8; 3136 * 8];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.tiles = tiles.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        assert!(ai.occupy_for(0xC40, 0));
        assert!(!ai.occupy_for(0xC41, 0));
    }
}
