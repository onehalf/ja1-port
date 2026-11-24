//! The tactical interface strips (docs/spec/panel.md "The interface strips (rows 0..13 and 182..199)").
//! The pieces (time meter, sector box, button label / shade, strip boxes) are the inventory drawing's (invdraw.rs).
use crate::data::Tileset;
use crate::invdraw::{button_label, button_shade, load_pcx, sector_box, strip_box, time_meter, InvData, BUTTONS};
use crate::panel::{Painter, SlotEnv, PW};
use crate::screen::slot_origin;
use crate::soldier::REC;

/// The inputs of one strips drawing.
pub struct StripsEnv<'a> {
    pub top: u8,
    pub keep: u8,
    pub opt: u8,
    pub combat: u8,
    pub sector: u8,
    /// Bytes 2 and 1 of the current sector's sector_state record.
    pub mercs: u8,
    pub enemies: u8,
    pub strip_flag: u8,
    pub selected: u8,
    pub turn_slot: u8,
    /// Record `selected` +0x138 (0 when selected >= 48).
    pub sel_in_play: u8,
    pub small_panel: i32,
    pub time_w: i32,
    // The slot redraws of step 5 ("Drawing one slot").
    pub speaker: u8,
    pub hover: u8,
    pub ap_warned: [u8; 8],
    pub faces: &'a [u8],
    pub tileset: &'a Tileset,
    /// A slot whose record has +0x17F = 1 (the switch view, not described): true = drawn as a normal slot
    /// (the front-end's frame), false = not drawn (only copied).
    pub switch_as_normal: bool,
    /// The last button element shaded by 0x10454 since the last 0x10398 (0x18 = all six in the enemies' turn), 0 = none.
    pub shade_last: i32,
}

pub struct StripsOut {
    pub load_result: i32,
    pub mouse_x: i32,
}

/// Mouse x stored by step 1.1 (no map tile under the mouse until the next poll).
pub const MOUSE_X_NONE: i32 = 3137;

/// Element number of button 0 in the button table 0x9345A.
pub const BUTTON_FIRST: i32 = 0x13;

/// Rows of the top strip that 0x10398, 0x10454 and step 1 copy to the screen (0x1180 bytes).
pub const TOP_ROWS: i32 = 14;

/// The last button that SquadLost? 0x3A454 shades (INV).
pub const LOST_LAST: i32 = 0x15;

