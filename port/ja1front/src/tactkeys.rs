//! The key part of TacticalKeyHandler 0x13490: which action one key press of the tactical screen starts, and which
//! gates it passes. The window (main.rs) turns an SDL key into the DOS key bytes (`dos_key`) and acts on the `Route`.
//! keyhandler_diff compares `route` with the original for every key byte under fuzzed gates.
//!
//! Order in the original:
//! 1. View mode != 0: any key clears the spotted mark and the view tile mark (0x134F8), also a key that only ends a pause.
//! 2. Paused (0x22A129): the key only ends the pause. An extended key eats its second byte (0x13526).
//! 3. Time compression on (0x9300C): the key only ends it, in the same way (0x13546). Key C has no special rule here.
//! 4. An extended key goes through the scan code compares (0x13563), any other key through toupper + the jump table
//!    0x13328 (0x139BE).
//!
//! The number keys have no arm in the jump table. F1..F8 and Alt+F1..F8 choose a merc (0x136A3..0x13768).

/// The state that the key arms read.
#[derive(Clone, Copy, Debug, Default)]
pub struct Gates {
    /// 0x284F11.
    pub selected: u8,
    /// 0x92DCB, enemies near.
    pub combat: bool,
    /// 0x92DCC, the tactical screen is drawn.
    pub drawn: bool,
    /// 0x92DCA, the player's turn shows.
    pub turn_shown: bool,
    /// 0x92DCE.
    pub view_hold: bool,
    /// 0x22A129.
    pub paused: bool,
    /// 0x9300C.
    pub compress: bool,
    /// 0x222B50 g_cursorItem: an item is on the cursor.
    pub held: bool,
    /// 0x955EC, the debug level.
    pub level: u8,
    /// 0x92DC3, the save-anywhere byte.
    pub save_anywhere: bool,
    /// 0x92DDF, the view mode.
    pub mode: u8,
    /// QUICK.SAV exists.
    pub quick_file: bool,
    /// +0x138 of the selected merc.
    pub in_play: bool,
    /// +0x183 of the selected merc, signed.
    pub hp: i8,
    /// 0x92DD0 turn_moves: an enemy won an interrupt and waits in the queue (read only by port fix P21).
    pub turn_moves: bool,
}

/// What one key press does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    None,
    /// The key only ends the pause.
    EndPause,
    /// The key only ends time compression.
    EndCompress,
    /// Alt+X, ExitToDosPrompt (1).
    AskQuit,
    /// Alt+S, QuickSave.
    QuickSave,
    /// Alt+R, QuickRestore (1).
    QuickRestore,
    /// A level key, or a level-4 key (debugkeys codes: a char, or 0x100 + scan code).
    Debug(u16),
    /// F1..F8, Alt+F1..F8: SelectMerc (k, Alt, 1).
    Merc(u8),
    /// F9 with selected < 8.
    SeesLine,
    /// F10 or O.
    Options,
    /// Up, Left, Right, Down: the scan code.
    Arrow(u8),
    /// PgUp 1, PgDn 2, Insert 3, Tab 4.
    View(u8),
    /// Ctrl+F with selected >= 8: NewTurn (1), then turn_shown = 1.
    ForceTurn,
    /// Enter.
    NextEnemy,
    /// Escape.
    Escape,
    /// Space.
    NextMerc,
    /// `+` `-` `.` `,`.
    Volume(u8),
    /// A.
    Abort,
    /// C out of combat.
    Compress,
    /// C in combat: only the status line.
    NoCompress,
    /// D with its gates: EndTurn.
    EndTurn,
    /// D without them: only the cursor item goes back.
    CursorBack,
    /// I.
    Inventory,
    /// L.
    SayAgain,
    /// M.
    Map,
    /// P.
    Pause,
    /// S.
    StatPopups,
    /// T with selected < 8.
    Traverse,
    /// V.
    Version,
    /// X.
    Swap,
    /// Z outside the overview.
    Popup,
}

impl Route {
    /// CursorItemBack? 0x53660 runs first.
    pub fn back(self) -> bool {
        matches!(self, Route::Options | Route::Abort | Route::EndTurn | Route::CursorBack | Route::Traverse)
    }

    /// CursorItemBack? 0x53660 runs first, with fan fix F67 (#70 row 86; 1.15m caves 0x8AF43 / 0x8AF5B / 0x8AF70):
    /// also before Quick Restore (Alt+R), Quick Save (Alt+S) and Exit to DOS (Alt+X), so a held item is in its
    /// merc's pocket when the game saves or loads (changelog 1.15 jan 08, found by DarzaR).
    pub fn back_fan(self, fan67: bool) -> bool {
        self.back() || fan67 && matches!(self, Route::AskQuit | Route::QuickSave | Route::QuickRestore)
    }

    /// Fan fix F67: LoadStripPccs? 0x2F928 (1, 0) after the box of the key (1.15m caves 0x8AF11 Control Panel,
    /// 0x8AF21 traverse, 0x8AF31 abort day, 0x8AF43 / 0x8AF5B / 0x8AF70 quick keys and Exit to DOS).
    pub fn strip_reload(self, fan67: bool) -> bool {
        fan67 && matches!(self, Route::Options | Route::Traverse | Route::Abort | Route::AskQuit | Route::QuickSave | Route::QuickRestore)
    }
}

