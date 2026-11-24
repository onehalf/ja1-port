//! The traverse popup's own loop, TraversePrompt 0x1CBA8 (soldier_update.md "Traverse to a neighbour sector").
//! The tail after the pick (CanTraverseEdge 0x30FA0, TraverseCheck 0x31344, TraverseMove 0x315DC) is `ja1_traverse`.

use crate::cursor::CursorState;
use crate::strips::trav_word_at;

/// The cursor tile "off the map" that the popup writes to 0x229D2C: before the T key's redraw and after each close.
pub const TILE_OFF: i32 = 0xC41;

/// 0x1CBA8 .. 0x1CC09: the popup opens only for selected < 8 (0x284F11), a sector with mercs (byte 2 of its row) and
/// hp (+0x183, signed) above 15.
pub fn gate(selected: u8, mercs: u8, hp: u8) -> bool {
    selected < 8 && mercs >= 1 && (hp as i8) > 15
}

/// One poll of the loop at 0x1CD4C.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The word (0 NORTH, 1 WEST, 2 EAST, 3 SOUTH) that this poll lights; a change redraws (0x1CE0F).
    pub lit: Option<usize>,
    pub redraw: bool,
    /// The loop ends with this pick.
    pub done: bool,
    pub pick: Option<usize>,
}

/// The state of the loop: the popup's corner and the word lit at the last redraw.
pub struct Prompt {
    pub x0: i32,
    pub y0: i32,
    lit: Option<usize>,
}

impl Prompt {
    pub fn new(x0: i32, y0: i32) -> Prompt {
        Prompt { x0, y0, lit: None }
    }

    /// One poll: `key` = the key that GetKey gives in this poll, `buttons` = g_mouseButtons (bit 0 left, bit 1 right).
    /// The word under the mouse takes the place of a key's pick (0x1CDBB). The redraw comes before the end test.
    pub fn poll(&mut self, mx: i32, my: i32, buttons: u8, key: Option<u8>) -> Step {
        let (mut pick, mut end) = (None, false);
        if let Some(k) = key {
            (pick, end) = match k.to_ascii_uppercase() {
                0x1B => (None, true),
                b'N' => (Some(0), true),
                b'W' => (Some(1), true),
                b'E' => (Some(2), true),
                b'S' => (Some(3), true),
                _ => (None, false),
            };
        }
        if let Some(i) = trav_word_at(self.x0, self.y0, mx, my) {
            pick = Some(i);
        }
        let redraw = pick != self.lit;
        self.lit = pick;
        let lit = pick;
        if buttons & 2 != 0 {
            return Step { lit, redraw, done: true, pick: None };
        }
        Step { lit, redraw, done: buttons & 1 != 0 || end, pick }
    }
}

/// 0x1CEED .. 0x1CFE3 without the tail: the cursor tile goes off the map. A pick, or no pick with the mouse in the
/// map window, sets over_map 0x284F13. The result: ShowMouse runs (no pick and over_map still 0).
pub fn close(c: &mut CursorState, pick: Option<usize>, in_map: bool) -> bool {
    c.tile = TILE_OFF;
    if pick.is_some() || in_map {
        c.over_map = 1;
    }
    pick.is_none() && c.over_map == 0
}
