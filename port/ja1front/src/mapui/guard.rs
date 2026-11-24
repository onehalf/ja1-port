//! The guard screen in the window (strategic.md "The guard screen"): tactical key M and the top strip's map button
//! open it. A child module of `mapui`: it uses the map view's picture, frames, message boxes and host. Close runs
//! through play's Session (`Play::guard_close`).
use super::{cb_call, cb_random, MapHost, MapUi};
use crate::host::{Ja1Host, Ja1World};
use crate::island;
use crate::mappanel;
use crate::panel::{self, Painter};
use crate::world::World;
use std::ffi::c_void;

/// The screen's own state (`Ja1GuardScreen`, 580 bytes).
#[repr(C)]
#[derive(Clone)]
pub struct GuardScreen {
    pub moves: [u8; 480],
    pub moves_n: [i8; 60],
    pub pick: i32,
    pub pick_n: i32,
    pub travel: i32,
    pub pick_no: i32,
    pub here: i32,
    pub moving: i32,
    pub all: i32,
    pub hover: i32,
    pub last: i32,
    pub changed: u8,
}

extern "C" {
    fn ja1_guard_open(w: *mut Ja1World, h: *const Ja1Host, g: *mut GuardScreen) -> i32;
    fn ja1_guard_hover(w: *mut Ja1World, h: *const Ja1Host, g: *mut GuardScreen, sector: i32) -> i32;
    fn ja1_guard_click(w: *mut Ja1World, h: *const Ja1Host, g: *mut GuardScreen, sector: i32, buttons: i32,
                       ctrl: i32) -> i32;
    fn ja1_guard_exit(w: *mut Ja1World, h: *const Ja1Host, g: *mut GuardScreen) -> i32;
}

/// The start button (button 7 of the button table).
const START: (i32, i32, i32, i32) = (281, 176, 306, 191);
/// The palette ranges that rotate every 500 ms.
pub(crate) const ROTATE: [(usize, usize); 2] = [(0xD7, 0xDA), (0xDB, 0xDE)];

/// The guard screen while it is open: the library state and what the front-end drew on the picture.
pub struct Guard {
    pub g: Box<GuardScreen>,
    /// The travel text (or the "   " that clears it) at (219, 185).
    travel: Option<String>,
    /// Number field 15 was cleared ("  " at (290, 165)).
    clear15: bool,
    /// The picked numbers drawn since the last redraw of their cell: (sector, number), oldest first.
    picks: Vec<(i32, i32)>,
    /// The cells that GuardCellRedraw? 0x42500 repainted and the frames, in order.
    redraws: Vec<GuardPaint>,
    /// The game clock (dostime, BIOS tick steps) at the last rotation. None = no rotation yet: the original does not
    /// start the timer 0x22A3FC when the guard screen opens, so its first loop pass rotates. Live (tac60, key M): the
    /// timer keeps its old value up to the first rotation, 0.6 s after the key, then a rotation each 0.54 s.
    rotated: Option<i64>,
    /// The counts of the first picture: the old counts of the last screen (InitStrategicMap runs before the clear at 0x4283B).
    shown: [i8; 60],
    /// The sector table at the opening: the first picture never changes, `redraws` go over it.
    open_state: Vec<u8>,
    /// A click already drew the number fields: its calls of 0x36010 end at field 0x3A (the sector number of the last
    /// pick at (220, 174)), the opening's call ends at field 0x39.
    from_field: bool,
    /// The exit test gave 0: the screen closes at the end of this pass.
    pub closing: bool,
}

/// One paint on the guard screen's map, in the order of play.
enum GuardPaint {
    /// Sector, its record, its marks.
    Cell(usize, [u8; 9], i8),
    /// Sector, colour.
    Frame(i32, u8),
}

/// 0x424BC: the mark of a sector cell at (x0, y0) with the number n.
pub fn guard_cell(p: &mut Painter, n: i32, x0: i32, y0: i32) {
    p.element(0x75, x0, y0, None);
    p.text_mode(n.to_string().as_bytes(), x0 + 7, y0 + 8, false, 1, 0, 0, 300);
}

impl MapUi {
    fn guard_host(&mut self) -> Ja1Host {
        Ja1Host { ctx: &mut self.host as *mut MapHost as *mut c_void, random: cb_random, call: cb_call, host_ops: 0 }
    }

