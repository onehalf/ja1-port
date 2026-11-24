//! The assignment screen (docs/spec/strategic.md "The assignment screen"): one column per merc in the sector with
//! the face, the duty, the stats and the health bar, and the pay numbers below.
use crate::invdraw::{load_pcx, InvData};
use crate::panel::{Painter, PW};

/// Soldier record size and the fields read here.
pub const SOLDIER: usize = 0x200;
const NICKNAME: usize = 0x16E;
const PROFILE: usize = 0x179;
pub(crate) const SLOT: usize = 0x17A;
pub const DUTY: usize = 0x17E;
const IN_SECTOR: usize = 0x182;
const HP: usize = 0x183;
const HP_MAX: usize = 0x184;
const WOUNDS: usize = 0x185;
const AGILITY: usize = 0x18B;
const DEXTERITY: usize = 0x18C;
const WISDOM: usize = 0x18D;
const MEDICAL: usize = 0x18E;
const MECHANICAL: usize = 0x190;
const LEVEL: usize = 0x192;
const TRAIN_WHAT: usize = 0x199;

pub const DUTY_NAMES: [&[u8]; 8] = [b"ON DUTY", b"REST", b"TRAIN", b"DOCTOR", b"PATIENT", b"REPAIR", b"FIRE", b"ESCORT"];
const TRAIN_NAMES: [&[u8]; 5] = [b"PHYS.", b"MED.", b"EXPL.", b"MECH.", b"MARKS."];

fn sb(s: &[u8], off: usize) -> i32 {
    s[off] as i8 as i32
}

/// The mercs of the picture: slots 0.. while in the sector (at most 8).
pub fn merc_count(soldiers: &[u8]) -> usize {
    (0..8).take_while(|&k| soldiers[k * SOLDIER + IN_SECTOR] != 0).count()
}

/// Med points (strategic.md "Morning duties"): place i = 12 bytes at i * 12.
fn med_points(s: &[u8]) -> i32 {
    let mut sum = 0i32;
    for i in 0..12 {
        let b = i * 12;
        let m = match s[b] {
            0x49 => 1,
            0x4A => 2,
            _ => 0,
        };
        if m == 0 {
            continue;
        }
        for j in 0..sb(s, b + 1).max(0) as usize {
            let cond = if j == 0 { sb(s, b + 2) } else { sb(s, b + 7 + j) };
            sum = sum.wrapping_add(cond.wrapping_mul(m));
        }
    }
    sum
}

/// (Doctor budget, its cap).
fn doctor(s: &[u8]) -> (i32, i32) {
    let t = (100 - sb(s, DEXTERITY)) * 20 / 100;
    let cap = (sb(s, WISDOM) - t) * sb(s, MEDICAL) / 100 * 2;
    (med_points(s).min(cap), cap)
}

/// (Points, Repair value).
fn repair(s: &[u8]) -> (i32, i32) {
    let v = (sb(s, MECHANICAL) * sb(s, DEXTERITY) + (sb(s, LEVEL) - 1) * 5) * 3 / 200;
    let t = if s[0] == 0x4B { sb(s, 2) } else { 0 };
    (t * v / 100, v)
}

pub(crate) fn number(p: &mut Painter, v: i32, d: usize, x: i32, y: i32, fg: u8, shadow: u8) {
    let mut t = v.to_string().into_bytes();
    while t.len() < d {
        t.insert(0, b'^');
    }
    p.text(&t, x, y, false, fg, shadow, 0);
}

fn stat_value(s: &[u8], i: usize) -> i32 {
    match i {
        0 => u16::from_le_bytes([s[0x112], s[0x113]]) as i32,
        1 => sb(s, HP),
        _ => sb(s, AGILITY + i - 2),
    }
}

fn status_field(p: &mut Painter, s: &[u8], x: i32, y: i32) {
    let (fg, sh) = (0x27, 9);
    let bc = match s[DUTY] {
        2 => {
            if let Some(t) = TRAIN_NAMES.get(s[TRAIN_WHAT] as usize) {
                p.text(t, x + 17, y, true, fg, sh, 0);
            }
            None
        }
        3 => Some(doctor(s)),
        4 => {
            number(p, sb(s, HP) - sb(s, HP_MAX), 3, x + 8, y, fg, sh);
            None
        }
        5 => Some(repair(s)),
        _ => None,
    };
    if let Some((b, c)) = bc {
        number(p, b, 3, x, y, fg, sh);
        p.text(b"/", x + 15, y, true, fg, sh, 0);
        number(p, c, if c < 10 { 1 } else if c < 100 { 2 } else { 3 }, x + 18, y, fg, sh);
    }
}

