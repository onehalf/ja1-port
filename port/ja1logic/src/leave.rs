//! Leaving the sector (docs/spec/soldier_update.md "Leaving the sector"; C interface in docs/spec/port_api.md):
//! Leave path (`ja1_leave_path`), Leave sector (`ja1_leave_sector`, library op 214) and End of day
//! (`ja1_end_of_day`, library op 217). A child module of `ai`:
//! every op goes through `Ai::call` (the host_ops rule), also the library ops 214, 78 kill and 82 merc_dies.
//! World data is read again after every host call.
use super::damage::{get8, put8};
use super::{op, Ai, Ja1Host, Ja1World};

const SECTOR_STATE_LEN: usize = 80 * 9;

/// Byte i of sector_state entry c (0 outside the table).
fn sector8(w: *mut Ja1World, c: u8, i: usize) -> u8 {
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, c as i64 * 9 + i as i64)
}

/// Leave path: runs once when the tactical loop's leave counter is not 0.
pub(super) fn leave_path(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    // Fan fix F28 (1.15m TacticalLoop 0x11358..0x113EA, #70 row 17): the fade out and the End of day screen come
    // after the cleanup, so the battle show of guards and enemies still in the sector after the last turn plays on
    // a lit screen (1.13b fades first: the show is black).
    let late = crate::fanfix::on(28);
    // 1.
    if !late {
        a.call(op::FADE, &[0, 0x38, 0x100, 0]);
    }
    a.call(op::CURSOR_BACK, &[]);
    a.call(op::MUSIC_STOP, &[]);
    // 2.
    if unsafe { (*w).quit_tactical == 0 && (*w).leave_reason == 0 } {
        // a.
        if !late {
            a.call(op::END_OF_DAY, &[]);
        }
        // b.
        let c = unsafe { (*w).cur_sector };
        if sector8(w, c, 2) != 0 {
            let sc = c as i8 as i32;
            a.call(op::REPLAY_EVENT, &[sc, sc, 0x59]);
            put8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, c as i64 * 9 + 2, 0);
        }
        // c.
        a.call(op::LEAVE_SECTOR, &[1]);
        if late {
            a.call(op::FADE, &[0, 0x38, 0x100, 0]);
            a.call(op::END_OF_DAY, &[]);
        }
        // d.
        unsafe {
            if (*w).santino_spoke != 0 && (*w).leave_reason == 0 {
                (*w).leave_reason = 1;
            }
        }
    } else if late {
        a.call(op::FADE, &[0, 0x38, 0x100, 0]);
    }
    // 3.
    a.call(op::TIMER_OFF, &[]);
    unsafe { (*w).cur_sector = 0xFF };
}

/// Leave sector (library op 214): dying soldiers die unless their side still has someone standing and the other
/// side does not; then the strategic layer gets the sector's result and the sector is saved.
pub(super) fn leave_sector(w: *mut Ja1World, h: *const Ja1Host, flag: i32) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    let hp = |k: i32| a.rd8_of(k as usize, 0x183);
    let in_sector = |k: i32| a.rd8_of(k as usize, 0x182) != 0;
    // 1.
    for k in 0..8 {
        let Some(r) = Ai::new(w, h, k) else { return };
        if flag as u8 != 0 && r.rd8(0x17E) == 8 {
            r.wr8(0x17E, 0);
        }
        if r.rd8(0x182) != 0 && r.rd8(0x138) != 0 && r.rd8(0x188) == 0 && r.rd8(0x183) == 0 {
            a.call(op::MERC_DIES, &[k, 1]);
        }
    }
    // 2.
    let up = |ks: std::ops::Range<i32>| ks.into_iter().any(|k| in_sector(k) && hp(k) as i8 >= 15);
    let enemy_up = up(0x28..0x30);
    let guard_up = up(8..0x28);
    // 3. and 4.
    let dying = |k: i32| {
        let v = hp(k) as i8;
        0 < v && v < 15
    };
    for k in 0x28..0x30 {
        if !in_sector(k) {
            continue;
        }
        if dying(k) && (!enemy_up || guard_up) {
            a.call(op::KILL, &[k, 0]);
        }
        if hp(k) == 0 {
            a.call(op::MERC_DIES, &[k, 0]);
        }
    }
    for k in 8..0x28 {
        if !in_sector(k) {
            continue;
        }
        if dying(k) && (!guard_up || enemy_up) {
            a.call(op::KILL, &[k, 0]);
        }
        if hp(k) == 0 {
            a.call(op::MERC_DIES, &[k, 0]);
        }
    }
    // 5.
    let c = unsafe { (*w).cur_sector };
    let b0 = sector8(w, c, 0);
    if sector8(w, c, 1) != 0 {
        if sector8(w, c, 4) != 0 || sector8(w, c, 3) != 0 || b0 & 1 != 0 {
            a.call(op::RESOLVE_BATTLE, &[c as i32, 1]);
        }
    } else if b0 & 1 == 0 {
        a.call(op::SECTOR_GAINED, &[c as i8 as i32]);
        // F22 (1.15m 0x30F7D -> cave 0x4889B): the territory totals and workable trees are counted again at once,
        // so the day-end retreat test sees the sector as won (ResolveSectorBattle above ends with this recount).
        if crate::fanfix::on(22) {
            super::ledger::recalc_territory(w);
        }
    }
    // 6.
    a.call(op::SAVE_TMP, &[]);
}

