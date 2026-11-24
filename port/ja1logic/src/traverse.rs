//! Traverse to a neighbour sector (docs/spec/soldier_update.md "Traverse to a neighbour sector"; C interface in
//! docs/spec/port_api.md): Traverse edge, Team nearby, Traverse check, Traverse, Traverse out and Traverse in.
//! A child module of `ai`: every op goes through `Ai::call` (the host_ops rule), also the library ops 7 tile_dist
//! and 214 leave_sector. World data is read again after every host call.
use super::damage::{get8, put8};
use super::{op, Ai, Ja1Host, Ja1World};

const SECTOR_STATE_LEN: usize = 80 * 9;
/// Host op 125 message texts.
const CANT_TRAVERSE: i32 = 11;
const INTERRUPT: i32 = 12;

fn sector8(w: *mut Ja1World, c: i64, i: usize) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, c * 9 + i as i64)
}

/// t(r) = s16 +0x114.
fn tile_of(a: &Ai, k: usize) -> i32 {
    (a.rd8_of(k, 0x114) as u16 | (a.rd8_of(k, 0x115) as u16) << 8) as i16 as i32
}

/// Traverse edge (slot, dir).
fn edge(a: &Ai, dir: i32) -> i32 {
    let t = tile_of(a, a.slot() as usize);
    let c = a.w_cur_sector() as i32;
    // 1.
    let stop = match dir {
        10 => t > 0x118 || c < 10,
        11 => t % 56 < 51 || c % 10 == 9,
        12 => t < 0xB28 || c > 49,
        13 => t % 56 > 5 || c % 10 == 0,
        _ => false,
    };
    if stop {
        return 0;
    }
    // 2.
    a.on_screen() as i32
}

