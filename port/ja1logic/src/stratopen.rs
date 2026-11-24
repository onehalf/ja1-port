//! Strategic screen opening (docs/spec/strategic.md "Strategic screen opening"; C interface in
//! docs/spec/port_api.md): `ja1_strategic_open`. A child module of `ai`; host ops 302 guide_say, 305 merc_gone,
//! 306 map_show and random go through `Ai`; step 1 is `storyday` (host ops 307, 308), step 7 `guide` (host ops 302, 307, 317..320). Workable trees, All accounts and Worker pool are those of `ledger`,
//! Connected to home that of `home`, Merc leaves, Close up the team and Collect the cash those of `pay`.
use super::damage::{get8, put8};
use super::home::connected_home;
use super::ledger::{add, calc_ledger_all, workable, worker_pool};
use super::pay::{collect_cash, merc_leaves, team_compact};
use super::guide::guide_advice_step;
use super::storyday::story_day_step;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const PROFILE: i64 = 0x5A;
const PROFILES_LEN: usize = 64 * PROFILE as usize;
const SECTOR_STATE_LEN: usize = 80 * 9;
const ACCOUNTS_LEN: usize = 0xA8;
const KILLED: i64 = 0x00;
const RESIGNATIONS: i64 = 0xA0;
const FLAGS: i64 = 0;
const TAPPERS: i64 = 3;
const TREES: i64 = 6;
const UNHEALTHY: i64 = 8;
const PROFILE_ID: i64 = 0x179;
const IN_CUR_SECTOR: i64 = 0x182;
const HP_MAX: i64 = 0x184;
const P_B2F: i64 = 0x2F;
const P_DISLIKES: i64 = 0x3A;
const P_B3C: i64 = 0x3C;
const P_TEAM_STATUS: i64 = 0x4B;
const CANTEEN: u8 = 0x41;
const NONE: u8 = 99;
/// Guard for the random redraw loop (a host that never returns a free index).
const DRAW_CAP: u32 = 1_000_000;

fn sol(w: *mut Ja1World, k: i32, off: i64) -> u8 {
    get8(unsafe { (*w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off)
}
fn sol_put(w: *mut Ja1World, k: i32, off: i64, v: u8) {
    put8(unsafe { (*w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off, v)
}
fn prof(w: *mut Ja1World, p: u8, off: i64) -> u8 {
    get8(unsafe { (*w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off)
}
fn prof_put(w: *mut Ja1World, p: u8, off: i64, v: u8) {
    put8(unsafe { (*w).profiles }, PROFILES_LEN, p as i64 * PROFILE + off, v)
}
fn sec(w: *mut Ja1World, s: i32, i: i64) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + i)
}

/// Step 2: Tapper limit.
fn tapper_limit(w: *mut Ja1World) {
    for s in 0..60 {
        if sec(w, s, FLAGS) & 1 == 0 || connected_home(w, s) == 0 {
            continue;
        }
        let wk = workable(w, s as u8);
        let mut a = sec(w, s, TREES).wrapping_sub(sec(w, s, UNHEALTHY));
        if a >= wk {
            a = wk;
        }
        let t = sec(w, s, TAPPERS);
        let lim = 3 * a as u32;
        let v = if t as u32 > lim { lim as u8 } else { t - t % 3 };
        put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + TAPPERS, v);
    }
}

/// Step 3: Canteens refilled.
fn canteens(w: *mut Ja1World) {
    for k in 0..8 {
        if sol(w, k, IN_CUR_SECTOR) == 0 {
            continue;
        }
        for i in 0..12i64 {
            let b = i * 12;
            if sol(w, k, b) != CANTEEN {
                continue;
            }
            sol_put(w, k, b + 2, 100);
            let c = sol(w, k, b + 1) as i8 as i64;
            if c > 1 {
                for j in 8..=c + 6 {
                    sol_put(w, k, b + j, 100);
                }
            }
        }
    }
}

/// n(x): soldiers 0..7 in the sector with profile x.
fn count(w: *mut Ja1World, x: u8) -> usize {
    (0..8).filter(|&j| sol(w, j, IN_CUR_SECTOR) != 0 && sol(w, j, PROFILE_ID) == x).count()
}

/// Step 4: Overnight departures.
fn departures(w: *mut Ja1World, h: *const Ja1Host, ai: &Ai) {
    for p in 0..60u8 {
        let t = prof(w, p, P_TEAM_STATUS) as i8;
        if (1..=49).contains(&t) {
            prof_put(w, p, P_TEAM_STATUS, (t - 1) as u8);
        }
    }
    let mut gone = [0x40u8; 8];
    let mut picked = [false; 8];
    let mut left = false;
    for _ in 0..8 {
        let mut k = -1i32;
        for _ in 0..DRAW_CAP {
            let r = ai.random(8);
            if (0..8).contains(&r) && !picked[r as usize] {
                k = r;
                break;
            }
        }
        if k < 0 {
            // SPEC GAP: a host that never answers a free index ends the draws.
            break;
        }
        picked[k as usize] = true;
        if sol(w, k, IN_CUR_SECTOR) == 0 {
            continue;
        }
        let p = sol(w, k, PROFILE_ID);
        let d = prof(w, p, P_DISLIKES);
        let wv = prof(w, p, P_B3C);
        let mut leave = false;
        if d == NONE {
            if wv != NONE && count(w, wv) != 0 {
                prof_put(w, p, P_DISLIKES, wv);
            }
        } else if d == wv && count(w, d) != 0 {
            if prof(w, p, P_B2F) == 3 {
                for j in 0..8 {
                    if sol(w, j, IN_CUR_SECTOR) == 0 || sol(w, j, PROFILE_ID) != d {
                        continue;
                    }
                    gone[k as usize] = d;
                    left = true;
                    sol_put(w, j, HP_MAX, 0);
                    merc_leaves(w, h, j, 0);
                    sol_put(w, j, IN_CUR_SECTOR, 0);
                    add(w, KILLED, 1);
                    sol_put(w, j, PROFILE_ID, NONE);
                }
            } else {
                leave = true;
            }
        }
        if leave {
            gone[k as usize] = sol(w, k, PROFILE_ID);
            left = true;
            merc_leaves(w, h, k, 0);
            sol_put(w, k, IN_CUR_SECTOR, 0);
            let acc = unsafe { (*w).accounts };
            put8(acc, ACCOUNTS_LEN, RESIGNATIONS, get8(acc, ACCOUNTS_LEN, RESIGNATIONS).wrapping_add(1));
            sol_put(w, k, PROFILE_ID, NONE);
        }
    }
    for _ in 0..8 {
        team_compact(w, h, 0);
    }
    calc_ledger_all(w);
    ai.call(op::MAP_SHOW, &[]);
    if left {
        ai.call(op::GUIDE_SAY, &[0x98, 3]);
        for g in gone {
            if g < 0x40 {
                ai.call(op::MERC_GONE, &[g as i32]);
            }
        }
    }
}

fn strategic_open(w: *mut Ja1World, h: *const Ja1Host) {
    let ai = Ai { w, h, s: 0 };
    story_day_step(w, h);
    tapper_limit(w);
    canteens(w);
    departures(w, h, &ai);
    collect_cash(w);
    worker_pool(w, h);
    guide_advice_step(w, h);
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_strategic_open(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| strategic_open(w, h)));
}
