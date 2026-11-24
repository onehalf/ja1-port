//! Sunrise (docs/spec/strategic.md "Sunrise"; C interface in docs/spec/port_api.md): `ja1_sunrise`.
//! A child module of `ai`; host op 304 sunrise_show and random go through `Ai`.
use super::damage::{get8, put16, put8};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const ACCOUNTS_LEN: usize = 0xA8;
const DAY_COPY: i64 = 0x04;
const ARRIVALS: i64 = 0x8C;
const CHANGED_LEN: usize = 640;
const SOLDIERS_LEN: usize = 8 * REC;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;

fn sunrise(w: *mut Ja1World, h: *const Ja1Host) {
    let day = unsafe { (*w).day }.wrapping_add(1);
    unsafe { (*w).day = day };
    let acc = unsafe { (*w).accounts };
    let d = day as u32;
    put16(acc, ACCOUNTS_LEN, DAY_COPY, d as u16);
    put16(acc, ACCOUNTS_LEN, DAY_COPY + 2, (d >> 16) as u16);
    // SPEC GAP: a null stat_changed table is not cleared (put8 skips it).
    let sc = unsafe { (*w).stat_changed };
    for i in 0..CHANGED_LEN {
        put8(sc, CHANGED_LEN, i as i64, 0);
    }
    let ai = Ai { w, h, s: 0 };
    ai.call(op::SUNRISE_SHOW, &[]);
    // The host may change world data: read again from here.
    let acc = unsafe { (*w).accounts };
    put16(acc, ACCOUNTS_LEN, ARRIVALS, 0);
    // SPEC GAP: a null soldier table: no healing (reads give 0, so no record is in the sector).
    let sol = unsafe { (*w).soldiers };
    for k in 0..8usize {
        let b = (k * REC) as i64;
        if get8(sol, SOLDIERS_LEN, b + IN_CUR_SECTOR as i64) == 0 {
            continue;
        }
        let m = get8(sol, SOLDIERS_LEN, b + HP_MAX as i64) as i8 as i32;
        let hp = get8(sol, SOLDIERS_LEN, b + HP as i64);
        let d = m - hp as i8 as i32;
        if d == 0 {
            continue;
        }
        let q = m / 15;
        let mut r = if q > 0 { ai.random(q) } else { 0 };
        if r > d {
            r = d;
        }
        put8(sol, SOLDIERS_LEN, b + HP as i64, hp.wrapping_add(r as u8));
    }
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed); null accounts skip
// only the accounts writes.
#[no_mangle]
pub extern "C" fn ja1_sunrise(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sunrise(w, h)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    static SHOWS: AtomicI32 = AtomicI32::new(0);
    static LAST_N: AtomicI32 = AtomicI32::new(-1);
    extern "C" fn rnd(_c: *mut std::ffi::c_void, n: i32) -> i32 {
        LAST_N.store(n, Ordering::SeqCst);
        n - 1
    }
    extern "C" fn call(_c: *mut std::ffi::c_void, o: i32, _a: *const i32, n: i32, _out: *mut u8, _sz: i32) -> u32 {
        if o == 304 && n == 0 {
            SHOWS.fetch_add(1, Ordering::SeqCst);
        }
        0
    }

    #[test]
    fn sunrise_heals_and_counts() {
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        let mut acc = vec![0u8; ACCOUNTS_LEN];
        let mut sc = vec![1u8; CHANGED_LEN];
        let mut sol = vec![0u8; SOLDIERS_LEN];
        w.accounts = acc.as_mut_ptr();
        w.stat_changed = sc.as_mut_ptr();
        w.soldiers = sol.as_mut_ptr();
        w.day = 0xFFFF;
        acc[ARRIVALS as usize] = 3;
        acc[ARRIVALS as usize + 1] = 1;
        let h = Ja1Host { ctx: std::ptr::null_mut(), random: Some(rnd), call: Some(call), host_ops: 0 };
        let wp: *mut Ja1World = &mut *w;
        ja1_sunrise(std::ptr::null_mut(), &h);
        ja1_sunrise(wp, std::ptr::null());
        assert_eq!(SHOWS.load(Ordering::SeqCst), 0);
        // 0: in sector, 60/40 -> q 4, r 3; 1: d 1 < r -> 1; 2: hp > max -> drops to max;
        // 3: not in sector; 4: max 10 -> q 0, no change
        let set = |s: &mut Vec<u8>, k: usize, ins: u8, hp: u8, mx: u8| {
            s[k * REC + IN_CUR_SECTOR] = ins;
            s[k * REC + HP] = hp;
            s[k * REC + HP_MAX] = mx;
        };
        set(&mut sol, 0, 1, 40, 60);
        set(&mut sol, 1, 1, 59, 60);
        set(&mut sol, 2, 1, 70, 60);
        set(&mut sol, 3, 0, 10, 60);
        set(&mut sol, 4, 1, 5, 10);
        ja1_sunrise(wp, &h);
        assert_eq!(SHOWS.load(Ordering::SeqCst), 1);
        assert_eq!(w.day, 0x10000);
        assert_eq!(&acc[4..8], &[0, 0, 1, 0]);
        assert_eq!(&acc[0x8C..0x8E], &[0, 0]);
        assert!(sc.iter().all(|&b| b == 0));
        assert_eq!(sol[HP], 43);
        assert_eq!(sol[REC + HP], 60);
        assert_eq!(sol[2 * REC + HP], 60);
        assert_eq!(sol[3 * REC + HP], 10);
        assert_eq!(sol[4 * REC + HP], 5);
    }
}
