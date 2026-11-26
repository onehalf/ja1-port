//! panel.md "The mouse pointer": the pointer shapes of the panel sheet, made into pictures for the window.

use crate::invdraw::{picture, InvData};
use crate::panel::{element, PanelData, Painter};

/// One pointer shape: w x h palette indexes (0 = not drawn), hot spot (hx, hy) inside it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub w: i32,
    pub h: i32,
    pub hx: i32,
    pub hy: i32,
    pub pix: Vec<u8>,
}

/// The hold of a pressed panel button or field box (PressPanelButton? 0x35D44, PressedFieldBox? 0x35ED4). MouseSavePos?
/// 0x6742C keeps the mouse at the press; MouseQueueNext? 0x6782C reads the moves until the left button is up; then
/// MouseRestorePos? 0x67444 puts the mouse back with MouseSetPos? 0x681D0. So a move during the hold does not count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonHold {
    saved: (i32, i32),
}

/// The press of the running button hold: x << 16 | y, or -1. The game draws its pointer only at a poll, and the hold polls
/// nothing, so the original's pointer stands still; frame::hold_still keeps the window's system pointer at this place.
pub static HOLD_AT: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(-1);

impl ButtonHold {
    pub fn press(mouse: (i32, i32)) -> Self {
        HOLD_AT.store((mouse.0 as i64) << 16 | (mouse.1 as i64 & 0xFFFF), std::sync::atomic::Ordering::Relaxed);
        ButtonHold { saved: mouse }
    }

    /// One read of the mouse queue. Some(place) = the left button is up: the mouse goes to that place.
    pub fn step(&self, buttons: u8) -> Option<(i32, i32)> {
        if buttons & 1 != 0 {
            None
        } else {
            HOLD_AT.store(-1, std::sync::atomic::Ordering::Relaxed);
            Some(self.saved)
        }
    }
}

/// ShowMouse 0x673AC + MousePoll 0x678E8: with byte 0x92E4F = 0 (`flag`: only the plain tactical screen; LoadTacticalSector
/// clears it, the boxes, the inventory, the Control Panel and a held strip button set it) the game draws no pointer on
/// the top 16 rows, measured at the pointer's top (mouse y + hot spot y 0x229C48).
pub fn top_row_hidden(y: i32, hot_y: i32, flag: u8) -> bool {
    y.wrapping_add(hot_y) < 16 && flag == 0
}

/// TacticalKeyHandler 0x143AD: when the mouse leaves the map, ShowMouse runs unless the view has no panel (mode 3 with no
/// small panel). So in that view rows 196..199 show no pointer. Live in the original, issue #31.
pub fn leave_hides(mode: u8, small: bool) -> bool {
    mode >= 3 && !small
}

/// The off byte of that rule after one pass, live in the original (issue #31): `shown` = something ran ShowMouse (a box, the
/// stat popups, a left press), and a change of the view sets the byte again from the rule of the new view.
pub fn left_step(left: bool, last: (u8, bool), now: (u8, bool), off_map: bool, shown: bool) -> bool {
    if shown {
        false
    } else if now != last {
        leave_hides(now.0, now.1) && off_map
    } else {
        left
    }
}

/// HideMouse? 0x67368: the off byte 0x284F0B after the call. With the mouse on the map (over_map 0x284F13) it does nothing.
pub fn hide_mouse(off: bool, over_map: u8) -> bool {
    off || over_map == 0
}

/// HideMouse? 0x67368 for a held item: the item goes away only with the mouse off the map (over_map 0x284F13 = 0).
pub fn held_gone(over_map: u8) -> bool {
    over_map == 0
}

/// A held item in frame `n` of SpeechBalloon? 0x29544 (0..3 the face comes in, 4 the wait, 5.. the face goes out). HideMouse?
/// 0x29589 takes the item away only with the mouse off the map, ShowMouse? 0x283A8 of the wait brings it back, and
/// HidePointer? 0x29765 before the face goes out takes it away also over the map.
pub fn held_in_balloon_frame(n: usize, over_map: u8) -> bool {
    n == 4 || (n < 4 && !held_gone(over_map))
}

