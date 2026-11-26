//! Shooting batch 5 (docs/spec/shooting.md "Batch 5"; C interface in port_api.md): Start attack (library op 79) with
//! Aim posture, Attack go (library op 80), Fire gun, Chance to hit (library op 46), Hit test (library op 48) and
//! Blast kind. A child module of `ai`: every op goes through `Ai::call` (the host_ops rule). World data is read
//! again after every host call.
use super::damage::{get16, get8};
use super::fire::vt;
use super::search::{px, py, s16, world_dist};
use super::{op, run, Ai, Ja1Host, Ja1World, NONE, NO_SLOT};

const POSTURE_LEN: usize = 256;
/// The item place of hand and pockets 1..6 (0x93D59 = 0 7 8 9 10 11 1).
const POCKETS: [usize; 7] = [0, 7, 8, 9, 10, 11, 1];
const WINDOW_LEN: usize = 2056;
const ATTACK_SOUND_LEN: usize = 10;
const DIFF_MOD_LEN: usize = 6;
const WEAPONS_LEN: usize = 256 * 6;
const ITEM_TYPES_LEN: usize = 136 * 12;
const CLASS_LEN: usize = 256;

/// Blast kind of an item type t (ExplosiveKind? 0x5AB78).
pub(super) fn blast_kind(t: u8) -> u8 {
    match t {
        0..=0x15 => 0,
        0x16..=0x1B => t - 0x12,
        0x1C | 0x1D => 2,
        0x5D => 1,
        _ => 3,
    }
}

/// Water tile (t0, t1) (soldier_update.md).
fn water_tile(t0: u8, t1: u8) -> bool {
    t0 >= 0x6C && t1 != 0x1C && t1 != 0x1F
}

impl Ai {
    /// The current animation word of record r.
    fn a_anim_of(&self, r: usize) -> u16 {
        self.rd16_of(r, 0x1E8 + 2 * self.rd8_of(r, 0x1F3) as usize)
    }
    fn a_s16_of(&self, r: usize, off: usize) -> i32 {
        self.rd16_of(r, off) as i16 as i32
    }
    fn a_wr8_of(&self, r: usize, off: usize, v: u8) {
        if let Some(o) = Ai::new(self.w, self.h, r as i32) {
            o.wr8(off, v);
        }
    }
    fn a_weapon(&self, item: u8, b: usize) -> u8 {
        get8(unsafe { (*self.w).weapons }, WEAPONS_LEN, item as i64 * 6 + b as i64)
    }
    // SPEC GAP: off the map (e.g. the tile of a missed shot after Throw scatter, read by Attack go) the original
    // reads past the tile map; the port reads byte 5 as 0 there (occupant 0), as the other tile reads do.
    fn a_occupant(&self, t: i32) -> u8 {
        let o = self.tb(t, 5) & 0x7F;
        if o >= 0x30 { NO_SLOT } else { o }
    }
    fn a_t1_class(&self, b: u8) -> u8 {
        get8(unsafe { (*self.w).t1_class }, CLASS_LEN, b as i64)
    }
    /// Aim direction from the soldier's tile to `tile`: (direction, the u16 for +0x10A).
    fn aim_dir(&self, tile: i32) -> (u8, u16) {
        let me = s16(self.rd16(0x114) as i32);
        super::update::aim_dir(px(me), py(me), px(tile), py(tile))
    }

