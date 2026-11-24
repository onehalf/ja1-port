//! First aid (docs/spec/combat.md "First aid, exact steps"; C interface in docs/spec/port_api.md): `ja1_first_aid`
//! (library op 191 first_aid) and `ja1_aid_tick` (library op 151 aid_tick). A child module of `ai`; the texts,
//! portraits, panel, marker and animation calls are host ops through `Ai`.
use super::{op, run, Ai, Ja1Host, Ja1World, NO_SLOT};

// texts of host op 57 merc_text
const T_ABANDONED: i32 = 5;
const T_BANDAGED: i32 = 6;
const T_OUT: i32 = 7;
const T_BUSY: i32 = 8;
const T_NO_NEED: i32 = 9;

impl Ai {
    /// Charges: the sum of the unit conditions of item entry 0.
    fn fa_charges(&self) -> i32 {
        let n = self.s8(0x01);
        (0..n.max(0)).map(|k| self.s8(if k == 0 { 0x02 } else { 0x07 + k as usize })).sum()
    }

    /// Use charges (n) on item entry 0.
    fn fa_use_charges(&self, mut n: i32) {
        let mut k = self.s8(0x01) - 1;
        while k >= 0 && n > 0 {
            let o = if k == 0 { 0x02 } else { 0x07 + k as usize };
            let c = self.s8(o);
            if c > n {
                self.wr8(o, (self.rd8(o) as i32 - n) as u8);
                n = 0;
            } else if k == 0 {
                n -= c;
                for i in 0..12 {
                    self.wr8(i, 0);
                }
            } else {
                self.wr8(o, 0);
                n -= c;
                self.wr8(0x01, self.rd8(0x01).wrapping_sub(1));
            }
            k -= 1;
        }
    }

    /// Apply first aid: self = the medic M, p = the patient P; the result is the charges used.
    fn fa_apply(&self, p: &Ai, sup: i32, cap: i32) -> i32 {
        // 1.
        if p.s8(0x185) < 1 && p.s8(0x183) >= 15 {
            return 0;
        }
        // 2.
        let ap = self.s8(0x1A5);
        let inner = self.s8(0x18C) + 2 * cap + 3 * self.s8(0x18E) + 10 * self.s8(0x192);
        let full = self.cdiv(ap.wrapping_mul(self.cdiv(inner, 7)), 50);
        if full == 0 {
            return 0;
        }
        // 3.
        let kit = self.rd8(0x00) == 0x4A;
        let base = if kit { full + self.cdiv(full, 2) } else { full };
        let mut pts = base;
        // 4.
        let short = if p.s8(0x183) < 15 { (15 - p.rd8(0x183) as i32) & 0xFF } else { 0 };
        let mut need = 2 * short;
        if p.s8(0x185) - short > 0 {
            need += p.s8(0x185) - short;
        }
        if need < pts {
            pts = need;
        }
        // 5.
        let used;
        if kit {
            let mut u = self.cdiv(pts, 2);
            if u == 0 && pts > 0 {
                u = 1;
            }
            if u > sup {
                u = sup;
                pts = 2 * sup;
            }
            used = u;
        } else {
            let mut u = pts;
            if u > sup {
                u = sup;
                pts = sup;
            }
            used = u;
        }
        // 6.
        let mut left = pts & 0xFF;
        if left != 0 && p.s8(0x183) < 15 {
            if left >= 2 * short {
                p.wr8(0x183, 15);
                p.wr8(0x185, (p.rd8(0x185) as i32 - short) as u8);
                left = (left - 2 * short) & 0xFF;
            } else {
                let half = left / 2;
                p.wr8(0x183, (p.s8(0x183) + half) as u8);
                p.wr8(0x185, (p.s8(0x185) - half) as u8);
                left %= 2;
            }
            if p.s8(0x185) < 0 {
                p.wr8(0x185, 0);
            }
            if p.s8(0x183) >= 15 {
                p.wr8(0x187, 0);
                p.wr8(0x186, 0);
            }
            self.call(op::PORTRAIT_REFRESH, &[p.rd8(0x17A) as i32, 1]);
        }
        // 7.
        if left != 0 && p.rd8(0x185) != 0 && p.rd8(0x186) == 0 {
            if p.s8(0x185) <= left {
                left = (left - p.rd8(0x185) as i32) & 0xFF;
                p.wr8(0x185, 0);
            } else {
                p.wr8(0x185, (p.rd8(0x185) as i32 - left) as u8);
                left = 0;
            }
            self.call(op::WOUND_BAR, &[p.slot(), 1]);
        }
        // 8.
        pts -= left;
        let mut spent = self.cdiv(pts.wrapping_mul(ap), base);
        if kit {
            spent = self.cdiv(spent.wrapping_mul(2), 3);
        }
        // 9.
        let slot = self.slot();
        self.call(op::SPEND_AP, &[slot, spent as i8 as i32, spent.wrapping_mul(-50) as i16 as i32]);
        // 10.
        if pts != 0 {
            self.call(op::STAT_PRACTICE, &[slot, 5, self.cdiv(pts, 3) as i16 as i32, 0]);
        }
        let h = self.cdiv(pts, 2);
        if h != 0 {
            self.call(op::STAT_PRACTICE, &[slot, 3, self.cdiv(h, 3) as i16 as i32, 0]);
        }
        used
    }

