//! A new game (docs/spec/world_load.md section 11 "ja1_new_game"): the library part after the front-end loaded the
//! new-game save. A child module of `ai`; Set owned is that of `battle`, Processing capacity and Worker pool those
//! of `ledger`.
use super::battle::set_owned;
use super::damage::{get8, put8};
use super::ledger::{calc_process_cap, worker_pool};
use super::{Ja1Host, Ja1World};

const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const HOME: u8 = 59;

fn st_put(w: *mut Ja1World, s: u8, k: i64, v: u8) {
    put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + k, v)
}
fn st(w: *mut Ja1World, s: u8, k: i64) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, s as i64 * 9 + k)
}
fn rec_put(w: *mut Ja1World, s: u8, o: i64, v: u8) {
    put8(unsafe { (*w).sectors }, SECTORS_LEN, s as i64 * 0xB1 + o, v)
}

fn new_game(w: *mut Ja1World, h: *const Ja1Host) {
    let d = unsafe { (*w).difficulty };
    // Step 1: Set owned (59).
    set_owned(w, HOME, true);
    // Step 2: the sectors.
    for i in 0..60u8 {
        for k in 0x96..=0x9D {
            rec_put(w, i, k, 0xFF);
        }
        if i == HOME {
            st_put(w, i, 4, 8);
            st_put(w, i, 1, 0);
            rec_put(w, i, 0xAA, 1);
        } else {
            match d {
                0 => st_put(w, i, 1, 4),
                1 => st_put(w, i, 1, 6),
                2 => st_put(w, i, 1, 8),
                _ => {}
            }
            let mut n = st(w, i, 1);
            if i == 28 || i == 37 {
                if (d as i8) < 2 {
                    n = n.wrapping_add(2);
                }
            } else if i == 48 || i == 49 || i == 58 {
                n = n.wrapping_sub(2);
            }
            if n > 8 {
                n = 8;
            }
            st_put(w, i, 1, n);
            for k in 0..n as i64 {
                rec_put(w, i, 0x96 + k, 1);
            }
            rec_put(w, i, 0xAA, 0);
        }
        for k in (0x9E..=0xA9).chain(0xAB..=0xB0) {
            rec_put(w, i, k, 0);
        }
        for k in 0..10 {
            rec_put(w, i, 0x82 + 2 * k, 0x41);
            rec_put(w, i, 0x83 + 2 * k, 0x0C);
        }
    }
    // Step 3.
    calc_process_cap(w);
    worker_pool(w, h);
}

/// SPEC GAP: a null world does nothing.
#[no_mangle]
pub extern "C" fn ja1_new_game(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| new_game(w, h)));
}
