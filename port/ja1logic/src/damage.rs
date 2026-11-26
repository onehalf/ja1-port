//! Shooting batches 2 and 3 (docs/spec/shooting.md; C interface in port_api.md): Bullet damage (library op 175), Hit
//! location (op 180), Shot alert (op 173), Interrupt check (op 182), Take damage (library op 110),
//! Hit reaction, Hit alert, Mark seen and Orders bump (BumpOrders, was BumpTrait1). A child module of `ai`: every op goes through `Ai::call` (the
//! host_ops rule); Hit reaction, Hit alert and Mark seen are plain procedures here, never host calls. World data
//! is read again after every host call.
use super::fire::vt;
use super::search::{px, py, s16};
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT};

pub(super) const DMG_TEXT_LEN: usize = 240;
pub(super) const KNOWLEDGE_LEN: usize = 64;
pub(super) const LAST_SEEN_M_LEN: usize = 16 * 8 * 2;
pub(super) const LAST_SEEN_E_LEN: usize = 8 * 40 * 2;
pub(super) const REMEMBER_M_LEN: usize = 16 * 8;
pub(super) const REMEMBER_E_LEN: usize = 8 * 40;
const SFX_CHANNEL: i32 = 0x2B11;
const ITEM_TYPES_LEN: usize = 136 * 12;
const WEAPONS_LEN: usize = 256 * 6;
const ABSORB_LEN: usize = 288 * 2;

// SPEC GAP: an index outside a world table (or a null table) reads 0 and is not written.
pub(super) fn get8(p: *const u8, len: usize, i: i64) -> u8 {
    if p.is_null() || i < 0 || i as u64 >= len as u64 {
        return 0;
    }
    unsafe { p.add(i as usize).read() }
}
pub(super) fn put8(p: *mut u8, len: usize, i: i64, v: u8) {
    if p.is_null() || i < 0 || i as u64 >= len as u64 {
        return;
    }
    unsafe { p.add(i as usize).write(v) }
}
/// u16 at byte offset i.
pub(super) fn get16(p: *const u8, len: usize, i: i64) -> u16 {
    get8(p, len, i) as u16 | (get8(p, len, i + 1) as u16) << 8
}
pub(super) fn put16(p: *mut u8, len: usize, i: i64, v: u16) {
    put8(p, len, i, v as u8);
    put8(p, len, i + 1, (v >> 8) as u8);
}

/// The integer part of the square root of d (d ≥ 0), else −0x80000000.
fn isqrt(d: i32) -> i32 {
    if d < 0 {
        return i32::MIN;
    }
    let mut q = (d as f64).sqrt() as i64;
    while q * q > d as i64 {
        q -= 1;
    }
    while (q + 1) * (q + 1) <= d as i64 {
        q += 1;
    }
    q as i32
}

impl Ai {
    fn d_anim(&self, code: i32, keep: i32) {
        self.call(op::SET_ANIM, &[self.slot(), code, keep, 1]);
    }

