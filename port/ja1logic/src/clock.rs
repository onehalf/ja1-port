//! The day clock and time compression (docs/spec/turns.md "The day clock", "Time compression"; C interface in
//! docs/spec/port_api.md): `ja1_day_clock` (= library op 159 guards_arrive) and `ja1_time_compress`. A child module
//! of `ai`; host ops 309..316 and the existing ops go through `Ai` (op 43 connected_home stays a library op).
use super::damage::{get8, put8};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const SECTOR_STATE_LEN: usize = 80 * 9;
const ETA_LEN: usize = 960;
const GUARD_REC_LEN: usize = 480;
const IN_CUR_SECTOR: i64 = 0x182;
const IN_PLAY: i64 = 0x138;
const HP: i64 = 0x183;
const ENERGY: i64 = 0x18A;
const WAVE_DAY: [i32; 3] = [3, 2, 1];
const WAVE_PERIOD: [i32; 3] = [4, 3, 2];
const NONE_ETA: i16 = 9999;

/// The world, read again at every use (a host call may change it).
fn wm<'a>(w: *mut Ja1World) -> &'a mut Ja1World {
    unsafe { &mut *w }
}
fn rec8(w: *mut Ja1World, k: i64, o: i64) -> u8 {
    get8(wm(w).soldiers, SOLDIERS_LEN, k * REC as i64 + o)
}
fn sec(w: *mut Ja1World, i: i64) -> u8 {
    get8(wm(w).sector_state, SECTOR_STATE_LEN, i)
}
fn sec_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(wm(w).sector_state, SECTOR_STATE_LEN, i, v)
}
fn eta(w: *mut Ja1World, e: i64) -> i16 {
    let p = wm(w).guard_eta;
    i16::from_le_bytes([get8(p, ETA_LEN, 2 * e), get8(p, ETA_LEN, 2 * e + 1)])
}
fn eta_put(w: *mut Ja1World, e: i64, v: i16) {
    let p = wm(w).guard_eta;
    let b = v.to_le_bytes();
    put8(p, ETA_LEN, 2 * e, b[0]);
    put8(p, ETA_LEN, 2 * e + 1, b[1]);
}
/// The table row of the difficulty. SPEC GAP: a difficulty above 2 counts as 2.
fn diff(w: *mut Ja1World) -> usize {
    (wm(w).difficulty as usize).min(2)
}

/// Redraw: op 112 tac_redraw (1) when opt_onscreen ≠ 0, else op 313 mode_banner.
fn redraw(w: *mut Ja1World, ai: &Ai) {
    if wm(w).opt_onscreen != 0 {
        ai.call(op::TAC_REDRAW, &[1]);
    } else {
        ai.call(op::MODE_BANNER, &[]);
    }
}

/// Pick night merc: a team member able to speak, 0x40 = none.
fn pick_night_merc(w: *mut Ja1World, ai: &Ai) -> u8 {
    let mut tried = [false; 8];
    for _ in 0..8 {
        let k = loop {
            // SPEC GAP: a random value outside 0..7 is taken mod 8.
            let k = ai.random(8).rem_euclid(8) as usize;
            if !tried[k] {
                break k;
            }
        };
        let r = k as i64;
        if rec8(w, r, IN_CUR_SECTOR) != 0
            && rec8(w, r, IN_PLAY) != 0
            && rec8(w, r, HP) as i8 >= 15
            && rec8(w, r, ENERGY) as i8 > 10
        {
            return k as u8;
        }
        tried[k] = true;
    }
    0x40
}

/// How many day clock steps ran (test only: the front-end's script line `clockseed`, validate/replay_week.py).
static CLOCK_STEPS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[no_mangle]
pub extern "C" fn ja1_clock_steps() -> u64 {
    CLOCK_STEPS.load(std::sync::atomic::Ordering::Relaxed)
}