    /// Open: false when the screen does not open (`cur_sector` >= 90). The window moves the mouse to `warp`.
    pub fn guard_show(&mut self, w: &mut World) -> Result<bool, i32> {
        if !self.guard_open(w)? {
            return Ok(false);
        }
        self.guard_pass(w)?;
        Ok(true)
    }

    /// `guard_show` up to the first picture, before the first pass of the loop.
    pub fn guard_open(&mut self, w: &mut World) -> Result<bool, i32> {
        // GuardAssignScreen? 0x427B4 starts with its last sector 0x3C: the mouse counts as below the map.
        self.map_limit = crate::mouselimit::MapLimit::new();
        // SAFETY: GuardScreen is plain bytes and integers; all zero is a valid value (Open sets every field).
        let mut g: Box<GuardScreen> = Box::new(unsafe { std::mem::zeroed() });
        let mut cw = Ja1World::view(w);
        let h = self.guard_host();
        // SAFETY: the pointers of `cw` point into `w`, which does not move during the call.
        let r = unsafe { ja1_guard_open(&mut cw, &h, &mut *g) };
        cw.store(w);
        if r != 0 {
            return Ok(false);
        }
        self.open = true;
        self.cur = -1;
        self.msg = None;
        self.boxes.clear();
        self.box_up = None;
        self.frames.clear();
        self.mouse_num = None;
        self.anim.clear();
        self.anim_sfx.clear();
        self.anim_ptr.clear();
        let shown = island::guard_marks();
        island::set_guard_marks(g.moves_n);
        let (_, p6) = island::island_map(&self.iso, &self.exe, &w.sector_state[..540], &shown)?;
        self.pal = crate::eod::scale_palette(&p6);
        let cur = w.cur_sector as i32;
        let at = ((cur % 10) * 31 + 3, (cur / 10) * 23 + 3);
        self.warp = Some(at);
        self.mouse = at;
        self.guard = Some(Guard { g, travel: None, clear15: false, picks: Vec::new(), redraws: Vec::new(), rotated: None,
                                  closing: false, shown, open_state: w.sector_state[..540].to_vec(), from_field: false });
        self.guard_redraw(w)?;
        Ok(true)
    }

    /// The library state while the guard screen is open.
    pub fn guard_state(&self) -> Option<&GuardScreen> {
        self.guard.as_ref().map(|g| &*g.g)
    }

    /// The guard screen is open.
    pub fn guarding(&self) -> bool {
        self.guard.is_some()
    }

    /// The exit test passed: the screen state for Close (the window runs `Play::guard_close`, then the tactical
    /// screen shows again).
    pub fn guard_take_closing(&mut self) -> Option<Box<GuardScreen>> {
        if !self.guard.as_ref().is_some_and(|g| g.closing) {
            return None;
        }
        self.open = false;
        self.frames.clear();
        self.guard.take().map(|g| g.g)
    }

    /// The picture: the island map with marks = moves_n, the frames, GUARDPAN.PCC, the panel numbers, field 15,
    /// the travel text, the picked numbers and a message box on top.
    pub(super) fn guard_redraw(&mut self, w: &World) -> Result<(), i32> {
        let Some(gd) = self.guard.as_ref() else { return Ok(()) };
        let (img, _) = island::island_map(&self.iso, &self.exe, &gd.open_state, &gd.shown)?;
        self.img = img;
        // GuardCellRedraw? 0x42500 repaints one cell in the old picture: a row of more than 8 marks stays over the next cell
        // until that cell's own redraw, and over a frame that came before it. So the cells and the frames go in their order.
        let m = island::map_data(&self.iso, &self.exe)?;
        let d = panel::panel_data(&self.iso, &self.exe)?;
        for ev in &gd.redraws {
            match ev {
                GuardPaint::Cell(c, r, mark) => island::sector(&m, &d.sheet, r, *mark, *c, &mut self.img),
                GuardPaint::Frame(sec, col) => island::sector_frame(&self.iso, &self.exe, *sec, *col, &mut self.img)?,
            }
        }
        let (c, d, inv) = (crate::common(&self.iso)?, panel::panel_data(&self.iso, &self.exe)?,
                           crate::invdraw::inv_data(&self.iso, &self.exe)?);
        let mut table0 = c.remap[0];
        let mut p = Painter { d: &d, buf: &mut self.img, table0: &mut table0, table29: panel::table29(&c) };
        crate::invdraw::load_pcx(&mut p, &inv, "GUARDPAN.PCC", 0, 146);
        let g = &gd.g;
        for (v, x, y) in [(g.here, 112, 163), (g.moving, 112, 174), (g.all, 112, 185), (g.pick_n, 220, 163)] {
            p.text_mode(&padded(v, 3), x, y, false, 50, 12, 0, 13);
        }
        if gd.from_field {
            p.text_mode(&padded(g.pick_no, 3), 220, 174, false, 50, 12, 0, 13);
        }
        if let Some(n) = self.mouse_num {
            p.text_mode(&padded(n, 2), 290, 165, false, 29, 50, 0, 13);
        } else if gd.clear15 {
            p.text_mode(b"  ", 290, 165, false, 29, 50, 0, 13);
        }
        if let Some(t) = &gd.travel {
            p.text_mode(t.as_bytes(), 219, 185, false, 29, 50, 0, 13);
        }
        for &(s, n) in &gd.picks {
            guard_cell(&mut p, n, (s % 10) * 31 + 20, (s / 10) * 23 + 10);
        }
        if self.start_down {
            mappanel::back_button_at(&mut p, (START.0, START.1), true);
        }
        self.draw_box()
    }

