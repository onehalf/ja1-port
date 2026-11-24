//! Sector click on the island map (docs/spec/strategic.md "Sector click on the island map"; C interface in
//! docs/spec/port_api.md "The morning entry point"): `ja1_map_click`, `ja1_map_start`. A child module of `ai`;
//! host op 302 guide_say goes through `Ai`. Connected to home and All accounts are those of `home` and `ledger`.
use super::damage::{get8, put8};
use super::home::connected_home;
use super::ledger::{calc_ledger_all, rd};
use super::{op, Ai, Ja1Host, Ja1World};

const SECTOR_STATE_LEN: usize = 80 * 9;
const FLAGS: usize = 0;
const MERCS: usize = 2;
const TAPPERS_B: usize = 3;
const GUARDS_B: usize = 4;
const TREES: usize = 6;
const WORKABLE: usize = 7;

// g_* offsets in `accounts`.
const BALANCE_AFTER_DAY: i64 = 0x28;
const TAPPERS: i64 = 0x40;
const IDLE_POOL: i64 = 0x4C;
const PROCESS_CAP: i64 = 0x84;

const QUOTE: i32 = 0x2F;
const CATEGORY: i32 = 6;

fn sec(w: *mut Ja1World, s: i32, i: usize) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + i as i64)
}
fn sec_put(w: *mut Ja1World, s: i32, i: usize, v: u8) {
    put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + i as i64, v)
}

fn click(w: *mut Ja1World, h: *const Ja1Host, s: i32, mode: i32, buttons: i32, warned: *mut u8) -> i32 {
    let left = buttons & 1 != 0;
    let right = buttons & 2 != 0;
    if !left && !right {
        return 0;
    }
    if sec(w, s, FLAGS) & 1 == 0 {
        return 1;
    }
    if connected_home(w, s) == 0 {
        return 2;
    }
    let mut msg = 0;
    match mode {
        0 => {
            let on_duty = unsafe { (*w).team_on_duty };
            let cur = unsafe { (*w).cur_sector };
            if left && on_duty != 0 {
                if cur == 0xFF {
                    sec_put(w, s, MERCS, sec(w, s, MERCS).wrapping_add(on_duty as u8));
                    unsafe { (*w).cur_sector = s as u8 };
                } else {
                    msg = 5;
                }
            } else if cur as i32 == s {
                sec_put(w, s, MERCS, 0);
                unsafe { (*w).cur_sector = 0xFF };
            } else {
                msg = if right { 4 } else { 3 };
            }
        }
        1 => {
            let t = sec(w, s, TAPPERS_B);
            if left {
                if t / 3 < sec(w, s, WORKABLE) {
                    if rd(w, IDLE_POOL) >= 3 {
                        sec_put(w, s, TAPPERS_B, t.wrapping_add(3));
                    } else {
                        msg = 7;
                    }
                } else if t != 0 {
                    msg = 10;
                } else if sec(w, s, TREES) == 0 {
                    msg = 8;
                } else {
                    msg = 9;
                }
            } else if t >= 3 {
                sec_put(w, s, TAPPERS_B, t.wrapping_sub(3));
            } else {
                msg = 6;
            }
            calc_ledger_all(w);
            let q = rd(w, TAPPERS) / 3;
            let cap = rd(w, PROCESS_CAP);
            if unsafe { *warned } != 0 {
                if q <= cap {
                    unsafe { *warned = 0 };
                }
            } else if q > cap {
                if !h.is_null() {
                    let ai = Ai { w, h, s: 0 };
                    ai.call(op::GUIDE_SAY, &[QUOTE, CATEGORY]);
                }
                unsafe { *warned = 1 };
            }
        }
        2 => {
            let g = sec(w, s, GUARDS_B);
            if left {
                if g < 8 {
                    if rd(w, IDLE_POOL) >= 1 {
                        sec_put(w, s, GUARDS_B, g.wrapping_add(1));
                    } else {
                        msg = 12;
                    }
                } else {
                    msg = 13;
                }
            } else if g != 0 {
                sec_put(w, s, GUARDS_B, g - 1);
            } else {
                msg = 11;
            }
            calc_ledger_all(w);
        }
        _ => {}
    }
    msg
}