/// MouseRedraw? 0x67398, the end of a HideMouse? hold: the off byte 0x284F0B after the call. It calls ShowMouse? 0x673AC
/// only when the mouse is off the map and the pointer is off, and ShowMouse? does nothing while the tactical screen is off
/// (`ready`, 0x92DCC). The top-row rule of ShowMouse? also clears the byte, see `top_row_hidden`.
pub fn mouse_redraw(off: bool, over_map: u8, ready: bool) -> bool {
    off && (over_map != 0 || !ready)
}

/// MousePoll 0x678E8: the shape after a poll. A button down (left or right): 0 -> 200, 202 -> 203; none down: 200 -> 0,
/// 203 -> 202. Other shapes stay (201 and the held items).
pub fn poll_shape(cur: i32, buttons: u8) -> i32 {
    match (buttons & 3 != 0, cur) {
        (true, 0) => 200,
        (true, 202) => 203,
        (false, 200) => 0,
        (false, 203) => 202,
        _ => cur,
    }
}

/// The panel element of shape s (0 arrow, 200 button held, 201..203), None for other shapes.
pub fn shape_element(s: i32) -> Option<usize> {
    match s {
        0 => Some(78),
        200 => Some(62),
        201 => Some(63),
        202 => Some(97),
        203 => Some(98),
        _ => None,
    }
}

/// Shape s from the panel sheet: the element's pixels, colours 0 and 0x17 not drawn; hot spot (3, 0), for
/// shapes 202 and 203 (2, 0).
pub fn shape(d: &PanelData, s: i32) -> Option<Shape> {
    let (sx, sy, w, h) = element(shape_element(s)?);
    let mut pix = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            let p = d.sheet[((sy + y) * 320 + sx + x) as usize];
            pix.push(if p == 0x17 { 0 } else { p });
        }
    }
    let hx = if s >= 202 { 2 } else { 3 };
    Some(Shape { w, h, hx, hy: 0, pix })
}

/// The window's pointer set (shapes 0, 200, 202, 203, 201 in that order): the one to show for pointer base `base`
/// with SDL's button mask `buttons` held.
pub fn window_index(base: i32, buttons: u32) -> usize {
    // SDL's button mask: bit 0 left, bit 1 middle, bit 2 right; MousePoll 0x678E8 reads only left and right.
    let dos = (buttons & 1 != 0) as u8 | ((buttons & 4 != 0) as u8) << 1;
    match poll_shape(base, dos) {
        201 => 4,
        200 => 1,
        202 => 2,
        203 => 3,
        _ => 0,
    }
}

/// The size in pixels of `rgba(sh, _, scale)`.
pub fn rgba_size(sh: &Shape, scale: i32) -> (u32, u32) {
    ((sh.w * scale) as u32, (sh.h * scale) as u32)
}

/// The shape as RGBA bytes (alpha 0 where not drawn), each pixel `scale` x `scale`; `pal` = 8-bit RGB palette.
pub fn rgba(sh: &Shape, pal: &[u8; 768], scale: i32) -> Vec<u8> {
    let (w, h) = rgba_size(sh, scale);
    let (w, h) = (w as i32, h as i32);
    let mut out = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let p = sh.pix[((y / scale) * sh.w + x / scale) as usize] as usize;
            if p != 0 {
                let o = ((y * w + x) * 4) as usize;
                out[o..o + 3].copy_from_slice(&pal[3 * p..3 * p + 3]);
                out[o + 3] = 0xFF;
            }
        }
    }
    out
}

/// The pointer while an item is held (shapes 1..199 = item type): the item's picture (colour 0) and element 61 (the
/// hand) 14 pixels to the right of the shape's left edge; the shape's top-left corner goes to (mouse x - 24,
/// mouse y - 8).
/// DrawMousePointer? 0x67A28 cuts the 42 x 16 shape at the screen's edges, so no pixel comes back on the other side.
pub fn held_item(p: &mut Painter, d: &InvData, pic: u8, mx: i32, my: i32) {
    const W: i32 = 42;
    const H: i32 = 16;
    let mut shape = vec![0u8; 320 * H as usize];
    {
        let mut q = Painter { d: p.d, buf: &mut shape, table0: &mut *p.table0, table29: p.table29 };
        picture(&mut q, d, pic, 0, 0, 0);
        q.element(61, 14, 0, None);
    }
    for y in 0..H {
        for x in 0..W {
            let (cx, cy, v) = (mx - 24 + x, my - 8 + y, shape[(y * 320 + x) as usize]);
            if v != 0 && (0..320).contains(&cx) && (0..200).contains(&cy) {
                p.put(cx, cy, v);
            }
        }
    }
}

