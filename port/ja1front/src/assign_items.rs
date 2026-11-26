//! Item moves on the item panel of the assignment screen (docs/spec/strategic.md "Item moves on the item panel",
//! "The item store", "The vest map"): the item store records the host keeps and the host ops of
//! `ja1_assign_item_press` (port_api.md).
use crate::assign::{self, Store, ITEM_PLACES, SOLDIER};
use crate::host::{Ja1Host, Ja1World};
use crate::invdraw::{self, InvData};
use crate::panel::Painter;
use std::collections::VecDeque;
use std::ffi::c_void;

extern "C" {
    pub fn ja1_assign_item_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32, armour: *mut i32) -> i32;
    pub fn ja1_assign_key_e(w: *mut Ja1World, h: *const Ja1Host, slot: i32);
    pub fn ja1_assign_right_press(w: *mut Ja1World, h: *const Ja1Host, slot: i32) -> i32;
}

/// "No current record" (any index past the records).
pub const NONE: usize = 1 << 20;
const CANTEEN: u8 = 0x41;

/// The item store: records (type, count, condition) in list order and the state of the store list.
pub struct ItemStore {
    pub view: u8,
    pub records: Vec<[u8; 3]>,
    pub cur: usize,
    pub top: u8,
    pub shown: [u8; 4],
    pub v: u8,
}

impl ItemStore {
    /// `f` with the store as assign::Store; the list state comes back.
    pub fn with<R>(&mut self, counts: &[u8], f: impl FnOnce(&mut Store) -> R) -> R {
        let mut st = Store { view: self.view, counts, records: &self.records, cur: self.cur, top: self.top,
                             shown: self.shown, v: self.v };
        let r = f(&mut st);
        let (view, cur, top, shown, v) = (st.view, st.cur, st.top, st.shown, st.v);
        self.view = view;
        self.cur = if cur >= self.records.len() { NONE } else { cur };
        self.top = top;
        self.shown = shown;
        self.v = v;
        r
    }

    /// Op 264 stash_put: sorted insert (type ascending, condition descending as a signed byte, after equals); a
    /// canteen gets condition 100. The current record stays on its record.
    pub fn put(&mut self, t: u8, q: u8, c: u8) -> u32 {
        let c = if t == CANTEEN { 100 } else { c };
        let at = self.records.iter().position(|r| r[0] > t || (r[0] == t && (r[2] as i8) < (c as i8)))
            .unwrap_or(self.records.len());
        self.records.insert(at, [t, q, c]);
        if self.cur != NONE && at <= self.cur {
            self.cur += 1;
        }
        1
    }

    /// Op 323 stash_take_rec.
    pub fn take(&mut self, t: u8, q: u8, c: u8, exact: bool) -> u32 {
        let Some(at) = self.records.iter().position(|r| r[0] == t && (!exact || (r[1] == q && r[2] == c))) else {
            return 0;
        };
        let r = self.records.remove(at);
        if self.cur != NONE && at < self.cur {
            self.cur -= 1;
        }
        if self.cur >= self.records.len() {
            self.cur = NONE;
        }
        0x10000 + r[1] as u32 * 256 + r[2] as u32
    }

    /// Op 324 store_place (`last`: a walk past the last record stays on it).
    pub fn place(&self, j: usize, last: bool) -> u32 {
        if self.view == 0 {
            // No current record: the original reads machine memory (strategic.md); the library stops there.
            if self.cur == NONE {
                return 1 << 26;
            }
            let i = if last { (self.cur + j).min(self.records.len().saturating_sub(1)) } else { self.cur + j };
            return match self.records.get(i) {
                Some(&[t, q, c]) => 1 << 24 | 1 << 25 | t as u32 | (q as u32) << 8 | (c as u32) << 16,
                None => 0,
            };
        }
        1 << 24 | self.shown[j] as u32
    }

    /// The first part of op 325 store_again (took ≠ 0): the top type and the current record.
    pub fn after_take(&mut self, counts: &[u8]) {
        self.top = assign::fix_top(counts, self.top as i32, self.top as i32);
        self.cur = self.records.iter().position(|r| r[0] == self.top).unwrap_or(NONE);
    }
}

