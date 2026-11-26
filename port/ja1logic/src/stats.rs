//! Morning stat changes (docs/spec/strategic.md "Morning stat changes"; C interface in docs/spec/port_api.md):
//! `ja1_stat_changes`. A child module of `ai`; host ops 282..287 go through `Ai`.
use super::damage::{get16, get8, put8};
use super::{Ai, Ja1World, Ja1Host, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROGRESS_LEN: usize = 8 * 0x12;
const CHANGED_LEN: usize = 640;
const XP_BEGIN: i32 = 282;
const XP_COUNT: i32 = 283;
const XP_HEALTH_BAR: i32 = 284;
const XP_SALARY_MSG: i32 = 285;
const XP_END_STAT: i32 = 286;
const XP_END: i32 = 287;
/// Guard for the loops of the level steps (they end after at most 32,769 rounds, strategic.md "Level steps").
const LOOP_CAP: u32 = 1_000_000;

// soldier record offsets (defs/types.toml "soldier").
const SALARY: usize = 0x110;
const SALARY2: usize = 0x112;
const PROFILE_ID: usize = 0x179;
const DUTY: usize = 0x17E;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;
const LEVEL: usize = 0x192;
const HEALED: usize = 0x19A;

/// Field offset and lowest value of stats 1..9.
fn field(stat: i32) -> (usize, i32) {
    match stat {
        1 => (HP, 15),
        2 => (0x18B, 1),
        3 => (0x18C, 1),
        4 => (0x18D, 1),
        5 => (0x18E, 0),
        6 => (0x18F, 0),
        7 => (0x190, 0),
        8 => (0x191, 0),
        _ => (LEVEL, 1),
    }
}

struct Ctx {
    w: *mut Ja1World,
    ai: Option<Ai>,
}

impl Ctx {
    fn sol(&self) -> *mut u8 {
        unsafe { (*self.w).soldiers }
    }
    fn s(&self, k: i32, off: usize) -> u8 {
        get8(self.sol(), SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn si(&self, k: i32, off: usize) -> i32 {
        self.s(k, off) as i8 as i32
    }
    fn s16(&self, k: i32, off: usize) -> i32 {
        get16(self.sol(), SOLDIERS_LEN, k as i64 * REC as i64 + off as i64) as i32
    }
    fn sput(&self, k: i32, off: usize, v: u8) {
        put8(self.sol(), SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn sput16(&self, k: i32, off: usize, v: i32) {
        self.sput(k, off, v as u8);
        self.sput(k, off + 1, (v >> 8) as u8);
    }
    fn op(&self, op: i32, args: &[i32]) {
        if let Some(a) = &self.ai {
            a.call(op, args);
        }
    }
    fn progress(&self, k: i32, stat: i32) -> i32 {
        let p = unsafe { (*self.w).stat_progress };
        get16(p, PROGRESS_LEN, k as i64 * 0x12 + (stat as i64 - 1) * 2) as i16 as i32
    }
    fn set_progress(&self, k: i32, stat: i32, v: i32) {
        let p = unsafe { (*self.w).stat_progress };
        let i = k as i64 * 0x12 + (stat as i64 - 1) * 2;
        put8(p, PROGRESS_LEN, i, v as u8);
        put8(p, PROGRESS_LEN, i + 1, (v >> 8) as u8);
    }
    fn mark(&self, k: i32, stat: i32) {
        let i = self.s(k, PROFILE_ID) as i64 * 10 + stat as i64;
        put8(unsafe { (*self.w).stat_changed }, CHANGED_LEN, i, 1);
    }

    /// Count (k, stat, d): stat 0 = salary2 (u16), else the field as a signed byte.
    fn count(&self, k: i32, stat: i32, d: i32) {
        let d = d as i16 as i32;
        if stat == 0 {
            let value = self.s16(k, SALARY2);
            self.op(XP_COUNT, &[k, 0, value, d]);
            self.sput16(k, SALARY2, value.wrapping_add(d));
        } else {
            let off = field(stat).0;
            let value = self.si(k, off);
            self.op(XP_COUNT, &[k, stat, value, d]);
            self.sput(k, off, value.wrapping_add(d) as u8);
        }
    }

    /// Level steps: returns (d, p).
    fn level_steps(v: i32, p: i32) -> (i32, i32) {
        let mut d = 0i32;
        let mut p = p;
        let mut n = 0;
        if p < 0 {
            while (v + d) * -1000 >= p && n < LOOP_CAP {
                n += 1;
                let s = v + d;
                d = (d - 1) as i16 as i32;
                p = (p + s * 1000) as i16 as i32;
            }
        } else {
            while p >= (v + d) * 1000 && n < LOOP_CAP {
                n += 1;
                let s = v + d;
                d = (d + 1) as i16 as i32;
                p = (p - s * 1000) as i16 as i32;
            }
        }
        if v + d > 10 {
            d = 10 - v;
            p = 0;
        } else if v + d < 1 {
            d = 1 - v;
            p = 0;
        }
        (d as i16 as i32, p)
    }

    fn salary_step(salary: i32, up: bool) -> i32 {
        let f = salary as f64;
        let x = if up { f * 1.5 } else { f * 0.66666666666666663 } as i64;
        let u = x & 0xFFFF;
        let g = if u <= 500 {
            5
        } else if u <= 1000 {
            10
        } else if u <= 1500 {
            25
        } else if u <= 2500 {
            50
        } else if u <= 5000 {
            100
        } else if u <= 10000 {
            500
        } else {
            1000
        };
        let r = u % g;
        (((x - r).rem_euclid(65536) + (2 * r / g) * g) & 0xFFFF) as i32
    }

    fn stat_changes(&self) {
        for k in 0..8 {
            if self.s(k, IN_CUR_SECTOR) == 0 {
                break;
            }
            if self.s(k, HP) == 0 {
                continue;
            }
            for stat in 1..=9 {
                self.op(XP_BEGIN, &[]);
                let mut p = self.progress(k, stat);
                let (off, lowest) = field(stat);
                // Fan fix F03 (#70 row 13, #2 row 15; changelog 1.15z "In case a merc finish a day in incapacitated
                // state (<15 HP), bug caused a wrong calculation of MaxHP", found by DarzaR). 1.13b clamps the
                // health step against the current health (+0x183): a merc below 15 gains maximum health and loses
                // his health progress. 1.15m (0x45C3D: +0x183 -> +0x184) clamps against the maximum health.
                let f03 = stat == 1 && crate::fanfix::on(3);
                let v = if f03 { self.si(k, HP_MAX) } else { self.si(k, off) };
                let mut d;
                if stat == 9 {
                    let (dd, pp) = Self::level_steps(v, p);
                    d = dd;
                    p = pp;
                } else {
                    d = p / 100;
                    p -= 100 * d;
                    if v + d > 100 {
                        d = 100 - v;
                        p = 0;
                    } else if v + d < lowest {
                        d = lowest - v;
                        p = 0;
                    }
                    d = d as i16 as i32;
                }
                self.set_progress(k, stat, p);
                if f03 {
                    // 1.15m 0x45F27..0x45F50 (signed bytes): floor = 15 - health; the count adds
                    // max(max(healed, floor) + step, floor) to the health, so it ends at 15 or more.
                    let floor = (lowest as u8).wrapping_sub(self.s(k, HP)) as i8;
                    self.sput(k, HP_MAX, (self.si(k, HP_MAX) + d) as u8);
                    let mut c = (self.s(k, HEALED) as i8).max(floor);
                    self.sput(k, HEALED, 0);
                    c = c.wrapping_add(d as u8 as i8).max(floor);
                    d = c as i32;
                } else if stat == 1 {
                    self.sput(k, HP_MAX, (self.si(k, HP_MAX) + d) as u8);
                    d = (d + self.s(k, HEALED) as i32) as i16 as i32;
                    self.sput(k, HEALED, 0);
                }
                if d != 0 {
                    self.mark(k, stat);
                }
                self.count(k, stat, d);
                if stat == 1 {
                    self.op(XP_HEALTH_BAR, &[k]);
                }
                if stat == 9 && d != 0 {
                    let up = d > 0;
                    for _ in 0..d.abs() {
                        let sal = Self::salary_step(self.s16(k, SALARY), up);
                        self.sput16(k, SALARY, sal);
                    }
                    let sal = self.s16(k, SALARY);
                    self.op(XP_SALARY_MSG, &[k, up as i32, self.si(k, LEVEL), sal]);
                    let salary = self.s16(k, SALARY);
                    let duty = self.s(k, DUTY);
                    let base = if duty == 1 || duty == 4 { (salary + 1) / 2 } else { salary };
                    let e = (base - self.s16(k, SALARY2)) as i16 as i32;
                    if e != 0 {
                        self.mark(k, 0);
                    }
                    self.count(k, 0, e);
                }
                self.op(XP_END_STAT, &[]);
            }
        }
        self.op(XP_END, &[]);
    }
}

#[no_mangle]
pub extern "C" fn ja1_stat_changes(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || unsafe { (*w).soldiers.is_null() } {
        return;
    }
    let c = Ctx { w, ai: Ai::new(w, h, 0) };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.stat_changes()));
}