/// The assignment health bar; `gap_rows` = false: the gap rows are not drawn (the office ledger). Returns the gap.
pub(crate) fn health_bar(p: &mut Painter, s: &[u8], x: i32, mut y: i32, gap_rows: bool) -> i32 {
    let (hp, hp_max, wounds) = (sb(s, HP), sb(s, HP_MAX), sb(s, WOUNDS));
    let nh = hp / 4;
    let nm = hp_max / 4;
    let nw = if wounds == 0 {
        0
    } else if wounds / 4 == 0 {
        1
    } else {
        wounds / 4
    };
    let ne = if hp_max - hp <= wounds { 0 } else { (nm - nh - nw).max(0) };
    let gap = 25 - nw - ne - nh;
    if gap > 0 && gap_rows {
        p.element(88, x, y, Some(gap));
    }
    y += gap;
    if nw > 0 {
        p.bar_rows(87, x, y, nw);
    }
    if ne > 0 {
        p.bar_rows(86, x, y + nw, ne);
    }
    y += nw + ne;
    if nh > 0 {
        p.element(81, x, y, None);
        p.bar_rows(80, x, y + 1, nh - 1);
    }
    gap
}

/// Step 2a: the face of merc k (face buffer k) at (37 k + 13, 20).
pub fn face(p: &mut Painter, faces: &[u8], k: usize) {
    let xk = 37 * k as i32;
    for r in 0..25 {
        for c in 0..28 {
            p.put(xk + 13 + c, 20 + r, faces[k * 1024 + (r as usize + 2) * 32 + c as usize + 2]);
        }
    }
}

/// Steps 2d..2g of merc k (soldier record `s`): the stats, the duty name, the nickname and the health bar.
pub fn column(p: &mut Painter, s: &[u8], k: usize, stat_changed: &[u8]) {
    let xk = 37 * k as i32;
    let sx = 27 + 37 * s[SLOT] as i32;
    for i in 0..10 {
        let sy = 59 + 8 * i as i32;
        let v = stat_value(s, i);
        let changed = stat_changed.get(s[PROFILE] as usize * 10 + i).copied().unwrap_or(0) != 0;
        let fg = if changed { 0xFF } else if i % 2 == 0 { 237 } else { 41 };
        number(p, v, 3, sx + 1, sy + 1, 7, 0);
        number(p, v, 3, sx, sy, fg, 0);
    }
    if let Some(n) = DUTY_NAMES.get(s[DUTY] as usize) {
        p.text(n, xk + 29, 9, true, 0xB5, 8, 0);
    }
    let nick = &s[NICKNAME..NICKNAME + 9];
    let nick = &nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)];
    p.text(nick, xk + 28, 48, true, 0x0E, 5, 0);
    health_bar(p, s, xk + 44, 20, true);
}

/// The assignment picture into the painter's buffer (the panel buffer). `soldiers` = team slots 0..8 (9 records:
/// slot 8 ends the list), `faces` = the 8 face buffers (32 x 32), `stat_changed` = 64 x 10 bytes, `acc` = the
/// accounts. Changes `duty` of a doctor / mechanic who cannot do the work (step 2b).
pub fn draw(p: &mut Painter, main: &InvData, table30: &[u8; 256], soldiers: &mut [u8], faces: &[u8],
            stat_changed: &[u8], acc: &[u8]) {
    load_pcx(p, main, "ASSMAP.PCC", 0, 0);
    for k in 0..merc_count(soldiers) {
        let s = &mut soldiers[k * SOLDIER..(k + 1) * SOLDIER];
        let xk = 37 * k as i32;
        face(p, faces, k);
        let doc = matches!(s[0], 0x49 | 0x4A) && s[MEDICAL] != 0;
        let rep = s[0] == 0x4B && s[MECHANICAL] != 0;
        if (s[DUTY] == 3 && !doc) || (s[DUTY] == 5 && !rep) {
            s[DUTY] = 0;
        }
        let s = &*s;
        if s[DUTY] != 0 {
            for r in 0..25 {
                let o = ((20 + r) * PW + xk + 13) as usize;
                for v in &mut p.buf[o..o + 28] {
                    *v = table30[*v as usize];
                }
            }
            status_field(p, s, xk + 11, 38);
        }
        column(p, s, k, stat_changed);
    }
    load_pcx(p, main, "ASSPAN.PCC", 0, 146);
    for k in 27..31 {
        crate::mappanel::assign_field(p, acc, k);
    }
}

/// The exit button 9 (strategic.md "Opening and leaving the assignment screen").
pub const EXIT: (i32, i32) = (269, 166);