/// Key M's rule (0x13E68), also the rule of the guard screen way in the window.
pub fn map_opens(combat: bool, drawn: bool) -> bool {
    !combat || drawn
}

/// Step 1 of the order above: this key press clears the spotted mark and the view tile mark.
pub fn clears_marks(mode: u8) -> bool {
    mode != 0
}

/// One key press: `first` = the first key byte, `second` = the second byte of an extended key (`first` = 0).
pub fn route(first: u8, second: u8, g: &Gates) -> Route {
    if g.paused {
        return Route::EndPause;
    }
    if g.compress {
        return Route::EndCompress;
    }
    if first == 0 {
        extended(second, g)
    } else {
        plain(first, g)
    }
}

fn extended(scan: u8, g: &Gates) -> Route {
    let debug = |code: u16| if g.level == 4 { Route::Debug(code) } else { Route::None };
    match scan {
        0x10 | 0x12 | 0x26 | 0x5E => debug(0x100 + scan as u16),
        0x13 if g.quick_file => Route::QuickRestore,
        0x1F if !g.combat || g.save_anywhere => Route::QuickSave,
        0x2D => Route::AskQuit,
        0x3B..=0x42 => Route::Merc(scan - 0x3B),
        0x68..=0x6F => Route::Merc(scan - 0x68),
        0x43 if g.selected < 8 => Route::SeesLine,
        0x44 => Route::Options,
        0x48 | 0x4B | 0x4D | 0x50 => Route::Arrow(scan),
        0x49 => Route::View(1),
        0x51 => Route::View(2),
        0x52 => Route::View(3),
        _ => Route::None,
    }
}

fn plain(key: u8, g: &Gates) -> Route {
    let merc = g.selected < 8;
    let may_act = !g.combat || g.drawn;
    let debug = |code: u8| if g.level == 4 { Route::Debug(code as u16) } else { Route::None };
    match key.to_ascii_uppercase() {
        // Ctrl+B, Ctrl+A, Ctrl+L, Ctrl+D: the level keys.
        k @ (1 | 2 | 4 | 12) => Route::Debug(k as u16),
        6 if !merc => Route::ForceTurn,
        9 => Route::View(4),
        13 if merc && g.combat && g.drawn => Route::NextEnemy,
        k @ (15 | 16 | 18 | 20 | b'@' | b'E' | b'F' | b'G' | b'K') => debug(k),
        27 if merc && may_act => Route::Escape,
        b' ' if (!g.combat || (g.drawn && !g.view_hold)) && !g.held => Route::NextMerc,
        k @ (b'+' | b',' | b'-' | b'.') => Route::Volume(k),
        b'A' => Route::Abort,
        b'C' if g.combat => Route::NoCompress,
        b'C' => Route::Compress,
        // Port fix P21 (#71, #2 row 63): no EndTurn while an enemy interrupt waits (turn_moves 0x92DD0 != 0); in 1.13b
        // EndTurn pops that enemy, leaves turn_moves = 1, and the enemies' turn stops for good after its shot.
        b'D' if g.combat && g.drawn && g.turn_shown && !(g.turn_moves && ja1logic::fanfix::port_on(21)) => Route::EndTurn,
        b'D' => Route::CursorBack,
        b'I' if merc && g.in_play && may_act && g.hp >= 15 => Route::Inventory,
        b'L' => Route::SayAgain,
        b'M' if map_opens(g.combat, g.drawn) => Route::Map,
        b'O' => Route::Options,
        b'P' => Route::Pause,
        b'S' => Route::StatPopups,
        b'T' if merc => Route::Traverse,
        b'V' => Route::Version,
        b'X' if merc && may_act => Route::Swap,
        b'Z' if g.mode != 0 => Route::Popup,
        _ => Route::None,
    }
}

/// The scan code of a letter, for Alt + letter (the BIOS gives 0 + scan code).
fn letter_scan(upper: u8) -> u8 {
    const ROWS: [(&[u8], u8); 3] = [(b"QWERTYUIOP", 0x10), (b"ASDFGHJKL", 0x1E), (b"ZXCVBNM", 0x2C)];
    for (row, base) in ROWS {
        if let Some(i) = row.iter().position(|&c| c == upper) {
            return base + i as u8;
        }
    }
    0xFF
}

