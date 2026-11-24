//! The A.I.M. screen's game logic (docs/spec/strategic.md "The A.I.M. screen"; C interface in docs/spec/port_api.md
//! "A.I.M. entry points"): `ja1_aim_open`, `ja1_aim_browse`, `ja1_aim_refusal`, `ja1_aim_hire`,
//! `ja1_aim_cancel_pending`, `ja1_aim_pay`. A child module of `ai`; host ops 337 `aim_remark` and 338 `aim_removed`
//! and the host random go through `Ai`. Merc join is that of `join`, Fire the merc that of `dismiss`, Close up the
//! team that of `pay`. Drawing, quotes, messages and questions stay in the host (the front-end's A.I.M. screen).
use super::damage::{get16, get8, put16, put8};
use super::dismiss::ja1_fire_merc_core;
use super::join::ja1_merc_join;
use super::ledger::{add, BALANCE};
use super::pay::team_compact;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: i64 = 0x5A;
const PROFILES_LEN: usize = 64 * 0x5A;
const ACCOUNTS_LEN: usize = 0xA8;

const A_DEATHS: i64 = 0x00;
const A_DAY: i64 = 0x04;
const A_ARRIVALS: i64 = 0x8C;
const A_SUCCESS: i64 = 0x9A;
const A_BAD_DAYS: i64 = 0x9B;
const A_TERMINATIONS: i64 = 0x9C;
const A_HIRINGS: i64 = 0x9D;
const A_RESIGNATIONS: i64 = 0xA0;
const A_NON_PAYMENTS: i64 = 0xA1;
const A_LINK: i64 = 0xA3;
const A_PAGE: i64 = 0xA4;

const PROFILE_ID: usize = 0x179;
const IN_CUR_SECTOR: usize = 0x182;
const B19D: usize = 0x19D;
const DAY_HIRED: usize = 0x1E5;

const P_WISDOM: usize = 0x2E;
const P_AVAIL_DAY: usize = 0x35;
const P_MAX_FATAL: usize = 0x36;
const P_MAX_TURNOVER: usize = 0x37;
const P_MAX_BAD_DAYS: usize = 0x38;
const P_MAX_NON_PAY: usize = 0x39;
const P_HATES: usize = 0x3B;
const P_B3C: usize = 0x3C;
const P_FIRED: usize = 0x3D;
const P_B3E: usize = 0x3E;
const P_TEAM_STATUS: usize = 0x4B;
const P_B4C: usize = 0x4C;
const P_B4D: usize = 0x4D;

/// The helicopter charge by difficulty 0 / 1 / 2.
const HELI: [i32; 3] = [600, 800, 1000];