/// The host of `ja1_assign_item_press`: the panel buffer, the screen, the store, the input script.
pub struct AssignHost<'a> {
    pub pp: Painter<'a>,
    pub sp: Painter<'a>,
    pub inv: &'a InvData,
    pub soldiers: *mut u8,
    pub faces: &'a [u8],
    pub store: ItemStore,
    pub world: *mut Ja1World,
    /// Op 221: the next mouse states (x, y, buttons); none left = stop.
    pub events: VecDeque<(i32, i32, u8)>,
    /// (kind, value): 1 click, 2 cursor picture (type), 3 inv_extra (sound), 4 sfx (effect), 5 merc_text (text),
    /// 6 no_room, 7 a question (discard / merge), 8 quote / yes_no / stat_practice (op), 9 random, 10 message,
    /// 99 another op.
    pub log: Vec<(i32, i32)>,
    pub loader: i32,
    /// World_load.md "The last quote": the record of op 252 merge_quote, for the world's `last_quote`.
    pub quote: Option<[u8; 6]>,
    pub answer: i32,
    /// The window: op 231 draws the item info box on the screen and keeps the screen before it here (the box
    /// stays until the next button down). Off (the diff test): op 231 answers `answer`.
    pub show_info: bool,
    pub info_under: Option<Vec<u8>>,
    /// ItemInfoBox 0x260C6: a box with text and the mouse flag (argument 6): the window moves the mouse.
    pub info_mouse: bool,
    /// A picture item (letter, photo, map piece; panel.md "Item info box" step 4): the screen with its picture. The
    /// original waits for a button inside op 231 and only then draws the screen again (result 1); the window shows
    /// this screen until the next button down and the redrawn one after it.
    pub info_shot: Option<Vec<u8>>,
    /// The Control Panel switches Item Info (6) and Subtitles (5) for the box.
    pub opt_info: bool,
    pub opt_sub: bool,
    /// The window: the text of an op 237 inv_extra to show in a 1000 ms message box (inventory.md "inv_extra").
    pub extra: Option<&'static str>,
}