    // ---- Start attack (op 79) ----
    pub(super) fn start_attack(&self, tile: i32) {
        let slot = self.slot();
        let tile = s16(tile);
        // 1.
        if self.rd8(0x1BE) != 1 {
            return;
        }
        // 2.
        let (d, word) = self.aim_dir(tile);
        self.wr16(0x10A, word);
        self.wr8(0x1A2, d);
        // 3.
        self.aim_posture(tile);
        // 4.
        let occ = self.a_occupant(tile);
        // 5.
        if self.rd8(1) != 0 {
            self.wr8(0x1BE, 2);
            self.wr16(0xDC, tile as u16);
        }
        // 6.
        if self.s8(0) <= 0xE && self.rd8(1) == 0 && self.call(op::RELOADED, &[slot]) != 0 {
            self.shot_end(1, 0);
            self.u_forget_attack(true);
            return;
        }
        // 7.
        self.wr8(0x1C9, blast_kind(self.rd8(0)));
        // 8.
        if occ == NO_SLOT || slot >= 8 || occ >= 8 {
            return;
        }
        // 9.
        let o = occ as usize;
        if self.rd8(0x162 + o) == 0 && self.rd8(0x19C) != 3 {
            self.call(op::MERC_MESSAGE, &[slot, 8, occ as i32, 0, 1]);
        }
        // 10.
        self.wr8(0x162 + o, 1);
        self.a_wr8_of(o, 0x162 + self.s, 1);
        self.wr8(0x1D8, NO_SLOT);
    }

    /// Aim posture (slot, tile).
    pub(super) fn aim_posture(&self, tile: i32) {
        // 1.
        let a = self.a_anim_of(self.s) as i16 as i32;
        if self.anim_col(a as u16, 1) == 0 || !(0..256).contains(&a) || water_tile(self.rd8(0x1B2), self.rd8(0x1B4)) {
            return;
        }
        // 2.
        let it = if self.rd8(1) != 0 { self.rd8(0) } else { 0 };
        let (d, word) = self.aim_dir(tile);
        self.wr16(0x10A, word);
        // 3.
        let mut code = 0;
        let si = it as i8 as i32;
        if it != 0 && si <= 0x10 {
            if si >= 0xF {
                if !(0x22..=0x24).contains(&a) {
                    code = 0x22;
                }
            } else if si >= 8 {
                if !(0x1E..=0x20).contains(&a) {
                    code = 0x1E;
                }
            } else if !(0x1A..=0x1C).contains(&a) {
                code = 0x1A;
            }
        }
        // 4.
        if code != 0 {
            self.call(op::SET_ANIM, &[self.slot(), code, 0, 1]);
        }
        // 5.
        if d != self.rd8(0x1A2) {
            if self.rd8(0) < 0xF && code == 0 {
                self.call(op::SPEND_AP, &[self.slot(), 1, 0xA]);
            }
            self.wr8(0x1A2, d);
        }
    }

    // ---- Attack go (op 80) ----
    pub(super) fn attack_go(&self) {
        let slot = self.slot();
        let g = |o: usize| s16(self.rd16(o) as i32);
        // 1.
        if self.rd8(0x1BE) != 2 {
            return;
        }
        // #70 F32 (row 23): 1.15m 0x5AD6A waits while something still animates (busy 0x92E38 > 0).
        if crate::fanfix::on(32) && unsafe { (*self.w).busy } > 0 {
            return;
        }
        if !matches!(self.a_anim_of(self.s), 0x1B | 0x1F | 0x23) || g(0xDC) == NONE as i32 {
            return;
        }
        // 2.
        self.fire_gun(g(0xDC));
        // 3.
        let t = g(0xDC);
        let occ = self.a_occupant(t);
        let (x, y, ex, ey) = (g(0x116), g(0x118), g(0xDE), g(0xE0));
        // 4.
        if occ >= 0x30 || (occ < 0x28 && slot < 0x28) {
            let last = if occ >= 0x30 { 0x40 } else { occ as i32 };
            if self.call(op::SIGHT_LINE, &[slot, x + 2, y + 4, t, ex, ey, 0x58, 14, last]) != 0 {
                self.wr8(0x1FF, (x - ex).abs().max((y - ey).abs()) as u8);
            }
        } else {
            let i = if occ >= 0x28 { occ - 0x28 } else { occ } as usize;
            if self.s8(0x13A + i) == 1 {
                let (ox, oy) = (self.a_s16_of(occ as usize, 0x116), self.a_s16_of(occ as usize, 0x118));
                self.wr8(0x1FF, (x - ox).abs().max((y - oy).abs()) as u8);
            }
        }
        // 5.
        let it = self.rd8(0);
        let code = if it <= 7 {
            let near = self.call(op::TILE_DIST, &[t, g(0x114)]) as i32;
            if near < 2 && (occ >= 0x30 || self.a_anim_of(occ as usize) < 4) {
                0x2D
            } else {
                0x1C
            }
        } else if it <= 0xE {
            0x20
        } else if it <= 0x10 {
            if unsafe { (*self.w).combat } != 0 && slot >= 8 {
                self.call(op::FOCUS, &[slot, 0]);
            }
            0x24
        } else {
            return;
        };
        // 6.
        self.call(op::SET_ANIM, &[slot, code, 0, 1]);
    }

