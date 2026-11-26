//! Shooting batch 1 (docs/spec/shooting.md; C interface in port_api.md): Launch (library op 81), Projectile step
//! (library op 136), Fire now (library op 39), Shot end, Projectile impact and Throw one. A child module of `ai`:
//! every op goes through `Ai::call` (the host_ops rule); Shot end, Projectile impact and Throw one are plain
//! procedures here, never host calls. World data is read again after every host call.
use super::fire::vt;
use super::search::{px, py, s16, step};
use super::{op, run, Ai, Ja1Host, Ja1World, KIND_THROW, NONE, NO_SLOT};

const NONE_I: i32 = NONE as i32;
const ITEM_TYPES_LEN: usize = 136 * 12;
const WEAPONS_LEN: usize = 256 * 6;
const SILENCER_LEN: usize = 17;
const SHOT_SOUND_LEN: usize = 34;
const IMPACT_NOISE_LEN: usize = 33;
const SHOT_RANGE_LEN: usize = 9;
const CLASS_LEN: usize = 256;
const SILENCER: u8 = 0x5B;
const SFX_CHANNEL: i32 = 0x2B11;

/// Byte i of a world table of `len` bytes; null or out of range reads as 0.
// SPEC GAP: an index outside a table (e.g. shot_range[s8 facing] with a facing above 8 or below 0, an item above
// the table) reads 0.
fn tbl(p: *const u8, len: usize, i: i64) -> u8 {
    if p.is_null() || i < 0 || i as u64 >= len as u64 {
        return 0;
    }
    unsafe { p.add(i as usize).read() }
}

impl Ai {
    fn sh_item_type(&self, item: u8, b: usize) -> u8 {
        tbl(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, item as i64 * 12 + b as i64)
    }
    fn sh_weapon(&self, item: u8, b: usize) -> u8 {
        tbl(unsafe { (*self.w).weapons }, WEAPONS_LEN, item as i64 * 6 + b as i64)
    }
    fn sh_silencer(&self, i: u8) -> u8 {
        tbl(unsafe { (*self.w).silencer }, SILENCER_LEN, i as i64)
    }
    fn sh_t0_class(&self, t: i32) -> u8 {
        tbl(unsafe { (*self.w).t0_class }, CLASS_LEN, self.tb(t, 0) as i64)
    }
    fn sh_story(&self, i: usize) -> u8 {
        let p = unsafe { (*self.w).story };
        if p.is_null() { 0 } else { unsafe { p.add(i).read() } }
    }

    // ---- Throw one (the entry at record offset e) ----
    fn throw_one(&self, e: usize) {
        let c = self.s8(e + 1);
        if c <= 0 {
            return;
        }
        if self.sh_item_type(self.rd8(e), 6) == 1 || c == 1 {
            for k in 0..12 {
                self.wr8(e + k, 0);
            }
            return;
        }
        self.wr8(e + 2, self.rd8(e + 8));
        for k in 1..=(c - 2).max(0) as usize {
            self.wr8(e + 7 + k, self.rd8(e + 8 + k));
        }
        self.wr8(e + c as usize + 6, 0);
        self.wr8(e + 1, self.rd8(e + 1).wrapping_sub(1));
    }

    /// The silenced loudness u8(cdiv(100 − s8 +5, cdiv(100, v − 1)) + 1) (div_zero when a divisor is 0).
    fn silenced(&self, v: u8) -> u8 {
        let q = self.cdiv(100, v as i32 - 1);
        (self.cdiv(100 - self.s8(5), q) + 1) as u8
    }

    // ---- Launch (op 81) ----
    /// A silencer on the gun. Fan fix F43 (#70 row 93; 1.15m LaunchProjectile 0x59187 / ShotEnd 0x56E05 ->
    /// tail 0x8AD5A): it works only at condition >= 10 (signed byte +5).
    fn silencer_on(&self) -> bool {
        self.rd8(3) == SILENCER && (!crate::fanfix::on(43) || self.s8(5) >= 10)
    }

