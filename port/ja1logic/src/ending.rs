//! The end of the game (docs/spec/ending.md "Game end" and "The performance rating"; C interface in
//! docs/spec/port_api.md): `ja1_win_vars` (script variables v0..v3 of the win sequence, step 1) and
//! `ja1_performance_rating` (the score 0..100 of the rating text). A child module of `ai`. Squad search is that of
//! `storyday` (no item store), the accounts recalculation that of `ledger`.
use super::damage::{get16, get8};
use super::ledger::calc_ledger_all;
use super::storyday::squad_search;
use super::{Ai, Ja1Host, Ja1World};

const STORY_LEN: usize = 43;
const ACCOUNTS_LEN: usize = 0xA8;
const SECTOR_STATE_LEN: usize = 80 * 9;

fn s(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).story }, STORY_LEN, i)
}
fn s8(w: *mut Ja1World, i: i64) -> i32 {
    s(w, i) as i8 as i32
}
fn a8(w: *mut Ja1World, o: i64) -> u8 {
    get8(unsafe { (*w).accounts }, ACCOUNTS_LEN, o)
}
fn a16(w: *mut Ja1World, o: i64) -> i16 {
    get16(unsafe { (*w).accounts }, ACCOUNTS_LEN, o) as i16
}
fn a32(w: *mut Ja1World, o: i64) -> i32 {
    (a16(w, o) as u16 as u32 | (a16(w, o + 2) as u16 as u32) << 16) as i32
}
fn sec(w: *mut Ja1World, n: i64, i: i64) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n * 9 + i)
}

/// Item 0x5F found in the squad, then (only then) item 0x7B not found.
fn squad_items(w: *mut Ja1World, ai: &Ai) -> (bool, bool) {
    if squad_search(w, ai, 0x5F, false, false) == -1 {
        return (false, false);
    }
    (true, squad_search(w, ai, 0x7B, false, false) != -1)
}

fn win_vars(w: *mut Ja1World, h: *const Ja1Host) -> [i16; 4] {
    let Some(ai) = Ai::new(w, h, 0) else { return [0; 4] };
    let v0 = (s8(w, 0x0B) == 1 || s(w, 0x15) != 0) as i16;
    let v1 = (s8(w, 0x0B) > 0) as i16;
    let v2 = (s8(w, 0x0E) == 2 && s(w, 0x1E) == 0) as i16;
    let (found, also) = squad_items(w, &ai);
    [v0, v1, v2, (found && !also) as i16]
}

fn rating(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let Some(ai) = Ai::new(w, h, 0) else { return 0 };
    let mut v: i16 = 0;
    let mut add = |x: i32| v = v.wrapping_add(x as i16);
    // Steps 1..7: the story bytes.
    if s8(w, 5) == 2 {
        add(10);
    }
    if s8(w, 6) == 1 || s8(w, 7) == 1 {
        add(-5);
    }
    match s8(w, 8) {
        2 => add(10),
        1 => add(-(s(w, 0x24) as i32)),
        _ => {}
    }
    match s8(w, 9) {
        1 => add(-10),
        2 => add(10),
        _ => {}
    }
    if s8(w, 0x0A) == 1 {
        add(-10);
    }
    if s(w, 0x15) != 0 {
        add(-25);
    } else {
        match s(w, 0x0B) {
            0 if sec(w, 0x1B, 0) & 1 != 0 => add(10),
            1 => add(-10),
            2 => add(25),
            _ => {}
        }
    }
    match s8(w, 0x0C) {
        2 => add(10),
        1 => add(-(s(w, 0x18) as i32)),
        _ => {}
    }
    if s8(w, 0x0E) == 2 && s(w, 0x1E) == 0 {
        add(25);
    }
    if squad_search(w, &ai, 0x5F, false, false) != -1 {
        add(25);
    } else if squad_search(w, &ai, 0x7B, false, false) != -1 {
        add(-25);
    }
    // Step 8: the accounts.
    calc_ledger_all(w);
    add(a32(w, 0x1C) / 10);
    add(a32(w, 0x84) / 10);
    let trees = (0..60).fold(0u8, |t, k| t.wrapping_add(sec(w, k, 6)));
    add(trees as i32 - 100);
    add(-(a16(w, 4) as i32 - 50));
    add(-(60 - a16(w, 0x74) as i32));
    add(-(a8(w, 0xA0) as i32 + a16(w, 0) as i32));
    add(-(a32(w, 0x50) / 10));
    add(a32(w, 0x6C) / 8);
    add(a8(w, 0x9A) as i32 / 10);
    add(-(a8(w, 0x9B) as i32 * 10));
    add(2 * a16(w, 0x78) as i32 - 3 * a16(w, 0x7C) as i32);
    // Step 9: the balance from day 3 on.
    if a32(w, 4) > 2 {
        let b = a32(w, 0x24);
        add(if b > 50000 { 10 } else { b / 5000 });
    }
    let mut v = v.clamp(0, 100);
    if unsafe { (*w).leave_reason } as i8 > 1 {
        v /= 2;
    }
    v as i32
}

/// v0..v3 into `out` (4 x i16). SPEC GAP: a null pointer does nothing.
#[no_mangle]
pub extern "C" fn ja1_win_vars(w: *mut Ja1World, h: *const Ja1Host, out: *mut i16) {
    if w.is_null() || h.is_null() || out.is_null() {
        return;
    }
    if let Ok(v) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| win_vars(w, h))) {
        unsafe { std::ptr::copy_nonoverlapping(v.as_ptr(), out, 4) };
    }
}

/// The performance rating score 0..100 (the accounts are recalculated first). SPEC GAP: a null pointer gives 0.
#[no_mangle]
pub extern "C" fn ja1_performance_rating(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| rating(w, h))).unwrap_or(0)
}