/// Team nearby (slot).
fn team_nearby(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32 {
    let Some(a) = Ai::new(w, h, slot) else { return 0 };
    let me = slot as u8;
    for k in 0..8usize {
        let hp = a.rd8_of(k, 0x183) as i8 as i32;
        if a.rd8_of(k, 0x182) == 0 || hp <= 0 || a.rd8_of(k, 0x138) == 0 || a.rd8_of(k, 0x17A) == me {
            continue;
        }
        // 1.
        if hp < 15 {
            if hp >= 10 {
                let Some(r) = Ai::new(w, h, k as i32) else { return 0 };
                if !r.in_water() {
                    a.call(op::MERC_MESSAGE, &[k as i32, 9, 0x40, 0, 1]);
                }
            }
            a.call(op::MERC_MESSAGE, &[slot, 0x11, 0x40, 0, 1]);
            return 0;
        }
        // 2.
        if a.tile_dist_i(tile_of(&a, k), tile_of(&a, me as usize)) > 7 {
            a.call(op::MERC_MESSAGE, &[k as i32, 7, 0x40, 0, 1]);
            return 0;
        }
        // 3.
        if a.rd8_of(k, 0x17E) == 8 || unsafe { (*w).turn_timer } > 0 {
            a.call(op::MESSAGE, &[INTERRUPT]);
            return 0;
        }
    }
    1
}

/// Step 3 of Traverse check: the search along the edge. None = both sides off.
fn search(a: &Ai, t: i32, kind: i32, sa: i32, sb: i32) -> Option<i32> {
    if a.tile_standable(t) != 0 || a.occupant(t as u16) < 0x28 {
        return Some(t);
    }
    let (mut ta, mut tb, mut on_a, mut on_b) = (t, t, true, true);
    loop {
        if on_a {
            ta += sa;
            if match kind {
                1 | 3 => ta < 0,
                5 => ta < 0xC08,
                _ => ta > 0xC08,
            } {
                on_a = false;
            }
            if on_a && a.tile_standable(ta) == 1 {
                return Some(ta);
            }
        }
        if on_b {
            tb += sb;
            if match kind {
                1 => tb > 0x38,
                3 | 5 => tb > 0xC3F,
                _ => tb < 0,
            } {
                on_b = false;
            }
            if on_b && a.tile_standable(tb) == 1 {
                return Some(tb);
            }
        }
        if !on_a && !on_b {
            return None;
        }
    }
}

/// Traverse check (slot, dir, out).
fn check(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32, out: &mut [i16; 3]) -> i32 {
    // 1. (Team nearby gets the record's own slot byte +0x17A)
    let id = match Ai::new(w, h, slot) {
        Some(a) => a.rd8(0x17A) as i32,
        None => return 0,
    };
    if team_nearby(w, h, id) == 0 {
        return 0;
    }
    let Some(a) = Ai::new(w, h, slot) else { return 0 };
    let me = slot as usize;
    let mut t = tile_of(&a, me);
    // 2. and 3.
    let (kind, dt, ds) = match dir {
        10 => {
            while t > 55 {
                t -= 56;
            }
            (1, 0xC08, -10)
        }
        11 => {
            while t % 56 < 55 {
                t += 1;
            }
            (3, -55, 1)
        }
        12 => {
            while t < 0xC08 {
                t += 56;
            }
            (5, -0xC08, 10)
        }
        13 => {
            while t % 56 > 0 {
                t -= 1;
            }
            (7, 55, -1)
        }
        _ => (0, 0, 0),
    };
    if kind != 0 {
        let (sa, sb) = match kind {
            1 | 5 => (-1, 1),
            3 => (-56, 56),
            _ => (56, -56),
        };
        match search(&a, t, kind, sa, sb) {
            Some(v) => t = v,
            None => return 0,
        }
    }
    // 4.
    if a.tile_dist_i(t, tile_of(&a, me)) > 5 {
        return 0;
    }
    // 5.
    if unsafe { (*w).combat } != 0 {
        for k in 0..8usize {
            if a.rd8_of(k, 0x182) != 0 && a.rd8_of(k, 0x138) != 0 && (a.rd8_of(k, 0x1A5) as i8) < 5 {
                a.call(op::MERC_TEXT, &[k as i32, 10, 2000]);
                return -1;
            }
        }
    }
    // 6.
    let c = unsafe { (*w).cur_sector } as i32;
    out[0] = (t + dt) as i16;
    out[1] = (c + ds) as i16;
    out[2] = kind as i16;
    1
}

/// Traverse out (dest): the steps before the new map.
fn out_steps(w: *mut Ja1World, h: *const Ja1Host, dest: i32) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    // SPEC GAP: a sector outside the 80 sector_state entries reads 0 and is not written.
    let d = dest as i16 as i64;
    let c = unsafe { (*w).cur_sector };
    let m = sector8(w, c as i64, 2);
    put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, d * 9 + 2, m);
    put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, c as i64 * 9 + 2, 0);
    a.call(op::LEAVE_SECTOR, &[0]);
    let c = unsafe { (*w).cur_sector };
    a.call(op::REPLAY_EVENT, &[c as i8 as i32, dest as i8 as i32, 0x4A]);
    a.call(op::FADE, &[0, 0x38, 0x100, 0]);
    unsafe { (*w).cur_sector = dest as u8 };
}

/// Traverse in (theme, playing): step 4 of the move.
fn in_steps(w: *mut Ja1World, theme: i32, playing: i32) -> i32 {
    unsafe {
        if (*w).selected < 8 {
            (*w).merc_to_show = (*w).selected;
        }
        let enemies = sector8(w, (*w).cur_sector as i64, 1) != 0;
        let seen = (*w).seen_enemies != 0;
        let m = match theme as u16 {
            1 | 2 => {
                if enemies {
                    if seen { 3 } else { 2 }
                } else {
                    4
                }
            }
            3 => {
                if seen { 0 } else { 4 }
            }
            4 | 5 => (playing == 0) as i32,
            _ => 0,
        };
        (*w).redraw_pending = 1;
        if ((*w).first_hit as i8) < 2 {
            (*w).first_hit = enemies as u8;
        }
        m
    }
}

fn guarded<T>(def: T, f: impl FnOnce() -> T) -> T {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or(def)
}

/// `out` = 3 int16 values or NULL (then the results are dropped).
unsafe fn out_ref<'a>(out: *mut i16, local: &'a mut [i16; 3]) -> &'a mut [i16; 3] {
    if out.is_null() {
        local
    } else {
        &mut *(out as *mut [i16; 3])
    }
}