    /// First aid tick (library op 151).
    pub(super) fn aid_tick(&self) {
        let slot = self.slot();
        // 1.
        if self.rd8(0x1B9) < 5 || self.cur_anim() != 0xF {
            return;
        }
        // 2.
        let o = self.occupant(self.rd16(0xD6));
        let mut text = T_ABANDONED;
        // 3.
        if o != NO_SLOT {
            if let Some(p) = Ai::new(self.w, self.h, o as i32) {
                if p.rd8(0x188) == 0 && p.rd8(0x183) != 0 && matches!(p.rd8(0x1B9), 4 | 6) {
                    // a.
                    if p.rd8(0x185) == 0 && p.s8(0x183) >= 15 {
                        self.call(op::END_TASK, &[slot, 1]);
                        return;
                    }
                    // b.
                    let c = self.fa_charges();
                    let used = self.fa_apply(&p, c, c.min(100));
                    // c.
                    if p.rd8(0x185) == 0 && p.s8(0x183) >= 15 {
                        self.call(op::MERC_TEXT, &[o as i32, T_BANDAGED, 1000]);
                        self.call(op::END_TASK, &[o as i32, 1]);
                    }
                    // d.
                    self.fa_use_charges(used);
                    let c = self.fa_charges();
                    // e.
                    if !self.w_combat() && self.rd8(0x17A) == unsafe { (*self.w).selected } {
                        self.call(op::ITEM_PANEL, &[slot, 0, 1]);
                    }
                    // f.
                    if c > 0 {
                        return;
                    }
                    text = T_OUT;
                }
            }
        }
        // 4.
        self.call(op::MERC_TEXT, &[slot, text, 1000]);
        self.call(op::END_TASK, &[slot, 1]);
    }

    /// First aid start (library op 191): 1 when first aid starts.
    pub(super) fn first_aid_start(&self, tile: i32, task: u8) -> u32 {
        let slot = self.slot();
        // 1.
        let mine = self.rd8(0x1B9);
        if mine != 0 && !(mine == 4 && task == 6) {
            self.call(op::MERC_TEXT, &[slot, T_BUSY, 2000]);
            return 0;
        }
        // 2.
        if !(5..=6).contains(&task) {
            return 0;
        }
        // 3.
        let o = self.occupant(tile as u16);
        if o >= 0x40 {
            self.call(op::MESSAGE, &[2]);
            return 0;
        }
        let p = match Ai::new(self.w, self.h, o as i32) {
            Some(p) => p,
            None => return 0,
        };
        // 4.
        if p.rd8(0x185) == 0 && p.rd8(0x186) != 1 {
            self.call(op::MERC_TEXT, &[o as i32, T_NO_NEED, 1000]);
            return 0;
        }
        // 5.
        self.u_forget_attack(false);
        self.call(op::SPEND_AP, &[slot, 5, 0]);
        self.wr8(0x1B9, task);
        self.wr16(0xD6, tile as u16);
        self.call(op::SET_ANIM, &[slot, 0xE, 0, 0]);
        p.wr8(0x1CA, p.rd8(0x1CA).wrapping_add(1));
        let point = |r: &Ai| [r.rd16(0x116) as i16 as i32, r.rd16(0x118) as i16 as i32];
        if task == 5 {
            // 6.
            self.wr8(0x181, 0xB8);
            let (m, q) = (point(self), point(&p));
            self.wr8(0x1A2, self.call(op::DIRECTION, &[m[0], m[1], q[0], q[1]]) as u8);
            if p.rd8(0x1B9) != 6 {
                p.wr8(0x1B9, 4);
                p.wr8(0x181, 0xB9);
                let cur = p.cur_anim();
                if p.s8(0x183) >= 15 && cur >= 4 && p.anim_col(cur, 2) == 0 {
                    p.u_forget_attack(false);
                    self.call(op::SET_ANIM, &[o as i32, 4, 0, 0]);
                    let (q, m) = (point(&p), point(self));
                    p.wr8(0x1A2, self.call(op::DIRECTION, &[q[0], q[1], m[0], m[1]]) as u8);
                }
                self.call(op::PORTRAIT, &[o as i32, 1]);
            }
        } else {
            // 7.
            self.wr8(0x181, 0xB7);
            self.wr8(0x1A2, 5);
        }
        // 8.
        self.call(op::PORTRAIT, &[slot, 1]);
        self.call(op::TILE_MARKER, &[0x93, tile]);
        1
    }
}