    // ---- Fire gun ----
    pub(super) fn fire_gun(&self, tile: i32) {
        let slot = self.slot();
        let tile = s16(tile);
        let g = |o: usize| s16(self.rd16(o) as i32);
        // 1.
        if unsafe { (*self.w).combat } != 0 {
            if slot >= 8 && self.s8(0x9C) > 0 {
                self.call(op::FOCUS, &[slot, 0]);
            }
            unsafe { (*self.w).effect_tile = tile as i16 };
        }
        // 2.
        let it = self.s8(0);
        if it == 0xF || it == 0x10 {
            return;
        }
        // 3.
        let c = self.call(op::SHOT_AP, &[slot, tile]) as u8 as i32;
        let class = self.a_weapon(it as u8, 0);
        let snd = get16(unsafe { (*self.w).attack_sound }, ATTACK_SOUND_LEN, 2 * class as i64) as i16 as i32;
        let amount = (c + self.s8(0x1FD)) as i8 as i32;
        self.call(op::SPEND_AP, &[slot, amount, snd]);
        // 4.
        let occ = self.a_occupant(tile);
        if occ < NO_SLOT {
            let o = occ as usize;
            // a.
            self.wr16(0xDE, self.rd16_of(o, 0x116).wrapping_add(2));
            self.wr16(0xE0, self.rd16_of(o, 0x118).wrapping_add(4));
            // b.
            let r = self.random(20) as i8 as i32;
            let e2 = if r < 16 {
                self.random(2500) + 2500
            } else if r < 19 {
                self.random(1001) + 5000
            } else {
                self.random(2500)
            };
            self.wr16(0xE2, e2 as u16);
            // c.
            let a = self.a_anim_of(o);
            if a <= 3 || a >= 0x39 {
                let v = self.cdiv(g(0xE2), 6);
                self.wr16(0xE2, v as u16);
            }
        } else {
            // a.
            self.wr16(0xDE, px(tile) as u16);
            self.wr16(0xE0, py(tile) as u16);
            match self.rd8(0x1A2) {
                1 | 2 | 8 => self.wr16(0xE0, self.rd16(0xE0).wrapping_add(5)),
                4 | 5 | 6 => self.wr16(0xE0, self.rd16(0xE0).wrapping_sub(5)),
                _ => {}
            }
            // b.
            let b = self.tb(tile, 1);
            let k = self.a_t1_class(b);
            let e2 = if (6..=8).contains(&k) || (k == 9 && matches!(b, 0xE2 | 0xE3)) {
                5000
            } else if k == 9 {
                2000
            } else {
                0
            };
            self.wr16(0xE2, e2);
        }
        // 5.
        let ch = self.call(op::CHANCE_TO_HIT, &[slot, tile, self.rd8(0x1FD) as i32]) as i8 as i32;
        let r = (self.random(100) + 1) as i8 as i32;
        if r <= ch {
            // 6.
            self.wr8(0x1BC, 1);
            self.wr16(0xDC, tile as u16);
            self.wr8(0x1FC, (ch - r) as u8);
        } else {
            // 7.
            self.wr8(0x1BC, 0);
            let m = (r - ch) as u8;
            self.wr8(0x1FC, 0);
            let d = world_dist(g(0x116), g(0x118), g(0xDE), g(0xE0));
            self.throw_scatter(m, s16(d));
            let mut v = self.cdiv(g(0xE2) * (m as i8 as i32), 100);
            if self.random(2) != 0 {
                v = -v;
            }
            self.wr16(0xE2, (self.rd16(0xE2) as i32 + v - d) as u16);
            if g(0xE2) < 0 {
                self.wr16(0xE2, 0);
            }
        }
        // 8.
        self.wr16(0xD0, tile as u16);
    }

