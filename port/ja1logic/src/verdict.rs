//! The day's verdict (docs/spec/strategic.md "The day's verdict"; C interface in docs/spec/port_api.md):
//! `ja1_day_verdict` (crate check, the day's score, the end of the game, the guide's words on the day) and
//! `ja1_day_bonus`. A child module of `ai`; host ops 302 guide_say, 317 brenda_say, 308 stash_take, 330
//! home_lost_msg, 331 rating_word and random go through `Ai`. Squad search is that of `storyday`, Plants that of
//! `ledger`, Connected to home that of `home`, Crate lost that of `sector`.
use super::damage::{get16, get8, put16, put8};
use super::home::connected_home;
use super::ledger::{calc_process_cap, rd};
use super::storyday::squad_search;
use super::{op, Ai, Ja1Host, Ja1World};

const STORY_LEN: usize = 43;
const ACCOUNTS_LEN: usize = 0xA8;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const PLANTS_LEN: usize = 4 * 12;
// accounts (Day accounts)
const TREES_WORKABLE: i64 = 0x14;
const TREES_OWNED: i64 = 0x18;
const TREES_TAPPED: i64 = 0x1C;
const DAY_INCOME: i64 = 0x20;
const BALANCE: i64 = 0x24;
/// The accounts byte +0x91 (the "byte at 0x22A031" of the Worker pool).
const LOST_DAYS: i64 = 0x91;
const DAY_NET: i64 = 0x2C;
const DAY_EXPENSES: i64 = 0x64;
const SECTORS_OWNED: i64 = 0x74;
const PROCESS_CAP: i64 = 0x84;
const PLANTS_OWNED: i64 = 0x88;
const COUNT_92: i64 = 0x92;
const COUNT_94: i64 = 0x94;
const SUCCESS: i64 = 0x9A;
const STREAK: i64 = 0x9B;
// sector_state bytes
const TREES: i64 = 6;
const UNHEALTHY: i64 = 8;
const HOME: i32 = 0x3B;
const NONE: i32 = 999;
// the limits by difficulty
const L1: [i32; 3] = [10, 8, 6];
const L2: [i32; 3] = [3, 2, 1];
const L3: [u16; 3] = [10, 15, 20];
const L4: [u16; 3] = [20, 25, 30];
const L5: [i32; 3] = [12, 10, 8];
const LIST: [i32; 5] = [28, 29, 38, 48, 49];

fn s(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).story }, STORY_LEN, i)
}
fn s8(w: *mut Ja1World, i: i64) -> i32 {
    s(w, i) as i8 as i32
}
fn s_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(unsafe { (*w).story }, STORY_LEN, i, v)
}
fn a8(w: *mut Ja1World, o: i64) -> u8 {
    get8(unsafe { (*w).accounts }, ACCOUNTS_LEN, o)
}
fn a8_put(w: *mut Ja1World, o: i64, v: u8) {
    put8(unsafe { (*w).accounts }, ACCOUNTS_LEN, o, v)
}
fn a16_add(w: *mut Ja1World, o: i64, v: u16) {
    let p = unsafe { (*w).accounts };
    put16(p, ACCOUNTS_LEN, o, get16(p, ACCOUNTS_LEN, o).wrapping_add(v));
}
fn a32_put(w: *mut Ja1World, o: i64, v: i32) {
    let p = unsafe { (*w).accounts };
    put16(p, ACCOUNTS_LEN, o, v as u16);
    put16(p, ACCOUNTS_LEN, o + 2, (v as u32 >> 16) as u16);
}
/// Byte i of the sector_state entry of sector n (SPEC GAP: outside 0..79 reads 0).
fn sec(w: *mut Ja1World, n: i32, i: i64) -> u8 {
    if !(0..80).contains(&n) {
        return 0;
    }
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n as i64 * 9 + i)
}
fn owned(w: *mut Ja1World, n: i32) -> bool {
    sec(w, n, 0) & 1 != 0
}
fn plant8(w: *mut Ja1World, i: i64, o: i64) -> u8 {
    get8(unsafe { (*w).plants }, PLANTS_LEN, 12 * i + o)
}
fn plant_put(w: *mut Ja1World, i: i64, o: i64, v: u8) {
    put8(unsafe { (*w).plants }, PLANTS_LEN, 12 * i + o, v)
}
/// The low byte of the plant's `sector`.
fn plant_sector(w: *mut Ja1World, i: i64) -> i32 {
    plant8(w, i, 4) as i32
}
const STATE: i64 = 8;
const COUNTDOWN: i64 = 0xB;
fn d_index(w: *mut Ja1World) -> usize {
    // SPEC GAP: a difficulty other than 0..2 uses the values of 2.
    (unsafe { (*w).difficulty } as usize).min(2)
}
fn day(w: *mut Ja1World) -> i32 {
    unsafe { (*w).day }
}
fn by_score(q: i32, score: i32) -> i32 {
    q + score.signum()
}

