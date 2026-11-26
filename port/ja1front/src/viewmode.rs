//! The tactical view modes (panel.md "View modes (keys PgUp, PgDn, Insert, Tab)"): 0 overview, 1 normal, 2 wide,
//! 3 full; Set mode and its keys.

/// The values that Set mode reads and writes (all i32 so that a test can pass them as one array).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewMode {
    /// opt_onscreen, 0..3.
    pub mode: i32,
    pub view_x: i32,
    pub view_y: i32,
    pub cols: i32,
    pub rows: i32,
    pub left: i32,
    pub top: i32,
    pub small_panel: i32,
    /// The last mode 2 or 3 minus 2 (signed byte).
    pub last_wide: i32,
    pub old_mode: i32,
    /// "Full view shown" byte (mode 3 without the small panel sets it).
    pub full_shown: i32,
    /// Slot frame slot (0x40 = none).
    pub frame_slot: i32,
    /// "Panel moved" byte (0x40 = none).
    pub panel_moved: i32,
    /// The overview filter (1 = only the tiles with byte 4 bit 0x80).
    pub filter: i32,
    /// The mouse flag (1 in tactical play): the mouse moves only when it is not 0.
    pub mouse_on: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
    /// Out: how often the mouse was moved, the path marks went away and the strips were drawn.
    pub mouse_moves: i32,
    pub marks_off: i32,
    pub strips: i32,
    /// Out: 1 = the whole screen is drawn again.
    pub redraw: i32,
    /// The strip flag (Set mode to 0, 1 or 2 sets it).
    pub strip_flag: i32,
}

impl ViewMode {
    /// The normal screen of a fresh sector (mode 1, window 14 x 12 at (48, 14)).
    pub fn new() -> ViewMode {
        ViewMode { mode: 1, cols: 14, rows: 12, left: 48, top: 14, frame_slot: 0x40, panel_moved: 0x40, mouse_on: 1,
                   ..Default::default() }
    }

    pub fn view_w(&self) -> i32 {
        self.cols.wrapping_mul(16)
    }

    pub fn view_h(&self) -> i32 {
        self.rows.wrapping_mul(14)
    }

    fn window(&mut self, top: i32, cols: i32, rows: i32) {
        self.cols = cols;
        self.rows = rows;
        self.left = if cols < 20 { 48 } else { 0 };
        self.top = top;
    }

    /// panel.md "The small panel in the full view" step 3 (End): the full view comes back.
    pub fn full_view(&mut self) {
        self.full_shown = 1;
        self.window(0, 20, 14);
        self.clamp();
    }

    fn clamp(&mut self) {
        let (mx, my) = (0x380i32.wrapping_sub(self.view_w()), 0x310i32.wrapping_sub(self.view_h()));
        if self.view_x < 0 {
            self.view_x = 0;
        } else if self.view_x > mx {
            self.view_x = mx;
        }
        if self.view_y < 0 {
            self.view_y = 0;
        } else if self.view_y > my {
            self.view_y = my;
        }
    }