std::thread_local! {
    /// 0x229C40, 0x229C44: the one place of MouseSavePos2? 0x67E18. All callers share it: TacticalLoop, EndTurn?, the view
    /// slide 0x3F688, ConfirmDialog 0x27646, QuestionBox 0x279B1, and it stays over a sector load. A program start
    /// holds (0, 0).
    static SAVED: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((0, 0)) };
}

/// The place that MouseRestorePos2? 0x67E30 reads.
pub fn mouse_saved() -> (i32, i32) {
    SAVED.with(|s| s.get())
}

thread_local! {
    /// The game's mouse place 0x229D00 / 0x229D04: MousePoll? and MouseSetPos? write it, and no poll runs with 0x92DCC at 0 (live, #31).
    static GAME_PLACE: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((160, 100)) };
}

/// The game's mouse place: the driver's place at the last MousePoll?.
pub fn game_place() -> (i32, i32) {
    GAME_PLACE.with(|g| g.get())
}

/// A MousePoll? or MouseSetPos? at place `at`.
pub fn mouse_poll(at: (i32, i32)) {
    GAME_PLACE.with(|g| g.set(at));
}

/// MouseSavePos2? 0x67E18 at mouse place `at`; a quick restore also writes the place from the file.
pub fn mouse_save(at: (i32, i32)) {
    SAVED.with(|s| s.set(at));
}

/// The mouse ops of a turn change (EndTurn? 0x121D8 + NewTurn? 0x126C8, InterruptTurn? 0x1304C): op 146 = MouseSetPos? 0x681D0
/// to (x, y); op 147 = MouseSavePos2? 0x67E18: the place goes to `saved`; op 157 = MouseRestorePos2? 0x67E30: the mouse goes
/// to `saved`. Some(place) = the mouse moves there.
///
/// The one place 0x229C40 also serves the view slide, ConfirmDialog, QuestionBox and the item box, so op 157 can send
/// the pointer to where the last box or slide opened (#2 row 57). Port fix P19 (#71): op 157 of the turn code (NewTurn?
/// 0x126E8 / 0x1298C / 0x12A44, InterruptTurn? 0x13150) leaves the pointer where it is.
pub fn turn_mouse(op: i32, x: i32, y: i32, now: (i32, i32), saved: &mut (i32, i32)) -> Option<(i32, i32)> {
    match op {
        146 => Some((x, y)),
        147 => {
            *saved = now;
            None
        }
        157 if ja1logic::fanfix::port_on(19) => None,
        157 => Some(*saved),
        _ => None,
    }
}

/// The picture id of held item type `item` (SetMouseShape? 0x674B0 reads the type table's byte +4).
pub fn held_picture(item_types: &[u8], item: usize) -> u8 {
    item_types.get(12 * item + 4).copied().unwrap_or(0)
}

