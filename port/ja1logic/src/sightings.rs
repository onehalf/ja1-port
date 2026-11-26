//! Sightings as steps (docs/spec/ai.md "Sightings as steps"): Sight check (library op 115), No ally sees,
//! Perceive (op 35), Share sightings (op 34), Enemy turned (op 63), Guard turned (op 64) and Clear rooms (op 327).
//! A child module of `ai`: every op goes through `Ai::call` (the host_ops rule). World data is read again after
//! every host call.
use super::damage::{get16, get8, put16, put8, KNOWLEDGE_LEN, LAST_SEEN_E_LEN, LAST_SEEN_M_LEN, REMEMBER_E_LEN, REMEMBER_M_LEN};
use super::search::{px, py, s16, world_dist};
use super::{op, run, Ai, Ja1Host, Ja1World, NO_SLOT};

const CALLS_LEN: usize = 8;

/// Sight range table (JA.EXE 0x92FC3), index 8 × facing + d.
const RANGE: [u8; 73] = [
    0x00, 0x37, 0x00, 0xFF, 0xFF, 0xC7, 0xFF, 0xC7, 0xFF, //
    14, 12, 8, 3, 0, 3, 8, 12, //
    12, 14, 12, 8, 3, 0, 3, 8, //
    8, 12, 14, 12, 8, 3, 0, 3, //
    3, 8, 12, 14, 12, 8, 3, 0, //
    0, 3, 8, 12, 14, 12, 8, 3, //
    3, 0, 3, 8, 12, 14, 12, 8, //
    8, 3, 0, 3, 8, 12, 14, 12, //
    12, 8, 3, 0, 3, 8, 12, 14,
];

