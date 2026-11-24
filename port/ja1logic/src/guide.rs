//! The guide's advice (docs/spec/strategic.md "The guide's advice"; C interface in docs/spec/port_api.md):
//! `ja1_guide_advice`, the last step of the strategic screen opening. A child module of `ai`; host ops 302
//! guide_say, 307 quest_record, 317 brenda_say, 318 panel_draw, 319 guide_offer, 320 merc_join and random go
//! through `Ai`. All accounts is that of `ledger`, Connected to home that of `home`.
use super::damage::{get16, get8, put16, put8};
use super::home::connected_home;
use super::ledger::{add, calc_ledger_all, rd};
use super::{op, Ai, Ja1Host, Ja1World, REC};

const SOLDIERS_LEN: usize = 8 * REC;
const SECTOR_STATE_LEN: usize = 80 * 9;
const SECTORS_LEN: usize = 60 * 0xB1;
const MISSION_DONE_LEN: usize = 80;
const STORY_LEN: usize = 43;
/// The quote of an unlisted process capacity: EBP at PlayLoop 0x4FBAD in DOSBox-X (image 0x90E60 + 0x1F1000).
pub const ENDSAB_ADDR: i32 = 0x281E60;
const QUEUE_LEN: usize = 7;
const PLANTS_LEN: usize = 4 * 12;
// accounts (Day accounts)
const TEAM_COUNT: i64 = 0x08;
const TREE_PRICE: i64 = 0x10;
const TREES_OWNED: i64 = 0x18;
const TREES_TAPPED: i64 = 0x1C;
const SECTORS_OWNED: i64 = 0x74;
const PROCESS_CAP: i64 = 0x84;
// sector_state bytes
const FLAGS: i64 = 0;
const ENEMIES: i64 = 1;
const TAPPERS: i64 = 3;
const GUARDS: i64 = 4;
const UNHEALTHY: i64 = 8;
// soldier record
const PROFILE_ID: i64 = 0x179;
const IN_CUR_SECTOR: i64 = 0x182;
const HP: i64 = 0x183;
const NONE: i32 = 999;

fn s(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).story }, STORY_LEN, i)
}
fn s8(w: *mut Ja1World, i: i64) -> i32 {
    s(w, i) as i8 as i32
}
fn s_put(w: *mut Ja1World, i: i64, v: u8) {
    put8(unsafe { (*w).story }, STORY_LEN, i, v)
}
fn s_add(w: *mut Ja1World, i: i64, d: i8) -> u8 {
    let v = s(w, i).wrapping_add(d as u8);
    s_put(w, i, v);
    v
}
fn q(w: *mut Ja1World, i: i64) -> u8 {
    get8(unsafe { (*w).guide_queue }, QUEUE_LEN, i)
}
/// Byte i of the sector_state entry of sector n (SPEC GAP: outside 0..79 reads 0).
fn sec(w: *mut Ja1World, n: i32, i: i64) -> u8 {
    if !(0..80).contains(&n) {
        return 0;
    }
    get8(unsafe { (*w).sector_state }, SECTOR_STATE_LEN, n as i64 * 9 + i)
}
fn owned(w: *mut Ja1World, n: i32) -> bool {
    sec(w, n, FLAGS) & 1 != 0
}
/// Connected(s): Connected to home of the low byte of s.
fn connected(w: *mut Ja1World, n: i32) -> i32 {
    connected_home(w, n as u8 as i32)
}
/// R(n, o) writes (SPEC GAP: a sector record outside 0..59 is not written).
fn rec_put(w: *mut Ja1World, n: i32, o: i64, v: u8) {
    if (0..60).contains(&n) {
        put8(unsafe { (*w).sectors }, SECTORS_LEN, n as i64 * 0xB1 + o, v)
    }
}
fn rec_put16(w: *mut Ja1World, n: i32, o: i64, v: u16) {
    if (0..60).contains(&n) {
        put16(unsafe { (*w).sectors }, SECTORS_LEN, n as i64 * 0xB1 + o, v)
    }
}
fn plant8(w: *mut Ja1World, i: i64, o: i64) -> u8 {
    get8(unsafe { (*w).plants }, PLANTS_LEN, 12 * i + o)
}
fn plant_put(w: *mut Ja1World, i: i64, o: i64, v: u8) {
    put8(unsafe { (*w).plants }, PLANTS_LEN, 12 * i + o, v)
}
fn plant_sector(w: *mut Ja1World, i: i64) -> i32 {
    let p = unsafe { (*w).plants };
    (get16(p, PLANTS_LEN, 12 * i + 4) as u32 | (get16(p, PLANTS_LEN, 12 * i + 6) as u32) << 16) as i32
}
const STATE: i64 = 8;
const COUNTDOWN: i64 = 0xB;
fn mission_clear(w: *mut Ja1World, n: i32) {
    // SPEC GAP: a plant sector outside 0..79 writes nothing.
    if (0..80).contains(&n) {
        put8(unsafe { (*w).mission_done }, MISSION_DONE_LEN, n as i64, 0)
    }
}
fn sol(w: *mut Ja1World, k: i32, off: i64) -> u8 {
    get8(unsafe { (*w).soldiers }, SOLDIERS_LEN, k as i64 * REC as i64 + off)
}
fn d_index(w: *mut Ja1World) -> usize {
    // SPEC GAP: a difficulty other than 0..2 uses the values of 2.
    (unsafe { (*w).difficulty } as usize).min(2)
}