/// The mouse at (x, y) is in the exit button.
pub fn exit_at(x: i32, y: i32) -> bool {
    (269..=294).contains(&x) && (166..=181).contains(&y)
}

// ---------------------------------------------------------------------------------------------------------------
// The duty menu (strategic.md "The duty menu")

const SUBJECTS: [&[u8]; 5] = [b"PHYSICAL", b"MEDICAL", b"EXPLOSIVES", b"MECHANICAL", b"MRKSMNSHIP"];

/// The duty choice flags F[0..6] of a merc.
pub fn duty_flags(s: &[u8]) -> [bool; 7] {
    let doc = matches!(s[0], 0x49 | 0x4A) && s[MEDICAL] != 0;
    let rep = s[0] == 0x4B && s[MECHANICAL] != 0;
    [true, true, true, doc, sb(s, HP) < sb(s, HP_MAX), rep, true]
}

/// The team slot whose duty name box holds the mouse at (x, y).
pub fn duty_box_at(x: i32, y: i32) -> Option<usize> {
    (0..8).find(|&k| {
        let bx = 11 + 37 * k as i32;
        (bx..=bx + 36).contains(&x) && (7..=15).contains(&y)
    })
}

/// Slot k is in the sector (the duty name box press opens the menu only then).
pub fn in_sector(soldiers: &[u8], k: usize) -> bool {
    soldiers[k * SOLDIER + IN_SECTOR] != 0
}

/// The merc pay panel of S into the panel buffer (Open step 1).
pub fn merc_pay_panel(p: &mut Painter, main: &InvData, s: &[u8], acc: &[u8]) {
    pay_panel(p, main, acc);
    for i in 0..10 {
        let (px, py) = if i <= 4 { (50, 156 + 7 * i as i32) } else { (112, 156 + 7 * (i as i32 - 5)) };
        let v = stat_value(s, i);
        number(p, v, 4, px + 1, py + 1, 7, 0);
        number(p, v, 4, px, py, if i % 2 == 0 { 237 } else { 41 }, 0);
    }
}

/// ASSPAN.PCC and the number fields 27..30.
pub fn pay_panel(p: &mut Painter, main: &InvData, acc: &[u8]) {
    load_pcx(p, main, "ASSPAN.PCC", 0, 146);
    for k in 27..31 {
        crate::mappanel::assign_field(p, acc, k);
    }
}

struct SubMenu {
    x: i32,
    y: i32,
    hover: Option<usize>,
    released: bool,
}

/// An open duty menu of team slot k.
pub struct DutyMenu {
    pub k: usize,
    x0: i32,
    flags: [bool; 7],
    hover: Option<usize>,
    released: bool,
    sub: Option<SubMenu>,
}

/// The end of a duty menu: None = nothing chosen, else (duty, training subject for TRAIN).
pub type DutyChoice = Option<(u8, Option<u8>)>;

impl DutyMenu {
    /// Open steps 1..5 for slot k: `panel` paints the panel buffer, `screen` the screen. Returns the menu and the
    /// mouse y of step 6. The caller plays the click sound.
    pub fn open(panel: &mut Painter, screen: &mut Painter, main: &InvData, soldiers: &[u8], acc: &[u8], k: usize)
                -> (DutyMenu, i32) {
        let s = &soldiers[k * SOLDIER..(k + 1) * SOLDIER];
        merc_pay_panel(panel, main, s, acc);
        screen.buf[..64000].copy_from_slice(&panel.buf[..64000]);
        let bx = 11 + 37 * k as i32;
        screen.element(0x41, bx, 7, None);
        if let Some(n) = DUTY_NAMES.get(s[DUTY] as usize) {
            screen.text(n, bx + 17, 9, true, 0xB5, 8, 0);
        }
        let x0 = bx - 3;
        screen.element(0x3B, x0, 12, None);
        let flags = duty_flags(s);
        for i in 0..7 {
            if flags[i] {
                screen.text(DUTY_NAMES[i], x0 + 7, 19 + 7 * i as i32, false, 8, 0x0E, 0);
            }
        }
        (DutyMenu { k, x0, flags, hover: None, released: false, sub: None }, 24 + 7 * s[DUTY] as i32)
    }

    fn label(&self, screen: &mut Painter, i: usize, lit: bool) {
        let (fg, sh) = if lit { (0xFF, 8) } else { (8, 0x0E) };
        screen.text(DUTY_NAMES[i], self.x0 + 7, 19 + 7 * i as i32, false, fg, sh, 0);
    }

    fn unlight(&mut self, screen: &mut Painter) {
        if let Some(h) = self.hover.take() {
            self.label(screen, h, false);
        }
    }