struct Aim {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

impl Aim {
    fn sb(&self, k: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn sb_put(&self, k: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64, v)
    }
    fn pb(&self, p: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off as i64)
    }
    fn pb_put(&self, p: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off as i64, v)
    }
    fn acc(&self, off: i64) -> u8 {
        get8(unsafe { (*self.w).accounts }, ACCOUNTS_LEN, off)
    }
    fn acc_put(&self, off: i64, v: u8) {
        put8(unsafe { (*self.w).accounts }, ACCOUNTS_LEN, off, v)
    }
    fn acc32(&self, off: i64) -> i32 {
        let p = unsafe { (*self.w).accounts };
        (get16(p, ACCOUNTS_LEN, off) as u32 | (get16(p, ACCOUNTS_LEN, off + 2) as u32) << 16) as i32
    }
    fn on_team(&self, p: i32) -> bool {
        (0..8).any(|k| self.sb(k, IN_CUR_SECTOR) != 0 && self.sb(k, PROFILE_ID) as i32 == p)
    }
    fn team_size(&self) -> i32 {
        (0..8).take_while(|&k| self.sb(k, IN_CUR_SECTOR) != 0).count() as i32
    }

    /// AimScreen 0x2011C opening block (0x2011C..0x20199; steps 1 and 2): the face timer t (returned), then the
    /// three "on assignment" picks. diff-tested (aim_diff --port open).
    fn open(&self) -> i32 {
        let t = self.a.random(1000) + 500;
        for _ in 0..3 {
            let r = self.a.random(80);
            if r < 60 && !self.on_team(r) {
                let s = self.a.random(5);
                self.pb_put(r, P_TEAM_STATUS, s as u8);
            }
        }
        t
    }

    /// Browsing from q with step d; P = the result (`accounts` +0xA4).
    fn browse(&self, q: i32, d: i32) -> i32 {
        let mut q = q;
        // SPEC GAP: the spec loops until a profile with wisdom ≠ 0; with none the port stops after 60 steps (−1).
        for _ in 0..60 {
            q += d;
            if q >= 60 {
                q = 0;
            } else if q < 0 {
                q = 59;
            }
            if self.pb(q, P_WISDOM) != 0 {
                self.acc_put(A_PAGE, q as u8);
                return q;
            }
        }
        -1
    }

    /// The AIM rate without "one more" (strategic.md Salary raise).
    fn rate(&self, kind: i32) -> u8 {
        let d = self.acc(A_DAY);
        let o = if kind == 0 { d } else { self.acc(A_HIRINGS) };
        let mut t = (d / 2).wrapping_add(o);
        if t == 0 {
            return 0;
        }
        if t < 10 {
            t = 10;
        }
        let n = match kind {
            0 => self.acc(A_DEATHS),
            1 => self.acc(A_RESIGNATIONS).wrapping_add(self.acc(A_TERMINATIONS)),
            _ => self.acc(A_NON_PAYMENTS),
        };
        if n == 0 {
            return 0;
        }
        let r = ((n as u32 * 100) / t as u32) as u8;
        r.min(100)
    }

    /// The refusal reason of profile p (0 = signs).
    fn refusal(&self, p: i32) -> i32 {
        let mut r = 0;
        let mut m = 0x40u8;
        if get16(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + P_B3E as i64) != 0 {
            r = 12;
        }
        if self.pb(p, P_FIRED) != 0 {
            r = 9;
        }
        if r == 0 && self.pb(p, P_B4D) < 60 {
            r = 13;
            m = self.pb(p, P_B4D);
        }
        if r == 0 && self.pb(p, P_HATES) < 60 {
            r = 7;
            m = self.pb(p, P_HATES);
        }
        if r == 0 {
            let a = self.pb(p, P_AVAIL_DAY) as i32;
            let free = if a <= 100 { a <= self.acc32(A_DAY) } else { a - 100 <= self.acc(A_SUCCESS) as i32 };
            if !free {
                r = 1;
            }
        }
        if r == 0 && self.rate(0) >= self.pb(p, P_MAX_FATAL) {
            r = 2;
        }
        if r == 0 && self.rate(1) >= self.pb(p, P_MAX_TURNOVER) {
            r = 3;
        }
        if r == 0 && self.rate(2) >= self.pb(p, P_MAX_NON_PAY) {
            r = 5;
        }
        if r == 0 && self.pb(p, P_MAX_BAD_DAYS) <= self.acc(A_BAD_DAYS) {
            r = 4;
        }
        if r == 0 && self.on_team(self.pb(p, P_B3C) as i32) {
            r = 8;
        }
        if r == 0 && self.pb(p, P_B4C) < 60 {
            r = 6;
            m = self.pb(p, P_B4C);
        }
        if m < 0x40 {
            self.a.call(op::AIM_REMARK, &[p, m as i32]);
        }
        r
    }

    /// Hire of profile p (without the quote): the team slot k, or −1 when the team is full.
    fn hire(&self, p: i32) -> i32 {
        let k = self.team_size();
        if k >= 8 {
            return -1;
        }
        self.pb_put(p, P_TEAM_STATUS, 50);
        let k = ja1_merc_join(self.w, self.h, p);
        if k < 0 {
            return -1;
        }
        self.sb_put(k, B19D, 1);
        self.sb_put(k, DAY_HIRED, (self.acc(A_DAY)).wrapping_add(1));
        k
    }

    /// Remove every merc hired in this visit (leaving without the helicopter charge).
    fn cancel_pending(&self) {
        let mut k = 0;
        // SPEC GAP: guard against a record that stays after Fire the merc (the original would loop forever).
        let mut left = 64;
        while k < 8 && self.sb(k, IN_CUR_SECTOR) == 1 && left > 0 {
            left -= 1;
            if self.sb(k, B19D) == 1 {
                ja1_fire_merc_core(self.w, self.h, k, 0, 0);
                self.a.call(op::AIM_REMOVED, &[k]);
                team_compact(self.w, self.h, 1);
            } else {
                k += 1;
            }
        }
    }

    /// Leaving with the helicopter charge paid: the number n of mercs on the way.
    fn pay(&self) -> i32 {
        let n = (0..8).filter(|&k| self.sb(k, B19D) == 1).count() as i32;
        let d = unsafe { (*self.w).difficulty } as usize;
        // SPEC GAP: a difficulty above 2 charges the hard price.
        let h = HELI[d.min(2)];
        put16(unsafe { (*self.w).accounts }, ACCOUNTS_LEN, A_ARRIVALS, n as u16);
        add(self.w, BALANCE, -h);
        self.acc_put(A_LINK, 0);
        for k in 0..8 {
            if self.sb(k, B19D) == 1 {
                self.sb_put(k, B19D, 0);
                self.pb_put(self.sb(k, PROFILE_ID) as i32, P_TEAM_STATUS, 100);
                self.acc_put(A_HIRINGS, self.acc(A_HIRINGS).wrapping_add(1));
            }
        }
        n
    }
}