#[no_mangle]
pub extern "C" fn ja1_leave_path(w: *mut Ja1World, h: *const Ja1Host) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| leave_path(w, h)));
}
#[no_mangle]
pub extern "C" fn ja1_leave_sector(w: *mut Ja1World, h: *const Ja1Host, flag: i32) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| leave_sector(w, h, flag)));
}
#[no_mangle]
pub extern "C" fn ja1_end_of_day(w: *mut Ja1World, h: *const Ja1Host) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| end_of_day(w, h)));
}

/// End of day (library op 217): the sunset screen with "End of Day N", then the retreat of a squad that ends the
/// day in a sector not joined to home land.
pub(super) fn end_of_day(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    // Sunset 1.
    let day = unsafe { (*w).day };
    a.call(op::EOD_BEGIN, &[day]);
    // 2.
    let mut skipped = false;
    for i in 0..119 {
        if !skipped {
            a.call(op::EOD_WAIT, &[]);
        }
        a.call(op::PAL_CYCLE, &[3, 0xFF]);
        if !skipped {
            a.call(op::PAL_SET, &[0, if i == 0 { 0xFF } else { 0xFE }]);
        }
        if i == 30 {
            a.call(op::EOD_TEXT, &[day]);
        }
        if a.call(op::EOD_POLL, &[]) != 0 {
            if i < 30 {
                a.call(op::EOD_TEXT, &[day]);
                a.call(op::PAL_SET, &[0, 0xFF]);
            }
            skipped = true;
        }
        if i % 4 == 0 && a.random(5) == 0 {
            let x = a.random(2) + 1;
            let y = a.random(10) + 5;
            let z = a.random(4) + 0x13;
            a.call(op::SFX, &[z, 0x2B11, y, x]);
        }
    }
    // 3.
    a.call(op::EOD_SOUND_STOP, &[]);
    // 4.
    let c = unsafe { (*w).cur_sector };
    let stranded = a.call(op::CONNECTED_HOME, &[c as i32]) as u8 == 0
        && unsafe { (*w).leave_reason == 0 && (*w).santino_spoke == 0 }
        && (a.story(0x1C) == 0 || a.story(0x0E) == 2);
    if stranded {
        if skipped {
            a.call(op::PAL_SET, &[0, 0xFE]);
        }
        a.call(op::OVERHEAD_SOUNDS, &[]);
        retreat(w, h);
        a.call(op::SHOW_SCREEN, &[]);
        a.call(op::FADE, &[0, 0x38, 0x100, 0]);
    } else {
        a.call(op::FADE, &[0, 0x18, 0x100, 0]);
        a.call(op::OVERHEAD_SOUNDS, &[]);
    }
    // 5. (only record 0, eight times)
    for _ in 0..8 {
        if a.rd8(0x17E) >= 8 {
            a.wr8(0x17E, 0);
        }
    }
}

/// Retreat: each merc in the sector may be hit on the way out; a merc left below 15 hp dies.
fn retreat(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    let c = unsafe { (*w).cur_sector };
    let b = sector8(w, c, 0);
    // 1.
    let ci = c as i32;
    let sides = [
        (c > 9, ci - 10, 0x02u8),
        (c % 10 != 9, ci + 1, 0x04),
        (c < 50, ci + 10, 0x08),
        (c % 10 != 0, ci - 1, 0x10),
    ];
    let mut way_out = false;
    for (edge, n, bit) in sides {
        if edge && a.call(op::CONNECTED_HOME, &[n]) as u8 != 0 && b & bit == 0 {
            way_out = true;
        }
    }
    // 2.
    // F21 (1.15m 0x30B28 / 0x30C4C): a way out lowers the risk instead of raising it (2 for an unowned sector, minus
    // the way out); the risk can be -30 then, and no merc gets hit.
    let risk = if crate::fanfix::on(21) {
        30 * (2 * (b & 1 == 0) as i32 - way_out as i32)
    } else {
        30 * ((b & 1 == 0) as i32 + way_out as i32)
    };
    // 3.
    for k in 0..8 {
        let Some(m) = Ai::new(w, h, k) else { return };
        if m.rd8(0x182) == 0 || m.rd8(0x138) == 0 || m.rd8(0x188) != 0 {
            continue;
        }
        // a.
        if a.random(100) <= risk {
            a.call(op::SHOW_SCREEN, &[]);
            a.call(op::DELAY, &[200]);
            let d = if risk == 0 {
                a.call(op::DIV_ZERO, &[]);
                1
            } else {
                a.random(risk) + 1
            };
            retreat_hit(w, h, k, d);
            a.call(op::MERC_TEXT, &[k, 3, 1500]);
        }
        // b.
        if (m.rd8(0x183) as i8) < 15 {
            a.call(op::KILL, &[k, 1]);
            a.call(op::SHOW_SCREEN, &[]);
            a.call(op::DELAY, &[200]);
            a.call(op::MERC_TEXT, &[k, 4, 1500]);
        }
    }
}

/// Retreat hit (k, d): the hit goes through hit_location; hp drops by at most what is left.
fn retreat_hit(w: *mut Ja1World, h: *const Ja1Host, k: i32, d: i32) {
    let Some(m) = Ai::new(w, h, k) else { return };
    let mut r = m.call(op::HIT_LOCATION, &[k, 0x9C4, d]) as i32;
    if r == 0 {
        return;
    }
    let hp = m.rd8(0x183) as i8 as i32;
    if r > hp {
        r = hp;
    }
    m.wr8(0x183, (hp - r) as u8);
    m.call(op::STAT_PRACTICE, &[k, 9, r & 0xFF, 0]);
    let slot = if (m.rd8(0x183) as i8) < 15 { 3 } else { 2 };
    m.call(op::EFFECT_SLOT, &[k, slot]);
}