thread_local! {
    /// Held item type that lost its pointer picture (0 = none), see `speech_arrow`.
    static ARROW_FOR: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

/// SantinoSpeech? 0x5F519, GuideSpeech? 0x5F611 and BrendaSpeech? 0x5F7C7 call SetMouseShape?(0), and nothing sets the shape back:
/// the item stays on the cursor (`held`), but the pointer is the arrow until the next SetMouseShape?(g_cursorItem) at a move of
/// the item (UseInventoryItem?, StripClick, CursorItemBack?, CursorItemToSoldier?). stderr `pointer arrow over held N`.
pub fn speech_arrow(held: u8) {
    if held != 0 {
        ARROW_FOR.with(|v| v.set(held));
        eprintln!("pointer arrow over held {}", held);
    }
}

/// StripClick 0x54F3C: each left press on the strip by a merc in play with hp >= 15 runs SetMouseShape?(g_cursorItem), also when
/// the box refuses the item and the item stays the same (live, session 1105-1108). The arrow of `speech_arrow` ends.
pub fn shape_from_held() {
    ARROW_FOR.with(|v| v.set(0));
}

/// True when the pointer shows the held item `held`. A change of `held` ends the arrow of `speech_arrow`.
/// cursor.md: the system pointer of the tactical screen shows when the mouse is off the map (`over_map` false), no held
/// item is the pointer, and no mouse hold runs in combat.
pub fn tactical_pointer_shows(over_map: bool, held: u8, mouse_hold: bool, combat: u8) -> bool {
    !over_map && !held_is_pointer(held) && !(mouse_hold && combat != 0)
}

/// #20 D: on the island map and the assignment screen the system arrow shows when no held item is drawn as the
/// pointer, the pointer is not flipped off (`ap` -1) and the mouse is not over the map where the pointer goes.
pub fn map_pointer_shows(item_pointer: bool, ap: Option<i32>, over: bool) -> bool {
    !item_pointer && ap != Some(-1) && !over
}

pub fn held_is_pointer(held: u8) -> bool {
    ARROW_FOR.with(|v| {
        if v.get() != held {
            v.set(0);
        }
        held != 0 && v.get() == 0
    })
}

/// The held item (the world's cursor item, `held`) at mouse (mx, my) over screen `buf` (320 x 200), as
/// `held_item`; nothing when the cursor is empty. True when it drew (the window's system pointer is off then).
pub fn held_on_screen(iso: &str, exe: &str, w: &crate::world::World, buf: &mut [u8], mx: i32, my: i32) -> Result<bool, i32> {
    if !held_is_pointer(w.held[0]) {
        return Ok(false);
    }
    let (c, d, inv) = (crate::common(iso)?, crate::panel::panel_data(iso, exe)?, crate::invdraw::inv_data(iso, exe)?);
    let pic = held_picture(&w.item_types, w.held[0] as usize);
    let mut table0 = c.remap[0];
    let mut p = Painter { d: &d, buf, table0: &mut table0, table29: crate::panel::table29(&c) };
    held_item(&mut p, &inv, pic, mx, my);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #71 P19 (#2 row 57): at the turn change (op 157) the pointer goes to the place of the last save (here a box at
    /// (200, 150)) in 1.13b; with the fix it stays where it is. The save (op 147) and the set (op 146) stay.
    #[test]
    fn the_turn_change_keeps_the_pointer_with_p19() {
        let _lock = crate::sound::TEST_FIX_MASK.lock().unwrap_or_else(|e| e.into_inner());
        let mask = ja1logic::fanfix::port_mask();
        for (m, want) in [(0u64, Some((200, 150))), (1 << 18, None)] {
            ja1logic::fanfix::port_set(m);
            let mut saved = (0, 0);
            assert_eq!(turn_mouse(147, 0, 0, (200, 150), &mut saved), None);
            assert_eq!(turn_mouse(157, 0, 0, (40, 30), &mut saved), want);
            assert_eq!(turn_mouse(146, 80, 8, (40, 30), &mut saved), Some((80, 8)));
            assert_eq!(saved, (200, 150));
        }
        ja1logic::fanfix::port_set(mask);
    }

    /// Ledger row c2aea50:ja1front:1611: on the map screens the arrow goes for a held item, a flipped pointer or the
    /// mouse over the map.
    #[test]
    fn the_map_arrow_goes_for_each_reason() {
        assert!(map_pointer_shows(false, None, false));
        assert!(map_pointer_shows(false, Some(201), false));
        assert!(!map_pointer_shows(true, None, false));
        assert!(!map_pointer_shows(false, Some(-1), false));
        assert!(!map_pointer_shows(false, None, true));
    }

    /// Ledger row c2aea50:ja1front:1275 (cursor.md): on the tactical screen the system pointer shows off the map, not
    /// while a held item is the pointer, and not under a mouse hold in combat.
    #[test]
    fn the_tactical_pointer_shows_off_the_map_with_no_held_item() {
        assert!(tactical_pointer_shows(false, 0, false, 0));
        assert!(!tactical_pointer_shows(true, 0, false, 0));
        assert!(!tactical_pointer_shows(false, 5, false, 0));
        assert!(!tactical_pointer_shows(false, 0, true, 1));
        assert!(tactical_pointer_shows(false, 0, true, 0));
    }

    /// Ledger row c2aea50:ja1front:869: the window shows the arrow (0) with no button, the held shape 200 under
    /// a left or right press (MousePoll 0x678E8; the middle button does not count), 202 / 203 in the office.
    #[test]
    fn the_window_pointer_follows_the_buttons() {
        assert_eq!(window_index(0, 0), 0);
        assert_eq!(window_index(0, 1), 1);
        assert_eq!(window_index(0, 4), 1);
        assert_eq!(window_index(0, 2), 0);
        assert_eq!(window_index(202, 0), 2);
        assert_eq!(window_index(202, 1), 3);
        assert_eq!(window_index(201, 1), 4);
    }

    /// Ledger row c2aea50:ja1front:12568 (the question box's item picture): item type records are 12 bytes, the
    /// picture is byte +4.
    #[test]
    fn the_held_picture_is_byte_4_of_the_item_record() {
        let t: Vec<u8> = (0..120u8).collect();
        assert_eq!(held_picture(&t, 3), 40);
        assert_eq!(held_picture(&t, 0), 4);
        assert_eq!(held_picture(&t, 50), 0);
    }

    /// Ledger rows c2aea50:ja1front:887, 19088, 19093: each shape pixel becomes a scale x scale block of its
    /// palette colour; colour 0 stays clear.
    #[test]
    fn the_window_pointer_is_the_shape_scaled() {
        let sh = Shape { w: 2, h: 3, hx: 0, hy: 0, pix: vec![1, 0, 0, 2, 2, 1] };
        let mut pal = [0u8; 768];
        pal[3..6].copy_from_slice(&[10, 20, 30]);
        pal[6..9].copy_from_slice(&[40, 50, 60]);
        assert_eq!(rgba_size(&sh, 2), (4, 6));
        assert_eq!(rgba_size(&sh, 3), (6, 9));
        let px = rgba(&sh, &pal, 2);
        assert_eq!(px.len(), 4 * 6 * 4);
        let at = |x: usize, y: usize| &px[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4];
        assert_eq!(at(0, 0), &[10, 20, 30, 255]);
        assert_eq!(at(1, 1), &[10, 20, 30, 255]);
        assert_eq!(at(2, 0), &[0, 0, 0, 0]);
        assert_eq!(at(3, 3), &[40, 50, 60, 255]);
        assert_eq!(at(0, 4), &[40, 50, 60, 255]);
        assert_eq!(at(3, 5), &[10, 20, 30, 255]);
    }

    #[test]
    fn speech_arrow_stays_until_the_item_changes() {
        assert!(!held_is_pointer(0));
        assert!(held_is_pointer(5));
        speech_arrow(5);
        assert!(!held_is_pointer(5));
        assert!(!held_is_pointer(5));
        assert!(held_is_pointer(7));
        speech_arrow(7);
        assert!(!held_is_pointer(7));
        shape_from_held();
        assert!(held_is_pointer(7));
        speech_arrow(7);
        assert!(!held_is_pointer(0));
        assert!(held_is_pointer(7));
        speech_arrow(7);
        assert!(!held_is_pointer(7));
        shape_from_held();
        assert!(held_is_pointer(7));
    }

    #[test]
    fn no_pointer_below_the_map_in_the_view_with_no_panel() {
        use super::{leave_hides, left_step};
        assert!(leave_hides(3, false));
        for (m, small) in [(3, true), (2, false), (1, false), (0, false)] {
            assert!(!leave_hides(m, small));
        }
        // The rows of the live run: mode 2 -> 3 off the map hides, 3 -> 2 and 3 -> 0 show, a press or a box shows.
        assert!(left_step(false, (2, false), (3, false), true, false));
        assert!(!left_step(false, (2, false), (3, false), false, false));
        assert!(!left_step(true, (3, false), (2, false), true, false));
        assert!(!left_step(true, (3, false), (0, false), true, false));
        assert!(!left_step(true, (3, false), (3, false), true, true));
        assert!(left_step(true, (3, false), (3, false), true, false));
        assert!(!left_step(false, (3, false), (3, false), true, false));
    }


    #[test]
    fn held_item_in_balloon_frames() {
        let on: Vec<usize> = (0..9).filter(|&n| held_in_balloon_frame(n, 1)).collect();
        assert_eq!(on, [0, 1, 2, 3, 4]);
        let off: Vec<usize> = (0..9).filter(|&n| held_in_balloon_frame(n, 0)).collect();
        assert_eq!(off, [4]);
    }
}