fn set_guard_skill(w: *mut Ja1World, v: u8) {
    unsafe { (*w).guard_skill = v };
}

/// Crate check.
fn crate_check(w: *mut Ja1World, h: *const Ja1Host) {
    set_guard_skill(w, 0);
    let cr = |k: i64, i: i64| get8(unsafe { (*w).crates }, 16, 4 * k + i);
    let st = |k: i64| get8(unsafe { (*w).crate_state }, 4, k);
    let st_put = |k: i64, v: u8| put8(unsafe { (*w).crate_state }, 4, k, v);
    for k in 0..4i64 {
        let sector = cr(k, 0) as i8 as i32;
        let kept = cr(k, 3);
        let settle = |v: u8| {
            if owned(w, sector) {
                st_put(k, v);
            } else if kept != 0 {
                st_put(k, 4);
            } else {
                crate::sector::ja1_crate_lost(w, h, k as i32);
            }
        };
        if cr(k, 1) as i8 as i32 == unsafe { (*w).day } {
            if st(k) == 0 {
                settle(5);
            }
            if st(k) == 1 {
                settle(2);
            }
        }
        if st(k) == 2 && kept == 0 {
            set_guard_skill(w, unsafe { (*w).guard_skill }.wrapping_add(10));
        }
    }
}

/// The day's score, −3..3 (all sums 16-bit).
fn day_score(w: *mut Ja1World) -> i32 {
    let wd = unsafe { &*w };
    let lo = |v: i32| v as i16;
    let x = lo(rd(w, TREES_OWNED))
        .wrapping_sub(lo(wd.start_trees))
        .wrapping_add(lo(rd(w, TREES_WORKABLE)).wrapping_sub(lo(wd.start_workable)))
        .wrapping_add(lo(rd(w, TREES_TAPPED)).wrapping_sub(lo(wd.start_tapped)))
        .wrapping_add(lo(rd(w, SECTORS_OWNED)).wrapping_sub(lo(wd.start_sectors)).wrapping_mul(5))
        .wrapping_add(lo(rd(w, PLANTS_OWNED)).wrapping_sub(lo(wd.start_plants)).wrapping_mul(10))
        .wrapping_add(lo(wd.enemy_deaths))
        .wrapping_sub(lo(wd.dead_mercs).wrapping_mul(5))
        .wrapping_sub(lo(wd.dead_npc))
        .wrapping_sub(lo(wd.dead_allies));
    let xi = x as i32;
    let mut v: i16 = if xi.abs() < 5 {
        0
    } else if xi > 0 {
        ((xi + 5) / 10) as i16
    } else {
        ((xi - 5) / 10) as i16
    };
    v = v.wrapping_sub((wd.day / 50) as i16);
    if wd.day > 250 && v >= 0 {
        v = -1;
    }
    if wd.idle_day != 0 && v > 0 {
        v = 0;
    }
    if v < 0 {
        let streak = a8(w, STREAK).wrapping_add(1);
        a8_put(w, STREAK, streak);
        if streak >= 3 {
            v = v.wrapping_sub(1);
        }
        if streak == 4 {
            v = v.wrapping_sub(1);
        }
    } else {
        a8_put(w, STREAK, 0);
        a8_put(w, SUCCESS, a8(w, SUCCESS).wrapping_add(v as u8));
    }
    (v as i32).clamp(-3, 3)
}

struct V<'a> {
    w: *mut Ja1World,
    ai: &'a Ai,
    said: bool,
}

