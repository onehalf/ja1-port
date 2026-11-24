//! The guard screen (docs/spec/strategic.md "The guard screen"; C interface `ja1_guard_open`, `ja1_guard_hover`,
//! `ja1_guard_click`, `ja1_guard_exit`, `ja1_guard_close`). A child module of `ai`: host op 89 and op 82 merc_dies
//! go through `Ai`; Connected to home and Forget the soldier are the library's own.
use super::damage::{get8, put8};
use super::home::connected_home;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const SECTOR_STATE_LEN: usize = 80 * 9;
const ETA_LEN: usize = 960;
const GUARD_REC_LEN: usize = 480;
const NONE: i32 = 99;
const MAP_OFF: i32 = 60;

/// The screen's own state (strategic.md "The guard screen", `Ja1GuardScreen`).
#[repr(C)]
pub struct Ja1GuardScreen {
    pub moves: [u8; 480],
    pub moves_n: [i8; 60],
    pub pick: i32,
    pub pick_n: i32,
    pub travel: i32,
    pub pick_no: i32,
    pub here: i32,
    pub moving: i32,
    pub all: i32,
    pub hover: i32,
    pub last: i32,
    pub changed: u8,
}

fn wm<'a>(w: *mut Ja1World) -> &'a mut Ja1World {
    unsafe { &mut *w }
}
fn sec(w: *mut Ja1World, s: i32, o: i64) -> u8 {
    get8(wm(w).sector_state, SECTOR_STATE_LEN, 9 * s as i64 + o)
}
fn sec_add(w: *mut Ja1World, s: i32, o: i64, d: u8) {
    let v = sec(w, s, o).wrapping_add(d);
    put8(wm(w).sector_state, SECTOR_STATE_LEN, 9 * s as i64 + o, v)
}
fn guards(w: *mut Ja1World, s: i32) -> u8 {
    sec(w, s, 4)
}
fn b5(w: *mut Ja1World, s: i32) -> u8 {
    sec(w, s, 5)
}
/// |a mod 10 − b mod 10| + |a div 10 − b div 10|.
fn distance(a: i32, b: i32) -> i32 {
    (a % 10 - b % 10).abs() + (a / 10 - b / 10).abs()
}
/// The side of d that guards from src come in from.
fn side(d: i32, src: i32) -> u8 {
    let (dx, dy) = (d % 10 - src % 10, d / 10 - src / 10);
    if dx.abs() < dy.abs() {
        if dy < 0 {
            1
        } else {
            5
        }
    } else if dx < 0 {
        7
    } else {
        3
    }
}
fn ix(s: i32) -> usize {
    s.clamp(0, 59) as usize
}

fn totals(w: *mut Ja1World, g: &mut Ja1GuardScreen) {
    let (mut here, mut moving) = (0i32, 0i32);
    for s in 0..60 {
        here = here.wrapping_add(guards(w, s) as i32);
        moving = moving.wrapping_add(b5(w, s) as i32 + g.moves_n[s as usize] as i32);
    }
    g.here = here;
    g.moving = moving;
    g.all = here.wrapping_add(moving);
}

fn open(w: *mut Ja1World, g: &mut Ja1GuardScreen) -> i32 {
    g.pick_n = 0;
    g.pick = NONE;
    g.travel = 30;
    g.moves = [99; 480];
    g.moves_n = [0; 60];
    totals(w, g);
    g.hover = NONE;
    g.last = NONE;
    g.changed = 0;
    (wm(w).cur_sector >= 90) as i32
}

fn hover(w: *mut Ja1World, g: &mut Ja1GuardScreen, s: i32) -> i32 {
    if s >= MAP_OFF {
        if g.hover < MAP_OFF {
            g.last = MAP_OFF;
            g.hover = MAP_OFF;
        }
        return 0;
    }
    g.hover = s;
    if s != g.last {
        g.changed = 1;
        g.last = s;
        return 0;
    }
    if g.changed != 0 && g.pick_n != 0 {
        if s == g.pick {
            g.travel = 0;
            g.changed = 0;
        } else if connected_home(w, s) as u8 != 0 {
            g.travel = 30 * distance(s, g.pick);
            g.changed = 0;
            return 1;
        }
    }
    0
}