// SPEC GAP: a null world, null sector table, null accounts (modes 1, 2), null `warned` (all modes), or a sector
// outside 0..59 does nothing (answer 0). A null host only skips the host op 302.
#[no_mangle]
pub extern "C" fn ja1_map_click(
    w: *mut Ja1World,
    h: *const Ja1Host,
    sector: i32,
    mode: i32,
    buttons: i32,
    warned: *mut u8,
) -> i32 {
    if w.is_null() || warned.is_null() || !(0..60).contains(&sector) {
        return 0;
    }
    if unsafe { (*w).sector_state.is_null() || (*w).accounts.is_null() } {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| click(w, h, sector, mode, buttons, warned))).unwrap_or(0)
}

/// Start the day. button: 0 = the key, nonzero = the start button.
// SPEC GAP: a null world or null `hinted` gives 0. The spec does not say how the front-end tells the key from the
// button; `button` ≠ 0 = the button.
#[no_mangle]
pub extern "C" fn ja1_map_start(w: *const Ja1World, button: i32, hinted: *mut u8) -> i32 {
    if w.is_null() || hinted.is_null() || unsafe { (*w).accounts.is_null() } {
        return 0;
    }
    let w = w as *mut Ja1World;
    if unsafe { (*w).cur_sector } == 0xFF && unsafe { (*w).team_on_duty } != 0 {
        return 14;
    }
    if rd(w, BALANCE_AFTER_DAY) < 0 {
        return 15;
    }
    if button != 0 && unsafe { *hinted } == 0 {
        unsafe { *hinted = 1 };
        return 16;
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    static SAID: AtomicI32 = AtomicI32::new(0);
    extern "C" fn rnd(_c: *mut std::ffi::c_void, _n: i32) -> i32 {
        0
    }
    extern "C" fn call(_c: *mut std::ffi::c_void, o: i32, a: *const i32, n: i32, _out: *mut u8, _sz: i32) -> u32 {
        if o == 302 && n == 2 && unsafe { *a } == 0x2F && unsafe { *a.add(1) } == 6 {
            SAID.fetch_add(1, Ordering::SeqCst);
        }
        0
    }

    struct T {
        w: Box<Ja1World>,
        acc: Vec<u8>,
        ss: Vec<u8>,
        sol: Vec<u8>,
        order: Vec<u8>,
    }
    fn world() -> T {
        let mut t = T {
            w: Box::new(unsafe { std::mem::zeroed() }),
            acc: vec![0; 0xA8],
            ss: vec![0; SECTOR_STATE_LEN],
            sol: vec![0; 48 * super::super::REC],
            order: (0..61).map(|i| if i < 60 { 59 - i } else { 59 }).collect(),
        };
        t.w.accounts = t.acc.as_mut_ptr();
        t.w.sector_state = t.ss.as_mut_ptr();
        t.w.soldiers = t.sol.as_mut_ptr();
        t.w.sector_order = t.order.as_mut_ptr();
        t.w.cur_sector = 0xFF;
        t
    }
    fn host() -> Ja1Host {
        Ja1Host { ctx: std::ptr::null_mut(), random: Some(rnd), call: Some(call), host_ops: 0 }
    }

    /// Port rule (SPEC GAP): a null `warned` pointer does nothing (0), even for a sector and button that act.
    #[test]
    fn guard_null_warned() {
        let mut t = world();
        let h = host();
        let w: *mut Ja1World = &mut *t.w;
        assert_eq!(ja1_map_click(w, &h, 59, 2, 1, std::ptr::null_mut()), 0);
        let mut warned = 0u8;
        assert_eq!(ja1_map_click(w, &h, 59, 2, 1, &mut warned), 1);
    }

    #[test]
    fn guards_and_messages() {
        let mut t = world();
        let h = host();
        let mut warned = 0u8;
        let w: *mut Ja1World = &mut *t.w;
        // bad arguments
        assert_eq!(ja1_map_click(w, &h, 60, 2, 1, &mut warned), 0);
        assert_eq!(ja1_map_click(w, &h, 59, 2, 0, &mut warned), 0);
        // enemy sector (59 stays linked)
        assert_eq!(ja1_map_click(w, &h, 59, 2, 1, &mut warned), 1);
        t.ss[59 * 9] = 1;
        // other mode: only the checks
        assert_eq!(ja1_map_click(w, &h, 59, 7, 1, &mut warned), 0);
        // no guards to remove
        assert_eq!(ja1_map_click(w, &h, 59, 2, 2, &mut warned), 11);
        // idle pool 0
        assert_eq!(ja1_map_click(w, &h, 59, 2, 1, &mut warned), 12);
    }

    #[test]
    fn place_team() {
        // sector 59 is always linked to home
        let mut t = world();
        let h = host();
        let mut warned = 0u8;
        t.ss[59 * 9] = 1;
        t.w.team_on_duty = 0x103;
        let w: *mut Ja1World = &mut *t.w;
        assert_eq!(ja1_map_click(w, &h, 59, 0, 1, &mut warned), 0);
        assert_eq!(t.ss[59 * 9 + 2], 3);
        assert_eq!(t.w.cur_sector, 59);
        assert_eq!(ja1_map_click(w, &h, 59, 0, 3, &mut warned), 5);
        assert_eq!(ja1_map_click(w, &h, 59, 0, 2, &mut warned), 0);
        assert_eq!(t.w.cur_sector, 0xFF);
        assert_eq!(t.ss[59 * 9 + 2], 0);
        assert_eq!(ja1_map_click(w, &h, 59, 0, 2, &mut warned), 4);
        t.w.team_on_duty = 0;
        let w: *mut Ja1World = &mut *t.w;
        assert_eq!(ja1_map_click(w, &h, 59, 0, 1, &mut warned), 3);
    }

    #[test]
    fn tappers() {
        let mut t = world();
        let h = host();
        let mut warned = 0u8;
        t.ss[59 * 9] = 1;
        t.ss[59 * 9 + 6] = 4; // trees
        let w: *mut Ja1World = &mut *t.w;
        // workable 0, no tappers, trees: too scared
        assert_eq!(ja1_map_click(w, &h, 59, 1, 1, &mut warned), 9);
        assert_eq!(ja1_map_click(w, &h, 59, 1, 2, &mut warned), 6);
        t.ss[59 * 9 + 7] = 2; // workable
        assert_eq!(ja1_map_click(w, &h, 59, 1, 1, &mut warned), 7); // idle pool 0
        assert_eq!(ja1_map_click(w, &h, 59, 1, 1, &mut warned), 7);
        assert_eq!(warned, 0);
    }

    #[test]
    fn start_day() {
        let mut t = world();
        let mut hinted = 0u8;
        t.w.team_on_duty = 2;
        let w: *const Ja1World = &*t.w;
        assert_eq!(ja1_map_start(w, 1, &mut hinted), 14);
        t.w.cur_sector = 3;
        assert_eq!(ja1_map_start(w, 1, &mut hinted), 16);
        assert_eq!(hinted, 1);
        assert_eq!(ja1_map_start(w, 1, &mut hinted), 0);
        hinted = 0;
        assert_eq!(ja1_map_start(w, 0, &mut hinted), 0);
        assert_eq!(hinted, 0);
        t.acc[0x28..0x2C].copy_from_slice(&(-1i32).to_le_bytes());
        assert_eq!(ja1_map_start(w, 1, &mut hinted), 15);
    }
}