    // ---- Chance to hit (op 46) ----
    pub(super) fn chance_to_hit(&self, tile: i32, aim: u8) -> i32 {
        let slot = self.slot();
        let tile = s16(tile);
        let aim = aim as i32;
        let g = |o: usize| s16(self.rd16(o) as i32);
        let u = |r: usize, o: usize| self.rd8_of(r, o) as i32;
        let s = |r: usize, o: usize| self.rd8_of(r, o) as i8 as i32;
        let me = self.s;
        // 1.
        let mk = s(me, 0x191);
        let mut c = if u(me, 2) >= mk { mk } else { self.cdiv(mk + u(me, 2), 2) };
        // 2.
        if tile == g(0xD0) {
            c += 10;
        }
        c += 10 * aim;
        // 3.
        if slot >= 0x28 {
            let dif = unsafe { (*self.w).difficulty } as i8 as i64;
            c += get16(unsafe { (*self.w).diff_mod }, DIFF_MOD_LEN, 2 * dif) as i16 as i32;
        }
        // 4.
        if u(me, 0x1B9) == 4 {
            c = (c - 20).max(1);
        }
        // 5.
        let mut d = world_dist(g(0x116), g(0x118), px(tile), py(tile));
        // 6.
        // Fan fix F43 (#70 row 93; 1.15m 0x566F7 -> tail 0x8AD3A, 0x5671D -> tail 0x8AD4D): sun goggles need
        // condition >= 10 and the scope condition > 9 (signed bytes), else they give nothing.
        let worn = crate::fanfix::on(43);
        if u(me, 0x24) == 0x27 && (!worn || s(me, 0x26) >= 10) {
            d -= self.cdiv(d, 10);
        }
        if u(me, 3) == 0x42 && (!worn || s(me, 5) > 9) && d > 150 && aim != 0 {
            let q = s16(self.cdiv((d - 150) * aim * 15, 100));
            d -= self.cdiv(q * s(me, 5), 100);
        }
        // 7.
        c += self.cdiv(75 - d, 3);
        // 8.
        let o = self.a_occupant(tile);
        if o < NO_SLOT {
            let t = o as usize;
            let a = self.a_anim_of(t) as u8 as i32;
            // a.
            if (a <= 3 || a >= 0x39) && d > 20 {
                c -= self.cdiv(d - 20, 3);
            }
            // b.
            if (5..=7).contains(&a) || (14..=16).contains(&a) {
                c -= 20;
            }
            // c.
            if d > 75 && u(t, 0x1B1) != 0 && matches!(u(t, 0x1B3), 1 | 2 | 4 | 5 | 6 | 7) {
                c -= 5;
            }
            // d.
            if s(t, 0x183) >= 15 {
                let p = self.max_ap(t);
                let used = if p > s(t, 0x1A5) { (p - u(t, 0x1A5)) as u8 as u32 } else { 0 };
                let mut m = if used != 0 {
                    ((p.wrapping_mul(u(t, 0x1F2)) as u32) / used) as u8 as i32
                } else {
                    u(t, 0x1F2)
                };
                if s(t, 0x1D5) > 0 {
                    let f = if u(t, 0x17A) >= 0x28 {
                        u(t, 0x13A + me)
                    } else if slot >= 0x28 {
                        u(t, 0x112 + me)
                    } else {
                        0
                    };
                    if f == 1 {
                        m = (m + self.cdiv(s(t, 0x18B), 20) + self.cdiv(s(t, 0x192), 2)) as u8 as i32;
                    }
                }
                let tr = self.a_weapon(u(me, 0) as u8, 4) as i32;
                if m > tr {
                    c -= (m - tr) * 5;
                }
            }
        }
        // 9.
        let (hp, mh) = (s(me, 0x183), s(me, 0x184));
        if hp < mh {
            let half = ((u(me, 0x184) - u(me, 0x183) - u(me, 0x185)) as u8 as i32) / 2;
            // Fan fix F06 (#70 row 88, switch HEALTHY; 1.15m 0x5698F -> 0x3BFCF): bandages lower the loss.
            let lost = if crate::fanfix::on(6) { (mh - hp) - half } else { half + (mh - hp) };
            let part = self.cdiv(c * 2 * lost, mh * 3) as u8 as i32;
            c -= self.cdiv(part * (100 - (s(me, 0x192) - 1) * 10), 100);
        }
        // 10.
        let e = s(me, 0x18A);
        if e < 100 {
            let part = self.cdiv((100 - e) * c, 200) as u8 as i32;
            c -= self.cdiv((100 - (s(me, 0x18C) - 10)) * part, 100);
        }
        // 11.
        let range = get8(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, 12 * u(me, 0) as i64 + 11) as i32;
        if d > range * 15 {
            c = self.cdiv(c, 2);
        }
        // 12.
        c.clamp(1, 99)
    }