/// Undo (d): the last guard sent to d goes back to his sector.
fn undo(w: *mut Ja1World, g: &mut Ja1GuardScreen, d: i32) {
    let n = g.moves_n[ix(d)].wrapping_sub(1);
    g.moves_n[ix(d)] = n;
    // SPEC GAP: an entry index outside 0..7 reads source 99 (no sector): nothing goes back.
    let src = if (0..8).contains(&n) { g.moves[8 * ix(d) + n as usize] as i32 } else { NONE };
    if src >= 60 {
        return;
    }
    sec_add(w, src, 4, 1);
    if b5(w, src) as i32 + guards(w, src) as i32 + g.moves_n[ix(src)] as i32 > 8 {
        undo(w, g, src);
    }
}

fn click(w: *mut Ja1World, g: &mut Ja1GuardScreen, s: i32, buttons: i32, ctrl: i32) -> i32 {
    if connected_home(w, s) as u8 == 0 {
        return if sec(w, s, 0) & 1 == 0 { 1 } else { 2 };
    }
    if buttons & 1 != 0 {
        let n = g.moves_n[ix(s)] as i32 + b5(w, s) as i32 + guards(w, s) as i32;
        if n < 8 && g.pick_n != 0 {
            if wm(w).day_minutes.wrapping_add(g.travel) > 719 {
                return 19;
            }
            if s == g.pick {
                sec_add(w, s, 4, 1);
            } else {
                let k = g.moves_n[ix(s)];
                // SPEC GAP: n < 8 keeps the entry index in 0..7.
                if (0..8).contains(&k) {
                    g.moves[8 * ix(s) + k as usize] = g.pick as u8;
                }
                g.moves_n[ix(s)] = k.wrapping_add(1);
            }
            g.pick_n -= 1;
            if g.pick_n == 0 {
                g.pick = NONE;
            }
            totals(w, g);
        }
        return 0;
    }
    if ctrl == 0 {
        if g.pick != NONE && g.pick != s {
            return 18;
        }
        if guards(w, s) != 0 {
            g.pick = s;
            g.pick_n += 1;
            g.pick_no = s + 1;
            sec_add(w, s, 4, 0xFF);
            totals(w, g);
        }
    } else if g.moves_n[ix(s)] != 0 {
        undo(w, g, s);
        totals(w, g);
    }
    0
}

fn close(w: *mut Ja1World, h: *const Ja1Host, g: &mut Ja1GuardScreen) {
    let mut left = [0u8; 60];
    let mut arrive = [0u8; 60];
    for s in 0..60i32 {
        let mut j = 0i32;
        while j < g.moves_n[s as usize] as i32 {
            let e = 8 * s + j;
            // SPEC GAP: a source sector outside 0..59 counts as 59 for Distance, Side and its count.
            let src = (g.moves[e as usize] as i32).min(59);
            let t = (30 * distance(s, src) as i16).wrapping_add(wm(w).day_minutes as i16);
            let p = wm(w).guard_eta;
            put8(p, ETA_LEN, 2 * e as i64, t.to_le_bytes()[0]);
            put8(p, ETA_LEN, 2 * e as i64 + 1, t.to_le_bytes()[1]);
            put8(wm(w).guard_rec, GUARD_REC_LEN, e as i64, side(s, src));
            sec_add(w, s, 5, 1);
            left[src as usize] = left[src as usize].wrapping_add(1);
            arrive[s as usize] = arrive[s as usize].wrapping_add(1);
            j += 1;
        }
    }
    for s in 0..60i32 {
        if left[s as usize] != 0 {
            Ai { w, h, s: 0 }.call(op::REPLAY_EVENT, &[s, left[s as usize] as i32, 0x5B]);
            if s == wm(w).cur_sector as i32 {
                for _ in 0..left[s as usize] {
                    let rec = |k: i64, o: i64| get8(wm(w).soldiers, SOLDIERS_LEN, k * REC as i64 + o);
                    let Some(k) = (8..16).rev().find(|&k| rec(k, 0x182) != 0 && rec(k, 0x183) != 0 && rec(k, 0x188) == 0)
                    else {
                        break;
                    };
                    let ai = Ai { w, h, s: k as usize };
                    ai.forget_soldier();
                    ai.call(op::MERC_DIES, &[k as i32, 0]);
                }
            }
        }
        if arrive[s as usize] != 0 {
            Ai { w, h, s: 0 }.call(op::REPLAY_EVENT, &[s, arrive[s as usize] as i32, 0x5C]);
        }
    }
    g.moves = [99; 480];
    g.moves_n = [0; 60];
}

