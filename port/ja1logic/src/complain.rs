//! AIM rate complaints (docs/spec/strategic.md "AIM rate complaints"; C interface in docs/spec/port_api.md):
//! `ja1_complain_pick`, `ja1_complain`. A child module of `ai`; host ops 288..290 and the host random go through `Ai`.
use super::damage::{get8, put8};
use super::{Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILES_LEN: usize = 64 * 0x5A;
const ACCOUNTS_LEN: usize = 0xA8;
const CP_BEGIN: i32 = 288;
const CP_QUOTE: i32 = 289;
const CP_END: i32 = 290;
const HATE_QUOTE: i32 = 291;
/// Guard for the random redraw loop (a host that never returns a free index).
const DRAW_CAP: u32 = 1_000_000;

const SLOT: usize = 0x17A;
const PROFILE_ID: usize = 0x179;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const FLAG_19C: usize = 0x19C;

struct Ctx {
    w: *mut Ja1World,
    ai: Ai,
}

impl Ctx {
    fn s(&self, k: i32, off: usize) -> u8 {
        let p = unsafe { (*self.w).soldiers };
        get8(p, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn prof(&self, p: i32, off: usize) -> u8 {
        let b = unsafe { (*self.w).profiles };
        get8(b, PROFILES_LEN, p as i64 * 0x5A + off as i64)
    }
    fn acc(&self, off: usize) -> i32 {
        let b = unsafe { (*self.w).accounts };
        get8(b, ACCOUNTS_LEN, off as i64) as i32
    }

    /// AIM rate with one more (kind 0 fatality, 1 turnover, 2 non-payment).
    fn rate(&self, kind: i32) -> i32 {
        let d = (self.acc(0x04) + 1) & 255;
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
        let n = (n + 1) & 255;
        let r = ((n * 100) / t) & 255;
        if r > 100 {
            100
        } else {
            r
        }
    }

    fn hated(&self) {
        for k in 0..8 {
            if self.s(k, IN_CUR_SECTOR) == 0 {
                break;
            }
            let p = self.s(k, PROFILE_ID) as i32;
            if self.s(k, HP) == 0 || p >= 60 {
                continue;
            }
            let d = self.prof(p, 0x3A);
            let n = (0..8)
                .filter(|&j| self.s(j, IN_CUR_SECTOR) != 0 && self.s(j, PROFILE_ID) == d)
                .count();
            if n != 0 {
                let b = unsafe { (*self.w).profiles };
                put8(b, PROFILES_LEN, p as i64 * 0x5A + 0x3C, d);
                self.ai.call(HATE_QUOTE, &[k, d as i32]);
            }
        }
    }

    fn complain(&self, slot: i32) {
        let sp = slot & 255;
        self.ai.call(CP_BEGIN, &[sp]);
        let mut said = 0;
        let fat = self.rate(0);
        let turn = self.rate(1);
        let nonpay = self.rate(2);
        let bad = self.acc(0x9B);
        for k in 0..8 {
            if self.s(k, IN_CUR_SECTOR) == 0 || self.s(k, HP) == 0 {
                break;
            }
            let p = self.s(k, PROFILE_ID) as i32;
            if self.s(k, SLOT) as i32 == sp || p >= 60 {
                continue;
            }
            let quote = |q: i32, said: &mut i32| {
                self.ai.call(CP_QUOTE, &[sp, q, p, 1 - *said]);
                *said = 1;
            };
            if fat >= self.prof(p, 0x36) as i32 {
                quote(8, &mut said);
            }
            if turn >= self.prof(p, 0x37) as i32 {
                quote(9, &mut said);
            }
            if nonpay >= self.prof(p, 0x39) as i32 {
                quote(11, &mut said);
            }
            if bad != 0 && bad + 1 >= self.prof(p, 0x38) as i32 {
                quote(10, &mut said);
            }
        }
        if said == 1 {
            self.ai.call(CP_END, &[]);
        }
    }

    fn pick(&self) {
        let mut picked = [false; 8];
        let mut draws = 0u32;
        for _ in 0..8 {
            let r = loop {
                if draws >= DRAW_CAP {
                    return;
                }
                draws += 1;
                let r = self.ai.random(8);
                if (0..8).contains(&r) && !picked[r as usize] {
                    break r;
                }
            };
            picked[r as usize] = true;
            if self.s(r, IN_CUR_SECTOR) != 0
                && self.s(r, HP) != 0
                && self.s(r, FLAG_19C) == 1
                && self.s(r, PROFILE_ID) <= 59
            {
                self.complain(self.s(r, SLOT) as i32);
                return;
            }
        }
    }
}

fn ctx(w: *mut Ja1World, h: *const Ja1Host) -> Option<Ctx> {
    if w.is_null() {
        return None;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).accounts.is_null() {
            return None;
        }
    }
    Ai::new(w, h, 0).map(|ai| Ctx { w, ai })
}

#[no_mangle]
pub extern "C" fn ja1_complain_pick(w: *mut Ja1World, h: *const Ja1Host) {
    if let Some(c) = ctx(w, h) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.pick()));
    }
}

#[no_mangle]
pub extern "C" fn ja1_complain(w: *mut Ja1World, h: *const Ja1Host, slot: i32) {
    if let Some(c) = ctx(w, h) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.complain(slot)));
    }
}

#[no_mangle]
pub extern "C" fn ja1_hated_check(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() {
            return;
        }
    }
    if let Some(ai) = Ai::new(w, h, 0) {
        let c = Ctx { w, ai };
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| c.hated()));
    }
}