    fn guard_frame(&mut self, s: i32, colour: u8) {
        self.frames.retain(|f| f.0 != s);
        self.frames.push((s, colour));
        if let Some(gd) = self.guard.as_mut() {
            // An older frame of s with no cell after it is under this one: the list stays short.
            let from = gd.redraws.iter().rposition(|e| matches!(e, GuardPaint::Cell(..))).map_or(0, |i| i + 1);
            let mut i = 0;
            gd.redraws.retain(|e| {
                i += 1;
                i <= from || !matches!(e, GuardPaint::Frame(f, _) if *f == s)
            });
            gd.redraws.push(GuardPaint::Frame(s, colour));
        }
    }

    /// One Hover (s) with the front-end's drawing; true when the picture changed.
    fn guard_hover(&mut self, w: &mut World, s: i32) -> Result<bool, i32> {
        super::mouse_sector(w, s);
        let mut cw = Ja1World::view(w);
        let h = self.guard_host();
        let Some(gd) = self.guard.as_mut() else { return Ok(false) };
        let (old_hover, old_last) = (gd.g.hover, gd.g.last);
        // SAFETY: as in `guard_show`.
        let r = unsafe { ja1_guard_hover(&mut cw, &h, &mut *gd.g, s) };
        cw.store(w);
        let mut changed = false;
        if s >= 60 {
            if old_hover < 60 {
                gd.clear15 = true;
                self.mouse_num = None;
                if (0..60).contains(&old_last) {
                    self.guard_frame(old_last, 0x6E);
                }
                changed = true;
            }
            return Ok(changed);
        }
        if s != old_last {
            if (0..60).contains(&old_last) {
                self.guard_frame(old_last, 0x6E);
            }
            let c = if w.sector_state[s as usize * 9] & 1 != 0 { 0xFF } else { 0x6B };
            self.guard_frame(s, c);
            let gd = self.guard.as_mut().unwrap();
            gd.travel = Some("   ".into());
            // The old cell comes back from the back buffer (0x429FC..): its picked number goes.
            gd.picks.clear();
            if gd.g.pick_n != 0 {
                gd.picks.push((s, gd.g.pick_n));
            }
            changed = true;
        }
        let gd = self.guard.as_mut().unwrap();
        if r == 1 {
            let t = gd.g.travel;
            gd.travel = Some(format!("{}.{}", t / 60, (t % 60) * 100 / 600));
            changed = true;
        }
        if self.mouse_num != Some(s + 1) {
            self.mouse_num = Some(s + 1);
            gd.clear15 = false;
            changed = true;
        }
        Ok(changed)
    }

    /// The sector under the mouse for a pass: 0..59 in the map part (y < 141), else 60.
    fn guard_sector(&self) -> i32 {
        let (x, y) = self.mouse;
        if y < 141 {
            island::sector_at(x, y).clamp(0, 59)
        } else {
            60
        }
    }