    /// Set mode (m), steps 1..6.
    pub fn set(&mut self, m: i8) {
        let strips_before = self.strips;
        let cur = self.mode as u8;
        if m as i32 == cur as i32 {
            return;
        }
        self.marks_off += 1;
        let m: u8 = if m < 0 { 3 } else if m > 3 { 0 } else { m as u8 };
        self.old_mode = cur as i32;
        self.frame_slot = 0x40;
        let mut cur = cur;
        match m {
            2 | 3 => {
                self.panel_moved = 0x40;
                if m == 3 {
                    self.view_y = self.view_y.wrapping_sub(14);
                    if self.small_panel != 0 {
                        self.window(0, 20, 13);
                    } else {
                        self.window(0, 20, 14);
                        self.full_shown = 1;
                    }
                } else {
                    self.window(14, 20, 12);
                    self.small_panel = 0;
                    if cur == 3 {
                        self.view_y = self.view_y.wrapping_add(14);
                        cur = 2;
                        self.strips += 1;
                    }
                }
                // Port fix P15 (#2 row 33): the overview holds mode-1 coordinates, so 0 -> 2 / 3 shifts as 1 -> 2 / 3 does.
                if cur == 1 || (cur == 0 && ja1logic::fanfix::port_on(15)) {
                    self.view_x = self.view_x.wrapping_sub(48);
                }
                self.last_wide = (m as i8 - 2) as i32;
            }
            1 => {
                self.window(14, 14, 12);
                if cur != 0 {
                    self.view_x = self.view_x.wrapping_add(48);
                    if cur == 3 {
                        self.view_y = self.view_y.wrapping_add(14);
                    }
                    self.small_panel = 0;
                    self.strips += 1;
                }
            }
            _ => {
                if self.old_mode as u8 as i8 >= 2 {
                    self.strips += 1;
                }
                if self.mouse_on as u8 != 0 {
                    let bottom = self.top.wrapping_sub(1).wrapping_add(self.view_h());
                    if self.mouse_y > bottom {
                        self.mouse_y = bottom - 1;
                    }
                    let col = self.view_x.wrapping_add(self.mouse_x.wrapping_sub(self.left)) / 16;
                    let row = self.view_y.wrapping_add(self.mouse_y.wrapping_sub(self.top)) / 14;
                    self.mouse_x = col.wrapping_mul(4).wrapping_add(48);
                    self.mouse_y = row.wrapping_mul(3).wrapping_add(14);
                    self.mouse_moves += 1;
                }
                // Port fix P15 (#2 row 33): 2 / 3 -> 0 takes back the wide-view shifts (after the mouse point, which the
                // shown view gives), so the overview holds mode-1 coordinates and 0 -> 1 lands where 1 left.
                let o = self.old_mode as u8 as i8;
                if o >= 2 && ja1logic::fanfix::port_on(15) {
                    self.view_x = self.view_x.wrapping_add(48);
                    if o == 3 {
                        self.view_y = self.view_y.wrapping_add(14);
                    }
                }
                self.window(14, 14, 12);
                self.small_panel = 0;
            }
        }
        if m != 3 {
            self.strip_flag = 1;
        }
        // Each count of `strips` is a top strip reload of SetViewMode 0x24AA7 / 0x24AE1 (live: 3 -> 2 ends the squad
        // lost shade; 1 -> 2, 2 -> 3, 1 -> 0, 0 -> 1 and 0 -> 3 keep it).
        if self.strips != strips_before {
            crate::strips::clear_lost_shade();
        }
        self.mode = m as i32;
        self.clamp();
        self.redraw = 1;
    }

    /// Key PgUp.
    pub fn page_up(&mut self) {
        let m = if (self.mode as u8) < 3 { self.mode as u8 as i8 + 1 } else { 0 };
        self.set(m);
    }

    /// Key PgDn.
    pub fn page_down(&mut self) {
        self.set((self.mode as u8).wrapping_sub(1) as i8);
    }

    /// Key Insert.
    pub fn insert(&mut self) {
        if self.mode as u8 != 0 {
            self.filter = 1;
            self.set(0);
        } else if ja1logic::fanfix::on(45) {
            // F45 (1.15l changelog, "the Insert button now correctly toggles between fog-of-war and fully revealed
            // map views", found by DarzaR; 1.15m TacticalKeyHandler 0x137E2): in the overview Insert clears filter
            // bit 0 (fog of war off: the whole map); with the filter on the overview is set again, with it off the normal
            // view (mode 1) comes back.
            let old = self.filter as u8 & 1;
            self.filter = (self.filter as u8 & 0xFE) as i32;
            self.set(if old != 0 { 0 } else { 1 });
        } else {
            self.filter = (self.filter as u8 & 1) as i32;
        }
    }

    /// Key Tab.
    pub fn tab(&mut self) {
        match self.mode as u8 {
            0 => self.filter = (self.filter as u8 ^ 1) as i32,
            1 => self.set((self.last_wide as i8).wrapping_add(2)),
            _ => self.set(1),
        }
    }

