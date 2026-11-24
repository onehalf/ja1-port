//! Morning duties (docs/spec/strategic.md "Morning duties"; C interface in docs/spec/port_api.md "Strategic
//! entry points"): `ja1_morning_duties`. A child module of `ai`; host op 281 `practice` and the host random go
//! through `Ai`. Bytes are unsigned unless stated; sums are i32 with wrap-around; divisions truncate toward 0.
use super::damage::{get8, put8};
use super::{Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const ITEM: i64 = 12;
const ITEM_TYPES_LEN: usize = 136 * 12;
const PRACTICE: i32 = 281;

// soldier record offsets (defs/types.toml "soldier").
const SLOT: usize = 0x17A;
const DUTY: usize = 0x17E;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;
const B188: usize = 0x188;
const AGILITY: usize = 0x18B;
const DEXTERITY: usize = 0x18C;
const WISDOM: usize = 0x18D;
const MEDICAL: usize = 0x18E;
const MECHANICAL: usize = 0x190;
const MARKSMANSHIP: usize = 0x191;
const LEVEL: usize = 0x192;
const TRAIN_WHAT: usize = 0x199;
const HEALED: usize = 0x19A;

struct Ctx {
    w: *mut Ja1World,
    ai: Option<Ai>,
}

impl Ctx {
    fn sol(&self) -> *mut u8 {
        unsafe { (*self.w).soldiers }
    }
    fn rd(&self, abs: i64) -> u8 {
        get8(self.sol(), SOLDIERS_LEN, abs)
    }
    fn wr(&self, abs: i64, v: u8) {
        put8(self.sol(), SOLDIERS_LEN, abs, v)
    }
    fn s(&self, k: i32, off: usize) -> u8 {
        self.rd(k as i64 * REC as i64 + off as i64)
    }
    fn si(&self, k: i32, off: usize) -> i32 {
        self.s(k, off) as i8 as i32
    }
    fn sput(&self, k: i32, off: usize, v: u8) {
        self.wr(k as i64 * REC as i64 + off as i64, v)
    }
    /// Offset of place i of soldier k.
    fn pl(k: i32, i: usize) -> i64 {
        k as i64 * REC as i64 + i as i64 * ITEM
    }
    fn random(&self, n: i32) -> i32 {
        self.ai.as_ref().map_or(0, |a| a.random(n))
    }
    fn practice(&self, k: i32, stat: i32, amount: i32, flag: i32) {
        if let Some(a) = &self.ai {
            a.call(PRACTICE, &[k, stat, amount as i16 as i32, flag]);
        }
    }
    fn itype(&self, t: u8, off: i64) -> u8 {
        get8(unsafe { (*self.w).item_types }, ITEM_TYPES_LEN, t as i64 * 12 + off)
    }

    fn need(&self, k: i32) -> i32 {
        self.si(k, HP_MAX).wrapping_sub(self.si(k, HP).wrapping_add(self.s(k, HEALED) as i32))
    }
    fn healed_inc(&self, k: i32) {
        self.sput(k, HEALED, self.s(k, HEALED).wrapping_add(1));
    }

    fn weight(t: u8) -> i32 {
        match t {
            0x49 => 1,
            0x4A => 2,
            _ => 0,
        }
    }

    fn med_points(&self, k: i32) -> i32 {
        let mut sum = 0i32;
        for i in 0..12 {
            let b = Self::pl(k, i);
            let m = Self::weight(self.rd(b));
            if m == 0 {
                continue;
            }
            let n = self.rd(b + 1) as i8 as i32;
            for j in 0..n.max(0) {
                let cond = if j == 0 { self.rd(b + 2) } else { self.rd(b + 7 + j as i64) } as i8 as i32;
                sum = sum.wrapping_add(cond.wrapping_mul(m));
            }
        }
        sum
    }

    fn budget(&self, k: i32) -> i32 {
        let med = self.med_points(k);
        let t = (100i32.wrapping_sub(self.si(k, DEXTERITY))).wrapping_mul(20) / 100;
        let cap = (self.si(k, WISDOM).wrapping_sub(t).wrapping_mul(self.si(k, MEDICAL)) / 100).wrapping_mul(2);
        med.min(cap)
    }

    fn doctor(&self, k: i32) -> i32 {
        let mut a = self.budget(k);
        if a == 0 {
            return 0;
        }
        let mut own = 0i32;
        if a < 0 && self.si(k, HP_MAX) < self.si(k, HP) {
            // Need never reaches 0 (healed is a byte): the original runs about 2^32 rounds.
            self.stop_point();
            return 0;
        }
        while self.need(k) != 0 && a != 0 {
            a = a.wrapping_sub(1);
            own += 1;
            self.healed_inc(k);
        }
        if own != 0 {
            self.practice(k, 5, own / 3, 0);
        }
        if a == 0 {
            return own;
        }
        let me = self.s(k, SLOT) as i32;
        let patients: Vec<i32> =
            (0..8).filter(|&j| j != me && matches!(self.s(j, DUTY), 3 | 4)).collect();
        if patients.is_empty() {
            return own;
        }
        let per = a / patients.len() as i32;
        let mut done = 0i32;
        for &p in &patients {
            let mut n = 0i32;
            while a != 0 && self.need(p) != 0 && n < per {
                a = a.wrapping_sub(1);
                done += 1;
                n += 1;
                self.healed_inc(p);
            }
        }
        if a != 0 {
            for &p in &patients {
                if a != 0 && self.need(p) != 0 {
                    a = a.wrapping_sub(1);
                    done += 1;
                    self.healed_inc(p);
                }
            }
        }
        if done != 0 {
            self.practice(k, 5, done / 3, 0);
        }
        own + done
    }

    fn use_supplies(&self, k: i32, used: i32, total: i32) {
        let mut used = used.min(total);
        for i in 0..12 {
            let b = Self::pl(k, i);
            let m = Self::weight(self.rd(b));
            if m == 0 {
                continue;
            }
            let n = self.rd(b + 1) as i8 as i32;
            let mut c = n - 1;
            while c >= 0 {
                if c == 0 {
                    let v = self.rd(b + 2) as i8 as i32;
                    if v.wrapping_mul(m) <= used {
                        (0..ITEM).for_each(|x| self.wr(b + x, 0));
                        used = used.wrapping_sub(v.wrapping_mul(m));
                    } else {
                        self.wr(b + 2, v.wrapping_sub(used / m) as u8);
                        used = 0;
                    }
                } else {
                    let o = b + 7 + c as i64;
                    let v = self.rd(o) as i8 as i32;
                    if v.wrapping_mul(m) <= used {
                        self.wr(b + 1, self.rd(b + 1).wrapping_sub(1));
                        used = used.wrapping_sub(v.wrapping_mul(m));
                        self.wr(o, 0);
                    } else {
                        self.wr(o, v.wrapping_sub(used / m) as u8);
                        used = 0;
                    }
                }
                c -= 1;
            }
        }
    }

    /// A loop that the original runs about 2^32 times (strategic.md "Repair"): host call 47, the step stops.
    fn stop_point(&self) {
        if let Some(ai) = &self.ai {
            ai.call(47, &[]);
        }
    }

    fn repair(&self, k: i32) {
        let mech = self.si(k, MECHANICAL);
        let dex = self.si(k, DEXTERITY);
        let lvl = self.si(k, LEVEL);
        let v = mech
            .wrapping_mul(dex)
            .wrapping_add(lvl.wrapping_sub(1).wrapping_mul(5))
            .wrapping_mul(3)
            / 200;
        let b0 = Self::pl(k, 0);
        let t = if self.rd(b0) == 0x4B { self.rd(b0 + 2) as i8 as i32 } else { 0 };
        let mut p = t.wrapping_mul(v) / 100;
        if p == 0 {
            return;
        }
        let mut done = 0i32;
        for i in [1usize, 7, 8, 9, 10, 11] {
            let b = Self::pl(k, i);
            let ty = self.rd(b);
            if self.itype(ty, 9) == 0 {
                continue;
            }
            let q = self.rd(b + 1) as i8 as i32;
            if q < 0 && p >= 2 {
                self.wr(b + 1, q.wrapping_neg() as u8);
                done += 2;
                p -= 2;
            }
            let mut n = 100 - self.rd(b + 2) as i8 as i32;
            if n < 0 && p < 0 {
                return self.stop_point();
            }
            while n != 0 && p != 0 {
                n -= 1;
                done += 1;
                p -= 1;
                self.wr(b + 2, self.rd(b + 2).wrapping_add(1));
            }
            if self.itype(ty, 6) > 1 && self.rd(b + 1) as i8 as i32 > 1 {
                let last = self.rd(b + 1) as i8 as i32 - 2;
                for c in 0..=last {
                    let o = b + 8 + c as i64;
                    let mut n = 100 - self.rd(o) as i8 as i32;
                    if n < 0 && p < 0 {
                        return self.stop_point();
                    }
                    while n != 0 && p != 0 {
                        self.wr(o, self.rd(o).wrapping_add(1));
                        n -= 1;
                        done += 1;
                        p -= 1;
                    }
                }
            }
        }
        if done == 0 {
            return;
        }
        self.practice(k, 7, done, 0);
        let r = self.random(100);
        if r < done && self.rd(b0 + 2) as i8 as i32 > 10 {
            let x = self.random(done / 15 + 1);
            let c = self.rd(b0 + 2).wrapping_sub(x.wrapping_add(1) as u8);
            self.wr(b0 + 2, c);
            if (c as i8) < 0 {
                (0..ITEM).for_each(|j| self.wr(b0 + j, 0));
            }
        }
    }

    fn train(&self, k: i32) {
        let amt = |f: usize, m: i32| (100 - self.si(k, f)).wrapping_mul(m);
        match self.s(k, TRAIN_WHAT) {
            0 => {
                self.practice(k, 1, amt(HP_MAX, 4), 1);
                self.practice(k, 2, amt(AGILITY, 4), 1);
                self.practice(k, 3, amt(DEXTERITY, 4), 1);
            }
            1 => self.practice(k, 5, amt(MEDICAL, 12), 1),
            3 => self.practice(k, 7, amt(MECHANICAL, 12), 1),
            4 => self.practice(k, 8, amt(MARKSMANSHIP, 12), 1),
            2 => self.practice(k, 6, amt(LEVEL, 12), 1),
            _ => {}
        }
    }

    fn morning(&self) {
        for k in 0..8 {
            let d = self.si(k, HP_MAX) - self.si(k, HP);
            if d == 0 || !matches!(self.s(k, DUTY), 1 | 4) {
                continue;
            }
            let n = (self.si(k, HP_MAX) / 10) as u8 as i32;
            if n == 0 {
                continue;
            }
            let a = self.random(n);
            let a = a.wrapping_add(1);
            let b = self.random(n);
            let mut r = a.wrapping_add(b).wrapping_add(1) as u8;
            if r as i32 > d {
                r = d as u8;
            }
            self.sput(k, HEALED, self.s(k, HEALED).wrapping_add(r));
        }
        for k in 0..8 {
            match self.s(k, DUTY) {
                0 | 7 => self.practice(k, 1, 100 - self.si(k, HP_MAX), 0),
                2 => self.train(k),
                3 => {
                    let total = self.med_points(k);
                    let used = self.doctor(k);
                    self.use_supplies(k, used, total);
                }
                5 => self.repair(k),
                _ => {}
            }
            if self.s(k, IN_CUR_SECTOR) != 0 && self.s(k, B188) == 0 && self.si(k, HP) >= 15 {
                self.practice(k, 9, 0x19, 0);
            }
        }
    }
}

// SPEC GAP: a null world or soldier buffer does nothing. A null host skips the host calls (random gives 0).
#[no_mangle]
pub extern "C" fn ja1_morning_duties(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || unsafe { (*w).soldiers.is_null() } {
        return;
    }
    let c = Ctx { w, ai: Ai::new(w, h, 0) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.morning()));
}