    // ---- Hit test (op 48) ----
    /// Returns (result, out skip, out chance); out_skip and chance are the out record's starting values.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn hit_test(
        &self,
        h: i32,
        x: i32,
        y: i32,
        flown: i32,
        dir: u8,
        skip: u8,
        cap: i32,
        target: i32,
        roll: i32,
        out_skip: u8,
        chance: i32,
    ) -> (i32, u8, i32) {
        let target = s16(target);
        // 1.
        if h < 1 {
            return (1, out_skip, chance);
        }
        // 2.
        let t = vt(x, y);
        let b = self.tb(t, 1);
        let k = self.a_t1_class(b);
        let mut ob = match k {
            6 => 9000,
            7 | 8 => 10000,
            9 => {
                if matches!(b, 0xE2 | 0xE3) {
                    7000
                } else {
                    3000
                }
            }
            _ => 0,
        };
        // 3.
        if cap > 9000
            && get8(unsafe { (*self.w).t0_class }, CLASS_LEN, self.tb(t, 0) as i64) == 3
            && self.tb(t, 3) >= 0x52
        {
            ob = 10001;
        }
        // 4.
        let o = self.a_occupant(t);
        if o < NO_SLOT {
            let r = o as usize;
            let ttile = self.rd16_of(r, 0x114);
            let a = self.a_anim_of(r);
            // a.
            if target as u16 != ttile && flown < 0x4B {
                // SPEC GAP: an animation word of 256 and up reads posture 0.
                if self.random(30) != 0 || (get8(unsafe { (*self.w).posture }, POSTURE_LEN, a as i64) as i8) < 6 {
                    return (0, out_skip, chance);
                }
            }
            // b.
            ob += if a < 4 || a > 0x37 { 1000 } else { 6000 };
            // c.
            let dx = (x - (self.a_s16_of(r, 0x116) + 2)).abs();
            let dy = (y - (self.a_s16_of(r, 0x118) + 4)).abs();
            if ob >= h && dx.max(dy) <= 1 {
                return (1, out_skip, chance);
            }
            // d.
            let skip = if target as u16 == ttile { 0 } else { out_skip };
            return (0, skip, chance);
        }
        // 5.
        if ob < h {
            return (0, out_skip, chance);
        }
        // 6.
        if ob == 9000 {
            if flown >= 30 || t == target {
                let n = (25 - self.cdiv(flown, 15)).max(1);
                if self.random(s16(n)) == 0 && chance == 0 {
                    return (1, out_skip, chance);
                }
                return (0, out_skip, 100 - self.cdiv(100, n));
            } else if roll != 0 {
                return (0, out_skip, 100);
            } else if self.random(100) == 0 && chance == 0 {
                return (1, out_skip, chance);
            }
            return (0, out_skip, 99);
        }
        // 7.
        if ob < 10000 {
            let r = if (x - px(t)).abs().max((y - py(t)).abs()) <= 2 { 1 } else { 0 };
            return (r, out_skip, chance);
        }
        // 8.
        if skip != 0 {
            return (0, out_skip, chance);
        }
        // 9.
        // SPEC GAP: an index 8 × object + direction past the 2056-byte table reads 0.
        let wi = 8 * b as i64 + dir as i64;
        let wn = crate::fanfix::window_byte(wi, get8(unsafe { (*self.w).window }, WINDOW_LEN, wi)) as i8 as i32;
        if wn >= 0x62 && h < 9000 {
            let p = 100 - self.cdiv(flown - 15, 5);
            // (0x57AB7..0x57AC0: only a call that enters with chance 0 can stop here. Line of fire enters with 100.)
            if self.random(100) >= p && chance == 0 {
                return (1, out_skip, chance);
            }
            return (0, out_skip, p);
        }
        // 10.
        if wn < 4 && cap <= 9000 {
            return (0, out_skip, chance);
        }
        // 11.
        if cap <= 9000 || ob <= 10000 {
            (1, out_skip, chance)
        } else {
            (-1, out_skip, chance)
        }
    }

    /// Hit test with the host-op argument list (height, x, y, flown, byte direction, byte skip, cap, target, roll);
    /// the out record (+0 skip, +4 chance i32) gives the starting values and receives the new ones.
    pub(super) fn hit_test_op(&self, args: &[i32], out: &mut [u8]) -> i32 {
        let g = |k: usize| args.get(k).copied().unwrap_or(0);
        let skip0 = out.first().copied().unwrap_or(0);
        let ch0 = if out.len() >= 8 { i32::from_le_bytes([out[4], out[5], out[6], out[7]]) } else { 0 };
        let (r, skip, ch) = self.hit_test(g(0), g(1), g(2), g(3), g(4) as u8, g(5) as u8, g(6), g(7), g(8), skip0, ch0);
        if !out.is_empty() {
            out[0] = skip;
        }
        if out.len() >= 8 {
            out[4..8].copy_from_slice(&ch.to_le_bytes());
        }
        r
    }
}

