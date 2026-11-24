//! The story-object find scene (TacticalLoop 0x10BFB -> 0x6179C; C interface `ja1_story_find`): a merc saw a story
//! tile (story_finder, soldier_update.md "Merc view"). Not hit yet (story[0x15] = 0): quotes 0x18 / 0x19 / 0x1A,
//! Brenda, and one merc takes the object away on duty 7 (ESCORT) until the morning; the story tiles and the sector's
//! story slots are cleared. Already hit: quote 0x0B or 0x14 and the other mercs' quote 0x0C. Then story[0x16] = 1
//! (no more story objects) and the mission step = op 33 radio_result. A child module of `ai`: every op goes through
//! `Ai::call` (the host_ops rule); Forget soldier, Vacate tile, the move codes and First merc are plain procedures.
//! The front-end runs it when story_finder < 0x40 and the finder's hp >= 15, then sets story_finder = 0x40.
use super::damage::{get16, get8, put16, put8};
use super::{op, Ai, Ja1Host, Ja1World, NONE, NO_SLOT, TILES};

const STORY_LEN: usize = 35;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
/// s16 table 0x95CA8 by difficulty: story[0x14] below it -> quote 0x0B, else 0x14.
const HIT_LIMIT: [i16; 3] = [10, 8, 6];
/// SPEC GAP: the original's Random loops never end when no merc fits; the port gives up after this many tries.
const TRIES: usize = 100_000;

fn story(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).story }, STORY_LEN, i)
}
fn story_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(unsafe { (*w).story }, STORY_LEN, i, v)
}

impl Ai {
    /// In the sector, on no duty, hp >= 15 (the scene's head count).
    fn sf_ready(&self, k: usize) -> bool {
        self.rd8_of(k, 0x182) != 0 && self.rd8_of(k, 0x17E) == 0 && self.rd8_of(k, 0x183) as i8 >= 15
    }
    fn sf_say(&self, k: usize, q: i32) {
        self.call(op::MERC_MESSAGE, &[k as i32, q, NO_SLOT as i32, 0, 1]);
    }
    /// t1 of tile t = 0, then its move code again (MoveCode 0x2BE54).
    fn sf_clear_tile(&self, t: i32) {
        let w = unsafe { &*self.w };
        put8(w.tiles, TILES * 8, 8 * t as i64 + 1, 0);
        // SPEC GAP: a null t0_class / t1_class / move_codes buffer skips the rebuild.
        if w.t0_class.is_null() || w.t1_class.is_null() || w.move_codes.is_null() {
            return;
        }
        unsafe {
            let (gc, oc) = (&*(w.t0_class as *const [u8; 256]), &*(w.t1_class as *const [u8; 256]));
            let i = t as usize;
            let prev = u16::from_le_bytes([*w.move_codes.add(2 * i), *w.move_codes.add(2 * i + 1)]);
            let code = crate::move_code(self.tb(t, 0), self.tb(t, 1), self.tb(t, 3), prev, gc, oc);
            let [lo, hi] = code.to_le_bytes();
            *w.move_codes.add(2 * i) = lo;
            *w.move_codes.add(2 * i + 1) = hi;
        }
    }
}