    // ---- Take damage (op 110) ----
    pub(super) fn take_damage(&self, height: i32, damage: i32, k: u8, a: u8, tile: u16) {
        let mut k = k;
        let slot = self.slot();
        let mut d = damage;
        let cur = self.cur_anim();
        // 1.
        let mut extra: u8 = 0;
        let pp = (unsafe { (*self.w).path_preview } != 0) as i32;
        self.call(op::UI_REFRESH, &[pp]);
        // 2.
        if self.s8(0xBA) == 4 {
            return;
        }
        // 3.
        let prev = self.s8(0x184) - self.s8(0x183) - self.s8(0x185);
        self.wr8(0x1BE, 0);
        if unsafe { (*self.w).marked_slot } as i32 == slot {
            unsafe { (*self.w).marked_slot = NO_SLOT };
        }
        // 4.
        if self.rd8(0x1E1) == 0 {
            match cur {
                0x1B => self.wr8(0x1E1, 0x1A),
                0x1F => self.wr8(0x1E1, 0x1E),
                0x23 => self.wr8(0x1E1, 0x22),
                5 => self.wr8(0x1E1, 6),
                0x0E | 0x0F => {
                    let t = s16(self.rd16(0x114) as i32);
                    let (x, y) = (px(t) as u16, py(t) as u16);
                    self.wr16(0x126, x);
                    self.wr16(0x116, x);
                    self.wr16(0x128, y);
                    self.wr16(0x118, y);
                    self.wr8(0x1E1, 0x0E);
                }
                _ => {}
            }
            if self.rd8(0x1E1) != 0 {
                let q = self.rd8(0x1E1);
                let c = self.row_byte(q as i32, 0);
                if c != 0x63 {
                    self.wr8(0x1E3, self.rd8(0x1E3).wrapping_add(c));
                }
            }
        }
        // 5.
        if k == 1 || k == 4 {
            let v = 10000 - s16(self.rd16(0x10C) as i32);
            let r = 50i32.wrapping_mul(d).wrapping_mul(v).wrapping_div(10000);
            self.rest(s16(r).wrapping_neg(), 1);
            if height != 0x1356 {
                d = self.call(op::HIT_LOCATION, &[slot, height, d]) as i32;
            }
        }
        // 6.
        self.wr8(0x195, 1);
        // 7.
        let dt = unsafe { (*self.w).dmg_text };
        let base = 5 * slot as i64;
        if d != 0 {
            // a.
            let txt = d.wrapping_neg().to_string();
            for (n, c) in txt.bytes().chain(std::iter::once(0)).enumerate() {
                put8(dt, DMG_TEXT_LEN, base + n as i64, c);
            }
            // b.
            self.wr8(0x1D2, 0x28);
            self.wr32(0xC4, 10);
            self.wr16(0xD8, 0xFFFC);
            self.wr16(0xDA, 2);
            // c.
            if self.s8(0x183) < d {
                d = self.s8(0x183);
            }
            self.wr8(0x183, self.rd8(0x183).wrapping_sub(d as u8));
            if slot >= 0x28 && self.s8(0x183) < 15 && self.rd8(0x183) != 0 {
                self.wr8(0x183, 0);
            }
            // d.
            if slot < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 9, s16(d), 0]);
            }
            // e.
            let hp = self.rd8(0x183) as i32;
            self.wr8(0x185, (self.rd8(0x184) as i32).wrapping_sub(hp + prev) as u8);
            let q = self.cdiv(self.s8(0x183), self.s8(0x1F2) + 10) + 1;
            self.wr16(0x1F0, q as u16);
            self.wr8(0x189, 7);
            self.call(op::PORTRAIT, &[slot, 1]);
            // f.
            if self.s8(0x183) < 15 {
                self.wr16(0xD4, 0);
                self.u_forget_attack(false);
                if self.s8(0x186) >= 0 {
                    self.wr8(0x186, 1);
                }
                self.wr8(0x181, 0);
                self.wr8(0x1AC, (self.s8(0x183) < 10) as u8);
                if unsafe { (*self.w).selected } as i32 == slot {
                    self.call(op::CURSOR_RESET, &[]);
                }
            }
            // g.
            self.call(op::WOUND_BAR, &[slot, 1]);
        } else {
            put8(dt, DMG_TEXT_LEN, base, 0);
        }
        // 8.
        self.wr8(0x1C1, 1);
        self.wr16(0xDC, NONE);
        if self.rd8(0x19C) != 3 {
            self.wr16(0xD0, NONE);
        }
        // 9.
        let mut hk = 2;
        let sc = cur as i16 as i32;
        if sc > 0x31 {
            if k == 2 {
                self.wr8(0x1E1, 0);
                self.wr8(0x195, 0);
                self.d_anim(0x3C, 0);
                hk = 3;
            } else if sc <= 0x35 {
                self.d_anim(0x35, 1);
            } else if sc <= 0x38 {
                self.d_anim(0x38, 1);
            } else if sc < 0x40 {
                self.d_anim(0x3B, 1);
            }
        } else if matches!(sc, 0 | 2 | 0x29 | 0x2A | 0x2F) {
            self.wr8(0x1E1, 0);
            if k == 1 {
                self.d_anim(0x2F, 0);
            } else {
                self.d_anim(0x29, 0);
                hk = 3;
            }
        } else if sc == 0x2B {
            hk = 3;
            self.wr8(0x1E1, 0);
        } else if matches!(sc, 1 | 3 | 0x0D) {
            self.wr8(0x1E1, 0);
            if k == 1 {
                self.d_anim(0x30, 0);
            } else {
                self.d_anim(3, 0);
                hk = 3;
            }
        } else {
            if k == 1 {
                // a.
                if unsafe { (*self.w).knockdown } != 0 && self.s8(0x183) >= 15 {
                    // Fan fix F11 (#70 row 3; changelog 1.15m, found by MitchShudlem, fixed by DarzaR; 1.15m TakeDamage
                    // 0x3996B): ReplaceSoldier first, so a crouching merc knocked down by a leg hit falls at once.
                    if crate::fanfix::on(11) {
                        self.call(op::REPLACE, &[slot]);
                    }
                    self.call(op::KNOCKDOWN_ANIM, &[slot, 0x28, 0x2B, 4]);
                    extra = 4;
                    if !crate::fanfix::on(10) {
                        unsafe { (*self.w).knockdown = 0 };
                    }
                } else {
                    if self.s8(0x183) < 15 && a < 0x40 {
                        let ai = a as usize;
                        let g = |o: usize| s16(self.rd16_of(ai, o) as i32);
                        let args = [s16(self.rd16(0x116) as i32), s16(self.rd16(0x118) as i32), g(0x116), g(0x118)];
                        let dir = self.call(op::DIRECTION, &args) as u8;
                        self.wr8(0x1A2, dir);
                    }
                    self.d_anim(0x28, 1);
                }
            } else if k == 3 {
                // b.
                self.wr8(0x195, 0);
            } else {
                // c.
                // Fan fix F08 (#70 row 2; changelog 1.15m, found by MitchShudlem, fixed by DarzaR; 1.15m TakeDamage
                // 0x39A2F..0x39A5D + 0x39A8B): a blast (k 4) of 5 or less damage off the merc's tile turns into
                // k 7, so the fall check below runs (k >= 6 there) and a merc left at 0 health falls; the end of a
                // task then follows the 0x2C / 0x2A pictures for any damage >= 0 instead of damage > 5.
                let small = !(d > 0x28 || tile == self.rd16(0x114) || d > 5);
                if d > 0x28 || tile == self.rd16(0x114) {
                    self.d_anim(0x2C, 0);
                } else if d > 5 {
                    self.d_anim(0x2A, 0);
                } else {
                    self.wr8(0x195, 0);
                    if k == 4 && crate::fanfix::on(8) {
                        k = 7;
                    }
                }
                extra = 4;
                let end = if crate::fanfix::on(8) { !small && d >= 0 } else { d > 5 };
                if end && self.rd8(0x1B9) != 0 {
                    self.call(op::END_TASK, &[slot, 0]);
                }
            }
            // d.
            // SPEC GAP: the spec reads as if the fall check (anim 0x2B) ran for every k; the original runs it only
            // when k = 3 or k = 6 (inside the hk = 3 condition).
            let fall_k = if crate::fanfix::on(8) { k as i8 >= 6 } else { k == 6 };
            if k == 3 || fall_k {
                hk = 3;
                // Fan fix F09 (#70 row 24; changelog 1.15z, found by DarzaR; 1.15m TakeDamage 0x39A9B): the fall
                // picture comes below 15 health, not only at 0, so a merc who collapses from bleeding (Bleed? passes
                // k 3) falls.
                let down = if crate::fanfix::on(9) { self.s8(0x183) < 15 } else { self.rd8(0x183) == 0 };
                if down && self.cur_anim() >= 4 || k == 6 && d == 1 {
                    self.d_anim(0x2B, 1);
                    if self.rd8(0x1B9) != 0 {
                        self.call(op::END_TASK, &[slot, 0]);
                    }
                }
            }
        }
        // 10.
        // Fan fix F10 (#70 row 82; changelog 1.15l, found by DarzaR; 1.15m TakeDamage 0x3974D + 0x3998A): the knock-down
        // flag that a leg hit sets is cleared here on every way out, not only after the knock-down picture, so a flag
        // left by a hit that did not knock its target down no longer drops the next merc that takes a bullet.
        if crate::fanfix::on(10) {
            unsafe { (*self.w).knockdown = 0 };
        }
        if k != 3 {
            self.call(op::EFFECT_SLOT, &[slot, hk]);
        }
        // 11.
        let v = isqrt(d).wrapping_sub(self.s8(0x192) / 2) as u8;
        if (v as i8) > 0 {
            self.call(op::MAKE_NOISE, &[slot, s16(self.rd16(0x114) as i32), self.rd8(0x1B3) as i32, v as i8 as i32, 7]);
        }
        // 12.
        if hk == 2 {
            self.call(op::SFX, &[0x20, SFX_CHANNEL, 0x32, 1]);
        }
        // 13.
        self.anim_delay();
        self.wr32(0xB0, s16(self.rd16(0xD2) as i32) as u32);
        // 14.
        let mut ap = (d.wrapping_div(4)).wrapping_add(extra as i32) as u8;
        if (ap as i8 as i32) > self.s8(0x1A5) {
            ap = self.rd8(0x1A5);
        }
        self.call(op::SPEND_AP, &[slot, ap as i8 as i32, 0]);
        // 15.
        if slot < 8 {
            self.call(op::RESERVE_AP, &[slot]);
        }
        // 16.
        if a < 0x40 {
            self.wr8(0x1DA, a);
            self.hit_reaction(a, 1, d as u8);
        }
    }

    // ---- Hit reaction ----
    pub(super) fn hit_reaction(&self, a: u8, b: u8, damage: u8) {
        // 1.
        let mine = self.s < 0x28;
        let theirs = self.rd8_of(a as usize, 0x17A) < 0x28;
        if mine == theirs || self.s8(0x183) < 15 || self.rd8(0xBA) == 2 {
            return;
        }
        // 2.
        let base = if mine { 0x112 } else { 0x13A };
        if self.s8(base + a as usize) == 1 {
            return;
        }
        // 3.
        unsafe { (*self.w).hit_scale = damage / 10 };
        self.hit_alert(a as usize, b);
        unsafe { (*self.w).hit_scale = 0 };
    }

    // ---- Hit alert ----
    pub(super) fn hit_alert(&self, i: usize, flag: u8) {
        let slot = self.slot();
        let o = self.rd8_of(i, 0x17A);
        // 1.
        if slot < 8 {
            if flag != 0 {
                self.call(op::FOCUS, &[slot, 0]);
                self.call(op::CRY, &[self.rd8(0x179) as i32, -1, 0, 1, 1]);
                self.call(op::VOICE, &[SFX_CHANNEL, 0x58, 1]);
            }
            self.call(op::STAT_PRACTICE, &[slot, 9, 1, 0]);
        }
        // 2.
        let mine = slot < 0x28;
        if mine == (o < 0x28) {
            return;
        }
        // 3.
        let foff = if mine { 0x112 } else { 0x13A } + o as usize;
        let old = self.rd8(foff);
        let kn = unsafe { (*self.w).knowledge };
        let tell = !(mine && matches!(get8(kn, KNOWLEDGE_LEN, o as i64) as i8, 1 | 2 | -1));
        // 4.
        let g = |off: usize| s16(self.rd16_of(i, off) as i32);
        let mut seen = false;
        if self.call(op::TILE_DIST, &[s16(self.rd16(0x114) as i32), g(0x114)]) as i32 <= 14 {
            let args = [
                slot,
                s16(self.rd16(0x116) as i32) + 2,
                s16(self.rd16(0x118) as i32) + 4,
                g(0x114),
                g(0x116) + 2,
                g(0x118) + 4,
                1,
                0x0E,
                o as i32,
            ];
            seen = self.call(op::SIGHT_LINE, &args) != 0;
        }
        if seen {
            // 5.
            self.mark_seen(i);
            if mine {
                if self.rd8_of(i, 0x9C) as i8 > 0 || unsafe { (*self.w).follow_all } != 0 {
                    unsafe {
                        (*self.w).spotted = o as i32;
                        (*self.w).focus_last = o;
                        (*self.w).focus_slot = o;
                    }
                    self.call(op::FOCUS, &[o as i32, 0]);
                }
                put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, o as i64, 1);
            }
        } else {
            // 6.
            self.wr8(0x1C1, 1);
            if self.s8(0xBB) < 2 {
                self.wr8(0xBB, 2);
                self.orders_bump();
            }
            if self.rd8(foff) as i8 != 2 {
                self.wr8(foff, 0xFF);
            }
            let t = self.rd16_of(i, 0x114);
            if !mine {
                let idx = (slot as i64 - 0x28) * 0x50 + 2 * o as i64;
                put16(unsafe { (*self.w).last_seen_e }, LAST_SEEN_E_LEN, idx, t);
            } else {
                let idx = slot as i64 * 16 + 2 * (o as i64 - 0x28);
                put16(unsafe { (*self.w).last_seen_m }, LAST_SEEN_M_LEN, idx, t);
                if tell && slot < 8 {
                    put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, o as i64, 0xFF);
                }
            }
        }
        // 7.
        self.call(op::INTERRUPT_CHECK, &[slot, i as i32, old as i32]);
    }

    /// Orders bump (BumpOrders 0x6F29C; shooting.md "Orders bump").
    pub(super) fn orders_bump(&self) {
        let t = self.rd8(0x1D4);
        if t < 2 && !(self.s8(0xBA) == 3 && t == 0 && self.s8(0x18F) == 99) {
            self.wr8(0x1D4, t + 1);
        }
    }

    // ---- Mark seen ----
    pub(super) fn mark_seen(&self, i: usize) {
        let slot = self.s;
        let b = self.rd8_of(i, 0x17A) as usize;
        // 1.
        let ok = |r: usize| self.rd8_of(r, 0x182) != 0 && self.rd8_of(r, 0x138) != 0 && self.rd8_of(r, 0x183) != 0;
        if !(slot < 0x30 && b < 0x30 && ok(slot) && self.sg_opp_in_play(i) && (slot < 0x28) != (b < 0x28) && self.s8(0xBA) != 2) {
            return;
        }
        // 2.
        // SPEC GAP: the spec indexes by i (the record index); the original takes the opponent's slot b = O +0x17A
        // for the test b < 0x28, j and the table entries (O's tile still comes from record i).
        let (j, rp, rlen, ri, lp, llen, li) = if b < 0x28 {
            (
                b,
                unsafe { (*self.w).remember_e },
                REMEMBER_E_LEN,
                (slot as i64 - 0x28) * 0x28 + b as i64,
                unsafe { (*self.w).last_seen_e },
                LAST_SEEN_E_LEN,
                (slot as i64 - 0x28) * 0x50 + 2 * b as i64,
            )
        } else {
            let j = b - 0x28;
            (
                j,
                unsafe { (*self.w).remember_m },
                REMEMBER_M_LEN,
                slot as i64 * 8 + j as i64,
                unsafe { (*self.w).last_seen_m },
                LAST_SEEN_M_LEN,
                slot as i64 * 16 + 2 * j as i64,
            )
        };
        // 3.
        let old = self.rd8(0x13A + j);
        self.wr8(0x13A + j, 1);
        let last = get16(lp, llen, li);
        let ot = self.rd16_of(i, 0x114);
        put16(lp, llen, li, ot);
        // 4.
        if old != 1 {
            self.wr8(0x1D5, self.rd8(0x1D5).wrapping_add(1));
            if self.s8(0xBB) < 2 {
                self.orders_bump();
            }
            self.wr8(0xBB, 3);
            if old as i8 != 2 || last != self.rd16_of(i, 0x114) {
                self.wr8(0x1C1, 1);
            }
            if matches!(self.rd8(0x1D1), 3 | 4) {
                self.wr8(0x1C1, 1);
            }
        }
        // 5.
        let r = get8(rp, rlen, ri);
        put8(rp, rlen, ri, if r == 0 { 0xFF } else { 1 });
        if (self.rd8_of(i, 0x9C) as i8) <= 0 {
            if let Some(oa) = Ai::new(self.w, self.h, i as i32) {
                oa.wr8(0x9C, 1);
            }
        }
    }

    // ================= Batch 3 =================

    // ---- Bullet damage (op 175) ----
    pub(super) fn bullet_damage(&self) -> i32 {
        // 1.
        let g = self.rd8(0x1F5) as i64;
        if !(1..=14).contains(&g) {
            return 0;
        }
        // 2.
        let it = unsafe { (*self.w).item_types };
        let r = s16(get8(it, ITEM_TYPES_LEN, 12 * g + 0x0B) as i32 * 15);
        if r == 0 {
            return 0;
        }
        // 3.
        let fall = s16((s16(self.rd16(0xC8) as i32) * 50).wrapping_div(r));
        let roll = (self.random(0x33) - 0x19) as i8 as i32;
        let m = (self.rd8(0x1FC) / 2) as i8 as i32;
        // 4.
        let wp = unsafe { (*self.w).weapons };
        let b = get8(wp, WEAPONS_LEN, 6 * g + 5) as i32;
        let d = s16(b.wrapping_mul(100 - fall + roll + m).wrapping_div(100));
        d.max(1)
    }

    // ---- Hit location (op 180) ----
    /// Fan fix F43 (#70 row 93): with the fix on, the entry at `at` works only at condition (byte at + 2) >= 9.
    fn worn_ok(&self, at: usize) -> bool {
        !crate::fanfix::on(43) || self.rd8(at + 2) as i8 >= 9
    }
    /// Armour piece: entry e, absorb byte offsets a_off / w_off; the amount absorbed (d is reduced).
    fn armour_piece(&self, e: usize, helmet: bool, r: i32, d: &mut i32) -> i32 {
        let t = self.rd8(12 * e) as i64;
        let ab = unsafe { (*self.w).absorb };
        let (ao, wo) = if helmet { (0x1E + 2 * t, 2 + 2 * t) } else { (2 * t, 2 * t - 10) };
        // SPEC GAP: an absorb offset outside the 576-byte table (e.g. 2 t − 10 < 0 for body armour of type < 5)
        // reads 0.
        let mut a = get16(ab, ABSORB_LEN, ao) as i16 as i32;
        let c = self.rd8(12 * e + 2);
        let o = r - c as i8 as i32;
        if o > 0 {
            a = (a - o).max(0);
        }
        a = a.min(*d);
        if a != 0 {
            let wear = get16(ab, ABSORB_LEN, wo) as i16 as i32;
            let loss = wear.wrapping_mul((a.wrapping_mul(50)).wrapping_div(100)).wrapping_div(100);
            self.wr8(12 * e + 2, (c as i32).wrapping_sub(loss) as u8);
            *d -= a;
        }
        a
    }
    /// Stat loss on byte f: Some(n) for a loss.
    fn stat_loss(&self, f: usize, d: i32) -> Option<i32> {
        if self.random(s16(d)) + 1 <= 20 {
            return None;
        }
        let mut n = self.random(s16(d.wrapping_div(2))) + 1;
        let v = self.s8(f);
        if v - n < 1 {
            n = v - 1;
        }
        self.wr8(f, (self.rd8(f) as i32).wrapping_sub(n) as u8);
        Some(n)
    }
    /// The stat-loss message of a hit (MsgMercText 0x15910 "NAME <text>", mercs only: the port emits LOSS_MSG with
    /// the same slot/amount/part; the front-end shows no loss message yet). ai_diff's loss_msg section runs the
    /// original body in the TacticalLoop block and compares its ShowMessage text with the mirror's.
    fn loss_msg(&self, part: i32, amount: i32) {
        if part == 0 || self.s < 8 {
            self.call(op::LOSS_MSG, &[self.slot(), amount as u8 as i32, part]);
        }
    }
    pub(super) fn hit_location(&self, height: i32, damage: i32) -> i32 {
        let mut d = damage;
        let mut height = height;
        // 1.
        let r = self.random(100) + 1;
        if d == 0 {
            return 0;
        }
        // 2.
        let a0 = self.cur_anim() as i16 as i32;
        if (a0 <= 3 || a0 >= 0x39) && height <= 1000 {
            height = height.wrapping_mul(6);
        }
        // 3.
        if height > 6000 || height < 0 {
            return 0;
        }
        if height >= 5000 {
            // 7. Head.
            // Fan fix F43 (#70 row 93; 1.15m HitLocationDamage 0x38DF9 / 0x38FA9 / 0x39031 -> tails 0x8ADCC /
            // 0x8ADF0 / 0x8AE29): helmet, item 0x31 and body armour stop a hit only at condition >= 9 (signed).
            let mut a = 0;
            if self.rd8(0x18) != 0 && self.worn_ok(0x18) {
                a = self.armour_piece(2, true, r, &mut d);
            }
            if a == 0 {
                d = d.wrapping_add(d.wrapping_mul(50).wrapping_div(100));
                match self.stat_loss(0x18D, d) {
                    Some(n) => self.loss_msg(1, n),
                    None => self.loss_msg(0, 0xFF),
                }
            }
        } else if height >= 2500 {
            // 8. Torso.
            let mut total = 0;
            if self.rd8(0x48) == 0x31 && self.worn_ok(0x48) {
                total += self.armour_piece(6, false, r, &mut d);
            }
            if self.rd8(0x3C) != 0 && self.worn_ok(0x3C) {
                total += self.armour_piece(5, false, r, &mut d);
            }
            if total == 0 {
                if let Some(n) = self.stat_loss(0x18C, d) {
                    self.loss_msg(2, n);
                }
            }
        } else {
            // 9. Legs.
            d = d.wrapping_sub(d.wrapping_mul(50).wrapping_div(100));
            if let Some(n) = self.stat_loss(0x18B, d) {
                self.loss_msg(3, n);
                if (4..0x29).contains(&a0) && a0 != 0x0D {
                    unsafe { (*self.w).knockdown = 1 };
                }
            }
        }
        // 10.
        d.max(0)
    }

    // ---- Shot alert (op 173) ----
    pub(super) fn shot_alert(&self, flag: u8) {
        let slot = self.slot();
        let k = self.rd8(0x1D8) as usize;
        // 1.
        if k >= 0x40 {
            return;
        }
        let oa = match Ai::new(self.w, self.h, k as i32) {
            Some(a) => a,
            // SPEC GAP: a target slot 0x30..0x3F (no record) reads as zeros: done (as O +0x182 = 0).
            None => return,
        };
        if oa.rd8(0x182) == 0 || oa.rd8(0x138) == 0 || oa.s8(0x183) < 15 {
            return;
        }
        // 2.
        if oa.s8(0xBA) == 2 {
            oa.wr8(0x1C4, 1);
            return;
        }
        // 3.
        let o = oa.rd8(0x17A) as i32;
        if slot < 0x28 && o < 0x28 || slot >= 0x28 && o > 0x28 {
            return;
        }
        // 4.
        let mut news = true;
        let kn = unsafe { (*self.w).knowledge };
        let foff = if o < 0x28 {
            if matches!(get8(kn, KNOWLEDGE_LEN, slot as i64) as i8, 1 | 2 | -1) {
                news = false;
            }
            0x112 + self.s
        } else {
            0x13A + self.s
        };
        let old = oa.rd8(foff);
        // 5.
        let mut q = NO_SLOT;
        if flag == 0 {
            let t = vt(s16(self.rd16(0xE4) as i32), s16(self.rd16(0xE6) as i32)) as u16;
            q = self.occupant(t);
            if q != NO_SLOT {
                if let Some(qa) = Ai::new(self.w, self.h, q as i32) {
                    qa.wr8(0x1C4, 1);
                }
            }
        }
        // 6.
        let t = s16(oa.rd16(0x114) as i32);
        let dx = (s16(self.rd16(0x116) as i32) + 2 - (px(t) + 2)).abs();
        let dy = (s16(self.rd16(0x118) as i32) + 4 - (py(t) + 4)).abs();
        let near = s16(dx.max(dy));
        if s16(self.rd16(0xC8) as i32) < near - 0x2D || flag == 0 && q != NO_SLOT {
            return;
        }
        // 7.
        oa.wr8(0x1C4, 1);
        let mut seen = false;
        let my = |off: usize| s16(self.rd16(off) as i32);
        if self.call(op::TILE_DIST, &[t, my(0x114)]) as i32 <= 14 {
            let og = |off: usize| s16(oa.rd16(off) as i32);
            let args = [k as i32, og(0x116) + 2, og(0x118) + 4, my(0x114), my(0x116) + 2, my(0x118) + 4, 1, 0x0E, slot];
            seen = self.call(op::SIGHT_LINE, &args) != 0;
        }
        if seen {
            // 8.
            oa.mark_seen(self.s);
            if o < 0x28 {
                self.call(op::SHARE_SIGHTINGS, &[k as i32, 1]);
            }
        } else {
            // 9.
            oa.wr8(0x1C1, 1);
            if oa.s8(0xBB) < 2 {
                oa.wr8(0xBB, 2);
                oa.orders_bump();
            }
            if oa.rd8(foff) as i8 != 2 {
                oa.wr8(foff, 0xFF);
            }
            let me = self.rd16(0x114);
            if o >= 0x28 {
                let idx = (o as i64 - 0x28) * 0x50 + 2 * slot as i64;
                put16(unsafe { (*self.w).last_seen_e }, LAST_SEEN_E_LEN, idx, me);
            } else {
                let idx = o as i64 * 16 + 2 * (slot as i64 - 0x28);
                put16(unsafe { (*self.w).last_seen_m }, LAST_SEEN_M_LEN, idx, me);
                if news && o < 8 {
                    self.call(op::STAT_PRACTICE, &[k as i32, 9, 1, 0]);
                    self.call(op::MERC_SEES, &[o, 1, 0x40]);
                    put8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, slot as i64, 0xFF);
                }
            }
        }
        // 10.
        self.call(op::INTERRUPT_CHECK, &[k as i32, slot, old as i32]);
    }

    // ---- Interrupt check (op 182) ----
    pub(super) fn interrupt_check(&self, i: usize, old: u8) {
        let slot = self.slot();
        // 1.
        if self.s8(0xBA) == 2 || self.rd8(0x18A) == 0 || self.cur_anim() < 4 {
            return;
        }
        // 2.
        let o = self.rd8_of(i, 0x17A) as usize;
        let (mine, theirs) = if o >= 0x28 {
            (self.rd8(0x112 + o), self.rd8_of(i, 0x13A + self.s))
        } else {
            (self.rd8(0x13A + o), self.rd8_of(i, 0x112 + self.s))
        };
        // 3.
        let need: u8;
        if mine as i8 == 1 {
            if matches!(old as i8, 1 | 2) {
                return;
            }
            need = self.call(op::MOVE_AP, &[slot]) as u8;
        } else {
            if theirs as i8 != 1 || self.s8(0x1D5) > 0 || old as i8 == -1 {
                return;
            }
            need = 1;
        }
        // 4.
        if self.s8(0x1A5) < need as i32 {
            return;
        }
        // 5.
        unsafe { (*self.w).interrupted = 1 };
        // 6.
        if slot < 0x28 {
            if self.call(op::ASK_MERC_INTERRUPT, &[slot, i as i32]) as u8 == 0 {
                return;
            }
            if unsafe { (*self.w).interrupt_go } != 0 {
                self.call(op::MERC_INTERRUPT, &[]);
                return;
            }
            if unsafe { (*self.w).turn_moves } == 0 {
                return;
            }
        } else {
            if self.call(op::ASK_INTERRUPT, &[slot, i as i32]) as u8 == 0 || unsafe { (*self.w).turn_moves } == 0 {
                return;
            }
            let sel = unsafe { (*self.w).selected };
            if sel < 8 {
                unsafe { (*self.w).kept_merc = sel };
            }
        }
        // 7.
        self.call(op::NEXT_TURN, &[1]);
    }

    // ---- Interrupt contests (combat.md "Interrupt contests, exact steps") ----
    /// sees (r, x) = s8 r +0x13A + x.
    // SPEC GAP: an index past the record (a bad slot byte) reads 0.
    fn sees_of(&self, r: usize, x: u8) -> i8 {
        self.rd8_of(r, 0x13A + x as usize) as i8
    }
    /// BONUS (difficulty): the low byte of (-1, 0, 1).
    // SPEC GAP: another difficulty reads the original's neighbouring data; the port uses 0.
    fn diff_bonus(&self) -> u8 {
        match unsafe { (*self.w).difficulty } {
            0 => 0xFF,
            2 => 1,
            _ => 0,
        }
    }
    /// push (x): itq[itq_len] = x, itq_len + 1 (8-bit).
    /// itq_len ≥ 50: a stop point (port_api.md "Division by zero"; the original writes past the queue).
    fn contest_push(&self, v: u8) {
        unsafe {
            let n = (*self.w).itq_len;
            if n as usize >= 50 {
                self.call(op::DIV_ZERO, &[]);
                return;
            }
            put8((*self.w).itq, 50, n as i64, v);
            (*self.w).itq_len = n.wrapping_add(1);
        }
    }
    /// Grant: s +0x1B0 = 0; selected >= 8 and s8 interrupted = 1: interrupted = 0, push (selected).
    fn contest_grant(&self) {
        self.wr8(0x1B0, 0);
        crate::trace::grant(self.s as u8, unsafe { (*self.w).selected });
        let sel = unsafe { (*self.w).selected };
        if sel >= 8 && unsafe { (*self.w).interrupted } as i8 == 1 {
            unsafe { (*self.w).interrupted = 0 };
            self.contest_push(sel);
        }
    }
    /// The level part both sides share: + 1 if rem = 1; − (+0x1D5 − 1) if s8 +0x1D5 > 1.
    fn contest_level(&self, r: usize, rem: u8) -> u8 {
        let mut a = self.rd8_of(r, 0x192).wrapping_add((rem == 1) as u8);
        let x = self.rd8_of(r, 0x1D5);
        if x as i8 > 1 {
            a = a.wrapping_sub(x.wrapping_sub(1));
        }
        a
    }
    /// Merc contest (library op 128 ask_merc_interrupt): opponent = the enemy's record index. 1 = the merc wins.
    pub(super) fn merc_contest(&self, i: usize) -> u32 {
        let slot = self.slot();
        let m = self.rd8(0x17A);
        let j = self.rd8_of(i, 0x17A).wrapping_sub(0x28);
        // 1.
        self.wr8(0x1C1, 1);
        let need = if self.sees_of(self.s, j) == 1 { self.call(op::MOVE_AP, &[slot]) as u8 } else { 1 };
        // 2.
        if (need as i8) > self.s8(0x1A5) as i8 || unsafe { (*self.w).screen_drawn } != 0 {
            return 0;
        }
        // 3. / 4.
        if self.sees_of(i, m) == 1 {
            let rm = get8(unsafe { (*self.w).remember_m }, REMEMBER_M_LEN, 8 * m as i64 + j as i64);
            let re = get8(unsafe { (*self.w).remember_e }, REMEMBER_E_LEN, 0x28 * j as i64 + m as i64);
            let mut a = self.contest_level(self.s, rm);
            if self.sees_of(self.s, j) == -1 {
                a = a.wrapping_sub(1);
            }
            a = a.wrapping_sub(unsafe { (*self.w).hit_scale });
            let mut b = self.contest_level(i, re).wrapping_add(self.diff_bonus());
            if get8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, m as i64) != 0 {
                b = b.wrapping_add(1);
            }
            if a as i8 <= b as i8 {
                return 0;
            }
        }
        // 5.
        self.contest_grant();
        if (self.rd8_of(i, 0x9C) as i8) > 0 || unsafe { (*self.w).follow_all } != 0 {
            unsafe {
                (*self.w).show_timer = 5000;
                (*self.w).spotted = self.rd8_of(i, 0x17A) as i32;
            }
        }
        // 6.
        unsafe {
            if m < 8 {
                (*self.w).interrupter = m;
                (*self.w).interrupt_go = 1;
                if (*self.w).new_round != 0 && (*self.w).kept_merc < 8 {
                    (*self.w).interrupter = (*self.w).kept_merc;
                }
            } else {
                self.contest_push(m);
                (*self.w).turn_moves = 1;
            }
        }
        1
    }
    /// Enemy contest (library op 116 ask_interrupt): opponent = the merc's (or guard's) record index. 1 = the enemy wins.
    pub(super) fn enemy_contest(&self, i: usize) -> u32 {
        let slot = self.slot();
        let j = self.rd8(0x17A).wrapping_sub(0x28);
        let m = self.rd8_of(i, 0x17A);
        // 1.
        self.wr8(0x1C1, 1);
        let need = if self.sees_of(self.s, m) == 1 { self.call(op::MOVE_AP, &[slot]) as u8 } else { 1 };
        // 2.
        if (need as i8) > self.s8(0x1A5) as i8 {
            return 0;
        }
        // 3. / 4.
        if self.sees_of(i, j) == 1 && self.s8(0xBA) != 4 {
            let rm = get8(unsafe { (*self.w).remember_m }, REMEMBER_M_LEN, 8 * m as i64 + j as i64);
            let re = get8(unsafe { (*self.w).remember_e }, REMEMBER_E_LEN, 0x28 * j as i64 + m as i64);
            let mut a = self.contest_level(self.s, re).wrapping_add(self.diff_bonus());
            if get8(unsafe { (*self.w).knowledge }, KNOWLEDGE_LEN, m as i64) != 0 {
                a = a.wrapping_add(1);
            }
            if self.sees_of(self.s, m) == -1 {
                a = a.wrapping_sub(1);
            }
            a = a.wrapping_sub(unsafe { (*self.w).hit_scale });
            let b = self.contest_level(i, rm);
            if a as i8 <= b as i8 {
                return 0;
            }
        }
        // 5.
        self.contest_grant();
        self.contest_push(self.rd8(0x17A));
        unsafe { (*self.w).turn_moves = 1 };
        1
    }
}