pub(super) fn day_clock(w: *mut Ja1World, h: *const Ja1Host, n: i32) -> i32 {
    CLOCK_STEPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let ai = Ai { w, h, s: 0 };
    let mut arrivals: u8 = 0;
    // 1.
    wm(w).clock_minute = wm(w).clock_minute.wrapping_add(n as u8);
    wm(w).day_minutes = wm(w).day_minutes.wrapping_add(n);
    // 2.
    let v = wm(w).day_minutes.wrapping_mul(100) / 720 / 5;
    if v != wm(w).day_bar {
        wm(w).day_bar = v;
        let tc = wm(w).time_compress as i32;
        ai.call(op::DAY_BAR, &[tc]);
    }
    // 3.
    if wm(w).clock_minute as i8 > 59 {
        let x = wm(w);
        x.clock_minute = ((x.clock_minute as i8) % 60) as u8;
        x.clock_hour = x.clock_hour.wrapping_add(1);
        if x.clock_hour == 13 {
            x.clock_hour = 1;
        }
        x.day_hours = x.day_hours.wrapping_add(1);
        if x.day_hours >= 12 {
            return 1;
        }
        x.alone_warned = 0;
        let d = (6 - x.day_hours as i32).abs();
        x.weather_tire = (6 - d) * if x.hard_day != 0 { 25 } else { 15 };
    }
    // 4. Enemy waves.
    let p = 5 * WAVE_PERIOD[diff(w)];
    if WAVE_DAY[diff(w)] < wm(w).day && wm(w).day_minutes % p == 0 {
        let s = ai.call(op::ENEMY_PICK, &[]) as u8;
        arrivals = arrivals.wrapping_add(ai.call(op::ENEMY_MOVE, &[s as i32]) as u8);
    }
    if wm(w).cur_sector == 0 && wm(w).day_minutes % p == 0 {
        arrivals = arrivals.wrapping_add(ai.call(op::ENEMY_MOVE, &[0]) as u8);
    }
    // 5. Hired guards.
    if wm(w).day_minutes % 5 == 0 {
        for s in 0..60i64 {
            let mut count: u8 = 0;
            for j in 0..8i64 {
                let e = 8 * s + j;
                if eta(w, e) as i32 > wm(w).day_minutes {
                    continue;
                }
                let came = s != wm(w).cur_sector as i64 || {
                    let r = get8(wm(w).guard_rec, GUARD_REC_LEN, e) as i8 as i32;
                    ai.call(op::GUARD_ENTER, &[r]) as u8 != 0
                };
                if came {
                    sec_put(w, 9 * s + 4, sec(w, 9 * s + 4).wrapping_add(1));
                    sec_put(w, 9 * s + 5, sec(w, 9 * s + 5).wrapping_sub(1));
                    eta_put(w, e, NONE_ETA);
                    count = count.wrapping_add(1);
                }
            }
            if count != 0 {
                redraw(w, &ai);
                ai.call(op::GUARDS_MSG, &[count as i32, s as i32 + 1]);
                redraw(w, &ai);
                ai.call(op::REPLAY_EVENT, &[s as i32, count as i8 as i32, 0x5D]);
                if s == wm(w).cur_sector as i64 {
                    arrivals = arrivals.wrapping_add(count);
                } else if sec(w, 9 * s) & 1 == 0 {
                    ai.call(op::RESOLVE_BATTLE, &[s as i32, 1]);
                }
            }
        }
    }
    // 6.
    if arrivals != 0 {
        time_compress(w, h, 0);
        wm(w).redraw_next = 1;
        return 0;
    }
    // 7.
    if wm(w).day_hours >= 11 && wm(w).night_warned == 0 {
        let cs = wm(w).cur_sector as i32;
        if ai.call(op::CONNECTED_HOME, &[cs]) as u8 == 0 {
            let r = pick_night_merc(w, &ai);
            if r < 8 {
                let x = wm(w);
                x.night_warned = 1;
                x.quote_event = 0x20;
                x.quote_delay = 2;
                x.quote_slot = r;
            }
        }
    }
    0
}

pub(crate) fn time_compress(w: *mut Ja1World, h: *const Ja1Host, on: i32) {
    let ai = Ai { w, h, s: 0 };
    if on != 0 {
        wm(w).time_compress = 1;
        for k in 0..8i64 {
            if rec8(w, k, IN_CUR_SECTOR) != 0 && rec8(w, k, IN_PLAY) != 0 && rec8(w, k, HP) as i8 >= 15 {
                if let Some(sub) = Ai::new(w, h, k as i32) {
                    sub.stop_own_tile();
                }
            }
        }
        ai.call(op::REFRESH, &[]);
        wm(w).tick_step = 1000;
    } else {
        wm(w).time_compress = 0;
        wm(w).tick_step = 10;
        ai.call(op::MOUSE_WAIT, &[]);
        ai.call(op::MOUSE_RELEASE, &[]);
    }
}

// SPEC GAP: a null world or null host does nothing (result 0, no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_day_clock(w: *mut Ja1World, h: *const Ja1Host, minutes: i32) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| day_clock(w, h, minutes))).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ja1_time_compress(w: *mut Ja1World, h: *const Ja1Host, on: i32) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| time_compress(w, h, on)));
}
