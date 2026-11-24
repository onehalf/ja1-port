//! panel.md "Pause (key P) and the clock": the pause of the tactical screen and the blinking clock in the
//! window's top-left corner.

use crate::data::Tileset;

/// The pause state (paused, the clock switch, the lock this pause took, the blink byte and its timer).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pause {
    pub paused: bool,
    pub clock_on: bool,
    pub lock_owned: bool,
    pub blink: bool,
    pub blink_timer: i32,
    /// The count of blink steps that flipped the blink byte (the sector box flips with each one, panel.md).
    pub steps: u32,
}

impl Pause {
    /// Pause (on): the lock and the clock on, or off. Returns true when the screen must be drawn again (on).
    pub fn set(&mut self, on: bool, round_lock: &mut u8) -> bool {
        if on && !self.paused {
            self.lock(true, round_lock);
            self.paused = true;
            return true;
        }
        // 0x1002C: a second call with "on" also ends the pause.
        if self.paused {
            self.unlock(round_lock);
            self.paused = false;
        }
        false
    }

    /// Lock (clock switch `clock`): the round lock when it is free, then the blink byte on for 800 ms.
    /// The original's Lock? 0x4B730 (panel.md "Other users of Lock").
    pub fn lock(&mut self, clock: bool, round_lock: &mut u8) {
        if *round_lock == 0 {
            *round_lock = 1;
            self.lock_owned = true;
            self.clock_on = clock;
        }
        self.blink = true;
        self.blink_timer = 800;
    }

    /// Unlock: frees the round lock when this lock took it; clock switch off.
    /// The original's Unlock? 0x4B764.
    pub fn unlock(&mut self, round_lock: &mut u8) {
        if self.lock_owned {
            *round_lock = 0;
        }
        self.clock_on = false;
        self.lock_owned = false;
    }

    /// One timer tick (the timer lowers blink_timer by 10). TacticalTimer? 0x5EEFE `sub edx, 0xa` on the dword
    /// 0x229D48 wraps at 32 bits (overflow-check probe, sticky #1 c12726).
    pub fn tick(&mut self) {
        self.blink_timer = self.blink_timer.wrapping_sub(10);
    }

    /// The loop's blink step: at blink_timer <= 0 the blink byte flips (800 on, 400 off).
    pub fn pass(&mut self) {
        if self.blink_timer <= 0 {
            self.blink = !self.blink;
            self.blink_timer = if self.blink { 800 } else { 400 };
            self.steps = self.steps.wrapping_add(1);
        }
    }

    /// The clock shows (clock switch and blink byte on).
    pub fn clock_shows(&self) -> bool {
        self.clock_on && self.blink
    }
}

/// The clock at back buffer point (x, y): 8 tile bank sprites in 2 columns of 4 (16 x 14 each), then the time
/// `%2d:%02d` (hour, minute; signed bytes) by Text at (x + 5, y + 27), fg 1, shadow 0, font 0.
#[allow(clippy::too_many_arguments)]
pub fn clock(bb: &mut [u8], ts: &Tileset, font0: &crate::panel::Font, table0: &mut [u8; 256], x: i32, y: i32,
             hour: i8, minute: i8) {
    for k in 0..8 {
        let n = if k == 7 { 413 + 0xD1 } else { 413 + 0xE1 + k };
        let (dx, dy) = if k < 4 { (0, 14 * k as i32) } else { (16, 14 * (k as i32 - 4)) };
        crate::overview::tile_sprite(bb, ts, n, x + dx, y + dy);
    }
    let s = format!("{:2}:{:02}", hour, minute);
    crate::cursor::text(bb, font0, table0, s.as_bytes(), x + 5, y + 27, false, 1, 0, true);
}

#[cfg(test)]
mod tests {
    use super::Pause;

    #[test]
    fn lock_owns_only_a_free_lock() {
        let mut p = Pause::default();
        let mut rl = 0u8;
        p.lock(false, &mut rl);
        assert_eq!((rl, p.lock_owned, p.clock_on, p.blink, p.blink_timer), (1, true, false, true, 800));
        p.unlock(&mut rl);
        assert_eq!((rl, p.lock_owned), (0, false));
        // a lock that someone else holds stays held
        rl = 1;
        p.lock(true, &mut rl);
        assert_eq!((rl, p.lock_owned, p.clock_on), (1, false, false));
        p.unlock(&mut rl);
        assert_eq!(rl, 1);
    }

    /// Live on #31 (fight59): a message box that shows during a pause frees the round lock and takes the clock away;
    /// the pause stays on, so the next key only ends it.
    #[test]
    fn a_box_in_a_pause_frees_the_lock() {
        let mut p = Pause::default();
        let mut rl = 0u8;
        assert!(p.set(true, &mut rl));
        assert_eq!((rl, p.paused, p.clock_on), (1, true, true));
        p.lock(false, &mut rl);
        p.unlock(&mut rl);
        assert_eq!((rl, p.paused, p.clock_on, p.lock_owned), (0, true, false, false));
        assert!(!p.set(false, &mut rl));
        assert_eq!((rl, p.paused), (0, false));
    }

    #[test]
    fn blink_steps_come_after_800_and_400_ms() {
        // panel.md "The front-end's tactical screen": the sector box flips at each blink step.
        let mut p = Pause::default();
        let mut at = vec![];
        for ms in (0..=2400).step_by(10) {
            let n = p.steps;
            p.pass();
            if p.steps != n {
                at.push(ms);
            }
            p.tick();
        }
        assert_eq!(at, [0, 800, 1200, 2000, 2400]);
    }
}