#[no_mangle]
pub extern "C" fn ja1_traverse_edge(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32) -> i32 {
    // SPEC GAP: a slot outside 0..47 or a null world gives 0.
    guarded(0, || Ai::new(w, h, slot).map_or(0, |a| edge(&a, dir)))
}

#[no_mangle]
pub unsafe extern "C" fn ja1_traverse_check(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32, out: *mut i16) -> i32 {
    let mut local = [0i16; 3];
    let o = out_ref(out, &mut local);
    guarded(0, || if Ai::new(w, h, slot).is_some() { check(w, h, slot, dir, o) } else { 0 })
}

#[no_mangle]
pub unsafe extern "C" fn ja1_traverse(w: *mut Ja1World, h: *const Ja1Host, slot: i32, dir: i32, out: *mut i16) -> i32 {
    let mut local = [0i16; 3];
    let o = out_ref(out, &mut local);
    guarded(0, || {
        let Some(a) = Ai::new(w, h, slot) else { return 0 };
        if edge(&a, dir) != 0 {
            let v = check(w, h, slot, dir, o);
            if v < 0 {
                return -1;
            }
            if v == 1 {
                return 1;
            }
        }
        if let Some(a) = Ai::new(w, h, slot) {
            a.call(op::MESSAGE, &[CANT_TRAVERSE]);
        }
        0
    })
}

#[no_mangle]
pub extern "C" fn ja1_traverse_out(w: *mut Ja1World, h: *const Ja1Host, dest: i32) {
    guarded((), || out_steps(w, h, dest))
}

#[no_mangle]
pub extern "C" fn ja1_traverse_in(w: *mut Ja1World, h: *const Ja1Host, theme: i32, playing: i32) -> i32 {
    // SPEC GAP: a null world gives 0 (the host is not used).
    let _ = h;
    if w.is_null() {
        return 0;
    }
    guarded(0, || in_steps(w, theme, playing))
}

#[cfg(test)]
mod search_tests {
    use super::*;

    /// A world where every tile is free (occupant 0x40) and not standable (t1 class 6), except tiles whose byte 1 is 1.
    fn world(tiles: &mut [u8], class: &mut [u8]) -> Box<Ja1World> {
        class.fill(6);
        class[1] = 0;
        for t in tiles.chunks_mut(8) {
            if t[5] == 0 {
                t[5] = 0x40;
            }
        }
        let mut w: Box<Ja1World> = Box::new(unsafe { std::mem::zeroed() });
        w.tiles = tiles.as_mut_ptr();
        w.t1_class = class.as_mut_ptr();
        w
    }

    /// Ledger row ja1logic:12178: the edge search 0x310D0 takes the start tile when its occupant is below 0x28
    /// (asm 0x3110B `cmp eax,0x28; jl 0x311cc`); occupant 0x28 (an enemy slot) walks along the edge.
    #[test]
    fn traverse_search_start_tile_needs_a_merc_slot() {
        let (mut tiles, mut class) = (vec![0u8; 3136 * 8], vec![0u8; 256]);
        tiles[20 * 8 + 5] = 0x27;
        tiles[30 * 8 + 5] = 0x28;
        let mut w = world(&mut tiles, &mut class);
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let a = Ai { w: &mut *w, h: &h, s: 0 };
        assert_eq!(search(&a, 20, 1, -1, 1), Some(20));
        assert_eq!(search(&a, 30, 1, -1, 1), None);
    }

    /// Ledger row ja1logic:12191: for kind 7 (jump table 0x31098 entry 6) the A walk stops only past tile 0xC08 (asm 0x31141
    /// `cmp ebx,0xc08; jle` keeps it on), so a standable tile 0xC08 is found.
    #[test]
    fn traverse_search_reaches_tile_c08() {
        let (mut tiles, mut class) = (vec![0u8; 3136 * 8], vec![0u8; 256]);
        tiles[0xC08 * 8 + 1] = 1;
        let mut w = world(&mut tiles, &mut class);
        let h = Ja1Host { ctx: std::ptr::null_mut::<std::os::raw::c_void>(), random: None, call: None, host_ops: 0 };
        let a = Ai { w: &mut *w, h: &h, s: 0 };
        assert_eq!(search(&a, 0xC08 - 56, 7, 56, -56), Some(0xC08));
    }
}
