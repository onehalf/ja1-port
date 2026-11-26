//! Sector battles (docs/spec/strategic.md "Sector battles"; C interface in docs/spec/port_api.md):
//! `ja1_resolve_battle` (library op 218) and `ja1_sector_gained` (library op 201). A child module of `ai`; random,
//! the show (host ops 332 battle_ui, 333 battle_wait, 334 battle_box, 335 battle_msg), op 89 replay_event and op
//! 316 refresh go through `Ai`. Attack weight is that of `enemy`, Workable trees and Territory those of `ledger`,
//! Time compression (0) is the clock's.
use super::clock::time_compress;
use super::damage::{get8, put8};
use super::enemy::attack_weight;
use super::ledger::{add, recalc_territory, workable, wr};
use super::{op, Ai, Ja1Host, Ja1World};

const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
// accounts (Day accounts)
const SECTORS_GAINED: i64 = 0x78;
const SECTORS_LOST: i64 = 0x7C;
const LAST_GAINED: i64 = 0x80;
// sector_state bytes
const ENEMIES: i64 = 1;
const TAPPERS: i64 = 3;
const GUARDS: i64 = 4;
// sector record bytes
const SLOTS: i64 = 0x96;
const KEEP: i64 = 0xAA;
const MINES: i64 = 0xAF;
const MINES_USED: i64 = 0xB0;

fn wm<'a>(w: *mut Ja1World) -> &'a mut Ja1World {
    unsafe { &mut *w }
}
fn st(w: *mut Ja1World, s: u8, k: i64) -> u8 {
    get8(wm(w).sector_state, SECTOR_STATE_LEN, s as i64 * 9 + k)
}
fn st_put(w: *mut Ja1World, s: u8, k: i64, v: u8) {
    put8(wm(w).sector_state, SECTOR_STATE_LEN, s as i64 * 9 + k, v)
}
fn rec(w: *mut Ja1World, s: u8, o: i64) -> u8 {
    get8(wm(w).sectors, SECTORS_LEN, s as i64 * 0xB1 + o)
}
fn rec_put(w: *mut Ja1World, s: u8, o: i64, v: u8) {
    put8(wm(w).sectors, SECTORS_LEN, s as i64 * 0xB1 + o, v)
}
fn s16(x: i32) -> i32 {
    x as i16 as i32
}

/// Workable trees of s and of its neighbours (Set owned).
fn neighbours(w: *mut Ja1World, s: u8) {
    let (row, col) = (s / 10, s % 10);
    workable(w, s);
    if row < 5 {
        workable(w, s.wrapping_add(10));
    }
    if row > 0 {
        workable(w, s.wrapping_sub(10));
    }
    if col < 9 {
        workable(w, s.wrapping_add(1));
    }
    if col > 0 {
        workable(w, s.wrapping_sub(1));
    }
}

/// Set owned (s, v).
pub(super) fn set_owned(w: *mut Ja1World, s: u8, v: bool) {
    let b = st(w, s, 0);
    st_put(w, s, 0, if v { b | 1 } else { b & 0xFE });
    neighbours(w, s);
}

/// Sector gained (op 201).
fn sector_gained(w: *mut Ja1World, h: *const Ja1Host, s: u8) {
    let ai = Ai { w, h, s: 0 };
    let b = st(w, s, 0);
    st_put(w, s, 0, b | 1);
    neighbours(w, s);
    add(w, SECTORS_GAINED, 1);
    wr(w, LAST_GAINED, s as i8 as i32);
    let si = s as i8 as i32;
    ai.call(op::REPLAY_EVENT, &[si, si, 0x4F]);
}

struct Show<'a> {
    ai: &'a Ai,
    skip: bool,
}

impl Show<'_> {
    fn ui(&self, k: i32, n: i32) {
        self.ai.call(op::BATTLE_UI, &[k, n]);
    }
    /// Wait (ms).
    fn wait(&mut self, ms: i32) {
        self.skip = self.ai.call(op::BATTLE_WAIT, &[ms]) as u8 != 0;
    }
    /// Pick a sound (m, last).
    fn pick(&self, m: i32, last: &mut i32) -> i32 {
        let mut n = self.ai.random(m);
        while n == *last {
            n = self.ai.random(m);
        }
        *last = n;
        n
    }
}

