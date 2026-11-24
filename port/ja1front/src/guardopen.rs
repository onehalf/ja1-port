//! FUN_1328C (tactical key M and the map button of the top strip): the way from the tactical screen to the guard screen and back
//! (strategic.md "The guard screen"). The window (main.rs) and the script's `gmap` line walk these lists; guardopen_diff compares
//! them with the original's calls.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// CursorItemBack? 0x53660: an item on the cursor goes back to its strip box.
    CursorBack,
    /// HideMouse? 0x67368.
    HideMouse,
    /// FadePalette? 0x52908 (steps, from, to) on the tactical palette 0xEDA08.
    Fade(i32, i32, i32),
    /// HidePathMarks? 0x1D8B8 with force 0.
    HideMarks,
    /// Mouse wobble 0x22A47E / 0x22A47C = 0, over_map 0x284F13 = 0, cursor tile 0x229D2C = 0xC41: see `mouse_off`.
    MouseOff,
    /// LoadTilePalette 0x2A990: the window keeps the tactical palette apart, so nothing is lost.
    Palette,
    /// LoadStripPccs? 0x2F928 (1, 0): the window draws the strips from the world in each frame.
    Strips,
    /// 0x1CB78 (view mode): the next frame of the window.
    Redraw,
}

/// Before GuardAssignScreen? 0x427B4.
pub const ENTER: [Step; 5] = [Step::CursorBack, Step::HideMouse, Step::Fade(24, 256, 0), Step::HideMarks, Step::MouseOff];
/// After it.
pub const LEAVE: [Step; 4] = [Step::Palette, Step::Strips, Step::Redraw, Step::Fade(24, 0, 256)];

/// Key M acts with no enemies near, or while the tactical screen is drawn (TacticalKeyHandler 0x13E68).
pub fn key_m_opens(combat: u8, screen_drawn: u8) -> bool {
    crate::tactkeys::map_opens(combat != 0, screen_drawn != 0)
}

/// `Step::MouseOff`.
pub fn mouse_off(st: &mut crate::cursor::CursorState) {
    st.wobble_dx = 0;
    st.wobble_dy = 0;
    st.over_map = 0;
    st.tile = 0xC41;
}

#[cfg(test)]
mod tests {
    /// Ledger row c2aea50:ja1front:3631: key M opens the map out of combat, and in combat only while the tactical
    /// screen is drawn (TacticalKeyHandler 0x13E68).
    #[test]
    fn key_m_needs_the_drawn_screen_in_combat() {
        assert!(super::key_m_opens(0, 0));
        assert!(super::key_m_opens(1, 1));
        assert!(!super::key_m_opens(1, 0));
    }
}
