//! Slice 2b part 2: the Situation score (docs/spec/ai.md "Situation score": Max AP, Armour, Soldier power,
//! Situation score; C interface in docs/spec/port_api.md).
//!
//! A child module of `ai`, like `search`: it extends the `Ai` view of one soldier. The situation score is library
//! op 20 (opp_strength). Its inner calls are host-only ops: direction (6), sector_value (9) and fire_line (42).
//! Arithmetic is 32-bit signed with C division (Rust `/` truncates toward zero = `cdiv`).
use super::{f, op, Ai, NONE};

const REMEMBER_M_LEN: usize = 16 * 8;
const REMEMBER_E_LEN: usize = 8 * 40;

/// Soldier record offsets used only here (defs/types.toml struct.soldier).
const HELMET: usize = 0x018; // inventory entry: byte 0 = item type, byte 2 = condition
const BODY_ARMOUR: usize = 0x03C;
const VEST_POCKET: usize = 0x048;
const SEES: usize = 0x13A;
const SLOT_BYTE: usize = 0x17A;
const WOUNDS: usize = 0x185;
const AGILITY: usize = 0x18B;
const DEXTERITY: usize = 0x18C;
const MARKSMANSHIP: usize = 0x191;
const LEVEL: usize = 0x192;

fn s16(v: i32) -> i32 {
    v as i16 as i32
}
fn u8v(v: i32) -> i32 {
    v as u8 as i32
}
/// Pixel point of a signed tile number (Search procedures, "Tile parts").
fn px(t: i32) -> i32 {
    (t - (t / 56) * 56) * 16 + 5
}
fn py(t: i32) -> i32 {
    (t / 56) * 14 + 5
}

/// weight(lvl) for lvl −3..4.
fn weight(lvl: i32) -> i32 {
    match lvl {
        -3 => 60,
        -2 => 70,
        -1 => 80,
        0 => 50,
        1 => 100,
        2 => 90,
        3 => 80,
        4 => 70,
        // SPEC GAP: "levels are always in −3..4"; a level outside that range has no weight listed; 0 here.
        _ => 0,
    }
}

impl Ai {
    fn u8_of(&self, r: usize, off: usize) -> i32 {
        self.rd8_of(r, off) as i32
    }
    fn i8_of(&self, r: usize, off: usize) -> i32 {
        self.rd8_of(r, off) as i8 as i32
    }

    /// Max AP of record r.
    pub(super) fn max_ap(&self, r: usize) -> i32 {
        // 1.
        if self.u8_of(r, f::HP) == 0 {
            return 0;
        }
        let (h, mh) = (self.i8_of(r, f::HP), self.i8_of(r, f::HP_MAX));
        // 2. No upper limit.
        let mut ap = u8v(
            (self.i8_of(r, LEVEL) * 10 + self.i8_of(r, AGILITY) * 2 + mh + self.i8_of(r, DEXTERITY) + 10) / 20,
        );
        // 3.
        if h < mh {
            // half = the bandaged part / 2. Fan fix F06 (#70 row 88, 1.15m switch HEALTHY, 1.15m 0x562AA -> 0x3C042):
            // bandages lower the loss ("(mh - h) - half") instead of raising it.
            let half = u8v(self.u8_of(r, f::HP_MAX) - self.u8_of(r, f::HP) - self.u8_of(r, WOUNDS)) / 2;
            let lost = if crate::fanfix::on(6) { (mh - h) - half } else { half + (mh - h) };
            // s8 maxHealth × 3 can be 0: div_zero (op 47), quotient 0.
            let q = self.cdiv(ap * 2 * lost, mh * 3);
            ap = u8v(ap - q);
        }
        // 4.
        let e = self.i8_of(r, f::ENERGY);
        if e < 100 {
            ap = u8v(ap - (100 - e) * ap / 200);
        }
        // 5.
        ap.max(8)
    }