    /// One pass with no button pressed (the window runs it every loop round while the screen shows): Hover and
    /// the palette rotation.
    pub fn guard_pass(&mut self, w: &mut World) -> Result<(), i32> {
        let Some(gd) = self.guard.as_ref() else { return Ok(()) };
        let (before, ms) = (gd.rotated, gd.rotated.map_or(i64::MAX, |t| crate::dostime::now_ms() - t));
        self.guard_pass_at(w, ms)?;
        if before.is_some() && self.guard.as_ref().is_some_and(|g| g.rotated != before) {
            eprintln!("guard screen water rotates after {} ms", ms);
        }
        Ok(())
    }

    /// `guard_pass` when `ms` passed on the screen timer: the colours rotate above 500 ms (cmp eax, 0x1F4 / jle at
    /// 0x42F39).
    pub fn guard_pass_at(&mut self, w: &mut World, ms: i64) -> Result<(), i32> {
        if self.guard.is_none() || self.box_up.is_some() {
            return Ok(());
        }
        let s = self.guard_sector();
        let mut changed = self.guard_hover(w, s)?;
        let gd = self.guard.as_mut().unwrap();
        if ms > 500 {
            gd.rotated = Some(crate::dostime::now_ms());
            for (a, b) in ROTATE {
                self.pal[a * 3..b * 3 + 3].rotate_left(3);
            }
            changed = true;
        }
        if changed {
            self.guard_redraw(w)?;
        }
        Ok(())
    }

    /// The exit test (Enter or the start button).
    fn guard_exit(&mut self, w: &mut World) -> Result<(), i32> {
        let mut cw = Ja1World::view(w);
        let h = self.guard_host();
        let Some(gd) = self.guard.as_mut() else { return Ok(()) };
        // SAFETY: as in `guard_show`.
        let n = unsafe { ja1_guard_exit(&mut cw, &h, &mut *gd.g) };
        if n == 0 {
            gd.closing = true;
            return Ok(());
        }
        self.message_number(n)
    }

    /// A press of the left or right button (buttons bit 0 / bit 1) at the mouse; Ctrl from `keys`.
    pub fn guard_click(&mut self, w: &mut World, buttons: i32) -> Result<(), i32> {
        let s = self.guard_sector();
        self.guard_hover(w, s)?;
        if s >= 60 {
            let (x, y) = self.mouse;
            if buttons & 1 != 0 && (START.0..=START.2).contains(&x) && (START.1..=START.3).contains(&y) {
                // The button routine 0x67CD8(7, 1): press sound and the pressed face (PressPanelButton 0x35D44) until
                // the button goes up, the release sound, then the exit test (`guard_release`).
                self.start_down = true;
            }
            return self.guard_redraw(w);
        }
        let mut cw = Ja1World::view(w);
        let h = self.guard_host();
        let ctrl = self.keys.1 as i32;
        let gd = self.guard.as_mut().unwrap();
        let before = gd.g.clone();
        // SAFETY: as in `guard_show`.
        let n = unsafe { ja1_guard_click(&mut cw, &h, &mut *gd.g, s, buttons, ctrl) };
        cw.store(w);
        let g = &gd.g;
        let sent = |m: &[i8]| m.iter().map(|&v| v as i32).sum::<i32>();
        if sent(&g.moves_n) < sent(&before.moves_n) {
            eprintln!("guard screen undo at sector {}", s);
        }
        if g.moves_n != before.moves_n || g.pick_n != before.pick_n || g.here != before.here {
            // The cell of s again (its old picked numbers go), then the picked number on it. An undo also redraws
            // the cell of each guard that went home (strategic.md "Undo"): those cells lose their picked number too.
            let home = home_cells(&g.moves_n, &before.moves_n, &g.moves);
            picks_drop(&mut gd.picks, s, &home);
            // GuardCellRedraw? 0x42500 draws these cells again with the counts of now.
            let mut redrawn = Vec::new();
            for c in 0..60i32 {
                if c == s || home.contains(&c) || g.moves_n[c as usize] != before.moves_n[c as usize] {
                    redrawn.push(c as usize);
                }
            }
            // UndoGuardMove? 0x4257C repaints the home cells first, the click's cell comes last.
            redrawn.sort_by_key(|&c| c == s as usize);
            for c in redrawn {
                let mut r = [0u8; 9];
                r.copy_from_slice(&w.sector_state[c * 9..c * 9 + 9]);
                gd.redraws.push(GuardPaint::Cell(c, r, g.moves_n[c]));
            }
            island::set_guard_marks(g.moves_n);
            gd.from_field = true;
            if g.pick_n != 0 {
                gd.picks.push((s, g.pick_n));
            }
        }
        self.guard_redraw(w)?;
        self.message_number(n)
    }