    /// The overview click at mouse point (x, y): returns the cursor tile. The view goes there and the old mode
    /// comes back (the caller runs the tile's cursor afterwards).
    pub fn overview_click(&mut self, x: i32, y: i32) -> i32 {
        let y = if y < 14 { 15 } else if y > 181 { 181 } else { y };
        let x = x.clamp(48, 270);
        let (col, row) = ((x - 48) / 4, (y - 14) / 3);
        let mut t = row * 56 + col;
        self.view_x = ((col - 7) * 16).max(0);
        self.view_y = ((row - 7) * 14).max(0);
        while t < 0 {
            t += 56;
        }
        while t > 3136 {
            t -= 56;
        }
        self.mouse_x = (t % 4) * 4 + 48;
        self.mouse_y = (t / 3) * 3 + 14;
        self.mouse_moves += 2;
        self.set(self.old_mode as u8 as i8);
        // The mouse goes to the tile's corner in the view of the mode that came back.
        self.mouse_x = 16 * (t % 56) - self.view_x + self.left;
        self.mouse_y = 14 * (t / 56) - self.view_y + self.top;
        self.mouse_moves += 1;
        t
    }
}

/// The values of the overview mouse step (panel.md "The overview mouse step"; all i32 so that a test can pass
/// them as one array). In / out unless marked.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OvMouse {
    /// The "mouse shown" byte and the mouse flag.
    pub shown: i32,
    pub mouse_on: i32,
    /// In: the mouse point and buttons (1 left, 2 right).
    pub x: i32,
    pub y: i32,
    pub buttons: i32,
    pub tile: i32,
    pub ap_timer: i32,
    pub colour: i32,
    pub cost: i32,
    pub over: i32,
    /// In: the selected slot and its bytes +0x1B8, hp +0x183, apLeft +0x1A5, +0x177 (signed where the step reads them
    /// signed).
    pub sel: i32,
    pub sel_1b8: i32,
    pub sel_hp: i32,
    pub sel_ap: i32,
    pub sel_177: i32,
    pub shift: i32,
    pub alt: i32,
}

/// The calls of the overview mouse step, in their order; the caller does them (and answers Occupant, Hp, Cost).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OvCall {
    ShowMouse,
    /// Hide path marks (force 1).
    HideMarks,
    Occupant(i32),
    /// The hp byte (+0x183) of soldier o, signed.
    Hp(u8),
    /// The visible byte (+0x9C) of soldier o, signed (#70 F46 only; o = 0..0x3F or 0x80..0xFF).
    Visible(u8),
    /// CursorItemBack? 0x53660: a held item goes back to the old merc (#70 F61 only).
    CursorBack,
    /// Select merc (o, force 0, quote 1).
    Select(u8),
    NotAvailable,
    /// Walk (S, T).
    Walk(i32),
    /// PathApCost (S, T, mark 1, extra 0, sneak, byte177, alt).
    Cost(i32),
    /// Wait until the right button is up, then the overview click.
    Jump,
}

impl OvMouse {
    /// Hide path marks (force 1): it first sets the AP number (cost) to 0 and the cursor tile to 0xC41.
    fn hide_marks(&mut self, call: &mut dyn FnMut(OvCall) -> i32) {
        self.cost = 0;
        self.tile = 0xC41;
        call(OvCall::HideMarks);
    }

