//! soldier_update.md "Merc sighting": host op 184 merc_sees, the reaction at the loop head, and Merc message (also
//! host op 54 merc_message).
use crate::host::{Ja1Host, Ja1World};

extern "C" {
    fn ja1_time_compress(w: *mut Ja1World, h: *const Ja1Host, on: i32);
    fn ja1_hide_path_marks(w: *mut Ja1World, h: *const Ja1Host, force: i32);
}

/// Byte size of one `fx_anims` entry and the number of entries.
const FX_ENTRY: usize = 0x12;
const FX_N: usize = 16;

/// The three sighting bytes (sight_slot, sight_how, sight_near).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sight {
    pub slot: u8,
    pub how: i8,
    pub near: u8,
}

impl Sight {
    /// 0x229E00..2 as they stand in QUICK.SAV.
    pub fn bytes(&self) -> [u8; 3] {
        [self.slot, self.how as u8, self.near]
    }
    pub fn from_bytes(b: [u8; 3]) -> Sight {
        Sight { slot: b[0], how: b[1] as i8, near: b[2] }
    }
}

impl Default for Sight {
    /// The Sighting reset values (FUN_2F7A4 0x2F7A4 writes slot/how/near = 0x40/0x7F/0x40 and fx_id = 0xFF).
    fn default() -> Sight {
        Sight { slot: 0x40, how: 0x7F, near: 0x40 }
    }
}

/// What a Merc message did, for the script's `sees` line and the tests.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reaction {
    pub slot: u8,
    pub how: i8,
    pub text: String,
    /// The time compression went off.
    pub stopped: bool,
    /// The soldier slot or tile (how = 3) the view went to.
    pub focus: Option<i32>,
    /// The speech balloon (panel.md "Speech balloon", quote = how): the soldier record when it shows.
    pub balloon: Option<Vec<u8>>,
    /// The native guide's sector line (host op 168): the balloon's quote is NATQUOTE record `how` (the sector).
    pub nat: bool,
    /// The view move of a message with a balloon, for the window. 0x299B8 moves the view after its wait, inside the call:
    /// the game state moves at once here too, and the window draws the balloon over `before`.
    pub undo: Option<ViewUndo>,
    /// The path marks (tile, byte 7) when a balloon shows: 0x299B8 hides them after its wait, the state goes at once here.
    pub marks: Vec<(usize, u8)>,
}

/// The values that the view move of a message writes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewBytes {
    pub view_x: i32,
    pub view_y: i32,
    pub show_timer: i32,
    pub view_tile: i32,
    pub spotted: i32,
    pub focus_slot: u8,
    pub focus_last: u8,
    pub shown_slot: i32,
}

/// See `Reaction::undo`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewUndo {
    pub before: ViewBytes,
    pub after: ViewBytes,
    /// The slide of the view move: the window plays it after the balloon.
    pub slide: Option<crate::focus::Slide>,
    /// A right click on the balloon skips the view move (byte 0x92F65; not for the sector line).
    pub right_skips: bool,
}

unsafe fn view_bytes(w: *const Ja1World) -> ViewBytes {
    let w = &*w;
    ViewBytes { view_x: w.view_x, view_y: w.view_y, show_timer: w.show_timer, view_tile: w.view_tile, spotted: w.spotted,
                focus_slot: w.focus_slot, focus_last: w.focus_last, shown_slot: w.shown_slot }
}

/// Runs `m` at once and returns its `ViewUndo`.
unsafe fn moved_view(w: *mut Ja1World, m: ViewMove) -> ViewUndo {
    let before = view_bytes(w);
    let n = crate::focus::slide_count();
    run_view_move(w, m);
    ViewUndo { before, after: view_bytes(w), slide: crate::focus::last_slide(n), right_skips: m.right_skips() }
}

/// The tail of ShowMercMessage? 0x299B8 and SectorLine? 0x297D4 (see `Reaction::undo`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMove {
    /// how = 3: the view jumps to this tile.
    Tile(i32),
    /// `near` is a soldier that the player sees: the view goes to it.
    Near(i32),
    /// The view goes to the merc's own slot.
    Own(i32),
    /// The sector line: the view goes to the merc's own slot. A right click does not skip it.
    Line(i32),
}

impl ViewMove {
    /// True when a right click on the balloon skips this view move.
    pub fn right_skips(self) -> bool {
        !matches!(self, ViewMove::Line(_))
    }
}

