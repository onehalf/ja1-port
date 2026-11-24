//! The tactical screen (docs/spec/panel.md "The tactical screen"): the 8 merc slots and the map view in one
//! 320 x 200 picture, the face buffers filled at sector entry, and the mouse areas of the slots.
use crate::panel::{self, Painter, SlotEnv, PH, PW};
use crate::soldier::REC;
use crate::world::World;
use crate::{common, tileset_data};

/// Where the map view goes in the screen (the view is 14 x 12 tiles = 224 x 168 pixels).
pub const VIEW_X: i32 = 48;
pub const VIEW_Y: i32 = 14;

/// Slot origin of slot k (0..7).
pub fn slot_origin(k: usize) -> (i32, i32) {
    (if k < 4 { 0 } else { 272 }, 14 + 42 * (k as i32 % 4))
}

/// Health-bar offset of a column (soldier +0x180: 0 left, else right).
fn bar_offset(column: u8) -> (i32, i32) {
    if column == 0 { (3, 4) } else { (35, 4) }
}

/// What a screen pixel (x, y) is: a slot's bar area (11 x 27 at the health-bar offset), another part of a slot,
/// the map view (view pixel), or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spot {
    Bars(u8),
    Slot(u8),
    View(i32, i32),
    None,
}

/// `mode` = opt_onscreen (panel.md "View modes"): the slots are there only in modes 0 and 1; the view window is the
/// mode's.
pub fn spot(soldiers: &[u8], x: i32, y: i32, view_w: i32, view_h: i32, mode: u8) -> Spot {
    // TacticalKeyHandler 0x14650: at x 0 and x 319 the mouse scrolls the view and no slot takes it.
    for k in 0..if mode < 2 && (1..=318).contains(&x) { 8usize } else { 0 } {
        let (sx, sy) = slot_origin(k);
        if x < sx || x >= sx + 48 || y < sy || y >= sy + 42 {
            continue;
        }
        let (bx, by) = bar_offset(soldiers[k * REC + 0x180]);
        if x >= sx + bx && x < sx + bx + 11 && y >= sy + by && y < sy + by + 27 {
            return Spot::Bars(k as u8);
        }
        return Spot::Slot(k as u8);
    }
    let (_, _, left, top) = crate::viewmode::window(mode, false);
    let (vx, vy) = (x - left, y - top);
    if vx >= 0 && vy >= 0 && vx < view_w && vy < view_h {
        return Spot::View(vx, vy);
    }
    Spot::None
}

/// The bar-area press rule (panel.md "The front-end's tactical screen"): a left button press on the bar area of
/// slot k (= the hover slot) with merc k on the map (+0x138) and hp (+0x183, signed) above 0 starts the stat popup.
pub fn popup_press(soldiers: &[u8], hover: u8, at: Spot) -> Option<u8> {
    match at {
        Spot::Bars(k) if k == hover && k < 8 => {
            let r = &soldiers[k as usize * REC..(k as usize + 1) * REC];
            crate::slotmouse::left_acts(r).then_some(k)
        }
        _ => None,
    }
}

/// panel.md "The switch view": the right button on slot k opens it when opt_onscreen < 2 and merc k is in the
/// sector (+0x182), not shaded (+0x194 = 0), on the map (+0x138), on no assignment (+0x17E = 0) and hp ≠ 0.
pub fn switch_can_open(w: &World, k: u8) -> bool {
    if k >= 8 || !panel::slot_view_runs(w.opt_onscreen) || !crate::slotmouse::pass_runs(w.update.screen_drawn) {
        return false;
    }
    let r = &w.soldiers[k as usize * REC..(k as usize + 1) * REC];
    crate::slotmouse::slot_takes_mouse(r) && crate::slotmouse::right_opens(r)
}

/// Key S of the tactical key handler (panel.md "Key S: all stat popups"): the slots whose stat popup it shows, in
/// slot order: merc k in the current sector (+0x182) and on the map (+0x138).
pub fn stat_all(soldiers: &[u8]) -> Vec<u8> {
    (0..8u8).filter(|&k| { let r = &soldiers[k as usize * REC..(k as usize + 1) * REC]; r[0x182] != 0 && r[0x138] != 0 }).collect()
}

/// ShowAllStatPopups 0x40024 up to its wait: the slots of `stat_all`, and over_map 0x284F13 = 0.
pub fn key_s(soldiers: &[u8], cursor_over_map: &mut u8) -> Vec<u8> {
    let ks = stat_all(soldiers);
    *cursor_over_map = 0;
    ks
}