    /// Armour of record r.
    pub(super) fn armour(&self, r: usize) -> i32 {
        let part = |entry: usize, value: fn(u8) -> i32| -> i32 {
            let v = value(self.rd8_of(r, entry));
            self.i8_of(r, entry + 2) * v / 100
        };
        part(BODY_ARMOUR, |t| match t { 0x2D => 20, 0x2E => 30, 0x2F => 40, 0x30 => 60, _ => 0 })
            + part(HELMET, |t| match t { 0x23 => 5, 0x24 => 7, 0x25 => 10, 0x26 => 15, _ => 0 })
            + part(VEST_POCKET, |t| if t == 0x31 { 20 } else { 0 })
    }

    /// Soldier power of record r, rated against tile t (signed 16-bit; NONE = no tile) with flag `flag`.
    pub(super) fn soldier_power(&self, r: usize, t: i32, flag: u8) -> i32 {
        let t = s16(t);
        let has_t = t != NONE as i32;
        // 1.
        if self.u8_of(r, f::IN_CUR_SECTOR) == 0 || self.u8_of(r, f::IN_PLAY) == 0 || self.u8_of(r, f::HP) == 0 {
            return -999;
        }
        // 2.
        let mut v = self.max_ap(r) + self.i8_of(r, f::AP_LEFT) / 2 + self.armour(r) / 4
            + self.i8_of(r, MARKSMANSHIP) / 5
            - (100 - self.i8_of(r, f::HP)) / 10
            - (100 - self.i8_of(r, f::ENERGY)) / 10;
        // 3.
        let it = self.rd8_of(r, f::ITEMS);
        if (1..=14).contains(&it) {
            v += 3 * it as i32;
        } else if it == 0x5D || it == 0x0F {
            v += 2;
        } else if it == 0x10 {
            v += 5;
        } else if self.item_kind(it) == super::KIND_THROW {
            v += 15;
        }
        // 4.
        if has_t {
            let last = self.rd8_of(r, f::LAST_TARGET_TILE) as u16 | (self.rd8_of(r, f::LAST_TARGET_TILE + 1) as u16) << 8;
            if t as u16 == last {
                v += v / 10;
            } else {
                let me = s16(self.rd8_of(r, f::TILE) as i32 | (self.rd8_of(r, f::TILE + 1) as i32) << 8);
                let d = self.call(op::DIRECTION, &[px(me), py(me), px(t), py(t)]) as i32;
                if self.i8_of(r, f::FACING) == d {
                    v += v / 20;
                }
            }
        }
        // 5.
        v = v.wrapping_mul(self.i8_of(r, LEVEL));
        // 6.
        // SPEC GAP: "If s8 health < 15: if v > 0, v = cdiv(v, 19 − s8 health). Otherwise, if F is not 0 ..." —
        // "Otherwise" is read as the else of "s8 health < 15" (not of "v > 0").
        let h = self.i8_of(r, f::HP);
        if h < 15 {
            if v > 0 {
                v /= 19 - h;
            }
        } else if flag != 0 && has_t {
            let slot = self.u8_of(r, SLOT_BYTE);
            let res = self.call(op::FIRE_LINE, &[slot, t, 1]) as i32;
            v = v.wrapping_mul(res) / 100;
        }
        // 7.
        v.max(1)
    }

    /// S's remember byte for opponent index j.
    fn remember(&self, j: usize) -> u8 {
        let (p, len, i) = if self.s >= 0x28 {
            (unsafe { (*self.w).remember_e }, REMEMBER_E_LEN, (self.s - 0x28) * 40 + j)
        } else {
            // SPEC GAP: a merc-side soldier of slot 16..0x27 indexes past the 16 x 8 remember_m table; that reads
            // as 0 (as for last_seen_m in the searches).
            (unsafe { (*self.w).remember_m }, REMEMBER_M_LEN, self.s * 8 + j)
        };
        if p.is_null() || i >= len {
            return 0;
        }
        unsafe { p.add(i).read() }
    }

