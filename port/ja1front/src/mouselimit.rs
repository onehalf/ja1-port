//! The mouse limit, 0x682AC SetMouseLimit? (x0, y0, x1, y1): the original asks the mouse driver to keep the pointer in
//! a rectangle (int 0x33 AX 7 and AX 8), so a player sees the pointer stop at these lines. The window port keeps the
//! rectangle in frame.rs, which clamps the positions it reads and moves the desktop pointer back.
pub type Limit = (i32, i32, i32, i32);

/// The full screen: 0x43687, 0x43878, 0x43925 (map) and others.
pub const FULL: Limit = (0, 0, 319, 199);
/// Island map and guard screen with the mouse over the map: 0x43412, 0x42A51.
pub const OVER_MAP: Limit = (3, 3, 0x138, 199);
/// Above this row the mouse is over the map (0x433D0, 0x429D0 compare with 0x8D).
pub const MAP_ROWS: i32 = 0x8D;
/// The native wage box: 0x43547, until the click that ends it.
pub const WAGE_BOX: Limit = (0x8A, 0xA2, 0xAC, 0xAA);
/// The finance page 0x437C0, PayrollScreen 0x44445 and PayDayDialog? 0x45556: the panel rows.
pub const PANEL_ROWS: Limit = (0, 0x92, 0x13F, 199);

/// The position that the driver gives for a mouse at (x, y).
pub fn clamp(l: Limit, x: i32, y: i32) -> (i32, i32) {
    (x.min(l.2).max(l.0), y.min(l.3).max(l.1))
}

/// The morning's limits, by host op (DayEndScreen 0x65BD4 runs them in this order): 301 PayrollScreen 0x44445 sets the
/// panel rows; 273 is the top of each pass of the funds dialog 0x45198 (the full screen: the player picks a face); 278
/// is its end 0x45556 (the panel rows again); 287 is the end of ExperienceChangeMsg? 0x462CB (the full screen, so the
/// salary box and the payroll wait for button 16 run with no limit).
pub fn of_op(op: i32) -> Option<Limit> {
    match op {
        301 | 278 => Some(PANEL_ROWS),
        273 | 287 => Some(FULL),
        _ => None,
    }
}

/// The mouse place of a morning host op. Op 278 is the end of PayDayDialog? 0x4552A: every road out of the dialog (also money
/// enough at once) runs MouseSetPos? 0x681D0 at 0x45544 with the words 0x934FA, 0x934FC, the corner of the funds box.
pub fn place_of_op(op: i32) -> Option<(i32, i32)> {
    (op == 278).then_some((279, 166))
}

/// The state of the map loops (StrategicScreen 0x432F4, GuardAssignScreen? 0x427B4): only the crossing of row 0x8D
/// sets a new limit. `below` starts true: their "last sector" starts at 0x3C or more.
#[derive(Clone, Copy, Debug)]
pub struct MapLimit {
    pub below: bool,
}

impl MapLimit {
    pub fn new() -> MapLimit {
        MapLimit { below: true }
    }

    /// One pass of a map loop with the mouse in row `y`: the new limit, if the pass sets one.
    pub fn step(&mut self, y: i32) -> Option<Limit> {
        if y < MAP_ROWS {
            if self.below {
                self.below = false;
                return Some(OVER_MAP);
            }
        } else if !self.below {
            self.below = true;
            return Some(FULL);
        }
        None
    }
}

impl Default for MapLimit {
    fn default() -> MapLimit {
        MapLimit::new()
    }
}