fn guarded<T>(w: *mut Ja1World, g: *mut Ja1GuardScreen, none: T, f: impl FnOnce(&mut Ja1GuardScreen) -> T) -> T {
    if w.is_null() || g.is_null() || wm(w).sector_state.is_null() {
        return none;
    }
    let g = unsafe { &mut *g };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(g))).unwrap_or(none)
}

// SPEC GAP: a null world, host, screen state or sector table does nothing (result 0; open: 1).
#[no_mangle]
pub extern "C" fn ja1_guard_open(w: *mut Ja1World, _h: *const Ja1Host, g: *mut Ja1GuardScreen) -> i32 {
    guarded(w, g, 1, |g| open(w, g))
}
#[no_mangle]
pub extern "C" fn ja1_guard_hover(w: *mut Ja1World, h: *const Ja1Host, g: *mut Ja1GuardScreen, sector: i32) -> i32 {
    if h.is_null() {
        return 0;
    }
    // SPEC GAP: a sector below 0 counts as 60 (below the map).
    let s = if sector < 0 { MAP_OFF } else { sector.min(MAP_OFF) };
    guarded(w, g, 0, |g| hover(w, g, s))
}
#[no_mangle]
pub extern "C" fn ja1_guard_click(w: *mut Ja1World, h: *const Ja1Host, g: *mut Ja1GuardScreen, sector: i32, buttons: i32,
                                  ctrl: i32) -> i32 {
    // SPEC GAP: a sector outside 0..59 or no button does nothing.
    if h.is_null() || !(0..60).contains(&sector) || buttons & 3 == 0 {
        return 0;
    }
    guarded(w, g, 0, |g| click(w, g, sector, buttons, ctrl))
}
#[no_mangle]
pub extern "C" fn ja1_guard_exit(_w: *mut Ja1World, _h: *const Ja1Host, g: *mut Ja1GuardScreen) -> i32 {
    if g.is_null() {
        return 0;
    }
    if unsafe { (*g).pick_n } == 0 {
        0
    } else {
        20
    }
}
#[no_mangle]
pub extern "C" fn ja1_guard_close(w: *mut Ja1World, h: *const Ja1Host, g: *mut Ja1GuardScreen) {
    if h.is_null() || wm_null(w) {
        return;
    }
    guarded(w, g, (), |g| close(w, h, g))
}
fn wm_null(w: *mut Ja1World) -> bool {
    w.is_null() || unsafe { (*w).soldiers.is_null() || (*w).guard_eta.is_null() || (*w).guard_rec.is_null() }
}

#[cfg(test)]
mod guard_world_tests {
    use super::*;

    /// Port rule (SPEC GAP): a sector outside 0..59 does nothing (0), even with a button.
    #[test]
    fn guard_click_sector_range() {
        let mut ss = vec![0u8; 80 * 9];
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.sector_state = ss.as_mut_ptr();
        let mut g: Box<Ja1GuardScreen> = Box::new(unsafe { std::mem::zeroed() });
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        assert_eq!(ja1_guard_click(&mut *w, &h, &mut *g, 60, 1, 0), 0);
    }
}