// ================= Melee hit (shooting.md batch 6, op 77 as a library op) =================
impl Ai {
    fn m_story(&self, i: usize) -> u8 {
        let p = unsafe { (*self.w).story };
        if p.is_null() { 0 } else { unsafe { p.add(i).read() } }
    }

    pub(super) fn melee_hit(&self) {
        let slot = self.slot();
        // 1.
        unsafe {
            (*self.w).active_slot = slot as u8;
            (*self.w).anim_signal = -2;
        }
        let t = s16(self.rd16(0xDC) as i32);
        // #70 F59 (row 49): 1.15m 0x3523F refuses a target tile >= 0xC40 (signed) and goes straight to step 6;
        // 1.13b reads the occupant past the tile map, which can hit a wrong soldier.
        let off_map = crate::fanfix::on(59) && t >= 0xC40;
        let occ = if off_map { NO_SLOT } else { self.a_occupant(t) };
        if occ >= NO_SLOT {
            // 2.
            if !off_map
                && unsafe { (*self.w).cur_sector } as i8 as i32 == self.m_story(0x17) as i8 as i32
                && self.m_story(0x15) == 0
                && self.m_story(0x16) == 0
                && self.tb(t, 1) == 0xAE
            {
                self.call(op::STORY_HIT, &[]);
            }
        } else {
            // 3.
            let o = match Ai::new(self.w, self.h, occ as i32) {
                Some(o) => o,
                None => return,
            };
            o.wr8(0x1C4, 1);
            if slot < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 3, 1, 0]);
            }
            let mut sure = false;
            if occ < 0x28 && slot >= 0x28 {
                sure = o.s8(0x112 + slot as usize) != 1;
            } else if occ >= 0x28 && slot < 0x28 {
                sure = o.s8(0x13A + slot as usize) != 1;
            }
            if o.s8(0x183) < 15 {
                sure = true;
            }
            let (height, chance) = if sure {
                (0x1356, 99)
            } else {
                let c = self.melee_hit_chance(occ as usize, self.rd8(0x1FD)) & 0xFF;
                (s16(self.random(2500) + 2500), c)
            };
            // 4.
            let roll = self.random(100) & 0xFF;
            if roll < chance {
                let it = self.rd8(0);
                let mut d = s16(self.a_weapon(it, 5) as i32 + self.random(31) - 15);
                d = s16(d + (self.s8(0x192) - 1) * 2);
                let lost = (self.rd8(0x184) as i32 - self.rd8(0x183) as i32 - self.rd8(0x185) as i32) & 0xFF;
                d = s16(d + self.cdiv(lost / 2 + self.s8(0x183) - 50, 5));
                d = s16(d + self.cdiv(chance - roll, 5));
                d = s16(self.cdiv(self.s8(2) * d, 100));
                if d < 1 {
                    d = 1;
                }
                self.call(op::TAKE_DAMAGE, &[occ as i32, height, d, 1, slot, NONE as i32]);
                if slot < 8 {
                    self.call(op::STAT_PRACTICE, &[slot, 3, 1, 0]);
                }
                let c = self.s8(2);
                if c > 10 {
                    let mut q = self.cdiv(d, 10) & 0xFF;
                    if it == 0x10 {
                        q >>= 1;
                    }
                    let cap = self.cdiv(c, 10);
                    if q > cap {
                        q = cap & 0xFF;
                    }
                    if q > 1 {
                        self.wr8(2, self.rd8(2).wrapping_sub(self.random(q) as u8));
                    }
                }
            } else {
                // 5.
                if sure && o.s8(0x183) >= 15 && o.s8(0xBA) != 2 {
                    o.hit_alert(slot as usize, 1);
                }
                // #70 F30 (row 20): 1.15m 0x35517 rolls the target's practice only when the attacker is an enemy
                // (slot >= 0x28), so a merc missed by his own side gets no roll.
                if occ < 8 && (slot >= 0x28 || !crate::fanfix::on(30)) {
                    self.call(op::STAT_PRACTICE, &[occ as i32, 2, 1, 0]);
                }
            }
        }
        // 6.
        let c = self.call(op::SHOT_AP, &[slot, t]) as u8 as i32;
        let class = self.a_weapon(self.rd8(0), 0);
        let snd = get16(unsafe { (*self.w).attack_sound }, ATTACK_SOUND_LEN, 2 * class as i64) as i16 as i32;
        self.call(op::SPEND_AP, &[slot, (c + self.s8(0x1FD)) as i8 as i32, snd]);
        self.wr8(0x1BE, 0);
        // #70 F33 (row 25): 1.15m 0x355C2 also clears the queued action +0x1A6 (the knife of a walk-to-stab), so
        // the next click with no one on the tile does not say "No one there".
        if crate::fanfix::on(33) {
            self.wr8(0x1A6, 0);
        }
    }
}

