//! Enemy moves (docs/spec/strategic.md "Enemy moves"; C interface in docs/spec/port_api.md): `ja1_enemy_pick`
//! (library op 310) and `ja1_enemy_move` (library op 311). A child module of `ai`; Random, op 9 sector_value, op 89
//! replay_event, op 218 resolve_battle and op 329 enemy_walk_in go through `Ai`; Time compression (0) is the clock's.
use super::clock::time_compress;
use super::damage::{get8, put8};
use super::{op, Ai, Ja1Host, Ja1World};
use std::cmp::Ordering;

const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const ORDER_LEN: usize = 61;
const STORY_LEN: usize = 35;
const SPLASH_LEN: usize = 60;
const SLOTS: i64 = 0x96;
const OFFSET: [i32; 4] = [1, 10, -10, -1];
const BONUS: [i32; 4] = [15, 10, -10, -15];
const WALL: [u8; 4] = [4, 8, 2, 0x10];
const EDGE: [i32; 4] = [3, 5, 1, 7];

fn wm<'a>(w: *mut Ja1World) -> &'a mut Ja1World {
    unsafe { &mut *w }
}
/// Byte k of sector s's sector_state record.
fn st(w: *mut Ja1World, s: i32, k: i64) -> u8 {
    get8(wm(w).sector_state, SECTOR_STATE_LEN, 9 * s as i64 + k)
}
fn st_put(w: *mut Ja1World, s: i32, k: i64, v: u8) {
    put8(wm(w).sector_state, SECTOR_STATE_LEN, 9 * s as i64 + k, v)
}
fn rec(w: *mut Ja1World, s: i32, o: i64) -> u8 {
    get8(wm(w).sectors, SECTORS_LEN, 0xB1 * s as i64 + o)
}
fn rec_put(w: *mut Ja1World, s: i32, o: i64, v: u8) {
    put8(wm(w).sectors, SECTORS_LEN, 0xB1 * s as i64 + o, v)
}
fn story(w: *mut Ja1World, i: i64) -> u8 {
    get8(wm(w).story, STORY_LEN, i)
}
fn order(w: *mut Ja1World, i: i64) -> u8 {
    get8(wm(w).sector_order, ORDER_LEN, i)
}
fn order_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(wm(w).sector_order, ORDER_LEN, i, v)
}

// ================= x87 arithmetic (strategic.md Enemy moves, Arithmetic) =================

/// ±m × 2^e (m = 0: zero), m rounded to the mantissa width of the last operation.
#[derive(Clone, Copy)]
struct Fx {
    neg: bool,
    m: u128,
    e: i32,
}

impl Fx {
    /// num / den × 2^e0 rounded to `bits` mantissa bits (to nearest, ties to even). den > 0.
    fn round(neg: bool, num: u128, den: u128, e0: i32, bits: u32) -> Fx {
        if num == 0 {
            return Fx { neg, m: 0, e: 0 };
        }
        let lo = 1u128 << (bits - 1);
        let hi = 1u128 << bits;
        // q = floor(num × 2^k / den) in [2^(bits-1), 2^bits)
        let lz = |v: u128| 128 - v.leading_zeros() as i32;
        let mut k = bits as i32 - 1 + lz(den) - lz(num);
        loop {
            let (n, d) = if k >= 0 { (num << k, den) } else { (num, den << -k) };
            let (q, r) = (n / d, n % d);
            if q >= hi {
                k -= 1;
                continue;
            }
            if q < lo {
                k += 1;
                continue;
            }
            let mut q = q;
            let mut e = e0 - k;
            if 2 * r > d || (2 * r == d && q & 1 == 1) {
                q += 1;
                if q == hi {
                    q = lo;
                    e += 1;
                }
            }
            return Fx { neg, m: q, e };
        }
    }
    fn ratio(n: i64, d: i64) -> Fx {
        Fx::round((n < 0) != (d < 0), n.unsigned_abs() as u128, d.unsigned_abs() as u128, 0, 64)
    }
    fn int(n: i64) -> Fx {
        Fx::ratio(n, 1)
    }
    fn mul(self, o: Fx) -> Fx {
        if self.m == 0 || o.m == 0 {
            return Fx { neg: false, m: 0, e: 0 };
        }
        // both mantissas have at most 64 bits: the product fits in 128
        Fx::round(self.neg != o.neg, self.m * o.m, 1, self.e + o.e, 64)
    }
    /// self + 1/2, rounded to 64 bits. |self| ≥ 2^-70 or 0 (shares: tot × val / sum).
    fn add_half(self) -> Fx {
        if self.m == 0 {
            return Fx::ratio(1, 2);
        }
        if self.e >= -1 || self.e < -120 {
            // SPEC GAP: not reached by the shares (|x| < 2^20 gives e < -1; x ≠ 0 gives |x| ≥ 1/2200)
            return self;
        }
        let half = 1i128 << (-1 - self.e);
        let v = if self.neg { half - self.m as i128 } else { half + self.m as i128 };
        Fx::round(v < 0, v.unsigned_abs(), 1, self.e, 64)
    }
    fn f32(self) -> Fx {
        Fx::round(self.neg, self.m, 1, self.e, 24)
    }
    /// The value truncated toward zero, and the sign of what is cut off (−1, 0, 1).
    fn trunc(self) -> (i64, i32) {
        if self.m == 0 {
            return (0, 0);
        }
        let (t, cut) = if self.e >= 0 {
            (if self.e < 64 { i64::try_from(self.m << self.e).unwrap_or(i64::MAX) } else { i64::MAX }, false)
        } else if -self.e >= 128 {
            (0, true)
        } else {
            let s = -self.e as u32;
            ((self.m >> s) as i64, self.m & ((1u128 << s) - 1) != 0)
        };
        let c = cut as i32;
        if self.neg {
            (-t, -c)
        } else {
            (t, c)
        }
    }
    /// __CHP + fistp dword: truncated toward zero; outside the 32-bit range 0x80000000.
    fn chop(self) -> i32 {
        let (t, _) = self.trunc();
        if t < i32::MIN as i64 || t > i32::MAX as i64 {
            i32::MIN
        } else {
            t as i32
        }
    }
    fn cmp_int(self, k: i64) -> Ordering {
        let (t, c) = self.trunc();
        t.cmp(&k).then(c.cmp(&0))
    }
}