impl V<'_> {
    fn say(&self, q: i32) {
        self.ai.call(op::GUIDE_SAY, &[q, 3]);
    }
    fn brenda(&self, q: i32) {
        self.ai.call(op::BRENDA_SAY, &[q]);
    }
    fn search(&self, t: u8, take: bool, store: bool) -> bool {
        squad_search(self.w, self.ai, t, take, store) != -1
    }
    /// s8 S[0xB] = 1 or S[0x15] ≠ 0.
    fn ransom_open(&self) -> bool {
        s8(self.w, 0xB) == 1 || s(self.w, 0x15) != 0
    }
    /// S[0x21] = 1, then Brenda's words on the cure by S[0xE] / S[0x1E].
    fn cure_words(&self, early: i32, late: i32) {
        let w = self.w;
        s_put(w, 0x21, 1);
        if s8(w, 0xE) < 2 || s(w, 0x1E) != 0 {
            self.brenda(early);
            s_put(w, 0x22, 3);
        } else {
            self.brenda(late);
            s_put(w, 0x22, 1);
        }
    }
}

fn day_verdict(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let ai = Ai { w, h, s: 0 };
    let mut v = V { w, ai: &ai, said: false };
    let d = d_index(w);
    let mut late = false;
    let mut q = NONE;

    // 1.
    crate_check(w, h);
    let score = day_score(w);
    // 2.-4. The end of the game.
    if !owned(w, HOME) {
        ai.call(op::HOME_LOST_MSG, &[]);
        return 4;
    }
    if s8(w, 0xA) == 1 && !(0..60).any(|n| owned(w, n) && sec(w, n, TREES) != 0) {
        v.say(0xB2);
        return 3;
    }
    if a8(w, STREAK) > 4 {
        v.say(0x95);
        return 2;
    }
    // 5.
    ai.call(op::RATING_WORD, &[score]);
    // 6. Ransom.
    if s8(w, 0xB) == 1 {
        v.said = true;
        if s(w, 0x16) == 0 {
            let n14 = s(w, 0x14).wrapping_add(1);
            s_put(w, 0x14, n14);
            let mut told = false;
            for (j, item, quote) in [(0i64, 0x86u8, 0xAB), (1, 0x7A, 0xA0), (2, 0x5C, 0x9F)] {
                let it = unsafe { (*w).item_told };
                if get8(it, 3, j) == 0 && v.search(item, false, false) {
                    v.say(quote);
                    put8(it, 3, j, 1);
                    told = true;
                }
            }
            if !told {
                if s(w, 0x14) < 11 {
                    v.say(s(w, 0x14) as i32 + 0xA0);
                } else {
                    v.said = false;
                }
            }
            if s(w, 0x14) as i32 >= L1[d] && s(w, 0x15) == 0 {
                s_put(w, 0x15, 1);
                let n = s8(w, 0x17);
                // SPEC GAP: a record outside 0..59 is neither read nor written.
                if (0..60).contains(&n) {
                    let sp = unsafe { (*w).sectors };
                    for i in 0..5i64 {
                        let o = n as i64 * 0xB1 + 0x9F + i;
                        if get8(sp, SECTORS_LEN, o) == 0xAE {
                            put8(sp, SECTORS_LEN, o, 0xAC);
                        }
                        if get8(sp, SECTORS_LEN, o) == 0xAF {
                            put8(sp, SECTORS_LEN, o, 0xAD);
                        }
                    }
                }
            }
        } else {
            if s(w, 0x15) == 0 {
                v.say(0xAC);
                unsafe {
                    (*w).bonus_due = 1;
                    (*w).bonus = (*w).bonus.wrapping_add(15000);
                }
                if s8(w, 0xA) == 1 && s(w, 0x20) == 0 && s(w, 0x21) == 0 && v.search(0x51, true, true) {
                    v.cure_words(0xF, 0xD);
                }
            } else {
                v.say(0xAD);
                late = true;
            }
            s_put(w, 0xB, 2);
        }
    }
    // 7.
    if a8(w, STREAK) == 4 {
        v.say(0x94);
        v.said = true;
    }
    // 8. Losses.
    if !v.said {
        let wd = unsafe { &*w };
        let m = wd.dead_mercs;
        let a = wd.dead_allies > 5 || wd.dead_npc > 15;
        if m <= 0 {
            if wd.idle_day != 0 {
                q = score + 0x42;
            }
        } else if m <= 2 {
            if a {
                q = score + 0x34;
            }
        } else if wd.team_on_duty == 0 {
            q = score + 0x2D;
        } else {
            q = score + if a { 0x3B } else { 0x26 };
        }
    }
    if q != NONE {
        v.say(q);
        v.said = true;
    }
    // 9. The purifier part.
    if s8(w, 5) == 1 {
        if v.search(0x79, true, false) {
            s_put(w, 5, 2);
            q = by_score(0x4A, score);
            v.said = true;
            v.say(q);
            plant_put(w, 0, STATE, 2);
            calc_process_cap(w);
            if day(w) <= L2[d] {
                v.say(0x91);
                unsafe {
                    (*w).bonus = (*w).bonus.wrapping_add(1000);
                    (*w).bonus_due = 1;
                }
            }
        } else if day(w) == 1 {
            q = by_score(0x47, score);
            v.say(q);
            v.said = true;
        }
    }
    // 10. Blight.
    if s8(w, 0xA) == 1 {
        if (0..60).any(|n| owned(w, n) && sec(w, n, UNHEALTHY) != 0) {
            if v.search(0x5E, true, false) {
                v.say(0xAF);
                v.said = true;
                s_put(w, 0x20, 1);
            } else if s(w, 0x21) == 0 && v.search(0x51, false, true) {
                v.said = true;
                if v.ransom_open() {
                    if s(w, 0x15) != 0 {
                        v.say(0xB0);
                    }
                } else {
                    v.search(0x51, true, true);
                    v.cure_words(0x10, 0xE);
                }
            }
        } else {
            v.say(0xB1);
            v.said = true;
            s_put(w, 0xA, 2);
        }
    }
    // 11. Seeds.
    if s8(w, 0xE) == 1 {
        if v.search(0x6E, true, false) {
            if v.ransom_open() {
                v.say(0xB7);
            } else {
                v.brenda(0x13);
                if s8(w, 0xA) == 1 && s(w, 0x21) != 0 && s8(w, 0x22) > 1 {
                    s_put(w, 0x22, 1);
                }
            }
            v.said = true;
            s_put(w, 0xE, 2);
        } else if v.search(0x7C, true, false) {
            if v.ransom_open() {
                v.say(0xB8);
            } else {
                v.brenda(0x14);
            }
            v.said = true;
            s_put(w, 0xE, 2);
        }
    }
    // 12.
    for i in 1..4i64 {
        if plant8(w, i, STATE) as i8 == 3 {
            let n = plant_sector(w, i);
            if !owned(w, n) {
                plant_put(w, i, STATE, 2);
            } else if connected_home(w, n) != 0 {
                plant_put(w, i, STATE, 2);
                calc_process_cap(w);
            }
        }
    }
    // 13. Capacity.
    let cap = rd(w, PROCESS_CAP);
    let start_cap = unsafe { (*w).start_cap };
    if cap > start_cap && cap > 20 {
        q = match cap as u32 {
            45 => 0x50,
            50 => 0x53,
            55 => 0x4D,
            75 => 0x59,
            80 => 0x56,
            85 => 0x5C,
            110 => 0x5F,
            _ => q,
        };
        v.say(by_score(q, score));
        v.said = true;
    } else if cap < start_cap {
        let p = if rd(w, PLANTS_OWNED) < unsafe { (*w).start_plants } { 0x62 } else { 0x65 };
        v.say(by_score(p, score));
        v.said = true;
    }
    // 14.
    if !v.said {
        let r = ai.random(5);
        v.say(7 * r + score + 3);
    }
    // 15.
    if s8(w, 5) == 1 && day(w) > 1 {
        v.say(0x9D);
    }
    // 16.
    let list_owned = || LIST.iter().all(|&n| owned(w, n));
    if s8(w, 6) == 1 && list_owned() {
        s_put(w, 6, 2);
        v.say(0x92);
        s_put(w, 2, 2);
    }
    if s8(w, 7) == 1 && list_owned() {
        s_put(w, 7, 2);
        v.say(0x9B);
        s_put(w, 2, 2);
    }
    // 17.
    if connected_home(w, 5) == 0 {
        if s8(w, 8) >= 1 {
            a16_add(w, COUNT_92, L3[d]);
            s_put(w, 0x24, s(w, 0x24).wrapping_add(1));
        }
    } else if s(w, 0x13) == 0 {
        s_put(w, 0x13, 1);
        a16_add(w, COUNT_94, if s8(w, 8) == 1 { 100 } else { 50 });
        v.say(0x9E);
        s_put(w, 8, 2);
        s_put(w, 0x12, 1);
    }
    // 18.
    if s8(w, 0xC) == 1 {
        if v.search(0x77, true, false) {
            v.say(0xB6);
            a16_add(w, COUNT_94, 150);
            s_put(w, 0xC, 2);
        } else {
            let n = s(w, 0x18).wrapping_add(1);
            s_put(w, 0x18, n);
            a16_add(w, COUNT_92, L4[d]);
            if n == 3 {
                v.say(0xB5);
            }
        }
    }
    // 19.
    for i in 1..4i64 {
        if plant8(w, i, STATE) as i8 == 3 {
            v.say(0x9A);
            plant_put(w, i, STATE, 2);
        }
    }
    // 20.
    for i in 1..4i64 {
        if plant8(w, i, STATE) == 0 {
            let n = plant_sector(w, i);
            if owned(w, n) {
                v.say(if connected_home(w, n) != 0 { 0x93 } else { 0x99 });
            }
            plant_put(w, i, STATE, 1);
            let r = ai.random(3);
            plant_put(w, i, COUNTDOWN, (r as u8).wrapping_add(2));
        }
    }
    // 21.
    if s8(w, 9) < 2 && connected_home(w, 0x1B) != 0 {
        if !v.ransom_open() {
            v.brenda(0xB);
        }
        s_put(w, 9, 2);
        if day(w) <= L5[d] {
            s_put(w, 0xF, 2);
        }
    }
    // 22.
    if late {
        v.say(0xAE);
    }
    0
}

