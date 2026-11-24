//! The main menu, Set Difficulty and Set Quick-Save Mode (world_load.md section 12).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::data::Common;
use crate::panel::{self, Painter, PanelData};

const PW: usize = 320;
const PH: usize = 200;

type Pic = (usize, usize, Vec<u8>);

/// The screen the menu shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Main,
    Difficulty,
    QuickSave,
}

/// The end of the menu: a new game (difficulty d, quick-save mode q), Continue, or the program ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NewGame(u8, u8),
    Continue,
    Exit,
}

/// The menu's state across its three screens.
pub struct Menu {
    pd: Arc<PanelData>,
    c: Arc<Common>,
    main: Pic,
    rows: Pic,
    main_pal: [u8; 768],
    room_pal: [u8; 768],
    save_dir: Option<PathBuf>,
    pub screen: Screen,
    /// The screen picture and its palette (6-bit).
    pub buf: Vec<u8>,
    pub pal: [u8; 768],
    /// C: Continue allowed.
    pub cont: bool,
    /// L, V of the main menu loop (−1 / 99 = none).
    l: i32,
    v: i32,
    /// S, K of the choice screens.
    s: i32,
    k: u8,
    /// The difficulty chosen on Set Difficulty.
    d: u8,
    /// The last choice of the main menu (the row the menu opens with after a game).
    pub chosen: i32,
    /// A wait until the left button is up, then this choice of the main menu (or none).
    wait_up: Option<Option<i32>>,
    /// The mouse moved by a key: the window moves its pointer there.
    pub warp: Option<(i32, i32)>,
    /// Start New Game was chosen: the caller plays the new-game intro (ending.md "The new-game intro"), then shows
    /// Set Difficulty (already in `buf`).
    pub intro: bool,
    /// The stdout lines of the polls, in order.
    pub lines: Vec<String>,
    /// The fades to show, in order (world_load.md "Fades of the menu screens"): the picture, its palette (6-bit) and
    /// the fade (steps n, level from, level to; office::fade). The window takes them.
    pub fades: Vec<(Vec<u8>, [u8; 768], i32, i32, i32)>,
    /// menu_diff: the draw calls in order (None in the game).
    pub trace: Option<Vec<String>>,
}

/// C: a file GAMEn.SAV (n = 0..9) exists in the save directory.
pub fn any_game_save(dir: Option<&Path>) -> bool {
    dir.is_some_and(|d| (0..10).any(|n| d.join(format!("GAME{}.SAV", n)).is_file()))
}

fn member(arc: &[u8], name: &str) -> Result<Vec<u8>, i32> {
    panel::archive_member(arc, name).map_err(|_| crate::E_FILE)
}

fn palette(f: &[u8]) -> Result<[u8; 768], i32> {
    let tail = f.get(f.len().saturating_sub(768)..).filter(|t| t.len() == 768).ok_or(crate::E_FILE)?;
    let mut pal = [0u8; 768];
    for (i, v) in tail.iter().enumerate() {
        pal[i] = v >> 2;
    }
    Ok(pal)
}

const DIFF_ROWS: [&str; 3] = ["EASIER", "NORMAL", "HARDER"];
const DIFF_Y: [i32; 3] = [80, 100, 120];
const QS_ROWS: [&str; 2] = ["No Quick-Save in Combat", "Quick-Save Anytime!"];
const QS_Y: [i32; 2] = [90, 110];