/// The face step of 0x2884C for one soldier record (QuickRestore, LoadGame): None = not in the current sector, the
/// buffer is cleared (0x46AC4); else the LoadMercFace 0x286C4 kind: 0 alive, 1 dead (+0x183 <= 0).
pub fn face_kind(s: &[u8]) -> Option<i32> {
    if s[0x182] == 0 {
        None
    } else if s[0x183] as i8 > 0 {
        Some(0)
    } else {
        Some(1)
    }
}

/// 0x46AC4: clears the first 0x380 bytes of a face buffer. The last 128 bytes (rows 28..31) keep their old values.
pub fn clear_face(face: &mut [u8]) {
    face[..0x380].fill(0);
}

/// The panel state the front-end keeps across frames.
pub struct Screen {
    /// 8 x 1024 face buffers (32 x 32), slot order.
    pub faces: Vec<u8>,
    /// Remap table 0 as the drawing leaves it.
    pub table0: [u8; 256],
    /// Per slot: the face buffer holds an alive face (a death in the sector then loads the morph face).
    alive: [bool; 8],
    /// Per slot: the profile whose face the buffer holds (a team change, e.g. a fired merc, loads the faces again).
    profiles: [u8; 8],
    /// Per slot: the frame picture in the face buffer (panel::LIGHT_OFF, LIGHT_ON).
    lit: [u8; 8],
    /// The bar area under the mouse (0xFF none).
    pub hover: u8,
    /// SlotMouse 0x92DC8: the slot whose face shows "cont" (0x40 none).
    pub cont: u8,
    /// The front-end's combat seen byte of the interface strips (start 0).
    pub combat_seen: u8,
    /// The stored mouse x of the strips' step 1.1 (not used by the front-end's mouse).
    pub mouse_x: i32,
    /// The top-strip button held down (panel.md "Clicks on the top strip": the pressed face until release).
    pub pressed: Option<usize>,
    /// The tile cursor (cursor.md); over_map = 1 while the mouse is over the map window.
    pub cursor: crate::cursor::CursorState,
    /// The overview cursor's values of the play loop (panel.md "The overview picture" step 4), view mode 0.
    pub ov_cursor: Option<crate::overview::OvCursor>,
}

/// LoadStripPccs? 0x2F928 step 3 reads the in-play byte +0x138 of record `selected`.
/// panel.md "Selected ≥ 8 in step 3": record 64's in-play byte is byte 0 of tile 1039 (the tile map follows).
fn sel_in_play(w: &World, inv: &[u8]) -> u8 {
    let sel = w.selected as usize;
    match sel {
        0..48 => w.soldiers[sel * REC + 0x138],
        64 => w.tiles.get(1039 * 8).copied().unwrap_or(0),
        // record 255's in-play byte lies in the loaded INV.DAT (file byte 0x44C8, a picture byte: 0xDF).
        255 => inv.get(0x44C4).copied().unwrap_or(0),
        _ => 0,
    }
}

impl Screen {
    /// Sector entry: the face buffer of each of the 8 slots from the merc's face number (`profileId` +0x179):
    /// alive (hp > 0, the eye row +0x1AA from JAGGED.DAT) or dead; a record not in the sector (+0x182 = 0) keeps a
    /// cleared face buffer.
    pub fn new(iso: &str, exe: &str, w: &mut World) -> Result<Screen, i32> {
        let c = common(iso)?;
        let d = panel::panel_data(iso, exe)?;
        let mut faces = vec![0u8; 8 * 1024];
        let mut alive = [false; 8];
        let mut profiles = [0u8; 8];
        for k in 0..8usize {
            let s = &mut w.soldiers[k * REC..(k + 1) * REC];
            let n = s[0x179] as i32;
            profiles[k] = s[0x179];
            let face = &mut faces[k * 1024..(k + 1) * 1024];
            match face_kind(s) {
                // Not in the current sector (an empty place after a merc left the team): the face stays cleared.
                None => continue,
                Some(0) => {
                    if let Some(eye) = panel::load_face(iso, &d, n, 0, face)? {
                        s[0x1AA] = eye;
                    }
                    alive[k] = true;
                }
                Some(kind) => {
                    panel::load_face(iso, &d, n, kind, face)?;
                }
            }
        }
        // LoadTacticalSector? 0x2B127 / QuickRestore 0x2D7C6: frame picture 5 on all 8 buffers, then 0xC on the selected
        // slot (0x2B142: the sector has mercs).
        let mercs = w.sector_state.get(w.cur_sector as usize * 9 + 2).copied().unwrap_or(0);
        let mut lit = [panel::LIGHT_OFF; 8];
        if (w.selected as usize) < 8 && mercs != 0 {
            lit[w.selected as usize] = panel::LIGHT_ON;
        }
        for k in 0..8 {
            panel::face_light(&d, &c.remap[0], &mut faces[k * 1024..(k + 1) * 1024], lit[k]);
        }
        panel::take_face_lights();
        Ok(Screen { faces, table0: c.remap[0], alive, profiles, lit, hover: 0xFF, cont: 0x40, combat_seen: 0, mouse_x: 0, pressed: None, cursor: Default::default(), ov_cursor: None })
    }

