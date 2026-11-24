//! Story events at the day start (docs/spec/strategic.md "Story events at the day start"; C interface in
//! docs/spec/port_api.md): `ja1_story_day`, the tree blight. A child module of `ai`; host ops 307 quest_record and
//! 308 stash_take go through `Ai`. Workable trees is that of `ledger`.
use super::damage::{get8, put8};
use super::ledger::workable;
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 48 * REC;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const STORY_LEN: usize = 35;
const QUEUE_LEN: usize = 7;
const ACCOUNTS_LEN: usize = 0xA8;
const STASH_LEN: usize = 136;
const OWNED: i64 = 0x74;
const FLAGS: i64 = 0;
const TREES: i64 = 6;
const UNHEALTHY: i64 = 8;
const IN_CUR_SECTOR: i64 = 0x182;
const CURE: u8 = 0x51;
const IN_STORE: i32 = 0x63;

fn s(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).story }, STORY_LEN, i)
}
fn s_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(unsafe { (*w).story }, STORY_LEN, i, v)
}
fn q_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(unsafe { (*w).guide_queue }, QUEUE_LEN, i, v)
}
fn sec(w: *mut Ja1World, n: i32, i: i64) -> u8 {
    if !(0..80).contains(&n) {
        return 0; // SPEC GAP: a sector outside the table reads 0
    }
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n as i64 * 9 + i)
}
fn sec_put(w: *mut Ja1World, n: i32, i: i64, v: u8) {
    if (0..80).contains(&n) {
        put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n as i64 * 9 + i, v)
    }
}
/// R(n, o): byte o of sector record n.
fn rec(w: *mut Ja1World, n: i32, o: i64) -> u8 {
    if !(0..60).contains(&n) {
        return 0;
    }
    get8(unsafe { (*w).sectors }, SECTORS_LEN, n as i64 * 0xB1 + o)
}
fn rec_put(w: *mut Ja1World, n: i32, o: i64, v: u8) {
    if (0..60).contains(&n) {
        put8(unsafe { (*w).sectors }, SECTORS_LEN, n as i64 * 0xB1 + o, v)
    }
}
fn rec_add(w: *mut Ja1World, n: i32, o: i64) {
    rec_put(w, n, o, rec(w, n, o).wrapping_add(1));
}
fn sec_add(w: *mut Ja1World, n: i32, i: i64, d: u8) {
    sec_put(w, n, i, sec(w, n, i).wrapping_add(d));
}

/// Squad search (type, take): the team slot holding the item, 0x63 = the item store, −1 = nowhere. The item store
/// is asked only when `store` (The day's verdict searches without it).
pub(super) fn squad_search(w: *mut Ja1World, ai: &Ai, t: u8, take: bool, store: bool) -> i32 {
    let so = unsafe { (*w).soldiers };
    for k in 0..8i64 {
        if get8(so, SOLDIERS_LEN, k * REC as i64 + IN_CUR_SECTOR) == 0 {
            continue;
        }
        for i in 0..12i64 {
            let p = k * REC as i64 + 12 * i;
            if get8(so, SOLDIERS_LEN, p) == t {
                if take {
                    for j in 0..12 {
                        put8(so, SOLDIERS_LEN, p + j, 0);
                    }
                }
                return k as i32;
            }
        }
    }
    if store && ai.call(op::STASH_TAKE, &[t as i32, take as i32]) & 0xFF != 0 {
        if take {
            let sc = unsafe { (*w).stash_counts };
            put8(sc, STASH_LEN, t as i64, get8(sc, STASH_LEN, t as i64).wrapping_sub(1));
        }
        return IN_STORE;
    }
    -1
}

/// Blight start: the first quest-record pair (sick sector a, story sector b) that fits.
fn blight_start(w: *mut Ja1World, ai: &Ai) -> bool {
    let mut q = [0u8; 250];
    ai.call_out(op::QUEST_RECORD, &[5], &mut q);
    let s16 = |o: usize| i16::from_le_bytes([q[o], q[o + 1]]) as i32;
    for i in 0..25 {
        let (a, b) = (s16(10 * i), s16(10 * i + 6));
        if sec(w, a, FLAGS) & 1 != 0 && sec(w, a, TREES) != 0 && sec(w, b, FLAGS) & 1 == 0 {
            sec_put(w, a, UNHEALTHY, 1);
            rec_put(w, a, 0xAD, 1);
            rec_put(w, a, 0xAB, 0);
            rec_put(w, b, 0xA4, 0x5E);
            let tile = (s16(10 * i + 8) as u16).to_le_bytes();
            rec_put(w, b, 0x8C, tile[0]);
            rec_put(w, b, 0x8D, tile[1]);
            return true;
        }
    }
    false
}