impl Ai {
    fn sg_in_play(&self, r: usize) -> bool {
        self.rd8_of(r, 0x182) != 0 && self.rd8_of(r, 0x138) != 0 && self.rd8_of(r, 0x183) != 0
    }
    /// The opponent test of Sight check / Mark seen / Guard turned. F17 (fan fix, #70 row 9; changelog 1.15z, found
    /// by DarzaR): an opponent at 0 health that is still down (+0x186 > 0, not yet dead = 0xFF) also counts (1.15m
    /// SightCheck 0x72FCD, MarkSeen 0x73360, 0x7351C).
    pub(super) fn sg_opp_in_play(&self, r: usize) -> bool {
        self.rd8_of(r, 0x182) != 0
            && self.rd8_of(r, 0x138) != 0
            && (self.rd8_of(r, 0x183) != 0 || (crate::fanfix::on(17) && self.rd8_of(r, 0x186) as i8 > 0))
    }
    fn sg_wr8_of(&self, r: usize, off: usize, v: u8) {
        if let Some(o) = Ai::new(self.w, self.h, r as i32) {
            o.wr8(off, v);
        }
    }
    fn sg_knowledge_put(&self, i: usize, v: u8) {
        put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, i as i64, v);
    }
    /// Opponents of this soldier: (record i, flag index j).
    fn sg_opponents(&self) -> Vec<(usize, usize)> {
        if self.rd8(0x17A) >= 0x28 {
            (0..0x28).map(|i| (i, i)).collect()
        } else {
            (0x28..0x30).map(|i| (i, i - 0x28)).collect()
        }
    }

    // ---- Sight check (op 115) ----
    pub(super) fn sight_check(&self, i: usize) -> u8 {
        let slot = self.rd8(0x17A) as usize;
        let b = self.rd8_of(i, 0x17A) as usize;
        // 1.
        if slot >= 0x30 || b >= 0x30 {
            return 0;
        }
        if !(self.sg_in_play(self.s) && self.sg_opp_in_play(i) && (slot < 0x28) != (b < 0x28) && self.s8(0xBA) != 2) {
            return 0;
        }
        let anim = 0x1E8 + 2 * self.rd8(0x1F3) as usize;
        if self.rd16(anim) < 4 {
            return 0;
        }
        if self.s8(0xBA) == 4 && self.s8(0x9C) <= 0 {
            return 0;
        }
        // 2.
        let j = if i < 0x28 { i } else { i - 0x28 };
        let old = self.s8(0x13A + j);
        let mut seen = false;
        if self.s8(0xBA) == 4 {
            // 3.
            if self.rd8_of(i, 0x13A) as i8 == 1 {
                self.call(op::CLEAR_ROOMS, &[]);
                seen = true;
            }
        } else {
            // 4.
            let g = |r: usize, off: usize| s16(self.rd16_of(r, off) as i32);
            let r = if old != 0 {
                14
            } else {
                let d = self.call(op::DIRECTION, &[g(self.s, 0x116), g(self.s, 0x118), g(i, 0x116), g(i, 0x118)]) & 0xFF;
                let k = 8 * self.s8(0x1A1) + d as i32;
                if (0..73).contains(&k) { RANGE[k as usize] as i32 } else { 0 }
            };
            if (self.call(op::TILE_DIST, &[g(self.s, 0x114), g(i, 0x114)]) & 0xFF) as i32 <= r {
                let args = [
                    self.slot(),
                    g(self.s, 0x116) + 2,
                    g(self.s, 0x118) + 4,
                    g(i, 0x114),
                    g(i, 0x116) + 2,
                    g(i, 0x118) + 4,
                    old,
                    r,
                    i as i32,
                ];
                seen = self.call(op::SIGHT_LINE, &args) != 0;
            }
        }
        if seen {
            // 5.
            self.mark_seen(i);
            if old == 1 || old == 2 {
                if slot < 0x28 {
                    self.sg_knowledge_put(i, 1);
                }
                return 0;
            }
            return 1;
        }
        // 6.
        if old == 1 {
            let c = self.rd8(0x1D5).wrapping_sub(1);
            self.wr8(0x1D5, c);
            if c == 0 {
                self.wr8(0xBB, 2);
            }
            self.wr8(0x13A + j, 2);
            let (p, len, k) = if i < 0x28 {
                (unsafe { (*self.w).remember_e }, REMEMBER_E_LEN, (slot as i64 - 0x28) * 0x28 + i as i64)
            } else {
                (unsafe { (*self.w).remember_m }, REMEMBER_M_LEN, slot as i64 * 8 + j as i64)
            };
            if get8(p, len, k) == 0xFF {
                put8(p, len, k, 1);
            }
            if self.knowledge(i) == 1 && self.no_ally_sees(i) {
                self.sg_knowledge_put(i, 2);
            }
        }
        0
    }

    // ---- No ally sees ----
    fn no_ally_sees(&self, i: usize) -> bool {
        let b = self.rd8_of(i, 0x17A) as usize;
        let (first, end, idx) = if self.s8(0xBA) < 3 { (0, 0x10, b.wrapping_sub(0x28) & 0xFF) } else { (0x28, 0x30, b) };
        for k in first..end {
            if k != self.rd8(0x17A) as usize
                && self.sg_in_play(k)
                && self.rd8_of(k, 0x183) as i8 >= 15
                && self.rd8_of(k, 0x13A + idx) == 1
            {
                return false;
            }
        }
        true
    }

    // ---- Perceive (op 35) ----
    pub(super) fn perceive(&self, age: u8) {
        // 1.
        if self.s8(0xBA) == 2 {
            return;
        }
        let mut new: u8 = 0;
        // 2.
        for (i, j) in self.sg_opponents() {
            if !self.sg_in_play(i) || self.rd8(0x13A + j) == 0 {
                continue;
            }
            new = new.wrapping_add(self.call(op::SIGHT_CHECK, &[self.slot(), i as i32]) as u8);
            if age != 0 {
                let mut v = self.s8(0x13A + j);
                if v >= 2 {
                    v += 1;
                    if v > 4 {
                        v = 0;
                    }
                } else if v <= -1 {
                    v -= 1;
                    if v < -3 {
                        v = 0;
                    }
                }
                self.wr8(0x13A + j, v as u8);
            }
        }
        // 3.
        if new != 0 && self.rd8(0x17A) < 0x28 {
            self.call(op::SHARE_SIGHTINGS, &[self.slot(), 0]);
        }
    }

    // ---- Share sightings (op 34) ----
    pub(super) fn share_sightings(&self, flag: u8) {
        let slot = self.rd8(0x17A) as usize;
        // 1.
        if slot >= 0x28 {
            unsafe { (*self.w).last_spotter = slot as u8 };
        }
        let (mut best, mut near, mut seen, mut heard, mut unseen) = (1500i32, NO_SLOT as usize, 0u8, 0u8, true);
        // 2.
        for (i, j) in self.sg_opponents() {
            let f = self.s8(0x13A + j);
            if !self.sg_in_play(i) || f == 0 {
                continue;
            }
            let known = self.knowledge(i);
            if self.fresher(known, f) == 0 && f != known {
                continue;
            }
            let dist = || {
                let t = s16(self.rd16_of(i, 0x114) as i32);
                world_dist(s16(self.rd16(0x116) as i32), s16(self.rd16(0x118) as i32), px(t), py(t))
            };
            // a.
            if slot < 0x10 && self.rd8_of(i, 0x183) as i8 >= 15 && f == 1 {
                if known == 0 {
                    seen = seen.wrapping_add(1);
                    let d = dist();
                    if seen == 1 || s16(d) < s16(best) {
                        best = d;
                        near = i;
                    }
                    if slot < 8 {
                        if self.rd8_of(i, 0x13A + slot) as i8 == 1 {
                            unseen = false;
                        } else {
                            self.call(op::STAT_PRACTICE, &[self.slot(), 9, 2, 0]);
                        }
                    }
                } else if known != 1 && known != 2 {
                    heard = heard.wrapping_add(1);
                    if seen == 0 {
                        let d = dist();
                        if heard == 1 || s16(d) < s16(best) {
                            best = d;
                            near = i;
                        }
                    }
                }
            }
            // b.
            self.sg_knowledge_put(i, self.rd8(0x13A + j));
            let b = self.rd8_of(i, 0x17A) as i64;
            let v = if slot < 0x28 {
                get16(unsafe { (*self.w).last_seen_m }, LAST_SEEN_M_LEN, 2 * (slot as i64 * 8 - 40 + b))
            } else {
                get16(unsafe { (*self.w).last_seen_e }, LAST_SEEN_E_LEN, 2 * ((slot as i64 - 0x28) * 40 + b))
            };
            self.sg_wr8_of(i, 0x108, v as u8);
            self.sg_wr8_of(i, 0x109, (v >> 8) as u8);
        }
        // 3.
        if heard != 0 || seen != 0 {
            if slot < 8 {
                let how = if seen == 0 {
                    0xFF
                } else if unseen {
                    0
                } else if flag != 0 {
                    2
                } else {
                    0xFF
                };
                // MercSees? 0x56D28 (AL, DL, BL -> 0x229E00/01/02): the merc's reaction to the new sighting.
                self.call(op::MERC_SEES, &[slot as i32, how, near as i32]);
                if unsafe { (*self.w).screen_drawn } != 0 && slot as u8 == unsafe { (*self.w).selected } {
                    self.call(op::STOP_WALK, &[self.slot(), 0]);
                    self.call(op::STOP_ANIM, &[self.slot(), 0]);
                    self.wr8(0x197, 1);
                }
            } else if seen != 0 {
                self.call(op::FOCUS, &[slot as i32, 0]);
                if self.rd8_of(near, 0x9C) as i8 > 0 || unsafe { (*self.w).follow_all } != 0 {
                    unsafe {
                        (*self.w).spotted = near as i32;
                        (*self.w).focus_last = near as u8;
                        (*self.w).focus_slot = near as u8;
                    }
                    self.call(op::FOCUS, &[near as i32, 0]);
                }
                self.call(op::GUARD_SEES_MSG, &[]);
            }
        }
        // 4.
        let p = unsafe { (*self.w).calls };
        let base = if slot < 0x28 { 0 } else { 4 };
        if get8(p, CALLS_LEN, base + 2) < self.rd8(0x1C0) {
            put16(p, CALLS_LEN, base, self.rd16(0x122));
            put8(p, CALLS_LEN, base + 2, self.rd8(0x1C0));
        }
    }

    // ---- Enemy turned (op 63) / Guard turned (op 64) ----
    pub(super) fn enemy_turned(&self) {
        for i in 0..0x28usize {
            if self.sg_in_play(i) {
                self.call(op::SIGHT_CHECK, &[self.slot(), i as i32]);
            }
        }
    }
    pub(super) fn guard_turned(&self) {
        for i in 0x28..0x30usize {
            if self.sg_opp_in_play(i) {
                self.call(op::SIGHT_CHECK, &[self.slot(), i as i32]);
            }
        }
        if unsafe { (*self.w).sees_slot } == self.rd8(0x17A) {
            self.call(op::SEES_MSG, &[self.slot()]);
        }
    }
}

