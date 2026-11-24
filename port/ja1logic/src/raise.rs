//! Salary raise (docs/spec/strategic.md "Salary raise"; C interface in docs/spec/port_api.md "Salary raise entry
//! point"): `ja1_raise_salary`. A child module of `ai`; host ops 295..300 go through `Ai`.
//! Merc leaves and Close up the team are those of `pay`.
use super::damage::{get16, get8, put16, put8};
use super::pay::{merc_leaves, team_compact};
use super::{Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILES_LEN: usize = 64 * 0x5A;
const ACCOUNTS_LEN: usize = 0xA8;
const RAISE_VOICE: i32 = 295;
const RAISE_ASK: i32 = 296;
const RAISE_OFFER: i32 = 297;
const RAISE_ANSWER: i32 = 298;
const RAISE_DONE: i32 = 299;
const RAISE_REDRAW: i32 = 300;
const SALARY: usize = 0x110;
const OLD_SALARY: usize = 0x112;
const B18D: usize = 0x18D;
const B19C: usize = 0x19C;
const B1D9: usize = 0x1D9;
const PROFILE_ID: usize = 0x179;
const SLOT: usize = 0x17A;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;

struct Ctx {
    w: *mut Ja1World,
    h: *const Ja1Host,
    ai: Ai,
}

impl Ctx {
    fn s(&self, k: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn s_put(&self, k: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn s16(&self, k: i32, off: usize) -> u16 {
        get16(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn s16_put(&self, k: i32, off: usize, v: u16) {
        put16(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn prof(&self, p: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * 0x5A + off as i64)
    }
    fn prof_put(&self, p: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * 0x5A + off as i64, v)
    }
    fn prof16_put(&self, p: i32, off: usize, v: u16) {
        put16(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * 0x5A + off as i64, v)
    }
    fn acc(&self, off: usize) -> i32 {
        get8(unsafe { (*self.w).accounts }, ACCOUNTS_LEN, off as i64) as i32
    }
    fn acc_put(&self, off: usize, v: u8) {
        put8(unsafe { (*self.w).accounts }, ACCOUNTS_LEN, off as i64, v)
    }

    /// AIM rate without "one more" (kind 0 fatality, 1 turnover, 2 non-payment).
    fn rate(&self, kind: i32) -> i32 {
        let d = self.acc(0x04);
        let o = if kind == 0 { d } else { self.acc(0x9D) };
        let mut t = (d / 2 + o) & 255;
        if t == 0 {
            return 0;
        }
        if t < 10 {
            t = 10;
        }
        let n = match kind {
            0 => self.acc(0x00),
            1 => (self.acc(0xA0) + self.acc(0x9C)) & 255,
            _ => self.acc(0xA1),
        };
        if n == 0 {
            return 0;
        }
        let r = ((n * 100) / t) & 255;
        if r > 100 {
            100
        } else {
            r
        }
    }

    /// Reason 0..5; reason 5 says the voice line (op 295) first. Returns (reason, grudge profile).
    fn reason(&self, k: i32) -> (i32, i32) {
        let p = self.s(k, PROFILE_ID) as i32;
        if self.s(k, B19C) == 8 && self.s(k, B1D9) != 0 {
            return (3, -1);
        }
        if self.rate(0) >= self.prof(p, 0x36) as i32 {
            return (1, -1);
        }
        if self.rate(1) >= self.prof(p, 0x37) as i32 {
            return (2, -1);
        }
        if self.rate(2) >= self.prof(p, 0x39) as i32 {
            return (4, -1);
        }
        if self.acc(0x9B) >= self.prof(p, 0x38) as i32 {
            return (3, -1);
        }
        let g = self.prof(p, 0x4D) as i32;
        if g >= 60 {
            return (0, -1);
        }
        self.ai.call(RAISE_VOICE, &[k, g]);
        (5, g)
    }

    /// Answer to offer o: -1 no offer, 0 refused, 1 try again, 2 accepted.
    fn answer(&self, k: i32, o: i16) -> i32 {
        if o < 1 {
            return -1;
        }
        let s = self.s16(self.s(k, SLOT) as i32, SALARY) as i32;
        if s == 0 {
            return 0;
        }
        let mut pct = ((o as i32 - s).wrapping_mul(100) / s) as i16;
        let p = self.s(k, PROFILE_ID) as i32;
        let a = self.prof(p, 0x30) as i16;
        let r = self.prof(p, 0x31) as i16;
        if a == 1 && pct < 1 {
            pct = 1;
        }
        if pct >= a && a != 0 {
            2
        } else if pct < r {
            0
        } else {
            1
        }
    }

    fn run(&self) {
        let mut k = 0i32;
        while k < 8 && self.s(k, IN_CUR_SECTOR) != 0 {
            let p = self.s(k, PROFILE_ID) as i32;
            if self.s(k, HP) == 0 || p >= 60 {
                k += 1;
                continue;
            }
            let (reason, g) = self.reason(k);
            if reason == 0 {
                k += 1;
                continue;
            }
            // 2.
            self.ai.call(RAISE_ASK, &[k, reason, g]);
            let mut o = self.s16(k, SALARY) as i16;
            let mut leaves = true;
            let mut n = 0u32;
            // 3.
            while self.prof(p, 0x32) as u32 > n {
                o = self.ai.call(RAISE_OFFER, &[k, o as i32]) as i16;
                let a = self.answer(k, o);
                if a == 2 {
                    if self.s16(k, OLD_SALARY) < self.s16(k, SALARY) {
                        self.s16_put(k, OLD_SALARY, (o / 2) as u16);
                    } else {
                        self.s16_put(k, OLD_SALARY, o as u16);
                    }
                    self.s16_put(k, SALARY, o as u16);
                    self.prof16_put(p, 0x28, o as u16);
                    leaves = false;
                    match reason {
                        1 => self.prof_put(p, 0x36, 101),
                        2 => self.prof_put(p, 0x37, 101),
                        3 => self.prof_put(p, 0x38, 10),
                        4 => self.prof_put(p, 0x39, 101),
                        _ => self.prof_put(p, 0x4D, 99),
                    }
                }
                if a >= 0 {
                    self.ai.call(RAISE_ANSWER, &[k, a]);
                }
                if a == 1 {
                    n += 1;
                } else {
                    break;
                }
            }
            // 4.
            self.ai.call(RAISE_DONE, &[]);
            if leaves {
                if self.s(k, B19C) == 8 && self.s(k, B1D9) != 0 {
                    self.s_put(k, B18D, 0);
                }
                merc_leaves(self.w, self.h, k, 1);
                self.s_put(k, PROFILE_ID, 99);
                self.s_put(k, IN_CUR_SECTOR, 0);
                self.acc_put(0xA0, (self.acc(0xA0) + 1) as u8);
                team_compact(self.w, self.h, 0);
                k -= 1;
            }
            // 5.
            self.ai.call(RAISE_REDRAW, &[]);
            k += 1;
        }
    }
}

// SPEC GAP: a null world, host, soldier, profile or accounts buffer does nothing.
#[no_mangle]
pub extern "C" fn ja1_raise_salary(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).accounts.is_null() {
            return;
        }
    }
    if let Some(ai) = Ai::new(w, h, 0) {
        let c = Ctx { w, h, ai };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.run()));
    }
}