/// Blight spread: one day of sickness on every sector; yes when it reached more trees.
fn blight_spread(w: *mut Ja1World) -> bool {
    let mut seen = [false; 60];
    let mut changed = false;
    // SPEC GAP: a difficulty above 2 counts as 2.
    let limit = match unsafe { (*w).difficulty } {
        0 => 4,
        1 => 3,
        _ => 2,
    };
    for s in 0..60i32 {
        if seen[s as usize] {
            continue;
        }
        seen[s as usize] = true;
        if sec(w, s, UNHEALTHY) == 0 {
            continue;
        }
        rec_add(w, s, 0xAB);
        if rec(w, s, 0xAB) >= limit {
            sec_add(w, s, TREES, 0xFF);
            sec_add(w, s, UNHEALTHY, 0xFF);
            rec_add(w, s, 0xAC);
            workable(w, s as u8);
        }
        if sec(w, s, UNHEALTHY) < sec(w, s, TREES) {
            sec_add(w, s, UNHEALTHY, 1);
            rec_add(w, s, 0xAD);
            changed = true;
            continue;
        }
        let (top, bottom, left, right) = (s < 10, s >= 50, s % 10 == 0, s % 10 == 9);
        let near = [
            (-10, top),
            (1, right),
            (10, bottom),
            (-1, left),
            (-11, top || left),
            (-9, top || right),
            (11, bottom || right),
            (9, bottom || left),
        ];
        for (d, off) in near {
            if off {
                continue;
            }
            let n = s + d;
            if sec(w, n, TREES) != 0 && sec(w, n, UNHEALTHY) == 0 && sec(w, n, FLAGS) & 1 != 0 {
                rec_put(w, n, 0xAB, 0);
                seen[n as usize] = true;
                sec_add(w, n, UNHEALTHY, 1);
                rec_add(w, n, 0xAD);
                changed = true;
                break;
            }
        }
    }
    changed
}

fn story_day(w: *mut Ja1World, h: *const Ja1Host) {
    let ai = Ai { w, h, s: 0 };
    q_put(w, 0, 0);
    for i in [1, 3, 5, 6] {
        q_put(w, i, 0xFF);
    }
    let helper = |w| s(w, 0x0B) as i8 == 1 || s(w, 0x15) != 0;
    if s(w, 0x0A) as i8 == 1 {
        if s(w, 0x20) != 0 {
            if helper(w) {
                q_put(w, 1, 0x4D);
                q_put(w, 2, 6);
            } else {
                q_put(w, 5, 0x11);
            }
            for n in 0..60 {
                rec_put(w, n, 0xAE, sec(w, n, UNHEALTHY));
                sec_put(w, n, UNHEALTHY, 0);
                rec_put(w, n, 0xAB, 0);
            }
            s_put(w, 0x0A, 2);
        } else if s(w, 0x21) != 0 && s(w, 0x0B) as i8 != 1 && s(w, 0x15) == 0 {
            s_put(w, 0x22, s(w, 0x22).wrapping_sub(1));
            if s(w, 0x22) as i8 <= 0 {
                s_put(w, 0x20, 1);
                q_put(w, 5, 0x12);
            }
        }
        if (s(w, 0x0A) as i8) < 2 && blight_spread(w) {
            q_put(w, 0, 1);
        }
        return;
    }
    let owned = i32::from_le_bytes([0, 1, 2, 3].map(|i| get8(unsafe { (*w).accounts }, ACCOUNTS_LEN, OWNED + i)));
    if owned >= 30 && s(w, 0x0A) == 0 && blight_start(w, &ai) {
        if helper(w) {
            q_put(w, 1, 0x18);
            q_put(w, 2, 6);
            if squad_search(w, &ai, CURE, false, true) != -1 && s(w, 0x15) != 0 {
                q_put(w, 3, 0xB0);
                q_put(w, 4, 3);
            }
        } else {
            q_put(w, 5, 4);
            if squad_search(w, &ai, CURE, true, true) != -1 {
                s_put(w, 0x21, 1);
                if (s(w, 0x0E) as i8) < 2 || s(w, 0x1E) != 0 {
                    q_put(w, 6, 0x0F);
                    s_put(w, 0x22, 3);
                } else {
                    s_put(w, 0x22, 1);
                    q_put(w, 6, 0x0D);
                }
            }
        }
        s_put(w, 0x0A, 1);
    }
}

/// Step 1 of the strategic screen opening (`stratopen`).
pub(super) fn story_day_step(w: *mut Ja1World, h: *const Ja1Host) {
    story_day(w, h);
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_story_day(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| story_day(w, h)));
}