    /// "Counts": inCurSector ≠ 0, inPlay ≠ 0 and s8 health ≥ 15.
    fn counts(&self, r: usize) -> bool {
        self.u8_of(r, f::IN_CUR_SECTOR) != 0 && self.u8_of(r, f::IN_PLAY) != 0 && self.i8_of(r, f::HP) >= 15
    }

    /// Situation score of this soldier (library op 20 opp_strength), 0..4.
    pub(super) fn situation_score(&self) -> i32 {
        let me = self.s;
        // 1. Sides: (slot, index) lists.
        let (opps, allies): (Vec<(usize, usize)>, Vec<(usize, usize)>) = if me >= 0x28 {
            ((0..16).map(|i| (i, i)).collect(), (0x28..0x30).map(|i| (i, i - 0x28)).collect())
        } else {
            ((0x28..0x30).map(|i| (i, i - 0x28)).collect(), (0..16).map(|i| (i, i)).collect())
        };
        // 3.
        let mut level: Vec<i32> = allies.iter().map(|&(a, _)| self.knowledge(a)).collect();
        let (mut opp_sum, mut ally_sum) = (0i32, 0i32);
        let my = s16(self.rd16(f::TILE) as i32);
        // 4.
        for &(o, j) in &opps {
            if !self.counts(o) {
                continue;
            }
            // a.
            let a = self.s8(SEES + j);
            let b = self.knowledge(o);
            // b.
            let lvl = if a == 0 && b == 0 {
                if self.remember(j) == 0 {
                    continue;
                }
                0
            } else if self.fresher(b, a) != 0 || a == b {
                a
            } else {
                b
            };
            // c.
            let p = s16(self.soldier_power(o, my, 0));
            opp_sum = s16(opp_sum + weight(lvl) * p / 100);
            // d.
            for (n, &(ally, k)) in allies.iter().enumerate() {
                let c = self.i8_of(o, SEES + k);
                if c != 0 && self.counts(ally) && (self.fresher(c, level[n]) != 0 || c == level[n]) {
                    level[n] = c;
                }
            }
        }
        // 5.
        for (n, &(ally, _)) in allies.iter().enumerate() {
            if !self.counts(ally) {
                continue;
            }
            let w = if ally == me { 100 } else { weight(level[n]) };
            let p = s16(self.soldier_power(ally, NONE as i32, 0));
            ally_sum = s16(ally_sum + w * p / 100);
        }
        // 6.
        let ratio = if opp_sum == 0 { 500 } else { s16(ally_sum * 100 / opp_sum) };
        let mut s = match ratio {
            r if r <= 25 => 0,
            r if r <= 50 => 1,
            r if r <= 150 => 2,
            r if r <= 300 => 3,
            _ => 4,
        };
        // 7.
        let sv = s16(self.call(op::SECTOR_VALUE, &[self.w_cur_sector() as i32, 1]) as i32);
        if sv > 50 {
            s += 1;
        }
        if sv < 35 {
            s -= 1;
        }
        // 8. attitude, not orders.
        match self.attitude() {
            1 | 2 => s += 1,
            0 => s -= 1,
            _ => {}
        }
        // 9.
        let e = self.energy();
        if e > 75 {
            s += 1;
        } else {
            if e < 40 {
                s -= 1;
            }
            if e < 10 {
                s -= 1;
            }
        }
        // 10.
        let h = self.s8(f::HP);
        if h > 75 {
            s += 1;
        } else {
            if h < 40 {
                s -= 1;
            }
            if h < 20 {
                s -= 1;
            }
        }
        // 11.
        if self.rd8(f::B1C4) == 0 {
            s += 1;
        }
        // 12.
        if unsafe { (*self.w).santino_spoke } != 0 && me >= 0x28 {
            s -= 2;
        }
        // 13.
        s.clamp(0, 4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn weights_and_pixels() {
        assert_eq!(weight(-3), 60);
        assert_eq!(weight(0), 50);
        assert_eq!(weight(1), 100);
        assert_eq!(weight(5), 0);
        assert_eq!((px(57), py(57)), (21, 19));
        assert_eq!((px(-57), py(-57)), (-11, -9));
    }
}