    /// One overview mouse step.
    pub fn step(&mut self, call: &mut dyn FnMut(OvCall) -> i32) {
        if self.shown as u8 == 0 {
            call(OvCall::ShowMouse);
            self.mouse_on = 1;
        }
        let t = overview_tile(self.x, self.y);
        if t != self.tile {
            self.ap_timer = 800;
            self.hide_marks(call);
            self.tile = t;
        }
        if self.sel_1b8 as u8 == 0 {
            self.ap_timer = 0;
            self.hide_marks(call);
        }
        let o = call(OvCall::Occupant(t)) as u8;
        if self.buttons & 1 != 0 {
            let fan = ja1logic::fanfix::on(46);
            // #70 F46 (1.15m 0x24733 -> helper 0x2C57C): a soldier tile walks unless that soldier is seen.
            let vis = if fan && !(o < 8 && self.sel_hp as u8 != 0) && (o as i8) < 0x40 { call(OvCall::Visible(o)) } else { 0 };
            if press_walks(o, self.sel_hp, vis) {
                call(OvCall::Walk(t));
            } else if o < 8 && self.sel_hp as u8 != 0 {
                if call(OvCall::Hp(o)) as i8 > 14 {
                    // #70 F61 (row 87): 1.15m 0x24727 calls cave 0x8AF04: CursorItemBack? 0x53660, then SelectMerc?.
                    if ja1logic::fanfix::on(61) {
                        call(OvCall::CursorBack);
                    }
                    call(OvCall::Select(o));
                } else if fan {
                    // #70 F46 (1.15m 0x2471A): a merc under 15 hp gives "Not available!" in place of nothing.
                    call(OvCall::NotAvailable);
                }
            } else {
                call(OvCall::NotAvailable);
            }
            return;
        }
        if self.buttons & 2 != 0 {
            call(OvCall::Jump);
            return;
        }
        self.colour = if o < 8 && o != self.sel as u8 { 0xAF } else { 0x4F };
        if self.sel_hp as i8 > 14 && self.ap_timer > 0 && o >= 0x40 {
            self.ap_timer -= 10;
            if self.ap_timer < 1 {
                self.ap_timer = 0;
                self.cost = call(OvCall::Cost(self.tile));
                self.over = if self.cost > self.sel_ap as i8 as i32 { 1 } else { 0xFF };
            }
        }
    }
}

/// The screen point of overview tile `t` (its top left pixel in the overview: 4 x 3 pixels per tile at (48, 14)).
pub fn overview_point(t: i32) -> (i32, i32) {
    (48 + 4 * t.rem_euclid(56), 14 + 3 * t.div_euclid(56))
}

/// The overview tile at screen point (x, y) (OverviewTileAtMouse 0x24488; panel.md "The overview mouse step"
/// step 2). The original has no upper clamp; the port clamps to the last tile for its window mouse positions.
pub fn overview_tile(x: i32, y: i32) -> i32 {
    (((y - 14) / 3) * 56 + (x - 48) / 4).min(3135)
}

/// Tile under a tactical view pixel (TacticalKeyHandler helper 0x159CC; used by main.rs `map_tile`).
pub fn tactical_tile(c0: i32, r0: i32, vx: i32, vy: i32) -> i32 {
    // The division cuts the whole map pixel: a mouse left of or above the view differs from a cut of vx alone.
    ((r0 * 14 + vy) / 14) * 56 + (c0 * 16 + vx) / 16
}

/// The window (cols, rows, left, top) of a mode (panel.md "View modes" table; small = the small panel shows).
/// TacticalKeyHandler 0x14308 + 0x143CA: the mouse off the map window, in its rows, on the strip beside the map. `rect` =
/// left, top, right, bottom (0x229CB8, 0x229CC4, 0x229CD8, 0x229CDC). The frame the mouse leaves the map (over_map 1) it
/// goes on to x 39 or x 276 (MouseSetPos 0x14391); else it goes in to the map's edge column (0x143FB..0x14434). Proof:
/// live in the original, issue #31. Only views with a left panel have such x.
pub fn margin_mouse(rect: (i32, i32, i32, i32), over_map: bool, x: i32, y: i32) -> Option<(i32, i32)> {
    let (left, top, right, bottom) = rect;
    if !(top..=bottom).contains(&y) || (left..=right).contains(&x) {
        return None;
    }
    let to = if over_map {
        match x {
            41..=199 => 39,
            200..=274 => 276,
            _ => return None,
        }
    } else {
        match x {
            41..=199 => left + 1,
            200..=275 => right,
            _ => return None,
        }
    };
    Some((to, y))
}

pub fn window(mode: u8, small: bool) -> (i32, i32, i32, i32) {
    match mode {
        2 => (20, 12, 0, 14),
        3 => (20, if small { 13 } else { 14 }, 0, 0),
        _ => (14, 12, 48, 14),
    }
}