impl AssignHost<'_> {
    fn counts(&self) -> &'static [u8] {
        // SAFETY: the world's stash_counts (136 bytes) lives longer than the host.
        unsafe { std::slice::from_raw_parts((*self.world).stash_counts, 136) }
    }

    /// The vest map (strategic.md "The vest map") of soldier `s`.
    fn vestmap(&mut self, s: usize) {
        let (x7, y7) = ITEM_PLACES[7];
        self.loader = invdraw::load_pcx(&mut self.pp, self.inv, "VESTMAP.PCC", x7, y7 + 1);
        let so = unsafe { std::slice::from_raw_parts_mut(self.soldiers, 9 * SOLDIER) };
        for j in 7..12 {
            if !invdraw::pocket_missing(&so[s * SOLDIER..(s + 1) * SOLDIER], j) {
                invdraw::item_place(&mut self.pp, self.inv, &ITEM_PLACES, &[], &[], so, s * SOLDIER, j);
            }
        }
        for y in y7 as usize..=140 {
            let a = y * 320;
            self.sp.buf[a + x7 as usize..a + 320].copy_from_slice(&self.pp.buf[a + x7 as usize..a + 320]);
        }
    }

    /// One host call.
    pub fn op(&mut self, op: i32, a: &[i32]) -> u32 {
        let arg = |i: usize| a.get(i).copied().unwrap_or(0);
        let w = self.world;
        match op {
            221 => match self.events.pop_front() {
                Some((x, y, b)) => {
                    // SAFETY: `world` is the test's world for the host's life.
                    unsafe {
                        (*w).mouse_x = x;
                        (*w).mouse_y = y;
                        (*w).mouse_buttons = b;
                    }
                    0
                }
                None => 1,
            },
            238 => {
                let (s, p) = (arg(0) as usize, arg(1) as usize);
                if s < 9 && p < 14 {
                    let so = unsafe { std::slice::from_raw_parts_mut(self.soldiers, 9 * SOLDIER) };
                    invdraw::item_place(&mut self.sp, self.inv, &ITEM_PLACES, &[], &[], so, s * SOLDIER, p);
                }
                0
            }
            239 => {
                if (0..9).contains(&arg(0)) {
                    self.vestmap(arg(0) as usize);
                }
                0
            }
            321 => {
                let k = arg(0) as usize;
                if k < 9 {
                    let counts = self.counts();
                    let so = unsafe { std::slice::from_raw_parts_mut(self.soldiers, 9 * SOLDIER) };
                    let AssignHost { pp, inv, faces, store, .. } = self;
                    self.loader = store.with(counts, |st| assign::item_panel(pp, inv, so, k, faces, st));
                    self.sp.buf[..64000].copy_from_slice(&self.pp.buf[..64000]);
                }
                0
            }
            322 | 251 => {
                self.log.push((7, 0));
                if op == 251 {
                    // MergeItemsPrompt 0x4AA26 from AssignScreen? 0x497DB: the window asks inside the press.
                    let items = crate::question::merge_items(&self.inv.item_types, arg(0) as u8, arg(1) as u8);
                    if let Some(yes) = crate::question::merge_wait(&*self.sp.buf, items, true) {
                        return yes as u32;
                    }
                }
                self.answer as u32
            }
            323 => self.store.take(arg(0) as u8, arg(1) as u8, arg(2) as u8, arg(3) != 0),
            324 => self.store.place(arg(0).clamp(0, 3) as usize, arg(1) != 0),
            325 => {
                let counts = self.counts();
                if arg(0) != 0 {
                    self.store.after_take(counts);
                }
                let AssignHost { pp, sp, inv, store, .. } = self;
                store.with(counts, |st| assign::store_again(pp, sp, inv, st));
                0
            }
            264 => self.store.put(arg(0) as u8, arg(1) as u8, arg(2) as u8),
            231 => {
                // item_info: logged for the test, the answer as its result; the window draws the box
                self.log.extend([(11, arg(0) & 0xFF), (12, arg(1) & 0xFF), (13, arg(2)), (14, arg(3)),
                                 (15, arg(4) * 4 + arg(5) * 2 + arg(6))]);
                if !self.show_info {
                    return self.answer as u32;
                }
                let under = self.sp.buf.to_vec();
                let i = crate::iteminfo::Info {
                    profile: arg(0) as u8, t: arg(1) as u8, count: arg(2) as i8, value: arg(3) as i16,
                    numbers: numbers_flag(arg(4)), opt_info: self.opt_info, opt_sub: self.opt_sub,
                };
                let shown = crate::iteminfo::draw(&mut self.sp, self.inv, i);
                if shown != crate::iteminfo::Shown::Nothing {
                    self.info_under = Some(under);
                }
                self.info_mouse = info_waits_for_mouse(arg(6), shown);
                if shown == crate::iteminfo::Shown::Picture {
                    self.info_shot = Some(self.sp.buf.to_vec());
                }
                // panel.md "The item's voice" on the screen sound; the next button down stops every sound.
                if i.t != 0 {
                    let names = crate::sound::screen(|snd| {
                        let v = crate::iteminfo::voice(&i, &self.inv.item_types, arg(5) != 0, &|arc, m| snd.inv_has(arc, m));
                        v.map(|v| snd.item_voice(&v))
                    });
                    eprintln!("assign item voice {}", names.map_or("none".to_string(), |n| n.join(" ")));
                }
                shown.result() as u32
            }
            227 => {
                self.log.push((1, 0));
                if self.show_info {
                    // sound.md "Click sound" on the screen sound (the window loaded the main bank)
                    crate::sound::screen(|snd| snd.sfx(0, 0x2B11, 6, 1));
                    eprintln!("screen sound IN");
                }
                0
            }
            225 | 226 => {
                self.log.push((2, arg(0) & 0xFF));
                0
            }
            237 => {
                self.log.push((3, arg(0)));
                if self.show_info {
                    // inventory.md "inv_extra": the sound on the screen sound; with Subtitles on the text in a box
                    let (member, text) = crate::invscreen::extra_names(arg(0), arg(1));
                    let played = play_extra(member, |m| crate::sound::screen(|snd| snd.inv_extra(m)));
                    if self.opt_sub && !text.is_empty() {
                        self.extra = Some(text);
                    }
                    eprintln!("assign inv extra {} {} box {}", if played { member } else { "-" },
                              if text.is_empty() { "-" } else { text }, self.extra.is_some());
                }
                0
            }
            60 => {
                self.log.push((4, arg(0) & 0xFF));
                if self.show_info {
                    // the reload sounds (main-bank effects 0x19 / 0x1A)
                    crate::sound::screen(|snd| snd.sfx(arg(0), arg(1), arg(2) as u8 as i8 as i32, arg(3)));
                    eprintln!("screen sound {}", crate::sound::main_name(arg(0)));
                }
                0
            }
            57 => {
                self.log.push((5, arg(1)));
                0
            }
            240 => {
                self.log.push((6, 0));
                0
            }
            252 | 253 | 105 => {
                if op == 252 {
                    let mut q = [0; 6];
                    crate::play::set_quote(&mut q, arg(0) as u8, 1, 6);
                    self.quote = Some(q);
                }
                self.log.push((8, op));
                0
            }
            125 => {
                self.log.push((10, 0));
                0
            }
            145 | 224 | 245 | 246 => 0,
            47 => {
                eprintln!("op 47: the original game stops here (store press with no current record)");
                self.log.push((99, 47));
                0
            }
            _ => {
                self.log.push((99, op));
                0
            }
        }
    }
}