/// Share (tot): tot × (val / (val + v0)) + 0.5 truncated toward zero, the low byte signed; 0 when val + v0 = 0.
fn share(tot: i32, val: i32, v0: i32) -> i8 {
    let sum = val as i64 + v0 as i64;
    if sum == 0 {
        return 0;
    }
    Fx::int(tot as i64).mul(Fx::ratio(val as i64, sum)).add_half().chop() as i8
}

// ================= Helpers =================

/// Story sector test (n): story bytes 9 = 2, 6 = 2, s8 0x0B < 1, n = 0x11 or 0x1B.
fn story_sector(w: *mut Ja1World, n: u8) -> bool {
    story(w, 9) == 2 && story(w, 6) == 2 && (story(w, 0x0B) as i8) < 1 && (n == 0x11 || n == 0x1B)
}
fn merc_weight(w: *mut Ja1World) -> i32 {
    wm(w).guard_skill as i32 / 2 + 50
}
fn guard_weight(w: *mut Ja1World) -> i32 {
    wm(w).guard_skill as i32 / 2 + 40
}
pub(super) fn attack_weight(w: *mut Ja1World, n: u8, f: u8) -> i32 {
    let a = get8(wm(w).supplies, 4, 0) as i32 / 2 + 60;
    if story_sector(w, n) || (n == 0x35 && story(w, 0x0D) as i8 == 1) {
        4 * a
    } else if f != 0 {
        a
    } else {
        a / 2
    }
}

/// Take an enemy (s): 1 = one left the sector.
fn take_enemy(w: *mut Ja1World, s: i32) -> bool {
    for k in (0..8).rev() {
        let v = rec(w, s, SLOTS + k) as i8;
        if v > 0 {
            rec_put(w, s, SLOTS + k, v.wrapping_neg() as u8);
            st_put(w, s, 1, st(w, s, 1).wrapping_sub(1));
            return true;
        }
    }
    false
}

/// Put an enemy (s, edge): 1 = one came into the sector.
fn put_enemy(w: *mut Ja1World, ai: &Ai, s: i32, edge: i32) -> bool {
    let Some(k) = (0..8).find(|&k| (rec(w, s, SLOTS + k) as i8) < 0) else {
        return false;
    };
    if st(w, s, 2) != 0 && ai.call(op::ENEMY_WALK_IN, &[k as i32 + 0x28, edge]) as u8 == 0 {
        return false;
    }
    let v = rec(w, s, SLOTS + k) as i8;
    rec_put(w, s, SLOTS + k, v.wrapping_neg() as u8);
    st_put(w, s, 1, st(w, s, 1).wrapping_add(1));
    true
}

fn time_off(w: *mut Ja1World, h: *const Ja1Host) {
    if wm(w).time_compress != 0 {
        time_compress(w, h, 0);
    }
}

// ================= Enemy pick =================