/// The window of a front-end world: in mode 3 it is 20 x 13 while the small panel shows (panel.md "The small panel
/// in the full view": the window stays 20 x 13 after the countdown until the panel ends).
pub fn world_window(w: &crate::world::World) -> (i32, i32, i32, i32) {
    window(w.opt_onscreen, w.opt_onscreen == 3 && w.view_h == 14 * 13)
}

/// The view scroll of TacticalKeyHandler 0x13490 (panel.md "Arrow-key scroll"): the steps dy 0x9344C / dx 0x9344D
/// (signed bytes, 0 = none), the hover timer 0x229D28 and the delay 0x229D44 (both ms, the 10 ms timer counts them
/// down, see cursor::clocks_tick).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Scroll {
    pub dy: i32,
    pub dx: i32,
    pub hover: i32,
    pub delay: i32,
}

impl Scroll {
    /// The arrow key cases (scan codes 0x48 up 0x137F1, 0x4B left 0x1382F, 0x4D right 0x13850, 0x50 down 0x13815):
    /// one step of 14 (rows) or 16 (columns, 0x229D10 / 0x229D0C) pixels. Returns true for an arrow key (the
    /// handler's "key scroll" byte; the direction byte 0x284F17 it also writes has no reader).
    pub fn arrow_key(&mut self, key: u8) -> bool {
        match key {
            0x48 => self.dy = -14,
            0x4B => self.dx = -16,
            0x4D => self.dx = 16,
            0x50 => self.dy = 14,
            _ => return false,
        }
        true
    }

    /// The mouse at the screen edge (TacticalKeyHandler 0x14201..0x14721, panel.md "Mouse-edge scroll"; the
    /// handler runs it after the key cases). The window rect is left..left + 16 cols - 1, top..top + 14 rows - 1
    /// (SetViewWindow 0x24880, MouseInRect 0x67C0C). In the rect and not in the overview: x < 1 left, x > 318
    /// right, y < 1 up. Outside: y above the rect and < 1 up, y below it and >= 199 down; x right of the left edge
    /// and >= 319 right, else x < 1 left. A step that is not set here keeps its key value.
    pub fn mouse_edge(&mut self, v: &ViewMode, x: i32, y: i32) {
        let (right, bottom) = (v.left + 16 * v.cols - 1, v.top + 14 * v.rows - 1);
        if (v.left..=right).contains(&x) && (v.top..=bottom).contains(&y) {
            if v.mode == 0 {
                return;
            }
            if x < 1 {
                self.dx = -16;
            } else if x > 318 {
                self.dx = 16;
            }
            if y < 1 {
                self.dy = -14;
            }
            return;
        }
        if y < v.top {
            if y < 1 {
                self.dy = -14;
            }
        } else if y > bottom && y >= 199 {
            self.dy = 14;
        }
        if x > v.left {
            if x >= 319 {
                self.dx = 16;
            }
        } else if x < 1 {
            self.dx = -16;
        }
    }

    /// The end of the handler (0x14721..0x147A4): `key` = an arrow key came in this pass, `fast` / `alt_scroll` =
    /// the switches Fast Scroll 0x1163BA / Alt Scroll 0x1163BB, `alt` = the Alt key. Without a step the hover timer
    /// starts again (200 ms). A step runs (0x1EDAC) when it came from a key, or when the hover time is over without
    /// Alt (with Alt Scroll: when Alt is held); and then only when the delay is over or Fast Scroll is on. A step
    /// that may not run now waits for the next pass if it came from a key or from the hover rule, else it goes away.
    /// Returns true when the view moved.
    pub fn pass(&mut self, v: &mut ViewMode, cursor_tile: &mut i32, key: bool, fast: u8, alt_scroll: u8, alt: u8)
                -> bool {
        if self.dy as i8 == 0 && self.dx as i8 == 0 {
            self.hover = 200;
            return false;
        }
        let go = if alt_scroll != 0 { alt != 0 } else { alt == 0 && self.hover <= 0 };
        if !go && !key {
            self.dy = 0;
            self.dx = 0;
            return false;
        }
        if self.delay > 0 && fast == 0 {
            return false;
        }
        self.step(v, cursor_tile);
        true
    }