thread_local! {
    static SHADE_LAST: std::cell::Cell<i32> = const { std::cell::Cell::new(0) };
    static LOST_SHADE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Host op 144 (0x10454 from EndTurn 0x121D8, last = 0x18; 0x12CBC) sets it, host op 148 (0x10398) and a reload of the strip
/// (QuickRestore) clear it with 0. CombatEnd reloads the strip: the frame reads 0 out of combat.
pub fn set_shade_last(last: i32) {
    SHADE_LAST.with(|c| c.set(last));
    clear_lost_shade();
}

/// SquadLost? 0x3A44A + 0x3A454: buttons up, then the shade on DONE, TRAV and INV, with combat already 0. The shade
/// stays in the panel buffer until the next strip reload, also when the sector holds mercs (live proof on #45).
pub fn set_lost_shade() {
    LOST_SHADE.with(|c| c.set(true));
}

/// A top strip reload 0x2F928 outside a fight: the shade of the squad lost move ends.
pub fn clear_lost_shade() {
    LOST_SHADE.with(|c| c.set(false));
}

/// InvScreenRun? at its end (0x54DC2: cmp [0x92DDF], 2; jge): the top strip reload 0x2F928 runs only in view
/// modes 0 and 1. The wide and full views (2, 3) keep the strip buffer, and with it the squad lost shade.
pub fn inv_end_strip(opt_onscreen: u8) {
    if opt_onscreen < 2 {
        clear_lost_shade();
    }
}

#[cfg(test)]
mod inv_end_tests {
    /// InvScreenRun? 0x54DC2..0x54DD3: view mode 0x92DDF below 2 reloads the top strip (the shade ends); 2 and 3
    /// take the other branch.
    #[test]
    fn the_inventory_end_reloads_the_strip_below_view_mode_2() {
        for (mode, left) in [(0u8, false), (1, false), (2, true), (3, true)] {
            super::set_lost_shade();
            super::inv_end_strip(mode);
            assert_eq!(super::LOST_SHADE.with(|c| c.get()), left, "view mode {mode}");
        }
        super::clear_lost_shade();
    }
}

/// The last shaded button that a tactical frame draws. A fight reads `shade_of`, and TacticalLoop 0x109CC reloads
/// the strip at its start. CombatEnd 0x106F4 reloads the strip too, so only the squad lost shade lives outside a fight.
pub fn frame_shade(combat: u8, view_hold: u8) -> i32 {
    if combat != 0 {
        clear_lost_shade();
        return shade_of(view_hold);
    }
    if LOST_SHADE.with(|c| c.get()) { LOST_LAST } else { 0 }
}

pub fn shade_last() -> i32 {
    SHADE_LAST.with(|c| c.get())
}

/// StopWalk 0x104C8 (op 72, slot, flag): only a merc's record, the selected one, in a fight. Flag != 0 and no busy
/// display: view_hold 0x92DCE = 1, turn_shown 0x92DCA = 0 (the turn cannot end), button 0 gets its shade 0x254F0 (`shade_of`).
/// Flag 0 and a busy display: 0x10310 (button 0 up with its label, turn_shown = 1), view_hold = 0.
pub fn stop_walk(slot: u8, flag: u8, combat: u8, selected: u8, view_hold: &mut u8, turn_shown: &mut u8) {
    if slot >= 8 || combat == 0 || slot != selected {
        return;
    }
    if flag != 0 && *view_hold == 0 {
        *view_hold = 1;
        *turn_shown = 0;
    } else if flag == 0 && *view_hold != 0 {
        *view_hold = 0;
        *turn_shown = 1;
    }
}

/// The last shaded button element of a frame in a fight: `shade_last`, and button 0 (0x13) during the busy display.
pub fn shade_of(view_hold: u8) -> i32 {
    shade_last().max(if view_hold != 0 { BUTTON_FIRST } else { 0 })
}

/// 0x10398 without its copy: the six buttons up, each with its label.
pub fn buttons_up(p: &mut Painter) {
    for i in 0..BUTTONS.len() {
        p.element(0x78, BUTTONS[i].0, 1, None);
        button_label(p, i);
    }
}

/// 0x10454 without its copy: the shade on the button elements 0x13..=last.
pub fn buttons_shade(p: &mut Painter, last: i32) {
    for e in BUTTON_FIRST..=last {
        button_shade(p, (e - BUTTON_FIRST) as usize);
    }
}

/// The copy at the end of 0x10398 and 0x10454: when `show` and redraw-off 0x92DDF is below 3.
pub fn buttons_copy(panel: &[u8], screen: &mut [u8], show: bool, redraw_off: u8) {
    if show && redraw_off < 3 {
        copy_rows(panel, screen, 0, TOP_ROWS);
    }
}

fn copy_rows(panel: &[u8], screen: &mut [u8], y0: i32, y1: i32) {
    let (a, b) = ((y0 * PW) as usize, (y1 * PW) as usize);
    screen[a..b].copy_from_slice(&panel[a..b]);
}

/// Steps 1..5. `p.buf` = the panel buffer, `screen` = 320 x 200, `so` = 48 x 512 records (in/out).
/// `blink` and `combat_seen` in/out; `mouse_x_in` = the stored mouse x before the call.
pub fn draw_strips(
    p: &mut Painter,
    d: &InvData,
    env: &StripsEnv,
    blink: &mut u8,
    combat_seen: &mut u8,
    mouse_x_in: i32,
    so: &mut [u8],
    screen: &mut [u8],
) -> StripsOut {
    let mut mouse_x = mouse_x_in;
    // 1.
    if env.top != 0 && env.opt < 3 {
        mouse_x = MOUSE_X_NONE;
        if env.keep == 0 {
            // Its loader result is overwritten by step 2.
            load_pcx(p, d, "TSTRIP.PCC", 0, 0);
        }
        if env.keep != 0 {
            buttons_up(p);
        } else {
            for i in 0..BUTTONS.len() {
                button_label(p, i);
            }
        }
        for i in 0..BUTTONS.len() {
            // A button that 0x10454 shades does not get the shade twice (0x10398 put it up first).
            if ((env.combat == 0 && i == 0) || (env.mercs == 0 && i <= 2)) && BUTTON_FIRST + i as i32 > env.shade_last {
                button_shade(p, i);
            }
        }
        buttons_shade(p, env.shade_last);
        copy_rows(p.buf, screen, 0, TOP_ROWS);
        time_meter(p, env.time_w, env.opt);
        sector_box(p, env.opt, env.sector, env.enemies, blink);
    }
    // 2.
    let load_result = load_pcx(p, d, "BSTRIP.PCC", 0, 182);
    // 3.
    if env.strip_flag != 0 {
        let boxes = env.opt < 3 || env.small_panel != 0;
        let s = if env.selected < 8 && env.sel_in_play != 0 {
            Some((env.selected as usize, false))
        } else if env.turn_slot < 8 && env.sel_in_play != 0 {
            Some((env.turn_slot as usize, true))
        } else {
            None
        };
        match s {
            // Boxes and copy only when S's slot +0x17A < 8 and (opt < 3 or the small panel flag != 0).
            Some((rec, dark)) => {
                if boxes && so[rec * REC + 0x17A] <= 7 {
                    for k in 0..7 {
                        strip_box(p, d, so, rec * REC, k, dark);
                    }
                    copy_rows(p.buf, screen, 182, 200);
                }
            }
            None => copy_rows(p.buf, screen, 182, 200),
        }
    }
    // 4.
    if env.combat == 0 {
        *combat_seen = 0;
    }
    // 5.
    if env.opt < 2 {
        for k in 0..8usize {
            let j = so[k * REC + 0x17A];
            if j >= 8 {
                continue;
            }
            let view = so[j as usize * REC + 0x17F];
            if env.switch_as_normal || view <= 1 {
                let se = SlotEnv {
                    slot: so[j as usize * REC + 0x17A],
                    selected: env.selected,
                    combat: env.combat,
                    opt_onscreen: env.opt,
                    speaker: env.speaker,
                    hover: env.hover,
                    ap_warned: env.ap_warned,
                    faces: env.faces,
                    tileset: env.tileset,
                };
                if env.switch_as_normal || view == 0 {
                    crate::panel::draw_slot(p, &se, so);
                } else {
                    crate::panel::draw_switch_view(p, &se, so);
                }
            }
            let (sx, sy) = slot_origin(j as usize);
            for r in 0..42 {
                let a = ((sy + r) * PW + sx) as usize;
                screen[a..a + 48].copy_from_slice(&p.buf[a..a + 48]);
            }
        }
    }
    StripsOut { load_result, mouse_x }
}

/// panel.md "Clicks on the top strip": the face of button i (0..5) up or pressed (element 0x78 / 0x79 at (x, 1),
/// then its label; pressed = one pixel left and up, fg 0x2B, shadow 2).
pub fn button_face(p: &mut Painter, i: usize, pressed: bool) {
    let (x, label) = BUTTONS[i];
    let d = pressed as i32;
    p.element(if pressed { 0x79 } else { 0x78 }, x, 1, None);
    p.text(label, x + 19 - d, 4 - d, true, 0x2A + d as u8, 3 - d as u8, 1);
}

/// soldier_update.md "Traverse to a neighbour sector", the popup: the words (text, dx, dy) and their direction codes.
pub const TRAV_WORDS: [(&[u8], i32, i32); 4] = [(b"NORTH", 28, 1), (b"WEST", 16, 11), (b"EAST", 42, 11), (b"SOUTH", 28, 21)];
pub const TRAV_CODES: [i32; 4] = [10, 13, 11, 12];

/// The popup's corner for the T key and for the TRAV button (button 1: y + 10, x − 10).
pub const TRAV_KEY_CORNER: (i32, i32) = (126, 60);
pub fn trav_button_corner() -> (i32, i32) {
    (BUTTONS[1].0 - 10, 1 + 10)
}

/// The popup at corner (x0, y0): elements 0x28 and 0x2A, the 4 words (font 0, centred; `lit` = 0xFF with shadow 8).
pub fn trav_popup(p: &mut Painter, x0: i32, y0: i32, lit: Option<usize>) {
    p.element(0x28, x0, y0, None);
    p.element(0x2A, x0, y0 + 20, None);
    for (i, &(s, dx, dy)) in TRAV_WORDS.iter().enumerate() {
        let (fg, shadow) = if lit == Some(i) { (0xFF, 8) } else { (8, 0x0E) };
        p.text(s, x0 + dx, y0 + dy + 7, true, fg, shadow, 0);
    }
}

/// The word (0..3) whose rectangle (x0 + dx − 10, y0 + 4 + dy) .. (x0 + dx + 10, y0 + 10 + dy) holds (x, y).
pub fn trav_word_at(x0: i32, y0: i32, x: i32, y: i32) -> Option<usize> {
    TRAV_WORDS.iter().position(|&(_, dx, dy)| (x0 + dx - 10..=x0 + dx + 10).contains(&x) && (y0 + 4 + dy..=y0 + 10 + dy).contains(&y))
}

/// The button (0..5) whose mouse area holds (x, y): x .. x + 36, 1 .. 12.
pub fn button_at(x: i32, y: i32) -> Option<usize> {
    (0..BUTTONS.len()).find(|&i| BUTTONS[i].0 <= x && x <= BUTTONS[i].0 + 36 && (1..=12).contains(&y))
}

/// What a released top-strip button does in the front-end (panel.md "Clicks on the top strip").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripAction {
    /// End the turn (`done`).
    EndTurn,
    /// The inventory screen (`inv`).
    Inventory,
    /// The map view (`map`).
    Map,
    /// The abort question (`abort`).
    Abort,
    /// The traverse popup (`trav`, soldier_update.md "Traverse to a neighbour sector").
    Traverse,
    /// The Control Panel (`opt`, panel.md "The Control Panel").
    Options,
    /// A button that reacts but does nothing (`inv` with hp ≤ 14, `done` out of combat).
    Nothing,
}