impl Menu {
    /// Loads the pictures; the main menu opens with row C lit (L = 0, as at the program start).
    pub fn new(iso: &str, exe: &str, save_dir: Option<PathBuf>) -> Result<Menu, i32> {
        // music.md "Who plays what": the program start loads INTRO; the title part ends with a stop, so the main
        // menu is silent.
        crate::music::set_exe(exe);
        crate::music::with(|m| {
            m.load(crate::startup::START.music);
            if m.playing() > 0 {
                m.stop_theme();
            }
        });
        // sound.md "Main bank": the main menu loads it (its menu sound is effect 0, IN).
        crate::sound::screen_main(iso, exe);
        let (c, pd) = (crate::common(iso)?, panel::panel_data(iso, exe)?);
        let dir = Path::new(exe).parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let arc = std::fs::read(dir.join("DAT").join("OFFICE.DAT")).map_err(|_| crate::E_FILE)?;
        let mm = member(&arc, "MAINMENU.PCX")?;
        let st = member(&arc, "STARTEXT.PCC")?;
        let room = member(&arc, "ROOM.PCX")?;
        let pic = |f: &[u8]| panel::pcx(f).map_err(|_| crate::E_FILE);
        let cont = any_game_save(save_dir.as_deref());
        let mut m = Menu { pd, c, main: pic(&mm)?, rows: pic(&st)?, main_pal: palette(&mm)?, room_pal: palette(&room)?,
                           save_dir, screen: Screen::Main, buf: vec![0; PW * PH], pal: [0; 768], cont, l: 0, v: 99,
                           s: -1, k: 0, d: 0, chosen: 0, wait_up: None, warp: None, intro: false, trace: None, lines: vec![], fades: vec![] };
        m.open(cont as i32);
        Ok(m)
    }

    /// A fade of the screen as it is now (a line `fade N FROM TO`).
    fn tr(&mut self, f: impl FnOnce() -> String) {
        if let Some(t) = self.trace.as_mut() {
            t.push(f());
        }
    }

    /// The left button waits to come up (WaitLeftUp).
    pub fn waiting(&self) -> bool {
        self.wait_up.is_some()
    }

    fn fade(&mut self, n: i32, from: i32, to: i32) {
        self.tr(|| format!("fade {} {} {}", n, from, to));
        self.fades.push((self.buf.clone(), self.pal, n, from, to));
        self.lines.push(format!("fade {} {} {}", n, from, to));
    }

    /// Open the menu with row `r`: the screen cleared and the menu palette faded out, the main picture, rows plain,
    /// row r lit, the mouse to (240, 52 + 22r), the fade in.
    pub fn open(&mut self, r: i32) {
        self.screen = Screen::Main;
        self.buf.iter_mut().for_each(|b| *b = 0);
        self.pal = self.main_pal;
        self.fade(24, 256, 0);
        let (w, h, px) = &self.main;
        for y in 0..(*h).min(PH) {
            for x in 0..(*w).min(PW) {
                self.buf[y * PW + x] = px[y * w + x];
            }
        }
        self.pal = self.main_pal;
        for k in 0..3 {
            self.row(k, false);
        }
        self.row(r, true);
        self.warp = Some((240, 52 + 22 * r));
        self.tr_warp();
        self.wait_up = None;
        self.lines.push(format!("menu open {} {}", r, self.cont as u8));
        self.fade(24, 0, 256);
    }

    fn tr_warp(&mut self) {
        if let Some((x, y)) = self.warp {
            self.tr(|| format!("warp {} {}", x, y));
        }
    }

    /// The menu sound (sound.md "Menu sound"): main-bank effect 0 at 11025, volume 6, once.
    fn sound(&mut self) {
        crate::sound::screen(|s| s.sfx(0, 0x2B11, 6, 1));
        self.lines.push(format!("screen sound {}", crate::sound::main_name(0)));
        self.tr(|| "sound".into());
    }

    /// Draw row k (0..2) plain or lit: its 190 x 22 part of STARTEXT at (60, 43 + 22k).
    fn row(&mut self, k: i32, lit: bool) {
        self.tr(|| format!("row {} {}", k, lit as u8));
        if !(0..3).contains(&k) {
            return;
        }
        let (w, h, px) = &self.rows;
        let sy = 22 * k as usize + if lit { 66 } else { 0 };
        for r in 0..22 {
            for c in 0..190 {
                let v = if sy + r < *h && c < *w { px[(sy + r) * w + c] } else { 0 };
                self.buf[(43 + 22 * k as usize + r) * PW + 60 + c] = v;
            }
        }
    }

