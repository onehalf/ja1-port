//! Day start (docs/spec/strategic.md "Day start"; C interface in docs/spec/port_api.md): `ja1_day_start`.
//! A child module of `ai`; host op 303 day_ask goes through `Ai`.
use super::damage::{get8, put8};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const ACCOUNTS_LEN: usize = 0xA8;
const SORTIES: i64 = 0x9F;
const SECTOR_STATE_LEN: usize = 80 * 9;
const REPLAY_LEN: usize = 60 * 9;
const SOLDIERS_LEN: usize = 8 * REC;
const IN_CUR_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const PROFILE_ID: usize = 0x179;
const DUTY: usize = 0x17E;

fn start(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let t = unsafe { (*w).team_on_duty };
    if t == 0 {
        unsafe { (*w).cur_sector = 59 };
    }
    let (kind, wait_row, go_row, number) = if t == 0 {
        (0, 0, 0, 0)
    } else {
        let (a, b) = match t {
            1 => (6, 7),
            2 => (12, 13),
            3 => (14, 15),
            _ => (16, 17),
        };
        (1, a, b, unsafe { (*w).cur_sector } as i32 + 1)
    };
    let ai = Ai { w, h, s: 0 };
    if ai.call(op::DAY_ASK, &[kind, wait_row, go_row, number]) == 0 {
        return 0;
    }
    // The host may change world data: read again from here.
    let acc = unsafe { (*w).accounts };
    let sorties = get8(acc, ACCOUNTS_LEN, SORTIES).wrapping_add(t as u8);
    put8(acc, ACCOUNTS_LEN, SORTIES, sorties);
    // SPEC GAP: a null soldier table: no guide merc (0x40), speaker is set to 0x40.
    let sol = unsafe { (*w).soldiers };
    let mut guide = 0x40usize;
    for k in 0..8usize {
        let b = (k * REC) as i64;
        let g = |o: usize| get8(sol, SOLDIERS_LEN, b + o as i64);
        if g(IN_CUR_SECTOR) != 0 && g(HP) != 0 && (60..=64).contains(&g(PROFILE_ID)) {
            guide = k;
            break;
        }
    }
    unsafe { (*w).speaker = guide as u8 };
    if guide < 0x40 && get8(sol, SOLDIERS_LEN, (guide * REC + DUTY) as i64) == 0 {
        let s = get8(acc, ACCOUNTS_LEN, SORTIES).wrapping_sub(1);
        put8(acc, ACCOUNTS_LEN, SORTIES, s);
    }
    // SPEC GAP: a null sector table or null replay copy skips the copy.
    let (ss, rp) = unsafe { ((*w).sector_state, (*w).sector_replay) };
    if !ss.is_null() && !rp.is_null() {
        for i in 0..REPLAY_LEN {
            put8(rp, REPLAY_LEN, i as i64, get8(ss, SECTOR_STATE_LEN, i as i64));
        }
    }
    1
}

// SPEC GAP: a null world, null accounts or null host gives 0 (no question, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_day_start(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() || h.is_null() || unsafe { (*w).accounts.is_null() } {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| start(w, h))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    static LAST: [AtomicI32; 4] = [AtomicI32::new(-1), AtomicI32::new(-1), AtomicI32::new(-1), AtomicI32::new(-1)];
    static ANSWER: AtomicI32 = AtomicI32::new(1);
    extern "C" fn rnd(_c: *mut std::ffi::c_void, _n: i32) -> i32 {
        0
    }
    extern "C" fn call(_c: *mut std::ffi::c_void, o: i32, a: *const i32, n: i32, _out: *mut u8, _sz: i32) -> u32 {
        if o == 303 && n == 4 {
            for i in 0..4 {
                LAST[i].store(unsafe { *a.add(i) }, Ordering::SeqCst);
            }
            return ANSWER.load(Ordering::SeqCst) as u32;
        }
        0
    }
    fn host() -> Ja1Host {
        Ja1Host { ctx: std::ptr::null_mut(), random: Some(rnd), call: Some(call), host_ops: 0 }
    }
    fn last() -> [i32; 4] {
        [0, 1, 2, 3].map(|i| LAST[i].load(Ordering::SeqCst))
    }

    #[test]
    fn day_start() {
        // one test body: the host statics are shared
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        let mut acc = vec![0u8; ACCOUNTS_LEN];
        let mut ss = vec![0u8; SECTOR_STATE_LEN];
        let mut rp = vec![0u8; REPLAY_LEN];
        let mut sol = vec![0u8; SOLDIERS_LEN];
        w.accounts = acc.as_mut_ptr();
        w.sector_state = ss.as_mut_ptr();
        w.sector_replay = rp.as_mut_ptr();
        w.soldiers = sol.as_mut_ptr();
        let h = host();
        let wp: *mut Ja1World = &mut *w;
        assert_eq!(ja1_day_start(std::ptr::null_mut(), &h), 0);
        assert_eq!(ja1_day_start(wp, std::ptr::null()), 0);
        // nobody on duty, player says no
        w.cur_sector = 3;
        ANSWER.store(0, Ordering::SeqCst);
        assert_eq!(ja1_day_start(wp, &h), 0);
        assert_eq!(last(), [0, 0, 0, 0]);
        assert_eq!(w.cur_sector, 59);
        // team of 2 in sector 255, guide merc 2 (profile 61) on duty, soldier 1 not in sector
        ANSWER.store(1, Ordering::SeqCst);
        w.team_on_duty = 2;
        w.cur_sector = 255;
        acc[SORTIES as usize] = 255;
        ss[0] = 7;
        ss[539] = 9;
        ss[540] = 1;
        sol[REC + PROFILE_ID] = 60;
        sol[REC + HP] = 5;
        sol[2 * REC + IN_CUR_SECTOR] = 1;
        sol[2 * REC + HP] = 5;
        sol[2 * REC + PROFILE_ID] = 61;
        assert_eq!(ja1_day_start(wp, &h), 1);
        assert_eq!(last(), [1, 12, 13, 256]);
        assert_eq!(w.speaker, 2);
        assert_eq!(acc[SORTIES as usize], 0);
        assert_eq!(rp[0], 7);
        assert_eq!(rp[539], 9);
        // no guide merc, team of 5 (rows 16, 17)
        sol[2 * REC + HP] = 0;
        w.team_on_duty = 5;
        w.cur_sector = 9;
        assert_eq!(ja1_day_start(wp, &h), 1);
        assert_eq!(last(), [1, 16, 17, 10]);
        assert_eq!(w.speaker, 0x40);
    }
}