    pub(super) fn launch(&self) {
        let slot = self.slot();
        // 1.
        if self.s8(1) < 0 {
            return;
        }
        // 2.
        self.wr16(0xE4, (s16(self.rd16(0x116) as i32) + 2) as u16);
        self.wr16(0xE6, (s16(self.rd16(0x118) as i32) + 4) as u16);
        let g = |o: usize| s16(self.rd16(o) as i32);
        let dist = (g(0xE4) - g(0xDE)).abs().max((g(0xE6) - g(0xE0)).abs());
        self.wr16(0xC8, 0);
        if dist < 2 {
            return;
        }
        // 3.
        self.wr8(0x1BF, 3);
        self.wr8(0x1F5, self.rd8(0));
        self.wr16(0x1E6, self.rd16(0xDC));
        self.wr8(0x1C6, self.rd8(0x1A1));
        self.wr8(0x1BE, 0);
        if self.rd8(0x1C2) == 0xFE {
            self.wr8(0x1C2, 0x40);
        }
        if self.rd8(0x1C3) != 0 {
            self.wr8(0x1C3, 0);
            self.wr8(0x1C2, 0x40);
        }
        // 4.
        let slope = |d: i32, n: i32| self.cdiv((if d < 0 { -1 } else { 1 }) + self.cdiv(2 * d, n), 2);
        let it = self.rd8(0x1F5);
        if self.sh_item_type(it, 2) == KIND_THROW {
            self.wr16(0xF2, 8500);
            self.wr16(0xCA, if self.rd8(0x1B3) == 3 { 9000 } else { 22000 });
            let half = self.cdiv(dist, 2);
            let e8 = slope(g(0xCA) - g(0xF2), half);
            self.wr16(0xE8, e8 as u16);
            let ea = slope(g(0xE2) - g(0xCA), half);
            self.wr16(0xEA, ea as u16);
            self.wr8(0x1CB, 2);
            if slot < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 3, 2, 0]);
                self.call(op::STAT_PRACTICE, &[slot, 6, 2, 0]);
            }
        } else {
            self.wr16(0xCA, 0);
            self.wr32(0xCC, 15);
            let h = self.random(1001) + 4500;
            self.wr16(0xF2, h as u16);
            let e8 = slope(g(0xE2) - g(0xF2), dist);
            self.wr16(0xE8, e8 as u16);
            self.wr8(0x1CB, self.sh_weapon(self.rd8(0), 4));
            if slot < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 8, 1, 0]);
            }
        }
        // 5.
        let mut dx = self.rd16(0xDE).wrapping_sub(self.rd16(0xE4));
        let mut dy = self.rd16(0xE0).wrapping_sub(self.rd16(0xE6));
        self.wr8(0x1FB, 1);
        self.wr16(0xFA, if (dx as i16) < 0 { 0xFFFF } else { 1 });
        if (dx as i16) < 0 {
            dx = dx.wrapping_neg();
        }
        self.wr16(0xFC, if (dy as i16) < 0 { 0xFFFF } else { 1 });
        if (dy as i16) < 0 {
            dy = dy.wrapping_neg();
        }
        let (major, minor);
        if (dx as i16) < (dy as i16) {
            self.wr16(0xFE, 0);
            self.wr16(0x100, self.rd16(0xFC));
            major = dy;
            minor = dx;
        } else {
            self.wr16(0x100, 0);
            self.wr16(0xFE, self.rd16(0xFA));
            major = dx;
            minor = dy;
        }
        self.wr16(0x104, minor.wrapping_mul(2));
        self.wr16(0xF8, minor.wrapping_mul(2).wrapping_sub(major));
        self.wr16(0x102, minor.wrapping_mul(2).wrapping_sub(major.wrapping_mul(2)));
        // 6.
        unsafe {
            (*self.w).busy = (*self.w).busy.wrapping_add(1);
            (*self.w).busy_timer = 5000;
        }
        // 7.
        if it < 17 {
            if self.silencer_on() {
                let q = self.cdiv(100, self.sh_silencer(it) as i32 - 1);
                let v = ((self.cdiv(100 - self.s8(5), q) + 1) as u8).wrapping_mul(10).wrapping_add(20);
                self.call(op::SFX, &[0x17, SFX_CHANNEL, v.min(120) as i32, 1]);
            } else {
                let p = unsafe { (*self.w).shot_sound };
                let snd = tbl(p, SHOT_SOUND_LEN, 2 * it as i64);
                if snd < 0x63 {
                    let vol = tbl(p, SHOT_SOUND_LEN, 2 * it as i64 + 1) as i8 as i32;
                    self.call(op::SFX, &[snd as i32, SFX_CHANNEL, vol, 1]);
                }
            }
        }
        // 8.
        if slot < 8 || self.sh_item_type(self.rd8(0), 2) == KIND_THROW {
            // a.
            if self.rd8(0) <= 0x0E {
                let n = (self.sh_item_type(self.rd8(0), 5) as i8 as i32) * 2;
                if self.random(s16(n)) == 0 {
                    self.wr8(2, self.rd8(2).wrapping_sub(1));
                }
            }
            // b.
            self.wr8(0x1E4, self.rd8(2));
            // c.
            if self.sh_item_type(self.rd8(0), 2) == KIND_THROW {
                self.throw_one(0);
            } else {
                self.wr8(1, self.rd8(1).wrapping_sub(1));
            }
            // d.
            if self.rd8(1) == 0 {
                if self.rd8(0x1F5) <= 0x0E {
                    self.wr8(0x1CD, 0);
                    self.wr32(0xB4, 2000);
                } else {
                    for k in 0..12 {
                        self.wr8(k, 0);
                    }
                }
            }
            // e.
            if unsafe { (*self.w).selected } as i32 == slot {
                self.call(op::ITEM_COND, &[0, slot, 0, 1]);
            }
        }
        // 9.
        if dist <= 20 {
            self.wr16(0xC8, dist as u16);
            self.wr16(0xE4, self.rd16(0xDE));
            self.wr16(0xE6, self.rd16(0xE0));
            // SPEC GAP: the spec says only that t is read before Shot end; x and y must be read before it too
            // (Shot end sets +0xE4 / +0xE6 to NONE; reading them after gives x = 3129, y = 3133, which the
            // original never passes: ai_diff --only launch --port).
            let t = s16(self.rd16(0xDC) as i32);
            let x = s16(self.rd16(0xE4) as i32) - 8;
            let y = s16(self.rd16(0xE6) as i32) - 4;
            self.shot_end(0, 0);
            self.projectile_impact(t, x, y, 1);
        }
    }

    // ---- Shot end ----
    pub(super) fn shot_end(&self, silent: u8, notify: u8) {
        let slot = self.slot();
        // 1.
        if silent == 0 {
            if self.rd8(0x1D8) < 0x30 {
                self.call(op::SHOT_ALERT, &[slot, notify as i32]);
            }
            let it = self.rd8(0x1F5);
            if it <= 0x0E {
                let mut v = self.sh_silencer(it);
                if self.silencer_on() {
                    v = self.silenced(v);
                }
                let tile = s16(self.rd16(0x114) as i32);
                self.call(op::MAKE_NOISE, &[slot, tile, self.rd8(0x1B3) as i32, v as i32, 5]);
            }
        }
        // 2.
        for o in [0xDC, 0xDE, 0xE0, 0xE4, 0xE6, 0x1E6] {
            self.wr16(o, NONE);
        }
        self.wr8(0x1BF, 0);
        self.wr8(0x1C7, 0);
        self.wr8(0x1FF, 0);
        self.wr8(0x1BC, 1);
        // 3.
        unsafe {
            if (*self.w).busy > 0 {
                (*self.w).busy -= 1;
            }
        }
    }

    // ---- Projectile step (op 136) ----
    pub(super) fn projectile_step(&self) {
        let slot = self.slot();
        let g = |o: usize| s16(self.rd16(o) as i32);
        // 1.
        if self.rd32(0xCC) as i32 > 0 {
            return;
        }
        self.wr32(0xCC, 15);
        // 2.
        let mut prev = vt(g(0xE4), g(0xE6));
        let mut n: u8 = 0;
        let me = g(0x114);
        // 3.
        let mut t;
        loop {
            // a.
            if g(0xF8) < 0 {
                self.wr16(0xE4, self.rd16(0xE4).wrapping_add(self.rd16(0xFE)));
                self.wr16(0xE6, self.rd16(0xE6).wrapping_add(self.rd16(0x100)));
                self.wr16(0xF8, self.rd16(0xF8).wrapping_add(self.rd16(0x104)));
            } else {
                self.wr16(0xE4, self.rd16(0xE4).wrapping_add(self.rd16(0xFA)));
                self.wr16(0xE6, self.rd16(0xE6).wrapping_add(self.rd16(0xFC)));
                self.wr16(0xF8, self.rd16(0xF8).wrapping_add(self.rd16(0x102)));
            }
            // b.
            t = vt(g(0xE4), g(0xE6));
            self.wr16(0xC8, self.rd16(0xC8).wrapping_add(1));
            self.wr16(0xF2, self.rd16(0xF2).wrapping_add(self.rd16(0xE8)));
            if g(0xF2) < 0 {
                self.wr16(0xF2, 0);
            }
            if g(0xCA) != 0 && g(0xF2) > g(0xCA) {
                self.wr16(0xF2, self.rd16(0xCA));
                self.wr16(0xE8, if self.rd8(0x1C9) != 0 { self.rd16(0xEA) } else { 0 });
            }
            // c.
            if t != prev && t != me {
                self.wr8(0x1FB, self.rd8(0x1FB).wrapping_add(1));
                if step(prev, t.wrapping_sub(prev)) != t {
                    self.shot_end(0, 1);
                    self.call(op::FIRE, &[slot]);
                    self.wr16(0xE2, 0);
                    return;
                }
                prev = t;
            }
            // d.
            if t != me {
                let skip = self.rd8(0x1FF);
                if skip != 0 && skip as u16 <= self.rd16(0xC8) {
                    self.wr8(0x1FF, 0);
                }
                // The out record starts as skip = the skip argument, chance = 0 (shooting.md batch 5).
                let mut out = [self.rd8(0x1FF), 0, 0, 0, 0, 0, 0, 0];
                let args = [
                    g(0xF2),
                    g(0xE4),
                    g(0xE6),
                    g(0xC8),
                    self.rd8(0x1C6) as i32,
                    self.rd8(0x1FF) as i32,
                    g(0xCA),
                    g(0xDC),
                    self.s8(0x1BC),
                ];
                let r = self.call_out(op::HIT_TEST, &args, &mut out);
                self.wr8(0x1FF, out[0]);
                if r != 0 {
                    let x = g(0xE4) - 8;
                    let y = g(0xE6) - 6 - self.cdiv(g(0xF2), 600);
                    self.shot_end(0, 0);
                    self.projectile_impact(t, x, y, r as i32);
                    return;
                }
            }
            // e.
            n = n.wrapping_add(1);
            if !((n as i32) < self.s8(0x1CB) || t == me) {
                break;
            }
        }
        // 4.
        if self.rd8(0x1C9) == 0
            && (g(0xE4) == NONE_I
                || tbl(unsafe { (*self.w).shot_range }, SHOT_RANGE_LEN, self.s8(0x1A1) as i64) as i8 as i32 >= g(0xC8)
                || self.tb(t, 1) >= 0x27
                || self.tb(t, 2) >= 0x10)
        {
            self.wr8(0x1C7, 0);
            return;
        }
        // 5.
        self.wr8(0x1C7, 1);
        self.wr16(0xF6, self.rd16(0xE4));
        let h = g(0xF2);
        let thrown = self.rd8(0x1C9) != 0;
        if thrown && h > 18000 {
            self.wr16(0x9E, self.rd16(0xE6).wrapping_sub(18));
        } else {
            let v = g(0xE6) - self.cdiv(h, if thrown { 1000 } else { 400 });
            self.wr16(0x9E, v as u16);
        }
        // 6.
        let q = self.cdiv(h, 1000);
        if q <= 2 {
            self.wr8(0x1C5, 0);
        } else if q <= 22 {
            self.wr8(0x1C5, (0xD2 + (q - 3) / 2) as u8);
        }
    }

    // ---- Fire now (op 39) ----
    pub(super) fn fire_now(&self) {
        // 1.
        if self.rd8(0x1D6) != 0 && matches!(self.rd8(0x1D1), 0x0B | 0x0C | 0x0E | 0x10) {
            self.wr8(0x1D1, 0);
            self.wr16(0x132, NONE);
            self.wr8(0x1D6, 0);
        }
        // 2.
        self.wr16(0xDC, NONE);
        unsafe { (*self.w).active_slot = NO_SLOT };
        // 3.
        if unsafe { (*self.w).turn_moves } != 0 {
            self.call(op::NEXT_TURN, &[1]);
        } else if unsafe { (*self.w).interrupt_go } != 0 {
            self.call(op::MERC_INTERRUPT, &[]);
        }
    }

    // ---- Projectile impact ----
    fn roll_explode(&self, c: i32) -> bool {
        c >= 10 && self.random(100) - self.random(50) < c
    }
    fn impact_effect(&self, t: i32) {
        let r = self.random(6500);
        let f = self.random(2);
        let combat = unsafe { (*self.w).combat } as i8 as i32;
        self.call(op::EFFECT, &[t, self.rd8(0x1C9) as i32, f, r - 2000 * combat, self.slot(), 0]);
    }
    /// "Explosive" of Projectile impact step 2. Returns the tile the caller's later steps use: `t`, or under fan
    /// fix F52 the tile the landed item really went to (1.15m ProjectileImpact keeps LandItem's EAX in EDI).
    fn explosive(&self, t: i32, x: i32, y: i32, hit: i32, it: u8, c: i32) -> i32 {
        let slot = self.slot();
        // SPEC GAP: Roll is evaluated where it is used (not when hit = −1; again for "then if Roll").
        if hit == -1 || self.roll_explode(c) {
            self.call(op::EXPLODE, &[slot, t, self.s8(0x1C9), hit]);
            return t;
        }
        let landed = self.call(op::LAND_ITEM, &[slot, t, it as i32, c, x, y]) as i32;
        let t = if crate::fanfix::on(52) { s16(landed) } else { t };
        if self.roll_explode(c) {
            self.impact_effect(t);
        }
        t
    }
    fn set_tile_effect(&self, t: i32) {
        unsafe { (*self.w).effect_tile = t as i16 };
        self.call(op::TILE_EFFECT, &[t, 0]);
    }

    pub(super) fn projectile_impact(&self, t: i32, x: i32, y: i32, hit: i32) {
        let slot = self.slot();
        let t = s16(t);
        let (mut x, mut y) = (x, y);
        // 1.
        unsafe { (*self.w).active_slot = slot as u8 };
        let it = self.rd8(0x1F5);
        let mut vol = 0x58;
        let loud = if it < 33 {
            tbl(unsafe { (*self.w).impact_noise }, IMPACT_NOISE_LEN, it as i64) as i32
        } else {
            self.random(3)
        };
        self.call(op::MAKE_NOISE, &[slot, t, self.sh_t0_class(t) as i32, loud as u8 as i32, 4]);
        // 2. / 3.
        let o = self.occupant(t as u16);
        let c = self.s8(0x1E4);
        if o != NO_SLOT {
            let o = o as usize;
            // a.
            if unsafe { (*self.w).combat } != 0 {
                self.set_tile_effect(t);
            }
            // b.
            unsafe { (*self.w).in_flight = (*self.w).in_flight.wrapping_add(1) };
            // c.
            if self.rd8_of(o, 0x17A) >= 0x28 || slot >= 0x28 {
                if unsafe { (*self.w).replay_m } == 0 {
                    unsafe { (*self.w).replay_m = 1 };
                    let cs = unsafe { (*self.w).cur_sector } as i8 as i32;
                    self.call(op::REPLAY_EVENT, &[cs, cs, 0x4D]);
                }
                if self.rd8_of(o, 0x17A) >= 0x28 && unsafe { (*self.w).first_hit } as i8 == 1 {
                    unsafe { (*self.w).first_hit = 2 };
                }
            }
            // d.
            if self.rd8(0x1C9) != 0 {
                if self.s8(0x1C9) > 1 {
                    let _ = self.explosive(t, x, y, hit, it, c);
                } else {
                    let d = self.random(4) + 1;
                    self.call(op::TAKE_DAMAGE, &[o as i32, 2500, d, 1, slot, NONE_I]);
                    self.call(op::LAND_ITEM, &[slot, t, 0x5D, 100, x, y]);
                }
                return;
            }
            // e.
            let d = s16(self.call(op::BULLET_DAMAGE, &[slot]) as i32);
            let h = s16(self.rd16(0xF2) as i32);
            self.call(op::TAKE_DAMAGE, &[o as i32, h, d, 1, slot, NONE_I]);
            if slot < 8 {
                self.call(op::STAT_PRACTICE, &[slot, 8, 1, 0]);
            }
            return;
        }
        // 4.
        let st = self.tile_standable(t);
        // a.
        if self.s8(0x1C9) > 0 {
            if self.s8(0x1C9) > 1 {
                let t = self.explosive(t, x, y, hit, it, c);
                if unsafe { (*self.w).combat } != 0 {
                    self.set_tile_effect(t);
                }
            } else {
                self.call(op::LAND_ITEM, &[slot, t, 0x5D, 100, x, y]);
            }
            return;
        }
        // b.
        if slot >= 0x28 && self.rd8(0x1D8) < 8 {
            self.call(op::STAT_PRACTICE, &[self.rd8(0x1D8) as i32, 2, 1, 0]);
        }
        // c.
        if unsafe { (*self.w).cur_sector } as i32 == self.sh_story(0x17) as i8 as i32
            && self.sh_story(0x15) == 0
            && self.sh_story(0x16) == 0
            && self.tb(t, 1) == 0xAE
        {
            self.call(op::STORY_HIT, &[]);
        }
        // d.
        if self.tb(t, 2) >= 0x10 && s16(self.rd16(0x116) as i32) <= 0 {
            unsafe { (*self.w).fx_id = 0 };
        } else {
            let (r, sound);
            if self.tb(t, 1) == 0 && self.sh_t0_class(t) >= 8 {
                r = self.call(op::PUFF, &[t, x, y, 0, 5, self.s8(0x1C9), 1]) as u8;
                sound = 0x11;
            } else {
                if st < 0 {
                    x = px(t) - 8;
                    y = py(t) - 7;
                }
                r = self.call(op::PUFF, &[t, x, y, 1, 5, 0, 1]) as u8;
                if self.tb(t, 1) != 0 {
                    sound = self.random(3) + 0x14;
                    vol = 0x28;
                } else {
                    sound = 0x13;
                }
            }
            unsafe {
                (*self.w).fx_id = r;
                (*self.w).anim_signal = r as i8 as i32;
            }
            self.call(op::SFX, &[sound, SFX_CHANNEL, vol, 1]);
        }
        // e.
        if st < 0 {
            let mut v = self.call(op::DOOR_LOOKUP, &[t]) as i32;
            if v > 100 {
                v -= 70;
            }
            if v < s16(self.call(op::BULLET_DAMAGE, &[slot]) as i32) && self.random(3) == 0 {
                self.call(op::BREAK_OBJECT, &[t, self.tb(t, 1) as i32, 1]);
                self.wr8(0x9D, 1);
                self.call(op::STAT_PRACTICE, &[slot, 8, 2, 0]);
            }
        }
    }
}