struct G<'a> {
    w: *mut Ja1World,
    ai: &'a Ai,
    said: i32,
}

impl G<'_> {
    fn say(&mut self, quote: i32) {
        self.ai.call(op::GUIDE_SAY, &[quote, 6]);
        self.said = quote;
    }
    fn brenda(&mut self, quote: i32) {
        self.ai.call(op::BRENDA_SAY, &[quote]);
        self.said = quote;
    }
    fn redraw(&self) {
        calc_ledger_all(self.w);
        self.ai.call(op::PANEL_DRAW, &[0, 14]);
    }
}

/// Steps 1 and 2: the plant whose countdown is lowest among those that fit; Some(i) = a pick.
fn plant_pick(w: *mut Ja1World, fits: impl Fn(i64) -> bool) -> Option<i64> {
    let mut best: i32 = 99;
    let mut pick = None;
    for i in 1..=3i64 {
        let c = plant8(w, i, COUNTDOWN);
        if plant8(w, i, STATE) as i8 == 1 && c != 0 && (c as i8 as i32) < best && fits(i) {
            pick = Some(i);
            best = c as i8 as i32;
        }
    }
    pick
}
/// Countdown − 1; yes when it is then 0 (state 2, mission_done cleared).
fn plant_tick(w: *mut Ja1World, i: i64) -> bool {
    let c = plant8(w, i, COUNTDOWN).wrapping_sub(1);
    plant_put(w, i, COUNTDOWN, c);
    if c != 0 {
        return false;
    }
    plant_put(w, i, STATE, 2);
    mission_clear(w, plant_sector(w, i));
    true
}

/// Tip table (k = 0..4): e0 day, e1, e2, e3, e4..e8 sectors, e9, e10, e11, quote.
const TIPS: [([i8; 12], i32); 5] = [
    ([1, 100, 0, 100, 0, 0, 0, 0, 0, 0, 0, 1], 0),
    ([2, 0, 1, 100, 28, 29, 38, 48, 49, 0, 0, 1], 2),
    ([3, 1, 100, 100, 28, 29, 38, 48, 49, 0, 1, 1], 6),
    ([12, 100, 7, 3, 5, 59, 59, 59, 59, 5, 1, 24], 0x10),
    ([1, 100, 4, 100, 0, 0, 0, 0, 0, 27, 1, 7], 500),
];