unsafe extern "C" fn host_call(c: *mut c_void, op: i32, a: *const i32, n: i32, _o: *mut u8, _s: i32) -> u32 {
    let h = &mut *(c as *mut AssignHost);
    let args = if n > 0 && !a.is_null() { std::slice::from_raw_parts(a, n as usize).to_vec() } else { vec![] };
    h.op(op, &args)
}

unsafe extern "C" fn host_random(c: *mut c_void, _n: i32) -> i32 {
    let h = &mut *(c as *mut AssignHost);
    h.log.push((9, 0));
    0
}

/// A Ja1Host over `h` (valid while `h` is not moved).
pub fn ja1_host(h: &mut AssignHost) -> Ja1Host {
    Ja1Host { ctx: h as *mut AssignHost as *mut c_void, random: host_random, call: host_call, host_ops: 0 }
}

/// The item places table of the assignment screen as the library reads it (i16 x, y per place).
pub fn item_rects() -> Vec<u8> {
    let mut v = Vec::with_capacity(64);
    for (x, y) in ITEM_PLACES {
        v.extend_from_slice(&(x as i16).to_le_bytes());
        v.extend_from_slice(&(y as i16).to_le_bytes());
    }
    v.resize(64, 0);
    v
}

/// item_info (op 231) argument 6 = the box waits for the mouse; only a text box waits (a picture stays until the next
/// button down, no box = nothing to wait for).
fn info_waits_for_mouse(arg6: i32, shown: crate::iteminfo::Shown) -> bool {
    arg6 != 0 && !matches!(shown, crate::iteminfo::Shown::Picture | crate::iteminfo::Shown::Nothing)
}

/// inventory.md "inv_extra" (op 237): the sound plays only when the remark has a sound member.
fn play_extra(member: &str, play: impl FnOnce(&str) -> bool) -> bool {
    !member.is_empty() && play(member)
}

/// Host op 231 item_info: the numbers flag a (argument 4) shows the numbers only when it is 1 (panel.md "Item info box").
fn numbers_flag(a: i32) -> bool {
    a == 1
}

#[cfg(test)]
mod rule_tests {
    use super::*;
    use crate::iteminfo::Shown;

    /// Ledger row 0b4d529:ja1front:5637 (panel.md "Item info box", numbers flag a of host op 231): the box shows the
    /// numbers only for a = 1.
    #[test]
    fn the_item_box_shows_its_numbers_only_for_flag_1() {
        assert!(numbers_flag(1));
        assert!(!numbers_flag(0));
        assert!(!numbers_flag(2));
    }

    /// Ledger row d057aca:ja1front:5593: the item info box waits for the mouse only with argument 6 set and a box shown.
    #[test]
    fn only_a_text_box_with_the_mouse_flag_waits_for_the_mouse() {
        for shown in [Shown::Picture, Shown::Nothing] {
            assert!(!info_waits_for_mouse(1, shown));
            assert!(!info_waits_for_mouse(0, shown));
        }
        assert!(info_waits_for_mouse(1, Shown::Box));
        assert!(!info_waits_for_mouse(0, Shown::Box));
    }

    /// Ledger row d057aca:ja1front:5603: a remark with a sound member plays it; one without plays nothing.
    #[test]
    fn a_remark_sound_plays_only_with_a_member() {
        let mut asked = Vec::new();
        assert!(play_extra("X_RUSTY", |m| { asked.push(m.to_string()); true }));
        assert!(!play_extra("X_RUSTY", |_| false));
        assert!(!play_extra("", |m| { asked.push(m.to_string()); true }));
        assert_eq!(asked, ["X_RUSTY"]);
    }
}

/// One left press of the window on the item panel of merc `k` (strategic.md "Item moves on the item panel"):
/// `ja1_assign_item_press` with the world's cursor item, swap item, vest owner and item store; the left button is
/// up at the first poll. `keys` = (shift, ctrl); `answer` = the discard question's answer (the window asks first).
/// x < 0: the key E instead (`ja1_assign_key_e`, strategic.md "Key E: all items into the store"); x ≥ 1000: a right
/// press at (x − 1000, y) (`ja1_assign_right_press`, its waits see the right button up). Returns the store and, when
/// an item info box shows, the screen before it.
#[allow(clippy::too_many_arguments)]
#[derive(Debug, PartialEq)]
enum PressWay {
    KeyE,
    Right,
    Left,
}