/// Runs the view move of a message (see `ViewMove`).
///
/// # Safety
/// `w` is a valid world whose soldier buffer holds 48 records.
unsafe fn run_view_move(w: *mut Ja1World, m: ViewMove) {
    match m {
        ViewMove::Tile(near) => {
            (*w).show_timer = 5000;
            (*w).view_tile = near;
            let mut v = view(&*w);
            crate::focus::jump_tile(&mut v, near, 0);
            (*w).view_x = v.view_x;
            (*w).view_y = v.view_y;
        }
        ViewMove::Near(near) => {
            (*w).show_timer = 5000;
            (*w).spotted = near;
            (*w).focus_slot = near as u8;
            (*w).focus_last = near as u8;
            crate::focus::focus_world(w, near, 0);
        }
        ViewMove::Own(me) => crate::focus::focus_world(w, me, 0),
        ViewMove::Line(me) => {
            crate::focus::focus_world(w, me, 0);
            (*w).show_timer = 5000;
        }
    }
}

/// Host op 184 merc_sees (slot, how, near): keeps the low bytes.
pub fn merc_sees(s: &mut Sight, a: &[i32]) {
    let g = |i: usize| a.get(i).copied().unwrap_or(0);
    *s = Sight { slot: g(0) as u8, how: g(1) as u8 as i8, near: g(2) as u8 };
}

/// Hand gun state (shooting.md) of record `r`: main-hand type 1..0x10: 1 if s8 +2 >= 10 else -1; else 0.
fn hand_gun_state(r: &[u8]) -> i32 {
    if (1..=0x10).contains(&r[0]) {
        if r[2] as i8 >= 10 { 1 } else { -1 }
    } else {
        0
    }
}

/// The loop head part (FUN_56D3C 0x56D3C): when a sighting waits (slot < 0x30) and no impact effect runs, the
/// merc reacts (WeaponStatusText 0x56F4C picks the text), then the Sighting reset. Returns the reaction (None: nothing waits, or it still waits for the effect).
///
/// # Safety
/// `w` is a valid world whose soldier buffer (when not null) holds 48 records and `fx_anims` (when not null) 16
/// entries; `h` is its host.
pub unsafe fn loop_head(s: &mut Sight, w: *mut Ja1World, h: *const Ja1Host, msg_box: &mut Option<(String, u32)>,
                        cry: &mut Option<u8>) -> Option<Reaction> {
    loop_head_with(s, w, |slot, how, near, text| merc_message(w, h, slot, how, near, text, msg_box, cry))
}

/// `loop_head` with the Merc message call given as `msg` (slot, how, near, text): the decision part of 0x56D3C.
///
/// # Safety
/// As in `loop_head`.
pub unsafe fn loop_head_with(s: &mut Sight, w: *mut Ja1World,
                             msg: impl FnOnce(u8, i8, i32, &str) -> Option<Reaction>) -> Option<Reaction> {
    if s.slot >= 0x30 || w.is_null() || (*w).soldiers.is_null() {
        return None;
    }
    let fx = (*w).fx_id as i8;
    if fx >= 0 && (fx as usize) < FX_N && !(*w).fx_anims.is_null() && *(*w).fx_anims.add(fx as usize * FX_ENTRY) as i8 > 0 {
        return None;
    }
    let r = std::slice::from_raw_parts((*w).soldiers.add(s.slot as usize * 512), 512);
    let g = hand_gun_state(r);
    let text = if g != 0 && (r[1] as i8 > 0 || s.how == 5) { "" } else { weapon_text(r) };
    let out = msg(s.slot, s.how, s.near as i32, text);
    *s = Sight::default();
    (*w).fx_id = 0xFF;
    out
}

/// WeaponStatusText 0x56F4C: the text of the main hand of record `r`.
pub fn weapon_text(r: &[u8]) -> &'static str {
    match hand_gun_state(r) {
        1 if (r[1] as i8) < 0 => "JAMMED",
        1 => "NO AMMO",
        -1 => "ITEM BROKEN",
        _ => "NO WEAPON",
    }
}

