//! The mouse on the merc slots: SlotMouse 0x11714 and its frame in TacticalKeyHandler (0x1463A .. 0x14721). The state
//! is the four bytes of the original; the window runs `pass` with no button at each mouse move and each pass, and a
//! press asks `left` / `right_opens`. slotmouse_diff.py compares this with the original.
use crate::panel::{slot_origin, slot_side};

/// No slot, in the bytes 0x92DC8, 0x93448 and 0x92E58.
pub const NONE: u8 = 0x40;

/// TacticalKeyHandler 0x141EC: with 0x92DCC at 0 (the enemies' turn) the handler returns here, so the mouse on the map,
/// the mouse off the map and the merc slots all take no step. A press that ends a pause or a time compression comes before.
pub fn pass_runs(screen_drawn: u8) -> bool {
    screen_drawn != 0
}

/// TacticalKeyHandler 0x14308 + 0x143CA in the window: the mouse rest pass of the strip beside the map runs only when
/// nothing in `blocked` holds (map open, sector left, aim or confirm open, drag, switch box, stat keys, held press,
/// right press, left button down, pause) and `pass_runs`.
pub fn rest_pass_runs(blocked: [bool; 10], screen_drawn: u8) -> bool {
    blocked.iter().all(|&b| !b) && pass_runs(screen_drawn)
}

/// What a pass draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// RedrawMercSlot 0x331A4 (slot, 1); the slot can be 0x40, the original does that too.
    Redraw(u8),
    /// The hover frame, colour 0x27 (DrawFrame on the panel, inclusive corners).
    Frame(i32, i32, i32, i32),
    /// CopySlotToScreen 0x33174
    Copy(u8),
    /// "cont" at this text point.
    ContText(i32, i32),
}

/// What a button does on slot k.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    /// 0x119A6: the inventory popup (held), the game Locked meanwhile.
    Popup,
    /// 0x119DE: the selected merc goes on with its order, see `cont_plan`.
    Cont,
    /// 0x11B18: SelectMerc (k, Alt, 1).
    Select,
    /// 0x11B5E: the switch view opens.
    Switch,
}

/// The order a "cont" click gives (0x11A1A).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContPlan {
    /// MoveMercTo (dest, 1, +0x1A8, +0x177, +0x196).
    Walk { dest: i32, sneak: u8, b177: u8, b196: u8 },
    /// UseCost (here, tile, ground item at tile, task), the AP message, then TacticalClick (tile, `click_item(task)`).
    Use { here: i32, tile: i32, task: u8 },
}

/// The mouse of one pass.
#[derive(Clone, Copy, Debug, Default)]
pub struct Mouse {
    pub x: i32,
    pub y: i32,
    /// 1 left, 2 right
    pub buttons: i32,
}

/// The four bytes of the original.
#[derive(Clone, Copy, Debug)]
pub struct SlotMouse {
    /// 0x92DC8: the slot that shows "cont".
    pub cont: u8,
    /// 0x93448: the slot whose bar area has the mouse.
    pub hover: u8,
    /// 0x92E58: the slot whose rows have the mouse.
    pub last: u8,
    /// 0x284F16: the mouse is on a bar area in this pass.
    pub over: u8,
}

impl Default for SlotMouse {
    fn default() -> Self {
        SlotMouse { cont: NONE, hover: NONE, last: NONE, over: 0 }
    }
}

fn s16(r: &[u8], o: usize) -> i32 {
    i16::from_le_bytes([r[o], r[o + 1]]) as i32
}

/// SlotMouse 0x11760: a slot takes the mouse only when the merc is in the sector, not shaded and on the map.
pub fn slot_takes_mouse(r: &[u8]) -> bool {
    r[0x182] != 0 && r[0x194] == 0 && r[0x138] != 0
}

/// 0x11B44: the right button opens the switch view with no assignment and hp not 0 (after `slot_takes_mouse`).
pub fn right_opens(r: &[u8]) -> bool {
    r[0x17E] == 0 && r[0x183] != 0
}