pub(crate) fn enemy_pick(w: *mut Ja1World, h: *const Ja1Host) -> u8 {
    let ai = Ai { w, h, s: 0 };
    let c = wm(w).enemy_counter.wrapping_add(1);
    wm(w).enemy_counter = c;
    if c > 60 {
        for i in (1..=60i64).rev() {
            let j = if i > 1 { ai.random(i as i32) as u8 as i64 } else { 0 };
            let (a, b) = (order(w, i), order(w, j));
            order_put(w, j, a);
            order_put(w, i, b);
        }
        wm(w).enemy_counter = 0;
    }
    order(w, wm(w).enemy_counter as i64)
}

// ================= Enemy move =================

struct Choice {
    nb: i32,
    f: u8,
    cnt: i8,
    d: usize,
}

pub(crate) fn enemy_move(w: *mut Ja1World, h: *const Ja1Host, src: u8) -> u8 {
    let ai = Ai { w, h, s: 0 };
    let s = src as i32;
    let mut res: u8 = 0;
    // 1.
    if st(w, s, 0) & 1 != 0 {
        return 0;
    }
    // 2.
    if st(w, s, 2) == 0 {
        let keep: i8 = if s == 0 {
            let g = wm(w).santino_word as i32;
            if g < 8 { 8u8.wrapping_sub(g as u8) as i8 } else { 0 }
        } else if rec(w, s, 0xAA) == 0 {
            3
        } else {
            1
        };
        let mut surplus = st(w, s, 1).wrapping_sub(keep as u8) as i8;
        if surplus > 0 {
            let v0 = ai.call(op::SECTOR_VALUE, &[s, 1]) as i16 as i32;
            loop {
                // c. A pass.
                let (mut best, mut last): (i32, i32) = (-100, -100);
                let mut pick: Option<Choice> = None;
                for d in 0..4 {
                    let off_map = match d {
                        0 => s % 10 == 9,
                        1 => s > 49,
                        2 => s < 10,
                        _ => s % 10 == 0,
                    };
                    if off_map {
                        continue;
                    }
                    let nb = (src.wrapping_add(OFFSET[d] as u8)) as i8 as i32;
                    let room = 8u8.wrapping_sub(st(w, nb, 1)) as i8;
                    if room < 1 {
                        continue;
                    }
                    let f = (st(w, s, 0) & WALL[d] == 0 || story_sector(w, nb as u8)) as u8;
                    let sv = ai.call(op::SECTOR_VALUE, &[nb & 0xFF, f as i32]) as i16;
                    if sv == -100 {
                        continue;
                    }
                    let val = sv.wrapping_add(BONUS[d] as i16) as i32;
                    let mut cnt = room.min(surplus);
                    // the score: Some(value) as a float, None = +infinity
                    let score: Option<Fx>;
                    if st(w, nb, 0) & 1 != 0 || st(w, nb, 2) != 0 {
                        // Attack.
                        if surplus < 2 {
                            continue;
                        }
                        cnt = cnt.min(7);
                        let de = (st(w, nb, 2) as i32 * merc_weight(w)
                            + st(w, nb, 4) as i32 * guard_weight(w)
                            + 2 * st(w, nb, 3) as i32) as i16 as i32;
                        if de == 0 {
                            let n = share(st(w, s, 1) as i32, val, v0);
                            if n < 2 {
                                continue;
                            }
                            cnt = cnt.min(n);
                        }
                        let att = (attack_weight(w, src, f) * cnt as i32
                            + attack_weight(w, nb as u8, 1) * st(w, nb, 1) as i32) as i16 as i32;
                        if de + att == 0 {
                            if att == 0 || (att > 0) != (v0 > 0) {
                                continue;
                            }
                            score = None;
                        } else {
                            let x = Fx::ratio(att as i64 * 100, (de + att) as i64);
                            score = Some(x.mul(Fx::ratio(val as i64, v0 as i64)).f32());
                        }
                    } else {
                        // Fill.
                        let mut n = share(st(w, nb, 1) as i32 + st(w, s, 1) as i32, val, v0);
                        if n > 8 {
                            n = 8;
                        }
                        let n = (n as u8).wrapping_sub(st(w, nb, 1)) as i8;
                        if n < 1 {
                            continue;
                        }
                        cnt = cnt.min(n);
                        score = Some(Fx::int((cnt as i64 - 1) * 10 + 50));
                    }
                    let new_best = match score {
                        None => 0, // +infinity: fistp gives 0x80000000, low word 0
                        Some(x) => {
                            if x.cmp_int(50) == Ordering::Less || x.cmp_int(best as i64) != Ordering::Greater {
                                continue;
                            }
                            x.chop() as i16 as i32
                        }
                    };
                    last = best;
                    best = new_best;
                    pick = Some(Choice { nb, f, cnt, d });
                }
                // d.
                if best > -100 {
                    if let Some(c) = &pick {
                        let mut moved: u8 = 0;
                        for _ in 0..c.cnt.max(0) {
                            if c.f == 0 {
                                let go = ai.random(6) < ai.random(get8(wm(w).splash, SPLASH_LEN, s as i64) as i8 as i32)
                                    && ai.random(6) < ai.random(get8(wm(w).splash, SPLASH_LEN, c.nb as i64) as i8 as i32);
                                if !go && ai.random(100) < 75 {
                                    ai.call(op::REPLAY_EVENT, &[s, s, 0x54]);
                                    continue;
                                }
                            }
                            if take_enemy(w, s) && put_enemy(w, &ai, c.nb, EDGE[c.d]) {
                                moved = moved.wrapping_add(1);
                            }
                        }
                        if moved as i8 > 0 {
                            ai.call(op::REPLAY_EVENT, &[s, c.nb, moved.wrapping_add(100) as i8 as i32]);
                            if st(w, c.nb, 2) != 0 {
                                time_off(w, h);
                            }
                            if st(w, c.nb, 0) & 1 == 0 && st(w, c.nb, 2) == 0 {
                                ai.call(op::REPLAY_EVENT, &[s, c.nb, 0x4B]);
                            } else {
                                ai.call(op::REPLAY_EVENT, &[s, c.nb, 0x4C]);
                                if st(w, c.nb, 2) == 0 {
                                    ai.call(op::RESOLVE_BATTLE, &[c.nb & 0xFF, c.f as i32]);
                                } else {
                                    res = res.wrapping_add(moved);
                                }
                            }
                        }
                        surplus = (surplus as u8).wrapping_sub(c.cnt as u8) as i8;
                    }
                }
                // e.
                if last <= -100 || surplus <= keep {
                    break;
                }
            }
        }
    }
    // 3. Santino.
    if s == 0 {
        let mut n = 8u8.wrapping_sub(st(w, 0, 1)) as i8;
        if st(w, 0, 2) != 0 && n > 2 {
            n = 2;
        }
        let g = wm(w).santino_word as i32;
        if n as i32 > g {
            n = g as u8 as i8;
        }
        if n != 0 {
            for _ in 0..n.max(0) {
                if put_enemy(w, &ai, 0, 9) {
                    wm(w).santino_word = wm(w).santino_word.wrapping_sub(1);
                }
            }
            if st(w, 0, 2) != 0 {
                time_off(w, h);
                res = res.wrapping_add(n as u8);
            }
            ai.call(op::REPLAY_EVENT, &[0, 0, (n as u8).wrapping_add(100) as i8 as i32]);
            ai.call(op::REPLAY_EVENT, &[0, 0, 0x57]);
        }
    }
    res
}

