//! #64 step 3: the DOS mouse mode of the window. The original reads a captured DOS mouse: JA.EXE never sets the mickey
//! ratio (0x683A8 / 0x683D8 have no callers) and takes game x = driver x / 2 (0x6809A..0x680A7), so one mouse count
//! moves the pointer half a game pixel on both axes, with no acceleration. The mouse cannot leave the screen, so an
//! edge push holds. MousePoll 0x678E8 draws the pointer into the picture only when its place changed.

/// Game pixels per mouse count (driver x / 2, the DOSBox-X default of 1 count per driver unit).
pub const GAME_PX_PER_COUNT: f32 = 0.5;

/// SDL's relative speed scale (window pixels per mouse count) for a picture `fw` window pixels wide that shows `sw`
/// game pixels.
pub fn speed_scale(fw: u32, sw: u32) -> f32 {
    fw as f32 / sw.max(1) as f32 * GAME_PX_PER_COUNT
}

/// The window place inside the picture rectangle `(fx, fy, fw, fh)`: the DOS mouse stops at the screen edge.
pub fn pin((fx, fy, fw, fh): (i32, i32, u32, u32), x: i32, y: i32) -> (i32, i32) {
    (x.clamp(fx, fx + fw.max(1) as i32 - 1), y.clamp(fy, fy + fh.max(1) as i32 - 1))
}

/// What the pointer drawing shows: the game pixel, the shape index, and the pointer on or off.
pub type Drawn = (i32, i32, usize, bool);

/// MousePoll 0x678E8: the pointer is drawn again only when its place (or its shape, or its on/off) changed.
pub fn needs_draw(last: Option<Drawn>, now: Drawn) -> bool {
    last != Some(now)
}

/// One pointer line of the `JA1_FRAMES` record (`PATH.pointer`, #64 step 2): ms on the frame clock, the game pixel
/// x y, the shape index and on (1) or off (0). A frame plus the newest pointer line at or before its ms gives the
/// picture with the pointer, as the DOS mouse mode shows it.
pub fn pointer_line(ms: u32, (x, y, k, on): Drawn) -> String {
    format!("{} {} {} {} {}\n", ms, x, y, k, on as u8)
}

/// The pointer record writes a line only when the drawn pointer changed (`last` = the newest written state).
pub fn pointer_record(last: &mut Option<Drawn>, ms: u32, now: Drawn) -> Option<String> {
    if !needs_draw(*last, now) {
        return None;
    }
    *last = Some(now);
    Some(pointer_line(ms, now))
}

/// Ctrl+F10 (the DOSBox-X key) releases the captured mouse to the desktop; a click in the window captures it again.
pub fn release_key(f10: bool, ctrl: bool) -> bool {
    f10 && ctrl
}

/// A captured mouse cannot leave the window: a leave event (a focus change) counts only with the mouse released.
pub fn leave_counts(captured: bool) -> bool {
    !captured
}

/// MousePoll 0x678E8 draws the pointer into the 8-bit picture, so the screen's palette gives its colours (#64 rig:
/// main menu arrow (60, 89, 130), office arrow (247, 0, 0)). `pix` = colour indexes of the shape, 0 = not drawn;
/// `pal` = the 8-bit palette of the shown picture. RGBA bytes, alpha 0 where not drawn.
pub fn pointer_rgba(pix: &[u8], pal: &[u8; 768]) -> Vec<u8> {
    pix.iter().flat_map(|&p| {
        let i = p as usize * 3;
        if p == 0 { [0, 0, 0, 0] } else { [pal[i], pal[i + 1], pal[i + 2], 0xFF] }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #64 rig: the pointer takes the colours of the shown picture's palette; colour 0 is not drawn.
    #[test]
    fn the_pointer_takes_the_palette_of_the_picture() {
        let mut menu = [0u8; 768];
        menu[3 * 7..3 * 7 + 3].copy_from_slice(&[60, 89, 130]);
        menu[3 * 9..3 * 9 + 3].copy_from_slice(&[1, 2, 3]);
        let mut office = [0u8; 768];
        office[3 * 7..3 * 7 + 3].copy_from_slice(&[247, 0, 0]);
        assert_eq!(pointer_rgba(&[0, 7, 9], &menu), vec![0, 0, 0, 0, 60, 89, 130, 255, 1, 2, 3, 255]);
        assert_eq!(pointer_rgba(&[7, 0], &office), vec![247, 0, 0, 255, 0, 0, 0, 0]);
    }

    #[test]
    fn pointer_lines_hold_place_shape_and_on() {
        assert_eq!(pointer_line(1234, (17, 199, 3, true)), "1234 17 199 3 1\n");
        assert_eq!(pointer_line(5, (0, 4, 0, false)), "5 0 4 0 0\n");
        let mut last = None;
        assert_eq!(pointer_record(&mut last, 10, (5, 6, 1, true)).as_deref(), Some("10 5 6 1 1\n"));
        assert_eq!(pointer_record(&mut last, 20, (5, 6, 1, true)), None);
        assert_eq!(pointer_record(&mut last, 30, (5, 7, 1, true)).as_deref(), Some("30 5 7 1 1\n"));
        assert_eq!(pointer_record(&mut last, 40, (5, 7, 2, true)).as_deref(), Some("40 5 7 2 1\n"));
        assert_eq!(pointer_record(&mut last, 50, (5, 7, 2, false)).as_deref(), Some("50 5 7 2 0\n"));
    }

    /// 100 counts move the pointer 50 game pixels, at any window size.
    #[test]
    fn a_hundred_counts_are_fifty_pixels() {
        for fw in [320u32, 1280, 1920] {
            let window_px = 100.0 * speed_scale(fw, 320);
            assert_eq!((window_px * 320.0 / fw as f32).round() as i32, 50, "frame width {}", fw);
        }
    }

    /// The pointer stops at the picture edge; it stays there while the push goes on, so the edge scroll holds.
    #[test]
    fn the_edge_pins_the_pointer_and_holds_the_scroll() {
        let r = (160, 0, 1280, 800);
        assert_eq!(pin(r, 3000, 400), (1439, 400));
        assert_eq!(pin(r, -50, -9), (160, 0));
        assert_eq!(pin(r, 700, 300), (700, 300));
        assert!(!leave_counts(true));
        assert!(leave_counts(false));
    }

    /// No draw while the place, the shape and the on/off stay the same.
    #[test]
    fn no_draw_when_the_place_does_not_change() {
        assert!(needs_draw(None, (10, 20, 0, true)));
        assert!(!needs_draw(Some((10, 20, 0, true)), (10, 20, 0, true)));
        assert!(needs_draw(Some((10, 20, 0, true)), (11, 20, 0, true)));
        assert!(needs_draw(Some((10, 20, 0, true)), (10, 21, 0, true)));
        assert!(needs_draw(Some((10, 20, 0, true)), (10, 20, 1, true)));
        assert!(needs_draw(Some((10, 20, 0, true)), (10, 20, 0, false)));
    }

    /// Only Ctrl+F10 releases the mouse.
    #[test]
    fn ctrl_f10_releases_the_mouse() {
        assert!(release_key(true, true));
        assert!(!release_key(true, false));
        assert!(!release_key(false, true));
    }
}
