//! cursor.md "Drag to an edge" and "ALL-move": the loop of AllMoveTraverse? 0x1CFEC, one poll at a time. The original
//! stays in this loop while the left button is down: only the timer runs. The mouse is a level: a right button that is
//! down with no direction active acts at that poll, also when it went down during a direction. The window, the script
//! lines and the diff test (ja1f_drag_run) are hosts.

/// The cursor tile "off the map" of the release.
pub const TILE_OFF: i32 = 0xC41;

/// What the loop asks of its host, in the original's call order.
pub trait Host {
    /// CanTraverseEdge 0x30FA0.
    fn edge(&mut self, dir: u8) -> bool;
    /// TacticalRedraw 0x176C8 or OverviewRedraw 0x23B4C with this cursor value and point (the overview's arguments).
    fn redraw(&mut self, value: u8, x: i32, y: i32);
    /// StatusMessage? 0x27D24 "ALL-MOVE not available during Turn-based".
    fn refuse(&mut self);
}

/// How the loop ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// The right button in combat: no restore of the value and the tile.
    Refused,
    /// The release with a direction: TraverseCheck 0x31344 toward this edge.
    Traverse(u8),
    /// The release after a failed edge test with no direction now: "Can't traverse here!" and no other call.
    NoEdge,
    /// The release with no direction: MoveMercTo 0x1C608.
    Walk { all: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wait {
    /// WaitRightUp 0x67C68 after the combat refusal.
    Refuse,
    /// WaitRightUp 0x67C68 after ALL turned on.
    All,
}

/// The state of the loop.
#[derive(Clone, Copy, Debug)]
pub struct Drag {
    /// The press tile (the EDX argument).
    pub tile: i32,
    /// The point that directions count from (SI, DI): the press, or the point of the last failed edge test.
    pub press: (i32, i32),
    /// Cursor value 0x284F1D at the press.
    pub saved: u8,
    /// Cursor value 0x284F1D now.
    pub value: u8,
    /// Cursor tile 0x229D2C now.
    pub cursor_tile: i32,
    /// The arguments of the last redraw (value, x, y): the picture stays until the next redraw.
    pub shown: Option<(u8, i32, i32)>,
    /// [esp+0xC]: a direction is active.
    pub active: bool,
    /// [esp+0x14]: the last edge test failed.
    pub failed: bool,
    /// [esp+4]: ALL is on.
    pub all: bool,
    wait: Option<Wait>,
}

impl Drag {
    pub fn new(tile: i32, at: (i32, i32), value: u8, cursor_tile: i32) -> Drag {
        Drag { tile, press: at, saved: value, value, cursor_tile, shown: None, active: false, failed: false, all: false, wait: None }
    }

    /// True: the loop waits in WaitRightUp.
    pub fn waiting(&self) -> bool {
        self.wait.is_some()
    }

    /// One mouse poll. `buttons`: 1 left, 2 right. `combat` = 0x92DCB, `locked` = round lock 0x92DE0.
    pub fn poll(&mut self, x: i32, y: i32, buttons: u8, combat: bool, locked: bool, host: &mut impl Host) -> Option<End> {
        match self.wait {
            Some(_) if buttons & 2 != 0 => return None,
            Some(Wait::Refuse) => return Some(End::Refused),
            Some(Wait::All) => {
                self.wait = None;
                self.value = self.saved;
                return self.bottom(buttons);
            }
            None => {}
        }
        if !self.active && buttons & 2 != 0 {
            if combat {
                host.refuse();
                self.wait = Some(Wait::Refuse);
                return None;
            }
            self.value = 0x0F;
            self.cursor_tile = self.tile;
            self.all = true;
            self.shown = Some((0x0F, 0, 0));
            host.redraw(0x0F, 0, 0);
            self.wait = Some(Wait::All);
            return None;
        }
        if !locked && !self.all {
            let (px, py) = self.press;
            let dir = if py - y > 16 {
                10
            } else if y - py > 16 {
                12
            } else if px - x > 16 {
                13
            } else if x - px > 16 {
                11
            } else {
                0
            };
            self.active = dir != 0;
            if self.active {
                self.value = dir;
                self.failed = false;
                if !host.edge(dir) {
                    self.value = 0xA1;
                    self.press = (x, y);
                    self.failed = true;
                    self.active = false;
                }
                self.cursor_tile = self.tile;
                self.shown = Some((self.value, self.press.0, self.press.1));
                host.redraw(self.value, self.press.0, self.press.1);
            }
        }
        self.bottom(buttons)
    }

    /// 0x1D1D3: the test of the left button and the tail's choice.
    fn bottom(&mut self, buttons: u8) -> Option<End> {
        if buttons & 1 != 0 {
            return None;
        }
        let value = self.value;
        self.cursor_tile = TILE_OFF;
        self.value = self.saved;
        Some(if self.active {
            End::Traverse(value)
        } else if self.failed {
            End::NoEdge
        } else {
            End::Walk { all: self.all }
        })
    }
}

/// The arguments of MoveMercTo 0x1C608 at 0x1D23A: BL target marker 1, CL sneak (Shift 0x284F21), stack: byte +0x177 = 0,
/// Alt 0x284F20, then the squad flag. The confirm argument comes from the caller: UseItemAt 1, OverviewMouse 0.
/// Returns (marker, sneak, b177, alt, squad).
pub fn move_args(shift: i32, alt: i32, all: i32) -> (i32, i32, i32, i32, i32) {
    (1, shift, 0, alt, all)
}

/// 0x1D26E: the "ALL" tile marker follows a squad order that gave 1.
pub fn all_marker(all: bool, result: i32) -> bool {
    all && result == 1
}

/// 0x1CFFC..0x1D03E with ClearTask 0x3DDB8 (DL = 1): the fields of soldier record `r` that the press resets.
pub fn press_fields(r: &mut [u8]) {
    for off in [0x177, 0x1A8, 0x196, 0x16D, 0x1A6, 0x1A7, 0x95] {
        r[off] = 0;
    }
    for off in [0x134, 0x96] {
        r[off..off + 2].copy_from_slice(&(TILE_OFF as i16).to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct H(u32);
    impl Host for H {
        fn edge(&mut self, _: u8) -> bool {
            false
        }
        fn redraw(&mut self, _: u8, _: i32, _: i32) {}
        fn refuse(&mut self) {
            self.0 += 1;
        }
    }

    /// AllMoveTraverse? 0x1CFEC: after the right press the loop stays in WaitRightUp 0x67C68 (it polls the mouse
    /// until the right button is up); the loop goes on only at the release.
    #[test]
    fn the_right_press_waits_for_its_release() {
        // in combat: the refusal message once, then the end at the release
        let mut h = H(0);
        let mut d = Drag::new(100, (50, 50), 3, 100);
        assert_eq!(d.poll(50, 50, 3, true, false, &mut h), None);
        for _ in 0..5 {
            assert_eq!(d.poll(50, 50, 3, true, false, &mut h), None);
            assert!(d.waiting());
        }
        assert_eq!(d.poll(50, 50, 1, true, false, &mut h), Some(End::Refused));
        assert_eq!(h.0, 1);
        // out of combat: ALL-MOVE stays in the wait with the right button down, the left button still held after it
        let mut d = Drag::new(100, (50, 50), 3, 100);
        assert_eq!(d.poll(50, 50, 3, false, false, &mut h), None);
        assert_eq!(d.poll(50, 50, 3, false, false, &mut h), None);
        assert!(d.waiting() && d.all);
        assert_eq!(d.poll(50, 50, 1, false, false, &mut h), None);
        assert!(!d.waiting());
    }
}