    /// One input poll: the mouse at (x, y), `buttons` bit 0 = left, bit 1 = right. Returns Some(choice) when the
    /// menu ends (the caller then sets the duty and draws the whole picture); `click` is set when the click
    /// sound plays.
    pub fn poll(&mut self, screen: &mut Painter, x: i32, y: i32, buttons: i32, click: &mut bool) -> Option<DutyChoice> {
        if let Some(sm) = &mut self.sub {
            if x < sm.x || x > sm.x + 56 || y <= sm.y || y >= sm.y + 60 {
                if let Some(h) = sm.hover.take() {
                    screen.text(SUBJECTS[h], sm.x + 7, sm.y + 9 + 7 * h as i32, false, 8, 0x0E, 0);
                }
            } else {
                for i in 0..5 {
                    let r = sm.y + 7 * i as i32;
                    if r + 9 < y && y < r + 17 && sm.hover != Some(i) {
                        if let Some(h) = sm.hover {
                            screen.text(SUBJECTS[h], sm.x + 7, sm.y + 9 + 7 * h as i32, false, 8, 0x0E, 0);
                        }
                        sm.hover = Some(i);
                        screen.text(SUBJECTS[i], sm.x + 7, r + 9, false, 0xFF, 8, 0);
                    }
                }
            }
            if buttons & 1 != 0 {
                if sm.released {
                    *click = true;
                    return Some(sm.hover.map(|h| (2, Some(h as u8))));
                }
            } else if buttons & 2 != 0 {
                return Some(None);
            } else {
                sm.released = true;
            }
            return None;
        }
        if x < self.x0 || x > self.x0 + 41 || y <= 12 || y >= 73 {
            self.unlight(screen);
        } else {
            for i in 0..7 {
                let r = 19 + 7 * i as i32;
                if r < y && y < r + 8 {
                    if !self.flags[i] {
                        self.unlight(screen);
                    } else if self.hover != Some(i) {
                        self.unlight(screen);
                        self.hover = Some(i);
                        self.label(screen, i, true);
                    }
                }
            }
        }
        let c = if buttons & 1 != 0 {
            if !self.released {
                return None;
            }
            *click = true;
            self.hover
        } else if buttons & 2 != 0 {
            None
        } else {
            self.released = true;
            return None;
        };
        if c == Some(2) {
            // The subject menu.
            let x = if self.k < 7 { self.x0 + 10 } else { self.x0 - 8 };
            let sm = SubMenu { x, y: 30, hover: None, released: false };
            screen.element(0x28, x, 30, None);
            screen.element(0x29, x, 50, None);
            screen.element(0x2A, x, 60, None);
            for (i, n) in SUBJECTS.iter().enumerate() {
                screen.text(n, x + 7, 39 + 7 * i as i32, false, 8, 0x0E, 0);
            }
            self.sub = Some(sm);
            return None;
        }
        Some(c.map(|c| (c as u8, None)))
    }
}