impl Ai {
    /// End of a task (library op 50 end_task; combat.md "End of a task, exact steps").
    pub(super) fn end_task(&self, flag: i32) {
        let slot = self.slot();
        // 1.
        if self.rd8(0x1B9) != 0 {
            self.wr8(0x1C1, 1);
        }
        // 2.
        if matches!(self.rd8(0x1B9), 5 | 6) {
            let o = self.occupant(self.rd16(0xD6));
            if o < NO_SLOT {
                if let Some(p) = Ai::new(self.w, self.h, o as i32) {
                    let c = p.rd8(0x1CA).wrapping_sub(1);
                    p.wr8(0x1CA, c);
                    if c as i8 <= 0 {
                        p.wr8(0x1CA, 0);
                        p.wr8(0x1B9, 0);
                        p.wr8(0x181, 0);
                        self.call(op::PORTRAIT, &[p.rd8(0x17A) as i32, 1]);
                    }
                }
            }
            self.wr8(0x1B9, if self.s8(0x1CA) > 0 { 4 } else { 0 });
            self.wr16(0xD6, 0x0C41);
            self.wr8(0x1CA, 0);
            self.wr8(0x181, 0);
            if flag != 0 {
                self.u_tile_point_stop(0);
                self.call(op::SET_ANIM, &[slot, 0x10, 0, 0]);
            }
            self.call(op::PORTRAIT, &[self.rd8(0x17A) as i32, 1]);
        }
        // 3.
        if self.rd8(0x1B9) == 4 {
            for k in 0..8 {
                if let Some(p) = Ai::new(self.w, self.h, k) {
                    if p.rd8(0x182) != 0
                        && p.rd8(0x138) != 0
                        && p.rd8(0x183) != 0
                        && p.rd8(0x1B9) == 5
                        && p.rd16(0xD6) == self.rd16(0x114)
                    {
                        p.end_task(1);
                    }
                }
            }
            if self.rd8(0x1B9) == 4 {
                self.wr8(0x181, 0);
                self.wr8(0x1B9, 0);
                self.wr8(0x1CA, 0);
                self.call(op::PORTRAIT, &[self.rd8(0x17A) as i32, 1]);
                if flag != 0 {
                    self.u_tile_point_stop(0);
                }
            }
            if self.rd8(0x17A) < 8 {
                self.call(op::RESERVE_AP, &[slot]);
            }
        }
    }

    /// Walking cost (library op 13 move_ap; combat.md "Walking cost").
    pub(super) fn move_ap_cost(&self) -> i32 {
        let me = self.s16_at(0x114);
        let mut best = 99;
        for d in 1..=8 {
            let n = super::search::step(me, self.dir_step(d));
            if n != me {
                best = best.min(crate::step_ap(self.u_move_code(n), 0, 0, 0) as i8 as i32);
            }
        }
        best
    }
}

#[no_mangle]
pub extern "C" fn ja1_end_task(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    run(w, h, slot, (), |a| a.end_task(flag))
}
#[no_mangle]
pub extern "C" fn ja1_move_ap(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.move_ap_cost())
}
#[no_mangle]
pub extern "C" fn ja1_aid_tick(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.aid_tick())
}
#[no_mangle]
pub extern "C" fn ja1_first_aid(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, task: i32, b: i32) -> i32 {
    let _ = b;
    run(w, h, slot, 0, |a| a.first_aid_start(tile, task as u8) as i32)
}