    /// The left button goes up on the guard screen: a held start button runs the exit test.
    pub(super) fn guard_release(&mut self, w: &mut World) -> Result<(), i32> {
        if self.start_down {
            self.start_down = false;
            self.guard_exit(w)?;
        }
        self.guard_redraw(w)
    }

    /// A key: Enter (13) is the exit test; T and L as on the island map.
    pub fn guard_key(&mut self, w: &mut World, c: u8) -> Result<super::Key, i32> {
        if c == 13 {
            self.guard_exit(w)?;
            return Ok(super::Key::Changed);
        }
        // MapKeyHandler? 0x426D0 runs on this screen too (0x42924, no Alt-X: DL 0). A shown box holds the keys.
        if self.box_shown() {
            return Ok(super::Key::Nothing);
        }
        match c {
            b'T' => self.subtitle_key(w),
            b'L' => self.say_again(w),
            _ => Ok(super::Key::Nothing),
        }
    }
}

/// `v` with '^' (a blank digit) in front up to `digits` places.
fn padded(v: i32, digits: usize) -> Vec<u8> {
    let mut s = v.to_string().into_bytes();
    while s.len() < digits {
        s.insert(0, b'^');
    }
    s
}

/// GuardCellRedraw? 0x42500 paints the cell of the click (s) and, after an undo, the cell of each guard that went
/// home again: the picked numbers that showed on those cells are gone. The other cells keep theirs.
/// UndoGuardMove? 0x4257C: an undo lowers the move count of a cell (0x229D8C + c) and sends its guard home to
/// entry moves[8 c + count] (0x10D680), whose cell GuardCellRedraw? 0x42500 paints again; cells whose count did not
/// drop send nobody home.
fn home_cells(now_n: &[i8], before_n: &[i8], moves: &[u8]) -> Vec<i32> {
    (0..now_n.len().min(before_n.len()))
        .filter(|&c| now_n[c] < before_n[c] && now_n[c] >= 0)
        .map(|c| moves[8 * c + now_n[c] as usize] as i32)
        .collect()
}

fn picks_drop(picks: &mut Vec<(i32, i32)>, s: i32, home: &[i32]) {
    picks.retain(|p| p.0 != s && !home.contains(&p.0));
}

#[cfg(test)]
mod picks_tests {
    /// strategic.md "Undo": the click's cell and the home cells are painted again (GuardCellRedraw? 0x42500), so
    /// their picked numbers go; a cell that is not painted again keeps its number.
    /// Ledger row ja1front:22688 (Insert #12730): only a cell whose move count dropped sends a guard home
    /// (UndoGuardMove? 0x4257C: dec byte 0x229D8C + s, home = byte 0x10D680 + 8 s + count); a cell whose count
    /// stayed, with moves or none, sends nobody home.
    #[test]
    fn only_an_undone_cell_sends_a_guard_home() {
        let mut moves = vec![0u8; 480];
        let (mut now, mut before) = (vec![0i8; 60], vec![0i8; 60]);
        moves[8 * 5 + 1] = 12;
        moves[8 * 9 + 1] = 33;
        moves[8 * 9] = 34;
        (now[5], before[5]) = (1, 2);
        (now[9], before[9]) = (1, 1);
        assert_eq!(super::home_cells(&now, &before, &moves), [12]);
    }

    #[test]
    fn a_repainted_guard_cell_loses_its_picked_number() {
        let mut p = vec![(3, 1), (5, 2), (7, 1), (5, 1)];
        super::picks_drop(&mut p, 5, &[7]);
        assert_eq!(p, [(3, 1)]);
        let mut p = vec![(3, 1), (5, 2)];
        super::picks_drop(&mut p, 9, &[]);
        assert_eq!(p, [(3, 1), (5, 2)]);
    }
}

#[cfg(test)]
mod sector_tests {
    /// GuardAssignScreen? 0x427B4 takes the sector under the mouse only for y < 0x8D (141); the panel rows from
    /// 141 down are no sector (60).
    #[test]
    fn row_141_is_the_panel() {
        let mut m = super::MapUi::new("", "", 1);
        m.mouse = (100, 140);
        assert_eq!(m.guard_sector(), 53);
        m.mouse = (100, 141);
        assert_eq!(m.guard_sector(), 60);
    }
}