/// The way of a window press: a negative x = key E (the caller's marker), x >= 1000 = the right button, else the
/// left button. A press at x 0 is a left press.
fn press_way(x: i32, right: bool) -> PressWay {
    if x < 0 {
        PressWay::KeyE
    } else if right {
        PressWay::Right
    } else {
        PressWay::Left
    }
}

pub fn window_press(pp: Painter, sp: Painter, inv: &InvData, faces: &[u8], w: &mut crate::world::World,
                    store: ItemStore, k: usize, (x, y): (i32, i32), (shift, ctrl): (bool, bool), answer: i32,
                    armour: &mut i32) -> (ItemStore, Option<Vec<u8>>, Option<&'static str>, bool) {
    let rects = item_rects();
    let mut jw = Ja1World::view(w);
    jw.item_rects = rects.as_ptr();
    jw.key_shift = shift as u8;
    jw.key_ctrl = ctrl as u8;
    jw.mouse_x = x;
    jw.mouse_y = y;
    jw.mouse_buttons = 1;
    let (x, right) = if x >= 1000 { (x - 1000, true) } else { (x, false) };
    jw.mouse_x = x;
    jw.mouse_buttons = if right { 2 } else { 1 };
    let mut h = AssignHost {
        pp, sp, inv, soldiers: w.soldiers.as_mut_ptr(), faces, store, world: &mut jw,
        events: VecDeque::from(vec![(x, y, 0); 1 + right as usize]), log: Vec::new(), loader: 0, quote: None, answer,
        show_info: true, info_under: None, info_mouse: false, info_shot: None,
        opt_info: crate::ctlpanel::get(w, 6) != 0, opt_sub: crate::ctlpanel::subtitles(w), extra: None,
    };
    let jh = ja1_host(&mut h);
    // SAFETY: jw points into `w`, which stays in place for the call; h outlives the call.
    unsafe {
        match press_way(x, right) {
            PressWay::KeyE => ja1_assign_key_e(h.world, &jh, k as i32),
            PressWay::Right => {
                ja1_assign_right_press(h.world, &jh, k as i32);
            }
            PressWay::Left => {
                ja1_assign_item_press(h.world, &jh, k as i32, armour);
            }
        }
    }
    let mut under = h.info_under.take();
    if let Some(shot) = h.info_shot.take() {
        // #20 E: the picture stays until the next button down; the screen the press drew after it comes back then.
        under = Some(h.sp.buf.to_vec());
        h.sp.buf.copy_from_slice(&shot);
        eprintln!("assign item picture");
    }
    let (store, quote, extra, info_mouse) = (h.store, h.quote, h.extra, h.info_mouse);
    jw.store(w);
    if let Some(q) = quote {
        w.last_quote = q;
    }
    (store, under, extra, info_mouse)
}

/// The discard question asks first: an item on the cursor, no ctrl, and the first item place at (x, y) is 13
/// (the body, step 1, lies elsewhere).
pub fn asks_discard(cursor_type: u8, ctrl: bool, (x, y): (i32, i32)) -> bool {
    cursor_type != 0 && !ctrl
        && ITEM_PLACES.iter().position(|&(px, py)| px - 4 <= x && x <= px + 45 && py - 4 <= y && y <= py + 19)
            == Some(13)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Window press ways: x 0 is a left press, only a negative x is key E. Ledger row c2aea50:ja1front:5722.
    #[test]
    fn press_at_x_0_is_a_left_press() {
        assert_eq!(press_way(0, false), PressWay::Left);
        assert_eq!(press_way(-1, false), PressWay::KeyE);
        assert_eq!(press_way(0, true), PressWay::Right);
    }

    /// The discard question: an item on the cursor, no Ctrl, the press inside place 13 (159, 115) grown by 4 left/up,
    /// 45 right and 19 down.
    #[test]
    fn discard_asks_only_on_place_13() {
        assert!(asks_discard(1, false, (159, 115)));
        assert!(asks_discard(1, false, (155, 111)));
        assert!(asks_discard(1, false, (204, 134)));
        assert!(!asks_discard(1, false, (205, 115)));
        assert!(!asks_discard(1, false, (159, 135)));
        assert!(!asks_discard(1, true, (159, 115)));
        assert!(!asks_discard(0, false, (159, 115)));
        assert!(!asks_discard(1, false, (108, 73)));
    }
}