    /// 0x1EDAC: the cursor tile moves one column / row with the view (StepTile, stays on the map), the view moves by
    /// the steps and is kept on the map (ClampView 0x24830), the delay starts (70 ms), the steps end and the screen
    /// is drawn again.
    fn step(&mut self, v: &mut ViewMode, cursor_tile: &mut i32) {
        let (dy, dx) = (self.dy as i8 as i32, self.dx as i8 as i32);
        if dx != 0 {
            *cursor_tile = ja1logic::ai::step_tile(*cursor_tile, dx / 16);
            v.view_x = v.view_x.wrapping_add(dx);
        }
        if dy != 0 {
            *cursor_tile = ja1logic::ai::step_tile(*cursor_tile, dy / 14 * 56);
            v.view_y = v.view_y.wrapping_add(dy);
        }
        v.clamp();
        self.delay = 70;
        self.dy = 0;
        self.dx = 0;
        v.redraw = 1;
    }
}

/// The view values of a front-end world, the mouse at screen point `mouse`.
pub fn from_world(w: &crate::world::World, mouse: (i32, i32)) -> ViewMode {
    let (cols, rows, left, top) = world_window(w);
    ViewMode { mode: w.opt_onscreen as i32, view_x: w.view_x, view_y: w.view_y, cols, rows, left, top,
               small_panel: w.small_panel, last_wide: w.view_last_wide as i32, old_mode: w.view_old_mode as i32,
               full_shown: w.full_shown as i32, frame_slot: 0x40, panel_moved: 0x40, filter: w.overview_filter as i32,
               mouse_on: 1, mouse_x: mouse.0, mouse_y: mouse.1, strip_flag: w.strip_flag as i32, ..Default::default() }
}

/// Writes the view values back (view_w / view_h from the window).
pub fn to_world(v: &ViewMode, w: &mut crate::world::World) {
    w.opt_onscreen = v.mode as u8;
    w.view_x = v.view_x;
    w.view_y = v.view_y;
    w.view_w = v.view_w();
    w.view_h = v.view_h();
    w.view_last_wide = v.last_wide as i8;
    w.view_old_mode = v.old_mode as u8;
    w.overview_filter = v.filter as u8;
    w.small_panel = v.small_panel;
    w.full_shown = v.full_shown as u8;
    w.strip_flag = v.strip_flag as u8;
}

/// Test entry for validate/diff/viewmode_diff.py: `v` = the 22 values of `ViewMode` (in / out); `key` = 0 Set mode
/// (m), 1 PgUp, 2 PgDn, 3 Insert, 4 Tab, 5 the overview click at the mouse point (returns the cursor tile; else 0).
///
/// # Safety
/// `v` points to 22 int32 values.
#[no_mangle]
pub unsafe extern "C" fn ja1f_view_mode(v: *mut ViewMode, key: i32, m: i32) -> i32 {
    if v.is_null() {
        return 0;
    }
    let v = &mut *v;
    match key {
        0 => v.set(m as i8),
        1 => v.page_up(),
        2 => v.page_down(),
        3 => v.insert(),
        4 => v.tab(),
        _ => return v.overview_click(v.mouse_x, v.mouse_y),
    }
    0
}

/// Test entry for validate/diff/scroll_diff.py: `s` = the 4 values of `Scroll`, `v` = the 22 values of `ViewMode`
/// (in / out), `cursor_tile` (in / out), `key` = 0 or an arrow scan code (the key case runs first), `opts` = Fast
/// Scroll, Alt Scroll, Alt, then the mouse: on (1 = the mouse edge test runs after the key case), x, y. Returns 1
/// when the view moved.
///
/// # Safety
/// `s` points to 4 int32 values, `v` to 22, `cursor_tile` to 1, `opts` to 6.
#[no_mangle]
pub unsafe extern "C" fn ja1f_scroll(s: *mut Scroll, v: *mut ViewMode, cursor_tile: *mut i32, key: i32,
                                     opts: *const i32) -> i32 {
    if s.is_null() || v.is_null() || cursor_tile.is_null() || opts.is_null() {
        return -1;
    }
    let (s, v, o) = (&mut *s, &mut *v, std::slice::from_raw_parts(opts, 6));
    let k = key != 0 && s.arrow_key(key as u8);
    if o[3] != 0 {
        let vv = *v;
        s.mouse_edge(&vv, o[4], o[5]);
    }
    s.pass(v, &mut *cursor_tile, k, o[0] as u8, o[1] as u8, o[2] as u8) as i32
}