// ---- Clear rooms (op 327) ----
pub(super) fn clear_rooms(w: *mut Ja1World) {
    let p = unsafe { (*w).tiles };
    if p.is_null() {
        return;
    }
    clear_rooms_at(p);
}

/// ClearRoomFlags 0x61744: tile byte 3 of two rectangles of the Santino room (tiles `base + 56 x row + col`).
pub(super) fn clear_rooms_at(p: *mut u8) {
    for (start, rows, cols) in [(232usize, 7usize, 6usize), (625, 3, 5)] {
        for r in 0..rows {
            for c in 0..cols {
                unsafe { p.add((start + 56 * r + c) * 8 + 3).write(0) };
            }
        }
    }
}

// ---- entry points ----
/// ClearRoomFlags 0x61744 on a raw tile array (3136 x 8 bytes); the AI op 327 runs it on the world.
#[no_mangle]
pub extern "C" fn ja1f_clear_rooms(tiles: *mut u8) {
    clear_rooms_at(tiles)
}
// SPEC GAP: an opponent index outside 0..47 reads its record as zeros and is never written.
#[no_mangle]
pub extern "C" fn ja1_sight_check(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32) -> i32 {
    run(w, h, slot, 0, |a| a.sight_check((opponent as u32).min(0xFFFF) as usize) as i32)
}
#[no_mangle]
pub extern "C" fn ja1_perceive(w: *mut Ja1World, h: *const Ja1Host, slot: i32, age: i32) {
    run(w, h, slot, (), |a| a.perceive(age as u8))
}
#[no_mangle]
pub extern "C" fn ja1_share_sightings(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    run(w, h, slot, (), |a| a.share_sightings(flag as u8))
}
#[no_mangle]
pub extern "C" fn ja1_enemy_turned(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.enemy_turned())
}
#[no_mangle]
pub extern "C" fn ja1_guard_turned(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.guard_turned())
}