/// 0x1197C: the left button acts on a merc on the map with hp above 0.
pub fn left_acts(r: &[u8]) -> bool {
    r[0x138] != 0 && (r[0x183] as i8) > 0
}

/// A point on the face of slot k (not on the bar area): for script lines, which name a slot and not a mouse point.
pub fn face_point(k: u8, column: u8) -> (i32, i32) {
    let (x, y) = slot_origin(k);
    (x + if column == 0 { 20 } else { 10 }, y + 20)
}

/// 0x119DE: the writes of a "cont" click and the order it gives.
pub fn cont_plan(r: &mut [u8]) -> ContPlan {
    r[0x16D] = 0;
    r[0x1A6] = r[0x95];
    r[0x134] = r[0x96];
    r[0x135] = r[0x97];
    let (task, here, dest) = (r[0x1A6], s16(r, 0x114), s16(r, 0x120));
    if task != 0 && (here == dest || dest == 0xC41) {
        ContPlan::Use { here, tile: s16(r, 0x134), task }
    } else {
        ContPlan::Walk { dest, sneak: r[0x1A8], b177: r[0x177], b196: r[0x196] }
    }
}

/// 0x11A8C: the third argument of TacticalClick in a "cont" click: the task, 0 for the tasks 0xFE and 0xFF.
pub fn click_item(task: u8) -> i32 {
    if task >= 0xFE { 0 } else { task as i32 }
}

/// 0x11B76: the view byte goes up by 1 and SetSlotView 0x332E4 keeps it; true = the switch view is now open.
pub fn view_step(r: &mut [u8]) -> bool {
    r[0x17F] = crate::panel::set_slot_view(r[0x17F].wrapping_add(1));
    r[0x17F] == 1
}

/// 0x11C3C: a left press on switch i: the flag flips; the answer is the element for DrawSwitchFace 0x33650.
pub fn switch_press(r: &mut [u8], k: u8, selected: u8, i: usize) -> u8 {
    r[0x16A + i] ^= 1;
    let base = if selected == k { 0x4A } else { 0x46 };
    base + (r[0x16A + i] != 0) as u8
}

/// 0x11CC4: Reserve AP follows the release of switches 0 and 1 only.
pub fn switch_reserves(i: usize) -> bool {
    i < 2
}

/// 0x11BA1: the mouse limit of the open switch view (x0, y0, x1, y1).
pub fn switch_limit(k: u8) -> (i32, i32, i32, i32) {
    let (x, y) = slot_origin(k);
    (x + 2, y, x + 0x2F, y + 0x29)
}

impl SlotMouse {
    /// One pass off the map, no button: what the window runs at each mouse move and each pass.
    pub fn hover_pass(&mut self, x: i32, y: i32, redraw_off: u8, soldiers: &mut [u8], selected: u8, on_screen: bool) {
        let mut out = Vec::new();
        self.pass(Mouse { x, y, buttons: 0 }, redraw_off, soldiers, selected, &mut |_| on_screen, &mut out, &mut |_, _, _, _, _| {});
    }

    /// One pass off the map (0x1463A .. 0x14721), `soldiers` = at least 8 records. `on_screen` = OnScreen 0x1FC34 of a
    /// merc. A button on a slot calls `act`.
    #[allow(clippy::too_many_arguments)]
    pub fn pass(&mut self, m: Mouse, redraw_off: u8, soldiers: &mut [u8], selected: u8, on_screen: &mut dyn FnMut(u8) -> bool, out: &mut Vec<Call>,
                act: &mut dyn FnMut(&mut SlotMouse, u8, Act, &mut [u8], &mut Vec<Call>)) {
        self.last = NONE;
        self.over = 0;
        // 0x1463A: x 1..48 is the left column, 49..318 the right column (only rows decide in each), 0 and 319 scroll the view.
        let col = if m.x < 1 || m.x > 318 || redraw_off >= 2 { 0..0 } else if m.x > 48 { 4..8u8 } else { 0..4u8 };
        for k in col {
            let at = k as usize * crate::soldier::REC;
            self.slot(k, m, soldiers, at, selected, on_screen, out, act);
        }
        self.frame_end(out);
    }