// ---- entry points (shooting.md batch 2) ----
// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_take_damage(
    w: *mut Ja1World,
    h: *const Ja1Host,
    slot: i32,
    height: i32,
    damage: i32,
    kind: i32,
    attacker: i32,
    tile: i32,
) {
    run(w, h, slot, (), |a| a.take_damage(height, damage, kind as u8, attacker as u8, tile as u16))
}
#[no_mangle]
pub extern "C" fn ja1_hit_reaction(w: *mut Ja1World, h: *const Ja1Host, slot: i32, attacker: i32, flag: i32, damage: i32) {
    run(w, h, slot, (), |a| a.hit_reaction(attacker as u8, flag as u8, damage as u8))
}
// SPEC GAP: an opponent index outside 0..47 reads its record as zeros and is never written.
#[no_mangle]
pub extern "C" fn ja1_hit_alert(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32, flag: i32) {
    run(w, h, slot, (), |a| a.hit_alert((opponent as u32).min(0xFFFF) as usize, flag as u8))
}
#[no_mangle]
pub extern "C" fn ja1_mark_seen(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32) {
    run(w, h, slot, (), |a| a.mark_seen((opponent as u32).min(0xFFFF) as usize))
}

// ---- entry points (shooting.md batch 3) ----
#[no_mangle]
pub extern "C" fn ja1_bullet_damage(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    run(w, h, slot, 0, |a| a.bullet_damage())
}
#[no_mangle]
pub extern "C" fn ja1_hit_location(w: *mut Ja1World, h: *const Ja1Host, slot: i32, height: i32, damage: i32) -> i32 {
    run(w, h, slot, 0, |a| a.hit_location(height, damage))
}
#[no_mangle]
pub extern "C" fn ja1_shot_alert(w: *mut Ja1World, h: *const Ja1Host, slot: i32, flag: i32) {
    run(w, h, slot, (), |a| a.shot_alert(flag as u8))
}
// SPEC GAP: an opponent index outside 0..47 reads its record as zeros and is never written.
#[no_mangle]
pub extern "C" fn ja1_ask_merc_interrupt(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32) -> i32 {
    run(w, h, slot, 0, |a| a.merc_contest((opponent as u32).min(0xFFFF) as usize) as i32)
}
#[no_mangle]
pub extern "C" fn ja1_ask_interrupt(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32) -> i32 {
    run(w, h, slot, 0, |a| a.enemy_contest((opponent as u32).min(0xFFFF) as usize) as i32)
}
#[no_mangle]
pub extern "C" fn ja1_interrupt_check(w: *mut Ja1World, h: *const Ja1Host, slot: i32, opponent: i32, old: i32) {
    run(w, h, slot, (), |a| a.interrupt_check((opponent as u32).min(0xFFFF) as usize, old as u8))
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;
    use std::os::raw::c_void;

    unsafe extern "C" fn ops(ctx: *mut c_void, op: i32, _a: *const i32, _n: i32, _o: *mut u8, _os: i32) -> u32 {
        (*(ctx as *mut Vec<i32>)).push(op);
        0
    }

    /// HitLocationDamage 0x38D80: the stat-loss message is armed only for a merc (asm 0x38EE7 cmp +0x17A, 8 / jge
    /// past it); part 0 (head hit, no loss) is for any soldier.
    #[test]
    fn loss_msg_mercs_only() {
        let mut sol = vec![0u8; 48 * 0x200];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        let mut log: Vec<i32> = vec![];
        let h = Ja1Host { ctx: &mut log as *mut Vec<i32> as *mut c_void, random: None, call: Some(ops), host_ops: !0 };
        let wp: *mut Ja1World = &mut *w;
        Ai { w: wp, h: &h, s: 8 }.loss_msg(1, 2);
        let n8 = log.len();
        Ai { w: wp, h: &h, s: 7 }.loss_msg(1, 2);
        Ai { w: wp, h: &h, s: 8 }.loss_msg(0, 0xFF);
        assert_eq!((n8, log.iter().filter(|&&o| o == op::LOSS_MSG).count()), (0, 2));
    }
}