/// OverviewMouse 0x24630, left press: only a tile with no soldier (and with a living selected merc or none on it) reaches
/// AllMoveTraverse?, so only this press starts the edge drag. `vis` = the visible byte (+0x9C, signed) of soldier o.
/// #70 F46 (1.15m helper 0x2C57C, a signed byte compare): o 0x40..0x7F walks, and any other o walks when vis <= 0.
pub fn press_walks(o: u8, sel_hp: i32, vis: i32) -> bool {
    if o < 8 && sel_hp as u8 != 0 {
        return false;
    }
    if ja1logic::fanfix::on(46) {
        (o as i8) >= 0x40 || vis <= 0
    } else {
        o >= 0x40
    }
}

/// Test entry for validate/diff/ovmouse_diff.py: `v` = the 17 values of `OvMouse` (in / out); `ans` = the answers
/// (occupant, hp of the occupant, cost, visible byte of the occupant); `log` gets the calls (2 int32 each: kind 1 show mouse, 2 hide marks, 3
/// select, 4 not available, 5 walk, 6 cost, 7 jump; then the slot or tile; at most 16 calls). Returns the count.
///
/// # Safety
/// `v` points to 17 int32 values, `ans` to 4, `log` to 32.
#[no_mangle]
pub unsafe extern "C" fn ja1f_ov_mouse(v: *mut OvMouse, ans: *const i32, log: *mut i32) -> i32 {
    if v.is_null() || ans.is_null() || log.is_null() {
        return -1;
    }
    let (v, ans, log) = (&mut *v, std::slice::from_raw_parts(ans, 4), std::slice::from_raw_parts_mut(log, 32));
    let mut n = 0usize;
    let mut put = |k: i32, a: i32| {
        if n < 16 {
            log[2 * n] = k;
            log[2 * n + 1] = a;
            n += 1;
        }
    };
    v.step(&mut |c| match c {
        OvCall::Occupant(_) => ans[0],
        OvCall::Hp(_) => ans[1],
        OvCall::Visible(_) => ans[3],
        OvCall::ShowMouse => { put(1, 0); 0 }
        OvCall::HideMarks => { put(2, 1); 0 }
        OvCall::Select(o) => { put(3, o as i32); 0 }
        OvCall::NotAvailable => { put(4, 0); 0 }
        OvCall::Walk(t) => { put(5, t); 0 }
        OvCall::Cost(t) => { put(6, t); ans[2] }
        OvCall::Jump => { put(7, 0); 0 }
        OvCall::CursorBack => { put(8, 0); 0 }
    });
    n as i32
}

#[cfg(test)]
mod tests {
    #[test]
    fn margin_mouse_as_the_original() {
        // The live run of the original (issue #31): each row = over_map, x, y, the place after it.
        let r = (48, 14, 271, 181);
        for (over, x, y, to) in [(true, 44, 100, Some(39)), (false, 44, 100, Some(49)), (true, 45, 100, Some(39)),
                                 (false, 272, 100, Some(271)), (true, 274, 100, Some(276)), (false, 273, 100, Some(271)),
                                 (false, 275, 100, Some(271)), (true, 275, 100, None), (false, 30, 100, None), (false, 290, 100, None),
                                 (false, 44, 10, None), (false, 44, 190, None), (false, 40, 100, None), (true, 100, 100, None)] {
            assert_eq!(margin_mouse(r, over, x, y), to.map(|t| (t, y)), "{} {} {}", over, x, y);
        }
    }

    use super::*;