    /// panel.md "Faces": frame i of slot k's death morph into its face buffer (the window plays frames 0..9 at a loud
    /// death); the buffer then counts as a dead face.
    pub fn morph_frame(&mut self, iso: &str, w: &World, k: usize, i: usize) -> Result<(), i32> {
        if k >= 8 {
            return Err(crate::E_ARG);
        }
        let n = w.soldiers[k * REC + 0x179] as i32;
        panel::load_morph(iso, n, i, &mut self.faces[k * 1024..(k + 1) * 1024])?;
        self.alive[k] = false;
        Ok(())
    }

    /// A merc who died since the last call gets the death-morph face (the buffer keeps its last frame). A slot
    /// whose profile changed (the team closed up after a merc left) gets its face as at the sector entry.
    pub fn update_faces(&mut self, iso: &str, exe: &str, w: &World) -> Result<(), i32> {
        let lights = panel::take_face_lights();
        for k in 0..8usize {
            if lights[k] != 0 && lights[k] != self.lit[k] {
                self.lit[k] = lights[k];
                let d = panel::panel_data(iso, exe)?;
                panel::face_light(&d, &self.table0, &mut self.faces[k * 1024..(k + 1) * 1024], lights[k]);
            }
        }
        for k in 0..8usize {
            let s = &w.soldiers[k * REC..(k + 1) * REC];
            if s[0x179] != self.profiles[k] {
                let d = panel::panel_data(iso, exe)?;
                let face = &mut self.faces[k * 1024..(k + 1) * 1024];
                clear_face(face);
                self.profiles[k] = s[0x179];
                self.alive[k] = s[0x182] != 0 && s[0x183] as i8 > 0;
                if s[0x182] == 0 {
                    continue;
                }
                panel::face_light(&d, &self.table0, face, self.lit[k]);
                panel::load_face(iso, &d, s[0x179] as i32, if self.alive[k] { 0 } else { 1 }, face)?;
                continue;
            }
            if self.alive[k] && s[0x183] as i8 <= 0 {
                let d = panel::panel_data(iso, exe)?;
                panel::load_face(iso, &d, s[0x179] as i32, 2, &mut self.faces[k * 1024..(k + 1) * 1024])?;
                self.alive[k] = false;
            }
        }
        Ok(())
    }

    /// The stat popup of slot k on the screen as drawn so far (`frame`, 320 x 200).
    /// The traverse popup over `frame` at corner (x0, y0), word `lit` lit (soldier_update.md "Traverse to a
    /// neighbour sector").
    pub fn trav_popup(&self, iso: &str, exe: &str, x0: i32, y0: i32, lit: Option<usize>, frame: &mut [u8]) -> Result<(), i32> {
        let c = common(iso)?;
        let d = panel::panel_data(iso, exe)?;
        let mut table0 = self.table0;
        let mut p = Painter { d: &d, buf: frame, table0: &mut table0, table29: panel::table29(&c) };
        crate::strips::trav_popup(&mut p, x0, y0, lit);
        Ok(())
    }

    /// panel.md "The switch view" over a drawn frame: slot k's open / close frame (`anim` = element 1..3) or
    /// switch i's pressed face (`pressed` = (i, on)).
    pub fn switch_part(&mut self, iso: &str, exe: &str, w: &World, k: u8, anim: Option<usize>,
                       pressed: Option<(usize, bool)>, frame: &mut [u8]) -> Result<(), i32> {
        let c = common(iso)?;
        let d = panel::panel_data(iso, exe)?;
        let mut table0 = self.table0;
        let mut p = Painter { d: &d, buf: frame, table0: &mut table0, table29: panel::table29(&c) };
        if let Some(e) = anim {
            panel::switch_frame(&mut p, k, w.selected, e);
        }
        if let Some((i, on)) = pressed {
            panel::switch_face(&mut p, k, w.selected, i, on, true);
        }
        Ok(())
    }