#[no_mangle]
pub extern "C" fn ja1_enemy_pick(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| enemy_pick(w, h) as i32)).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ja1_enemy_move(w: *mut Ja1World, h: *const Ja1Host, src: i32) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| enemy_move(w, h, src as u8) as i32)).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x87 rules the original's score math runs on (fistp after __CHP; default rounding = to nearest, ties to even):
    /// a value outside the 32-bit range stores the integer indefinite 0x80000000 (Intel SDM, FIST/FISTP), and an
    /// exact tie rounds to the even mantissa.
    #[test]
    fn fx_chop_range_and_tie_to_even() {
        assert_eq!(Fx::int(i32::MAX as i64).chop(), i32::MAX);
        assert_eq!(Fx::int(-5).chop(), -5);
        assert_eq!(Fx::int(1 << 33).chop(), i32::MIN);
        assert_eq!(Fx::int(-(1 << 33)).chop(), i32::MIN);
        let v = |f: Fx| (f.m as i64) << f.e;
        assert_eq!(v(Fx::round(false, 5, 1, 0, 2)), 4);
        assert_eq!(v(Fx::round(false, 7, 1, 0, 2)), 8);
        assert_eq!(v(Fx::round(false, 11, 1, 0, 3)), 12);
        assert_eq!(v(Fx::round(false, 13, 1, 0, 3)), 12);
    }

    #[test]
    fn x87_rounding() {
        // 1/3 to 64 bits, then a float: 0x3EAAAAAB
        let t = Fx::ratio(1, 3).f32();
        assert_eq!((t.m, t.e), (0xAAAAAB, -25));
        assert_eq!(share(3, 50, 100), 1); // 3 × 1/3 + 0.5 = 1.5 → 1
        assert_eq!(share(5, 50, 50), 3); // 2.5 + 0.5 = 3
        assert_eq!(share(4, 10, -10), 0);
        assert_eq!(Fx::int(-7).add_half().chop(), -6);
        assert_eq!(Fx::ratio(101, 2).cmp_int(50), Ordering::Greater);
        assert_eq!(Fx::int(50).cmp_int(50), Ordering::Equal);
    }
}