// ---- entry points (shooting.md batch 5) ----
// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_start_attack(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    run(w, h, slot, (), |a| a.start_attack(tile))
}
#[no_mangle]
pub extern "C" fn ja1_attack_go(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.attack_go())
}
#[no_mangle]
pub extern "C" fn ja1_fire_gun(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32) {
    run(w, h, slot, (), |a| a.fire_gun(tile))
}
#[no_mangle]
pub extern "C" fn ja1_chance_to_hit(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, aim: i32) -> i32 {
    run(w, h, slot, 0, |a| a.chance_to_hit(tile, aim as u8))
}
/// The out record (8 bytes) is written as host op 48's: +0 the new skip byte, +4 the chance (i32); its old
/// contents are the starting values ("unchanged unless written").
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn ja1_hit_test(
    w: *mut Ja1World,
    h: *const Ja1Host,
    height: i32,
    x: i32,
    y: i32,
    flown: i32,
    direction: i32,
    skip: i32,
    cap: i32,
    target: i32,
    roll: i32,
    out: *mut u8,
) -> i32 {
    run(w, h, 0, 0, |a| {
        let mut rec = [0u8; 8];
        if !out.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(out, rec.as_mut_ptr(), 8) };
        }
        let r = a.hit_test_op(&[height, x, y, flown, direction, skip, cap, target, roll], &mut rec);
        if !out.is_null() {
            unsafe { std::ptr::copy_nonoverlapping(rec.as_ptr(), out, 8) };
        }
        r
    })
}
#[no_mangle]
pub extern "C" fn ja1_melee_hit(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.melee_hit())
}