    pub fn popup(&mut self, iso: &str, exe: &str, w: &World, k: u8, frame: &mut [u8]) -> Result<(), i32> {
        let c = common(iso)?;
        let d = panel::panel_data(iso, exe)?;
        // The panel buffer is the screen as drawn so far; the popup goes into a copy, the block is copied back.
        let mut buf = frame.to_vec();
        let mut table0 = self.table0;
        let mut p = Painter { d: &d, buf: &mut buf, table0: &mut table0, table29: panel::table29(&c) };
        panel::draw_popup(&mut p, &w.soldiers, k as usize);
        let (sx, sy) = slot_origin(k as usize);
        panel::popup_to_screen(&buf, frame, k as usize, sx, sy);
        Ok(())
    }

    /// The whole screen (panel.md "The front-end's tactical screen"): the 8 slots ("Drawing one slot"), then the map
    /// view (view_w x view_h) at (48, 14), then the interface strips (top 1, keep 0). `blink` = the front-end's
    /// blink byte: the strips' new value stays only when `blink_step` (the loop's blink step came since the last frame).
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, iso: &str, exe: &str, w: &mut World, tileset: u8, view: &[u8], view_w: i32, view_h: i32,
                blink: &mut u8, blink_step: bool) -> Result<Vec<u8>, i32> {
        let c = common(iso)?;
        let d = panel::panel_data(iso, exe)?;
        let t = tileset_data(iso, tileset)?;
        let mut buf = vec![0u8; (PW * PH) as usize];
        // cursor.md "Light": the portrait under the mouse (or of the merc under the mouse on the map) is lit.
        let lit = crate::enter::lit_slot();
        let mut ap_warned = [0u8; 8];
        for (k, a) in ap_warned.iter_mut().enumerate() {
            *a = w.ap_warned.get(k).copied().unwrap_or(0);
        }
        {
            let mut p = Painter { d: &d, buf: &mut buf, table0: &mut self.table0, table29: panel::table29(&c) };
            for k in 0..8u8 {
                let env = SlotEnv {
                    slot: k,
                    selected: w.selected,
                    combat: w.combat,
                    opt_onscreen: w.opt_onscreen,
                    speaker: lit,
                    hover: self.hover,
                    ap_warned,
                    faces: &self.faces,
                    tileset: &t,
                };
                panel::draw_slot(&mut p, &env, &mut w.soldiers);
            }
        }
        // panel.md "View modes": the view goes into the mode's window; in mode 3 (no strips) after the strips.
        let (_, _, left, top) = crate::viewmode::world_window(w);
        let put_view = |buf: &mut Vec<u8>| {
            for y in 0..view_h.min(PH - top) {
                let n = view_w.min(PW - left) as usize;
                let src = &view[(y * view_w) as usize..(y * view_w) as usize + n];
                let at = ((top + y) * PW + left) as usize;
                buf[at..at + n].copy_from_slice(src);
            }
        };
        if w.opt_onscreen != 3 {
            put_view(&mut buf);
        }
        let id = crate::invdraw::inv_data(iso, exe)?;
        let cur = w.cur_sector as usize;
        // SPEC GAP: a current sector past the 60 sector_state records reads mercs = enemies = 0.
        let byte = |i: usize| w.sector_state.get(cur * 9 + i).copied().filter(|_| cur < 60).unwrap_or(0);
        let (mercs, enemies) = (byte(2), byte(1));
        let sel_in_play = sel_in_play(w, &id.inv);
        let env = crate::strips::StripsEnv {
            top: 1,
            keep: 0,
            opt: w.opt_onscreen,
            combat: w.combat,
            sector: w.cur_sector,
            mercs,
            enemies,
            strip_flag: w.strip_flag,
            selected: w.selected,
            // The port keeps no turn slot: 64 = none.
            turn_slot: 64,
            sel_in_play,
            small_panel: w.small_panel,
            time_w: w.clock.bar,
            speaker: lit,
            hover: self.hover,
            ap_warned,
            faces: &self.faces,
            tileset: &t,
            switch_as_normal: false,
            shade_last: crate::strips::frame_shade(w.combat, w.update.view_hold),
        };
        let mut scr = buf.clone();
        let mut b = *blink;
        {
            let mut p = Painter { d: &d, buf: &mut buf, table0: &mut self.table0, table29: panel::table29(&c) };
            let r = crate::strips::draw_strips(&mut p, &id, &env, &mut b, &mut self.combat_seen, self.mouse_x, &mut w.soldiers, &mut scr);
            self.mouse_x = r.mouse_x;
            if let Some(i) = self.pressed {
                crate::strips::button_face(&mut p, i, true);
            }
            // SlotMouse 0x118F3 draws "cont" on the screen, over the strips' slot redraw.
            if w.opt_onscreen < 2 {
                panel::draw_cont(&mut p, self.cont, &w.soldiers);
            }
        }
        if blink_step {
            *blink = b;
        }
        if w.opt_onscreen == 3 {
            put_view(&mut buf);
            // panel.md "The small panel in the full view" step 4: under the 196-pixel map the rows 196..199 are
            // black (the original clears them once when the full view shows; its screen keeps them).
            if view_h == 196 {
                buf[(196 * PW) as usize..(200 * PW) as usize].fill(0);
            }
            // panel.md "The small panel in the full view" step 4: the redraw after the full view showed.
            if w.full_shown != 0 {
                w.strip_flag = 0;
            }
            w.full_shown = 0;
        }
        // The picture shown = the panel buffer (it also holds the time meter and the sector box, which the
        // original's screen gets with the next copy).
        Ok(buf)
    }
}