    fn text(&mut self, s: &str, y: i32, fg: u8) {
        self.tr(|| format!("text {} 159 {} 1 {} 1 2", s, y, fg));
        let mut table0 = self.c.remap[0];
        let mut p = Painter { d: &self.pd, buf: &mut self.buf, table0: &mut table0, table29: panel::table29(&self.c) };
        p.text(s.as_bytes(), 159, y, true, fg, 1, 2);
    }

    fn rows_of(&self) -> (&'static [&'static str], &'static [i32]) {
        if self.screen == Screen::Difficulty { (&DIFF_ROWS, &DIFF_Y) } else { (&QS_ROWS, &QS_Y) }
    }

    /// Set Difficulty or Set Quick-Save Mode: the black screen with the title and the plain rows; S = −1.
    fn open_choice(&mut self, screen: Screen) {
        self.screen = screen;
        self.buf.iter_mut().for_each(|b| *b = 0);
        self.pal = self.room_pal;
        let (title, ty) = if screen == Screen::Difficulty { ("Set Difficulty Level:", 60) } else { ("Set Quick-Save Mode:", 65) };
        self.text(title, ty, 0x28);
        let (rows, ys) = self.rows_of();
        for (t, &y) in rows.iter().zip(ys) {
            self.text(t, y, 0x20);
        }
        self.s = -1;
        self.k = 0;
        self.wait_up = None;
        self.fade(24, 0, 256);
    }

    /// A game ended with Leave: C = 1 and the menu opens with the row of the last choice.
    pub fn reopen(&mut self) {
        // music.md "Who plays what": the main menu starts no music; the office's theme goes on after Leave.
        self.cont = true;
        self.open(self.chosen);
    }

    /// One key from `keys`: an extended key (0, scan code) gives (true, s), else (false, byte).
    fn key(keys: &mut VecDeque<u8>) -> Option<(bool, u8)> {
        let c = keys.pop_front()?;
        if c == 0 {
            Some((true, keys.pop_front().unwrap_or(0)))
        } else {
            Some((false, c))
        }
    }

    /// One poll: the mouse at (x, y), the left button held or not, the waiting key bytes (one key is taken).
    pub fn poll(&mut self, x: i32, y: i32, left: bool, keys: &mut VecDeque<u8>) -> Option<Outcome> {
        self.warp = None;
        if let Some(choice) = self.wait_up {
            if left {
                return None;
            }
            self.wait_up = None;
            return choice.and_then(|k| self.choose(k));
        }
        match self.screen {
            Screen::Main => self.poll_main(x, y, left, keys),
            _ => self.poll_choice(x, y, left, keys),
        }
    }

    fn poll_main(&mut self, x: i32, y: i32, left: bool, keys: &mut VecDeque<u8>) -> Option<Outcome> {
        let v0 = self.v;
        self.v = 99;
        for k in 0..3 {
            if (60..=250).contains(&x) && (43 + 22 * k..=65 + 22 * k).contains(&y) && (k != 1 || self.cont) {
                if k != self.l {
                    if self.l != -1 {
                        self.sound();
                        self.row(self.l, false);
                    }
                    self.row(k, true);
                    self.l = k;
                }
                self.v = k;
                if left {
                    // WaitLeftUp at once; the key step of this poll still runs (0x50100). Its Enter changes nothing.
                    self.wait_up = Some(Some(k));
                }
                break;
            }
        }
        if self.v == 99 && v0 < 99 {
            self.row(self.l, false);
            self.l = -1;
        }
        let mut choice = None;
        match Self::key(keys) {
            Some((true, s)) if s == 0x48 || s == 0x50 => {
                if self.l >= 0 {
                    self.row(self.l, false);
                }
                if s == 0x48 {
                    self.l -= 1;
                    if self.l < 0 {
                        self.l = 2;
                    }
                    if self.l == 1 && !self.cont {
                        self.l = 0;
                    }
                } else {
                    self.l += 1;
                    if self.l > 2 {
                        self.l = 0;
                    }
                    if self.l == 1 && !self.cont {
                        self.l = 2;
                    }
                }
                self.sound();
                self.row(self.l, true);
                self.warp = Some((240, 52 + 22 * self.l));
                self.tr_warp();
            }
            Some((false, 13)) if self.v < 99 && self.wait_up.is_none() => choice = Some(self.l),
            _ => {}
        }
        choice.and_then(|k| self.choose(k))
    }

    /// The choice k of the main menu.
    fn choose(&mut self, k: i32) -> Option<Outcome> {
        self.lines.push(format!("menu choice {}", k));
        self.chosen = k;
        match k {
            0 => {
                // ending.md "The new-game intro": the music stops, then the intro (by the caller), then Set Difficulty.
                crate::music::with(|m| m.stop_theme());
                self.intro = true;
                self.open_choice(Screen::Difficulty);
                None
            }
            1 => {
                // music.md "Who plays what": Continue stops no music (the office's theme 1 goes on after Leave).
                self.fade(56, 256, 0);
                Some(Outcome::Continue)
            }
            _ => {
                self.lines.push("menu exit".into());
                self.fade(56, 256, 0);
                Some(Outcome::Exit)
            }
        }
    }

    fn poll_choice(&mut self, mut x: i32, mut y: i32, left: bool, keys: &mut VecDeque<u8>) -> Option<Outcome> {
        let n = if self.screen == Screen::Difficulty { 3 } else { 2 };
        let s0 = self.s;
        if let Some((ext, b)) = Self::key(keys) {
            self.k = b;
            if ext && (b == 0x48 || b == 0x50) {
                self.s = if b == 0x48 {
                    match self.s {
                        s if s > 0 => s - 1,
                        0 => n - 1,
                        _ => 0,
                    }
                } else {
                    match self.s {
                        -1 => n - 1,
                        s if s < n - 1 => s + 1,
                        _ => 0,
                    }
                };
                let ys = self.rows_of().1;
                (x, y) = (180, ys[self.s as usize] + 2);
                self.warp = Some((x, y));
                self.tr_warp();
            }
        }
        let ys = self.rows_of().1;
        self.s = (0..n).find(|&k| (125..=195).contains(&x) && (ys[k as usize]..=ys[k as usize] + 12).contains(&y)).unwrap_or(-1);
        if self.s != s0 {
            let (rows, ys) = self.rows_of();
            if s0 >= 0 {
                self.text(rows[s0 as usize], ys[s0 as usize], 0x20);
            }
            if self.s >= 0 {
                self.text(rows[self.s as usize], ys[self.s as usize], 0xFF);
            }
            self.sound();
        }
        if left || self.k == 13 {
            if self.s >= 0 {
                let v = self.s as u8;
                if self.screen == Screen::Difficulty {
                    self.d = v;
                    self.lines.push(format!("menu difficulty {}", v));
                    self.quickstart_rename();
                    self.fade(24, 256, 0);
                    self.open_choice(Screen::QuickSave);
                    return None;
                }
                self.lines.push(format!("menu quick-save {}", v));
                self.fade(24, 256, 0);
                crate::music::with(|m| m.stop_theme());
                return Some(Outcome::NewGame(self.d, v));
            }
            if left {
                self.wait_up = Some(None);
            }
        }
        None
    }

    /// No GAME0.SAV and a QICKSTRT.SAV in the save directory: QICKSTRT.SAV becomes GAME0.SAV.
    fn quickstart_rename(&self) {
        if let Some(d) = self.save_dir.as_deref() {
            let (g, q) = (d.join("GAME0.SAV"), d.join("QICKSTRT.SAV"));
            if !g.exists() && q.exists() {
                if let Err(e) = std::fs::rename(&q, &g) {
                    eprintln!("menu: QICKSTRT.SAV: {}", e);
                }
            }
        }
    }
}