/// Merc message (R = record `slot`, how, near, text). None when R +0x17A > 7 or (how >= 0) the radio test fails.
/// `cry` gets R's profile when the merc's "BAD" cry with sample rate 0x2B11 must play (how < 0).
///
/// # Safety
/// As in `loop_head`; `slot` < 48.
pub unsafe fn merc_message(w: *mut Ja1World, h: *const Ja1Host, slot: u8, how: i8, near: i32, text: &str,
                           msg_box: &mut Option<(String, u32)>, cry: &mut Option<u8>) -> Option<Reaction> {
    if w.is_null() || (*w).soldiers.is_null() || slot as usize >= 48 {
        return None;
    }
    let sp = (*w).soldiers;
    let rec = |k: usize| std::slice::from_raw_parts(sp.add(k * 512), 512);
    let r = rec(slot as usize);
    let me = r[0x17A];
    if me > 7 {
        return None;
    }
    let mut balloon = None;
    if how < 0 {
        ja1_time_compress(w, h, 0);
        *cry = Some(r[0x179]);
        if (*w).opt_step_text != 0 {
            *msg_box = Some(("Enemy spotted!".to_string(), 700));
        }
    } else {
        if r[0x30] == 0 || (r[0x32] as i8) < 10 {
            return None;
        }
        ja1_time_compress(w, h, 0);
        let hp = r[0x183] as i8;
        if hp >= 10 && (hp >= 15 || how == 9) {
            balloon = Some(r.to_vec());
        }
        (*w).show_timer = 5000;
        (*w).shown_slot = me as i32;
    }
    let (m, focus) = if how == 3 {
        (ViewMove::Tile(near), near)
    } else if (0..0x40).contains(&near) && (rec(near as usize)[0x9C] as i8 > 0 || (*w).follow_all != 0) {
        (ViewMove::Near(near), near)
    } else {
        (ViewMove::Own(me as i32), me as i32)
    };
    let mut out = Reaction { slot, how, text: text.to_string(), stopped: true, focus: Some(focus), balloon, nat: false, ..Default::default() };
    let undo = moved_view(w, m);
    if out.balloon.is_some() {
        out.undo = Some(undo);
    }
    if out.balloon.is_some() {
        out.marks = mark_tiles(w);
    }
    ja1_hide_path_marks(w, std::ptr::null(), ((*w).path_preview != 0) as i32);
    Some(out)
}

/// The tiles with a path mark (tile, byte 7) of the shown marks, as the hide step clears them.
unsafe fn mark_tiles(w: *const Ja1World) -> Vec<(usize, u8)> {
    if (*w).marks_shown == 0 || (*w).opt_path_marks == 0 || (*w).marks.is_null() || (*w).tiles.is_null() {
        return Vec::new();
    }
    (0..(*w).marks_n.clamp(0, 128) as usize)
        .filter_map(|i| {
            let t = i16::from_le_bytes([*(*w).marks.add(2 * i), *(*w).marks.add(2 * i + 1)]) as i32;
            (0..3136).contains(&t).then(|| (t as usize, *(*w).tiles.add(t as usize * 8 + 7)))
        })
        .collect()
}

/// Puts `marks` into the world's tiles for one picture (`on`), or takes them out.
pub fn show_marks(world: &mut crate::world::World, marks: &[(usize, u8)], on: bool) {
    for &(t, v) in marks {
        if let Some(b) = world.tiles.get_mut(t * 8 + 7) {
            *b = if on { v } else { 0 };
        }
    }
}

/// Host op 167 speech_ready (CountLiveMercs? 0x410C4): the number of the 8 merc records with profile +0x179 < 0x3C,
/// +0x182 != 0 and s8 +0x183 > 0.
///
/// # Safety
/// `w` is a valid world whose soldier buffer (when not null) holds 48 records.
pub unsafe fn live_mercs(w: *const Ja1World) -> i32 {
    if w.is_null() || (*w).soldiers.is_null() {
        return 0;
    }
    live_count(std::slice::from_raw_parts((*w).soldiers, 8 * 512))
}

/// The count of `live_mercs` over the 8 merc records `mercs` (8 x 512 bytes).
pub fn live_count(mercs: &[u8]) -> i32 {
    mercs.chunks(512).take(8).filter(|r| r.len() == 512 && r[0x179] < 0x3C && r[0x182] != 0 && r[0x183] as i8 > 0).count() as i32
}

