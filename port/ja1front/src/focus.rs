//! Host op 99 focus (slot, flag): the view goes to a soldier (world_load.md section 8 "Focus").
use crate::host::Ja1World;
use std::cell::RefCell;

/// One view slide of step 3 (world_load.md section 8 "Focus", "The slide in time"): the view went from `from` toward
/// `target` in `steps` steps (it ends on the target only when 16 / 14 pixel steps reach it). The window shows the
/// steps after the pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slide {
    pub from: (i32, i32),
    pub target: (i32, i32),
    pub steps: u32,
}

// One list per thread: only the game thread makes and takes slides (each unit test has its own list).
std::thread_local! {
    static SLIDES: RefCell<Vec<Slide>> = const { RefCell::new(Vec::new()) };
}

/// The number of slides that wait for the window.
pub fn slide_count() -> usize {
    SLIDES.with(|l| l.borrow().len())
}

/// The last waiting slide, when the list is longer than `n`.
pub fn last_slide(n: usize) -> Option<Slide> {
    SLIDES.with(|l| {
        let l = l.borrow();
        if l.len() > n { l.last().copied() } else { None }
    })
}

/// Puts a held slide back in the list.
pub fn push_slide(s: Slide) {
    SLIDES.with(|l| l.borrow_mut().push(s));
}

/// Takes out one waiting slide that a balloon holds.
pub fn hold_slide(s: &Slide) {
    SLIDES.with(|l| {
        let mut l = l.borrow_mut();
        if let Some(i) = l.iter().position(|o| o == s) {
            l.remove(i);
        }
    });
}

/// The slides since the last call, in their order.
pub fn take_slides() -> Vec<Slide> {
    SLIDES.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

impl Slide {
    /// Timer ticks that the slide takes: ripple_timer = 20 before the slide, each step waits until ripple_timer
    /// ≤ 0 and then sets it to 50 (10 per tick): 2 ticks for the first step, 5 for each later one.
    pub fn ticks(&self) -> u32 {
        2 + 5 * self.steps.saturating_sub(1)
    }

    /// The view after step i (1 ..= steps).
    pub fn at(&self, i: u32) -> (i32, i32) {
        let (mut x, mut y) = self.from;
        for _ in 0..i {
            if (x, y) == self.target {
                break;
            }
            x += 16 * (self.target.0 - x).signum();
            y += 14 * (self.target.1 - y).signum();
        }
        (x, y)
    }
}

/// The world values that the focus reads and writes.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub view_x: i32,
    pub view_y: i32,
    pub view_w: i32,
    pub view_h: i32,
    pub opt_onscreen: i32,
    pub follow_all: i32,
    pub focus_slot: i32,
    pub shown_slot: i32,
    pub show_timer: i32,
}

/// Step 2: the new view for tile pixel point (x, y).
fn target(v: &View, x: i32, y: i32) -> (i32, i32) {
    let half = if v.opt_onscreen < 2 { 0x70 } else { 0xA0 };
    let nx = x.wrapping_sub(half - 16);
    let mut ny = y.wrapping_sub(v.view_h / 2 - 14);
    while ny % 14 != 0 {
        ny -= 1;
    }
    let mx = 0x380i32.wrapping_sub(v.view_w);
    let my = 0x310i32.wrapping_sub(v.view_h);
    let nx = if nx < 0 { 0 } else if nx > mx { mx } else { nx };
    let ny = if ny < 0 { 0 } else if ny > my { my } else { ny };
    (nx, ny)
}

/// SlideViewToTile? 0x3F578 on tile `t`: with the view mode on (opt_onscreen != 0) and the tile's pixel point on or
/// past a view edge, the view goes to the target in 16 / 14 pixel steps (at most 51). Result: the steps (0: no slide)
/// and the target.
fn slide(v: &mut View, t: i32) -> (u32, (i32, i32)) {
    let mut steps = 0;
    let mut goal = (0, 0);
    let (x, y) = ((t % 56) * 16, (t / 56) * 14);
    let (vx, vy, r, b) = (v.view_x, v.view_y, v.view_x.wrapping_add(v.view_w), v.view_y.wrapping_add(v.view_h));
    if v.opt_onscreen as u8 != 0 && (x <= vx || x >= r || y <= vy || y >= b) {
        let (nx, ny) = target(v, x, y);
        goal = (nx, ny);
        // The library moves the view at once; the window shows the steps after the pass (Slide).
        for _ in 0..51 {
            if (v.view_x, v.view_y) == (nx, ny) {
                break;
            }
            steps += 1;
            if nx > v.view_x {
                v.view_x += 16;
            } else if nx < v.view_x {
                v.view_x -= 16;
            }
            if ny > v.view_y {
                v.view_y += 14;
            } else if ny < v.view_y {
                v.view_y -= 14;
            }
        }
    }
    (steps, goal)
}