/// The DOS key bytes of a key: `key` = the SDL key code (ASCII below 128), `f` = 1..12 for F1..F12 (else 0),
/// `scan` = the scan code of an extended key without a modifier rule (else 0). None: a key with no BIOS key, as Shift.
pub fn dos_key(key: i32, f: u8, scan: u8, alt: bool, ctrl: bool, shift: bool) -> Option<(u8, u8)> {
    if (1..=10).contains(&f) {
        // F1 0x3B, Shift+F1 0x54, Ctrl+F1 0x5E, Alt+F1 0x68.
        let base = if alt { 0x68 } else if ctrl { 0x5E } else if shift { 0x54 } else { 0x3B };
        return Some((0, base + f - 1));
    }
    if scan != 0 {
        return Some((0, scan));
    }
    if !(1..128).contains(&key) {
        return None;
    }
    let c = key as u8;
    if c.is_ascii_lowercase() {
        let up = c.to_ascii_uppercase();
        return Some(if alt {
            (0, letter_scan(up))
        } else if ctrl {
            (up - b'A' + 1, 0)
        } else {
            (up, 0)
        });
    }
    if alt {
        return Some((0, 0xFF));
    }
    // BIOS: Ctrl+[ gives byte 27, the byte of Escape (live in the original, issue #31: it stops a walk).
    if ctrl && c == b'[' {
        return Some((27, 0));
    }
    // Live in the original (issue #31): Ctrl+- and Ctrl+= or Ctrl++ reach no volume arm; Ctrl+. and Ctrl+, act as the plain keys.
    if ctrl && c == b'-' {
        return Some((0x1F, 0));
    }
    if ctrl && c == b'=' {
        return None;
    }
    Some((match (c, shift) {
        (b'2', true) => b'@',
        (b'=', true) => b'+',
        (b'-', true) => b'_',
        (b',', true) => b'<',
        (b'.', true) => b'>',
        _ => c,
    }, 0))
}

/// The state that the key arms of TacticalKeyHandler read.
pub fn gates_of(w: &crate::world::World, paused: bool, quick_file: bool) -> Gates {
    let rec = (w.selected < 8).then(|| w.selected as usize * crate::soldier::REC);
    Gates {
        selected: w.selected,
        combat: w.combat != 0,
        drawn: w.update.screen_drawn != 0,
        turn_shown: w.update.turn_shown != 0,
        view_hold: w.update.view_hold != 0,
        paused,
        compress: w.update.time_compress != 0,
        held: w.held[0] != 0,
        level: crate::debugkeys::level(),
        save_anywhere: crate::debugkeys::save_anywhere(),
        mode: w.opt_onscreen,
        quick_file,
        in_play: rec.is_some_and(|r| w.soldiers[r + 0x138] != 0),
        hp: rec.map_or(0, |r| w.soldiers[r + 0x183] as i8),
        turn_moves: w.update.turn_moves != 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row c2aea50:ja1front:2440: the key gates read the selected merc's own record (in play +0x138, health
    /// +0x183), and none when no merc is selected.
    #[test]
    fn the_gates_read_the_selected_merc() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = crate::world::World::from_files(&exe, &jag, &save).unwrap();
        let r = crate::soldier::REC;
        for k in 0..8 {
            (w.soldiers[k * r + 0x138], w.soldiers[k * r + 0x183]) = (0, 50 + k as u8);
        }
        w.soldiers[2 * r + 0x138] = 1;
        w.selected = 2;
        let g = gates_of(&w, false, false);
        assert_eq!((g.selected, g.in_play, g.hp), (2, true, 52));
        w.selected = 3;
        let g = gates_of(&w, false, false);
        assert_eq!((g.in_play, g.hp), (false, 53));
        w.selected = 0xFF;
        let g = gates_of(&w, false, false);
        assert_eq!((g.in_play, g.hp), (false, 0));
    }

    #[test]
    fn head_order() {
        let g = Gates { paused: true, compress: true, ..Gates::default() };
        assert_eq!(route(b'C', 0, &g), Route::EndPause);
        let g = Gates { compress: true, ..Gates::default() };
        assert_eq!(route(b'C', 0, &g), Route::EndCompress);
        assert_eq!(route(0, 0x2D, &g), Route::EndCompress);
    }

    #[test]
    fn dos_bytes() {
        assert_eq!(dos_key(b'[' as i32, 0, 0, false, true, false), Some((27, 0)));
        assert_eq!(dos_key(b'[' as i32, 0, 0, false, false, false), Some((b'[', 0)));
        assert_eq!(dos_key(b'-' as i32, 0, 0, false, true, false), Some((0x1F, 0)));
        assert_eq!(dos_key(b'=' as i32, 0, 0, false, true, true), None);
        assert_eq!(dos_key(b'.' as i32, 0, 0, false, true, false), Some((b'.', 0)));
        assert_eq!(dos_key(b'c' as i32, 0, 0, false, false, true), Some((b'C', 0)));
        assert_eq!(dos_key(b's' as i32, 0, 0, true, false, false), Some((0, 0x1F)));
        assert_eq!(dos_key(b'x' as i32, 0, 0, true, false, false), Some((0, 0x2D)));
        assert_eq!(dos_key(b'b' as i32, 0, 0, false, true, false), Some((2, 0)));
        assert_eq!(dos_key(0, 1, 0, false, true, false), Some((0, 0x5E)));
        assert_eq!(dos_key(0, 8, 0, true, false, false), Some((0, 0x6F)));
        assert_eq!(dos_key(b'2' as i32, 0, 0, false, false, true), Some((b'@', 0)));
        assert_eq!(dos_key(0, 0, 0, false, false, true), None);
    }
}