/// The fades of a sector change (SquadLost? 0x3A374, TraverseMove 0x3164E, QuickRestore 0x2D7AB): the step count (56,
/// QuickRestore 24), the fade out before the load (a move that is not a Quick Restore) and the fade out after the
/// read (only a Quick Restore: it already uses the palette of the restored sector).
pub fn sector_fade(moved: bool, quick_restored: bool) -> (i32, bool, bool) {
    let steps = if quick_restored { 24 } else { 56 };
    (steps, moved && !quick_restored, moved && quick_restored)
}

/// The bar area under the mouse for `Screen::hover` from SlotMouse 0x93448: slots 0..7, else 0xFF (none).
pub fn bar_hover(slot_hover: u8) -> u8 {
    if slot_hover < 8 { slot_hover } else { 0xFF }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ledger row ja1front:19246 (panel.md "The front-end's tactical screen"): only merc slots 0..7 start the stat popup.
    #[test]
    fn only_merc_slots_start_the_popup() {
        let mut s = vec![0u8; 48 * REC];
        for k in [7usize, 8] {
            s[k * REC + 0x138] = 1;
            s[k * REC + 0x183] = 20;
        }
        assert_eq!(popup_press(&s, 7, Spot::Bars(7)), Some(7));
        assert_eq!(popup_press(&s, 8, Spot::Bars(8)), None);
    }

    /// Ledger row d057aca:ja1front:20331 (panel.md "The switch view"): each slot's right button reads that merc's
    /// own record; merc 5 on an assignment does not open it, the others do.
    #[test]
    fn the_switch_view_reads_the_record_of_its_slot() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.update.screen_drawn = 1;
        w.opt_onscreen = 0;
        for k in 0..8 {
            let r = &mut w.soldiers[k * REC..(k + 1) * REC];
            (r[0x182], r[0x194], r[0x138], r[0x17E], r[0x183]) = (1, 0, 1, (k == 5) as u8, 50);
        }
        let open: Vec<bool> = (0..9u8).map(|k| switch_can_open(&w, k)).collect();
        assert_eq!(open, [true, true, true, true, true, false, true, true, false]);
    }

    /// Ledger row c2aea50:ja1front:1419: a sector move fades out once, before the load (56 steps); a Quick Restore
    /// fades out once, after the read (24 steps).
    #[test]
    fn a_sector_change_fades_out_once() {
        assert_eq!(sector_fade(true, false), (56, true, false));
        assert_eq!(sector_fade(true, true), (24, false, true));
        assert_eq!(sector_fade(false, false), (56, false, false));
    }

    /// Ledger row c2aea50:ja1front:1266: the bar area of slot 0..7 under the mouse lights; no slot (0x93448 >= 8) is none.
    #[test]
    fn a_bar_hover_names_slots_0_to_7() {
        assert_eq!(bar_hover(0), 0);
        assert_eq!(bar_hover(3), 3);
        assert_eq!(bar_hover(7), 7);
        assert_eq!(bar_hover(8), 0xFF);
        assert_eq!(bar_hover(0x40), 0xFF);
    }

    /// LoadStripPccs? 0x2F928 step 3: the item boxes show by the in-play byte of the selected record, not of
    /// record 0 (strips_diff feeds this byte from g_soldiers[selected] + 0x138).
    #[test]
    fn the_strip_reads_the_selected_record() {
        let (exe, jag, save) = crate::world::tests::files(&[]);
        let mut w = World::from_files(&exe, &jag, &save).unwrap();
        w.soldiers[0x138] = 0;
        w.soldiers[3 * REC + 0x138] = 7;
        w.selected = 3;
        assert_eq!(sel_in_play(&w, &[]), 7);
        w.selected = 0;
        assert_eq!(sel_in_play(&w, &[]), 0);
    }

    #[test]
    fn spots() {
        let mut s = vec![0u8; 48 * REC];
        for k in 4..8 {
            s[k * REC + 0x180] = 1;
        }
        assert_eq!(spot(&s, 3, 18, 224, 168, 1), Spot::Bars(0));
        assert_eq!(spot(&s, 13, 44, 224, 168, 1), Spot::Bars(0));
        assert_eq!(spot(&s, 14, 44, 224, 168, 1), Spot::Slot(0));
        assert_eq!(spot(&s, 30, 20, 224, 168, 1), Spot::Slot(0));
        assert_eq!(spot(&s, 1, 140, 224, 168, 1), Spot::Slot(3));
        assert_eq!(spot(&s, 0, 140, 224, 168, 1), Spot::None);
        assert_eq!(spot(&s, 319, 60, 224, 168, 1), Spot::None);
        assert_eq!(spot(&s, 47, 181, 224, 168, 1), Spot::Slot(3));
        assert_eq!(spot(&s, 307, 60, 224, 168, 1), Spot::Bars(5));
        assert_eq!(spot(&s, 300, 60, 224, 168, 1), Spot::Slot(5));
        assert_eq!(spot(&s, 48, 14, 224, 168, 1), Spot::View(0, 0));
        assert_eq!(spot(&s, 271, 181, 224, 168, 1), Spot::View(223, 167));
        assert_eq!(spot(&s, 100, 5, 224, 168, 1), Spot::None);
        assert_eq!(spot(&s, 10, 190, 224, 168, 1), Spot::None);
        assert_eq!(spot(&s, 3, 18, 320, 168, 2), Spot::View(3, 4));
        assert_eq!(spot(&s, 3, 18, 320, 196, 3), Spot::View(3, 18));
    }

    /// LoadMercFace 0x286C4 at the sector entry takes kind 1 for hp <= 0; a slot that gets a dead merc's profile holds
    /// no alive face, so the next call plays no death.
    #[test]
    fn profile_change_to_a_dead_merc() {
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
        let (iso, exe) = (format!("{}/rom/JA1.gog", root), format!("{}/build/game113b/JA.EXE", root));
        if !std::path::Path::new(&exe).exists() {
            eprintln!("rom/ or build/game113b missing: skipped");
            return;
        }
        let mut w = World::new_game(&iso, &exe, 2, 1).unwrap();
        w.soldiers[0x182] = 1;
        w.soldiers[0x183] = 20;
        w.soldiers[0x179] = 5;
        let mut sc = Screen::new(&iso, &exe, &mut w).unwrap();
        assert!(sc.alive[0]);
        w.soldiers[0x179] = 6;
        w.soldiers[0x183] = 0;
        sc.update_faces(&iso, &exe, &w).unwrap();
        assert!(!sc.alive[0]);
        let mut want = sc.faces[..1024].to_vec();
        panel::load_face(&iso, &panel::panel_data(&iso, &exe).unwrap(), 6, 1, &mut want).unwrap();
        assert_eq!(sc.faces[..1024], want[..]);
    }

    #[test]
    fn popup_press_rule() {
        let mut s = vec![0u8; 48 * REC];
        s[0x138] = 1;
        s[0x183] = 20;
        assert_eq!(popup_press(&s, 0, Spot::Bars(0)), Some(0));
        assert_eq!(popup_press(&s, 0xFF, Spot::Bars(0)), None); // not the hover slot
        assert_eq!(popup_press(&s, 0, Spot::Slot(0)), None);
        s[0x183] = 0;
        assert_eq!(popup_press(&s, 0, Spot::Bars(0)), None); // hp 0
        s[0x183] = 20;
        s[0x138] = 0;
        assert_eq!(popup_press(&s, 0, Spot::Bars(0)), None); // not on the map
    }
}