fn guide_advice(w: *mut Ja1World, h: *const Ja1Host) {
    let ai = Ai { w, h, s: 0 };
    let mut g = G { w, ai: &ai, said: NONE };
    let d = d_index(w);

    // 1. Plant nobody owns.
    if let Some(i) = plant_pick(w, |i| !owned(w, plant_sector(w, i))) {
        plant_tick(w, i);
    }
    // 2. Owned plant.
    let s5 = s8(w, 5);
    let pick2 = plant_pick(w, |i| {
        let c = plant8(w, i, COUNTDOWN) as i8 as i32;
        connected(w, plant_sector(w, i)) == 1 && (s5 == 2 || (s5 < 2 && c > 1))
    });
    if let Some(i) = pick2 {
        if plant_tick(w, i) {
            calc_ledger_all(w);
            ai.call(op::PANEL_DRAW, &[0, 14]);
            // Other capacities: the original passes the register EBP, the run-time address of `DAT\ENDSAB.DAT`
            // (strategic.md "The guide's advice" step 2); the port uses the DOSBox-X value. The record is past
            // the end of NATQUOTE.EDT: no voice, no text, the face waits for a key or a click.
            let quote = match rd(w, PROCESS_CAP) as u32 {
                45 => 0x31,
                50 => 0x32,
                55 => 0x30,
                75 => 0x34,
                80 => 0x33,
                85 => 0x35,
                110 => 0x36,
                _ => ENDSAB_ADDR,
            };
            ai.call(op::GUIDE_SAY, &[quote, 6]);
        }
    }
    // 3. The tapping limit.
    g.said = NONE;
    let mut gap: i32 = 0;
    if rd(w, PROCESS_CAP) < 110 && pick2.is_none() {
        gap = rd(w, PROCESS_CAP).wrapping_sub(rd(w, TREES_TAPPED));
        if gap < 6 && s8(w, 5) == 2 {
            g.say(if gap < 0 { 0x2E } else { 0x2D - gap });
        }
    }
    // 4. Tree price story.
    let stage = s(w, 0x27);
    match stage {
        0 => {
            if rd(w, TREES_TAPPED) > 16 || rd(w, TREES_OWNED) > 32 {
                g.say(8);
                s_add(w, 0x27, 1);
            }
        }
        1 => {
            if rd(w, TREES_TAPPED) > 20 || rd(w, TREES_OWNED) > 40 {
                g.say(10);
                let r = ai.random(3);
                s_put(w, 0x28, (r + 1) as u8);
                s_add(w, 0x27, 1);
            }
        }
        2 => {
            if s_add(w, 0x28, -1) == 0 {
                g.say(12);
                add(w, TREE_PRICE, -100);
                g.redraw();
                s_add(w, 0x27, 1);
            }
        }
        3 => {
            let t = (rd(w, TREES_TAPPED) / 10).wrapping_mul(20).wrapping_add(300);
            let l = [4i32, 3, 2][d];
            let price = rd(w, TREE_PRICE);
            let mut quote = None;
            if t < price {
                let c = s_add(w, 0x28, -1);
                if c as i8 as i32 == -l {
                    s_put(w, 0x28, 0);
                    add(w, TREE_PRICE, -20);
                    if s(w, 0x29) == 0 {
                        s_put(w, 0x29, 1);
                        quote = Some(0x0E);
                    } else {
                        quote = Some(0x47);
                    }
                }
            } else if t > price {
                let c = s_add(w, 0x28, 1);
                if c as i8 as i32 == l {
                    add(w, TREE_PRICE, 20);
                    s_put(w, 0x28, 0);
                    if rd(w, TREE_PRICE) == 500 {
                        s_add(w, 0x27, 1);
                        quote = Some(0x49);
                    } else if s(w, 0x2A) == 0 {
                        s_put(w, 0x2A, 1);
                        quote = Some(0x16);
                    } else {
                        quote = Some(0x48);
                    }
                }
            }
            if let Some(qv) = quote {
                g.say(qv);
                g.redraw();
            }
        }
        _ => {}
    }
    // 5. Brenda.
    if s8(w, 6) == 1 && s(w, 0x25) == 0 {
        g.brenda(10);
        s_put(w, 0x25, 1);
    }
    // 6.
    if s(w, 2) != 0 && s_add(w, 2, -1) == 0 {
        g.say(if s8(w, 7) == 2 { 0x37 } else { 4 });
        unsafe { (*w).merc_tire = 0 };
    }
    // 7. Plant 2.
    if s(w, 0x0D) == 0 {
        let ps = plant_sector(w, 2);
        if owned(w, ps)
            && connected(w, ps) == 1
            && plant8(w, 2, STATE) as i8 == 2
            && gap.wrapping_mul(2) < 25
            && (sec(w, 43, ENEMIES) > 6 || sec(w, 52, ENEMIES) > 6 || sec(w, 54, ENEMIES) > 6)
            && sec(w, ps, UNHEALTHY) == 0
        {
            g.say(0x4A);
            s_put(w, 0x0D, 1);
        }
    } else if s8(w, 0x0D) == 1 {
        s_put(w, 0x0D, 2);
    }
    // 8. The queued messages.
    if q(w, 1) != 0xFF {
        let (a, b) = (q(w, 1) as i32, q(w, 2) as i32);
        ai.call(op::GUIDE_SAY, &[a, b]);
        g.said = a;
    }
    if q(w, 3) != 0xFF {
        let (a, b) = (q(w, 3) as i32, q(w, 4) as i32);
        ai.call(op::GUIDE_SAY, &[a, b]);
        g.said = a;
    }
    if q(w, 5) != 0xFF {
        g.brenda(q(w, 5) as i32);
    }
    if q(w, 6) != 0xFF {
        g.brenda(q(w, 6) as i32);
    }
    // 9a.
    if s8(w, 9) == 2 && s8(w, 6) == 2 && s8(w, 0x0B) < 1 && !owned(w, 27) && q(w, 5) == 0xFF && q(w, 6) == 0xFF {
        let mut qr = [0u8; 250];
        ai.call_out(op::QUEST_RECORD, &[6], &mut qr);
        let w16 = |o: usize| i16::from_le_bytes([qr[o], qr[o + 1]]) as i32;
        for i in 0..25 {
            let (a, b) = (w16(10 * i), w16(10 * i + 6));
            if owned(w, a) || owned(w, b) {
                continue;
            }
            s_put(w, 0x0B, 1);
            rec_put(w, a, 0x9F, 0xAE);
            rec_put(w, a, 0xA4, 0x5C);
            rec_put(w, a, 0xA0, 0xAF);
            rec_put16(w, a, 0x82, w16(10 * i + 2) as u16);
            rec_put16(w, a, 0x84, w16(10 * i + 2).wrapping_add(1) as u16);
            rec_put16(w, a, 0x8C, w16(10 * i + 4) as u16);
            s_put(w, 0x17, qr[10 * i]);
            rec_put(w, b, 0xA4, 0x7A);
            rec_put16(w, b, 0x8C, w16(10 * i + 8) as u16);
            rec_put16(w, 27, 0x8C, 0x55C);
            rec_put(w, 27, 0xA4, 0x86);
            g.say(0x14);
            break;
        }
    }
    // 9b.
    let s14 = s(w, 0x14);
    if s8(w, 0x0B) == 1 && s14 > 0 && s14 < 11 {
        g.say(s14 as i32 + 0x38);
    }
    // 9c.
    if s8(w, 0x0C) < 1 && (sec(w, 5, GUARDS) != 0 || sec(w, 5, TAPPERS) != 0) {
        let mut qr = [0u8; 250];
        ai.call_out(op::QUEST_RECORD, &[7], &mut qr);
        let w16 = |o: usize| i16::from_le_bytes([qr[o], qr[o + 1]]) as i32;
        let mut found = false;
        for i in 0..4 {
            let a = w16(10 * i);
            if owned(w, a) {
                continue;
            }
            rec_put(w, a, 0xA4, 0x77);
            rec_put16(w, a, 0x8C, w16(10 * i + 2) as u16);
            s_put(w, 0x0C, 1);
            g.say(0x12);
            found = true;
            break;
        }
        if !found {
            s_put(w, 0x0C, 2);
        }
    }
    // 10.
    if s(w, 0x0E) == 0 {
        s_put(w, 0x0E, 1);
    }
    if s8(w, 0x0E) >= 2 && s(w, 0x1E) == 0 && s(w, 0x1F) == 0 {
        if s8(w, 0x0B) == 1 || s(w, 0x15) != 0 {
            g.say(0x1A);
        } else {
            g.brenda(6);
        }
        s_put(w, 0x1F, 1);
    }
    // 11. Five tips.
    for (k, (e, quote)) in TIPS.iter().enumerate() {
        let k = k as i64;
        let mut n = 0;
        if e[0] as i32 > unsafe { (*w).day } {
            n += 1;
        }
        if e[1] != 100 && s8(w, 5 + e[1] as i64) < 2 {
            n += 1;
        }
        for x in [e[2], e[3]] {
            if x != 100 && s(w, 5 + x as i64) != 0 {
                n += 1;
            }
        }
        if (k == 1 || k == 4) && (s8(w, 0x0B) == 1 || s(w, 0x15) != 0) {
            n += 1;
        }
        if e[4..9].iter().all(|&c| owned(w, c as i32)) {
            n += 1;
        }
        n += connected(w, e[9] as i32);
        if g.said != NONE && e[10] != 0 {
            n += 1;
        }
        if e[11] as i32 > rd(w, SECTORS_OWNED) {
            n += 1;
        }
        if s8(w, 5 + k) == 1 {
            n += 1;
        }
        if k == 2 && n == 0 && ai.random(100) >= [20, 50, 80][d] {
            n += 1;
        }
        if n != 0 {
            continue;
        }
        let qv = *quote;
        if qv < 500 {
            ai.call(op::GUIDE_SAY, &[qv, 6]);
        } else {
            ai.call(op::BRENDA_SAY, &[qv - 500]);
        }
        g.said = qv;
        s_put(w, 5 + k, 1);
        if k == 0 {
            if unsafe { (*w).difficulty } as i8 <= 1 {
                unsafe { (*w).purifier = 1 };
            } else {
                let c = match ai.random(3) {
                    0 => Some(0x30),
                    1 => Some(0x31),
                    2 => Some(0x3A),
                    _ => None, // SPEC GAP: a random answer outside 0..2 writes nothing
                };
                if let Some(c) = c {
                    rec_put(w, c, 0x9E, 0x79);
                }
            }
        } else if k == 1 || k == 2 {
            s_put(w, 2, 0);
            unsafe { (*w).merc_tire = [30i8, 50, 70][d] };
        }
    }
    // 12.
    if s8(w, 9) == 1 && s_add(w, 0x26, 1) == 3 {
        g.say(0x4B);
    }
    if s8(w, 8) == 1 && connected(w, 5) == 0 && s(w, 0x24) == 3 {
        g.say(0x4C);
    }
    if s8(w, 8) == 2 && connected(w, 5) == 0 && s(w, 0x12) != 0 && g.said == NONE {
        g.say(0x38);
        s_put(w, 0x12, 0);
    }
    if q(w, 0) != 0 && (g.said == NONE || s(w, 0x23) == 0) {
        ai.call(op::GUIDE_SAY, &[0x4E, 6]);
        s_put(w, 0x23, 1);
    }
    // 13. Hire offer.
    let guide = (0..8).any(|k| {
        sol(w, k, IN_CUR_SECTOR) != 0 && sol(w, k, HP) != 0 && (60..=64).contains(&sol(w, k, PROFILE_ID))
    });
    if !guide {
        let gw = unsafe { (*w).guide_wait }.wrapping_add(1);
        unsafe { (*w).guide_wait = gw };
        let gh = unsafe { (*w).guides_hired } as i8 as i32;
        let m = (0..8)
            .filter(|&k| sol(w, k, PROFILE_ID) < 60 && sol(w, k, IN_CUR_SECTOR) != 0 && sol(w, k, HP) as i8 > 0)
            .count();
        if gw as i8 > 1 && gh < 4 && (1..=7).contains(&m) && ai.random(100) < 70 - 20 * gh + 5 * (gw as i8 as i32) {
            ai.call(op::GUIDE_SAY, &[(gh + 0x43) as i8 as i32, 6]);
            let p = (unsafe { (*w).guides_hired }.wrapping_add(60)) as i32;
            if ai.call(op::GUIDE_OFFER, &[p]) != 0 {
                ai.call(op::MERC_JOIN, &[p]);
                unsafe { (*w).guides_hired = (*w).guides_hired.wrapping_add(1) };
                add(w, TEAM_COUNT, 1);
                unsafe { (*w).team_on_duty = (*w).team_on_duty.wrapping_add(1) };
                ai.call(op::PANEL_DRAW, &[9, 10]);
            }
            unsafe { (*w).guide_wait = 0 };
        }
    }
    // 14. Weather.
    if ai.random(100) < 10 {
        ai.call(op::GUIDE_SAY, &[0x4F, 6]);
        unsafe { (*w).hard_day = 1 };
    } else {
        unsafe { (*w).hard_day = 0 };
    }
}

/// Step 7 of the strategic screen opening (`stratopen`).
pub(super) fn guide_advice_step(w: *mut Ja1World, h: *const Ja1Host) {
    guide_advice(w, h);
}

// SPEC GAP: a null world or null host does nothing (no host call, nothing changed).
#[no_mangle]
pub extern "C" fn ja1_guide_advice(w: *mut Ja1World, h: *const Ja1Host) {
    if w.is_null() || h.is_null() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| guide_advice(w, h)));
}