fn day_bonus(w: *mut Ja1World, h: *const Ja1Host) {
    let ai = Ai { w, h, s: 0 };
    if s(w, 0xF) != 0 {
        let n = s(w, 0xF).wrapping_sub(1);
        s_put(w, 0xF, n);
        if n == 0 {
            ai.call(op::GUIDE_SAY, &[0x9C, 3]);
            unsafe {
                (*w).bonus_due = 1;
                (*w).bonus = (*w).bonus.wrapping_add(5000);
            }
        }
    }
    let wd = unsafe { &mut *w };
    if wd.bonus_due != 0 {
        if wd.bonus != 0 {
            let income = rd(w, DAY_INCOME).wrapping_add(wd.bonus as i16 as i32);
            a32_put(w, DAY_INCOME, income);
            let exp = rd(w, DAY_EXPENSES);
            a32_put(w, BALANCE, wd.prev_balance.wrapping_add(income).wrapping_sub(exp));
            a32_put(w, DAY_NET, income.wrapping_sub(exp));
            wd.bonus = 0;
        }
        wd.bonus_due = 0;
    }
}

// SPEC GAP: a null world or null host does nothing (result 0, no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_day_verdict(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| day_verdict(w, h))).unwrap_or(0)
}

#[no_mangle]
pub extern "C" fn ja1_day_bonus(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| day_bonus(w, h)));
}

/// strategic.md "The day-end screen", the day-end steps 1 and 2 (after the payroll screen's OK).
fn day_end(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    let ai = Ai { w, h, s: 0 };
    let mut r = 0;
    if rd(w, BALANCE) < 0 {
        ai.call(op::GUIDE_SAY, &[if s8(w, 5) < 2 { 0xB4 } else { 0xB3 }, 3]);
        r = 3;
    }
    if (unsafe { (*w).first_hit } as i8) < 2 {
        a8_put(w, LOST_DAYS, a8(w, LOST_DAYS).wrapping_add(1));
    }
    r
}

// SPEC GAP: a null world or null host does nothing (result 0).
#[no_mangle]
pub extern "C" fn ja1_day_end(w: *mut Ja1World, h: *const Ja1Host) -> i32 {
    if w.is_null() || h.is_null() {
        return 0;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| day_end(w, h))).unwrap_or(0)
}