fn story_find(w: *mut Ja1World, h: *const Ja1Host) {
    let finder = unsafe { (*w).story_finder };
    // SPEC GAP: a finder outside the 48 records does nothing.
    let Some(a) = Ai::new(w, h, finder as i32) else { return };
    // 1. The head count of soldiers 0..7.
    let count = (0..8).filter(|&k| a.sf_ready(k)).count() as i32;
    if count == 0 {
        return;
    }
    a.call(op::FOCUS, &[finder as i32, 0]);
    let cur = unsafe { (*w).cur_sector };
    let sc = cur as i8 as i32;
    let mut s = finder as usize;
    let mut all_gone = false;
    if story(w, 0x15) == 0 {
        // 2. Not hit: the find.
        a.sf_say(s, 0x18);
        a.call(op::REDRAW, &[]);
        if count <= 1 {
            a.call(op::BRENDA_SAY, &[2]);
            all_gone = true;
        } else {
            a.call(op::BRENDA_SAY, &[0x0C]);
            for _ in 0..TRIES {
                let k = a.random(8) as usize;
                let ok = a.rd8_of(k, 0x182) != 0
                    && a.rd8_of(k, 0x138) != 0
                    && a.rd8_of(k, 0x17E) == 0
                    && a.rd8_of(k, 0x183) as i8 >= 15
                    && a.rd8_of(k, 0x17A) != finder;
                if ok {
                    s = k;
                    break;
                }
            }
            a.sf_say(s, 0x19);
            a.call(op::REDRAW, &[]);
            // The best paid merc on duty carries it (strictly more pay replaces).
            for k in 0..8 {
                if a.rd8_of(k, 0x138) != 0 && a.sf_ready(k) && a.rd16_of(k, 0x110) > a.rd16_of(s, 0x110) {
                    s = k;
                }
            }
        }
        // 3. Merc s leaves on duty 7.
        let Some(m) = Ai::new(w, h, s as i32) else { return };
        let me = m.rd8(0x17A);
        unsafe { (*w).duty7_merc = me };
        a.sf_say(s, 0x1A);
        a.call(op::MOUSE_OFF, &[]);
        a.call(op::REDRAW, &[]);
        a.call(op::FADE, &[0, 0x38, 0x100, 0]);
        a.call(op::IDLE, &[s as i32]);
        m.forget_soldier();
        let selected = move || unsafe { (*w).selected };
        if selected() == m.rd8(0x17A) {
            a.call(op::CURSOR_BACK, &[]);
        }
        if m.rd8(0x1B9) != 0 {
            a.call(op::END_TASK, &[s as i32, 0]);
        }
        m.wr16(0x126, m.rd16(0x116));
        m.wr16(0x128, m.rd16(0x118));
        if selected() == m.rd8(0x17A) {
            unsafe { (*w).view_hold = 0 };
        }
        for o in [0x185, 0x17F, 0x181, 0x189, 0x1D5] {
            m.wr8(o, 0);
        }
        if m.rd8(0x17A) == selected() {
            a.call(op::CLICK_PANEL, &[s as i32, 0]);
        }
        m.wr8(0x17E, 7);
        m.wr8(0x138, 0);
        m.wr16(0xE4, NONE);
        m.wr16(0xE6, NONE);
        m.wr8(0x1BF, 0);
        m.wr8(0x1C4, 0);
        m.wr8(0x195, 0);
        m.vacate(m.s16_at(0x114));
        let ss = unsafe { (*w).sector_state };
        let i = 9 * cur as i64 + 2;
        put8(ss, SECTOR_STATE_LEN, i, get8(ss, SECTOR_STATE_LEN, i).wrapping_sub(1));
        unsafe { (*w).team_on_duty = (*w).team_on_duty.wrapping_sub(1) };
        a.call(op::MOVE_SOUND, &[m.rd8(0x17A) as i32, 1]);
        a.call(op::REPLAY_EVENT, &[sc, sc, 0x5A]);
        let d7 = unsafe { (*w).duty7_merc };
        if selected() == d7 {
            let k = super::turns::first_merc(w, h);
            if k < NO_SLOT as i32 {
                a.call(op::LOW_AP_STOP, &[k, 1, 0]);
            }
        }
        unsafe {
            if (*w).shown_slot == d7 as i32 {
                (*w).shown_slot = NO_SLOT as i32;
            }
            // The lit face slot 0x284F0E is front-end state: the front-end clears it.
            if (*w).active_slot == d7 {
                (*w).active_slot = NO_SLOT;
            }
        }
        // 4. The story tiles and the sector's story slots go.
        let tiles_ok = unsafe { !(*w).tiles.is_null() };
        for t in (0..TILES as i32).filter(|_| tiles_ok) {
            if (0xAC..=0xAF).contains(&a.tb(t, 1)) {
                a.sf_clear_tile(t);
            }
        }
        // SPEC GAP: a current sector outside the 60 records clears no slot.
        if cur < 60 {
            let p = unsafe { (*w).sectors };
            let r = cur as i64 * 0xB1;
            for k in 0..5 {
                if (0xAC..=0xAF).contains(&get8(p, SECTORS_LEN, r + 0x9F + k)) {
                    put8(p, SECTORS_LEN, r + 0x9F + k, 0);
                    put16(p, SECTORS_LEN, r + 0x82 + 2 * k, NONE);
                }
            }
        }
        story_put(w, 0x17, 0xFF);
        a.call(op::REDRAW, &[]);
        a.call(op::FADE, &[0, 0x38, 0, 0x100]);
    } else {
        // 5. Already hit.
        let d = unsafe { (*w).difficulty } as usize;
        // SPEC GAP: a difficulty above 2 uses the hard limit.
        let q = if (story(w, 0x14) as i16) < HIT_LIMIT[d.min(2)] { 0x0B } else { 0x14 };
        a.sf_say(s, q);
        a.call(op::REDRAW, &[]);
        if count > 1 {
            let mut said = [false; 8];
            said[finder as usize & 7] = finder < 8;
            for _ in 0..count - 1 {
                for _ in 0..TRIES {
                    let k = a.random(8) as i8;
                    if !(0..8).contains(&k) {
                        continue; // SPEC GAP: the original reads past its 8 flags
                    }
                    let k = k as usize;
                    if a.sf_ready(k) && !said[k] {
                        s = k;
                        a.sf_say(k, 0x0C);
                        a.call(op::REDRAW, &[]);
                        said[k] = true;
                        break;
                    }
                }
            }
        }
    }
    // 6.
    if all_gone {
        a.call(op::REPLAY_EVENT, &[sc, sc, 0x59]);
        Ai::deselect(w, a.h, 1);
        unsafe { (*w).selected = a.rd8_of(s, 0x17A) };
        a.call(op::SQUAD_LOST, &[]);
    }
    story_put(w, 0x16, 1);
    let m = a.call(op::RADIO_RESULT, &[]);
    unsafe {
        (*w).mission_step = m as u8 as i8;
        (*w).mission_slot = NO_SLOT;
    }
}