    /// The overview AP number: a path cost equal to the AP left is reachable (over = 0xFF); one more is not (1).
    #[test]
    fn overview_cost_equal_to_ap_is_reachable() {
        for (cost, over) in [(5, 0xFF), (6, 1)] {
            let mut m = OvMouse { shown: 1, mouse_on: 1, x: 100, y: 100, ap_timer: 10, sel: 0, sel_1b8: 1, sel_hp: 20,
                                  sel_ap: 5, ..Default::default() };
            m.tile = overview_tile(m.x, m.y);
            m.step(&mut |c| match c {
                OvCall::Occupant(_) => 0x40,
                OvCall::Cost(_) => cost,
                _ => 0,
            });
            assert_eq!((m.ap_timer, m.cost, m.over), (0, cost, over), "cost {cost}");
        }
    }

    /// Mouse-edge scroll: the window rect ends at top + 14 rows - 1; y = 199 below a rect that ends at 197 scrolls down.
    #[test]
    fn mouse_below_the_rect_scrolls_down() {
        let v = ViewMode { mode: 1, left: 0, top: 30, cols: 14, rows: 12, ..Default::default() };
        let mut s = Scroll::default();
        s.mouse_edge(&v, 100, 199);
        assert_eq!((s.dx, s.dy), (0, 14));
        let mut s = Scroll::default();
        s.mouse_edge(&v, 100, 197);
        assert_eq!((s.dx, s.dy), (0, 0));
    }

    /// from_world takes the wide-view memory and the mouse point from the world and the caller.
    #[test]
    fn from_world_keeps_last_wide_and_mouse() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let (iso, exe) = (root.join("rom/JA1.gog"), root.join("build/game113b/JA.EXE"));
        if !iso.exists() || !exe.exists() {
            return;
        }
        let mut w = crate::world::World::new_game(iso.to_str().unwrap(), exe.to_str().unwrap(), 0, 0).unwrap();
        w.view_last_wide = 3;
        let v = from_world(&w, (123, 45));
        assert_eq!((v.last_wide, v.mouse_x, v.mouse_y), (3, 123, 45));
    }

    /// Live original, comment on #45: after a squad lost these mode changes keep the grey buttons, and 3 -> 2 ends them.
    #[test]
    fn squad_lost_shade_ends_at_a_strip_reload() {
        let shade = || crate::strips::frame_shade(0, 0);
        let mut v = ViewMode::new();
        crate::strips::set_lost_shade();
        for m in [0i8, 1, 2, 3] {
            v.set(m);
            assert_eq!(shade(), crate::strips::LOST_LAST, "mode {}", m);
        }
        v.set(2);
        assert_eq!(shade(), 0);
        crate::strips::set_lost_shade();
        assert_eq!(crate::strips::frame_shade(1, 0), 0);
        assert_eq!(shade(), 0);
    }

    #[test]
    fn page_up_round() {
        let mut v = ViewMode::new();
        v.view_x = 256;
        v.view_y = 168;
        let mut seen = vec![];
        for _ in 0..4 {
            v.page_up();
            seen.push((v.mode, v.view_x, v.view_y));
        }
        // Live check (panel.md "View modes" evidence).
        assert_eq!(seen, vec![(2, 208, 168), (3, 208, 154), (0, 208, 154), (1, 208, 154)]);
        assert_eq!((v.view_w(), v.view_h(), v.left, v.top), (224, 168, 48, 14));
    }

    #[test]
    fn tab_and_insert() {
        let mut v = ViewMode::new();
        v.page_up();
        v.page_up();
        v.tab();
        assert_eq!(v.mode, 1);
        v.tab();
        assert_eq!(v.mode, 3);
        v.insert();
        assert_eq!((v.mode, v.filter), (0, 1));
        v.tab();
        assert_eq!((v.mode, v.filter), (0, 0));
        v.page_down();
        assert_eq!(v.mode, 3);
    }
}

/// 0x1CB78, the redraw of the tactical screen: view mode byte 0x92DDF != 0 -> TacticalRedraw(1) (`None`), mode 0 ->
/// OverviewRedraw(1, AL, 0, 0) (`Some(AL)`). AL is the argument of 0x1CB78.
pub fn redraw_choice(mode: u8, al: u8) -> Option<u8> {
    if mode != 0 {
        None
    } else {
        Some(al)
    }
}
