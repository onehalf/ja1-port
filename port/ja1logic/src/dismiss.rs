//! Firing a merc (docs/spec/strategic.md "Firing a merc"; C interface in docs/spec/port_api.md "Firing a merc
//! entry point"): `ja1_fire_merc` (steps 5 and 6: the hire-day refund, then Fire the merc). A child module of `ai`;
//! host op 328 `fire_quote` and the host random go through `Ai`. Merc leaves is that of `pay`, Native leaves that of
//! `corpse`, the key E store that of `inventory`.
use super::corpse::native_leaves;
use super::damage::{get16, get8, put16, put8};
use super::inventory::store_all;
use super::pay::merc_leaves;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: i64 = 0x5A;
const PROFILES_LEN: usize = 64 * 0x5A;
const ACCOUNTS_LEN: usize = 0xA8;
const DAY: i64 = 0x04;
const TERMINATIONS: i64 = 0x9C;
const NON_PAYMENTS: i64 = 0xA1;

const SALARY2: usize = 0x112;
const PROFILE_ID: usize = 0x179;
const SLOT: usize = 0x17A;
const IN_CUR_SECTOR: usize = 0x182;
const B19D: usize = 0x19D;
const DAY_HIRED: usize = 0x1E5;

const P_SAME_AS: usize = 0x2F;
const P_B3C: usize = 0x3C;
const P_FIRED: usize = 0x3D;
const P_B3E: usize = 0x3E;
const P_GRUDGE: usize = 0x4C;

struct Fm {
    w: *mut Ja1World,
    h: *const Ja1Host,
    a: Ai,
}

impl Fm {
    fn sb(&self, k: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off as i64)
    }
    fn pb(&self, p: i32, off: usize) -> u8 {
        get8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off as i64)
    }
    fn pb_put(&self, p: i32, off: usize, v: u8) {
        put8(unsafe { (*self.w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off as i64, v)
    }
    fn acc_inc(&self, off: i64) {
        let p = unsafe { (*self.w).accounts };
        put8(p, ACCOUNTS_LEN, off, get8(p, ACCOUNTS_LEN, off).wrapping_add(1));
    }
    fn day(&self) -> i32 {
        let p = unsafe { (*self.w).accounts };
        (get16(p, ACCOUNTS_LEN, DAY) as u32 | (get16(p, ACCOUNTS_LEN, DAY + 2) as u32) << 16) as i32
    }
    /// g_day − the signed hire-day byte of soldier k (32-bit, wrapping).
    fn days_on_team(&self, k: i32) -> i32 {
        self.day().wrapping_sub(self.sb(k, DAY_HIRED) as i8 as i32)
    }

    /// Steps 5 and 6 of "Firing a merc" for soldier k (the duty menu's slot) with the duty name box x.
    fn fire_yes(&self, k: i32, x: i32) {
        if self.days_on_team(k) == 0 {
            let s2 = get16(unsafe { (*self.w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + SALARY2 as i64);
            let p = self.sb(k, PROFILE_ID) as i64;
            put16(unsafe { (*self.w).profiles }, PROFILES_LEN, p * PROFILE + P_B3E as i64, s2);
            self.acc_inc(NON_PAYMENTS);
        }
        self.fire(self.sb(k, SLOT) as i32, x, 0x14);
    }

    /// Fire the merc (soldier k, balloon place (x, y)).
    fn fire(&self, k: i32, x: i32, y: i32) {
        let p = self.sb(k, PROFILE_ID) as i32;
        if p >= 60 {
            store_all(self.w, self.h, k, false);
            native_leaves(self.w, self.h, k);
            return;
        }
        if self.sb(k, B19D) != 0 {
            merc_leaves(self.w, self.h, k, 0);
            return;
        }
        self.pb_put(p, P_FIRED, 1);
        unsafe { (*self.w).voice_test = 1 };
        self.a.call(op::FIRE_QUOTE, &[k, x, y, p]);
        let d = self.days_on_team(k);
        if d > 0 {
            if self.a.random(d as i16 as i32) == 0 {
                let n = self.a.random(3) + 1;
                self.grudge(p, n);
            }
        } else {
            self.grudge(p, 4);
        }
        self.acc_inc(TERMINATIONS);
        store_all(self.w, self.h, k, false);
        merc_leaves(self.w, self.h, k, 1);
    }

    /// Grudge (p, n).
    fn grudge(&self, p: i32, n: i32) {
        for _ in 0..n {
            let u = (self.a.random(60) & 0xFF) as i32;
            let on_team = (0..8).any(|j| self.sb(j, IN_CUR_SECTOR) != 0 && self.sb(j, PROFILE_ID) as i32 == u);
            if !on_team
                && u != p
                && self.pb(u, P_B3C) as i32 != p
                && self.pb(u, P_GRUDGE) == 99
                && self.pb(u, P_SAME_AS) != 3
                && u != 7
            {
                self.pb_put(u, P_GRUDGE, p as u8);
            }
        }
    }
}

fn fm(w: *mut Ja1World, h: *const Ja1Host) -> Option<Fm> {
    // SPEC GAP: a null world, host, soldier, profile or accounts buffer does nothing.
    if w.is_null() {
        return None;
    }
    unsafe {
        if (*w).soldiers.is_null() || (*w).profiles.is_null() || (*w).accounts.is_null() {
            return None;
        }
    }
    Ai::new(w, h, 0).map(|a| Fm { w, h, a })
}

#[no_mangle]
pub extern "C" fn ja1_fire_merc(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32) {
    // SPEC GAP: a slot outside 0..7 does nothing.
    if !(0..8).contains(&slot) {
        return;
    }
    if let Some(f) = fm(w, h) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.fire_yes(slot, x)));
    }
}

/// Fire the merc alone (the original 0x5097C; test entry point of validate/diff/ai_diff.py pay "fire").
#[no_mangle]
pub extern "C" fn ja1_fire_merc_core(w: *mut Ja1World, h: *const Ja1Host, slot: i32, x: i32, y: i32) {
    if !(0..8).contains(&slot) {
        return;
    }
    if let Some(f) = fm(w, h) {
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f.fire(slot, x, y)));
    }
}