    /// One pass with the mouse on the map (0x14225): only the "cont" goes. This path does not reach the frame end at
    /// 0x146DD, so a hover frame stays while the mouse jumps from a bar area to the map.
    pub fn on_map(&mut self, out: &mut Vec<Call>) {
        if self.cont < NONE {
            out.push(Call::Redraw(self.cont));
            self.cont = NONE;
        }
    }

    /// 0x146DD: the frame after the slots.
    fn frame_end(&mut self, out: &mut Vec<Call>) {
        if self.over == 0 && self.hover < NONE {
            self.clear_hover(out);
        }
        if self.last != self.cont && self.cont < NONE {
            out.push(Call::Redraw(self.cont));
            self.cont = NONE;
        }
    }

    /// ClearStripHover 0x1CB58.
    fn clear_hover(&mut self, out: &mut Vec<Call>) {
        let old = self.hover;
        self.hover = NONE;
        out.push(Call::Redraw(old));
    }

    /// The left button on slot k (0x1196C), for a merc that takes the mouse.
    pub fn left(&self, k: u8, r: &[u8], selected: u8) -> Option<Act> {
        if !left_acts(r) {
            None
        } else if self.hover == k {
            Some(Act::Popup)
        } else if self.cont == selected {
            Some(Act::Cont)
        } else {
            Some(Act::Select)
        }
    }

    /// SlotMouse 0x11714.
    #[allow(clippy::too_many_arguments)]
    fn slot(&mut self, k: u8, m: Mouse, soldiers: &mut [u8], at: usize, selected: u8, on_screen: &mut dyn FnMut(u8) -> bool,
            out: &mut Vec<Call>, act: &mut dyn FnMut(&mut SlotMouse, u8, Act, &mut [u8], &mut Vec<Call>)) {
        let (sx, sy) = slot_origin(k);
        if m.y < sy || m.y > sy + 0x29 {
            return;
        }
        let r = &mut soldiers[at..at + crate::soldier::REC];
        self.last = k;
        if !slot_takes_mouse(r) {
            return;
        }
        let side = slot_side(r[0x180]);
        let (bx, by) = (sx + side[2].0, sy + side[2].1);
        let on_bars = m.x >= bx && m.x <= bx + 10 && m.y >= by && m.y <= by + 0x1A;
        if on_bars || (self.cont < NONE && self.cont != k) {
            out.push(Call::Redraw(self.cont));
            self.cont = NONE;
        }
        if on_bars {
            if self.hover != k {
                if self.hover < 8 {
                    self.clear_hover(out);
                }
                self.hover = k;
                if r[0x183] != 0 {
                    out.push(Call::Frame(bx, by, bx + 10, by + 0x1A));
                    out.push(Call::Copy(k));
                }
            }
            self.over = 1;
        } else {
            if self.hover == k {
                self.hover = NONE;
                out.push(Call::Redraw(k));
            }
            if k == selected && r[0x16D] != 0 && self.cont != k && on_screen(k) {
                out.push(Call::ContText(sx + side[0].0 + 15, sy + side[0].1 + 12));
                self.cont = k;
            }
        }
        if m.buttons & 1 != 0 {
            if let Some(a) = self.left(k, r, selected) {
                act(self, k, a, r, out);
                return;
            }
        }
        if m.buttons & 2 != 0 && right_opens(r) {
            act(self, k, Act::Switch, r, out);
        }
    }
}

#[cfg(test)]
mod rest_tests {
    /// Ledger rows c2aea50:ja1front:1322/1325/1328/1331: any one blocker (a map, a left sector, an aim, a drag, a box, a
    /// press, a pause) stops the mouse rest pass; the enemies' turn (0x92DCC at 0) stops it too.
    #[test]
    fn any_blocker_stops_the_rest_pass() {
        assert!(super::rest_pass_runs([false; 10], 1));
        assert!(!super::rest_pass_runs([false; 10], 0));
        for k in 0..10 {
            let mut b = [false; 10];
            b[k] = true;
            assert!(!super::rest_pass_runs(b, 1), "blocker {k}");
        }
    }
}