impl Ai {
    /// ReviveRoll? 0x401B4 (animation word 0x1EA; combat.md "Getting up after a collapse"): a collapsed soldier rolls
    /// to get up. Every path that passes the first test sets a new animation or kills the soldier, so the script
    /// never steps past word 0x1EA (script 0x3E, the only script with it, ends with word 999 = SET_ANIM 499).
    pub(super) fn revive_roll(&self) {
        let slot = self.slot();
        // 1.
        if self.s8(0x183) <= 0 || self.rd8(0x182) == 0 || self.rd8(0x138) == 0 || self.rd8(0x188) != 0 {
            return;
        }
        // 2.
        self.call(op::MAKE_NOISE, &[self.rd8(0x17A) as i32, s16(self.rd16(0x114) as i32), self.rd8(0x1B3) as i32, 7, 3]);
        // 3.
        let mut kit = false;
        if (0xF..=0x10).contains(&self.rd8(0)) {
            kit = true;
        } else {
            for i in 1..7 {
                if (0xF..=0x10).contains(&self.rd8(12 * POCKETS[i])) {
                    let dex = self.s8(0x18C);
                    if (self.random(100) < dex && self.random(100) < dex)
                        || self.rd8(0x179) >= 0x3C
                        || self.s8(0xBA) == 1
                    {
                        kit = true;
                        self.call(op::SWAP_HAND, &[slot, 0, i as i32]);
                    }
                }
            }
        }
        // 4.
        if self.random(5) == 0 {
            self.call(op::SET_ANIM, &[slot, 0x3D, 1, 1]);
            return;
        }
        // 5.
        let lost = (self.rd8(0x184) as i32 - self.rd8(0x183) as i32 - self.rd8(0x185) as i32) & 0xFF;
        let mut c = (lost / 2 + self.s8(0x183)) * self.s8(0x18A);
        c = self.cdiv(c, 100) - self.cdiv(100 - self.s8(0x18C), 4);
        if kit {
            c += self.cdiv(c * 80, 100);
        }
        if self.rd8(0x179) >= 0x3C || self.s8(0xBA) == 1 {
            c += self.cdiv(c * 40, 100);
        }
        if c > 0x5F {
            c = 0x5F;
        }
        // 6.
        if self.random(100) >= c {
            self.call(op::KILL, &[slot, 0]);
            return;
        }
        // 7.
        self.call(op::SET_ANIM, &[slot, 0x3F, 0, 1]);
        for stat in [4, 3, 2] {
            self.call(op::STAT_PRACTICE, &[slot, stat, if kit { 0x19 } else { 0xA }, 0]);
        }
        if kit {
            self.call(op::SET_ANIM, &[slot, 0x3F, 0, 1]);
            return;
        }
        self.call(op::TAKE_DAMAGE, &[slot, 0x1356, self.random(30), 2, 0x40, NONE as i32]);
        self.call(op::ANIM_STEP, &[slot]);
        self.call(op::STOP_WALK, &[slot, 0]);
    }
}

/// ReviveRoll? 0x401B4 (library op 76 revive_roll; the byte argument is not used).
#[no_mangle]
pub extern "C" fn ja1_revive_roll(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.revive_roll())
}