/// Steps 1..4 on record `rec` (512 bytes) of soldier `slot`. Result: the number of slide steps (0: no slide) and
/// the slide's target.
pub fn focus(v: &mut View, rec: &[u8], slot: u8, flag: i32) -> (u32, (i32, i32)) {
    let mut steps = 0;
    let mut goal = (0, 0);
    let fs = v.focus_slot as u8;
    if rec[0x182] == 0 || rec[0x183] == 0 || rec[0x138] == 0 || (rec[0x9C] as i8 <= 0 && v.follow_all as u8 == 0) {
        if slot == fs {
            v.focus_slot = 0x40;
        }
        return (0, goal);
    }
    let t = i16::from_le_bytes([rec[0x114], rec[0x115]]) as i32;
    let (x, y) = ((t % 56) * 16, (t / 56) * 14);
    let on = v.opt_onscreen as u8;
    let (vx, vy, r, b) = (v.view_x, v.view_y, v.view_x.wrapping_add(v.view_w), v.view_y.wrapping_add(v.view_h));
    if slot == fs && on != 0 {
        (steps, goal) = slide(v, t);
    } else if on != 0 && (x < vx || x >= r || y < vy || y >= b) {
        (v.view_x, v.view_y) = target(v, x, y);
    }
    v.focus_slot = 0x40;
    if flag == 0 {
        v.shown_slot = slot as i32;
        v.show_timer = 5000;
    }
    (steps, goal)
}

/// Op 99 on the library world: reads the record of `slot` (< 48) and writes the view values back.
///
/// # Safety
/// `w` is a valid world whose soldier buffer (when not null) holds 48 records.
pub unsafe fn focus_world(w: *mut Ja1World, slot: i32, flag: i32) {
    let k = slot as u8;
    if w.is_null() || (*w).soldiers.is_null() || k as usize >= 48 {
        return;
    }
    let rec = std::slice::from_raw_parts((*w).soldiers.add(k as usize * 512), 512);
    let w = &mut *w;
    let mut v = View {
        view_x: w.view_x,
        view_y: w.view_y,
        view_w: w.view_w,
        view_h: w.view_h,
        opt_onscreen: w.opt_onscreen as i32,
        follow_all: w.follow_all as i32,
        focus_slot: w.focus_slot as i32,
        shown_slot: w.shown_slot,
        show_timer: w.show_timer,
    };
    let from = (w.view_x, w.view_y);
    let (steps, target) = focus(&mut v, rec, k, flag);
    if steps > 0 {
        let s = Slide { from, target, steps };
        // SlideViewToTile? 0x3F688 saves the place at this point of the pass: a turn change behind it restores this place.
        crate::pointer::mouse_save(crate::pointer::game_place());
        push_slide(s);
    }
    w.view_x = v.view_x;
    w.view_y = v.view_y;
    w.focus_slot = v.focus_slot as u8;
    w.shown_slot = v.shown_slot;
    w.show_timer = v.show_timer;
}

/// Host op 357 slide_view (tile; only fan fix F15, EnemiesTurn? step 6): SlideViewToTile? 0x3F578 on the library world.
///
/// # Safety
/// `w` is a valid world.
pub unsafe fn slide_world(w: *mut Ja1World, tile: i32) {
    if w.is_null() {
        return;
    }
    let w = &mut *w;
    let mut v = View { view_x: w.view_x, view_y: w.view_y, view_w: w.view_w, view_h: w.view_h, opt_onscreen: w.opt_onscreen as i32, ..View::default() };
    let from = (w.view_x, w.view_y);
    let (steps, target) = slide(&mut v, tile);
    if steps > 0 {
        crate::pointer::mouse_save(crate::pointer::game_place());
        push_slide(Slide { from, target, steps });
    }
    w.view_x = v.view_x;
    w.view_y = v.view_y;
}

/// Test entry for validate/diff/focus_diff.py: `view` = the 9 values of `View` (in / out), `rec` = 512 bytes;
/// `ripple` (unused: the slide sets ripple_timer = 20 first). `slide` (out, 2 + 2 x 51 values): the slide's steps and timer ticks, then
/// the view after each step (x, y).
///
/// # Safety
/// `view` points to 9 int32 values, `rec` to 512 bytes, `slide` to 104 int32 values.
#[no_mangle]
pub unsafe extern "C" fn ja1f_focus(view: *mut View, rec: *const u8, slot: i32, flag: i32, _ripple: i32, slide: *mut i32) {
    if view.is_null() || rec.is_null() || slide.is_null() {
        return;
    }
    let from = ((*view).view_x, (*view).view_y);
    let (steps, target) = focus(&mut *view, std::slice::from_raw_parts(rec, 512), slot as u8, flag);
    let out = std::slice::from_raw_parts_mut(slide, 104);
    out.fill(0);
    if steps > 0 {
        let s = Slide { from, target, steps };
        out[0] = steps as i32;
        out[1] = s.ticks() as i32;
        for i in 1..=steps.min(51) {
            let (x, y) = s.at(i);
            out[2 * i as usize] = x;
            out[2 * i as usize + 1] = y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slide_steps_and_ticks() {
        let s = Slide { from: (672, 630), target: (272, 154), steps: 34 };
        assert_eq!(s.at(1), (656, 616));
        assert_eq!(s.at(25), (272, 280));
        assert_eq!(s.at(34), (272, 154));
        assert_eq!(s.ticks(), 2 + 5 * 33);
    }
}

/// soldier_update.md "Merc sighting", Focus step for how = 3: the view jumps to `tile` (world_load.md "Focus" step
/// 2) when opt_onscreen ≠ 0 and the tile's pixel point is outside the view (flag ≠ 0: always).
pub fn jump_tile(v: &mut View, tile: i32, flag: i32) {
    if v.opt_onscreen as u8 == 0 {
        return;
    }
    let (x, y) = ((tile % 56) * 16, (tile / 56) * 14);
    let inside = x >= v.view_x && x < v.view_x.wrapping_add(v.view_w) && y >= v.view_y && y < v.view_y.wrapping_add(v.view_h);
    if flag == 0 && inside {
        return;
    }
    (v.view_x, v.view_y) = target(v, x, y);
}