/// An enemy falls.
fn enemy_falls(w: *mut Ja1World, ai: &Ai, s: u8) {
    wm(w).enemy_deaths = wm(w).enemy_deaths.wrapping_add(1);
    st_put(w, s, ENEMIES, st(w, s, ENEMIES).wrapping_sub(1));
    let si = s as i8 as i32;
    ai.call(op::REPLAY_EVENT, &[si, si, 0x54]);
    for k in (0..8).rev() {
        let v = rec(w, s, SLOTS + k) as i8;
        if v > 0 {
            rec_put(w, s, SLOTS + k, v.wrapping_neg() as u8);
            break;
        }
    }
}

fn resolve_battle(w: *mut Ja1World, h: *const Ja1Host, s: u8, f: u8) {
    let ai = Ai { w, h, s: 0 };
    let mut sh = Show { ai: &ai, skip: false };
    let si = s as i8 as i32;
    let n = s as i32 + 1;
    let (mut fought, mut told, mut sounds) = (false, false, false);
    let (mut last_die, mut last_mine) = (0xFF, 0xFF);
    // 1.
    sh.ui(0, 0);
    if wm(w).time_compress != 0 {
        time_compress(w, h, 0);
    }
    ai.call(op::REFRESH, &[]);
    // Fan fix F27 (1.15m ResolveSectorBattle 0x3E54D..0x3E56B, #70 row 22): the key byte 0x95DC3 is cleared after
    // the refresh, so an old Escape left there no longer skips the whole show at the first click (WaitMs? answers
    // "skip" by that byte). The library has no key byte: battle_ui kind 9 asks the host to clear it.
    if crate::fanfix::on(27) {
        sh.ui(9, 0);
    }
    // 2. Guards fight.
    if st(w, s, GUARDS) != 0 || st(w, s, TAPPERS) != 0 {
        fought = true;
        told = true;
        ai.call(op::REPLAY_EVENT, &[si, si, 0x4D]);
        sh.ui(1, 0);
        sounds = true;
        sh.ui(2, 0);
        ai.call(op::BATTLE_BOX, &[s as i32, st(w, s, ENEMIES) as i32, st(w, s, GUARDS) as i32, st(w, s, TAPPERS) as i32, 99]);
        sh.ui(5, 0);
        let a = attack_weight(w, s, f);
        let wt = wm(w).guard_skill as i32 / 2 + 40;
        if st(w, s, ENEMIES) != 0 {
            loop {
                let (t1, t3, t4) = (st(w, s, ENEMIES) as i32, st(w, s, TAPPERS) as i32, st(w, s, GUARDS) as i32);
                if t4 == 0 && t3 == 0 {
                    break;
                }
                let g = wt.wrapping_mul(t4);
                let p = 2 * t3;
                let m = rec(w, s, MINES) as i32;
                let x = m * t1 * 5;
                let d = p.wrapping_add(g).wrapping_add(x);
                let k = m * (t4 + t3);
                let att = s16(t1.wrapping_mul(a).wrapping_add(k));
                if att == 0 || s16(d) == 0 {
                    break;
                }
                let r = s16(ai.random(s16(d.wrapping_add(att))));
                let mine;
                let kind;
                if r < att {
                    mine = r < s16(k);
                    if ai.random(s16(5 * p + g)) < s16(g) {
                        st_put(w, s, GUARDS, (t4 as u8).wrapping_sub(1));
                        wm(w).dead_npc = wm(w).dead_npc.wrapping_add(1);
                        ai.call(op::REPLAY_EVENT, &[si, si, 0x52]);
                        kind = 1;
                    } else {
                        st_put(w, s, TAPPERS, (t3 as u8).wrapping_sub(1));
                        wm(w).dead_allies = wm(w).dead_allies.wrapping_add(1);
                        ai.call(op::REPLAY_EVENT, &[si, si, 0x53]);
                        kind = 2;
                    }
                } else {
                    mine = r < att + s16(x);
                    enemy_falls(w, &ai, s);
                    kind = 0;
                }
                if !sh.skip {
                    let ms = ai.random(2000) + 800;
                    sh.wait(ms);
                }
                if !sh.skip {
                    if mine {
                        let n = sh.pick(3, &mut last_mine);
                        sh.ui(4, n);
                    } else {
                        let n = sh.pick(5, &mut last_die);
                        sh.ui(3, n);
                    }
                }
                if mine {
                    rec_put(w, s, MINES, rec(w, s, MINES).wrapping_sub(1));
                    rec_put(w, s, MINES_USED, rec(w, s, MINES_USED).wrapping_add(1));
                }
                if !sh.skip {
                    sh.wait(500);
                } else {
                    sh.ui(6, 0);
                }
                if !sh.skip {
                    ai.call(
                        op::BATTLE_BOX,
                        &[s as i32, st(w, s, ENEMIES) as i32, st(w, s, GUARDS) as i32, st(w, s, TAPPERS) as i32, kind],
                    );
                    sh.ui(5, 0);
                } else {
                    sh.ui(6, 0);
                }
                if st(w, s, ENEMIES) == 0 {
                    break;
                }
            }
        }
        if !sh.skip {
            sh.wait(1000);
        }
        if !sh.skip {
            sh.ui(6, 0);
        }
        ai.call(op::REFRESH, &[]);
    }
    // 3. Mines.
    while st(w, s, ENEMIES) != 0 && rec(w, s, MINES) != 0 {
        if !sounds {
            sh.ui(1, 0);
            sounds = true;
        }
        let c = s16(st(w, s, ENEMIES) as i32 * rec(w, s, MINES) as i32 * 5);
        if ai.random(100) >= c {
            break;
        }
        ai.call(op::BATTLE_MSG, &[0, n]);
        if !sh.skip {
            let n = sh.pick(3, &mut last_mine);
            sh.ui(4, n);
            sh.ui(5, 0);
            let ms = ai.random(2000) + 800;
            sh.wait(ms);
        } else {
            sh.ui(6, 0);
        }
        if !told {
            told = true;
            ai.call(op::REPLAY_EVENT, &[si, si, 0x4D]);
        }
        enemy_falls(w, &ai, s);
        rec_put(w, s, MINES, rec(w, s, MINES).wrapping_sub(1));
        rec_put(w, s, MINES_USED, rec(w, s, MINES_USED).wrapping_add(1));
        ai.call(op::REFRESH, &[]);
        if !sh.skip {
            sh.wait(500);
        }
    }
    // 4.
    if sounds {
        sh.ui(7, 0);
    }
    // 5. The outcome.
    let owned = st(w, s, 0) & 1 != 0;
    if st(w, s, ENEMIES) == 0 {
        if !owned {
            ai.call(op::BATTLE_MSG, &[1, n]);
            set_owned(w, s, true);
            wr(w, LAST_GAINED, s as i8 as i32);
            add(w, SECTORS_GAINED, 1);
            ai.call(op::REPLAY_EVENT, &[si, si, 0x4F]);
        } else {
            ai.call(op::BATTLE_MSG, &[2, n]);
            ai.call(op::REPLAY_EVENT, &[si, si, 0x50]);
        }
    } else if owned {
        ai.call(op::BATTLE_MSG, &[if fought { 4 } else { 3 }, n]);
        set_owned(w, s, false);
        add(w, SECTORS_LOST, 1);
        ai.call(op::REPLAY_EVENT, &[si, si, 0x4E]);
        rec_put(w, s, KEEP, 1);
    } else {
        ai.call(op::BATTLE_MSG, &[5, n]);
        ai.call(op::REPLAY_EVENT, &[si, si, 0x4E]);
    }
    // 6.
    recalc_territory(w);
    sh.ui(8, 0);
    ai.call(op::REFRESH, &[]);
}

/// Library op 218 (s, f).
pub(crate) fn resolve_battle_op(w: *mut Ja1World, h: *const Ja1Host, s: u8, f: u8) {
    resolve_battle(w, h, s, f)
}

/// Library op 201 (s8 s).
pub(crate) fn sector_gained_op(w: *mut Ja1World, h: *const Ja1Host, s: u8) {
    sector_gained(w, h, s)
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed); a sector above 59 reads and
// writes nothing outside the tables (get8 / put8 bounds).
#[no_mangle]
pub extern "C" fn ja1_resolve_battle(w: *mut Ja1World, h: *const Ja1Host, s: i32, f: i32) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolve_battle(w, h, s as u8, f as u8)));
}

#[no_mangle]
pub extern "C" fn ja1_sector_gained(w: *mut Ja1World, h: *const Ja1Host, s: i32) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| sector_gained(w, h, s as u8)));
}