/// Host op 168 sector_speech (slot, sector) = 0x297D4: when record `slot` +0x30 != 0, time compression off, the
/// balloon with NATQUOTE record `sector` (no hurt face, no text), shown_slot = +0x17A, the view on it, show_timer
/// 5000, sector_spoken[cur_sector] = 1. None when +0x30 = 0 (nothing happens).
///
/// # Safety
/// As in `loop_head`; `slot` < 48.
pub unsafe fn sector_line(w: *mut Ja1World, h: *const Ja1Host, slot: i32, sector: i32) -> Option<Reaction> {
    if w.is_null() || (*w).soldiers.is_null() || !(0..48).contains(&slot) {
        return None;
    }
    let r = std::slice::from_raw_parts((*w).soldiers.add(slot as usize * 512), 512);
    if r[0x30] == 0 {
        return None;
    }
    ja1_time_compress(w, h, 0);
    let rec = r.to_vec();
    let me = rec[0x17A];
    (*w).shown_slot = me as i32;
    let undo = moved_view(w, ViewMove::Line(me as i32));
    // SPEC GAP: a cur_sector of 60 or more writes nothing (the original writes past the 60 bytes).
    let cur = (*w).cur_sector as usize;
    if cur < 60 && !(*w).sector_spoken.is_null() {
        *(*w).sector_spoken.add(cur) = 1;
    }
    Some(Reaction { slot: slot as u8, how: sector as i8, text: String::new(), stopped: true, focus: Some(me as i32),
                    balloon: Some(rec), nat: true, undo: Some(undo), marks: Vec::new() })
}

fn view(w: &Ja1World) -> crate::focus::View {
    crate::focus::View {
        view_x: w.view_x,
        view_y: w.view_y,
        view_w: w.view_w,
        view_h: w.view_h,
        opt_onscreen: w.opt_onscreen as i32,
        follow_all: w.follow_all as i32,
        focus_slot: w.focus_slot as i32,
        shown_slot: w.shown_slot,
        show_timer: w.show_timer,
    }
}

/// TeamQuote? 0x61688 (soldier_update.md "Team quote"): all 8 slots in random order (Random(8) until an untried slot); each merc
/// in the sector, in play, with hp >= 15, byte +0x188 = 0 and within `range` of tile 0x15B calls `say`. It does not stop after
/// the first merc.
pub fn team_quote(recs: &[u8], range: i32, mut random: impl FnMut(i32) -> i32, mut say: impl FnMut(u8)) {
    let mut tried = [false; 8];
    for _ in 0..8 {
        let k = loop {
            let k = random(8) as usize & 7;
            if !tried[k] {
                break k;
            }
        };
        tried[k] = true;
        let r = &recs[k * 512..k * 512 + 512];
        let tile = i16::from_le_bytes([r[0x114], r[0x115]]) as i32;
        if r[0x182] == 0 || r[0x138] == 0 || (r[0x183] as i8) < 15 || r[0x188] != 0 || crate::play::tile_dist(tile, 0x15B) > range {
            continue;
        }
        say(k as u8);
    }
}

#[cfg(test)]
mod marks_tests {
    use super::*;