// ---- entry points (shooting.md) ----
// SPEC GAP: a slot outside 0..47 (or a null world / host / soldier buffer) does nothing.
#[no_mangle]
pub extern "C" fn ja1_launch(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.launch())
}
#[no_mangle]
pub extern "C" fn ja1_projectile_step(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.projectile_step())
}
#[no_mangle]
pub extern "C" fn ja1_fire_now(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    run(w, h, slot, (), |a| a.fire_now())
}
#[no_mangle]
pub extern "C" fn ja1_shot_end(w: *mut Ja1World, h: *const Ja1Host, slot: i32, silent: i32, notify: i32) {
    run(w, h, slot, (), |a| a.shot_end(silent as u8, notify as u8))
}
#[no_mangle]
pub extern "C" fn ja1_projectile_impact(w: *mut Ja1World, h: *const Ja1Host, slot: i32, tile: i32, x: i32, y: i32, hit: i32) {
    run(w, h, slot, (), |a| a.projectile_impact(s16(tile), x, y, hit))
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    unsafe extern "C" fn seq(ctx: *mut std::os::raw::c_void, _n: i32) -> i32 {
        let v = &mut *(ctx as *mut Vec<i32>);
        v.remove(0)
    }

    /// Roll of Projectile impact (0x5713C: Random(100) - Random(50), cmp with the condition, jge 0x572D3 = no roll):
    /// true only when the difference is below c.
    #[test]
    fn roll_explode_strict_below() {
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        let mut vals: Vec<i32> = vec![30, 0, 29, 0, 9, 0];
        let h = Ja1Host { ctx: &mut vals as *mut Vec<i32> as *mut std::os::raw::c_void, random: Some(seq), call: None,
                          host_ops: 0 };
        let ai = Ai { w: &mut *w, h: &h, s: 0 };
        assert!(!ai.roll_explode(30));
        assert!(ai.roll_explode(30));
        assert!(!ai.roll_explode(9));
        assert_eq!(vals, vec![9, 0]);
    }

    #[test]
    fn guard_tbl_range() {
        // Port rule (SPEC GAP): an index outside the table or a null table reads 0; the byte past the end is 9.
        let buf = [7u8, 1, 2, 3, 4, 9];
        let p = buf[1..].as_ptr();
        assert_eq!(tbl(p, 4, 3), 4);
        assert_eq!(tbl(p, 4, 4), 0);
        assert_eq!(tbl(std::ptr::null(), 4, 4), 0);
        assert_eq!(tbl(p, 4, -1), 0);
    }
}