/// Choose (after the menu and the subject menu): sets `duty` / `trainWhat` of slot k. Returns true when the
/// accounts must be counted again (CalcLedgerAll). FIRE: the window asks first (mapui.rs `answer_fire`).
pub fn choose(soldiers: &mut [u8], k: usize, c: DutyChoice) -> bool {
    let s = &mut soldiers[k * SOLDIER..(k + 1) * SOLDIER];
    match c {
        Some((d, tw)) if d <= 5 => {
            if let Some(t) = tw {
                s[TRAIN_WHAT] = t;
            }
            s[DUTY] = d;
            true
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Selecting a merc (strategic.md "Selecting a merc")

/// The team slot whose face box holds the mouse at (x, y).
pub fn face_box_at(x: i32, y: i32) -> Option<usize> {
    (0..8).find(|&k| {
        let fx = 11 + 37 * k as i32;
        (fx..=fx + 30).contains(&x) && (20..=46).contains(&y)
    })
}

/// The item places table of the assignment screen.
pub const ITEM_PLACES: [(i32, i32); 14] = [
    (108, 73), (203, 89), (161, 8), (108, 31), (200, 28), (108, 52), (257, 18), (257, 37), (257, 56), (257, 75),
    (257, 94), (257, 113), (108, 92), (159, 115),
];

/// The item store as the store list shows it. `view` 0 = records, 1 = counts per type; `records` = the chain of
/// records (type, quantity, condition), `cur` = the current record (`records.len()` or more = none); `top` = the
/// top type of view 1; `v` = button 14's overlay element V (strategic.md "The panel buttons").
pub struct Store<'a> {
    pub view: u8,
    pub counts: &'a [u8],
    pub records: &'a [[u8; 3]],
    pub cur: usize,
    pub top: u8,
    pub shown: [u8; 4],
    pub v: u8,
}

/// Fix(T, p) of the store list: the top type of view 1.
pub fn fix_top(counts: &[u8], t: i32, p: i32) -> u8 {
    let first4 = |from: i32| -> Vec<i32> {
        (from.max(0)..=135).filter(|&u| counts.get(u as usize).copied().unwrap_or(0) != 0).take(4).collect()
    };
    let mut l = first4(t);
    if !l.contains(&p) {
        l = first4(p);
    }
    if l.len() == 4 {
        return l[0] as u8;
    }
    let down: Vec<i32> = (1..=135).rev().filter(|&u| counts[u as usize] != 0).take(4).collect();
    down.last().copied().unwrap_or(0) as u8
}

/// The store list into the panel buffer.
pub fn store_list(p: &mut Painter, main: &InvData, st: &mut Store) {
    let place = |j: usize| (29, 59 + 19 * j as i32);
    if st.view == 0 {
        for j in 0..4 {
            let (x, y) = place(j);
            p.element(0x32, x, y, None);
            p.element(0x34, 12, 61 + 19 * j as i32, None);
        }
        for j in 0..4 {
            let (x, y) = place(j);
            let Some(&[t, q, c]) = st.records.get(st.cur + j) else {
                crate::invdraw::cond_bar(p, 0, x - 11, y + 4);
                continue;
            };
            crate::invdraw::picture(p, main, main.ib(t, 4), x + 1, y + 1, 8);
            let (q, c) = (q as i8 as i32, c as i8 as i32);
            if c < 10 && (main.ib(t, 9) != 0 || main.ib(t, 7) != 0) {
                p.text(b"useless", x + 22, y + 7, true, 0xED, 1, 0);
            } else if q < 0 {
                p.text(b"jammed", x + 22, y + 7, true, 0xED, 1, 0);
            }
            crate::invdraw::cond_bar(p, c, x - 11, y + 4);
        }
        return;
    }
    st.top = fix_top(st.counts, st.top as i32, st.top as i32);
    let mut v = st.top;
    for j in 0..4 {
        let (x, y) = place(j);
        p.element(0x32, x, y, None);
        crate::invdraw::picture(p, main, main.ib(v, 4), x + 1, y + 1, 8);
        let n = st.counts.get(v as usize).copied().unwrap_or(0);
        let txt = if n != 0 { format!("{:^>2}", n).into_bytes() } else { b"  ".to_vec() };
        p.text_mode(&txt, x - 14, y + 7, false, 0x27, 0x32, 0, 11);
        st.shown[j] = v;
        if v != 0 {
            v = (v as usize + 1..=135).find(|&u| st.counts[u] != 0).unwrap_or(0) as u8;
        }
    }
}

/// The merc's item panel of S (team slot k of `soldiers`) into the panel buffer; returns the loader result.
pub fn item_panel(p: &mut Painter, main: &InvData, soldiers: &mut [u8], k: usize, faces: &[u8], st: &mut Store)
                  -> i32 {
    let sb = k * SOLDIER;
    load_pcx(p, main, "INVMAP.PCC", 0, 0);
    let body = soldiers[sb + 0x17C];
    let mut res = load_pcx(p, main, &format!("BODY{}.PCC", body), 159, 27);
    if soldiers[sb + 0x1B1] != 0 {
        res = load_pcx(p, main, &format!("CAMO{}.PCC", body), 159, 42);
    }
    let s = &soldiers[sb..sb + SOLDIER];
    number(p, crate::invdraw::armour(s), 3, 175, 53, 0xFF, 1);
    p.text(b"%", 187, 53, false, 0xFF, 1, 1);
    let slot = s[SLOT] as usize;
    // SPEC GAP: a slot byte above 7 has no face buffer; no face is drawn then.
    if slot < 8 {
        for r in 0..25 {
            for c in 0..28 {
                p.put(34 + c, 12 + r, faces[slot * 1024 + (r as usize + 2) * 32 + c as usize + 2]);
            }
        }
    }
    if let Some(n) = DUTY_NAMES.get(s[DUTY] as usize) {
        p.text(n, 72, 12, false, 0x0E, 5, 1);
    }
    let nick = &s[NICKNAME..NICKNAME + 9];
    let nick = &nick[..nick.iter().position(|&b| b == 0).unwrap_or(9)];
    p.text(nick, 48, 42, true, 0x0E, 5, 0);
    p.element(0x2D, 79, 90, None);
    p.element(st.v as usize, 82, 93, None);
    p.element(if st.view == 0 { 0x36 } else { 0x37 }, 11, 52, None);
    for place in 0..13 {
        if !crate::invdraw::pocket_missing(&soldiers[sb..sb + SOLDIER], place) {
            crate::invdraw::item_place(p, main, &ITEM_PLACES, &[], &[], soldiers, sb, place);
        }
    }
    store_list(p, main, st);
    res
}

/// Select merc k (the press on its face box): the merc pay panel and the item panel into the panel buffer, then
/// the whole panel buffer to the screen. Returns the loader result: 0, as the mouse move to `SELECT_MOUSE` sets it.
pub fn select(panel: &mut Painter, screen: &mut Painter, main: &InvData, soldiers: &mut [u8], k: usize,
              faces: &[u8], acc: &[u8], st: &mut Store) -> i32 {
    merc_pay_panel(panel, main, &soldiers[k * SOLDIER..(k + 1) * SOLDIER], acc);
    item_panel(panel, main, soldiers, k, faces, st);
    screen.buf[..64000].copy_from_slice(&panel.buf[..64000]);
    0
}

/// Where the mouse goes after a selection.
pub const SELECT_MOUSE: (i32, i32) = (50, 26);

// ---------------------------------------------------------------------------------------------------------------
// The panel buttons (strategic.md "The panel buttons")

/// The face of the item panel: a press deselects the merc.
pub fn panel_face_at(x: i32, y: i32) -> bool {
    (34..=62).contains(&x) && (12..=36).contains(&y)
}

/// The body of the item panel.
pub fn panel_body_at(x: i32, y: i32) -> bool {
    (161..=199).contains(&x) && (48..=84).contains(&y)
}

/// Buttons 10..14: (bx, by), overlay element (0 = V), (dx, dy).
pub const PANEL_BUTTONS: [(i32, i32, usize, i32, i32); 5] =
    [(8, 25, 0x19, 4, 3), (69, 25, 0x1A, 4, 3), (79, 72, 0x1B, 4, 3), (79, 108, 0x1C, 4, 3), (79, 90, 0, 3, 3)];

/// The panel button (0..4 = buttons 10..14) under the mouse.
pub fn panel_button_at(x: i32, y: i32) -> Option<usize> {
    PANEL_BUTTONS.iter().position(|&(bx, by, ..)| (bx..=bx + 19).contains(&x) && (by..=by + 15).contains(&y))
}

/// The face of panel button i on the screen: pressed or up; `v` = the overlay V of button 14.
pub fn panel_button(screen: &mut Painter, i: usize, pressed: bool, v: u8) {
    let (bx, by, ov, dx, dy) = PANEL_BUTTONS[i];
    let ov = if ov == 0 { v as usize } else { ov };
    let d = if pressed { 1 } else { 0 };
    screen.element(if pressed { 0x2C } else { 0x2D }, bx, by, None);
    screen.element(ov, bx + dx - d, by + dy - d, None);
}

/// Where the mouse goes when merc k is deselected.
pub fn deselect_mouse(k: usize) -> (i32, i32) {
    (28 + 37 * k as i32, 33)
}

/// Previous (`next` = false) or next merc after slot k; `last` = the last slot of the picture's list.
pub fn other_merc(k: usize, last: usize, next: bool) -> usize {
    let j = if next { k as i32 + 1 } else { k as i32 - 1 };
    if j < 0 {
        last
    } else if j as usize > last {
        0
    } else {
        j as usize
    }
}

/// Merc k's panels after a merc change: the merc pay panel, the item panel (no mouse move), all rows to the
/// screen. Returns the loader result.
pub fn show_merc(panel: &mut Painter, screen: &mut Painter, main: &InvData, soldiers: &mut [u8], k: usize,
                 faces: &[u8], acc: &[u8], st: &mut Store) -> i32 {
    merc_pay_panel(panel, main, &soldiers[k * SOLDIER..(k + 1) * SOLDIER], acc);
    let r = item_panel(panel, main, soldiers, k, faces, st);
    screen.buf[..64000].copy_from_slice(&panel.buf[..64000]);
    r
}

impl Store<'_> {
    fn none(&self) -> bool {
        self.cur >= self.records.len()
    }

    fn prev(&mut self) {
        if self.none() || self.cur == 0 {
            return;
        }
        self.cur -= 1;
        self.top = self.records[self.cur][0];
    }

    fn next(&mut self) {
        if self.none() {
            return;
        }
        if self.cur + 4 < self.records.len() {
            self.cur += 1;
        }
        self.top = self.records[self.cur][0];
    }

    fn count(&self, u: i32) -> u8 {
        self.counts.get(u as usize).copied().unwrap_or(0)
    }

    fn fix(&self, t: i32) -> u8 {
        fix_top(self.counts, t, t)
    }

    /// Store up (`down` = false) or store down; true when the list must be drawn again.
    pub fn scroll(&mut self, down: bool) -> bool {
        if self.view == 0 {
            let c0 = self.cur;
            for _ in 0..4 {
                let c = self.cur;
                if down { self.next() } else { self.prev() }
                if self.cur == c {
                    break;
                }
            }
            return self.cur != c0;
        }
        let t0 = self.top;
        let mut n = 0;
        if down {
            loop {
                let t = self.top as i32;
                let u = (t + 1..=135).find(|&u| self.count(u) != 0);
                self.top = self.fix(u.unwrap_or(t0 as i32));
                if self.top == t0 {
                    break;
                }
                n += 1;
                if n > 3 {
                    break;
                }
            }
        } else {
            loop {
                let t = self.top as i32;
                let Some(u) = (1..t).rev().find(|&u| self.count(u) != 0) else {
                    self.top = self.fix(t);
                    break;
                };
                let before = self.top;
                self.top = self.fix(u);
                if self.top != before {
                    n += 1;
                }
                if n >= 4 {
                    break;
                }
            }
        }
        self.top != t0
    }
}

/// The store list drawn again: into the panel buffer, then its part x 0..72, y 52..140 to the screen.
pub fn store_again(panel: &mut Painter, screen: &mut Painter, main: &InvData, st: &mut Store) {
    store_list(panel, main, st);
    for y in 52..141 {
        let a = y * 320;
        screen.buf[a..a + 73].copy_from_slice(&panel.buf[a..a + 73]);
    }
}

/// Button 14's action after the toggle of the view (the toggle and V are done at the press).
pub fn store_view(panel: &mut Painter, screen: &mut Painter, main: &InvData, st: &mut Store) {
    panel.element(if st.view == 0 { 0x36 } else { 0x37 }, 11, 52, None);
    for j in 0..4 {
        panel.element(if st.view == 0 { 0x34 } else { 0x35 }, 12, 61 + 19 * j, None);
    }
    if st.view == 0 {
        st.cur = st.records.iter().position(|r| r[0] == st.shown[0]).unwrap_or(st.records.len());
    }
    store_again(panel, screen, main, st);
}

/// The toggle of button 14 at its press: the view and V.
pub fn toggle_view(st: &mut Store) {
    st.view ^= 1;
    st.v = if st.view == 1 { 0x38 } else { 0x39 };
}

// ---------------------------------------------------------------------------------------------------------------
// Hover (strategic.md "Hover on the assignment screen", panel.md "Tooltip box")

/// The tooltip box (panel.md "Tooltip box"): the number T that shows (0 = none), the kept position and the saved
/// screen rectangle.
#[derive(Clone, Debug, Default)]
pub struct Tooltip {
    pub n: u8,
    at: (i32, i32),
    saved: Vec<u8>,
}

impl Tooltip {
    /// The kept position (kx, ky).
    pub fn at(&self) -> (i32, i32) {
        self.at
    }

    fn size(n: u8) -> (i32, i32) {
        if n == 9 {
            (64, 53)
        } else {
            (36, 28)
        }
    }

    /// The linear screen offsets of the rectangle of number n at the kept position (the screen is 64000 bytes).
    fn offsets(&self, n: u8) -> impl Iterator<Item = (usize, Option<usize>)> {
        let ((w, h), (x, y)) = (Self::size(n), self.at);
        (0..h).flat_map(move |r| (0..w).map(move |c| {
            let o = (y + r) * PW + x + c;
            ((r * w + c) as usize, (0..64000).contains(&o).then_some(o as usize))
        }))
    }

    /// Hide: the saved rectangle (T's size) back on the screen; T = 0.
    pub fn hide(&mut self, p: &mut Painter) {
        if self.n == 0 {
            return;
        }
        for (i, o) in self.offsets(self.n).collect::<Vec<_>>() {
            if let (Some(o), Some(&v)) = (o, self.saved.get(i)) {
                p.buf[o] = v;
            }
        }
        self.n = 0;
    }

    /// Show tooltip n at (x0, y0, x1, y1) with the texts t and font f.
    pub fn show(&mut self, p: &mut Painter, n: u8, (x0, y0, x1, y1): (i32, i32, i32, i32), t: [&[u8]; 3], f: usize) {
        if n == self.n {
            return;
        }
        self.hide(p);
        self.n = n;
        self.at = (x0 - if n == 9 { 5 } else { 4 }, y0 - 1);
        let (w, h) = Self::size(n);
        let mut saved = vec![0u8; (w * h) as usize];
        for (i, o) in self.offsets(n) {
            if let Some(o) = o {
                saved[i] = p.buf[o];
            }
        }
        self.saved = saved;
        // Step 4: main-bank effect 0 at 11025 Hz, volume 5, once, on the screen sound (only where it has its files:
        // the window; stderr `tooltip sound`).
        crate::sound::screen(|s| {
            if s.has_files() {
                s.sfx(0, 0x2B11, 5, 1);
                eprintln!("tooltip sound");
            }
        });
        p.frame(x0 - 1, y0 - 1, x1, y1, 0xFF);
        let (cx, cy) = (x0 + (x1 - x0) / 2, y0 + (y1 - y0) / 2);
        if t[1].is_empty() {
            p.text(t[0], cx, cy - 3, true, 0xFF, 0xFE, f);
        } else {
            p.text(t[0], cx - 1, cy - 12, true, 0xFF, 0xFE, f);
            p.text(t[1], cx - 1, cy - 4, true, 0xFF, 0xFE, f);
            if !t[2].is_empty() {
                p.text(t[2], cx - 1, cy + 4, true, 0xFF, 0xFE, f);
            }
        }
    }
}

/// Light stat row r (99 = none) of the picture; `lit` = the lit row L (99 = none).
pub fn light(p: &mut Painter, soldiers: &[u8], lit: &mut u8, r: u8) {
    for k in 0..merc_count(soldiers) {
        let s = &soldiers[k * SOLDIER..(k + 1) * SOLDIER];
        let x = 27 + 37 * s[SLOT] as i32;
        if *lit != 99 {
            let l = *lit as usize;
            number(p, stat_value(s, l), 3, x, 59 + 8 * l as i32, if l % 2 == 0 { 237 } else { 41 }, 0);
        }
        if r != 99 {
            number(p, stat_value(s, r as usize), 3, x, 59 + 8 * r as i32, 0xFF, 0);
        }
    }
    *lit = r;
}

/// One poll with no button down: the face tooltips and the stat row light (no merc selected), or the "exit" tooltip
/// over the item panel's face (merc `selected`).
pub fn hover(p: &mut Painter, soldiers: &[u8], selected: Option<usize>, (mx, my): (i32, i32), tip: &mut Tooltip,
             lit: &mut u8) {
    let inside = |x0: i32, y0: i32, x1: i32, y1: i32| x0 <= mx && mx <= x1 && y0 <= my && my <= y1;
    if let Some(k) = selected {
        if panel_face_at(mx, my) {
            tip.show(p, k as u8 + 1, (34, 12, 62, 37), [b"exit", b"", b""], 0);
        } else {
            tip.hide(p);
        }
        return;
    }
    let mut any = false;
    for k in 0..8 {
        let x = 12 + 37 * k as i32;
        if inside(x, 20, x + 30, 46) && in_sector(soldiers, k) {
            tip.show(p, k as u8 + 1, (x + 1, 20, x + 29, 45), [b"gear", b"", b""], 0);
            any = true;
        }
    }
    if !any {
        tip.hide(p);
    }
    if !(59..137).contains(&my) {
        if *lit != 99 {
            light(p, soldiers, lit, 99);
        }
    } else {
        let r = (1..10).find(|&i| 59 + 8 * i > my).map_or(9, |i| i - 1) as u8;
        if r != *lit {
            light(p, soldiers, lit, r);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pressed panel button: the down face 0x2C and its overlay one pixel up and left of the up place.
    #[test]
    fn a_pressed_button_moves_its_overlay_up_left() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let (iso, exe) = (iso.to_str().unwrap(), exe.to_str().unwrap());
        let (c, pd) = (crate::common(iso).unwrap(), crate::panel::panel_data(iso, exe).unwrap());
        let draw = |f: &mut dyn FnMut(&mut Painter)| {
            let mut buf = vec![0u8; 64000];
            let mut table0 = c.remap[0];
            let mut p = Painter { d: &pd, buf: &mut buf, table0: &mut table0, table29: crate::panel::table29(&c) };
            f(&mut p);
            buf
        };
        for i in 0..PANEL_BUTTONS.len() {
            let (bx, by, ov, dx, dy) = PANEL_BUTTONS[i];
            let ov = if ov == 0 { 3 } else { ov };
            let got = draw(&mut |p| panel_button(p, i, true, 3));
            let want = draw(&mut |p| {
                p.element(0x2C, bx, by, None);
                p.element(ov, bx + dx - 1, by + dy - 1, None);
            });
            assert!(got == want, "button {i}");
            let other = draw(&mut |p| {
                p.element(0x2C, bx, by, None);
                p.element(ov, bx + dx + 1, by + dy + 1, None);
            });
            assert!(got != other, "button {i}: the overlay place shows");
        }
    }
}