/// StoryHit? 0x61D28, library op 177 story_hit (a shot, a melee hit or a blast hit a story object; tools/ai.py
/// story_hit): story byte 0x15 = 1 (Brenda dead). For the current sector's 5 story slots (record bytes +0x9F..+0xA3,
/// tiles at the words +0x82..+0x8A): 0xAE becomes 0xAC and 0xAF becomes 0xAD, and a slot that holds 0xAC / 0xAD puts
/// that object on its tile (tile byte 1). Then mission_step = radio_result, mission_slot = 0x40.
/// (Until 2026-11-08 no front-end handled op 177: an order enemy shot Brenda's tile turn after turn and she lived;
/// the original kills her on the first hit, ja1-re#61.)
fn story_hit(w: *mut Ja1World, h: *const Ja1Host) {
    let Some(a) = Ai::new(w, h, 0) else { return };
    story_put(w, 0x15, 1);
    let cur = unsafe { (*w).cur_sector };
    // SPEC GAP: a current sector outside the 60 records changes no slot and no tile (the original reads past them).
    if cur < 60 {
        let (p, tiles) = unsafe { ((*w).sectors, (*w).tiles) };
        let r = cur as i64 * 0xB1;
        for k in 0..5 {
            let tile = get16(p, SECTORS_LEN, r + 0x82 + 2 * k) as i16 as i64;
            for (hit, broken) in [(0xAE, 0xAC), (0xAF, 0xAD)] {
                if get8(p, SECTORS_LEN, r + 0x9F + k) == hit {
                    put8(p, SECTORS_LEN, r + 0x9F + k, broken);
                }
                let v = get8(p, SECTORS_LEN, r + 0x9F + k);
                if v == hit || v == broken {
                    // SPEC GAP: a tile outside the map is not written (the original writes past the map).
                    put8(tiles, TILES * 8, 8 * tile + 1, broken);
                }
            }
        }
    }
    let m = a.call(op::RADIO_RESULT, &[]);
    unsafe {
        (*w).mission_step = m as u8 as i8;
        (*w).mission_slot = NO_SLOT;
    }
}

pub(crate) fn story_hit_op(w: *mut Ja1World, h: *const Ja1Host) {
    story_hit(w, h)
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_story_hit(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| story_hit(w, h)));
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_story_find(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| story_find(w, h)));
}