fn aim(w: *mut Ja1World, h: *const Ja1Host) -> Option<Aim> {
    // SPEC GAP: a null world, soldier, profile or accounts buffer does nothing.
    if w.is_null() {
        return None;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).accounts.is_null() {
            return None;
        }
    }
    Ai::new(w, h, 0).map(|a| Aim { w, h, a })
}

fn run(w: *mut Ja1World, h: *const Ja1Host, f: impl FnOnce(&Aim) -> i32) -> i32 {
    match aim(w, h) {
        Some(a) => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(&a))).unwrap_or(-1),
        None => -1,
    }
}

/// Opening steps 1 and 2; returns the face timer.
#[no_mangle]
pub extern "C" fn ja1_aim_open(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    run(w, h, |a| a.open())
}

/// Browsing from q with step d (+1 or −1); returns the new page profile P (also `accounts` +0xA4) or −1.
#[no_mangle]
pub extern "C" fn ja1_aim_browse(w: *mut Ja1World, h: *const Ja1Host, q: i32, d: i32) -> i32 {
    run(w, h, |a| a.browse(q, d))
}

/// The refusal reason of profile p (0..59), with host op 337 for the named merc.
#[no_mangle]
pub extern "C" fn ja1_aim_refusal(w: *mut Ja1World, h: *const Ja1Host, p: i32) -> i32 {
    if !(0..64).contains(&p) {
        return -1;
    }
    run(w, h, |a| a.refusal(p))
}

/// Hire of profile p without the quote; returns the team slot or −1.
#[no_mangle]
pub extern "C" fn ja1_aim_hire(w: *mut Ja1World, h: *const Ja1Host, p: i32) -> i32 {
    if !(0..64).contains(&p) {
        return -1;
    }
    run(w, h, |a| a.hire(p))
}

/// Remove every merc with `b19D` = 1 (host op 338 after each).
#[no_mangle]
pub extern "C" fn ja1_aim_cancel_pending(w: *mut Ja1World, h: *const Ja1Host) {
    run(w, h, |a| {
        a.cancel_pending();
        0
    });
}

/// Pay the helicopter charge and send the hired mercs; returns their number.
#[no_mangle]
pub extern "C" fn ja1_aim_pay(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    run(w, h, |a| a.pay())
}

#[cfg(test)]
mod guard_tests {
    use super::*;

    /// Port rule (SPEC GAP, no original counterpart): a panic inside an A.I.M. entry point returns -1, the same as a
    /// null buffer. Ledger row ja1logic:20033.
    #[test]
    fn panic_in_entry_returns_minus_one() {
        let mut sol = vec![0u8; 48 * REC];
        let mut prof = vec![0u8; 64 * 0x5A];
        let mut acc = vec![0u8; 0x200];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.soldiers = sol.as_mut_ptr();
        w.profiles = prof.as_mut_ptr();
        w.accounts = acc.as_mut_ptr();
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        assert_eq!(run(&mut *w, &h, |_| 7), 7);
        assert_eq!(run(&mut *w, &h, |_| panic!("planted")), -1);
        w.accounts = std::ptr::null_mut();
        assert_eq!(run(&mut *w, &h, |_| 7), -1);
    }
}