    /// Ledger row 0b4d529:ja1front:20772 (0x299B8 draws the picture under the balloon before it hides the marks): the
    /// marks go back into tile byte 7 for that picture and leave again after it.
    #[test]
    fn show_marks_puts_the_old_marks_in_for_one_picture() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = crate::world::World::from_files(&exe, &jag, &save).unwrap();
        let marks = [(100usize, 0x30u8), (101, 0x02)];
        w.tiles[100 * 8 + 7] = 0;
        w.tiles[101 * 8 + 7] = 0;
        show_marks(&mut w, &marks, true);
        assert_eq!((w.tiles[100 * 8 + 7], w.tiles[101 * 8 + 7]), (0x30, 0x02));
        show_marks(&mut w, &marks, false);
        assert_eq!((w.tiles[100 * 8 + 7], w.tiles[101 * 8 + 7]), (0, 0));
    }

    /// 0x299B8 hides the path marks after the balloon's wait; the state goes at once, and the marks stay in `Reaction::marks`.
    #[test]
    fn a_balloon_keeps_the_marks_of_the_old_picture() {
        let mut soldiers = vec![0u8; 48 * 512];
        let r = &mut soldiers[2 * 512..3 * 512];
        (r[0x17A], r[0x179], r[0x182], r[0x183], r[0x30], r[0x32]) = (2, 5, 1, 50, 1, 50);
        let mut tiles = vec![0u8; 3136 * 8];
        let mut marks = vec![0u8; 256];
        for (i, t) in [1200usize, 1201].iter().enumerate() {
            marks[2 * i..2 * i + 2].copy_from_slice(&(*t as u16).to_le_bytes());
            tiles[t * 8 + 7] = 0x10 + i as u8;
        }
        let mut w: Box<Ja1World> = unsafe { Box::new(std::mem::zeroed()) };
        w.soldiers = soldiers.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        w.marks = marks.as_mut_ptr();
        (w.marks_shown, w.marks_n, w.opt_path_marks) = (1, 2, 1);
        (w.view_w, w.view_h) = (320, 200);
        let (mut msg, mut cry) = (None, None);
        let wp: *mut Ja1World = &mut *w;
        let got = unsafe { merc_message(wp, std::ptr::null(), 2, 32, 0x40, "", &mut msg, &mut cry) }.unwrap();
        assert!(got.balloon.is_some());
        assert_eq!(got.marks, vec![(1200, 0x10), (1201, 0x11)]);
        assert_eq!((tiles[1200 * 8 + 7], tiles[1201 * 8 + 7], w.marks_shown), (0, 0, 0), "the state hides them at once");
        // A second balloon of the same pass finds no marks, as the original after the first one's end.
        let got = unsafe { merc_message(wp, std::ptr::null(), 2, 32, 0x40, "", &mut msg, &mut cry) }.unwrap();
        assert!(got.marks.is_empty());
    }

    /// The tail of ShowMercMessage? 0x299B8 moves the view after the balloon (world_load.md section 8 "Focus", "The
    /// slide in time"): the balloon holds only the slide that its own view move made. A slide that waited before the
    /// message (an earlier focus of the same pass) is not the balloon's slide.
    #[test]
    fn a_view_move_holds_only_its_own_slide() {
        let mut soldiers = vec![0u8; 48 * 512];
        // slot 3 stands at tile 40 * 56 + 50 (pixel 800, 560), outside the 320 x 200 view at (0, 0)
        let r = &mut soldiers[3 * 512..4 * 512];
        (r[0x182], r[0x183], r[0x138], r[0x9C]) = (1, 50, 1, 1);
        r[0x114..0x116].copy_from_slice(&(40u16 * 56 + 50).to_le_bytes());
        let mut w: Box<Ja1World> = unsafe { Box::new(std::mem::zeroed()) };
        w.soldiers = soldiers.as_mut_ptr();
        (w.view_w, w.view_h, w.opt_onscreen) = (320, 200, 1);
        let wp: *mut Ja1World = &mut *w;
        let _ = crate::focus::take_slides();
        // no slide waits: the move toward slot 3 makes one, and the balloon holds it
        let u = unsafe { moved_view(wp, ViewMove::Near(3)) };
        let own = u.slide.expect("the view move made a slide");
        assert_eq!(own.from, (0, 0));
        assert_eq!(crate::focus::take_slides(), vec![own]);
        // an older slide waits and the move makes none (the view is on the soldier now): the balloon holds none
        let old = crate::focus::Slide { from: (16, 14), target: (32, 28), steps: 1 };
        crate::focus::push_slide(old);
        let u = unsafe { moved_view(wp, ViewMove::Near(3)) };
        assert_eq!(u.slide, None);
        // an older slide waits and the move makes one: the balloon holds the new one
        (w.view_x, w.view_y) = (0, 0);
        let u = unsafe { moved_view(wp, ViewMove::Near(3)) };
        assert_eq!(u.slide.map(|s| s.from), Some((0, 0)));
        assert_eq!(crate::focus::take_slides().len(), 2);
    }

    #[test]
    fn a_message_with_no_balloon_keeps_no_marks() {
        let mut soldiers = vec![0u8; 48 * 512];
        let r = &mut soldiers[2 * 512..3 * 512];
        (r[0x17A], r[0x179], r[0x182], r[0x183], r[0x30], r[0x32]) = (2, 5, 1, 12, 1, 50);
        let mut tiles = vec![0u8; 3136 * 8];
        let mut marks = vec![0u8; 256];
        marks[..2].copy_from_slice(&1200u16.to_le_bytes());
        tiles[1200 * 8 + 7] = 0x10;
        let mut w: Box<Ja1World> = unsafe { Box::new(std::mem::zeroed()) };
        w.soldiers = soldiers.as_mut_ptr();
        w.tiles = tiles.as_mut_ptr();
        w.marks = marks.as_mut_ptr();
        (w.marks_shown, w.marks_n, w.opt_path_marks) = (1, 1, 1);
        (w.view_w, w.view_h) = (320, 200);
        let (mut msg, mut cry) = (None, None);
        let wp: *mut Ja1World = &mut *w;
        let got = unsafe { merc_message(wp, std::ptr::null(), 2, 3, 0x40, "", &mut msg, &mut cry) }.unwrap();
        assert!(got.balloon.is_none() && got.marks.is_empty());
        assert_eq!(tiles[1200 * 8 + 7], 0);
    }
}