/// The game state the reaction rule reads.
pub struct ClickEnv {
    pub turn_shown: u8,
    pub combat: u8,
    pub selected: u8,
    /// The current sector's mercs byte (sector_state +2).
    pub mercs: u8,
    /// The selected merc's hp (0 when selected ≥ 8).
    pub sel_hp: u8,
}

/// Button i reacts to a left click (press face + click sound) or not; when it reacts, its action on release.
pub fn button_action(i: usize, e: &ClickEnv) -> Option<StripAction> {
    let merc = e.selected < 8 && e.mercs != 0;
    match i {
        0 if e.turn_shown != 0 => {
            Some(if e.combat != 0 && e.selected < 8 { StripAction::EndTurn } else { StripAction::Nothing })
        }
        1 if merc => Some(StripAction::Traverse),
        2 if merc => Some(if e.sel_hp > 14 { StripAction::Inventory } else { StripAction::Nothing }),
        3 => Some(StripAction::Map),
        4 => Some(StripAction::Options),
        5 => Some(StripAction::Abort),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Host op 144 sets the last shaded button element; a fight frame shades up to it, and a reload clears it.
    #[test]
    fn op_144_shade_reaches_the_fight_frame() {
        set_lost_shade();
        set_shade_last(0x18);
        assert_eq!(shade_last(), 0x18);
        assert_eq!(frame_shade(1, 0), 0x18);
        assert_eq!(frame_shade(0, 0), 0);
        set_shade_last(0);
        assert_eq!(frame_shade(1, 0), 0);
        assert_eq!(frame_shade(1, 1), BUTTON_FIRST);
    }

    #[test]
    fn strip_button_rule() {
        let mut e = ClickEnv { turn_shown: 0, combat: 1, selected: 2, mercs: 3, sel_hp: 15 };
        assert_eq!(button_at(49, 1), Some(0));
        assert_eq!(button_at(85, 12), Some(0));
        assert_eq!(button_at(86, 0), None);
        assert_eq!(button_at(234, 13), None);
        assert_eq!(button_action(0, &e), None);
        e.turn_shown = 1;
        assert_eq!(button_action(0, &e), Some(StripAction::EndTurn));
        assert_eq!(button_action(1, &e), Some(StripAction::Traverse));
        assert_eq!(button_action(2, &e), Some(StripAction::Inventory));
        e.sel_hp = 14;
        assert_eq!(button_action(2, &e), Some(StripAction::Nothing));
        e.selected = 8;
        assert_eq!(button_action(0, &e), Some(StripAction::Nothing));
        assert_eq!(button_action(1, &e), None);
        assert_eq!(button_action(2, &e), None);
        assert_eq!(button_action(3, &e), Some(StripAction::Map));
        assert_eq!(button_action(5, &e), Some(StripAction::Abort));
    }
}
